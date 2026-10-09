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
//   test/com/google/javascript/rhino/jstype/FunctionTypeTest.java.

use closure_jstype::{
    TypeId,
    function_param_builder::FunctionParamBuilder,
    function_type::{FunctionType, FunctionTypeBuilder},
    js_type::JSType,
    object_type::ObjectType,
    property::Property,
    proxy_object_type::{self, ProxyObjectType},
    template_type_replacer::TemplateTypeReplacer,
    testing::{
        asserts::Asserts, base_js_type_test_case::BaseJSTypeTestCase,
        map_based_scope::MapBasedScope,
    },
};
use closure_rhino::{ir::IR, js_string::JsString, token::Token};
use std::sync::Arc;

// port: FunctionTypeTest#assertLeastSupertype
fn assert_least_supertype(f: &mut BaseJSTypeTestCase, s: &str, t1: TypeId, t2: TypeId) {
    let left = t1.get_least_supertype(&mut f.reg, &f.ast, t2);
    assert_eq!(left.to_string(&mut f.reg, &f.ast), s);
    let right = t2.get_least_supertype(&mut f.reg, &f.ast, t1);
    assert_eq!(right.to_string(&mut f.reg, &f.ast), s);
}
// port: FunctionTypeTest#assertGreatestSubtype
fn assert_greatest_subtype(f: &mut BaseJSTypeTestCase, s: &str, t1: TypeId, t2: TypeId) {
    let left = t1.get_greatest_subtype(&mut f.reg, &f.ast, t2);
    assert_eq!(left.to_string(&mut f.reg, &f.ast), s);
    let right = t2.get_greatest_subtype(&mut f.reg, &f.ast, t1);
    assert_eq!(right.to_string(&mut f.reg, &f.ast), s);
}
// port: FunctionTypeTest#createInterfaceType
fn create_interface_type(
    f: &mut BaseJSTypeTestCase,
    name: &str,
    template_keys: Vec<TypeId>,
    struct_: bool,
) -> TypeId {
    let fn_ = FunctionTypeBuilder::new()
        .for_interface()
        .with_name(name)
        .with_no_parameters()
        .with_template_keys(&mut f.reg, &f.ast, template_keys)
        .build(&mut f.reg, &f.ast);
    if struct_ {
        fn_.set_struct(&mut f.reg);
    }
    fn_
}

