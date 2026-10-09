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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/rhino/jstype/PropertyMapTest.java.

use closure_jstype::{
    JSTypeRegistry, TypeId,
    js_type::JSType,
    js_type_native::JSTypeNative,
    known_symbol_type::KnownSymbolType,
    object_type::ObjectType,
    property::{Property, PropertyId, PropertyKey},
    property_map::{AllKeys, PropertyMap},
};
use closure_rhino::{
    js_string::JsString, node::Ast, testing::test_error_reporter::TestErrorReporter,
};
use std::sync::Arc;

struct TestCase {
    ast: Ast,
    reg: JSTypeRegistry,
    symbol_key: PropertyKey,
    number_type: TypeId,
}
// port: PropertyMapTest#setUp
fn set_up() -> TestCase {
    let mut ast = Ast::new();
    let mut reg = JSTypeRegistry::new(
        &mut ast,
        Box::new(TestErrorReporter::new()),
        Vec::<JsString>::new(),
    );
    let symbol_key = PropertyKey::Symbol(KnownSymbolType::new(&mut reg, &ast, "Symbol.foo"));
    let number_type = reg.get_native_type(JSTypeNative::NUMBER_TYPE);
    TestCase {
        ast,
        reg,
        symbol_key,
        number_type,
    }
}

// port: PropertyMapTest#emptyMap_hasNoProperties
#[test]
fn empty_map_has_no_properties() {
    let mut f = set_up();
    let empty = PropertyMap::immutable_empty_map();
    assert_eq!(empty.get_properties_count(&mut f.reg, &f.ast), 0);
    assert!(empty.get_own_property_names().is_empty());
    assert!(empty.get_own_known_symbols().is_empty());
    let keys = empty.get_all_keys(&mut f.reg, &f.ast);
    assert_eq!(
        keys,
        AllKeys::new(
            Arc::new(std::collections::BTreeSet::new()),
            Arc::new(Vec::new())
        )
    );
    assert!(empty.get_primary_parent(&mut f.reg, &f.ast).is_none());
    assert!(
        empty
            .get_own_property(&mut f.reg, &f.ast, &"x".into())
            .is_none()
    );
    assert!(
        empty
            .get_own_property(&mut f.reg, &f.ast, &f.symbol_key)
            .is_none()
    );
}

