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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/rhino/jstype/JSTypeTest.java.

use closure_jstype::{
    JSTypeNative, TypeId,
    boolean_literal_set::BooleanLiteralSet,
    enum_type::{EnumType, EnumTypeBuilder},
    function_type::{FunctionType, FunctionTypeBuilder},
    js_type::{JSType, UnitTestingJSType, UnitTestingJSTypeData},
    js_type_class::JSTypeClass,
    js_type_registry::JSTypeRegistry,
    named_type::{NamedTypeBuilder, ResolutionKind},
    object_type::ObjectType,
    property::PropertyKey,
    prototype_object_type::PrototypeObjectTypeBuilder,
    record_type_builder::RecordTypeBuilder,
    testing::{
        asserts::Asserts, base_js_type_test_case::BaseJSTypeTestCase,
        map_based_scope::MapBasedScope, type_subject::TypeSubject,
    },
    union_type::UnionType,
};
use closure_rhino::{
    js_string::JsString,
    jscomp_base::Tri,
    jsdoc_info::{JSDocInfo, Visibility},
    outcome::Outcome,
};
use std::sync::Arc;

struct Fixture {
    base: BaseJSTypeTestCase,
    date_method: TypeId,
    function_type: TypeId,
    unresolved_named_type: TypeId,
    goog_bar: TypeId,
    goog_sub_bar: TypeId,
    goog_sub_sub_bar: TypeId,
    goog_bar_inst: TypeId,
    goog_sub_bar_inst: TypeId,
    goog_sub_sub_bar_inst: TypeId,
    named_goog_bar: TypeId,
    subclass_of_unresolved_named_type: TypeId,
    subclass_ctor: TypeId,
    interface_type: TypeId,
    interface_inst_type: TypeId,
    sub_interface_type: TypeId,
    sub_interface_inst_type: TypeId,
    record_type: TypeId,
    enum_type: TypeId,
    elements_type: TypeId,
    forward_declared_named_type: TypeId,
    types: Vec<TypeId>,
}
impl Fixture {
    // port: JSTypeTest#setUp
    fn set_up() -> Self {
        let mut f = Self {
            base: BaseJSTypeTestCase::new(),
            date_method: TypeId(0),
            function_type: TypeId(0),
            unresolved_named_type: TypeId(0),
            goog_bar: TypeId(0),
            goog_sub_bar: TypeId(0),
            goog_sub_sub_bar: TypeId(0),
            goog_bar_inst: TypeId(0),
            goog_sub_bar_inst: TypeId(0),
            goog_sub_sub_bar_inst: TypeId(0),
            named_goog_bar: TypeId(0),
            subclass_of_unresolved_named_type: TypeId(0),
            subclass_ctor: TypeId(0),
            interface_type: TypeId(0),
            interface_inst_type: TypeId(0),
            sub_interface_type: TypeId(0),
            sub_interface_inst_type: TypeId(0),
            record_type: TypeId(0),
            enum_type: TypeId(0),
            elements_type: TypeId(0),
            forward_declared_named_type: TypeId(0),
            types: Vec::new(),
        };
        f.base
            .reset_registry_with_forward_declared_name("forwardDeclared");
        {
            let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
            let _expr2 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
            let goog_object = _expr2;
            let _expr3 = vec![(JsString::from("goog"), goog_object)];
            let _expr4 = Arc::new(MapBasedScope::new(_expr3));
            let scope = _expr4;
            let _expr5 = RecordTypeBuilder::new();
            let mut builder = _expr5;
            builder.add_property("a", f.base.number_type, None);
            builder.add_property("b", f.base.string_type, None);
            let _expr6 = builder.build(&mut f.base.reg, &f.base.ast);
            f.record_type = _expr6;
            let _expr7 = EnumTypeBuilder::new();
            let _expr8 = _expr7.set_name("Enum");
            let _expr9 = _expr8.set_element_type(f.base.number_type);
            let _expr10 = _expr9.build(&mut f.base.reg, &f.base.ast);
            f.enum_type = _expr10;
            let _expr11 = f.enum_type.get_elements_type(&f.base.reg);
            f.elements_type = _expr11;
            let _expr12 = FunctionTypeBuilder::new();
            let _expr13 = _expr12.with_return_type(f.base.number_type);
            let _expr14 = _expr13.build(&mut f.base.reg, &f.base.ast);
            f.function_type = _expr14;
            let _expr15 = FunctionTypeBuilder::new();
            let _expr16 = f.base.reg.create_parameters(&[]);
            let _expr17 = _expr15.with_parameters(_expr16);
            let _expr18 = _expr17.with_return_type(f.base.number_type);
            let _expr19 = _expr18.with_type_of_this(f.base.date_type);
            let _expr20 = _expr19.build(&mut f.base.reg, &f.base.ast);
            f.date_method = _expr20;
            f.base.error_reporter.lock().unwrap().expect_all_warnings(&[
                "Bad type annotation. Unknown type not.resolved.named.type",
            ]);
            let _expr21 = -1;
            let _expr22 = -1;
            let _expr23 = f.base.reg.create_named_type(
                &f.base.ast,
                Some(scope.clone()),
                "not.resolved.named.type",
                "",
                _expr21,
                _expr22,
            );
            f.unresolved_named_type = _expr23;
            let _expr24 = -1;
            let _expr25 = -1;
            let _expr26 = f.base.reg.create_named_type(
                &f.base.ast,
                Some(scope.clone()),
                "goog.Bar",
                "",
                _expr24,
                _expr25,
            );
            f.named_goog_bar = _expr26;
            let _expr27 = FunctionTypeBuilder::new();
            let _expr28 = _expr27.for_constructor();
            let _expr29 = _expr28.build(&mut f.base.reg, &f.base.ast);
            f.subclass_ctor = _expr29;
            f.subclass_ctor.set_prototype_based_on(
                &mut f.base.reg,
                &f.base.ast,
                f.unresolved_named_type,
            );
            let _expr30 = f.subclass_ctor.get_instance_type(&f.base.reg);
            f.subclass_of_unresolved_named_type = _expr30.unwrap();
            let _expr31 = FunctionTypeBuilder::new();
            let _expr32 = _expr31.for_interface();
            let _expr33 = _expr32.with_name("Interface");
            let _expr34 = _expr33.build(&mut f.base.reg, &f.base.ast);
            f.interface_type = _expr34;
            let _expr35 = f.interface_type.get_instance_type(&f.base.reg);
            f.interface_inst_type = _expr35.unwrap();
            let _expr36 = FunctionTypeBuilder::new();
            let _expr37 = _expr36.for_interface();
            let _expr38 = _expr37.with_name("SubInterfacce");
            let _expr39 = _expr38.build(&mut f.base.reg, &f.base.ast);
            f.sub_interface_type = _expr39;
            let _expr40 = vec![f.interface_inst_type];
            f.sub_interface_type.set_extended_interfaces(
                &mut f.base.reg,
                &f.base.ast,
                _expr40.clone(),
            );
            let _expr41 = f.sub_interface_type.get_instance_type(&f.base.reg);
            f.sub_interface_inst_type = _expr41.unwrap();
            let _expr42 = f.base.reg.create_constructor_type(
                &f.base.ast,
                Some("goog.Bar".into()),
                None,
                None,
                None,
                None,
                false,
            );
            f.goog_bar = _expr42;
            let _expr43 = f.goog_bar.get_prototype(&mut f.base.reg, &f.base.ast);
            let _expr44 = _expr43.define_declared_property(
                &mut f.base.reg,
                &f.base.ast,
                "date",
                f.base.date_type,
                None,
            );
            let _expr45 = vec![f.interface_inst_type];
            f.goog_bar
                .set_implemented_interfaces(&mut f.base.reg, &f.base.ast, _expr45.clone());
            let _expr46 = f.goog_bar.get_instance_type(&f.base.reg);
            f.goog_bar_inst = _expr46.unwrap();
            let _expr47 = f.base.reg.create_constructor_type(
                &f.base.ast,
                Some("googSubBar".into()),
                None,
                None,
                None,
                None,
                false,
            );
            f.goog_sub_bar = _expr47;
            let _expr48 = f.goog_bar.get_instance_type(&f.base.reg);
            f.goog_sub_bar
                .set_prototype_based_on(&mut f.base.reg, &f.base.ast, _expr48.unwrap());
            let _expr49 = f.goog_sub_bar.get_instance_type(&f.base.reg);
            f.goog_sub_bar_inst = _expr49.unwrap();
            let _expr50 = f.base.reg.create_constructor_type(
                &f.base.ast,
                Some("googSubSubBar".into()),
                None,
                None,
                None,
                None,
                false,
            );
            f.goog_sub_sub_bar = _expr50;
            let _expr51 = f.goog_sub_bar.get_instance_type(&f.base.reg);
            f.goog_sub_sub_bar.set_prototype_based_on(
                &mut f.base.reg,
                &f.base.ast,
                _expr51.unwrap(),
            );
            let _expr52 = f.goog_sub_sub_bar.get_instance_type(&f.base.reg);
            f.goog_sub_sub_bar_inst = _expr52.unwrap();
            let _expr53 = goog_object.define_declared_property(
                &mut f.base.reg,
                &f.base.ast,
                "Bar",
                f.goog_bar,
                None,
            );
            let _expr54 = f.base.reg.create_named_type(
                &f.base.ast,
                Some(scope.clone()),
                "forwardDeclared",
                "source",
                1,
                0,
            );
            f.forward_declared_named_type = _expr54;
            _expr1.close(&mut f.base.reg, &f.base.ast);
        }
        let _expr55 = f
            .named_goog_bar
            .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
        assert!(_expr55.is_some());
        let _expr56 = closure_jstype::no_resolved_type::NoResolvedType::new(
            &mut f.base.reg,
            &f.base.ast,
            "Foo",
            None,
        );
        let _expr57 = f.goog_bar.get_instance_type(&f.base.reg);
        let _expr58 = vec![
            f.base.no_object_type,
            _expr56,
            f.base.no_type,
            f.base.boolean_object_type,
            f.base.boolean_type,
            f.base.string_object_type,
            f.base.string_type,
            f.base.symbol_object_type,
            f.base.symbol_type,
            f.base.void_type,
            f.base.unknown_type,
            f.base.null_type,
            f.base.number_object_type,
            f.base.number_type,
            f.base.date_type,
            f.date_method,
            f.function_type,
            f.unresolved_named_type,
            f.goog_bar,
            f.goog_sub_bar,
            f.goog_sub_sub_bar,
            f.named_goog_bar,
            _expr57.unwrap(),
            f.subclass_of_unresolved_named_type,
            f.subclass_ctor,
            f.record_type,
            f.enum_type,
            f.elements_type,
            f.goog_bar,
            f.goog_sub_bar,
            f.forward_declared_named_type,
        ];
        f.types = _expr58;
        f
    }

    // port: JSTypeTest#assertPropertyTypeDeclared
    fn assert_property_type_declared(&mut self, owner_type: TypeId, prop: &str) {
        let f = self;
        let _expr1 = owner_type.is_property_type_declared(&mut f.base.reg, &f.base.ast, prop);
        assert!(_expr1);
        let _expr2 = owner_type.is_property_type_inferred(&mut f.base.reg, &f.base.ast, prop);
        assert!(!_expr2);
    }

    // port: JSTypeTest#assertPropertyTypeInferred
    fn assert_property_type_inferred(&mut self, owner_type: TypeId, prop: &str) {
        let f = self;
        let _expr1 = owner_type.is_property_type_declared(&mut f.base.reg, &f.base.ast, prop);
        assert!(!_expr1);
        let _expr2 = owner_type.is_property_type_inferred(&mut f.base.reg, &f.base.ast, prop);
        assert!(_expr2);
    }

    // port: JSTypeTest#assertPropertyTypeUnknown
    fn assert_property_type_unknown(&mut self, owner_type: TypeId, prop: &str) {
        let f = self;
        let _expr1 = owner_type.is_property_type_declared(&mut f.base.reg, &f.base.ast, prop);
        assert!(!_expr1);
        let _expr2 = owner_type.is_property_type_inferred(&mut f.base.reg, &f.base.ast, prop);
        assert!(!_expr2);
        let _expr3 = owner_type.get_property_type(&mut f.base.reg, &f.base.ast, prop);
        let _expr4 = _expr3.is_unknown_type(&mut f.base.reg, &f.base.ast);
        assert!(_expr4);
    }

    // port: JSTypeTest#assertReturnTypeEquals
    fn assert_return_type_equals(&mut self, expected_return_type: TypeId, function: TypeId) {
        let f = self;
        assert!(function.is_function_type(&f.base.reg));
        let _expr1 = function.get_return_type(&f.base.reg);
        TypeSubject::assert_type(_expr1).is_equal_to(
            &mut f.base.reg,
            &f.base.ast,
            expected_return_type,
        );
    }

    // port: JSTypeTest#compare
    fn compare(&mut self, r: Tri, t1: TypeId, t2: TypeId) {
        let f = self;
        let _expr1 = t1.test_for_equality(&mut f.base.reg, &f.base.ast, t2);
        assert_eq!(_expr1, Some(r));
        let _expr2 = t2.test_for_equality(&mut f.base.reg, &f.base.ast, t1);
        assert_eq!(_expr2, Some(r));
    }

    // port: JSTypeTest#getTypesToTestForSymmetry
    fn get_types_to_test_for_symmetry(&mut self) -> Vec<TypeId> {
        let f = self;
        let _expr1 = closure_jstype::no_resolved_type::NoResolvedType::new(
            &mut f.base.reg,
            &f.base.ast,
            "Foo",
            None,
        );
        let _expr2 = vec![f.base.string_type];
        let _expr3 = closure_jstype::no_resolved_type::NoResolvedType::new(
            &mut f.base.reg,
            &f.base.ast,
            "Foo",
            Some(_expr2),
        );
        let _expr4 = closure_jstype::no_resolved_type::NoResolvedType::new(
            &mut f.base.reg,
            &f.base.ast,
            "Bar",
            None,
        );
        let _expr5 = closure_jstype::no_resolved_type::NoResolvedType::new(
            &mut f.base.reg,
            &f.base.ast,
            "Foo",
            None,
        );
        let _expr6 = f.base.create_union_type(&[_expr5, f.base.null_type]);
        let _expr7 = f
            .base
            .create_union_type(&[f.base.boolean_type, f.base.string_type]);
        let _expr8 = f
            .base
            .create_union_type(&[f.base.number_type, f.base.string_type]);
        let _expr9 = f.base.create_union_type(&[f.base.null_type, f.date_method]);
        let _expr10 = f
            .base
            .create_union_type(&[f.base.unknown_type, f.date_method]);
        let _expr11 = f.base.create_union_type(&[f.named_goog_bar, f.date_method]);
        let _expr12 = f
            .base
            .create_union_type(&[f.base.null_type, f.unresolved_named_type]);
        let _expr13 = f.goog_bar.get_instance_type(&f.base.reg);
        let _expr14 = f
            .base
            .create_union_type(&[f.forward_declared_named_type, f.base.null_type]);
        let _expr15 = f
            .base
            .create_templatized_type(f.base.object_type, &[f.base.string_type]);
        let _expr16 = f
            .base
            .create_templatized_type(f.base.object_type, &[f.base.number_type]);
        let _expr17 = f
            .base
            .create_templatized_type(f.base.array_type, &[f.base.string_type]);
        let _expr18 = f
            .base
            .create_templatized_type(f.base.array_type, &[f.base.number_type]);
        let _expr19 = f
            .base
            .create_templatized_type(f.base.array_type, &[f.base.boolean_type]);
        let _expr20 = f.base.create_union_type(&[_expr19, f.base.null_type]);
        let _expr21 = f
            .base
            .create_templatized_type(f.base.object_type, &[f.base.boolean_type]);
        let _expr22 = f.base.create_union_type(&[_expr21, f.base.null_type]);
        let _expr23 = vec![
            f.base.unknown_type,
            f.base.checked_unknown_type,
            f.base.null_type,
            f.base.void_type,
            f.base.number_type,
            f.base.string_type,
            f.base.boolean_type,
            f.base.object_type,
            f.base.function_type,
            f.base.least_function_type,
            f.base.greatest_function_type,
            f.base.all_type,
            f.base.no_type,
            f.base.no_object_type,
            _expr1,
            _expr3,
            _expr4,
            _expr6,
            _expr7,
            _expr8,
            _expr9,
            _expr10,
            _expr11,
            _expr12,
            f.enum_type,
            f.elements_type,
            f.date_method,
            f.function_type,
            f.unresolved_named_type,
            f.goog_bar,
            f.named_goog_bar,
            _expr13.unwrap(),
            f.named_goog_bar,
            f.subclass_of_unresolved_named_type,
            f.subclass_ctor,
            f.record_type,
            f.forward_declared_named_type,
            _expr14,
            _expr15,
            _expr16,
            _expr17,
            _expr18,
            _expr20,
            _expr22,
        ];
        _expr23
    }

    // port: JSTypeTest#verifySubtypeChain
    fn verify_subtype_chain_default(&mut self, type_chain: &[TypeId]) {
        let f = self;
        f.verify_subtype_chain(type_chain, true);
    }

    // port: JSTypeTest#verifySubtypeChain
    fn verify_subtype_chain(&mut self, type_chain: &[TypeId], check_subtyping: bool) {
        let f = self;
        let _expr1 = type_chain.len();
        for i in 0.._expr1 {
            let _expr2 = type_chain.len();
            for j in 0.._expr2 {
                let _expr3 = type_chain[i];
                let type_i = _expr3;
                let _expr4 = type_chain[j];
                let type_j = _expr4;
                let _expr5 = f.get_named_wrapper("TypeI", type_i);
                let named_type_i = _expr5;
                let _expr6 = f.get_named_wrapper("TypeJ", type_j);
                let named_type_j = _expr6;
                let _expr7 = closure_jstype::proxy_object_type::create(
                    &mut f.base.reg,
                    &f.base.ast,
                    type_i,
                    None,
                );
                let proxy_type_i = _expr7;
                let _expr8 = closure_jstype::proxy_object_type::create(
                    &mut f.base.reg,
                    &f.base.ast,
                    type_j,
                    None,
                );
                let proxy_type_j = _expr8;
                let _expr9 = i == j;
                if _expr9 {
                    let _expr10 = type_i.equals(&mut f.base.reg, &f.base.ast, type_i);
                    let _expr11 = closure_rhino::jscomp_base::guava_format(
                        "%s should equal itself",
                        &[(type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string()],
                    );
                    assert!(_expr10, "{}", _expr11);
                    let _expr12 = named_type_i.equals(&mut f.base.reg, &f.base.ast, named_type_i);
                    let _expr13 = closure_rhino::jscomp_base::guava_format(
                        "Named %s should equal itself",
                        &[(type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string()],
                    );
                    assert!(_expr12, "{}", _expr13);
                    let _expr14 = proxy_type_i.equals(&mut f.base.reg, &f.base.ast, proxy_type_i);
                    let _expr15 = closure_rhino::jscomp_base::guava_format(
                        "Proxy %s should equal itself",
                        &[(type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string()],
                    );
                    assert!(_expr14, "{}", _expr15);
                } else {
                    let mut should_check = true;
                    let _expr16 = type_i.to_object_type(&f.base.reg);
                    let object_i = _expr16;
                    let _expr17 = type_j.to_object_type(&f.base.reg);
                    let object_j = _expr17;
                    let _expr18 = object_i.is_some();
                    let _expr20 = _expr18 && { object_j.is_some() };
                    if _expr20 {
                        let _expr21 = object_i.unwrap().get_constructor(&f.base.reg);
                        let constructor_i = _expr21;
                        let _expr22 = object_j.unwrap().get_constructor(&f.base.reg);
                        let constructor_j = _expr22;
                        let _expr23 = constructor_i.is_some();
                        let _expr25 = _expr23 && { constructor_j.is_some() };
                        let _expr27 = _expr25 && {
                            constructor_i.unwrap().is_structural_interface(&f.base.reg)
                        };
                        let _expr29 = _expr27 && {
                            constructor_j.unwrap().is_structural_interface(&f.base.reg)
                        };
                        if _expr29 {
                            let _expr30 = constructor_i.unwrap().equals(
                                &mut f.base.reg,
                                &f.base.ast,
                                constructor_j,
                            );
                            if _expr30 {
                                should_check = false;
                            }
                        }
                    }
                    if should_check {
                        let _expr31 = type_i.equals(&mut f.base.reg, &f.base.ast, type_j);
                        let _expr32 = closure_rhino::jscomp_base::guava_format(
                            "%s should not equal %s",
                            &[
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(!_expr31, "{}", _expr32);
                        let _expr33 =
                            named_type_i.equals(&mut f.base.reg, &f.base.ast, named_type_j);
                        let _expr34 = closure_rhino::jscomp_base::guava_format(
                            "Named %s should not equal %s",
                            &[
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(!_expr33, "{}", _expr34);
                        let _expr35 =
                            proxy_type_i.equals(&mut f.base.reg, &f.base.ast, proxy_type_j);
                        let _expr36 = closure_rhino::jscomp_base::guava_format(
                            "Proxy %s should not equal %s",
                            &[
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(!_expr35, "{}", _expr36);
                    }
                }
                let _expr37 = type_j.can_cast_to(&mut f.base.reg, &f.base.ast, type_i);
                let _expr38 = closure_rhino::jscomp_base::guava_format(
                    "%s should be castable to %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr37, "{}", _expr38);
                let _expr39 = type_j.can_cast_to(&mut f.base.reg, &f.base.ast, named_type_i);
                let _expr40 = closure_rhino::jscomp_base::guava_format(
                    "%s should be castable to Named %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (named_type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr39, "{}", _expr40);
                let _expr41 = type_j.can_cast_to(&mut f.base.reg, &f.base.ast, proxy_type_i);
                let _expr42 = closure_rhino::jscomp_base::guava_format(
                    "%s should be castable to Proxy %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (proxy_type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr41, "{}", _expr42);
                let _expr43 = named_type_j.can_cast_to(&mut f.base.reg, &f.base.ast, type_i);
                let _expr44 = closure_rhino::jscomp_base::guava_format(
                    "Named %s should be castable to %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr43, "{}", _expr44);
                let _expr45 = named_type_j.can_cast_to(&mut f.base.reg, &f.base.ast, named_type_i);
                let _expr46 = closure_rhino::jscomp_base::guava_format(
                    "Named %s should be castable to Named %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr45, "{}", _expr46);
                let _expr47 = named_type_j.can_cast_to(&mut f.base.reg, &f.base.ast, proxy_type_i);
                let _expr48 = closure_rhino::jscomp_base::guava_format(
                    "Named %s should be castable to Proxy %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr47, "{}", _expr48);
                let _expr49 = proxy_type_j.can_cast_to(&mut f.base.reg, &f.base.ast, type_i);
                let _expr50 = closure_rhino::jscomp_base::guava_format(
                    "Proxy %s should be castable to %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr49, "{}", _expr50);
                let _expr51 = proxy_type_j.can_cast_to(&mut f.base.reg, &f.base.ast, named_type_i);
                let _expr52 = closure_rhino::jscomp_base::guava_format(
                    "Proxy %s should be castable to Named %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr51, "{}", _expr52);
                let _expr53 = proxy_type_j.can_cast_to(&mut f.base.reg, &f.base.ast, proxy_type_i);
                let _expr54 = closure_rhino::jscomp_base::guava_format(
                    "Proxy %s should be castable to Proxy %s",
                    &[
                        (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                assert!(_expr53, "{}", _expr54);
                let _expr55 = type_i.is_subtype_of(&mut f.base.reg, &f.base.ast, type_j);
                let _expr57 =
                    _expr55 && { type_j.is_subtype_of(&mut f.base.reg, &f.base.ast, type_i) };
                if _expr57 {
                    continue;
                }
                if check_subtyping {
                    let _expr58 = i <= j;
                    if _expr58 {
                        let _expr59 = type_j.is_subtype_of(&mut f.base.reg, &f.base.ast, type_i);
                        let _expr60 = closure_rhino::jscomp_base::guava_format(
                            "%s should be a subtype of %s",
                            &[
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(_expr59, "{}", _expr60);
                        let _expr61 =
                            named_type_j.is_subtype_of(&mut f.base.reg, &f.base.ast, named_type_i);
                        let _expr62 = closure_rhino::jscomp_base::guava_format(
                            "Named %s should be a subtype of Named %s",
                            &[
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(_expr61, "{}", _expr62);
                        let _expr63 =
                            proxy_type_j.is_subtype_of(&mut f.base.reg, &f.base.ast, proxy_type_i);
                        let _expr64 = closure_rhino::jscomp_base::guava_format(
                            "Proxy %s should be a subtype of Proxy %s",
                            &[
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(_expr63, "{}", _expr64);
                    } else {
                        let _expr65 = type_j.is_subtype_of(&mut f.base.reg, &f.base.ast, type_i);
                        let _expr66 = closure_rhino::jscomp_base::guava_format(
                            "%s should not be a subtype of %s",
                            &[
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(!_expr65, "{}", _expr66);
                        let _expr67 =
                            named_type_j.is_subtype_of(&mut f.base.reg, &f.base.ast, named_type_i);
                        let _expr68 = closure_rhino::jscomp_base::guava_format(
                            "Named %s should not be a subtype of Named %s",
                            &[
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(!_expr67, "{}", _expr68);
                        let _expr69 =
                            proxy_type_j.is_subtype_of(&mut f.base.reg, &f.base.ast, proxy_type_i);
                        let _expr70 = closure_rhino::jscomp_base::guava_format(
                            "Named %s should not be a subtype of Named %s",
                            &[
                                (type_j.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                                (type_i.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                            ],
                        );
                        assert!(!_expr69, "{}", _expr70);
                    }
                    let _expr71 = i < j;
                    let _expr72 = if _expr71 { type_i } else { type_j };
                    let expected_supremum = _expr72;
                    let _expr73 = i > j;
                    let _expr74 = if _expr73 { type_i } else { type_j };
                    let expected_infimum = _expr74;
                    let _expr75 = type_i.get_least_supertype(&mut f.base.reg, &f.base.ast, type_j);
                    let type_ileast_supertype = _expr75;
                    let _expr76 = type_j.get_least_supertype(&mut f.base.reg, &f.base.ast, type_i);
                    let type_jleast_supertype = _expr76;
                    let _expr77 = type_ileast_supertype.is_subtype_of(
                        &mut f.base.reg,
                        &f.base.ast,
                        type_jleast_supertype,
                    );
                    let _expr78 = !_expr77;
                    if _expr78 {
                        let _expr79 = format!(
                            "{}{}",
                            expected_supremum.to_string(&mut f.base.reg, &f.base.ast),
                            " should be the least supertype of "
                        );
                        let _expr80 = format!(
                            "{}{}",
                            _expr79,
                            type_i.to_string(&mut f.base.reg, &f.base.ast)
                        );
                        let _expr81 = format!("{}{}", _expr80, " and ");
                        let _expr82 = format!(
                            "{}{}",
                            _expr81,
                            type_j.to_string(&mut f.base.reg, &f.base.ast)
                        );
                        let _expr83 =
                            type_i.get_least_supertype(&mut f.base.reg, &f.base.ast, type_j);
                        assert_type_equal_with_message(
                            &mut f.base.reg,
                            &f.base.ast,
                            expected_supremum,
                            _expr83,
                            &(_expr82).to_string(),
                        );
                    }
                    let _expr84 = format!(
                        "{}{}",
                        expected_infimum.to_string(&mut f.base.reg, &f.base.ast),
                        " should be the greatest subtype of "
                    );
                    let _expr85 = format!(
                        "{}{}",
                        _expr84,
                        type_i.to_string(&mut f.base.reg, &f.base.ast)
                    );
                    let _expr86 = format!("{}{}", _expr85, " and ");
                    let _expr87 = format!(
                        "{}{}",
                        _expr86,
                        type_j.to_string(&mut f.base.reg, &f.base.ast)
                    );
                    let _expr88 = type_i.get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_j);
                    assert_type_equal_with_message(
                        &mut f.base.reg,
                        &f.base.ast,
                        expected_infimum,
                        _expr88,
                        &(_expr87).to_string(),
                    );
                }
            }
        }
    }

    // port: JSTypeTest#getNamedWrapper
    fn get_named_wrapper(&mut self, name: &str, jstype: TypeId) -> TypeId {
        let f = self;
        let _expr1 = jstype.is_no_type(&f.base.reg);
        let _expr2 = !_expr1;
        if _expr2 {
            let _expr3 = NamedTypeBuilder::new(&f.base.reg, name);
            let _expr4 = _expr3.set_resolution_kind(ResolutionKind::NONE);
            let _expr5 = _expr4.set_referenced_type(jstype);

            _expr5.build(&mut f.base.reg, &f.base.ast)
        } else {
            jstype
        }
    }

    // port: JSTypeTest#testGetTypeUnderEquality
    fn test_get_type_under_equality(
        &mut self,
        t1: TypeId,
        t2: TypeId,
        t1_eq: TypeId,
        t2_eq: TypeId,
    ) {
        let f = self;
        let _expr1 = t1.get_types_under_equality(&mut f.base.reg, &f.base.ast, t2);
        let p12 = _expr1;
        let _expr2 = t2.get_types_under_equality(&mut f.base.reg, &f.base.ast, t1);
        let p21 = _expr2;
        TypeSubject::assert_type(p12.type_a).is_equal_to(&mut f.base.reg, &f.base.ast, t1_eq);
        TypeSubject::assert_type(p21.type_b).is_equal_to(&mut f.base.reg, &f.base.ast, t1_eq);
        TypeSubject::assert_type(p12.type_b).is_equal_to(&mut f.base.reg, &f.base.ast, t2_eq);
        TypeSubject::assert_type(p21.type_a).is_equal_to(&mut f.base.reg, &f.base.ast, t2_eq);
    }

    // port: JSTypeTest#testGetTypesUnderInequality
    fn test_get_types_under_inequality(
        &mut self,
        t1: TypeId,
        t2: TypeId,
        t1_eq: TypeId,
        t2_eq: TypeId,
    ) {
        let f = self;
        let _expr1 = t1.get_types_under_inequality(&mut f.base.reg, &f.base.ast, t2);
        let p12 = _expr1;
        let _expr2 = t2.get_types_under_inequality(&mut f.base.reg, &f.base.ast, t1);
        let p21 = _expr2;
        TypeSubject::assert_type(p12.type_a).is_equal_to(&mut f.base.reg, &f.base.ast, t1_eq);
        TypeSubject::assert_type(p21.type_b).is_equal_to(&mut f.base.reg, &f.base.ast, t1_eq);
        TypeSubject::assert_type(p12.type_b).is_equal_to(&mut f.base.reg, &f.base.ast, t2_eq);
        TypeSubject::assert_type(p21.type_a).is_equal_to(&mut f.base.reg, &f.base.ast, t2_eq);
    }

    // port: JSTypeTest#assertUnionContains
    fn assert_union_contains(&mut self, union: TypeId, type_: TypeId) {
        let f = self;
        let _expr1 = union.contains(&mut f.base.reg, &f.base.ast, type_);
        let _expr2 = closure_rhino::jscomp_base::guava_format(
            "%s should contain %s",
            &[
                (union.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                (type_.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
            ],
        );
        assert!(_expr1, "{}", _expr2);
    }

    // port: JSTypeTest#assertTypeCanAssignToItself
    fn assert_type_can_assign_to_itself(&mut self, type_: TypeId) {
        let f = self;
        let _expr1 = type_.is_subtype_of(&mut f.base.reg, &f.base.ast, type_);
        assert!(_expr1);
    }

    // port: JSTypeTest#getBottomType
    fn get_bottom_type(&mut self) -> TypeId {
        let f = self;

        f.base.reg.get_native_object_type(JSTypeNative::NO_TYPE)
    }

    // port: JSTypeTest#withOpenRegistry
    fn with_open_registry<T>(&mut self, cb: impl FnOnce(&mut Self) -> T) -> T {
        let mut closer = self.base.reg.get_resolver().open_for_definition();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cb(self)));
        closer.close(&mut self.base.reg, &self.base.ast);
        match result {
            Ok(result) => result,
            Err(error) => std::panic::resume_unwind(error),
        }
    }
}

fn assert_type_equal_with_message(
    reg: &mut JSTypeRegistry,
    ast: &closure_rhino::node::Ast,
    expected: impl Into<Option<TypeId>>,
    actual: impl Into<Option<TypeId>>,
    message: &str,
) {
    let expected = expected.into();
    let actual = actual.into();
    let equal = match (actual, expected) {
        (Some(actual), Some(expected)) => actual.equals(reg, ast, expected),
        (None, None) => true,
        _ => false,
    };
    assert!(equal, "{message}");
    TypeSubject::assert_type(actual).is_equal_to(reg, ast, expected);
}

fn assert_type_multiset(
    reg: &mut JSTypeRegistry,
    ast: &closure_rhino::node::Ast,
    actual: &[TypeId],
    expected: &[TypeId],
) {
    assert_eq!(actual.len(), expected.len());
    let mut remaining = expected.to_vec();
    for actual in actual {
        let index = remaining
            .iter()
            .position(|expected| actual.equals(reg, ast, *expected))
            .expect("missing collection value");
        remaining.remove(index);
    }
    assert!(remaining.is_empty());
}
// port: JSTypeTest#testUniversalConstructorType
#[test]
fn test_universal_constructor_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.function_type.is_no_object_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.function_type.is_no_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.function_type.is_array_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.function_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.function_type.is_date_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.function_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.function_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.function_type.is_named_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.function_type.is_null_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.function_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr10);
    let _expr11 = f.base.function_type.is_number_object_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.function_type.is_number_value_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.function_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr13);
    let _expr14 = f.base.function_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.function_type.is_regexp_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.function_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.base.function_type.is_string_object_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.function_type.is_string_value_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.function_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr19);
    let _expr20 = f.base.function_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.function_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.function_type.is_enum_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.function_type.is_union_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.function_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f.base.function_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr25);
    let _expr26 = f.base.function_type.is_all_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.function_type.is_void_type(&f.base.reg);
    assert!(!_expr27);
    let _expr28 = f.base.function_type.is_constructor(&f.base.reg);
    assert!(_expr28);
    let _expr29 = f.base.function_type.is_instance_type(&f.base.reg);
    assert!(_expr29);
    let _expr30 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr30);
    let _expr31 =
        f.base
            .function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr31);
    let _expr32 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr32);
    let _expr33 =
        f.base
            .function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr33);
    let _expr34 =
        f.base
            .function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(!_expr34);
    let _expr35 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr35);
    let _expr36 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(_expr36);
    let _expr37 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.record_type);
    assert!(!_expr37);
    let _expr38 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr39);
    let _expr40 =
        f.base
            .function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr41);
    let _expr42 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr42);
    let _expr43 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr43);
    let _expr44 =
        f.base
            .function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr44);
    let _expr45 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr45);
    let _expr46 =
        f.base
            .function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr46);
    let _expr47 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr47);
    let _expr48 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr48);
    let _expr49 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr49);
    let _expr50 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr50);
    let _expr51 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr51);
    let _expr52 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(_expr52);
    let _expr53 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr53);
    let _expr54 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(_expr54);
    let _expr55 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(_expr55);
    let _expr56 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(_expr56);
    let _expr57 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.record_type,
    );
    assert!(_expr57);
    let _expr58 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr58);
    let _expr59 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr59);
    let _expr60 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(_expr60);
    let _expr61 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr61);
    let _expr62 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(_expr62);
    let _expr63 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr63);
    let _expr64 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(_expr64);
    let _expr65 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr65);
    let _expr66 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr66);
    let _expr67 = f.base.function_type.can_test_for_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr67);
    let _expr68 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr68);
    let _expr69 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr69);
    let _expr70 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr70);
    let _expr71 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr71);
    let _expr72 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr72);
    let _expr73 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr73);
    let _expr74 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(_expr74);
    let _expr75 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.record_type,
    );
    assert!(!_expr75);
    let _expr76 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr76);
    let _expr77 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr77);
    let _expr78 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr78);
    let _expr79 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr79);
    let _expr80 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr80);
    let _expr81 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr81);
    let _expr82 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr82);
    let _expr83 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr83);
    let _expr84 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr84);
    let _expr85 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr85);
    let _expr86 = f.base.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr86);
    let _expr87 = f
        .base
        .function_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr87);
    let _expr88 = f
        .base
        .function_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr88);
    let _expr89 = f.base.function_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr89);
    let _expr90 = f
        .base
        .function_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr90);
    let _expr91 = f
        .base
        .function_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr91);
    let _expr92 = f
        .base
        .function_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr92);
    let _expr93 = f
        .base
        .function_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr93);
    let _expr94 = f.base.function_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr94, "Function");
    let _expr95 = f.base.function_type.has_display_name(&f.base.reg);
    assert!(_expr95);
    let _expr96 = f.base.function_type.get_display_name(&f.base.reg);
    assert_eq!(_expr96, Some("Function".into()));
    let _expr97 =
        f.base
            .function_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "anyProperty");
    TypeSubject::assert_type(_expr97).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr98 = f.base.function_type.is_native_object_type(&f.base.reg);
    assert!(_expr98);
    let _expr99 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.function_type);
    let _expr100 = f
        .base
        .function_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(_expr100);
}

