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
 * Copyright 2019 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
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
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/serialization/SerializationOptions.java,
//   src/com/google/javascript/jscomp/serialization/SerializeTypedAstPass.java,
//   test/com/google/javascript/jscomp/serialization/SerializeAndDeserializeAstTest.java,
//   test/com/google/javascript/jscomp/serialization/SerializeTypedAstPassTest.java.

//! Replay adapters for `SerializeTypedAstPass#<init>(AbstractCompiler, Consumer,
//! SerializationOptions)`, `SerializationOptions#builder` and its AutoBuilder setters, and the
//! replay helpers holding the test's `Consumer<TypedAst>` lambda:
//! `oracle/replay/helpers/.../serialization/SerializeTypedAstPassTest_Compile.java` and
//! `oracle/replay/helpers/.../SerializeAndDeserializeAstTest_Helpers.java` (each a verbatim copy
//! of `TypedAst[] result = new TypedAst[1]; consumer = ast -> result[0] = ast;`).
use crate::{
    jscomp_api::Compiler,
    proto_neutral,
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    serialization::{
        serialization_options::{SerializationOptions, SerializationOptionsBuilder},
        serialize_typed_ast_pass::SerializeTypedAstPass,
        typed_ast_proto::TypedAst,
    },
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const TYPED_AST: &str = "com.google.javascript.jscomp.serialization.TypedAst";
const COMPILE_HOLDER: &str =
    "com.google.javascript.jscomp.serialization.SerializeTypedAstPassTest_Compile";
const AST_TEST_HOLDER: &str = "com.google.javascript.jscomp.SerializeAndDeserializeAstTest_Helpers";
const OPTIONS: &str = "com.google.javascript.jscomp.serialization.SerializationOptions";
const OPTIONS_BUILDER: &str =
    "com.google.javascript.jscomp.serialization.AutoBuilder_SerializationOptions_Builder";
const PASS: &str = "com.google.javascript.jscomp.serialization.SerializeTypedAstPass";

/// The lambda `ast -> result[0] = ast` over a fresh `TypedAst[] result = new TypedAst[1]`.
pub struct TypedAstConsumerLambda {
    class: String,
    result: Rc<RefCell<[Option<TypedAst>; 1]>>,
}

impl TypedAstConsumerLambda {
    // port: SerializeTypedAstPassTest_Compile#SerializeTypedAstPassTest_Compile (the lambda)
    fn new(holder: &str) -> Rc<RefCell<TypedAstConsumerLambda>> {
        Rc::new(RefCell::new(TypedAstConsumerLambda {
            class: format!("{holder}$$Lambda"),
            result: Rc::new(RefCell::new([None])),
        }))
    }
}

impl NativeObject for TypedAstConsumerLambda {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        &self.class
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: UnitRecorder#dump (lambda captures: the TypedAst[1] array)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        let items = self
            .result
            .borrow()
            .iter()
            .map(|ast| match ast {
                None => Ok(DslValue::Null),
                Some(ast) => proto_neutral::to_proto_message("jscomp.TypedAst", ast)
                    .map(DslValue::Proto)
                    .map_err(Throwable::HarnessError),
            })
            .collect::<Result<Vec<_>, _>>();
        Some(items.map(|items| {
            let mut captures = IndexMap::<_, _>::default();
            captures.insert(
                "arg$1".to_string(),
                DslValue::Array {
                    component: TYPED_AST.to_string(),
                    items,
                },
            );
            captures
        }))
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A holder with one `Consumer<TypedAst>` field (`astConsumer` / `consumer`).
pub struct ConsumerHolder {
    class: &'static str,
    field: &'static str,
    consumer: Option<Rc<RefCell<TypedAstConsumerLambda>>>,
}

impl NativeObject for ConsumerHolder {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        self.class
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            self.field.to_string(),
            match &self.consumer {
                None => DslValue::Null,
                Some(consumer) => {
                    let consumer: Rc<RefCell<dyn NativeObject>> = consumer.clone();
                    DslValue::Native(consumer)
                }
            },
        );
        Ok(fields)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: SerializeTypedAstPassTest_Compile#SerializeTypedAstPassTest_Compile
pub fn compile_holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    // TypedAst[] resultAst = new TypedAst[1];
    // astConsumer = (ast) -> resultAst[0] = ast;
    Ok(DslValue::Native(Rc::new(RefCell::new(ConsumerHolder {
        class: COMPILE_HOLDER,
        field: "astConsumer",
        consumer: Some(TypedAstConsumerLambda::new(COMPILE_HOLDER)),
    }))))
}

// port: SerializeAndDeserializeAstTest_Helpers#SerializeAndDeserializeAstTest_Helpers
pub fn ast_test_holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(ConsumerHolder {
        class: AST_TEST_HOLDER,
        field: "consumer",
        consumer: None,
    }))))
}

// port: SerializeAndDeserializeAstTest_Helpers#toInputStream
// port: SerializeAndDeserializeAstTest_Helpers#runWithoutExpected
pub fn ast_test_new_consumer(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this)] = args.as_slice() else {
        return Err(bad());
    };
    let mut this = this.borrow_mut();
    let this = this
        .as_any_mut()
        .downcast_mut::<ConsumerHolder>()
        .ok_or_else(bad)?;
    // TypedAst[] result = new TypedAst[1];
    // consumer = ast -> result[0] = ast;
    this.consumer = Some(TypedAstConsumerLambda::new(AST_TEST_HOLDER));
    Ok(DslValue::Null)
}

