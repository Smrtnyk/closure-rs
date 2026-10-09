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
//   test/com/google/javascript/rhino/jstype/JSTypeRegistryTest.java.

use closure_jstype::{
    TypeId,
    function_type::{FunctionType, FunctionTypeBuilder},
    js_type::JSType,
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    object_type::ObjectType,
    simple_slot::SimpleSlot,
    static_typed_ref::StaticTypedRef,
    static_typed_scope::StaticTypedScope,
    static_typed_slot::StaticTypedSlot,
    testing::{map_based_scope::MapBasedScope, type_subject::TypeSubject},
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    error_reporter::NullErrorReporter,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::{Arc, OnceLock};

// port: JSTypeRegistryTest#setUp
fn set_up() -> (Ast, JSTypeRegistry) {
    let mut ast = Ast::new();
    let mut registry = JSTypeRegistry::new(&mut ast, Box::new(NullErrorReporter), Vec::new());
    registry.get_resolver().open_for_definition();
    (ast, registry)
}
// port: JSTypeRegistryTest#union
fn union(reg: &mut JSTypeRegistry, ast: &Ast, types: &[JSTypeNative]) -> TypeId {
    reg.create_union_type_from_native(ast, types)
}
// port: JSTypeRegistryTest#getReadableTypeNameHelper
fn get_readable_type_name_helper(
    reg: &mut JSTypeRegistry,
    ast: &mut Ast,
    type_: TypeId,
    deref: bool,
) -> String {
    let n = ast.new_node(Token::ADD);
    n.set_jstype(ast, Some(type_));
    reg.get_readable_js_type_name(ast, n, deref)
}

// port: JSTypeRegistryTest#testGetBuiltInType_boolean
#[test]
fn test_get_built_in_type_boolean() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "boolean").unwrap();
    let native = registry.get_native_type(JSTypeNative::BOOLEAN_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_bigint
#[test]
fn test_get_built_in_type_bigint() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "bigint").unwrap();
    let native = registry.get_native_type(JSTypeNative::BIGINT_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_iterable
#[test]
fn test_get_built_in_type_iterable() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "Iterable").unwrap();
    let native = registry.get_native_type(JSTypeNative::ITERABLE_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_iterator
#[test]
fn test_get_built_in_type_iterator() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "Iterator").unwrap();
    let native = registry.get_native_type(JSTypeNative::ITERATOR_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_iteratorLike
#[test]
fn test_get_built_in_type_iterator_like() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "IteratorLike").unwrap();
    let native = registry.get_native_type(JSTypeNative::ITERATOR_LIKE_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_generator
#[test]
fn test_get_built_in_type_generator() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "Generator").unwrap();
    let native = registry.get_native_type(JSTypeNative::GENERATOR_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_async_iterable
#[test]
fn test_get_built_in_type_async_iterable() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "AsyncIterable").unwrap();
    let native = registry.get_native_type(JSTypeNative::ASYNC_ITERABLE_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_async_iterator
#[test]
fn test_get_built_in_type_async_iterator() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "AsyncIterator").unwrap();
    let native = registry.get_native_type(JSTypeNative::ASYNC_ITERATOR_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_async_generator
#[test]
fn test_get_built_in_type_async_generator() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "AsyncGenerator").unwrap();
    let native = registry.get_native_type(JSTypeNative::ASYNC_GENERATOR_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_gbigint
#[test]
fn test_get_built_in_type_gbigint() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "gbigint").unwrap();
    let native = registry.get_native_type(JSTypeNative::GBIGINT_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuildInType_iTemplateArray
#[test]
fn test_get_build_in_type_i_template_array() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "ITemplateArray").unwrap();
    let native = registry.get_native_type(JSTypeNative::I_TEMPLATE_ARRAY_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_ReadonlyMap
#[test]
fn test_get_built_in_type_readonly_map() {
    let (ast, mut registry) = set_up();
    let type_ = registry.get_global_type(&ast, "ReadonlyMap").unwrap();
    let native = registry.get_native_type(JSTypeNative::READONLY_MAP_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}

// port: JSTypeRegistryTest#testIteratorExtendsIteratorLike
#[test]
fn test_iterator_extends_iterator_like() {
    let (ast, mut registry) = set_up();
    let iterator = registry.get_native_type(JSTypeNative::ITERATOR_TYPE);
    let iterator_like = registry.get_native_type(JSTypeNative::ITERATOR_LIKE_TYPE);
    assert!(iterator.is_subtype_of(&mut registry, &ast, iterator_like));
}
// port: JSTypeRegistryTest#testGetBuiltInType_gbigint_unification
#[test]
fn test_get_built_in_type_gbigint_unification() {
    let (ast, mut registry) = set_up();
    let interface_type = FunctionTypeBuilder::new()
        .for_interface()
        .with_name("gbigint")
        .build(&mut registry, &ast);
    registry.declare_type(
        &ast,
        None,
        "gbigint",
        interface_type.get_instance_type(&registry).unwrap(),
    );
    let type_ = registry.get_global_type(&ast, "gbigint").unwrap();
    let native = registry.get_native_type(JSTypeNative::GBIGINT_TYPE);
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, native);
}
// port: JSTypeRegistryTest#testGetBuiltInType_Promise
#[test]
fn test_get_built_in_type_promise() {
    let (ast, mut registry) = set_up();
    let promise_type = registry.get_native_object_type(JSTypeNative::PROMISE_TYPE);
    let type_ = registry.get_global_type(&ast, "Promise");
    TypeSubject::assert_type(type_).is_equal_to(&mut registry, &ast, promise_type);
    let promise_ctor = promise_type.get_constructor(&registry).unwrap();
    let param_list = promise_ctor.get_parameters(&registry);
    assert_eq!(param_list.len(), 1);
    let first_parameter = &param_list[0];
    let param_type = first_parameter
        .get_jstype()
        .to_maybe_function_type(&registry)
        .unwrap();
    assert_eq!(
        param_type.to_string(&mut registry, &ast),
        "function(function((IThenable<TYPE>|TYPE|null|{then: ?})=): ?, function(*=): ?): ?"
    );
}
// port: JSTypeRegistryTest#testGetDeclaredType
#[test]
fn test_get_declared_type() {
    let (mut ast, mut registry) = set_up();
    let type_ = registry.create_anonymous_object_type(&ast, None);
    let name = "Foo";
    registry.declare_type(&ast, None, name, type_);
    let actual = registry.get_type(&ast, None, name);
    TypeSubject::assert_type(actual).is_equal_to(&mut registry, &ast, type_);
    let mut registry2 = JSTypeRegistry::new(&mut ast, Box::new(NullErrorReporter), Vec::new());
    assert!(registry2.get_type(&ast, None, name).is_none());
    let actual = registry.get_type(&ast, None, name);
    TypeSubject::assert_type(actual).is_equal_to(&mut registry, &ast, type_);
}
// port: JSTypeRegistryTest#testReadableTypeName
#[test]
fn test_readable_type_name() {
    let (mut ast, mut registry) = set_up();
    let all = registry.get_native_type(JSTypeNative::ALL_TYPE);
    assert_eq!(
        get_readable_type_name_helper(&mut registry, &mut ast, all, false),
        "*"
    );
    let boolean = registry.get_native_type(JSTypeNative::BOOLEAN_TYPE);
    assert_eq!(
        get_readable_type_name_helper(&mut registry, &mut ast, boolean, false),
        "boolean"
    );
    let boolean_object = registry.get_native_type(JSTypeNative::BOOLEAN_OBJECT_TYPE);
    assert_eq!(
        get_readable_type_name_helper(&mut registry, &mut ast, boolean_object, false),
        "Boolean"
    );
    let boolean_function = registry.get_native_type(JSTypeNative::BOOLEAN_OBJECT_FUNCTION_TYPE);
    assert_eq!(
        get_readable_type_name_helper(&mut registry, &mut ast, boolean_function, false),
        "function"
    );
    let null_void = registry.get_native_type(JSTypeNative::NULL_VOID);
    assert_eq!(
        get_readable_type_name_helper(&mut registry, &mut ast, null_void, false),
        "(null|undefined)"
    );
    assert_eq!(
        get_readable_type_name_helper(&mut registry, &mut ast, null_void, true),
        "(null|undefined)"
    );
    let types = [
        JSTypeNative::NUMBER_TYPE,
        JSTypeNative::STRING_TYPE,
        JSTypeNative::NULL_TYPE,
    ];
    let t = union(&mut registry, &ast, &types);
    assert_eq!(
        get_readable_type_name_helper(&mut registry, &mut ast, t, false),
        "(number|string|null)"
    );
    let t = union(&mut registry, &ast, &types);
    assert_eq!(
        get_readable_type_name_helper(&mut registry, &mut ast, t, true),
        "(Number|String)"
    );
}
fn bang(ast: &mut Ast, name: &str) -> NodeId {
    let string = ast.new_string(name);
    ast.new_node_with_child(Token::BANG, string)
}
// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_createsNamedTypeIfNameIsUndefined
#[test]
fn test_create_type_from_comment_node_creates_named_type_if_name_is_undefined() {
    let (mut ast, mut registry) = set_up();
    let global_scope = Arc::new(MapBasedScope::empty_scope());
    let n = bang(&mut ast, "a.b.c");
    let type_ = registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "srcfile.js",
        Some(global_scope),
    );
    assert!(type_.is_named_type(&registry));
}
// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_multipleLookupsInSameScope
#[test]
fn test_create_type_from_comment_node_multiple_lookups_in_same_scope() {
    let (mut ast, mut registry) = set_up();
    let global_scope = Arc::new(MapBasedScope::empty_scope());
    let n = bang(&mut ast, "Foo");
    let type_one = registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "srcfile.js",
        Some(global_scope.clone()),
    );
    let n = bang(&mut ast, "Foo");
    let type_two = registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "srcfile.js",
        Some(global_scope),
    );
    assert!(type_one.is_named_type(&registry));
    assert!(type_two.is_named_type(&registry));
    TypeSubject::assert_type(type_one).is_equal_to(&mut registry, &ast, type_two);
}

