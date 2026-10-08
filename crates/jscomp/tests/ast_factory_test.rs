/*
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/AstFactoryTest.java.

//! Port of `AstFactoryTest.java`.
//!
//! The cases that read `Compiler#getTranspilationNamespace` (GlobalNamespace) pass it as the
//! Rust-only `TranspilationNamespace` handle (crates/jscomp/src/transpilation_namespace.rs).

use closure_jscomp::{
    Compiler,
    ast_factory::AstFactory,
    compiler_input::CompilerInput,
    js::runtime_js_lib_manager::{RuntimeJsLibManager, RuntimeLibraryMode},
    scope::ScopeId,
    syntactic_scope_creator::{RedeclarationHandler, SyntacticScopeCreator},
    transpilation_namespace::TranspilationNamespace,
    type_check::TypeCheck,
    typed_scope::TypedScope,
};
use closure_jstype::{JSTypeNative, object_type::ObjectType, testing::type_subject::TypeSubject};
use closure_jstype::{
    static_typed_scope::StaticTypedScope, testing::map_based_scope::MapBasedScope,
};
use closure_rhino::{
    ir::IR,
    jscomp_colors::{Color, ColorId, color_registry::ColorRegistry, standard_colors},
    node::{NodeId, Prop},
    token::Token,
};
use closure_rhino::{js_string::JsString, static_scope::StaticScope, static_slot::StaticSlot};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

struct AstFactoryTest {
    compiler: Compiler,
    runtime_js_lib_manager: Arc<Mutex<RuntimeJsLibManager>>,
}

impl AstFactoryTest {
    // port: AstFactoryTest#setUp
    fn set_up() -> Self {
        let compiler = Compiler::new();
        let runtime_js_lib_manager = RuntimeJsLibManager::create(
            RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS,
            // TestResourceProvider
            Box::new(|_: &mut Compiler, _unused1: &str, _unused2: &str| {
                panic!("UnsupportedOperationException")
            }),
            Compiler::get_change_tracker_and_ast,
            // AstFactoryTest::alwaysThrowNodeSupplier
            Box::new(|_: &mut Compiler| panic!("UnsupportedOperationException")),
        );
        Self {
            compiler,
            runtime_js_lib_manager: Arc::new(Mutex::new(runtime_js_lib_manager)),
        }
    }

    // port: AstFactoryTest#getNativeType
    fn get_native_type(&mut self, native_type: JSTypeNative) -> closure_jstype::TypeId {
        self.compiler
            .get_type_registry()
            .get_native_type(native_type)
    }

    // port: AstFactoryTest#parseWithoutTypes(String)
    fn parse_without_types(&mut self, source: &str) -> NodeId {
        self.parse_without_types_with_externs("", source)
    }

    // port: AstFactoryTest#parseWithoutTypes(String, String)
    fn parse_without_types_with_externs(&mut self, externs: &str, source: &str) -> NodeId {
        use closure_jscomp::{compiler_options::CompilerOptions, source_file::SourceFile};
        // parse the test code
        let options = CompilerOptions::new();
        self.compiler.init(
            &[Arc::new(SourceFile::from_code("externs", externs))],
            &[Arc::new(SourceFile::from_code("source", source))],
            options,
        );
        self.compiler.parse_inputs();
        assert!(self.compiler.get_errors().is_empty(), "parse error");
        self.compiler.get_js_root().unwrap()
    }

    // port: AstFactoryTest#parseAndAddTypes(String)
    fn parse_and_add_types(&mut self, source: &str) -> NodeId {
        self.parse_and_add_types_with_externs("", source)
    }

    // port: AstFactoryTest#parseAndAddTypes(String, String)
    fn parse_and_add_types_with_externs(&mut self, externs: &str, source: &str) -> NodeId {
        self.parse_without_types_with_externs(externs, source);

        // then add types to the AST & do type checks
        // TODO(bradfordcsmith): Fail if there are type checking errors.
        let interpreter = self.compiler.get_reverse_abstract_interpreter();
        let mut type_check = TypeCheck::new(&mut self.compiler, interpreter);
        let externs_root = self.compiler.get_externs_root();
        let js_root = self.compiler.get_js_root().unwrap();
        type_check.process_for_testing(&mut self.compiler, externs_root, js_root);
        self.compiler.set_type_checking_has_run(true);
        self.compiler.get_js_root().unwrap()
    }

    // port: AstFactoryTest#parseAndAddColors(String)
    fn parse_and_add_colors(&mut self, source: &str) -> NodeId {
        self.parse_and_add_colors_with_externs("", source)
    }

    // port: AstFactoryTest#parseAndAddColors(String, String)
    fn parse_and_add_colors_with_externs(&mut self, externs: &str, source: &str) -> NodeId {
        use closure_jscomp::compiler_pass::CompilerPass;
        use closure_jscomp::serialization::{
            convert_types_to_colors::ConvertTypesToColors,
            serialization_options::SerializationOptions,
        };
        self.parse_and_add_types_with_externs(externs, source);
        let externs_root = self.compiler.get_externs_root().unwrap();
        let js_root = self.compiler.get_js_root().unwrap();
        ConvertTypesToColors::new(
            SerializationOptions::builder()
                .set_include_debug_info(true)
                .build(),
        )
        .process(&mut self.compiler, externs_root, js_root);
        self.compiler.get_js_root().unwrap()
    }

    // port: AstFactoryTest#getScope
    fn get_scope(&mut self, root: NodeId) -> ScopeId {
        // Normal passes use SyntacticScopeCreator, so that's what we use here.
        let redeclaration_handler = NoopRedeclarationHandler;
        let mut scope_creator =
            SyntacticScopeCreator::new_with_redeclaration_handler(Box::new(redeclaration_handler));
        scope_creator.create_scope(&mut self.compiler, root, None)
    }

    // port: AstFactoryTest#createTestAstFactory
    fn create_test_ast_factory(&mut self) -> AstFactory {
        let stage = self.compiler.get_life_cycle_stage();
        AstFactory::create_factory_with_types(
            stage,
            self.compiler.get_type_registry(),
            Some(self.runtime_js_lib_manager.clone()),
        )
    }

    // port: AstFactoryTest#createTestAstFactoryWithColors
    fn create_test_ast_factory_with_colors(&mut self) -> AstFactory {
        AstFactory::create_factory_with_colors(
            self.compiler.get_life_cycle_stage(),
            // the built-in color registry is available only if we've run parseAndAddColors()
            if self.compiler.has_optimization_colors() {
                Arc::clone(self.compiler.get_color_registry())
            } else {
                Arc::new(
                    ColorRegistry::builder()
                        .set_default_native_colors_for_testing()
                        .build(),
                )
            },
            Some(self.runtime_js_lib_manager.clone()),
        )
    }

    // port: AstFactoryTest#createTestAstFactoryWithoutTypes
    fn create_test_ast_factory_without_types(&mut self) -> AstFactory {
        AstFactory::create_factory_without_types(
            self.compiler.get_life_cycle_stage(),
            Some(self.runtime_js_lib_manager.clone()),
        )
    }

    /// `assertType(type).isString()`
    fn assert_is_string(&mut self, type_: Option<closure_jstype::TypeId>) {
        TypeSubject::assert_type(type_.unwrap()).is_string(self.compiler.get_type_registry());
    }

    /// `assertType(type).isNumber()`
    fn assert_is_number(&mut self, type_: Option<closure_jstype::TypeId>) {
        TypeSubject::assert_type(type_.unwrap()).is_number(self.compiler.get_type_registry());
    }

    /// `assertType(type).isBoolean()`
    fn assert_is_boolean(&mut self, type_: Option<closure_jstype::TypeId>) {
        TypeSubject::assert_type(type_.unwrap()).is_boolean(self.compiler.get_type_registry());
    }

    /// `assertType(type).isVoid()`
    fn assert_is_void(&mut self, type_: Option<closure_jstype::TypeId>) {
        TypeSubject::assert_type(type_.unwrap()).is_void(self.compiler.get_type_registry());
    }

    /// `assertType(type).toStringIsEqualTo(typeString)`
    fn assert_to_string_is_equal_to(
        &mut self,
        type_: Option<closure_jstype::TypeId>,
        type_string: &str,
    ) {
        let (registry, ast) = self.compiler.get_type_registry_and_ast();
        TypeSubject::assert_type(type_.unwrap()).to_string_is_equal_to(registry, ast, type_string);
    }

    /// `assertType(actual).isEqualTo(expected)` (JSType#equals)
    fn assert_type_is_equal_to(
        &mut self,
        actual: Option<closure_jstype::TypeId>,
        expected: closure_jstype::TypeId,
    ) {
        let (registry, ast) = self.compiler.get_type_registry_and_ast();
        TypeSubject::assert_type(actual.unwrap()).is_equal_to(registry, ast, expected);
    }

    /// `assertType(type).isUnknown()`
    fn assert_is_unknown(&mut self, type_: Option<closure_jstype::TypeId>) {
        let (registry, ast) = self.compiler.get_type_registry_and_ast();
        TypeSubject::assert_type(type_.unwrap()).is_unknown(registry, ast);
    }
}

/// `(Scope s, String name, Node n, CompilerInput input) -> {}`
struct NoopRedeclarationHandler;

impl RedeclarationHandler for NoopRedeclarationHandler {
    // port: AstFactoryTest#getScope (lambda)
    fn on_redeclaration(
        &mut self,
        _compiler: &mut Compiler,
        _s: ScopeId,
        _name: &JsString,
        _n: NodeId,
        _input: Option<CompilerInput>,
    ) {
    }
}

fn child_list(compiler: &Compiler, n: NodeId) -> Vec<NodeId> {
    n.children(compiler).collect()
}

/// `assertNode(n).hasColorThat().hasAlternates(alternates)`
fn assert_has_alternates(color: &Color, alternates: &[Color]) {
    assert!(color.is_union());
    let elements = color.get_union_elements();
    assert_eq!(elements.len(), alternates.len());
    for alternate in alternates {
        assert!(elements.contains(alternate));
    }
}

// port: AstFactoryTest#testStringLiteral_jstypes
#[test]
fn test_string_literal_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    assert_eq!(string_literal.get_token(&t.compiler), Token::STRINGLIT);
    assert_eq!(string_literal.get_string(&t.compiler), "hello");
    let type_ = string_literal.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testStringLiteral_colors
#[test]
fn test_string_literal_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    assert_eq!(string_literal.get_token(&t.compiler), Token::STRINGLIT);
    assert_eq!(string_literal.get_string(&t.compiler), "hello");
    assert_eq!(
        string_literal.get_color(&t.compiler),
        Some(standard_colors::STRING.clone())
    );
}

// port: AstFactoryTest#testCreateSingleConstNameDeclaration_jstypes
#[test]
fn test_create_single_const_name_declaration_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let value_node = ast_factory.create_boolean(&mut t.compiler, true);
    let const_node =
        ast_factory.create_single_const_name_declaration(&mut t.compiler, "myTrue", value_node);
    assert!(const_node.is_const(&t.compiler));
    let name_node = const_node.get_only_child(&t.compiler);
    assert!(name_node.is_name(&t.compiler));
    assert_eq!(name_node.get_string(&t.compiler), "myTrue");
    assert_eq!(name_node.get_only_child(&t.compiler), value_node);
    assert_eq!(
        name_node.get_jstype(&t.compiler),
        value_node.get_jstype(&t.compiler)
    );
}

// port: AstFactoryTest#testCreateSingleConstNameDeclaration_colors
#[test]
fn test_create_single_const_name_declaration_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let value_node = ast_factory.create_boolean(&mut t.compiler, true);
    let const_node =
        ast_factory.create_single_const_name_declaration(&mut t.compiler, "myTrue", value_node);
    assert!(const_node.is_const(&t.compiler));
    let name_node = const_node.get_only_child(&t.compiler);
    assert!(name_node.is_name(&t.compiler));
    assert_eq!(name_node.get_string(&t.compiler), "myTrue");
    assert_eq!(name_node.get_only_child(&t.compiler), value_node);
    assert_eq!(
        name_node.get_color(&t.compiler),
        value_node.get_color(&t.compiler)
    );
}

// port: AstFactoryTest#testCreateNameWithJSType
#[test]
fn test_create_name_with_js_type() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let x = ast_factory.create_name(
        &mut t.compiler,
        "x",
        AstFactory::type_native(JSTypeNative::STRING_TYPE),
    );
    assert_eq!(x.get_token(&t.compiler), Token::NAME);
    assert_eq!(x.get_string(&t.compiler), "x");
    let type_ = x.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateNameWithNativeType
#[test]
fn test_create_name_with_native_type() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let x = ast_factory.create_name(
        &mut t.compiler,
        "x",
        AstFactory::type_native(JSTypeNative::STRING_TYPE),
    );
    assert_eq!(x.get_token(&t.compiler), Token::NAME);
    assert_eq!(x.get_string(&t.compiler), "x");
    let type_ = x.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateNameWithColor
#[test]
fn test_create_name_with_color() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let x = ast_factory.create_name(
        &mut t.compiler,
        "x",
        AstFactory::type_native_and_color(
            JSTypeNative::STRING_TYPE,
            standard_colors::STRING.clone(),
        ),
    );
    assert_eq!(x.get_token(&t.compiler), Token::NAME);
    assert_eq!(x.get_string(&t.compiler), "x");
    assert_eq!(
        x.get_color(&t.compiler),
        Some(standard_colors::STRING.clone())
    );
}

// port: AstFactoryTest#testCreateGetPropFromScope_defaultsToUnknownJSTypeWhenNull
#[test]
fn test_create_get_prop_from_scope_defaults_to_unknown_js_type_when_null() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "JQ");
    let type_template = IR::name(&mut t.compiler, "JQ");

    assert!(
        type_template.get_jstype(&t.compiler).is_none(),
        "getJSType does not return null "
    );

    let x = ast_factory.create_get_prop(
        &mut t.compiler,
        receiver,
        "$",
        AstFactory::type_node(type_template),
    );

    assert_eq!(x.get_token(&t.compiler), Token::GETPROP);
    assert!(x.matches_qualified_name(&t.compiler, "JQ.$"));
    let type_ = x.get_jstype(&t.compiler);
    t.assert_is_unknown(type_);
}

// port: AstFactoryTest#testCreateGetPropFromScope_defaultsToUnknownColorWhenNull
#[test]
fn test_create_get_prop_from_scope_defaults_to_unknown_color_when_null() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "JQ");
    let type_template = IR::name(&mut t.compiler, "JQ");

    assert!(
        type_template.get_color(&t.compiler).is_none(),
        "getColor does not return null "
    );

    let x = ast_factory.create_get_prop(
        &mut t.compiler,
        receiver,
        "$",
        AstFactory::type_node(type_template),
    );

    assert_eq!(x.get_token(&t.compiler), Token::GETPROP);
    assert!(x.matches_qualified_name(&t.compiler, "JQ.$"));
    assert_eq!(
        x.get_color(&t.compiler),
        Some(standard_colors::UNKNOWN.clone())
    );
}

// port: AstFactoryTest#testCreateThisReference
#[test]
fn test_create_this_reference() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let string_type = t.get_native_type(JSTypeNative::STRING_TYPE);
    let x = ast_factory.create_this(&mut t.compiler, AstFactory::type_jstype(Some(string_type)));
    assert_eq!(x.get_token(&t.compiler), Token::THIS);
    let type_ = x.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#createThisAliasReferenceForFunctionWithoutTypes
#[test]
fn create_this_alias_reference_for_function_without_types() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_without_types();

    let root = t.parse_without_types(
        r"class C {
  method() {}
}
",
    );

    let class_node = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // class
        .unwrap();

    let this_alias = ast_factory.create_this_alias_reference_for_es6_class(
        &mut t.compiler,
        "thisAlias",
        class_node,
    );
    assert_eq!(this_alias.get_token(&t.compiler), Token::NAME);
    assert_eq!(this_alias.get_string(&t.compiler), "thisAlias");
    assert!(this_alias.get_jstype(&t.compiler).is_none());
}

// port: AstFactoryTest#testCreateGetpropWithColorFromNode
#[test]
fn test_create_getprop_with_color_from_node() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let type_template = ast_factory.create_number(&mut t.compiler, 0.0);

    let get_prop = ast_factory.create_get_prop(
        &mut t.compiler,
        receiver,
        "y",
        AstFactory::type_node(type_template),
    );

    assert_eq!(get_prop.get_token(&t.compiler), Token::GETPROP);
    assert_eq!(get_prop.get_string(&t.compiler), "y");
    assert_eq!(get_prop.get_first_child(&t.compiler), Some(receiver));
    assert_eq!(
        get_prop.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateStartOptChainCall
#[test]
fn test_create_start_opt_chain_call() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let type_template = ast_factory.create_number(&mut t.compiler, 0.0);
    let arg1 = ast_factory.create_number(&mut t.compiler, 1.0);
    let arg2 = ast_factory.create_number(&mut t.compiler, 2.0);

    let call = ast_factory.create_start_opt_chain_call(
        &mut t.compiler,
        receiver,
        AstFactory::type_node(type_template),
        &[arg1, arg2],
    );

    assert_eq!(call.get_token(&t.compiler), Token::OPTCHAIN_CALL);
    assert_eq!(call.get_first_child(&t.compiler), Some(receiver));
    assert_eq!(call.get_second_child(&t.compiler), Some(arg1));
    assert_eq!(call.get_last_child(&t.compiler), Some(arg2));
    assert_eq!(
        call.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
    assert!(call.is_optional_chain_start(&t.compiler));
}

// port: AstFactoryTest#testCreateStringKey_colors
#[test]
fn test_create_string_key_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let number_node = ast_factory.create_number(&mut t.compiler, 2112.0);
    let string_key_node = ast_factory.create_string_key(&mut t.compiler, "key", number_node);

    assert_eq!(string_key_node.get_token(&t.compiler), Token::STRING_KEY);
    assert_eq!(string_key_node.get_string(&t.compiler), "key");
    assert_eq!(child_list(&t.compiler, string_key_node), vec![number_node]);
    assert_eq!(
        string_key_node.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateGetElem_jstypes
#[test]
fn test_create_get_elem_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let object_name = ast_factory.create_name(
        &mut t.compiler,
        "obj",
        AstFactory::type_native(JSTypeNative::OBJECT_TYPE),
    );
    let string_literal = ast_factory.create_string(&mut t.compiler, "string literal key");
    let get_elem_node = ast_factory.create_get_elem(&mut t.compiler, object_name, string_literal);

    assert_eq!(get_elem_node.get_token(&t.compiler), Token::GETELEM);
    assert_eq!(
        child_list(&t.compiler, get_elem_node),
        vec![object_name, string_literal]
    );
    // TODO(bradfordcsmith): When receiver is an Array<T> or an Object<K, V>, use the template type
    // here.
    let type_ = get_elem_node.get_jstype(&t.compiler);
    t.assert_is_unknown(type_);
}

// port: AstFactoryTest#testCreateMethodCall_throws
#[test]
fn test_create_method_call_throws() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    t.parse_and_add_types(
        "class Foo {\n  /**\n   * @param {string} arg1\n   * @param {number} arg2\n   * @return {string}\n   */\n  method(arg1, arg2) { return arg1; }\n}\nconst foo = new Foo();\n",
    );
    let scope = TranspilationNamespace::get(&mut t.compiler);

    // createQName only accepts globally qualified qnames. foo.method is a prototype method access.
    // foo.method("hi", 2112)
    // assertThrows(Exception.class, ...)
    let result = catch_unwind(AssertUnwindSafe(|| {
        ast_factory.create_qname(&mut t.compiler, &scope, "foo.method")
    }));
    assert!(result.is_err());
}

