/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteGoogJsImports.java.

//! Looks for references to Closure's goog.js file and globalizes. The goog.js file is an ES6
//! module that forwards some symbols on goog from Closure's base.js file, like require.
//!
//! This pass scans the goog.js file to find what keys are exported. When rewriting imports only
//! those in the goog.js file are globalized; if there is a reference to something not in the
//! goog.js file it is rewritten in such a way to cause an error later at type checking.
//!
//! This is a separate pass that needs to run before Es6RewriteModules which will remove import
//! statements.
//!
//! This pass enforces the following so that later compiler passes and regex based tools can
//! correctly recognize references to these properties of goog (like goog.require):
//!
//! - No other file is named goog.js
//! - The file is always imported as `import * as goog`
//! - No other module level variable is named `goog`
//! - `export from` is never used on goog.js
//!
//! Example:
//!
//! `import * as goog from 'path/to/closure/goog.js'; const myNamespace =
//! goog.require('my.namespace');`
//!
//! Will be rewritten to:
//!
//! `import 'path/to/closure/goog.js'; const myNamespace = goog.require('my.namespace');`
#![allow(clippy::collapsible_if)] // Keep Java's control flow.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_input::CompilerInput;
use crate::compiler_pass::CompilerPass;
use crate::deps::module_loader::ModulePath;
use crate::diagnostic_type::DiagnosticType;
use crate::es6_rewrite_modules::Es6RewriteModules;
use crate::js_error::JSError;
use crate::modules::binding::Binding;
use crate::modules::module::Module;
use crate::modules::module_map::ModuleMap;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::scope::ScopeId;
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use closure_rhino::{check_not_null, check_state};
use std::sync::Arc;

// port: RewriteGoogJsImports#GOOG_JS_IMPORT_MUST_BE_GOOG_STAR
pub static GOOG_JS_IMPORT_MUST_BE_GOOG_STAR: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_JS_IMPORT_MUST_BE_GOOG_STAR",
    "Closure''s goog.js file must be imported as `import * as goog`.",
);

// Since many tools scan for "goog.require" ban re-exporting.
// port: RewriteGoogJsImports#GOOG_JS_REEXPORTED
pub static GOOG_JS_REEXPORTED: DiagnosticType =
    DiagnosticType::error("JSC_GOOG_JS_REEXPORTED", "Do not re-export from goog.js.");

// port: RewriteGoogJsImports#CANNOT_NAME_FILE_GOOG
pub static CANNOT_NAME_FILE_GOOG: DiagnosticType = DiagnosticType::error(
    "JSC_CANNOT_NAME_FILE_GOOG",
    "Do not name files goog.js, it is reserved for Closure Library.",
);

// port: RewriteGoogJsImports#CANNOT_HAVE_MODULE_VAR_NAMED_GOOG
pub static CANNOT_HAVE_MODULE_VAR_NAMED_GOOG: DiagnosticType = DiagnosticType::error(
    "JSC_CANNOT_HAVE_MODULE_VAR_NAMED_GOOG",
    "Module scoped variables named ''goog'' must come from importing Closure Library''s goog.js file..",
);

/// Possible traversal modes - either linting or linting+rewriting.
// port: RewriteGoogJsImports.Mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Mode {
    /* Lint only. */
    LINT_ONLY,
    /* Lint and perform a rewrite on an entire compilation job */
    LINT_AND_REWRITE,
}

// port: RewriteGoogJsImports#EXPECTED_BASE_PROVIDE
const EXPECTED_BASE_PROVIDE: &str = "goog";

// port: RewriteGoogJsImports
pub struct RewriteGoogJsImports {
    mode: Mode,
    module_map: Arc<ModuleMap>,
    goog_module: Option<Arc<Module>>,
    /// Java's `LinkedHashMap<Module, Module>`. Every key is a Module taken from `moduleMap`, so
    /// the AutoValue key equality is the identity of the shared `Arc<Module>`; the key keeps the
    /// replaced module alive.
    module_replacements: IndexMap<*const Module, (Arc<Module>, Arc<Module>)>,
}

