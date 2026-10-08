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
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/RewriteDynamicImportsTest.java.

//! Port of the unit-corpus helper `RewriteDynamicImportsTest_Helpers.java` (oracle/replay/helpers),
//! itself copied from RewriteDynamicImportsTest#getProcessor: the holder of the test's fields
//! `dynamicImportAlias` and `chunkOutputType`, and `GetProcessor#getProcessor` with its lambda
//! CompilerPass (TypeCheck#processForTesting, Es6RewriteModules, RewriteDynamicImports).
use crate::{
    jscomp_api::Compiler,
    replay::{
        options_values::OptionValue,
        replay_dsl::{Ctx, DslValue, NativeObject, Object},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_options::ChunkOutputType, compiler_pass::CompilerPass,
    es6_rewrite_modules::Es6RewriteModules, rewrite_dynamic_imports::RewriteDynamicImports,
};
use closure_rhino::node::NodeId;
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.RewriteDynamicImportsTest_Helpers";
const GET_PROCESSOR: &str =
    "com.google.javascript.jscomp.RewriteDynamicImportsTest_Helpers$GetProcessor";
const CHUNK_OUTPUT_TYPE: &str = "com.google.javascript.jscomp.CompilerOptions$ChunkOutputType";

fn bad() -> Throwable {
    Throwable::HarnessError("RewriteDynamicImportsTest_Helpers arguments".into())
}

// port: RewriteDynamicImportsTest_Helpers#RewriteDynamicImportsTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::new();
    let mut field_types = IndexMap::new();
    // private @Nullable String dynamicImportAlias = "imprt_";
    fields.insert(
        "dynamicImportAlias".to_string(),
        DslValue::String("imprt_".into()),
    );
    field_types.insert(
        "dynamicImportAlias".to_string(),
        "java.lang.String".to_string(),
    );
    // private ChunkOutputType chunkOutputType = ChunkOutputType.GLOBAL_NAMESPACE;
    fields.insert(
        "chunkOutputType".to_string(),
        DslValue::Enum {
            class: CHUNK_OUTPUT_TYPE.into(),
            name: "GLOBAL_NAMESPACE".into(),
        },
    );
    field_types.insert("chunkOutputType".to_string(), CHUNK_OUTPUT_TYPE.to_string());
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: HOLDER.into(),
        fields,
        field_types,
    }))))
}

/// `private final class GetProcessor` (an inner class: it reads the holder's fields).
struct GetProcessor {
    outer: Rc<RefCell<Object>>,
}

impl NativeObject for GetProcessor {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        GET_PROCESSOR
    }
    // port: ReplayDsl#invoke (native method receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: RewriteDynamicImportsTest_Helpers.GetProcessor#GetProcessor
pub fn get_processor_init(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessor {
        outer: outer.clone(),
    }))))
}

// port: RewriteDynamicImportsTest_Helpers.GetProcessor#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let outer = this
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<GetProcessor>()
        .ok_or_else(bad)?
        .outer
        .clone();
    Ok(DslValue::Native(Rc::new(RefCell::new(Processor { outer }))))
}

/// The lambda `(externs, root) -> { ... }` that getProcessor returns; it captures the compiler
/// (the harness lends that same compiler to `process`) and reads the holder's fields when it runs.
struct Processor {
    outer: Rc<RefCell<Object>>,
}

impl NativeObject for Processor {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.CompilerPass"
    }
    // port: UnitRecorder#collect (fields of a value reachable from the processor): the lambda has
    // only its captured arguments, which the recorder skips.
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: ReplayDsl#invoke (native method receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    // port: RewriteDynamicImportsTest_Helpers.GetProcessor#getProcessor (lambda body)
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        // NOTE: we cannot just use enableTypeCheck(), because we need to be able
        // to retrieve globalTypedScope for use by Es6RewriteModules.
        // new TypeCheck(compiler, new SemanticReverseAbstractInterpreter(registry), registry)
        //     .processForTesting(externs, root), on the compiler the harness lends this pass.
        let global_typed_scope = Some(crate::harness_passes::type_check_scope(
            compiler, externs, root, false,
        )?);
        compiler.set_type_checking_has_run(true);
        // We need to make sure modules are rewritten, because RewriteDynamicImports
        // expects to be able to see the module variables.
        let module_metadata_map = compiler.get_module_metadata_map().cloned();
        let module_map = compiler.get_module_map().cloned();
        Es6RewriteModules::new(
            compiler,
            module_metadata_map,
            module_map,
            /* preprocessorSymbolTable= */ None,
            global_typed_scope,
        )
        .process(compiler, externs, root);

        let (dynamic_import_alias, chunk_output_type) = {
            let outer = self.outer.borrow();
            let alias = match outer.fields.get("dynamicImportAlias") {
                Some(DslValue::String(s)) => Some(s.to_string_lossy()),
                Some(DslValue::Null) | None => None,
                Some(_) => return Err(bad()),
            };
            let chunk = ChunkOutputType::decode_value(
                outer.fields.get("chunkOutputType").ok_or_else(bad)?,
            )?;
            (alias, chunk)
        };
        RewriteDynamicImports::new(compiler, dynamic_import_alias.as_deref(), chunk_output_type)
            .process(compiler, externs, root);
        Ok(())
    }
}
