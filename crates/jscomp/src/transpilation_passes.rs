/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/TranspilationPasses.java.

#![allow(clippy::needless_return)] // Preserve Java statement order and borrowing branches.
use crate::es6_relativize_import_paths::Es6RelativizeImportPaths;
pub use crate::es6_rewrite_destructuring::ObjectDestructuringRewriteMode;
use crate::es6_rewrite_modules::Es6RewriteModules;
use crate::es6_rewrite_modules_to_common_js_modules::Es6RewriteModulesToCommonJsModules;
use crate::{
    abstract_peephole_transpilation::AbstractPeepholeTranspilation,
    compiler_options::{ChunkOutputType, CompilerOptions},
    es6_normalize_classes::Es6NormalizeClasses,
    es6_normalize_shorthand_properties::Es6NormalizeShorthandProperties,
    pass_factory::PassFactory,
    pass_list_builder::PassListBuilder,
    pass_names,
    peephole_transpilations_pass::PeepholeTranspilationsPass,
    preprocessor_symbol_table::CachedInstanceFactory,
    report_untranspilable_features::ReportUntranspilableFeatures,
    rewrite_catch_with_no_binding::RewriteCatchWithNoBinding,
    rewrite_new_dot_target::RewriteNewDotTarget,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use std::sync::{Arc, LazyLock};
#[derive(Default)]
pub struct TranspilationPasses;
// port: TranspilationPasses#peepholeTranspilationsPasses
pub static PEEPHOLE_TRANSPILATIONS_PASSES: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("peepholeTranspilationsPasses")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            let mut peephole_transpilations: Vec<Box<dyn AbstractPeepholeTranspilation>> =
                Vec::new();
            peephole_transpilations.push(Box::new(ReportUntranspilableFeatures::new(
                compiler,
                compiler.get_options().get_browser_featureset_year_object(),
                compiler.get_options().get_output_feature_set(),
            )));
            if compiler
                .get_options()
                .needs_transpilation_of(Feature::OPTIONAL_CATCH_BINDING)
            {
                peephole_transpilations.push(Box::new(RewriteCatchWithNoBinding::new(compiler)));
            }
            if compiler
                .get_options()
                .needs_transpilation_of(Feature::SHORTHAND_OBJECT_PROPERTIES)
            {
                peephole_transpilations
                    .push(Box::new(Es6NormalizeShorthandProperties::new(compiler)));
            }
            if compiler
                .get_options()
                .needs_transpilation_of(Feature::NEW_TARGET)
            {
                peephole_transpilations.push(Box::new(RewriteNewDotTarget::new(compiler)));
            }
            Box::new(PeepholeTranspilationsPass::create(
                compiler,
                peephole_transpilations,
            ))
        }))
        .build()
});
pub static ES6_REWRITE_MODULE_TO_CJS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("es6RewriteModuleToCjs")
        .set_internal_factory(Arc::new(|compiler| {
            Box::new(Es6RewriteModulesToCommonJsModules::new(compiler))
        }))
        .build()
});
pub static ES6_RELATIVIZE_IMPORT_PATHS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("es6RelativizeImportPaths")
        .set_internal_factory(Arc::new(|compiler| {
            Box::new(Es6RelativizeImportPaths::new(compiler))
        }))
        .build()
});
pub static REWRITE_ASYNC_FUNCTIONS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("rewriteAsyncFunctions")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::rewrite_async_functions::RewriteAsyncFunctions::create(compiler))
        }))
        .build()
});
pub static REWRITE_ASYNC_ITERATION: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("rewriteAsyncIteration")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::rewrite_async_iteration::RewriteAsyncIteration::create(compiler))
        }))
        .build()
});
pub static REWRITE_OBJECT_SPREAD: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("rewriteObjectSpread")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::rewrite_object_spread::RewriteObjectSpread::new(
                compiler,
            ))
        }))
        .build()
});
pub static REWRITE_EXPONENTIAL_OPERATOR: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("rewriteExponentialOperator")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(
                crate::es7_rewrite_exponential_operator::Es7RewriteExponentialOperator::new(
                    compiler,
                ),
            )
        }))
        .build()
});
// port: TranspilationPasses#es6NormalizeClasses
pub static ES6_NORMALIZE_CLASSES: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name(pass_names::ES6_NORMALIZE_CLASSES)
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(Es6NormalizeClasses::new(compiler))
        }))
        .build()
});
pub static ES6_REWRITE_CLASS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("Es6RewriteClass")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            let es6_subclass_transpilation =
                compiler.get_options().get_es6_subclass_transpilation();
            Box::new(crate::es6_rewrite_class::Es6RewriteClass::new(
                compiler,
                es6_subclass_transpilation,
            ))
        }))
        .build()
});
pub static ES6_REWRITE_ARROW_FUNCTION: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("Es6RewriteArrowFunction")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::es6_rewrite_arrow_function::Es6RewriteArrowFunction::new(compiler))
        }))
        .build()
});
pub static REWRITE_POLYFILLS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("RewritePolyfills")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            let inject_polyfills = compiler.get_options().get_rewrite_polyfills();
            let isolate_polyfills = compiler.get_options().get_isolate_polyfills();
            let inject_polyfills_newer_than =
                compiler.get_options().get_inject_polyfills_newer_than();
            Box::new(crate::rewrite_polyfills::RewritePolyfills::new(
                compiler,
                inject_polyfills,
                isolate_polyfills,
                inject_polyfills_newer_than,
            ))
        }))
        .build()
});
// port: TranspilationPasses#instrumentAsyncContext
pub static INSTRUMENT_ASYNC_CONTEXT: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("instrumentAsyncContext")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            let should_instrument_await = compiler
                .get_options()
                .get_output_feature_set()
                .contains(Feature::ASYNC_FUNCTIONS);
            Box::new(
                crate::instrument_async_context::InstrumentAsyncContext::new(
                    compiler,
                    should_instrument_await,
                ),
            )
        }))
        .build()
});
pub static ES6_CONVERT_SUPER: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("es6ConvertSuper")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::es6_convert_super::Es6ConvertSuper::new(compiler))
        }))
        .build()
});
pub static INJECT_TRANSPILATION_RUNTIME_LIBRARIES: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("es6InjectRuntimeLibraries")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(
                crate::inject_transpilation_runtime_libraries::InjectTranspilationRuntimeLibraries::new(
                    compiler,
                ),
            )
        }))
        .build()
});
pub static ES6_REWRITE_REST_PARAMETERS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("es6RewriteRestParameters")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::es6_rewrite_rest_parameters::Es6RewriteRestParameters::new(compiler))
        }))
        .build()
});
pub static ES6_REWRITE_SPREAD_EXPRESSIONS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("es6RewriteSpreadExpressions")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(
                crate::es6_rewrite_spread_expressions::Es6RewriteSpreadExpressions::new(compiler),
            )
        }))
        .build()
});
pub static LATE_CONVERT_ES6_TO_ES3: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("lateConvertEs6")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::late_es6_to_es3_converter::LateEs6ToEs3Converter::new(compiler))
        }))
        .build()
});
pub static ES6_FOR_OF: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("es6ForOf")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::es6_for_of_converter::Es6ForOfConverter::new(
                compiler,
            ))
        }))
        .build()
});
pub static REWRITE_BLOCK_SCOPED_FUNCTION_DECLARATION: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("Es6RewriteBlockScopedFunctionDeclaration")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::es6_rewrite_block_scoped_function_declaration::Es6RewriteBlockScopedFunctionDeclaration::new(compiler))
        }))
        .build()
});
pub static REWRITE_BLOCK_SCOPED_DECLARATION: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("Es6RewriteBlockScopedDeclaration")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(
                crate::es6_rewrite_block_scoped_declaration::Es6RewriteBlockScopedDeclaration::new(
                    compiler,
                ),
            )
        }))
        .build()
});
pub static REWRITE_GENERATORS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("rewriteGenerators")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::es6_rewrite_generators::Es6RewriteGenerators::new(
                compiler,
            ))
        }))
        .build()
});
pub static REWRITE_LOGICAL_ASSIGNMENT_OPERATORS_PASS: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("rewriteLogicalAssignmentOperatorsPass")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(crate::rewrite_logical_assignment_operators_pass::RewriteLogicalAssignmentOperatorsPass::new(compiler))
        }))
        .build()
});
pub static REWRITE_OPTIONAL_CHAINING_OPERATOR: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("rewriteOptionalChainingOperator")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(
                crate::rewrite_optional_chaining_operator::RewriteOptionalChainingOperator::new(
                    compiler,
                ),
            )
        }))
        .build()
});
pub static REWRITE_NULLISH_COALESCE_OPERATOR: LazyLock<PassFactory> = LazyLock::new(|| {
    PassFactory::builder()
        .set_name("rewriteNullishCoalesceOperator")
        .set_internal_factory(std::sync::Arc::new(|compiler| {
            Box::new(
                crate::rewrite_nullish_coalesce_operator::RewriteNullishCoalesceOperator::new(
                    compiler,
                ),
            )
        }))
        .build()
});
impl TranspilationPasses {
    // port: TranspilationPasses#TranspilationPasses
    pub fn new() -> Self {
        Self
    }
    // port: TranspilationPasses#addEs6ModulePass
    pub fn add_es6_module_pass(
        passes: &mut PassListBuilder,
        preprocessor_table_factory: &CachedInstanceFactory,
    ) {
        let preprocessor_table_factory = preprocessor_table_factory.clone();
        passes.maybe_add(
            PassFactory::builder()
                .set_name("es6RewriteModule")
                .set_internal_factory(Arc::new(move |compiler| {
                    preprocessor_table_factory.maybe_initialize(compiler);
                    let module_metadata_map = compiler.get_module_metadata_map().cloned();
                    let module_map = compiler.get_module_map().cloned();
                    let preprocessor_symbol_table =
                        preprocessor_table_factory.get_instance_or_null();
                    let top_scope = compiler.get_top_scope();
                    let chunk_output_type = compiler.get_options().get_chunk_output_type();
                    Box::new(Es6RewriteModules::new_with_chunk_output_type(
                        compiler,
                        module_metadata_map,
                        module_map,
                        preprocessor_symbol_table,
                        top_scope,
                        chunk_output_type,
                    ))
                }))
                .build(),
        );
    }
    // port: TranspilationPasses#addTranspilationRuntimeLibraries
    pub fn add_transpilation_runtime_libraries(passes: &mut PassListBuilder) {
        passes.maybe_add(INJECT_TRANSPILATION_RUNTIME_LIBRARIES.clone());
    }
    // port: TranspilationPasses#addEs6ModuleToCjsPass
    pub fn add_es6_module_to_cjs_pass(passes: &mut PassListBuilder) {
        passes.maybe_add(ES6_REWRITE_MODULE_TO_CJS.clone());
    }
    // port: TranspilationPasses#addEs6RewriteImportPathPass
    pub fn add_es6_rewrite_import_path_pass(passes: &mut PassListBuilder) {
        passes.maybe_add(ES6_RELATIVIZE_IMPORT_PATHS.clone());
    }
    // port: TranspilationPasses#addRewritePolyfillPass
    pub fn add_rewrite_polyfill_pass(passes: &mut PassListBuilder) {
        passes.maybe_add(REWRITE_POLYFILLS.clone());
    }
    // port: TranspilationPasses#getEs6RewriteDestructuring
    pub fn get_es6_rewrite_destructuring(
        rewrite_mode: ObjectDestructuringRewriteMode,
    ) -> PassFactory {
        PassFactory::builder()
            .set_name("Es6RewriteDestructuring")
            .set_internal_factory(std::sync::Arc::new(move |compiler| {
                Box::new(
                    crate::es6_rewrite_destructuring::Builder::new(compiler)
                        .set_destructuring_rewrite_mode(rewrite_mode)
                        .build(),
                )
            }))
            .build()
    }
    // port: TranspilationPasses#createPostTranspileUnsupportedFeaturesRemovedCheck
    pub fn create_post_transpile_unsupported_features_removed_check(
        pass_name: &str,
    ) -> PassFactory {
        PassFactory::builder()
            .set_name(pass_name)
            .set_internal_factory(std::sync::Arc::new(|_| {
                Box::new(UnsupportedFeaturesRemovedCheck)
            }))
            .build()
    }
    // port: TranspilationPasses#createFeatureRemovalPass
    fn create_feature_removal_pass(pass_name: &str, features_to_remove: FeatureSet) -> PassFactory {
        PassFactory::builder()
            .set_name(pass_name)
            .set_internal_factory(std::sync::Arc::new(move |_| {
                Box::new(FeatureRemovalPass { features_to_remove })
            }))
            .build()
    }
    // port: TranspilationPasses#addTranspilationPasses
    pub fn add_transpilation_passes(passes: &mut PassListBuilder, options: &CompilerOptions) {
        passes.maybe_add(PEEPHOLE_TRANSPILATIONS_PASSES.clone());
        passes.maybe_add(Self::create_unified_feature_removal_pass(
            "featureRemovalPasses",
            options,
        ));
        passes.maybe_add(ES6_NORMALIZE_CLASSES.clone());
        if options.needs_transpilation_of(Feature::LOGICAL_ASSIGNMENT) {
            passes.maybe_add(REWRITE_LOGICAL_ASSIGNMENT_OPERATORS_PASS.clone());
        }
        if options.needs_transpilation_of(Feature::OPTIONAL_CHAINING) {
            passes.maybe_add(REWRITE_OPTIONAL_CHAINING_OPERATOR.clone());
        }
        if options.needs_transpilation_of(Feature::NULL_COALESCE_OP) {
            passes.maybe_add(REWRITE_NULLISH_COALESCE_OPERATOR.clone());
        }
        if options.get_instrument_async_context() {
            passes.maybe_add(INSTRUMENT_ASYNC_CONTEXT.clone());
        }
        if options.needs_transpilation_of(Feature::FOR_AWAIT_OF)
            || options.needs_transpilation_of(Feature::ASYNC_GENERATORS)
        {
            passes.maybe_add(REWRITE_ASYNC_ITERATION.clone());
        }
        if options.needs_transpilation_of(Feature::OBJECT_LITERALS_WITH_SPREAD)
            || options.needs_transpilation_of(Feature::OBJECT_PATTERN_REST)
        {
            passes.maybe_add(REWRITE_OBJECT_SPREAD.clone());
            if !options.needs_transpilation_of(Feature::OBJECT_DESTRUCTURING)
                && options.needs_transpilation_of(Feature::OBJECT_PATTERN_REST)
            {
                passes.maybe_add(Self::get_es6_rewrite_destructuring(
                    ObjectDestructuringRewriteMode::REWRITE_OBJECT_REST,
                ));
            }
        }
        if options.needs_transpilation_of(Feature::ASYNC_FUNCTIONS) {
            passes.maybe_add(REWRITE_ASYNC_FUNCTIONS.clone());
        }
        if options.needs_transpilation_of(Feature::EXPONENT_OP) {
            passes.maybe_add(REWRITE_EXPONENTIAL_OPERATOR.clone());
        }
        if options.needs_transpilation_of(Feature::CLASSES) {
            passes.maybe_add(ES6_CONVERT_SUPER.clone());
        }
        if options.needs_transpilation_from(
            FeatureSet::BARE_MINIMUM
                .with_features(&[Feature::ARRAY_DESTRUCTURING, Feature::OBJECT_DESTRUCTURING]),
        ) {
            passes.maybe_add(Self::get_es6_rewrite_destructuring(
                ObjectDestructuringRewriteMode::REWRITE_ALL_OBJECT_PATTERNS,
            ));
        }
        if options.needs_transpilation_of(Feature::ARROW_FUNCTIONS) {
            passes.maybe_add(ES6_REWRITE_ARROW_FUNCTION.clone());
        }
        if options.needs_transpilation_of(Feature::CLASSES) {
            passes.maybe_add(ES6_REWRITE_CLASS.clone());
        }
        if options.needs_transpilation_of(Feature::REST_PARAMETERS) {
            passes.maybe_add(ES6_REWRITE_REST_PARAMETERS.clone());
        }
        if options.needs_transpilation_of(Feature::SPREAD_EXPRESSIONS) {
            passes.maybe_add(ES6_REWRITE_SPREAD_EXPRESSIONS.clone());
        }
        if options.needs_transpilation_from(FeatureSet::BARE_MINIMUM.with_features(&[
            Feature::COMPUTED_PROPERTIES,
            Feature::MEMBER_DECLARATIONS,
            Feature::TEMPLATE_LITERALS,
        ])) {
            passes.maybe_add(LATE_CONVERT_ES6_TO_ES3.clone());
        }
        if options.needs_transpilation_of(Feature::FOR_OF) {
            passes.maybe_add(ES6_FOR_OF.clone());
        }
        if options.needs_transpilation_of(Feature::BLOCK_SCOPED_FUNCTION_DECLARATION) {
            passes.maybe_add(REWRITE_BLOCK_SCOPED_FUNCTION_DECLARATION.clone());
        }
        if options.needs_transpilation_from(
            FeatureSet::BARE_MINIMUM
                .with_features(&[Feature::LET_DECLARATIONS, Feature::CONST_DECLARATIONS]),
        ) {
            passes.maybe_add(REWRITE_BLOCK_SCOPED_DECLARATION.clone());
        }
        if options.needs_transpilation_of(Feature::GENERATORS) {
            passes.maybe_add(REWRITE_GENERATORS.clone());
        }
        passes.maybe_add(
            Self::create_post_transpile_unsupported_features_removed_check(
                "postTranspileUnsupportedFeaturesRemovedCheck",
            ),
        );
    }
    // port: TranspilationPasses#createUnifiedFeatureRemovalPass
    pub fn create_unified_feature_removal_pass(
        pass_name: &str,
        options: &CompilerOptions,
    ) -> PassFactory {
        let mut features_to_mark_removed = FeatureSet::BARE_MINIMUM;
        if options.needs_transpilation_of(Feature::REGEXP_FLAG_D) {
            features_to_mark_removed =
                features_to_mark_removed.with_features(&[Feature::REGEXP_FLAG_D]);
        }
        if options.needs_transpilation_of(Feature::BIGINT) {
            features_to_mark_removed = features_to_mark_removed.with_features(&[Feature::BIGINT]);
        }
        if options.needs_transpilation_of(Feature::NUMERIC_SEPARATOR) {
            features_to_mark_removed =
                features_to_mark_removed.with_features(&[Feature::NUMERIC_SEPARATOR]);
        }
        if options.get_chunk_output_type() != ChunkOutputType::ES_MODULES {
            features_to_mark_removed = features_to_mark_removed.with_features(&[Feature::MODULES]);
            features_to_mark_removed =
                features_to_mark_removed.with_features(&[Feature::IMPORT_META]);
            features_to_mark_removed =
                features_to_mark_removed.with_features(&[Feature::DYNAMIC_IMPORT]);
        }
        if options.needs_transpilation_from(FeatureSet::BARE_MINIMUM.with_features(&[
            Feature::BINARY_LITERALS,
            Feature::OCTAL_LITERALS,
            Feature::REGEXP_FLAG_U,
            Feature::REGEXP_FLAG_Y,
        ])) {
            features_to_mark_removed = features_to_mark_removed.with_features(&[
                Feature::BINARY_LITERALS,
                Feature::OCTAL_LITERALS,
                Feature::REGEXP_FLAG_U,
                Feature::REGEXP_FLAG_Y,
            ]);
        }
        if options.needs_transpilation_from(FeatureSet::ES5) {
            features_to_mark_removed = features_to_mark_removed.with_features(&[
                Feature::ES3_KEYWORDS_AS_IDENTIFIERS,
                Feature::KEYWORDS_AS_PROPERTIES,
                Feature::GETTER,
                Feature::SETTER,
            ]);
        }
        features_to_mark_removed = features_to_mark_removed
            .with_features(&[Feature::STRING_CONTINUATION, Feature::TRAILING_COMMA]);
        features_to_mark_removed = features_to_mark_removed
            .with_features(&[Feature::ES_NEXT_RUNTIME, Feature::ES_UNSTABLE_RUNTIME]);
        return Self::create_feature_removal_pass(pass_name, features_to_mark_removed);
    }
}

