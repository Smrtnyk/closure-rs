/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/InjectTranspilationRuntimeLibraries.java.

//! Injects JS library code that may be needed by the transpiled form of the input source code.
//! Port of `InjectTranspilationRuntimeLibraries.java`.
//!
//! The intention here is to add anything that could be needed and rely on `RemoveUnusedCode` to
//! remove the parts that don't end up getting used. This pass should run before type checking so
//! the type checking code can add type information to the injected JavaScript for checking and
//! optimization purposes.
//!
//! TODO(b/120486392): consider merging this pass with `InjectRuntimeLibraries` and
//! `RewritePolyfills`.
use crate::js::runtime_js_lib_manager::RuntimeJsLibManager;
use crate::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass, node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::node::{Ast, NodeId};
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};

// port: InjectTranspilationRuntimeLibraries
pub struct InjectTranspilationRuntimeLibraries {
    runtime_libs: Arc<Mutex<RuntimeJsLibManager>>,
    injected_class_extends_libraries: bool,
    should_instrument_async_context: bool,
}

impl InjectTranspilationRuntimeLibraries {
    // port: InjectTranspilationRuntimeLibraries#InjectTranspilationRuntimeLibraries(AbstractCompiler)
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        let should_instrument_async_context = compiler.get_options().get_instrument_async_context();
        Self::new_with_instrument_async_context(compiler, should_instrument_async_context)
    }

    // port: InjectTranspilationRuntimeLibraries#InjectTranspilationRuntimeLibraries(AbstractCompiler,boolean)
    pub fn new_with_instrument_async_context(
        compiler: &mut AbstractCompiler,
        should_instrument_async_context: bool,
    ) -> Self {
        Self {
            runtime_libs: compiler.get_runtime_js_lib_manager(),
            injected_class_extends_libraries: false,
            should_instrument_async_context,
        }
    }

    /// `runtimeLibs.injectLibForField(fieldName)`.
    fn inject_lib_for_field(&self, compiler: &mut AbstractCompiler, field_name: &str) {
        self.runtime_libs
            .lock()
            .unwrap()
            .inject_lib_for_field(compiler, field_name);
    }

    // port: InjectTranspilationRuntimeLibraries#getScriptFeatures
    fn get_script_features(ast: &Ast, script: NodeId) -> FeatureSet {
        let features = NodeUtil::get_feature_set_of_script(ast, script);
        features.unwrap_or(FeatureSet::ES3)
    }

    /// The visitor body of `checkForClassExtends`. Java injects the three libraries from inside
    /// the `NodeUtil.visitPostOrder` visitor; a Rust visitor only sees the `Ast`, so it records
    /// the class here and [`Self::check_for_class_extends`] injects right after the traversal.
    /// Nothing else happens in between, and once `injectedClassExtendsLibraries` is set the rest
    /// of the Java traversal neither descends nor acts, so the effect is the same.
    // port: InjectTranspilationRuntimeLibraries#checkForClassExtends
    fn check_for_class_extends_visit(
        injected_class_extends_libraries: &Cell<bool>,
        ast: &Ast,
        n: NodeId,
    ) {
        if !n.is_class(ast) {
            return;
        }
        // This is technically an optimization - we could just always inject these when we see
        // Feature.CLASSES. That's fine for real code, but just makes some unit testing
        // harder because more runtime libraries are injected.
        let superclass = n.get_second_child(ast).unwrap();
        if !injected_class_extends_libraries.get() && !superclass.is_empty(ast) {
            injected_class_extends_libraries.set(true);
        }
    }

    /// `NodeUtil.visitPostOrder(root, this::checkForClassExtends, (unused) ->
    /// !this.injectedClassExtendsLibraries)`.
    // port: InjectTranspilationRuntimeLibraries#checkForClassExtends
    fn check_for_class_extends(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        let injected = Cell::new(self.injected_class_extends_libraries);
        let found = Cell::new(false);
        NodeUtil::visit_post_order_with_predicate(
            compiler,
            root,
            &mut |ast: &mut Ast, n: NodeId| {
                let was_injected = injected.get();
                Self::check_for_class_extends_visit(&injected, ast, n);
                if !was_injected && injected.get() {
                    found.set(true);
                }
            },
            &|_, _| !injected.get(),
        );
        if found.get() {
            self.inject_lib_for_field(compiler, "$jscomp.construct");
            self.inject_lib_for_field(compiler, "$jscomp.inherits");
            // We must automatically generate the default constructor for descendent classes,
            // and those must call super(...arguments), so we end up injecting our own spread
            // expressions for such cases.
            self.inject_lib_for_field(compiler, "$jscomp.arrayFromIterable");
        }
        self.injected_class_extends_libraries = injected.get();
    }
}

impl CompilerPass for InjectTranspilationRuntimeLibraries {
    // port: InjectTranspilationRuntimeLibraries#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut used = FeatureSet::ES3;
        let mut script = root.get_first_child(compiler);
        while let Some(s) = script {
            used = used.with_feature_set(Self::get_script_features(compiler, s));
            script = s.get_next(compiler);
        }

        let output_features = compiler.get_options().get_output_feature_set();

        // Check for references to class `extends` clauses
        if !output_features.contains(used) {
            self.check_for_class_extends(compiler, root);
        }

        let must_be_compiled_away = used.without_set(output_features);

        // We will need these runtime methods when we transpile, but we want the runtime
        // functions to be have JSType applied to it by the type inferrence.

