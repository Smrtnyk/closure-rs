/*
 * Copyright 2026 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 030d049:
//   src/com/google/javascript/jscomp/ClosureExportTypeClassifier.java.

//! Syntactically classifies whether each goog.provide or goog.module namespace in a file is seen
//! from TypeScript as a set of "named" exports or as a single "default" export.
//!
//! This approximates the type-based logic originally implemented in Clutz. So most of the
//! (potentially surprising) logic is in place to mirror Clutz & allow migration to this syntactic
//! classifier.
//!
//! The core rules:
//!
//! - Object literals with at least one non-computed, non-spread key are NAMED.
//! - Properties assigned or declared (`/** @type {T} */ ns.x;`) on the namespace, or on a local
//!   alias of it, make an empty object NAMED.
//! - `exports = {a, b};` in a goog.module is NAMED even with a declared type, since module
//!   rewriting turns it into `exports.a = a; exports.b = b;` and drops the JSDoc.
//! - A dotted goog.provide namespace assigned without `@const` or a type isn't declared by the
//!   typechecker, so it's DEFAULT.
//! - A goog.provide namespace with a nested provide in this file has a property for it.
//!
//! Besides top-level statements, it only scans the bodies of top-level goog.scope callbacks.
//!
//! Users can make a namespace `named` with `@legacyNamedTsExport` on its
//! `goog.provide`/`goog.module` call. The annotation has no effect elsewhere.

#![allow(clippy::collapsible_match)] // Keep Java's control flow.
use crate::modules::module_metadata_map::TsExportType;
use crate::node_util::NodeUtil;
use closure_rhino::check_argument;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::js_type_expression::JSTypeExpression;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::qualified_name::QualifiedName;
use closure_rhino::token::Token;
use std::collections::BTreeSet;
use std::sync::{Arc, LazyLock};

// port: ClosureExportTypeClassifier#GOOG_SCOPE
static GOOG_SCOPE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.scope"));
// port: ClosureExportTypeClassifier#EXPORTS
const EXPORTS: &str = "exports";

/// How many local aliases to follow, e.g. `const a = {}; const b = a; exports = b;`.
///
/// Clutz follows aliases without a limit; this is an arbitrary cap to keep the work bounded.
/// Comparing against Clutz on a large codebase found no namespace that needs a longer chain.
// port: ClosureExportTypeClassifier#MAX_ALIAS_DEPTH
const MAX_ALIAS_DEPTH: i32 = 4;

/// The syntactic "shape" of a value, as relevant for determining its export type.
// port: ClosureExportTypeClassifier.Kind
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Classes, functions, primitives, etc.
    NOT_NAMESPACE,
    /// An object with no known properties, like `{}`. May be treated as either NAMED or DEFAULT
    /// depending on whether properties are added to it later on in the file.
    EMPTY_OBJECT,
    /// An object with at least one property.
    NAMESPACE,
}

/// Which kind of top-level name a `NameInfo` is about.
// port: ClosureExportTypeClassifier.NameRole
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NameRole {
    /// A goog.module's `exports`.
    GOOG_MODULE_EXPORTS,
    /// A dotted namespace provided by this goog.provide file, e.g. 'a.b' but not 'a'.
    /// goog.provide implicitly initializes it to {}. An explicit assignment to one only counts
    /// with declaring JSDoc: JSCompiler doesn't declare `a.b = {x: 1};` (no `@const` or type).
    DOTTED_PROVIDE,
    /// An undotted namespace provided by this goog.provide file, e.g. 'a'. goog.provide
    /// implicitly initializes it to {}.
    UNDOTTED_PROVIDE,
    /// A local variable.
    LOCAL,
}

/// Facts gathered about a top-level name: `exports`, a provide, or a local variable.
// port: ClosureExportTypeClassifier.NameInfo
struct NameInfo {
    /// The last value assigned to this name, if any.
    value: Option<NodeId>,

    /// The JSDoc on the declaration or assignment of `value`.
    jsdoc: Option<Arc<JSDocInfo>>,

    /// Whether this name is declared or assigned in a way that is never a namespace (class,
    /// function, or stub typedef).
    is_never_namespace: bool,

    /// Whether this name is a class or function declaration annotated `@nodts`.
    is_no_dts_declaration: bool,

    /// Number of properties assigned or declared on this name (`name.x = ...;`).
    property_count: i32,

    role: NameRole,

    /// Whether `value` is assigned as `exports = {a, b};` in a goog.module. JSCompiler's module
    /// rewriting turns that into `exports.a = a; exports.b = b;` (see
    /// `NodeUtil#isNamedExportsLiteral`), dropping any JSDoc on the assignment, so it's always
    /// NAMED.
    is_named_exports_literal: bool,
}