// port: JSTypeTest#testNoObjectType
#[test]
fn test_no_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.no_object_type.is_no_object_type(&f.base.reg);
    assert!(_expr1);
    let _expr2 = f.base.no_object_type.is_no_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.no_object_type.is_array_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.no_object_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.no_object_type.is_date_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.no_object_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.no_object_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.no_object_type.is_named_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.no_object_type.is_null_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f
        .base
        .no_object_type
        .is_number(&mut f.base.reg, &f.base.ast);
    assert!(_expr10);
    let _expr11 = f.base.no_object_type.is_number_object_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.no_object_type.is_number_value_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f
        .base
        .no_object_type
        .is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr13);
    let _expr14 = f
        .base
        .no_object_type
        .is_function_prototype_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.no_object_type.is_regexp_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f
        .base
        .no_object_type
        .is_string(&mut f.base.reg, &f.base.ast);
    assert!(_expr16);
    let _expr17 = f.base.no_object_type.is_string_object_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.no_object_type.is_string_value_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f
        .base
        .no_object_type
        .is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(_expr19);
    let _expr20 = f.base.no_object_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.no_object_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.no_object_type.is_enum_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.no_object_type.is_union_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f
        .base
        .no_object_type
        .is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f.base.no_object_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr25);
    let _expr26 = f.base.no_object_type.is_all_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.no_object_type.is_void_type(&f.base.reg);
    assert!(!_expr27);
    let _expr28 = f.base.no_object_type.is_constructor(&f.base.reg);
    assert!(!_expr28);
    let _expr29 = f.base.no_object_type.is_instance_type(&f.base.reg);
    assert!(!_expr29);
    let _expr30 = f
        .base
        .no_object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr30);
    let _expr31 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr31);
    let _expr32 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(_expr32);
    let _expr33 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr33);
    let _expr34 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(_expr34);
    let _expr35 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr35);
    let _expr36 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(_expr36);
    let _expr37 = f
        .base
        .no_object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.record_type);
    assert!(_expr37);
    let _expr38 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr38);
    let _expr39 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr39);
    let _expr40 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(_expr40);
    let _expr41 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr41);
    let _expr42 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(_expr42);
    let _expr43 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr43);
    let _expr44 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(_expr44);
    let _expr45 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr45);
    let _expr46 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(_expr46);
    let _expr47 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr47);
    let _expr48 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr48);
    f.base
        .assert_cannot_test_for_equality_with(f.base.no_object_type, f.base.no_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.no_object_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.boolean_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.boolean_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.record_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.null_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.number_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.string_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_object_type, f.base.void_type);
    let _expr49 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr49);
    let _expr50 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr50);
    let _expr51 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(_expr51);
    let _expr52 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr52);
    let _expr53 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(_expr53);
    let _expr54 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(_expr54);
    let _expr55 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(_expr55);
    let _expr56 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.record_type,
    );
    assert!(_expr56);
    let _expr57 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr57);
    let _expr58 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr58);
    let _expr59 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(_expr59);
    let _expr60 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr60);
    let _expr61 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(_expr61);
    let _expr62 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr62);
    let _expr63 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(_expr63);
    let _expr64 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr64);
    let _expr65 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(_expr65);
    let _expr66 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr66);
    let _expr67 = f.base.no_object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr67);
    let _expr68 = f
        .base
        .no_object_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr68);
    let _expr69 = f
        .base
        .no_object_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr69);
    let _expr70 = f
        .base
        .no_object_type
        .is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr70);
    let _expr71 = f
        .base
        .no_object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr71);
    let _expr72 = f
        .base
        .no_object_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr72);
    let _expr73 = f
        .base
        .no_object_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr73);
    let _expr74 = f
        .base
        .no_object_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr74);
    let _expr75 = f
        .base
        .no_object_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr75, "NoObject");
    let _expr76 = f.base.no_object_type.has_display_name(&f.base.reg);
    assert!(!_expr76);
    let _expr77 = f.base.no_object_type.get_display_name(&f.base.reg);
    assert!(_expr77.is_none());
    let _expr78 =
        f.base
            .no_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "anyProperty");
    TypeSubject::assert_type(_expr78).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr79 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    let _expr80 = f
        .base
        .no_object_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr80);
}

// port: JSTypeTest#testNoType
#[test]
fn test_no_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.no_type.is_no_object_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.no_type.is_no_type(&f.base.reg);
    assert!(_expr2);
    let _expr3 = f.base.no_type.is_array_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.no_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.no_type.is_date_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.no_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.no_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.no_type.is_named_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.no_type.is_null_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.no_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(_expr10);
    let _expr11 = f.base.no_type.is_number_object_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.no_type.is_number_value_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.no_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr13);
    let _expr14 = f.base.no_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.no_type.is_regexp_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.no_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(_expr16);
    let _expr17 = f.base.no_type.is_string_object_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.no_type.is_string_value_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.no_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(_expr19);
    let _expr20 = f.base.no_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.no_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.no_type.is_enum_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.no_type.is_union_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.no_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f.base.no_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr25);
    let _expr26 = f.base.no_type.is_all_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.no_type.is_void_type(&f.base.reg);
    assert!(!_expr27);
    let _expr28 = f.base.no_type.is_constructor(&f.base.reg);
    assert!(!_expr28);
    let _expr29 = f.base.no_type.is_instance_type(&f.base.reg);
    assert!(!_expr29);
    let _expr30 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(_expr30);
    let _expr31 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr31);
    let _expr32 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(_expr32);
    let _expr33 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(_expr33);
    let _expr34 =
        f.base
            .no_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(_expr34);
    let _expr35 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr35);
    let _expr36 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(_expr36);
    let _expr37 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(_expr37);
    let _expr38 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr38);
    let _expr39 =
        f.base
            .no_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(_expr39);
    let _expr40 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr40);
    let _expr41 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(_expr41);
    let _expr42 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(_expr42);
    let _expr43 =
        f.base
            .no_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(_expr43);
    let _expr44 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(_expr44);
    let _expr45 =
        f.base
            .no_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(_expr45);
    let _expr46 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr46);
    let _expr47 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(_expr47);
    f.base
        .assert_cannot_test_for_equality_with(f.base.no_type, f.base.no_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.no_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.boolean_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.boolean_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.null_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.number_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.string_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.no_type, f.base.void_type);
    let _expr48 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr48);
    let _expr49 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr49);
    let _expr50 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(_expr50);
    let _expr51 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(_expr51);
    let _expr52 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(_expr52);
    let _expr53 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(_expr53);
    let _expr54 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(_expr54);
    let _expr55 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(_expr55);
    let _expr56 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(_expr56);
    let _expr57 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(_expr57);
    let _expr58 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr58);
    let _expr59 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(_expr59);
    let _expr60 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(_expr60);
    let _expr61 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(_expr61);
    let _expr62 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(_expr62);
    let _expr63 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(_expr63);
    let _expr64 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr64);
    let _expr65 = f.base.no_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(_expr65);
    let _expr66 = f.base.no_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr66);
    let _expr67 = f.base.no_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr67);
    let _expr68 = f.base.no_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr68);
    let _expr69 = f
        .base
        .no_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr69);
    let _expr70 = f
        .base
        .no_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr70);
    let _expr71 = f
        .base
        .no_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr71);
    let _expr72 = f
        .base
        .no_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr72);
    let _expr73 = f.base.no_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr73, "None");
    let _expr74 = f.base.no_type.get_display_name(&f.base.reg);
    assert!(_expr74.is_none());
    let _expr75 = f.base.no_type.has_display_name(&f.base.reg);
    assert!(!_expr75);
    let _expr76 = f
        .base
        .no_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "anyProperty");
    TypeSubject::assert_type(_expr76).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr77 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr78 = f
        .base
        .no_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr78);
}

// port: JSTypeTest#testNoResolvedType
#[test]
fn test_no_resolved_type() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::no_resolved_type::NoResolvedType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Foo",
        None,
    );
    let no_resolved_foo_type = _expr1;
    let _expr2 = no_resolved_foo_type.is_no_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = no_resolved_foo_type.is_no_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = no_resolved_foo_type.is_no_resolved_type(&f.base.reg);
    assert!(_expr4);
    let _expr5 = no_resolved_foo_type.is_array_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = no_resolved_foo_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = no_resolved_foo_type.is_date_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = no_resolved_foo_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = no_resolved_foo_type.is_null_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = no_resolved_foo_type.is_named_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = no_resolved_foo_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr11);
    let _expr12 = no_resolved_foo_type.is_number_object_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = no_resolved_foo_type.is_number_value_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = no_resolved_foo_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr14);
    let _expr15 = no_resolved_foo_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = no_resolved_foo_type.is_regexp_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = no_resolved_foo_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr17);
    let _expr18 = no_resolved_foo_type.is_string_object_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = no_resolved_foo_type.is_string_value_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = no_resolved_foo_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr20);
    let _expr21 = no_resolved_foo_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = no_resolved_foo_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = no_resolved_foo_type.is_enum_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = no_resolved_foo_type.is_union_type(&f.base.reg);
    assert!(!_expr24);
    let _expr25 = no_resolved_foo_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr25);
    let _expr26 = no_resolved_foo_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr26);
    let _expr27 = no_resolved_foo_type.is_all_type(&f.base.reg);
    assert!(!_expr27);
    let _expr28 = no_resolved_foo_type.is_void_type(&f.base.reg);
    assert!(!_expr28);
    let _expr29 = no_resolved_foo_type.is_constructor(&f.base.reg);
    assert!(!_expr29);
    let _expr30 = no_resolved_foo_type.is_instance_type(&f.base.reg);
    assert!(!_expr30);
    TypeSubject::assert_type(no_resolved_foo_type).is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        no_resolved_foo_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, no_resolved_foo_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.no_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.boolean_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.boolean_object_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.null_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.number_object_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.string_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(no_resolved_foo_type, f.base.void_type);
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        no_resolved_foo_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    TypeSubject::assert_type(no_resolved_foo_type).can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    let _expr31 = no_resolved_foo_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr31);
    let _expr32 = no_resolved_foo_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr32);
    let _expr33 = no_resolved_foo_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr33);
    let _expr34 = no_resolved_foo_type.matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr34);
    let _expr35 = no_resolved_foo_type.matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr35);
    let _expr36 = no_resolved_foo_type.matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr36);
    let _expr37 = no_resolved_foo_type.matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr37);
    let _expr38 = no_resolved_foo_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr38, "NoResolvedType<Foo>");
    let _expr39 = no_resolved_foo_type.get_display_name(&f.base.reg);
    assert_eq!(_expr39, Some("Foo".into()));
    let _expr40 = no_resolved_foo_type.has_display_name(&f.base.reg);
    assert!(_expr40);
    let _expr41 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, no_resolved_foo_type);
    let _expr42 = f.forward_declared_named_type.is_empty_type(&f.base.reg);
    assert!(_expr42);
    let _expr43 = f
        .forward_declared_named_type
        .is_no_resolved_type(&f.base.reg);
    assert!(_expr43);
    let _expr44 = f
        .base
        .reg
        .create_nullable_type(&f.base.ast, no_resolved_foo_type);
    let nullable = _expr44;
    let _expr45 = nullable.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    TypeSubject::assert_type(_expr45).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr46 = nullable.get_restricted_union(&mut f.base.reg, &f.base.ast, f.base.null_type);
    TypeSubject::assert_type(_expr46).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        no_resolved_foo_type,
    );
    let _expr47 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[no_resolved_foo_type, f.base.void_type]);
    let test_is_voidable = _expr47;
    let _expr48 = test_is_voidable.get_possible_to_boolean_outcomes(&f.base.reg);
    assert_eq!(_expr48, BooleanLiteralSet::BOTH);
}

// port: JSTypeTest#testArrayType
#[test]
fn test_array_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.array_type.is_array_type(&f.base.reg);
    assert!(_expr1);
    let _expr2 = f.base.array_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.array_type.is_date_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.array_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.array_type.is_named_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.array_type.is_null_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.array_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr7);
    let _expr8 = f.base.array_type.is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.array_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.array_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr10);
    let _expr11 = f.base.array_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f
        .base
        .array_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr13 = _expr12.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr13);
    let _expr14 = f.base.array_type.is_regexp_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.array_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr15);
    let _expr16 = f.base.array_type.is_string_object_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = f.base.array_type.is_string_value_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.array_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr18);
    let _expr19 = f.base.array_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.array_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.array_type.is_enum_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.array_type.is_union_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.array_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr23);
    let _expr24 = f.base.array_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f.base.array_type.is_all_type(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.array_type.is_void_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.array_type.is_constructor(&f.base.reg);
    assert!(!_expr27);
    let _expr28 = f.base.array_type.is_instance_type(&f.base.reg);
    assert!(_expr28);
    let _expr29 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr29);
    let _expr30 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr30);
    let _expr31 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr31);
    let _expr32 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr32);
    let _expr33 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr33);
    let _expr34 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr34);
    let _expr35 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr35);
    let _expr36 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr36);
    let _expr37 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.record_type);
    assert!(!_expr37);
    let _expr38 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr39);
    let _expr40 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr40);
    let _expr41 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr41);
    let _expr42 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr42);
    let _expr43 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr43);
    let _expr44 = f
        .base
        .array_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr44);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.base.no_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.base.string_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.array_type, f.base.symbol_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.array_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.record_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.array_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.array_type, f.base.regexp_type);
    let _expr45 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr45);
    let _expr46 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr46);
    let _expr47 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(_expr47);
    let _expr48 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr48);
    let _expr49 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr49);
    let _expr50 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr50);
    let _expr51 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr51);
    let _expr52 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.record_type,
    );
    assert!(!_expr52);
    let _expr53 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr53);
    let _expr54 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr54);
    let _expr55 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr55);
    let _expr56 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr56);
    let _expr57 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr57);
    let _expr58 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr58);
    let _expr59 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr59);
    let _expr60 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr60);
    let _expr61 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr61);
    let _expr62 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr62);
    let _expr63 = f.base.array_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr63);
    let _expr64 = f.base.array_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr64);
    let _expr65 = f.base.array_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr65);
    let _expr66 = f
        .base
        .create_union_type(&[f.base.array_type, f.base.null_type]);
    let _expr67 = _expr66.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr67);
    let _expr68 = f
        .base
        .create_union_type(&[f.base.array_type, f.base.void_type]);
    let _expr69 = _expr68.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr69);
    let _expr70 = f.base.array_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr70);
    let _expr71 =
        f.base
            .array_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr71).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr72 = f
        .base
        .create_union_type(&[f.base.string_object_type, f.base.array_type]);
    let _expr73 = f.base.array_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr73).is_equal_to(&mut f.base.reg, &f.base.ast, _expr72);
    let _expr74 = f
        .base
        .create_union_type(&[f.base.symbol_object_type, f.base.array_type]);
    let _expr75 = f.base.array_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(_expr75).is_equal_to(&mut f.base.reg, &f.base.ast, _expr74);
    let _expr76 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.array_type]);
    let _expr77 =
        f.base
            .array_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr77).is_equal_to(&mut f.base.reg, &f.base.ast, _expr76);
    let _expr78 = f
        .base
        .create_union_type(&[f.base.array_type, f.function_type]);
    let _expr79 =
        f.base
            .array_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr79).is_equal_to(&mut f.base.reg, &f.base.ast, _expr78);
    let _expr80 =
        f.base
            .array_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr80).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr81 = f
        .base
        .create_union_type(&[f.base.date_type, f.base.array_type]);
    let _expr82 =
        f.base
            .array_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr82).is_equal_to(&mut f.base.reg, &f.base.ast, _expr81);
    let _expr83 = f
        .base
        .create_union_type(&[f.base.regexp_type, f.base.array_type]);
    let _expr84 =
        f.base
            .array_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr84).is_equal_to(&mut f.base.reg, &f.base.ast, _expr83);
    let _expr85 = f
        .base
        .array_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr86 = _expr85
        .unwrap()
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr86, 17);
    let _expr87 = f
        .base
        .array_type
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr87, 18);
    let _expr88 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "constructor");
    f.assert_return_type_equals(f.base.array_type, _expr88);
    let _expr89 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toString");
    f.assert_return_type_equals(f.base.string_type, _expr89);
    let _expr90 =
        f.base
            .array_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "toLocaleString");
    f.assert_return_type_equals(f.base.string_type, _expr90);
    let _expr91 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "concat");
    f.assert_return_type_equals(f.base.array_type, _expr91);
    let _expr92 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "join");
    f.assert_return_type_equals(f.base.string_type, _expr92);
    let _expr93 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "pop");
    f.assert_return_type_equals(f.base.unknown_type, _expr93);
    let _expr94 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "push");
    f.assert_return_type_equals(f.base.number_type, _expr94);
    let _expr95 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "reverse");
    f.assert_return_type_equals(f.base.array_type, _expr95);
    let _expr96 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "shift");
    f.assert_return_type_equals(f.base.unknown_type, _expr96);
    let _expr97 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "slice");
    f.assert_return_type_equals(f.base.array_type, _expr97);
    let _expr98 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "sort");
    f.assert_return_type_equals(f.base.array_type, _expr98);
    let _expr99 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "splice");
    f.assert_return_type_equals(f.base.array_type, _expr99);
    let _expr100 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "unshift");
    f.assert_return_type_equals(f.base.number_type, _expr100);
    let _expr101 = f
        .base
        .array_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "length");
    TypeSubject::assert_type(_expr101).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    f.assert_property_type_declared(f.base.array_type, "pop");
    let _expr102 = f
        .base
        .array_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr102);
    let _expr103 = f
        .base
        .array_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr103);
    let _expr104 = f
        .base
        .array_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr104);
    let _expr105 = f
        .base
        .array_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr105);
    let _expr106 = f.base.array_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr106, "Array");
    let _expr107 = f.base.array_type.has_display_name(&f.base.reg);
    assert!(_expr107);
    let _expr108 = f.base.array_type.get_display_name(&f.base.reg);
    assert_eq!(_expr108, Some("Array".into()));
    let _expr109 = f.base.array_type.is_native_object_type(&f.base.reg);
    assert!(_expr109);
    let _expr110 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.array_type);
    let _expr111 = f
        .base
        .array_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr111);
    let _expr112 = f.base.array_type.get_constructor(&f.base.reg);
    let _expr113 = _expr112
        .unwrap()
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(_expr113);
}

// port: JSTypeTest#testUnknownType
#[test]
fn test_unknown_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.unknown_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.unknown_type.is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.unknown_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.unknown_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.unknown_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.unknown_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.unknown_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.unknown_type.is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.unknown_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.unknown_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr10);
    let _expr11 = f.base.unknown_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.unknown_type.is_regexp_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.unknown_type.is_string_object_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.base.unknown_type.is_string_value_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.unknown_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.unknown_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = f.base.unknown_type.is_enum_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.unknown_type.is_union_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.unknown_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr19);
    let _expr20 = f.base.unknown_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr20);
    let _expr21 = f
        .base
        .unknown_type
        .is_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(_expr21);
    let _expr22 = f.base.unknown_type.is_void_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.unknown_type.is_constructor(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.unknown_type.is_instance_type(&f.base.reg);
    assert!(!_expr24);
    let _expr25 = f.base.unknown_type.autoboxes_to(&f.base.reg);
    assert!(_expr25.is_none());
    let _expr26 =
        f.base
            .unknown_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr26);
    let _expr27 =
        f.base
            .unknown_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(_expr27);
    let _expr28 =
        f.base
            .unknown_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(_expr28);
    let _expr29 =
        f.base
            .unknown_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr29);
    let _expr30 = f
        .base
        .unknown_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(_expr30);
    let _expr31 = f
        .base
        .unknown_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.record_type);
    assert!(_expr31);
    let _expr32 = f
        .base
        .unknown_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(_expr32);
    let _expr33 =
        f.base
            .unknown_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr33);
    let _expr34 = f
        .base
        .unknown_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr34);
    let _expr35 = f
        .base
        .unknown_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(_expr35);
    let _expr36 =
        f.base
            .unknown_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr36);
    let _expr37 =
        f.base
            .unknown_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(_expr37);
    let _expr38 = f
        .base
        .unknown_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(_expr38);
    let _expr39 = f
        .base
        .unknown_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(_expr39);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.unknown_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.string_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.record_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.unknown_type, f.base.boolean_type);
    let _expr40 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr40);
    let _expr41 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(_expr41);
    let _expr42 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(_expr42);
    let _expr43 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(_expr43);
    let _expr44 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(_expr44);
    let _expr45 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.record_type,
    );
    assert!(_expr45);
    let _expr46 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(_expr46);
    let _expr47 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr47);
    let _expr48 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(_expr48);
    let _expr49 = f.base.unknown_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(_expr49);
    let _expr50 = f
        .base
        .unknown_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr50);
    let _expr51 = f
        .base
        .unknown_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr51);
    let _expr52 =
        f.base
            .unknown_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    TypeSubject::assert_type(_expr52).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr53 =
        f.base
            .unknown_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.string_type);
    TypeSubject::assert_type(_expr53).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr54 =
        f.base
            .unknown_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    TypeSubject::assert_type(_expr54).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr55 =
        f.base
            .unknown_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr55).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr56 =
        f.base
            .unknown_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr56).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr57 =
        f.base
            .unknown_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr57).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr58 =
        f.base
            .unknown_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr58).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr59 =
        f.base
            .unknown_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr59).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr60 = f
        .base
        .unknown_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr60);
    let _expr61 = f
        .base
        .unknown_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr61);
    let _expr62 = f
        .base
        .unknown_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr62);
    let _expr63 = f
        .base
        .unknown_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr63);
    f.assert_property_type_unknown(f.base.unknown_type, "XXX");
    let _expr64 = f.base.unknown_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr64, "?");
    let _expr65 = f.base.unknown_type.has_display_name(&f.base.reg);
    assert!(_expr65);
    let _expr66 = f.base.unknown_type.get_display_name(&f.base.reg);
    assert_eq!(_expr66, Some("Unknown".into()));
    let _expr67 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    let _expr68 = f
        .base
        .unknown_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr68);
    let _expr69 = f
        .base
        .unknown_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "abc");
    TypeSubject::assert_type(_expr69).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
}

// port: JSTypeTest#testCheckedUnknownType
#[test]
fn test_checked_unknown_type() {
    let mut f = Fixture::set_up();
    f.assert_property_type_unknown(f.base.checked_unknown_type, "XXX");
    let _expr1 = f
        .base
        .checked_unknown_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr1, "??");
    let _expr2 = f.base.checked_unknown_type.has_display_name(&f.base.reg);
    assert!(_expr2);
    let _expr3 = f.base.checked_unknown_type.get_display_name(&f.base.reg);
    assert_eq!(_expr3, Some("Unknown".into()));
    let _expr4 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.checked_unknown_type);
    let _expr5 = f
        .base
        .checked_unknown_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f
        .base
        .checked_unknown_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "abc");
    TypeSubject::assert_type(_expr6).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.checked_unknown_type,
    );
}

// port: JSTypeTest#testAllType
#[test]
fn test_all_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.all_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.all_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.all_type.is_date_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.all_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.all_type.is_named_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.all_type.is_null_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.all_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr7);
    let _expr8 = f.base.all_type.is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.all_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.all_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(!_expr10);
    let _expr11 = f.base.all_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.all_type.is_regexp_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.all_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr13);
    let _expr14 = f.base.all_type.is_string_object_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.all_type.is_string_value_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.all_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.base.all_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.all_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.all_type.is_enum_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.all_type.is_union_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.all_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr21);
    let _expr22 = f.base.all_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr22);
    let _expr23 = f.base.all_type.is_all_type(&f.base.reg);
    assert!(_expr23);
    let _expr24 = f.base.all_type.is_void_type(&f.base.reg);
    assert!(!_expr24);
    let _expr25 = f.base.all_type.is_constructor(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.all_type.is_instance_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr27);
    let _expr28 =
        f.base
            .all_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr28);
    let _expr29 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr29);
    let _expr30 =
        f.base
            .all_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr30);
    let _expr31 =
        f.base
            .all_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr31);
    let _expr32 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr32);
    let _expr33 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr33);
    let _expr34 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr34);
    let _expr35 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.record_type);
    assert!(!_expr35);
    let _expr36 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr36);
    let _expr37 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr37);
    let _expr38 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr38);
    let _expr39 =
        f.base
            .all_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr39);
    let _expr40 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr41);
    let _expr42 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr42);
    let _expr43 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr43);
    let _expr44 = f.base.all_type.can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr44);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.record_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.all_type, f.base.regexp_type);
    let _expr45 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr45);
    let _expr46 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr46);
    let _expr47 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(_expr47);
    let _expr48 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(_expr48);
    let _expr49 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(_expr49);
    let _expr50 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(_expr50);
    let _expr51 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(_expr51);
    let _expr52 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.record_type,
    );
    assert!(_expr52);
    let _expr53 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(_expr53);
    let _expr54 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(_expr54);
    let _expr55 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(_expr55);
    let _expr56 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr56);
    let _expr57 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(_expr57);
    let _expr58 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(_expr58);
    let _expr59 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(_expr59);
    let _expr60 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(_expr60);
    let _expr61 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(_expr61);
    let _expr62 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr62);
    let _expr63 = f.base.all_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(_expr63);
    let _expr64 = f.base.all_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr64);
    let _expr65 = f.base.all_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr65);
    let _expr66 =
        f.base
            .all_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr66).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr67 =
        f.base
            .all_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    TypeSubject::assert_type(_expr67).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr68 = f.base.all_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr68).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr69 =
        f.base
            .all_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    TypeSubject::assert_type(_expr69).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr70 = f.base.all_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(_expr70).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr71 =
        f.base
            .all_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr71).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr72 =
        f.base
            .all_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr72).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr73 =
        f.base
            .all_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr73).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr74 =
        f.base
            .all_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr74).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr75 =
        f.base
            .all_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr75).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr76 = f
        .base
        .all_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr76);
    let _expr77 = f
        .base
        .all_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr77);
    let _expr78 = f
        .base
        .all_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr78);
    let _expr79 = f
        .base
        .all_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr79);
    let _expr80 = f.base.all_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr80, "*");
    let _expr81 = f.base.all_type.has_display_name(&f.base.reg);
    assert!(_expr81);
    let _expr82 = f.base.all_type.get_display_name(&f.base.reg);
    assert_eq!(_expr82, Some("<Any Type>".into()));
    let _expr83 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr84 = f
        .base
        .all_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr84);
}

// port: JSTypeTest#testTheObjectType
#[test]
fn test_the_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr1).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_prototype,
    );
    let _expr2 = f.base.object_type.is_no_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.object_type.is_no_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.object_type.is_array_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.object_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.object_type.is_date_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.object_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.object_type.is_null_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.object_type.is_named_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.object_type.is_null_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f.base.object_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr11);
    let _expr12 = f.base.object_type.is_number_object_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.object_type.is_number_value_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.base.object_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr14);
    let _expr15 = f.base.object_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f
        .base
        .object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr17 = _expr16.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr17);
    let _expr18 = f.base.object_type.is_regexp_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.object_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr19);
    let _expr20 = f.base.object_type.is_string_object_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.object_type.is_string_value_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.object_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr22);
    let _expr23 = f.base.object_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.object_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr24);
    let _expr25 = f.base.object_type.is_enum_type(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.object_type.is_union_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.object_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr27);
    let _expr28 = f.base.object_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr28);
    let _expr29 = f.base.object_type.is_all_type(&f.base.reg);
    assert!(!_expr29);
    let _expr30 = f.base.object_type.is_void_type(&f.base.reg);
    assert!(!_expr30);
    let _expr31 = f.base.object_type.is_constructor(&f.base.reg);
    assert!(!_expr31);
    let _expr32 = f.base.object_type.is_instance_type(&f.base.reg);
    assert!(_expr32);
    let _expr33 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr33);
    let _expr34 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr34);
    let _expr35 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr35);
    let _expr36 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr36);
    let _expr37 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr37);
    let _expr38 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr39);
    let _expr40 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.record_type);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr41);
    let _expr42 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr42);
    let _expr43 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr43);
    let _expr44 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr44);
    let _expr45 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr45);
    let _expr46 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr46);
    let _expr47 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr47);
    let _expr48 =
        f.base
            .object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr48);
    let _expr49 = f
        .base
        .object_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr49);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.string_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.boolean_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.record_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.object_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(f.base.object_type, f.base.unknown_type);
    let _expr50 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr50);
    let _expr51 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr51);
    let _expr52 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(_expr52);
    let _expr53 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr53);
    let _expr54 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(_expr54);
    let _expr55 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(_expr55);
    let _expr56 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(_expr56);
    let _expr57 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.record_type,
    );
    assert!(_expr57);
    let _expr58 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr58);
    let _expr59 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr59);
    let _expr60 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(_expr60);
    let _expr61 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr61);
    let _expr62 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(_expr62);
    let _expr63 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr63);
    let _expr64 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(_expr64);
    let _expr65 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr65);
    let _expr66 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(_expr66);
    let _expr67 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr67);
    let _expr68 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr68);
    let _expr69 = f.base.object_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr69);
    let _expr70 = f.base.object_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr70);
    let _expr71 = f.base.object_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr71);
    let _expr72 =
        f.base
            .object_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr72).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr73 = f.base.object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr73).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr74 = f.base.object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(_expr74).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr75 = f
        .base
        .create_union_type(&[f.base.object_type, f.base.symbol_type]);
    let _expr76 =
        f.base
            .object_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    TypeSubject::assert_type(_expr76).is_equal_to(&mut f.base.reg, &f.base.ast, _expr75);
    let _expr77 = f
        .base
        .create_union_type(&[f.base.object_type, f.base.number_type]);
    let _expr78 =
        f.base
            .object_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr78).is_equal_to(&mut f.base.reg, &f.base.ast, _expr77);
    let _expr79 =
        f.base
            .object_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr79).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr80 =
        f.base
            .object_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr80).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr81 =
        f.base
            .object_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr81).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr82 =
        f.base
            .object_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr82).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr83 = f
        .base
        .object_type
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr83, 7);
    let _expr84 = f
        .base
        .object_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "constructor");
    f.assert_return_type_equals(f.base.object_type, _expr84);
    let _expr85 = f
        .base
        .object_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toString");
    f.assert_return_type_equals(f.base.string_type, _expr85);
    let _expr86 =
        f.base
            .object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "toLocaleString");
    f.assert_return_type_equals(f.base.string_type, _expr86);
    let _expr87 = f
        .base
        .object_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "valueOf");
    f.assert_return_type_equals(f.base.unknown_type, _expr87);
    let _expr88 =
        f.base
            .object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "hasOwnProperty");
    f.assert_return_type_equals(f.base.boolean_type, _expr88);
    let _expr89 =
        f.base
            .object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "isPrototypeOf");
    f.assert_return_type_equals(f.base.boolean_type, _expr89);
    let _expr90 =
        f.base
            .object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "propertyIsEnumerable");
    f.assert_return_type_equals(f.base.boolean_type, _expr90);
    let _expr91 = f
        .base
        .object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr91);
    let _expr92 = f
        .base
        .object_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr92);
    let _expr93 = f
        .base
        .object_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr93);
    let _expr94 = f
        .base
        .object_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr94);
    let _expr95 = f
        .base
        .object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr95).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_prototype,
    );
    let _expr96 = f.base.object_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr96, "Object");
    let _expr97 = f.base.object_type.is_native_object_type(&f.base.reg);
    assert!(_expr97);
    let _expr98 = f
        .base
        .object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr99 = _expr98.unwrap().is_native_object_type(&f.base.reg);
    assert!(_expr99);
    let _expr100 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr101 = f
        .base
        .object_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr101);
    let _expr102 = f.base.object_type.get_constructor(&f.base.reg);
    let _expr103 = _expr102
        .unwrap()
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(_expr103);
}

// port: JSTypeTest#testNumberObjectType
#[test]
fn test_number_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.number_object_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f
        .base
        .number_object_type
        .is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.number_object_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.number_object_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.number_object_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.number_object_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.number_object_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f
        .base
        .number_object_type
        .is_number(&mut f.base.reg, &f.base.ast);
    assert!(_expr8);
    let _expr9 = f.base.number_object_type.is_number_object_type(&f.base.reg);
    assert!(_expr9);
    let _expr10 = f.base.number_object_type.is_number_value_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f
        .base
        .number_object_type
        .is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr11);
    let _expr12 = f
        .base
        .number_object_type
        .is_function_prototype_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f
        .base
        .number_object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr14 = _expr13.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr14);
    let _expr15 = f.base.number_object_type.is_regexp_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f
        .base
        .number_object_type
        .is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.base.number_object_type.is_string_object_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.number_object_type.is_string_value_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f
        .base
        .number_object_type
        .is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr19);
    let _expr20 = f.base.number_object_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.number_object_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.number_object_type.is_enum_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.number_object_type.is_union_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f
        .base
        .number_object_type
        .is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f
        .base
        .number_object_type
        .is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr25);
    let _expr26 = f.base.number_object_type.is_all_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.number_object_type.is_void_type(&f.base.reg);
    assert!(!_expr27);
    let _expr28 = f.base.number_object_type.is_constructor(&f.base.reg);
    assert!(!_expr28);
    let _expr29 = f.base.number_object_type.is_instance_type(&f.base.reg);
    assert!(_expr29);
    let _expr30 = f.base.number_type.autoboxes_to(&f.base.reg);
    TypeSubject::assert_type(_expr30).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    let _expr31 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr31);
    let _expr32 = f.base.number_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr32);
    let _expr33 = f.base.number_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr33);
    let _expr34 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr34);
    let _expr35 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr35);
    let _expr36 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr36);
    let _expr37 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr37);
    let _expr38 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr38);
    let _expr39 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr39);
    let _expr40 = f.base.number_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.unresolved_named_type,
    );
    assert!(_expr40);
    let _expr41 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr41);
    let _expr42 = f
        .base
        .create_union_type(&[f.base.number_object_type, f.base.null_type]);
    let _expr43 = f
        .base
        .number_object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr42);
    assert!(_expr43);
    let _expr44 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.null_type]);
    let _expr45 = f
        .base
        .number_object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr44);
    assert!(!_expr45);
    let _expr46 =
        f.base
            .number_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr46);
    let _expr47 = f
        .base
        .number_object_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr47);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.no_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.string_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.number_object_type, f.base.symbol_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.number_object_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.elements_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.number_object_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_object_type, f.base.array_type);
    let _expr48 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(_expr48);
    let _expr49 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr49);
    let _expr50 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr50);
    let _expr51 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr51);
    let _expr52 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.boolean_object_type,
        );
    assert!(!_expr52);
    let _expr53 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr53);
    let _expr54 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr54);
    let _expr55 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr55);
    let _expr56 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr56);
    let _expr57 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.number_object_type,
        );
    assert!(_expr57);
    let _expr58 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr58);
    let _expr59 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr59);
    let _expr60 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr60);
    let _expr61 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.string_object_type,
        );
    assert!(!_expr61);
    let _expr62 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr62);
    let _expr63 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.symbol_object_type,
        );
    assert!(!_expr63);
    let _expr64 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr64);
    let _expr65 = f
        .base
        .number_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr65);
    let _expr66 = f
        .base
        .number_object_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr66);
    let _expr67 = f
        .base
        .number_object_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr67);
    let _expr68 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    TypeSubject::assert_type(_expr68).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr69 = f
        .base
        .create_union_type(&[f.base.number_object_type, f.base.string_object_type]);
    let _expr70 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr70).is_equal_to(&mut f.base.reg, &f.base.ast, _expr69);
    let _expr71 = f
        .base
        .create_union_type(&[f.base.number_object_type, f.base.symbol_object_type]);
    let _expr72 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(_expr72).is_equal_to(&mut f.base.reg, &f.base.ast, _expr71);
    let _expr73 = f
        .base
        .create_union_type(&[f.base.number_object_type, f.base.symbol_type]);
    let _expr74 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    TypeSubject::assert_type(_expr74).is_equal_to(&mut f.base.reg, &f.base.ast, _expr73);
    let _expr75 = f
        .base
        .create_union_type(&[f.base.number_object_type, f.base.number_type]);
    let _expr76 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    TypeSubject::assert_type(_expr76).is_equal_to(&mut f.base.reg, &f.base.ast, _expr75);
    let _expr77 = f
        .base
        .create_union_type(&[f.base.number_object_type, f.function_type]);
    let _expr78 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    TypeSubject::assert_type(_expr78).is_equal_to(&mut f.base.reg, &f.base.ast, _expr77);
    let _expr79 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    TypeSubject::assert_type(_expr79).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr80 = f
        .base
        .create_union_type(&[f.base.number_object_type, f.base.date_type]);
    let _expr81 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    TypeSubject::assert_type(_expr81).is_equal_to(&mut f.base.reg, &f.base.ast, _expr80);
    let _expr82 = f
        .base
        .create_union_type(&[f.base.number_object_type, f.base.regexp_type]);
    let _expr83 = f.base.number_object_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    TypeSubject::assert_type(_expr83).is_equal_to(&mut f.base.reg, &f.base.ast, _expr82);
    let _expr84 = f
        .base
        .number_object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr84);
    let _expr85 = f
        .base
        .number_object_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr85);
    let _expr86 = f
        .base
        .number_object_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr86);
    let _expr87 = f
        .base
        .number_object_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr87);
    let _expr88 = f
        .base
        .number_object_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr88, "Number");
    let _expr89 = f.base.number_object_type.has_display_name(&f.base.reg);
    assert!(_expr89);
    let _expr90 = f.base.number_object_type.get_display_name(&f.base.reg);
    assert_eq!(_expr90, Some("Number".into()));
    let _expr91 = f.base.number_object_type.is_native_object_type(&f.base.reg);
    assert!(_expr91);
    let _expr92 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
}

// port: JSTypeTest#testNumberValueType
#[test]
fn test_number_value_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.number_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.number_type.is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.number_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.number_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.number_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.number_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.number_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.number_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(_expr8);
    let _expr9 = f.base.number_type.is_number_object_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.number_type.is_number_value_type(&f.base.reg);
    assert!(_expr10);
    let _expr11 = f.base.number_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.number_type.is_regexp_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.number_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr13);
    let _expr14 = f.base.number_type.is_string_object_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.number_type.is_string_value_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.number_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.base.number_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.number_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.number_type.is_enum_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.number_type.is_union_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.number_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr21);
    let _expr22 = f.base.number_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr22);
    let _expr23 = f.base.number_type.is_all_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.number_type.is_void_type(&f.base.reg);
    assert!(!_expr24);
    let _expr25 = f.base.number_type.is_constructor(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.number_type.is_instance_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.number_type.autoboxes_to(&f.base.reg);
    TypeSubject::assert_type(_expr27).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    let _expr28 = f
        .base
        .number_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr28);
    let _expr29 =
        f.base
            .number_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr29);
    let _expr30 =
        f.base
            .number_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr30);
    let _expr31 =
        f.base
            .number_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .number_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr32);
    let _expr33 = f
        .base
        .number_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr33);
    let _expr34 = f
        .base
        .number_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr34);
    let _expr35 =
        f.base
            .number_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr35);
    let _expr36 = f
        .base
        .number_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr36);
    let _expr37 =
        f.base
            .number_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr37);
    let _expr38 = f
        .base
        .number_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.null_type]);
    let _expr40 = f
        .base
        .number_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr39);
    assert!(_expr40);
    let _expr41 =
        f.base
            .number_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr41);
    let _expr42 = f
        .base
        .number_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr42);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.no_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.bigint_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.string_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.number_type, f.base.symbol_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.number_type, f.base.symbol_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.number_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.number_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(f.base.number_type, f.base.unknown_type);
    let _expr43 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr43);
    let _expr44 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr44);
    let _expr45 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr45);
    let _expr46 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr46);
    let _expr47 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr47);
    let _expr48 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr48);
    let _expr49 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr49);
    let _expr50 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr50);
    let _expr51 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(_expr51);
    let _expr52 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr52);
    let _expr53 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(!_expr53);
    let _expr54 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr54);
    let _expr55 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr55);
    let _expr56 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr56);
    let _expr57 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr57);
    let _expr58 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr58);
    let _expr59 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr59);
    let _expr60 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr60);
    let _expr61 = f.base.number_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr61);
    let _expr62 = f.base.number_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr62);
    let _expr63 = f.base.number_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr63);
    let _expr64 =
        f.base
            .number_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr64).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr65 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_object_type]);
    let _expr66 = f.base.number_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr66).is_equal_to(&mut f.base.reg, &f.base.ast, _expr65);
    let _expr67 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.symbol_object_type]);
    let _expr68 = f.base.number_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(_expr68).is_equal_to(&mut f.base.reg, &f.base.ast, _expr67);
    let _expr69 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.symbol_type]);
    let _expr70 =
        f.base
            .number_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    TypeSubject::assert_type(_expr70).is_equal_to(&mut f.base.reg, &f.base.ast, _expr69);
    let _expr71 =
        f.base
            .number_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr71).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr72 = f
        .base
        .create_union_type(&[f.base.number_type, f.function_type]);
    let _expr73 =
        f.base
            .number_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr73).is_equal_to(&mut f.base.reg, &f.base.ast, _expr72);
    let _expr74 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.object_type]);
    let _expr75 =
        f.base
            .number_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr75).is_equal_to(&mut f.base.reg, &f.base.ast, _expr74);
    let _expr76 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.date_type]);
    let _expr77 =
        f.base
            .number_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr77).is_equal_to(&mut f.base.reg, &f.base.ast, _expr76);
    let _expr78 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.regexp_type]);
    let _expr79 =
        f.base
            .number_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr79).is_equal_to(&mut f.base.reg, &f.base.ast, _expr78);
    let _expr80 = f
        .base
        .number_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr80);
    let _expr81 = f
        .base
        .number_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr81);
    let _expr82 = f
        .base
        .number_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr82);
    let _expr83 = f
        .base
        .number_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr83);
    let _expr84 = f.base.number_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr84, "number");
    let _expr85 = f.base.number_type.has_display_name(&f.base.reg);
    assert!(_expr85);
    let _expr86 = f.base.number_type.get_display_name(&f.base.reg);
    assert_eq!(_expr86, Some("number".into()));
    let _expr87 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr88 = f
        .base
        .number_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr88);
}

