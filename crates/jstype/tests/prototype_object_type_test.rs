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
 *   Nick Santos
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
//   test/com/google/javascript/rhino/jstype/PrototypeObjectTypeTest.java.

use closure_jstype::{
    js_type::JSType, object_type::ObjectType, testing::base_js_type_test_case::BaseJSTypeTestCase,
};

// port: PrototypeObjectTypeTest#testToString
#[test]
fn test_to_string() {
    let mut f = BaseJSTypeTestCase::new();
    let type_ = f.reg.create_anonymous_object_type(&f.ast, None);
    assert_eq!(type_.to_string(&mut f.reg, &f.ast), "{}");
    type_.define_declared_property(&mut f.reg, &f.ast, "foo", f.number_type, None);
    assert_eq!(type_.to_string(&mut f.reg, &f.ast), "{foo: number}");
    type_.define_declared_property(&mut f.reg, &f.ast, "bar", type_, None);
    assert_eq!(
        type_.to_string(&mut f.reg, &f.ast),
        "{\n  bar: {...},\n  foo: number\n}"
    );
}