        if must_be_compiled_away.contains(Feature::TEMPLATE_LITERALS) {
            self.inject_lib_for_field(compiler, "$jscomp.createTemplateTagFirstArg");
            self.inject_lib_for_field(compiler, "$jscomp.createTemplateTagFirstArgWithRaw");
        }

        if must_be_compiled_away.contains(Feature::FOR_OF)
            || must_be_compiled_away.contains(Feature::ARRAY_DESTRUCTURING)
            || must_be_compiled_away.contains(Feature::OBJECT_PATTERN_REST)
        {
            // `makeIterator` isn't needed directly for `OBJECT_PATTERN_REST`, but when we
            // transpile a destructuring case that contains it, we transpile the entire
            // destructured assignment, which may also include `ARRAY_DESTRUCTURING`.
            self.inject_lib_for_field(compiler, "$jscomp.makeIterator");
        }

        if must_be_compiled_away.contains(Feature::FOR_OF) {
            self.inject_lib_for_field(compiler, "$jscomp.iteratorClose");
        }

        if must_be_compiled_away.contains(Feature::ARRAY_PATTERN_REST) {
            self.inject_lib_for_field(compiler, "$jscomp.arrayFromIterator");
        }

        if must_be_compiled_away.contains(Feature::SPREAD_EXPRESSIONS) {
            // We must automatically generate the default constructor for descendent classes,
            // and those must call super(...arguments), so we end up injecting our own spread
            // expressions for such cases.
            self.inject_lib_for_field(compiler, "$jscomp.arrayFromIterable");
        }

        if (must_be_compiled_away.contains(Feature::OBJECT_LITERALS_WITH_SPREAD)
            || must_be_compiled_away.contains(Feature::OBJECT_PATTERN_REST))
            && !output_features.contains(FeatureSet::ES2015)
        {
            // We need `Object.assign` to transpile `obj = {a, ...rest};` or
            // `const {a, ...rest} = obj;`, but the output language level doesn't indicate that it
            // is guaranteed to be present, so we'll include our polyfill.
            // Use "ensureLibraryInjected" instead of "injectLibForField" because we're injecting
            // the polyfill
            // for Object.assign, which is thus not an actual $jscomp.* method we can lookup.
            self.runtime_libs.lock().unwrap().ensure_library_injected(
                compiler,
                "es6/object/assign",
                /* force= */ false,
            );
        }

        if must_be_compiled_away.contains(Feature::CLASS_GETTER_SETTER) {
            self.inject_lib_for_field(compiler, "$jscomp.global");
        }

        if must_be_compiled_away.contains(Feature::GENERATORS) {
            self.inject_lib_for_field(compiler, "$jscomp.generator.createGenerator");
            self.inject_lib_for_field(compiler, "$jscomp.generator.Context");
        }

        if must_be_compiled_away.contains(Feature::ASYNC_FUNCTIONS) {
            self.inject_lib_for_field(compiler, "$jscomp.asyncExecutePromiseGeneratorFunction");
            if !output_features.contains(Feature::GENERATORS) {
                self.inject_lib_for_field(compiler, "$jscomp.asyncExecutePromiseGeneratorProgram");
                self.inject_lib_for_field(compiler, "$jscomp.generator.Context");
            }
        }

        if must_be_compiled_away.contains(Feature::ASYNC_GENERATORS) {
            self.inject_lib_for_field(compiler, "$jscomp.asyncExecutePromiseGeneratorFunction");
            self.inject_lib_for_field(compiler, "$jscomp.AsyncGeneratorWrapper");
            self.inject_lib_for_field(compiler, "$jscomp.AsyncGeneratorWrapper$ActionRecord");
            self.inject_lib_for_field(
                compiler,
                "$jscomp.AsyncGeneratorWrapper$ActionEnum.AWAIT_VALUE",
            );
            self.inject_lib_for_field(
                compiler,
                "$jscomp.AsyncGeneratorWrapper$ActionEnum.YIELD_VALUE",
            );
            self.inject_lib_for_field(
                compiler,
                "$jscomp.AsyncGeneratorWrapper$ActionEnum.YIELD_STAR",
            );
            if !output_features.contains(Feature::GENERATORS) {
                self.inject_lib_for_field(compiler, "$jscomp.asyncExecutePromiseGeneratorProgram");
                self.inject_lib_for_field(compiler, "$jscomp.generator.Context");
            }
        }

        if must_be_compiled_away.contains(Feature::FOR_AWAIT_OF) {
            self.inject_lib_for_field(compiler, "$jscomp.makeAsyncIterator");
        }

        if must_be_compiled_away.contains(Feature::REST_PARAMETERS) {
            self.inject_lib_for_field(compiler, "$jscomp.getRestArguments");
        }

        if used.contains(Feature::PRIVATE_ELEMENTS)
            && must_be_compiled_away.contains(Feature::PRIVATE_ELEMENTS)
        {
            self.inject_lib_for_field(compiler, "$jscomp.PrivateMap");
        }

        if self.should_instrument_async_context
            // NOTE: async functions only matter for output features, since we don't bother
            // instrumenting them if they're being transpiled away.  Generators are relevant
            // regardless of whether they're transpiled or not.
            && (output_features.contains(Feature::ASYNC_FUNCTIONS)
                || used.contains(Feature::GENERATORS)
                || used.contains(Feature::ASYNC_GENERATORS))
        {
            self.inject_lib_for_field(compiler, "$jscomp.asyncContextStart");
        }
    }
}