// port: PropertyMapTest#getAllKeys_returnsBothSymbolAndStringKeys
#[test]
fn get_all_keys_returns_both_symbol_and_string_keys() {
    let mut f = set_up();
    let mut properties = PropertyMap::new();
    let p = PropertyId::new(&mut f.reg, "x", f.number_type, false, None);
    properties.put_property(&mut f.reg, &f.ast, "x", p);
    let p = PropertyId::new(&mut f.reg, f.symbol_key.clone(), f.number_type, false, None);
    properties.put_property(&mut f.reg, &f.ast, f.symbol_key.clone(), p);
    assert_eq!(
        properties
            .get_all_keys(&mut f.reg, &f.ast)
            .string_keys
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![JsString::from("x")]
    );
    let symbols = properties
        .get_all_keys(&mut f.reg, &f.ast)
        .known_symbol_keys;
    assert_eq!(symbols.len(), 1);
    assert!(symbols[0].equals(&mut f.reg, &f.ast, f.symbol_key.symbol()));
}
// port: PropertyMapTest#getAllKeys_cachesStringKeyResult
#[test]
fn get_all_keys_caches_string_key_result() {
    let mut f = set_up();
    let mut properties = PropertyMap::new();
    let p = PropertyId::new(&mut f.reg, "x", f.number_type, false, None);
    properties.put_property(&mut f.reg, &f.ast, "x", p);
    let result1 = properties.get_all_keys(&mut f.reg, &f.ast).string_keys;
    let result2 = properties.get_all_keys(&mut f.reg, &f.ast).string_keys;
    assert!(Arc::ptr_eq(&result1, &result2));
}
// port: PropertyMapTest#getAllKeys_cachesSymbolKeyResult
#[test]
fn get_all_keys_caches_symbol_key_result() {
    let mut f = set_up();
    let mut properties = PropertyMap::new();
    let p = PropertyId::new(&mut f.reg, f.symbol_key.clone(), f.number_type, false, None);
    properties.put_property(&mut f.reg, &f.ast, f.symbol_key.clone(), p);
    let result1 = properties
        .get_all_keys(&mut f.reg, &f.ast)
        .known_symbol_keys;
    let result2 = properties
        .get_all_keys(&mut f.reg, &f.ast)
        .known_symbol_keys;
    assert!(Arc::ptr_eq(&result1, &result2));
}
// port: PropertyMapTest#getAllKeys_beforeAndAfterMutation_stringKey_invalidatesCache
#[test]
fn get_all_keys_before_and_after_mutation_string_key_invalidates_cache() {
    let mut f = set_up();
    let mut properties = PropertyMap::new();
    assert!(
        properties
            .get_all_keys(&mut f.reg, &f.ast)
            .string_keys
            .is_empty()
    );
    let p = PropertyId::new(&mut f.reg, "x", f.number_type, false, None);
    properties.put_property(&mut f.reg, &f.ast, "x", p);
    assert_eq!(
        properties
            .get_all_keys(&mut f.reg, &f.ast)
            .string_keys
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![JsString::from("x")]
    );
}
// port: PropertyMapTest#getAllKeys_beforeAndAfterMutation_symbolKey_invalidatesCache
#[test]
fn get_all_keys_before_and_after_mutation_symbol_key_invalidates_cache() {
    let mut f = set_up();
    let mut properties = PropertyMap::new();
    assert!(
        properties
            .get_all_keys(&mut f.reg, &f.ast)
            .known_symbol_keys
            .is_empty()
    );
    let p = PropertyId::new(&mut f.reg, f.symbol_key.clone(), f.number_type, false, None);
    properties.put_property(&mut f.reg, &f.ast, f.symbol_key.clone(), p);
    let symbols = properties
        .get_all_keys(&mut f.reg, &f.ast)
        .known_symbol_keys;
    assert_eq!(symbols.len(), 1);
    assert!(symbols[0].equals(&mut f.reg, &f.ast, f.symbol_key.symbol()));
}
// port: PropertyMapTest#lookUpKeysFromParent_stringKey
#[test]
fn look_up_keys_from_parent_string_key() {
    let mut f = set_up();
    let parent_type = f
        .reg
        .create_record_type(&f.ast, [(JsString::from("x"), f.number_type)])
        .assert_object_type(&mut f.reg, &f.ast);
    let child_type = f.reg.create_object_type(&f.ast, "Child", Some(parent_type));
    let mut child = PropertyMap::new();
    child.set_parent_source(child_type);
    assert!(
        child
            .get_own_property(&mut f.reg, &f.ast, &"x".into())
            .is_none()
    );
    let type_ = child
        .find_closest(&mut f.reg, &f.ast, "x")
        .unwrap()
        .get_value()
        .get_type(&f.reg);
    assert!(type_.equals(&mut f.reg, &f.ast, f.number_type));
    let type_ = child
        .find_closest(
            &mut f.reg,
            &f.ast,
            closure_jstype::property::StringKey::new("x"),
        )
        .unwrap()
        .get_value()
        .get_type(&f.reg);
    assert!(type_.equals(&mut f.reg, &f.ast, f.number_type));
}
// port: PropertyMapTest#lookUpKeysFromParent_symbolKey
#[test]
fn look_up_keys_from_parent_symbol_key() {
    let mut f = set_up();
    let parent_type = f
        .reg
        .create_record_type(&f.ast, std::iter::empty::<(JsString, TypeId)>())
        .assert_object_type(&mut f.reg, &f.ast);
    parent_type.define_declared_property(
        &mut f.reg,
        &f.ast,
        f.symbol_key.clone(),
        f.number_type,
        None,
    );
    let child_type = f.reg.create_object_type(&f.ast, "Child", Some(parent_type));
    let mut child = PropertyMap::new();
    child.set_parent_source(child_type);
    assert!(
        child
            .get_own_property(&mut f.reg, &f.ast, &f.symbol_key)
            .is_none()
    );
    let type_ = child
        .find_closest(&mut f.reg, &f.ast, f.symbol_key.clone())
        .unwrap()
        .get_value()
        .get_type(&f.reg);
    assert!(type_.equals(&mut f.reg, &f.ast, f.number_type));
}
