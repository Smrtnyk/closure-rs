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
/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Es6RewriteArrowFunction.java,
//   src/com/google/javascript/jscomp/InjectTranspilationRuntimeLibraries.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/Es6ConvertSuperTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteClassTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteDestructuringTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteRestParametersTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteSpreadExpressionsTest.java.

//! Replay adapters for the transpile-es2015 classes: the Es6RewriteArrowFunction constructor
//! (Es6RewriteArrowFunctionTest's processor expression) and ports of the replay helpers
//! `oracle/replay/helpers/.../Es6{RewriteClass,ConvertSuper,RewriteDestructuring,
//! RewriteRestParameters,RewriteSpreadExpressions}Test_Helpers.java`, themselves copied from the
//! `getProcessor` methods of the matching Java tests.
use crate::{
    replay::{
        native_registry::NativePhaseOptimizer,
        options_values::OptionValue,
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::inject_transpilation_runtime_libraries::InjectTranspilationRuntimeLibraries;
use closure_jscomp::{
    compiler_options::Es6SubclassTranspilation,
    compiler_pass::CompilerPass,
    es6_convert_super::Es6ConvertSuper,
    es6_normalize_classes::Es6NormalizeClasses,
    es6_rewrite_arrow_function::Es6RewriteArrowFunction,
    es6_rewrite_class::Es6RewriteClass,
    es6_rewrite_destructuring::{self, ObjectDestructuringRewriteMode},
    es6_rewrite_rest_parameters::Es6RewriteRestParameters,
    es6_rewrite_spread_expressions::Es6RewriteSpreadExpressions,
    pass_factory::PassFactory,
    phase_optimizer::PhaseOptimizer,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc, sync::Arc};

const CLASS_HELPERS: &str = "com.google.javascript.jscomp.Es6RewriteClassTest_Helpers";
const CLASS_GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.Es6RewriteClassTest_Helpers$GetProcessor";
const CONVERT_SUPER_GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.Es6ConvertSuperTest_Helpers$GetProcessor";
const DESTRUCTURING_HELPERS: &str =
    "com.google.javascript.jscomp.Es6RewriteDestructuringTest_Helpers";
const DESTRUCTURING_GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.Es6RewriteDestructuringTest_Helpers$GetProcessor";
const OBJECT_DESTRUCTURING_REWRITE_MODE: &str =
    "com.google.javascript.jscomp.Es6RewriteDestructuring$ObjectDestructuringRewriteMode";

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn register(registry: &mut Registry) {
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteArrowFunction#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_rewrite_arrow_function,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6ConvertSuperTest_Helpers$GetProcessor#<init>()",
        convert_super_get_processor_new,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6ConvertSuperTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)",
        convert_super_get_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteClassTest_Helpers#<init>()",
        class_helpers_new,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteClassTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.Es6RewriteClassTest_Helpers)",
        class_get_processor_new,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteClassTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)",
        class_get_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteDestructuringTest_Helpers#<init>()",
        destructuring_helpers_new,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteDestructuringTest_Helpers$GetProcessor#<init>(com.google.javascript.jscomp.Es6RewriteDestructuringTest_Helpers)",
        destructuring_get_processor_new,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteDestructuringTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)",
        destructuring_get_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteRestParametersTest_Helpers$GetProcessorPass#<init>(com.google.javascript.jscomp.Compiler)",
        rest_parameters_get_processor_pass,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6RewriteSpreadExpressionsTest_Helpers$GetProcessorLambda#<init>(com.google.javascript.jscomp.Compiler)",
        spread_expressions_get_processor_lambda,
    );
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

// port: ReplayDsl#invoke (receiver cast)
fn compiler(args: &[DslValue]) -> Result<CompilerHandle, Throwable> {
    if let Some(DslValue::Compiler(c)) = args.first() {
        Ok(c.clone())
    } else {
        Err(bad())
    }
}

