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
//   src/com/google/javascript/rhino/jstype/EnumType.java.

//! EnumType.Builder with Java's null arguments (TypedScopeCreator#createEnumTypeFromNodes passes a
//! null name for an unnamed enum literal and a null goog.module id outside goog.modules).
use closure_jstype::{
    enum_type::{EnumType, EnumTypeBuilder},
    prelude::*,
    testing::base_js_type_test_case::BaseJSTypeTestCase,
};
use closure_rhino::js_string::JsString;

// port: EnumType.Builder#setName ("enum{" + null + "}", elementName null)
#[test]
fn null_name_gives_enum_null_reference_name_and_null_element_name() {
    let mut base = BaseJSTypeTestCase::new();
    let enum_type = EnumTypeBuilder::new()
        .set_name_nullable(None)
        .set_goog_module_id_nullable(None)
        .set_element_type(base.number_type)
        .build(&mut base.reg, &base.ast);
    assert_eq!(
        enum_type.get_reference_name(&base.reg),
        Some(JsString::from("enum{null}"))
    );
    let elements_type = enum_type.get_elements_type(&base.reg);
    assert_eq!(elements_type.get_reference_name(&base.reg), None);
    assert_eq!(EnumType::get_goog_module_id(enum_type, &base.reg), None);
}

// port: EnumType.Builder#setGoogModuleId (null and non-null)
#[test]
fn nullable_setters_match_the_non_null_setters() {
    let mut base = BaseJSTypeTestCase::new();
    let a = EnumTypeBuilder::new()
        .set_name_nullable(Some("E".into()))
        .set_goog_module_id_nullable(Some("my.mod".into()))
        .set_element_type(base.number_type)
        .build(&mut base.reg, &base.ast);
    assert_eq!(
        a.get_reference_name(&base.reg),
        Some(JsString::from("enum{E}"))
    );
    assert_eq!(
        a.get_elements_type(&base.reg).get_reference_name(&base.reg),
        Some(JsString::from("E"))
    );
    assert_eq!(
        EnumType::get_goog_module_id(a, &base.reg),
        Some(JsString::from("my.mod"))
    );
}
