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
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/InferJSDocInfo.java,
//   src/com/google/javascript/jscomp/TypeInferencePass.java,
//   src/com/google/javascript/jscomp/TypedScopeCreator.java.

//! Replay adapters for the type-inference classes descriptors construct (InferJSDocInfoTest's
//! processor: TypedScopeCreator, TypeInferencePass#inferAllScopes, InferJSDocInfo#process) and the
//! compiler's ReverseAbstractInterpreter they take.
use crate::{
    replay::{
        registry::BorrowedEntry,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass, infer_js_doc_info::InferJSDocInfo,
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
    type_inference_pass::TypeInferencePass, typed_scope_creator::TypedScopeCreator,
};
use closure_rhino::node::NodeId;
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc, sync::Arc};

const TYPED_SCOPE_CREATOR_INIT: &str = "com.google.javascript.jscomp.TypedScopeCreator#<init>(com.google.javascript.jscomp.AbstractCompiler)";
const TYPE_INFERENCE_PASS_INIT: &str = "com.google.javascript.jscomp.TypeInferencePass#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.type.ReverseAbstractInterpreter,com.google.javascript.jscomp.TypedScopeCreator)";
const INFER_ALL_SCOPES: &str = "com.google.javascript.jscomp.TypeInferencePass#inferAllScopes(com.google.javascript.rhino.Node)";
const INFER_JS_DOC_INFO_INIT: &str = "com.google.javascript.jscomp.InferJSDocInfo#<init>(com.google.javascript.jscomp.AbstractCompiler)";
const INFER_JS_DOC_INFO_PROCESS: &str = "com.google.javascript.jscomp.InferJSDocInfo#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)";

// port: ReplayDsl#invoke (type-inference signatures while the processor has the compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        TYPED_SCOPE_CREATOR_INIT => typed_scope_creator,
        TYPE_INFERENCE_PASS_INIT => type_inference_pass,
        INFER_ALL_SCOPES => infer_all_scopes,
        INFER_JS_DOC_INFO_INIT => infer_js_doc_info,
        INFER_JS_DOC_INFO_PROCESS => infer_js_doc_info_process,
        _ => return None,
    })
}

/// `Compiler#getReverseAbstractInterpreter()` as a DSL value: the chain's first interpreter
/// (Java's runtime class is SemanticReverseAbstractInterpreter).
pub struct NativeReverseAbstractInterpreter(pub Arc<dyn ReverseAbstractInterpreter>);

impl NativeObject for NativeReverseAbstractInterpreter {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.type.SemanticReverseAbstractInterpreter"
    }
    // port: UnitRecorder#isInstance
    fn is_instance_of(&self, class: &str) -> bool {
        matches!(
            class,
            "com.google.javascript.jscomp.type.SemanticReverseAbstractInterpreter"
                | "com.google.javascript.jscomp.type.ChainableReverseAbstractInterpreter"
                | "com.google.javascript.jscomp.type.ReverseAbstractInterpreter"
        )
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: Compiler#getReverseAbstractInterpreter
pub fn reverse_abstract_interpreter(compiler: &mut crate::jscomp_api::Compiler) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativeReverseAbstractInterpreter(
        compiler.get_reverse_abstract_interpreter(),
    ))))
}

/// A `TypedScopeCreator`. Java shares one object between the TypeInferencePass and later users;
/// the Rust pass owns its creator, so constructing the pass takes it out of this value.
struct NativeTypedScopeCreator(Option<TypedScopeCreator>);

impl NativeObject for NativeTypedScopeCreator {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.TypedScopeCreator"
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

struct NativeTypeInferencePass(TypeInferencePass);

impl NativeObject for NativeTypeInferencePass {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.TypeInferencePass"
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

struct NativeInferJSDocInfo(InferJSDocInfo);

impl NativeObject for NativeInferJSDocInfo {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.InferJSDocInfo"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: UnitRecorder#collect (the processor's fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: InferJSDocInfo#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.0.process(compiler, externs, root);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn native(object: impl NativeObject) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(object)))
}

fn same_compiler(ctx: &Ctx, value: &DslValue) -> Result<(), Throwable> {
    match (value, ctx.compiler.as_ref()) {
        (DslValue::Compiler(c), Some(current)) if Rc::ptr_eq(c, current) => Ok(()),
        _ => Err(bad()),
    }
}

// port: TypedScopeCreator#TypedScopeCreator
fn typed_scope_creator(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [c] = args.as_slice() else {
        return Err(bad());
    };
    same_compiler(ctx, c)?;
    Ok(native(NativeTypedScopeCreator(Some(
        TypedScopeCreator::new(compiler),
    ))))
}

// port: TypeInferencePass#TypeInferencePass
fn type_inference_pass(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [c, DslValue::Native(reverse), DslValue::Native(creator)] = args.as_slice() else {
        return Err(bad());
    };
    same_compiler(ctx, c)?;
    let reverse = reverse
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<NativeReverseAbstractInterpreter>()
        .ok_or_else(bad)?
        .0
        .clone();
    let creator = creator
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<NativeTypedScopeCreator>()
        .ok_or_else(bad)?
        .0
        .take()
        .ok_or_else(|| {
            Throwable::HarnessError(
                "a TypedScopeCreator handed to a second TypeInferencePass (DSL adapter)".into(),
            )
        })?;
    Ok(native(NativeTypeInferencePass(TypeInferencePass::new(
        compiler, reverse, creator,
    ))))
}

// port: TypeInferencePass#inferAllScopes
fn infer_all_scopes(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(pass), DslValue::Node(root)] = args.as_slice() else {
        return Err(bad());
    };
    let mut pass = pass.borrow_mut();
    let pass = pass
        .as_any_mut()
        .downcast_mut::<NativeTypeInferencePass>()
        .ok_or_else(bad)?;
    // The returned TypedScope is not read by any descriptor.
    pass.0.infer_all_scopes(compiler, *root);
    Ok(DslValue::Null)
}

// port: InferJSDocInfo#InferJSDocInfo
fn infer_js_doc_info(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [c] = args.as_slice() else {
        return Err(bad());
    };
    same_compiler(ctx, c)?;
    Ok(native(NativeInferJSDocInfo(InferJSDocInfo::new(compiler))))
}

// port: InferJSDocInfo#process
fn infer_js_doc_info_process(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [
        DslValue::Native(pass),
        DslValue::Node(externs),
        DslValue::Node(root),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    pass.borrow_mut().process(compiler, *externs, *root)?;
    Ok(DslValue::Null)
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