impl TranspilationPasses {
    // port: TranspilationPasses#doesScriptHaveAnyOfTheseFeatures
    pub fn does_script_have_any_of_these_features(
        ast: &closure_rhino::node::Ast,
        script: closure_rhino::node::NodeId,
        feature_set: FeatureSet,
    ) -> bool {
        crate::node_util::NodeUtil::get_feature_set_of_script(ast, script)
            .is_some_and(|features| features.contains_at_least_one_of(feature_set))
    }
    // port: TranspilationPasses#processTranspile
    pub fn process_transpile(
        compiler: &mut crate::AbstractCompiler,
        combined_root: closure_rhino::node::NodeId,
        features_to_run_for: FeatureSet,
        callbacks: &mut [&mut dyn crate::node_traversal::Callback],
    ) {
        let language_out_features = compiler.get_options().get_output_feature_set();
        if language_out_features.contains(features_to_run_for) {
            return;
        }
        let mut single_root = combined_root.get_first_child(compiler);
        while let Some(root) = single_root {
            assert!(root.is_script(compiler));
            if Self::does_script_have_any_of_these_features(compiler, root, features_to_run_for) {
                for callback in callbacks.iter_mut() {
                    crate::node_traversal::NodeTraversal::traverse(compiler, root, *callback);
                }
            }
            single_root = root.get_next(compiler);
        }
    }
    // port: TranspilationPasses#maybeMarkFeatureAsTranspiledAway
    pub fn maybe_mark_feature_as_transpiled_away(
        compiler: &mut crate::AbstractCompiler,
        root: closure_rhino::node::NodeId,
        feature: Feature,
    ) {
        if !compiler.has_halting_errors() {
            compiler.mark_feature_not_allowed(feature);
            crate::node_util::NodeUtil::remove_feature_from_all_scripts(compiler, root, feature);
        }
    }
    // port: TranspilationPasses#maybeMarkFeaturesAsTranspiledAway
    pub fn maybe_mark_features_as_transpiled_away(
        compiler: &mut crate::AbstractCompiler,
        root: closure_rhino::node::NodeId,
        transpiled_features: FeatureSet,
    ) {
        if !compiler.has_halting_errors() {
            crate::node_util::NodeUtil::remove_features_from_all_scripts(
                compiler,
                root,
                transpiled_features,
            );
        }
    }
    // port: TranspilationPasses#postTranspileCheckUnsupportedFeaturesRemoved
    pub fn post_transpile_check_unsupported_features_removed(compiler: &crate::AbstractCompiler) {
        let output_features = compiler.get_options().get_output_feature_set();
        let mut current_features = compiler.get_allowable_features();
        if compiler.get_options().get_chunk_output_type()
            == crate::compiler_options::ChunkOutputType::ES_MODULES
        {
            current_features = current_features
                .without(Feature::MODULES)
                .without(Feature::IMPORT_META)
                .without(Feature::DYNAMIC_IMPORT);
        }
        if output_features.get_features().is_empty() {
            return;
        }
        if !output_features.contains(current_features) {
            let diff = current_features.without(output_features);
            panic!("Unsupported feature(s) leaked to output code:{}", diff);
        }
    }
}
struct FeatureRemovalPass {
    features_to_remove: FeatureSet,
}
impl crate::compiler_pass::CompilerPass for FeatureRemovalPass {
    // port: TranspilationPasses#createFeatureRemovalPass (process callback)
    fn process(
        &mut self,
        compiler: &mut crate::AbstractCompiler,
        _externs: closure_rhino::node::NodeId,
        root: closure_rhino::node::NodeId,
    ) {
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            self.features_to_remove,
        );
    }
}
struct UnsupportedFeaturesRemovedCheck;
impl crate::compiler_pass::CompilerPass for UnsupportedFeaturesRemovedCheck {
    // port: TranspilationPasses#createPostTranspileUnsupportedFeaturesRemovedCheck (process callback)
    fn process(
        &mut self,
        compiler: &mut crate::AbstractCompiler,
        _externs: closure_rhino::node::NodeId,
        _root: closure_rhino::node::NodeId,
    ) {
        TranspilationPasses::post_transpile_check_unsupported_features_removed(compiler);
    }
}