// port: AstFactoryTest#testCreateStaticMethodCallDotCallThrows
#[test]
fn test_create_static_method_call_dot_call_throws() {
    // NOTE: This method is testing both createCall() and createQName()
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    t.parse_and_add_types(
        "class Foo {\n  /**\n   * @param {string} arg1\n   * @param {number} arg2\n   * @return {string}\n   */\n  static method(arg1, arg2) { return arg1; }\n}\n",
    );
    let scope = TranspilationNamespace::get(&mut t.compiler);

    // createQName only accepts globally qualified qnames. While Foo.method is a global qualified
    // name, its '.call' property is not.
    // Foo.method.call(null, "hi", 2112)
    // assertThrows(Exception.class, ...)
    let result = catch_unwind(AssertUnwindSafe(|| {
        ast_factory.create_qname(&mut t.compiler, &scope, "Foo.method.call")
    }));
    assert!(result.is_err());
}

// port: AstFactoryTest#testCreateQNameWithUnknownTypeFromString_jstype
#[test]
fn test_create_qname_with_unknown_type_from_string_jstype() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let obj_dot_inner_dot_str =
        ast_factory.create_qname_with_unknown_type(&mut t.compiler, "obj.inner.str");

    assert!(obj_dot_inner_dot_str.matches_qualified_name(&t.compiler, "obj.inner.str"));
    let obj_dot_inner = obj_dot_inner_dot_str.get_first_child(&t.compiler).unwrap();
    let obj = obj_dot_inner.get_first_child(&t.compiler).unwrap();

    for n in [obj, obj_dot_inner, obj_dot_inner_dot_str] {
        let type_ = n.get_jstype(&t.compiler);
        t.assert_is_unknown(type_);
    }
}

// port: AstFactoryTest#testCreateQNameWithUnknownTypeFromString_colors
#[test]
fn test_create_qname_with_unknown_type_from_string_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let obj_dot_inner_dot_str =
        ast_factory.create_qname_with_unknown_type(&mut t.compiler, "obj.inner.str");

    assert!(obj_dot_inner_dot_str.matches_qualified_name(&t.compiler, "obj.inner.str"));
    let obj_dot_inner = obj_dot_inner_dot_str.get_first_child(&t.compiler).unwrap();
    let obj = obj_dot_inner.get_first_child(&t.compiler).unwrap();

    for n in [obj, obj_dot_inner, obj_dot_inner_dot_str] {
        assert_eq!(
            n.get_color(&t.compiler),
            Some(standard_colors::UNKNOWN.clone())
        );
    }
}

// port: AstFactoryTest#testCreateQNameFromString
#[test]
fn test_create_qname_from_string() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    t.parse_and_add_types("const obj = {\n  inner: {\n    str: 'hi',\n  }\n};\n");
    let scope = TranspilationNamespace::get(&mut t.compiler);

    let obj_dot_inner_dot_str = ast_factory.create_qname(&mut t.compiler, &scope, "obj.inner.str");

    assert!(obj_dot_inner_dot_str.matches_qualified_name(&t.compiler, "obj.inner.str"));
    let obj_dot_inner = obj_dot_inner_dot_str.get_first_child(&t.compiler).unwrap();
    let obj = obj_dot_inner.get_first_child(&t.compiler).unwrap();

    t.assert_to_string_is_equal_to(obj.get_jstype(&t.compiler), "{inner: {str: string}}");
    t.assert_to_string_is_equal_to(obj_dot_inner.get_jstype(&t.compiler), "{str: string}");
    let type_ = obj_dot_inner_dot_str.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateQNameFromBaseNamePlusStringIterable
#[test]
fn test_create_qname_from_base_name_plus_string_iterable() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    t.parse_and_add_types("const obj = {\n  inner: {\n    str: 'hi',\n  }\n};\n");
    let scope = TranspilationNamespace::get(&mut t.compiler);

    let obj_dot_inner_dot_str =
        ast_factory.create_qname_with_base_name(&mut t.compiler, &scope, "obj", &["inner", "str"]);

    assert!(obj_dot_inner_dot_str.matches_qualified_name(&t.compiler, "obj.inner.str"));
    let obj_dot_inner = obj_dot_inner_dot_str.get_first_child(&t.compiler).unwrap();
    let obj = obj_dot_inner.get_first_child(&t.compiler).unwrap();

    t.assert_to_string_is_equal_to(obj.get_jstype(&t.compiler), "{inner: {str: string}}");
    t.assert_to_string_is_equal_to(obj_dot_inner.get_jstype(&t.compiler), "{str: string}");
    let type_ = obj_dot_inner_dot_str.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateNameMaintainsNormalization
#[test]
fn test_create_name_maintains_normalization() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types("const obj = {}");

    // Simulate normalization adding the IS_CONSTANT_NAME property ot `obj` NAME node
    let obj_name = root
        .get_first_child(&t.compiler) // SCRIPT
        .unwrap()
        .get_first_child(&t.compiler) // CONST
        .unwrap()
        .get_first_child(&t.compiler) // obj NAME
        .unwrap();
    assert_eq!(obj_name.get_token(&t.compiler), Token::NAME);
    assert_eq!(obj_name.get_string(&t.compiler), "obj");
    obj_name.put_boolean_prop(&mut t.compiler, Prop::IS_CONSTANT_NAME, true);

    let scope = TranspilationNamespace::get(&mut t.compiler);
    let new_obj_name = ast_factory.create_name_in_scope(&mut t.compiler, Some(&scope), "obj");

    // Assert that the newly created NAME node gets the IS_CONSTANT_NAME property it should in
    // order to be consistent with normalization.
    assert!(new_obj_name.get_boolean_prop(&t.compiler, Prop::IS_CONSTANT_NAME));
}