// port: FunctionTypeTest#testDefaultReturnType
#[test]
fn test_default_return_type() {
    let mut f = BaseJSTypeTestCase::new();
    let fn_ = FunctionTypeBuilder::new().build(&mut f.reg, &f.ast);
    assert!(
        fn_.get_return_type(&f.reg)
            .equals(&mut f.reg, &f.ast, f.unknown_type)
    );
}
// port: FunctionTypeTest#testSupAndInfOfReturnTypes
#[test]
fn test_sup_and_inf_of_return_types() {
    let mut f = BaseJSTypeTestCase::new();
    let ret_string = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_inferred_return_type(f.string_type)
        .build(&mut f.reg, &f.ast);
    let ret_number = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_return_type(f.number_type)
        .build(&mut f.reg, &f.ast);
    assert_least_supertype(
        &mut f,
        "function(): (number|string)",
        ret_string,
        ret_number,
    );
    assert_greatest_subtype(&mut f, "function(): None", ret_string, ret_number);
    assert!(ret_string.is_return_type_inferred(&f.reg));
    assert!(!ret_number.is_return_type_inferred(&f.reg));
    assert!(
        ret_string
            .get_least_supertype(&mut f.reg, &f.ast, ret_number)
            .is_return_type_inferred(&f.reg)
    );
    assert!(
        ret_string
            .get_greatest_subtype(&mut f.reg, &f.ast, ret_string)
            .is_return_type_inferred(&f.reg)
    );
}
// port: FunctionTypeTest#testSupAndInfOfReturnTypesWithDifferentParams
#[test]
fn test_sup_and_inf_of_return_types_with_different_params() {
    let mut f = BaseJSTypeTestCase::new();
    let ret_string = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[f.number_type]))
        .with_inferred_return_type(f.string_type)
        .build(&mut f.reg, &f.ast);
    let ret_number = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_return_type(f.number_type)
        .build(&mut f.reg, &f.ast);
    assert_least_supertype(&mut f, "Function", ret_string, ret_number);
    assert_greatest_subtype(&mut f, "function(...*): None", ret_string, ret_number);
}
// port: FunctionTypeTest#testSupAndInfWithDifferentParams
#[test]
fn test_sup_and_inf_with_different_params() {
    let mut f = BaseJSTypeTestCase::new();
    let ret_string = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[f.number_type]))
        .with_return_type(f.string_type)
        .build(&mut f.reg, &f.ast);
    let ret_number = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[f.string_type]))
        .with_return_type(f.number_type)
        .build(&mut f.reg, &f.ast);
    assert_least_supertype(&mut f, "Function", ret_string, ret_number);
    assert_greatest_subtype(&mut f, "function(...*): None", ret_string, ret_number);
}
// port: FunctionTypeTest#testSupAndInfWithDifferentThisTypes
#[test]
fn test_sup_and_inf_with_different_this_types() {
    let mut f = BaseJSTypeTestCase::new();
    let ret_string = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_type_of_this(f.object_type)
        .with_return_type(f.string_type)
        .build(&mut f.reg, &f.ast);
    let ret_number = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_type_of_this(f.date_type)
        .with_return_type(f.number_type)
        .build(&mut f.reg, &f.ast);
    assert_least_supertype(
        &mut f,
        "function(this:Object): (number|string)",
        ret_string,
        ret_number,
    );
    assert_greatest_subtype(&mut f, "function(this:Date): None", ret_string, ret_number);
}
// port: FunctionTypeTest#testSupAndInfWithDifferentThisTypes2
#[test]
fn test_sup_and_inf_with_different_this_types2() {
    let mut f = BaseJSTypeTestCase::new();
    let ret_string = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_type_of_this(f.array_type)
        .with_return_type(f.string_type)
        .build(&mut f.reg, &f.ast);
    let ret_number = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_type_of_this(f.date_type)
        .with_return_type(f.number_type)
        .build(&mut f.reg, &f.ast);
    assert_least_supertype(
        &mut f,
        "function(this:(Array|Date)): (number|string)",
        ret_string,
        ret_number,
    );
    assert_greatest_subtype(
        &mut f,
        "function(this:NoObject): None",
        ret_string,
        ret_number,
    );
}
// port: FunctionTypeTest#testSupAndInfOfReturnTypesWithNumOfParams
#[test]
fn test_sup_and_inf_of_return_types_with_num_of_params() {
    let mut f = BaseJSTypeTestCase::new();
    let two_numbers = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[f.number_type, f.number_type]))
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    let one_number = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[f.number_type]))
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    assert_least_supertype(
        &mut f,
        "function(number, number): boolean",
        two_numbers,
        one_number,
    );
    assert_greatest_subtype(&mut f, "function(number): boolean", two_numbers, one_number);
}
// port: FunctionTypeTest#testSubtypeWithInterfaceThisType
#[test]
fn test_subtype_with_interface_this_type() {
    let mut f = BaseJSTypeTestCase::new();
    let iface = create_interface_type(&mut f, "I", vec![], false);
    let iface_return_boolean = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_type_of_this(iface.get_instance_type(&f.reg))
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    let obj_return_boolean = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[]))
        .with_type_of_this(f.object_type)
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    assert!(obj_return_boolean.is_subtype(&mut f.reg, &f.ast, iface_return_boolean));
}
// port: FunctionTypeTest#testOrdinaryFunctionPrototype
#[test]
fn test_ordinary_function_prototype() {
    let mut f = BaseJSTypeTestCase::new();
    let one_number = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[f.number_type]))
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    assert!(one_number.get_own_property_names(&f.reg).is_empty());
}
// port: FunctionTypeTest#testCtorWithPrototypeSet
#[test]
fn test_ctor_with_prototype_set() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let ctor =
        f.reg
            .create_constructor_type(&f.ast, Some("Foo".into()), None, None, None, None, false);
    assert!(
        !ctor
            .get_instance_type(&f.reg)
            .unwrap()
            .is_unknown_type(&mut f.reg, &f.ast)
    );
    let node = f.ast.new_node(Token::OBJECTLIT);
    ctor.define_declared_property(&mut f.reg, &f.ast, "prototype", f.unknown_type, Some(node));
    assert!(
        ctor.get_instance_type(&f.reg)
            .unwrap()
            .is_unknown_type(&mut f.reg, &f.ast)
    );
    assert_eq!(
        ctor.get_own_property_names(&f.reg)
            .into_iter()
            .collect::<Vec<_>>(),
        vec![JsString::from("prototype")]
    );
    assert!(ctor.is_property_type_inferred(&mut f.reg, &f.ast, "prototype"));
    assert!(
        ctor.get_property_type(&mut f.reg, &f.ast, "prototype")
            .is_unknown_type(&mut f.reg, &f.ast)
    );
    assert_eq!(
        ctor.get_property_node(&mut f.reg, &f.ast, "prototype"),
        Some(node)
    );
    closer.close(&mut f.reg, &f.ast);
}
// port: FunctionTypeTest#testCtorWithInstanceInheritance
#[test]
fn test_ctor_with_instance_inheritance() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let foo_ctor = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Foo")
        .build(&mut f.reg, &f.ast);
    let bar_ctor = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Bar")
        .build(&mut f.reg, &f.ast);
    let foo_instance = foo_ctor.get_instance_type(&f.reg).unwrap();
    bar_ctor.set_prototype_based_on(&mut f.reg, &f.ast, foo_instance);
    foo_ctor
        .get_prototype(&mut f.reg, &f.ast)
        .define_declared_property(&mut f.reg, &f.ast, "bar", f.string_type, None);
    let proto = bar_ctor
        .get_prototype(&mut f.reg, &f.ast)
        .get_implicit_prototype(&mut f.reg, &f.ast)
        .unwrap();
    assert!(proto.equals(&mut f.reg, &f.ast, foo_instance));
    let type_ = foo_ctor
        .get_instance_type(&f.reg)
        .unwrap()
        .get_slot(&mut f.reg, &f.ast, "bar")
        .unwrap()
        .get_type(&f.reg);
    assert!(type_.equals(&mut f.reg, &f.ast, f.string_type));
    closer.close(&mut f.reg, &f.ast);
}
// port: FunctionTypeTest#testCtorWithClassSideInheritance
#[test]
fn test_ctor_with_class_side_inheritance() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let foo_ctor = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Foo")
        .build(&mut f.reg, &f.ast);
    let source = f.ast.new_node(Token::CLASS);
    let bar_ctor = FunctionTypeBuilder::new()
        .with_source_node(Some(source))
        .for_constructor()
        .with_name("Bar")
        .build(&mut f.reg, &f.ast);
    let foo_instance = foo_ctor.get_instance_type(&f.reg).unwrap();
    bar_ctor.set_prototype_based_on(&mut f.reg, &f.ast, foo_instance);
    foo_ctor.define_declared_property(&mut f.reg, &f.ast, "foo", f.number_type, None);
    let proto = bar_ctor.get_implicit_prototype(&mut f.reg, &f.ast).unwrap();
    assert!(proto.equals(&mut f.reg, &f.ast, foo_ctor));
    let type_ = bar_ctor
        .get_slot(&mut f.reg, &f.ast, "foo")
        .unwrap()
        .get_type(&f.reg);
    assert!(type_.equals(&mut f.reg, &f.ast, f.number_type));
    closer.close(&mut f.reg, &f.ast);
}
// port: FunctionTypeTest#testEqualityOfProxyForCtor
#[test]
fn test_equality_of_proxy_for_ctor() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let empty_scope = Arc::new(MapBasedScope::new(vec![]));
    let key = f.reg.create_template_type(&f.ast, "KEY");
    let foo_ctor = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Foo")
        .with_template_keys(&mut f.reg, &f.ast, vec![key])
        .build(&mut f.reg, &f.ast);
    let bar_type = f
        .reg
        .create_named_type(&f.ast, Some(empty_scope.clone()), "Bar", "", -1, -1);
    f.reg
        .declare_type(&f.ast, Some(empty_scope.as_ref()), "Bar", foo_ctor);
    closer.close(&mut f.reg, &f.ast);
    assert!(foo_ctor.equals(&mut f.reg, &f.ast, bar_type));
    assert!(foo_ctor.is_subtype_of(&mut f.reg, &f.ast, bar_type));
    assert!(bar_type.is_function_type(&f.reg));
}
// port: FunctionTypeTest#testCtorsSpecializedOnTemplateTypes
#[test]
fn test_ctors_specialized_on_template_types() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let key = f.reg.create_template_type(&f.ast, "KEY");
    let foo_ctor = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Foo")
        .with_template_keys(&mut f.reg, &f.ast, vec![key])
        .with_return_type(key)
        .build(&mut f.reg, &f.ast);
    let mut string_replacer = TemplateTypeReplacer::for_inference(
        &mut f.reg,
        &f.ast,
        &closure_rhino::fast_hash::IndexMap::<_, _>::from_iter([(key, Some(f.string_type))]),
    );
    let mut number_replacer = TemplateTypeReplacer::for_inference(
        &mut f.reg,
        &f.ast,
        &closure_rhino::fast_hash::IndexMap::<_, _>::from_iter([(key, Some(f.number_type))]),
    );
    let foo_of_string = foo_ctor
        .visit(&mut f.reg, &f.ast, &mut string_replacer)
        .to_maybe_function_type(&f.reg)
        .unwrap();
    let foo_of_number = foo_ctor
        .visit(&mut f.reg, &f.ast, &mut number_replacer)
        .to_maybe_function_type(&f.reg)
        .unwrap();
    closure_jstype::testing::asserts::assert_equality_groups(
        &mut f.reg,
        &f.ast,
        &[vec![foo_ctor], vec![foo_of_string], vec![foo_of_number]],
    );
    closer.close(&mut f.reg, &f.ast);
}
// port: FunctionTypeTest#testEmptyFunctionTypes
#[test]
fn test_empty_function_types() {
    let f = BaseJSTypeTestCase::new();
    assert!(f.least_function_type.is_empty_type(&f.reg));
    assert!(!f.greatest_function_type.is_empty_type(&f.reg));
}

