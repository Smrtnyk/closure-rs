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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 030d049:
//   test/com/google/javascript/rhino/jstype/JSTypeRegistryTest.java.

use closure_jstype::{
    TypeId,
    function_type::{FunctionType, FunctionTypeBuilder},
    js_type::{JSType, Nullability},
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    object_type::ObjectType,
    simple_slot::SimpleSlot,
    static_typed_ref::StaticTypedRef,
    static_typed_scope::StaticTypedScope,
    static_typed_slot::StaticTypedSlot,
    testing::{
        abstract_static_scope::AbstractStaticScope,
        base_js_type_test_case::SharedTestErrorReporter, map_based_scope::MapBasedScope,
        type_subject::TypeSubject,
    },
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    error_reporter::NullErrorReporter,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    testing::test_error_reporter::TestErrorReporter,
    token::Token,
};
use std::sync::{Arc, Mutex, OnceLock};

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
// port: JSTypeRegistryTest#testGeneratorExtendsIterator
#[test]
fn test_generator_extends_iterator() {
    let (ast, mut registry) = set_up();
    let generator = registry.get_native_type(JSTypeNative::GENERATOR_TYPE);
    let iterator = registry.get_native_type(JSTypeNative::ITERATOR_TYPE);
    assert!(generator.is_subtype_of(&mut registry, &ast, iterator));
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

/// `new JSTypeRegistry(reporter)`: a registry whose resolver stays closed, reporting to a
/// TestErrorReporter the test verifies.
fn closed_registry_with_reporter(ast: &mut Ast) -> (JSTypeRegistry, Arc<Mutex<TestErrorReporter>>) {
    let reporter = Arc::new(Mutex::new(TestErrorReporter::new()));
    let registry = JSTypeRegistry::new(
        ast,
        Box::new(SharedTestErrorReporter(reporter.clone())),
        Vec::new(),
    );
    (registry, reporter)
}

// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_internedBuiltinTypesAndTemplates_closedResolver
#[test]
fn test_create_type_from_comment_node_interned_builtin_types_and_templates_closed_resolver() {
    let mut ast = Ast::new();
    let (mut closed_registry, reporter) = closed_registry_with_reporter(&mut ast);
    let global_scope: Arc<dyn StaticTypedScope> = create_static_typed_scope(
        ast.new_node(Token::ROOT),
        None,
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );

    // Builtin generic specializations (non-null via BANG, nullable via bare STRINGLIT/QMARK,
    // optional).
    let arg = ast.new_string("string");
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let n = bang_node(&mut ast, array);
    let non_null_array_str = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    let arg = ast.new_string("string");
    let n = templatized_comment_node(&mut ast, "Array", &[arg]);
    let nullable_array_str = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "b.js",
        Some(global_scope.clone()),
    );
    let arg = ast.new_string("string");
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let n = ast.new_node_with_child(Token::QMARK, array);
    let qmark_array_str = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "b.js",
        Some(global_scope.clone()),
    );
    let array = nullable_array_str_node(&mut ast);
    let n = ast.new_node_with_child(Token::EQUALS, array);
    let opt_array_str = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "b.js",
        Some(global_scope.clone()),
    );
    let arg = ast.new_node(Token::QMARK);
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let n = bang_node(&mut ast, array);
    let non_null_array_unknown = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "c.js",
        Some(global_scope.clone()),
    );
    let arg = ast.new_node(Token::QMARK);
    let n = templatized_comment_node(&mut ast, "Array", &[arg]);
    let array_unknown = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "c.js",
        Some(global_scope.clone()),
    );

    let array_type = closed_registry.get_native_object_type(JSTypeNative::ARRAY_TYPE);
    let string_type = closed_registry.get_native_type(JSTypeNative::STRING_TYPE);
    let expected = closed_registry.create_templatized_type(&ast, array_type, &[string_type]);
    TypeSubject::assert_type(non_null_array_str).is_equal_to(&mut closed_registry, &ast, expected);
    assert_eq!(
        non_null_array_str.to_string(&mut closed_registry, &ast),
        "Array<string>"
    );
    assert_eq!(
        non_null_array_str.to_annotation_string(&mut closed_registry, &ast, Nullability::EXPLICIT),
        "!Array<string>"
    );
    let expected = closed_registry.create_nullable_type(&ast, non_null_array_str);
    TypeSubject::assert_type(nullable_array_str).is_equal_to(&mut closed_registry, &ast, expected);
    TypeSubject::assert_type(qmark_array_str).is_equal_to(
        &mut closed_registry,
        &ast,
        nullable_array_str,
    );
    let expected = closed_registry.create_optional_type(&ast, nullable_array_str);
    TypeSubject::assert_type(opt_array_str).is_equal_to(&mut closed_registry, &ast, expected);
    let array_type = closed_registry.get_native_object_type(JSTypeNative::ARRAY_TYPE);
    let unknown_type = closed_registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
    let expected = closed_registry.create_templatized_type(&ast, array_type, &[unknown_type]);
    TypeSubject::assert_type(non_null_array_unknown).is_equal_to(
        &mut closed_registry,
        &ast,
        expected,
    );
    TypeSubject::assert_type(non_null_array_unknown).is_not_equal_to(
        &mut closed_registry,
        &ast,
        non_null_array_str,
    );
    assert_eq!(
        non_null_array_unknown.to_string(&mut closed_registry, &ast),
        "Array<?>"
    );
    assert_eq!(
        array_unknown.to_string(&mut closed_registry, &ast),
        "(Array<?>|null)"
    );

    // Repeated evaluation yields structurally equal types with identical template arguments.
    let arg = ast.new_string("string");
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let n = bang_node(&mut ast, array);
    let non_null_array_str2 =
        closed_registry.create_type_from_comment_node_with_location(&ast, n, "d.js", None);
    TypeSubject::assert_type(non_null_array_str2).is_equal_to(
        &mut closed_registry,
        &ast,
        non_null_array_str,
    );
    assert!(non_null_array_str2.is_resolved(&closed_registry));
    // containsExactlyElementsIn(...).inOrder(): element-wise JSType#equals, in order.
    let actual_values = non_null_array_str2
        .get_template_type_map(&closed_registry)
        .get_template_values()
        .to_vec();
    let expected_values = non_null_array_str
        .get_template_type_map(&closed_registry)
        .get_template_values()
        .to_vec();
    assert_eq!(actual_values.len(), expected_values.len());
    for (actual, expected) in actual_values.into_iter().zip(expected_values) {
        assert!(actual.equals(&mut closed_registry, &ast, expected));
    }

    // ReadonlyArray, Promise, IThenable, IIterableResult, ReadonlyMap, Map, and unions.
    let arg = ast.new_string("number");
    let readonly_array = templatized_comment_node(&mut ast, "ReadonlyArray", &[arg]);
    let n = bang_node(&mut ast, readonly_array);
    let readonly_arr_num = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    assert_eq!(
        readonly_arr_num.to_string(&mut closed_registry, &ast),
        "ReadonlyArray<number>"
    );

    let arg = ast.new_string("string");
    let promise = templatized_comment_node(&mut ast, "Promise", &[arg]);
    let n = bang_node(&mut ast, promise);
    let promise_str = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    assert_eq!(
        promise_str.to_string(&mut closed_registry, &ast),
        "Promise<string>"
    );

    let arg = ast.new_string("boolean");
    let ithenable = templatized_comment_node(&mut ast, "IThenable", &[arg]);
    let n = bang_node(&mut ast, ithenable);
    let ithenable_bool = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    assert_eq!(
        ithenable_bool.to_string(&mut closed_registry, &ast),
        "IThenable<boolean>"
    );

    let arg = ast.new_string("number");
    let iter_result = templatized_comment_node(&mut ast, "IIterableResult", &[arg]);
    let n = bang_node(&mut ast, iter_result);
    let iter_result_num = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    assert_eq!(
        iter_result_num.to_string(&mut closed_registry, &ast),
        "IIterableResult<number>"
    );

    let key = ast.new_string("string");
    let value = ast.new_string("number");
    let readonly_map = templatized_comment_node(&mut ast, "ReadonlyMap", &[key, value]);
    let n = bang_node(&mut ast, readonly_map);
    let readonly_map_str_num = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    assert_eq!(
        readonly_map_str_num.to_string(&mut closed_registry, &ast),
        "ReadonlyMap<string,number>"
    );

    let key = ast.new_string("string");
    let arg = ast.new_string("string");
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let value = bang_node(&mut ast, array);
    let map = templatized_comment_node(&mut ast, "Map", &[key, value]);
    let n = bang_node(&mut ast, map);
    let nested_map = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    assert_eq!(
        nested_map.to_string(&mut closed_registry, &ast),
        "Map<string,Array<string>>"
    );

    let union_node = ast.new_node(Token::PIPE);
    for name in [
        "string",
        "number",
        "boolean",
        "bigint",
        "null",
        "undefined",
        "symbol",
    ] {
        let child = ast.new_string(name);
        union_node.add_child_to_back(&mut ast, child);
    }
    let big_union = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        union_node,
        "a.js",
        Some(global_scope.clone()),
    );
    assert_eq!(
        big_union.to_string(&mut closed_registry, &ast),
        "(bigint|boolean|null|number|string|symbol|undefined)"
    );

    let n = ast.new_node(Token::STAR);
    let actual = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    let expected = closed_registry.get_native_type(JSTypeNative::ALL_TYPE);
    TypeSubject::assert_type(actual).is_equal_to(&mut closed_registry, &ast, expected);
    let n = ast.new_node(Token::QMARK);
    let actual = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    let expected = closed_registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
    TypeSubject::assert_type(actual).is_equal_to(&mut closed_registry, &ast, expected);
    let n = ast.new_node(Token::EMPTY);
    let actual = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    let expected = closed_registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
    TypeSubject::assert_type(actual).is_equal_to(&mut closed_registry, &ast, expected);
    let n = ast.new_string("null");
    let actual = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    let expected = closed_registry.get_native_type(JSTypeNative::NULL_TYPE);
    TypeSubject::assert_type(actual).is_equal_to(&mut closed_registry, &ast, expected);
    // !null removes null from the type, yielding bottom (None).
    let null = ast.new_string("null");
    let n = bang_node(&mut ast, null);
    let actual = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    let expected = closed_registry.get_native_type(JSTypeNative::NO_TYPE);
    TypeSubject::assert_type(actual).is_equal_to(&mut closed_registry, &ast, expected);
    let string = ast.new_string("string");
    let bang_string = bang_node(&mut ast, string);
    let n = ast.new_node_with_child(Token::QMARK, bang_string);
    let actual = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    let string_type = closed_registry.get_native_type(JSTypeNative::STRING_TYPE);
    let expected = closed_registry.create_nullable_type(&ast, string_type);
    TypeSubject::assert_type(actual).is_equal_to(&mut closed_registry, &ast, expected);
    let string = ast.new_string("string");
    let qmark_string = ast.new_node_with_child(Token::QMARK, string);
    let n = bang_node(&mut ast, qmark_string);
    let actual = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope),
    );
    let expected = closed_registry.get_native_type(JSTypeNative::STRING_TYPE);
    TypeSubject::assert_type(actual).is_equal_to(&mut closed_registry, &ast, expected);

    reporter
        .lock()
        .unwrap()
        .verify_has_encountered_all_warnings_and_errors();
}