struct TestScope {
    root: NodeId,
    parent: Option<Arc<dyn StaticTypedScope>>,
    // Slot initialization is deferred until after scope construction, as in the Java helper.
    slots: OnceLock<IndexMap<JsString, ScopedSlot>>,
    reserved: IndexSet<JsString>,
}
struct ScopedSlot {
    slot: SimpleSlot,
    scope: Arc<TestScope>,
}
// port: JSTypeRegistryTest#createStaticTypedScope
fn create_static_typed_scope(
    root: NodeId,
    parent: Option<Arc<dyn StaticTypedScope>>,
    slots: IndexMap<JsString, SimpleSlot>,
    reserved: IndexSet<JsString>,
) -> Arc<TestScope> {
    let scope = Arc::new(TestScope {
        root,
        parent,
        slots: OnceLock::new(),
        reserved,
    });
    if !slots.is_empty() {
        let slots = slots
            .into_iter()
            .map(|(name, slot)| {
                (
                    name,
                    ScopedSlot {
                        slot,
                        scope: scope.clone(),
                    },
                )
            })
            .collect();
        scope
            .slots
            .set(slots)
            .unwrap_or_else(|_| panic!("slot map already initialized"));
    }
    scope
}
impl StaticTypedScope for TestScope {
    // port: JSTypeRegistryTest#getRootNode
    fn get_root_node(&self) -> Option<NodeId> {
        Some(self.root)
    }
    // port: JSTypeRegistryTest#getParentScope
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope> {
        self.parent.as_deref()
    }
    // port: JSTypeRegistryTest#getSlot
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.slots
            .get()
            .and_then(|slots| slots.get(name))
            .map(|s| s as &dyn StaticTypedSlot)
    }
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.get_slot(name)
    }
    fn get_type_of_this(&self, _reg: &JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        None
    }
    // port: JSTypeRegistryTest#getTopmostScopeOfEventualDeclaration
    fn get_topmost_scope_of_eventual_declaration(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedScope> {
        if self
            .slots
            .get()
            .is_some_and(|slots| slots.contains_key(name))
            || self.reserved.contains(name)
        {
            Some(self)
        } else {
            self.parent
                .as_ref()
                .and_then(|p| p.get_topmost_scope_of_eventual_declaration(reg, ast, name))
        }
    }
}
impl StaticTypedSlot for ScopedSlot {
    fn get_name(&self, reg: &JSTypeRegistry) -> JsString {
        self.slot.get_name(reg)
    }
    fn get_type(&self, reg: &JSTypeRegistry) -> Option<TypeId> {
        self.slot.get_type(reg)
    }
    fn is_type_inferred(&self, reg: &JSTypeRegistry) -> bool {
        self.slot.is_type_inferred(reg)
    }
    fn get_declaration(&self, _reg: &JSTypeRegistry) -> Option<&dyn StaticTypedRef> {
        None
    }
    fn get_jsdoc_info(&self, _reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>> {
        None
    }
    // port: JSTypeRegistryTest#getScope
    fn get_scope(&self, _reg: &JSTypeRegistry) -> Option<&dyn StaticTypedScope> {
        Some(self.scope.as_ref())
    }
}

// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_usesGlobalTypeIfExists
#[test]
fn test_create_type_from_comment_node_uses_global_type_if_exists() {
    let (mut ast, mut registry) = set_up();
    let global = create_static_typed_scope(
        ast.new_node(Token::ROOT),
        None,
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );
    let unknown = registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
    registry.declare_type(&ast, Some(global.as_ref()), "Foo", unknown);
    let local = create_static_typed_scope(
        ast.new_node(Token::BLOCK),
        Some(global),
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );
    let n = bang(&mut ast, "Foo");
    let type_ =
        registry.create_type_from_comment_node_with_location(&ast, n, "srcfile.js", Some(local));
    assert!(!type_.is_named_type(&registry));
}
// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_createsNamedTypeIfLocalShadowsGlobalType
#[test]
fn test_create_type_from_comment_node_creates_named_type_if_local_shadows_global_type() {
    let (mut ast, mut registry) = set_up();
    let unknown = registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
    let global = create_static_typed_scope(
        ast.new_node(Token::ROOT),
        None,
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );
    registry.declare_type(&ast, Some(global.as_ref()), "Foo", unknown);
    let local = create_static_typed_scope(
        ast.new_node(Token::BLOCK),
        Some(global),
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );
    let slots = IndexMap::<_, _>::from_iter([(
        "Foo".into(),
        ScopedSlot {
            slot: SimpleSlot::new("Foo", unknown, true),
            scope: local.clone(),
        },
    )]);
    local
        .slots
        .set(slots)
        .unwrap_or_else(|_| panic!("slot map already initialized"));
    let n = bang(&mut ast, "Foo");
    let type_ =
        registry.create_type_from_comment_node_with_location(&ast, n, "srcfile.js", Some(local));
    assert!(type_.is_named_type(&registry));
}
// port: JSTypeRegistryTest#testNativeTypesAreUnique
#[test]
fn test_native_types_are_unique() {
    let (ast, mut registry) = set_up();
    for n1 in JSTypeNative::VALUES {
        for n2 in JSTypeNative::VALUES {
            let t1 = registry.get_native_type(n1);
            let t2 = registry.get_native_type(n2);
            if !t1.equals(&mut registry, &ast, t2) {
                continue;
            }
            assert_eq!(t1, t2);
            assert_eq!(n1, n2);
        }
    }
}
// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_usesTopMostScopeOfName
#[test]
fn test_create_type_from_comment_node_uses_top_most_scope_of_name() {
    let (mut ast, mut registry) = set_up();
    let global = create_static_typed_scope(
        ast.new_node(Token::ROOT),
        None,
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );
    let unknown = registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
    registry.declare_type(&ast, Some(global.as_ref()), "Foo", unknown);
    let local = create_static_typed_scope(
        ast.new_node(Token::BLOCK),
        Some(global),
        IndexMap::<_, _>::default(),
        IndexSet::<_>::from_iter(["Foo".into()]),
    );
    let n = bang(&mut ast, "Foo");
    let type_ =
        registry.create_type_from_comment_node_with_location(&ast, n, "srcfile.js", Some(local));
    assert!(type_.is_named_type(&registry));
}
// port: JSTypeRegistryTest#testGetBuiltInType_Map
#[test]
fn test_get_built_in_type_map() {
    let (ast, mut registry) = set_up();
    let map_type = registry.get_native_object_type(JSTypeNative::MAP_TYPE);
    let global = registry.get_global_type(&ast, "Map");
    TypeSubject::assert_type(global).is_equal_to(&mut registry, &ast, map_type);
    let readonly_map_type = registry.get_native_object_type(JSTypeNative::READONLY_MAP_TYPE);
    assert_eq!(
        map_type
            .get_ctor_implemented_interfaces(&mut registry, &ast)
            .len(),
        1
    );
    let first =
        map_type.get_ctor_implemented_interfaces(&mut registry, &ast)[0].get_raw_type(&registry);
    assert!(first.equals(&mut registry, &ast, readonly_map_type));
    let param_list = map_type
        .get_constructor(&registry)
        .unwrap()
        .get_parameters(&registry);
    assert_eq!(param_list.len(), 1);
    assert!(param_list[0].is_optional());
    assert_eq!(
        param_list[0].get_jstype().to_string(&mut registry, &ast),
        "(Array<Array<(K|V)>>|Iterable<Array<(K|V)>,?,?>|null|undefined)"
    );
}