// port: CompilerTestCase#makePassFactory
fn make_pass_factory(
    name: &'static str,
    pass: impl Fn(&mut closure_jscomp::AbstractCompiler) -> Box<dyn CompilerPass>
    + Send
    + Sync
    + 'static,
) -> PassFactory {
    PassFactory::builder()
        .set_name(name)
        .set_internal_factory(Arc::new(pass))
        .build()
}

/// `makePassFactory("injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new)`
// port: CompilerTestCase#makePassFactory
fn inject_transpilation_runtime_libraries_factory() -> PassFactory {
    PassFactory::builder()
        .set_name("injectTranspilationRuntimeLibraries")
        .set_internal_factory(Arc::new(|compiler| {
            Box::new(InjectTranspilationRuntimeLibraries::new(compiler))
        }))
        .build()
}

// port: ReplayDsl#invoke (PhaseOptimizer return value)
fn phase_optimizer_value(optimizer: PhaseOptimizer) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativePhaseOptimizer(optimizer))))
}

// port: Es6RewriteArrowFunction#Es6RewriteArrowFunction
fn es6_rewrite_arrow_function(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = Es6RewriteArrowFunction::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

/// A helper value with no fields; its methods are dispatched by the registered signatures.
struct HelperInstance {
    class: &'static str,
    outer: Option<Rc<RefCell<dyn NativeObject>>>,
}

impl NativeObject for HelperInstance {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        self.class
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayDsl#invoke (inner helper receiver)
fn receiver_outer(
    args: &[DslValue],
    class: &str,
) -> Result<Rc<RefCell<dyn NativeObject>>, Throwable> {
    let Some(DslValue::Native(receiver)) = args.first() else {
        return Err(bad());
    };
    let mut receiver = receiver.borrow_mut();
    let receiver = receiver
        .as_any_mut()
        .downcast_mut::<HelperInstance>()
        .ok_or_else(bad)?;
    if receiver.class != class {
        return Err(bad());
    }
    receiver.outer.clone().ok_or_else(bad)
}

// port: ReplayDsl#invoke (receiver and compiler arguments of getProcessor)
fn get_processor_compiler(args: &[DslValue]) -> Result<CompilerHandle, Throwable> {
    let [DslValue::Native(_), DslValue::Compiler(compiler)] = args else {
        return Err(bad());
    };
    Ok(compiler.clone())
}

// ---------------------------------------------------------------------------------------------
// Es6ConvertSuperTest_Helpers

// port: Es6ConvertSuperTest_Helpers.GetProcessor#GetProcessor
fn convert_super_get_processor_new(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(HelperInstance {
        class: CONVERT_SUPER_GET_PROCESSOR,
        outer: None,
    }))))
}

// port: Es6ConvertSuperTest_Helpers.GetProcessor#getProcessor
fn convert_super_get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let compiler = get_processor_compiler(&args)?;
    let mut optimizer = PhaseOptimizer::new(&compiler.borrow(), None);
    optimizer.add_one_time_pass(make_pass_factory("es6NormalizeClasses", |c| {
        Box::new(Es6NormalizeClasses::new(c))
    }));
    optimizer.add_one_time_pass(make_pass_factory("es6ConvertSuper", |c| {
        Box::new(Es6ConvertSuper::new(c))
    }));
    Ok(phase_optimizer_value(optimizer))
}

// ---------------------------------------------------------------------------------------------
// Es6RewriteClassTest_Helpers

/// The helper holder with its field `es6SubclassTranspilation` (Java default null).
struct Es6RewriteClassTestHelpers {
    es6_subclass_transpilation: Option<Es6SubclassTranspilation>,
}

impl NativeObject for Es6RewriteClassTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        CLASS_HELPERS
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "es6SubclassTranspilation".into(),
            match &self.es6_subclass_transpilation {
                Some(value) => value.encode_value()?,
                None => DslValue::Null,
            },
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        if name != "es6SubclassTranspilation" {
            return Err(Throwable::Unported(format!("{CLASS_HELPERS}#{name}")));
        }
        self.es6_subclass_transpilation = match value.untyped() {
            DslValue::Null => None,
            _ => Some(Es6SubclassTranspilation::decode_value(&value)?),
        };
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: Es6RewriteClassTest_Helpers#Es6RewriteClassTest_Helpers
fn class_helpers_new(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        Es6RewriteClassTestHelpers {
            es6_subclass_transpilation: None,
        },
    ))))
}

