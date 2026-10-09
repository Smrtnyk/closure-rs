/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Norris Boyd
 *   Roger Lawrence
 *   Mike McCabe
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Bob Jervis
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/JSDocInfo.java,
//   src/com/google/javascript/rhino/JSTypeExpression.java,
//   src/com/google/javascript/rhino/Msg.java, src/com/google/javascript/rhino/Node.java.

use closure_rhino::{
    java_lang::{message_format::MessageFormat, properties},
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::JSDocInfo,
    msg::Msg,
    node::Ast,
    token::Token,
};

// port: Msg#format(Object...)
#[test]
fn message_arguments_preserve_utf16_and_literal_quote_replacement() {
    let value = JsString::from_units(vec![0xd800, u16::from(b'\''), 0xdc00]);
    assert_eq!(
        Msg::INVALID_VARIABLE_NAME.format_with_js_strings(std::slice::from_ref(&value)),
        JsString::from("invalid param name \"")
            .concat(&value)
            .concat(&"\"".into())
    );
    assert_eq!(
        MessageFormat::format_js_strings("a''{0} '{0}'", std::slice::from_ref(&value)),
        JsString::from("a'").concat(&value).concat(&" {0}".into())
    );
}

// port: Node#toString
// port: Node#toStringTree
// port: Node#appendJsonTree
// port: JSDocInfo#toStringVerbose
// port: JSTypeExpression#toString
#[test]
fn node_and_jsdoc_rendering_preserves_utf16() {
    let value = JsString::from_units(vec![0xd800, 0xdc00, 0xd800]);
    let mut ast = Ast::new();
    let node = ast.new_string_with_token(Token::NAME, value.clone());
    assert_eq!(
        node.to_string_utf16(&ast),
        JsString::from("NAME ").concat(&value)
    );
    assert_eq!(
        node.to_string_tree_utf16(&ast),
        JsString::from("NAME ").concat(&value).concat(&"\n".into())
    );
    assert_eq!(
        node.to_json_tree_utf16(&ast),
        JsString::from("{\"token\":\"NAME\",\"string\":\"")
            .concat(&value)
            .concat(&"\"}".into())
    );
    let expression = JSTypeExpression::new(node, "testcode");
    assert_eq!(
        expression.to_string_utf16(&ast),
        JsString::from("type: NAME ")
            .concat(&value)
            .concat(&"\n".into())
    );
    let mut builder = JSDocInfo::builder();
    assert!(builder.record_description(value.clone()));
    let info = builder.build().unwrap();
    assert!(info.to_string_utf16(&ast).index_of(&value) >= 0);
    node.set_original_name(&mut ast, Some(value.clone()));
    assert!(
        node.to_string_utf16(&ast)
            .index_of(JsString::from("[original_name: ").concat(&value))
            >= 0
    );
    assert!(
        node.to_json_tree_utf16(&ast)
            .index_of(JsString::from("\"original_name\":\"").concat(&value))
            >= 0
    );
}

// port: String#replace(CharSequence,CharSequence)
#[test]
fn string_replacement_uses_non_overlapping_utf16_sequences() {
    let raw = JsString::from_units(vec![0xd800, 0xd800, 0xd800]);
    assert_eq!(
        raw.replace(&JsString::from_units(vec![0xd800, 0xd800]), &"x".into()),
        JsString::from("x").concat(&JsString::from_units(vec![0xd800]))
    );
    assert_eq!(
        JsString::from("ab").replace(&"".into(), &"-".into()),
        "-a-b-"
    );
}

// port: Properties#loadConvert
#[test]
fn properties_escape_conversion_preserves_utf16() {
    let values = properties::load_js_strings(&"k=\\ud800\\udc00\\ud800".into());
    assert_eq!(
        values.get(&JsString::from("k")),
        Some(&JsString::from_units(vec![0xd800, 0xdc00, 0xd800]))
    );
}
