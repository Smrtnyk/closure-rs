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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/serialization/TypedAstSerializerTest.java.

//! Port of the replay helper
//! `oracle/replay/helpers/.../serialization/TypedAstSerializerTest_Helpers.java` (built with
//! `{"new": FQCN}`), itself copied from TypedAstSerializerTest.java: the holder field
//! `testResult` and `getProcessor` with its CompilerPass lambda.
use crate::{
    jscomp_api::Compiler,
    proto_neutral,
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::serialization::{
    serialization_options::SerializationOptions, typed_ast_proto::TypedAst,
    typed_ast_serializer::TypedAstSerializer,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.serialization.TypedAstSerializerTest_Helpers";
const PROCESSOR: &str =
    "com.google.javascript.jscomp.serialization.TypedAstSerializerTest_Helpers$$Lambda";

/// `final class TypedAstSerializerTest_Helpers`.
pub struct TypedAstSerializerTestHelpers {
    /// Holds the serialized AST created by the last executed test method.
    test_result: Option<TypedAst>,
}

impl NativeObject for TypedAstSerializerTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "testResult".into(),
            match &self.test_result {
                None => DslValue::Null,
                Some(ast) => DslValue::Proto(
                    proto_neutral::to_proto_message("jscomp.TypedAst", ast)
                        .map_err(Throwable::HarnessError)?,
                ),
            },
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value) {
            ("testResult", DslValue::Null) => {
                self.test_result = None;
                Ok(())
            }
            (name, _) => Err(Throwable::Unported(format!("{HOLDER}#{name}"))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: TypedAstSerializerTest_Helpers#TypedAstSerializerTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        TypedAstSerializerTestHelpers { test_result: None },
    ))))
}

// port: TypedAstSerializerTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    if this.borrow().class_name() != HOLDER {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessorPass {
        outer: this.clone(),
    }))))
}

/// The lambda `(externs, root) -> { ... }` returned by getProcessor. It captures `compiler` and
/// the holder; the Rust pass receives the compiler at process time.
struct GetProcessorPass {
    outer: Rc<RefCell<dyn NativeObject>>,
}

impl NativeObject for GetProcessorPass {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        PROCESSOR
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: TypedAstSerializerTest_Helpers#getProcessor (the CompilerPass lambda)
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        // In general we avoid serializing types and properties of types that are not referenced by
        // AST nodes. In practice this avoids serializing properties and types that have been entirely
        // removed due to optimizations.
        //
        // We will simulate this behavior here by clearing the externs so only types and
        // properties that are referenced in the sources branch of the AST will be serialized.
        let externs_root = compiler
            .get_root()
            .and_then(|r| r.get_first_child(compiler))
            .ok_or_else(bad)?;
        let mut externs_script = externs_root.get_first_child(compiler);
        while let Some(script) = externs_script {
            script.remove_children(compiler);
            compiler.report_change_to_change_scope(script);
            externs_script = script.get_next(compiler);
        }

        let mut typed_ast_serializer = TypedAstSerializer::new(
            compiler,
            SerializationOptions::builder()
                .set_include_debug_info(true)
                .set_run_validation(true)
                .build(),
        );
        let result = typed_ast_serializer.serialize_roots(externs, root);
        let mut outer = self.outer.borrow_mut();
        let holder = outer
            .as_any_mut()
            .downcast_mut::<TypedAstSerializerTestHelpers>()
            .ok_or_else(bad)?;
        holder.test_result = Some(result);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
