/*
 * Copyright 2019 The Closure Compiler Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/lint/CheckRequiresSorted.java.

//! Checks that Closure import statements (goog.require, goog.requireType, and goog.forwardDeclare)
//! are sorted and deduplicated, exposing the necessary information to produce a suggested fix.

use crate::{
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument,
    js_string::JsString,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
};
use indexmap::{IndexMap, IndexSet};
use std::{cmp::Ordering, fmt, sync::LazyLock};

// port: CheckRequiresSorted#REQUIRES_NOT_SORTED
pub static REQUIRES_NOT_SORTED: DiagnosticType = DiagnosticType::warning(
    "JSC_REQUIRES_NOT_SORTED",
    "goog.require() and goog.requireType() statements are not in recommended format. The correct order is:\n\n{0}\n",
);

/// Operation modes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Collect information to determine whether a fix is required, but do not report a warning.
    COLLECT_ONLY,
    /// Additionally report a warning.
    COLLECT_AND_REPORT,
}

/// Primitives that may be called in an import statement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ImportPrimitive {
    REQUIRE,
    REQUIRE_TYPE,
    FORWARD_DECLARE,
}

impl ImportPrimitive {
    const VALUES: [ImportPrimitive; 3] = [
        ImportPrimitive::REQUIRE,
        ImportPrimitive::REQUIRE_TYPE,
        ImportPrimitive::FORWARD_DECLARE,
    ];

    // port: CheckRequiresSorted.ImportPrimitive#WEAKEST
    const WEAKEST: ImportPrimitive = ImportPrimitive::FORWARD_DECLARE;

    // port: CheckRequiresSorted.ImportPrimitive#name
    fn name(self) -> &'static str {
        match self {
            Self::REQUIRE => "goog.require",
            Self::REQUIRE_TYPE => "goog.requireType",
            Self::FORWARD_DECLARE => "goog.forwardDeclare",
        }
    }

    /// Returns the primitive with the given name.
    // port: CheckRequiresSorted.ImportPrimitive#fromName
    fn from_name(name: &JsString) -> ImportPrimitive {
        for primitive in Self::VALUES {
            if name == primitive.name() {
                return primitive;
            }
        }
        panic!("Invalid primitive name {}", name.to_string_lossy());
    }

    /// Returns the stronger of two primitives.
    ///
    /// `goog.require` is stronger than `goog.requireType`, which is stronger than
    /// `goog.forwardDeclare`.
    // port: CheckRequiresSorted.ImportPrimitive#stronger
    fn stronger(p1: ImportPrimitive, p2: ImportPrimitive) -> ImportPrimitive {
        if (p1 as u8) < (p2 as u8) { p1 } else { p2 }
    }
}

impl fmt::Display for ImportPrimitive {
    // port: CheckRequiresSorted.ImportPrimitive#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One of the bindings of a destructuring pattern.
///
/// `exportedName` and `localName` are equal in the case where the binding does not explicitly
/// specify a local name.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct DestructuringBinding {
    exported_name: JsString,
    local_name: JsString,
    is_shorthand_property: bool,
}

impl DestructuringBinding {
    // port: CheckRequiresSorted.DestructuringBinding#DestructuringBinding
    // port: CheckRequiresSorted.DestructuringBinding#of
    fn of(exported_name: JsString, local_name: JsString, is_shorthand_property: bool) -> Self {
        check_argument!(!is_shorthand_property || exported_name == local_name);
        Self {
            exported_name,
            local_name,
            is_shorthand_property,
        }
    }

    /// Compares two bindings according to the style guide sort order.
    // port: CheckRequiresSorted.DestructuringBinding#compareTo
    fn compare_to(&self, other: &DestructuringBinding) -> Ordering {
        self.exported_name
            .cmp(&other.exported_name)
            .then_with(|| self.local_name.cmp(&other.local_name))
    }

    /// Returns true if the destructuring binding is not canonical.
    ///
    /// For example: `{Foo}` is canonical, `{Foo: Bar}` is canonical, `{Foo: Foo}` is not
    /// canonical.
    // port: CheckRequiresSorted.DestructuringBinding#isCanonical
    fn is_canonical(&self) -> bool {
        self.exported_name != self.local_name || self.is_shorthand_property
    }

    /// Canonicalizes the destructuring to a shorthand property when applicable.
    ///
    /// In practice, `{Foo: Foo}` gets simplified to `{Foo}`.
    // port: CheckRequiresSorted.DestructuringBinding#canonicalizeShorthandProperties
    fn canonicalize_shorthand_properties(&self) -> DestructuringBinding {
        if self.is_canonical() {
            self.clone()
        } else {
            DestructuringBinding::of(
                self.exported_name.clone(),
                self.local_name.clone(),
                /* isShorthandProperty= */ true,
            )
        }
    }
}

