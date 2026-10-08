/*
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckAccessControls.java,
//   src/com/google/javascript/jscomp/ImplicitNullabilityCheck.java,
//   src/com/google/javascript/jscomp/lint/CheckArrayWithGoogObject.java,
//   src/com/google/javascript/jscomp/lint/CheckNestedNames.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for the access-controls passes (CheckAccessControls, ImplicitNullabilityCheck,
//! CheckArrayWithGoogObject, CheckNestedNames): their descriptors construct each pass with
//! `new <Pass>(compiler)` and run its `process(Node, Node)`.
use crate::{
    replay::{
        native_closure_primitives::NamedPass,
        registry::Entry,
        replay_dsl::{Ctx, DslValue},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    check_access_controls::CheckAccessControls,
    implicit_nullability_check::ImplicitNullabilityCheck,
    lint::{
        check_array_with_goog_object::CheckArrayWithGoogObject,
        check_nested_names::CheckNestedNames,
    },
};

// port: ReplayDsl#invoke (resolved access-controls signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.CheckAccessControls#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_access_controls
        }
        "com.google.javascript.jscomp.ImplicitNullabilityCheck#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            implicit_nullability_check
        }
        "com.google.javascript.jscomp.lint.CheckArrayWithGoogObject#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_array_with_goog_object
        }
        "com.google.javascript.jscomp.lint.CheckNestedNames#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_nested_names
        }
        _ => return None,
    })
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

// port: CheckAccessControls#CheckAccessControls
fn check_access_controls(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(NamedPass::value(
        "com.google.javascript.jscomp.CheckAccessControls",
        Box::new(CheckAccessControls::new(&mut c.borrow_mut())),
    ))
}

// port: ImplicitNullabilityCheck#ImplicitNullabilityCheck
fn implicit_nullability_check(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(NamedPass::value(
        "com.google.javascript.jscomp.ImplicitNullabilityCheck",
        Box::new(ImplicitNullabilityCheck::new(&c.borrow())),
    ))
}

// port: CheckArrayWithGoogObject#CheckArrayWithGoogObject
fn check_array_with_goog_object(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(NamedPass::value(
        "com.google.javascript.jscomp.lint.CheckArrayWithGoogObject",
        Box::new(CheckArrayWithGoogObject::new(&c.borrow())),
    ))
}

// port: CheckNestedNames#CheckNestedNames
fn check_nested_names(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(NamedPass::value(
        "com.google.javascript.jscomp.lint.CheckNestedNames",
        Box::new(CheckNestedNames::new(&c.borrow())),
    ))
}