// port: FunctionTypeTest#testInterfacePrototypeChain1
#[test]
fn test_interface_prototype_chain1() {
    let mut f = BaseJSTypeTestCase::new();
    let iface = create_interface_type(&mut f, "I", vec![], false);
    let prototype = iface.get_prototype(&mut f.reg, &f.ast);
    let implicit = iface
        .get_instance_type(&f.reg)
        .unwrap()
        .get_implicit_prototype(&mut f.reg, &f.ast)
        .unwrap();
    assert!(prototype.equals(&mut f.reg, &f.ast, implicit));
    let implicit = prototype
        .get_implicit_prototype(&mut f.reg, &f.ast)
        .unwrap();
    assert!(f.object_type.equals(&mut f.reg, &f.ast, implicit));
}
// port: FunctionTypeTest#testInterfacePrototypeChain2
#[test]
fn test_interface_prototype_chain2() {
    let mut f = BaseJSTypeTestCase::new();
    let iface = create_interface_type(&mut f, "I", vec![], false);
    iface
        .get_prototype(&mut f.reg, &f.ast)
        .define_declared_property(&mut f.reg, &f.ast, "numberProp", f.number_type, None);
    let sub_iface = create_interface_type(&mut f, "SubI", vec![], false);
    let iface_instance = iface.get_instance_type(&f.reg).unwrap();
    sub_iface.set_extended_interfaces(&mut f.reg, &f.ast, vec![iface_instance]);
    let prototype = sub_iface.get_prototype(&mut f.reg, &f.ast);
    let implicit = sub_iface
        .get_instance_type(&f.reg)
        .unwrap()
        .get_implicit_prototype(&mut f.reg, &f.ast)
        .unwrap();
    assert!(prototype.equals(&mut f.reg, &f.ast, implicit));
    let implicit = prototype
        .get_implicit_prototype(&mut f.reg, &f.ast)
        .unwrap();
    assert!(f.object_type.equals(&mut f.reg, &f.ast, implicit));
    let instance = sub_iface.get_instance_type(&f.reg).unwrap();
    assert!(instance.has_property(&mut f.reg, &f.ast, "numberProp"));
    assert!(instance.is_property_type_declared(&mut f.reg, &f.ast, "numberProp"));
    assert!(!instance.is_property_type_inferred(&mut f.reg, &f.ast, "numberProp"));
}
// port: FunctionTypeTest#testInterfacePrototypeChain3
#[test]
fn test_interface_prototype_chain3() {
    let mut f = BaseJSTypeTestCase::new();
    let template_t = f.reg.create_template_type(&f.ast, "T");
    let iface = create_interface_type(&mut f, "I", vec![template_t], false);
    iface
        .get_prototype(&mut f.reg, &f.ast)
        .define_declared_property(&mut f.reg, &f.ast, "genericProp", template_t, None);
    let sub_iface = create_interface_type(&mut f, "SubI", vec![], false);
    let iface_instance = iface.get_instance_type(&f.reg).unwrap();
    sub_iface.set_extended_interfaces(&mut f.reg, &f.ast, vec![iface_instance]);
    let prototype = sub_iface.get_prototype(&mut f.reg, &f.ast);
    let implicit = sub_iface
        .get_instance_type(&f.reg)
        .unwrap()
        .get_implicit_prototype(&mut f.reg, &f.ast)
        .unwrap();
    assert!(prototype.equals(&mut f.reg, &f.ast, implicit));
    let implicit = prototype
        .get_implicit_prototype(&mut f.reg, &f.ast)
        .unwrap();
    assert!(f.object_type.equals(&mut f.reg, &f.ast, implicit));
    let instance = sub_iface.get_instance_type(&f.reg).unwrap();
    assert!(instance.has_property(&mut f.reg, &f.ast, "genericProp"));
    assert!(instance.is_property_type_declared(&mut f.reg, &f.ast, "genericProp"));
    assert!(!instance.is_property_type_inferred(&mut f.reg, &f.ast, "genericProp"));
    let type_ = instance.get_property_type(&mut f.reg, &f.ast, "genericProp");
    assert!(type_.equals(&mut f.reg, &f.ast, template_t));
}