// port: Es6RewriteClassTest_Helpers.GetProcessor#GetProcessor
fn class_get_processor_new(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(outer)] = args.as_slice() else {
        return Err(bad());
    };
    if outer.borrow().class_name() != CLASS_HELPERS {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(HelperInstance {
        class: CLASS_GET_PROCESSOR,
        outer: Some(outer.clone()),
    }))))
}

// port: Es6RewriteClassTest_Helpers.GetProcessor#getProcessor
fn class_get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let outer = receiver_outer(&args, CLASS_GET_PROCESSOR)?;
    let compiler = get_processor_compiler(&args)?;
    let es6_subclass_transpilation = {
        let mut outer = outer.borrow_mut();
        let holder = outer
            .as_any_mut()
            .downcast_mut::<Es6RewriteClassTestHelpers>()
            .ok_or_else(bad)?;
        holder.es6_subclass_transpilation
    };
    let mut optimizer = PhaseOptimizer::new(&compiler.borrow(), None);
    optimizer.add_one_time_pass(inject_transpilation_runtime_libraries_factory());
    optimizer.add_one_time_pass(make_pass_factory("es6NormalizeClasses", |c| {
        Box::new(Es6NormalizeClasses::new(c))
    }));
    optimizer.add_one_time_pass(make_pass_factory("es6ConvertSuper", |c| {
        Box::new(Es6ConvertSuper::new(c))
    }));
    optimizer.add_one_time_pass(make_pass_factory("es6RewriteClass", move |c| {
        // Java passes a null field through; Es6RewriteClass only dereferences it when it
        // rewrites a subclass constructor.
        let es6_subclass_transpilation = es6_subclass_transpilation.unwrap_or_else(|| {
            panic!("NullPointerException: es6SubclassTranspilation");
        });
        Box::new(Es6RewriteClass::new(c, es6_subclass_transpilation))
    }));
    Ok(phase_optimizer_value(optimizer))
}

// ---------------------------------------------------------------------------------------------
// Es6RewriteDestructuringTest_Helpers

/// The helper holder with its field `destructuringRewriteMode` (initialized to
/// REWRITE_ALL_OBJECT_PATTERNS).
struct Es6RewriteDestructuringTestHelpers {
    destructuring_rewrite_mode: Option<ObjectDestructuringRewriteMode>,
}

// port: ReplayValues#enumValue (ObjectDestructuringRewriteMode constants)
fn encode_rewrite_mode(mode: ObjectDestructuringRewriteMode) -> DslValue {
    DslValue::Enum {
        class: OBJECT_DESTRUCTURING_REWRITE_MODE.into(),
        name: format!("{mode:?}"),
    }
}

// port: ReplayValues#enumValue (ObjectDestructuringRewriteMode constants)
fn decode_rewrite_mode(
    value: &DslValue,
) -> Result<Option<ObjectDestructuringRewriteMode>, Throwable> {
    match value.untyped() {
        DslValue::Null => Ok(None),
        DslValue::Enum { class, name } if class == OBJECT_DESTRUCTURING_REWRITE_MODE => {
            match name.as_str() {
                "REWRITE_ALL_OBJECT_PATTERNS" => Ok(Some(
                    ObjectDestructuringRewriteMode::REWRITE_ALL_OBJECT_PATTERNS,
                )),
                "REWRITE_OBJECT_REST" => {
                    Ok(Some(ObjectDestructuringRewriteMode::REWRITE_OBJECT_REST))
                }
                _ => Err(bad()),
            }
        }
        _ => Err(bad()),
    }
}

