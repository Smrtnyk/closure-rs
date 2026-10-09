/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/RemoveUnusedCode.java,
//   test/com/google/javascript/jscomp/RemoveUnusedCodeNameAnalyzerTest.java,
//   test/com/google/javascript/jscomp/RemoveUnusedCodeTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for RemoveUnusedCode (its Builder chain and `process`), and the ports of the
//! unit-corpus helpers `RemoveUnusedCodeTestHelpers.java` and
//! `RemoveUnusedCodeNameAnalyzerTest_Helpers.java` (oracle/replay/helpers).
use crate::{
    jscomp_api::Compiler,
    replay::{
        registry::{BorrowedEntry, Entry},
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject, Object},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass,
    pure_function_identifier::Driver, remove_unused_code::RemoveUnusedCode,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const BUILDER: &str = "com.google.javascript.jscomp.RemoveUnusedCode$Builder";
const TEST_HELPERS: &str = "com.google.javascript.jscomp.RemoveUnusedCodeTestHelpers";

/// `RemoveUnusedCodeTestHelpers`'s declared no-argument constructor, which `ReplayDsl#helper`
/// instantiates for the non-static inner helper (no recorded signature row).
pub const TEST_HELPERS_INIT: &str =
    "com.google.javascript.jscomp.RemoveUnusedCodeTestHelpers#<init>()";

// port: ReplayDsl#invoke (resolved signatures backed by RemoveUnusedCode)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            builder_init
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#removeLocalVars(boolean)" => {
            remove_local_vars
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#removeGlobals(boolean)" => {
            remove_globals
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#preserveFunctionExpressionNames(boolean)" => {
            preserve_function_expression_names
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#removeUnusedPrototypeProperties(boolean)" => {
            remove_unused_prototype_properties
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#removeUnusedThisProperties(boolean)" => {
            remove_unused_this_properties
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#removeUnusedObjectDefinePropertiesDefinitions(boolean)" => {
            remove_unused_object_define_properties_definitions
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#removeUnusedPolyfills(boolean)" => {
            remove_unused_polyfills
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#assumeGettersArePure(boolean)" => {
            assume_getters_are_pure
        }
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#build()" => build,
        TEST_HELPERS_INIT => test_helpers,
        "com.google.javascript.jscomp.RemoveUnusedCodeTestHelpers$Processor#<init>(com.google.javascript.jscomp.RemoveUnusedCodeTestHelpers,com.google.javascript.jscomp.Compiler)" => {
            test_helpers_processor
        }
        "com.google.javascript.jscomp.RemoveUnusedCodeNameAnalyzerTest_Helpers$MarkNoSideEffectCallsAndRemoveUnusedCodeRunner#<init>(com.google.javascript.jscomp.Compiler)" => {
            mark_no_side_effect_calls_and_remove_unused_code_runner
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (constructors and methods while a factory or lambda borrows the compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        "com.google.javascript.jscomp.RemoveUnusedCode$Builder#build()" => build_borrowed,
        "com.google.javascript.jscomp.RemoveUnusedCode#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)" => {
            process_borrowed
        }
        "com.google.javascript.jscomp.RemoveUnusedCodeNameAnalyzerTest_Helpers$MarkNoSideEffectCallsAndRemoveUnusedCodeRunner#<init>(com.google.javascript.jscomp.Compiler)" => {
            mark_no_side_effect_calls_and_remove_unused_code_runner_borrowed
        }
        _ => return None,
    })
}

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!("{what} arguments"))
}

/// A `RemoveUnusedCode.Builder` setter, recorded until `build()` applies it to the Rust builder
/// (which borrows the compiler and so cannot live in a DSL value).
#[derive(Copy, Clone)]
enum Setter {
    RemoveLocalVars,
    RemoveGlobals,
    PreserveFunctionExpressionNames,
    RemoveUnusedPrototypeProperties,
    RemoveUnusedThisProperties,
    RemoveUnusedObjectDefinePropertiesDefinitions,
    RemoveUnusedPolyfills,
    AssumeGettersArePure,
}

/// The DSL receiver for `RemoveUnusedCode.Builder`.
struct BuilderObject {
    compiler: CompilerHandle,
    calls: Vec<(Setter, bool)>,
}

