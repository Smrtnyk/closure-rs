/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ClosureRewriteModule.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Process aliases in goog.modules.
//!
//! ```text
//! goog.module('foo.Bar');
//! var Baz = goog.require('foo.Baz');
//! class Bar extends Baz {}
//! exports = Bar;
//! ```
//!
//! becomes
//!
//! ```text
//! class module$contents$foo$Bar_Bar extends module$exports$foo$Baz {}
//! var module$exports$foo$Bar = module$contents$foo$Bar_Bar;
//! ```
//!
//! and
//!
//! ```text
//! goog.loadModule(function(exports) {
//!   goog.module('foo.Bar');
//!   var Baz = goog.require('foo.Baz');
//!   class Bar extends Baz {}
//!   exports = Bar;
//!   return exports;
//! })
//! ```
//!
//! becomes
//!
//! ```text
//! class module$contents$foo$Bar_Bar extends module$exports$foo$Baz {}
//! var module$exports$foo$Bar = module$contents$foo$Bar_Bar;
//! ```

#![allow(clippy::collapsible_if, clippy::collapsible_match)] // Retain Java control flow.

use crate::AbstractCompiler;
use crate::ast_factory::{AstFactory, AstFactoryStaticScope, Type};
use crate::change_tracker::ChangeTracker;
use crate::closure_primitive_errors::{
    GOOG_MODULE_GET_OF_WEAK_MODULE, INVALID_FORWARD_DECLARE_NAMESPACE, INVALID_GET_NAMESPACE,
    INVALID_REQUIRE_DYNAMIC, INVALID_REQUIRE_NAMESPACE, INVALID_REQUIRE_TYPE_NAMESPACE,
};
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::preprocessor_symbol_table::PreprocessorSymbolTable;
use crate::scope::ScopeId;
use crate::typed_scope::TypedScope;
use crate::var::VarId;
use crate::xid::Xid;
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::jsdoc_info::{Builder as JSDocInfoBuilder, JSDocInfo};
use closure_rhino::node::{Ast, NodeId, Prop};
use closure_rhino::qualified_name::QualifiedName;
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null, check_state};
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex};

// port: ClosureRewriteModule#INVALID_MODULE_ID_ARG
pub static INVALID_MODULE_ID_ARG: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_MODULE_ID_ARG",
    "goog.module parameter must be a string literal",
);

// port: ClosureRewriteModule#INVALID_PROVIDE_NAMESPACE
pub static INVALID_PROVIDE_NAMESPACE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_PROVIDE_NAMESPACE",
    "goog.provide parameter must be a string literal.",
);

// port: ClosureRewriteModule#INVALID_PROVIDE_CALL
pub static INVALID_PROVIDE_CALL: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_PROVIDE_CALL",
    "goog.provide can not be called in goog.module.",
);

// port: ClosureRewriteModule#INVALID_GET_ALIAS
pub static INVALID_GET_ALIAS: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_GET_ALIAS",
    "goog.module.get should not be aliased.",
);

// port: ClosureRewriteModule#INVALID_EXPORT_COMPUTED_PROPERTY
pub static INVALID_EXPORT_COMPUTED_PROPERTY: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_MODULE_INVALID_EXPORT_COMPUTED_PROPERTY",
    "Computed properties are not yet supported in goog.module exports.",
);

// port: ClosureRewriteModule#USELESS_USE_STRICT_DIRECTIVE
pub static USELESS_USE_STRICT_DIRECTIVE: DiagnosticType = DiagnosticType::disabled(
    "JSC_USELESS_USE_STRICT_DIRECTIVE",
    "'use strict' is unnecessary in goog.module files.",
);

// port: ClosureRewriteModule#IMPORT_INLINING_SHADOWS_VAR
pub static IMPORT_INLINING_SHADOWS_VAR: DiagnosticType = DiagnosticType::error(
    "JSC_IMPORT_INLINING_SHADOWS_VAR",
    "Inlining of reference to import \"{1}\" shadows var \"{0}\".",
);

// port: ClosureRewriteModule#ILLEGAL_DESTRUCTURING_DEFAULT_EXPORT
pub static ILLEGAL_DESTRUCTURING_DEFAULT_EXPORT: DiagnosticType = DiagnosticType::error(
    "JSC_ILLEGAL_DESTRUCTURING_DEFAULT_EXPORT",
    "Destructuring import only allowed for importing module with named exports.\nSee https://github.com/google/closure-compiler/wiki/goog.module-style",
);

// port: ClosureRewriteModule#ILLEGAL_DESTRUCTURING_NOT_EXPORTED
pub static ILLEGAL_DESTRUCTURING_NOT_EXPORTED: DiagnosticType = DiagnosticType::error(
    "JSC_ILLEGAL_DESTRUCTURING_NOT_EXPORTED",
    "Destructuring import reference to name \"{0}\" was not exported in module {1}",
);

// port: ClosureRewriteModule#LOAD_MODULE_FN_MISSING_RETURN
pub static LOAD_MODULE_FN_MISSING_RETURN: DiagnosticType = DiagnosticType::error(
    "JSC_LOAD_MODULE_FN_MISSING_RETURN",
    "goog.loadModule function should end with 'return exports;'",
);

// port: ClosureRewriteModule#ILLEGAL_MODULE_RENAMING_CONFLICT
pub static ILLEGAL_MODULE_RENAMING_CONFLICT: DiagnosticType = DiagnosticType::error(
    "JSC_ILLEGAL_MODULE_RENAMING_CONFLICT",
    "Internal compiler error: rewritten module global name {0} is already in use.\nOriginal definition: {1}",
);

// port: ClosureRewriteModule#ILLEGAL_STMT_OF_GOOG_REQUIRE_DYNAMIC_IN_AWAIT
pub static ILLEGAL_STMT_OF_GOOG_REQUIRE_DYNAMIC_IN_AWAIT: DiagnosticType = DiagnosticType::error(
    "ILLEGAL_STMT_OF_GOOG_REQUIRE_DYNAMIC_IN_AWAIT",
    "Illegal use of dynamic import: LHS of await goog.requireDynamic() must be a destructing LHS or name, and it must be in a declaration statement.",
);

// port: ClosureRewriteModule#MODULE_EXPORTS_PREFIX
pub const MODULE_EXPORTS_PREFIX: &str = "module$exports$";

// port: ClosureRewriteModule#MODULE_CONTENTS_PREFIX
const MODULE_CONTENTS_PREFIX: &str = "module$contents$";

// port: ClosureRewriteModule#GOOG_MODULE_PREVENTMODULEEXPORTSEALING
static GOOG_MODULE_PREVENTMODULEEXPORTSEALING: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.preventModuleExportSealing"));
// port: ClosureRewriteModule#GOOG_REQUIREDYNAMIC_NAME
const GOOG_REQUIREDYNAMIC_NAME: &str = "goog.requireDynamic";
// port: ClosureRewriteModule#IMPORT_HANDLER_NAME
const IMPORT_HANDLER_NAME: &str = "importHandler_";

/// Prebuilt Nodes to speed up Node.matchesQualifiedName() calls.
///
/// Java keeps them in static fields; a Rust node lives in one compilation's arena, so each pass
/// instance builds the same detached trees, in the same order, in its compiler's arena.
struct PrebuiltNodes {
    goog_forwarddeclare: NodeId,
    goog_loadmodule: NodeId,
    goog_module: NodeId,
    goog_module_declarelegacynamespace: NodeId,
    goog_module_get: NodeId,
    goog_provide: NodeId,
    goog_require: NodeId,
    goog_requiretype: NodeId,
    goog_requiredynamic: NodeId,
}

impl PrebuiltNodes {
    // port: ClosureRewriteModule#<clinit>
    fn new(ast: &mut Ast) -> Self {
        let goog = IR::name(ast, "goog");
        let goog_forwarddeclare = IR::getprop(ast, goog, "forwardDeclare");
        let goog = IR::name(ast, "goog");
        let goog_loadmodule = IR::getprop(ast, goog, "loadModule");
        let goog = IR::name(ast, "goog");
        let goog_module = IR::getprop(ast, goog, "module");
        let goog_module_declarelegacynamespace =
            IR::getprop(ast, goog_module, "declareLegacyNamespace");
        let goog_module_clone = goog_module.clone_tree(ast);
        let goog_module_get = IR::getprop(ast, goog_module_clone, "get");
        let goog = IR::name(ast, "goog");
        let goog_provide = IR::getprop(ast, goog, "provide");
        let goog = IR::name(ast, "goog");
        let goog_require = IR::getprop(ast, goog, "require");
        let goog = IR::name(ast, "goog");
        let goog_requiretype = IR::getprop(ast, goog, "requireType");
        let goog = IR::name(ast, "goog");
        let goog_requiredynamic = IR::getprop(ast, goog, "requireDynamic");
        Self {
            goog_forwarddeclare,
            goog_loadmodule,
            goog_module,
            goog_module_declarelegacynamespace,
            goog_module_get,
            goog_provide,
            goog_require,
            goog_requiretype,
            goog_requiredynamic,
        }
    }
}

/// Indicates where new nodes should be added in relation to some other node.
// port: ClosureRewriteModule.AddAt
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AddAt {
    BEFORE,
    AFTER,
}

// port: ClosureRewriteModule.ScopeType
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScopeType {
    EXEC_CONTEXT,
    BLOCK,
}

/// Describes the context of an "unrecognized require" scenario so that it will be possible to
/// categorize and report it as either a "not provided yet" or "not provided at all" error at the
/// end.
// port: ClosureRewriteModule.UnrecognizedRequire
struct UnrecognizedRequire {
    // A goog.require() call, or a goog.module.get() call.
    require_node: NodeId,
    namespace_id: JsString,
}

impl UnrecognizedRequire {
    // port: ClosureRewriteModule.UnrecognizedRequire#UnrecognizedRequire
    fn new(require_node: NodeId, namespace_id: JsString) -> Self {
        Self {
            require_node,
            namespace_id,
        }
    }
}

// port: ClosureRewriteModule.ExportDefinition#INLINABLE_NAME_PARENTS
const INLINABLE_NAME_PARENTS: [Token; 5] = [
    Token::VAR,
    Token::CONST,
    Token::LET,
    Token::FUNCTION,
    Token::CLASS,
];

/// Java shares one ExportDefinition object between `defaultExport` and `exportsToInline`; its
/// fields are only written by `newNamedExport`, so a copy behaves identically.
// port: ClosureRewriteModule.ExportDefinition
#[derive(Clone, Default)]
struct ExportDefinition {
    // Null if the export is a default export (exports = expr)
    export_name: Option<JsString>,
    // Null if the export is of a @typedef
    rhs: Option<NodeId>,
    // Null if the export is of anything other than a name
    name_decl: Option<VarId>,
}

impl ExportDefinition {
    // port: ClosureRewriteModule.ExportDefinition#toString
    fn to_string(&self, compiler: &AbstractCompiler) -> String {
        // MoreObjects.toStringHelper(this).add(...).omitNullValues()
        let mut parts = Vec::new();
        if let Some(export_name) = &self.export_name {
            parts.push(format!("exportName={}", export_name.to_string_lossy()));
        }
        if let Some(rhs) = self.rhs {
            parts.push(format!("rhs={}", rhs.to_string(compiler)));
        }
        if let Some(name_decl) = self.name_decl {
            parts.push(format!("nameDecl={}", name_decl.to_string(compiler)));
        }
        format!("ExportDefinition{{{}}}", parts.join(", "))
    }

    // port: ClosureRewriteModule.ExportDefinition#newDefaultExport
    fn new_default_export(t: &mut NodeTraversal<'_>, rhs: NodeId) -> Self {
        Self::new_named_export(t, None, Some(rhs))
    }

    // port: ClosureRewriteModule.ExportDefinition#newNamedExport
    fn new_named_export(
        t: &mut NodeTraversal<'_>,
        name: Option<JsString>,
        rhs: Option<NodeId>,
    ) -> Self {
        let mut new_export = ExportDefinition {
            export_name: name,
            rhs,
            ..Default::default()
        };
        if let Some(rhs) = rhs
            && (rhs.is_name(t) || rhs.is_string_key(t))
        {
            let scope = t.get_scope();
            let rhs_name = rhs.get_string(t);
            new_export.name_decl = scope.get_var(t.get_compiler(), rhs_name);
        }
        new_export
    }

    // port: ClosureRewriteModule.ExportDefinition#getExportPostfix
    fn get_export_postfix(&self) -> JsString {
        match &self.export_name {
            None => JsString::from(""),
            Some(export_name) => JsString::from(".").concat(export_name),
        }
    }

    // port: ClosureRewriteModule.ExportDefinition#hasInlinableName
    fn has_inlinable_name(
        &self,
        compiler: &AbstractCompiler,
        exported_names: &IndexMap<VarId, ExportDefinition>,
    ) -> bool {
        let Some(name_decl) = self.name_decl else {
            return false;
        };
        let name_decl_parent = name_decl.get_parent_node(compiler).unwrap();
        if exported_names.contains_key(&name_decl)
            || !INLINABLE_NAME_PARENTS.contains(&name_decl_parent.get_token(compiler))
            || NodeUtil::is_function_declaration(compiler, name_decl_parent)
        {
            return false;
        }
        let initial_value = name_decl.get_initial_value(compiler);
        let Some(initial_value) = initial_value.filter(|v| v.is_call(compiler)) else {
            return true;
        };
        let method = initial_value.get_first_child(compiler).unwrap();
        if !method.is_get_prop(compiler) {
            return true;
        }
        let maybe_goog = method.get_first_child(compiler).unwrap();
        if !maybe_goog.is_name(compiler) || maybe_goog.get_string_ref(compiler) != "goog" {
            return true;
        }
        let name = method.get_string(compiler);
        name != "require" && name != "forwardDeclare" && name != "getMsg"
    }

    // port: ClosureRewriteModule.ExportDefinition#getLocalName
    fn get_local_name(&self, compiler: &AbstractCompiler) -> Option<JsString> {
        self.name_decl.map(|name_decl| name_decl.get_name(compiler))
    }
}

// port: ClosureRewriteModule.AliasName
struct AliasName {
    new_name: JsString,
    namespace_id: Option<JsString>, // non-null only if this is an alias of a module itself
}

impl AliasName {
    // port: ClosureRewriteModule.AliasName#AliasName
    fn new(new_name: JsString, namespace_id: Option<JsString>) -> Self {
        Self {
            new_name,
            namespace_id,
        }
    }
}