/// UTF-16 `StringBuilder` for the code text compared by `ImportStatement#compareTo`.
#[derive(Default)]
struct Utf16Builder(Vec<u16>);

impl Utf16Builder {
    fn append(&mut self, s: &str) -> &mut Self {
        self.0.extend(s.encode_utf16());
        self
    }

    fn append_js(&mut self, s: &JsString) -> &mut Self {
        self.0.extend_from_slice(s.as_units());
        self
    }

    fn to_js_string(&self) -> JsString {
        JsString::from_units(self.0.clone())
    }
}

/// An import statement, which may have been merged from several import statements for the same
/// namespace in the original code.
///
/// An import statement has exactly one of three shapes:
///
/// - Standalone: has no LHS, as in `goog.require('namespace')`.
/// - Aliasing: has an LHS with an alias, as in `const alias = goog.require('namespace')`.
/// - Destructuring: has an LHS with a destructuring pattern, as in `const {name: localName} =
///   goog.require('namespace')`.
#[derive(Clone, PartialEq, Eq, Debug)]
struct ImportStatement {
    nodes: Vec<NodeId>,
    primitive: ImportPrimitive,
    namespace: JsString,
    alias: Option<JsString>,
    destructures: Option<Vec<DestructuringBinding>>,
}

impl ImportStatement {
    /// Creates a new import statement.
    // port: CheckRequiresSorted.ImportStatement#ImportStatement
    // port: CheckRequiresSorted.ImportStatement#of
    fn of(
        nodes: Vec<NodeId>,
        primitive: ImportPrimitive,
        namespace: JsString,
        alias: Option<JsString>,
        destructures: Option<Vec<DestructuringBinding>>,
    ) -> Self {
        check_argument!(
            alias.is_none() || destructures.is_none(),
            "Import statement cannot be simultaneously aliasing and destructuring"
        );
        Self {
            nodes,
            primitive,
            namespace,
            alias,
            destructures,
        }
    }

    /// Returns whether the import is standalone.
    // port: CheckRequiresSorted.ImportStatement#isStandalone
    fn is_standalone(&self) -> bool {
        !self.is_aliasing() && !self.is_destructuring()
    }

    /// Returns whether the import is aliasing.
    // port: CheckRequiresSorted.ImportStatement#isAliasing
    fn is_aliasing(&self) -> bool {
        self.alias.is_some()
    }

    /// Returns whether the import is destructuring.
    // port: CheckRequiresSorted.ImportStatement#isDestructuring
    fn is_destructuring(&self) -> bool {
        self.destructures.is_some()
    }

    /// Returns an import statement identical to the current one, except for its primitive, which
    /// is upgraded to the given one if stronger.
    // port: CheckRequiresSorted.ImportStatement#upgrade
    fn upgrade(&self, other_primitive: ImportPrimitive) -> ImportStatement {
        if ImportPrimitive::stronger(self.primitive, other_primitive) != self.primitive {
            return ImportStatement {
                nodes: self.nodes.clone(),
                primitive: other_primitive,
                namespace: self.namespace.clone(),
                alias: self.alias.clone(),
                destructures: self.destructures.clone(),
            };
        }
        self.clone()
    }