impl RewriteGoogJsImports {
    // port: RewriteGoogJsImports#RewriteGoogJsImports
    pub fn new(mode: Mode, module_map: Option<Arc<ModuleMap>>) -> Self {
        let module_map = check_not_null!(module_map);
        Self {
            mode,
            module_map,
            goog_module: None,
            module_replacements: IndexMap::<_, _>::default(),
        }
    }

    // port: RewriteGoogJsImports#changeModules
    fn change_modules(&mut self, compiler: &mut AbstractCompiler) {
        let mut resolved_modules: IndexMap<String, Arc<Module>> = IndexMap::<_, _>::default();
        let mut closure_modules: IndexMap<JsString, Arc<Module>> = IndexMap::<_, _>::default();

        for (key, value) in self.module_map.get_modules_by_path() {
            let new_module = self.get_replacement_or_default(value);
            // ImmutableMap.Builder#buildOrThrow: duplicate keys throw
            check_state!(resolved_modules.insert(key.clone(), new_module).is_none());
        }

        for (key, value) in self.module_map.get_modules_by_closure_namespace() {
            let new_module = self.get_replacement_or_default(value);
            check_state!(closure_modules.insert(key.clone(), new_module).is_none());
        }

        compiler.set_module_map(Arc::new(ModuleMap::new(resolved_modules, closure_modules)));
        self.module_replacements.clear();
    }

    /// `moduleReplacements.getOrDefault(module, module)`
    fn get_replacement_or_default(&self, module: &Arc<Module>) -> Arc<Module> {
        self.module_replacements
            .get(&Arc::as_ptr(module))
            .map_or_else(|| module.clone(), |(_, new_module)| new_module.clone())
    }

    /// Determines if a script imports the goog.js file and if so ensures that property accesses
    /// are valid and then detaches the import.
    // port: RewriteGoogJsImports.ReferenceReplacer#ReferenceReplacer
    fn reference_replacer(
        &mut self,
        compiler: &mut AbstractCompiler,
        script: NodeId,
        goog_import_node: NodeId,
        module: Arc<Module>,
        globalize_all_references: bool,
    ) {
        let mut replacer = ReferenceReplacer {
            has_bad_export: false,
            goog_import_node,
            globalize_all_references,
            goog_module: self.goog_module.clone(),
        };
        NodeTraversal::traverse(compiler, script, &mut replacer);

        let second_child = goog_import_node.get_second_child(compiler).unwrap();
        if second_child.is_import_star(compiler) {
            let mut new_bindings: IndexMap<JsString, Binding> = module.bound_names().clone();
            new_bindings.shift_remove(&JsString::from("goog"));
            if replacer.has_bad_export {
                // We might be able to get rid of this case now. Originally this was to cause a
                // type error later in compilation. But now the ModuleMapCreator should log an
                // error for the bad export. However if we're still running we might be in some
                // tool that expects us to be fault tolerant and still rewrite modules. So assume
                // this is needed in those tools.
                second_child.set_original_name(compiler, Some(JsString::from("goog")));
                second_child.set_string(compiler, "$goog");
                // ImmutableMap.copyOf rejects the null that boundNames().get("goog") may return.
                let goog_binding = module
                    .bound_names()
                    .get(&JsString::from("goog"))
                    .cloned()
                    .expect("java.lang.NullPointerException");
                new_bindings.insert(JsString::from("$goog"), goog_binding);
            } else {
                let empty = IR::empty(compiler);
                second_child.replace_with(compiler, empty);
            }
            let new_module = Arc::new(module.to_builder().bound_names(new_bindings).build());
            self.module_replacements
                .insert(Arc::as_ptr(&module), (module.clone(), new_module));
            compiler.report_change_to_enclosing_scope(goog_import_node);
        }
    }