/// The anonymous AbstractStaticScope of
/// testCreateTypeFromCommentNode_templateScopeShadowing_doesNotLeakCachedBuiltins.
struct TypeOfThisScope {
    // Java's getRootNode returns a new IR.block() on each call. Nothing is registered under this
    // scope's root, so one detached BLOCK answers the same lookups.
    root: NodeId,
    parent: Arc<dyn StaticTypedScope>,
    owner_this: TypeId,
}
impl AbstractStaticScope for TypeOfThisScope {
    // port: JSTypeRegistryTest#getRootNode
    fn get_root_node(&self) -> Option<NodeId> {
        Some(self.root)
    }
    // port: JSTypeRegistryTest#getParentScope
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope> {
        Some(self.parent.as_ref())
    }
    // port: JSTypeRegistryTest#getSlot
    fn get_slot(&self, _name: &JsString) -> Option<&dyn StaticTypedSlot> {
        None
    }
    // port: JSTypeRegistryTest#getTypeOfThis
    fn get_type_of_this(&self, _reg: &JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
        Some(self.owner_this)
    }
}

// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_templateScopeShadowing_doesNotLeakCachedBuiltins
#[test]
fn test_create_type_from_comment_node_template_scope_shadowing_does_not_leak_cached_builtins() {
    let mut ast = Ast::new();
    let (mut closed_registry, reporter) = closed_registry_with_reporter(&mut ast);
    let global_scope: Arc<dyn StaticTypedScope> = create_static_typed_scope(
        ast.new_node(Token::ROOT),
        None,
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );

    // Warm builtin Array evaluation first.
    let arg = ast.new_string("string");
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let n = bang_node(&mut ast, array);
    let builtin_array_str = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    assert!(builtin_array_str.is_templatized_type(&closed_registry));

    // Shadow "Array" via @template in SyntheticTemplateScope.
    let template_array = closed_registry.create_template_type(&ast, "Array");
    let template_scope =
        closed_registry.create_scope_with_templates(global_scope.clone(), vec![template_array]);

    let n = ast.new_string("Array");
    let shadowed_bare = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(template_scope.clone()),
    );
    let arg = ast.new_string("string");
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let n = bang_node(&mut ast, array);
    let shadowed_with_args = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(template_scope),
    );
    TypeSubject::assert_type(shadowed_bare).is_equal_to(&mut closed_registry, &ast, template_array);
    TypeSubject::assert_type(shadowed_with_args).is_equal_to(
        &mut closed_registry,
        &ast,
        template_array,
    );

    // Shadow "Promise" via typeOfThis template key map.
    let template_promise = closed_registry.create_template_type(&ast, "Promise");
    let owner_this;
    {
        let mut closer = closed_registry.get_resolver().open_for_definition();
        owner_this = FunctionTypeBuilder::new()
            .for_constructor()
            .with_name("PromiseOwner")
            .with_template_keys(&mut closed_registry, &ast, vec![template_promise])
            .build(&mut closed_registry, &ast)
            .get_instance_type(&closed_registry)
            .unwrap();
        closer.close(&mut closed_registry, &ast);
    }
    let this_scope: Arc<dyn StaticTypedScope> = Arc::new(TypeOfThisScope {
        root: ast.new_node(Token::BLOCK),
        parent: global_scope.clone(),
        owner_this,
    });
    let arg = ast.new_string("string");
    let promise = templatized_comment_node(&mut ast, "Promise", &[arg]);
    let n = bang_node(&mut ast, promise);
    let shadowed_this_promise = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(this_scope),
    );
    TypeSubject::assert_type(shadowed_this_promise).is_equal_to(
        &mut closed_registry,
        &ast,
        template_promise,
    );

    // Shadowing must not leak back to clean global scope.
    let arg = ast.new_string("string");
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let n = bang_node(&mut ast, array);
    let clean_again = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope),
    );
    TypeSubject::assert_type(clean_again).is_equal_to(
        &mut closed_registry,
        &ast,
        builtin_array_str,
    );

    reporter
        .lock()
        .unwrap()
        .verify_has_encountered_all_warnings_and_errors();
}

// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_localScopeAndNonNullableShadowing
#[test]
fn test_create_type_from_comment_node_local_scope_and_non_nullable_shadowing() {
    let mut ast = Ast::new();
    let (mut closed_registry, reporter) = closed_registry_with_reporter(&mut ast);
    let global_scope = create_static_typed_scope(
        ast.new_node(Token::ROOT),
        None,
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );

    let arg = ast.new_string("string");
    let n = templatized_comment_node(&mut ast, "Promise", &[arg]);
    let global_promise = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "a.js",
        Some(global_scope.clone()),
    );
    assert_eq!(
        global_promise.to_string(&mut closed_registry, &ast),
        "(Promise<string>|null)"
    );

    // Local slot shadowing built-in Array must produce NamedType, not builtin Array.
    let local_scope = create_static_typed_scope(
        ast.new_node(Token::BLOCK),
        Some(global_scope.clone()),
        IndexMap::<_, _>::default(),
        IndexSet::<_>::default(),
    );
    let unknown = closed_registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
    let slots = IndexMap::<_, _>::from_iter([(
        "Array".into(),
        ScopedSlot {
            slot: SimpleSlot::new("Array", unknown, true),
            scope: local_scope.clone(),
        },
    )]);
    local_scope
        .slots
        .set(slots)
        .unwrap_or_else(|_| panic!("slot map already initialized"));
    reporter.lock().unwrap().expect_all_warnings(&[concat!(
        "Bad type annotation. Unknown type Array\n",
        "It's possible that 'Array' refers to a value, not a type."
    )]);
    let arg = ast.new_string("string");
    let array = templatized_comment_node(&mut ast, "Array", &[arg]);
    let n = bang_node(&mut ast, array);
    let shadowed_local = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "local.js",
        Some(local_scope),
    );
    assert!(shadowed_local.is_named_type(&closed_registry));

    // Registering a non-nullable name must change subsequent un-banged evaluations.
    closed_registry.identify_non_nullable_name(&ast, None, "Promise");
    let arg = ast.new_string("string");
    let n = templatized_comment_node(&mut ast, "Promise", &[arg]);
    let non_nullable_promise = closed_registry.create_type_from_comment_node_with_location(
        &ast,
        n,
        "b.js",
        Some(global_scope),
    );
    assert_eq!(
        non_nullable_promise.to_string(&mut closed_registry, &ast),
        "Promise<string>"
    );

    reporter
        .lock()
        .unwrap()
        .verify_has_encountered_all_warnings_and_errors();
}

// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_repeatedTooManyTemplateParametersEmitsAllWarnings
#[test]
fn test_create_type_from_comment_node_repeated_too_many_template_parameters_emits_all_warnings() {
    let mut ast = Ast::new();
    let warning_reporter = Arc::new(Mutex::new(TestErrorReporter::new()));
    let expected = "Too many template parameters\nFound 2, required at most 1";
    warning_reporter
        .lock()
        .unwrap()
        .expect_all_warnings(&[expected, expected]);
    let mut warn_registry = JSTypeRegistry::new(
        &mut ast,
        Box::new(SharedTestErrorReporter(warning_reporter.clone())),
        Vec::new(),
    );

    let first_extra = ast.new_string("number");
    first_extra.set_lineno_charno(&mut ast, 10, 20);
    let first_array = ast.new_string("Array");
    let string = ast.new_string("string");
    let block = ast.new_node_with_children2(Token::BLOCK, string, first_extra);
    first_array.add_child_to_front(&mut ast, block);

    let second_extra = ast.new_string("number");
    second_extra.set_lineno_charno(&mut ast, 25, 4);
    let second_array = ast.new_string("Array");
    let string = ast.new_string("string");
    let block = ast.new_node_with_children2(Token::BLOCK, string, second_extra);
    second_array.add_child_to_front(&mut ast, block);

    let n = bang_node(&mut ast, first_array);
    let t1 = warn_registry.create_type_from_comment_node_with_location(&ast, n, "warn.js", None);
    let n = bang_node(&mut ast, second_array);
    let t2 = warn_registry.create_type_from_comment_node_with_location(&ast, n, "warn.js", None);

    TypeSubject::assert_type(t1).is_equal_to(&mut warn_registry, &ast, t2);
    assert_eq!(t1.to_string(&mut warn_registry, &ast), "Array<string>");
    warning_reporter
        .lock()
        .unwrap()
        .verify_has_encountered_all_warnings_and_errors();
}

