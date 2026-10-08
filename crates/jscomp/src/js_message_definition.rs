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
//   src/com/google/javascript/jscomp/JsMessageDefinition.java.

//! Port of `com.google.javascript.jscomp.JsMessageDefinition`.

use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use indexmap::IndexMap;

use crate::js_message::JsMessage;

/// Container class that holds a `JsMessage` and information about the `goog.getMsg()` call it was
/// built from.
pub trait JsMessageDefinition {
    /// The JsMessage object built from the `goog.getMsg()` call.
    // port: JsMessageDefinition#getMessage
    fn get_message(&self) -> &JsMessage;

    /// The RHS node of the message assignment statement.
    // port: JsMessageDefinition#getMessageNode
    fn get_message_node(&self) -> NodeId;

    /// The Node representing the message text template.
    ///
    /// This node may be a literal string or a concatenation of literal strings.
    ///
    /// For a `goog.getMsg()` call this is the first argument.
    // port: JsMessageDefinition#getTemplateTextNode
    fn get_template_text_node(&self) -> NodeId;

    /// The object literal `Node` that maps placeholder names to expressions providing their
    /// values.
    ///
    /// This value will be `None` if the message definition didn't specify placeholder values.
    // port: JsMessageDefinition#getPlaceholderValuesNode
    fn get_placeholder_values_node(&self) -> Option<NodeId>;

    /// A map from placehlolder name to the Node assigned to it in the values map argument of
    /// `goog.getMsg()`.
    ///
    /// This will be an empty map if there was no object or it was an empty object literal.
    // port: JsMessageDefinition#getPlaceholderValueMap
    fn get_placeholder_value_map(&self) -> &IndexMap<JsString, NodeId>;

    /// The value of the 'html' options key in the options bag argument.
    ///
    /// This value will be `false` if there was no options bag argument, or if it didn't contain an
    /// 'html' property.
    // port: JsMessageDefinition#shouldEscapeLessThan
    fn should_escape_less_than(&self) -> bool;

    /// The value of the 'unescapeHtmlEntities' options key in the options bag argument.
    ///
    /// This value will be `false` if there was no options bag argument, or if it didn't contain an
    /// 'unescapeHtmlEntities' property.
    // port: JsMessageDefinition#shouldUnescapeHtmlEntities
    fn should_unescape_html_entities(&self) -> bool;
}