impl NativeObject for Es6RewriteDestructuringTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        DESTRUCTURING_HELPERS
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "destructuringRewriteMode".into(),
            self.destructuring_rewrite_mode
                .map_or(DslValue::Null, encode_rewrite_mode),
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        if name != "destructuringRewriteMode" {
            return Err(Throwable::Unported(format!(
                "{DESTRUCTURING_HELPERS}#{name}"
            )));
        }
        self.destructuring_rewrite_mode = decode_rewrite_mode(&value)?;
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: Es6RewriteDestructuringTest_Helpers#Es6RewriteDestructuringTest_Helpers
fn destructuring_helpers_new(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        Es6RewriteDestructuringTestHelpers {
            destructuring_rewrite_mode: Some(
                ObjectDestructuringRewriteMode::REWRITE_ALL_OBJECT_PATTERNS,
            ),
        },
    ))))
}

// port: Es6RewriteDestructuringTest_Helpers.GetProcessor#GetProcessor
fn destructuring_get_processor_new(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(outer)] = args.as_slice() else {
        return Err(bad());
    };
    if outer.borrow().class_name() != DESTRUCTURING_HELPERS {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(HelperInstance {
        class: DESTRUCTURING_GET_PROCESSOR,
        outer: Some(outer.clone()),
    }))))
}

// port: Es6RewriteDestructuringTest_Helpers.GetProcessor#getProcessor
fn destructuring_get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let outer = receiver_outer(&args, DESTRUCTURING_GET_PROCESSOR)?;
    let compiler = get_processor_compiler(&args)?;
    let destructuring_rewrite_mode = {
        let mut outer = outer.borrow_mut();
        let holder = outer
            .as_any_mut()
            .downcast_mut::<Es6RewriteDestructuringTestHelpers>()
            .ok_or_else(bad)?;
        holder.destructuring_rewrite_mode
    };
    let mut optimizer = PhaseOptimizer::new(&compiler.borrow(), None);
    optimizer.add_one_time_pass(inject_transpilation_runtime_libraries_factory());
    optimizer.add_one_time_pass(make_pass_factory("es6RewriteDestructuring", move |c| {
        let destructuring_rewrite_mode = destructuring_rewrite_mode.unwrap_or_else(|| {
            panic!("NullPointerException: destructuringRewriteMode");
        });
        Box::new(
            es6_rewrite_destructuring::Builder::new(c)
                .set_destructuring_rewrite_mode(destructuring_rewrite_mode)
                .build(),
        )
    }));
    Ok(phase_optimizer_value(optimizer))
}

// ---------------------------------------------------------------------------------------------
// Es6RewriteRestParametersTest_Helpers / Es6RewriteSpreadExpressionsTest_Helpers

/// `new InjectTranspilationRuntimeLibraries(compiler).process(externs, root)`
// port: InjectTranspilationRuntimeLibraries#process
fn inject_transpilation_runtime_libraries(
    compiler: &mut closure_jscomp::AbstractCompiler,
    externs: NodeId,
    root: NodeId,
) {
    InjectTranspilationRuntimeLibraries::new(compiler).process(compiler, externs, root);
}

// port: Es6RewriteRestParametersTest_Helpers.GetProcessorPass#GetProcessorPass
fn rest_parameters_get_processor_pass(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    // port: Es6RewriteRestParametersTest_Helpers.GetProcessorPass#process
    let pass: Box<dyn CompilerPass> = Box::new(
        |compiler: &mut closure_jscomp::AbstractCompiler, externs: NodeId, root: NodeId| {
            inject_transpilation_runtime_libraries(compiler, externs, root);
            Es6RewriteRestParameters::new(compiler).process(compiler, externs, root);
        },
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: Es6RewriteSpreadExpressionsTest_Helpers.GetProcessorLambda#GetProcessorLambda
fn spread_expressions_get_processor_lambda(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    // port: Es6RewriteSpreadExpressionsTest_Helpers.GetProcessorLambda#process
    let pass: Box<dyn CompilerPass> = Box::new(
        |compiler: &mut closure_jscomp::AbstractCompiler, externs: NodeId, root: NodeId| {
            inject_transpilation_runtime_libraries(compiler, externs, root);
            Es6RewriteSpreadExpressions::new(compiler).process(compiler, externs, root);
        },
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