// port: AstFactoryTest#testCreateQNameFromStringVarArgs
#[test]
fn test_create_qname_from_string_var_args() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    t.parse_and_add_types("const obj = {\n  inner: {\n    str: 'hi',\n  }\n};\n");

    let scope = TranspilationNamespace::get(&mut t.compiler);
    let obj_dot_inner_dot_str =
        ast_factory.create_qname_with_base_name(&mut t.compiler, &scope, "obj", &["inner", "str"]);

    assert!(obj_dot_inner_dot_str.matches_qualified_name(&t.compiler, "obj.inner.str"));
    let obj_dot_inner = obj_dot_inner_dot_str.get_first_child(&t.compiler).unwrap();
    let obj = obj_dot_inner.get_first_child(&t.compiler).unwrap();

    t.assert_to_string_is_equal_to(obj.get_jstype(&t.compiler), "{inner: {str: string}}");
    t.assert_to_string_is_equal_to(obj_dot_inner.get_jstype(&t.compiler), "{str: string}");
    let type_ = obj_dot_inner_dot_str.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testNumberLiteral_jstypes
#[test]
fn test_number_literal_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let number_literal = ast_factory.create_number(&mut t.compiler, 2112.0);
    assert_eq!(number_literal.get_token(&t.compiler), Token::NUMBER);
    assert_eq!(number_literal.get_double(&t.compiler), 2112.0);
    let type_ = number_literal.get_jstype(&t.compiler);
    t.assert_is_number(type_);
}

// port: AstFactoryTest#testNumberLiteral_colors
#[test]
fn test_number_literal_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let number_literal = ast_factory.create_number(&mut t.compiler, 2112.0);
    assert_eq!(number_literal.get_token(&t.compiler), Token::NUMBER);
    assert_eq!(number_literal.get_double(&t.compiler), 2112.0);
    assert_eq!(
        number_literal.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testBooleanLiteral_jstypes
#[test]
fn test_boolean_literal_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let true_node = ast_factory.create_boolean(&mut t.compiler, true);
    assert_eq!(true_node.get_token(&t.compiler), Token::TRUE);
    let type_ = true_node.get_jstype(&t.compiler);
    t.assert_is_boolean(type_);
    let false_node = ast_factory.create_boolean(&mut t.compiler, false);
    assert_eq!(false_node.get_token(&t.compiler), Token::FALSE);
    let type_ = false_node.get_jstype(&t.compiler);
    t.assert_is_boolean(type_);
}

// port: AstFactoryTest#testBooleanLiteral_colors
#[test]
fn test_boolean_literal_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let true_node = ast_factory.create_boolean(&mut t.compiler, true);
    assert_eq!(true_node.get_token(&t.compiler), Token::TRUE);
    assert_eq!(
        true_node.get_color(&t.compiler),
        Some(standard_colors::BOOLEAN.clone())
    );
}

// port: AstFactoryTest#testVoidExpression_jstypes
#[test]
fn test_void_expression_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let void_node = ast_factory.create_void(&mut t.compiler, zero);
    assert_eq!(void_node.get_token(&t.compiler), Token::VOID);
    let type_ = void_node.get_jstype(&t.compiler);
    t.assert_is_void(type_);
}

// port: AstFactoryTest#testVoidExpression_colors
#[test]
fn test_void_expression_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let void_node = ast_factory.create_void(&mut t.compiler, zero);
    assert_eq!(void_node.get_token(&t.compiler), Token::VOID);
    assert_eq!(
        void_node.get_color(&t.compiler),
        Some(standard_colors::NULL_OR_VOID.clone())
    );
}

// port: AstFactoryTest#testNotExpression_jstypes
#[test]
fn test_not_expression_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let not_node = ast_factory.create_not(&mut t.compiler, zero);
    assert_eq!(not_node.get_token(&t.compiler), Token::NOT);
    let type_ = not_node.get_jstype(&t.compiler);
    t.assert_is_boolean(type_);
}

// port: AstFactoryTest#testNotExpression_colors
#[test]
fn test_not_expression_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let not_node = ast_factory.create_not(&mut t.compiler, zero);
    assert_eq!(not_node.get_token(&t.compiler), Token::NOT);
    assert_eq!(
        not_node.get_color(&t.compiler),
        Some(standard_colors::BOOLEAN.clone())
    );
}

// port: AstFactoryTest#testCreateSingleVarNameDeclaration_jstypes
#[test]
fn test_create_single_var_name_declaration_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let value_node = ast_factory.create_boolean(&mut t.compiler, true);
    let const_node = ast_factory.create_single_var_name_declaration_with_value(
        &mut t.compiler,
        "myTrue",
        value_node,
    );
    assert!(const_node.is_var(&t.compiler));
    let name_node = const_node.get_only_child(&t.compiler);
    assert!(name_node.is_name(&t.compiler));
    assert_eq!(name_node.get_string(&t.compiler), "myTrue");
    assert_eq!(name_node.get_only_child(&t.compiler), value_node);
    assert_eq!(
        name_node.get_jstype(&t.compiler),
        value_node.get_jstype(&t.compiler)
    );
}

// port: AstFactoryTest#testCreateSingleVarNameDeclaration_colors
#[test]
fn test_create_single_var_name_declaration_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let value_node = ast_factory.create_boolean(&mut t.compiler, true);
    let const_node = ast_factory.create_single_var_name_declaration_with_value(
        &mut t.compiler,
        "myTrue",
        value_node,
    );
    assert!(const_node.is_var(&t.compiler));
    let name_node = const_node.get_only_child(&t.compiler);
    assert!(name_node.is_name(&t.compiler));
    assert_eq!(name_node.get_string(&t.compiler), "myTrue");
    assert_eq!(name_node.get_only_child(&t.compiler), value_node);
    assert_eq!(
        name_node.get_color(&t.compiler),
        value_node.get_color(&t.compiler)
    );
}

// port: AstFactoryTest#testCreateSuperReference
#[test]
fn test_create_super_reference() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let string_type = t.get_native_type(JSTypeNative::STRING_TYPE);
    let x = ast_factory.create_super(&mut t.compiler, AstFactory::type_jstype(Some(string_type)));
    assert_eq!(x.get_token(&t.compiler), Token::SUPER);
    let type_ = x.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateNewTargetReference
#[test]
fn test_create_new_target_reference() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let string_type = t.get_native_type(JSTypeNative::STRING_TYPE);
    let x =
        ast_factory.create_new_target(&mut t.compiler, AstFactory::type_jstype(Some(string_type)));
    assert_eq!(x.get_token(&t.compiler), Token::NEW_TARGET);
    let type_ = x.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateStartOptChainGetprop
#[test]
fn test_create_start_opt_chain_getprop() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let type_template = ast_factory.create_number(&mut t.compiler, 0.0);
    let get_prop = ast_factory.create_start_opt_chain_getprop(
        &mut t.compiler,
        receiver,
        "y",
        AstFactory::type_node(type_template),
    );
    assert_eq!(get_prop.get_token(&t.compiler), Token::OPTCHAIN_GETPROP);
    assert_eq!(get_prop.get_string(&t.compiler), "y");
    assert_eq!(get_prop.get_first_child(&t.compiler), Some(receiver));
    assert_eq!(
        get_prop.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
    assert!(get_prop.is_optional_chain_start(&t.compiler));
}

// port: AstFactoryTest#testCreateContinueOptChainGetprop
#[test]
fn test_create_continue_opt_chain_getprop() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let type_template = ast_factory.create_number(&mut t.compiler, 0.0);
    let get_prop = ast_factory.create_continue_opt_chain_getprop(
        &mut t.compiler,
        receiver,
        "y",
        AstFactory::type_node(type_template),
    );
    assert_eq!(get_prop.get_token(&t.compiler), Token::OPTCHAIN_GETPROP);
    assert_eq!(get_prop.get_string(&t.compiler), "y");
    assert_eq!(get_prop.get_first_child(&t.compiler), Some(receiver));
    assert_eq!(
        get_prop.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
    assert!(!get_prop.is_optional_chain_start(&t.compiler));
}

// port: AstFactoryTest#testCreateStartOptChainGetelem
#[test]
fn test_create_start_opt_chain_getelem() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let elem = ast_factory.create_number(&mut t.compiler, 0.0);
    let type_template = ast_factory.create_number(&mut t.compiler, 0.0);
    let get_elem = ast_factory.create_start_opt_chain_getelem(
        &mut t.compiler,
        receiver,
        elem,
        AstFactory::type_node(type_template),
    );
    assert_eq!(get_elem.get_token(&t.compiler), Token::OPTCHAIN_GETELEM);
    assert_eq!(get_elem.get_first_child(&t.compiler), Some(receiver));
    assert_eq!(get_elem.get_second_child(&t.compiler), Some(elem));
    assert_eq!(
        get_elem.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
    assert!(get_elem.is_optional_chain_start(&t.compiler));
}

// port: AstFactoryTest#testCreateContinueOptChainGetelem
#[test]
fn test_create_continue_opt_chain_getelem() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let elem = ast_factory.create_number(&mut t.compiler, 0.0);
    let type_template = ast_factory.create_number(&mut t.compiler, 0.0);
    let get_elem = ast_factory.create_continue_opt_chain_getelem(
        &mut t.compiler,
        receiver,
        elem,
        AstFactory::type_node(type_template),
    );
    assert_eq!(get_elem.get_token(&t.compiler), Token::OPTCHAIN_GETELEM);
    assert_eq!(get_elem.get_first_child(&t.compiler), Some(receiver));
    assert_eq!(get_elem.get_second_child(&t.compiler), Some(elem));
    assert_eq!(
        get_elem.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
    assert!(!get_elem.is_optional_chain_start(&t.compiler));
}

// port: AstFactoryTest#testCreateContinueOptChainCall
#[test]
fn test_create_continue_opt_chain_call() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let receiver = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let type_template = ast_factory.create_number(&mut t.compiler, 0.0);
    let arg1 = ast_factory.create_number(&mut t.compiler, 1.0);
    let arg2 = ast_factory.create_number(&mut t.compiler, 2.0);
    let call = ast_factory.create_continue_opt_chain_call(
        &mut t.compiler,
        receiver,
        AstFactory::type_node(type_template),
        &[arg1, arg2],
    );
    assert_eq!(call.get_token(&t.compiler), Token::OPTCHAIN_CALL);
    assert_eq!(call.get_first_child(&t.compiler), Some(receiver));
    assert_eq!(call.get_second_child(&t.compiler), Some(arg1));
    assert_eq!(call.get_last_child(&t.compiler), Some(arg2));
    assert_eq!(
        call.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
    assert!(!call.is_optional_chain_start(&t.compiler));
}

// port: AstFactoryTest#testCreateStringKey_jstypes
#[test]
fn test_create_string_key_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let number_node = ast_factory.create_number(&mut t.compiler, 2112.0);
    let string_key_node = ast_factory.create_string_key(&mut t.compiler, "key", number_node);
    assert_eq!(string_key_node.get_token(&t.compiler), Token::STRING_KEY);
    assert_eq!(string_key_node.get_string(&t.compiler), "key");
    assert_eq!(child_list(&t.compiler, string_key_node), vec![number_node]);
    let type_ = string_key_node.get_jstype(&t.compiler);
    t.assert_is_number(type_);
}

// port: AstFactoryTest#testCreateComputedProperty_jstypes
#[test]
fn test_create_computed_property_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let number_node = ast_factory.create_number(&mut t.compiler, 2112.0);
    let string_literal = ast_factory.create_string(&mut t.compiler, "string literal key");
    let computed_property_node =
        ast_factory.create_computed_property(&mut t.compiler, string_literal, number_node);
    assert_eq!(
        computed_property_node.get_token(&t.compiler),
        Token::COMPUTED_PROP
    );
    assert_eq!(
        child_list(&t.compiler, computed_property_node),
        vec![string_literal, number_node]
    );
    let type_ = computed_property_node.get_jstype(&t.compiler);
    t.assert_is_number(type_);
}