// port: JSTypeTest#testNullType
#[test]
fn test_null_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.null_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.null_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.null_type.is_date_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.null_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.null_type.is_named_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.null_type.is_null_type(&f.base.reg);
    assert!(_expr6);
    let _expr7 = f.base.null_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr7);
    let _expr8 = f.base.null_type.is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.null_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.null_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f.base.null_type.is_regexp_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.null_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(!_expr12);
    let _expr13 = f.base.null_type.is_string_object_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.base.null_type.is_string_value_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.null_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr15);
    let _expr16 = f.base.null_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = f.base.null_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.null_type.is_enum_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.null_type.is_union_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.null_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr20);
    let _expr21 = f.base.null_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr21);
    let _expr22 = f.base.null_type.is_all_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.null_type.is_void_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.null_type.is_constructor(&f.base.reg);
    assert!(!_expr24);
    let _expr25 = f.base.null_type.is_instance_type(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.null_type.autoboxes_to(&f.base.reg);
    assert!(_expr26.is_none());
    let _expr27 =
        f.base
            .null_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr27);
    let _expr28 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr28);
    let _expr29 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(_expr29);
    let _expr30 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr30);
    let _expr31 =
        f.base
            .null_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .null_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr32);
    let _expr33 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr33);
    let _expr34 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr34);
    let _expr35 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr35);
    let _expr36 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr36);
    let _expr37 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr37);
    let _expr38 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr39);
    let _expr40 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr40);
    TypeSubject::assert_type(f.base.null_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.forward_declared_named_type,
    );
    let _expr41 = f.base.create_nullable_type(f.base.no_object_type);
    let _expr42 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr41);
    assert!(_expr42);
    let _expr43 = f.base.create_nullable_type(f.base.no_type);
    let _expr44 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr43);
    assert!(_expr44);
    let _expr45 = f.base.create_nullable_type(f.base.null_type);
    let _expr46 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr45);
    assert!(_expr46);
    let _expr47 = f.base.create_nullable_type(f.base.all_type);
    let _expr48 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr47);
    assert!(_expr48);
    let _expr49 = f.base.create_nullable_type(f.base.string_object_type);
    let _expr50 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr49);
    assert!(_expr50);
    let _expr51 = f.base.create_nullable_type(f.base.symbol_object_type);
    let _expr52 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr51);
    assert!(_expr52);
    let _expr53 = f.base.create_nullable_type(f.base.symbol_type);
    let _expr54 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr53);
    assert!(_expr54);
    let _expr55 = f.base.create_nullable_type(f.base.number_type);
    let _expr56 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr55);
    assert!(_expr56);
    let _expr57 = f.base.create_nullable_type(f.function_type);
    let _expr58 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr57);
    assert!(_expr58);
    let _expr59 = f.base.create_nullable_type(f.base.object_type);
    let _expr60 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr59);
    assert!(_expr60);
    let _expr61 = f.base.create_nullable_type(f.base.date_type);
    let _expr62 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr61);
    assert!(_expr62);
    let _expr63 = f.base.create_nullable_type(f.base.regexp_type);
    let _expr64 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr63);
    assert!(_expr64);
    let _expr65 = f.base.create_nullable_type(f.base.array_type);
    let _expr66 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr65);
    assert!(_expr66);
    let _expr67 = f.base.null_type.can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr67);
    f.base
        .assert_can_test_for_equality_with(f.base.null_type, f.base.no_type);
    f.base
        .assert_can_test_for_equality_with(f.base.null_type, f.base.no_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.null_type, f.base.all_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.array_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.boolean_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.boolean_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.date_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.null_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.number_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.number_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.regexp_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.string_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.string_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.symbol_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.symbol_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.void_type);
    let _expr68 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr68);
    let _expr69 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr69);
    let _expr70 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr70);
    let _expr71 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr71);
    let _expr72 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr72);
    let _expr73 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr73);
    let _expr74 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr74);
    let _expr75 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(_expr75);
    let _expr76 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr76);
    let _expr77 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr77);
    let _expr78 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(!_expr78);
    let _expr79 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr79);
    let _expr80 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr80);
    let _expr81 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr81);
    let _expr82 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr82);
    let _expr83 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr83);
    let _expr84 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr84);
    let _expr85 = f.base.null_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr85);
    let _expr86 = f.base.create_nullable_type(f.base.string_object_type);
    let _expr87 =
        f.base
            .null_type
            .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, _expr86);
    assert!(_expr87);
    let _expr88 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    TypeSubject::assert_type(_expr88).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr89 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr89).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr90 = f.base.create_nullable_type(f.base.string_object_type);
    let _expr91 = f.base.null_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr91).is_equal_to(&mut f.base.reg, &f.base.ast, _expr90);
    let _expr92 = f.base.create_nullable_type(f.base.symbol_object_type);
    let _expr93 = f.base.null_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    TypeSubject::assert_type(_expr93).is_equal_to(&mut f.base.reg, &f.base.ast, _expr92);
    let _expr94 = f.base.create_nullable_type(f.base.symbol_type);
    let _expr95 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    TypeSubject::assert_type(_expr95).is_equal_to(&mut f.base.reg, &f.base.ast, _expr94);
    let _expr96 = f.base.create_nullable_type(f.base.number_type);
    let _expr97 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr97).is_equal_to(&mut f.base.reg, &f.base.ast, _expr96);
    let _expr98 = f.base.create_nullable_type(f.function_type);
    let _expr99 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr99).is_equal_to(&mut f.base.reg, &f.base.ast, _expr98);
    let _expr100 = f.base.create_nullable_type(f.base.object_type);
    let _expr101 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr101).is_equal_to(&mut f.base.reg, &f.base.ast, _expr100);
    let _expr102 = f.base.create_nullable_type(f.base.date_type);
    let _expr103 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr103).is_equal_to(&mut f.base.reg, &f.base.ast, _expr102);
    let _expr104 = f.base.create_nullable_type(f.base.regexp_type);
    let _expr105 =
        f.base
            .null_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr105).is_equal_to(&mut f.base.reg, &f.base.ast, _expr104);
    let _expr106 = f
        .base
        .null_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr106);
    let _expr107 = f
        .base
        .null_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr107);
    let _expr108 = f
        .base
        .null_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr108);
    let _expr109 = f
        .base
        .null_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr109);
    let _expr110 = f.base.null_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr110, "null");
    let _expr111 = f.base.null_type.has_display_name(&f.base.reg);
    assert!(_expr111);
    let _expr112 = f.base.null_type.get_display_name(&f.base.reg);
    assert_eq!(_expr112, Some("null".into()));
    let _expr113 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr114 = f
        .base
        .create_union_type(&[f.forward_declared_named_type, f.base.null_type]);
    let _expr115 = f
        .base
        .null_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr114);
    assert!(_expr115);
    let _expr116 = f
        .base
        .create_union_type(&[f.forward_declared_named_type, f.base.null_type]);
    let fwd_declared_null_union = _expr116;
    let _expr117 = f.base.null_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        fwd_declared_null_union,
    );
    TypeSubject::assert_type(_expr117).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr118 = f
        .base
        .null_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr118);
    let _expr119 = f
        .base
        .null_type
        .differs_from(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr119);
}

// port: JSTypeTest#testDateTypeX
#[test]
fn test_date_type_x() {
    let mut f = Fixture::set_up();
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_object_type, f.function_type);
}

// port: JSTypeTest#testDateType
#[test]
fn test_date_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.date_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.date_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.date_type.is_date_type(&f.base.reg);
    assert!(_expr3);
    let _expr4 = f.base.date_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.date_type.is_named_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.date_type.is_null_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.date_type.is_number_value_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.date_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f
        .base
        .date_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr10 = _expr9.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr10);
    let _expr11 = f.base.date_type.is_regexp_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.date_type.is_string_value_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.date_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.base.date_type.is_enum_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.date_type.is_union_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.date_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.base.date_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr17);
    let _expr18 = f.base.date_type.is_all_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.date_type.is_void_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.date_type.is_constructor(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.date_type.is_instance_type(&f.base.reg);
    assert!(_expr21);
    let _expr22 = f.base.date_type.autoboxes_to(&f.base.reg);
    assert!(_expr22.is_none());
    let _expr23 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr23);
    let _expr24 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr24);
    let _expr25 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr25);
    let _expr26 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr26);
    let _expr27 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(!_expr27);
    let _expr28 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr28);
    let _expr29 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr29);
    let _expr30 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr30);
    let _expr31 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(!_expr32);
    let _expr33 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr33);
    let _expr34 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr34);
    let _expr35 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr35);
    let _expr36 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr36);
    let _expr37 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr37);
    let _expr38 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr39);
    let _expr40 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr40);
    let _expr41 = f.base.date_type.can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr41);
    f.base
        .assert_can_test_for_equality_with(f.base.date_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.date_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.date_type, f.base.number_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.date_type, f.base.symbol_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.date_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.date_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.date_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.date_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.date_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.date_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.date_type, f.base.array_type);
    let _expr42 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr42);
    let _expr43 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr43);
    let _expr44 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr44);
    let _expr45 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr45);
    let _expr46 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr46);
    let _expr47 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(_expr47);
    let _expr48 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr48);
    let _expr49 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr49);
    let _expr50 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr50);
    let _expr51 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr51);
    let _expr52 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr52);
    let _expr53 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr53);
    let _expr54 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr54);
    let _expr55 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr55);
    let _expr56 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr56);
    let _expr57 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr57);
    let _expr58 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr58);
    let _expr59 = f.base.date_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr59);
    let _expr60 = f.base.date_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr60);
    let _expr61 = f.base.date_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr61);
    let _expr62 = f.base.create_nullable_type(f.base.date_type);
    let _expr63 = _expr62.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr63);
    let _expr64 = f
        .base
        .create_union_type(&[f.base.date_type, f.base.void_type]);
    let _expr65 = _expr64.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr65);
    let _expr66 =
        f.base
            .date_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr66).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr67 = f
        .base
        .create_union_type(&[f.base.date_type, f.base.string_object_type]);
    let _expr68 = f.base.date_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr68).is_equal_to(&mut f.base.reg, &f.base.ast, _expr67);
    let _expr69 = f
        .base
        .create_union_type(&[f.base.date_type, f.base.number_type]);
    let _expr70 =
        f.base
            .date_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr70).is_equal_to(&mut f.base.reg, &f.base.ast, _expr69);
    let _expr71 = f
        .base
        .create_union_type(&[f.base.date_type, f.function_type]);
    let _expr72 =
        f.base
            .date_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr72).is_equal_to(&mut f.base.reg, &f.base.ast, _expr71);
    let _expr73 =
        f.base
            .date_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr73).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr74 =
        f.base
            .date_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr74).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.date_type);
    let _expr75 = f
        .base
        .create_union_type(&[f.base.date_type, f.base.regexp_type]);
    let _expr76 =
        f.base
            .date_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr76).is_equal_to(&mut f.base.reg, &f.base.ast, _expr75);
    let _expr77 = f
        .base
        .date_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr78 = _expr77
        .unwrap()
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr78, 46);
    let _expr79 = f
        .base
        .date_type
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr79, 46);
    let _expr80 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "constructor");
    f.assert_return_type_equals(f.base.date_type, _expr80);
    let _expr81 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toString");
    f.assert_return_type_equals(f.base.string_type, _expr81);
    let _expr82 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toDateString");
    f.assert_return_type_equals(f.base.string_type, _expr82);
    let _expr83 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toTimeString");
    f.assert_return_type_equals(f.base.string_type, _expr83);
    let _expr84 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "toLocaleString");
    f.assert_return_type_equals(f.base.string_type, _expr84);
    let _expr85 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "toLocaleDateString");
    f.assert_return_type_equals(f.base.string_type, _expr85);
    let _expr86 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "toLocaleTimeString");
    f.assert_return_type_equals(f.base.string_type, _expr86);
    let _expr87 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "valueOf");
    f.assert_return_type_equals(f.base.number_type, _expr87);
    let _expr88 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getTime");
    f.assert_return_type_equals(f.base.number_type, _expr88);
    let _expr89 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getFullYear");
    f.assert_return_type_equals(f.base.number_type, _expr89);
    let _expr90 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "getUTCFullYear");
    f.assert_return_type_equals(f.base.number_type, _expr90);
    let _expr91 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getMonth");
    f.assert_return_type_equals(f.base.number_type, _expr91);
    let _expr92 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getUTCMonth");
    f.assert_return_type_equals(f.base.number_type, _expr92);
    let _expr93 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getDate");
    f.assert_return_type_equals(f.base.number_type, _expr93);
    let _expr94 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getUTCDate");
    f.assert_return_type_equals(f.base.number_type, _expr94);
    let _expr95 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getDay");
    f.assert_return_type_equals(f.base.number_type, _expr95);
    let _expr96 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getUTCDay");
    f.assert_return_type_equals(f.base.number_type, _expr96);
    let _expr97 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getHours");
    f.assert_return_type_equals(f.base.number_type, _expr97);
    let _expr98 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getUTCHours");
    f.assert_return_type_equals(f.base.number_type, _expr98);
    let _expr99 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getMinutes");
    f.assert_return_type_equals(f.base.number_type, _expr99);
    let _expr100 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "getUTCMinutes");
    f.assert_return_type_equals(f.base.number_type, _expr100);
    let _expr101 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "getSeconds");
    f.assert_return_type_equals(f.base.number_type, _expr101);
    let _expr102 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "getUTCSeconds");
    f.assert_return_type_equals(f.base.number_type, _expr102);
    let _expr103 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "getMilliseconds");
    f.assert_return_type_equals(f.base.number_type, _expr103);
    let _expr104 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "getUTCMilliseconds");
    f.assert_return_type_equals(f.base.number_type, _expr104);
    let _expr105 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "getTimezoneOffset");
    f.assert_return_type_equals(f.base.number_type, _expr105);
    let _expr106 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setTime");
    f.assert_return_type_equals(f.base.number_type, _expr106);
    let _expr107 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "setMilliseconds");
    f.assert_return_type_equals(f.base.number_type, _expr107);
    let _expr108 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "setUTCMilliseconds");
    f.assert_return_type_equals(f.base.number_type, _expr108);
    let _expr109 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setSeconds");
    f.assert_return_type_equals(f.base.number_type, _expr109);
    let _expr110 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "setUTCSeconds");
    f.assert_return_type_equals(f.base.number_type, _expr110);
    let _expr111 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "setUTCSeconds");
    f.assert_return_type_equals(f.base.number_type, _expr111);
    let _expr112 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setMinutes");
    f.assert_return_type_equals(f.base.number_type, _expr112);
    let _expr113 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "setUTCMinutes");
    f.assert_return_type_equals(f.base.number_type, _expr113);
    let _expr114 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setHours");
    f.assert_return_type_equals(f.base.number_type, _expr114);
    let _expr115 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setUTCHours");
    f.assert_return_type_equals(f.base.number_type, _expr115);
    let _expr116 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setDate");
    f.assert_return_type_equals(f.base.number_type, _expr116);
    let _expr117 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setUTCDate");
    f.assert_return_type_equals(f.base.number_type, _expr117);
    let _expr118 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setMonth");
    f.assert_return_type_equals(f.base.number_type, _expr118);
    let _expr119 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setUTCMonth");
    f.assert_return_type_equals(f.base.number_type, _expr119);
    let _expr120 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "setFullYear");
    f.assert_return_type_equals(f.base.number_type, _expr120);
    let _expr121 =
        f.base
            .date_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "setUTCFullYear");
    f.assert_return_type_equals(f.base.number_type, _expr121);
    let _expr122 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toUTCString");
    f.assert_return_type_equals(f.base.string_type, _expr122);
    let _expr123 = f
        .base
        .date_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toGMTString");
    f.assert_return_type_equals(f.base.string_type, _expr123);
    let _expr124 = f
        .base
        .date_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr124);
    let _expr125 = f
        .base
        .date_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr125);
    let _expr126 = f
        .base
        .date_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr126);
    let _expr127 = f
        .base
        .date_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr127);
    let _expr128 = f.base.date_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr128, "Date");
    let _expr129 = f.base.date_type.has_display_name(&f.base.reg);
    assert!(_expr129);
    let _expr130 = f.base.date_type.get_display_name(&f.base.reg);
    assert_eq!(_expr130, Some("Date".into()));
    let _expr131 = f.base.date_type.is_native_object_type(&f.base.reg);
    assert!(_expr131);
    let _expr132 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.date_type);
    let _expr133 = f
        .base
        .date_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr133);
    let _expr134 = f.base.date_type.get_constructor(&f.base.reg);
    let _expr135 = _expr134
        .unwrap()
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(_expr135);
}

// port: JSTypeTest#testRegExpType
#[test]
fn test_reg_exp_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.regexp_type.is_no_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.regexp_type.is_no_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.regexp_type.is_array_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.regexp_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.regexp_type.is_date_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.regexp_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.regexp_type.is_named_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.regexp_type.is_null_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.regexp_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.regexp_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f
        .base
        .regexp_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr12 = _expr11.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr12);
    let _expr13 = f.base.regexp_type.is_regexp_type(&f.base.reg);
    assert!(_expr13);
    let _expr14 = f.base.regexp_type.is_string_value_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.regexp_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.regexp_type.is_enum_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = f.base.regexp_type.is_union_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.regexp_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr18);
    let _expr19 = f.base.regexp_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr19);
    let _expr20 = f.base.regexp_type.is_all_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.regexp_type.is_void_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.regexp_type.autoboxes_to(&f.base.reg);
    assert!(_expr22.is_none());
    let _expr23 = f
        .base
        .regexp_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr23);
    let _expr24 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr24);
    let _expr25 = f
        .base
        .regexp_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr25);
    let _expr26 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr26);
    let _expr27 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(!_expr27);
    let _expr28 = f
        .base
        .regexp_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr28);
    let _expr29 = f
        .base
        .regexp_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr29);
    let _expr30 = f
        .base
        .regexp_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr30);
    let _expr31 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(!_expr32);
    let _expr33 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr33);
    let _expr34 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(_expr34);
    let _expr35 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr35);
    let _expr36 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr36);
    let _expr37 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr37);
    let _expr38 =
        f.base
            .regexp_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .regexp_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr39);
    let _expr40 = f
        .base
        .regexp_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .regexp_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr41);
    f.base
        .assert_can_test_for_equality_with(f.base.regexp_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.regexp_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.regexp_type, f.base.number_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.regexp_type, f.base.symbol_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.regexp_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.regexp_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.regexp_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.regexp_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.regexp_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.regexp_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.regexp_type, f.base.array_type);
    let _expr42 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr42);
    let _expr43 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr43);
    let _expr44 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr44);
    let _expr45 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr45);
    let _expr46 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr46);
    let _expr47 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr47);
    let _expr48 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr48);
    let _expr49 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr49);
    let _expr50 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr50);
    let _expr51 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr51);
    let _expr52 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr52);
    let _expr53 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(_expr53);
    let _expr54 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr54);
    let _expr55 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr55);
    let _expr56 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr56);
    let _expr57 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr57);
    let _expr58 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr58);
    let _expr59 = f.base.regexp_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr59);
    let _expr60 = f.base.regexp_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr60);
    let _expr61 = f.base.regexp_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr61);
    let _expr62 = f.base.create_nullable_type(f.base.regexp_type);
    let _expr63 = _expr62.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr63);
    let _expr64 = f
        .base
        .create_union_type(&[f.base.regexp_type, f.base.void_type]);
    let _expr65 = _expr64.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr65);
    let _expr66 =
        f.base
            .regexp_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr66).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr67 = f
        .base
        .create_union_type(&[f.base.regexp_type, f.base.string_object_type]);
    let _expr68 = f.base.regexp_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr68).is_equal_to(&mut f.base.reg, &f.base.ast, _expr67);
    let _expr69 = f
        .base
        .create_union_type(&[f.base.regexp_type, f.base.number_type]);
    let _expr70 =
        f.base
            .regexp_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr70).is_equal_to(&mut f.base.reg, &f.base.ast, _expr69);
    let _expr71 = f
        .base
        .create_union_type(&[f.base.regexp_type, f.function_type]);
    let _expr72 =
        f.base
            .regexp_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr72).is_equal_to(&mut f.base.reg, &f.base.ast, _expr71);
    let _expr73 =
        f.base
            .regexp_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr73).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr74 = f
        .base
        .create_union_type(&[f.base.date_type, f.base.regexp_type]);
    let _expr75 =
        f.base
            .regexp_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr75).is_equal_to(&mut f.base.reg, &f.base.ast, _expr74);
    let _expr76 =
        f.base
            .regexp_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr76).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    let _expr77 = f
        .base
        .regexp_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr78 = _expr77
        .unwrap()
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr78, 9);
    let _expr79 = f
        .base
        .regexp_type
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr79, 14);
    let _expr80 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "constructor");
    f.assert_return_type_equals(f.base.regexp_type, _expr80);
    let _expr81 = f.base.create_nullable_type(f.base.array_type);
    let _expr82 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "exec");
    f.assert_return_type_equals(_expr81, _expr82);
    let _expr83 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "test");
    f.assert_return_type_equals(f.base.boolean_type, _expr83);
    let _expr84 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "toString");
    f.assert_return_type_equals(f.base.string_type, _expr84);
    let _expr85 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "source");
    TypeSubject::assert_type(_expr85).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let _expr86 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "global");
    TypeSubject::assert_type(_expr86).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    let _expr87 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "ignoreCase");
    TypeSubject::assert_type(_expr87).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    let _expr88 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "multiline");
    TypeSubject::assert_type(_expr88).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    let _expr89 = f
        .base
        .regexp_type
        .get_property_type(&mut f.base.reg, &f.base.ast, "lastIndex");
    TypeSubject::assert_type(_expr89).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr90 = f
        .base
        .regexp_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr90);
    let _expr91 = f
        .base
        .regexp_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr91);
    let _expr92 = f
        .base
        .regexp_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr92);
    let _expr93 = f
        .base
        .regexp_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr93);
    let _expr94 = f.base.regexp_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr94, "RegExp");
    let _expr95 = f.base.regexp_type.has_display_name(&f.base.reg);
    assert!(_expr95);
    let _expr96 = f.base.regexp_type.get_display_name(&f.base.reg);
    assert_eq!(_expr96, Some("RegExp".into()));
    let _expr97 = f.base.regexp_type.is_native_object_type(&f.base.reg);
    assert!(_expr97);
    let _expr98 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    let _expr99 = f
        .base
        .regexp_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr99);
    let _expr100 = f.base.regexp_type.get_constructor(&f.base.reg);
    let _expr101 = _expr100
        .unwrap()
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(_expr101);
}

// port: JSTypeTest#testStringObjectType
#[test]
fn test_string_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.string_object_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f
        .base
        .string_object_type
        .is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.string_object_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.string_object_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.string_object_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.string_object_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.string_object_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f
        .base
        .string_object_type
        .is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr8);
    let _expr9 = f.base.string_object_type.is_number_object_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.string_object_type.is_number_value_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f
        .base
        .string_object_type
        .is_function_prototype_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f
        .base
        .string_object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr13 = _expr12.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr13);
    let _expr14 = f.base.string_object_type.is_regexp_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f
        .base
        .string_object_type
        .is_string(&mut f.base.reg, &f.base.ast);
    assert!(_expr15);
    let _expr16 = f.base.string_object_type.is_string_object_type(&f.base.reg);
    assert!(_expr16);
    let _expr17 = f.base.string_object_type.is_string_value_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f
        .base
        .string_object_type
        .is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr18);
    let _expr19 = f.base.string_object_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.string_object_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.string_object_type.is_enum_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.string_object_type.is_union_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f
        .base
        .string_object_type
        .is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr23);
    let _expr24 = f
        .base
        .string_object_type
        .is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f.base.string_object_type.is_all_type(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.string_object_type.is_void_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.string_object_type.is_constructor(&f.base.reg);
    assert!(!_expr27);
    let _expr28 = f.base.string_object_type.is_instance_type(&f.base.reg);
    assert!(_expr28);
    let _expr29 = f.base.string_type.autoboxes_to(&f.base.reg);
    TypeSubject::assert_type(_expr29).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    let _expr30 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr30);
    let _expr31 = f.base.string_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(_expr31);
    let _expr32 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr32);
    let _expr33 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr33);
    let _expr34 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr34);
    let _expr35 = f.base.string_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr35);
    let _expr36 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr36);
    let _expr37 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr37);
    let _expr38 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr38);
    let _expr39 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr39);
    let _expr40 =
        f.base
            .string_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .string_object_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr41);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.string_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.string_object_type, f.base.symbol_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.string_object_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.boolean_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.boolean_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_object_type, f.base.unknown_type);
    let _expr42 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(_expr42);
    let _expr43 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr43);
    let _expr44 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr44);
    let _expr45 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr45);
    let _expr46 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.boolean_object_type,
        );
    assert!(!_expr46);
    let _expr47 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr47);
    let _expr48 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr48);
    let _expr49 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr49);
    let _expr50 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr50);
    let _expr51 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.number_object_type,
        );
    assert!(!_expr51);
    let _expr52 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr52);
    let _expr53 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr53);
    let _expr54 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr54);
    let _expr55 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.string_object_type,
        );
    assert!(_expr55);
    let _expr56 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr56);
    let _expr57 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.symbol_object_type,
        );
    assert!(!_expr57);
    let _expr58 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr58);
    let _expr59 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr59);
    let _expr60 = f
        .base
        .string_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr60);
    let _expr61 = f
        .base
        .string_object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr62 = _expr61
        .unwrap()
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr62, 24);
    let _expr63 = f
        .base
        .string_object_type
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr63, 25);
    let _expr64 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "toString");
    f.assert_return_type_equals(f.base.string_type, _expr64);
    let _expr65 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "valueOf");
    f.assert_return_type_equals(f.base.string_type, _expr65);
    let _expr66 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "charAt");
    f.assert_return_type_equals(f.base.string_type, _expr66);
    let _expr67 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "charCodeAt");
    f.assert_return_type_equals(f.base.number_type, _expr67);
    let _expr68 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "concat");
    f.assert_return_type_equals(f.base.string_type, _expr68);
    let _expr69 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "indexOf");
    f.assert_return_type_equals(f.base.number_type, _expr69);
    let _expr70 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "lastIndexOf");
    f.assert_return_type_equals(f.base.number_type, _expr70);
    let _expr71 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "localeCompare");
    f.assert_return_type_equals(f.base.number_type, _expr71);
    let _expr72 = f.base.create_nullable_type(f.base.array_type);
    let _expr73 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "match");
    f.assert_return_type_equals(_expr72, _expr73);
    let _expr74 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "replace");
    f.assert_return_type_equals(f.base.string_type, _expr74);
    let _expr75 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "search");
    f.assert_return_type_equals(f.base.number_type, _expr75);
    let _expr76 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "slice");
    f.assert_return_type_equals(f.base.string_type, _expr76);
    let _expr77 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "split");
    f.assert_return_type_equals(f.base.array_type, _expr77);
    let _expr78 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "substr");
    f.assert_return_type_equals(f.base.string_type, _expr78);
    let _expr79 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "substring");
    f.assert_return_type_equals(f.base.string_type, _expr79);
    let _expr80 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "toLowerCase");
    f.assert_return_type_equals(f.base.string_type, _expr80);
    let _expr81 = f.base.string_object_type.get_property_type(
        &mut f.base.reg,
        &f.base.ast,
        "toLocaleLowerCase",
    );
    f.assert_return_type_equals(f.base.string_type, _expr81);
    let _expr82 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "toUpperCase");
    f.assert_return_type_equals(f.base.string_type, _expr82);
    let _expr83 = f.base.string_object_type.get_property_type(
        &mut f.base.reg,
        &f.base.ast,
        "toLocaleUpperCase",
    );
    f.assert_return_type_equals(f.base.string_type, _expr83);
    let _expr84 =
        f.base
            .string_object_type
            .get_property_type(&mut f.base.reg, &f.base.ast, "length");
    TypeSubject::assert_type(_expr84).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr85 = f
        .base
        .string_object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr85);
    let _expr86 = f
        .base
        .string_object_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr86);
    let _expr87 = f
        .base
        .string_object_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr87);
    let _expr88 = f
        .base
        .string_object_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr88);
    let _expr89 = f
        .base
        .string_object_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr89);
    let _expr90 = f
        .base
        .string_object_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr90);
    let _expr91 = f.base.create_nullable_type(f.base.string_object_type);
    let _expr92 = _expr91.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr92);
    let _expr93 = f
        .base
        .create_union_type(&[f.base.string_object_type, f.base.void_type]);
    let _expr94 = _expr93.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr94);
    let _expr95 = f.base.string_object_type.is_native_object_type(&f.base.reg);
    assert!(_expr95);
    let _expr96 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    let _expr97 = f.base.string_object_type.has_display_name(&f.base.reg);
    assert!(_expr97);
    let _expr98 = f.base.string_object_type.get_display_name(&f.base.reg);
    assert_eq!(_expr98, Some("String".into()));
    let _expr99 = f
        .base
        .string_object_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr99);
    let _expr100 = f.base.string_object_type.get_constructor(&f.base.reg);
    let _expr101 = _expr100
        .unwrap()
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(_expr101);
}

// port: JSTypeTest#testStringValueType
#[test]
fn test_string_value_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.string_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.string_type.is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.string_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.string_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.string_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.string_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.string_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.string_type.is_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr8);
    let _expr9 = f.base.string_type.is_number_object_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.string_type.is_number_value_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f.base.string_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.string_type.is_regexp_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.string_type.is_string(&mut f.base.reg, &f.base.ast);
    assert!(_expr13);
    let _expr14 = f.base.string_type.is_string_object_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.string_type.is_string_value_type(&f.base.reg);
    assert!(_expr15);
    let _expr16 = f.base.string_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.base.string_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f.base.string_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.string_type.is_enum_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.string_type.is_union_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.string_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr21);
    let _expr22 = f.base.string_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr22);
    let _expr23 = f.base.string_type.is_all_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.string_type.is_void_type(&f.base.reg);
    assert!(!_expr24);
    let _expr25 = f.base.string_type.is_constructor(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = f.base.string_type.is_instance_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 = f.base.string_type.autoboxes_to(&f.base.reg);
    TypeSubject::assert_type(_expr27).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    let _expr28 = f
        .base
        .string_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr28);
    let _expr29 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(_expr29);
    let _expr30 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr30);
    let _expr31 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr32);
    let _expr33 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr33);
    let _expr34 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr34);
    let _expr35 = f
        .base
        .string_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr35);
    let _expr36 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr36);
    let _expr37 = f
        .base
        .string_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr37);
    let _expr38 =
        f.base
            .string_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr38);
    let _expr39 = f
        .base
        .string_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr39);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.string_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.string_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.bigint_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.boolean_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.boolean_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.string_type, f.base.symbol_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.string_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.array_type);
    f.base
        .assert_can_test_for_equality_with(f.base.string_type, f.base.unknown_type);
    let _expr40 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr40);
    let _expr41 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr41);
    let _expr42 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr42);
    let _expr43 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr43);
    let _expr44 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr44);
    let _expr45 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr45);
    let _expr46 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr46);
    let _expr47 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr47);
    let _expr48 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr48);
    let _expr49 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr49);
    let _expr50 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(!_expr50);
    let _expr51 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr51);
    let _expr52 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(_expr52);
    let _expr53 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr53);
    let _expr54 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr54);
    let _expr55 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr55);
    let _expr56 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr56);
    let _expr57 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr57);
    let _expr58 = f.base.string_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr58);
    let _expr59 = f
        .base
        .string_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr59);
    let _expr60 = f
        .base
        .string_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr60);
    let _expr61 = f
        .base
        .string_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr61);
    let _expr62 = f
        .base
        .string_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr62);
    let _expr63 = f.base.string_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr63);
    let _expr64 = f.base.string_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr64);
    let _expr65 = f.base.create_nullable_type(f.base.string_type);
    let _expr66 = _expr65.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr66);
    let _expr67 = f
        .base
        .create_union_type(&[f.base.string_type, f.base.void_type]);
    let _expr68 = _expr67.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr68);
    let _expr69 = f.base.string_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr69, "string");
    let _expr70 = f.base.string_type.has_display_name(&f.base.reg);
    assert!(_expr70);
    let _expr71 = f.base.string_type.get_display_name(&f.base.reg);
    assert_eq!(_expr71, Some("string".into()));
    let _expr72 = f
        .base
        .string_type
        .find_property_type(&mut f.base.reg, &f.base.ast, "length");
    TypeSubject::assert_type(_expr72).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr73 =
        f.base
            .string_type
            .find_property_type(&mut f.base.reg, &f.base.ast, "unknownProperty");
    assert!(_expr73.is_none());
    let _expr74 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let _expr75 = f
        .base
        .string_type
        .is_nominal_constructor_or_interface(&f.base.reg);
    assert!(!_expr75);
}

// port: JSTypeTest#testSymbolValueType
#[test]
fn test_symbol_value_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.symbol_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.symbol_type.is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.symbol_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.symbol_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.symbol_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.symbol_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.symbol_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.symbol_type.is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.symbol_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.symbol_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f.base.symbol_type.is_regexp_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.symbol_type.is_string_object_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.symbol_type.is_string_value_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.base.symbol_type.is_enum_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.symbol_type.is_union_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.symbol_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.base.symbol_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr17);
    let _expr18 = f.base.symbol_type.is_all_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.symbol_type.is_void_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.symbol_type.is_constructor(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.symbol_type.is_instance_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.symbol_type.is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(_expr22);
    let _expr23 = f.base.symbol_type.is_symbol_object_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = f.base.symbol_type.is_symbol_value_type(&f.base.reg);
    assert!(_expr24);
    let _expr25 = f.base.symbol_type.autoboxes_to(&f.base.reg);
    TypeSubject::assert_type(_expr25).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    let _expr26 = f
        .base
        .symbol_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr26);
    let _expr27 =
        f.base
            .symbol_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr27);
    let _expr28 =
        f.base
            .symbol_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr28);
    let _expr29 = f
        .base
        .symbol_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr29);
    let _expr30 = f
        .base
        .symbol_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr30);
    let _expr31 =
        f.base
            .symbol_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr31);
    let _expr32 = f
        .base
        .symbol_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr32);
    let _expr33 =
        f.base
            .symbol_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr33);
    let _expr34 = f
        .base
        .symbol_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr34);
    let _expr35 =
        f.base
            .symbol_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr35);
    let _expr36 = f
        .base
        .symbol_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr36);
    f.base
        .assert_can_test_for_equality_with(f.base.symbol_type, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(f.base.symbol_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.symbol_type, f.base.all_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_type, f.base.string_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_type, f.base.number_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.symbol_type, f.base.object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_type, f.base.date_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.symbol_type, f.base.unknown_type);
    let _expr37 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(_expr37);
    let _expr38 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr38);
    let _expr39 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr39);
    let _expr40 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr40);
    let _expr41 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr41);
    let _expr42 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(_expr42);
    let _expr43 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr43);
    let _expr44 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr44);
    let _expr45 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr45);
    let _expr46 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr46);
    let _expr47 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr47);
    let _expr48 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr48);
    let _expr49 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(!_expr49);
    let _expr50 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr50);
    let _expr51 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr51);
    let _expr52 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr52);
    let _expr53 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr53);
    let _expr54 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr54);
    let _expr55 = f.base.symbol_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr55);
    let _expr56 = f.base.symbol_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr56);
    let _expr57 = f.base.symbol_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr57);
    let _expr58 = f
        .base
        .symbol_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr58);
    let _expr59 = f
        .base
        .symbol_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr59);
    let _expr60 = f
        .base
        .symbol_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr60);
    let _expr61 = f
        .base
        .symbol_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr61);
    let _expr62 = f.base.symbol_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr62, "symbol");
    let _expr63 = f.base.symbol_type.has_display_name(&f.base.reg);
    assert!(_expr63);
    let _expr64 = f.base.symbol_type.get_display_name(&f.base.reg);
    assert_eq!(_expr64, Some("symbol".into()));
    let _expr65 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
}