impl NativeObject for BuilderObject {
    fn class_name(&self) -> &str {
        BUILDER
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl BuilderObject {
    // port: RemoveUnusedCode.Builder#build (the recorded setters, in call order)
    fn build(&self, compiler: &AbstractCompiler) -> RemoveUnusedCode {
        let mut builder = RemoveUnusedCode::builder(compiler);
        for (setter, value) in &self.calls {
            let value = *value;
            builder = match setter {
                Setter::RemoveLocalVars => builder.remove_local_vars(value),
                Setter::RemoveGlobals => builder.remove_globals(value),
                Setter::PreserveFunctionExpressionNames => {
                    builder.preserve_function_expression_names(value)
                }
                Setter::RemoveUnusedPrototypeProperties => {
                    builder.remove_unused_prototype_properties(value)
                }
                Setter::RemoveUnusedThisProperties => builder.remove_unused_this_properties(value),
                Setter::RemoveUnusedObjectDefinePropertiesDefinitions => {
                    builder.remove_unused_object_define_properties_definitions(value)
                }
                Setter::RemoveUnusedPolyfills => builder.remove_unused_polyfills(value),
                Setter::AssumeGettersArePure => builder.assume_getters_are_pure(value),
            };
        }
        builder.build()
    }
}

fn pass(p: impl CompilerPass + 'static) -> DslValue {
    DslValue::Pass(Rc::new(RefCell::new(Box::new(p))))
}

/// The value of `Builder#build()`: a pass whose runtime class (RemoveUnusedCode) resolves the
/// `RemoveUnusedCode.process` call of RemoveUnusedCodePrototypePropertiesTest's processor.
fn remove_unused_code_value(p: RemoveUnusedCode) -> DslValue {
    DslValue::Typed {
        class: "com.google.javascript.jscomp.RemoveUnusedCode".into(),
        value: Box::new(pass(p)),
    }
}

// port: RemoveUnusedCode.Builder#Builder
fn builder_init(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad("RemoveUnusedCode$Builder"));
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(BuilderObject {
        compiler: compiler.clone(),
        calls: vec![],
    }))))
}

// port: ReplayDsl#invoke (a boolean setter of RemoveUnusedCode.Builder; returns `this`)
fn set(args: Vec<DslValue>, setter: Setter) -> Result<DslValue, Throwable> {
    let [receiver, value] = args.as_slice() else {
        return Err(bad("RemoveUnusedCode$Builder setter"));
    };
    let DslValue::Bool(value) = value.untyped() else {
        return Err(bad("RemoveUnusedCode$Builder setter"));
    };
    let DslValue::Native(object) = receiver else {
        return Err(bad("RemoveUnusedCode$Builder receiver"));
    };
    {
        let mut borrowed = object.borrow_mut();
        let Some(this) = borrowed.as_any_mut().downcast_mut::<BuilderObject>() else {
            return Err(bad("RemoveUnusedCode$Builder receiver"));
        };
        this.calls.push((setter, *value));
    }
    Ok(receiver.clone())
}

// port: RemoveUnusedCode.Builder#removeLocalVars
fn remove_local_vars(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    set(args, Setter::RemoveLocalVars)
}

// port: RemoveUnusedCode.Builder#removeGlobals
fn remove_globals(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    set(args, Setter::RemoveGlobals)
}

// port: RemoveUnusedCode.Builder#preserveFunctionExpressionNames
fn preserve_function_expression_names(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    set(args, Setter::PreserveFunctionExpressionNames)
}

// port: RemoveUnusedCode.Builder#removeUnusedPrototypeProperties
fn remove_unused_prototype_properties(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    set(args, Setter::RemoveUnusedPrototypeProperties)
}

// port: RemoveUnusedCode.Builder#removeUnusedThisProperties
fn remove_unused_this_properties(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    set(args, Setter::RemoveUnusedThisProperties)
}

// port: RemoveUnusedCode.Builder#removeUnusedObjectDefinePropertiesDefinitions
fn remove_unused_object_define_properties_definitions(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    set(args, Setter::RemoveUnusedObjectDefinePropertiesDefinitions)
}

// port: RemoveUnusedCode.Builder#removeUnusedPolyfills
fn remove_unused_polyfills(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    set(args, Setter::RemoveUnusedPolyfills)
}

// port: RemoveUnusedCode.Builder#assumeGettersArePure
fn assume_getters_are_pure(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    set(args, Setter::AssumeGettersArePure)
}

fn with_builder_object<R>(
    args: &[DslValue],
    f: impl FnOnce(&BuilderObject) -> R,
) -> Result<R, Throwable> {
    let [DslValue::Native(object)] = args else {
        return Err(bad("RemoveUnusedCode$Builder.build"));
    };
    let mut borrowed = object.borrow_mut();
    let Some(this) = borrowed.as_any_mut().downcast_mut::<BuilderObject>() else {
        return Err(bad("RemoveUnusedCode$Builder.build"));
    };
    Ok(f(this))
}

// port: RemoveUnusedCode.Builder#build
fn build(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let built = with_builder_object(&args, |this| {
        let compiler = this.compiler.clone();
        let compiler = compiler.borrow();
        this.build(&compiler)
    })?;
    Ok(remove_unused_code_value(built))
}

