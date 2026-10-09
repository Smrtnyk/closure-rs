/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ProcessDefines.java.

//! Port of `ProcessDefines.java`.
//!
//! Process variables annotated as `@define`. A define is a special constant that may be overridden
//! by later files and manipulated by the compiler, much like C preprocessor `#define`s.

use crate::abstract_compiler::AbstractCompiler;
use crate::closure_primitive_errors::{
    INVALID_ARGUMENT_ERROR, INVALID_CLOSURE_CALL_SCOPE_ERROR, NULL_ARGUMENT_ERROR,
    TOO_MANY_ARGUMENTS_ERROR,
};
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::global_namespace::{GlobalNamespace, Name, Ref};
use crate::j2cl_source_file_checker::J2clSourceFileChecker;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_jstype::JSTypeNative;
use closure_jstype::prelude::*;
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::ir::IR;
use closure_rhino::java_lang::pattern::Pattern;
use closure_rhino::js_string::JsString;
use closure_rhino::js_type_expression::JSTypeExpression;
use closure_rhino::jscomp_base::tri::Tri;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

/// Defines in this set will not be flagged with "unknown define" warnings. There are flags that
/// always set these defines, even when they might not be in the binary.
// port: ProcessDefines#KNOWN_DEFINES
pub const KNOWN_DEFINES: [&str; 7] = [
    "COMPILED",
    "goog.DEBUG",
    "$jscomp.ASSUME_ES5",
    "$jscomp.ASSUME_ES6",
    "$jscomp.ASSUME_ES2020",
    "$jscomp.ISOLATE_POLYFILLS",
    "$jscomp.INSTRUMENT_ASYNC_CONTEXT",
];

fn known_defines_contains(name: &JsString) -> bool {
    KNOWN_DEFINES.iter().any(|k| *name == **k)
}

// port: ProcessDefines#GOOG_DEFINE
const GOOG_DEFINE: &str = "goog.define";

// Warnings

// port: ProcessDefines#UNKNOWN_DEFINE_WARNING
pub static UNKNOWN_DEFINE_WARNING: DiagnosticType =
    DiagnosticType::warning("JSC_UNKNOWN_DEFINE_WARNING", "unknown @define variable {0}");

// Errors

// port: ProcessDefines#INVALID_DEFINE_NAME_ERROR
pub static INVALID_DEFINE_NAME_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_DEFINE_NAME_ERROR",
    "\"{0}\" is not a valid JS identifier name",
);

// port: ProcessDefines#MISSING_DEFINE_ANNOTATION
pub static MISSING_DEFINE_ANNOTATION: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_MISSING_DEFINE_ANNOTATION",
    "Missing @define annotation",
);

// port: ProcessDefines#INVALID_DEFINE_TYPE
pub static INVALID_DEFINE_TYPE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_DEFINE_TYPE",
    "@define tag only permits primitive types",
);

// port: ProcessDefines#INVALID_DEFINE_VALUE
pub static INVALID_DEFINE_VALUE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_DEFINE_VALUE",
    "invalid initialization value for @define {0}",
);

// port: ProcessDefines#INVALID_DEFINE_LOCATION
pub static INVALID_DEFINE_LOCATION: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_DEFINE_LOCATION",
    "@define must be initialized on a static qualified name in global or module scope",
);

// port: ProcessDefines#NON_CONST_DEFINE
pub static NON_CONST_DEFINE: DiagnosticType = DiagnosticType::error(
    "JSC_NON_CONST_DEFINE",
    "@define {0} has already been set at {1}.",
);

// port: ProcessDefines#CLOSURE_DEFINES_ERROR
pub static CLOSURE_DEFINES_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_CLOSURE_DEFINES_ERROR",
    "Invalid CLOSURE_DEFINES definition",
);

// port: ProcessDefines#CLOSURE_DEFINES_MULTIPLE
pub static CLOSURE_DEFINES_MULTIPLE: DiagnosticType = DiagnosticType::error(
    "JSC_CLOSURE_DEFINES_MULTIPLE",
    "Multiple CLOSURE_DEFINES definitions for {0}. First occurrence: {1}",
);

// port: ProcessDefines#NON_GLOBAL_CLOSURE_DEFINES_ERROR
pub static NON_GLOBAL_CLOSURE_DEFINES_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_NON_GLOBAL_CLOSURE_DEFINES_ERROR",
    "CLOSURE_DEFINES definition must be in top-level global scope",
);

// port: ProcessDefines#DEFINE_CALL_WITHOUT_ASSIGNMENT
pub static DEFINE_CALL_WITHOUT_ASSIGNMENT: DiagnosticType = DiagnosticType::error(
    "JSC_DEFINE_CALL_WITHOUT_ASSIGNMENT",
    "The result of a goog.define call must be assigned as an isolated statement.",
);

// port: ProcessDefines#ZONE_NOT_SUPPORTED_WITH_NATIVE_ASYNC_AWAIT
pub static ZONE_NOT_SUPPORTED_WITH_NATIVE_ASYNC_AWAIT: DiagnosticType = DiagnosticType::error(
    "JSC_ZONE_NOT_SUPPORTED_WITH_NATIVE_ASYNC_AWAIT",
    "ZoneJS is incompatible with language level ES2017 or higher (See go/ngissue/31730)\nPlease set `--language_out=ECMASCRIPT_2016` (or older) in your flags.",
);

// port: ProcessDefines#DEFINE_WITHOUT_GOOG_DEFINE
pub static DEFINE_WITHOUT_GOOG_DEFINE: DiagnosticType = DiagnosticType::disabled(
    "JSC_DEFINE_WITHOUT_GOOG_DEFINE",
    "The const declaration for @define ''{0}'' must be initialized with a call to goog.define().",
);