    // port: CheckRequiresSorted.ImportStatement#formatWithoutDoc
    fn format_without_doc(&self) -> JsString {
        let mut sb = Utf16Builder::default();
        if !self.is_standalone() {
            sb.append("const ");
        }
        if let Some(alias) = &self.alias {
            sb.append_js(alias);
        }
        if let Some(destructures) = &self.destructures {
            sb.append("{");
            let mut first = true;
            for binding in destructures {
                let exported_name = &binding.exported_name;
                let local_name = &binding.local_name;
                if first {
                    first = false;
                } else {
                    sb.append(", ");
                }
                sb.append_js(exported_name);
                if exported_name != local_name {
                    sb.append(": ");
                    sb.append_js(local_name);
                }
            }
            sb.append("}");
        }
        if !self.is_standalone() {
            sb.append(" = ");
        }
        sb.append(self.primitive.name());
        sb.append("('");
        sb.append_js(&self.namespace);
        sb.append("');");
        sb.to_js_string()
    }

    /// Formats the import statement into code.
    // port: CheckRequiresSorted.ImportStatement#format
    fn format(&self, ast: &Ast) -> JsString {
        let mut sb = Utf16Builder::default();
        for &node in &self.nodes {
            let comment = node.get_non_jsdoc_comment_string(ast);
            if !comment.is_empty() {
                sb.append_js(&node.get_non_jsdoc_comment_string(ast))
                    .append("\n");
            }
            let js_doc = NodeUtil::get_best_jsdoc_info(ast, node);
            if let Some(js_doc) = js_doc {
                // StringBuilder#append(String) appends "null" for a null original comment.
                match js_doc.get_original_comment_string() {
                    Some(original) => sb.append_js(&original),
                    None => sb.append("null"),
                };
                sb.append("\n");
            }
        }
        sb.append_js(&self.format_without_doc());
        sb.to_js_string()
    }

    /// Compares two import statements according to the style guide sort order.
    // port: CheckRequiresSorted.ImportStatement#compareTo
    fn compare_to(&self, other: &ImportStatement) -> Ordering {
        self.format_without_doc().cmp(&other.format_without_doc())
    }
}

// port: CheckRequiresSorted#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
// port: CheckRequiresSorted#GOOG_REQUIRETYPE
static GOOG_REQUIRETYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));
// port: CheckRequiresSorted#GOOG_FORWARDDECLARE
static GOOG_FORWARDDECLARE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.forwardDeclare"));

pub struct CheckRequiresSorted {
    mode: Mode,

    // Maps each namespace into the existing import statements for that namespace.
    // Use an ArrayListMultimap so that values for a key are iterated in a deterministic order.
    // (Java iterates its keys in HashMap order; canonicalizeImports stable-sorts the result by
    // formatWithoutDoc, which contains the namespace, so the key order never reaches output.)
    imports_by_namespace: IndexMap<JsString, Vec<ImportStatement>>,

    // The import statements in the order they appear.
    original_imports: Vec<ImportStatement>,

    // The import statements in canonical order.
    first_node: Option<NodeId>,
    last_node: Option<NodeId>,
    finished: bool,
    needs_fix: bool,
    replacement: Option<String>,
}

