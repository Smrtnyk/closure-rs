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
 *   John Lenz
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
//   test/com/google/javascript/rhino/jstype/TemplateTypeTest.java.

use closure_jstype::js_type::JSType;
use closure_jstype::named_type::{NamedTypeBuilder, ResolutionKind};
use closure_jstype::testing::{
    base_js_type_test_case::BaseJSTypeTestCase, type_subject::TypeSubject,
};

// port: TemplateTypeTest#templateTypes_boundedByUnknown_areNotEqual
#[test]
fn template_types_bounded_by_unknown_are_not_equal() {
    let mut f = BaseJSTypeTestCase::new();
    let unknown_type = f.unknown_type;
    let t1 = f
        .reg
        .create_template_type_with_bound(&f.ast, "T", unknown_type);
    let t2 = f
        .reg
        .create_template_type_with_bound(&f.ast, "T", unknown_type);
    t1.resolve(&mut f.reg, &f.ast);
    t2.resolve(&mut f.reg, &f.ast);
    TypeSubject::assert_type(t1).is_not_equal_to(&mut f.reg, &f.ast, t2);
    TypeSubject::assert_type(t1).is_subtype_of(&mut f.reg, &f.ast, t2);
}

// port: TemplateTypeTest#templateTypes_boundedByNominalTypes_areNotEqual
#[test]
fn template_types_bounded_by_nominal_types_are_not_equal() {
    let mut f = BaseJSTypeTestCase::new();
    let foo_type = f.reg.create_object_type(&f.ast, "Foo", Some(f.object_type));
    let t1 = f.reg.create_template_type_with_bound(&f.ast, "T", foo_type);
    let t2 = f.reg.create_template_type_with_bound(&f.ast, "T", foo_type);
    t1.resolve(&mut f.reg, &f.ast);
    t2.resolve(&mut f.reg, &f.ast);
    TypeSubject::assert_type(t1).is_not_equal_to(&mut f.reg, &f.ast, t2);
    TypeSubject::assert_type(t1).is_not_subtype_of(&mut f.reg, &f.ast, t2);
}

// port: TemplateTypeTest#proxiesOf_templateTypes_boundedByNominalTypes_areNotEqual
#[test]
fn proxies_of_template_types_bounded_by_nominal_types_are_not_equal() {
    let mut f = BaseJSTypeTestCase::new();
    let foo_type = f.reg.create_object_type(&f.ast, "Foo", Some(f.object_type));
    let t1 = f.reg.create_template_type_with_bound(&f.ast, "T", foo_type);
    let t2 = f.reg.create_template_type_with_bound(&f.ast, "T", foo_type);
    let t1_proxy = NamedTypeBuilder::new(&f.reg, "T")
        .set_resolution_kind(ResolutionKind::NONE)
        .set_referenced_type(t1)
        .build(&mut f.reg, &f.ast);
    let t2_proxy = NamedTypeBuilder::new(&f.reg, "T")
        .set_resolution_kind(ResolutionKind::NONE)
        .set_referenced_type(t2)
        .build(&mut f.reg, &f.ast);
    TypeSubject::assert_type(t1_proxy).is_not_equal_to(&mut f.reg, &f.ast, t2_proxy);
    TypeSubject::assert_type(t1_proxy).is_not_subtype_of(&mut f.reg, &f.ast, t2_proxy);
}

// port: TemplateTypeTest#templateTypes_areNotEqualToUnknown
#[test]
fn template_types_are_not_equal_to_unknown() {
    let mut f = BaseJSTypeTestCase::new();
    let t = f.reg.create_template_type(&f.ast, "T");
    assert!(t.is_unknown_type(&mut f.reg, &f.ast));
    TypeSubject::assert_type(t).is_not_equal_to(&mut f.reg, &f.ast, f.unknown_type);
    TypeSubject::assert_type(t).is_not_equal_to(&mut f.reg, &f.ast, f.checked_unknown_type);
}

// port: TemplateTypeTest#templateTypes_withBound_areNotEqualToBound
#[test]
fn template_types_with_bound_are_not_equal_to_bound() {
    let mut f = BaseJSTypeTestCase::new();
    let t = f
        .reg
        .create_template_type_with_bound(&f.ast, "T", f.number_type);
    TypeSubject::assert_type(t).is_not_equal_to(&mut f.reg, &f.ast, f.number_type);
}