/// `ProcessDefines.Mode`.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    CHECK,
    OPTIMIZE,
    CHECK_AND_OPTIMIZE,
}

impl Mode {
    // port: ProcessDefines.Mode#check
    fn check(self) -> bool {
        match self {
            Mode::CHECK => true,
            Mode::OPTIMIZE => false,
            Mode::CHECK_AND_OPTIMIZE => true,
        }
    }

    // port: ProcessDefines.Mode#optimize
    fn optimize(self) -> bool {
        match self {
            Mode::CHECK => false,
            Mode::OPTIMIZE => true,
            Mode::CHECK_AND_OPTIMIZE => true,
        }
    }

    // port: Enum#name
    pub fn name(self) -> &'static str {
        match self {
            Mode::CHECK => "CHECK",
            Mode::OPTIMIZE => "OPTIMIZE",
            Mode::CHECK_AND_OPTIMIZE => "CHECK_AND_OPTIMIZE",
        }
    }
}

/// Java's `Supplier<GlobalNamespace>` (the supplied namespace may be null).
pub type NamespaceSupplier = Box<dyn FnMut() -> Option<Rc<RefCell<GlobalNamespace>>>>;

/// A `JSDocInfo` compared by identity, as Java's `LinkedHashSet<JSDocInfo>` does (JSDocInfo does
/// not override equals/hashCode).
struct JsDocByIdentity(Arc<JSDocInfo>);

impl PartialEq for JsDocByIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for JsDocByIdentity {}
impl Hash for JsDocByIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

/// Builder for ProcessDefines.
pub struct Builder {
    replacement_values: IndexMap<String, NodeId>,
    mode: Option<Mode>,
    namespace_supplier: Option<NamespaceSupplier>,
    recognize_closure_defines: bool,
    enable_zones_define_name: Option<String>,
    zone_input_pattern: Option<Pattern>,
    unknown_defines_to_ignore: Vec<String>,
}

impl Builder {
    // port: ProcessDefines.Builder#Builder
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            replacement_values: IndexMap::<_, _>::default(),
            mode: None,
            namespace_supplier: None,
            recognize_closure_defines: true,
            enable_zones_define_name: None,
            zone_input_pattern: None,
            unknown_defines_to_ignore: Vec::new(),
        }
    }

    // port: ProcessDefines.Builder#putReplacements
    pub fn put_replacements(mut self, replacement_values: IndexMap<String, NodeId>) -> Self {
        self.replacement_values.extend(replacement_values);
        self
    }

    // port: ProcessDefines.Builder#setMode
    pub fn set_mode(mut self, x: Mode) -> Self {
        self.mode = Some(x);
        self
    }

    /// Injects a pre-computed global namespace, so that the same namespace can be re-used for
    /// multiple check passes. Accepts a supplier because the namespace may not exist at
    /// pass-creation time.
    // port: ProcessDefines.Builder#injectNamespace
    pub fn inject_namespace(mut self, namespace_supplier: NamespaceSupplier) -> Self {
        self.namespace_supplier = Some(namespace_supplier);
        self
    }

    // port: ProcessDefines.Builder#setUnknownDefinesToIgnore
    pub fn set_unknown_defines_to_ignore(mut self, unknown_defines_to_ignore: Vec<String>) -> Self {
        self.unknown_defines_to_ignore = unknown_defines_to_ignore;
        self
    }

    // port: ProcessDefines.Builder#setRecognizeClosureDefines
    pub fn set_recognize_closure_defines(mut self, recognize_closure_defines: bool) -> Self {
        self.recognize_closure_defines = recognize_closure_defines;
        self
    }

    // port: ProcessDefines.Builder#setEnableZonesDefineName
    pub fn set_enable_zones_define_name(
        mut self,
        enable_zones_define_name: Option<String>,
    ) -> Self {
        self.enable_zones_define_name = enable_zones_define_name;
        self
    }

    // port: ProcessDefines.Builder#setZoneInputPattern
    pub fn set_zone_input_pattern(mut self, zone_input_pattern: Option<Pattern>) -> Self {
        self.zone_input_pattern = zone_input_pattern;
        self
    }

    // port: ProcessDefines.Builder#build
    pub fn build(self, compiler: &mut AbstractCompiler) -> ProcessDefines {
        ProcessDefines::new(self, compiler)
    }
}

/// Process variables annotated as `@define`.
pub struct ProcessDefines {
    /// Java's `@Nullable JSTypeRegistry registry`: non-null exactly in check mode.
    has_registry: bool,
    replacement_values_from_flags: IndexMap<JsString, NodeId>,
    mode: Mode,
    namespace_supplier: Option<NamespaceSupplier>,
    recognize_closure_defines: bool,
    enable_zones_define_name: Option<String>,
    zone_input_pattern: Option<Pattern>,
    unknown_defines_to_ignore: IndexSet<JsString>,

    known_define_jsdocs: IndexSet<JsDocByIdentity>,
    known_goog_define_calls: IndexSet<NodeId>,
    define_by_define_name: IndexMap<JsString, Define>,
    // from var CLOSURE_DEFINES = {
    replacement_values_from_closure_defines: IndexMap<JsString, NodeId>,
    valid_define_value_expressions: IndexSet<NodeId>,
    has_zone_input: bool,

    namespace: Option<Rc<RefCell<GlobalNamespace>>>,
}