// port: JSTypeTest#testSymbolObjectType
#[test]
fn test_symbol_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.symbol_object_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f
        .base
        .symbol_object_type
        .is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.symbol_object_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.symbol_object_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.symbol_object_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.symbol_object_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.symbol_object_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.symbol_object_type.is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.symbol_object_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f
        .base
        .symbol_object_type
        .is_function_prototype_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f
        .base
        .symbol_object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr12 = _expr11.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr12);
    let _expr13 = f.base.symbol_object_type.is_regexp_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.base.symbol_object_type.is_string_object_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.symbol_object_type.is_string_value_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.symbol_object_type.is_enum_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = f.base.symbol_object_type.is_union_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f
        .base
        .symbol_object_type
        .is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr18);
    let _expr19 = f
        .base
        .symbol_object_type
        .is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr19);
    let _expr20 = f.base.symbol_object_type.is_all_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.symbol_object_type.is_void_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.symbol_object_type.is_constructor(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.symbol_object_type.is_instance_type(&f.base.reg);
    assert!(_expr23);
    let _expr24 = f
        .base
        .symbol_object_type
        .is_symbol(&mut f.base.reg, &f.base.ast);
    assert!(_expr24);
    let _expr25 = f.base.symbol_object_type.is_symbol_object_type(&f.base.reg);
    assert!(_expr25);
    let _expr26 = f.base.symbol_object_type.is_symbol_value_type(&f.base.reg);
    assert!(!_expr26);
    let _expr27 =
        f.base
            .symbol_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr27);
    let _expr28 = f.base.symbol_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr28);
    let _expr29 =
        f.base
            .symbol_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr29);
    let _expr30 =
        f.base
            .symbol_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr30);
    let _expr31 =
        f.base
            .symbol_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .symbol_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr32);
    let _expr33 =
        f.base
            .symbol_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr33);
    let _expr34 = f.base.symbol_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.unresolved_named_type,
    );
    assert!(_expr34);
    let _expr35 =
        f.base
            .symbol_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr35);
    let _expr36 =
        f.base
            .symbol_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr36);
    let _expr37 = f
        .base
        .symbol_object_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr37);
    f.base
        .assert_can_test_for_equality_with(f.base.symbol_object_type, f.base.all_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_object_type, f.base.string_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_object_type, f.base.number_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_object_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_object_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.symbol_object_type, f.base.object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_object_type, f.base.date_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.symbol_object_type, f.base.regexp_type);
    let _expr38 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(_expr38);
    let _expr39 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr39);
    let _expr40 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr41);
    let _expr42 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.symbol_object_type,
        );
    assert!(_expr42);
    let _expr43 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr43);
    let _expr44 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr44);
    let _expr45 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr45);
    let _expr46 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr46);
    let _expr47 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.number_object_type,
        );
    assert!(!_expr47);
    let _expr48 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr48);
    let _expr49 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr49);
    let _expr50 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr50);
    let _expr51 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.string_object_type,
        );
    assert!(!_expr51);
    let _expr52 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr52);
    let _expr53 = f
        .base
        .symbol_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr53);
    let _expr54 = f
        .base
        .symbol_object_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr54);
    let _expr55 = f
        .base
        .symbol_object_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr55);
    let _expr56 = f
        .base
        .symbol_object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr56);
    let _expr57 = f
        .base
        .symbol_object_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr57);
    let _expr58 = f
        .base
        .symbol_object_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr58);
    let _expr59 = f
        .base
        .symbol_object_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr59);
    let _expr60 = f
        .base
        .symbol_object_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr60, "Symbol");
    let _expr61 = f.base.symbol_object_type.has_display_name(&f.base.reg);
    assert!(_expr61);
    let _expr62 = f.base.symbol_object_type.get_display_name(&f.base.reg);
    assert_eq!(_expr62, Some("Symbol".into()));
    let _expr63 = f.base.symbol_object_type.is_native_object_type(&f.base.reg);
    assert!(_expr63);
    let _expr64 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
}

// port: JSTypeTest#testRecordType
#[test]
fn test_record_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.record_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr1);
    let _expr2 = f.record_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr3);
    let _expr4 =
        f.record_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr4);
    let _expr5 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr5);
    let _expr6 =
        f.record_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr6);
    let _expr7 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr7);
    let _expr8 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr8);
    let _expr9 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr9);
    let _expr10 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr10);
    let _expr11 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr11);
    let _expr12 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(!_expr12);
    let _expr13 = f.record_type.autoboxes_to(&f.base.reg);
    assert!(_expr13.is_none());
    let _expr14 = f.record_type.can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr14);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.record_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.base.symbol_object_type);
    f.base
        .assert_can_test_for_equality_with(f.record_type, f.base.symbol_type);
    let _expr15 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr15);
    let _expr16 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr16);
    let _expr17 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr17);
    let _expr18 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr18);
    let _expr19 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr19);
    let _expr20 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr20);
    let _expr21 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.record_type,
    );
    assert!(_expr21);
    let _expr22 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr22);
    let _expr23 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr23);
    let _expr24 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr24);
    let _expr25 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr25);
    let _expr26 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr26);
    let _expr27 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr27);
    let _expr28 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr28);
    let _expr29 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr29);
    let _expr30 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr30);
    let _expr31 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr31);
    let _expr32 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr32);
    let _expr33 = f.record_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr33);
    let _expr34 = f
        .record_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr34);
    let _expr35 = f
        .record_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr35);
    let _expr36 = f
        .record_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr36);
    let _expr37 = f
        .record_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr37);
    let _expr38 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.record_type);
}

// port: JSTypeTest#testFunctionInstanceType
#[test]
fn test_function_instance_type() {
    let mut f = Fixture::set_up();
    let function_inst = f.base.function_type;
    let _expr1 = function_inst.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr1);
    let _expr2 = function_inst.is_function_prototype_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = function_inst.get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr4 = _expr3.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr4);
    let _expr5 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr5);
    let _expr6 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr6);
    let _expr7 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr7);
    let _expr8 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr8);
    let _expr9 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr9);
    let _expr10 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr10);
    let _expr11 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr11);
    let _expr12 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr12);
    let _expr13 = function_inst.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr13);
    let _expr14 = function_inst.autoboxes_to(&f.base.reg);
    assert!(_expr14.is_none());
    let _expr15 = function_inst.can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(_expr15);
    f.base
        .assert_can_test_for_equality_with(function_inst, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(function_inst, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(function_inst, function_inst);
    f.base
        .assert_can_test_for_equality_with(function_inst, f.base.object_type);
    f.base
        .assert_cannot_test_for_equality_with(function_inst, f.base.number_type);
    f.base
        .assert_cannot_test_for_equality_with(function_inst, f.base.symbol_object_type);
    f.base
        .assert_cannot_test_for_equality_with(function_inst, f.base.symbol_type);
    f.base
        .assert_can_test_for_equality_with(function_inst, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(function_inst, f.base.regexp_type);
    let _expr16 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr16);
    let _expr17 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr17);
    let _expr18 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr18);
    let _expr19 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr19);
    let _expr20 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr20);
    let _expr21 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr21);
    let _expr22 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        function_inst,
    );
    assert!(_expr22);
    let _expr23 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr23);
    let _expr24 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr24);
    let _expr25 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr25);
    let _expr26 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr26);
    let _expr27 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr27);
    let _expr28 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr28);
    let _expr29 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr29);
    let _expr30 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr30);
    let _expr31 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr31);
    let _expr32 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr32);
    let _expr33 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr33);
    let _expr34 = function_inst.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr34);
    let _expr35 = function_inst.matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr35);
    let _expr36 = function_inst.matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr36);
    let _expr37 = function_inst.matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr37);
    let _expr38 = function_inst.matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr38);
    let _expr39 = function_inst.has_property(&mut f.base.reg, &f.base.ast, "prototype");
    assert!(_expr39);
    f.assert_property_type_inferred(function_inst, "prototype");
    let _expr40 = function_inst.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr40).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_function_type,
    );
    let _expr41 = function_inst.get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr41).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_prototype,
    );
    let _expr42 = f.base.function_function_type.get_instance_type(&f.base.reg);
    TypeSubject::assert_type(_expr42).is_equal_to(&mut f.base.reg, &f.base.ast, function_inst);
    let _expr43 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, function_inst);
}

// port: JSTypeTest#testFunctionType
#[test]
fn test_function_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.function_type.is_object(&mut f.base.reg, &f.base.ast);
    assert!(_expr1);
    let _expr2 = f.function_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f
        .function_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr4 = _expr3.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr4);
    let _expr5 = f
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr5);
    let _expr6 =
        f.function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr6);
    let _expr7 = f
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr7);
    let _expr8 =
        f.function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.symbol_object_type);
    assert!(!_expr8);
    let _expr9 = f
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.symbol_type);
    assert!(!_expr9);
    let _expr10 = f
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr10);
    let _expr11 = f
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr11);
    let _expr12 = f
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    assert!(_expr12);
    let _expr13 = f
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr13);
    let _expr14 = f.function_type.autoboxes_to(&f.base.reg);
    assert!(_expr14.is_none());
    let _expr15 = f.function_type.can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(_expr15);
    f.base
        .assert_can_test_for_equality_with(f.function_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.function_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.function_type, f.function_type);
    f.base
        .assert_can_test_for_equality_with(f.function_type, f.base.object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.function_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.function_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.function_type, f.base.regexp_type);
    f.base
        .assert_cannot_test_for_equality_with(f.function_type, f.base.symbol_object_type);
    f.base
        .assert_cannot_test_for_equality_with(f.function_type, f.base.symbol_type);
    let _expr16 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr16);
    let _expr17 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr17);
    let _expr18 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr18);
    let _expr19 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr19);
    let _expr20 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr20);
    let _expr21 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr21);
    let _expr22 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(_expr22);
    let _expr23 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr23);
    let _expr24 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr24);
    let _expr25 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr25);
    let _expr26 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr26);
    let _expr27 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr27);
    let _expr28 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr28);
    let _expr29 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr29);
    let _expr30 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_type,
    );
    assert!(!_expr30);
    let _expr31 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.symbol_object_type,
    );
    assert!(!_expr31);
    let _expr32 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr32);
    let _expr33 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr33);
    let _expr34 = f.function_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr34);
    let _expr35 = f
        .function_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr35);
    let _expr36 = f
        .function_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr36);
    let _expr37 = f
        .function_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr37);
    let _expr38 = f
        .function_type
        .matches_symbol_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr38);
    let _expr39 = f
        .function_type
        .has_property(&mut f.base.reg, &f.base.ast, "prototype");
    assert!(_expr39);
    f.assert_property_type_inferred(f.function_type, "prototype");
    let _expr40 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.function_type);
    let _expr41 = FunctionTypeBuilder::new();
    let _expr42 = _expr41.with_name("aFunctionName");
    let _expr43 = _expr42.build(&mut f.base.reg, &f.base.ast);
    let _expr44 = _expr43.get_display_name(&f.base.reg);
    assert_eq!(_expr44, Some("aFunctionName".into()));
}

// port: JSTypeTest#testRecordTypeSubtyping
#[test]
fn test_record_type_subtyping() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("a", f.base.number_type, None);
    builder.add_property("b", f.base.string_type, None);
    builder.add_property("c", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let sub_record_type = _expr2;
    let _expr3 = sub_record_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.record_type);
    assert!(_expr3);
    let _expr4 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, sub_record_type);
    assert!(!_expr4);
    let mut _expr5 = RecordTypeBuilder::new();
    builder = _expr5;
    builder.add_property("a", f.base.object_type, None);
    builder.add_property("b", f.base.string_type, None);
    let _expr6 = builder.build(&mut f.base.reg, &f.base.ast);
    let different_record_type = _expr6;
    let _expr7 = different_record_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.record_type);
    assert!(!_expr7);
    let _expr8 = f
        .record_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, different_record_type);
    assert!(!_expr8);
}

// port: JSTypeTest#testRecordTypeSubtypingWithInferredProperties
#[test]
fn test_record_type_subtyping_with_inferred_properties() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("a", f.goog_sub_bar_inst, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let record = _expr2;
    let _expr3 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let subtype_prop = _expr3;
    let _expr4 = subtype_prop.define_inferred_property(
        &mut f.base.reg,
        &f.base.ast,
        "a",
        f.goog_sub_sub_bar_inst,
        None,
    );
    let _expr5 = subtype_prop.is_subtype_of(&mut f.base.reg, &f.base.ast, record);
    assert!(_expr5);
    let _expr6 = record.is_subtype_of(&mut f.base.reg, &f.base.ast, subtype_prop);
    assert!(!_expr6);
    let _expr7 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let supertype_prop = _expr7;
    let _expr8 = supertype_prop.define_inferred_property(
        &mut f.base.reg,
        &f.base.ast,
        "a",
        f.goog_bar_inst,
        None,
    );
    let _expr9 = supertype_prop.is_subtype_of(&mut f.base.reg, &f.base.ast, record);
    assert!(!_expr9);
    let _expr10 = record.is_subtype_of(&mut f.base.reg, &f.base.ast, supertype_prop);
    assert!(!_expr10);
    let _expr11 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let declared_subtype_prop = _expr11;
    let _expr12 = declared_subtype_prop.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "a",
        f.goog_sub_sub_bar_inst,
        None,
    );
    let _expr13 = declared_subtype_prop.is_subtype_of(&mut f.base.reg, &f.base.ast, record);
    assert!(_expr13);
    let _expr14 = record.is_subtype_of(&mut f.base.reg, &f.base.ast, declared_subtype_prop);
    assert!(!_expr14);
}

// port: JSTypeTest#testRecordTypeLeastSuperType1
#[test]
fn test_record_type_least_super_type1() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("a", f.base.number_type, None);
    builder.add_property("b", f.base.string_type, None);
    builder.add_property("c", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let sub_record_type = _expr2;
    let _expr3 = f
        .record_type
        .get_least_supertype(&mut f.base.reg, &f.base.ast, sub_record_type);
    let least_supertype = _expr3;
    let _expr4 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.record_type, sub_record_type]);
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, least_supertype);
}

// port: JSTypeTest#testRecordTypeLeastSuperType2
#[test]
fn test_record_type_least_super_type2() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("e", f.base.number_type, None);
    builder.add_property("b", f.base.string_type, None);
    builder.add_property("c", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let other_record_type = _expr2;
    let _expr3 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.record_type, other_record_type]);
    let _expr4 = f
        .record_type
        .get_least_supertype(&mut f.base.reg, &f.base.ast, other_record_type);
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, _expr3);
}

// port: JSTypeTest#testRecordTypeLeastSuperType3
#[test]
fn test_record_type_least_super_type3() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("d", f.base.number_type, None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let other_record_type = _expr2;
    let _expr3 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.record_type, other_record_type]);
    let _expr4 = f
        .record_type
        .get_least_supertype(&mut f.base.reg, &f.base.ast, other_record_type);
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, _expr3);
}

// port: JSTypeTest#testRecordTypeLeastSuperType4
#[test]
fn test_record_type_least_super_type4() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.record_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let least_supertype = _expr1;
    TypeSubject::assert_type(f.base.object_type).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        least_supertype,
    );
}

// port: JSTypeTest#testRecordTypeGreatestSubType1
#[test]
fn test_record_type_greatest_sub_type1() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("d", f.base.number_type, None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let sub_record_type = _expr2;
    let _expr3 = f
        .record_type
        .get_greatest_subtype(&mut f.base.reg, &f.base.ast, sub_record_type);
    let subtype = _expr3;
    let mut _expr4 = RecordTypeBuilder::new();
    builder = _expr4;
    builder.add_property("d", f.base.number_type, None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    builder.add_property("a", f.base.number_type, None);
    builder.add_property("b", f.base.string_type, None);
    let _expr5 = builder.build(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, subtype);
}

// port: JSTypeTest#testRecordTypeGreatestSubType2
#[test]
fn test_record_type_greatest_sub_type2() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let sub_record_type = _expr2;
    let _expr3 = f
        .record_type
        .get_greatest_subtype(&mut f.base.reg, &f.base.ast, sub_record_type);
    let subtype = _expr3;
    let mut _expr4 = RecordTypeBuilder::new();
    builder = _expr4;
    builder.add_property("a", f.base.number_type, None);
    builder.add_property("b", f.base.string_type, None);
    let _expr5 = builder.build(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, subtype);
}

// port: JSTypeTest#testRecordTypeGreatestSubType3
#[test]
fn test_record_type_greatest_sub_type3() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("a", f.base.number_type, None);
    builder.add_property("b", f.base.string_type, None);
    builder.add_property("c", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let sub_record_type = _expr2;
    let _expr3 = f
        .record_type
        .get_greatest_subtype(&mut f.base.reg, &f.base.ast, sub_record_type);
    let subtype = _expr3;
    let mut _expr4 = RecordTypeBuilder::new();
    builder = _expr4;
    builder.add_property("a", f.base.number_type, None);
    builder.add_property("b", f.base.string_type, None);
    builder.add_property("c", f.base.string_type, None);
    let _expr5 = builder.build(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, subtype);
}

// port: JSTypeTest#testRecordTypeGreatestSubType4
#[test]
fn test_record_type_greatest_sub_type4() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("a", f.base.string_type, None);
    builder.add_property("b", f.base.string_type, None);
    builder.add_property("c", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let sub_record_type = _expr2;
    let _expr3 = f
        .record_type
        .get_greatest_subtype(&mut f.base.reg, &f.base.ast, sub_record_type);
    let subtype = _expr3;
    TypeSubject::assert_type(f.base.no_type).is_equal_to(&mut f.base.reg, &f.base.ast, subtype);
}

// port: JSTypeTest#testRecordTypeGreatestSubType5
#[test]
fn test_record_type_greatest_sub_type5() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("a", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type = _expr2;
    let _expr3 =
        record_type.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    TypeSubject::assert_type(_expr3).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr4 = f.base.function_type.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "a",
        f.base.string_type,
        None,
    );
    let _expr5 =
        record_type.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    TypeSubject::assert_type(_expr5).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_type,
    );
    let _expr6 =
        f.base
            .function_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_type);
    TypeSubject::assert_type(_expr6).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_type,
    );
}

// port: JSTypeTest#testRecordTypeGreatestSubType6
#[test]
fn test_record_type_greatest_sub_type6() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("x", f.base.unknown_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type = _expr2;
    let _expr3 =
        record_type.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    TypeSubject::assert_type(_expr3).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr4 = f.base.function_type.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "x",
        f.base.string_type,
        None,
    );
    let _expr5 =
        record_type.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    TypeSubject::assert_type(_expr5).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_type,
    );
    let _expr6 =
        f.base
            .function_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_type);
    TypeSubject::assert_type(_expr6).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_type,
    );
}

// port: JSTypeTest#testRecordTypeGreatestSubType7
#[test]
fn test_record_type_greatest_sub_type7() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("x", f.base.number_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type = _expr2;
    let _expr3 = f.base.function_type.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "x",
        f.base.string_type,
        None,
    );
    let _expr4 =
        record_type.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    TypeSubject::assert_type(_expr4).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
}

// port: JSTypeTest#testRecordTypeGreatestSubType8
#[test]
fn test_record_type_greatest_sub_type8() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("xyz", f.base.unknown_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type = _expr2;
    let _expr3 =
        record_type.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    TypeSubject::assert_type(_expr3).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr4 = f.goog_bar.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "xyz",
        f.base.string_type,
        None,
    );
    let _expr5 =
        record_type.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, f.goog_bar);
    let _expr6 =
        f.base
            .function_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_type);
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.goog_bar);
    let _expr7 = f.goog_bar.get_instance_type(&f.base.reg);
    let goog_bar_inst = _expr7;
    let _expr8 =
        record_type.get_greatest_subtype(&mut f.base.reg, &f.base.ast, goog_bar_inst.unwrap());
    TypeSubject::assert_type(_expr8).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr9 =
        goog_bar_inst
            .unwrap()
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_type);
    TypeSubject::assert_type(_expr9).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
}

// port: JSTypeTest#testRecordTypeGreatestSubType9
#[test]
fn test_record_type_greatest_sub_type9() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    let _expr2 = f.goog_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    builder.add_property("d", _expr2, None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr3 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type1 = _expr3;
    let mut _expr4 = RecordTypeBuilder::new();
    builder = _expr4;
    let _expr5 = f.goog_bar.get_instance_type(&f.base.reg);
    builder.add_property("d", _expr5.unwrap(), None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr6 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type2 = _expr6;
    let _expr7 = record_type1.get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_type2);
    let subtype = _expr7;
    let mut _expr8 = RecordTypeBuilder::new();
    builder = _expr8;
    let _expr9 = f.goog_bar.get_instance_type(&f.base.reg);
    builder.add_property("d", _expr9.unwrap(), None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr10 = builder.build(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr10).is_equal_to(&mut f.base.reg, &f.base.ast, subtype);
}

// port: JSTypeTest#testRecordTypeGreatestSubType10
#[test]
fn test_record_type_greatest_sub_type10() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    let _expr2 = f.goog_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    builder.add_property("d", _expr2, None);
    builder.add_property("e", f.base.string_type, None);
    let _expr3 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type1 = _expr3;
    let mut _expr4 = RecordTypeBuilder::new();
    builder = _expr4;
    let _expr5 = f.goog_bar.get_instance_type(&f.base.reg);
    builder.add_property("d", _expr5.unwrap(), None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr6 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type2 = _expr6;
    let _expr7 = record_type2.get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_type1);
    let subtype = _expr7;
    let mut _expr8 = RecordTypeBuilder::new();
    builder = _expr8;
    let _expr9 = f.goog_bar.get_instance_type(&f.base.reg);
    builder.add_property("d", _expr9.unwrap(), None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr10 = builder.build(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr10).is_equal_to(&mut f.base.reg, &f.base.ast, subtype);
}

// port: JSTypeTest#testRecordTypeGreatestSubType11
#[test]
fn test_record_type_greatest_sub_type11() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    let _expr2 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    builder.add_property("d", _expr2, None);
    builder.add_property("e", f.base.string_type, None);
    let _expr3 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type1 = _expr3;
    let mut _expr4 = RecordTypeBuilder::new();
    builder = _expr4;
    let _expr5 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.boolean_type]);
    builder.add_property("d", _expr5, None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr6 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type2 = _expr6;
    let _expr7 = record_type2.get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_type1);
    let subtype = _expr7;
    let mut _expr8 = RecordTypeBuilder::new();
    builder = _expr8;
    builder.add_property("d", f.base.number_type, None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr9 = builder.build(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr9).is_equal_to(&mut f.base.reg, &f.base.ast, subtype);
}

// port: JSTypeTest#testRecordTypeGreatestSubType12
#[test]
fn test_record_type_greatest_sub_type12() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    let _expr2 = f.goog_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    builder.add_property("d", _expr2, None);
    builder.add_property("e", f.base.string_type, None);
    let _expr3 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type1 = _expr3;
    let _expr6 = f.with_open_registry(|f| {
        let _expr4 = f.base.reg.create_parameters(&[f.goog_bar]);

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("barArg".into()),
            None,
            Some(_expr4),
            None,
            None,
            false,
        )
    });
    let goog_bar_arg_constructor = _expr6;
    let mut _expr7 = RecordTypeBuilder::new();
    builder = _expr7;
    builder.add_property("d", goog_bar_arg_constructor, None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr8 = builder.build(&mut f.base.reg, &f.base.ast);
    let record_type2 = _expr8;
    let _expr9 = record_type2.get_greatest_subtype(&mut f.base.reg, &f.base.ast, record_type1);
    let subtype = _expr9;
    let mut _expr10 = RecordTypeBuilder::new();
    builder = _expr10;
    let _expr11 = f
        .base
        .reg
        .get_native_object_type(JSTypeNative::NO_OBJECT_TYPE);
    builder.add_property("d", _expr11, None);
    builder.add_property("e", f.base.string_type, None);
    builder.add_property("f", f.base.string_type, None);
    let _expr12 = builder.build(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr12).is_equal_to(&mut f.base.reg, &f.base.ast, subtype);
}

// port: JSTypeTest#testApplyOfDateMethod
#[test]
fn test_apply_of_date_method() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .date_method
        .get_property_type(&mut f.base.reg, &f.base.ast, "apply");
    let apply_type = _expr1;
    let _expr2 = apply_type.is_function_type(&f.base.reg);
    let _expr3 = closure_rhino::jscomp_base::guava_format("apply should be a function", &[]);
    assert!(_expr2, "{}", _expr3);
    let apply_fn = apply_type;
    let _expr4 = apply_fn.get_return_type(&f.base.reg);
    assert_type_equal_with_message(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
        _expr4,
        "apply should have the same return type as its function",
    );
    let _expr5 = apply_fn.get_parameters(&f.base.reg);
    let params = _expr5;
    let _expr6 = closure_rhino::jscomp_base::guava_format("apply takes two args", &[]);
    assert_eq!(params.len(), 2, "{}", _expr6);
    let _expr7 = f
        .base
        .reg
        .create_optional_nullable_type(&f.base.ast, f.base.date_type);
    let _expr8 = params[0];
    let _expr9 = _expr8.get_jstype();
    assert_type_equal_with_message(
        &mut f.base.reg,
        &f.base.ast,
        _expr7,
        _expr9,
        "apply's first arg is the @this type",
    );
    let _expr10 = f
        .base
        .reg
        .create_optional_nullable_type(&f.base.ast, f.base.object_type);
    let _expr11 = params[1];
    let _expr12 = _expr11.get_jstype();
    assert_type_equal_with_message(
        &mut f.base.reg,
        &f.base.ast,
        _expr10,
        _expr12,
        "apply's second arg is an Array",
    );
    let _expr13 = params[0];
    let _expr14 = _expr13.is_optional();
    let _expr15 = closure_rhino::jscomp_base::guava_format("apply's args must be optional", &[]);
    assert!(_expr14, "{}", _expr15);
    let _expr16 = params[1];
    let _expr17 = _expr16.is_optional();
    let _expr18 = closure_rhino::jscomp_base::guava_format("apply's args must be optional", &[]);
    assert!(_expr17, "{}", _expr18);
}

// port: JSTypeTest#testCallOfDateMethod
#[test]
fn test_call_of_date_method() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .date_method
        .get_property_type(&mut f.base.reg, &f.base.ast, "call");
    let call_type = _expr1;
    let _expr2 = call_type.is_function_type(&f.base.reg);
    let _expr3 = closure_rhino::jscomp_base::guava_format("call should be a function", &[]);
    assert!(_expr2, "{}", _expr3);
    let call_fn = call_type;
    let _expr4 = call_fn.get_return_type(&f.base.reg);
    assert_type_equal_with_message(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
        _expr4,
        "call should have the same return type as its function",
    );
    let _expr5 = call_fn.get_parameters(&f.base.reg);
    let params = _expr5;
    let _expr6 =
        closure_rhino::jscomp_base::guava_format("call takes one argument in this case", &[]);
    assert_eq!(params.len(), 1, "{}", _expr6);
    let _expr7 = f
        .base
        .reg
        .create_optional_nullable_type(&f.base.ast, f.base.date_type);
    let _expr8 = params[0];
    let _expr9 = _expr8.get_jstype();
    assert_type_equal_with_message(
        &mut f.base.reg,
        &f.base.ast,
        _expr7,
        _expr9,
        "call's first arg is the @this type",
    );
    let _expr10 = params[0];
    let _expr11 = _expr10.is_optional();
    let _expr12 = closure_rhino::jscomp_base::guava_format("call's args must be optional", &[]);
    assert!(_expr11, "{}", _expr12);
}

// port: JSTypeTest#testFunctionTypeRepresentation
#[test]
fn test_function_type_representation() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.boolean_type,
        &[f.base.number_type, f.base.string_type],
    );
    let _expr2 = _expr1.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr2, "function(number, string): boolean");
    let _expr3 = f
        .base
        .array_function_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr3, "function(new:Array, ...*): Array");
    let _expr4 = f
        .base
        .boolean_object_function_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr4, "function(new:Boolean, *=): boolean");
    let _expr5 = f
        .base
        .number_object_function_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr5, "function(new:Number, *=): number");
    let _expr6 = f
        .base
        .string_object_function_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr6, "function(new:String, *=): string");
    let _expr7 = f.base.reg.create_function_type_with_var_args(
        &f.base.ast,
        f.base.boolean_type,
        &[f.base.number_type],
    );
    let _expr8 = _expr7.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr8, "function(...number): boolean");
    let _expr9 = f.base.reg.create_function_type_with_var_args(
        &f.base.ast,
        f.base.boolean_type,
        &[f.base.number_type, f.base.string_type],
    );
    let _expr10 = _expr9.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr10, "function(number, ...string): boolean");
    let _expr11 = FunctionTypeBuilder::new();
    let _expr12 = f.base.reg.create_parameters(&[f.base.number_type]);
    let _expr13 = _expr11.with_parameters(_expr12);
    let _expr14 = _expr13.with_return_type(f.base.number_string_boolean);
    let _expr15 = _expr14.with_type_of_this(f.base.date_type);
    let _expr16 = _expr15.build(&mut f.base.reg, &f.base.ast);
    let _expr17 = _expr16.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(
        _expr17,
        "function(this:Date, number): (boolean|number|string)"
    );
}

// port: JSTypeTest#testFunctionTypeRelationships
#[test]
fn test_function_type_relationships() {
    let mut f = Fixture::set_up();
    let _expr1 = FunctionTypeBuilder::new();
    let _expr2 = f.base.reg.create_parameters(&[]);
    let _expr3 = _expr1.with_parameters(_expr2);
    let _expr4 = _expr3.with_type_of_this(f.base.date_type);
    let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
    let date_method_empty = _expr5;
    let _expr6 = FunctionTypeBuilder::new();
    let _expr7 = f
        .base
        .reg
        .create_optional_parameters(&f.base.ast, &[f.base.number_type]);
    let _expr8 = _expr6.with_parameters(_expr7);
    let _expr9 = _expr8.with_type_of_this(f.base.date_type);
    let _expr10 = _expr9.build(&mut f.base.reg, &f.base.ast);
    let date_method_with_param = _expr10;
    let _expr11 = FunctionTypeBuilder::new();
    let _expr12 = _expr11.with_return_type(f.base.number_type);
    let _expr13 = _expr12.with_type_of_this(f.base.date_type);
    let _expr14 = _expr13.build(&mut f.base.reg, &f.base.ast);
    let date_method_with_return = _expr14;
    let _expr15 = FunctionTypeBuilder::new();
    let _expr16 = f.base.reg.create_parameters(&[]);
    let _expr17 = _expr15.with_parameters(_expr16);
    let _expr18 = _expr17.with_type_of_this(f.base.string_object_type);
    let _expr19 = _expr18.build(&mut f.base.reg, &f.base.ast);
    let string_method_empty = _expr19;
    let _expr20 = FunctionTypeBuilder::new();
    let _expr21 = f
        .base
        .reg
        .create_optional_parameters(&f.base.ast, &[f.base.number_type]);
    let _expr22 = _expr20.with_parameters(_expr21);
    let _expr23 = _expr22.with_type_of_this(f.base.string_object_type);
    let _expr24 = _expr23.build(&mut f.base.reg, &f.base.ast);
    let string_method_with_param = _expr24;
    let _expr25 = FunctionTypeBuilder::new();
    let _expr26 = _expr25.with_return_type(f.base.number_type);
    let _expr27 = _expr26.with_type_of_this(f.base.string_object_type);
    let _expr28 = _expr27.build(&mut f.base.reg, &f.base.ast);
    let string_method_with_return = _expr28;
    let _expr29 = string_method_empty.is_subtype(&mut f.base.reg, &f.base.ast, date_method_empty);
    assert!(!_expr29);
    let _expr30 = vec![
        date_method_empty,
        date_method_with_param,
        date_method_with_return,
        string_method_empty,
        string_method_with_param,
        string_method_with_return,
    ];
    let all_functions = _expr30;
    let _expr31 = all_functions.len();
    for i in 0.._expr31 {
        let _expr32 = all_functions.len();
        for j in 0.._expr32 {
            let _expr33 = all_functions[i];
            let type_a = _expr33;
            let _expr34 = all_functions[j];
            let type_b = _expr34;
            let _expr35 = type_a.equals(&mut f.base.reg, &f.base.ast, type_b);
            let _expr36 = i == j;
            let _expr37 = closure_rhino::jscomp_base::guava_format(
                "equals(%s, %s)",
                &[
                    (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                ],
            );
            assert_eq!(_expr35, _expr36, "{}", _expr37);
            let _expr38 = type_a.is_subtype(&mut f.base.reg, &f.base.ast, type_b);
            let _expr39 = type_a.get_type_of_this(&f.base.reg);
            let _expr40 = type_b.get_type_of_this(&f.base.reg);
            let _expr41 = _expr39
                .unwrap()
                .equals(&mut f.base.reg, &f.base.ast, _expr40);
            let _expr42 = closure_rhino::jscomp_base::guava_format(
                "isSubtype(%s, %s)",
                &[
                    (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                ],
            );
            assert_eq!(_expr38, _expr41, "{}", _expr42);
            let _expr43 = i == j;
            if _expr43 {
                let _expr44 = type_a.get_least_supertype(&mut f.base.reg, &f.base.ast, type_b);
                TypeSubject::assert_type(_expr44).is_equal_to(&mut f.base.reg, &f.base.ast, type_a);
                let _expr45 = type_a.get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_b);
                TypeSubject::assert_type(_expr45).is_equal_to(&mut f.base.reg, &f.base.ast, type_a);
            } else {
                let _expr46 = closure_rhino::jscomp_base::guava_format(
                    "sup(%s, %s)",
                    &[
                        (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                let _expr47 = type_a.get_least_supertype(&mut f.base.reg, &f.base.ast, type_b);
                assert_type_equal_with_message(
                    &mut f.base.reg,
                    &f.base.ast,
                    f.base.function_type,
                    _expr47,
                    &(_expr46).to_string(),
                );
                let _expr48 = closure_rhino::jscomp_base::guava_format(
                    "inf(%s, %s)",
                    &[
                        (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                let _expr49 = type_a.get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_b);
                assert_type_equal_with_message(
                    &mut f.base.reg,
                    &f.base.ast,
                    f.base.least_function_type,
                    _expr49,
                    &(_expr48).to_string(),
                );
            }
        }
    }
}

// port: JSTypeTest#testProxiedFunctionTypeRelationships
#[test]
fn test_proxied_function_type_relationships() {
    let mut f = Fixture::set_up();
    let _expr1 = FunctionTypeBuilder::new();
    let _expr2 = f.base.reg.create_parameters(&[]);
    let _expr3 = _expr1.with_parameters(_expr2);
    let _expr4 = _expr3.with_type_of_this(f.base.date_type);
    let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
    let _expr6 = _expr5.to_maybe_function_type(&f.base.reg);
    let date_method_empty = _expr6;
    let _expr7 = FunctionTypeBuilder::new();
    let _expr8 = f.base.reg.create_parameters(&[f.base.number_type]);
    let _expr9 = _expr7.with_parameters(_expr8);
    let _expr10 = _expr9.with_type_of_this(f.base.date_type);
    let _expr11 = _expr10.build(&mut f.base.reg, &f.base.ast);
    let _expr12 = _expr11.to_maybe_function_type(&f.base.reg);
    let date_method_with_param = _expr12;
    let _expr13 = closure_jstype::proxy_object_type::create(
        &mut f.base.reg,
        &f.base.ast,
        date_method_empty.unwrap(),
        None,
    );
    let proxy_date_method_empty = _expr13;
    let _expr14 = closure_jstype::proxy_object_type::create(
        &mut f.base.reg,
        &f.base.ast,
        date_method_with_param.unwrap(),
        None,
    );
    let proxy_date_method_with_param = _expr14;
    let _expr15 = proxy_date_method_empty.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        proxy_date_method_with_param,
    );
    TypeSubject::assert_type(_expr15).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_type,
    );
    let _expr16 = proxy_date_method_empty.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        proxy_date_method_with_param,
    );
    TypeSubject::assert_type(_expr16).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.least_function_type,
    );
}

// port: JSTypeTest#testFunctionSubTypeRelationships
#[test]
fn test_function_sub_type_relationships() {
    let mut f = Fixture::set_up();
    let _expr1 = FunctionTypeBuilder::new();
    let _expr2 = _expr1.with_type_of_this(f.goog_bar);
    let _expr3 = _expr2.build(&mut f.base.reg, &f.base.ast);
    let goog_bar_method = _expr3;
    let _expr4 = FunctionTypeBuilder::new();
    let _expr5 = f.base.reg.create_parameters(&[f.goog_bar]);
    let _expr6 = _expr4.with_parameters(_expr5);
    let _expr7 = _expr6.build(&mut f.base.reg, &f.base.ast);
    let goog_bar_param_fn = _expr7;
    let _expr8 = FunctionTypeBuilder::new();
    let _expr9 = f.base.reg.create_parameters(&[]);
    let _expr10 = _expr8.with_parameters(_expr9);
    let _expr11 = _expr10.with_return_type(f.goog_bar);
    let _expr12 = _expr11.build(&mut f.base.reg, &f.base.ast);
    let goog_bar_return_fn = _expr12;
    let _expr13 = FunctionTypeBuilder::new();
    let _expr14 = _expr13.with_type_of_this(f.goog_sub_bar);
    let _expr15 = _expr14.build(&mut f.base.reg, &f.base.ast);
    let goog_sub_bar_method = _expr15;
    let _expr16 = FunctionTypeBuilder::new();
    let _expr17 = f.base.reg.create_parameters(&[f.goog_sub_bar]);
    let _expr18 = _expr16.with_parameters(_expr17);
    let _expr19 = _expr18.build(&mut f.base.reg, &f.base.ast);
    let goog_sub_bar_param_fn = _expr19;
    let _expr20 = FunctionTypeBuilder::new();
    let _expr21 = _expr20.with_return_type(f.goog_sub_bar);
    let _expr22 = _expr21.build(&mut f.base.reg, &f.base.ast);
    let goog_sub_bar_return_fn = _expr22;
    let _expr23 = goog_bar_method.is_subtype(&mut f.base.reg, &f.base.ast, goog_sub_bar_method);
    assert!(_expr23);
    let _expr24 =
        goog_bar_return_fn.is_subtype(&mut f.base.reg, &f.base.ast, goog_sub_bar_return_fn);
    assert!(_expr24);
    let _expr25 = vec![
        goog_bar_method,
        goog_bar_param_fn,
        goog_bar_return_fn,
        goog_sub_bar_method,
        goog_sub_bar_param_fn,
        goog_sub_bar_return_fn,
    ];
    let all_functions = _expr25;
    let _expr26 = all_functions.len();
    for i in 0.._expr26 {
        let _expr27 = all_functions.len();
        for j in 0.._expr27 {
            let _expr28 = all_functions[i];
            let type_a = _expr28;
            let _expr29 = all_functions[j];
            let type_b = _expr29;
            let _expr30 = type_a.equals(&mut f.base.reg, &f.base.ast, type_b);
            let _expr31 = i == j;
            let _expr32 = closure_rhino::jscomp_base::guava_format(
                "equals(%s, %s)",
                &[
                    (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                ],
            );
            assert_eq!(_expr30, _expr31, "{}", _expr32);
            let _expr33 = i == j;
            if _expr33 {
                let _expr34 = type_a.get_least_supertype(&mut f.base.reg, &f.base.ast, type_b);
                TypeSubject::assert_type(_expr34).is_equal_to(&mut f.base.reg, &f.base.ast, type_a);
                let _expr35 = type_a.get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_b);
                TypeSubject::assert_type(_expr35).is_equal_to(&mut f.base.reg, &f.base.ast, type_a);
            } else {
                let _expr36 = closure_rhino::jscomp_base::guava_format(
                    "sup(%s, %s)",
                    &[
                        (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                let _expr37 = type_a.get_least_supertype(&mut f.base.reg, &f.base.ast, type_b);
                assert_type_equal_with_message(
                    &mut f.base.reg,
                    &f.base.ast,
                    f.base.function_type,
                    _expr37,
                    &(_expr36).to_string(),
                );
                let _expr38 = closure_rhino::jscomp_base::guava_format(
                    "inf(%s, %s)",
                    &[
                        (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                        (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    ],
                );
                let _expr39 = type_a.get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_b);
                assert_type_equal_with_message(
                    &mut f.base.reg,
                    &f.base.ast,
                    f.base.least_function_type,
                    _expr39,
                    &(_expr38).to_string(),
                );
            }
        }
    }
}

// port: JSTypeTest#testFunctionPrototypeAndImplicitPrototype1
#[test]
fn test_function_prototype_and_implicit_prototype1() {
    let mut f = Fixture::set_up();
    let _expr2 = f.with_open_registry(|f| {
        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("Foo".into()),
            None,
            None,
            None,
            None,
            false,
        )
    });
    let constructor = _expr2;
    let _expr3 = constructor.get_instance_type(&f.base.reg);
    let instance = _expr3;
    let _expr4 = constructor.get_property_type(&mut f.base.reg, &f.base.ast, "prototype");
    let prototype = _expr4;
    let _expr5 = prototype.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
        f.base.date_type,
        None,
    );
    let _expr6 = instance
        .unwrap()
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    let _expr7 = f.base.native_properties_count + 1;
    assert_eq!(_expr6, _expr7);
}

// port: JSTypeTest#testFunctionPrototypeAndImplicitPrototype2
#[test]
fn test_function_prototype_and_implicit_prototype2() {
    let mut f = Fixture::set_up();
    let _expr3 = f.with_open_registry(|f| {
        let _expr1 =
            f.base
                .reg
                .create_parameters(&[f.base.all_type, f.base.all_type, f.base.all_type]);

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("Bar".into()),
            None,
            Some(_expr1),
            None,
            None,
            false,
        )
    });
    let constructor = _expr3;
    let _expr4 = constructor.get_instance_type(&f.base.reg);
    let instance = _expr4;
    let _expr5 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let prototype = _expr5;
    let _expr6 = prototype.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "foo",
        f.base.date_type,
        None,
    );
    let _expr7 = constructor.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "prototype",
        prototype,
        None,
    );
    let _expr8 = instance
        .unwrap()
        .get_properties_count(&mut f.base.reg, &f.base.ast);
    let _expr9 = f.base.native_properties_count + 1;
    assert_eq!(_expr8, _expr9);
}

// port: JSTypeTest#testJSDocOnPrototypeProperty
#[test]
fn test_js_doc_on_prototype_property() {
    let mut f = Fixture::set_up();
    let mut _expr1 = JSDocInfo::builder();
    let _expr2 = _expr1.build();
    f.subclass_ctor.set_property_jsdoc_info(
        &mut f.base.reg,
        &f.base.ast,
        "prototype",
        _expr2.clone(),
    );
    let _expr3 =
        f.subclass_ctor
            .get_own_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "prototype");
    assert!(_expr3.is_none());
}