impl NameInfo {
    // port: ClosureExportTypeClassifier.NameInfo#NameInfo
    fn new(role: NameRole) -> Self {
        Self {
            value: None,
            jsdoc: None,
            is_never_namespace: false,
            is_no_dts_declaration: false,
            property_count: 0,
            role,
            is_named_exports_literal: false,
        }
    }

    /// Whether this is a namespace provided by this goog.provide file.
    // port: ClosureExportTypeClassifier.NameInfo#isProvide
    fn is_provide(&self) -> bool {
        self.role == NameRole::DOTTED_PROVIDE || self.role == NameRole::UNDOTTED_PROVIDE
    }
}

/// Java's `String#lastIndexOf(int, int)`.
fn last_index_of_char_from(s: &JsString, c: u16, from: i32) -> i32 {
    if from < 0 {
        return -1;
    }
    let units = s.as_units();
    let end = (from as usize).min(units.len().saturating_sub(1));
    units[..=end]
        .iter()
        .rposition(|v| *v == c)
        .map_or(-1, |i| i as i32)
}

// port: ClosureExportTypeClassifier
#[derive(Default)]
pub struct ClosureExportTypeClassifier {
    // Java's LinkedHashMap.
    top_level_names: IndexMap<JsString, NameInfo>,

    /// Provided namespaces that are not simple names, e.g. 'a.b.c' but not 'a', keyed by their
    /// last segment ('c').
    ///
    /// This index is just for performance: on any GETPROP node, `n.getString()` gives the last
    /// property name without allocating a new String. If that name isn't in this map, we
    /// immediately know the node is not one of our provided namespaces, avoiding both heap
    /// allocations from `n.getQualifiedName()` and O(N_provides) scans via
    /// `n.matchesQualifiedName(...)` on every property assignment in files with many provides
    /// (e.g. generated i18n data).
    dotted_provides_by_last_segment: IndexMap<JsString, Vec<JsString>>,

    /// Local names aliasing a provided namespace or a strict prefix of a dotted provide, mapped to
    /// the aliased qualified name, e.g. `const xhr = goog.labs.net.xhr;`, `const ns =
    /// confusion;`, or `const alignment = aplos.data.alignment;` for a provide
    /// 'aplos.data.alignment.Aligner', usually inside goog.scope. Qualified names rooted at an
    /// alias resolve to the aliased name, so properties assigned via an alias count as properties
    /// of the provide.
    aliases: IndexMap<JsString, JsString>,

    /// All strict prefixes of the dotted provides, e.g. 'a' and 'a.b' for 'a.b.c', to find
    /// `aliases` with a single lookup.
    dotted_provide_prefixes: IndexSet<JsString>,

    /// Namespaces whose goog.provide or goog.module call is annotated `@legacyNamedTsExport`.
    forced_named_export: IndexSet<JsString>,
}