impl ProcessDefines {
    /// Create a pass that overrides define constants.
    // port: ProcessDefines#ProcessDefines
    fn new(builder: Builder, compiler: &mut AbstractCompiler) -> Self {
        let mode = builder.mode.expect("mode");
        if mode.check() {
            compiler.get_type_registry();
        }
        Self {
            has_registry: mode.check(),
            replacement_values_from_flags: builder
                .replacement_values
                .into_iter()
                .map(|(k, v)| (JsString::from(k), v))
                .collect(),
            mode,
            namespace_supplier: builder.namespace_supplier,
            recognize_closure_defines: builder.recognize_closure_defines,
            enable_zones_define_name: builder.enable_zones_define_name,
            zone_input_pattern: builder.zone_input_pattern,
            unknown_defines_to_ignore: builder
                .unknown_defines_to_ignore
                .into_iter()
                .map(JsString::from)
                .collect(),
            known_define_jsdocs: IndexSet::<_>::default(),
            known_goog_define_calls: IndexSet::<_>::default(),
            define_by_define_name: IndexMap::<_, _>::default(),
            replacement_values_from_closure_defines: IndexMap::<_, _>::default(),
            valid_define_value_expressions: IndexSet::<_>::default(),
            has_zone_input: false,
            namespace: None,
        }
    }

    fn namespace(&self) -> Rc<RefCell<GlobalNamespace>> {
        Rc::clone(self.namespace.as_ref().expect("namespace"))
    }

    // port: ProcessDefines#initNamespace
    fn init_namespace(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        if let Some(supplier) = &mut self.namespace_supplier {
            self.namespace = supplier();
        }
        if self.namespace.is_none() {
            self.namespace = Some(Rc::new(RefCell::new(GlobalNamespace::new(
                compiler, externs, root,
            ))));
        }
    }

    // port: ProcessDefines#overrideDefines
    fn override_defines(&mut self, compiler: &mut AbstractCompiler) {
        if self.mode.optimize() {
            let defines = self
                .define_by_define_name
                .values()
                .map(|d| (d.define_name.clone(), d.value_parent, d.value))
                .collect::<Vec<_>>();
            for (define_name, value_parent, value) in defines {
                let Some(value_parent) = value_parent else {
                    continue;
                };

                let input_value = self.get_replacement_for_define(compiler, &define_name, value);
                let Some(input_value) = input_value.filter(|iv| Some(*iv) != value) else {
                    continue;
                };

                let changed = match value {
                    None => true,
                    Some(value) => {
                        input_value.get_token(compiler) != value.get_token(compiler)
                            || !input_value.is_equivalent_to(compiler, value)
                    }
                };
                if changed {
                    match value {
                        None => {
                            let clone = input_value.clone_tree(compiler);
                            value_parent.add_child_to_back(compiler, clone);
                        }
                        Some(value) => {
                            let clone = input_value.clone_tree(compiler);
                            value.replace_with(compiler, clone);
                        }
                    }

                    compiler.report_change_to_enclosing_scope(value_parent);
                }

                // When requested, create a globally accessible alias for all define values. This
                // will provide a hook for passes like J2clUtilGetDefineRewriterPass to read them
                // in out-of-scope contexts.
                if J2clSourceFileChecker::should_run_j2cl_passes(compiler) {
                    let alias = get_global_define_alias(&define_name);
                    if alias != define_name {
                        // If we had:
                        //  var x = goog.define('y', ...);
                        // we'll add an additional statement:
                        //  var goog$defines$y = x;
                        let global_define_name = NodeUtil::new_name_with_srcref(
                            compiler,
                            get_global_define_alias(&define_name),
                            value_parent,
                        );
                        let define_lhs = if value_parent.is_assign(compiler) {
                            value_parent
                                .get_first_child(compiler)
                                .unwrap()
                                .clone_tree(compiler)
                        } else {
                            value_parent.clone_node(compiler)
                        };
                        assert!(
                            define_lhs.is_name(compiler) || define_lhs.is_qualified_name(compiler),
                            "{define_lhs:?}"
                        );
                        let global_define_var =
                            IR::var_with_value(compiler, global_define_name, define_lhs)
                                .srcref_tree_if_missing(compiler, value_parent);
                        let statement = value_parent.get_parent(compiler).unwrap();
                        global_define_var.insert_after(compiler, statement);
                        compiler.report_change_to_enclosing_scope(statement);
                    }
                }
            }
        }

        if self.mode.optimize() {
            // Sets.difference(Sets.union(flags, closureDefines),
            //     Sets.union(Sets.union(KNOWN_DEFINES, defines), unknownDefinesToIgnore))
            let mut union = self
                .replacement_values_from_flags
                .keys()
                .cloned()
                .collect::<IndexSet<_>>();
            union.extend(self.replacement_values_from_closure_defines.keys().cloned());
            let unused_replacements = union
                .into_iter()
                .filter(|k| {
                    !(known_defines_contains(k)
                        || self.define_by_define_name.contains_key(k)
                        || self.unknown_defines_to_ignore.contains(k))
                })
                .collect::<Vec<_>>();

            for unknown_define in unused_replacements {
                compiler.report(JSError::make_without_location(
                    &UNKNOWN_DEFINE_WARNING,
                    &[&unknown_define.to_string()],
                ));
            }
        }
    }

    /// Returns the replacement value for a @define, if any.
    ///
    ///   1. First checks the flags/compiler options `--define=FOO=1`
    ///   2. If nothing was found, check for values in a "var CLOSURE_DEFINES = {'FOO': 1}`
    ///      definition
    ///   3. If nothing was found, and this is defined via a goog.define call, replace the call
    ///      with the default value.
    // port: ProcessDefines#getReplacementForDefine
    fn get_replacement_for_define(
        &self,
        compiler: &AbstractCompiler,
        define_name: &JsString,
        value: Option<NodeId>,
    ) -> Option<NodeId> {
        let replacement = self.get_replacement_for_define_name(Some(define_name));
        if replacement.is_some() {
            return replacement;
        }

        if self.is_goog_define_call(compiler, value)
            && value.unwrap().get_child_count(compiler) == 3
        {
            // Return the second argument of goog.define('name', false);
            return value.unwrap().get_child_at_index(compiler, 2);
        }
        None
    }