impl CheckRequiresSorted {
    // port: CheckRequiresSorted#CheckRequiresSorted
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            imports_by_namespace: IndexMap::new(),
            original_imports: Vec::new(),
            first_node: None,
            last_node: None,
            finished: false,
            needs_fix: false,
            replacement: None,
        }
    }

    /// Returns the node for the first recognized import statement.
    // port: CheckRequiresSorted#getFirstNode
    pub fn get_first_node(&self) -> Option<NodeId> {
        self.first_node
    }

    /// Returns the node for the last recognized import statement.
    // port: CheckRequiresSorted#getLastNode
    pub fn get_last_node(&self) -> Option<NodeId> {
        self.last_node
    }

    /// Returns a textual replacement yielding a canonical version of the imports.
    // port: CheckRequiresSorted#getReplacement
    pub fn get_replacement(&self) -> Option<&str> {
        self.replacement.as_deref()
    }

    /// Returns whether the imports need to be fixed, i.e., whether they are *not* already
    /// canonical.
    // port: CheckRequiresSorted#needsFix
    pub fn needs_fix(&self) -> bool {
        self.needs_fix
    }

    // port: CheckRequiresSorted#isValidImportCall
    fn is_valid_import_call(ast: &Ast, n: NodeId) -> bool {
        n.is_call(ast)
            && n.has_two_children(ast)
            && (GOOG_REQUIRE.matches(ast, n.get_first_child(ast).unwrap())
                || GOOG_REQUIRETYPE.matches(ast, n.get_first_child(ast).unwrap())
                || GOOG_FORWARDDECLARE.matches(ast, n.get_first_child(ast).unwrap()))
            && n.get_second_child(ast).unwrap().is_string_lit(ast)
    }

    // port: CheckRequiresSorted#parseImport
    fn parse_import(ast: &Ast, call_node: NodeId) -> ImportStatement {
        let primitive = ImportPrimitive::from_name(
            &call_node
                .get_first_child(ast)
                .unwrap()
                .get_qualified_name(ast)
                .unwrap(),
        );
        let namespace = call_node.get_second_child(ast).unwrap().get_string(ast);
        let parent = call_node.get_parent(ast).unwrap();
        if parent.is_expr_result(ast) {
            // goog.require('a');
            return ImportStatement::of(
                vec![parent],
                primitive,
                namespace,
                /* alias= */ None,
                /* destructures= */ None,
            );
        }
        let grandparent = parent.get_parent(ast).unwrap();
        if parent.is_name(ast) {
            // const a = goog.require('a');
            let alias = parent.get_string(ast);
            return ImportStatement::of(
                vec![grandparent],
                primitive,
                namespace,
                Some(alias),
                /* destructures= */ None,
            );
        }
        // const {a: b, c} = goog.require('a');
        let mut destructures = Vec::new();
        let mut name = parent.get_first_first_child(ast);
        while let Some(n) = name {
            let exported_name = n.get_string(ast);
            let local_name = n.get_first_child(ast).unwrap().get_string(ast);
            // {a: a} and {a: b} both yield false
            // {a} yields true
            let is_shorthand_property = n.is_shorthand_property(ast);
            destructures.push(DestructuringBinding::of(
                exported_name,
                local_name,
                is_shorthand_property,
            ));
            name = n.get_next(ast);
        }
        ImportStatement::of(
            vec![grandparent],
            primitive,
            namespace,
            /* alias= */ None,
            Some(destructures),
        )
    }

    // port: CheckRequiresSorted#checkCanonical
    fn check_canonical(&mut self, t: &mut NodeTraversal<'_>) {
        let canonical_imports = Self::canonicalize_imports(&self.imports_by_namespace);
        if self.original_imports != canonical_imports {
            self.needs_fix = true;
            let replacement = canonical_imports
                .iter()
                .map(|i| i.format(t).to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n");
            if self.mode == Mode::COLLECT_AND_REPORT {
                t.report(
                    self.first_node.unwrap(),
                    &REQUIRES_NOT_SORTED,
                    &[&replacement],
                );
            }
            self.replacement = Some(replacement);
        }
    }

    /// Canonicalizes a list of import statements by deduplicating and merging imports for the
    /// same namespace, and sorting the result.
    // port: CheckRequiresSorted#canonicalizeImports
    fn canonicalize_imports(
        imports_by_namespace: &IndexMap<JsString, Vec<ImportStatement>>,
    ) -> Vec<ImportStatement> {
        let mut canonical_imports: Vec<ImportStatement> = Vec::new();
        for (namespace, all_imports) in imports_by_namespace {
            // Find the strongest primitive across all existing imports. Every emitted import for
            // this namespace will use this primitive. This makes the logic simpler and cannot
            // change runtime behavior, but may produce spurious changes when multiple aliasing
            // imports of differing strength exist (which are already in violation of the style
            // guide).
            let strongest_primitive = all_imports
                .iter()
                .map(|i| i.primitive)
                .fold(ImportPrimitive::WEAKEST, ImportPrimitive::stronger);

            // Emit each aliasing import separately, as deduplicating them would require code
            // references to be rewritten.
            let mut has_aliasing = false;
            for stmt in all_imports.iter().filter(|i| i.is_aliasing()) {
                canonical_imports.push(stmt.upgrade(strongest_primitive));
                has_aliasing = true;
            }

            // Emit a single destructuring import with a non-empty pattern, merged from the
            // existing destructuring imports.
            let mut has_destructuring = false;
            let destructuring_nodes: Vec<NodeId> = all_imports
                .iter()
                .filter(|i| i.is_destructuring())
                .flat_map(|i| i.nodes.iter().copied())
                .collect();
            let mut sorted: Vec<DestructuringBinding> = all_imports
                .iter()
                .filter(|i| i.is_destructuring())
                .flat_map(|i| i.destructures.as_ref().unwrap().iter())
                .map(DestructuringBinding::canonicalize_shorthand_properties)
                .collect();
            // Stream#sorted is stable; Stream#distinct keeps the first of equal elements.
            sorted.sort_by(DestructuringBinding::compare_to);
            let destructures: Vec<DestructuringBinding> = sorted
                .into_iter()
                .collect::<IndexSet<_>>()
                .into_iter()
                .collect();
            if !destructures.is_empty() {
                canonical_imports.push(ImportStatement::of(
                    destructuring_nodes,
                    strongest_primitive,
                    namespace.clone(),
                    /* alias= */ None,
                    Some(destructures),
                ));
                has_destructuring = true;
            }

            // Emit a standalone import unless an aliasing or destructuring one already exists.
            if !has_aliasing && !has_destructuring {
                let standalone_nodes: Vec<NodeId> = all_imports
                    .iter()
                    .filter(|i| i.is_standalone())
                    .flat_map(|i| i.nodes.iter().copied())
                    .collect();
                canonical_imports.push(ImportStatement::of(
                    standalone_nodes,
                    strongest_primitive,
                    namespace.clone(),
                    /* alias= */ None,
                    /* destructures= */ None,
                ));
            }
        }

        // Sorting by natural order yields the correct result due to the implementation of
        // ImportStatement#compareTo.
        canonical_imports.sort_by(ImportStatement::compare_to);
        canonical_imports
    }
}

impl Callback for CheckRequiresSorted {
    // port: CheckRequiresSorted#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        _n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // Traverse top-level statements until a block of contiguous requires is found.
        !self.finished
            && parent.is_none_or(|parent| {
                parent.is_root(t) || parent.is_script(t) || parent.is_module_body(t)
            })
    }

    // port: CheckRequiresSorted#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_script(t) {
            self.check_canonical(t);
            return;
        }

        let mut call_node = None;
        if n.is_expr_result(t) {
            call_node = n.get_first_child(t);
        } else if NodeUtil::is_name_declaration(t, Some(n)) {
            call_node = n.get_first_child(t).unwrap().get_last_child(t);
        }

        if let Some(call_node) = call_node
            && Self::is_valid_import_call(t, call_node)
        {
            let stmt = Self::parse_import(t, call_node);
            self.original_imports.push(stmt.clone());
            self.imports_by_namespace
                .entry(stmt.namespace.clone())
                .or_default()
                .push(stmt);
            if self.first_node.is_none() {
                self.first_node = Some(n);
                self.last_node = Some(n);
            } else {
                self.last_node = Some(n);
            }
        } else if !self.imports_by_namespace.is_empty() {
            self.finished = true;
        }
    }
}
