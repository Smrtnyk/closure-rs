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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ReplaceMessagesConstants.java.

//! Port of `com.google.javascript.jscomp.ReplaceMessagesConstants`: constants that need to be
//! shared between `ReplaceMessages` and other passes.

use closure_rhino::node::{Ast, NodeId};

/// `goog.getMsg()` calls will be converted into a call to this method which is defined in
/// synthetic externs.
// port: ReplaceMessagesConstants#DEFINE_MSG_CALLEE
pub const DEFINE_MSG_CALLEE: &str = "__jscomp_define_msg__";

/// `goog.getMsgWithFallback(MSG_NEW, MSG_OLD)` will be converted into a call to this method which
/// is defined in synthetic externs.
// port: ReplaceMessagesConstants#FALLBACK_MSG_CALLEE
pub const FALLBACK_MSG_CALLEE: &str = "__jscomp_msg_fallback__";

// port: ReplaceMessagesConstants#PROTECTED_FUNCTION_NAMES
const PROTECTED_FUNCTION_NAMES: [&str; 2] = [DEFINE_MSG_CALLEE, FALLBACK_MSG_CALLEE];

// port: ReplaceMessagesConstants#isProtectedMessage
pub fn is_protected_message(ast: &Ast, n: NodeId) -> bool {
    if !n.is_call(ast) {
        return false;
    }
    let callee = n.get_first_child(ast).unwrap();
    if !callee.is_name(ast) {
        return false;
    }
    let callee_name = callee.get_string(ast);
    PROTECTED_FUNCTION_NAMES
        .iter()
        .any(|name| callee_name == **name)
}