    /// Returns the replacement value for a @define, if any.
    ///
    ///   1. First checks the flags/compiler options `--define=FOO=1`
    ///   2. If nothing was found, check for values in a "var CLOSURE_DEFINES = {'FOO': 1}`
    ///      definition
    ///   3. If still not found, returns `null`.
    // port: ProcessDefines#getReplacementForDefineName
    fn get_replacement_for_define_name(&self, define_name: Option<&JsString>) -> Option<NodeId> {
        let define_name = define_name?;
        let replacement_from_flags = self.replacement_values_from_flags.get(define_name);
        if let Some(r) = replacement_from_flags {
            return Some(*r);
        }

        let replacement_from_closure_defines = self
            .replacement_values_from_closure_defines
            .get(define_name);
        if let Some(r) = replacement_from_closure_defines {
            return Some(*r);
        }

        None
    }

    /// Only defines of literal number, string, or boolean are supported.
    // port: ProcessDefines#isValidDefineType
    fn is_valid_define_type(
        &self,
        compiler: &mut AbstractCompiler,
        expression: Option<Arc<JSTypeExpression>>,
    ) -> bool {
        assert!(self.has_registry, "NullPointerException: registry");
        let expression = expression.expect("NullPointerException: expression");
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let r#type = registry.evaluate_type_expression_in_global_scope(ast, &expression);
        let number_string_boolean = registry.get_native_type(JSTypeNative::NUMBER_STRING_BOOLEAN);
        !r#type.is_unknown_type(registry, ast)
            && r#type.is_subtype_of(registry, ast, number_string_boolean)
    }

    /// Finds all defines, and creates a [`Define`] data structure for each one.
    // port: ProcessDefines#collectDefines
    fn collect_defines(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        if self.recognize_closure_defines {
            let mut collector = ClosureDefinesCollector { outer: self };
            NodeTraversal::builder()
                .set_compiler(compiler)
                .set_callback(&mut collector)
                .traverse(root);
        }
        let namespace = self.namespace();
        let mut namespace = namespace.borrow_mut();
        for name in namespace.get_all_symbols(compiler) {
            let gn = &*namespace;
            let Some(mut declaration) = self.select_define_declaration(compiler, gn, name) else {
                continue;
            };

            let mut total_sets = name.get_total_sets(gn);
            let value_parent = get_value_parent_for_define(compiler, gn, declaration);
            let value = value_parent.and_then(|vp| vp.get_last_child(compiler));

            let define_name = if self.is_goog_define_call(compiler, value)
                && self.verify_goog_define(compiler, value.unwrap())
            {
                let name_node = value.unwrap().get_second_child(compiler).unwrap();
                name_node.get_string(compiler)
            } else {
                name.get_full_name(gn)
            };
            let existing_define = match self.define_by_define_name.get(&define_name) {
                Some(existing) => Some((existing.declaration, existing.name)),
                None => {
                    self.define_by_define_name.insert(
                        define_name.clone(),
                        Define::new(
                            compiler,
                            gn,
                            define_name.clone(),
                            name,
                            declaration,
                            value_parent,
                            value,
                        ),
                    );
                    None
                }
            };

            if let Some((existing_declaration, existing_name)) = existing_define {
                declaration = existing_declaration;
                total_sets += existing_name.get_total_sets(gn);
            }

            /*
             * We have to report this here because otherwise we don't remember which names have
             * the same define name. It's not worth it tracking a set of names, because it makes
             * the rest of the pass more complex.
             */
            if total_sets > 1 {
                for r in name.get_refs(gn) {
                    if r.is_set(gn) && r != declaration {
                        let location = declaration.get_node(gn).unwrap().get_location(compiler);
                        let error = JSError::make(
                            compiler,
                            r.get_node(gn).unwrap(),
                            &NON_CONST_DEFINE,
                            &[&define_name.to_string(), &location],
                        );
                        compiler.report(error);
                    }
                }
            }
        }
        let mut define_names = KNOWN_DEFINES
            .iter()
            .map(|s| s.to_string())
            .collect::<IndexSet<_>>();
        define_names.extend(self.define_by_define_name.keys().map(|k| k.to_string()));
        compiler.set_define_names(define_names);
    }

    // port: ProcessDefines#selectDefineDeclaration
    fn select_define_declaration(
        &mut self,
        compiler: &AbstractCompiler,
        gn: &GlobalNamespace,
        name: Name,
    ) -> Option<Ref> {
        for r in name.get_refs(gn) {
            // Make sure we don't select a local set as the declaration.
            if !r.is_set_from_global(gn) {
                continue;
            }

            let ref_node = r.get_node(gn).unwrap();
            if !ref_node.is_qualified_name(compiler) {
                continue;
            }

            let jsdoc = NodeUtil::get_best_jsdoc_info(compiler, ref_node);
            let Some(jsdoc) = jsdoc.filter(|j| j.is_define()) else {
                continue;
            };

            self.known_define_jsdocs.insert(JsDocByIdentity(jsdoc));
            return Some(r);
        }

        None
    }

