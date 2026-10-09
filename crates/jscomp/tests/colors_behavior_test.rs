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

use closure_jscomp::{
    colors::{Color, ColorId, ColorRegistry, color_registry::REQUIRED_IDS, standard_colors as sc},
    testing::assert_that,
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{js_string::JsString, node::Ast, token::Token};
use std::panic::catch_unwind;

// oracle: corpus-cache/colors/behavior_vectors.txt (ColorBehaviorVectors.java)
#[test]
fn node_color_display_and_json_match_java() {
    let color = Color::single_builder()
        .set_id(ColorId::from_ascii("Foo"))
        .set_own_properties(IndexSet::<_>::from_iter([JsString::from(
            "quote\"property",
        )]))
        .build();
    let mut ast = Ast::new();
    let node = ast.new_string_with_token(Token::NAME, "f");
    node.set_color(&mut ast, Some(color.clone()));
    assert_eq!(
        node.to_string(&ast),
        r#"NAME f : Color{id=466f6f, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[quote"property], boxId=null, closureAssert=false, unionElements=[]}"#
    );
    assert_eq!(
        node.to_string_with_options(&ast, true, true, false),
        "NAME f"
    );
    let mut json = String::new();
    node.append_json_tree(&ast, &mut json).unwrap();
    assert_eq!(
        json,
        r#"{"token":"NAME","string":"f","typeString":"Color{id=466f6f, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[quote\"property], boxId=null, closureAssert=false, unionElements=[]}"}"#
    );
    let copied = ast.new_string_with_token(Token::NAME, "f");
    copied.copy_type_from(&mut ast, node);
    assert!(copied.get_color(&ast).unwrap().ptr_eq(&color));
    let cloned = node.clone_node(&mut ast);
    assert!(cloned.get_color(&ast).unwrap().ptr_eq(&color));
    let equivalent = ast.new_string_with_token(Token::NAME, "f");
    equivalent.set_color(
        &mut ast,
        Some(
            Color::single_builder()
                .set_id(ColorId::from_ascii("Foo"))
                .set_own_properties(IndexSet::<_>::from_iter([JsString::from(
                    "quote\"property",
                )]))
                .build(),
        ),
    );
    assert!(node.is_equivalent_to_typed(&ast, equivalent));
}

// oracle: corpus-cache/colors/behavior_vectors.txt (ColorBehaviorVectors.java)
#[test]
fn precondition_messages_match_java() {
    let failure = catch_unwind(|| Color::single_builder().build()).unwrap_err();
    assert_eq!(
        failure.downcast_ref::<String>().unwrap(),
        "Missing required properties: id"
    );
    let failure = catch_unwind(|| ColorId::from_ascii("😀")).unwrap_err();
    assert_eq!(failure.downcast_ref::<String>().unwrap(), "128512");
    let failure = catch_unwind(|| ColorId::from_ascii("aaaaaaa😀")).unwrap_err();
    assert_eq!(failure.downcast_ref::<String>().unwrap(), "9");
    let registry = ColorRegistry::builder()
        .set_default_native_colors_for_testing()
        .build();
    let failure = catch_unwind(|| registry.get(ColorId::from_ascii("Foo"))).unwrap_err();
    assert_eq!(
        failure.downcast_ref::<String>().unwrap(),
        "Missing Color for 466f6f"
    );
    assert!(catch_unwind(|| ColorRegistry::builder().build()).is_err());
    assert!(catch_unwind(|| Color::single_builder().set_id(sc::NUMBER.get_id()).build()).is_err());
}

// oracle: corpus-cache/colors/behavior_vectors.txt (ColorBehaviorVectors.java)
#[test]
fn subtract_null_or_void_preserves_java_reference_identity() {
    let nullable = Color::create_union(&IndexSet::<_>::from_iter([
        sc::NULL_OR_VOID.clone(),
        sc::NUMBER.clone(),
    ]));
    assert_that(nullable.subtract_null_or_void()).is_same_instance_as(&sc::NUMBER);
    let union = Color::create_union(&IndexSet::<_>::from_iter([
        sc::NULL_OR_VOID.clone(),
        sc::NUMBER.clone(),
        sc::STRING.clone(),
    ]));
    let subtracted = union.subtract_null_or_void();
    assert_that(union.subtract_null_or_void()).is_same_instance_as(&subtracted);
    let no_null = Color::create_union(&IndexSet::<_>::from_iter([
        sc::NUMBER.clone(),
        sc::STRING.clone(),
    ]));
    assert_that(no_null.subtract_null_or_void()).is_same_instance_as(&no_null);
    assert_that(Color::create_union(&IndexSet::<_>::from_iter([
        sc::NUMBER.clone()
    ])))
    .is_same_instance_as(&sc::NUMBER);
    assert!(catch_unwind(|| sc::NULL_OR_VOID.subtract_null_or_void()).is_err());
}

// Java Set.equals/hashCode ignore iteration order, including WTF-16 property names.
#[test]
fn structural_equality_and_hash_are_independent_of_set_order() {
    let a = JsString::from_units(vec![0xd800]);
    let b = JsString::from("b");
    let first = Color::single_builder()
        .set_id(ColorId::from_ascii("Obj"))
        .set_own_properties(IndexSet::<_>::from_iter([a.clone(), b.clone()]))
        .set_prototypes(IndexSet::<_>::from_iter([
            sc::TOP_OBJECT.clone(),
            sc::TOP_FUNCTION.clone(),
        ]))
        .build();
    let second = Color::single_builder()
        .set_id(ColorId::from_ascii("Obj"))
        .set_own_properties(IndexSet::<_>::from_iter([b, a]))
        .set_prototypes(IndexSet::<_>::from_iter([
            sc::TOP_FUNCTION.clone(),
            sc::TOP_OBJECT.clone(),
        ]))
        .build();
    assert!(!first.ptr_eq(&second));
    assert_eq!(first, second);
    assert_eq!(first.hash_code(), second.hash_code());
    assert_eq!(IndexSet::<_>::from_iter([first, second]).len(), 1);
    let union = Color::create_union(&IndexSet::<_>::from_iter([
        sc::NUMBER.clone(),
        sc::STRING.clone(),
    ]));
    let reversed = Color::create_union(&IndexSet::<_>::from_iter([
        sc::STRING.clone(),
        sc::NUMBER.clone(),
    ]));
    assert_eq!(union, reversed);
    assert_eq!(union.hash_code(), reversed.hash_code());
    assert_ne!(union.to_string(), reversed.to_string());
}

// Java linkedHashKeys/linkedHashSetValues preserve first insertion and remove duplicates.
#[test]
fn registry_preserves_multimap_order_and_builder_snapshots() {
    let subtype = Color::single_builder()
        .set_id(ColorId::from_ascii("Child"))
        .set_closure_assert(true)
        .set_constructor(true)
        .set_properties_keep_original_name(true)
        .set_own_properties(IndexSet::<_>::from_iter([JsString::from("own")]))
        .build();
    let mut builder = ColorRegistry::builder();
    builder
        .set_default_native_colors_for_testing()
        .add_disambiguation_edge(subtype.clone(), sc::TOP_FUNCTION.clone())
        .add_disambiguation_edge(subtype.clone(), sc::TOP_OBJECT.clone())
        .add_disambiguation_edge(subtype.clone(), sc::TOP_FUNCTION.clone())
        .add_mismatch_location(subtype.get_id(), "b.js:2".into())
        .add_mismatch_location(sc::NUMBER.get_id(), "a.js:1".into())
        .add_mismatch_location(subtype.get_id(), "a.js:1".into())
        .add_mismatch_location(subtype.get_id(), "b.js:2".into());
    let registry = builder.build();
    for id in REQUIRED_IDS.iter() {
        assert_eq!(registry.get(*id).get_id(), *id);
    }
    let supertypes = registry.get_disambiguation_supertypes(&subtype);
    assert_eq!(
        supertypes.iter().cloned().collect::<Vec<_>>(),
        vec![sc::TOP_FUNCTION.clone(), sc::TOP_OBJECT.clone()]
    );
    assert!(
        registry
            .get_disambiguation_supertypes(&sc::STRING)
            .is_empty()
    );
    let mismatches = registry.get_mismatch_locations_for_debugging();
    assert_eq!(
        mismatches.keys().copied().collect::<Vec<_>>(),
        vec![subtype.get_id(), sc::NUMBER.get_id()]
    );
    assert_eq!(
        mismatches[&subtype.get_id()]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["b.js:2", "a.js:1"]
    );
    builder
        .add_disambiguation_edge(subtype.clone(), sc::UNKNOWN.clone())
        .add_mismatch_location(subtype.get_id(), "later".into());
    assert_eq!(registry.get_disambiguation_supertypes(&subtype).len(), 2);
    assert_eq!(
        registry.get_mismatch_locations_for_debugging()[&subtype.get_id()].len(),
        2
    );
    let subject = assert_that(subtype.clone());
    subject.has_id(subtype.get_id());
    subject.is_closure_assert();
    subject.is_constructor();
    subject.properties_keep_original_name();
    subject
        .has_own_properties_set_that()
        .contains_exactly(&[JsString::from("own")]);
    subject
        .has_disambiguation_supertypes_that(&registry)
        .contains_exactly(&[sc::TOP_OBJECT.clone(), sc::TOP_FUNCTION.clone()]);
    subject.is_equal_to_subject(&assert_that(subtype));
    assert_that(None)
        .has_disambiguation_supertypes_that(&registry)
        .is_empty();
}