// port: AstFactoryTest#testCreateComputedProperty_colors
#[test]
fn test_create_computed_property_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();
    let number_node = ast_factory.create_number(&mut t.compiler, 2112.0);
    let string_literal = ast_factory.create_string(&mut t.compiler, "string literal key");
    let computed_property_node =
        ast_factory.create_computed_property(&mut t.compiler, string_literal, number_node);
    assert_eq!(
        computed_property_node.get_token(&t.compiler),
        Token::COMPUTED_PROP
    );
    assert_eq!(
        child_list(&t.compiler, computed_property_node),
        vec![string_literal, number_node]
    );
    assert_eq!(
        computed_property_node.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateGetterDef
#[test]
fn test_create_getter_def() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let value_node = ast_factory.create_string(&mut t.compiler, "value");
    let getter_node = ast_factory.create_getter_def(&mut t.compiler, "name", value_node);

    let c = &t.compiler;
    assert_eq!(getter_node.get_token(c), Token::GETTER_DEF);
    assert_eq!(getter_node.get_string(c), "name");
    assert_eq!(getter_node.get_child_count(c), 1);
    let getter_function_node = getter_node.get_only_child(c);
    assert!(getter_function_node.is_function(c));
    assert_eq!(getter_function_node.get_child_count(c), 3);
    // The first child is an empty string representing that the function has no name
    let first = getter_function_node.get_first_child(c).unwrap();
    assert!(first.is_name(c));
    assert_eq!(first.get_string(c), "");
    // the second child is the parameter list, which should be empty.
    let second = getter_function_node.get_second_child(c).unwrap();
    assert!(second.is_param_list(c));
    assert!(!second.has_children(c));
    let getter_function_body = getter_function_node.get_last_child(c).unwrap();
    let return_node = getter_function_body.get_only_child(c);
    assert_eq!(return_node.get_token(c), Token::RETURN);
    assert_eq!(return_node.get_only_child(c), value_node);
}

// port: AstFactoryTest#testCreateGetElem_colors
#[test]
fn test_create_get_elem_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let object_name = ast_factory.create_name(
        &mut t.compiler,
        "obj",
        AstFactory::type_native_and_color(
            JSTypeNative::STRING_TYPE,
            standard_colors::STRING.clone(),
        ),
    );
    let string_literal = ast_factory.create_string(&mut t.compiler, "string literal key");
    let get_elem_node = ast_factory.create_get_elem(&mut t.compiler, object_name, string_literal);

    assert_eq!(get_elem_node.get_token(&t.compiler), Token::GETELEM);
    assert_eq!(
        child_list(&t.compiler, get_elem_node),
        vec![object_name, string_literal]
    );
    assert_eq!(
        get_elem_node.get_color(&t.compiler),
        Some(standard_colors::UNKNOWN.clone())
    );
}

// port: AstFactoryTest#testCreateComma_jstypes
#[test]
fn test_create_comma_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let string_node = ast_factory.create_string(&mut t.compiler, "hi");
    let number_node = ast_factory.create_number(&mut t.compiler, 2112.0);
    let comma_node = ast_factory.create_comma(&mut t.compiler, string_node, number_node);
    assert_eq!(comma_node.get_token(&t.compiler), Token::COMMA);
    assert_eq!(
        child_list(&t.compiler, comma_node),
        vec![string_node, number_node]
    );
    let type_ = comma_node.get_jstype(&t.compiler);
    t.assert_is_number(type_);
}

// port: AstFactoryTest#testCreateComma_colors
#[test]
fn test_create_comma_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let string_node = ast_factory.create_string(&mut t.compiler, "hi");
    let number_node = ast_factory.create_number(&mut t.compiler, 2112.0);
    let comma_node = ast_factory.create_comma(&mut t.compiler, string_node, number_node);
    assert_eq!(comma_node.get_token(&t.compiler), Token::COMMA);
    assert_eq!(
        child_list(&t.compiler, comma_node),
        vec![string_node, number_node]
    );
    assert_eq!(
        comma_node.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateCommas_jstypes
#[test]
fn test_create_commas_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let string_node = ast_factory.create_string(&mut t.compiler, "hi");
    let number_node = ast_factory.create_number(&mut t.compiler, 2112.0);
    let true_node = ast_factory.create_boolean(&mut t.compiler, true);
    let false_node = ast_factory.create_boolean(&mut t.compiler, false);

    // "hi", 2112, true, false
    let string_number_true_false = ast_factory.create_commas(
        &mut t.compiler,
        string_node,
        number_node,
        &[true_node, false_node],
    );

    // ("hi", 2112, true), false
    assert_eq!(
        string_number_true_false.get_token(&t.compiler),
        Token::COMMA
    );
    let string_number_true = string_number_true_false
        .get_first_child(&t.compiler)
        .unwrap();
    assert_eq!(
        child_list(&t.compiler, string_number_true_false),
        vec![string_number_true, false_node]
    );
    let type_ = string_number_true_false.get_jstype(&t.compiler);
    t.assert_is_boolean(type_);

    // ("hi", 2112), true
    let string_number = string_number_true.get_first_child(&t.compiler).unwrap();
    assert_eq!(
        child_list(&t.compiler, string_number_true),
        vec![string_number, true_node]
    );
    let type_ = string_number_true.get_jstype(&t.compiler);
    t.assert_is_boolean(type_);

    // "hi", 2112
    assert_eq!(
        child_list(&t.compiler, string_number),
        vec![string_node, number_node]
    );
    let type_ = string_number.get_jstype(&t.compiler);
    t.assert_is_number(type_);
}

// port: AstFactoryTest#testCreateIn_jstypes
#[test]
fn test_create_in_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let prop = ast_factory.create_string(&mut t.compiler, "prop");
    let obj = IR::name(&mut t.compiler, "obj"); // TODO(bradfordcsmith): This should have a type on it.
    let n = ast_factory.create_in(&mut t.compiler, prop, obj);
    assert_eq!(n.get_token(&t.compiler), Token::IN);
    let type_ = n.get_jstype(&t.compiler);
    t.assert_is_boolean(type_);
    assert_eq!(child_list(&t.compiler, n), vec![prop, obj]);
}

// port: AstFactoryTest#testCreateIn_colors
#[test]
fn test_create_in_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let prop = ast_factory.create_string(&mut t.compiler, "prop");
    let obj = IR::name(&mut t.compiler, "obj"); // TODO(bradfordcsmith): This should have a type on it.
    let n = ast_factory.create_in(&mut t.compiler, prop, obj);
    assert_eq!(n.get_token(&t.compiler), Token::IN);
    assert_eq!(
        n.get_color(&t.compiler),
        Some(standard_colors::BOOLEAN.clone())
    );
    assert_eq!(child_list(&t.compiler, n), vec![prop, obj]);
}

// port: AstFactoryTest#testCreateAnd_jstypes
#[test]
fn test_create_and_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let lhs = ast_factory.create_number(&mut t.compiler, 2112.0);
    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    let and_node = ast_factory.create_and(&mut t.compiler, lhs, string_literal);
    assert_eq!(and_node.get_token(&t.compiler), Token::AND);
    assert_eq!(child_list(&t.compiler, and_node), vec![lhs, string_literal]);
    let type_ = and_node.get_jstype(&t.compiler);
    t.assert_to_string_is_equal_to(type_, "(number|string)");
}

// port: AstFactoryTest#testCreateAnd_colors
#[test]
fn test_create_and_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let number_literal = ast_factory.create_number(&mut t.compiler, 2112.0);
    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    let and_node = ast_factory.create_and(&mut t.compiler, number_literal, string_literal);
    assert_eq!(and_node.get_token(&t.compiler), Token::AND);
    assert_eq!(
        child_list(&t.compiler, and_node),
        vec![number_literal, string_literal]
    );
    // hasColorThat().hasAlternates(STRING, NUMBER)
    let color = and_node.get_color(&t.compiler).unwrap();
    assert!(color.is_union());
    let alternates = color.get_union_elements();
    assert_eq!(alternates.len(), 2);
    assert!(alternates.contains(&standard_colors::STRING.clone()));
    assert!(alternates.contains(&standard_colors::NUMBER.clone()));
}

// port: AstFactoryTest#testCreateAndWithAlwaysFalsyLhs
#[test]
fn test_create_and_with_always_falsy_lhs() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let lhs = ast_factory.create_null(&mut t.compiler);
    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    let and_node = ast_factory.create_and(&mut t.compiler, lhs, string_literal);
    assert_eq!(and_node.get_token(&t.compiler), Token::AND);
    assert_eq!(child_list(&t.compiler, and_node), vec![lhs, string_literal]);
    let type_ = and_node.get_jstype(&t.compiler);
    t.assert_to_string_is_equal_to(type_, "(null|string)");
}

// port: AstFactoryTest#testCreateAndWithAlwaysTruthyLhs
#[test]
fn test_create_and_with_always_truthy_lhs() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let lhs = ast_factory.create_name(
        &mut t.compiler,
        "nonNullObject",
        AstFactory::type_native(JSTypeNative::OBJECT_TYPE),
    );
    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    let and_node = ast_factory.create_and(&mut t.compiler, lhs, string_literal);
    assert_eq!(and_node.get_token(&t.compiler), Token::AND);
    assert_eq!(child_list(&t.compiler, and_node), vec![lhs, string_literal]);
    let type_ = and_node.get_jstype(&t.compiler);
    t.assert_to_string_is_equal_to(type_, "(Object|string)");
}

// port: AstFactoryTest#testCreateOr
#[test]
fn test_create_or() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let lhs = ast_factory.create_number(&mut t.compiler, 2112.0);
    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    let and_node = ast_factory.create_or(&mut t.compiler, lhs, string_literal);
    assert_eq!(and_node.get_token(&t.compiler), Token::OR);
    assert_eq!(child_list(&t.compiler, and_node), vec![lhs, string_literal]);
    let type_ = and_node.get_jstype(&t.compiler);
    t.assert_to_string_is_equal_to(type_, "(number|string)");
}

// port: AstFactoryTest#testCreateOrWithAlwaysFalsyLhs
#[test]
fn test_create_or_with_always_falsy_lhs() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let lhs = ast_factory.create_null(&mut t.compiler);
    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    let and_node = ast_factory.create_or(&mut t.compiler, lhs, string_literal);
    assert_eq!(and_node.get_token(&t.compiler), Token::OR);
    assert_eq!(child_list(&t.compiler, and_node), vec![lhs, string_literal]);
    let type_ = and_node.get_jstype(&t.compiler);
    t.assert_to_string_is_equal_to(type_, "(null|string)");
}

// port: AstFactoryTest#testCreateOrWithAlwaysTruthyLhs
#[test]
fn test_create_or_with_always_truthy_lhs() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let lhs = ast_factory.create_name(
        &mut t.compiler,
        "nonNullObject",
        AstFactory::type_native(JSTypeNative::OBJECT_TYPE),
    );
    let string_literal = ast_factory.create_string(&mut t.compiler, "hello");
    let and_node = ast_factory.create_or(&mut t.compiler, lhs, string_literal);
    assert_eq!(and_node.get_token(&t.compiler), Token::OR);
    assert_eq!(child_list(&t.compiler, and_node), vec![lhs, string_literal]);
    let type_ = and_node.get_jstype(&t.compiler);
    t.assert_to_string_is_equal_to(type_, "(Object|string)");
}

// port: AstFactoryTest#testCreateAdd_stringAndNumber_jstypes
#[test]
fn test_create_add_string_and_number_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let str = ast_factory.create_string(&mut t.compiler, "x");
    let add = ast_factory.create_add(&mut t.compiler, zero, str);

    assert_eq!(add.get_token(&t.compiler), Token::ADD);
    assert_eq!(child_list(&t.compiler, add), vec![zero, str]);
    let expected = t.get_native_type(JSTypeNative::BIGINT_NUMBER_STRING);
    let actual = add.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateAdd_stringAndNumber_colors
#[test]
fn test_create_add_string_and_number_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let str = ast_factory.create_string(&mut t.compiler, "x");
    let add = ast_factory.create_add(&mut t.compiler, zero, str);

    assert_eq!(add.get_token(&t.compiler), Token::ADD);
    assert_eq!(child_list(&t.compiler, add), vec![zero, str]);
    assert_has_alternates(
        &add.get_color(&t.compiler).unwrap(),
        &[
            standard_colors::BIGINT.clone(),
            standard_colors::NUMBER.clone(),
            standard_colors::STRING.clone(),
        ],
    );
}

// port: AstFactoryTest#testCreateInc_prefix_jstypes
#[test]
fn test_create_inc_prefix_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let x = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let inc = ast_factory.create_inc(&mut t.compiler, x, /* is_post= */ false);

    assert_eq!(inc.get_token(&t.compiler), Token::INC);
    assert!(!inc.get_boolean_prop(&t.compiler, Prop::INCRDECR));
    assert_eq!(child_list(&t.compiler, inc), vec![x]);
    let expected = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let actual = inc.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateInc_postfix_jstypes