    // port: ProcessDefines#collectValidDefineValueExpressions
    fn collect_valid_define_value_expressions(&mut self, compiler: &mut AbstractCompiler) {
        let namespace = self.namespace();
        let mut namespace = namespace.borrow_mut();
        let all_symbols = namespace.get_all_symbols(compiler);
        let gn = &*namespace;
        let mut names_to_check = all_symbols
            .into_iter()
            .filter(|n| is_global_const(gn, *n))
            .collect::<IndexSet<Name>>();

        // All defines are implicitly valid in the values of other defines.
        for define in self.define_by_define_name.values() {
            names_to_check.shift_remove(&define.name);
            for r in define.name.get_refs(gn) {
                if !r.is_set(gn) {
                    self.valid_define_value_expressions
                        .insert(r.get_node(gn).unwrap());
                }
            }
        }

        // Do a breadth-first search of all const names to find those defined in terms of valid
        // values.
        loop {
            let mut names_to_check_again = IndexSet::<_>::default();

            for name in &names_to_check {
                let decl_node = name.get_declaration(gn).unwrap().get_node(gn).unwrap();
                let decl_value = get_constant_decl_value(compiler, decl_node);
                match self.is_valid_define_value(compiler, decl_value) {
                    Tri::TRUE => {
                        for r in name.get_refs(gn) {
                            self.valid_define_value_expressions
                                .insert(r.get_node(gn).unwrap());
                        }
                    }
                    Tri::UNKNOWN => {
                        names_to_check_again.insert(*name);
                    }
                    _ => {}
                }
            }

            if names_to_check_again.len() == names_to_check.len() {
                break;
            } else {
                names_to_check = names_to_check_again;
            }
        }
    }

    // port: ProcessDefines#validateDefineDeclarations
    fn validate_define_declarations(&mut self, compiler: &mut AbstractCompiler) {
        if !self.mode.check() {
            return;
        }

        let namespace = self.namespace();
        let namespace = namespace.borrow();
        let gn = &*namespace;
        let defines = self
            .define_by_define_name
            .values()
            .map(|d| {
                (
                    d.define_name.clone(),
                    d.declaration,
                    d.value_parent,
                    d.value,
                )
            })
            .collect::<Vec<_>>();
        for (define_name, declaration, value_parent, value) in defines {
            let declaration_node = declaration.get_node(gn).unwrap();

            if !self.is_goog_define_call(compiler, value)
                && !known_defines_contains(&define_name)
                && !define_name.to_string().starts_with("$jscomp.")
            {
                let error = JSError::make(
                    compiler,
                    value.or(value_parent).unwrap_or(declaration_node),
                    &DEFINE_WITHOUT_GOOG_DEFINE,
                    &[&define_name.to_string(), "go/js-practices/locals#defines"],
                ); // Example help link
                compiler.report(error);
            }

            if !self.has_valid_value(compiler, value_parent, value) {
                let error = JSError::make(
                    compiler,
                    value.or(value_parent).unwrap_or(declaration_node),
                    &INVALID_DEFINE_VALUE,
                    &[&define_name.to_string()],
                );
                compiler.report(error);
            }

            /*
             * Process defines should not depend on check types being enabled, so we look for the
             * JSDoc instead of the inferred type.
             */
            let jsdoc = NodeUtil::get_best_jsdoc_info(compiler, declaration_node);
            let valid = match jsdoc {
                None => false,
                Some(jsdoc) => self.is_valid_define_type(compiler, jsdoc.get_type()),
            };
            if !valid {
                let error = JSError::make(compiler, declaration_node, &INVALID_DEFINE_TYPE, &[]);
                compiler.report(error);
            }
        }
    }

    /// Checks for misplaced @define and goog.define calls
    // port: ProcessDefines#reportInvalidDefineLocations
    #[allow(clippy::collapsible_if)] // Java's nested ifs
    fn report_invalid_define_locations(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        if !self.mode.check() {
            return;
        }

        /*
         * This has to be done using a traversal because the global namespace doesn't record
         * symbols which only appear in local scopes.
         *
         * <p>We don't check the externs because they can't contain local vars.
         */
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback_post_order(
                |t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>| {
                    let compiler = t.get_compiler();
                    let jsdoc = n.get_jsdoc_info(compiler);
                    if let Some(jsdoc) = jsdoc {
                        if jsdoc.is_define()
                            && self.known_define_jsdocs.insert(JsDocByIdentity(jsdoc))
                        {
                            let error = JSError::make(compiler, n, &INVALID_DEFINE_LOCATION, &[]);
                            compiler.report(error);
                        }
                    }

                    if self.is_goog_define_call(compiler, Some(n))
                        && self.known_goog_define_calls.insert(n)
                    {
                        self.verify_goog_define(compiler, n);
                    }

                    if n.matches_name(compiler, "CLOSURE_DEFINES") {
                        let parent = parent.unwrap();
                        if (NodeUtil::is_name_declaration(compiler, Some(parent))
                            || (parent.is_get_elem(compiler)
                                && parent.get_parent(compiler).unwrap().is_assign(compiler)))
                            && !NodeUtil::get_enclosing_scope_root(compiler, n)
                                .unwrap()
                                .is_root(compiler)
                        {
                            let error =
                                JSError::make(compiler, n, &NON_GLOBAL_CLOSURE_DEFINES_ERROR, &[]);
                            compiler.report(error);
                        }
                    }
                },
            )
            .traverse(root);
    }

    // port: ProcessDefines#handleClosureDefinesValue
    fn handle_closure_defines_value(
        &mut self,
        compiler: &mut AbstractCompiler,
        string_node: NodeId,
        value_node: Option<NodeId>,
        error_node: NodeId,
    ) {
        if (string_node.is_string_key(compiler) || string_node.is_string_lit(compiler))
            && is_valid_closure_defines_value(compiler, value_node.unwrap())
        {
            let key = string_node.get_string(compiler);
            if let Some(first) = self.replacement_values_from_closure_defines.get(&key) {
                let location = first.get_location(compiler);
                let error = JSError::make(
                    compiler,
                    error_node,
                    &CLOSURE_DEFINES_MULTIPLE,
                    &[&key.to_string(), &location],
                );
                compiler.report(error);
            }
            self.replacement_values_from_closure_defines
                .insert(key, value_node.unwrap());
        } else if self.mode.check() {
            let error = JSError::make(compiler, error_node, &CLOSURE_DEFINES_ERROR, &[]);
            compiler.report(error);
        }
    }