/// Handle of a ScriptDescription in the pass's arena: Java shares ScriptDescription objects
/// between the script stack, the child script queues and the global rewrite state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ScriptId(usize);

// port: ClosureRewriteModule.ScriptDescription
#[derive(Default)]
struct ScriptDescription {
    is_module: bool,
    declare_legacy_namespace: bool,
    namespace_id: Option<JsString>,      // "a.b.c"
    contents_prefix: Option<JsString>,   // "module$contents$a$b$c_
    top_level_names: IndexSet<JsString>, // For prefixed content renaming.
    child_scripts: VecDeque<ScriptId>,
    names_to_inline_by_alias: IndexMap<JsString, AliasName>, // For alias inlining.

    // Transient state.
    will_create_exports_object: bool,

    has_created_export_object: bool,
    default_export: Option<ExportDefinition>,
    default_export_local_name: Option<JsString>,
    named_exports: IndexSet<JsString>,
    exports_to_inline: IndexMap<VarId, ExportDefinition>,

    // The root of the module. The MODULE_BODY node that contains the module contents.
    // For recognizing top level names.
    root_node: Option<NodeId>,
}

impl ScriptDescription {
    // port: ClosureRewriteModule.ScriptDescription#addChildScript
    fn add_child_script(&mut self, child_script: ScriptId) {
        self.child_scripts.push_back(child_script);
    }

    // port: ClosureRewriteModule.ScriptDescription#removeFirstChildScript
    fn remove_first_child_script(&mut self) -> ScriptId {
        self.child_scripts
            .pop_front()
            .expect("java.util.NoSuchElementException")
    }

    // "module$exports$a$b$c" for non-legacy modules
    // port: ClosureRewriteModule.ScriptDescription#getBinaryNamespace
    fn get_binary_namespace(&self) -> Option<JsString> {
        if !self.is_module || self.declare_legacy_namespace {
            return None;
        }
        Some(ClosureRewriteModule::get_binary_module_namespace(
            self.namespace_id
                .as_ref()
                .expect("java.lang.NullPointerException"),
        ))
    }

    // port: ClosureRewriteModule.ScriptDescription#getExportedNamespace
    fn get_exported_namespace(&self) -> Option<JsString> {
        if self.declare_legacy_namespace {
            return self.namespace_id.clone();
        }
        self.get_binary_namespace()
    }
}

/// Java's `String + String` with a nullable left operand: null prints as "null".
fn concat_nullable(a: Option<&JsString>, b: &JsString) -> JsString {
    match a {
        Some(a) => a.concat(b),
        None => JsString::from("null").concat(b),
    }
}

/// Java `Object#toString` of a ScriptDescription, used only in failed precondition messages.
const SCRIPT_DESCRIPTION_TO_STRING: &str =
    "com.google.javascript.jscomp.ClosureRewriteModule$ScriptDescription";

// Global state tracking an association between the dotted names of goog.module()s and whether
// the goog.module declares itself as a legacy namespace.
// Allows for detecting duplicate goog.module()s and for rewriting fully qualified
// JsDoc type references to goog.module() types in legacy scripts.
// port: ClosureRewriteModule.GlobalRewriteState
#[derive(Default)]
struct GlobalRewriteState {
    script_descriptions_by_goog_module_namespace: IndexMap<JsString, ScriptId>,
    // A HashMultimap that is only ever written to; no iteration order is observable.
    namespace_ids_by_script_node: IndexMap<NodeId, IndexSet<JsString>>,
    provided_namespaces: IndexSet<JsString>,
}

impl GlobalRewriteState {
    // port: ClosureRewriteModule.GlobalRewriteState#containsModule
    fn contains_module(&self, namespace_id: &JsString) -> bool {
        self.script_descriptions_by_goog_module_namespace
            .contains_key(namespace_id)
    }

    // port: ClosureRewriteModule.GlobalRewriteState#isLegacyModule
    fn is_legacy_module(&self, scripts: &[ScriptDescription], namespace_id: &JsString) -> bool {
        check_argument!(self.contains_module(namespace_id));
        scripts[self.script_descriptions_by_goog_module_namespace[namespace_id].0]
            .declare_legacy_namespace
    }

    // port: ClosureRewriteModule.GlobalRewriteState#getBinaryNamespace
    fn get_binary_namespace(
        &self,
        scripts: &[ScriptDescription],
        namespace_id: &JsString,
    ) -> Option<JsString> {
        let script = self
            .script_descriptions_by_goog_module_namespace
            .get(namespace_id);
        script.and_then(|script| scripts[script.0].get_binary_namespace())
    }

    /// Returns the type of a goog.require of the given goog.module, or null if not a module.
    // port: ClosureRewriteModule.GlobalRewriteState#getGoogModuleNamespaceType
    fn get_goog_module_namespace_type(
        &self,
        ast: &Ast,
        scripts: &[ScriptDescription],
        namespace_id: &JsString,
    ) -> Option<closure_rhino::jstype::TypeId> {
        let goog_module = self
            .script_descriptions_by_goog_module_namespace
            .get(namespace_id);
        goog_module
            .and_then(|goog_module| scripts[goog_module.0].root_node.unwrap().get_jstype(ast))
    }

    // port: ClosureRewriteModule.GlobalRewriteState#getExportedNamespaceOrScript
    fn get_exported_namespace_or_script(
        &self,
        scripts: &[ScriptDescription],
        namespace_id: &JsString,
    ) -> Option<JsString> {
        if self.provided_namespaces.contains(namespace_id) {
            return Some(namespace_id.clone());
        }
        let script = self
            .script_descriptions_by_goog_module_namespace
            .get(namespace_id);
        script.and_then(|script| scripts[script.0].get_exported_namespace())
    }
}

/// Rust-only: Java passes the nullable `TypedScope globalTypedScope` as AstFactory's
/// `StaticScope` argument. A null scope is only dereferenced when AstFactory needs the slot (typed,
/// colored or normalized mode), where Java throws a NullPointerException.
struct NullableTypedScope(Option<TypedScope>);

impl AstFactoryStaticScope<AbstractCompiler> for NullableTypedScope {
    fn get_slot_declaration_node(
        &self,
        cx: &mut AbstractCompiler,
        name: &JsString,
    ) -> Option<Option<NodeId>> {
        let scope = self.0.expect("java.lang.NullPointerException");
        let var = scope.get_slot(cx, name)?;
        Some(
            var.get_declaration(cx)
                .and_then(|declaration| declaration.get_node(cx)),
        )
    }
}

// port: ClosureRewriteModule
pub struct ClosureRewriteModule {
    ast_factory: AstFactory,
    preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,
    preserve_sugar: bool,
    synthetic_externs: IndexMap<JsString, NodeId>,

    global_scope: Option<ScopeId>, // non-final because it must be set after process() is called

    // Per script state needed for rewriting.
    script_stack: VecDeque<ScriptId>,
    current_script: Option<ScriptId>,

    rewrite_state: GlobalRewriteState,
    // All prefix namespaces from goog.provides and legacy goog.modules.
    legacy_script_namespaces_and_prefixes: IndexSet<JsString>,
    unrecognized_requires: Vec<UnrecognizedRequire>,
    goog_module_get_calls: Vec<NodeId>,
    goog_require_dynamic_calls: Vec<NodeId>,

    global_typed_scope: Option<TypedScope>,

    /// Rust-only arena of the ScriptDescription objects (Java object identity).
    scripts: Vec<ScriptDescription>,
    prebuilt: PrebuiltNodes,
}

// port: ClosureRewriteModule.ScriptPreprocessor
struct ScriptPreprocessor<'a> {
    outer: &'a mut ClosureRewriteModule,
}

impl Callback for ScriptPreprocessor<'_> {
    // port: ClosureRewriteModule.ScriptPreprocessor#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::ROOT | Token::MODULE_BODY => true,
            Token::SCRIPT => {
                if NodeUtil::is_goog_module_file(t, n) {
                    ClosureRewriteModule::check_and_set_strict_mode_directive(t, n);
                }
                true
            }
            Token::NAME => {
                self.outer.preprocess_export_declaration(t, n);
                true
            }
            // Don't traverse into non-module scripts.
            _ => !parent.unwrap().is_script(t),
        }
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

// port: ClosureRewriteModule.ScriptRecorder
struct ScriptRecorder<'a> {
    outer: &'a mut ClosureRewriteModule,
}

impl Callback for ScriptRecorder<'_> {
    // port: ClosureRewriteModule.ScriptRecorder#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        let outer = &mut *self.outer;
        match n.get_token(t) {
            Token::MODULE_BODY => outer.record_module_body(n),
            Token::CALL => {
                let method = n.get_first_child(t).unwrap();
                if method.is_get_prop(t) {
                    let p = &outer.prebuilt;
                    if method.matches_qualified_name_node(t, p.goog_module) {
                        outer.record_goog_module(t, n);
                    } else if method
                        .matches_qualified_name_node(t, p.goog_module_declarelegacynamespace)
                    {
                        outer.record_goog_declare_legacy_namespace();
                    } else if method.matches_qualified_name_node(t, p.goog_provide) {
                        outer.record_goog_provide(t, n);
                    } else if method.matches_qualified_name_node(t, p.goog_require) {
                        outer.record_goog_require(t, n);
                    } else if method.matches_qualified_name_node(t, p.goog_requiretype) {
                        outer.record_goog_require_type(t, n);
                    } else if method.matches_qualified_name_node(t, p.goog_requiredynamic) {
                        outer.record_goog_require_dynamic(t, n);
                    } else if method.matches_qualified_name_node(t, p.goog_forwarddeclare)
                        && !parent.unwrap().is_expr_result(t)
                    {
                        outer.record_goog_forward_declare(t, n);
                    } else if method.matches_qualified_name_node(t, p.goog_module_get) {
                        outer.record_goog_module_get(t, n);
                    }
                }
            }
            Token::CLASS | Token::FUNCTION => {
                if outer.is_top_level(t, n, ScopeType::BLOCK) {
                    outer.record_top_level_class_or_function_name(t, n);
                }
            }
            Token::CONST | Token::LET | Token::VAR => {
                let scope_type = if n.is_var(t) {
                    ScopeType::EXEC_CONTEXT
                } else {
                    ScopeType::BLOCK
                };
                if outer.is_top_level(t, n, scope_type) {
                    outer.record_top_level_var_names(t, n);
                }
            }
            Token::GETPROP => {
                if ClosureRewriteModule::is_export_property_assignment(t, n) {
                    outer.record_exports_property_assignment(t, n);
                }
            }
            Token::NAME => outer.maybe_record_export_declaration(t, n),
            _ => {}
        }

        true
    }

    // port: ClosureRewriteModule.ScriptRecorder#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_module_body(t) {
            self.outer.pop_script();
        }
    }
}

// port: ClosureRewriteModule.ScriptUpdater
struct ScriptUpdater<'a> {
    outer: &'a mut ClosureRewriteModule,
    script_descriptions: VecDeque<ScriptId>,
}

impl Callback for ScriptUpdater<'_> {
    // port: ClosureRewriteModule.ScriptUpdater#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        let outer = &mut *self.outer;
        match n.get_token(t) {
            Token::SCRIPT => {
                let current_description = self
                    .script_descriptions
                    .pop_front()
                    .expect("java.util.NoSuchElementException");
                check_state!(outer.scripts[current_description.0].root_node == Some(n));
                if n.is_from_externs(t) && !NodeUtil::is_from_type_summary(t, n) {
                    return false;
                }

                check_state!(outer.script_stack.is_empty());
                outer.push_script(current_description);

                // Capture the scope before doing any rewriting to the scope.
                t.get_scope();

                // Capture the global scope for later reference.
                if outer.global_scope.is_none() {
                    let scope = t.get_scope();
                    outer.global_scope = Some(scope.get_global_scope(t.get_compiler()));
                }
            }
            Token::MODULE_BODY => {
                if parent.unwrap().get_boolean_prop(t, Prop::GOOG_MODULE) {
                    outer.update_module_body_early(n);
                } else {
                    return false;
                }
            }
            Token::CALL => {
                let method = n.get_first_child(t).unwrap();
                if method.is_get_prop(t) {
                    let p = &outer.prebuilt;
                    if method.matches_qualified_name_node(t, p.goog_module) {
                        outer.update_goog_module(t, n);
                    } else if method
                        .matches_qualified_name_node(t, p.goog_module_declarelegacynamespace)
                    {
                        ClosureRewriteModule::update_goog_declare_legacy_namespace(t, n);
                    } else if method.matches_qualified_name_node(t, p.goog_require)
                        || method.matches_qualified_name_node(t, p.goog_requiretype)
                    {
                        outer.update_goog_require(t, n);
                    } else if method.matches_qualified_name_node(t, p.goog_forwarddeclare)
                        && !parent.unwrap().is_expr_result(t)
                    {
                        outer.update_goog_forward_declare(t, n);
                    } else if GOOG_MODULE_PREVENTMODULEEXPORTSEALING.matches(t, method) {
                        ClosureRewriteModule::update_goog_prevent_module_exports_sealing(t, n);
                    }
                }
            }
            Token::GETPROP => {
                if ClosureRewriteModule::is_export_property_assignment(t, n) {
                    outer.update_exports_property_assignment(n, t);
                }
            }
            _ => {}
        }

        if let Some(info) = n.get_jsdoc_info(t) {
            let scope = t.get_scope();
            outer.rewrite_jsdoc(t.get_compiler(), &info, scope);
        }

        true
    }

    // port: ClosureRewriteModule.ScriptUpdater#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let outer = &mut *self.outer;
        match n.get_token(t) {
            Token::MODULE_BODY => outer.update_module_body(t.get_compiler(), n),
            Token::NAME => {
                outer.maybe_update_top_level_name(t, n);
                outer.maybe_update_export_declaration(t, n);
                t.get_scope(); // Creating the scope here has load-bearing side-effects.
                outer.maybe_update_export_name_ref(t, n);
            }
            Token::SCRIPT => {
                check_state!(outer.cur().root_node == Some(n));
                outer.pop_script();
            }
            _ => {}
        }
    }
}

// port: ClosureRewriteModule.UnwrapGoogLoadModule
struct UnwrapGoogLoadModule<'a> {
    outer: &'a mut ClosureRewriteModule,
}

