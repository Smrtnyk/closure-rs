/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/EmptyMessageBundle.java.

//! Port of `com.google.javascript.jscomp.EmptyMessageBundle`.

use std::sync::Arc;

use closure_rhino::js_string::JsString;

use crate::js_message::{IdGenerator, JsMessage};
use crate::message_bundle::MessageBundle;

/// An implementation of MessageBundle that has no translations.
#[derive(Debug, Default)]
pub struct EmptyMessageBundle;

impl MessageBundle for EmptyMessageBundle {
    /// Gets a dummy message ID generator.
    // port: EmptyMessageBundle#idGenerator
    fn id_generator(&self) -> Option<Arc<dyn IdGenerator>> {
        None
    }

    /// Returns null, to indicate it has no message replacements.
    // port: EmptyMessageBundle#getMessage
    fn get_message(&self, _id: &JsString) -> Option<&JsMessage> {
        None
    }

    /// Returns an empty list of messages.
    // port: EmptyMessageBundle#getAllMessages
    fn get_all_messages(&self) -> Vec<&JsMessage> {
        Vec::new()
    }
}