    // port: ProcessDefines#hasValidValue
    fn has_valid_value(
        &self,
        compiler: &AbstractCompiler,
        value_parent: Option<NodeId>,
        value: Option<NodeId>,
    ) -> bool {
        match value_parent {
            None => false,
            Some(vp) if vp.is_from_externs(compiler) => true,
            Some(_) => self
                .is_valid_define_value(compiler, value)
                .to_boolean(false),
        }
    }

    /// Determines whether the given value may be assigned to a define.
    ///
    /// @param val The value being assigned.
    // port: ProcessDefines#isValidDefineValue
    #[allow(clippy::collapsible_match)] // Java's switch with fall-through
    fn is_valid_define_value(&self, compiler: &AbstractCompiler, val: Option<NodeId>) -> Tri {
        let Some(val) = val else {
            return Tri::FALSE;
        };

        match val.get_token(compiler) {
            Token::STRINGLIT | Token::NUMBER | Token::TRUE | Token::FALSE => {
                return Tri::TRUE;
                // Binary operators are only valid if both children are valid.
            }
            Token::AND
            | Token::OR
            | Token::COALESCE
            | Token::ADD
            | Token::BITAND
            | Token::BITNOT
            | Token::BITOR
            | Token::BITXOR
            | Token::DIV
            | Token::EQ
            | Token::EXPONENT
            | Token::GE
            | Token::GT
            | Token::LE
            | Token::LSH
            | Token::LT
            | Token::MOD
            | Token::MUL
            | Token::NE
            | Token::RSH
            | Token::SHEQ
            | Token::SHNE
            | Token::SUB
            | Token::URSH => {
                return self
                    .is_valid_define_value(compiler, val.get_first_child(compiler))
                    .and(self.is_valid_define_value(compiler, val.get_last_child(compiler)));
            }
            Token::HOOK => {
                return self
                    .is_valid_define_value(compiler, val.get_first_child(compiler))
                    .and(self.is_valid_define_value(compiler, val.get_second_child(compiler)))
                    .and(self.is_valid_define_value(compiler, val.get_last_child(compiler)));
                // Unary operators are valid if the child is valid.
            }
            Token::NOT | Token::NEG | Token::POS => {
                return self.is_valid_define_value(compiler, val.get_first_child(compiler));
                // Names are valid if and only if they are defines themselves.
            }
            Token::NAME | Token::GETPROP => {
                if val.is_qualified_name(compiler) {
                    return if self.valid_define_value_expressions.contains(&val) {
                        Tri::TRUE
                    } else {
                        Tri::UNKNOWN
                    };
                }
                // Allow goog.define('XYZ', <val>) calls if and only if <val> is valid.
            }
            Token::CALL => {
                if !self.is_goog_define_call(compiler, Some(val)) {
                    return Tri::FALSE;
                }
                if !val.has_x_children(compiler, 3) {
                    // goog.define call with wrong arg count. Warn elsewhere and treat this call
                    // as valid.
                    return Tri::TRUE;
                }
                return self.is_valid_define_value(compiler, val.get_child_at_index(compiler, 2));
            }
            _ => {}
        }

        Tri::FALSE
    }

    // port: ProcessDefines#validateDefines
    fn validate_defines(&mut self, compiler: &mut AbstractCompiler) {
        // Validate that a Zone-enabled app does not include native async/await, which is
        // incompatible.
        let will_output_async_functions = compiler
            .get_options()
            .get_output_feature_set()
            .contains(Feature::ASYNC_FUNCTIONS);
        if self.is_zone_enabled(compiler) && self.has_zone_input && will_output_async_functions {
            compiler.report(JSError::make_without_location(
                &ZONE_NOT_SUPPORTED_WITH_NATIVE_ASYNC_AWAIT,
                &[],
            ));
        }
    }

    // port: ProcessDefines#isZoneEnabled
    fn is_zone_enabled(&self, compiler: &AbstractCompiler) -> bool {
        let enable_zones_default = true;
        let enable_zones_define_name = self.enable_zones_define_name.as_deref().map(JsString::from);
        let zone_enabled = self.get_replacement_for_define_name(enable_zones_define_name.as_ref());
        let Some(zone_enabled) = zone_enabled else {
            return enable_zones_default;
        };

        NodeUtil::get_boolean_value(compiler, zone_enabled).to_boolean(enable_zones_default)
    }

    // port: ProcessDefines#isGoogDefineCall
    fn is_goog_define_call(&self, compiler: &AbstractCompiler, node: Option<NodeId>) -> bool {
        if !self.recognize_closure_defines {
            return false;
        }

        let Some(node) = node.filter(|n| n.is_call(compiler)) else {
            return false;
        };
        node.get_first_child(compiler)
            .unwrap()
            .matches_qualified_name(compiler, GOOG_DEFINE)
    }