// port: JSTypeTest#testIsVoidable
#[test]
fn test_is_voidable() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.void_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr1);
    let _expr2 = f.base.null_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr2);
    let _expr3 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.void_type]);
    let _expr4 = _expr3.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr4);
}

// port: JSTypeTest#testIsBigIntOrNumber
#[test]
fn test_is_big_int_or_number() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .bigint_type
        .is_big_int_or_number(&mut f.base.reg, &f.base.ast);
    assert!(_expr1);
    let _expr2 = f
        .base
        .bigint_object_type
        .is_big_int_or_number(&mut f.base.reg, &f.base.ast);
    assert!(_expr2);
    let _expr3 = f
        .base
        .bigint_number
        .is_big_int_or_number(&mut f.base.reg, &f.base.ast);
    assert!(_expr3);
    let _expr4 = f
        .base
        .bigint_number_object
        .is_big_int_or_number(&mut f.base.reg, &f.base.ast);
    assert!(_expr4);
    let _expr5 = f
        .base
        .bigint_number_string
        .is_big_int_or_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr5);
    let _expr6 = f
        .base
        .create_union_type(&[f.base.bigint_type, f.base.bigint_object_type]);
    let _expr7 = _expr6.is_big_int_or_number(&mut f.base.reg, &f.base.ast);
    assert!(!_expr7);
}

// port: JSTypeTest#testVoidType
#[test]
fn test_void_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .void_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr1);
    let _expr2 =
        f.base
            .void_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr2);
    let _expr3 = f
        .base
        .void_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr3);
    let _expr4 = f.base.void_type.autoboxes_to(&f.base.reg);
    assert!(_expr4.is_none());
    f.base
        .assert_can_test_for_equality_with(f.base.void_type, f.base.all_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.void_type, f.base.regexp_type);
    let _expr5 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr5);
    let _expr6 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr6);
    let _expr7 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr7);
    let _expr8 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr8);
    let _expr9 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr9);
    let _expr10 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr10);
    let _expr11 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr11);
    let _expr12 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr12);
    let _expr13 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr13);
    let _expr14 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr14);
    let _expr15 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(!_expr15);
    let _expr16 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr16);
    let _expr17 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr17);
    let _expr18 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr18);
    let _expr19 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr19);
    let _expr20 = f.base.void_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(_expr20);
    let _expr21 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.void_type]);
    let _expr22 =
        f.base
            .void_type
            .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, _expr21);
    assert!(_expr22);
    let _expr23 = f
        .base
        .void_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr23);
    let _expr24 = f
        .base
        .void_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = f
        .base
        .void_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr25);
    let _expr26 = f
        .base
        .void_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr26);
    let _expr27 = f
        .base
        .void_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr27);
    let _expr28 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.void_type);
}

// port: JSTypeTest#testBooleanValueType
#[test]
fn test_boolean_value_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.boolean_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.base.boolean_type.is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.base.boolean_type.is_boolean_value_type(&f.base.reg);
    assert!(_expr3);
    let _expr4 = f.base.boolean_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.boolean_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.boolean_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.boolean_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.base.boolean_type.is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.boolean_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.base.boolean_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f.base.boolean_type.is_regexp_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.base.boolean_type.is_string_object_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.base.boolean_type.is_string_value_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.base.boolean_type.is_enum_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.boolean_type.is_union_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.boolean_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.base.boolean_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr17);
    let _expr18 = f.base.boolean_type.is_all_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.base.boolean_type.is_void_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.base.boolean_type.is_constructor(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.boolean_type.is_instance_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.boolean_type.autoboxes_to(&f.base.reg);
    TypeSubject::assert_type(_expr22).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    let _expr23 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr23);
    let _expr24 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr24);
    let _expr25 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr25);
    let _expr26 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr26);
    let _expr27 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr27);
    let _expr28 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr28);
    let _expr29 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr29);
    let _expr30 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr30);
    let _expr31 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr32);
    let _expr33 = f
        .base
        .boolean_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr33);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_type, f.base.number_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.boolean_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.boolean_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_type, f.base.regexp_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_type, f.base.unknown_type);
    let _expr34 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr34);
    let _expr35 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr35);
    let _expr36 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr36);
    let _expr37 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(_expr37);
    let _expr38 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(!_expr38);
    let _expr39 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr39);
    let _expr40 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr40);
    let _expr41 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr41);
    let _expr42 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr42);
    let _expr43 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr43);
    let _expr44 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(!_expr44);
    let _expr45 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr45);
    let _expr46 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr46);
    let _expr47 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr47);
    let _expr48 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr48);
    let _expr49 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr49);
    let _expr50 = f.base.boolean_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    assert!(_expr50);
    let _expr51 = f
        .base
        .boolean_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr51);
    let _expr52 = f
        .base
        .boolean_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr52);
    let _expr53 = f
        .base
        .boolean_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr53);
    let _expr54 = f
        .base
        .boolean_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr54);
    let _expr55 = f
        .base
        .boolean_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr55);
    let _expr56 = f
        .base
        .boolean_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr56);
    let _expr57 = f
        .base
        .boolean_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr57);
    let _expr58 = f.base.boolean_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr58, "boolean");
    let _expr59 = f.base.boolean_type.has_display_name(&f.base.reg);
    assert!(_expr59);
    let _expr60 = f.base.boolean_type.get_display_name(&f.base.reg);
    assert_eq!(_expr60, Some("boolean".into()));
    let _expr61 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
}

// port: JSTypeTest#testBooleanObjectType
#[test]
fn test_boolean_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.boolean_object_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f
        .base
        .boolean_object_type
        .is_boolean_object_type(&f.base.reg);
    assert!(_expr2);
    let _expr3 = f
        .base
        .boolean_object_type
        .is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.base.boolean_object_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.base.boolean_object_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = f.base.boolean_object_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.base.boolean_object_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f
        .base
        .boolean_object_type
        .is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.base.boolean_object_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f
        .base
        .boolean_object_type
        .is_function_prototype_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f
        .base
        .boolean_object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr12 = _expr11.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr12);
    let _expr13 = f.base.boolean_object_type.is_regexp_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f
        .base
        .boolean_object_type
        .is_string_object_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.base.boolean_object_type.is_string_value_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.base.boolean_object_type.is_enum_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = f.base.boolean_object_type.is_union_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f
        .base
        .boolean_object_type
        .is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr18);
    let _expr19 = f
        .base
        .boolean_object_type
        .is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr19);
    let _expr20 = f.base.boolean_object_type.is_all_type(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.base.boolean_object_type.is_void_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f.base.boolean_object_type.is_constructor(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = f.base.boolean_object_type.is_instance_type(&f.base.reg);
    assert!(_expr23);
    let _expr24 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr24);
    let _expr25 = f.base.boolean_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr25);
    let _expr26 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr26);
    let _expr27 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr27);
    let _expr28 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr28);
    let _expr29 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr29);
    let _expr30 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr30);
    let _expr31 = f.base.boolean_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.unresolved_named_type,
    );
    assert!(_expr31);
    let _expr32 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr32);
    let _expr33 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr33);
    let _expr34 = f
        .base
        .boolean_object_type
        .can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr34);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_object_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_object_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_object_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_object_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.base.boolean_object_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_object_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_object_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.base.boolean_object_type, f.base.regexp_type);
    let _expr35 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(_expr35);
    let _expr36 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr36);
    let _expr37 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr37);
    let _expr38 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.boolean_object_type,
        );
    assert!(_expr39);
    let _expr40 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr41);
    let _expr42 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr42);
    let _expr43 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr43);
    let _expr44 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.number_object_type,
        );
    assert!(!_expr44);
    let _expr45 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr45);
    let _expr46 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr46);
    let _expr47 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr47);
    let _expr48 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(
            &mut f.base.reg,
            &f.base.ast,
            f.base.string_object_type,
        );
    assert!(!_expr48);
    let _expr49 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr49);
    let _expr50 = f
        .base
        .boolean_object_type
        .can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr50);
    let _expr51 = f
        .base
        .boolean_object_type
        .is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr51);
    let _expr52 = f
        .base
        .boolean_object_type
        .is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr52);
    let _expr53 = f
        .base
        .boolean_object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr53);
    let _expr54 = f
        .base
        .boolean_object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr54);
    let _expr55 = f
        .base
        .boolean_object_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr55);
    let _expr56 = f
        .base
        .boolean_object_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr56);
    let _expr57 = f
        .base
        .boolean_object_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr57);
    let _expr58 = f
        .base
        .boolean_object_type
        .to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr58, "Boolean");
    let _expr59 = f.base.boolean_object_type.has_display_name(&f.base.reg);
    assert!(_expr59);
    let _expr60 = f.base.boolean_object_type.get_display_name(&f.base.reg);
    assert_eq!(_expr60, Some("Boolean".into()));
    let _expr61 = f
        .base
        .boolean_object_type
        .is_native_object_type(&f.base.reg);
    assert!(_expr61);
    let _expr62 =
        Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
}

// port: JSTypeTest#testEnumType
#[test]
fn test_enum_type() {
    let mut f = Fixture::set_up();
    let _expr1 = EnumTypeBuilder::new();
    let _expr2 = _expr1.set_name("Enum");
    let _expr3 = _expr2.set_element_type(f.base.number_type);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let enum_type = _expr4;
    let _expr5 = enum_type.is_array_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = enum_type.is_boolean_object_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = enum_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = enum_type.is_date_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = enum_type.is_enum_element_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = enum_type.is_named_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = enum_type.is_null_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = enum_type.is_number_object_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = enum_type.is_number_value_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = enum_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = enum_type.is_regexp_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = enum_type.is_string_object_type(&f.base.reg);
    assert!(!_expr16);
    let _expr17 = enum_type.is_string_value_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = enum_type.is_enum_type(&f.base.reg);
    assert!(_expr18);
    let _expr19 = enum_type.is_union_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = enum_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr20);
    let _expr21 = enum_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr21);
    let _expr22 = enum_type.is_all_type(&f.base.reg);
    assert!(!_expr22);
    let _expr23 = enum_type.is_void_type(&f.base.reg);
    assert!(!_expr23);
    let _expr24 = enum_type.is_constructor(&f.base.reg);
    assert!(!_expr24);
    let _expr25 = enum_type.is_instance_type(&f.base.reg);
    assert!(!_expr25);
    let _expr26 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr26);
    let _expr27 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr27);
    let _expr28 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr28);
    let _expr29 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr29);
    let _expr30 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr30);
    let _expr31 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr31);
    let _expr32 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr32);
    let _expr33 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr33);
    let _expr34 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr34);
    let _expr35 = enum_type.is_subtype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr35);
    let _expr36 = enum_type.can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr36);
    f.base
        .assert_can_test_for_equality_with(enum_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(enum_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(enum_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(enum_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(enum_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(enum_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(enum_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(enum_type, f.base.regexp_type);
    let _expr37 =
        enum_type.can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(_expr37);
    let _expr38 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(_expr38);
    let _expr39 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr39);
    let _expr40 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr40);
    let _expr41 =
        enum_type.can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, enum_type);
    assert!(_expr41);
    let _expr42 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr42);
    let _expr43 =
        enum_type.can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr43);
    let _expr44 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr44);
    let _expr45 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(!_expr45);
    let _expr46 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr46);
    let _expr47 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(_expr47);
    let _expr48 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr48);
    let _expr49 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr49);
    let _expr50 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr50);
    let _expr51 =
        enum_type.can_test_for_shallow_equality_with(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr51);
    let _expr52 = enum_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr52);
    let _expr53 = enum_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr53);
    let _expr54 = enum_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr54);
    let _expr55 = enum_type.matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr55);
    let _expr56 = enum_type.matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr56);
    let _expr57 = enum_type.matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr57);
    let _expr58 = enum_type.matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr58);
    let _expr59 = enum_type.matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr59);
    let _expr60 = enum_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr60, "enum{Enum}");
    let _expr61 = enum_type.has_display_name(&f.base.reg);
    assert!(_expr61);
    let _expr62 = enum_type.get_display_name(&f.base.reg);
    assert_eq!(_expr62, Some("Enum".into()));
    let _expr63 = EnumTypeBuilder::new();
    let _expr64 = _expr63.set_name("AnotherEnum");
    let _expr65 = _expr64.set_element_type(f.base.number_type);
    let _expr66 = _expr65.build(&mut f.base.reg, &f.base.ast);
    let _expr67 = _expr66.get_display_name(&f.base.reg);
    assert_eq!(_expr67, Some("AnotherEnum".into()));
    let _expr68 = EnumTypeBuilder::new();
    let _expr69 = _expr68.set_element_type(f.base.number_type);
    let _expr70 = _expr69.build(&mut f.base.reg, &f.base.ast);
    let _expr71 = _expr70.has_display_name(&f.base.reg);
    assert!(!_expr71);
    let _expr72 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, enum_type);
}

// port: JSTypeTest#testEnumElementType
#[test]
fn test_enum_element_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.elements_type.is_array_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f.elements_type.is_boolean_object_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.elements_type.is_boolean_value_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.elements_type.is_date_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = f.elements_type.is_enum_element_type(&f.base.reg);
    assert!(_expr5);
    let _expr6 = f.elements_type.is_named_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = f.elements_type.is_null_type(&f.base.reg);
    assert!(!_expr7);
    let _expr8 = f.elements_type.is_number_object_type(&f.base.reg);
    assert!(!_expr8);
    let _expr9 = f.elements_type.is_number_value_type(&f.base.reg);
    assert!(!_expr9);
    let _expr10 = f.elements_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr10);
    let _expr11 = f.elements_type.is_regexp_type(&f.base.reg);
    assert!(!_expr11);
    let _expr12 = f.elements_type.is_string_object_type(&f.base.reg);
    assert!(!_expr12);
    let _expr13 = f.elements_type.is_string_value_type(&f.base.reg);
    assert!(!_expr13);
    let _expr14 = f.elements_type.is_enum_type(&f.base.reg);
    assert!(!_expr14);
    let _expr15 = f.elements_type.is_union_type(&f.base.reg);
    assert!(!_expr15);
    let _expr16 = f.elements_type.is_struct(&mut f.base.reg, &f.base.ast);
    assert!(!_expr16);
    let _expr17 = f.elements_type.is_dict(&mut f.base.reg, &f.base.ast);
    assert!(!_expr17);
    let _expr18 = f.elements_type.is_all_type(&f.base.reg);
    assert!(!_expr18);
    let _expr19 = f.elements_type.is_void_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = f.elements_type.is_constructor(&f.base.reg);
    assert!(!_expr20);
    let _expr21 = f.elements_type.is_instance_type(&f.base.reg);
    assert!(!_expr21);
    let _expr22 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr22);
    let _expr23 =
        f.elements_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr23);
    let _expr24 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr24);
    let _expr25 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr25);
    let _expr26 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr26);
    let _expr27 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr27);
    let _expr28 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr28);
    let _expr29 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr29);
    let _expr30 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr30);
    let _expr31 = f
        .elements_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr31);
    let _expr32 = f.elements_type.can_be_called(&mut f.base.reg, &f.base.ast);
    assert!(!_expr32);
    f.base
        .assert_can_test_for_equality_with(f.elements_type, f.base.all_type);
    f.base
        .assert_can_test_for_equality_with(f.elements_type, f.base.string_object_type);
    f.base
        .assert_can_test_for_equality_with(f.elements_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(f.elements_type, f.base.number_object_type);
    f.base
        .assert_can_test_for_equality_with(f.elements_type, f.elements_type);
    f.base
        .assert_cannot_test_for_equality_with(f.elements_type, f.function_type);
    f.base
        .assert_cannot_test_for_equality_with(f.elements_type, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.elements_type, f.base.object_type);
    f.base
        .assert_can_test_for_equality_with(f.elements_type, f.base.date_type);
    f.base
        .assert_can_test_for_equality_with(f.elements_type, f.base.regexp_type);
    let _expr33 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_type,
    );
    assert!(_expr33);
    let _expr34 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr34);
    let _expr35 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_type,
    );
    assert!(!_expr35);
    let _expr36 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_type,
    );
    assert!(!_expr36);
    let _expr37 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.elements_type,
    );
    assert!(_expr37);
    let _expr38 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_type,
    );
    assert!(!_expr38);
    let _expr39 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.function_type,
    );
    assert!(!_expr39);
    let _expr40 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.null_type,
    );
    assert!(!_expr40);
    let _expr41 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_type,
    );
    assert!(_expr41);
    let _expr42 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr42);
    let _expr43 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    assert!(!_expr43);
    let _expr44 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    assert!(!_expr44);
    let _expr45 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_type,
    );
    assert!(!_expr45);
    let _expr46 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr46);
    let _expr47 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.all_type,
    );
    assert!(_expr47);
    let _expr48 = f.elements_type.can_test_for_shallow_equality_with(
        &mut f.base.reg,
        &f.base.ast,
        f.base.void_type,
    );
    assert!(!_expr48);
    let _expr49 = f.elements_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr49);
    let _expr50 = f.elements_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr50);
    let _expr51 = f
        .elements_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr51);
    let _expr52 = f
        .elements_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr52);
    let _expr53 = f
        .elements_type
        .matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr53);
    let _expr54 = f
        .elements_type
        .matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr54);
    let _expr55 = f
        .elements_type
        .matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr55);
    let _expr56 = f.elements_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr56, "Enum<number>");
    let _expr57 = f.elements_type.has_display_name(&f.base.reg);
    assert!(_expr57);
    let _expr58 = f.elements_type.get_display_name(&f.base.reg);
    assert_eq!(_expr58, Some("Enum".into()));
    let _expr59 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.elements_type);
}

// port: JSTypeTest#testStringEnumType
#[test]
fn test_string_enum_type() {
    let mut f = Fixture::set_up();
    let _expr1 = EnumTypeBuilder::new();
    let _expr2 = _expr1.set_name("Enum");
    let _expr3 = _expr2.set_element_type(f.base.string_type);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let _expr5 = _expr4.get_elements_type(&f.base.reg);
    let string_enum = _expr5;
    let _expr6 = string_enum.get_property_type(&mut f.base.reg, &f.base.ast, "length");
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    let _expr7 = string_enum.find_property_type(&mut f.base.reg, &f.base.ast, "length");
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr8 = string_enum.has_property(&mut f.base.reg, &f.base.ast, "length");
    assert!(!_expr8);
    let _expr9 = string_enum.autoboxes_to(&f.base.reg);
    TypeSubject::assert_type(_expr9).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    let _expr10 = string_enum.get_constructor(&f.base.reg);
    assert!(_expr10.is_none());
    let _expr11 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, string_enum);
}

// port: JSTypeTest#testStringObjectEnumType
#[test]
fn test_string_object_enum_type() {
    let mut f = Fixture::set_up();
    let _expr1 = EnumTypeBuilder::new();
    let _expr2 = _expr1.set_name("Enum");
    let _expr3 = _expr2.set_element_type(f.base.string_object_type);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let _expr5 = _expr4.get_elements_type(&f.base.reg);
    let string_enum = _expr5;
    let _expr6 = string_enum.get_property_type(&mut f.base.reg, &f.base.ast, "length");
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr7 = string_enum.find_property_type(&mut f.base.reg, &f.base.ast, "length");
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr8 = string_enum.has_property(&mut f.base.reg, &f.base.ast, "length");
    assert!(_expr8);
    let _expr9 = string_enum.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr9).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_function_type,
    );
}

// port: JSTypeTest#testObjectType
#[test]
fn test_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = PrototypeObjectTypeBuilder::new();
    let _expr2 = _expr1.build(&mut f.base.reg, &f.base.ast);
    let object_type = _expr2;
    let _expr3 = object_type.is_all_type(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = object_type.is_array_type(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = object_type.is_date_type(&f.base.reg);
    assert!(!_expr5);
    let _expr6 = object_type.is_function_prototype_type(&f.base.reg);
    assert!(!_expr6);
    let _expr7 = object_type.get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    assert_eq!(Some(f.base.object_type), _expr7);
    let _expr8 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr8);
    let _expr9 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr9);
    let _expr10 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr10);
    let _expr11 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr11);
    let _expr12 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr12);
    let _expr13 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr13);
    let _expr14 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr14);
    let _expr15 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr15);
    let _expr16 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(!_expr16);
    let _expr17 = object_type.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr17);
    let _expr18 = object_type.autoboxes_to(&f.base.reg);
    assert!(_expr18.is_none());
    f.base
        .assert_can_test_for_equality_with(object_type, f.base.number_type);
    f.base
        .assert_can_test_for_equality_with(object_type, f.base.bigint_type);
    let _expr19 = object_type.matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr19);
    let _expr20 = object_type.matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr20);
    let _expr21 = object_type.matches_object_context(&mut f.base.reg, &f.base.ast);
    assert!(_expr21);
    let _expr22 = object_type.matches_string_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr22);
    let _expr23 = object_type.matches_number_context(&mut f.base.reg, &f.base.ast);
    assert!(!_expr23);
    let _expr24 = object_type.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr24);
    let _expr25 = object_type.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(!_expr25);
    let _expr26 = f.base.create_nullable_type(object_type);
    let _expr27 = _expr26.is_nullable(&mut f.base.reg, &f.base.ast);
    assert!(_expr27);
    let _expr28 = f.base.create_union_type(&[object_type, f.base.void_type]);
    let _expr29 = _expr28.is_voidable(&mut f.base.reg, &f.base.ast);
    assert!(_expr29);
    let _expr30 = object_type.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr30, "{...}");
    let _expr31 = object_type.get_display_name(&f.base.reg);
    assert!(_expr31.is_none());
    TypeSubject::assert_type(object_type).get_reference_name_is_null(&f.base.reg);
    let _expr32 = PrototypeObjectTypeBuilder::new();
    let _expr33 = _expr32.set_name("anObject");
    let _expr34 = _expr33.build(&mut f.base.reg, &f.base.ast);
    let _expr35 = _expr34.get_display_name(&f.base.reg);
    assert_eq!(_expr35, Some("anObject".into()));
    let _expr36 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, object_type);
}

// port: JSTypeTest#testGoogBar
#[test]
fn test_goog_bar() {
    let mut f = Fixture::set_up();
    let _expr1 = f.named_goog_bar.is_instance_type(&f.base.reg);
    assert!(_expr1);
    let _expr2 = f.goog_bar.is_instance_type(&f.base.reg);
    assert!(!_expr2);
    let _expr3 = f.named_goog_bar.is_constructor(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = f.goog_bar.is_constructor(&f.base.reg);
    assert!(_expr4);
    let _expr5 = f.goog_bar.get_instance_type(&f.base.reg);
    let _expr6 = _expr5.unwrap().is_instance_type(&f.base.reg);
    assert!(_expr6);
    let _expr7 = f.named_goog_bar.get_constructor(&f.base.reg);
    let _expr8 = _expr7.unwrap().is_constructor(&f.base.reg);
    assert!(_expr8);
    let _expr9 = f
        .named_goog_bar
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr10 = _expr9.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr10);
    f.assert_type_can_assign_to_itself(f.goog_bar);
    f.assert_type_can_assign_to_itself(f.named_goog_bar);
    let _expr11 = f
        .goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    let _expr12 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.goog_bar);
    TypeSubject::assert_type(f.goog_bar).is_equal_to(&mut f.base.reg, &f.base.ast, f.goog_bar);
    TypeSubject::assert_type(f.goog_sub_bar).is_not_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.goog_bar,
    );
    let _expr13 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.goog_bar);
    let _expr14 = Asserts::assert_resolves_to_same(&mut f.base.reg, &f.base.ast, f.goog_sub_bar);
}

// port: JSTypeTest#testObjectTypePropertiesCount
#[test]
fn test_object_type_properties_count() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let sup = _expr1;
    let _expr2 = sup.get_properties_count(&mut f.base.reg, &f.base.ast);
    let native_properties = _expr2;
    let _expr3 =
        sup.define_declared_property(&mut f.base.reg, &f.base.ast, "a", f.base.date_type, None);
    let _expr4 = sup.get_properties_count(&mut f.base.reg, &f.base.ast);
    let _expr5 = native_properties + 1;
    assert_eq!(_expr4, _expr5);
    let _expr6 =
        sup.define_declared_property(&mut f.base.reg, &f.base.ast, "b", f.base.date_type, None);
    let _expr7 = sup.get_properties_count(&mut f.base.reg, &f.base.ast);
    let _expr8 = native_properties + 2;
    assert_eq!(_expr7, _expr8);
    let _expr9 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(sup))
        .build(&mut f.base.reg, &f.base.ast);
    let sub = _expr9;
    let _expr10 = sub.get_properties_count(&mut f.base.reg, &f.base.ast);
    let _expr11 = native_properties + 2;
    assert_eq!(_expr10, _expr11);
}

// port: JSTypeTest#testDefineProperties
#[test]
fn test_define_properties() {
    let mut f = Fixture::set_up();
    let _expr1 = f.goog_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    let prototype = _expr1;
    let _expr2 = f.goog_bar.get_instance_type(&f.base.reg);
    let instance = _expr2;
    let _expr3 = instance
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(prototype).is_equal_to(&mut f.base.reg, &f.base.ast, _expr3);
    let _expr4 = prototype.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "declared",
        f.base.number_type,
        None,
    );
    assert!(_expr4);
    let _expr5 = prototype.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "declared",
        f.base.number_type,
        None,
    );
    assert!(!_expr5);
    let _expr6 = instance.unwrap().define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "declared",
        f.base.number_type,
        None,
    );
    assert!(!_expr6);
    let _expr7 = instance
        .unwrap()
        .get_property_type(&mut f.base.reg, &f.base.ast, "declared");
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr8 = prototype.define_inferred_property(
        &mut f.base.reg,
        &f.base.ast,
        "inferred1",
        f.base.string_type,
        None,
    );
    assert!(_expr8);
    let _expr9 = prototype.define_inferred_property(
        &mut f.base.reg,
        &f.base.ast,
        "inferred1",
        f.base.number_type,
        None,
    );
    assert!(_expr9);
    let _expr10 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let _expr11 = instance
        .unwrap()
        .get_property_type(&mut f.base.reg, &f.base.ast, "inferred1");
    TypeSubject::assert_type(_expr11).is_equal_to(&mut f.base.reg, &f.base.ast, _expr10);
    let _expr12 = prototype.define_inferred_property(
        &mut f.base.reg,
        &f.base.ast,
        "inferred2",
        f.base.string_type,
        None,
    );
    assert!(_expr12);
    let _expr13 = instance.unwrap().define_inferred_property(
        &mut f.base.reg,
        &f.base.ast,
        "inferred2",
        f.base.number_type,
        None,
    );
    assert!(_expr13);
    let _expr14 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let _expr15 = instance
        .unwrap()
        .get_property_type(&mut f.base.reg, &f.base.ast, "inferred2");
    TypeSubject::assert_type(_expr15).is_equal_to(&mut f.base.reg, &f.base.ast, _expr14);
    let _expr16 = prototype.define_inferred_property(
        &mut f.base.reg,
        &f.base.ast,
        "prop",
        f.base.string_type,
        None,
    );
    assert!(_expr16);
    let _expr17 = instance.unwrap().define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "prop",
        f.base.number_type,
        None,
    );
    assert!(_expr17);
    let _expr18 = instance
        .unwrap()
        .get_property_type(&mut f.base.reg, &f.base.ast, "prop");
    TypeSubject::assert_type(_expr18).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr19 = prototype.get_property_type(&mut f.base.reg, &f.base.ast, "prop");
    TypeSubject::assert_type(_expr19).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
}

// port: JSTypeTest#testObjectTypePropertiesCountWithShadowing
#[test]
fn test_object_type_properties_count_with_shadowing() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let sup = _expr1;
    let _expr2 = sup.get_properties_count(&mut f.base.reg, &f.base.ast);
    let native_properties = _expr2;
    let _expr3 =
        sup.define_declared_property(&mut f.base.reg, &f.base.ast, "a", f.base.object_type, None);
    let _expr4 = sup.get_properties_count(&mut f.base.reg, &f.base.ast);
    let _expr5 = native_properties + 1;
    assert_eq!(_expr4, _expr5);
    let _expr6 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(sup))
        .build(&mut f.base.reg, &f.base.ast);
    let sub = _expr6;
    let _expr7 =
        sub.define_declared_property(&mut f.base.reg, &f.base.ast, "a", f.base.object_type, None);
    let _expr8 = sub.get_properties_count(&mut f.base.reg, &f.base.ast);
    let _expr9 = native_properties + 1;
    assert_eq!(_expr8, _expr9);
}

// port: JSTypeTest#testNamedGoogBar
#[test]
fn test_named_goog_bar() {
    let mut f = Fixture::set_up();
    let _expr1 = f.named_goog_bar.is_function_prototype_type(&f.base.reg);
    assert!(!_expr1);
    let _expr2 = f
        .named_goog_bar
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr3 = _expr2.unwrap().is_function_prototype_type(&f.base.reg);
    assert!(_expr3);
    let _expr4 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr4);
    let _expr5 =
        f.named_goog_bar
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr5);
    let _expr6 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr6);
    let _expr7 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr7);
    let _expr8 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr8);
    let _expr9 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr9);
    let _expr10 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr10);
    let _expr11 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    assert!(_expr11);
    let _expr12 =
        f.named_goog_bar
            .is_subtype(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    assert!(_expr12);
    let _expr13 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr13);
    let _expr14 = f
        .named_goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr14);
    let _expr15 = f.named_goog_bar.autoboxes_to(&f.base.reg);
    assert!(_expr15.is_none());
    let _expr16 = f
        .named_goog_bar
        .get_property_type(&mut f.base.reg, &f.base.ast, "date");
    TypeSubject::assert_type(_expr16).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.date_type);
    let _expr17 = f.named_goog_bar.is_native_object_type(&f.base.reg);
    assert!(!_expr17);
    let _expr18 = f
        .named_goog_bar
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr19 = _expr18.unwrap().is_native_object_type(&f.base.reg);
    assert!(!_expr19);
    let _expr20 = Asserts::assert_valid_resolve(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
    let resolved_named_goog_bar = _expr20;
    assert_ne!(f.named_goog_bar, resolved_named_goog_bar);
    let _expr21 = f.goog_bar.get_instance_type(&f.base.reg);
    assert_eq!(_expr21, Some(resolved_named_goog_bar));
}

// port: JSTypeTest#testPrototypeChaining
#[test]
fn test_prototype_chaining() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .array_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr2 = _expr1
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(f.base.object_type).is_equal_to(&mut f.base.reg, &f.base.ast, _expr2);
    let _expr3 = f
        .base
        .boolean_object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr4 = _expr3
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(f.base.object_type).is_equal_to(&mut f.base.reg, &f.base.ast, _expr4);
    let _expr5 = f
        .base
        .date_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr6 = _expr5
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(f.base.object_type).is_equal_to(&mut f.base.reg, &f.base.ast, _expr6);
    let _expr7 = f
        .base
        .number_object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr8 = _expr7
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(f.base.object_type).is_equal_to(&mut f.base.reg, &f.base.ast, _expr8);
    let _expr9 = f
        .base
        .string_object_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr10 = _expr9
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(f.base.object_type).is_equal_to(&mut f.base.reg, &f.base.ast, _expr10);
    let _expr11 = f
        .base
        .regexp_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr12 = _expr11
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(f.base.object_type).is_equal_to(&mut f.base.reg, &f.base.ast, _expr12);
}

// port: JSTypeTest#testInstanceFunctionChaining
#[test]
fn test_instance_function_chaining() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.array_type.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr1).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.array_function_type,
    );
    let _expr2 = f.base.boolean_object_type.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr2).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_function_type,
    );
    let _expr3 = f.base.date_type.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr3).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.date_function_type,
    );
    let _expr4 = f.base.number_object_type.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr4).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_function_type,
    );
    let _expr5 = f.base.object_type.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr5).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_function_type,
    );
    let _expr6 = f.base.regexp_type.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr6).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_function_type,
    );
    let _expr7 = f.base.string_object_type.get_constructor(&f.base.reg);
    TypeSubject::assert_type(_expr7).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_function_type,
    );
}

// port: JSTypeTest#testCanTestForEqualityWithCornerCases
#[test]
fn test_can_test_for_equality_with_corner_cases() {
    let mut f = Fixture::set_up();
    f.base
        .assert_cannot_test_for_equality_with(f.base.null_type, f.base.void_type);
    let _expr1 = f
        .base
        .create_union_type(&[f.base.object_type, f.base.null_type]);
    let nullable_object = _expr1;
    f.base
        .assert_can_test_for_equality_with(nullable_object, f.base.void_type);
    f.base
        .assert_can_test_for_equality_with(f.base.void_type, nullable_object);
}

