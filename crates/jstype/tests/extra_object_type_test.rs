/*
 * Copyright 2026 The closure-rs Authors.
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

//! Additional regressions from the pinned Java implementation and printing oracle.
use closure_jstype::{
    arrow_type::ArrowType,
    function_param_builder::FunctionParamBuilder,
    function_type::{FunctionTypeBuilder, Parameter},
    js_type::{JSType, Nullability},
    known_symbol_type::KnownSymbolType,
    property::{Property, PropertyId, PropertyKey},
    property_map::PropertyMap,
    template_type::TemplateType,
    testing::base_js_type_test_case::BaseJSTypeTestCase,
};

#[test]
fn extra_arrow_printing_preserves_java_string_cast() {
    let mut f = BaseJSTypeTestCase::new();
    let mut params = FunctionParamBuilder::new();
    params.add_required_params(&[f.number_type]);
    params.add_optional_params(&mut f.reg, &f.ast, &[f.boolean_type]);
    params.add_var_args(f.string_type);
    let arrow = ArrowType::new(
        &mut f.reg,
        &f.ast,
        Some(params.build()),
        Some(f.string_type),
        true,
    );
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        arrow.to_string(&mut f.reg, &f.ast)
    }))
    .unwrap_err();
    let message = thrown
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| thrown.downcast_ref::<&str>().copied())
        .unwrap();
    assert_eq!(
        message,
        "class com.google.javascript.rhino.jstype.AutoValue_FunctionType_Parameter cannot be cast to class java.lang.String (com.google.javascript.rhino.jstype.AutoValue_FunctionType_Parameter is in unnamed module of loader 'app'; java.lang.String is in module java.base of loader 'bootstrap')"
    );
    let empty = ArrowType::new(&mut f.reg, &f.ast, Some(vec![]), Some(f.string_type), false);
    assert_eq!(empty.to_string(&mut f.reg, &f.ast), "() -> string");
    assert_eq!(
        arrow.to_annotation_string(&mut f.reg, &f.ast, Nullability::EXPLICIT),
        "[ArrowType]"
    );
}

#[test]
fn extra_property_and_delegate_printing() {
    let mut f = BaseJSTypeTestCase::new();
    let symbol = KnownSymbolType::new(&mut f.reg, &f.ast, "Symbol.foo");
    let property = PropertyId::new(
        &mut f.reg,
        PropertyKey::Symbol(symbol),
        f.number_type,
        false,
        None,
    );
    assert_eq!(property.get_name(&f.reg), "Symbol.foo");
    assert_eq!(
        property.to_string(&mut f.reg, &f.ast),
        "Property {  name: Symbol.foo, type:number, inferred: false}"
    );
    let delegate = f
        .reg
        .create_object_type(&f.ast, "Foo(Proxy).prototype", Some(f.object_type));
    assert_eq!(
        delegate.to_string(&mut f.reg, &f.ast),
        "Foo(Proxy).prototype"
    );
    assert_eq!(
        delegate.to_annotation_string(&mut f.reg, &f.ast, Nullability::EXPLICIT),
        "!Foo.prototype"
    );
    assert_eq!(
        delegate.to_annotation_string(&mut f.reg, &f.ast, Nullability::IMPLICIT),
        "Foo.prototype"
    );
}

#[test]
fn extra_property_slot_aliasing_and_nullable_map_owner() {
    let mut f = BaseJSTypeTestCase::new();
    let property = PropertyId::new(&mut f.reg, "x", f.number_type, false, None);
    let slot: &dyn closure_jstype::static_typed_slot::StaticTypedSlot = &property;
    assert_eq!(slot.get_type(&f.reg), Some(f.number_type));
    property.set_type(&mut f.reg, f.string_type);
    assert_eq!(slot.get_type(&f.reg), Some(f.string_type));
    let reference: &dyn closure_jstype::static_typed_ref::StaticTypedRef = &property;
    assert_eq!(
        reference.get_symbol(&f.reg).get_type(&f.reg),
        Some(f.string_type)
    );
    let mut map = PropertyMap::new();
    map.put_property(&mut f.reg, &f.ast, "x", property);
    let owned = map.find_closest(&mut f.reg, &f.ast, "x").unwrap();
    assert_eq!(owned.get_owner(), None);
    assert_eq!(owned.get_value(), property);
}

#[test]
fn extra_deeply_unwrap_preserves_null_reference() {
    let mut f = BaseJSTypeTestCase::new();
    let proxy = closure_jstype::proxy_object_type::create(&mut f.reg, &f.ast, f.number_type, None);
    assert_eq!(
        closure_jstype::object_type::deeply_unwrap(proxy, &f.reg),
        None
    );
    assert_eq!(
        closure_jstype::object_type::deeply_unwrap(None, &f.reg),
        None
    );
    assert_eq!(
        closure_jstype::object_type::deeply_unwrap(f.object_type, &f.reg),
        Some(f.object_type)
    );
}

#[test]
fn extra_parameter_equality_uses_structural_type_equality() {
    let mut f = BaseJSTypeTestCase::new();
    let first_type = f.reg.create_function_type(&f.ast, f.number_type, &[]);
    let second_type = f.reg.create_function_type(&f.ast, f.number_type, &[]);
    assert_ne!(first_type, second_type);
    let first = Parameter::create(first_type, true, false);
    let second = Parameter::create(second_type, true, false);
    assert!(first.equals(&mut f.reg, &f.ast, Some(&second)));
    assert_eq!(first.hash_code(&f.reg), second.hash_code(&f.reg));
    assert!(!first.equals(&mut f.reg, &f.ast, None));
    assert!(!first.equals(
        &mut f.reg,
        &f.ast,
        Some(&Parameter::create(second_type, false, false))
    ));
    assert!(!first.equals(
        &mut f.reg,
        &f.ast,
        Some(&Parameter::create(second_type, true, true))
    ));
}

#[test]
fn extra_function_builder_snapshots_template_bounds() {
    let mut f = BaseJSTypeTestCase::new();
    let key = f
        .reg
        .create_template_type_with_bound(&f.ast, "T", f.number_type);
    let builder = FunctionTypeBuilder::new().with_template_keys(&mut f.reg, &f.ast, vec![key]);
    key.set_bound(&mut f.reg, f.string_type);
    let function = builder.build(&mut f.reg, &f.ast);
    let map = function.get_template_type_map(&f.reg);
    assert_eq!(
        map.to_string(&mut f.reg, &f.ast),
        "{ (T extends string,,number) }"
    );
    assert_eq!(map.get_resolved_template_type(&mut f.reg, &f.ast, key), key);
    let without_keys = FunctionTypeBuilder::new()
        .with_template_keys(&mut f.reg, &f.ast, None)
        .build(&mut f.reg, &f.ast);
    assert!(std::sync::Arc::ptr_eq(
        &without_keys.get_template_type_map(&f.reg),
        &f.reg.get_empty_template_type_map()
    ));
}

// A NamedType whose referenced type is a TemplatizedType answers
// toMaybeTemplatizedType, but Java dispatches getCtorImplementedInterfaces,
// getCtorExtendedInterfaces and getPropertyType through ProxyObjectType, not
// TemplatizedType's overrides (D2 test262 hashbang case crashed with a
// ClassCastException in TemplatizedType#getCtorImplementedInterfaces).
#[test]
fn extra_named_type_wrapping_templatized_type_dispatches_as_proxy() {
    use closure_jstype::{
        TypeId, function_type::FunctionType, object_type::ObjectType,
        proxy_object_type::ProxyObjectType,
    };
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let template_t = f.reg.create_template_type(&f.ast, "T");
    let ctor = f.reg.create_constructor_type(
        &f.ast,
        Some("Foo".into()),
        None,
        None,
        None,
        Some(vec![template_t]),
        false,
    );
    let raw_type: TypeId = ctor.get_instance_type(&f.reg).unwrap();
    raw_type.define_declared_property(&mut f.reg, &f.ast, "property", template_t, None);
    let templatized_number = f.create_templatized_type(raw_type, &[f.number_type]);
    closer.close(&mut f.reg, &f.ast);
    let named = f.reg.create_named_type(&f.ast, None, "Y", "", -1, -1);
    named.set_referenced_type(&mut f.reg, templatized_number);

    assert!(named.is_templatized_type(&f.reg));
    assert!(
        named
            .get_ctor_implemented_interfaces(&mut f.reg, &f.ast)
            .is_empty()
    );
    assert!(
        named
            .get_ctor_extended_interfaces(&mut f.reg, &f.ast)
            .is_empty()
    );
    assert!(!named.detect_inheritance_cycle(&mut f.reg, &f.ast));
    // ObjectType#getRawType unwraps toMaybeTemplatizedType() to its referenced type.
    assert_eq!(named.get_raw_type(&f.reg), raw_type);
    // ProxyObjectType#getPropertyMap is the raw type's map: no TemplateTypeReplacer runs.
    let through_proxy = named.get_property_type(&mut f.reg, &f.ast, "property");
    assert_eq!(through_proxy, template_t);
    let direct = templatized_number.get_property_type(&mut f.reg, &f.ast, "property");
    assert!(direct.equals(&mut f.reg, &f.ast, f.number_type));
}