    /// Verifies that a goog.define method call has exactly two arguments, with the first a string
    /// literal whose contents is a valid JS qualified name. Reports a compile error if it doesn't.
    ///
    /// @return Whether the argument checked out okay
    // port: ProcessDefines#verifyGoogDefine
    #[allow(clippy::if_same_then_else)] // Java's two identical branches
    fn verify_goog_define(&mut self, compiler: &mut AbstractCompiler, call_node: NodeId) -> bool {
        self.known_goog_define_calls.insert(call_node);

        let mut parent = call_node.get_parent(compiler).unwrap();
        let method_name = call_node.get_first_child(compiler).unwrap();
        let args = call_node.get_second_child(compiler);

        // Calls to goog.define must be in the global hoist scope after module rewriting
        if NodeUtil::get_enclosing_function(compiler, call_node).is_some() {
            let error = JSError::make(
                compiler,
                method_name.get_parent(compiler).unwrap(),
                &INVALID_CLOSURE_CALL_SCOPE_ERROR,
                &[],
            );
            compiler.report(error);
            return false;
        }

        // It is an error for goog.define to show up anywhere except immediately after =.
        if parent.is_assign(compiler)
            && parent
                .get_parent(compiler)
                .unwrap()
                .is_expr_result(compiler)
        {
            parent = parent.get_parent(compiler).unwrap();
        } else if parent.is_name(compiler)
            && NodeUtil::is_name_declaration(compiler, parent.get_parent(compiler))
        {
            parent = parent.get_parent(compiler).unwrap();
        } else {
            let error = JSError::make(
                compiler,
                method_name.get_parent(compiler).unwrap(),
                &DEFINE_CALL_WITHOUT_ASSIGNMENT,
                &[],
            );
            compiler.report(error);
            return false;
        }

        // Verify first arg
        let arg = args;
        if !self.verify_not_null(compiler, method_name, arg)
            || !self.verify_of_type(compiler, method_name, arg.unwrap(), Token::STRINGLIT)
        {
            return false;
        }
        let args = args.unwrap();

        // Verify second arg
        let arg = args.get_next(compiler);
        if !args.is_from_externs(compiler)
            && (!self.verify_not_null(compiler, method_name, arg)
                || !self.verify_is_last(compiler, method_name, arg.unwrap()))
        {
            return false;
        }

        let name = args.get_string(compiler);
        let feature_set = compiler.get_options().get_language_in().to_feature_set();
        if !NodeUtil::is_valid_qualified_name_features(feature_set, &name) {
            let error = JSError::make(
                compiler,
                args,
                &INVALID_DEFINE_NAME_ERROR,
                &[&name.to_string()],
            );
            compiler.report(error);
            return false;
        }

        let info_node = if parent.is_expr_result(compiler) {
            parent.get_first_child(compiler).unwrap()
        } else {
            parent
        };
        let info = info_node.get_jsdoc_info(compiler);
        if !info.is_some_and(|i| i.is_define()) {
            let error = JSError::make(compiler, parent, &MISSING_DEFINE_ANNOTATION, &[]);
            compiler.report(error);
            return false;
        }
        true
    }

    /// @return Whether the argument checked out okay
    // port: ProcessDefines#verifyNotNull
    fn verify_not_null(
        &self,
        compiler: &mut AbstractCompiler,
        method_name: NodeId,
        arg: Option<NodeId>,
    ) -> bool {
        if arg.is_none() {
            let qname = method_name
                .get_qualified_name(compiler)
                .map(|q| q.to_string())
                .unwrap_or_else(|| "null".into());
            let error = JSError::make(compiler, method_name, &NULL_ARGUMENT_ERROR, &[&qname]);
            compiler.report(error);
            return false;
        }
        true
    }

    /// @return Whether the argument checked out okay
    // port: ProcessDefines#verifyIsLast
    fn verify_is_last(
        &self,
        compiler: &mut AbstractCompiler,
        method_name: NodeId,
        arg: NodeId,
    ) -> bool {
        if arg.get_next(compiler).is_some() {
            let qname = method_name
                .get_qualified_name(compiler)
                .map(|q| q.to_string())
                .unwrap_or_else(|| "null".into());
            let error = JSError::make(compiler, method_name, &TOO_MANY_ARGUMENTS_ERROR, &[&qname]);
            compiler.report(error);
            return false;
        }
        true
    }

    /// @return Whether the argument checked out okay
    // port: ProcessDefines#verifyOfType
    fn verify_of_type(
        &self,
        compiler: &mut AbstractCompiler,
        method_name: NodeId,
        arg: NodeId,
        desired_type: Token,
    ) -> bool {
        if arg.get_token(compiler) != desired_type {
            let qname = method_name
                .get_qualified_name(compiler)
                .map(|q| q.to_string())
                .unwrap_or_else(|| "null".into());
            let error = JSError::make(compiler, method_name, &INVALID_ARGUMENT_ERROR, &[&qname]);
            compiler.report(error);
            return false;
        }
        true
    }
}

impl CompilerPass for ProcessDefines {
    // port: ProcessDefines#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.init_namespace(compiler, externs, root);
        self.collect_defines(compiler, root);
        self.report_invalid_define_locations(compiler, root);
        self.collect_valid_define_value_expressions(compiler);
        self.validate_define_declarations(compiler);
        self.override_defines(compiler);
        self.validate_defines(compiler);
    }
}

// port: ProcessDefines#getGlobalDefineAlias
pub fn get_global_define_alias(define_name: &JsString) -> JsString {
    // Known defines are already globally accessible and may not be present in code. Therefore
    // we'll reference them directly.
    // goog.LOCALE is a special case as it's not processed as a define, it is instead
    // late-substituted.
    if known_defines_contains(define_name) || *define_name == *"goog.LOCALE" {
        return define_name.clone();
    }
    JsString::from(format!(
        "jscomp$defines${}",
        define_name.to_string().replace('.', "$")
    ))
}

// port: ProcessDefines#getValueParentForDefine
fn get_value_parent_for_define(
    compiler: &AbstractCompiler,
    gn: &GlobalNamespace,
    declaration: Ref,
) -> Option<NodeId> {
    // Note: this may be a NAME, a GETPROP, or even STRING_KEY or GETTER_DEF. We only care
    // about the first two, in which case the parent should be either VAR/CONST or ASSIGN.
    // We could accept STRING_KEY (i.e. `@define` on a property in an object literal), but
    // there's no reason to add another new way to do the same thing.
    let declaration_node = declaration.get_node(gn).unwrap();
    let declaration_parent = declaration_node.get_parent(compiler).unwrap();

    if declaration_parent.is_var(compiler) || declaration_parent.is_const(compiler) {
        // Simple case of `var` or `const`. There's no reason to support `let` here, and we
        // don't explicitly check that it's not `let` anywhere else.
        assert!(declaration_node.is_name(compiler), "{declaration_node:?}");
        return Some(declaration_node);
    } else if declaration_parent.is_assign(compiler)
        && declaration_node.is_first_child_of(compiler, Some(declaration_parent))
    {
        // Assignment. Must either assign to a qualified name, or else be a different ref than
        // the declaration to not emit an error (we don't allow assignment before it's
        // declared).
        return Some(declaration_parent);
    }
    None
}