// port: JSTypeRegistryTest#testCreateTypeFromCommentNode_recordTypePreservesDistinctPropertyNodes
#[test]
fn test_create_type_from_comment_node_record_type_preserves_distinct_property_nodes() {
    let mut ast = Ast::new();
    let (mut closed_registry, reporter) = closed_registry_with_reporter(&mut ast);

    let prop_node1 = ast.new_string("a");
    prop_node1.set_lineno_charno(&mut ast, 10, 3);
    let string = ast.new_string("string");
    let colon = ast.new_node_with_children2(Token::COLON, prop_node1, string);
    let lb = ast.new_node_with_child(Token::LB, colon);
    let rec_node1 = ast.new_node_with_child(Token::LC, lb);

    let prop_node2 = ast.new_string("a");
    prop_node2.set_lineno_charno(&mut ast, 20, 3);
    let string = ast.new_string("string");
    let colon = ast.new_node_with_children2(Token::COLON, prop_node2, string);
    let lb = ast.new_node_with_child(Token::LB, colon);
    let rec_node2 = ast.new_node_with_child(Token::LC, lb);

    let r1 = closed_registry
        .create_type_from_comment_node_with_location(&ast, rec_node1, "rec.js", None);
    let r2 = closed_registry
        .create_type_from_comment_node_with_location(&ast, rec_node2, "rec.js", None);

    TypeSubject::assert_type(r1).is_equal_to(&mut closed_registry, &ast, r2);
    // isSameInstanceAs: Node identity is NodeId equality.
    assert_eq!(
        r1.to_maybe_record_type(&closed_registry)
            .unwrap()
            .get_property_node(&mut closed_registry, &ast, "a"),
        Some(prop_node1)
    );
    assert_eq!(
        r2.to_maybe_record_type(&closed_registry)
            .unwrap()
            .get_property_node(&mut closed_registry, &ast, "a"),
        Some(prop_node2)
    );
    assert_ne!(
        r2.to_maybe_record_type(&closed_registry)
            .unwrap()
            .get_property_node(&mut closed_registry, &ast, "a"),
        Some(prop_node1)
    );
    reporter
        .lock()
        .unwrap()
        .verify_has_encountered_all_warnings_and_errors();
}

// port: JSTypeRegistryTest#nullableArrayStrNode
fn nullable_array_str_node(ast: &mut Ast) -> NodeId {
    let arg = ast.new_string("string");
    templatized_comment_node(ast, "Array", &[arg])
}

// port: JSTypeRegistryTest#bangNode
fn bang_node(ast: &mut Ast, child: NodeId) -> NodeId {
    ast.new_node_with_child(Token::BANG, child)
}

// port: JSTypeRegistryTest#templatizedCommentNode
fn templatized_comment_node(ast: &mut Ast, name: &str, template_args: &[NodeId]) -> NodeId {
    let node = ast.new_string(name);
    let block = ast.new_node(Token::BLOCK);
    for &arg in template_args {
        block.add_child_to_back(ast, arg);
    }
    node.add_child_to_front(ast, block);
    node
}