/// `SerializationOptions.Builder` (AutoBuilder_SerializationOptions_Builder). The recorded
/// `runtimeLibraries` value is kept for the object dump.
struct OptionsBuilder {
    builder: SerializationOptionsBuilder,
    runtime_libraries: DslValue,
}

impl NativeObject for OptionsBuilder {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        OPTIONS_BUILDER
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `SerializationOptions` (fields as the recorder dumps them).
struct Options {
    options: SerializationOptions,
    runtime_libraries: DslValue,
}

impl NativeObject for Options {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        OPTIONS
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "includeDebugInfo".into(),
            DslValue::Bool(self.options.include_debug_info()),
        );
        fields.insert(
            "runValidation".into(),
            DslValue::Bool(self.options.run_validation()),
        );
        fields.insert("runtimeLibraries".into(), self.runtime_libraries.clone());
        Ok(fields)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn immutable_list(items: Vec<DslValue>) -> DslValue {
    DslValue::Typed {
        class: "com.google.common.collect.RegularImmutableList".into(),
        value: Box::new(DslValue::List(items)),
    }
}

// port: SerializationOptions#builder
pub fn builder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    // .setRunValidation(false).setIncludeDebugInfo(false).setRuntimeLibraries(ImmutableList.of())
    Ok(DslValue::Native(Rc::new(RefCell::new(OptionsBuilder {
        builder: SerializationOptions::builder(),
        runtime_libraries: immutable_list(vec![]),
    }))))
}

fn with_builder(
    args: &[DslValue],
    f: impl FnOnce(&mut OptionsBuilder, &[DslValue]) -> Result<(), Throwable>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), rest @ ..] = args else {
        return Err(bad());
    };
    {
        let mut b = this.borrow_mut();
        let b = b
            .as_any_mut()
            .downcast_mut::<OptionsBuilder>()
            .ok_or_else(bad)?;
        f(b, rest)?;
    }
    Ok(DslValue::Native(this.clone()))
}

// port: SerializationOptions.Builder#setIncludeDebugInfo
pub fn set_include_debug_info(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    with_builder(&args, |b, rest| {
        let [DslValue::Bool(value)] = rest else {
            return Err(bad());
        };
        b.builder = b.builder.clone().set_include_debug_info(*value);
        Ok(())
    })
}

// port: SerializationOptions.Builder#setRuntimeLibraries
pub fn set_runtime_libraries(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    with_builder(&args, |b, rest| {
        let [value] = rest else {
            return Err(bad());
        };
        let (DslValue::List(items) | DslValue::Sequence(items)) = value.untyped() else {
            return Err(bad());
        };
        let libraries = items
            .iter()
            .map(|item| match item.untyped() {
                DslValue::String(s) => Ok(s.to_string_lossy()),
                _ => Err(bad()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        b.builder = b.builder.clone().set_runtime_libraries(libraries);
        // ImmutableList.copyOf
        b.runtime_libraries = immutable_list(items.clone());
        Ok(())
    })
}

// port: SerializationOptions.Builder#build
pub fn build(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this)] = args.as_slice() else {
        return Err(bad());
    };
    let mut b = this.borrow_mut();
    let b = b
        .as_any_mut()
        .downcast_mut::<OptionsBuilder>()
        .ok_or_else(bad)?;
    Ok(DslValue::Native(Rc::new(RefCell::new(Options {
        options: b.builder.clone().build(),
        runtime_libraries: b.runtime_libraries.clone(),
    }))))
}

/// `new SerializeTypedAstPass(compiler, consumer, serializationOptions)`.
struct NativeSerializeTypedAstPass {
    compiler: CompilerHandle,
    consumer: Rc<RefCell<dyn NativeObject>>,
    result: Rc<RefCell<[Option<TypedAst>; 1]>>,
    serialization_options: Rc<RefCell<dyn NativeObject>>,
    options: SerializationOptions,
}

impl NativeObject for NativeSerializeTypedAstPass {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        PASS
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert("compiler".into(), DslValue::Compiler(self.compiler.clone()));
        fields.insert("consumer".into(), DslValue::Native(self.consumer.clone()));
        fields.insert(
            "serializationOptions".into(),
            DslValue::Native(self.serialization_options.clone()),
        );
        Ok(fields)
    }
    // port: SerializeTypedAstPass#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let result = self.result.clone();
        // ast -> result[0] = ast
        let mut pass = SerializeTypedAstPass::new(
            move |ast: TypedAst| result.borrow_mut()[0] = Some(ast),
            self.options.clone(),
        );
        pass.process(compiler, externs, root);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: SerializeTypedAstPass#SerializeTypedAstPass
pub fn serialize_typed_ast_pass(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(compiler),
        DslValue::Native(consumer),
        DslValue::Native(serialization_options),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let result = {
        let mut c = consumer.borrow_mut();
        match c.as_any_mut().downcast_mut::<TypedAstConsumerLambda>() {
            Some(lambda) => lambda.result.clone(),
            None => {
                return Err(Throwable::Unported(format!(
                    "{PASS}#<init> with consumer {}",
                    c.class_name()
                )));
            }
        }
    };
    let options = {
        let mut o = serialization_options.borrow_mut();
        o.as_any_mut()
            .downcast_mut::<Options>()
            .ok_or_else(bad)?
            .options
            .clone()
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeSerializeTypedAstPass {
            compiler: compiler.clone(),
            consumer: consumer.clone(),
            result,
            serialization_options: serialization_options.clone(),
            options,
        },
    ))))
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