// port: ProcessDefines#isValidClosureDefinesValue
fn is_valid_closure_defines_value(compiler: &AbstractCompiler, val: NodeId) -> bool {
    // Values allowed in 'var CLOSURE_DEFINES = {'
    // Must be a subset of the values allowed for <val> in
    // /** @define {...} */ var DEF = <val>
    match val.get_token(compiler) {
        Token::STRINGLIT | Token::NUMBER | Token::TRUE | Token::FALSE => true,
        Token::NEG => val.get_first_child(compiler).unwrap().is_number(compiler),
        _ => false,
    }
}

// port: ProcessDefines#isGlobalConst
fn is_global_const(gn: &GlobalNamespace, name: Name) -> bool {
    name.get_total_sets(gn) == 1
        && name.get_declaration(gn).is_some()
        && name.get_declaration(gn).unwrap().is_set_from_global(gn)
}

/// Checks whether the NAME node is inside either a CONST or a @const VAR. Returns the RHS node if
/// so, otherwise returns null.
// port: ProcessDefines#getConstantDeclValue
fn get_constant_decl_value(compiler: &AbstractCompiler, name: NodeId) -> Option<NodeId> {
    let parent = name.get_parent(compiler)?;
    if name.is_name(compiler) {
        if parent.is_const(compiler) {
            return name.get_first_child(compiler);
        } else if !parent.is_var(compiler) {
            return None;
        }
        let jsdoc = NodeUtil::get_best_jsdoc_info(compiler, name);
        return if jsdoc.is_some_and(|j| j.is_constant()) {
            name.get_first_child(compiler)
        } else {
            None
        };
    } else if name.is_get_prop(compiler) && parent.is_assign(compiler) {
        let jsdoc = NodeUtil::get_best_jsdoc_info(compiler, name);
        return if jsdoc.is_some_and(|j| j.is_constant()) {
            name.get_next(compiler)
        } else {
            None
        };
    }
    None
}

// port: ProcessDefines.ClosureDefinesCollector
struct ClosureDefinesCollector<'a> {
    outer: &'a mut ProcessDefines,
}

impl Callback for ClosureDefinesCollector<'_> {
    // port: ProcessDefines.ClosureDefinesCollector#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let compiler = t.get_compiler();
        // In particular, don't traverse into modules or functions - only script top level scopes.
        n.is_root(compiler)
            || n.is_script(compiler)
            || n.is_expr_result(compiler)
            || n.is_assign(compiler)
            || NodeUtil::is_name_declaration(compiler, Some(n))
    }

    // port: ProcessDefines.ClosureDefinesCollector#visit
    #[allow(clippy::collapsible_if)] // Java's nested ifs
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        if n.is_script(compiler) {
            if let Some(zone_input_pattern) = &self.outer.zone_input_pattern {
                let source = n.get_source_file_name(compiler);
                if let Some(source) = source {
                    if zone_input_pattern.matcher(&source).matches() {
                        self.outer.has_zone_input = true;
                    }
                }
            }
        }

        if NodeUtil::is_name_declaration(compiler, Some(n))
            && n.get_first_child(compiler)
                .unwrap()
                .matches_name(compiler, "CLOSURE_DEFINES")
        {
            // var CLOSURE_DEFINES = {...};
            let value_node = n.get_first_first_child(compiler);
            if let Some(value_node) = value_node.filter(|v| v.is_object_lit(compiler)) {
                let mut c = value_node.get_first_child(compiler);
                while let Some(cn) = c {
                    let value = cn.get_first_child(compiler);
                    self.outer
                        .handle_closure_defines_value(compiler, cn, value, n);
                    c = cn.get_next(compiler);
                }
            }
        } else if n.is_assign(compiler) {
            // CLOSURE_DEFINES['...'] = ...;
            let lhs = n.get_first_child(compiler).unwrap();
            if lhs.is_get_elem(compiler)
                && lhs
                    .get_first_child(compiler)
                    .unwrap()
                    .matches_name(compiler, "CLOSURE_DEFINES")
            {
                let string_node = lhs.get_second_child(compiler).unwrap();
                let value = n.get_second_child(compiler);
                self.outer
                    .handle_closure_defines_value(compiler, string_node, value, n);
            }
        }
    }
}

// port: ProcessDefines.Define
struct Define {
    define_name: JsString,
    name: Name,

    /// The connonical set ref with an `@define` or `goog.define`.
    ///
    /// This may not be the same as `name.getDeclaration()`.
    declaration: Ref,

    value_parent: Option<NodeId>,
    value: Option<NodeId>,
}

impl Define {
    // port: ProcessDefines.Define#Define
    fn new(
        compiler: &AbstractCompiler,
        gn: &GlobalNamespace,
        define_name: JsString,
        name: Name,
        declaration: Ref,
        value_parent: Option<NodeId>,
        value: Option<NodeId>,
    ) -> Self {
        assert!(
            value_parent.is_none()
                || value.is_none()
                || value.unwrap().get_parent(compiler) == value_parent
        );
        assert!(declaration.is_set(gn));

        Self {
            define_name,
            name,
            declaration,
            value_parent,
            value,
        }
    }
}