impl Callback for UnwrapGoogLoadModule<'_> {
    // port: ClosureRewriteModule.UnwrapGoogLoadModule#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::ROOT | Token::SCRIPT => true,
            Token::EXPR_RESULT => {
                let call = n.get_first_child(t).unwrap();
                if NodeUtil::is_call_to_node(t, call, self.outer.prebuilt.goog_loadmodule)
                    && call.get_last_child(t).unwrap().is_function(t)
                {
                    let parent = parent.unwrap();
                    parent.put_boolean_prop(t, Prop::GOOG_MODULE, true);
                    let function_node = call.get_last_child(t).unwrap();
                    t.get_compiler().report_function_deleted(function_node);
                    let module_body = function_node.get_last_child(t).unwrap().detach(t);
                    module_body.set_token(t, Token::MODULE_BODY);
                    let exports_parameter =
                        NodeUtil::get_function_parameters(t, function_node).get_only_child(t);
                    let exports_type = exports_parameter.get_jstype(t);
                    module_body.set_jstype(t, exports_type);
                    n.replace_with(t, module_body);
                    let return_node = module_body.get_last_child(t).unwrap();
                    if !return_node.is_return(t) {
                        let error =
                            JSError::make(t, module_body, &LOAD_MODULE_FN_MISSING_RETURN, &[]);
                        t.get_compiler().report(error);
                    } else {
                        return_node.detach(t);
                    }
                    t.report_code_change();
                }
                false
            }
            _ => false,
        }
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

impl ClosureRewriteModule {
    // port: ClosureRewriteModule#ClosureRewriteModule
    pub fn new(
        compiler: &mut AbstractCompiler,
        preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,
        global_typed_scope: Option<TypedScope>,
    ) -> Self {
        check_argument!(global_typed_scope.is_none_or(|scope| scope.is_global(compiler)));

        let ast_factory = compiler.create_ast_factory();
        let preserve_sugar = compiler.get_options().should_preserve_goog_module();
        let prebuilt = PrebuiltNodes::new(compiler);
        Self {
            ast_factory,
            preprocessor_symbol_table,
            preserve_sugar,
            synthetic_externs: IndexMap::<_, _>::default(),
            global_scope: None,
            script_stack: VecDeque::new(),
            current_script: None,
            rewrite_state: GlobalRewriteState::default(),
            legacy_script_namespaces_and_prefixes: IndexSet::<_>::default(),
            unrecognized_requires: Vec::new(),
            goog_module_get_calls: Vec::new(),
            goog_require_dynamic_calls: Vec::new(),
            global_typed_scope,
            scripts: Vec::new(),
            prebuilt,
        }
    }

    /// `new ScriptDescription()`: allocates the description in the arena.
    fn new_script_description(&mut self) -> ScriptId {
        self.scripts.push(ScriptDescription::default());
        ScriptId(self.scripts.len() - 1)
    }

    /// `currentScript` (Java dereferences it, so it must be set).
    fn cur(&self) -> &ScriptDescription {
        &self.scripts[self
            .current_script
            .expect("java.lang.NullPointerException")
            .0]
    }

    fn cur_mut(&mut self) -> &mut ScriptDescription {
        let id = self.current_script.expect("java.lang.NullPointerException");
        &mut self.scripts[id.0]
    }

    // port: ClosureRewriteModule#getBinaryModuleNamespace
    pub fn get_binary_module_namespace(namespace_id: &JsString) -> JsString {
        JsString::from(MODULE_EXPORTS_PREFIX)
            .concat(&namespace_id.replace(&".".into(), &"$".into()))
    }

    /// Rewrites JsDoc type references to match AST changes resulting from imported alias
    /// inlining, module content renaming of top level constructor functions and classes, and
    /// module renaming from fully qualified legacy namespace to its binary name.
    // port: ClosureRewriteModule#rewriteJsdoc
    fn rewrite_jsdoc(&mut self, compiler: &mut AbstractCompiler, info: &JSDocInfo, scope: ScopeId) {
        let replacer = ReplaceJsDocRefs::new(scope);
        for type_node in info.get_type_nodes() {
            // NodeUtil.visitPreOrder(typeNode, replacer): the visitor only changes node strings,
            // so the pre-order list is collected first and visited with access to the compiler.
            let mut nodes = Vec::new();
            NodeUtil::visit_pre_order(compiler, type_node, &mut |_: &mut Ast, n: NodeId| {
                nodes.push(n)
            });
            for node in nodes {
                replacer.visit(self, compiler, node);
            }
        }
    }

    // port: ClosureRewriteModule#isWeakNamespace
    fn is_weak_namespace(&self, compiler: &AbstractCompiler, namespace_id: &JsString) -> bool {
        let metadata = compiler
            .get_module_metadata_map()
            .unwrap()
            .get_modules_by_goog_namespace()
            .get(namespace_id)
            .cloned();
        metadata.is_some_and(|metadata| {
            metadata
                .root_node()
                .unwrap()
                .get_static_source_file(compiler)
                .unwrap()
                .is_weak()
        })
    }

    // port: ClosureRewriteModule#isUnrequiredWeakNamespace
    fn is_unrequired_weak_namespace(
        &self,
        compiler: &AbstractCompiler,
        call: NodeId,
        namespace_id: &JsString,
    ) -> bool {
        if call.get_static_source_file(compiler).unwrap().is_weak()
            || !self.is_weak_namespace(compiler, namespace_id)
        {
            return false;
        }
        let caller_metadata = call.get_source_file_name(compiler).and_then(|name| {
            compiler
                .get_module_metadata_map()
                .unwrap()
                .get_modules_by_path()
                .get(&name)
                .cloned()
        });
        caller_metadata.is_none_or(|caller_metadata| {
            !caller_metadata
                .strongly_required_goog_namespaces()
                .contains(namespace_id)
        })
    }

    // port: ClosureRewriteModule#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Record all the scripts first so that the googModuleNamespaces global state can be
        // complete before doing any updating also queue up scriptDescriptions for later use in
        // ScriptUpdater runs.

        let mut script_descriptions: VecDeque<ScriptId> = VecDeque::new();
        for parent in [externs, root] {
            let mut script = parent.get_first_child(compiler);
            while let Some(s) = script {
                check_state!(s.is_script(compiler), "%s", s.to_string(compiler));
                NodeTraversal::traverse(compiler, s, &mut UnwrapGoogLoadModule { outer: self });

                let new_script = self.new_script_description();
                self.push_script(new_script); // sets currentScript

                self.cur_mut().root_node = Some(s);
                script_descriptions.push_back(self.current_script.unwrap());
                NodeTraversal::traverse(compiler, s, &mut ScriptPreprocessor { outer: self });
                NodeTraversal::traverse(compiler, s, &mut ScriptRecorder { outer: self });
                self.pop_script();
                script = s.get_next(compiler);
            }
        }

        self.report_unrecognized_requires(compiler);
        if compiler.has_halting_errors() {
            return;
        }

        // Update scripts using the now complete googModuleNamespaces global state and unspool
        // the scriptDescriptions that were queued up by all the recording.