    // port: RewriteGoogJsImports#findGoogImportNode
    fn find_goog_import_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        script_root: NodeId,
    ) -> Option<NodeId> {
        // Cannot use the module map here - information is lost about the imports. The "bound
        // names" could be from transitive imports, but we lose the original import.
        let mut valid = true;
        let mut goog_import_node = None;

        let mut child = script_root.get_first_first_child(compiler);
        while let Some(c) = child {
            if c.is_import(compiler)
                && c.get_last_child(compiler)
                    .unwrap()
                    .get_string(compiler)
                    .ends_with("/goog.js")
            {
                if c.get_first_child(compiler).unwrap().is_empty(compiler)
                    && c.get_second_child(compiler)
                        .unwrap()
                        .is_import_star(compiler)
                    && c.get_second_child(compiler)
                        .unwrap()
                        .get_string_ref(compiler)
                        == "goog"
                {
                    goog_import_node = Some(c);
                } else {
                    valid = false;
                    let source_file_name = script_root
                        .get_source_file_name(compiler)
                        .expect("java.lang.NullPointerException");
                    let error = JSError::make_with_source_location(
                        &source_file_name,
                        c.get_lineno(compiler),
                        c.get_charno(compiler),
                        &GOOG_JS_IMPORT_MUST_BE_GOOG_STAR,
                        &[],
                    );
                    compiler.report(error);
                }
            }
            child = c.get_next(compiler);
        }

        if !valid {
            return None;
        } else if goog_import_node.is_some() {
            return goog_import_node;
        }

        let module_body = script_root.get_first_child(compiler).unwrap();
        let global_scope = ScopeId::create_global_scope(compiler, script_root);
        let module_scope =
            SyntacticScopeCreator::new().create_scope(compiler, module_body, Some(global_scope));
        let goog_var = module_scope.get_var(compiler, JsString::from("goog"));

        if let Some(goog_var) = goog_var {
            if let Some(name_node) = goog_var.get_name_node(compiler) {
                let source_file_name = script_root
                    .get_source_file_name(compiler)
                    .expect("java.lang.NullPointerException");
                let error = JSError::make_with_source_location(
                    &source_file_name,
                    name_node.get_lineno(compiler),
                    name_node.get_charno(compiler),
                    &CANNOT_HAVE_MODULE_VAR_NAMED_GOOG,
                    &[],
                );
                compiler.report(error);
            }
        }

        None
    }

    // port: RewriteGoogJsImports#rewriteImports
    fn rewrite_imports(&mut self, compiler: &mut AbstractCompiler, script_root: NodeId) {
        if Es6RewriteModules::is_es6_module_root(compiler, script_root) {
            let goog_import_node = self.find_goog_import_node(compiler, script_root);
            NodeTraversal::traverse(
                compiler,
                script_root,
                &mut FindReexports::new(goog_import_node.is_some()),
            );
            let path = get_input(compiler, script_root).get_path(compiler);
            let module = self.module_map.get_module_by_path(&path).cloned();
            let module = check_not_null!(module);

            if let Some(goog_import_node) = goog_import_node {
                if self.mode == Mode::LINT_AND_REWRITE {
                    // If googModule is null then goog.js was not part of the input. Try to be
                    // fault tolerant and just assume that everything exported is on the global
                    // goog.
                    let globalize_all_references = self.goog_module.is_none();
                    self.reference_replacer(
                        compiler,
                        script_root,
                        goog_import_node,
                        module,
                        globalize_all_references,
                    );
                }
            }
        }
    }

    // port: RewriteGoogJsImports#findGoogJsScriptNode
    fn find_goog_js_script_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> Option<NodeId> {
        let mut expected_goog_path: Option<ModulePath> = None;

        // Find Closure's base.js file. goog.js should be right next to it.
        let mut script = root.get_first_child(compiler);
        while let Some(s) = script {
            let provides = get_input(compiler, s).get_provides(compiler);
            if provides.iter().any(|p| p == EXPECTED_BASE_PROVIDE) {
                // Use resolveModuleAsPath as if it is not part of the input we don't want to
                // report an error.
                expected_goog_path = Some(
                    get_input(compiler, s)
                        .get_path(compiler)
                        .resolve_module_as_path("./goog.js"),
                );
                break;
            }
            script = s.get_next(compiler);
        }

        if let Some(expected_goog_path) = expected_goog_path {
            let mut goog_script_node = None;

            let mut script = root.get_first_child(compiler);
            while let Some(s) = script {
                if get_input(compiler, s)
                    .get_path(compiler)
                    .equals_ignore_leading_slash(Some(&expected_goog_path))
                {
                    goog_script_node = Some(s);
                } else {
                    let source_file_name = s
                        .get_source_file_name(compiler)
                        .expect("java.lang.NullPointerException");
                    if source_file_name.ends_with("/goog.js") {
                        // Ban the name goog.js as input except for Closure's goog.js file. This
                        // simplifies a lot of logic if the only file that is allowed to be named
                        // goog.js is Closure's.
                        let error = JSError::make_with_source_location(
                            &source_file_name,
                            -1,
                            -1,
                            &CANNOT_NAME_FILE_GOOG,
                            &[],
                        );
                        compiler.report(error);
                    }
                }
                script = s.get_next(compiler);
            }

            return goog_script_node;
        }

        None
    }
}

