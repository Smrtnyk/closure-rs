/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
 * Copyright 2024 The Closure Compiler Authors.
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
/*
 * Copyright 2026 The closure-rs Authors.
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
//   src/com/google/javascript/jscomp/Es6ForOfConverter.java,
//   src/com/google/javascript/jscomp/Es6RewriteBlockScopedDeclaration.java,
//   src/com/google/javascript/jscomp/Es6RewriteBlockScopedFunctionDeclaration.java,
//   src/com/google/javascript/jscomp/Es7RewriteExponentialOperator.java,
//   src/com/google/javascript/jscomp/LateEs6ToEs3Converter.java,
//   src/com/google/javascript/jscomp/RewriteLogicalAssignmentOperatorsPass.java,
//   src/com/google/javascript/jscomp/RewriteNullishCoalesceOperator.java,
//   src/com/google/javascript/jscomp/RewriteObjectSpread.java,
//   src/com/google/javascript/jscomp/RewriteOptionalChainingOperator.java,
//   src/com/google/javascript/jscomp/VarCheck.java,
//   test/com/google/javascript/jscomp/Es6ForOfConverterTest.java,
//   test/com/google/javascript/jscomp/Es6TemplateLiteralsTest.java,
//   test/com/google/javascript/jscomp/Es6TranspilationIntegrationTest.java,
//   test/com/google/javascript/jscomp/InstrumentAsyncContextTest.java,
//   test/com/google/javascript/jscomp/RewriteOptionalChainingOperatorTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the transpile-misc passes (block scoping, for-of, late ES3 conversion and
//! the ES2016-2021 operator lowerings).
use crate::{
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject, Object},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    es6_for_of_converter::Es6ForOfConverter,
    es6_rewrite_block_scoped_declaration::Es6RewriteBlockScopedDeclaration,
    es6_rewrite_block_scoped_function_declaration::Es6RewriteBlockScopedFunctionDeclaration,
    es7_rewrite_exponential_operator::Es7RewriteExponentialOperator,
    inject_transpilation_runtime_libraries::InjectTranspilationRuntimeLibraries,
    instrument_async_context::InstrumentAsyncContext,
    late_es6_to_es3_converter::LateEs6ToEs3Converter,
    make_declared_names_unique::MakeDeclaredNamesUnique,
    normalize::Normalize,
    optional_chain_rewriter::TmpVarNameCreator,
    pass_factory::PassFactory,
    pass_list_builder::PassListBuilder,
    phase_optimizer::PhaseOptimizer,
    rewrite_logical_assignment_operators_pass::RewriteLogicalAssignmentOperatorsPass,
    rewrite_nullish_coalesce_operator::RewriteNullishCoalesceOperator,
    rewrite_object_spread::RewriteObjectSpread,
    rewrite_optional_chaining_operator::RewriteOptionalChainingOperator,
    rewrite_polyfills::RewritePolyfills,
    serialization::{
        convert_types_to_colors::ConvertTypesToColors, serialization_options::SerializationOptions,
    },
    transpilation_passes::TranspilationPasses,
};
use closure_rhino::fx_hash::IndexMap;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn register(registry: &mut Registry) {
    register_borrowed(registry);
    registry.register(
        "com.google.javascript.jscomp.VarCheck#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        var_check,
    );
    registry.register(
        "com.google.javascript.jscomp.InstrumentAsyncContextTest_Helpers#<init>()",
        instrument_async_context_test_helpers,
    );
    registry.register(
        "com.google.javascript.jscomp.InstrumentAsyncContextTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.InstrumentAsyncContextTest_Helpers)",
        instrument_async_context_test_get_processor_holder,
    );
    registry.register(
        "com.google.javascript.jscomp.InstrumentAsyncContextTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)",
        instrument_async_context_test_get_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteBlockScopedDeclaration#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_rewrite_block_scoped_declaration,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6ForOfConverter#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_for_of_converter,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6ForOfConverterTest_Helpers$GetProcessor#<init>()",
        es6_for_of_converter_test_get_processor_holder,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6ForOfConverterTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)",
        es6_for_of_converter_test_get_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6TemplateLiteralsTest_Helpers$GetProcessorHost#<init>()",
        es6_template_literals_test_get_processor_host,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6TemplateLiteralsTest_Helpers$GetProcessorHost#getProcessor(com.google.javascript.jscomp.Compiler)",
        es6_template_literals_test_get_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6TranspilationIntegrationTest_Helpers$GetProcessor#<init>()",
        es6_transpilation_integration_test_get_processor_holder,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6TranspilationIntegrationTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)",
        es6_transpilation_integration_test_get_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteBlockScopedFunctionDeclaration#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_rewrite_block_scoped_function_declaration,
    );
    registry.register(
        "com.google.javascript.jscomp.Es7RewriteExponentialOperator#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es7_rewrite_exponential_operator,
    );
    registry.register(
        "com.google.javascript.jscomp.LateEs6ToEs3Converter#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        late_es6_to_es3_converter,
    );
    registry.register(
        "com.google.javascript.jscomp.RewriteLogicalAssignmentOperatorsPass#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        rewrite_logical_assignment_operators_pass,
    );
    registry.register(
        "com.google.javascript.jscomp.RewriteNullishCoalesceOperator#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        rewrite_nullish_coalesce_operator,
    );
    registry.register(
        "com.google.javascript.jscomp.RewriteObjectSpread#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        rewrite_object_spread,
    );
    registry.register(
        "com.google.javascript.jscomp.RewriteOptionalChainingOperator#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.OptionalChainRewriter$TmpVarNameCreator)",
        rewrite_optional_chaining_operator,
    );
    registry.register(
        "com.google.javascript.jscomp.RewriteOptionalChainingOperatorTest_Helpers#baseTestClassTmpVarNameCreator()",
        counting_tmp_var_name_creator,
    );
    registry.register(
        "com.google.javascript.jscomp.RewriteOptionalChainingOperatorTest_Helpers#deleteOptChainTestsTmpVarNameCreator()",
        counting_tmp_var_name_creator,
    );
}

// port: ReplayDsl#invoke (constructors called while a running pass lends the compiler, e.g. inside
// a pass-factory lambda or a processor lambda)
fn register_borrowed(registry: &mut Registry) {
    registry.register_with_compiler(
        "com.google.javascript.jscomp.Es6RewriteBlockScopedDeclaration#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_rewrite_block_scoped_declaration_with_compiler,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.Es6RewriteBlockScopedFunctionDeclaration#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_rewrite_block_scoped_function_declaration_with_compiler,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.Es6ForOfConverter#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_for_of_converter_with_compiler,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.Es7RewriteExponentialOperator#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es7_rewrite_exponential_operator_with_compiler,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.LateEs6ToEs3Converter#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        late_es6_to_es3_converter_with_compiler,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.RewriteLogicalAssignmentOperatorsPass#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        rewrite_logical_assignment_operators_pass_with_compiler,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.RewriteNullishCoalesceOperator#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        rewrite_nullish_coalesce_operator_with_compiler,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.RewriteObjectSpread#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        rewrite_object_spread_with_compiler,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.VarCheck#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        var_check_with_compiler,
    );
}

// port: Es6RewriteBlockScopedDeclaration#Es6RewriteBlockScopedDeclaration (constructed while the compiler is lent to a running pass)
fn es6_rewrite_block_scoped_declaration_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = Es6RewriteBlockScopedDeclaration::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: Es6RewriteBlockScopedFunctionDeclaration#Es6RewriteBlockScopedFunctionDeclaration (constructed while the compiler is lent to a running pass)
fn es6_rewrite_block_scoped_function_declaration_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = Es6RewriteBlockScopedFunctionDeclaration::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: Es6ForOfConverter#Es6ForOfConverter (constructed while the compiler is lent to a running pass)
fn es6_for_of_converter_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = Es6ForOfConverter::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: Es7RewriteExponentialOperator#Es7RewriteExponentialOperator (constructed while the compiler is lent to a running pass)
fn es7_rewrite_exponential_operator_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = Es7RewriteExponentialOperator::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: LateEs6ToEs3Converter#LateEs6ToEs3Converter (constructed while the compiler is lent to a running pass)
fn late_es6_to_es3_converter_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = LateEs6ToEs3Converter::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: RewriteLogicalAssignmentOperatorsPass#RewriteLogicalAssignmentOperatorsPass (constructed while the compiler is lent to a running pass)
fn rewrite_logical_assignment_operators_pass_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = RewriteLogicalAssignmentOperatorsPass::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: RewriteNullishCoalesceOperator#RewriteNullishCoalesceOperator (constructed while the compiler is lent to a running pass)
fn rewrite_nullish_coalesce_operator_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = RewriteNullishCoalesceOperator::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: RewriteObjectSpread#RewriteObjectSpread (constructed while the compiler is lent to a running pass)
fn rewrite_object_spread_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = RewriteObjectSpread::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: VarCheck#VarCheck (constructed while the compiler is lent to a running pass)
fn var_check_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let pass = closure_jscomp::var_check::VarCheck::new(compiler);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: VarCheck#VarCheck
fn var_check(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = closure_jscomp::var_check::VarCheck::new(&c.borrow());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: ReplayDsl#invoke (receiver cast)
fn compiler(args: &[DslValue]) -> Result<CompilerHandle, Throwable> {
    if let Some(DslValue::Compiler(c)) = args.first() {
        Ok(c.clone())
    } else {
        Err(Throwable::HarnessError(
            "native method arguments do not match the resolved Java signature".into(),
        ))
    }
}

// port: Es7RewriteExponentialOperator#Es7RewriteExponentialOperator
fn es7_rewrite_exponential_operator(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = Es7RewriteExponentialOperator::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: LateEs6ToEs3Converter#LateEs6ToEs3Converter
fn late_es6_to_es3_converter(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = LateEs6ToEs3Converter::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: RewriteLogicalAssignmentOperatorsPass#RewriteLogicalAssignmentOperatorsPass
fn rewrite_logical_assignment_operators_pass(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = RewriteLogicalAssignmentOperatorsPass::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: RewriteNullishCoalesceOperator#RewriteNullishCoalesceOperator
fn rewrite_nullish_coalesce_operator(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = RewriteNullishCoalesceOperator::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: RewriteObjectSpread#RewriteObjectSpread
fn rewrite_object_spread(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = RewriteObjectSpread::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

const TMP_VAR_NAME_CREATOR_CLASS: &str =
    "com.google.javascript.jscomp.OptionalChainRewriter$TmpVarNameCreator";

/// A TmpVarNameCreator value, held until the RewriteOptionalChainingOperator constructor takes it.
struct NativeTmpVarNameCreator {
    creator: Arc<dyn TmpVarNameCreator + Send + Sync>,
}
impl NativeObject for NativeTmpVarNameCreator {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        TMP_VAR_NAME_CREATOR_CLASS
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The anonymous counting TmpVarNameCreator of RewriteOptionalChainingOperatorTest's
/// BaseTestClass and DeleteOptChainTests (lines 401-410 and 434-443, identical).
struct CountingTmpVarNameCreator {
    counter: AtomicI32,
}
impl TmpVarNameCreator for CountingTmpVarNameCreator {
    // port: RewriteOptionalChainingOperatorTest_Helpers (anonymous TmpVarNameCreator)#createTmpVarName
    fn create_tmp_var_name(&self, _compiler: &mut closure_jscomp::AbstractCompiler) -> String {
        format!("tmp{}", self.counter.fetch_add(1, Ordering::SeqCst))
    }
}

// port: RewriteOptionalChainingOperatorTest_Helpers#baseTestClassTmpVarNameCreator
// port: RewriteOptionalChainingOperatorTest_Helpers#deleteOptChainTestsTmpVarNameCreator
fn counting_tmp_var_name_creator(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    // Just name temporary variables "tmp0", "tmp1", etc. to make the tests clearer.
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeTmpVarNameCreator {
            creator: Arc::new(CountingTmpVarNameCreator {
                counter: AtomicI32::new(0),
            }),
        },
    ))))
}

// port: RewriteOptionalChainingOperator#RewriteOptionalChainingOperator(AbstractCompiler, TmpVarNameCreator)
fn rewrite_optional_chaining_operator(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let creator = match args.get(1).map(DslValue::untyped) {
        Some(DslValue::Native(native)) => {
            let mut native = native.borrow_mut();
            match native
                .as_any_mut()
                .downcast_mut::<NativeTmpVarNameCreator>()
            {
                Some(n) => n.creator.clone(),
                None => {
                    return Err(Throwable::HarnessError(
                        "native method arguments do not match the resolved Java signature".into(),
                    ));
                }
            }
        }
        _ => {
            return Err(Throwable::HarnessError(
                "native method arguments do not match the resolved Java signature".into(),
            ));
        }
    };
    let pass = RewriteOptionalChainingOperator::new_with_tmp_var_name_creator(
        &mut c.borrow_mut(),
        creator,
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: Es6RewriteBlockScopedFunctionDeclaration#Es6RewriteBlockScopedFunctionDeclaration
fn es6_rewrite_block_scoped_function_declaration(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = Es6RewriteBlockScopedFunctionDeclaration::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: Es6ForOfConverter#Es6ForOfConverter
fn es6_for_of_converter(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = Es6ForOfConverter::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

const ES6_FOR_OF_CONVERTER_TEST_GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.Es6ForOfConverterTest_Helpers$GetProcessor";

// port: Es6ForOfConverterTest_Helpers.GetProcessor#GetProcessor
fn es6_for_of_converter_test_get_processor_holder(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: ES6_FOR_OF_CONVERTER_TEST_GET_PROCESSOR.into(),
        fields: IndexMap::<_, _>::default(),
        field_types: IndexMap::<_, _>::default(),
    }))))
}

// port: Es6ForOfConverterTest_Helpers.GetProcessor#getProcessor
fn es6_for_of_converter_test_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(_), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "Es6ForOfConverterTest_Helpers$GetProcessor#getProcessor arguments".into(),
        ));
    };
    let mut optimizer = PhaseOptimizer::new(&compiler.borrow(), None);
    // makePassFactory(
    //     "injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new)
    optimizer.add_one_time_pass(
        PassFactory::builder()
            .set_name("injectTranspilationRuntimeLibraries")
            .set_internal_factory(Arc::new(|c| {
                Box::new(InjectTranspilationRuntimeLibraries::new(c))
            }))
            .build(),
    );
    // makePassFactory("es6ForOfConverter", Es6ForOfConverter::new)
    optimizer.add_one_time_pass(
        PassFactory::builder()
            .set_name("es6ForOfConverter")
            .set_internal_factory(Arc::new(|compiler| {
                Box::new(Es6ForOfConverter::new(compiler))
            }))
            .build(),
    );
    let pass: Box<dyn CompilerPass> = Box::new(optimizer);
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

const ES6_TEMPLATE_LITERALS_TEST_GET_PROCESSOR_HOST: &str =
    "com.google.javascript.jscomp.Es6TemplateLiteralsTest_Helpers$GetProcessorHost";

// port: Es6TemplateLiteralsTest_Helpers.GetProcessorHost#GetProcessorHost
fn es6_template_literals_test_get_processor_host(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: ES6_TEMPLATE_LITERALS_TEST_GET_PROCESSOR_HOST.into(),
        fields: IndexMap::<_, _>::default(),
        field_types: IndexMap::<_, _>::default(),
    }))))
}

// port: Es6TemplateLiteralsTest_Helpers.GetProcessorHost#getProcessor
// port: Es6TemplateLiteralsTest#getProcessor
fn es6_template_literals_test_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(_), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "Es6TemplateLiteralsTest_Helpers$GetProcessorHost#getProcessor arguments".into(),
        ));
    };
    let mut optimizer = PhaseOptimizer::new(&compiler.borrow(), None);
    // makePassFactory(
    //     "injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new)
    optimizer.add_one_time_pass(
        PassFactory::builder()
            .set_name("injectTranspilationRuntimeLibraries")
            .set_internal_factory(Arc::new(|c| {
                Box::new(InjectTranspilationRuntimeLibraries::new(c))
            }))
            .build(),
    );
    // makePassFactory("lateEs6ToEs3Converter", LateEs6ToEs3Converter::new)
    optimizer.add_one_time_pass(
        PassFactory::builder()
            .set_name("lateEs6ToEs3Converter")
            .set_internal_factory(Arc::new(|c| Box::new(LateEs6ToEs3Converter::new(c))))
            .build(),
    );
    let pass: Box<dyn CompilerPass> = Box::new(optimizer);
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

const ES6_TRANSPILATION_INTEGRATION_TEST_GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.Es6TranspilationIntegrationTest_Helpers$GetProcessor";

// port: Es6TranspilationIntegrationTest_Helpers.GetProcessor#GetProcessor
fn es6_transpilation_integration_test_get_processor_holder(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: ES6_TRANSPILATION_INTEGRATION_TEST_GET_PROCESSOR.into(),
        fields: IndexMap::<_, _>::default(),
        field_types: IndexMap::<_, _>::default(),
    }))))
}

// port: Es6TranspilationIntegrationTest_Helpers.GetProcessor#getProcessor
// port: Es6TranspilationIntegrationTest#getProcessor
fn es6_transpilation_integration_test_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(_), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "Es6TranspilationIntegrationTest_Helpers$GetProcessor#getProcessor arguments".into(),
        ));
    };
    let mut optimizer = PhaseOptimizer::new(&compiler.borrow(), None);

    let compiler_options = compiler.borrow().get_options().clone();
    let mut passes = PassListBuilder::new(compiler_options.clone());

    passes.maybe_add(
        PassFactory::builder()
            .set_name("es6InjectRuntimeLibraries")
            .set_internal_factory(Arc::new(|c| {
                Box::new(InjectTranspilationRuntimeLibraries::new(c))
            }))
            .build(),
    );

    passes.maybe_add(
        PassFactory::builder()
            .set_name("rewritePolyfills")
            .set_internal_factory(Arc::new(|c| {
                Box::new(RewritePolyfills::new(
                    c, /* injectPolyfills= */ true, /* isolatePolyfills= */ false, None,
                ))
            }))
            .build(),
    );

    passes.maybe_add(
        PassFactory::builder()
            .set_name("convertTypesToColors")
            .set_internal_factory(Arc::new(|_c| {
                Box::new(ConvertTypesToColors::new(
                    SerializationOptions::builder()
                        .set_include_debug_info(true)
                        .build(),
                ))
            }))
            .build(),
    );

    passes.maybe_add(
        PassFactory::builder()
            .set_name(closure_jscomp::pass_names::NORMALIZE)
            .set_internal_factory(Arc::new(|abstract_compiler| {
                Box::new(Normalize::builder(abstract_compiler).build())
            }))
            .build(),
    );
    TranspilationPasses::add_transpilation_passes(&mut passes, &compiler_options);
    // Since we're testing the transpile-only case, we need to put back the original variable names
    // where possible once transpilation is complete. This matches the behavior in
    // DefaultPassConfig. See comments there for further explanation.
    passes.maybe_add(
        PassFactory::builder()
            .set_name("invertContextualRenaming")
            .set_internal_factory(Arc::new(
                MakeDeclaredNamesUnique::get_contextual_rename_inverter,
            ))
            .build(),
    );
    optimizer.consume(passes.build_including_unported());

    let pass: Box<dyn CompilerPass> = Box::new(optimizer);
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: Es6RewriteBlockScopedDeclaration#Es6RewriteBlockScopedDeclaration
fn es6_rewrite_block_scoped_declaration(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = Es6RewriteBlockScopedDeclaration::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

const INSTRUMENT_ASYNC_CONTEXT_TEST_HELPERS: &str =
    "com.google.javascript.jscomp.InstrumentAsyncContextTest_Helpers";
const INSTRUMENT_ASYNC_CONTEXT_TEST_GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.InstrumentAsyncContextTest_Helpers$GetProcessor";

// port: InstrumentAsyncContextTest_Helpers#InstrumentAsyncContextTest_Helpers
fn instrument_async_context_test_helpers(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    // private boolean instrumentAwait = true;
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    fields.insert("instrumentAwait".to_string(), DslValue::Bool(true));
    field_types.insert("instrumentAwait".to_string(), "boolean".to_string());
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: INSTRUMENT_ASYNC_CONTEXT_TEST_HELPERS.into(),
        fields,
        field_types,
    }))))
}