        NodeTraversal::traverse_roots(
            compiler,
            // port: ClosureRewriteModule.ScriptUpdater#ScriptUpdater
            &mut ScriptUpdater {
                outer: self,
                script_descriptions,
            },
            externs,
            root,
        );
        self.declare_synthetic_externs(compiler);
        for call in self.goog_module_get_calls.clone() {
            self.update_goog_module_get_call(compiler, call);
        }
        for call in self.goog_require_dynamic_calls.clone() {
            self.update_goog_require_dynamic_call(compiler, call);
        }
    }

    /// Declares `var foo;` in the externs for all `synthetic_externs` names that aren't already
    /// in the global scope.
    ///
    /// Only add externs in the error case where there is an unrecognized goog.require or
    /// goog.module.get. Clutz depends on the compiler deleting local name declarations that alias
    /// or destructure the invalid call from this file. In order to preserve AST validity, we
    /// instead declare the deleted name in the externs. (which is fine with Clutz, as it just
    /// ignores the synthetic externs)
    // port: ClosureRewriteModule#declareSyntheticExterns
    fn declare_synthetic_externs(&mut self, compiler: &mut AbstractCompiler) {
        let mut vars = Vec::new();
        for lhs in self.synthetic_externs.values().copied().collect::<Vec<_>>() {
            // Skip roots of goog.provide or goog.module.declareLegacyNamespace();
            let lhs_name = lhs.get_string(compiler);
            if self.is_name_in_global_scope(compiler, &lhs_name) {
                continue;
            }
            let name = self
                .ast_factory
                .create_name_with_unknown_type(compiler, lhs_name);
            let var = IR::var(compiler, name).srcref_tree(compiler, lhs);
            vars.push(var);
        }

        if vars.is_empty() {
            return;
        }

        let input = compiler.get_synthesized_externs_input().clone();
        let root = input.get_ast_root(compiler);
        for var in vars {
            root.add_child_to_back(compiler, var);
        }
    }

    /// Returns whether the name is declared in the global scope, either explicitly with
    /// var/const/let or implicitly by a goog.provide or legacy goog.module.
    // port: ClosureRewriteModule#isNameInGlobalScope
    fn is_name_in_global_scope(&self, compiler: &mut AbstractCompiler, name: &JsString) -> bool {
        self.legacy_script_namespaces_and_prefixes.contains(name)
            || self
                .global_scope
                .expect("java.lang.NullPointerException")
                .get_var(compiler, name.clone())
                .is_some()
    }

    /// Rewrites object literal exports to the standard named exports style. i.e. exports = {Foo,
    /// Bar} to exports.Foo = Foo; exports.Bar = Bar; This makes the module exports into a more
    /// standard format for later passes.
    // port: ClosureRewriteModule#preprocessExportDeclaration
    fn preprocess_export_declaration(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !Self::is_goog_module_exports_ref(t, n)
            || !Self::is_assign_target(t, n)
            || !n.get_grandparent(t).unwrap().is_expr_result(t)
        {
            return;
        }

        check_state!(self.cur().default_export.is_none());
        let export_rhs = n.get_next(t).unwrap();
        if NodeUtil::is_named_exports_literal(t, export_rhs) {
            let mut insertion_point = n.get_grandparent(t).unwrap();
            let mut key = export_rhs.get_first_child(t);
            while let Some(k) = key {
                let export_name = k.get_string(t);
                let jsdoc = k.get_jsdoc_info(t);
                let rhs = k.remove_first_child(t).unwrap();
                let compiler = t.get_compiler();
                let n_type = AstFactory::type_node(n);
                let exports_name = self.ast_factory.create_name(compiler, "exports", n_type);
                let rhs_type = AstFactory::type_node(rhs);
                let lhs = self
                    .ast_factory
                    .create_get_prop(compiler, exports_name, export_name, rhs_type)
                    .srcref_tree(compiler, k);
                let assign = self
                    .ast_factory
                    .create_assign(compiler, lhs, rhs)
                    .srcref(compiler, k)
                    .set_jsdoc_info(compiler, jsdoc);
                let new_export = IR::expr_result(compiler, assign).srcref(compiler, k);
                new_export.insert_after(compiler, insertion_point);
                insertion_point = new_export;
                key = k.get_next(t);
            }
            n.get_grandparent(t).unwrap().detach(t);
        }
    }

    // port: ClosureRewriteModule#recordModuleBody
    fn record_module_body(&mut self, module_root: NodeId) {
        let new_script = self.new_script_description();
        self.push_script(new_script);

        self.cur_mut().root_node = Some(module_root);
        self.cur_mut().is_module = true;
    }

    // port: ClosureRewriteModule#recordGoogModule
    fn record_goog_module(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let namespace_id_node = call.get_last_child(t).unwrap();
        if !namespace_id_node.is_string_lit(t) {
            t.report(namespace_id_node, &INVALID_MODULE_ID_ARG, &[]);
            return;
        }
        let namespace_id = namespace_id_node.get_string(t);

        self.cur_mut().namespace_id = Some(namespace_id.clone());
        self.cur_mut().contents_prefix = Some(Self::to_module_contents_prefix(&namespace_id));

        let script_node = NodeUtil::get_enclosing_script(t, self.cur().root_node.unwrap()).unwrap();
        let current_script = self.current_script.unwrap();
        self.rewrite_state
            .script_descriptions_by_goog_module_namespace
            .insert(namespace_id.clone(), current_script);
        self.rewrite_state
            .namespace_ids_by_script_node
            .entry(script_node)
            .or_default()
            .insert(namespace_id);
    }

    // port: ClosureRewriteModule#recordGoogDeclareLegacyNamespace
    fn record_goog_declare_legacy_namespace(&mut self) {
        self.cur_mut().declare_legacy_namespace = true;
        let namespace_id = self.cur().namespace_id.clone();
        self.update_legacy_script_namespaces_and_prefixes(
            namespace_id.expect("java.lang.NullPointerException"),
        );
    }

    // port: ClosureRewriteModule#updateLegacyScriptNamespacesAndPrefixes
    fn update_legacy_script_namespaces_and_prefixes(&mut self, mut namespace: JsString) {
        self.legacy_script_namespaces_and_prefixes
            .insert(namespace.clone());
        let mut dot = namespace.last_index_of_char(u16::from(b'.'));
        while dot != -1 {
            namespace = namespace.substring(0, dot as usize);
            self.legacy_script_namespaces_and_prefixes
                .insert(namespace.clone());
            dot = namespace.last_index_of_char(u16::from(b'.'));
        }
    }

    // port: ClosureRewriteModule#recordGoogProvide
    fn record_goog_provide(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let namespace_id_node = call.get_last_child(t).unwrap();
        if !namespace_id_node.is_string_lit(t) {
            t.report(namespace_id_node, &INVALID_PROVIDE_NAMESPACE, &[]);
            return;
        }
        let namespace_id = namespace_id_node.get_string(t);

        if self.cur().is_module {
            t.report(namespace_id_node, &INVALID_PROVIDE_CALL, &[]);
        }

        let script_node = NodeUtil::get_enclosing_script(t, call).unwrap();
        // Log legacy namespaces and prefixes.
        self.rewrite_state
            .provided_namespaces
            .insert(namespace_id.clone());
        self.rewrite_state
            .namespace_ids_by_script_node
            .entry(script_node)
            .or_default()
            .insert(namespace_id.clone());
        self.update_legacy_script_namespaces_and_prefixes(namespace_id);
    }

    // port: ClosureRewriteModule#recordGoogRequire
    fn record_goog_require(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        Self::maybe_split_multi_var(t, call);

        let namespace_id_node = call.get_last_child(t).unwrap();
        if !namespace_id_node.is_string_lit(t) {
            t.report(namespace_id_node, &INVALID_REQUIRE_NAMESPACE, &[]);
            return;
        }
        let namespace_id = namespace_id_node.get_string(t);

        // Maybe report an error if there is an attempt to import something that is expected to
        // be a goog.module() but no such goog.module() has been defined.
        let target_is_a_module = self.rewrite_state.contains_module(&namespace_id);
        let target_is_a_legacy_script = self
            .rewrite_state
            .provided_namespaces
            .contains(&namespace_id);
        if self.cur().is_module && !target_is_a_module && !target_is_a_legacy_script {
            self.unrecognized_requires
                .push(UnrecognizedRequire::new(call, namespace_id));
        }
    }

    // port: ClosureRewriteModule#recordGoogRequireType
    fn record_goog_require_type(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let namespace_id_node = call.get_last_child(t).unwrap();
        if !namespace_id_node.is_string_lit(t) {
            t.report(namespace_id_node, &INVALID_REQUIRE_TYPE_NAMESPACE, &[]);
            return;
        }

        // For purposes of import collection, goog.requireType is the same as goog.require but
        // a goog.requireType call is not required to appear after the corresponding namespace
        // definition.
        self.record_goog_require(t, call);
    }

    // port: ClosureRewriteModule#recordGoogForwardDeclare
    fn record_goog_forward_declare(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let namespace_node = call.get_last_child(t).unwrap();
        if !call.has_two_children(t) || !namespace_node.is_string_lit(t) {
            t.report(namespace_node, &INVALID_FORWARD_DECLARE_NAMESPACE, &[]);
            return;
        }

        // For purposes of import collection, goog.forwardDeclare is the same as goog.require.
        self.record_goog_require(t, call);
    }

    // port: ClosureRewriteModule#recordGoogRequireDynamic
    fn record_goog_require_dynamic(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let namespace_id_node = call.get_last_child(t).unwrap();
        if !namespace_id_node.is_string_lit(t) {
            t.report(namespace_id_node, &INVALID_REQUIRE_DYNAMIC, &[]);
            return;
        }

        let namespace_id = namespace_id_node.get_string(t);

        if !self.rewrite_state.contains_module(&namespace_id) {
            self.unrecognized_requires
                .push(UnrecognizedRequire::new(call, namespace_id));
        }
        self.goog_require_dynamic_calls.push(call);
    }

    // port: ClosureRewriteModule#recordGoogModuleGet
    fn record_goog_module_get(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let namespace_id_node = call.get_last_child(t).unwrap();
        if !call.has_two_children(t) || !namespace_id_node.is_string_lit(t) {
            t.report(namespace_id_node, &INVALID_GET_NAMESPACE, &[]);
            return;
        }
        let namespace_id = namespace_id_node.get_string(t);

        if !self.rewrite_state.contains_module(&namespace_id) {
            self.unrecognized_requires
                .push(UnrecognizedRequire::new(call, namespace_id.clone()));
        }
        self.goog_module_get_calls.push(call);

        let maybe_assign = call.get_parent(t).unwrap();
        let is_filling_an_alias = maybe_assign.is_assign(t)
            && maybe_assign.get_first_child(t).unwrap().is_name(t)
            && maybe_assign.get_parent(t).unwrap().is_expr_result(t);
        if !is_filling_an_alias || !self.cur().is_module {
            return;
        }

        let alias_name = call
            .get_parent(t)
            .unwrap()
            .get_first_child(t)
            .unwrap()
            .get_string(t);

        // If the assignment isn't into a var in our scope then it's not ok.
        let scope = t.get_scope();
        let Some(alias_var) = scope.get_var(t.get_compiler(), alias_name) else {
            // Reported in CheckClosureImports
            return;
        };

        // Even if it was to a var in our scope it should still only rewrite if the var looked
        // like:
        //   let x = goog.forwardDeclare('a.namespace');
        let alias_var_node = alias_var.get_node(t.get_compiler()).unwrap();
        let alias_var_node_rhs = NodeUtil::get_r_value_of_l_value(t, alias_var_node);
        match alias_var_node_rhs {
            Some(rhs)
                if NodeUtil::is_call_to_node(t, rhs, self.prebuilt.goog_forwarddeclare)
                    && namespace_id == rhs.get_last_child(t).unwrap().get_string(t) => {}
            _ => {
                // Reported in CheckClosureImports
                return;
            }
        }

        if self.is_unrequired_weak_namespace(t.get_compiler(), call, &namespace_id) {
            return;
        }

        // Each goog.module.get() calling filling an alias will have the alias importing logic
        // handled at the goog.forwardDeclare call, and the corresponding goog.module.get can
        // simply be removed.
        t.get_compiler()
            .report_change_to_enclosing_scope(maybe_assign);
        maybe_assign.get_parent(t).unwrap().detach(t);
        self.goog_module_get_calls
            .remove(self.goog_module_get_calls.len() - 1);
    }

    // port: ClosureRewriteModule#recordTopLevelClassOrFunctionName
    fn record_top_level_class_or_function_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        class_or_function_node: NodeId,
    ) {
        let name_node = class_or_function_node.get_first_child(t).unwrap();
        if name_node.is_name(t) && !name_node.get_string_ref(t).is_empty() {
            let name = name_node.get_string(t);
            self.cur_mut().top_level_names.insert(name);
        }
    }

    // port: ClosureRewriteModule#recordTopLevelVarNames
    fn record_top_level_var_names(&mut self, t: &mut NodeTraversal<'_>, var_node: NodeId) {
        let current_script = self.current_script.expect("java.lang.NullPointerException");
        let scripts = &mut self.scripts;
        NodeUtil::visit_lhs_nodes_in_node(
            t.get_compiler(),
            var_node,
            &mut |compiler: &mut AbstractCompiler, lhs: NodeId| {
                scripts[current_script.0]
                    .top_level_names
                    .insert(lhs.get_string(compiler));
            },
        );
    }

    // port: ClosureRewriteModule#maybeRecordExportDeclaration
    fn maybe_record_export_declaration(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !self.cur().is_module
            || !Self::is_goog_module_exports_ref(t, n)
            || !Self::is_assign_target(t, n)
        {
            return;
        }

        // ClosureCheckModule reports an error for duplicate 'exports = ' assignments, but that
        // error may be suppressed. If so then use the final assignment as the canonical one.
        if let Some(previous_export) = self.cur().default_export.clone() {
            let local_name = previous_export.get_local_name(t.get_compiler());
            if let Some(local_name) = local_name
                && self
                    .cur()
                    .names_to_inline_by_alias
                    .contains_key(&local_name)
            {
                self.cur_mut()
                    .names_to_inline_by_alias
                    .shift_remove(&local_name);
            }
            self.cur_mut().default_export_local_name = None;
        }
        let export_rhs = n.get_next(t).unwrap();

        // Exports object should have already been converted in ScriptPreprocess step.
        check_state!(
            !NodeUtil::is_named_exports_literal(t, export_rhs),
            "Exports object should have been converted already"
        );

        self.cur_mut().will_create_exports_object = true;
        let default_export = ExportDefinition::new_default_export(t, export_rhs);
        self.cur_mut().default_export = Some(default_export.clone());
        if !self.cur().declare_legacy_namespace
            && default_export.has_inlinable_name(t.get_compiler(), &self.cur().exports_to_inline)
        {
            let local_name = default_export.get_local_name(t.get_compiler());
            self.cur_mut().default_export_local_name = local_name;
            self.record_export_to_inline(t.get_compiler(), default_export);
        }
    }

    // port: ClosureRewriteModule#updateModuleBodyEarly
    fn update_module_body_early(&mut self, module_scope_root: NodeId) {
        let child = self.cur_mut().remove_first_child_script();
        self.push_script(child);
        self.cur_mut().root_node = Some(module_scope_root);
    }

    // port: ClosureRewriteModule#updateGoogModule
    fn update_goog_module(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        if !self.cur().is_module {
            let compiler = t.get_compiler();
            compiler.report_change_to_enclosing_scope(call);
            let zero = self.ast_factory.create_number(compiler, 0.0);
            let undefined = self
                .ast_factory
                .create_void(compiler, zero)
                .srcref_tree(compiler, call);
            call.replace_with(compiler, undefined);
            return;
        }

        // If it's a goog.module() with a legacy namespace.
        if self.cur().declare_legacy_namespace {
            // Rewrite "goog.module('Foo');" as "goog.provide('Foo');".
            call.get_first_child(t).unwrap().set_string(t, "provide");
            t.get_compiler().report_change_to_enclosing_scope(call);
        }

        // If this script file isn't going to eventually create it's own exports object, then we
        // know we'll need to do it ourselves, and so we might as well create it as early as
        // possible to avoid ordering issues with goog.define().
        if !self.cur().will_create_exports_object {
            check_state!(
                !self.cur().has_created_export_object,
                "%s",
                SCRIPT_DESCRIPTION_TO_STRING
            );
            let statement = NodeUtil::get_enclosing_statement(t, call).unwrap();
            self.export_the_empty_binary_namespace_at(statement, AddAt::AFTER, t);
        }

        if !self.cur().declare_legacy_namespace && !self.preserve_sugar {
            // Otherwise it's a regular module and the goog.module() line can be removed.
            t.get_compiler().report_change_to_enclosing_scope(call);
            NodeUtil::get_enclosing_statement(t, call)
                .unwrap()
                .detach(t);
        }
    }

    // port: ClosureRewriteModule#updateGoogDeclareLegacyNamespace
    fn update_goog_declare_legacy_namespace(ast: &mut Ast, call: NodeId) {
        NodeUtil::get_enclosing_statement(ast, call)
            .unwrap()
            .detach(ast);
    }

    // port: ClosureRewriteModule#updateGoogPreventModuleExportsSealing
    fn update_goog_prevent_module_exports_sealing(ast: &mut Ast, call: NodeId) {
        NodeUtil::get_enclosing_statement(ast, call)
            .unwrap()
            .detach(ast);
    }

    // port: ClosureRewriteModule#updateGoogRequire
    fn update_goog_require(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let namespace_id_node = call.get_last_child(t).unwrap();
        let statement_node = NodeUtil::get_enclosing_statement(t, call).unwrap();
        let namespace_id = namespace_id_node.get_string(t);

        let target_is_non_legacy_goog_module = self.rewrite_state.contains_module(&namespace_id)
            && !self
                .rewrite_state
                .is_legacy_module(&self.scripts, &namespace_id);
        let import_has_alias = NodeUtil::is_name_declaration(t, Some(statement_node));
        let is_destructuring = statement_node
            .get_first_child(t)
            .unwrap()
            .is_destructuring_lhs(t);

        // If the current script is a module or the require statement has a return value that is
        // stored in an alias then the require is goog.module() style.
        let current_script_is_a_module = self.cur().is_module;
        // "var Foo = goog.require("bar.Foo");" or "const {Foo} = goog.require('bar');" style.
        let require_directly_stored_in_alias =
            NodeUtil::is_name_declaration(t, call.get_grandparent(t));
        if current_script_is_a_module
            && require_directly_stored_in_alias
            && self.is_top_level(t, statement_node, ScopeType::EXEC_CONTEXT)
        {
            // Record alias -> exportedNamespace associations for later inlining.
            let lhs = call.get_parent(t).unwrap();
            let exported_namespace = self
                .rewrite_state
                .get_exported_namespace_or_script(&self.scripts, &namespace_id);
            match exported_namespace {
                None => {
                    // There's nothing to inline. The missing provide/module will be reported
                    // elsewhere.
                }
                Some(exported_namespace) if lhs.is_name(t) => {
                    // `var Foo` case
                    let alias_name = statement_node.get_first_child(t).unwrap().get_string(t);
                    self.record_name_to_inline(
                        alias_name,
                        exported_namespace,
                        Some(namespace_id.clone()),
                    );
                    let module = self.cur().namespace_id.clone();
                    let alias_node = statement_node.get_first_child(t).unwrap();
                    self.maybe_add_alias_to_symbol_table(
                        t.get_compiler(),
                        alias_node,
                        module.as_ref(),
                    );
                }
                Some(exported_namespace)
                    if lhs.is_destructuring_lhs(t)
                        && lhs.get_first_child(t).unwrap().is_object_pattern(t) =>
                {
                    // `const {Foo}` case
                    self.maybe_warn_for_invalid_destructuring(
                        t,
                        lhs.get_parent(t).unwrap(),
                        &namespace_id,
                    );
                    let mut import_spec = lhs.get_first_first_child(t);
                    while let Some(spec) = import_spec {
                        check_state!(spec.has_children(t), "%s", spec.to_string(t));
                        let imported_property = spec.get_string(t);
                        let alias_node = spec.get_first_child(t).unwrap();
                        let alias_name = alias_node.get_string(t);
                        let full_name = exported_namespace
                            .concat(&".".into())
                            .concat(&imported_property);
                        self.record_name_to_inline(
                            alias_name.clone(),
                            full_name,
                            /* namespaceId= */ None,
                        );

                        // Record alias before we rename node.
                        let module = self.cur().namespace_id.clone();
                        self.maybe_add_alias_to_symbol_table(
                            t.get_compiler(),
                            alias_node,
                            module.as_ref(),
                        );
                        // Need to rename node otherwise it will stay global and messes up index
                        // if there are other files that use the same destructuring alias.
                        let new_string =
                            concat_nullable(self.cur().contents_prefix.as_ref(), &alias_name);
                        self.safe_set_string(t.get_compiler(), alias_node, new_string);
                        import_spec = spec.get_next(t);
                    }
                }
                Some(_) => {
                    panic!("Illegal goog.module import: {}", lhs.to_string(t));
                }
            }
        }

        if self.cur().is_module || target_is_non_legacy_goog_module {
            if is_destructuring {
                if !self.preserve_sugar {
                    // Delete the goog.require() because we're going to inline its alias later.
                    t.get_compiler()
                        .report_change_to_enclosing_scope(statement_node);
                    statement_node.detach(t);
                }
            } else if target_is_non_legacy_goog_module {
                check_state!(
                    self.is_top_level(t, statement_node, ScopeType::EXEC_CONTEXT),
                    "Unexpected non-top-level require at %s",
                    call.to_string(t)
                );
                if (import_has_alias
                    || !self
                        .rewrite_state
                        .is_legacy_module(&self.scripts, &namespace_id))
                    && !self.preserve_sugar
                {
                    // Delete the goog.require() because we're going to inline its alias later.
                    t.get_compiler()
                        .report_change_to_enclosing_scope(statement_node);
                    statement_node.detach(t);
                }
            } else {
                // TODO(bangert): make this compatible with preserveSugar. const B =
                // goog.require('b') runs into problems because the type checker cannot handle
                // const.
                // Rewrite
                //   "var B = goog.require('B');" to
                //   "goog.require('B');"
                // because even though we're going to inline the B alias,
                // ProcessClosurePrimitives is going to want to see this legacy require.
                call.detach(t);
                let expr_result = IR::expr_result(t, call);
                statement_node.replace_with(t, expr_result);
                t.get_compiler().report_change_to_enclosing_scope(call);
            }
        }
    }

    // These restrictions are in place to make it easier to migrate goog.modules to ES6 modules,
    // by structuring the imports/exports in a consistent way.
    // port: ClosureRewriteModule#maybeWarnForInvalidDestructuring
    fn maybe_warn_for_invalid_destructuring(
        &mut self,
        t: &mut NodeTraversal<'_>,
        import_node: NodeId,
        imported_namespace: &JsString,
    ) {
        check_argument!(
            import_node
                .get_first_child(t)
                .unwrap()
                .is_destructuring_lhs(t),
            "%s",
            import_node.to_string(t)
        );
        let Some(&imported_module) = self
            .rewrite_state
            .script_descriptions_by_goog_module_namespace
            .get(imported_namespace)
        else {
            // Don't know enough to give a good warning here.
            return;
        };
        if self.scripts[imported_module.0].default_export.is_some() {
            t.report(import_node, &ILLEGAL_DESTRUCTURING_DEFAULT_EXPORT, &[]);
            return;
        }
        let obj_pattern = import_node.get_first_first_child(t).unwrap();
        let mut key = obj_pattern.get_first_child(t);
        while let Some(k) = key {
            let export_name = k.get_string(t);
            if !self.scripts[imported_module.0]
                .named_exports
                .contains(&export_name)
            {
                t.report(
                    import_node,
                    &ILLEGAL_DESTRUCTURING_NOT_EXPORTED,
                    &[
                        &export_name.to_string_lossy(),
                        &imported_namespace.to_string_lossy(),
                    ],
                );
            }
            key = k.get_next(t);
        }
    }

    // port: ClosureRewriteModule#updateGoogForwardDeclare
    fn update_goog_forward_declare(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let namespace_id = call.get_last_child(t).unwrap().get_string(t);
        if self.is_unrequired_weak_namespace(t.get_compiler(), call, &namespace_id) {
            // For the delayed goog.module.get pattern where an alias variable is initialized by
            // goog.forwardDeclare('b') and later assigned by goog.module.get('b'):
            // When 'b' is in a weak chunk, the goog.module.get call is not inlined and is
            // rewritten to null. Therefore, the alias declaration must be initialized to null
            // rather than registered for inlining into references of the alias.
            let compiler = t.get_compiler();
            compiler.report_change_to_enclosing_scope(call);
            let null_node = self
                .ast_factory
                .create_null(compiler)
                .srcref(compiler, call);
            call.replace_with(compiler, null_node);
            return;
        }

        // For non-weak imports, goog.forwardDeclare is handled the same as goog.require:
        // the alias is recorded for inlining and the declaration statement is detached.
        self.update_goog_require(t, call);
    }

    // port: ClosureRewriteModule#updateGoogRequireDynamicCall
    fn update_goog_require_dynamic_call(&mut self, compiler: &mut AbstractCompiler, call: NodeId) {
        let Some(parent) = call.get_parent(compiler) else {
            // This call has been detached from the AST in unrecognized require handling. Nothing
            // else to do here.
            return;
        };
        check_state!(
            parent.is_await(compiler)
                || (parent.is_get_prop(compiler) && parent.get_string_ref(compiler) == "then"),
            "goog.requireDynamic() in only allowed in await/then expression"
        );

        if parent.is_await(compiler) {
            self.update_goog_require_dynamic_call_in_await(compiler, call);
        } else {
            self.update_goog_require_dynamic_call_in_then(compiler, call);
        }
    }

    // Rewrite
    //   goog.requireDynamic('a.b.c').then( ({Foo}) => { new Foo().render(); });
    //    to
    //   goog.importHandler_('h4sh').then(() => {
    //        const {Foo} = module$exports$a$b$c;
    //        new Foo().render();
    //   });
    // Note that rewriting works for both destructuring pattern and name, i.e., `{Foo}` or `foo`.
    // port: ClosureRewriteModule#updateGoogRequireDynamicCallInThen
    fn update_goog_require_dynamic_call_in_then(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
    ) {
        let namespace_id_node = call.get_second_child(compiler).unwrap();
        let namespace_id = namespace_id_node.get_string(compiler);
        let exported_namespace = self
            .rewrite_state
            .get_exported_namespace_or_script(&self.scripts, &namespace_id);
        check_state!(
            exported_namespace.is_some(),
            "Exported namespace for goog.requireDynamic() canot be null"
        );
        let exported_namespace = exported_namespace.unwrap();

        // `goog.requireDynamic('a.b.c').then()`
        let get_prop = call.get_parent(compiler).unwrap();
        let then_call_node = get_prop.get_parent(compiler);
        check_state!(
            then_call_node.is_some_and(|n| n.is_call(compiler)),
            "must be a 'then' call expression"
        );
        let then_call_node = then_call_node.unwrap();

        // Create a string literal containing the ID of the chunk containing the module we want
        let xid = self.namespace_id_to_xid(compiler, &namespace_id);
        let arg_node = self
            .ast_factory
            .create_string(compiler, xid)
            .srcref(compiler, namespace_id_node);

        // `goog.requireDynamic('a.b.c')` -> `goog.requireDynamic('chunkId')`
        namespace_id_node.replace_with(compiler, arg_node);

        // `goog.requireDynamic` -> `goog.importHandler_`
        let callee_node = call.get_first_child(compiler).unwrap();
        check_state!(
            callee_node.matches_qualified_name(compiler, GOOG_REQUIREDYNAMIC_NAME),
            "%s",
            callee_node.to_string(compiler)
        );
        callee_node.set_string(compiler, IMPORT_HANDLER_NAME);

        // `module$exports$a$b$c`
        let exported_namespace_name_node = self
            .ast_factory
            .create_name_in_scope(
                compiler,
                Some(&NullableTypedScope(self.global_typed_scope)),
                exported_namespace,
            )
            .srcref_tree(compiler, call);
        exported_namespace_name_node.set_original_name(compiler, Some(namespace_id.clone()));

        // Get `{Foo}` from `goog.requireDynamic().then(   ({Foo}) => {  }   )`;
        let function_node = then_call_node.get_second_child(compiler);
        check_state!(
            function_node.is_some_and(|n| n.is_function(compiler)),
            "must be a function in `then`"
        );
        let function_node = function_node.unwrap();

        // Get param list of the callback function
        let param_list_node = function_node.get_second_child(compiler).unwrap();
        check_state!(
            param_list_node.get_child_count(compiler) == 1,
            "function must have only one parameter"
        );

        // Get the callback function body
        let function_body = param_list_node.get_next(compiler).unwrap();

        // Get parameter, object pattern or name
        let object_pattern_or_name_node = param_list_node.get_only_child(compiler);
        check_state!(
            object_pattern_or_name_node.is_object_pattern(compiler)
                || object_pattern_or_name_node.is_name(compiler),
            "parameter of callback function must be object pattern or name"
        );
        object_pattern_or_name_node.detach(compiler);

        // `({Foo}) => {}` -> `() => {}`
        param_list_node.remove_children(compiler);

        // Dynamic require is not allowed for JS that is too old to have `const` (< ES6),
        // We don't expect to ever see `--language_in=ES5` in combination with dynamic require.
        let language_in = compiler.get_options().get_language_in();
        check_state!(
            language_in
                .to_feature_set()
                .contains(Feature::CONST_DECLARATIONS),
            "'%s' does not contain '%s'",
            language_in,
            Feature::CONST_DECLARATIONS
        );

        let enclosing_script = NodeUtil::get_enclosing_script(compiler, call).unwrap();
        NodeUtil::add_feature_to_script(compiler, enclosing_script, Feature::CONST_DECLARATIONS);

        // Create `const {Foo} = module$exports$a$b$c;`
        // Or `const foo = module$exports$a$b$c;'
        let declaration_node = if object_pattern_or_name_node.is_object_pattern(compiler) {
            self.ast_factory
                .create_single_const_object_pattern_declaration(
                    compiler,
                    object_pattern_or_name_node,
                    exported_namespace_name_node,
                )
                .srcref_tree_if_missing(compiler, call)
        } else {
            let name = object_pattern_or_name_node.get_string(compiler);
            self.ast_factory
                .create_single_const_name_declaration(compiler, name, exported_namespace_name_node)
                .srcref_tree_if_missing(compiler, call)
        };

        // If right hand side of the callback arrow function is an expression instead of BLOCK,
        // e.g., `({Foo}) => Foo` instead of `() => { return Foo; }`, create a BLOCK to host the
        // expression.
        if function_body.is_block(compiler) {
            function_body.add_child_to_front(compiler, declaration_node);
            compiler.report_change_to_enclosing_scope(declaration_node);
        } else {
            let detached_body = function_body.detach(compiler);
            let return_stmt = self.ast_factory.create_return(compiler, detached_body);
            let new_block = self
                .ast_factory
                .create_block(compiler, &[return_stmt])
                .srcref_tree(compiler, call);
            new_block.insert_after(compiler, param_list_node);
            new_block.add_child_to_front(compiler, declaration_node);
            compiler.report_change_to_enclosing_scope(new_block);
        }

        compiler.report_change_to_enclosing_scope(call);
    }

    // Rewrite
    //   const {Foo} = await goog.requireDynamic('a.b.c')`
    //    to
    //   await goog.importHandler_('tJJovc');
    //   const {Foo} = module$exports$a$b$c;
    // port: ClosureRewriteModule#updateGoogRequireDynamicCallInAwait
    fn update_goog_require_dynamic_call_in_await(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
    ) {
        let namespace_id_node = call.get_second_child(compiler).unwrap();
        let namespace_id = namespace_id_node.get_string(compiler);
        let exported_namespace = self
            .rewrite_state
            .get_exported_namespace_or_script(&self.scripts, &namespace_id);
        check_state!(
            exported_namespace.is_some(),
            "Exported namespace for goog.requireDynamic() cannot be null"
        );
        let exported_namespace = exported_namespace.unwrap();

        // `await goog.requireDynamic('a.b.c')`
        let existing_await = call.get_parent(compiler).unwrap();
        check_state!(
            existing_await.is_await(compiler),
            "Only goog.requireDynamic() in await expression is supported now"
        );

        // `{Foo} = await goog.requireDynamic('a.b.c')`
        let await_parent = existing_await.get_parent(compiler);
        if await_parent.is_none_or(|p| !p.is_destructuring_lhs(compiler) && !p.is_name(compiler)) {
            let error = JSError::make(
                compiler,
                call,
                &ILLEGAL_STMT_OF_GOOG_REQUIRE_DYNAMIC_IN_AWAIT,
                &[],
            );
            compiler.report(error);
        }

        // `const {Foo} = await goog.requireDynamic('a.b.c')`
        let declaration_statement = await_parent
            .expect("java.lang.NullPointerException")
            .get_parent(compiler)
            .unwrap();
        // Reject non-declarations or declarations in a for loop.
        if !NodeUtil::is_name_declaration(compiler, Some(declaration_statement))
            || !NodeUtil::is_statement(compiler, declaration_statement)
        {
            let error = JSError::make(
                compiler,
                call,
                &ILLEGAL_STMT_OF_GOOG_REQUIRE_DYNAMIC_IN_AWAIT,
                &[],
            );
            compiler.report(error);
        }

        // `module$exports$a$b$c`
        let exported_namespace_name_node = self
            .ast_factory
            .create_qname(
                compiler,
                &NullableTypedScope(self.global_typed_scope),
                &exported_namespace.to_string(),
            )
            .srcref_tree(compiler, call);
        let namespace_type = self.rewrite_state.get_goog_module_namespace_type(
            compiler,
            &self.scripts,
            &namespace_id,
        );
        exported_namespace_name_node.set_jstype(compiler, namespace_type);
        exported_namespace_name_node.set_original_name(compiler, Some(namespace_id.clone()));

        // Create a string literal containing the ID of the chunk containing the module we want
        let xid = self.namespace_id_to_xid(compiler, &namespace_id);
        let arg_node = self
            .ast_factory
            .create_string(compiler, xid)
            .srcref(compiler, namespace_id_node);

        // `goog.requireDynamic('a.b.c')` -> `goog.requireDynamic('chunkId')`
        namespace_id_node.replace_with(compiler, arg_node);

        // `goog.requireDynamic` -> `goog.importHandler_`
        let callee_node = call.get_first_child(compiler).unwrap();
        check_state!(
            callee_node.matches_qualified_name(compiler, GOOG_REQUIREDYNAMIC_NAME),
            "%s",
            callee_node.to_string(compiler)
        );
        callee_node.set_string(compiler, IMPORT_HANDLER_NAME);

        // Replace the await with the module object
        // `const {Foo} = module$exports$a$b$c;`
        existing_await.replace_with(compiler, exported_namespace_name_node);

        // Insert the await as a statement before the declaration
        let await_statement = self
            .ast_factory
            .expr_result(compiler, existing_await)
            .srcref(compiler, existing_await);
        await_statement.insert_before(compiler, declaration_statement);
        compiler.report_change_to_enclosing_scope(call);
    }

    // port: ClosureRewriteModule#namespaceIdToXid
    fn namespace_id_to_xid(&self, compiler: &AbstractCompiler, namespace_id: &JsString) -> String {
        let hash_function = compiler.get_options().get_chunk_id_hash_function().clone();
        let xid = match hash_function {
            None => Xid::new(),
            Some(hash_function) => Xid::with_hasher(hash_function),
        };
        xid.get(namespace_id.clone())
    }

    // port: ClosureRewriteModule#updateGoogModuleGetCall
    fn update_goog_module_get_call(&mut self, compiler: &mut AbstractCompiler, call: NodeId) {
        let namespace_id_node = call.get_second_child(compiler).unwrap();
        let namespace_id = namespace_id_node.get_string(compiler);

        if self.is_unrequired_weak_namespace(compiler, call, &namespace_id) {
            let error = JSError::make(
                compiler,
                call,
                &GOOG_MODULE_GET_OF_WEAK_MODULE,
                &[&namespace_id.to_string()],
            );
            compiler.report(error);
            compiler.report_change_to_enclosing_scope(call);
            let null_node = self
                .ast_factory
                .create_null(compiler)
                .srcref(compiler, call);
            call.replace_with(compiler, null_node);
            return;
        }

        // Remaining calls to goog.module.get() are not alias updates,
        // and should be replaced by a reference to the proper name.
        // Replace "goog.module.get('pkg.Foo')" with either "pkg.Foo" or "module$exports$pkg$Foo".
        let exported_namespace = self
            .rewrite_state
            .get_exported_namespace_or_script(&self.scripts, &namespace_id);
        if let Some(exported_namespace) = exported_namespace {
            compiler.report_change_to_enclosing_scope(call);
            let exported_namespace_name = self
                .ast_factory
                .create_qname_using_js_type_info(
                    compiler,
                    self.global_typed_scope,
                    &exported_namespace.to_string(),
                )
                .srcref_tree(compiler, call);
            let namespace_type = self.rewrite_state.get_goog_module_namespace_type(
                compiler,
                &self.scripts,
                &namespace_id,
            );
            exported_namespace_name.set_jstype(compiler, namespace_type);
            exported_namespace_name.set_original_name(compiler, Some(namespace_id));
            call.replace_with(compiler, exported_namespace_name);
        }
    }

    // port: ClosureRewriteModule#recordExportsPropertyAssignment
    fn record_exports_property_assignment(
        &mut self,
        t: &mut NodeTraversal<'_>,
        getprop_node: NodeId,
    ) {
        if !self.cur().is_module {
            return;
        }

        let parent = getprop_node.get_parent(t).unwrap();
        check_state!(
            parent.is_assign(t) || parent.is_expr_result(t),
            "%s",
            parent.to_string(t)
        );

        let exports_name_node = getprop_node.get_first_child(t).unwrap();
        check_state!(
            exports_name_node.get_string_ref(t) == "exports",
            "%s",
            exports_name_node.to_string(t)
        );

        if t.in_module_scope() {
            let export_name = getprop_node.get_string(t);
            self.cur_mut().named_exports.insert(export_name.clone());
            let export_rhs = getprop_node.get_next(t);
            let named_export = ExportDefinition::new_named_export(t, Some(export_name), export_rhs);
            if !self.cur().declare_legacy_namespace
                && self.cur().default_export.is_none()
                && named_export.has_inlinable_name(t.get_compiler(), &self.cur().exports_to_inline)
            {
                self.record_export_to_inline(t.get_compiler(), named_export);
                parent.get_parent(t).unwrap().detach(t);
            }
        }
    }

    // port: ClosureRewriteModule#updateExportsPropertyAssignment
    fn update_exports_property_assignment(
        &mut self,
        getprop_node: NodeId,
        t: &mut NodeTraversal<'_>,
    ) {
        if !self.cur().is_module {
            return;
        }

        let parent = getprop_node.get_parent(t).unwrap();
        check_state!(
            parent.is_assign(t) || parent.is_expr_result(t),
            "%s",
            parent.to_string(t)
        );

        // Update "exports.foo = Foo" to "module$exports$pkg$Foo.foo = Foo";
        let exports_name_node = getprop_node.get_first_child(t).unwrap();
        check_state!(exports_name_node.get_string_ref(t) == "exports");
        let exported_namespace = self.cur().get_exported_namespace();
        self.safe_set_maybe_qualified_string(
            t.get_compiler(),
            exports_name_node,
            exported_namespace,
            /* isModuleNamespace= */ false,
        );

        let jsdoc_node = if parent.is_assign(t) {
            parent
        } else {
            getprop_node
        };
        Self::mark_const_and_copy_js_doc(t, jsdoc_node, jsdoc_node);

        // When seeing the first "exports.foo = ..." line put a "var module$exports$pkg$Foo = {};"
        // before it.
        if !self.cur().has_created_export_object {
            let statement = NodeUtil::get_enclosing_statement(t, parent).unwrap();
            self.export_the_empty_binary_namespace_at(statement, AddAt::BEFORE, t);
        }
    }

    /// Rewrites top level var names from "var foo; console.log(foo);" to "var
    /// module$contents$Foo_foo; console.log(module$contents$Foo_foo);"
    // port: ClosureRewriteModule#maybeUpdateTopLevelName
    fn maybe_update_top_level_name(&mut self, t: &mut NodeTraversal<'_>, name_node: NodeId) {
        let name = name_node.get_string(t);
        if !self.cur().is_module || !self.cur().top_level_names.contains(&name) {
            return;
        }
        let scope = t.get_scope();
        let var = scope.get_var(t.get_compiler(), name.clone());
        // If the name refers to a var that is not from the top level scope.
        let root_node = self.cur().root_node;
        let Some(var) = var.filter(|var| {
            let compiler = &*t.get_compiler();
            Some(var.get_scope(compiler).get_root_node(compiler)) == root_node
        }) else {
            // Then it shouldn't be renamed.
            return;
        };

        // If the name is part of a destructuring import, the import rewriting will take care of
        // it
        if var.get_name_node(t.get_compiler()) == Some(name_node)
            && name_node.get_parent(t).unwrap().is_string_key(t)
            && name_node.get_grandparent(t).unwrap().is_object_pattern(t)
        {
            let destructuring_lhs_node =
                name_node.get_grandparent(t).unwrap().get_parent(t).unwrap();
            let last = destructuring_lhs_node.get_last_child(t).unwrap();
            if NodeUtil::is_call_to_node(t, last, self.prebuilt.goog_require)
                || NodeUtil::is_call_to_node(t, last, self.prebuilt.goog_requiretype)
            {
                return;
            }
        }

        // If the name is an alias for an imported namespace or an exported local, rewrite from
        // "new Foo;" to "new module$exports$Foo;" or "new Foo" to "new module$contents$bar$Foo".
        let name_is_an_alias = self.cur().names_to_inline_by_alias.contains_key(&name);
        if name_is_an_alias && var.get_node(t.get_compiler()) != Some(name_node) {
            let module = self.cur().namespace_id.clone();
            self.maybe_add_alias_to_symbol_table(t.get_compiler(), name_node, module.as_ref());

            let inline = &self.cur().names_to_inline_by_alias[&name];
            let namespace_to_inline = inline.new_name.clone();
            let inline_namespace_id = inline.namespace_id.clone();
            if Some(&namespace_to_inline) == self.cur().get_binary_namespace().as_ref() {
                self.cur_mut().has_created_export_object = true;
            }
            let is_module_namespace = inline_namespace_id.as_ref().is_some_and(|namespace_id| {
                self.rewrite_state
                    .script_descriptions_by_goog_module_namespace
                    .get(namespace_id)
                    .is_some_and(|script| !self.scripts[script.0].will_create_exports_object)
            });
            self.safe_set_maybe_qualified_string(
                t.get_compiler(),
                name_node,
                Some(namespace_to_inline.clone()),
                is_module_namespace,
            );

            // Make sure this action won't shadow a local variable.
            let dot = namespace_to_inline.index_of_char(u16::from(b'.'));
            if dot != -1 {
                let first_qualified_name = namespace_to_inline.substring(0, dot as usize);
                let scope = t.get_scope();
                let shadowed_var = scope.get_var(t.get_compiler(), first_qualified_name);
                let Some(shadowed_var) = shadowed_var else {
                    return;
                };
                let compiler = &*t.get_compiler();
                if shadowed_var.is_global(compiler)
                    || shadowed_var.get_scope(compiler).is_module_scope(compiler)
                {
                    return;
                }
                let shadowed_node = shadowed_var.get_node(compiler).unwrap();
                let shadowed_name = shadowed_var.get_name(compiler);
                t.report(
                    shadowed_node,
                    &IMPORT_INLINING_SHADOWS_VAR,
                    &[
                        &shadowed_name.to_string_lossy(),
                        &namespace_to_inline.to_string_lossy(),
                    ],
                );
            }
            return;
        }

        // For non-import alias names rewrite from
        // "var foo; console.log(foo);" to
        // "var module$contents$Foo_foo; console.log(module$contents$Foo_foo);"
        let new_string = concat_nullable(self.cur().contents_prefix.as_ref(), &name);
        self.safe_set_string(t.get_compiler(), name_node, new_string);
    }

    /// For exports like "exports = {prop: value}" update the declarations to enforce @const ness
    /// (and typedef exports).
    ///
    /// TODO(blickly): Remove as much of this functionality as possible, now that these style of
    /// exports are rewritten in ScriptPreprocess step.
    // port: ClosureRewriteModule#maybeUpdateExportObjectLiteral
    fn maybe_update_export_object_literal(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !self.cur().is_module {
            return;
        }

        let parent = n.get_parent(t).unwrap();
        let rhs = parent.get_last_child(t).unwrap();

        if rhs.is_object_lit(t) {
            let mut c = rhs.get_first_child(t);
            while let Some(child) = c {
                if child.is_computed_prop(t) {
                    t.report(child, &INVALID_EXPORT_COMPUTED_PROPERTY, &[]);
                } else if child.is_string_key(t) {
                    let value = child.get_first_child(t).unwrap();
                    self.maybe_update_export_decl_to_node(t, child, value);
                }
                c = child.get_next(t);
            }
        }
    }

    // port: ClosureRewriteModule#maybeUpdateExportDeclToNode
    fn maybe_update_export_decl_to_node(
        &mut self,
        t: &mut NodeTraversal<'_>,
        target: NodeId,
        value: NodeId,
    ) {
        if !self.cur().is_module {
            return;
        }

        // If the RHS is a typedef, clone the declaration.
        // Hack alert: clone the typedef declaration if one exists
        // this is a simple attempt that covers the common case of the
        // exports being in the same scope as the typedef declaration.
        // Otherwise the type name might be invalid.
        if value.is_name(t) {
            let current_scope = t.get_scope();
            let scope = t.get_scope();
            let value_name = value.get_string(t);
            let v = scope.get_var(t.get_compiler(), value_name);
            if let Some(v) = v {
                let compiler = &*t.get_compiler();
                let var_scope = v.get_scope(compiler);
                if var_scope.get_depth(compiler) == current_scope.get_depth(compiler) {
                    let info = v.get_jsdoc_info(compiler);
                    if let Some(info) = info
                        && info.has_typedef_type()
                    {
                        let mut builder = JSDocInfoBuilder::copy_from(&info);
                        let built = builder.build();
                        target.set_jsdoc_info(t, built);
                        return;
                    }
                }
            }
        }

        Self::mark_const_and_copy_js_doc(t, target, target);
    }

    /// In module "foo.Bar", rewrite "exports = Bar" to "var module$exports$foo$Bar = Bar".
    // port: ClosureRewriteModule#maybeUpdateExportDeclaration
    fn maybe_update_export_declaration(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !self.cur().is_module
            || !Self::is_goog_module_exports_ref(t, n)
            || !Self::is_assign_target(t, n)
        {
            return;
        }

        let assign_node = n.get_parent(t).unwrap();
        let rhs = assign_node.get_last_child(t).unwrap();
        if Some(rhs)
            != self
                .cur()
                .default_export
                .as_ref()
                .expect("java.lang.NullPointerException")
                .rhs
        {
            // This script has duplicate 'exports = ' assignments. Preserve the rhs as an
            // expression but don't declare it as a global variable.
            let detached = rhs.detach(t);
            assign_node.replace_with(t, detached);
            return;
        }

        if !self.cur().declare_legacy_namespace && self.cur().default_export_local_name.is_some() {
            assign_node.get_parent(t).unwrap().detach(t);

            let binary_namespace = self.cur().get_binary_namespace();
            let compiler = t.get_compiler();
            let n_type = AstFactory::type_node(n);
            let binary_namespace_name = self.ast_factory.create_name(
                compiler,
                binary_namespace.expect("java.lang.NullPointerException"),
                n_type,
            );
            self.declare_global_variable(binary_namespace_name, t);
            return;
        }

        // Rewrite "exports = ..." as "var module$exports$foo$Bar = ..."
        let jsdoc_node;
        if self.cur().declare_legacy_namespace {
            let namespace_id = self.cur().namespace_id.clone();
            let compiler = t.get_compiler();
            let legacy_qname = self
                .ast_factory
                .create_qname_using_js_type_info(
                    compiler,
                    self.global_typed_scope,
                    &namespace_id
                        .expect("java.lang.NullPointerException")
                        .to_string(),
                )
                .srcref_tree(compiler, n);
            let n_type = n.get_jstype(compiler);
            legacy_qname.set_jstype(compiler, n_type);
            n.replace_with(compiler, legacy_qname);
            jsdoc_node = assign_node;
        } else {
            rhs.detach(t);
            let expr_result_node = assign_node.get_parent(t).unwrap();
            let binary_namespace = self.cur().get_binary_namespace();
            let compiler = t.get_compiler();
            let n_type = AstFactory::type_node(n);
            let binary_namespace_name = self.ast_factory.create_name(
                compiler,
                binary_namespace.expect("java.lang.NullPointerException"),
                n_type,
            );
            binary_namespace_name.set_original_name(compiler, Some("exports".into()));
            self.declare_global_variable(binary_namespace_name, t);

            let exports_object_creation_node = IR::var_with_value(t, binary_namespace_name, rhs);
            exports_object_creation_node.srcref_tree_if_missing(t, expr_result_node);
            exports_object_creation_node.put_boolean_prop(t, Prop::IS_NAMESPACE, true);
            expr_result_node.replace_with(t, exports_object_creation_node);
            jsdoc_node = exports_object_creation_node;
            self.cur_mut().has_created_export_object = true;
        }
        Self::mark_const_and_copy_js_doc(t, assign_node, jsdoc_node);
        t.get_compiler()
            .report_change_to_enclosing_scope(jsdoc_node);

        self.maybe_update_export_object_literal(t, rhs);
    }

    // port: ClosureRewriteModule#maybeUpdateExportNameRef
    fn maybe_update_export_name_ref(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !self.cur().is_module
            || !Self::is_goog_module_exports_ref(t, n)
            || n.get_parent(t).is_none()
        {
            return;
        }
        if n.get_parent(t).unwrap().is_param_list(t) {
            return;
        }

        if self.cur().declare_legacy_namespace {
            let namespace_id = self.cur().namespace_id.clone();
            let compiler = t.get_compiler();
            let legacy_qname = self
                .ast_factory
                .create_qname(
                    compiler,
                    &NullableTypedScope(self.global_typed_scope),
                    &namespace_id
                        .expect("java.lang.NullPointerException")
                        .to_string(),
                )
                .srcref_tree(compiler, n);
            let n_type = n.get_jstype(compiler);
            legacy_qname.set_jstype(compiler, n_type);
            n.replace_with(compiler, legacy_qname);
            compiler.report_change_to_enclosing_scope(legacy_qname);
            return;
        }

        let binary_namespace = self.cur().get_binary_namespace();
        self.safe_set_string(
            t.get_compiler(),
            n,
            binary_namespace.expect("java.lang.NullPointerException"),
        );

        // Either this module is going to create it's own exports object at some point or else if
        // it's going to be defensively created automatically then that should have occurred at
        // the top of the file and been done by now.
        check_state!(self.cur().will_create_exports_object || self.cur().has_created_export_object);
    }

    // port: ClosureRewriteModule#updateModuleBody
    fn update_module_body(&mut self, compiler: &mut AbstractCompiler, module_body: NodeId) {
        check_argument!(
            module_body.is_module_body(compiler)
                && module_body
                    .get_parent(compiler)
                    .unwrap()
                    .get_boolean_prop(compiler, Prop::GOOG_MODULE),
            "%s",
            module_body.to_string(compiler)
        );
        module_body.set_token(compiler, Token::BLOCK);
        NodeUtil::try_merge_block(compiler, module_body, true);

        let exports: Vec<ExportDefinition> =
            self.cur().exports_to_inline.values().cloned().collect();
        for export in exports {
            let name_node = export.name_decl.unwrap().get_name_node(compiler).unwrap();
            let new_string = concat_nullable(
                self.cur().get_binary_namespace().as_ref(),
                &export.get_export_postfix(),
            );
            self.safe_set_maybe_qualified_string(compiler, name_node, Some(new_string), false);
        }
        check_state!(self.cur().is_module, "%s", SCRIPT_DESCRIPTION_TO_STRING);
        check_state!(
            self.cur().declare_legacy_namespace || self.cur().has_created_export_object,
            "%s",
            SCRIPT_DESCRIPTION_TO_STRING
        );

        self.pop_script();
    }

    /// Record the provided script as the current script at top of the script stack and add it as
    /// a child of the previous current script if there was one.
    ///
    /// Keeping track of the current script facilitates aggregation of accurate script state so
    /// that rewriting can run properly. Handles scripts and nested goog.modules.
    // port: ClosureRewriteModule#pushScript
    fn push_script(&mut self, new_current_script: ScriptId) {
        self.current_script = Some(new_current_script);
        if let Some(&parent_script) = self.script_stack.front() {
            self.scripts[parent_script.0].add_child_script(new_current_script);
        }
        self.script_stack.push_front(new_current_script);
    }

    // port: ClosureRewriteModule#popScript
    fn pop_script(&mut self) {
        self.script_stack
            .pop_front()
            .expect("java.util.NoSuchElementException");
        self.current_script = self.script_stack.front().copied();
    }

    /// Add the missing "var module$exports$pkg$Foo = {};" line.
    // port: ClosureRewriteModule#exportTheEmptyBinaryNamespaceAt
    fn export_the_empty_binary_namespace_at(
        &mut self,
        at_node: NodeId,
        add_at: AddAt,
        t: &mut NodeTraversal<'_>,
    ) {
        if self.cur().declare_legacy_namespace {
            return;
        }

        let binary_namespace_string = self.cur().get_binary_namespace();
        let root_node = self.cur().root_node.unwrap();
        let namespace_id = self.cur().namespace_id.clone();
        let compiler = t.get_compiler();
        let module_type: Type = AstFactory::type_node(root_node);
        let binary_namespace_name = self.ast_factory.create_name(
            compiler,
            binary_namespace_string.expect("java.lang.NullPointerException"),
            module_type,
        );
        binary_namespace_name.set_original_name(compiler, namespace_id);
        self.declare_global_variable(binary_namespace_name, t);

        let compiler = t.get_compiler();
        let object_lit = self.ast_factory.create_object_lit(compiler, &[]);
        let binary_namespace_export_node =
            IR::var_with_value(compiler, binary_namespace_name, object_lit);
        if add_at == AddAt::BEFORE {
            binary_namespace_export_node.insert_before(compiler, at_node);
        } else if add_at == AddAt::AFTER {
            binary_namespace_export_node.insert_after(compiler, at_node);
        }
        binary_namespace_export_node.put_boolean_prop(compiler, Prop::IS_NAMESPACE, true);
        binary_namespace_export_node.srcref_tree(compiler, at_node);
        Self::mark_const(compiler, binary_namespace_export_node);
        compiler.report_change_to_enclosing_scope(binary_namespace_export_node);
        self.cur_mut().has_created_export_object = true;
    }

    // port: ClosureRewriteModule#checkAndSetStrictModeDirective
    pub fn check_and_set_strict_mode_directive(t: &mut NodeTraversal<'_>, n: NodeId) {
        check_state!(n.is_script(t), "%s", n.to_string(t));

        if n.is_use_strict(t) {
            t.report(n, &USELESS_USE_STRICT_DIRECTIVE, &[]);
        } else {
            n.set_use_strict(t, true);
        }
    }

    // port: ClosureRewriteModule#markConst
    fn mark_const(ast: &mut Ast, n: NodeId) {
        let info = n.get_jsdoc_info(ast);
        let mut builder = JSDocInfoBuilder::maybe_copy_from(info.as_deref());
        builder.record_constancy();
        let built = builder.build();
        n.set_jsdoc_info(ast, built);
    }

    // port: ClosureRewriteModule#maybeSplitMultiVar
    fn maybe_split_multi_var(ast: &mut Ast, rhs_node: NodeId) {
        let statement_node = rhs_node.get_grandparent(ast).unwrap();
        if !statement_node.is_var(ast) || !statement_node.has_more_than_one_child(ast) {
            return;
        }

        let name_node = rhs_node.get_parent(ast).unwrap();
        name_node.detach(ast);
        rhs_node.detach(ast);
        IR::var_with_value(ast, name_node, rhs_node).insert_before(ast, statement_node);
    }

    // port: ClosureRewriteModule#markConstAndCopyJsDoc
    fn mark_const_and_copy_js_doc(ast: &mut Ast, from: NodeId, target: NodeId) {
        let info = from.get_jsdoc_info(ast);
        let mut builder = JSDocInfoBuilder::maybe_copy_from(info.as_deref());
        builder.record_constancy();
        let built = builder.build();
        target.set_jsdoc_info(ast, built);
    }

    // port: ClosureRewriteModule#recordExportToInline
    fn record_export_to_inline(
        &mut self,
        compiler: &AbstractCompiler,
        export_definition: ExportDefinition,
    ) {
        check_state!(
            export_definition.has_inlinable_name(compiler, &self.cur().exports_to_inline),
            "exportDefinition: %s\n\nexportsToInline keys: %s",
            export_definition.to_string(compiler),
            format!(
                "[{}]",
                self.cur()
                    .exports_to_inline
                    .keys()
                    .map(|v| v.to_string(compiler))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        );
        let name_decl = export_definition.name_decl.unwrap();
        check_state!(
            self.cur_mut()
                .exports_to_inline
                .insert(name_decl, export_definition.clone())
                .is_none(),
            "Already found a mapping for inlining export: %s",
            name_decl.to_string(compiler)
        );
        let local_name = export_definition.get_local_name(compiler);
        let full_exported_name = concat_nullable(
            self.cur().get_binary_namespace().as_ref(),
            &export_definition.get_export_postfix(),
        );
        self.record_name_to_inline(
            check_not_null!(local_name),
            full_exported_name,
            /* namespaceId= */ None,
        );
    }

    // port: ClosureRewriteModule#recordNameToInline
    fn record_name_to_inline(
        &mut self,
        alias_name: JsString,
        new_name: JsString,
        namespace_id: Option<JsString>,
    ) {
        // This intentionally overwrites a possibly pre-existing alias of the same name.
        // User code might import the same name twice, with the same variable name. That's an
        // error (duplicate variable definition, reported in TypeValidator), but this code still
        // shouldn't crash on it.
        self.cur_mut()
            .names_to_inline_by_alias
            .insert(alias_name, AliasName::new(new_name, namespace_id));
    }

    /// Examines queue'ed unrecognizedRequires to categorize and report them as either missing
    /// module, missing namespace or late provide.
    // port: ClosureRewriteModule#reportUnrecognizedRequires
    fn report_unrecognized_requires(&mut self, compiler: &mut AbstractCompiler) {
        for i in 0..self.unrecognized_requires.len() {
            let namespace_id = self.unrecognized_requires[i].namespace_id.clone();

            let require_node = self.unrecognized_requires[i].require_node;
            let target_goog_module_exists = self.rewrite_state.contains_module(&namespace_id);
            let target_legacy_script_exists = self
                .rewrite_state
                .provided_namespaces
                .contains(&namespace_id);

            if target_goog_module_exists || target_legacy_script_exists {
                // The required thing actually was available somewhere in the program but just
                // wasn't available as early as the require statement would have liked.
                continue;
            }

            // Remove the require node so this problem isn't reported again in
            // ProcessClosurePrimitives.
            if self.preserve_sugar {
                continue;
            }

            if NodeUtil::get_enclosing_script(compiler, require_node).is_none() {
                continue; // It's already been removed; nothing to do.
            }

            compiler.report_change_to_enclosing_scope(require_node);
            let enclosing_statement =
                NodeUtil::get_enclosing_statement(compiler, require_node).unwrap();

            // To make compilation with partial source information work for Clutz, delete any
            // name declarations in the enclosing statement completely. For non-declarations,
            // simply replace the invalid require with null.
            if !NodeUtil::is_name_declaration(compiler, Some(enclosing_statement)) {
                let null_node = self
                    .ast_factory
                    .create_null(compiler)
                    .srcref(compiler, require_node);
                require_node.replace_with(compiler, null_node);
                continue;
            }

            enclosing_statement.detach(compiler);
            let synthetic_externs = &mut self.synthetic_externs;
            NodeUtil::visit_lhs_nodes_in_node(
                compiler,
                enclosing_statement,
                &mut |compiler: &mut AbstractCompiler, lhs: NodeId| {
                    synthetic_externs
                        .entry(lhs.get_string(compiler))
                        .or_insert(lhs);
                },
            );
        }
    }

    // port: ClosureRewriteModule#safeSetString
    fn safe_set_string(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        new_string: JsString,
    ) {
        if n.get_string(compiler) == new_string {
            return;
        }

        let original_name = n.get_string(compiler);
        n.set_string(compiler, new_string);
        if n.get_original_name(compiler).is_none() {
            n.set_original_name(compiler, Some(original_name));
        }
        // TODO(blickly): It would be better not to be renaming detached nodes
        let change_scope = ChangeTracker::get_enclosing_change_scope_root(compiler, Some(n));
        if let Some(change_scope) = change_scope {
            compiler.report_change_to_change_scope(change_scope);
        }
    }

    /// Replaces an identifier with a potentially qualified name
    // port: ClosureRewriteModule#safeSetMaybeQualifiedString
    fn safe_set_maybe_qualified_string(
        &mut self,
        compiler: &mut AbstractCompiler,
        name_node: NodeId,
        new_string: Option<JsString>,
        is_module_namespace: bool,
    ) {
        let new_string = new_string.expect("java.lang.NullPointerException");
        if new_string.index_of_char(u16::from(b'.')) == -1 {
            self.safe_set_string(compiler, name_node, new_string);
            let parent = name_node.get_parent(compiler).unwrap();
            if is_module_namespace
                && parent.is_get_prop(compiler)
                && name_node
                    .get_grandparent(compiler)
                    .unwrap()
                    .is_call(compiler)
                && parent.is_first_child_of(compiler, name_node.get_grandparent(compiler))
            {
                // In cases where we're calling a function off a module namespace, don't pass the
                // module namespace as `this`.
                name_node
                    .get_grandparent(compiler)
                    .unwrap()
                    .put_boolean_prop(compiler, Prop::FREE_CALL, true);
            }
            return;
        }

        // When replacing with a dotted fully qualified name it's already better than an original
        // name.
        let name_parent = name_node.get_parent(compiler).unwrap();
        let new_qualified_name = self
            .ast_factory
            .create_qname_using_js_type_info(
                compiler,
                self.global_typed_scope,
                &new_string.to_string(),
            )
            .srcref_tree(compiler, name_node);
        // Sometimes the typechecker gave `nameNode` the correct type, but we can't infer the
        // right type for `newQualifiedName`. If so, giving `newQualifiedName` the same type
        // typechecking used for `nameNode` is less confusing.
        let name_type = name_node.get_jstype(compiler);
        new_qualified_name.set_jstype(compiler, name_type);

        let replaced = Self::safe_set_string_if_declaration(
            compiler,
            name_parent,
            name_node,
            new_qualified_name,
        );
        if replaced {
            return;
        }

        name_node.replace_with(compiler, new_qualified_name);
        // Given import "var Bar = goog.require('foo.Bar');" here we replace a usage of Bar with
        // foo.Bar if Bar is goog.provided. 'foo' node is generated and never visible to user.
        // Because of that we should mark all such nodes as non-indexable leaving only Bar
        // indexable. Given that replacement is GETPROP node, prefix is first child. It's also
        // possible that replacement is single-part namespace. Like goog.provide('Bar') in that
        // case replacement won't have children.
        if new_qualified_name.has_children(compiler) {
            new_qualified_name
                .get_first_child(compiler)
                .unwrap()
                .make_non_indexable_recursive(compiler);
        }
        compiler.report_change_to_enclosing_scope(new_qualified_name);
    }

    /// Sets the string if given a declaration, return whether or not the name was changed
    // port: ClosureRewriteModule#safeSetStringIfDeclaration
    fn safe_set_string_if_declaration(
        ast: &mut Ast,
        name_parent: NodeId,
        name_node: NodeId,
        new_qualified_name: NodeId,
    ) -> bool {
        let mut jsdoc = name_parent.get_jsdoc_info(ast);

        match name_parent.get_token(ast) {
            Token::FUNCTION | Token::CLASS => {
                if !NodeUtil::is_statement(ast, name_parent)
                    || name_parent.get_first_child(ast) != Some(name_node)
                {
                    return false;
                }

                let placeholder = IR::empty(ast);
                name_parent.replace_with(ast, placeholder);
                let new_declaration = NodeUtil::get_declaration_from_name(
                    ast,
                    new_qualified_name,
                    Some(name_parent),
                    Token::VAR,
                    jsdoc,
                );
                if NodeUtil::is_expr_assign(ast, new_declaration) {
                    let assign = new_declaration.get_only_child(ast);
                    let name_type = name_node.get_jstype(ast);
                    assign.set_jstype(ast, name_type);
                    Self::update_source_info_for_exported_top_level_variable(
                        ast, assign, name_node,
                    );
                }
                name_parent.set_jsdoc_info(ast, None);
                new_declaration.srcref_tree_if_missing(ast, name_parent);
                placeholder.replace_with(ast, new_declaration);
                NodeUtil::remove_name(ast, name_parent);
                true
            }
            Token::VAR | Token::LET | Token::CONST => {
                let rhs = if name_node.has_children(ast) {
                    Some(name_node.get_last_child(ast).unwrap().detach(ast))
                } else {
                    None
                };
                if jsdoc.is_none() {
                    // Get inline JSDocInfo if there is no JSDoc on the actual declaration.
                    jsdoc = name_node.get_jsdoc_info(ast);
                }
                let new_statement = NodeUtil::get_declaration_from_name(
                    ast,
                    new_qualified_name,
                    rhs,
                    Token::VAR,
                    jsdoc.clone(),
                );
                if NodeUtil::is_expr_assign(ast, new_statement) {
                    let assign = new_statement.get_only_child(ast);
                    let name_type = name_node.get_jstype(ast);
                    assign.set_jstype(ast, name_type);
                    Self::update_source_info_for_exported_top_level_variable(
                        ast, assign, name_node,
                    );
                    if name_parent.is_const(ast) {
                        // When replacing `const name = ...;` with `some.prop = ...`, ensure that
                        // `some.prop` is annotated @const.
                        let mut jsdoc_builder = JSDocInfoBuilder::maybe_copy_from(jsdoc.as_deref());
                        jsdoc_builder.record_constancy();
                        jsdoc = jsdoc_builder.build();
                        assign.set_jsdoc_info(ast, jsdoc);
                    }
                }
                new_statement.srcref_tree_if_missing(ast, name_parent);
                NodeUtil::replace_declaration_child(ast, name_node, new_statement);
                true
            }
            Token::OBJECT_PATTERN | Token::ARRAY_PATTERN | Token::PARAM_LIST => {
                panic!("Not supported")
            }
            _ => false,
        }
    }

    /// If we had something like `const FOO = "text"` and we export `FOO`, change the source
    /// location information for the rewritten FOO. The replacement should be something like
    /// MOD.FOO = "text", so we look for MOD.FOO and replace the source location for FOO to the
    /// original location of FOO.
    // port: ClosureRewriteModule#updateSourceInfoForExportedTopLevelVariable
    fn update_source_info_for_exported_top_level_variable(
        ast: &mut Ast,
        assign: NodeId,
        source_name: NodeId,
    ) {
        check_state!(assign.is_assign(ast));
        check_state!(source_name.is_name(ast));

        // ASSIGN always has two children.
        let get_prop = assign.get_first_child(ast).unwrap();
        if !get_prop.is_get_prop(ast) {
            return;
        }

        let name = source_name
            .get_original_name(ast)
            .unwrap_or_else(|| source_name.get_string(ast));

        // The source range of this NAME includes its declared value, which we don't want.
        let source_name = source_name.clone_node(ast);
        source_name.set_length(ast, name.length() as i32);

        // Receiver and prop string should both use the position of the source name.
        get_prop.srcref_tree(ast, source_name);
    }

    // port: ClosureRewriteModule#isTopLevel
    fn is_top_level(&self, t: &NodeTraversal<'_>, n: NodeId, scope_type: ScopeType) -> bool {
        if scope_type == ScopeType::EXEC_CONTEXT {
            t.in_global_scope() || t.get_closest_hoist_scope_root() == self.cur().root_node
        } else {
            // Must be ScopeType.BLOCK;
            n.get_parent(t) == self.cur().root_node
        }
    }

    // port: ClosureRewriteModule#toModuleContentsPrefix
    fn to_module_contents_prefix(namespace_id: &JsString) -> JsString {
        JsString::from(MODULE_CONTENTS_PREFIX)
            .concat(&namespace_id.replace(&".".into(), &"$".into()))
            .concat(&"_".into())
    }

    // port: ClosureRewriteModule#isModuleExport
    pub fn is_module_export(name: &JsString) -> bool {
        name.starts_with(MODULE_EXPORTS_PREFIX)
    }

    // port: ClosureRewriteModule#isModuleContent
    pub fn is_module_content(name: &JsString) -> bool {
        name.starts_with(MODULE_CONTENTS_PREFIX)
    }

    /// Returns whether this is a) a reference to the name "exports" and b) based on scoping,
    /// actually refers to the implicit goog.module exports object.
    // port: ClosureRewriteModule#isGoogModuleExportsRef
    fn is_goog_module_exports_ref(t: &mut NodeTraversal<'_>, target: NodeId) -> bool {
        if !target.is_name(t) || !target.matches_name(t, "exports") {
            return false;
        }
        let scope = t.get_scope();
        let exports_var = scope.get_var(t.get_compiler(), "exports");
        // note: exportsVar may be null for third-party code using UMD patterns like
        //   if ("object"==typeof exports) { [...]
        // and if null, is not in a goog.module => not a goog.module exports var
        exports_var.is_some_and(|exports_var| exports_var.is_goog_module_exports(t.get_compiler()))
    }

    /// Whether the getprop is used as an assignment target, and that target represents a module
    /// export. Note: that "export.name = value" is an export, while "export.name.foo = value" is
    /// not (it is an assignment to a property of an exported value).
    // port: ClosureRewriteModule#isExportPropertyAssignment
    fn is_export_property_assignment(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let target = n.get_first_child(t).unwrap();
        (Self::is_assign_target(t, n) || Self::is_typedef_target(t, n))
            && Self::is_goog_module_exports_ref(t, target)
    }

    // port: ClosureRewriteModule#isAssignTarget
    fn is_assign_target(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        parent.is_assign(ast) && parent.get_first_child(ast) == Some(n)
    }

    // port: ClosureRewriteModule#isTypedefTarget
    fn is_typedef_target(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        parent.is_expr_result(ast) && parent.get_first_child(ast) == Some(n)
    }

    /// Add alias nodes to the symbol table as they going to be removed by rewriter. Example
    /// aliases:
    ///
    /// const Foo = goog.require('my.project.Foo'); const bar = goog.require('my.project.baz');
    /// const {baz} = goog.require('my.project.utils');
    // port: ClosureRewriteModule#maybeAddAliasToSymbolTable
    fn maybe_add_alias_to_symbol_table(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        module: Option<&JsString>,
    ) {
        if let Some(preprocessor_symbol_table) = &self.preprocessor_symbol_table {
            let mut preprocessor_symbol_table = preprocessor_symbol_table.lock().unwrap();
            n.put_boolean_prop(compiler, Prop::MODULE_ALIAS, true);
            // Alias can be used in js types. Types have node type STRING and not NAME so we have
            // to use their name as string.
            let node_name = if n.is_string_lit(compiler) {
                Some(n.get_string(compiler))
            } else {
                preprocessor_symbol_table.get_qualified_name(compiler, n)
            };
            // We need to include module as part of the name because aliases are local to current
            // module. Aliases with the same name from different module should be completely
            // different entities.
            let name = JsString::from("alias_")
                .concat(&concat_nullable(module, &"_".into()))
                .concat(&node_name.unwrap_or_else(|| "null".into()));
            preprocessor_symbol_table.add_reference_with_name(n, Some(name));
        }
    }

    // port: ClosureRewriteModule#declareGlobalVariable
    fn declare_global_variable(&mut self, n: NodeId, t: &mut NodeTraversal<'_>) {
        check_state!(n.is_name(t));
        let Some(global_typed_scope) = self.global_typed_scope else {
            return;
        };

        let name = n.get_string(t);
        if global_typed_scope.has_own_slot(t.get_compiler(), name.clone()) {
            let original = global_typed_scope
                .get_own_slot(t.get_compiler(), name.clone())
                .unwrap()
                .get_node(t.get_compiler());
            let original = match original {
                Some(original) => crate::node_printing::to_string(t.get_compiler(), original),
                None => "<unknown>".to_string(),
            };
            let current_script = t.get_current_script().unwrap();
            t.report(
                current_script,
                &ILLEGAL_MODULE_RENAMING_CONFLICT,
                &[&name.to_string(), &original],
            );
        } else {
            let type_ = check_not_null!(n.get_jstype(t));
            let input = t.get_input().cloned();
            global_typed_scope.declare(t.get_compiler(), name, Some(n), Some(type_), input, false);
        }
    }
}