#[test]
fn test_create_inc_postfix_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let x = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let inc = ast_factory.create_inc(&mut t.compiler, x, /* is_post= */ true);

    assert_eq!(inc.get_token(&t.compiler), Token::INC);
    assert!(inc.get_boolean_prop(&t.compiler, Prop::INCRDECR));
    assert_eq!(child_list(&t.compiler, inc), vec![x]);
    let expected = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let actual = inc.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateInc_prefix_colors
#[test]
fn test_create_inc_prefix_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let x = ast_factory.create_name_with_unknown_type(&mut t.compiler, "x");
    let inc = ast_factory.create_inc(&mut t.compiler, x, /* is_post= */ false);

    assert_eq!(inc.get_token(&t.compiler), Token::INC);
    assert!(!inc.get_boolean_prop(&t.compiler, Prop::INCRDECR));
    assert_eq!(child_list(&t.compiler, inc), vec![x]);
    assert_eq!(
        inc.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateQNameFromSimpleStringAndTypedScope
#[test]
fn test_create_qname_from_simple_string_and_typed_scope() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = IR::root(&mut t.compiler, &[]);
    let scope = TypedScope::create_global_scope(&mut t.compiler, root);
    let x = IR::name(&mut t.compiler, "x");
    let number = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    scope.declare(&mut t.compiler, "x", Some(x), Some(number), None, true);

    let name = ast_factory.create_qname_using_js_type_info(&mut t.compiler, Some(scope), "x");

    assert_eq!(name.get_string(&t.compiler), "x");
    t.assert_is_number(name.get_jstype(&t.compiler));
}

// port: AstFactoryTest#testCreateQNameFromDottedStringAndTypedScope
#[test]
fn test_create_qname_from_dotted_string_and_typed_scope() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = IR::root(&mut t.compiler, &[]);
    let scope = TypedScope::create_global_scope(&mut t.compiler, root);
    // Declare a global "x" with the type "{y: number}".
    let number = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let object_with_y_prop = {
        let (registry, ast) = t.compiler.get_type_registry_and_ast();
        let object_with_y_prop = registry.create_anonymous_object_type(ast, None);
        object_with_y_prop.define_declared_property(registry, ast, "y", number, None);
        object_with_y_prop
    };
    let x = IR::name(&mut t.compiler, "x");
    scope.declare(
        &mut t.compiler,
        "x",
        Some(x),
        Some(object_with_y_prop),
        None,
        true,
    );

    let name = ast_factory.create_qname_using_js_type_info(&mut t.compiler, Some(scope), "x.y");

    assert!(name.matches_qualified_name(&t.compiler, "x.y"));
    t.assert_is_number(name.get_jstype(&t.compiler));
    let first_child = name.get_first_child(&t.compiler).unwrap();
    let first_child_type = first_child.get_jstype(&t.compiler).unwrap();
    let (registry, ast) = t.compiler.get_type_registry_and_ast();
    TypeSubject::assert_type(first_child_type).is_object_type_with_property(registry, ast, "y");
}

// port: AstFactoryTest#testCreateQNameFromStringAndTypedScope_crashesGivenMissingName
#[test]
fn test_create_qname_from_string_and_typed_scope_crashes_given_missing_name() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = IR::root(&mut t.compiler, &[]);
    let scope = TypedScope::create_global_scope(&mut t.compiler, root);

    // assertThrows(Exception.class, ...)
    let result = catch_unwind(AssertUnwindSafe(|| {
        ast_factory.create_qname_using_js_type_info(&mut t.compiler, Some(scope), "x")
    }));
    assert!(result.is_err());
}

// port: AstFactoryTest#testCreateQNameFromStringAndTypedScope_crashesGivenLocalScope
#[test]
fn test_create_qname_from_string_and_typed_scope_crashes_given_local_scope() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = IR::root(&mut t.compiler, &[]);
    let block = IR::block(&mut t.compiler);
    let script = IR::script_with_children(&mut t.compiler, &[block]);
    root.add_child_to_front(&mut t.compiler, script);

    let global_scope = TypedScope::create_global_scope(&mut t.compiler, root);
    let x = IR::name(&mut t.compiler, "x");
    let number = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    global_scope.declare(&mut t.compiler, "x", Some(x), Some(number), None, true);
    let local_scope = TypedScope::new(&mut t.compiler, global_scope, block);

    let _unused =
        ast_factory.create_qname_using_js_type_info(&mut t.compiler, Some(global_scope), "x");
    // assertThrows(IllegalArgumentException.class, ...): a failed checkArgument is a panic.
    let result = catch_unwind(AssertUnwindSafe(|| {
        ast_factory.create_qname_using_js_type_info(&mut t.compiler, Some(local_scope), "x")
    }));
    assert!(result.is_err());
}

// port: AstFactoryTest#testCreateAssignFromNodes_jstypes
#[test]
fn test_create_assign_from_nodes_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let lhs = ast_factory.create_name(
        &mut t.compiler,
        "x",
        AstFactory::type_native(JSTypeNative::STRING_TYPE),
    );
    let rhs = ast_factory.create_number(&mut t.compiler, 0.0);
    let assign = ast_factory.create_assign(&mut t.compiler, lhs, rhs);

    assert_eq!(assign.get_token(&t.compiler), Token::ASSIGN);
    assert_eq!(assign.get_first_child(&t.compiler), Some(lhs));
    assert_eq!(assign.get_second_child(&t.compiler), Some(rhs));
    let type_ = assign.get_jstype(&t.compiler);
    t.assert_is_number(type_); // take the rhs type, not lhs type
}

// port: AstFactoryTest#testCreateAssignFromNodes_colors
#[test]
fn test_create_assign_from_nodes_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let lhs = ast_factory.create_name(
        &mut t.compiler,
        "x",
        AstFactory::type_(standard_colors::STRING.clone()),
    );
    let rhs = ast_factory.create_number(&mut t.compiler, 0.0);
    let assign = ast_factory.create_assign(&mut t.compiler, lhs, rhs);

    assert_eq!(assign.get_token(&t.compiler), Token::ASSIGN);
    assert_eq!(assign.get_first_child(&t.compiler), Some(lhs));
    assert_eq!(assign.get_second_child(&t.compiler), Some(rhs));
    // rhs type, not lhs type
    assert_eq!(
        assign.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateObjectLit_empty_withColor
#[test]
fn test_create_object_lit_empty_with_color() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let obj_color = Color::single_builder()
        .set_id(ColorId::from_ascii("1"))
        .build();
    let object_lit = ast_factory.create_object_lit_with_type(
        &mut t.compiler,
        AstFactory::type_native_and_color(JSTypeNative::UNKNOWN_TYPE, obj_color.clone()),
        &[],
    );
    assert_eq!(object_lit.get_color(&t.compiler), Some(obj_color));
    assert_eq!(object_lit.get_token(&t.compiler), Token::OBJECTLIT);
    assert!(!object_lit.has_children(&t.compiler));
}

// port: AstFactoryTest#testCreateObjectLit_withElementsAndType
#[test]
fn test_create_object_lit_with_elements_and_type() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // Try creating an object literal that implements Thenable and type it as one.
    let thenable_type = t.get_native_type(JSTypeNative::THENABLE_TYPE);
    let hi = ast_factory.create_string(&mut t.compiler, "hi");
    let arrow = ast_factory.create_zero_arg_arrow_function_for_expression(&mut t.compiler, hi);
    let then_string_key = IR::string_key_with_value(&mut t.compiler, "then", arrow);
    let object_lit = ast_factory.create_object_lit_with_type(
        &mut t.compiler,
        AstFactory::type_jstype(Some(thenable_type)),
        &[then_string_key],
    );
    let actual = object_lit.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, thenable_type);
    assert_eq!(object_lit.get_token(&t.compiler), Token::OBJECTLIT);
    assert_eq!(object_lit.get_only_child(&t.compiler), then_string_key);
}

// port: AstFactoryTest#testCreateObjectLit_empty_colors
#[test]
fn test_create_object_lit_empty_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let object_lit = ast_factory.create_object_lit(&mut t.compiler, &[]);
    assert_eq!(
        object_lit.get_color(&t.compiler),
        Some(standard_colors::TOP_OBJECT.clone())
    );
    assert_eq!(object_lit.get_token(&t.compiler), Token::OBJECTLIT);
    assert!(!object_lit.has_children(&t.compiler));
}

// port: AstFactoryTest#testCreateDelProp_jstypes
#[test]
fn test_create_del_prop_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let obj = IR::name(&mut t.compiler, "obj");
    let getprop = IR::getprop(&mut t.compiler, obj, "prop");
    let delprop = ast_factory.create_del_prop(&mut t.compiler, getprop);

    assert_eq!(delprop.get_token(&t.compiler), Token::DELPROP);
    let expected = t.get_native_type(JSTypeNative::BOOLEAN_TYPE);
    let actual = delprop.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
    assert!(delprop.has_children(&t.compiler));
}

// port: AstFactoryTest#testCreateSheq_jstypes
#[test]
fn test_create_sheq_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let left = IR::string(&mut t.compiler, "left");
    let right = IR::number(&mut t.compiler, 0.0);
    let sheq = ast_factory.create_sheq(&mut t.compiler, left, right);

    assert_eq!(sheq.get_token(&t.compiler), Token::SHEQ);
    let expected = t.get_native_type(JSTypeNative::BOOLEAN_TYPE);
    let actual = sheq.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateHook_jstypes
#[test]
fn test_create_hook_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let string_type = t.get_native_type(JSTypeNative::STRING_TYPE);
    let number_type = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let condition = IR::false_node(&mut t.compiler);
    let left = IR::name(&mut t.compiler, "left").set_jstype(&mut t.compiler, Some(string_type));
    let right = IR::number(&mut t.compiler, 0.0).set_jstype(&mut t.compiler, Some(number_type));
    let hook = ast_factory.create_hook(&mut t.compiler, condition, left, right);

    assert_eq!(hook.get_token(&t.compiler), Token::HOOK);
    let (registry, ast) = t.compiler.get_type_registry_and_ast();
    let expected = registry.create_union_type(ast, &[string_type, number_type]);
    let actual = hook.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateHook_colors
#[test]
fn test_create_hook_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let condition = IR::false_node(&mut t.compiler);
    let left = ast_factory.create_string(&mut t.compiler, "left");
    let right = ast_factory.create_number(&mut t.compiler, 0.0);
    let hook = ast_factory.create_hook(&mut t.compiler, condition, left, right);

    assert_eq!(hook.get_token(&t.compiler), Token::HOOK);
    assert_has_alternates(
        &hook.get_color(&t.compiler).unwrap(),
        &[
            standard_colors::STRING.clone(),
            standard_colors::NUMBER.clone(),
        ],
    );
}

// port: AstFactoryTest#testCreateSub_jstypes
#[test]
fn test_create_sub_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let one = ast_factory.create_number(&mut t.compiler, 1.0);
    let sub = ast_factory.create_sub(&mut t.compiler, zero, one);

    assert_eq!(sub.get_token(&t.compiler), Token::SUB);
    assert_eq!(child_list(&t.compiler, sub), vec![zero, one]);
    let expected = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let actual = sub.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateSub_colors
#[test]
fn test_create_sub_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let one = ast_factory.create_number(&mut t.compiler, 1.0);
    let sub = ast_factory.create_sub(&mut t.compiler, zero, one);

    assert_eq!(sub.get_token(&t.compiler), Token::SUB);
    assert_eq!(child_list(&t.compiler, sub), vec![zero, one]);
    assert_eq!(
        sub.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateLessThan_jstypes
#[test]
fn test_create_less_than_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let one = ast_factory.create_number(&mut t.compiler, 1.0);
    let lt = ast_factory.create_less_than(&mut t.compiler, zero, one);

    assert_eq!(lt.get_token(&t.compiler), Token::LT);
    assert_eq!(child_list(&t.compiler, lt), vec![zero, one]);
    let expected = t.get_native_type(JSTypeNative::BOOLEAN_TYPE);
    let actual = lt.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateLessThan_colors
#[test]
fn test_create_less_than_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let one = ast_factory.create_number(&mut t.compiler, 1.0);
    let lt = ast_factory.create_less_than(&mut t.compiler, zero, one);

    assert_eq!(lt.get_token(&t.compiler), Token::LT);
    assert_eq!(child_list(&t.compiler, lt), vec![zero, one]);
    assert_eq!(
        lt.get_color(&t.compiler),
        Some(standard_colors::BOOLEAN.clone())
    );
}

// port: AstFactoryTest#testCreateBitwiseAnd_jstypes
#[test]
fn test_create_bitwise_and_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let one = ast_factory.create_number(&mut t.compiler, 1.0);
    let bit_and = ast_factory.create_bitwise_and(&mut t.compiler, zero, one);

    assert_eq!(bit_and.get_token(&t.compiler), Token::BITAND);
    assert_eq!(child_list(&t.compiler, bit_and), vec![zero, one]);
    let expected = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let actual = bit_and.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateBitwiseAnd_colors