// port: RemoveUnusedCode.Builder#build (while the compiler is borrowed)
fn build_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let built = with_builder_object(&args, |this| this.build(compiler))?;
    Ok(remove_unused_code_value(built))
}

// port: RemoveUnusedCode#process
fn process_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let (DslValue::Pass(p), [DslValue::Node(externs), DslValue::Node(root)]) =
        (args[0].untyped(), &args[1..])
    else {
        return Err(bad("RemoveUnusedCode.process"));
    };
    p.borrow_mut().process(compiler, *externs, *root);
    Ok(DslValue::Null)
}

// port: RemoveUnusedCodeTestHelpers#RemoveUnusedCodeTestHelpers
fn test_helpers(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    for name in ["removeGlobal", "preserveFunctionExpressionNames"] {
        fields.insert(name.to_string(), DslValue::Bool(false));
        field_types.insert(name.to_string(), "boolean".to_string());
    }
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: TEST_HELPERS.into(),
        fields,
        field_types,
    }))))
}

/// RemoveUnusedCodeTestHelpers.Processor: delegates to the anonymous pass of
/// RemoveUnusedCodeTest#getProcessor, which reads the outer instance's fields at process time.
struct TestHelpersProcessor {
    outer: Rc<RefCell<Object>>,
}

// port: RemoveUnusedCodeTestHelpers.Processor#Processor
fn test_helpers_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad("RemoveUnusedCodeTestHelpers$Processor"));
    };
    Ok(pass(TestHelpersProcessor {
        outer: outer.clone(),
    }))
}

impl TestHelpersProcessor {
    // port: RemoveUnusedCodeTestHelpers (boolean field read)
    fn field(&self, name: &str) -> bool {
        matches!(
            self.outer.borrow().fields.get(name),
            Some(DslValue::Bool(true))
        )
    }
}

impl CompilerPass for TestHelpersProcessor {
    // port: RemoveUnusedCodeTestHelpers#getProcessor (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        RemoveUnusedCode::builder(compiler)
            .remove_local_vars(true)
            .remove_globals(self.field("removeGlobal"))
            .remove_unused_polyfills(true)
            .preserve_function_expression_names(self.field("preserveFunctionExpressionNames"))
            .build()
            .process(compiler, externs, root);
    }
}

// port: RemoveUnusedCodeNameAnalyzerTest_Helpers.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner
struct MarkNoSideEffectCallsAndRemoveUnusedCodeRunner {
    pure_function_identifier: Driver,
    remove_unused_code: RemoveUnusedCode,
}

impl MarkNoSideEffectCallsAndRemoveUnusedCodeRunner {
    // port: RemoveUnusedCodeNameAnalyzerTest_Helpers.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner#MarkNoSideEffectCallsAndRemoveUnusedCodeRunner
    fn new(compiler: &AbstractCompiler) -> Self {
        Self {
            pure_function_identifier: Driver::new(),
            remove_unused_code: RemoveUnusedCode::builder(compiler)
                .remove_globals(true)
                .remove_local_vars(true)
                .remove_unused_prototype_properties(true)
                .remove_unused_this_properties(true)
                .remove_unused_object_define_properties_definitions(true)
                // Removal of function expression names isn't what these tests are about.
                // It just adds noise to the tests when we can't use testSame() because of it.
                .preserve_function_expression_names(true)
                .build(),
        }
    }
}

impl CompilerPass for MarkNoSideEffectCallsAndRemoveUnusedCodeRunner {
    // port: RemoveUnusedCodeNameAnalyzerTest_Helpers.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.pure_function_identifier
            .process(compiler, externs, root);
        self.remove_unused_code.process(compiler, externs, root);
    }
}

// port: RemoveUnusedCodeNameAnalyzerTest_Helpers.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner#MarkNoSideEffectCallsAndRemoveUnusedCodeRunner
fn mark_no_side_effect_calls_and_remove_unused_code_runner(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad("MarkNoSideEffectCallsAndRemoveUnusedCodeRunner"));
    };
    let runner = MarkNoSideEffectCallsAndRemoveUnusedCodeRunner::new(&compiler.borrow());
    Ok(pass(runner))
}

// port: RemoveUnusedCodeNameAnalyzerTest_Helpers.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner#MarkNoSideEffectCallsAndRemoveUnusedCodeRunner
fn mark_no_side_effect_calls_and_remove_unused_code_runner_borrowed(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    Ok(pass(MarkNoSideEffectCallsAndRemoveUnusedCodeRunner::new(
        compiler,
    )))
}