// port: JSTypeTest#testTestForEquality
#[test]
fn test_test_for_equality() {
    let mut f = Fixture::set_up();
    f.compare(Tri::TRUE, f.base.no_object_type, f.base.no_object_type);
    f.compare(Tri::UNKNOWN, f.base.all_type, f.base.all_type);
    f.compare(Tri::TRUE, f.base.no_type, f.base.no_type);
    let _expr1 = closure_jstype::no_resolved_type::NoResolvedType::new(
        &mut f.base.reg,
        &f.base.ast,
        "Foo",
        None,
    );
    let foo_no_resolved_type = _expr1;
    f.compare(Tri::UNKNOWN, foo_no_resolved_type, foo_no_resolved_type);
    f.compare(Tri::UNKNOWN, f.base.no_object_type, f.base.number_type);
    f.compare(Tri::UNKNOWN, f.base.all_type, f.base.number_type);
    f.compare(Tri::UNKNOWN, f.base.no_type, f.base.number_type);
    f.compare(Tri::FALSE, f.base.null_type, f.base.boolean_type);
    f.compare(Tri::TRUE, f.base.null_type, f.base.null_type);
    f.compare(Tri::FALSE, f.base.null_type, f.base.number_type);
    f.compare(Tri::FALSE, f.base.null_type, f.base.object_type);
    f.compare(Tri::FALSE, f.base.null_type, f.base.string_type);
    f.compare(Tri::TRUE, f.base.null_type, f.base.void_type);
    let _expr2 = f
        .base
        .create_union_type(&[f.base.unknown_type, f.base.void_type]);
    f.compare(Tri::UNKNOWN, f.base.null_type, _expr2);
    let _expr3 = f
        .base
        .create_union_type(&[f.base.object_type, f.base.void_type]);
    f.compare(Tri::UNKNOWN, f.base.null_type, _expr3);
    f.compare(Tri::UNKNOWN, f.base.null_type, f.unresolved_named_type);
    let _expr4 = f
        .base
        .create_union_type(&[f.unresolved_named_type, f.base.date_type]);
    f.compare(Tri::UNKNOWN, f.base.null_type, _expr4);
    f.compare(Tri::FALSE, f.base.void_type, f.base.regexp_type);
    f.compare(Tri::TRUE, f.base.void_type, f.base.void_type);
    let _expr5 = f
        .base
        .create_union_type(&[f.base.regexp_type, f.base.void_type]);
    f.compare(Tri::UNKNOWN, f.base.void_type, _expr5);
    f.compare(Tri::UNKNOWN, f.base.number_type, f.base.boolean_type);
    f.compare(Tri::UNKNOWN, f.base.number_type, f.base.number_type);
    f.compare(Tri::UNKNOWN, f.base.number_type, f.base.object_type);
    f.compare(Tri::UNKNOWN, f.base.array_type, f.base.boolean_type);
    f.compare(Tri::UNKNOWN, f.base.object_type, f.base.boolean_type);
    f.compare(Tri::UNKNOWN, f.base.object_type, f.base.string_type);
    f.compare(Tri::UNKNOWN, f.base.string_type, f.base.string_type);
    f.compare(Tri::UNKNOWN, f.base.string_type, f.base.boolean_type);
    f.compare(Tri::UNKNOWN, f.base.string_type, f.base.number_type);
    f.compare(Tri::FALSE, f.base.string_type, f.base.void_type);
    f.compare(Tri::FALSE, f.base.string_type, f.base.null_type);
    let _expr6 = f
        .base
        .create_union_type(&[f.base.null_type, f.base.void_type]);
    f.compare(Tri::FALSE, f.base.string_type, _expr6);
    f.compare(Tri::UNKNOWN, f.base.unknown_type, f.base.boolean_type);
    f.compare(Tri::UNKNOWN, f.base.unknown_type, f.base.null_type);
    f.compare(Tri::UNKNOWN, f.base.unknown_type, f.base.void_type);
    f.compare(Tri::FALSE, f.base.function_type, f.base.boolean_type);
    f.compare(Tri::FALSE, f.base.function_type, f.base.number_type);
    f.compare(Tri::FALSE, f.base.function_type, f.base.string_type);
    f.compare(Tri::FALSE, f.base.function_type, f.base.void_type);
    f.compare(Tri::FALSE, f.base.function_type, f.base.null_type);
    f.compare(Tri::UNKNOWN, f.base.function_type, f.base.object_type);
    f.compare(Tri::UNKNOWN, f.base.function_type, f.base.all_type);
    f.compare(
        Tri::UNKNOWN,
        f.base.null_type,
        f.subclass_of_unresolved_named_type,
    );
    let _expr7 = f.base.create_union_type(&[f.base.null_type, f.date_method]);
    let function_and_null = _expr7;
    f.compare(Tri::UNKNOWN, function_and_null, f.date_method);
    f.compare(Tri::UNKNOWN, f.base.null_type, f.base.no_type);
    f.compare(Tri::UNKNOWN, f.base.void_type, f.base.no_type);
    f.compare(Tri::UNKNOWN, f.base.null_type, f.unresolved_named_type);
    f.compare(Tri::UNKNOWN, f.base.void_type, f.unresolved_named_type);
    f.compare(Tri::TRUE, f.base.no_type, f.base.no_type);
}

// port: JSTypeTest#testSubtypingSimpleTypes
#[test]
fn test_subtyping_simple_types() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(_expr1);
    let _expr2 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr2);
    let _expr3 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(_expr3);
    let _expr4 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(_expr4);
    let _expr5 =
        f.base
            .no_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(_expr5);
    let _expr6 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr6);
    let _expr7 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(_expr7);
    let _expr8 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(_expr8);
    let _expr9 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr9);
    let _expr10 =
        f.base
            .no_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(_expr10);
    let _expr11 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr11);
    let _expr12 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(_expr12);
    let _expr13 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(_expr13);
    let _expr14 =
        f.base
            .no_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(_expr14);
    let _expr15 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr15);
    let _expr16 = f
        .base
        .no_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(_expr16);
    let _expr17 = f
        .base
        .no_object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr17);
    let _expr18 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(_expr18);
    let _expr19 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(_expr19);
    let _expr20 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr20);
    let _expr21 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(_expr21);
    let _expr22 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr22);
    let _expr23 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(_expr23);
    let _expr24 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr24);
    let _expr25 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr25);
    let _expr26 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(_expr26);
    let _expr27 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr27);
    let _expr28 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(_expr28);
    let _expr29 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr29);
    let _expr30 = f.base.no_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(_expr30);
    let _expr31 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr31);
    let _expr32 =
        f.base
            .no_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr32);
    let _expr33 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr33);
    let _expr34 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr34);
    let _expr35 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(_expr35);
    let _expr36 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr36);
    let _expr37 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(!_expr37);
    let _expr38 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr38);
    let _expr39 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr39);
    let _expr40 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr41);
    let _expr42 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(!_expr42);
    let _expr43 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr43);
    let _expr44 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr44);
    let _expr45 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr45);
    let _expr46 =
        f.base
            .array_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr46);
    let _expr47 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr47);
    let _expr48 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr48);
    let _expr49 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr49);
    let _expr50 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr50);
    let _expr51 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr51);
    let _expr52 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(_expr52);
    let _expr53 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(!_expr53);
    let _expr54 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr54);
    let _expr55 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr55);
    let _expr56 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr56);
    let _expr57 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr57);
    let _expr58 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(!_expr58);
    let _expr59 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr59);
    let _expr60 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr60);
    let _expr61 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr61);
    let _expr62 =
        f.base
            .boolean_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr62);
    let _expr63 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr63);
    let _expr64 = f
        .base
        .boolean_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr64);
    let _expr65 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr65);
    let _expr66 = f.base.boolean_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    assert!(!_expr66);
    let _expr67 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr67);
    let _expr68 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr68);
    let _expr69 = f.base.boolean_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_type,
    );
    assert!(_expr69);
    let _expr70 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(!_expr70);
    let _expr71 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr71);
    let _expr72 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr72);
    let _expr73 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr73);
    let _expr74 = f.base.boolean_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.number_object_type,
    );
    assert!(!_expr74);
    let _expr75 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr75);
    let _expr76 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr76);
    let _expr77 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr77);
    let _expr78 = f.base.boolean_object_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    assert!(!_expr78);
    let _expr79 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr79);
    let _expr80 =
        f.base
            .boolean_object_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr80);
    let _expr81 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr81);
    let _expr82 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr82);
    let _expr83 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr83);
    let _expr84 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr84);
    let _expr85 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(!_expr85);
    let _expr86 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.date_type);
    assert!(_expr86);
    let _expr87 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr87);
    let _expr88 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr88);
    let _expr89 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr89);
    let _expr90 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(!_expr90);
    let _expr91 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr91);
    let _expr92 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr92);
    let _expr93 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr93);
    let _expr94 =
        f.base
            .date_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr94);
    let _expr95 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr95);
    let _expr96 = f
        .base
        .date_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr96);
    let _expr97 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_type);
    assert!(!_expr97);
    let _expr98 =
        f.base
            .all_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.no_object_type);
    assert!(!_expr98);
    let _expr99 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr99);
    let _expr100 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr100);
    let _expr101 =
        f.base
            .all_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.boolean_object_type);
    assert!(!_expr101);
    let _expr102 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.function_type);
    assert!(!_expr102);
    let _expr103 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr103);
    let _expr104 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr104);
    let _expr105 =
        f.base
            .all_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.number_object_type);
    assert!(!_expr105);
    let _expr106 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr106);
    let _expr107 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    assert!(!_expr107);
    let _expr108 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr108);
    let _expr109 =
        f.base
            .all_type
            .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.string_object_type);
    assert!(!_expr109);
    let _expr110 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.all_type);
    assert!(_expr110);
    let _expr111 = f
        .base
        .all_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr111);
}

// port: JSTypeTest#testSubtypingObjectTopOfObjects
#[test]
fn test_subtyping_object_top_of_objects() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .object_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr1);
    let _expr2 = f
        .base
        .create_union_type(&[f.base.date_type, f.base.regexp_type]);
    let _expr3 = _expr2.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr3);
    let _expr4 = f
        .base
        .create_union_type(&[f.base.object_type, f.base.no_object_type]);
    let _expr5 = _expr4.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr5);
    let _expr6 = f
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr6);
}

// port: JSTypeTest#testSubtypingFunctionPrototypeType
#[test]
fn test_subtyping_function_prototype_type() {
    let mut f = Fixture::set_up();
    let _expr3 = f.with_open_registry(|f| {
        let _expr1 = f.base.reg.create_parameters(&[f.base.all_type]);

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("Foo1".into()),
            None,
            Some(_expr1),
            None,
            None,
            false,
        )
    });
    let sub1 = _expr3;
    sub1.set_prototype_based_on(&mut f.base.reg, &f.base.ast, f.goog_bar);
    let _expr6 = f.with_open_registry(|f| {
        let _expr4 = f.base.reg.create_parameters(&[f.base.all_type]);

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("Foo2".into()),
            None,
            Some(_expr4),
            None,
            None,
            false,
        )
    });
    let sub2 = _expr6;
    sub2.set_prototype_based_on(&mut f.base.reg, &f.base.ast, f.goog_bar);
    let _expr7 = sub1.get_instance_type(&f.base.reg);
    let o1 = _expr7;
    let _expr8 = sub2.get_instance_type(&f.base.reg);
    let o2 = _expr8;
    let _expr9 = o1
        .unwrap()
        .is_subtype_of(&mut f.base.reg, &f.base.ast, o2.unwrap());
    assert!(!_expr9);
    let _expr10 = o1
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr11 = o2
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr12 = _expr10
        .unwrap()
        .is_subtype_of(&mut f.base.reg, &f.base.ast, _expr11.unwrap());
    assert!(!_expr12);
    let _expr13 = o1
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr14 = _expr13
        .unwrap()
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.goog_bar);
    assert!(_expr14);
    let _expr15 = o2
        .unwrap()
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr16 = _expr15
        .unwrap()
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.goog_bar);
    assert!(_expr16);
}

// port: JSTypeTest#testSubtypingFunctionFixedArgs
#[test]
fn test_subtyping_function_fixed_args() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.base
            .reg
            .create_function_type(&f.base.ast, f.base.object_type, &[f.base.boolean_type]);
    let f1 = _expr1;
    let _expr2 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.string_object_type,
        &[f.base.boolean_type],
    );
    let f2 = _expr2;
    let _expr3 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr3);
    let _expr4 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(!_expr4);
    let _expr5 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr5);
    let _expr6 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr6);
    let _expr7 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr7);
    let _expr8 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr8);
    let _expr9 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr9);
    let _expr10 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr10);
}

// port: JSTypeTest#testSubtypingFunctionMultipleFixedArgs
#[test]
fn test_subtyping_function_multiple_fixed_args() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.object_type,
        &[f.base.number_type, f.base.string_type],
    );
    let f1 = _expr1;
    let _expr2 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.string_object_type,
        &[f.base.number_string, f.base.all_type],
    );
    let f2 = _expr2;
    let _expr3 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr3);
    let _expr4 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(!_expr4);
    let _expr5 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr5);
    let _expr6 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr6);
    let _expr7 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr7);
    let _expr8 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr8);
    let _expr9 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr9);
    let _expr10 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr10);
}

// port: JSTypeTest#testSubtypingFunctionFixedArgsNotMatching
#[test]
fn test_subtyping_function_fixed_args_not_matching() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.object_type,
        &[f.base.string_type, f.base.unknown_type],
    );
    let f1 = _expr1;
    let _expr2 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.string_object_type,
        &[f.base.number_string, f.base.all_type],
    );
    let f2 = _expr2;
    let _expr3 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr3);
    let _expr4 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(!_expr4);
    let _expr5 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr5);
    let _expr6 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr6);
    let _expr7 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr7);
    let _expr8 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr8);
    let _expr9 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr9);
    let _expr10 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr10);
}

// port: JSTypeTest#testSubtypingFunctionVariableArgsOneOnly
#[test]
fn test_subtyping_function_variable_args_one_only() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_function_type_with_var_args(
        &f.base.ast,
        f.base.object_type,
        &[f.base.string_type],
    );
    let f1 = _expr1;
    let _expr2 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.string_object_type,
        &[f.base.number_string, f.base.object_type],
    );
    let f2 = _expr2;
    let _expr3 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr3);
    let _expr4 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(!_expr4);
    let _expr5 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(!_expr5);
    let _expr6 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr6);
    let _expr7 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr7);
    let _expr8 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr8);
    let _expr9 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr9);
    let _expr10 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr10);
}

// port: JSTypeTest#testSubtypingFunctionVariableArgsBoth
#[test]
fn test_subtyping_function_variable_args_both() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_function_type_with_var_args(
        &f.base.ast,
        f.base.object_type,
        &[
            f.base.number_type,
            f.base.string_object_type,
            f.base.string_type,
        ],
    );
    let f1 = _expr1;
    let _expr2 = f.base.reg.create_function_type_with_var_args(
        &f.base.ast,
        f.base.string_object_type,
        &[f.base.number_string, f.base.object_type, f.base.string_type],
    );
    let f2 = _expr2;
    let _expr3 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr3);
    let _expr4 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(!_expr4);
    let _expr5 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr5);
    let _expr6 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr6);
    let _expr7 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr7);
    let _expr8 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(_expr8);
    let _expr9 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr9);
    let _expr10 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr10);
}

// port: JSTypeTest#testSubtypingMostGeneralFunction
#[test]
fn test_subtyping_most_general_function() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.object_type,
        &[f.base.string_type, f.base.string_type],
    );
    let f1 = _expr1;
    let _expr2 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.number_type,
        &[f.base.string_type, f.base.void_type],
    );
    let f2 = _expr2;
    let _expr3 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.no_object_type,
        &[f.base.date_type, f.base.string_type, f.base.number_type],
    );
    let f3 = _expr3;
    let _expr4 =
        f.base
            .reg
            .create_function_type(&f.base.ast, f.base.no_type, &[f.base.number_object_type]);
    let f4 = _expr4;
    let _expr5 = f.base.reg.create_function_type_with_var_args(
        &f.base.ast,
        f.base.object_type,
        &[f.base.string_type],
    );
    let f5 = _expr5;
    let _expr6 = f.base.reg.create_function_type(
        &f.base.ast,
        f.base.string_object_type,
        &[f.base.number_string, f.base.object_type],
    );
    let f6 = _expr6;
    let _expr7 = f.base.reg.create_function_type_with_var_args(
        &f.base.ast,
        f.base.object_type,
        &[f.base.number_type, f.base.string_type],
    );
    let f7 = _expr7;
    let _expr8 = f.base.reg.create_function_type_with_var_args(
        &f.base.ast,
        f.base.string_object_type,
        &[f.base.number_string, f.base.object_type, f.base.string_type],
    );
    let f8 = _expr8;
    let _expr9 = f.base.least_function_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.greatest_function_type,
    );
    assert!(_expr9);
    let _expr10 = f.base.least_function_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_type,
    );
    assert!(_expr10);
    let _expr11 =
        f.base
            .function_type
            .is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(_expr11);
    let _expr12 = f.base.greatest_function_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.least_function_type,
    );
    assert!(!_expr12);
    let _expr13 = f.base.greatest_function_type.is_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_type,
    );
    assert!(_expr13);
    let _expr14 = f.base.function_type.is_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.greatest_function_type,
    );
    assert!(_expr14);
    let _expr15 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f.base.greatest_function_type);
    assert!(_expr15);
    let _expr16 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f.base.greatest_function_type);
    assert!(_expr16);
    let _expr17 = f3.is_subtype(&mut f.base.reg, &f.base.ast, f.base.greatest_function_type);
    assert!(_expr17);
    let _expr18 = f4.is_subtype(&mut f.base.reg, &f.base.ast, f.base.greatest_function_type);
    assert!(_expr18);
    let _expr19 = f5.is_subtype(&mut f.base.reg, &f.base.ast, f.base.greatest_function_type);
    assert!(_expr19);
    let _expr20 = f6.is_subtype(&mut f.base.reg, &f.base.ast, f.base.greatest_function_type);
    assert!(_expr20);
    let _expr21 = f7.is_subtype(&mut f.base.reg, &f.base.ast, f.base.greatest_function_type);
    assert!(_expr21);
    let _expr22 = f8.is_subtype(&mut f.base.reg, &f.base.ast, f.base.greatest_function_type);
    assert!(_expr22);
    let _expr23 = f1.is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(!_expr23);
    let _expr24 = f2.is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(!_expr24);
    let _expr25 = f3.is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(!_expr25);
    let _expr26 = f4.is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(!_expr26);
    let _expr27 = f5.is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(!_expr27);
    let _expr28 = f6.is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(!_expr28);
    let _expr29 = f7.is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(!_expr29);
    let _expr30 = f8.is_subtype(&mut f.base.reg, &f.base.ast, f.base.least_function_type);
    assert!(!_expr30);
    let _expr31 = f
        .base
        .least_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f1);
    assert!(_expr31);
    let _expr32 = f
        .base
        .least_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f2);
    assert!(_expr32);
    let _expr33 = f
        .base
        .least_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f3);
    assert!(_expr33);
    let _expr34 = f
        .base
        .least_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f4);
    assert!(_expr34);
    let _expr35 = f
        .base
        .least_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f5);
    assert!(_expr35);
    let _expr36 = f
        .base
        .least_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f6);
    assert!(_expr36);
    let _expr37 = f
        .base
        .least_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f7);
    assert!(_expr37);
    let _expr38 = f
        .base
        .least_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f8);
    assert!(_expr38);
    let _expr39 = f
        .base
        .greatest_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f1);
    assert!(!_expr39);
    let _expr40 = f
        .base
        .greatest_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f2);
    assert!(!_expr40);
    let _expr41 = f
        .base
        .greatest_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f3);
    assert!(!_expr41);
    let _expr42 = f
        .base
        .greatest_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f4);
    assert!(!_expr42);
    let _expr43 = f
        .base
        .greatest_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f5);
    assert!(!_expr43);
    let _expr44 = f
        .base
        .greatest_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f6);
    assert!(!_expr44);
    let _expr45 = f
        .base
        .greatest_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f7);
    assert!(!_expr45);
    let _expr46 = f
        .base
        .greatest_function_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f8);
    assert!(!_expr46);
}

// port: JSTypeTest#testSymmetryOfTestForEquality
#[test]
fn test_symmetry_of_test_for_equality() {
    let mut f = Fixture::set_up();
    let _expr1 = f.get_types_to_test_for_symmetry();
    let list_a = _expr1;
    let _expr2 = f.get_types_to_test_for_symmetry();
    let list_b = _expr2;
    for type_a in list_a.clone() {
        for type_b in list_b.clone() {
            let _expr3 = type_a.test_for_equality(&mut f.base.reg, &f.base.ast, type_b);
            let a_on_b = _expr3;
            let _expr4 = type_b.test_for_equality(&mut f.base.reg, &f.base.ast, type_a);
            let b_on_a = _expr4;
            let _expr5 = BaseJSTypeTestCase::lines(&[
                "testForEquality not symmetrical:",
                "typeA: %s\ntypeB: %s",
                "a.testForEquality(b): %s",
                "b.testForEquality(a): %s",
            ]);
            let _expr6 = a_on_b == b_on_a;
            let _expr7 = closure_rhino::jscomp_base::guava_format(
                &(_expr5),
                &[
                    (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (format!("{:?}", a_on_b)).to_string(),
                    (format!("{:?}", b_on_a)).to_string(),
                ],
            );
            assert!(_expr6, "{}", _expr7);
        }
    }
}

// port: JSTypeTest#testSymmetryOfLeastSupertype
#[test]
fn test_symmetry_of_least_supertype() {
    let mut f = Fixture::set_up();
    let _expr1 = f.get_types_to_test_for_symmetry();
    let list_a = _expr1;
    let _expr2 = f.get_types_to_test_for_symmetry();
    let list_b = _expr2;
    for type_a in list_a.clone() {
        for type_b in list_b.clone() {
            let _expr3 = type_a.get_least_supertype(&mut f.base.reg, &f.base.ast, type_b);
            let a_on_b = _expr3;
            let _expr4 = type_b.get_least_supertype(&mut f.base.reg, &f.base.ast, type_a);
            let b_on_a = _expr4;
            let _expr5 = BaseJSTypeTestCase::lines(&[
                "getLeastSupertype not symmetrical:",
                "typeA: %s",
                "typeB: %s",
                "a.getLeastSupertype(b): %s",
                "b.getLeastSupertype(a): %s",
            ]);
            let _expr6 = a_on_b.equals(&mut f.base.reg, &f.base.ast, b_on_a);
            let _expr7 = closure_rhino::jscomp_base::guava_format(
                &(_expr5),
                &[
                    (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (a_on_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (b_on_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                ],
            );
            assert!(_expr6, "{}", _expr7);
        }
    }
}

// port: JSTypeTest#testWeirdBug
#[test]
fn test_weird_bug() {
    let mut f = Fixture::set_up();
    let _expr1 = f.goog_bar.get_instance_type(&f.base.reg);
    TypeSubject::assert_type(_expr1).is_not_equal_to(&mut f.base.reg, &f.base.ast, f.goog_bar);
    let _expr2 = f.goog_bar.get_instance_type(&f.base.reg);
    let _expr3 = f
        .goog_bar
        .is_subtype(&mut f.base.reg, &f.base.ast, _expr2.unwrap());
    assert!(!_expr3);
    let _expr4 = f.goog_bar.get_instance_type(&f.base.reg);
    let _expr5 = _expr4
        .unwrap()
        .is_subtype_of(&mut f.base.reg, &f.base.ast, f.goog_bar);
    assert!(!_expr5);
}

// port: JSTypeTest#testSymmetryOfGreatestSubtype
#[test]
fn test_symmetry_of_greatest_subtype() {
    let mut f = Fixture::set_up();
    let _expr1 = f.get_types_to_test_for_symmetry();
    let list_a = _expr1;
    let _expr2 = f.get_types_to_test_for_symmetry();
    let list_b = _expr2;
    for type_a in list_a.clone() {
        for type_b in list_b.clone() {
            let _expr3 = type_a.get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_b);
            let a_on_b = _expr3;
            let _expr4 = type_b.get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_a);
            let b_on_a = _expr4;
            let _expr5 = BaseJSTypeTestCase::lines(&[
                "getGreatestSubtype not symmetrical:",
                "typeA: %s",
                "typeB: %s",
                "a.getGreatestSubtype(b): %s",
                "b.getGreatestSubtype(a): %s",
            ]);
            let _expr6 = a_on_b.equals(&mut f.base.reg, &f.base.ast, b_on_a);
            let _expr7 = closure_rhino::jscomp_base::guava_format(
                &(_expr5),
                &[
                    (type_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (type_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (a_on_b.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                    (b_on_a.to_string(&mut f.base.reg, &f.base.ast)).to_string(),
                ],
            );
            assert!(_expr6, "{}", _expr7);
        }
    }
}

// port: JSTypeTest#testReflexivityOfLeastSupertype
#[test]
fn test_reflexivity_of_least_supertype() {
    let mut f = Fixture::set_up();
    let _expr1 = f.get_types_to_test_for_symmetry();
    let list = _expr1;
    for type_ in list.clone() {
        let _expr2 = type_.get_least_supertype(&mut f.base.reg, &f.base.ast, type_);
        assert_type_equal_with_message(
            &mut f.base.reg,
            &f.base.ast,
            type_,
            _expr2,
            "getLeastSupertype not reflexive",
        );
    }
}

// port: JSTypeTest#testReflexivityOfGreatestSubtype
#[test]
fn test_reflexivity_of_greatest_subtype() {
    let mut f = Fixture::set_up();
    let _expr1 = f.get_types_to_test_for_symmetry();
    let list = _expr1;
    for type_ in list.clone() {
        let _expr2 = type_.get_greatest_subtype(&mut f.base.reg, &f.base.ast, type_);
        assert_type_equal_with_message(
            &mut f.base.reg,
            &f.base.ast,
            type_,
            _expr2,
            "getGreatestSubtype not reflexive",
        );
    }
}

// port: JSTypeTest#testLeastSupertypeUnresolvedNamedType
#[test]
fn test_least_supertype_unresolved_named_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_union_type(
        &f.base.ast,
        &[f.unresolved_named_type, f.base.function_type],
    );
    let expected = _expr1;
    let _expr2 = f.unresolved_named_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.function_type,
    );
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, expected);
    let _expr3 = f.base.function_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.unresolved_named_type,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, expected);
    let _expr4 = expected.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr4, "(?|Function)");
}

// port: JSTypeTest#testLeastSupertypeUnresolvedNamedType2
#[test]
fn test_least_supertype_unresolved_named_type2() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.unresolved_named_type, f.base.unknown_type]);
    let expected = _expr1;
    let _expr2 = f.unresolved_named_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, expected);
    let _expr3 = f.base.unknown_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.unresolved_named_type,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, expected);
    TypeSubject::assert_type(expected).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
}

// port: JSTypeTest#testLeastSupertypeUnresolvedNamedType3
#[test]
fn test_least_supertype_unresolved_named_type3() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_union_type(
        &f.base.ast,
        &[f.unresolved_named_type, f.base.checked_unknown_type],
    );
    let expected = _expr1;
    let _expr2 = f.unresolved_named_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.checked_unknown_type,
    );
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, expected);
    let _expr3 = f.base.checked_unknown_type.get_least_supertype(
        &mut f.base.reg,
        &f.base.ast,
        f.unresolved_named_type,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, expected);
    TypeSubject::assert_type(expected).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.checked_unknown_type,
    );
}

// port: JSTypeTest#testSubclassOfUnresolvedNamedType
#[test]
fn test_subclass_of_unresolved_named_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .subclass_of_unresolved_named_type
        .is_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(_expr1);
}

// port: JSTypeTest#testSupertypeOfProxiedFunctionTypes
#[test]
fn test_supertype_of_proxied_function_types() {
    let mut f = Fixture::set_up();
    let _expr1 = FunctionTypeBuilder::new();
    let _expr2 = f.base.reg.create_parameters(&[]);
    let _expr3 = _expr1.with_parameters(_expr2);
    let _expr4 = _expr3.with_return_type(f.base.number_type);
    let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
    let fn1 = _expr5;
    let _expr6 = FunctionTypeBuilder::new();
    let _expr7 = f.base.reg.create_parameters(&[]);
    let _expr8 = _expr6.with_parameters(_expr7);
    let _expr9 = _expr8.with_return_type(f.base.string_type);
    let _expr10 = _expr9.build(&mut f.base.reg, &f.base.ast);
    let fn2 = _expr10;
    let _expr11 =
        closure_jstype::proxy_object_type::create(&mut f.base.reg, &f.base.ast, fn1, None);
    let p1 = _expr11;
    let _expr12 =
        closure_jstype::proxy_object_type::create(&mut f.base.reg, &f.base.ast, fn2, None);
    let p2 = _expr12;
    let _expr13 = FunctionTypeBuilder::new();
    let _expr14 = f.base.reg.create_parameters(&[]);
    let _expr15 = _expr13.with_parameters(_expr14);
    let _expr16 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[f.base.string_type, f.base.number_type]);
    let _expr17 = _expr15.with_return_type(_expr16);
    let _expr18 = _expr17.build(&mut f.base.reg, &f.base.ast);
    let supremum = _expr18;
    let _expr19 = fn1.get_least_supertype(&mut f.base.reg, &f.base.ast, fn2);
    let _expr20 = p1.get_least_supertype(&mut f.base.reg, &f.base.ast, p2);
    TypeSubject::assert_type(_expr20).is_equal_to(&mut f.base.reg, &f.base.ast, _expr19);
    let _expr21 = fn1.get_least_supertype(&mut f.base.reg, &f.base.ast, fn2);
    TypeSubject::assert_type(_expr21).is_equal_to(&mut f.base.reg, &f.base.ast, supremum);
    let _expr22 = fn1.get_least_supertype(&mut f.base.reg, &f.base.ast, p2);
    TypeSubject::assert_type(_expr22).is_equal_to(&mut f.base.reg, &f.base.ast, supremum);
    let _expr23 = p1.get_least_supertype(&mut f.base.reg, &f.base.ast, fn2);
    TypeSubject::assert_type(_expr23).is_equal_to(&mut f.base.reg, &f.base.ast, supremum);
    let _expr24 = p1.get_least_supertype(&mut f.base.reg, &f.base.ast, p2);
    TypeSubject::assert_type(_expr24).is_equal_to(&mut f.base.reg, &f.base.ast, supremum);
}

// port: JSTypeTest#testTypeOfThisIsProxied
#[test]
fn test_type_of_this_is_proxied() {
    let mut f = Fixture::set_up();
    let _expr1 = FunctionTypeBuilder::new();
    let _expr2 = _expr1.with_return_type(f.base.number_type);
    let _expr3 = _expr2.with_type_of_this(f.base.object_type);
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let fn_type = _expr4;
    let _expr5 =
        closure_jstype::proxy_object_type::create(&mut f.base.reg, &f.base.ast, fn_type, None);
    let proxy_type = _expr5;
    let _expr6 = fn_type.get_type_of_this(&f.base.reg);
    let _expr7 = proxy_type.get_type_of_this(&f.base.reg);
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, _expr6);
}

// port: JSTypeTest#testNamedTypeEquals
#[test]
fn test_named_type_equals() {
    let mut f = Fixture::set_up();
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        f.base
            .error_reporter
            .lock()
            .unwrap()
            .expect_all_warnings(&["Bad type annotation. Unknown type type1"]);
        let _expr2 = f.base.reg.create_named_type(
            &f.base.ast,
            Some(Arc::new(MapBasedScope::empty_scope()).clone()),
            "type1",
            "source",
            1,
            0,
        );
        let a = _expr2;
        let _expr3 = f.base.reg.create_named_type(
            &f.base.ast,
            Some(Arc::new(MapBasedScope::empty_scope()).clone()),
            "type1",
            "source",
            1,
            0,
        );
        let b = _expr3;
        let _expr4 = a.equals(&mut f.base.reg, &f.base.ast, b);
        assert!(_expr4);
        let _expr5 = f.goog_bar.get_instance_type(&f.base.reg);
        let _expr6 = f
            .named_goog_bar
            .equals(&mut f.base.reg, &f.base.ast, _expr5);
        assert!(_expr6);
        let _expr7 = f.goog_bar.get_instance_type(&f.base.reg);
        let _expr8 = _expr7
            .unwrap()
            .equals(&mut f.base.reg, &f.base.ast, f.named_goog_bar);
        assert!(_expr8);
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
}

// port: JSTypeTest#testNamedTypeEquals2
#[test]
fn test_named_type_equals2() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.get_resolver().open_for_definition();
    let mut closer = _expr1;
    let _expr2 = f.base.reg.create_named_type(
        &f.base.ast,
        Some(Arc::new(MapBasedScope::empty_scope()).clone()),
        "typeA",
        "source",
        1,
        0,
    );
    let a = _expr2;
    let _expr3 = f.base.reg.create_named_type(
        &f.base.ast,
        Some(Arc::new(MapBasedScope::empty_scope()).clone()),
        "typeB",
        "source",
        1,
        0,
    );
    let b = _expr3;
    let _expr4 = f.base.reg.create_constructor_type(
        &f.base.ast,
        Some("typeA".into()),
        None,
        None,
        None,
        None,
        false,
    );
    let _expr5 = _expr4.get_instance_type(&f.base.reg);
    let real_a = _expr5;
    let _expr6 = EnumTypeBuilder::new();
    let _expr7 = _expr6.set_name("typeB");
    let _expr8 = _expr7.set_element_type(f.base.number_type);
    let _expr9 = _expr8.build(&mut f.base.reg, &f.base.ast);
    let _expr10 = _expr9.get_elements_type(&f.base.reg);
    let real_b = _expr10;
    let _expr11 = f
        .base
        .reg
        .declare_type(&f.base.ast, None, "typeA", real_a.unwrap());
    let _expr12 = f.base.reg.declare_type(&f.base.ast, None, "typeB", real_b);
    closer.close(&mut f.base.reg, &f.base.ast);
    let _expr14 = a.is_resolved(&f.base.reg);
    assert!(_expr14);
    let _expr15 = b.is_resolved(&f.base.reg);
    assert!(_expr15);
    TypeSubject::assert_type(real_a).is_equal_to(&mut f.base.reg, &f.base.ast, a);
    TypeSubject::assert_type(real_b).is_equal_to(&mut f.base.reg, &f.base.ast, b);
    let _expr16 = Asserts::assert_valid_resolve(&mut f.base.reg, &f.base.ast, a);
    let resolved_a = _expr16;
    assert_ne!(a, resolved_a);
    assert_eq!(real_a, Some(resolved_a));
    let _expr17 = Asserts::assert_valid_resolve(&mut f.base.reg, &f.base.ast, b);
    let resolved_b = _expr17;
    assert_ne!(b, resolved_b);
    assert_eq!(real_b, resolved_b);
}

// port: JSTypeTest#testMeaningOfUnresolved
#[test]
fn test_meaning_of_unresolved() {
    let mut f = Fixture::set_up();
    let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
    let _expr2 = UnitTestingJSType::new(&mut f.base.reg);
    let under_test = _expr2;
    let _expr3 = under_test.is_resolved(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = under_test.is_successfully_resolved(&f.base.reg);
    assert!(!_expr4);
    let _expr5 = under_test.is_unsuccessfully_resolved(&f.base.reg);
    assert!(!_expr5);
}

// port: JSTypeTest#testMeaningOfSuccessfullyResolved
#[test]
fn test_meaning_of_successfully_resolved() {
    let mut f = Fixture::set_up();
    // port: JSTypeTest#isNoResolvedType
    // port: JSTypeTest#getTypeClass
    let _expr1 = UnitTestingJSType::with_data(
        &mut f.base.reg,
        UnitTestingJSTypeData {
            type_class: Some(JSTypeClass::NO),
            is_no_resolved_type: Some(false),
            ..Default::default()
        },
    );
    let under_test = _expr1;
    under_test.eagerly_resolve_to_self(&mut f.base.reg, &f.base.ast);
    let _expr2 = under_test.is_resolved(&f.base.reg);
    assert!(_expr2);
    let _expr3 = under_test.is_successfully_resolved(&f.base.reg);
    assert!(_expr3);
    let _expr4 = under_test.is_unsuccessfully_resolved(&f.base.reg);
    assert!(!_expr4);
}

// port: JSTypeTest#testMeaningOfUnsuccessfullyResolved
#[test]
fn test_meaning_of_unsuccessfully_resolved() {
    let mut f = Fixture::set_up();
    // port: JSTypeTest#isNoResolvedType
    // port: JSTypeTest#getTypeClass
    let _expr1 = UnitTestingJSType::with_data(
        &mut f.base.reg,
        UnitTestingJSTypeData {
            type_class: Some(JSTypeClass::NO),
            is_no_resolved_type: Some(true),
            ..Default::default()
        },
    );
    let under_test = _expr1;
    under_test.eagerly_resolve_to_self(&mut f.base.reg, &f.base.ast);
    let _expr2 = under_test.is_resolved(&f.base.reg);
    assert!(_expr2);
    let _expr3 = under_test.is_successfully_resolved(&f.base.reg);
    assert!(!_expr3);
    let _expr4 = under_test.is_unsuccessfully_resolved(&f.base.reg);
    assert!(_expr4);
}

// port: JSTypeTest#testGreatestSubtypeSimpleTypes
#[test]
fn test_greatest_subtype_simple_types() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.base
            .array_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.all_type);
    TypeSubject::assert_type(_expr1).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.array_type);
    let _expr2 =
        f.base
            .all_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.array_type);
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.array_type);
    let _expr3 = f.base.regexp_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr4 = f.base.no_object_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.regexp_type,
    );
    TypeSubject::assert_type(_expr4).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr5 = f.base.array_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr5).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr6 =
        f.base
            .array_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.number_type);
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr7 =
        f.base
            .array_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.function_type);
    TypeSubject::assert_type(_expr7).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr8 = f.base.string_object_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.object_type,
    );
    TypeSubject::assert_type(_expr8).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    let _expr9 = f.base.object_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    TypeSubject::assert_type(_expr9).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    let _expr10 =
        f.base
            .array_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.date_type);
    TypeSubject::assert_type(_expr10).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr11 =
        f.base
            .array_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.regexp_type);
    TypeSubject::assert_type(_expr11).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr12 =
        f.base
            .null_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.array_type);
    TypeSubject::assert_type(_expr12).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr13 =
        f.base
            .number_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
    TypeSubject::assert_type(_expr13).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr14 = f.base.number_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.checked_unknown_type,
    );
    TypeSubject::assert_type(_expr14).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr15 = f.base.no_object_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.forward_declared_named_type,
    );
    TypeSubject::assert_type(_expr15).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr16 = f.forward_declared_named_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    TypeSubject::assert_type(_expr16).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr17 = f.base.checked_unknown_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.checked_unknown_type,
    );
    TypeSubject::assert_type(_expr17).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.checked_unknown_type,
    );
    let _expr18 = f.base.checked_unknown_type.get_greatest_subtype(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    TypeSubject::assert_type(_expr18).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.checked_unknown_type,
    );
}

// port: JSTypeTest#testSubtypingDerivedExtendsNamedBaseType
#[test]
fn test_subtyping_derived_extends_named_base_type() {
    let mut f = Fixture::set_up();
    let _expr1 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(f.named_goog_bar))
        .build(&mut f.base.reg, &f.base.ast);
    let _expr2 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(_expr1))
        .build(&mut f.base.reg, &f.base.ast);
    let derived = _expr2;
    let _expr3 = f.goog_bar.get_instance_type(&f.base.reg);
    let _expr4 = derived.is_subtype_of(&mut f.base.reg, &f.base.ast, _expr3.unwrap());
    assert!(_expr4);
}

// port: JSTypeTest#testNamedSubtypeChain
#[test]
fn test_named_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr2 = f.base.reg.get_native_type(JSTypeNative::OBJECT_PROTOTYPE);
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr4 = f.goog_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr5 = f.goog_bar.get_instance_type(&f.base.reg);
    let _expr6 = f.goog_sub_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr7 = f.goog_sub_bar.get_instance_type(&f.base.reg);
    let _expr8 = f
        .goog_sub_sub_bar
        .get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr9 = f.goog_sub_sub_bar.get_instance_type(&f.base.reg);
    let _expr10 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr11 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr12 = vec![
        _expr1,
        _expr2,
        _expr3,
        _expr4,
        _expr5.unwrap(),
        _expr6,
        _expr7.unwrap(),
        _expr8,
        _expr9.unwrap(),
        _expr10,
        _expr11,
    ];
    let type_chain = _expr12;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testRecordSubtypeChain