#[test]
fn test_create_bitwise_and_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let one = ast_factory.create_number(&mut t.compiler, 1.0);
    let bit_and = ast_factory.create_bitwise_and(&mut t.compiler, zero, one);

    assert_eq!(bit_and.get_token(&t.compiler), Token::BITAND);
    assert_eq!(child_list(&t.compiler, bit_and), vec![zero, one]);
    assert_eq!(
        bit_and.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateRightShift_jstypes
#[test]
fn test_create_right_shift_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let one = ast_factory.create_number(&mut t.compiler, 1.0);
    let right_shift = ast_factory.create_right_shift(&mut t.compiler, zero, one);

    assert_eq!(right_shift.get_token(&t.compiler), Token::RSH);
    assert_eq!(child_list(&t.compiler, right_shift), vec![zero, one]);
    let expected = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let actual = right_shift.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateRightShift_colors
#[test]
fn test_create_right_shift_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let zero = ast_factory.create_number(&mut t.compiler, 0.0);
    let one = ast_factory.create_number(&mut t.compiler, 1.0);
    let right_shift = ast_factory.create_right_shift(&mut t.compiler, zero, one);

    assert_eq!(right_shift.get_token(&t.compiler), Token::RSH);
    assert_eq!(child_list(&t.compiler, right_shift), vec![zero, one]);
    assert_eq!(
        right_shift.get_color(&t.compiler),
        Some(standard_colors::NUMBER.clone())
    );
}

// port: AstFactoryTest#testCreateDelProp_colors
#[test]
fn test_create_del_prop_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let obj = IR::name(&mut t.compiler, "obj");
    let getprop = IR::getprop(&mut t.compiler, obj, "prop");
    let delprop = ast_factory.create_del_prop(&mut t.compiler, getprop);

    assert_eq!(delprop.get_token(&t.compiler), Token::DELPROP);
    assert_eq!(
        delprop.get_color(&t.compiler),
        Some(standard_colors::BOOLEAN.clone())
    );
    assert!(delprop.has_children(&t.compiler));
}

// port: AstFactoryTest#testCreateSheq_colors
#[test]
fn test_create_sheq_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let left = IR::string(&mut t.compiler, "left");
    let right = IR::number(&mut t.compiler, 0.0);
    let sheq = ast_factory.create_sheq(&mut t.compiler, left, right);

    assert_eq!(sheq.get_token(&t.compiler), Token::SHEQ);
    assert_eq!(
        sheq.get_color(&t.compiler),
        Some(standard_colors::BOOLEAN.clone())
    );
}

// port: AstFactoryTest#testCreateEq_jstypes
#[test]
fn test_create_eq_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let left = IR::string(&mut t.compiler, "left");
    let right = IR::number(&mut t.compiler, 0.0);
    let sheq = ast_factory.create_eq(&mut t.compiler, left, right);

    assert_eq!(sheq.get_token(&t.compiler), Token::EQ);
    let expected = t.get_native_type(JSTypeNative::BOOLEAN_TYPE);
    let actual = sheq.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateEq_colors
#[test]
fn test_create_eq_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let left = IR::string(&mut t.compiler, "left");
    let right = IR::number(&mut t.compiler, 0.0);
    let eq = ast_factory.create_eq(&mut t.compiler, left, right);

    assert_eq!(eq.get_token(&t.compiler), Token::EQ);
    assert_eq!(
        eq.get_color(&t.compiler),
        Some(standard_colors::BOOLEAN.clone())
    );
}

// port: AstFactoryTest#testCreateNe_jstypes
#[test]
fn test_create_ne_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let left = IR::string(&mut t.compiler, "left");
    let right = IR::number(&mut t.compiler, 0.0);
    let ne = ast_factory.create_ne(&mut t.compiler, left, right);

    assert_eq!(ne.get_token(&t.compiler), Token::NE);
    let expected = t.get_native_type(JSTypeNative::BOOLEAN_TYPE);
    let actual = ne.get_jstype(&t.compiler);
    t.assert_type_is_equal_to(actual, expected);
}

// port: AstFactoryTest#testCreateNe_colors
#[test]
fn test_create_ne_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let left = IR::string(&mut t.compiler, "left");
    let right = IR::number(&mut t.compiler, 0.0);
    let ne = ast_factory.create_ne(&mut t.compiler, left, right);

    assert_eq!(ne.get_token(&t.compiler), Token::NE);
    assert_eq!(
        ne.get_color(&t.compiler),
        Some(standard_colors::BOOLEAN.clone())
    );
}

/// Java's `MapBasedScope` is a `StaticTypedScope`, which extends `StaticScope`. closure-jstype's
/// `StaticTypedScope` is a separate trait (its slots need the registry), so the cases pass the
/// scope through this `StaticScope` view of it.
struct MapBasedStaticScope(MapBasedScope);

impl StaticScope for MapBasedStaticScope {
    fn get_root_node(&self) -> Option<NodeId> {
        StaticTypedScope::get_root_node(&self.0)
    }
    fn get_parent_scope(&self) -> Option<&dyn StaticScope> {
        // AbstractStaticScope#getParentScope: MapBasedScope has no parent.
        assert!(StaticTypedScope::get_parent_scope(&self.0).is_none());
        None
    }
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticSlot> {
        // The cases' scopes are empty: there is no typed slot to present as a StaticSlot.
        assert!(StaticTypedScope::get_slot(&self.0, name).is_none());
        None
    }
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticSlot> {
        assert!(StaticTypedScope::get_own_slot(&self.0, name).is_none());
        None
    }
}

/// `assertThrows(IllegalStateException.class, ...)`: a failed `checkState` is a panic.
fn assert_throws_illegal_state(f: impl FnOnce()) -> String {
    let err = catch_unwind(AssertUnwindSafe(f)).expect_err("expected IllegalStateException");
    if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    }
}

// port: AstFactoryTest#testCreateRuntimeField_throwsIfFieldNotInjected
#[test]
fn test_create_runtime_field_throws_if_field_not_injected() {
    // Given
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_without_types();
    let scope = MapBasedStaticScope(MapBasedScope::new(Vec::new()));
    let field = t
        .runtime_js_lib_manager
        .lock()
        .unwrap()
        .get_js_lib_field("$jscomp.global");

    // When
    let message = assert_throws_illegal_state(|| {
        ast_factory.create_qname_for_field(&mut t.compiler, &scope, field.as_ref());
    });
    assert_eq!(message, "Field $jscomp.global is not injected");
}

// port: AstFactoryTest#testCreateRuntimeField_succeeds
#[test]
fn test_create_runtime_field_succeeds() {
    // Given
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_without_types();
    let scope = MapBasedStaticScope(MapBasedScope::new(Vec::new()));
    let field = t
        .runtime_js_lib_manager
        .lock()
        .unwrap()
        .get_js_lib_field("$jscomp.global");
    t.runtime_js_lib_manager
        .lock()
        .unwrap()
        .inject_lib_for_field(&mut t.compiler, "$jscomp.global");

    // When
    let result = ast_factory.create_qname_for_field(&mut t.compiler, &scope, field.as_ref());

    // Then
    assert!(result.matches_qualified_name(&t.compiler, "$jscomp.global"));
}

// port: AstFactoryTest#testCreateRuntimeField_inExternMode_succeeds_andUsesUnderscoredName
#[test]
fn test_create_runtime_field_in_extern_mode_succeeds_and_uses_underscored_name() {
    use closure_jscomp::source_file::SourceFile;
    use closure_rhino::static_source_file::{SourceKind, StaticSourceFile};
    let mut t = AstFactoryTest::set_up();
    // Given
    let externs = IR::script(&mut t.compiler);
    let file: Arc<dyn StaticSourceFile> = Arc::new(SourceFile::from_code_with_kind(
        "externs.js",
        "",
        SourceKind::EXTERN,
    ));
    externs.set_static_source_file(&mut t.compiler, Some(file));
    t.runtime_js_lib_manager = Arc::new(Mutex::new(RuntimeJsLibManager::create(
        RuntimeLibraryMode::EXTERN_FIELD_NAMES,
        // TestResourceProvider
        Box::new(|_: &mut Compiler, _unused1: &str, _unused2: &str| {
            panic!("UnsupportedOperationException")
        }),
        Compiler::get_change_tracker_and_ast,
        Box::new(move |_: &mut Compiler| externs),
    )));
    let ast_factory = t.create_test_ast_factory_without_types();
    let scope = MapBasedStaticScope(MapBasedScope::new(Vec::new()));
    let field = t
        .runtime_js_lib_manager
        .lock()
        .unwrap()
        .get_js_lib_field("$jscomp.global");
    t.runtime_js_lib_manager
        .lock()
        .unwrap()
        .inject_lib_for_field(&mut t.compiler, "$jscomp.global");

    // When
    let result = ast_factory.create_qname_for_field(&mut t.compiler, &scope, field.as_ref());

    // Then
    assert!(result.matches_name(&t.compiler, "$jscomp_global"));
}

// port: AstFactoryTest#testCreateJscompMakeIteratorCall_throwsIfJscompMakeIteratorNotInjected
#[test]
fn test_create_jscomp_make_iterator_call_throws_if_jscomp_make_iterator_not_injected() {
    // Given
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_without_types();
    let iterable = IR::name(&mut t.compiler, "arr");
    let scope = MapBasedStaticScope(MapBasedScope::new(Vec::new()));

    // When
    let message = assert_throws_illegal_state(|| {
        ast_factory.create_jscomp_make_iterator_call(&mut t.compiler, iterable, &scope);
    });
    assert_eq!(message, "Field $jscomp.makeIterator is not injected");
}

// port: AstFactoryTest#testCreateJscompMakeIteratorCall_succeeds
#[test]
fn test_create_jscomp_make_iterator_call_succeeds() {
    // Given
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_without_types();
    let iterable = IR::name(&mut t.compiler, "arr");
    t.runtime_js_lib_manager
        .lock()
        .unwrap()
        .inject_lib_for_field(&mut t.compiler, "$jscomp.makeIterator");

    // When
    let result = ast_factory.create_jscomp_make_iterator_call(
        &mut t.compiler,
        iterable,
        &MapBasedStaticScope(MapBasedScope::empty_scope()),
    );

    // Then
    assert!(result.is_call(&t.compiler));
    assert!(
        result
            .get_first_child(&t.compiler)
            .unwrap()
            .matches_qualified_name(&t.compiler, "$jscomp.makeIterator")
    );
    assert_eq!(result.get_last_child(&t.compiler), Some(iterable));
}

/// `classNode.getJSTypeRequired().assertFunctionType().getInstanceType()`
fn instance_type_of(t: &mut AstFactoryTest, class_node: NodeId) -> closure_jstype::TypeId {
    use closure_jstype::{function_type::FunctionType, js_type::JSType};
    let class_type = class_node.get_jstype_required(&t.compiler);
    let registry = t.compiler.get_type_registry();
    let function_type = class_type.to_maybe_function_type(registry).unwrap();
    function_type.get_instance_type(registry).unwrap()
}

/// `root.getFirstChild() // script .getFirstChild() // function .getJSType()`
fn first_function_type(t: &AstFactoryTest, root: NodeId) -> Option<closure_jstype::TypeId> {
    let script = root.get_first_child(&t.compiler).unwrap();
    script
        .get_first_child(&t.compiler)
        .unwrap()
        .get_jstype(&t.compiler)
}

// port: AstFactoryTest#testCreateArgumentsReference_jstypes
#[test]
fn test_create_arguments_reference_jstypes() {
    let mut t = AstFactoryTest::set_up();
    // Make sure the compiler's type registry includes the standard externs definition for
    // Arguments.
    let externs = closure_testing::testing::test_externs_builder::TestExternsBuilder::new()
        .add_arguments()
        .build()
        .to_string_lossy();
    t.parse_and_add_types_with_externs(&externs, "");

    let ast_factory = t.create_test_ast_factory();

    let arguments_node = ast_factory.create_arguments_reference(&mut t.compiler);
    assert!(arguments_node.matches_name(&t.compiler, "arguments"));
    let arguments = {
        let (registry, ast) = t.compiler.get_type_registry_and_ast();
        registry.get_global_type(ast, "Arguments").unwrap()
    };
    t.assert_type_is_equal_to(arguments_node.get_jstype(&t.compiler), arguments);
}