impl ClosureExportTypeClassifier {
    /// Classifies each of `goog_namespaces` as named, default, or `@nodts`.
    ///
    /// `module_root` is the SCRIPT node for this goog.provide or goog.module file,
    /// `goog_namespaces` the namespaces provided by this file. Returns the export type of each
    /// of `goog_namespaces`, in the same order.
    // port: ClosureExportTypeClassifier#classify
    pub fn classify<'a>(
        ast: &Ast,
        module_root: NodeId,
        goog_namespaces: impl ExactSizeIterator<Item = &'a JsString> + Clone,
    ) -> IndexMap<JsString, TsExportType> {
        if goog_namespaces.len() == 0 {
            return IndexMap::default();
        }
        let is_goog_module = NodeUtil::is_goog_module_file(ast, module_root);
        let mut classifier = ClosureExportTypeClassifier::default();
        if is_goog_module {
            classifier.add_goog_module_exports();
        } else {
            for namespace in goog_namespaces.clone() {
                classifier.add_provide(namespace);
            }
        }
        // Java passes `classifier::visitStatement` as the callback.
        classifier.for_each_top_level_statement(ast, module_root, is_goog_module);
        if !is_goog_module && goog_namespaces.len() > 1 {
            classifier.add_implicit_properties_from_nested_provides(goog_namespaces.clone());
        }

        let mut export_types = IndexMap::<JsString, TsExportType>::default();
        for namespace in goog_namespaces {
            let key = if is_goog_module {
                JsString::from(EXPORTS)
            } else {
                namespace.clone()
            };
            // Java: NullPointerException when the namespace has no NameInfo.
            let info = classifier.top_level_names.get(&key).unwrap();
            if info.jsdoc.as_ref().is_some_and(|jsdoc| jsdoc.is_no_dts())
                || classifier.has_no_dts_type(ast, info, 0)
            {
                // ImmutableMap.Builder#buildOrThrow: the namespaces are distinct (a set).
                export_types.insert(namespace.clone(), TsExportType::NO_DTS);
                continue;
            }

            let is_named = classifier.forced_named_export.contains(namespace)
                || classifier.kind_of_name(ast, info, 0) == Kind::NAMESPACE;
            export_types.insert(
                namespace.clone(),
                if is_named {
                    TsExportType::NAMED
                } else {
                    TsExportType::DEFAULT
                },
            );
        }
        export_types
    }

    // port: ClosureExportTypeClassifier#addGoogModuleExports
    fn add_goog_module_exports(&mut self) {
        let info = NameInfo::new(NameRole::GOOG_MODULE_EXPORTS);
        self.top_level_names.insert(JsString::from(EXPORTS), info);
    }

    // port: ClosureExportTypeClassifier#addProvide
    fn add_provide(&mut self, namespace: &JsString) {
        let last_dot = namespace.last_index_of_char(b'.' as u16);
        let role = if last_dot != -1 {
            NameRole::DOTTED_PROVIDE
        } else {
            NameRole::UNDOTTED_PROVIDE
        };
        let info = NameInfo::new(role);
        self.top_level_names.insert(namespace.clone(), info);
        if last_dot != -1 {
            // Index by last segment (e.g. 'c' for 'a.b.c') so getInfoForQualifiedName can quickly
            // filter GETPROP nodes via n.getString() without allocating strings.
            self.dotted_provides_by_last_segment
                .entry(namespace.substring_from(last_dot as usize + 1))
                .or_insert_with(|| Vec::with_capacity(1))
                .push(namespace.clone());
            let mut dot = last_dot;
            while dot != -1 {
                self.dotted_provide_prefixes
                    .insert(namespace.substring(0, dot as usize));
                dot = last_index_of_char_from(namespace, b'.' as u16, dot - 1);
            }
        }
    }

    /// goog.provide('a.b'); goog.provide('a.b.c'); implicitly creates a property 'c' on 'a.b'.
    /// This cannot see nested provides in other files.
    // port: ClosureExportTypeClassifier#addImplicitPropertiesFromNestedProvides
    fn add_implicit_properties_from_nested_provides<'a>(
        &mut self,
        provides: impl Iterator<Item = &'a JsString> + Clone,
    ) {
        // In sorted order, any namespace nested under 'a.b' sorts in ['a.b.', 'a.b/'), since '/'
        // is the character after '.'. (Java's TreeSet: JsString orders as String#compareTo.)
        let sorted: BTreeSet<JsString> = provides.clone().cloned().collect();
        for namespace in provides {
            let lo = namespace.concat(&JsString::from("."));
            let hi = namespace.concat(&JsString::from("/"));
            let nested_count = sorted.range(lo..hi).count() as i32;
            if nested_count > 0 {
                let info = self.top_level_names.get_mut(namespace).unwrap();
                info.property_count = info.property_count.wrapping_add(nested_count);
            }
        }
    }

    /// Whether the value held by `info` has a type annotated `@nodts`: Clutz's
    /// `ClosureTypes.isNoDts` checks the JSDoc of the exported symbol's type, which is the JSDoc
    /// of the class, function, or object literal declaration that created it.
    // port: ClosureExportTypeClassifier#hasNoDtsType
    fn has_no_dts_type(&self, ast: &Ast, info: &NameInfo, depth: i32) -> bool {
        if info.is_no_dts_declaration {
            return true; // `/** @nodts */ class C {}`
        }
        if info.is_never_namespace {
            return false;
        }
        let Some(value) = info.value else {
            return false;
        };
        if info.jsdoc.as_ref().is_some_and(|jsdoc| jsdoc.is_no_dts())
            && (value.is_object_lit(ast) || value.is_class(ast) || value.is_function(ast))
        {
            return true; // `/** @nodts */ const C = class {};` or `/** @nodts */ const x = {...};`
        }
        if value.is_name(ast) && depth < MAX_ALIAS_DEPTH {
            let aliased = self.top_level_names.get(value.get_string_ref(ast));
            return aliased.is_some_and(|aliased| self.has_no_dts_type(ast, aliased, depth + 1));
        }
        false
    }

    // ---------------------------------------------------------------------------------------------
    // Gathering facts from top-level statements.
    // ---------------------------------------------------------------------------------------------

    /// Calls `visitStatement` (Java: a `callback`) on each top-level statement of the file,
    /// including statements directly inside a top-level goog.scope callback.
    // port: ClosureExportTypeClassifier#forEachTopLevelStatement
    fn for_each_top_level_statement(
        &mut self,
        ast: &Ast,
        module_root: NodeId,
        is_goog_module: bool,
    ) {
        check_argument!(module_root.is_script(ast), "%s", module_root.to_string(ast));
        // A goog.provide file is a script, so its statements are directly under the SCRIPT.
        check_argument!(
            is_goog_module
                || !module_root.has_children(ast)
                || !module_root
                    .get_first_child(ast)
                    .unwrap()
                    .is_module_body(ast),
            "%s",
            module_root.to_string(ast)
        );
        let body = if is_goog_module {
            // Java: NullPointerException when a goog.module file has no children.
            module_root.get_first_child(ast).unwrap()
        } else {
            module_root
        };
        let mut statement = body.get_first_child(ast);
        while let Some(s) = statement {
            let scope_body = Self::get_goog_scope_body(ast, s);
            if let Some(scope_body) = scope_body {
                let saved_aliases = if self.aliases.is_empty() {
                    None
                } else {
                    Some(self.aliases.clone())
                };
                let mut inner = scope_body.get_first_child(ast);
                while let Some(i) = inner {
                    self.visit_statement(ast, i);
                    inner = i.get_next(ast);
                }
                self.aliases.clear();
                if let Some(saved_aliases) = saved_aliases {
                    self.aliases.extend(saved_aliases);
                }
            } else {
                self.visit_statement(ast, s);
            }
            statement = s.get_next(ast);
        }
    }

    /// Returns the body of the callback of a `goog.scope(function() { ... });` statement.
    // port: ClosureExportTypeClassifier#getGoogScopeBody
    fn get_goog_scope_body(ast: &Ast, statement: NodeId) -> Option<NodeId> {
        if !NodeUtil::is_expr_call(ast, statement) {
            return None;
        }
        let call = statement.get_first_child(ast).unwrap();
        if !GOOG_SCOPE.matches(ast, call.get_first_child(ast).unwrap()) {
            return None;
        }
        let r#fn = call.get_second_child(ast);
        if let Some(r#fn) = r#fn
            && r#fn.is_function(ast)
            && !r#fn.is_generator_function(ast)
            && !r#fn.is_async_function(ast)
        {
            let body = NodeUtil::get_function_body(ast, r#fn);
            if body.is_block(ast) {
                // ignore invalid code passing an arrow function
                return Some(body);
            }
        }
        None
    }

    // port: ClosureExportTypeClassifier#visitStatement
    fn visit_statement(&mut self, ast: &Ast, statement: NodeId) {
        match statement.get_token(ast) {
            Token::VAR | Token::LET | Token::CONST => self.visit_name_declaration(ast, statement),
            Token::CLASS | Token::FUNCTION => {
                let name = statement.get_first_child(ast).unwrap();
                let jsdoc = NodeUtil::get_best_jsdoc_info(ast, statement);
                let info = self.get_or_create(name.get_string(ast));
                info.is_never_namespace = true;
                info.is_no_dts_declaration = jsdoc.is_some_and(|jsdoc| jsdoc.is_no_dts());
            }
            Token::EXPR_RESULT => {
                self.maybe_record_legacy_named_ts_export(ast, statement);
                self.visit_expression(ast, statement.get_first_child(ast).unwrap());
            }
            _ => {}
        }
    }

    /// Records an explicit @legacyNamedTsExport annotation on a goog.provide or goog.module call
    /// statement.
    // port: ClosureExportTypeClassifier#maybeRecordLegacyNamedTsExport
    fn maybe_record_legacy_named_ts_export(&mut self, ast: &Ast, statement: NodeId) {
        if !NodeUtil::is_goog_module_call(ast, statement)
            && !NodeUtil::is_goog_provide_call(ast, statement)
        {
            return;
        }
        let call = statement.get_first_child(ast).unwrap();
        let jsdoc = call.get_jsdoc_info_ref(ast);
        if !jsdoc.is_some_and(|jsdoc| jsdoc.is_legacy_named_ts_export()) {
            return;
        }
        let arg = call.get_second_child(ast);
        let Some(arg) = arg else {
            return;
        };
        if !arg.is_string_lit(ast) {
            return;
        }
        self.forced_named_export.insert(arg.get_string(ast));
    }

    // port: ClosureExportTypeClassifier#visitNameDeclaration
    fn visit_name_declaration(&mut self, ast: &Ast, declaration: NodeId) {
        let mut lhs = declaration.get_first_child(ast);
        while let Some(l) = lhs {
            if l.is_name(ast) {
                self.visit_declared_name(ast, l);
            }
            lhs = l.get_next(ast);
        }
    }

    // port: ClosureExportTypeClassifier#visitDeclaredName
    fn visit_declared_name(&mut self, ast: &Ast, lhs: NodeId) {
        let initializer = lhs.get_first_child(ast);
        let name = lhs.get_string(ast);
        if let Some(initializer) = initializer {
            self.maybe_record_alias(ast, name.clone(), initializer);
        }
        // Java keeps the NameInfo object across the visit of the initializer; it is looked up by
        // name again below (the map never drops an entry, so it is the same object).
        self.get_or_create(name.clone());
        if let Some(initializer) = initializer
            && initializer.is_assign(ast)
        {
            // e.g. `const fromJson = a.b.fromJson = function() {};`
            self.visit_expression(ast, initializer);
        }
        let jsdoc = NodeUtil::get_best_jsdoc_info(ast, lhs);
        let info = self.get_or_create(name);
        if initializer.is_some() {
            info.value = initializer;
            info.jsdoc = jsdoc;
        } else if info.value.is_none() && jsdoc.is_some() {
            // A stub like `/** @type {T} */ let x;`. A bare redeclaration like `var x;` keeps any
            // earlier value.
            info.jsdoc = jsdoc;
        }
    }

    /// Records `name` in `aliases` if `initializer` is a qualified name that is a provided
    /// namespace or a strict prefix of a dotted provide; or an alias thereof.
    ///
    /// For example, these are all treated as aliases: `const ns = confusion;`, `const a =
    /// aplos;`, or `const alignment = aplos.data.alignment;`.
    // port: ClosureExportTypeClassifier#maybeRecordAlias
    fn maybe_record_alias(&mut self, ast: &Ast, name: JsString, initializer: NodeId) {
        let mut target: Option<JsString> = None;
        if initializer.is_name(ast) {
            let initializer_name = initializer.get_string_ref(ast);
            target = Some(
                self.aliases
                    .get(initializer_name)
                    .unwrap_or(initializer_name)
                    .clone(),
            );
        } else if !self.dotted_provide_prefixes.is_empty() {
            // Only dotted provides and their prefixes can match, so skip files without any like
            // `goog.module` files.
            target = self.resolve_aliased_qualified_name(ast, initializer);
            if target.is_none() {
                target = initializer.get_qualified_name(ast);
            }
        }
        if let Some(target) = target
            && (self.dotted_provide_prefixes.contains(&target)
                || self.get_provide_info(&target).is_some())
        {
            self.aliases.insert(name, target);
        } else {
            self.aliases.shift_remove(&name);
        }
    }

    // port: ClosureExportTypeClassifier#visitExpression
    fn visit_expression(&mut self, ast: &Ast, expr: NodeId) {
        match expr.get_token(ast) {
            Token::ASSIGN => self.visit_assign(ast, expr),
            Token::GETPROP => {
                // Stub declarations like `/** @typedef {number} */ exports.T;`
                let declared = self.get_info_index_for_qualified_name(ast, expr);
                let jsdoc = expr.get_jsdoc_info_ref(ast);
                if let Some(declared) = declared {
                    let declared = &mut self.top_level_names[declared];
                    // Only a @typedef stub (before any assignment) suppresses goog.provide's
                    // implicit `{}`.
                    if declared.value.is_none()
                        && jsdoc.is_some_and(|jsdoc| jsdoc.has_typedef_type())
                    {
                        declared.is_never_namespace = true;
                    }
                    return;
                }
                if jsdoc.is_some_and(|jsdoc| {
                    jsdoc.has_type()
                        || Self::is_defining_type(Some(jsdoc))
                        || NodeUtil::is_function_type_declaration(jsdoc)
                }) {
                    self.record_property(ast, expr.get_first_child(ast).unwrap());
                }
            }
            _ => {}
        }
    }

    // port: ClosureExportTypeClassifier#visitAssign
    fn visit_assign(&mut self, ast: &Ast, expr: NodeId) {
        let lhs = expr.get_first_child(ast).unwrap();
        let rhs = expr.get_second_child(ast).unwrap();
        let jsdoc = expr.get_jsdoc_info(ast);
        if lhs.is_name(ast) {
            self.aliases.shift_remove(lhs.get_string_ref(ast));
        }
        let assigned = self.get_info_index_for_qualified_name(ast, lhs);
        if let Some(assigned) = assigned {
            // `exports = ...;`, `a.b.Provide = ...;`, or `localName = ...;`
            let assigned = &mut self.top_level_names[assigned];
            assigned.value = Some(rhs);
            assigned.jsdoc = jsdoc;
            assigned.is_never_namespace = false;
            assigned.is_no_dts_declaration = false;
            assigned.is_named_exports_literal = assigned.role == NameRole::GOOG_MODULE_EXPORTS
                && NodeUtil::is_named_exports_literal(ast, rhs);
            return;
        }
        if lhs.is_get_prop(ast) {
            // `exports.x = ...;`, `a.b.Provide.x = ...;`, or `localName.x = ...;`
            self.record_property(ast, lhs.get_first_child(ast).unwrap());
        }
    }

    // port: ClosureExportTypeClassifier#recordProperty
    fn record_property(&mut self, ast: &Ast, owner: NodeId) {
        let mut owner_info: Option<usize> = None;
        if owner.is_name(ast)
            && let Some(aliased) = self.aliases.get(owner.get_string_ref(ast))
        {
            // `alias.x = ...;` where the alias is of a provide.
            owner_info = self.get_provide_info(aliased);
        }
        if owner_info.is_none() {
            owner_info = self.get_info_index_for_qualified_name(ast, owner);
        }
        if let Some(owner_info) = owner_info {
            let info = &mut self.top_level_names[owner_info];
            info.property_count = info.property_count.wrapping_add(1);
        }
    }

    /// Returns the info for the given node if it is a simple name or a provided namespace
    /// (possibly via an alias, e.g. `alignment.Aligner`), or null.
    ///
    /// Avoids allocating qualified name strings (via `n.getQualifiedName()`) by first checking
    /// the unallocated last-segment string (`n.getString()`) against
    /// `dotted_provides_by_last_segment`, and only then testing candidates with zero-allocation
    /// `n.matchesQualifiedName(provide)`.
    ///
    /// Returns the index of the info in `top_level_names` (Java returns the object).
    // port: ClosureExportTypeClassifier#getInfoForQualifiedName
    fn get_info_index_for_qualified_name(&mut self, ast: &Ast, n: NodeId) -> Option<usize> {
        if n.is_name(ast) {
            return Some(self.get_or_create_index(n.get_string(ast)));
        }
        if !n.is_get_prop(ast) || self.dotted_provides_by_last_segment.is_empty() {
            return None;
        }
        let candidates = self
            .dotted_provides_by_last_segment
            .get(n.get_string_ref(ast))?;
        let resolved = self.resolve_aliased_qualified_name(ast, n);
        if let Some(resolved) = resolved {
            return self.get_provide_info(&resolved);
        }
        for provide in candidates {
            if n.matches_qualified_name(ast, provide) {
                return self.top_level_names.get_index_of(provide);
            }
        }
        None
    }

    /// Returns the qualified name of `n` with its root replaced by the aliased name, e.g.
    /// 'aplos.data.Aligner' for `a.data.Aligner` after `const a = aplos;`, or null if `n` isn't a
    /// qualified name rooted at an alias.
    // port: ClosureExportTypeClassifier#resolveAliasedQualifiedName
    fn resolve_aliased_qualified_name(&self, ast: &Ast, n: NodeId) -> Option<JsString> {
        if self.aliases.is_empty() || !n.is_qualified_name(ast) {
            return None;
        }
        let root = NodeUtil::get_root_of_qualified_name(ast, n);
        let target = if root.is_name(ast) {
            self.aliases.get(root.get_string_ref(ast)).cloned()
        } else {
            None
        };
        let target = target?;
        if n == root {
            return Some(target);
        }
        let qualified_name = n.get_qualified_name(ast).unwrap();
        Some(target.concat(&qualified_name.substring_from(root.get_string_ref(ast).length())))
    }

    /// Returns the info for `qname` if it is provided in this file, or null (as its index in
    /// `top_level_names`).
    // port: ClosureExportTypeClassifier#getProvideInfo
    fn get_provide_info(&self, qname: &JsString) -> Option<usize> {
        let (index, _, info) = self.top_level_names.get_full(qname)?;
        if info.is_provide() { Some(index) } else { None }
    }

    // port: ClosureExportTypeClassifier#getOrCreate
    fn get_or_create(&mut self, name: JsString) -> &mut NameInfo {
        self.top_level_names
            .entry(name)
            .or_insert_with(|| NameInfo::new(NameRole::LOCAL))
    }

    /// `getOrCreate`, as the index of the info in `top_level_names`.
    fn get_or_create_index(&mut self, name: JsString) -> usize {
        let entry = self.top_level_names.entry(name);
        let index = entry.index();
        entry.or_insert_with(|| NameInfo::new(NameRole::LOCAL));
        index
    }

    // ---------------------------------------------------------------------------------------------
    // Classifying values.
    // ---------------------------------------------------------------------------------------------

    // port: ClosureExportTypeClassifier#kindOfName
    fn kind_of_name(&self, ast: &Ast, info: &NameInfo, depth: i32) -> Kind {
        if info.is_never_namespace {
            return Kind::NOT_NAMESPACE;
        }
        if info.is_named_exports_literal {
            return Kind::NAMESPACE;
        }
        if info.role == NameRole::DOTTED_PROVIDE
            && info.value.is_some()
            && !Self::is_declaring_js_doc(info.jsdoc.as_deref())
        {
            return Kind::NOT_NAMESPACE;
        }
        let kind = if info.value.is_some() || info.jsdoc.is_some() {
            self.kind_of_value(ast, info.value, info.jsdoc.as_deref(), depth)
        } else if info.role != NameRole::LOCAL {
            // goog.module and goog.provide implicitly initialize exports and provides to {}.
            Kind::EMPTY_OBJECT
        } else {
            Kind::NOT_NAMESPACE
        };
        if kind == Kind::EMPTY_OBJECT && info.property_count > 0 {
            return Kind::NAMESPACE;
        }
        kind
    }

    // port: ClosureExportTypeClassifier#isDeclaringJsDoc
    fn is_declaring_js_doc(jsdoc: Option<&JSDocInfo>) -> bool {
        Self::is_defining_type(jsdoc)
            || jsdoc.is_some_and(|jsdoc| jsdoc.is_constant() || jsdoc.has_type())
    }

    // port: ClosureExportTypeClassifier#isDefiningType
    fn is_defining_type(jsdoc: Option<&JSDocInfo>) -> bool {
        // Anything defining a new type (i.e. some name that can be referred to in a type
        // annotation) is never a namespace.
        jsdoc.is_some_and(|jsdoc| {
            jsdoc.has_enum_parameter_type()
                || jsdoc.has_typedef_type()
                || jsdoc.is_constructor_or_interface()
        })
    }

    // port: ClosureExportTypeClassifier#kindOfValue
    fn kind_of_value(
        &self,
        ast: &Ast,
        value: Option<NodeId>,
        jsdoc: Option<&JSDocInfo>,
        depth: i32,
    ) -> Kind {
        if let Some(jsdoc) = jsdoc {
            if Self::is_defining_type(Some(jsdoc)) {
                return Kind::NOT_NAMESPACE;
            }
            if jsdoc.has_type() {
                return Self::kind_of_declared_type(ast, &jsdoc.get_type().unwrap());
            }
        }
        let Some(value) = value else {
            return Kind::NOT_NAMESPACE;
        };
        match value.get_token(ast) {
            Token::OBJECTLIT => {
                // Computed properties and spreads do not create named own properties.
                let mut member = value.get_first_child(ast);
                while let Some(m) = member {
                    if !m.is_computed_prop(ast) && !m.is_spread(ast) {
                        return Kind::NAMESPACE;
                    }
                    member = m.get_next(ast);
                }
                Kind::EMPTY_OBJECT
            }
            Token::NAME => {
                if depth >= MAX_ALIAS_DEPTH {
                    return Kind::NOT_NAMESPACE;
                }
                match self.top_level_names.get(value.get_string_ref(ast)) {
                    None => Kind::NOT_NAMESPACE,
                    Some(aliased) => self.kind_of_name(ast, aliased, depth + 1),
                }
            }
            Token::CAST => self.kind_of_value(
                ast,
                value.get_first_child(ast),
                value.get_jsdoc_info(ast).as_deref(),
                depth,
            ),
            // Classes, functions, calls, `new`, literals, expressions, etc.
            _ => Kind::NOT_NAMESPACE,
        }
    }

    /// Only record type literals like `{a: number}` are namespaces. Any named type (e.g.
    /// `!Object<string, number>` or `Foo`) is an instance type or a typedef, which the syntactic
    /// analysis cannot resolve, so is not a namespace.
    // port: ClosureExportTypeClassifier#kindOfDeclaredType
    fn kind_of_declared_type(ast: &Ast, type_: &JSTypeExpression) -> Kind {
        let root = type_.get_root();
        if root.get_token(ast) == Token::LC {
            Kind::NAMESPACE
        } else {
            Kind::NOT_NAMESPACE
        }
    }
}