/// `compiler.getInput(script.getInputId())`
fn get_input(compiler: &AbstractCompiler, script: NodeId) -> CompilerInput {
    let input_id = script
        .get_input_id(compiler)
        .expect("java.lang.NullPointerException");
    compiler
        .get_input(&input_id)
        .cloned()
        .expect("java.lang.NullPointerException")
}

/// Java's `b.originatingExport().modulePath() == googJsPath` compares ModulePath references. Both
/// come from the cached `CompilerInput#getPath` of goog.js's input, so the reference comparison
/// holds exactly when the paths are the same path string.
fn is_from_goog(b: &Binding, goog_js_path: &ModulePath) -> bool {
    b.originating_export()
        .expect("java.lang.NullPointerException")
        .module_path()
        .is_some_and(|path| path.to_string() == goog_js_path.to_string())
}

// port: RewriteGoogJsImports.ReferenceReplacer
struct ReferenceReplacer {
    has_bad_export: bool,
    goog_import_node: NodeId,
    globalize_all_references: bool,
    /// The outer class's `googModule`.
    goog_module: Option<Arc<Module>>,
}

impl ReferenceReplacer {
    // port: RewriteGoogJsImports.ReferenceReplacer#maybeRewriteBadGoogJsImportRef
    fn maybe_rewrite_bad_goog_js_import_ref(
        &mut self,
        t: &mut NodeTraversal<'_>,
        name_node: NodeId,
        parent: NodeId,
    ) {
        let compiler = t.get_compiler();
        if !parent.is_get_prop(compiler) || name_node.get_string_ref(compiler) != "goog" {
            return;
        }

        let scope = t.get_scope();
        let compiler = t.get_compiler();
        let name = name_node.get_string(compiler);
        let var = scope.get_var(compiler, &name);

        let Some(var) = var else {
            return;
        };
        let Some(var_name_node) = var.get_name_node(compiler) else {
            return;
        };
        if var_name_node.get_parent(compiler) != Some(self.goog_import_node) {
            return;
        }

        if self.globalize_all_references
            || self
                .goog_module
                .as_ref()
                .expect("java.lang.NullPointerException")
                .namespace()
                .contains_key(&parent.get_string(compiler))
        {
            return;
        }

        // Rewrite and keep the import so later type checking can report an error that this
        // property does not exist on the ES6 module.
        self.has_bad_export = true;
        name_node.set_original_name(compiler, Some(JsString::from("goog")));
        name_node.set_string(compiler, "$goog");
        t.report_code_change();
    }
}

impl Callback for ReferenceReplacer {
    // AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RewriteGoogJsImports.ReferenceReplacer#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        #[allow(clippy::single_match)]
        match n.get_token(t.get_compiler()) {
            Token::NAME => self.maybe_rewrite_bad_goog_js_import_ref(
                t,
                n,
                parent.expect("java.lang.NullPointerException"),
            ),
            _ => {}
        }
    }
}