/// Rewrites JsDoc type references to match AST changes resulting from imported alias inlining,
/// module content renaming of top level constructor functions and classes, and module renaming
/// from fully qualified legacy namespace to its binary name.
// port: ClosureRewriteModule.ReplaceJsDocRefs
struct ReplaceJsDocRefs {
    scope: ScopeId,
}

impl ReplaceJsDocRefs {
    // port: ClosureRewriteModule.ReplaceJsDocRefs#ReplaceJsDocRefs
    fn new(scope: ScopeId) -> Self {
        Self { scope }
    }

    // port: ClosureRewriteModule.ReplaceJsDocRefs#visit
    fn visit(
        &self,
        outer: &mut ClosureRewriteModule,
        compiler: &mut AbstractCompiler,
        type_ref_node: NodeId,
    ) {
        if !type_ref_node.is_string_lit(compiler) {
            return;
        }
        // A type name that might be simple like "Foo" or qualified like "foo.Bar".
        let type_name = type_ref_node.get_string(compiler);
        let dot = type_name.index_of_char(u16::from(b'.'));
        let root_of_type = if dot == -1 {
            type_name.clone()
        } else {
            type_name.substring(0, dot as usize)
        };

        let root_var = self.scope.get_var(compiler, root_of_type.clone());
        if let Some(root_var) = root_var {
            let root_var_scope = root_var.get_scope(compiler);
            if !root_var_scope
                .get_closest_hoist_scope(compiler)
                .unwrap()
                .is_global(compiler)
                && !root_var_scope.is_module_scope(compiler)
            {
                // this is a variable inside a local scope, not a module local or imported alias.
                return;
            }
        }
        // Rewrite the type node if any of the following hold, in priority order:
        //  - the root of the type name is in our set of aliases to inline
        //  - the root of the type name is a name defined in the module scope
        //  - a prefix of the type name matches a Closure namespace
        // AND the following is false:
        //  - the root of the type name is defined in an inner scope

        // If the name is an alias for an imported namespace rewrite from
        // "{Foo}" to
        // "{module$exports$bar$Foo}" or
        // "{bar.Foo}"
        if outer
            .cur()
            .names_to_inline_by_alias
            .contains_key(&root_of_type)
        {
            if outer.preprocessor_symbol_table.is_some() {
                // Jsdoc type node is a single STRING node that spans the whole type. For example
                // STRING node "bar.Foo". When rewriting modules potentially replace only "module"
                // part of the type: "bar.Foo" => "module$exports$bar$Foo". So we need to remember
                // that "bar" as alias. To do that we clone type node and make "bar" node from it.
                let module_only_node = type_ref_node.clone_node(compiler);
                outer.safe_set_string(compiler, module_only_node, root_of_type.clone());
                module_only_node.set_length(compiler, root_of_type.length() as i32);
                let module = outer.cur().namespace_id.clone();
                outer.maybe_add_alias_to_symbol_table(compiler, module_only_node, module.as_ref());
            }

            let aliased_namespace = outer.cur().names_to_inline_by_alias[&root_of_type]
                .new_name
                .clone();
            let remainder = if dot == -1 {
                JsString::from("")
            } else {
                type_name.substring_from(dot as usize)
            };
            outer.safe_set_string(
                compiler,
                type_ref_node,
                aliased_namespace.concat(&remainder),
            );
        } else if outer.cur().is_module && outer.cur().top_level_names.contains(&root_of_type) {
            // If this is a module and the type name is the name of a top level var/function/class
            // defined in this script then that var will have been previously renamed from Foo to
            // module$contents$Foo_Foo. Update the JsDoc reference to match.
            let new_string = concat_nullable(outer.cur().contents_prefix.as_ref(), &type_name);
            outer.safe_set_string(compiler, type_ref_node, new_string);
        } else if outer.cur().is_module && root_of_type == "exports" {
            // rewrite a type reference to the implicit "exports" variable
            // e.g. /** @type {exports.Foo} */ -> /** @type {module$exports$my$mod.Foo} */
            let namespace = outer.cur().get_binary_namespace();
            let remainder = if dot == -1 {
                JsString::from("")
            } else {
                type_name.substring_from(dot as usize)
            };
            let new_string = concat_nullable(namespace.as_ref(), &remainder);
            outer.safe_set_string(compiler, type_ref_node, new_string);
        } else {
            self.rewrite_if_closure_namespace_ref(outer, compiler, &type_name, type_ref_node);
        }
    }