#[test]
fn test_record_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("a", f.base.string_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let a_type = _expr2;
    let mut _expr3 = RecordTypeBuilder::new();
    builder = _expr3;
    builder.add_property("a", f.base.string_type, None);
    builder.add_property("b", f.base.string_type, None);
    let _expr4 = builder.build(&mut f.base.reg, &f.base.ast);
    let ab_type = _expr4;
    let mut _expr5 = RecordTypeBuilder::new();
    builder = _expr5;
    builder.add_property("a", f.base.string_type, None);
    builder.add_property("c", f.base.string_type, None);
    let _expr6 = builder.build(&mut f.base.reg, &f.base.ast);
    let ac_type = _expr6;
    let _expr7 = f
        .base
        .reg
        .create_union_type(&f.base.ast, &[ab_type, ac_type]);
    let ab_or_ac_type = _expr7;
    let mut _expr8 = RecordTypeBuilder::new();
    builder = _expr8;
    builder.add_property("a", f.base.string_type, None);
    builder.add_property("b", f.base.string_type, None);
    builder.add_property("c", f.base.number_type, None);
    let _expr9 = builder.build(&mut f.base.reg, &f.base.ast);
    let abc_type = _expr9;
    let _expr10 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr11 = f.base.reg.get_native_type(JSTypeNative::OBJECT_PROTOTYPE);
    let _expr12 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr13 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr14 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr15 = vec![
        _expr10,
        _expr11,
        _expr12,
        a_type,
        ab_or_ac_type,
        ab_type,
        abc_type,
        _expr13,
        _expr14,
    ];
    let type_chain = _expr15;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testRecordAndObjectChain2
#[test]
fn test_record_and_object_chain2() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("date", f.base.date_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let has_date_property = _expr2;
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr4 = f.goog_bar.get_instance_type(&f.base.reg);
    let _expr5 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr6 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr7 = vec![_expr3, has_date_property, _expr4.unwrap(), _expr5, _expr6];
    let type_chain = _expr7;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testRecordAndObjectChain3
#[test]
fn test_record_and_object_chain3() {
    let mut f = Fixture::set_up();
    let _expr1 = RecordTypeBuilder::new();
    let mut builder = _expr1;
    builder.add_property("date", f.base.unknown_type, None);
    let _expr2 = builder.build(&mut f.base.reg, &f.base.ast);
    let has_unknown_date_property = _expr2;
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr4 = f.goog_bar.get_instance_type(&f.base.reg);
    let _expr5 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr6 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr7 = vec![
        _expr3,
        has_unknown_date_property,
        _expr4.unwrap(),
        _expr5,
        _expr6,
    ];
    let type_chain = _expr7;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testNullableNamedTypeChain
#[test]
fn test_nullable_named_type_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr2 = f
        .base
        .reg
        .create_optional_nullable_type(&f.base.ast, _expr1);
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::OBJECT_PROTOTYPE);
    let _expr4 = f
        .base
        .reg
        .create_optional_nullable_type(&f.base.ast, _expr3);
    let _expr5 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr6 = f
        .base
        .reg
        .create_optional_nullable_type(&f.base.ast, _expr5);
    let _expr7 = f.goog_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr8 = f
        .base
        .reg
        .create_optional_nullable_type(&f.base.ast, _expr7);
    let _expr9 = f.goog_bar.get_instance_type(&f.base.reg);
    let _expr10 = f
        .base
        .reg
        .create_optional_nullable_type(&f.base.ast, _expr9.unwrap());
    let _expr11 = f.goog_sub_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr12 = f.base.reg.create_nullable_type(&f.base.ast, _expr11);
    let _expr13 = f.goog_sub_bar.get_instance_type(&f.base.reg);
    let _expr14 = f
        .base
        .reg
        .create_nullable_type(&f.base.ast, _expr13.unwrap());
    let _expr15 = f
        .goog_sub_sub_bar
        .get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr16 = f.goog_sub_sub_bar.get_instance_type(&f.base.reg);
    let _expr17 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr18 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr19 = vec![
        _expr2,
        _expr4,
        _expr6,
        _expr8,
        _expr10,
        _expr12,
        _expr14,
        _expr15,
        _expr16.unwrap(),
        _expr17,
        _expr18,
    ];
    let type_chain = _expr19;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testEnumTypeChain
#[test]
fn test_enum_type_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr2 = f.base.reg.get_native_type(JSTypeNative::OBJECT_PROTOTYPE);
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr4 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr5 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr6 = vec![_expr1, _expr2, _expr3, f.enum_type, _expr4, _expr5];
    let type_chain = _expr6;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testFunctionSubtypeChain
#[test]
fn test_function_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr2 = f.base.reg.get_native_type(JSTypeNative::OBJECT_PROTOTYPE);
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr4 = f.base.reg.get_native_type(JSTypeNative::FUNCTION_PROTOTYPE);
    let _expr5 = f
        .base
        .reg
        .get_native_type(JSTypeNative::GREATEST_FUNCTION_TYPE);
    let _expr6 = f
        .base
        .reg
        .get_native_type(JSTypeNative::LEAST_FUNCTION_TYPE);
    let _expr7 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr8 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr9 = vec![
        _expr1,
        _expr2,
        _expr3,
        _expr4,
        _expr5,
        f.date_method,
        _expr6,
        _expr7,
        _expr8,
    ];
    let type_chain = _expr9;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testFunctionUnionSubtypeChain
#[test]
fn test_function_union_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.object_type, f.base.string_type]);
    let _expr2 = f.base.create_union_type(&[
        f.base.greatest_function_type,
        f.goog_bar_inst,
        f.base.string_type,
    ]);
    let _expr3 = f
        .base
        .create_union_type(&[f.base.string_type, f.base.number_type]);
    let _expr4 = f.base.reg.create_function_type(&f.base.ast, _expr3, &[]);
    let _expr5 = f
        .base
        .create_union_type(&[f.base.string_type, _expr4, f.goog_bar_inst]);
    let _expr6 = f
        .base
        .reg
        .create_function_type(&f.base.ast, f.base.number_type, &[]);
    let _expr7 = f.base.create_union_type(&[_expr6, f.goog_sub_bar_inst]);
    let _expr8 = vec![
        _expr1,
        _expr2,
        _expr5,
        _expr7,
        f.base.least_function_type,
        f.base.no_object_type,
        f.base.no_type,
    ];
    let type_chain = _expr8;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testConstructorSubtypeChain
#[test]
fn test_constructor_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr2 = f.base.reg.get_native_type(JSTypeNative::OBJECT_PROTOTYPE);
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr4 = f.base.reg.get_native_type(JSTypeNative::FUNCTION_PROTOTYPE);
    let _expr5 = f.base.reg.get_native_type(JSTypeNative::FUNCTION_TYPE);
    let _expr6 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr7 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr8 = vec![_expr1, _expr2, _expr3, _expr4, _expr5, _expr6, _expr7];
    let type_chain = _expr8;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testGoogBarSubtypeChain
#[test]
fn test_goog_bar_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.get_native_type(JSTypeNative::FUNCTION_TYPE);
    let _expr2 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr3 = vec![
        _expr1,
        f.goog_bar,
        f.goog_sub_bar,
        f.goog_sub_sub_bar,
        _expr2,
    ];
    let type_chain = _expr3;
    f.verify_subtype_chain(&type_chain, false);
}

// port: JSTypeTest#testConstructorWithArgSubtypeChain
#[test]
fn test_constructor_with_arg_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr3 = f.with_open_registry(|f| {
        let _expr1 = f.base.reg.create_parameters(&[f.goog_bar]);

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("barArg".into()),
            None,
            Some(_expr1),
            None,
            None,
            false,
        )
    });
    let goog_bar_arg_constructor = _expr3;
    let _expr6 = f.with_open_registry(|f| {
        let _expr4 = f.base.reg.create_parameters(&[f.goog_sub_bar]);

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("subBarArg".into()),
            None,
            Some(_expr4),
            None,
            None,
            false,
        )
    });
    let goog_sub_bar_arg_constructor = _expr6;
    let _expr7 = f.base.reg.get_native_type(JSTypeNative::FUNCTION_TYPE);
    let _expr8 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr9 = vec![
        _expr7,
        goog_bar_arg_constructor,
        goog_sub_bar_arg_constructor,
        _expr8,
    ];
    let type_chain = _expr9;
    f.verify_subtype_chain(&type_chain, false);
}

// port: JSTypeTest#testInterfaceInstanceSubtypeChain
#[test]
fn test_interface_instance_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f.goog_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr2 = f.goog_sub_bar.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr4 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr5 = vec![
        f.base.all_type,
        f.base.object_type,
        f.interface_inst_type,
        _expr1,
        f.goog_bar_inst,
        _expr2,
        f.goog_sub_bar_inst,
        _expr3,
        _expr4,
    ];
    let type_chain = _expr5;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testInterfaceInheritanceSubtypeChain
#[test]
fn test_interface_inheritance_subtype_chain() {
    let mut f = Fixture::set_up();
    let _expr2 = f.with_open_registry(|f| {
        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("goog.TempType".into()),
            None,
            None,
            None,
            None,
            false,
        )
    });
    let temp_type = _expr2;
    let _expr3 = vec![f.sub_interface_inst_type];
    temp_type.set_implemented_interfaces(&mut f.base.reg, &f.base.ast, _expr3.clone());
    let _expr4 = temp_type.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr5 = temp_type.get_instance_type(&f.base.reg);
    let _expr6 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr7 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr8 = vec![
        f.base.all_type,
        f.base.object_type,
        f.interface_inst_type,
        f.sub_interface_inst_type,
        _expr4,
        _expr5.unwrap(),
        _expr6,
        _expr7,
    ];
    let type_chain = _expr8;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testAnonymousObjectChain
#[test]
fn test_anonymous_object_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.create_nullable_type(f.base.object_type);
    let _expr2 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let _expr3 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr4 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr5 = vec![
        f.base.all_type,
        _expr1,
        f.base.object_type,
        _expr2,
        _expr3,
        _expr4,
    ];
    let type_chain = _expr5;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testAnonymousEnumElementChain
#[test]
fn test_anonymous_enum_element_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = EnumTypeBuilder::new();
    let _expr2 = _expr1.set_name("typeB");
    let _expr3 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let _expr4 = _expr2.set_element_type(_expr3);
    let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
    let _expr6 = _expr5.get_elements_type(&f.base.reg);
    let enum_elem_type = _expr6;
    let _expr7 = f.base.create_nullable_type(f.base.object_type);
    let _expr8 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr9 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr10 = vec![
        f.base.all_type,
        _expr7,
        f.base.object_type,
        enum_elem_type,
        _expr8,
        _expr9,
    ];
    let type_chain = _expr10;
    f.verify_subtype_chain_default(&type_chain);
}

// port: JSTypeTest#testTemplatizedArrayChain
#[test]
fn test_templatized_array_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.no_type]);
    let array_of_no_type = _expr1;
    let _expr2 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.string_type]);
    let array_of_string = _expr2;
    let _expr3 = f
        .base
        .create_union_type(&[f.base.string_type, f.base.number_type]);
    let _expr4 = f.base.create_templatized_type(f.base.array_type, &[_expr3]);
    let array_of_string_or_number = _expr4;
    let _expr5 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.all_type]);
    let array_of_all_type = _expr5;
    let _expr6 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr7 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr8 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr9 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr10 = vec![
        _expr6,
        _expr7,
        array_of_all_type,
        array_of_string_or_number,
        array_of_string,
        array_of_no_type,
        _expr8,
        _expr9,
    ];
    let type_chain = _expr10;
    f.verify_subtype_chain(&type_chain, false);
}

// port: JSTypeTest#testTemplatizedArrayChain2
#[test]
fn test_templatized_array_chain2() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.no_type]);
    let array_of_no_type = _expr1;
    let _expr2 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.no_object_type]);
    let array_of_no_object_type = _expr2;
    let _expr3 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.array_type]);
    let array_of_array = _expr3;
    let _expr4 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.object_type]);
    let array_of_object = _expr4;
    let _expr5 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.all_type]);
    let array_of_all_type = _expr5;
    let _expr6 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr7 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr8 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr9 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr10 = vec![
        _expr6,
        _expr7,
        array_of_all_type,
        array_of_object,
        array_of_array,
        array_of_no_object_type,
        array_of_no_type,
        _expr8,
        _expr9,
    ];
    let type_chain = _expr10;
    f.verify_subtype_chain(&type_chain, false);
}

// port: JSTypeTest#testTemplatizedObjectChain
#[test]
fn test_templatized_object_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_templatized_type(f.base.object_type, &[f.base.no_type]);
    let object_of_no_type = _expr1;
    let _expr2 = f
        .base
        .create_templatized_type(f.base.object_type, &[f.base.string_type]);
    let object_of_string = _expr2;
    let _expr3 = f
        .base
        .create_union_type(&[f.base.string_type, f.base.number_type]);
    let _expr4 = f
        .base
        .create_templatized_type(f.base.object_type, &[_expr3]);
    let object_of_string_or_number = _expr4;
    let _expr5 = f
        .base
        .create_templatized_type(f.base.object_type, &[f.base.all_type]);
    let object_of_all_type = _expr5;
    let _expr6 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr7 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr8 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr9 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr10 = vec![
        _expr6,
        _expr7,
        object_of_all_type,
        object_of_string_or_number,
        object_of_string,
        object_of_no_type,
        _expr8,
        _expr9,
    ];
    let type_chain = _expr10;
    f.verify_subtype_chain(&type_chain, false);
}

// port: JSTypeTest#testMixedTemplatizedTypeChain
#[test]
fn test_mixed_templatized_type_chain() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.no_type]);
    let array_of_no_type = _expr1;
    let _expr2 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.string_type]);
    let array_of_string = _expr2;
    let _expr3 = f
        .base
        .create_templatized_type(f.base.object_type, &[f.base.string_type]);
    let object_of_string = _expr3;
    let _expr4 = f
        .base
        .create_union_type(&[f.base.string_type, f.base.number_type]);
    let _expr5 = f
        .base
        .create_templatized_type(f.base.object_type, &[_expr4]);
    let object_of_string_or_number = _expr5;
    let _expr6 = f
        .base
        .create_templatized_type(f.base.object_type, &[f.base.all_type]);
    let object_of_all_type = _expr6;
    let _expr7 = f.base.reg.get_native_type(JSTypeNative::ALL_TYPE);
    let _expr8 = f.base.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
    let _expr9 = f.base.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
    let _expr10 = f.base.reg.get_native_type(JSTypeNative::NO_TYPE);
    let _expr11 = vec![
        _expr7,
        _expr8,
        object_of_all_type,
        object_of_string_or_number,
        object_of_string,
        array_of_string,
        array_of_no_type,
        _expr9,
        _expr10,
    ];
    let type_chain = _expr11;
    f.verify_subtype_chain(&type_chain, false);
}

// port: JSTypeTest#testTemplatizedTypeSubtypes
#[test]
fn test_templatized_type_subtypes() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_templatized_type(f.base.object_type, &[f.base.string_type]);
    let object_of_string = _expr1;
    let _expr2 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.string_type]);
    let array_of_string = _expr2;
    let _expr3 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.number_type]);
    let array_of_number = _expr3;
    let _expr4 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.unknown_type]);
    let array_of_unknown = _expr4;
    let _expr5 = object_of_string.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr5);
    let _expr6 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, object_of_string);
    assert!(_expr6);
    let _expr7 = object_of_string.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr7);
    let _expr8 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, object_of_string);
    assert!(_expr8);
    let _expr9 = array_of_string.is_subtype_of(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(_expr9);
    let _expr10 = f
        .base
        .array_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, array_of_string);
    assert!(_expr10);
    let _expr11 = array_of_string.is_subtype_of(&mut f.base.reg, &f.base.ast, array_of_unknown);
    assert!(_expr11);
    let _expr12 = array_of_unknown.is_subtype_of(&mut f.base.reg, &f.base.ast, array_of_string);
    assert!(_expr12);
    let _expr13 = array_of_string.is_subtype_of(&mut f.base.reg, &f.base.ast, array_of_number);
    assert!(!_expr13);
    let _expr14 = array_of_number.is_subtype_of(&mut f.base.reg, &f.base.ast, array_of_string);
    assert!(!_expr14);
    let _expr15 = f
        .base
        .create_union_type(&[array_of_number, f.base.null_void]);
    let _expr16 = array_of_number.is_subtype_of(&mut f.base.reg, &f.base.ast, _expr15);
    assert!(_expr16);
    let _expr17 = f
        .base
        .create_union_type(&[array_of_number, f.base.null_void]);
    let _expr18 = _expr17.is_subtype_of(&mut f.base.reg, &f.base.ast, array_of_number);
    assert!(!_expr18);
    let _expr19 = f
        .base
        .create_union_type(&[array_of_number, f.base.null_void]);
    let _expr20 = array_of_string.is_subtype_of(&mut f.base.reg, &f.base.ast, _expr19);
    assert!(!_expr20);
}

// port: JSTypeTest#testTemplatizedTypeRelations
#[test]
fn test_templatized_type_relations() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_templatized_type(f.base.object_type, &[f.base.string_type]);
    let object_of_string = _expr1;
    let _expr2 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.string_type]);
    let array_of_string = _expr2;
    let _expr3 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.number_type]);
    let array_of_number = _expr3;
    let _expr4 =
        array_of_string.get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr5 =
        f.base
            .object_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, array_of_string);
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    let _expr6 =
        array_of_string.get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.array_type);
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.array_type);
    let _expr7 =
        f.base
            .array_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, array_of_string);
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.array_type);
    let _expr8 =
        object_of_string.get_least_supertype(&mut f.base.reg, &f.base.ast, f.base.array_type);
    let _expr9 = _expr8.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr9, "(Array|Object<string,?>)");
    let _expr10 =
        f.base
            .array_type
            .get_least_supertype(&mut f.base.reg, &f.base.ast, object_of_string);
    let _expr11 = _expr10.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr11, "(Array|Object<string,?>)");
    let _expr12 =
        array_of_string.get_least_supertype(&mut f.base.reg, &f.base.ast, array_of_number);
    TypeSubject::assert_type(_expr12).to_string_is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        "Array<?>",
    );
    let _expr13 =
        array_of_number.get_least_supertype(&mut f.base.reg, &f.base.ast, array_of_string);
    TypeSubject::assert_type(_expr13).to_string_is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        "Array<?>",
    );
    let _expr14 =
        array_of_string.get_least_supertype(&mut f.base.reg, &f.base.ast, array_of_string);
    TypeSubject::assert_type(_expr14).is_equal_to(&mut f.base.reg, &f.base.ast, array_of_string);
    let _expr15 =
        object_of_string.get_least_supertype(&mut f.base.reg, &f.base.ast, array_of_string);
    let _expr16 = _expr15.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr16, "(Array<string>|Object<string,?>)");
    let _expr17 =
        array_of_string.get_least_supertype(&mut f.base.reg, &f.base.ast, object_of_string);
    let _expr18 = _expr17.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr18, "(Array<string>|Object<string,?>)");
    let _expr19 =
        f.base
            .object_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, object_of_string);
    TypeSubject::assert_type(_expr19).is_equal_to(&mut f.base.reg, &f.base.ast, object_of_string);
    let _expr20 =
        object_of_string.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(_expr20).is_equal_to(&mut f.base.reg, &f.base.ast, object_of_string);
    let _expr21 =
        object_of_string.get_greatest_subtype(&mut f.base.reg, &f.base.ast, f.base.array_type);
    TypeSubject::assert_type(_expr21).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.array_type);
    let _expr22 =
        object_of_string.get_greatest_subtype(&mut f.base.reg, &f.base.ast, array_of_string);
    TypeSubject::assert_type(array_of_string).is_equal_to(&mut f.base.reg, &f.base.ast, _expr22);
    let _expr23 =
        f.base
            .object_type
            .get_greatest_subtype(&mut f.base.reg, &f.base.ast, array_of_string);
    TypeSubject::assert_type(array_of_string).is_equal_to(&mut f.base.reg, &f.base.ast, _expr23);
}

// port: JSTypeTest#testTemplatizedTypesWithBoundedGenerics
#[test]
fn test_templatized_types_with_bounded_generics() {
    let mut f = Fixture::set_up();
    let _expr5 = f.with_open_registry(|f| {
        let _expr1 =
            f.base
                .reg
                .create_template_type_with_bound(&f.base.ast, "A", f.base.number_type);
        let _expr2 =
            f.base
                .reg
                .create_template_type_with_bound(&f.base.ast, "B", f.base.string_type);
        let _expr3 = vec![_expr1, _expr2];

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("TestingType".into()),
            None,
            None,
            Some(f.base.unknown_type),
            Some(_expr3),
            false,
        )
    });
    let templatized_ctor = _expr5;
    let _expr6 = templatized_ctor.get_instance_type(&f.base.reg);
    let _expr7 = vec![f.base.number_type];
    let _expr8 = f
        .base
        .reg
        .create_templatized_type(&f.base.ast, _expr6.unwrap(), &_expr7);
    let templatized_instance = _expr8;
    let _expr9 = templatized_ctor.get_template_type_map(&f.base.reg);
    let ctr_type_map = _expr9;
    let _expr10 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "A");
    let key_a = _expr10;
    assert!(key_a.is_some());
    let _expr11 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "B");
    let key_b = _expr11;
    assert!(key_b.is_some());
    let _expr12 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "C");
    let key_c = _expr12;
    assert!(key_c.is_none());
    let _expr13 = f.base.reg.create_template_type(&f.base.ast, "C");
    let unknown_key = _expr13;
    let _expr14 = templatized_instance.get_template_type_map(&f.base.reg);
    let template_type_map = _expr14;
    let _expr15 = template_type_map.has_template_key(key_a.unwrap());
    assert!(_expr15);
    let _expr16 = template_type_map.has_template_key(key_b.unwrap());
    assert!(_expr16);
    let _expr17 = template_type_map.has_template_key(unknown_key);
    assert!(!_expr17);
    let _expr18 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, key_a.unwrap());
    TypeSubject::assert_type(_expr18).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr19 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, key_b.unwrap());
    TypeSubject::assert_type(_expr19).is_equal_to(&mut f.base.reg, &f.base.ast, key_b);
    let _expr20 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, unknown_key);
    TypeSubject::assert_type(_expr20).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
}

// port: JSTypeTest#testRestrictedTypeGivenOutcome
#[test]
fn test_restricted_type_given_outcome() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.boolean_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr1).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    let _expr2 = f.base.boolean_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    let _expr3 = f.base.null_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr4 = f.base.null_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr5 = f.base.number_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr6 = f.base.number_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr7 = f.base.string_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let _expr8 = f.base.string_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr8).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let _expr9 = f.base.string_object_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr9).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    let _expr10 = f.base.string_object_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr10).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr11 = f.base.void_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr11).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr12 = f.base.void_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr12).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.void_type);
    let _expr13 = f.base.no_object_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr13).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.no_object_type,
    );
    let _expr14 = f.base.no_object_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr14).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr15 = f.base.no_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr15).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr16 = f.base.no_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr16).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr17 = f.base.all_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr17).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr18 = f.base.all_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr18).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr19 = f.base.unknown_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr19).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.checked_unknown_type,
    );
    let _expr20 = f.base.unknown_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr20).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr21 = f.base.create_nullable_type(f.base.string_type);
    let nullable_string_value = _expr21;
    let _expr22 = nullable_string_value.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr22).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let _expr23 = nullable_string_value.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr23).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        nullable_string_value,
    );
    let _expr24 = f.base.create_nullable_type(f.base.string_object_type);
    let nullable_string_object = _expr24;
    let _expr25 = nullable_string_object.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::TRUE,
    );
    TypeSubject::assert_type(_expr25).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.string_object_type,
    );
    let _expr26 = nullable_string_object.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE,
    );
    TypeSubject::assert_type(_expr26).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
}

// port: JSTypeTest#nullishOutcomeGetRestrictedTypeGivenOutcome
#[test]
fn nullish_outcome_get_restricted_type_given_outcome() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.boolean_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr1).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr2 = f.base.null_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr3 = f.base.number_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr4 = f.base.string_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr5 = f.base.string_object_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr6 = f.base.void_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.void_type);
    let _expr7 = f.base.no_object_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr8 = f.base.no_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr8).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr9 = f.base.all_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr9).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr10 = f.base.unknown_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr10).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr11 = f.base.create_nullable_type(f.base.string_type);
    let nullable_string_value = _expr11;
    let _expr12 = nullable_string_value.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr12).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr13 = f.base.create_nullable_type(f.base.string_object_type);
    let nullable_string_object = _expr13;
    let _expr14 = nullable_string_object.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::NULLISH,
    );
    TypeSubject::assert_type(_expr14).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
}

// port: JSTypeTest#falseNotNullOutcomeGetRestrictedTypeGivenOutcome
#[test]
fn false_not_null_outcome_get_restricted_type_given_outcome() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.boolean_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr1).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    let _expr2 = f.base.null_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr3 = f.base.number_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr3).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr4 = f.base.string_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let _expr5 = f.base.string_object_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr5).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr6 = f.base.void_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.void_type);
    let _expr7 = f.base.no_object_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr7).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr8 = f.base.no_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr8).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.no_type);
    let _expr9 = f.base.all_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr9).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.all_type);
    let _expr10 = f.base.unknown_type.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr10).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr11 = f.base.create_nullable_type(f.base.string_type);
    let nullable_string_value = _expr11;
    let _expr12 = nullable_string_value.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr12).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        nullable_string_value,
    );
    let _expr13 = f.base.create_nullable_type(f.base.string_object_type);
    let nullable_string_object = _expr13;
    let _expr14 = nullable_string_object.get_restricted_type_given_outcome(
        &mut f.base.reg,
        &f.base.ast,
        Outcome::FALSE_NOT_NULL,
    );
    TypeSubject::assert_type(_expr14).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
}

// port: JSTypeTest#testGoodSetPrototypeBasedOn
#[test]
fn test_good_set_prototype_based_on() {
    let mut f = Fixture::set_up();
    let _expr2 = f.with_open_registry(|f| {
        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("fun".into()),
            None,
            None,
            None,
            None,
            false,
        )
    });
    let fun = _expr2;
    fun.set_prototype_based_on(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    let _expr3 = fun.get_instance_type(&f.base.reg);
    let _expr4 = _expr3
        .unwrap()
        .is_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(_expr4);
}

// port: JSTypeTest#testLateSetPrototypeBasedOn
#[test]
fn test_late_set_prototype_based_on() {
    let mut f = Fixture::set_up();
    let _expr2 = f.with_open_registry(|f| {
        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("fun".into()),
            None,
            None,
            None,
            None,
            false,
        )
    });
    let fun = _expr2;
    let _expr3 = fun.get_instance_type(&f.base.reg);
    let _expr4 = _expr3
        .unwrap()
        .is_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(!_expr4);
    fun.set_prototype_based_on(&mut f.base.reg, &f.base.ast, f.unresolved_named_type);
    let _expr5 = fun.get_instance_type(&f.base.reg);
    let _expr6 = _expr5
        .unwrap()
        .is_unknown_type(&mut f.base.reg, &f.base.ast);
    assert!(_expr6);
}

// port: JSTypeTest#testGetTypeUnderEquality1
#[test]
fn test_get_type_under_equality1() {
    let mut f = Fixture::set_up();
    for type_ in f.types.clone() {
        f.test_get_type_under_equality(type_, type_, type_, type_);
    }
}

// port: JSTypeTest#testGetTypesUnderEquality2
#[test]
fn test_get_types_under_equality2() {
    let mut f = Fixture::set_up();
    f.test_get_type_under_equality(
        f.base.number_type,
        f.base.object_type,
        f.base.number_type,
        f.base.object_type,
    );
}

// port: JSTypeTest#testGetTypesUnderEquality3
#[test]
fn test_get_types_under_equality3() {
    let mut f = Fixture::set_up();
    f.test_get_type_under_equality(
        f.base.null_type,
        f.base.void_type,
        f.base.null_type,
        f.base.void_type,
    );
}

// port: JSTypeTest#testGetTypesUnderEquality4
#[test]
fn test_get_types_under_equality4() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let string_number = _expr1;
    f.test_get_type_under_equality(
        string_number,
        f.base.string_type,
        string_number,
        f.base.string_type,
    );
    f.test_get_type_under_equality(
        string_number,
        f.base.number_type,
        string_number,
        f.base.number_type,
    );
}

// port: JSTypeTest#testGetTypesUnderEquality5
#[test]
fn test_get_types_under_equality5() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.void_type, f.base.null_type]);
    let null_undefined = _expr1;
    f.test_get_type_under_equality(
        null_undefined,
        f.base.null_type,
        null_undefined,
        f.base.null_type,
    );
    f.test_get_type_under_equality(
        null_undefined,
        f.base.void_type,
        null_undefined,
        f.base.void_type,
    );
}

// port: JSTypeTest#testGetTypesUnderEquality6
#[test]
fn test_get_types_under_equality6() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.base
            .create_union_type(&[f.base.void_type, f.base.null_type, f.base.number_type]);
    let opt_null_number = _expr1;
    let _expr2 = f
        .base
        .create_union_type(&[f.base.null_type, f.base.void_type]);
    f.test_get_type_under_equality(opt_null_number, f.base.null_type, _expr2, f.base.null_type);
}

// port: JSTypeTest#testGetTypesUnderInequality1
#[test]
fn test_get_types_under_inequality1() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.object_type]);
    let number_object = _expr1;
    f.test_get_types_under_inequality(
        number_object,
        f.base.number_type,
        number_object,
        f.base.number_type,
    );
    f.test_get_types_under_inequality(
        number_object,
        f.base.object_type,
        number_object,
        f.base.object_type,
    );
}

// port: JSTypeTest#testGetTypesUnderInequality2
#[test]
fn test_get_types_under_inequality2() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.void_type, f.base.null_type]);
    let null_undefined = _expr1;
    f.test_get_types_under_inequality(
        null_undefined,
        f.base.null_type,
        f.base.no_type,
        f.base.no_type,
    );
    f.test_get_types_under_inequality(
        null_undefined,
        f.base.void_type,
        f.base.no_type,
        f.base.no_type,
    );
}

// port: JSTypeTest#testGetTypesUnderInequality3
#[test]
fn test_get_types_under_inequality3() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.number_type, f.base.string_type]);
    let string_number = _expr1;
    f.test_get_types_under_inequality(
        string_number,
        f.base.number_type,
        string_number,
        f.base.number_type,
    );
    f.test_get_types_under_inequality(
        string_number,
        f.base.string_type,
        string_number,
        f.base.string_type,
    );
}

// port: JSTypeTest#testGetTypesUnderInequality4
#[test]
fn test_get_types_under_inequality4() {
    let mut f = Fixture::set_up();
    let _expr1 =
        f.base
            .create_union_type(&[f.base.null_type, f.base.void_type, f.base.number_type]);
    let nullable_optional_number = _expr1;
    f.test_get_types_under_inequality(
        nullable_optional_number,
        f.base.null_type,
        f.base.number_type,
        f.base.null_type,
    );
}

// port: JSTypeTest#testCreateOptionalType
#[test]
fn test_create_optional_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .reg
        .create_optional_type(&f.base.ast, f.base.number_type);
    let opt_number = _expr1;
    f.assert_union_contains(opt_number, f.base.number_type);
    f.assert_union_contains(opt_number, f.base.void_type);
    let _expr2 = f
        .base
        .create_union_type(&[f.base.string_object_type, f.base.date_type]);
    let _expr3 = f.base.reg.create_optional_type(&f.base.ast, _expr2);
    let opt_union = _expr3;
    f.assert_union_contains(opt_union, f.base.date_type);
    f.assert_union_contains(opt_union, f.base.string_object_type);
    f.assert_union_contains(opt_union, f.base.void_type);
}

// port: JSTypeTest#testCreateAnonymousObjectType
#[test]
fn test_create_anonymous_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let anonymous = _expr1;
    let _expr2 = anonymous.get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr2).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    TypeSubject::assert_type(anonymous).get_reference_name_is_null(&f.base.reg);
    let _expr3 = anonymous.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr3, "{}");
}

// port: JSTypeTest#testCreateAnonymousObjectType2
#[test]
fn test_create_anonymous_object_type2() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let anonymous = _expr1;
    let _expr2 = anonymous.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "a",
        f.base.number_type,
        None,
    );
    let _expr3 = anonymous.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "b",
        f.base.number_type,
        None,
    );
    let _expr4 = anonymous.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "c",
        f.base.number_type,
        None,
    );
    let _expr5 = anonymous.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "d",
        f.base.number_type,
        None,
    );
    let _expr6 = anonymous.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "e",
        f.base.number_type,
        None,
    );
    let _expr7 = anonymous.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "f",
        f.base.number_type,
        None,
    );
    let _expr8 = anonymous.to_string(&mut f.base.reg, &f.base.ast);
    let _expr9 = BaseJSTypeTestCase::lines(&[
        "{",
        "  a: number,",
        "  b: number,",
        "  c: number,",
        "  d: number,",
        "  e: number,",
        "  f: number",
        "}",
    ]);
    assert_eq!(_expr8, _expr9);
}

// port: JSTypeTest#testCreateObjectType
#[test]
fn test_create_object_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .date_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr2 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(_expr1)
        .build(&mut f.base.reg, &f.base.ast);
    let sub_date = _expr2;
    let _expr3 = f
        .base
        .date_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr4 = sub_date.get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr4).is_equal_to(&mut f.base.reg, &f.base.ast, _expr3);
    TypeSubject::assert_type(sub_date).get_reference_name_is_null(&f.base.reg);
    let _expr5 = sub_date.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr5, "{...}");
    let _expr6 = f
        .base
        .array_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr7 = f.base.reg.create_object_type(&f.base.ast, "Foo", _expr6);
    let sub_array = _expr7;
    let _expr8 = f
        .base
        .array_type
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr9 = sub_array.get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr9).is_equal_to(&mut f.base.reg, &f.base.ast, _expr8);
    TypeSubject::assert_type(sub_array).get_reference_name_is_equal_to(&f.base.reg, "Foo");
}

// port: JSTypeTest#testBug903110
#[test]
fn test_bug903110() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .create_union_type(&[f.base.function_type, f.base.void_type]);
    let union = _expr1;
    let _expr2 = f
        .base
        .void_type
        .is_subtype_of(&mut f.base.reg, &f.base.ast, union);
    assert!(_expr2);
    let _expr3 = f
        .base
        .function_type
        .is_subtype(&mut f.base.reg, &f.base.ast, union);
    assert!(_expr3);
    let _expr4 = union.is_subtype(&mut f.base.reg, &f.base.ast, union);
    assert!(_expr4);
}

// port: JSTypeTest#testHasOwnProperty
#[test]
fn test_has_own_property() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let _expr2 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(_expr1))
        .build(&mut f.base.reg, &f.base.ast);
    let sup = _expr2;
    let _expr3 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(sup))
        .build(&mut f.base.reg, &f.base.ast);
    let sub = _expr3;
    let _expr4 = f.get_bottom_type();
    let _expr5 = sup.define_property(&mut f.base.reg, &f.base.ast, "base", _expr4, false, None);
    let _expr6 = f.get_bottom_type();
    let _expr7 = sub.define_property(&mut f.base.reg, &f.base.ast, "sub", _expr6, false, None);
    let _expr8 = sup.has_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(_expr8);
    let _expr9 = sup.has_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(!_expr9);
    let _expr10 = sup.has_own_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(_expr10);
    let _expr11 = sup.has_own_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(!_expr11);
    let _expr12 = sup.has_own_property(&mut f.base.reg, &f.base.ast, "none");
    assert!(!_expr12);
    let _expr13 = sub.has_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(_expr13);
    let _expr14 = sub.has_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(_expr14);
    let _expr15 = sub.has_own_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(!_expr15);
    let _expr16 = sub.has_own_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(_expr16);
    let _expr17 = sub.has_own_property(&mut f.base.reg, &f.base.ast, "none");
    assert!(!_expr17);
}

// port: JSTypeTest#testHasOwnProperty_symbol
#[test]
fn test_has_own_property_symbol() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let _expr2 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(_expr1))
        .build(&mut f.base.reg, &f.base.ast);
    let sup = _expr2;
    let _expr3 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(sup))
        .build(&mut f.base.reg, &f.base.ast);
    let sub = _expr3;
    let _expr4 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "base",
    );
    let _expr5 = PropertyKey::Symbol(_expr4);
    let base_symbol = _expr5;
    let _expr6 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "sub",
    );
    let _expr7 = PropertyKey::Symbol(_expr6);
    let sub_symbol = _expr7;
    let _expr8 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "none",
    );
    let _expr9 = PropertyKey::Symbol(_expr8);
    let none_symbol = _expr9;
    let _expr10 = f.get_bottom_type();
    let _expr11 = sup.define_property(
        &mut f.base.reg,
        &f.base.ast,
        base_symbol.clone(),
        _expr10,
        false,
        None,
    );
    let _expr12 = f.get_bottom_type();
    let _expr13 = sub.define_property(
        &mut f.base.reg,
        &f.base.ast,
        sub_symbol.clone(),
        _expr12,
        false,
        None,
    );
    let _expr14 = sup.has_property(&mut f.base.reg, &f.base.ast, base_symbol.clone());
    assert!(_expr14);
    let _expr15 = sup.has_property(&mut f.base.reg, &f.base.ast, sub_symbol.clone());
    assert!(!_expr15);
    let _expr16 = sup.has_own_property(&mut f.base.reg, &f.base.ast, base_symbol.clone());
    assert!(_expr16);
    let _expr17 = sup.has_own_property(&mut f.base.reg, &f.base.ast, sub_symbol.clone());
    assert!(!_expr17);
    let _expr18 = sup.has_own_property(&mut f.base.reg, &f.base.ast, none_symbol.clone());
    assert!(!_expr18);
    let _expr19 = sup.has_own_property(&mut f.base.reg, &f.base.ast, "none");
    assert!(!_expr19);
    let _expr20 = sub.has_property(&mut f.base.reg, &f.base.ast, base_symbol.clone());
    assert!(_expr20);
    let _expr21 = sub.has_property(&mut f.base.reg, &f.base.ast, sub_symbol.clone());
    assert!(_expr21);
    let _expr22 = sub.has_own_property(&mut f.base.reg, &f.base.ast, base_symbol.clone());
    assert!(!_expr22);
    let _expr23 = sub.has_own_property(&mut f.base.reg, &f.base.ast, sub_symbol.clone());
    assert!(_expr23);
    let _expr24 = sub.has_own_property(&mut f.base.reg, &f.base.ast, none_symbol.clone());
    assert!(!_expr24);
    let _expr25 = sub.has_own_property(&mut f.base.reg, &f.base.ast, "none");
    assert!(!_expr25);
}