/// Finds instances where goog is reexported. Not an exhaustive search; does not find alias, e.g.
/// `export const x = goog;`. Not to say that people should export using aliases; they shouldn't.
/// Just that we're catching the 99% case and that should be an indication to people not to do bad
/// things.
// port: RewriteGoogJsImports.FindReexports
struct FindReexports {
    has_goog_import: bool,
}

impl FindReexports {
    // port: RewriteGoogJsImports.FindReexports#FindReexports
    fn new(has_goog_import: bool) -> Self {
        Self { has_goog_import }
    }

    // port: RewriteGoogJsImports.FindReexports#checkIfForwardingExport
    fn check_if_forwarding_export(&self, t: &mut NodeTraversal<'_>, export: NodeId) {
        let compiler = t.get_compiler();
        if export.has_two_children(compiler)
            && export
                .get_last_child(compiler)
                .unwrap()
                .get_string(compiler)
                .ends_with("/goog.js")
        {
            t.report(export, &GOOG_JS_REEXPORTED, &[]);
        }
    }

    // port: RewriteGoogJsImports.FindReexports#checkIfNameFowardedExport
    fn check_if_name_foworded_export(
        &self,
        t: &mut NodeTraversal<'_>,
        name_node: NodeId,
        parent: NodeId,
    ) {
        let compiler = t.get_compiler();
        if self.has_goog_import && name_node.get_string_ref(compiler) == "goog" {
            if (parent.is_export_spec(compiler)
                && parent.get_first_child(compiler) == Some(name_node))
                || parent.is_export(compiler)
            {
                t.report(name_node, &GOOG_JS_REEXPORTED, &[]);
            }
        }
    }
}

impl Callback for FindReexports {
    // port: RewriteGoogJsImports.FindReexports#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t.get_compiler()) {
            Token::ROOT
            | Token::SCRIPT
            | Token::MODULE_BODY
            | Token::EXPORT_SPECS
            | Token::EXPORT_SPEC => true,
            Token::EXPORT => {
                self.check_if_forwarding_export(t, n);
                true
            }
            Token::NAME => {
                // Visit names in export specs and export defaults.
                self.check_if_name_foworded_export(
                    t,
                    n,
                    parent.expect("java.lang.NullPointerException"),
                );
                false
            }
            _ => false,
        }
    }

    // AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

impl CompilerPass for RewriteGoogJsImports {
    // port: RewriteGoogJsImports#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.goog_module = None;

        let mut goog_js_script_node = self.find_goog_js_script_node(compiler, root);

        if goog_js_script_node.is_none() {
            // Potentially in externs if library level type checking.
            goog_js_script_node = self.find_goog_js_script_node(compiler, externs);
        }

        if self.mode == Mode::LINT_AND_REWRITE {
            if let Some(goog_js_script_node) = goog_js_script_node {
                let goog_js_path = get_input(compiler, goog_js_script_node).get_path(compiler);
                let goog_module = self.module_map.get_module_by_path(&goog_js_path).cloned();
                let goog_module = check_not_null!(goog_module);
                self.goog_module = Some(goog_module.clone());
                check_state!(
                    goog_module
                        .bound_names()
                        .values()
                        .all(|b| is_from_goog(b, &goog_js_path)),
                    "goog.js should never import anything"
                );
                check_state!(
                    !goog_module
                        .namespace()
                        .contains_key(&JsString::from("default")),
                    "goog.js should never have a default export."
                );
                check_state!(
                    goog_module
                        .namespace()
                        .values()
                        .all(|b| is_from_goog(b, &goog_js_path)),
                    "goog.js should never export from anything."
                );
            }
        } else {
            check_state!(self.mode == Mode::LINT_ONLY);
        }

        let mut script = root.get_first_child(compiler);
        while let Some(s) = script {
            self.rewrite_imports(compiler, s);
            script = s.get_next(compiler);
        }

        self.change_modules(compiler);
    }
}