    /// Tries to match the longest possible prefix of this type to a Closure namespace
    ///
    /// If the longest prefix match is a legacy module or provide, this is a no-op. If the longest
    /// prefix match is a non-legacy module, this method rewrites the type node.
    // port: ClosureRewriteModule.ReplaceJsDocRefs#rewriteIfClosureNamespaceRef
    fn rewrite_if_closure_namespace_ref(
        &self,
        outer: &mut ClosureRewriteModule,
        compiler: &mut AbstractCompiler,
        type_name: &JsString,
        type_ref_node: NodeId,
    ) {
        // Tries to rename progressively shorter type prefixes like "foo.Bar.Baz", then "foo.Bar",
        // then "foo".
        let mut prefix_type_name = type_name.clone();
        let mut suffix = JsString::from("");

        loop {
            let binary_namespace_if_module = outer
                .rewrite_state
                .get_binary_namespace(&outer.scripts, &prefix_type_name);
            if outer
                .legacy_script_namespaces_and_prefixes
                .contains(&prefix_type_name)
                && binary_namespace_if_module.is_none()
            {
                // This thing is definitely coming from a legacy script and so the fully qualified
                // type name will always resolve as is.
                return;
            }

            // If the typeName is a reference to a fully qualified legacy namespace like
            // "foo.bar.Baz" of something that is actually a module then rewrite the JsDoc
            // reference to "module$exports$Bar".
            // Note: we may want to ban this pattern in the future. See b/133501660.
            if let Some(binary_namespace_if_module) = binary_namespace_if_module {
                outer.safe_set_string(
                    compiler,
                    type_ref_node,
                    binary_namespace_if_module.concat(&suffix),
                );
                return;
            }

            if prefix_type_name.index_of_char(u16::from(b'.')) != -1 {
                let last_dot = prefix_type_name.last_index_of_char(u16::from(b'.'));
                prefix_type_name = prefix_type_name.substring(0, last_dot as usize);
                suffix = type_name.substring_from(prefix_type_name.length());
            } else {
                break;
            }
        }
    }
}

impl CompilerPass for ClosureRewriteModule {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        ClosureRewriteModule::process(self, compiler, externs, root);
    }
}