// port: AstFactoryTest#testCreateArgumentsReference_colors
#[test]
fn test_create_arguments_reference_colors() {
    let mut t = AstFactoryTest::set_up();
    let externs = closure_testing::testing::test_externs_builder::TestExternsBuilder::new()
        .add_arguments()
        .build()
        .to_string_lossy();
    let root = t.parse_and_add_colors_with_externs(&externs, "function f() { arguments; }");

    let ast_factory = t.create_test_ast_factory_with_colors();

    let first_first = root.get_first_first_child(&t.compiler).unwrap();
    let block = closure_jscomp::node_util::NodeUtil::get_function_body(&t.compiler, first_first);
    let arguments_reference_node = block.get_first_first_child(&t.compiler).unwrap();
    let arguments_reference_color = arguments_reference_node.get_color(&t.compiler);

    let arguments_node = ast_factory.create_arguments_reference(&mut t.compiler);
    assert!(arguments_node.matches_name(&t.compiler, "arguments"));
    assert_eq!(
        arguments_node.get_color(&t.compiler),
        arguments_reference_color
    );
}

// port: AstFactoryTest#testCreateNameFromScope_jstypes
#[test]
fn test_create_name_from_scope_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types("/** @type {string} */ const X = 'hi';");
    let scope = t.get_scope(root);

    let x = ast_factory.create_name_in_scope(&mut t.compiler, Some(&scope), "X");
    assert_eq!(x.get_token(&t.compiler), Token::NAME);
    assert_eq!(x.get_string(&t.compiler), "X");
    let type_ = x.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateNameFromScope_colors
#[test]
fn test_create_name_from_scope_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let root = t.parse_and_add_colors("/** @type {string} */ const X = 'hi';");
    let scope = t.get_scope(root);

    let x = ast_factory.create_name_in_scope(&mut t.compiler, Some(&scope), "X");
    assert_eq!(x.get_token(&t.compiler), Token::NAME);
    assert_eq!(x.get_string(&t.compiler), "X");
    assert_eq!(
        x.get_color(&t.compiler),
        Some(standard_colors::STRING.clone())
    );
}

// port: AstFactoryTest#testCreateNameFromScope_crashesIfMissingVariable
#[test]
fn test_create_name_from_scope_crashes_if_missing_variable() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types("/** @type {string} */ const X = 'hi';");
    let scope = t.get_scope(root);

    let result = catch_unwind(AssertUnwindSafe(|| {
        ast_factory.create_name_in_scope(&mut t.compiler, Some(&scope), "missing")
    }));
    assert!(result.is_err());
}

// port: AstFactoryTest#createThisForEs6ClassMember_jstypes
#[test]
fn create_this_for_es6_class_member_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types(
        "class C {\n\
         \x20 method() {}\n\
         }\n",
    );

    let class_node = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // class node
        .unwrap();
    let member_def = class_node
        .get_last_child(&t.compiler) // class members
        .unwrap()
        .get_first_child(&t.compiler) // member function def
        .unwrap();

    let instance_type = instance_type_of(&mut t, class_node);

    let this_alias = ast_factory.create_this_for_es6_class_member(&mut t.compiler, member_def);
    assert_eq!(this_alias.get_token(&t.compiler), Token::THIS);
    t.assert_type_is_equal_to(this_alias.get_jstype(&t.compiler), instance_type);
}

// port: AstFactoryTest#createThisForEs6ClassMember_colors
#[test]
fn create_this_for_es6_class_member_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let root = t.parse_and_add_colors(
        "class C {\n\
         \x20 method() {}\n\
         }\n",
    );

    let class_node = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // class node
        .unwrap();
    let member_def = class_node
        .get_last_child(&t.compiler) // class members
        .unwrap()
        .get_first_child(&t.compiler) // member function def
        .unwrap();

    let instance_type = Color::create_union(
        class_node
            .get_color(&t.compiler)
            .expect("NullPointerException")
            .get_instance_colors(),
    );

    let this_alias = ast_factory.create_this_for_es6_class_member(&mut t.compiler, member_def);
    assert_eq!(this_alias.get_token(&t.compiler), Token::THIS);
    assert_eq!(this_alias.get_color(&t.compiler), Some(instance_type));
}

// port: AstFactoryTest#createThisForEs6ClassStaticMember_jstypes
#[test]
fn create_this_for_es6_class_static_member_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types(
        "class C {\n\
         \x20 static method() {}\n\
         }\n",
    );

    let class_node = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // class node
        .unwrap();
    let member_def = class_node
        .get_last_child(&t.compiler) // class members
        .unwrap()
        .get_first_child(&t.compiler) // member function def
        .unwrap();

    let this_alias = ast_factory.create_this_for_es6_class_member(&mut t.compiler, member_def);
    assert_eq!(this_alias.get_token(&t.compiler), Token::THIS);
    let class_type = class_node.get_jstype(&t.compiler).unwrap();
    t.assert_type_is_equal_to(this_alias.get_jstype(&t.compiler), class_type);
}

// port: AstFactoryTest#createThisForEs6ClassStaticMember_colors
#[test]
fn create_this_for_es6_class_static_member_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let root = t.parse_and_add_colors(
        "class C {\n\
         \x20 static method() {}\n\
         }\n",
    );

    let class_node = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // class node
        .unwrap();
    let member_def = class_node
        .get_last_child(&t.compiler) // class members
        .unwrap()
        .get_first_child(&t.compiler) // member function def
        .unwrap();

    let this_alias = ast_factory.create_this_for_es6_class_member(&mut t.compiler, member_def);
    assert_eq!(this_alias.get_token(&t.compiler), Token::THIS);
    let class_color = class_node.get_color(&t.compiler);
    assert_eq!(this_alias.get_color(&t.compiler), class_color);
}

// port: AstFactoryTest#createThisAliasReferenceForFunction
#[test]
fn create_this_alias_reference_for_function() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types(
        "class C {\n\
         \x20 method() {}\n\
         }\n",
    );

    let class_node = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // class node
        .unwrap();
    let instance_type = instance_type_of(&mut t, class_node);

    let this_alias = ast_factory.create_this_alias_reference_for_es6_class(
        &mut t.compiler,
        "thisAlias",
        class_node,
    );
    assert_eq!(this_alias.get_token(&t.compiler), Token::NAME);
    assert_eq!(this_alias.get_string(&t.compiler), "thisAlias");
    t.assert_type_is_equal_to(this_alias.get_jstype(&t.compiler), instance_type);
}

const FOO_STRING_NUMBER_TO_STRING: &str = "/**\n \
     * @param {string} arg1\n \
     * @param {number} arg2\n \
     * @return {string}\n \
     */\n\
     function foo() { return arg1; }\n";

// port: AstFactoryTest#testCreateCallWithTypeFromNode
#[test]
fn test_create_call_with_type_from_node() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types(FOO_STRING_NUMBER_TO_STRING);
    let scope = t.get_scope(root);

    // foo("hi", 2112)
    let callee = ast_factory.create_name_in_scope(&mut t.compiler, Some(&scope), "foo");
    let arg1 = ast_factory.create_string(&mut t.compiler, "hi");
    let arg2 = ast_factory.create_number(&mut t.compiler, 2112.0);
    let tmp = ast_factory.create_string(&mut t.compiler, "tmp");
    let call_node = ast_factory.create_call(
        &mut t.compiler,
        callee,
        AstFactory::type_node(tmp),
        &[arg1, arg2],
    );

    assert_eq!(call_node.get_token(&t.compiler), Token::CALL);
    assert!(call_node.get_boolean_prop(&t.compiler, Prop::FREE_CALL));
    assert_eq!(child_list(&t.compiler, call_node), vec![callee, arg1, arg2]);
    let type_ = call_node.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateCallWithColorFromNode
#[test]
fn test_create_call_with_color_from_node() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let root = t.parse_and_add_colors(FOO_STRING_NUMBER_TO_STRING);
    let scope = t.get_scope(root);

    // foo("hi", 2112)
    let callee = ast_factory.create_name_in_scope(&mut t.compiler, Some(&scope), "foo");
    let arg1 = ast_factory.create_string(&mut t.compiler, "hi");
    let arg2 = ast_factory.create_number(&mut t.compiler, 2112.0);
    let tmp = ast_factory.create_string(&mut t.compiler, "tmp");
    let call_node = ast_factory.create_call(
        &mut t.compiler,
        callee,
        AstFactory::type_node(tmp),
        &[arg1, arg2],
    );

    assert_eq!(call_node.get_token(&t.compiler), Token::CALL);
    assert!(call_node.get_boolean_prop(&t.compiler, Prop::FREE_CALL));
    assert_eq!(child_list(&t.compiler, call_node), vec![callee, arg1, arg2]);
    assert_eq!(
        call_node.get_color(&t.compiler),
        Some(standard_colors::STRING.clone())
    );
}

// port: AstFactoryTest#testCreateFreeCall
#[test]
fn test_create_free_call() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types(FOO_STRING_NUMBER_TO_STRING);
    let scope = t.get_scope(root);

    // foo("hi", 2112)
    let callee = ast_factory.create_name_in_scope(&mut t.compiler, Some(&scope), "foo");
    let arg1 = ast_factory.create_string(&mut t.compiler, "hi");
    let arg2 = ast_factory.create_number(&mut t.compiler, 2112.0);
    let call_node = ast_factory.create_call(
        &mut t.compiler,
        callee,
        AstFactory::type_native(JSTypeNative::STRING_TYPE),
        &[arg1, arg2],
    );

    assert_eq!(call_node.get_token(&t.compiler), Token::CALL);
    assert!(call_node.get_boolean_prop(&t.compiler, Prop::FREE_CALL));
    assert_eq!(child_list(&t.compiler, call_node), vec![callee, arg1, arg2]);
    let type_ = call_node.get_jstype(&t.compiler);
    t.assert_is_string(type_);
}

// port: AstFactoryTest#testCreateConstructorCall
#[test]
fn test_create_constructor_call() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    let root = t.parse_and_add_types(
        "class A {}\n\
         class B extends A {}\n",
    );

    let class_b_node = root
        .get_first_child(&t.compiler) // script node
        .unwrap()
        .get_second_child(&t.compiler)
        .unwrap();
    let class_b_extends_node = class_b_node.get_second_child(&t.compiler).unwrap();
    let class_b_instance_type = instance_type_of(&mut t, class_b_node);

    // simulate creating a call to super() intended to go in a constructor for B
    let extends_type = class_b_extends_node.get_jstype(&t.compiler);
    let callee = IR::super_node(&mut t.compiler).set_jstype(&mut t.compiler, extends_type);
    let arg1 = ast_factory.create_string(&mut t.compiler, "hi");
    let arg2 = ast_factory.create_number(&mut t.compiler, 2112.0);
    let call_node = ast_factory.create_constructor_call(
        &mut t.compiler,
        AstFactory::type_node(class_b_node),
        callee,
        &[arg1, arg2],
    );

    assert_eq!(call_node.get_token(&t.compiler), Token::CALL);
    assert!(call_node.get_boolean_prop(&t.compiler, Prop::FREE_CALL));
    assert_eq!(child_list(&t.compiler, call_node), vec![callee, arg1, arg2]);
    t.assert_type_is_equal_to(call_node.get_jstype(&t.compiler), class_b_instance_type);
}

// port: AstFactoryTest#testCreateEmptyFunction
#[test]
fn test_create_empty_function() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // just a quick way to get a valid function type
    let root = t.parse_and_add_types("function foo() {}");
    let function_type = first_function_type(&t, root);

    let empty_function =
        ast_factory.create_empty_function(&mut t.compiler, AstFactory::type_jstype(function_type));
    assert_eq!(empty_function.get_token(&t.compiler), Token::FUNCTION);
    t.assert_type_is_equal_to(
        empty_function.get_jstype(&t.compiler),
        function_type.unwrap(),
    );
}

// port: AstFactoryTest#testCreateFunction
#[test]
fn test_create_function() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // just a quick way to get a valid function type
    let root = t.parse_and_add_types("function foo() {}");
    let function_type = first_function_type(&t, root);

    let param_list = IR::param_list(&mut t.compiler, &[]);
    let body = IR::block(&mut t.compiler);

    let function_node = ast_factory.create_function(
        &mut t.compiler,
        "bar",
        param_list,
        body,
        AstFactory::type_jstype(function_type),
    );
    assert_eq!(function_node.get_token(&t.compiler), Token::FUNCTION);
    t.assert_type_is_equal_to(
        function_node.get_jstype(&t.compiler),
        function_type.unwrap(),
    );
    let function_name_node = function_node.get_first_child(&t.compiler).unwrap();
    assert!(function_name_node.is_name(&t.compiler));
    assert_eq!(function_name_node.get_string(&t.compiler), "bar");
    assert_eq!(
        child_list(&t.compiler, function_node),
        vec![function_name_node, param_list, body]
    );
}

