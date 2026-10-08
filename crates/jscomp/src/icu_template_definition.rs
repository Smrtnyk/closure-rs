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
//   src/com/google/javascript/jscomp/IcuTemplateDefinition.java.

//! Port of `com.google.javascript.jscomp.IcuTemplateDefinition`.

use closure_rhino::node::NodeId;

use crate::js_message::JsMessage;

/// Contains a `JsMessage` representing an ICU template message and information about the
/// `goog.i18n.messages.declareIcuTemplate()` call from which it was extracted.
pub trait IcuTemplateDefinition {
    /// The JsMessage object built from the `declareIcuTemplate()` call.
    // port: IcuTemplateDefinition#getMessage
    fn get_message(&self) -> &JsMessage;

    /// The RHS node of the message assignment statement.
    // port: IcuTemplateDefinition#getMessageNode
    fn get_message_node(&self) -> NodeId;

    /// The Node representing the message text template.
    ///
    /// This node may be a literal string or a concatenation of literal strings.
    // port: IcuTemplateDefinition#getTemplateTextNode
    fn get_template_text_node(&self) -> NodeId;
}
