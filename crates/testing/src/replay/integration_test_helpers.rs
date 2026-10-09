/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/integration/IntegrationTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of the replay helper `oracle/replay/helpers/.../IntegrationTest_Helpers.java` (DSL names
//! `IntegrationTest_Helpers.XidRenamingMap` and `IntegrationTest_Helpers.AlwaysRunSafetyCheckPass`):
//! the anonymous classes IntegrationTest$1 (testReplaceIdGeneratorsTest) and IntegrationTest$2
//! (testAlwaysRunSafetyCheck) that the test stores into CompilerOptions.
use crate::{
    replay::{
        registry::Entry,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{AbstractCompiler, compiler_pass::CompilerPass, renaming_map::RenamingMap};
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

const XID_RENAMING_MAP: &str =
    "com.google.javascript.jscomp.IntegrationTest_Helpers$XidRenamingMap";
const ALWAYS_RUN_SAFETY_CHECK_PASS: &str =
    "com.google.javascript.jscomp.IntegrationTest_Helpers$AlwaysRunSafetyCheckPass";

// port: ReplayDsl#invoke (resolved signatures of IntegrationTest_Helpers)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.IntegrationTest_Helpers$XidRenamingMap#<init>()" => {
            xid_renaming_map
        }
        "com.google.javascript.jscomp.IntegrationTest_Helpers$AlwaysRunSafetyCheckPass#<init>()" => {
            always_run_safety_check_pass
        }
        _ => return None,
    })
}

/// `private static class XidRenamingMap implements RenamingMap`.
struct XidRenamingMap;

impl RenamingMap for XidRenamingMap {
    // port: IntegrationTest_Helpers.XidRenamingMap#get
    fn get(&self, value: &JsString) -> Option<JsString> {
        Some(JsString::from(format!(":{value}:").as_str()))
    }
}

/// The DSL object wrapping the helper instance.
struct XidRenamingMapObject;

impl NativeObject for XidRenamingMapObject {
    fn class_name(&self) -> &str {
        XID_RENAMING_MAP
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == XID_RENAMING_MAP || class == "com.google.javascript.jscomp.RenamingMap"
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_renaming_map(&mut self) -> Option<Arc<dyn RenamingMap + Send + Sync>> {
        Some(Arc::new(XidRenamingMap))
    }
}

// port: IntegrationTest_Helpers.XidRenamingMap#XidRenamingMap
fn xid_renaming_map(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        XidRenamingMapObject,
    ))))
}

/// `private static class AlwaysRunSafetyCheckPass implements CompilerPass`.
struct AlwaysRunSafetyCheckPass;

impl CompilerPass for AlwaysRunSafetyCheckPass {
    // port: IntegrationTest_Helpers.AlwaysRunSafetyCheckPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let var = root
            .get_last_child(compiler)
            .and_then(|c| c.get_first_child(compiler))
            .expect("root.getLastChild().getFirstChild()");
        assert_eq!(var.get_token(compiler), Token::VAR);
        var.detach(compiler);
    }
}

/// The DSL object wrapping the helper instance.
struct AlwaysRunSafetyCheckPassObject;

impl NativeObject for AlwaysRunSafetyCheckPassObject {
    fn class_name(&self) -> &str {
        ALWAYS_RUN_SAFETY_CHECK_PASS
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == ALWAYS_RUN_SAFETY_CHECK_PASS
            || class == "com.google.javascript.jscomp.CompilerPass"
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_custom_pass(&mut self) -> Option<Arc<Mutex<dyn CompilerPass + Send>>> {
        Some(Arc::new(Mutex::new(AlwaysRunSafetyCheckPass)))
    }
}

// port: IntegrationTest_Helpers.AlwaysRunSafetyCheckPass#AlwaysRunSafetyCheckPass
fn always_run_safety_check_pass(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        AlwaysRunSafetyCheckPassObject,
    ))))
}