// port: InstrumentAsyncContextTest_Helpers.GetProcessor#GetProcessor
fn instrument_async_context_test_get_processor_holder(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "InstrumentAsyncContextTest_Helpers$GetProcessor#<init> arguments".into(),
        ));
    };
    let mut fields = IndexMap::<_, _>::default();
    fields.insert("this$0".to_string(), DslValue::Object(outer.clone()));
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: INSTRUMENT_ASYNC_CONTEXT_TEST_GET_PROCESSOR.into(),
        fields,
        field_types: IndexMap::<_, _>::default(),
    }))))
}

// port: InstrumentAsyncContextTest_Helpers.GetProcessor#getProcessor
fn instrument_async_context_test_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(this), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "InstrumentAsyncContextTest_Helpers$GetProcessor#getProcessor arguments".into(),
        ));
    };
    // The lambda reads the outer instance's instrumentAwait field; it is fixed before the
    // processor runs.
    let instrument_await = match this.borrow().fields.get("this$0") {
        Some(DslValue::Object(outer)) => match outer.borrow().fields.get("instrumentAwait") {
            Some(DslValue::Bool(value)) => *value,
            _ => {
                return Err(Throwable::HarnessError(
                    "InstrumentAsyncContextTest_Helpers#instrumentAwait".into(),
                ));
            }
        },
        _ => {
            return Err(Throwable::HarnessError(
                "InstrumentAsyncContextTest_Helpers$GetProcessor outer instance".into(),
            ));
        }
    };
    let mut optimizer = PhaseOptimizer::new(&compiler.borrow(), None);
    // makePassFactory(
    //     "injectTranspilationRuntimeLibraries",
    //     (c) -> new InjectTranspilationRuntimeLibraries(compiler, true))
    optimizer.add_one_time_pass(
        PassFactory::builder()
            .set_name("injectTranspilationRuntimeLibraries")
            .set_internal_factory(Arc::new(|c| {
                Box::new(
                    InjectTranspilationRuntimeLibraries::new_with_instrument_async_context(c, true),
                )
            }))
            .build(),
    );
    // makePassFactory(
    //     "instrumentAsyncContext", (c) -> new InstrumentAsyncContext(compiler, instrumentAwait))
    optimizer.add_one_time_pass(
        PassFactory::builder()
            .set_name("instrumentAsyncContext")
            .set_internal_factory(Arc::new(move |compiler| {
                Box::new(InstrumentAsyncContext::new(compiler, instrument_await))
            }))
            .build(),
    );
    let pass: Box<dyn CompilerPass> = Box::new(optimizer);
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