// port: FunctionTypeTest#testequals
#[test]
fn testequals() {
    let mut f = BaseJSTypeTestCase::new();
    let type_ = FunctionTypeBuilder::new().build(&mut f.reg, &f.ast);
    assert!(!type_.equals(&mut f.reg, &f.ast, None));
    assert!(type_.equals(&mut f.reg, &f.ast, type_));
}
// port: FunctionTypeTest#testequalsParams
#[test]
fn testequals_params() {
    let mut f = BaseJSTypeTestCase::new();
    let one_num = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[f.number_type]))
        .build(&mut f.reg, &f.ast);
    let optional_params = f.reg.create_optional_parameters(&f.ast, &[f.number_type]);
    let opt_num = FunctionTypeBuilder::new()
        .with_parameters(optional_params)
        .build(&mut f.reg, &f.ast);
    let var_num = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters_with_var_args(&[f.number_type]))
        .build(&mut f.reg, &f.ast);
    Asserts::assert_equivalence_operations(&mut f.reg, &f.ast, one_num, one_num);
    Asserts::assert_equivalence_operations(&mut f.reg, &f.ast, opt_num, opt_num);
    Asserts::assert_equivalence_operations(&mut f.reg, &f.ast, var_num, var_num);
    assert!(!one_num.equals(&mut f.reg, &f.ast, opt_num));
    assert!(!one_num.equals(&mut f.reg, &f.ast, var_num));
    assert!(!opt_num.equals(&mut f.reg, &f.ast, var_num));
}
// port: FunctionTypeTest#testIsEquivalentOptAndVarArgs
#[test]
fn test_is_equivalent_opt_and_var_args() {
    let mut f = BaseJSTypeTestCase::new();
    let var_num = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters_with_var_args(&[f.number_type]))
        .build(&mut f.reg, &f.ast);
    let mut builder = FunctionParamBuilder::new();
    builder.add_optional_params(&mut f.reg, &f.ast, &[f.number_type]);
    builder.add_var_args(f.number_type);
    let opt_and_var_num = FunctionTypeBuilder::new()
        .with_parameters(builder.build())
        .build(&mut f.reg, &f.ast);
    assert!(!var_num.equals(&mut f.reg, &f.ast, opt_and_var_num));
    assert!(!opt_and_var_num.equals(&mut f.reg, &f.ast, var_num));
}
// port: FunctionTypeTest#testRecursiveFunction
#[test]
fn test_recursive_function() {
    let mut f = BaseJSTypeTestCase::new();
    let loop_ = proxy_object_type::create(&mut f.reg, &f.ast, f.number_type, None);
    let fn_ = FunctionTypeBuilder::new()
        .with_parameters(f.reg.create_parameters(&[loop_]))
        .with_return_type(loop_)
        .build(&mut f.reg, &f.ast);
    loop_.set_referenced_type(&mut f.reg, fn_);
    assert_eq!(
        fn_.to_string(&mut f.reg, &f.ast),
        "function(Function): Function"
    );
    Asserts::assert_equivalence_operations(&mut f.reg, &f.ast, fn_, loop_);
}
// port: FunctionTypeTest#testBindSignature
#[test]
fn test_bind_signature() {
    let mut f = BaseJSTypeTestCase::new();
    let fn_ = FunctionTypeBuilder::new()
        .with_type_of_this(f.date_type)
        .with_parameters(f.reg.create_parameters(&[f.string_type, f.number_type]))
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    let bind = fn_.get_property_type(&mut f.reg, &f.ast, "bind");
    assert_eq!(
        bind.to_string(&mut f.reg, &f.ast),
        "function((Date|null|undefined), string=, number=): function(...?): boolean"
    );
}
// port: FunctionTypeTest#testCallSignature1
#[test]
fn test_call_signature1() {
    let mut f = BaseJSTypeTestCase::new();
    let fn_ = FunctionTypeBuilder::new()
        .with_type_of_this(f.date_type)
        .with_parameters(f.reg.create_parameters(&[f.string_type, f.number_type]))
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    let call = fn_.get_property_type(&mut f.reg, &f.ast, "call");
    assert_eq!(
        call.to_string(&mut f.reg, &f.ast),
        "function((Date|null|undefined), string, number): boolean"
    );
}
// port: FunctionTypeTest#testCallSignature2
#[test]
fn test_call_signature2() {
    let mut f = BaseJSTypeTestCase::new();
    let fn_ = FunctionTypeBuilder::new()
        .with_type_of_this(f.date_type)
        .with_parameters(f.reg.create_parameters(&[]))
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    let call = fn_.get_property_type(&mut f.reg, &f.ast, "call");
    assert_eq!(
        call.to_string(&mut f.reg, &f.ast),
        "function((Date|null)=): boolean"
    );
}
// port: FunctionTypeTest#testTemplatedFunctionDerivedFunctions
#[test]
fn test_templated_function_derived_functions() {
    let mut f = BaseJSTypeTestCase::new();
    let template = f.reg.create_template_type(&f.ast, "T");
    let fn_ = FunctionTypeBuilder::new()
        .with_type_of_this(template)
        .with_template_keys(&mut f.reg, &f.ast, vec![template])
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    let call = fn_.get_property_type(&mut f.reg, &f.ast, "call");
    assert_eq!(template_keys_string(&mut f, call), "[T]");
    let apply = fn_.get_property_type(&mut f.reg, &f.ast, "apply");
    assert_eq!(template_keys_string(&mut f, apply), "[T]");
    let bind = fn_.get_property_type(&mut f.reg, &f.ast, "bind");
    assert_eq!(template_keys_string(&mut f, bind), "[T]");
    let bind_return = fn_.get_bind_return_type(&mut f.reg, &f.ast, 0);
    assert_eq!(template_keys_string(&mut f, bind_return), "[T]");
}
fn template_keys_string(f: &mut BaseJSTypeTestCase, type_: TypeId) -> String {
    let strings: Vec<_> = type_
        .get_template_type_map(&f.reg)
        .get_template_keys()
        .iter()
        .map(|t| t.to_string(&mut f.reg, &f.ast))
        .collect();
    format!("[{}]", strings.join(", "))
}
// port: FunctionTypeTest#testPrint_ordinaryFunction_withTemplatedThis
#[test]
fn test_print_ordinary_function_with_templated_this() {
    let mut f = BaseJSTypeTestCase::new();
    let template = f.reg.create_template_type(&f.ast, "T");
    let fn_ = FunctionTypeBuilder::new()
        .with_type_of_this(template)
        .with_return_type(f.boolean_type)
        .build(&mut f.reg, &f.ast);
    assert_eq!(
        fn_.to_string(&mut f.reg, &f.ast),
        "function(this:T, ...?): boolean"
    );
}
// port: FunctionTypeTest#testPrint_constructorFunction_withoutSource
#[test]
fn test_print_constructor_function_without_source() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let fn_ = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Foo")
        .build(&mut f.reg, &f.ast);
    assert_eq!(
        fn_.to_string(&mut f.reg, &f.ast),
        "function(new:Foo, ...?): ?"
    );
    closer.close(&mut f.reg, &f.ast);
}
// port: FunctionTypeTest#testPrint_interfaceFunction_withoutSource
#[test]
fn test_print_interface_function_without_source() {
    let mut f = BaseJSTypeTestCase::new();
    let fn_ = FunctionTypeBuilder::new()
        .for_interface()
        .with_name("Foo")
        .build(&mut f.reg, &f.ast);
    assert_eq!(fn_.to_string(&mut f.reg, &f.ast), "function(this:Foo): ?");
}
// port: FunctionTypeTest#testPrint_constructorFunction_withSource
#[test]
fn test_print_constructor_function_with_source() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let name = IR::name(&mut f.ast, "");
    let params = IR::param_list(&mut f.ast, &[]);
    let block = IR::block(&mut f.ast);
    let source = IR::function(&mut f.ast, name, params, block);
    let fn_ = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Foo")
        .with_source_node(Some(source))
        .build(&mut f.reg, &f.ast);
    assert_eq!(fn_.to_string(&mut f.reg, &f.ast), "(typeof Foo)");
    closer.close(&mut f.reg, &f.ast);
}
// port: FunctionTypeTest#testPrint_interfaceFunction_withSource
#[test]
fn test_print_interface_function_with_source() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let name = IR::name(&mut f.ast, "");
    let params = IR::param_list(&mut f.ast, &[]);
    let block = IR::block(&mut f.ast);
    let source = IR::function(&mut f.ast, name, params, block);
    let fn_ = FunctionTypeBuilder::new()
        .for_interface()
        .with_name("Foo")
        .with_source_node(Some(source))
        .build(&mut f.reg, &f.ast);
    assert_eq!(fn_.to_string(&mut f.reg, &f.ast), "(typeof Foo)");
    closer.close(&mut f.reg, &f.ast);
}
// port: FunctionTypeTest#testSetImplementsOnInterface
#[test]
fn test_set_implements_on_interface() {
    let mut f = BaseJSTypeTestCase::new();
    let iface = create_interface_type(&mut f, "I", vec![], false);
    let sub_iface = create_interface_type(&mut f, "SubI", vec![], false);
    let instance = iface.get_instance_type(&f.reg).unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || sub_iface.set_implemented_interfaces(&mut f.reg, &f.ast, vec![instance])
        ))
        .is_err()
    );
}