// port: AstFactoryTest#testCreateMemberFunctionDef_jstypes
#[test]
fn test_create_member_function_def_jstypes() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // just a quick way to get a valid function type
    let root = t.parse_and_add_types("function foo() {}");
    let function_type = first_function_type(&t, root);

    let param_list = IR::param_list(&mut t.compiler, &[]);
    let body = IR::block(&mut t.compiler);
    let function_node = ast_factory.create_function(
        &mut t.compiler,
        "",
        param_list,
        body,
        AstFactory::type_jstype(function_type),
    );

    let member_function_def =
        ast_factory.create_member_function_def(&mut t.compiler, "bar", function_node);
    assert_eq!(
        member_function_def.get_token(&t.compiler),
        Token::MEMBER_FUNCTION_DEF
    );
    assert_eq!(member_function_def.get_string(&t.compiler), "bar");
    t.assert_type_is_equal_to(
        member_function_def.get_jstype(&t.compiler),
        function_type.unwrap(),
    );
}

// port: AstFactoryTest#testCreateMemberFunctionDef_colors
#[test]
fn test_create_member_function_def_colors() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    // just a quick way to get a valid function type
    let root = t.parse_and_add_colors("function foo() {}");
    let function_type = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // function
        .unwrap()
        .get_color(&t.compiler);

    let param_list = IR::param_list(&mut t.compiler, &[]);
    let body = IR::block(&mut t.compiler);
    let name = IR::name(&mut t.compiler, "");
    let function_node = IR::function(&mut t.compiler, name, param_list, body)
        .set_color(&mut t.compiler, function_type.clone());

    let member_function_def =
        ast_factory.create_member_function_def(&mut t.compiler, "bar", function_node);
    assert_eq!(
        member_function_def.get_token(&t.compiler),
        Token::MEMBER_FUNCTION_DEF
    );
    assert_eq!(member_function_def.get_string(&t.compiler), "bar");
    assert_eq!(member_function_def.get_color(&t.compiler), function_type);
}

// port: AstFactoryTest#testCreateZeroArgFunction
#[test]
fn test_create_zero_arg_function() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // just a quick way to get a valid function type
    let root = t.parse_and_add_types("/** @return {number} */ function foo() {}");
    let function_type = first_function_type(&t, root);

    let body = IR::block(&mut t.compiler);
    let return_type = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let function_node =
        ast_factory.create_zero_arg_function(&mut t.compiler, "bar", body, Some(return_type));

    t.assert_type_is_equal_to(
        function_node.get_jstype(&t.compiler),
        function_type.unwrap(),
    );
    assert_eq!(function_node.get_token(&t.compiler), Token::FUNCTION);
}

// port: AstFactoryTest#createZeroArgGeneratorFunction
#[test]
fn create_zero_arg_generator_function() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // just a quick way to get a valid generator function type
    let root = t.parse_and_add_types("/** @return {number} */ function *foo() { return 1; }");
    let function_type = first_function_type(&t, root);

    let body = IR::block(&mut t.compiler);
    let return_type = t.get_native_type(JSTypeNative::NUMBER_TYPE);
    let function_node = ast_factory.create_zero_arg_generator_function(
        &mut t.compiler,
        "bar",
        body,
        Some(return_type),
    );

    t.assert_type_is_equal_to(
        function_node.get_jstype(&t.compiler),
        function_type.unwrap(),
    );
    assert_eq!(function_node.get_token(&t.compiler), Token::FUNCTION);
}

// port: AstFactoryTest#testCreateZeroArgFunctionForExpression
#[test]
fn test_create_zero_arg_function_for_expression() {
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // quick way to get a function to contain the new arrow function and another arrow function
    // to compare types with
    let root = t.parse_and_add_types(
        "class C {\n\
         \x20 /** @return {number} */\n\
         \x20 foo() {\n\
         // TODO(b/118435472): compiler should be able to infer the return type\n\
         \x20   /**\n\
         \x20    * @return {number}\n\
         \x20    */\n\
         \x20   const orig = () => 1; // new arrow function exactly like this one\n\
         \x20 }\n\
         }\n",
    );

    let c = &t.compiler;
    let existing_arrow_function_node = root
        .get_first_child(c) // script
        .unwrap()
        .get_first_child(c) // class
        .unwrap()
        .get_last_child(c) // class members
        .unwrap()
        .get_first_child(c) // foo member function def
        .unwrap()
        .get_first_child(c) // foo function node
        .unwrap()
        .get_last_child(c) // foo function body
        .unwrap()
        .get_first_child(c) // const
        .unwrap()
        .get_only_child(c) // orig name node
        .get_only_child(c);

    let expression = ast_factory.create_number(&mut t.compiler, 1.0);
    let new_arrow_function_node =
        ast_factory.create_zero_arg_arrow_function_for_expression(&mut t.compiler, expression);

    closure_rhino::testing::node_subject::assert_node(new_arrow_function_node)
        .is_equal_to(&t.compiler, existing_arrow_function_node);
    let existing_type = existing_arrow_function_node.get_jstype_required(&t.compiler);
    t.assert_type_is_equal_to(
        new_arrow_function_node.get_jstype(&t.compiler),
        existing_type,
    );
}

// port: AstFactoryTest#testCreateObjectLit_empty
#[test]
fn test_create_object_lit_empty() {
    use closure_jstype::js_type::JSType;
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // just a quick way to get a valid object literal type
    let root = t.parse_and_add_types("({})");
    let object_lit_type = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // expression result
        .unwrap()
        .get_only_child(&t.compiler) // object literal
        .get_jstype(&t.compiler)
        .unwrap();

    let object_lit = ast_factory.create_object_lit(&mut t.compiler, &[]);

    t.assert_to_string_is_equal_to(object_lit.get_jstype(&t.compiler), "{}");
    let object_lit_jstype = object_lit.get_jstype(&t.compiler).unwrap();
    let registry = t.compiler.get_type_registry();
    assert_eq!(
        object_lit_jstype.get_type_class(registry),
        object_lit_type.get_type_class(registry)
    );
    assert_eq!(object_lit.get_token(&t.compiler), Token::OBJECTLIT);
    assert!(!object_lit.has_children(&t.compiler));
}

// port: AstFactoryTest#testCreateObjectLit_withElements
#[test]
fn test_create_object_lit_with_elements() {
    use closure_jstype::js_type::JSType;
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();

    // just a quick way to get a valid object literal type
    let root = t.parse_and_add_types("({})");
    let object_lit_type = root
        .get_first_child(&t.compiler) // script
        .unwrap()
        .get_first_child(&t.compiler) // expression result
        .unwrap()
        .get_only_child(&t.compiler) // object literal
        .get_jstype(&t.compiler)
        .unwrap();

    let a = IR::name(&mut t.compiler, "a");
    let spread = IR::object_spread(&mut t.compiler, a);
    let zero = IR::number(&mut t.compiler, 0.0);
    let string_key = IR::string_key_with_value(&mut t.compiler, "b", zero);

    let object_lit = ast_factory.create_object_lit(&mut t.compiler, &[spread, string_key]);

    t.assert_to_string_is_equal_to(object_lit.get_jstype(&t.compiler), "{}");
    let object_lit_jstype = object_lit.get_jstype(&t.compiler).unwrap();
    let registry = t.compiler.get_type_registry();
    assert_eq!(
        object_lit_jstype.get_type_class(registry),
        object_lit_type.get_type_class(registry)
    );
    assert_eq!(object_lit.get_token(&t.compiler), Token::OBJECTLIT);

    assert_eq!(object_lit.get_first_child(&t.compiler), Some(spread));
    assert_eq!(object_lit.get_second_child(&t.compiler), Some(string_key));
}

// port: AstFactoryTest#testCreateArraylit_jstypes
#[test]
fn test_create_arraylit_jstypes() {
    // Given
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let number_type = t.get_native_type(JSTypeNative::NUMBER_TYPE);

    let first = IR::number(&mut t.compiler, 0.0).set_jstype(&mut t.compiler, Some(number_type));
    let second = IR::number(&mut t.compiler, 1.0).set_jstype(&mut t.compiler, Some(number_type));
    let third = IR::number(&mut t.compiler, 2.0).set_jstype(&mut t.compiler, Some(number_type));

    let expected = t
        .parse_and_add_types("[0, 1, 2]")
        .get_first_child(&t.compiler) // Script
        .unwrap()
        .get_first_child(&t.compiler) // Expression
        .unwrap()
        .get_first_child(&t.compiler) // Array
        .unwrap();

    // When
    let array = ast_factory.create_arraylit(&mut t.compiler, &[first, second, third]);

    // Then
    closure_rhino::testing::node_subject::assert_node(array)
        .is_equivalent_to(&t.compiler, expected);
    let expected_type = expected.get_jstype(&t.compiler).unwrap();
    t.assert_type_is_equal_to(array.get_jstype(&t.compiler), expected_type);
}

// port: AstFactoryTest#testCreateArraylit_colors
#[test]
fn test_create_arraylit_colors() {
    // Given
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let first = ast_factory.create_number(&mut t.compiler, 0.0);
    let second = ast_factory.create_number(&mut t.compiler, 1.0);
    let third = ast_factory.create_number(&mut t.compiler, 2.0);

    let expected = t
        .parse_and_add_colors("[0, 1, 2]")
        .get_first_child(&t.compiler) // Script
        .unwrap()
        .get_first_child(&t.compiler) // Expression
        .unwrap()
        .get_first_child(&t.compiler) // Array
        .unwrap();

    // When
    let array = ast_factory.create_arraylit(&mut t.compiler, &[first, second, third]);

    // Then
    closure_rhino::testing::node_subject::assert_node(array)
        .is_equivalent_to(&t.compiler, expected);
    assert_eq!(
        array.get_color(&t.compiler),
        expected.get_color(&t.compiler)
    );
}

// port: AstFactoryTest#testCreateNewNode_jstypes
#[test]
fn test_create_new_node_jstypes() {
    // Given
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory();
    let number_type = t.get_native_type(JSTypeNative::NUMBER_TYPE);

    let first = IR::number(&mut t.compiler, 0.0).set_jstype(&mut t.compiler, Some(number_type));
    let second = IR::number(&mut t.compiler, 1.0).set_jstype(&mut t.compiler, Some(number_type));

    let class_node = t
        .parse_and_add_types(
            "class Example { constructor(arg0, arg1) {} }\n\
             new Example(0, 1);\n",
        )
        .get_first_child(&t.compiler) // Script
        .unwrap()
        .get_first_child(&t.compiler) // class
        .unwrap();

    let expected = class_node
        .get_next(&t.compiler) // ExpressionResult
        .unwrap()
        .get_first_child(&t.compiler) // NewExpression
        .unwrap();

    // When
    let example = ast_factory.create_name(
        &mut t.compiler,
        "Example",
        AstFactory::type_node(class_node),
    );
    let new_expr = ast_factory.create_new_node(&mut t.compiler, example, &[first, second]);

    // Then
    closure_rhino::testing::node_subject::assert_node(new_expr)
        .is_equivalent_to(&t.compiler, expected);
    let expected_type = expected.get_jstype(&t.compiler).unwrap();
    t.assert_type_is_equal_to(new_expr.get_jstype(&t.compiler), expected_type);
}

// port: AstFactoryTest#testCreateNewNode_colors
#[test]
fn test_create_new_node_colors() {
    // Given
    let mut t = AstFactoryTest::set_up();
    let ast_factory = t.create_test_ast_factory_with_colors();

    let first = ast_factory.create_number(&mut t.compiler, 0.0);
    let second = ast_factory.create_number(&mut t.compiler, 1.0);

    let class_node = t
        .parse_and_add_colors(
            "class Example { constructor(arg0, arg1) {} }\n\
             new Example(0, 1);\n",
        )
        .get_first_child(&t.compiler) // Script
        .unwrap()
        .get_first_child(&t.compiler) // class
        .unwrap();

    let expected = class_node
        .get_next(&t.compiler) // ExpressionResult
        .unwrap()
        .get_first_child(&t.compiler) // NewExpression
        .unwrap();

    // When
    let class_color = class_node
        .get_color(&t.compiler)
        .expect("NullPointerException");
    let example =
        ast_factory.create_name(&mut t.compiler, "Example", AstFactory::type_(class_color));
    let new_expr = ast_factory.create_new_node(&mut t.compiler, example, &[first, second]);

    // Then
    closure_rhino::testing::node_subject::assert_node(new_expr)
        .is_equivalent_to(&t.compiler, expected);
    let expected_color = expected.get_color(&t.compiler);
    assert_eq!(new_expr.get_color(&t.compiler), expected_color);
}