// port: JSTypeTest#testNamedTypeHasOwnProperty
#[test]
fn test_named_type_has_own_property() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .named_goog_bar
        .get_implicit_prototype(&mut f.base.reg, &f.base.ast);
    let _expr2 = f.get_bottom_type();
    let _expr3 =
        _expr1
            .unwrap()
            .define_property(&mut f.base.reg, &f.base.ast, "base", _expr2, false, None);
    let _expr4 = f.get_bottom_type();
    let _expr5 =
        f.named_goog_bar
            .define_property(&mut f.base.reg, &f.base.ast, "sub", _expr4, false, None);
    let _expr6 = f
        .named_goog_bar
        .has_own_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(!_expr6);
    let _expr7 = f
        .named_goog_bar
        .has_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(_expr7);
    let _expr8 = f
        .named_goog_bar
        .has_own_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(_expr8);
    let _expr9 = f
        .named_goog_bar
        .has_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(_expr9);
}

// port: JSTypeTest#testInterfaceHasOwnProperty
#[test]
fn test_interface_has_own_property() {
    let mut f = Fixture::set_up();
    let _expr1 = f.get_bottom_type();
    let _expr2 = f.interface_inst_type.define_property(
        &mut f.base.reg,
        &f.base.ast,
        "base",
        _expr1,
        false,
        None,
    );
    let _expr3 = f.get_bottom_type();
    let _expr4 = f.sub_interface_inst_type.define_property(
        &mut f.base.reg,
        &f.base.ast,
        "sub",
        _expr3,
        false,
        None,
    );
    let _expr5 = f
        .interface_inst_type
        .has_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(_expr5);
    let _expr6 = f
        .interface_inst_type
        .has_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(!_expr6);
    let _expr7 = f
        .interface_inst_type
        .has_own_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(_expr7);
    let _expr8 = f
        .interface_inst_type
        .has_own_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(!_expr8);
    let _expr9 = f
        .interface_inst_type
        .has_own_property(&mut f.base.reg, &f.base.ast, "none");
    assert!(!_expr9);
    let _expr10 = f
        .sub_interface_inst_type
        .has_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(_expr10);
    let _expr11 = f
        .sub_interface_inst_type
        .has_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(_expr11);
    let _expr12 = f
        .sub_interface_inst_type
        .has_own_property(&mut f.base.reg, &f.base.ast, "base");
    assert!(!_expr12);
    let _expr13 = f
        .sub_interface_inst_type
        .has_own_property(&mut f.base.reg, &f.base.ast, "sub");
    assert!(_expr13);
    let _expr14 = f
        .sub_interface_inst_type
        .has_own_property(&mut f.base.reg, &f.base.ast, "none");
    assert!(!_expr14);
}

// port: JSTypeTest#testInterfaceHasOwnProperty_symbol
#[test]
fn test_interface_has_own_property_symbol() {
    let mut f = Fixture::set_up();
    let _expr1 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "base",
    );
    let _expr2 = PropertyKey::Symbol(_expr1);
    let base_symbol = _expr2;
    let _expr3 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "sub",
    );
    let _expr4 = PropertyKey::Symbol(_expr3);
    let sub_symbol = _expr4;
    let _expr5 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "none",
    );
    let _expr6 = PropertyKey::Symbol(_expr5);
    let none_symbol = _expr6;
    let _expr7 = f.get_bottom_type();
    let _expr8 = f.interface_inst_type.define_property(
        &mut f.base.reg,
        &f.base.ast,
        base_symbol.clone(),
        _expr7,
        false,
        None,
    );
    let _expr9 = f.get_bottom_type();
    let _expr10 = f.sub_interface_inst_type.define_property(
        &mut f.base.reg,
        &f.base.ast,
        sub_symbol.clone(),
        _expr9,
        false,
        None,
    );
    let _expr11 =
        f.interface_inst_type
            .has_property(&mut f.base.reg, &f.base.ast, base_symbol.clone());
    assert!(_expr11);
    let _expr12 =
        f.interface_inst_type
            .has_property(&mut f.base.reg, &f.base.ast, sub_symbol.clone());
    assert!(!_expr12);
    let _expr13 =
        f.interface_inst_type
            .has_own_property(&mut f.base.reg, &f.base.ast, base_symbol.clone());
    assert!(_expr13);
    let _expr14 =
        f.interface_inst_type
            .has_own_property(&mut f.base.reg, &f.base.ast, sub_symbol.clone());
    assert!(!_expr14);
    let _expr15 =
        f.interface_inst_type
            .has_own_property(&mut f.base.reg, &f.base.ast, none_symbol.clone());
    assert!(!_expr15);
    let _expr16 = f
        .interface_inst_type
        .has_own_property(&mut f.base.reg, &f.base.ast, "none");
    assert!(!_expr16);
    let _expr17 =
        f.sub_interface_inst_type
            .has_property(&mut f.base.reg, &f.base.ast, base_symbol.clone());
    assert!(_expr17);
    let _expr18 =
        f.sub_interface_inst_type
            .has_property(&mut f.base.reg, &f.base.ast, sub_symbol.clone());
    assert!(_expr18);
    let _expr19 = f.sub_interface_inst_type.has_own_property(
        &mut f.base.reg,
        &f.base.ast,
        base_symbol.clone(),
    );
    assert!(!_expr19);
    let _expr20 = f.sub_interface_inst_type.has_own_property(
        &mut f.base.reg,
        &f.base.ast,
        sub_symbol.clone(),
    );
    assert!(_expr20);
    let _expr21 = f.sub_interface_inst_type.has_own_property(
        &mut f.base.reg,
        &f.base.ast,
        none_symbol.clone(),
    );
    assert!(!_expr21);
    let _expr22 = f
        .sub_interface_inst_type
        .has_own_property(&mut f.base.reg, &f.base.ast, "none");
    assert!(!_expr22);
}

// port: JSTypeTest#testGetPropertyNames
#[test]
fn test_get_property_names() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let _expr2 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(_expr1))
        .build(&mut f.base.reg, &f.base.ast);
    let sup = _expr2;
    let _expr3 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(sup))
        .build(&mut f.base.reg, &f.base.ast);
    let sub = _expr3;
    let _expr4 = f.get_bottom_type();
    let bottom_type = _expr4;
    let _expr5 = sup.define_property(
        &mut f.base.reg,
        &f.base.ast,
        "base",
        bottom_type,
        false,
        None,
    );
    let _expr6 = sub.define_property(
        &mut f.base.reg,
        &f.base.ast,
        "sub",
        bottom_type,
        false,
        None,
    );
    let _expr7 = sub.get_property_names(&mut f.base.reg, &f.base.ast);
    let _expr8 = vec![
        JsString::from("isPrototypeOf"),
        JsString::from("toLocaleString"),
        JsString::from("propertyIsEnumerable"),
        JsString::from("toString"),
        JsString::from("valueOf"),
        JsString::from("hasOwnProperty"),
        JsString::from("constructor"),
        JsString::from("base"),
        JsString::from("sub"),
    ];
    assert_eq!(
        _expr7
            .into_iter()
            .collect::<std::collections::BTreeSet<JsString>>(),
        _expr8
            .into_iter()
            .collect::<std::collections::BTreeSet<JsString>>()
    );
    let _expr9 = sup.get_property_names(&mut f.base.reg, &f.base.ast);
    let _expr10 = vec![
        JsString::from("isPrototypeOf"),
        JsString::from("toLocaleString"),
        JsString::from("propertyIsEnumerable"),
        JsString::from("toString"),
        JsString::from("valueOf"),
        JsString::from("hasOwnProperty"),
        JsString::from("constructor"),
        JsString::from("base"),
    ];
    assert_eq!(
        _expr9
            .into_iter()
            .collect::<std::collections::BTreeSet<JsString>>(),
        _expr10
            .into_iter()
            .collect::<std::collections::BTreeSet<JsString>>()
    );
    let _expr11 = f
        .base
        .no_object_type
        .get_property_names(&mut f.base.reg, &f.base.ast);
    assert!(_expr11.is_empty());
}

// port: JSTypeTest#testGetPropertyNames_symbol
#[test]
fn test_get_property_names_symbol() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let _expr2 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(_expr1))
        .build(&mut f.base.reg, &f.base.ast);
    let sup = _expr2;
    let _expr3 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(sup))
        .build(&mut f.base.reg, &f.base.ast);
    let sub = _expr3;
    let _expr4 = f.get_bottom_type();
    let bottom_type = _expr4;
    let _expr5 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "base",
    );
    let _expr6 = PropertyKey::Symbol(_expr5);
    let base_symbol = _expr6;
    let _expr7 = closure_jstype::known_symbol_type::KnownSymbolType::new(
        &mut f.base.reg,
        &f.base.ast,
        "sub",
    );
    let _expr8 = PropertyKey::Symbol(_expr7);
    let sub_symbol = _expr8;
    let _expr9 = sup.define_property(
        &mut f.base.reg,
        &f.base.ast,
        base_symbol.clone(),
        bottom_type,
        false,
        None,
    );
    let _expr10 = sub.define_property(
        &mut f.base.reg,
        &f.base.ast,
        sub_symbol.clone(),
        bottom_type,
        false,
        None,
    );
    let _expr11 = sub.get_property_names(&mut f.base.reg, &f.base.ast);
    let _expr12 = vec![
        JsString::from("isPrototypeOf"),
        JsString::from("toLocaleString"),
        JsString::from("propertyIsEnumerable"),
        JsString::from("toString"),
        JsString::from("valueOf"),
        JsString::from("hasOwnProperty"),
        JsString::from("constructor"),
    ];
    assert_eq!(
        _expr11
            .into_iter()
            .collect::<std::collections::BTreeSet<JsString>>(),
        _expr12
            .into_iter()
            .collect::<std::collections::BTreeSet<JsString>>()
    );
    let _expr13 = sup.get_property_names(&mut f.base.reg, &f.base.ast);
    let _expr14 = vec![
        JsString::from("isPrototypeOf"),
        JsString::from("toLocaleString"),
        JsString::from("propertyIsEnumerable"),
        JsString::from("toString"),
        JsString::from("valueOf"),
        JsString::from("hasOwnProperty"),
        JsString::from("constructor"),
    ];
    assert_eq!(
        _expr13
            .into_iter()
            .collect::<std::collections::BTreeSet<JsString>>(),
        _expr14
            .into_iter()
            .collect::<std::collections::BTreeSet<JsString>>()
    );
    let _expr15 = sup.get_all_keys(&mut f.base.reg, &f.base.ast);
    let _expr16 = _expr15.known_symbol_keys();
    let _expr17 = base_symbol.symbol();
    assert_type_multiset(&mut f.base.reg, &f.base.ast, &_expr16, &[_expr17]);
    let _expr18 = sub.get_all_keys(&mut f.base.reg, &f.base.ast);
    let _expr19 = _expr18.known_symbol_keys();
    let _expr20 = base_symbol.symbol();
    let _expr21 = sub_symbol.symbol();
    assert_type_multiset(&mut f.base.reg, &f.base.ast, &_expr19, &[_expr20, _expr21]);
    let _expr22 = sub.get_own_property_known_symbols(&f.base.reg);
    let _expr23 = sub_symbol.symbol();
    assert_type_multiset(&mut f.base.reg, &f.base.ast, &_expr22, &[_expr23]);
    let _expr24 = f
        .base
        .no_object_type
        .get_property_names(&mut f.base.reg, &f.base.ast);
    assert!(_expr24.is_empty());
}

// port: JSTypeTest#testGetAndSetJSDocInfoWithNamedType
#[test]
fn test_get_and_set_jsdoc_info_with_named_type() {
    let mut f = Fixture::set_up();
    let _expr1 = JSDocInfo::builder();
    let mut builder = _expr1;
    builder.record_deprecated();
    let _expr2 = builder.build();
    let info = _expr2;
    let _expr3 = f
        .named_goog_bar
        .get_own_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X");
    assert!(_expr3.is_none());
    f.named_goog_bar
        .set_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X", info.clone());
    let _expr4 = f
        .named_goog_bar
        .get_own_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X");
    let _expr5 = _expr4.as_ref().unwrap().is_deprecated();
    assert!(_expr5);
    f.assert_property_type_inferred(f.named_goog_bar, "X");
    let _expr6 = f
        .named_goog_bar
        .get_property_type(&mut f.base.reg, &f.base.ast, "X");
    TypeSubject::assert_type(_expr6).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.unknown_type);
}

// port: JSTypeTest#testGetAndSetJSDocInfoWithObjectTypes
#[test]
fn test_get_and_set_jsdoc_info_with_object_types() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let _expr2 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(_expr1))
        .build(&mut f.base.reg, &f.base.ast);
    let sup = _expr2;
    let _expr3 = PrototypeObjectTypeBuilder::new()
        .set_implicit_prototype(Some(sup))
        .build(&mut f.base.reg, &f.base.ast);
    let sub = _expr3;
    let _expr4 = JSDocInfo::builder();
    let mut builder = _expr4;
    builder.record_deprecated();
    let _expr5 = builder.build();
    let deprecated = _expr5;
    let mut _expr6 = JSDocInfo::builder();
    builder = _expr6;
    builder.record_visibility(Visibility::PRIVATE);
    let _expr7 = builder.build();
    let private_info = _expr7;
    let _expr8 = sup.define_property(
        &mut f.base.reg,
        &f.base.ast,
        "X",
        f.base.number_type,
        true,
        None,
    );
    sup.set_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X", private_info.clone());
    let _expr9 = sub.define_property(
        &mut f.base.reg,
        &f.base.ast,
        "X",
        f.base.number_type,
        true,
        None,
    );
    sub.set_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X", deprecated.clone());
    let _expr10 = sup.get_own_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X");
    let _expr11 = _expr10.as_ref().unwrap().is_deprecated();
    assert!(!_expr11);
    let _expr12 = sup.get_own_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X");
    let _expr13 = _expr12.as_ref().unwrap().get_visibility();
    assert_eq!(_expr13, Visibility::PRIVATE);
    let _expr14 = sup.get_property_type(&mut f.base.reg, &f.base.ast, "X");
    TypeSubject::assert_type(_expr14).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr15 = sub.get_own_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X");
    let _expr16 = _expr15.as_ref().unwrap().is_deprecated();
    assert!(_expr16);
    let _expr17 = sub.get_own_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X");
    let _expr18 = _expr17.as_ref().unwrap().get_visibility();
    assert_eq!(_expr18, Visibility::INHERITED);
    let _expr19 = sub.get_property_type(&mut f.base.reg, &f.base.ast, "X");
    TypeSubject::assert_type(_expr19).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
}

// port: JSTypeTest#testGetAndSetJSDocInfoWithNoType
#[test]
fn test_get_and_set_jsdoc_info_with_no_type() {
    let mut f = Fixture::set_up();
    let _expr1 = JSDocInfo::builder();
    let mut builder = _expr1;
    builder.record_deprecated();
    let _expr2 = builder.build();
    let deprecated = _expr2;
    f.base
        .no_type
        .set_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X", deprecated.clone());
    let _expr3 = f
        .base
        .no_type
        .get_own_property_jsdoc_info(&mut f.base.reg, &f.base.ast, "X");
    assert!(_expr3.is_none());
}

// port: JSTypeTest#testIsTemplatedType
#[test]
fn test_is_templated_type() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_template_type(&f.base.ast, "T");
    let _expr2 = _expr1.has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(_expr2);
    let _expr3 = f
        .base
        .array_type
        .has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(!_expr3);
    let _expr4 = f.base.reg.create_template_type(&f.base.ast, "T");
    let _expr5 = f.base.create_templatized_type(f.base.array_type, &[_expr4]);
    let _expr6 = _expr5.has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(_expr6);
    let _expr7 = f
        .base
        .create_templatized_type(f.base.array_type, &[f.base.string_type]);
    let _expr8 = _expr7.has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(!_expr8);
    let _expr9 = FunctionTypeBuilder::new();
    let _expr10 = f.base.reg.create_template_type(&f.base.ast, "T");
    let _expr11 = _expr9.with_return_type(_expr10);
    let _expr12 = _expr11.build(&mut f.base.reg, &f.base.ast);
    let _expr13 = _expr12.has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(_expr13);
    let _expr14 = FunctionTypeBuilder::new();
    let _expr15 = f.base.reg.create_template_type(&f.base.ast, "T");
    let _expr16 = _expr14.with_type_of_this(_expr15);
    let _expr17 = _expr16.build(&mut f.base.reg, &f.base.ast);
    let _expr18 = _expr17.has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(_expr18);
    let _expr19 = FunctionTypeBuilder::new();
    let _expr20 = _expr19.with_return_type(f.base.string_type);
    let _expr21 = _expr20.build(&mut f.base.reg, &f.base.ast);
    let _expr22 = _expr21.has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(!_expr22);
    let _expr23 = f.base.reg.create_template_type(&f.base.ast, "T");
    let _expr24 = f.base.reg.create_union_type(
        &f.base.ast,
        &[f.base.null_type, _expr23, f.base.string_type],
    );
    let _expr25 = _expr24.has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(_expr25);
    let _expr26 = f.base.reg.create_union_type(
        &f.base.ast,
        &[f.base.null_type, f.base.array_type, f.base.string_type],
    );
    let _expr27 = _expr26.has_any_template_types(&mut f.base.reg, &f.base.ast);
    assert!(!_expr27);
}

// port: JSTypeTest#testTemplatizedType
#[test]
fn test_templatized_type() {
    let mut f = Fixture::set_up();
    let _expr5 = f.with_open_registry(|f| {
        let _expr1 = f.base.reg.create_template_type(&f.base.ast, "A");
        let _expr2 = f.base.reg.create_template_type(&f.base.ast, "B");
        let _expr3 = vec![_expr1, _expr2];

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("TestingType".into()),
            None,
            None,
            Some(f.base.unknown_type),
            Some(_expr3),
            false,
        )
    });
    let templatized_ctor = _expr5;
    let _expr6 = templatized_ctor.get_instance_type(&f.base.reg);
    let _expr7 = vec![f.base.number_type, f.base.string_type];
    let _expr8 = f
        .base
        .reg
        .create_templatized_type(&f.base.ast, _expr6.unwrap(), &_expr7);
    let templatized_instance = _expr8;
    let _expr9 = templatized_ctor.get_template_type_map(&f.base.reg);
    let ctr_type_map = _expr9;
    let _expr10 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "A");
    let key_a = _expr10;
    assert!(key_a.is_some());
    let _expr11 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "B");
    let key_b = _expr11;
    assert!(key_b.is_some());
    let _expr12 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "C");
    let key_c = _expr12;
    assert!(key_c.is_none());
    let _expr13 = f.base.reg.create_template_type(&f.base.ast, "C");
    let unknown_key = _expr13;
    let _expr14 = templatized_instance.get_template_type_map(&f.base.reg);
    let template_type_map = _expr14;
    let _expr15 = template_type_map.has_template_key(key_a.unwrap());
    assert!(_expr15);
    let _expr16 = template_type_map.has_template_key(key_b.unwrap());
    assert!(_expr16);
    let _expr17 = template_type_map.has_template_key(unknown_key);
    assert!(!_expr17);
    let _expr18 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, key_a.unwrap());
    TypeSubject::assert_type(_expr18).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr19 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, key_b.unwrap());
    TypeSubject::assert_type(_expr19).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let _expr20 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, unknown_key);
    TypeSubject::assert_type(_expr20).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr21 = templatized_instance.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr21, "TestingType<number,string>");
}

// port: JSTypeTest#testPartiallyTemplatizedType
#[test]
fn test_partially_templatized_type() {
    let mut f = Fixture::set_up();
    let _expr5 = f.with_open_registry(|f| {
        let _expr1 = f.base.reg.create_template_type(&f.base.ast, "A");
        let _expr2 = f.base.reg.create_template_type(&f.base.ast, "B");
        let _expr3 = vec![_expr1, _expr2];

        f.base.reg.create_constructor_type(
            &f.base.ast,
            Some("TestingType".into()),
            None,
            None,
            Some(f.base.unknown_type),
            Some(_expr3),
            false,
        )
    });
    let templatized_ctor = _expr5;
    let _expr6 = templatized_ctor.get_instance_type(&f.base.reg);
    let _expr7 = vec![f.base.number_type];
    let _expr8 = f
        .base
        .reg
        .create_templatized_type(&f.base.ast, _expr6.unwrap(), &_expr7);
    let templatized_instance = _expr8;
    let _expr9 = templatized_ctor.get_template_type_map(&f.base.reg);
    let ctr_type_map = _expr9;
    let _expr10 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "A");
    let key_a = _expr10;
    assert!(key_a.is_some());
    let _expr11 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "B");
    let key_b = _expr11;
    assert!(key_b.is_some());
    let _expr12 = ctr_type_map.get_last_template_type_key_by_name(&f.base.reg, "C");
    let key_c = _expr12;
    assert!(key_c.is_none());
    let _expr13 = f.base.reg.create_template_type(&f.base.ast, "C");
    let unknown_key = _expr13;
    let _expr14 = templatized_instance.get_template_type_map(&f.base.reg);
    let template_type_map = _expr14;
    let _expr15 = template_type_map.has_template_key(key_a.unwrap());
    assert!(_expr15);
    let _expr16 = template_type_map.has_template_key(key_b.unwrap());
    assert!(_expr16);
    let _expr17 = template_type_map.has_template_key(unknown_key);
    assert!(!_expr17);
    let _expr18 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, key_a.unwrap());
    TypeSubject::assert_type(_expr18).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr19 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, key_b.unwrap());
    TypeSubject::assert_type(_expr19).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr20 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, unknown_key);
    TypeSubject::assert_type(_expr20).is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.unknown_type,
    );
    let _expr21 = templatized_instance.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr21, "TestingType<number,?>");
}

// port: JSTypeTest#testTemplatizedTypeWithSubclass
#[test]
fn test_templatized_type_with_subclass() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_template_type(&f.base.ast, "A");
    let parent_key_a = _expr1;
    let _expr2 = f.base.reg.create_template_type(&f.base.ast, "A");
    let child_key_a = _expr2;
    let _expr3 = f.base.reg.create_template_type(&f.base.ast, "B");
    let child_key_b = _expr3;
    let _expr21 = f.with_open_registry(|f| {
        let _expr4 = FunctionTypeBuilder::new();
        let _expr5 = _expr4.with_name("ParentType");
        let _expr6 = _expr5.for_constructor();
        let _expr7 = vec![parent_key_a];
        let _expr8 = _expr6.with_template_keys(&mut f.base.reg, &f.base.ast, _expr7.clone());
        let _expr9 = _expr8.build(&mut f.base.reg, &f.base.ast);
        let parent = _expr9;
        let _expr10 = parent.get_instance_type(&f.base.reg);
        let _expr11 = vec![f.base.null_type];
        let _expr12 = f
            .base
            .reg
            .create_templatized_type(&f.base.ast, _expr10.unwrap(), &_expr11);
        let base_type = _expr12;
        let _expr13 = FunctionTypeBuilder::new();
        let _expr14 = _expr13.with_prototype_based_on(base_type);
        let _expr15 = _expr14.with_name("TestingType");
        let _expr16 = _expr15.for_constructor();
        let _expr17 = vec![child_key_a, child_key_b];
        let _expr18 = _expr16.with_template_keys(&mut f.base.reg, &f.base.ast, _expr17.clone());
        let _expr19 = _expr18.build(&mut f.base.reg, &f.base.ast);
        let child = _expr19;
        let _expr20 = child.get_instance_type(&f.base.reg);
        _expr20
            .unwrap()
            .merge_supertype_template_types(&mut f.base.reg, &f.base.ast, base_type);
        child
    });
    let templatized_ctor = _expr21;
    let _expr22 = templatized_ctor.get_instance_type(&f.base.reg);
    let _expr23 = vec![f.base.number_type, f.base.string_type];
    let _expr24 = f
        .base
        .reg
        .create_templatized_type(&f.base.ast, _expr22.unwrap(), &_expr23);
    let templatized_instance = _expr24;
    let _expr25 = templatized_instance.get_template_type_map(&f.base.reg);
    let template_type_map = _expr25;
    let _expr26 = template_type_map.get_last_template_type_key_by_name(&f.base.reg, "A");
    TypeSubject::assert_type(_expr26).is_equal_to(&mut f.base.reg, &f.base.ast, child_key_a);
    let _expr27 = template_type_map.get_last_template_type_key_by_name(&f.base.reg, "B");
    TypeSubject::assert_type(_expr27).is_equal_to(&mut f.base.reg, &f.base.ast, child_key_b);
    let _expr28 = template_type_map.has_template_key(parent_key_a);
    assert!(_expr28);
    let _expr29 = template_type_map.has_template_key(child_key_a);
    assert!(_expr29);
    let _expr30 = template_type_map.has_template_key(child_key_b);
    assert!(_expr30);
    let _expr31 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, parent_key_a);
    TypeSubject::assert_type(_expr31).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    let _expr32 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, child_key_a);
    TypeSubject::assert_type(_expr32).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    let _expr33 =
        template_type_map.get_resolved_template_type(&mut f.base.reg, &f.base.ast, child_key_b);
    TypeSubject::assert_type(_expr33).is_equal_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    let _expr34 = templatized_instance.to_string(&mut f.base.reg, &f.base.ast);
    assert_eq!(_expr34, "TestingType<number,string>");
}

// port: JSTypeTest#testTemplateTypeValidator
#[test]
fn test_template_type_validator() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_template_type(&f.base.ast, "T");
    let t = _expr1;
    let _expr2 = t.set_validator(
        &mut f.base.reg,
        &f.base.ast,
        Arc::new(|type_, reg, _ast| type_.is_template_type(reg)),
    );
    assert!(_expr2);
    let _expr3 = t.set_validator(
        &mut f.base.reg,
        &f.base.ast,
        Arc::new(|type_, reg, _ast| !type_.is_template_type(reg)),
    );
    assert!(!_expr3);
}

// port: JSTypeTest#testTemplateTypeHasReferenceName
#[test]
fn test_template_type_has_reference_name() {
    let mut f = Fixture::set_up();
    let _expr1 = f.base.reg.create_template_type(&f.base.ast, "T");
    let t = _expr1;
    TypeSubject::assert_type(t).get_reference_name_is_equal_to(&f.base.reg, "T");
}

// port: JSTypeTest#testCanCastTo
#[test]
fn test_can_cast_to() {
    let mut f = Fixture::set_up();
    let _expr1 = f
        .base
        .all_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(_expr1);
    let _expr2 = f
        .base
        .all_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(_expr2);
    let _expr3 = f
        .base
        .all_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(_expr3);
    let _expr4 = f
        .base
        .all_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr4);
    let _expr5 = f
        .base
        .all_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(_expr5);
    let _expr6 = f
        .base
        .all_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr6);
    let _expr7 = f
        .base
        .number_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr7);
    let _expr8 = f
        .base
        .number_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr8);
    let _expr9 = f
        .base
        .number_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr9);
    let _expr10 = f
        .base
        .number_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(_expr10);
    let _expr11 = f
        .base
        .number_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr11);
    let _expr12 = f
        .base
        .number_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr12);
    let _expr13 = f
        .base
        .string_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr13);
    let _expr14 = f
        .base
        .string_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr14);
    let _expr15 = f
        .base
        .string_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(_expr15);
    let _expr16 = f
        .base
        .string_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr16);
    let _expr17 = f
        .base
        .string_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr17);
    let _expr18 = f
        .base
        .string_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr18);
    let _expr19 = f
        .base
        .boolean_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr19);
    let _expr20 = f
        .base
        .boolean_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr20);
    let _expr21 = f
        .base
        .boolean_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr21);
    let _expr22 = f
        .base
        .boolean_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr22);
    let _expr23 =
        f.base
            .boolean_type
            .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(_expr23);
    let _expr24 = f
        .base
        .boolean_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(!_expr24);
    let _expr25 = f
        .base
        .object_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.null_type);
    assert!(!_expr25);
    let _expr26 = f
        .base
        .object_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.void_type);
    assert!(!_expr26);
    let _expr27 = f
        .base
        .object_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.string_type);
    assert!(!_expr27);
    let _expr28 = f
        .base
        .object_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.number_type);
    assert!(!_expr28);
    let _expr29 = f
        .base
        .object_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr29);
    let _expr30 = f
        .base
        .object_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.object_type);
    assert!(_expr30);
    let _expr31 =
        f.base
            .boolean_type
            .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.object_number_string);
    assert!(!_expr31);
    let _expr32 =
        f.base
            .object_number_string
            .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.boolean_type);
    assert!(!_expr32);
    let _expr33 = f
        .base
        .array_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.function_type);
    assert!(!_expr33);
    let _expr34 = f
        .base
        .function_type
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr34);
    let _expr35 = f
        .base
        .null_void
        .can_cast_to(&mut f.base.reg, &f.base.ast, f.base.array_type);
    assert!(!_expr35);
    let _expr36 = f
        .base
        .create_union_type(&[f.base.array_type, f.base.null_type]);
    let _expr37 = f
        .base
        .null_void
        .can_cast_to(&mut f.base.reg, &f.base.ast, _expr36);
    assert!(_expr37);
    let _expr38 = f.base.array_function_type.can_cast_to(
        &mut f.base.reg,
        &f.base.ast,
        f.base.boolean_object_function_type,
    );
    assert!(_expr38);
}

// port: JSTypeTest#testEqualityOfClassTypes_withSameReferenceName_preResolution
#[test]
fn test_equality_of_class_types_with_same_reference_name_pre_resolution() {
    let mut f = Fixture::set_up();
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = FunctionTypeBuilder::new();
        let _expr3 = _expr2.for_constructor();
        let _expr4 = _expr3.with_name("Foo");
        let _expr5 = _expr4.build(&mut f.base.reg, &f.base.ast);
        let class_a_ctor = _expr5;
        let _expr6 = FunctionTypeBuilder::new();
        let _expr7 = _expr6.for_constructor();
        let _expr8 = _expr7.with_name("Foo");
        let _expr9 = _expr8.build(&mut f.base.reg, &f.base.ast);
        let class_b_ctor = _expr9;
        let _expr10 = class_a_ctor.get_instance_type(&f.base.reg);
        let _expr11 = class_b_ctor.get_instance_type(&f.base.reg);
        TypeSubject::assert_type(_expr10).is_equal_to(&mut f.base.reg, &f.base.ast, _expr11);
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
}

// port: JSTypeTest#testEqualityOfClassTypes_withSameReferenceName_differentGoogModuleId_preResolution
#[test]
fn test_equality_of_class_types_with_same_reference_name_different_goog_module_id_pre_resolution() {
    let mut f = Fixture::set_up();
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = FunctionTypeBuilder::new();
        let _expr3 = _expr2.for_constructor();
        let _expr4 = _expr3.with_name("Foo");
        let _expr5 = _expr4.set_goog_module_id(Some("module1".into()));
        let _expr6 = _expr5.build(&mut f.base.reg, &f.base.ast);
        let class_a_ctor = _expr6;
        let _expr7 = FunctionTypeBuilder::new();
        let _expr8 = _expr7.for_constructor();
        let _expr9 = _expr8.with_name("Foo");
        let _expr10 = _expr9.build(&mut f.base.reg, &f.base.ast);
        let class_b_ctor = _expr10;
        let _expr11 = class_a_ctor.get_instance_type(&f.base.reg);
        let _expr12 = class_b_ctor.get_instance_type(&f.base.reg);
        TypeSubject::assert_type(_expr11).is_not_equal_to(&mut f.base.reg, &f.base.ast, _expr12);
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
}

// port: JSTypeTest#testEqualityOfClassTypes_withSameReferenceName_sameGoogModuleId_preResolution
#[test]
fn test_equality_of_class_types_with_same_reference_name_same_goog_module_id_pre_resolution() {
    let mut f = Fixture::set_up();
    {
        let mut _expr1 = f.base.reg.get_resolver().open_for_definition();
        let _expr2 = FunctionTypeBuilder::new();
        let _expr3 = _expr2.for_constructor();
        let _expr4 = _expr3.with_name("Foo");
        let _expr5 = _expr4.set_goog_module_id(Some("module1".into()));
        let _expr6 = _expr5.build(&mut f.base.reg, &f.base.ast);
        let class_a_ctor = _expr6;
        let _expr7 = FunctionTypeBuilder::new();
        let _expr8 = _expr7.for_constructor();
        let _expr9 = _expr8.with_name("Foo");
        let _expr10 = _expr9.set_goog_module_id(Some("module1".into()));
        let _expr11 = _expr10.build(&mut f.base.reg, &f.base.ast);
        let class_b_ctor = _expr11;
        let _expr12 = class_a_ctor.get_instance_type(&f.base.reg);
        let _expr13 = class_b_ctor.get_instance_type(&f.base.reg);
        TypeSubject::assert_type(_expr12).is_equal_to(&mut f.base.reg, &f.base.ast, _expr13);
        _expr1.close(&mut f.base.reg, &f.base.ast);
    }
}

// port: JSTypeTest#testEqualityOfClassTypes_withSameReferenceName_postResolution
#[test]
fn test_equality_of_class_types_with_same_reference_name_post_resolution() {
    let mut f = Fixture::set_up();
    let _expr5 = f.with_open_registry(|f| {
        let _expr1 = FunctionTypeBuilder::new();
        let _expr2 = _expr1.for_constructor();
        let _expr3 = _expr2.with_name("Foo");

        _expr3.build(&mut f.base.reg, &f.base.ast)
    });
    let class_a_ctor = _expr5;
    let _expr10 = f.with_open_registry(|f| {
        let _expr6 = FunctionTypeBuilder::new();
        let _expr7 = _expr6.for_constructor();
        let _expr8 = _expr7.with_name("Foo");

        _expr8.build(&mut f.base.reg, &f.base.ast)
    });
    let class_b_ctor = _expr10;
    TypeSubject::assert_type(class_a_ctor).is_not_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        class_b_ctor,
    );
    let _expr11 = class_a_ctor.get_instance_type(&f.base.reg);
    let _expr12 = class_b_ctor.get_instance_type(&f.base.reg);
    TypeSubject::assert_type(_expr11).is_not_equal_to(&mut f.base.reg, &f.base.ast, _expr12);
    let _expr13 = class_a_ctor.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr14 = class_b_ctor.get_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr13).is_not_equal_to(&mut f.base.reg, &f.base.ast, _expr14);
}

// port: JSTypeTest#testEqualityOfInterfaceTypes_withSameReferenceName_postResolution
#[test]
fn test_equality_of_interface_types_with_same_reference_name_post_resolution() {
    let mut f = Fixture::set_up();
    let _expr1 = FunctionTypeBuilder::new();
    let _expr2 = _expr1.for_interface();
    let _expr3 = _expr2.with_name("Foo");
    let _expr4 = _expr3.build(&mut f.base.reg, &f.base.ast);
    let interface_a_ctor = _expr4;
    let _expr5 = FunctionTypeBuilder::new();
    let _expr6 = _expr5.for_interface();
    let _expr7 = _expr6.with_name("Foo");
    let _expr8 = _expr7.build(&mut f.base.reg, &f.base.ast);
    let interface_b_ctor = _expr8;
    TypeSubject::assert_type(interface_a_ctor).is_not_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        interface_b_ctor,
    );
    let _expr9 = interface_a_ctor.get_instance_type(&f.base.reg);
    let _expr10 = interface_b_ctor.get_instance_type(&f.base.reg);
    TypeSubject::assert_type(_expr9).is_not_equal_to(&mut f.base.reg, &f.base.ast, _expr10);
    let _expr11 = interface_a_ctor.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr12 = interface_b_ctor.get_prototype(&mut f.base.reg, &f.base.ast);
    TypeSubject::assert_type(_expr11).is_not_equal_to(&mut f.base.reg, &f.base.ast, _expr12);
}

// port: JSTypeTest#testRecordTypeEquality
#[test]
fn test_record_type_equality() {
    let mut f = Fixture::set_up();
    let _expr1 = vec![(JsString::from("x"), f.base.number_type)];
    let _expr2 = f.base.reg.create_record_type(&f.base.ast, _expr1);
    let first_type = _expr2;
    let _expr3 = vec![(JsString::from("x"), f.base.number_type)];
    let _expr4 = f.base.reg.create_record_type(&f.base.ast, _expr3);
    let second_type = _expr4;
    TypeSubject::assert_type(first_type).is_not_same_instance_as(second_type);
    TypeSubject::assert_type(first_type).is_equal_to(&mut f.base.reg, &f.base.ast, second_type);
    TypeSubject::assert_type(first_type).is_equal_to(&mut f.base.reg, &f.base.ast, second_type);
}

// port: JSTypeTest#testRecordAndInterfaceObjectTypeNotEqualWithSameProperties
#[test]
fn test_record_and_interface_object_type_not_equal_with_same_properties() {
    let mut f = Fixture::set_up();
    let _expr1 = vec![(JsString::from("x"), f.base.number_type)];
    let _expr2 = f.base.reg.create_record_type(&f.base.ast, _expr1);
    let first_type = _expr2;
    let _expr3 = FunctionTypeBuilder::new();
    let _expr4 = _expr3.for_interface();
    let _expr5 = _expr4.with_name("Foo");
    let _expr6 = _expr5.build(&mut f.base.reg, &f.base.ast);
    let second_type_constructor = _expr6;
    let _expr7 = second_type_constructor.get_prototype(&mut f.base.reg, &f.base.ast);
    let _expr8 = _expr7.define_property(
        &mut f.base.reg,
        &f.base.ast,
        "x",
        f.base.number_type,
        false,
        None,
    );
    second_type_constructor.set_implicit_match(&mut f.base.reg, true);
    let _expr9 = second_type_constructor.get_instance_type(&f.base.reg);
    let second_type = _expr9;
    TypeSubject::assert_type(first_type).is_not_equal_to(&mut f.base.reg, &f.base.ast, second_type);
}

// port: JSTypeTest#testRecordAndObjectLiteralWithSameProperties
#[test]
fn test_record_and_object_literal_with_same_properties() {
    let mut f = Fixture::set_up();
    let _expr1 = vec![(JsString::from("x"), f.base.number_type)];
    let _expr2 = f.base.reg.create_record_type(&f.base.ast, _expr1);
    let first_type = _expr2;
    TypeSubject::assert_type(first_type).to_string_is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        "{x: number}",
    );
    let _expr3 = f.base.reg.create_anonymous_object_type(&f.base.ast, None);
    let second_type = _expr3;
    let _expr4 = second_type.define_declared_property(
        &mut f.base.reg,
        &f.base.ast,
        "x",
        f.base.number_type,
        None,
    );
    TypeSubject::assert_type(second_type).to_string_is_equal_to(
        &mut f.base.reg,
        &f.base.ast,
        "{x: number}",
    );
    TypeSubject::assert_type(first_type).is_not_equal_to(&mut f.base.reg, &f.base.ast, second_type);
    TypeSubject::assert_type(first_type).is_not_subtype_of(
        &mut f.base.reg,
        &f.base.ast,
        second_type,
    );
    TypeSubject::assert_type(second_type).is_subtype_of(&mut f.base.reg, &f.base.ast, first_type);
}
