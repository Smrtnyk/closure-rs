/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTypeTestCase.java,
//   test/com/google/javascript/jscomp/TypeCheckTest.java,
//   test/com/google/javascript/jscomp/TypeCheckTestCase.java.

//! Port of the TypeCheckTest methods whose post-call assertions are classified `rust_unit_test`
//! (corpus/unit/rust_unit_tests/TypeCheckTest.md, D-015 (a)) and of the matching
//! OptionalChainTypeCheckTest method (corpus/unit/rust_unit_tests/OptionalChainTypeCheckTest.md).
//!
//! Every other TypeCheckTest method is a record in corpus/unit/records/TypeCheckTest.jsonl.gz and
//! replays in the unit-record harness; these methods additionally inspect the JSTypes of the
//! type-checked AST and of the global TypedScope, which the records do not capture.
#![allow(clippy::too_many_lines)]
use closure_jscomp::{
    chainable_reverse_abstract_interpreter::ChainableReverseAbstractInterpreter,
    closure_reverse_abstract_interpreter::ClosureReverseAbstractInterpreter,
    coding_conventions::CodingConventions,
    compiler::Compiler,
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    gather_module_metadata::GatherModuleMetadata,
    js_error::JSError,
    process_closure_primitives::ProcessClosurePrimitives,
    semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter,
    type_check::{POSSIBLE_INEXISTENT_PROPERTY_EXPLANATION, TypeCheck},
    typed_scope::TypedScope,
    typed_scope_creator::TypedScopeCreator,
};
use closure_jstype::{
    JSTypeRegistry, TypeId, function_type::FunctionType, js_type::JSType,
    js_type_class::JSTypeClass, js_type_native::JSTypeNative, object_type::ObjectType,
    testing::type_subject::TypeSubject,
};
use closure_parsing::js_doc_info_parser::BAD_TYPE_WIKI_LINK;
use closure_rhino::{
    input_id::InputId, ir::IR, js_string::JsString, node::Ast, node::NodeId, token::Token,
};
use closure_testing::{
    compiler_test_case::java_trim,
    compiler_type_test_case::{CLOSURE_DEFS, DEFAULT_EXTERNS},
    harness_passes::NativeTypedScope,
    replay::replay_dsl::{CompilerHandle, DslValue},
    testing::{scope_subject::assert_scope, test_externs_builder::TestExternsBuilder},
    type_check_test_case::TypeCheckTestCase,
};
use indexmap::IndexSet;
use std::cell::RefMut;
use std::sync::Arc;

struct TypeCheckTest {
    tc: TypeCheckTestCase,
}

/// Java's `TypeCheckResult` with the scope unwrapped from the replay value.
struct TypeCheckResult {
    root: NodeId,
    scope: TypedScope,
}

fn default_externs() -> JsString {
    DEFAULT_EXTERNS.as_ref().unwrap().clone()
}

/// Rust-only: Java's `type instanceof ObjectType` (ObjectType and its subclasses).
fn is_instance_of_object_type(reg: &JSTypeRegistry, t: TypeId) -> bool {
    matches!(
        t.get_type_class(reg),
        JSTypeClass::ENUM
            | JSTypeClass::FUNCTION
            | JSTypeClass::INSTANCE_OBJECT
            | JSTypeClass::NAMED
            | JSTypeClass::NO
            | JSTypeClass::NO_OBJECT
            | JSTypeClass::NO_RESOLVED
            | JSTypeClass::PROTOTYPE_OBJECT
            | JSTypeClass::PROXY_OBJECT
            | JSTypeClass::RECORD
            | JSTypeClass::TEMPLATE
            | JSTypeClass::TEMPLATIZED
            | JSTypeClass::UNKNOWN
    )
}

/// Rust-only: Java's `type instanceof FunctionType` (FunctionType and its subclasses).
fn is_instance_of_function_type(reg: &JSTypeRegistry, t: TypeId) -> bool {
    matches!(
        t.get_type_class(reg),
        JSTypeClass::FUNCTION | JSTypeClass::NO_OBJECT | JSTypeClass::NO | JSTypeClass::NO_RESOLVED
    )
}

impl TypeCheckTest {
    // port: TypeCheckTestCase#setUp
    fn set_up() -> Self {
        let mut tc = TypeCheckTestCase::default();
        tc.set_up().unwrap();
        Self { tc }
    }

    fn handle(&self) -> &CompilerHandle {
        self.tc.base.compiler.as_ref().unwrap()
    }

    fn compiler(&self) -> RefMut<'_, Compiler> {
        self.handle().borrow_mut()
    }

    /// Runs `f` with the compiler's type registry and AST.
    fn with<T>(&self, f: impl FnOnce(&mut JSTypeRegistry, &Ast) -> T) -> T {
        let mut compiler = self.compiler();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        f(reg, ast)
    }

    // port: TypeCheckTestCase#parseAndTypeCheck(String)
    fn parse_and_type_check(&self, js: &str) -> NodeId {
        self.parse_and_type_check_externs("", js)
    }

    // port: TypeCheckTestCase#parseAndTypeCheck(String,String)
    fn parse_and_type_check_externs(&self, externs: impl Into<JsString>, js: &str) -> NodeId {
        self.parse_and_type_check_with_scope_externs(externs, js)
            .root
    }

    // port: TypeCheckTestCase#parseAndTypeCheckWithScope(String)
    fn parse_and_type_check_with_scope(&self, js: &str) -> TypeCheckResult {
        self.parse_and_type_check_with_scope_externs("", js)
    }

    // port: TypeCheckTestCase#parseAndTypeCheckWithScope(String,String)
    fn parse_and_type_check_with_scope_externs(
        &self,
        externs: impl Into<JsString>,
        js: &str,
    ) -> TypeCheckResult {
        let result = self
            .tc
            .parse_and_type_check_with_scope_externs(externs.into(), js)
            .unwrap();
        let DslValue::Native(scope) = result.scope else {
            panic!("processForTesting returns a TypedScope");
        };
        let scope = scope
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<NativeTypedScope>()
            .expect("a TypedScope")
            .scope;
        TypeCheckResult {
            root: result.root.unwrap(),
            scope,
        }
    }

    fn native(&self, t: JSTypeNative) -> TypeId {
        self.with(|reg, _| reg.get_native_type(t))
    }

    fn first(&self, n: NodeId) -> NodeId {
        n.get_first_child(&self.compiler()).unwrap()
    }

    fn last(&self, n: NodeId) -> NodeId {
        n.get_last_child(&self.compiler()).unwrap()
    }

    fn next(&self, n: NodeId) -> NodeId {
        n.get_next(&self.compiler()).unwrap()
    }

    fn first_first(&self, n: NodeId) -> NodeId {
        n.get_first_first_child(&self.compiler()).unwrap()
    }

    fn jstype(&self, n: NodeId) -> TypeId {
        n.get_jstype(&self.compiler()).unwrap()
    }

    fn token(&self, n: NodeId) -> Token {
        n.get_token(&self.compiler())
    }

    fn type_string(&self, t: TypeId) -> String {
        self.with(|reg, ast| t.to_string(reg, ast))
    }

    fn var_type(&self, scope: TypedScope, name: &str) -> TypeId {
        let compiler = self.compiler();
        let mut compiler = compiler;
        let var = scope.get_var(&mut compiler, name).unwrap();
        var.get_type(&compiler).unwrap()
    }

    fn property_type(&self, t: TypeId, name: &str) -> TypeId {
        self.with(|reg, ast| t.get_property_type(reg, ast, name))
    }

    fn has_property(&self, t: TypeId, name: &str) -> bool {
        self.with(|reg, ast| t.has_property(reg, ast, name))
    }

    fn reference_name(&self, t: TypeId) -> Option<String> {
        self.with(|reg, _| t.get_reference_name(reg).map(|s| s.to_string_lossy()))
    }

    fn is_object_type(&self, t: TypeId) -> bool {
        self.with(|reg, _| is_instance_of_object_type(reg, t))
    }

    fn is_function_type(&self, t: TypeId) -> bool {
        self.with(|reg, _| is_instance_of_function_type(reg, t))
    }

    fn is_unknown_type(&self, t: TypeId) -> bool {
        self.with(|reg, ast| t.is_unknown_type(reg, ast))
    }

    fn instance_type_of(&self, f: TypeId) -> TypeId {
        self.with(|reg, _| f.get_instance_type(reg)).unwrap()
    }

    // port: CompilerTypeTestCase#createUnionType
    fn create_union_type(&self, variants: &[TypeId]) -> TypeId {
        self.with(|reg, ast| reg.create_union_type(ast, variants))
    }

    // port: CompilerTypeTestCase#assertTypeEquals(JSType,JSType)
    fn assert_type_equals(&self, a: TypeId, b: TypeId) {
        self.with(|reg, ast| TypeSubject::assert_type(b).is_equal_to(reg, ast, a));
    }

    // port: CompilerTypeTestCase#assertTypeEquals(String,JSType,JSType)
    fn assert_type_equals_message(&self, msg: &str, a: TypeId, b: TypeId) {
        let equal = self.with(|reg, ast| b.equals(reg, ast, a));
        assert!(equal, "{msg}");
    }

    // port: TypeCheckTestCase#getInstanceType
    fn get_instance_type(&self, js1_node: NodeId) -> TypeId {
        TypeCheckTestCase::get_instance_type(self.handle(), js1_node)
            .unwrap()
            .unwrap()
    }

    // port: TypeCheckTestCase#checkObjectType
    fn check_object_type(&self, object_type: TypeId, property_name: &str, expected: TypeId) {
        self.tc
            .check_object_type(object_type, property_name, expected)
            .unwrap();
    }

    // port: TypeCheckTestCase#assertHasXMorePropertiesThanNativeObject
    fn assert_has_x_more_properties_than_native_object(&self, t: TypeId, n: i32) {
        self.tc
            .assert_has_x_more_properties_than_native_object(t, n)
            .unwrap();
    }

    // port: CompilerTypeTestCase#validateWarningsAndErrors
    fn validate_warnings_and_errors(&self) {
        self.tc.base.validate_warnings_and_errors().unwrap();
    }

    // port: TypeCheckTest#testAddingMethodsUsingPrototypeIdiomComplexNamespace
    fn test_adding_methods_using_prototype_idiom_complex_namespace(&self, p: &TypeCheckResult) {
        let goog = self.var_type(p.scope, "goog");
        self.assert_has_x_more_properties_than_native_object(goog, 1);
        let goog_a = self.property_type(goog, "A");
        assert!(self.is_function_type(goog_a));
        let class_a = self.instance_type_of(goog_a);
        self.assert_has_x_more_properties_than_native_object(class_a, 1);
        self.check_object_type(class_a, "m1", self.native(JSTypeNative::NUMBER_TYPE));
    }
}

impl TypeCheckTest {
    // port: TypeCheckTest#testClosureTypes
    fn test_closure_types(&self, js: &str, description: Option<&str>) {
        let descriptions = description.map(|d| vec![java_trim(d)]);
        self.test_closure_types_multiple_warnings(js, descriptions.as_deref());
    }

    // port: TypeCheckTest#testClosureTypesMultipleWarnings
    fn test_closure_types_multiple_warnings(&self, js: &str, descriptions: Option<&[&str]>) {
        let mut compiler = self.compiler();
        let options = compiler.get_options().clone();
        compiler.init_options(options);
        let parsed = compiler.parse_test_code(js);
        let js_root = IR::root(&mut compiler, &[parsed]);
        let externs_code = TestExternsBuilder::new()
            .add_string()
            .add_closure_externs()
            .add_extra(&[JsString::from(CLOSURE_DEFS)])
            .build();
        let parsed_externs = compiler.parse_test_code(externs_code);
        let externs = IR::root(&mut compiler, &[parsed_externs]);
        IR::root(&mut compiler, &[externs, js_root]);

        assert_eq!(
            compiler.get_error_count(),
            0,
            "parsing error: {}",
            join(&compiler.get_errors())
        );

        GatherModuleMetadata::new(false, ResolutionMode::BROWSER).process(
            &mut compiler,
            externs,
            js_root,
        );

        // For processing goog.forwardDeclare for forward typedefs.
        ProcessClosurePrimitives::new(&compiler).process(&mut compiler, externs, js_root);

        let semantic: Arc<dyn ChainableReverseAbstractInterpreter> =
            SemanticReverseAbstractInterpreter::new(compiler.get_type_registry());
        let interpreter = ClosureReverseAbstractInterpreter::new(compiler.get_type_registry())
            .append(semantic)
            .get_first();
        TypeCheck::new(&mut compiler, interpreter).process_for_testing(
            &mut compiler,
            Some(externs),
            js_root,
        );

        assert_eq!(
            compiler.get_error_count(),
            0,
            "unexpected error(s) : {}",
            join(&compiler.get_errors())
        );

        match descriptions {
            None => assert_eq!(
                compiler.get_warning_count(),
                0,
                "unexpected warning(s) : {}",
                join(&compiler.get_warnings())
            ),
            Some(descriptions) => {
                assert_eq!(
                    usize::try_from(compiler.get_warning_count()).unwrap(),
                    descriptions.len(),
                    "unexpected warning(s) : {}",
                    join(&compiler.get_warnings())
                );
                let warnings = compiler.get_warnings();
                let mut actual_warning_descriptions = IndexSet::new();
                for warning in warnings.iter().take(descriptions.len()) {
                    actual_warning_descriptions.insert(warning.description().to_string());
                }
                let expected: IndexSet<String> =
                    descriptions.iter().map(|d| (*d).to_string()).collect();
                assert_eq!(actual_warning_descriptions, expected);
            }
        }
    }

    // port: TypeCheckTest#testNameNode
    fn test_name_node(&self, name: &str) -> Option<TypeId> {
        let (node, externs_root, js_root) = {
            let mut compiler = self.compiler();
            let node = compiler.new_string_with_token(Token::NAME, name);
            let parent = compiler.new_node_with_child(Token::SCRIPT, node);
            parent.set_input_id(&mut compiler, Some(Arc::new(InputId::new("code"))));

            let externs = compiler.new_node(Token::SCRIPT);
            externs.set_input_id(&mut compiler, Some(Arc::new(InputId::new("externs"))));

            let externs_root = IR::root(&mut compiler, &[externs]);
            let js_root = IR::root(&mut compiler, &[parent]);
            IR::root(&mut compiler, &[externs_root, js_root]);
            (node, externs_root, js_root)
        };

        let mut checker = self.tc.make_type_check().unwrap();
        checker
            .process_for_testing(&mut self.compiler(), externs_root, js_root)
            .unwrap();
        node.get_jstype(&self.compiler())
    }

    // port: TypeCheckTestCase#typeCheck
    fn type_check(&self, n: NodeId) -> NodeId {
        self.tc.type_check(n).unwrap()
    }
}

/// `Joiner.on(", ").join(errors)`.
fn join(errors: &[JSError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Java string concatenation.
fn cat(parts: &[&str]) -> String {
    parts.concat()
}

/// Runs one test method between Java's @Before and @After.
fn run(test: impl FnOnce(&TypeCheckTest)) {
    let t = TypeCheckTest::set_up();
    test(&t);
    t.validate_warnings_and_errors();
}

use JSTypeNative::{
    ARRAY_TYPE, BOOLEAN_TYPE, FUNCTION_TYPE, NULL_TYPE, NUMBER_TYPE, OBJECT_FUNCTION_TYPE,
    OBJECT_TYPE, STRING_TYPE, UNKNOWN_TYPE, VOID_TYPE,
};

#[test]
fn test_add_methods_prototype_two_ways() {
    run(|t| {
        let js1_node = t.parse_and_type_check_externs(
            default_externs(),
            "/** @constructor */function A() {}\n\
             A.prototype = {m1: 5, m2: true};\n\
             A.prototype.m3 = 'third property!';\n",
        );
        let instance_type = t.get_instance_type(js1_node);
        assert_eq!(t.type_string(instance_type), "A");
        t.assert_has_x_more_properties_than_native_object(instance_type, 3);
        t.check_object_type(instance_type, "m1", t.native(NUMBER_TYPE));
        t.check_object_type(instance_type, "m2", t.native(BOOLEAN_TYPE));
        t.check_object_type(instance_type, "m3", t.native(STRING_TYPE));
    });
}

#[test]
fn test_add_singleton_getter() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/** @constructor */ function Foo() {};\n\
             goog.addSingletonGetter(Foo);\n",
        );
        let o = t.jstype(t.first(n));
        assert_eq!(
            t.type_string(t.property_type(o, "getInstance")),
            "function(): Foo"
        );
        assert_eq!(t.type_string(t.property_type(o, "instance_")), "Foo");
    });
}

#[test]
fn test_adding_methods_prototype_idiom_and_object_literal_simple_namespace() {
    run(|t| {
        let js1_node = t.parse_and_type_check_externs(
            default_externs(),
            "/** @constructor */function A() {}\n\
             A.prototype = {m1: 5, m2: true}\n",
        );
        let instance_type = t.get_instance_type(js1_node);
        t.assert_has_x_more_properties_than_native_object(instance_type, 2);
        t.check_object_type(instance_type, "m1", t.native(NUMBER_TYPE));
        t.check_object_type(instance_type, "m2", t.native(BOOLEAN_TYPE));
    });
}

#[test]
fn test_adding_methods_using_prototype_idiom_complex_namespace1() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope_externs(
            default_externs(),
            "var goog = {};\n\
             goog.A = /** @constructor */function() {};\n\
             /** @type {number} */goog.A.prototype.m1 = 5\n",
        );
        t.test_adding_methods_using_prototype_idiom_complex_namespace(&p);
    });
}

#[test]
fn test_adding_methods_using_prototype_idiom_complex_namespace2() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope_externs(
            default_externs(),
            "var goog = {};\n\
             /** @constructor */goog.A = function() {};\n\
             /** @type {number} */goog.A.prototype.m1 = 5\n",
        );
        t.test_adding_methods_using_prototype_idiom_complex_namespace(&p);
    });
}

#[test]
fn test_adding_methods_using_prototype_idiom_simple_namespace() {
    run(|t| {
        let js1_node = t.parse_and_type_check_externs(
            default_externs(),
            "/** @constructor */function A() {}\n\
             A.prototype.m1 = 5\n",
        );
        let instance_type = t.get_instance_type(js1_node);
        t.assert_has_x_more_properties_than_native_object(instance_type, 1);
        t.check_object_type(instance_type, "m1", t.native(NUMBER_TYPE));
    });
}

#[test]
fn test_assign_to_untyped_property() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/** @constructor */ function Foo() {}\n\
             Foo.prototype.a = 1;\n\
             (new Foo).a;\n",
        );
        let node = t.first(t.last(n));
        let ty = t.jstype(node);
        assert!(!t.is_unknown_type(ty));
        assert!(t.with(|reg, ast| ty.is_number(reg, ast)));
    });
}

#[test]
fn test_assign_to_untyped_variable() {
    run(|t| {
        let n = t.parse_and_type_check("var z; z = 1;");
        let assign = t.first(t.last(n));
        let node = t.first(assign);
        let ty = t.jstype(node);
        assert!(!t.is_unknown_type(ty));
        assert_eq!(t.type_string(ty), "number");
    });
}

#[test]
fn test_call_array_constructor_as_function() {
    run(|t| {
        let n = t.parse_and_type_check("Array()");
        t.assert_type_equals(t.native(ARRAY_TYPE), t.jstype(t.first_first(n)));
    });
}

#[test]
fn test_call_date_constructor_as_function() {
    run(|t| {
        // ECMA-262 15.9.2: When Date is called as a function rather than as a
        // constructor, it returns a string.
        let n = t.parse_and_type_check("Date()");
        t.assert_type_equals(t.native(STRING_TYPE), t.jstype(t.first_first(n)));
    });
}

#[test]
fn test_call_error_constructor_as_function() {
    run(|t| {
        let externs = "/** @constructor\n    \
                       @param {string} message\n    \
                       @return {!Error} */\n\
                       function Error(message) {}\n";
        let n = t.parse_and_type_check_externs(externs, "Error('x')");
        let call = t.first_first(n);
        assert_eq!(t.token(call), Token::CALL);
        let callee = t.jstype(t.first(call));
        let instance = t
            .with(|reg, _| callee.to_maybe_function_type(reg))
            .map(|f| t.instance_type_of(f))
            .unwrap();
        t.assert_type_equals(instance, t.jstype(call));
    });
}

#[test]
fn test_cast4_types() {
    run(|t| {
        // downcast must be explicit
        let root = t.parse_and_type_check(
            "/** @constructor */function base() {}\n\
             /** @constructor\n \
             * @extends {base} */function derived() {}\n\
             /** @type {!derived} */ var baz =\n\
             /** @type {!derived} */(new base());\n",
        );
        let casted_expr_node = t.first(t.first_first(t.last(root)));
        assert_eq!(t.type_string(t.jstype(casted_expr_node)), "derived");
        let before_cast = casted_expr_node
            .get_jstype_before_cast(&t.compiler())
            .unwrap();
        assert_eq!(t.type_string(before_cast), "base");
    });
}

#[test]
fn test_complex_namespace() {
    run(|t| {
        let js = "var goog = {};\n\
                  goog.foo = {};\n\
                  goog.foo.bar = 5;\n";

        let p = t.parse_and_type_check_with_scope(js);

        // goog type in the scope
        let goog_scope_type = t.var_type(p.scope, "goog");
        assert!(t.is_object_type(goog_scope_type));
        assert!(
            t.has_property(goog_scope_type, "foo"),
            "foo property not present on goog type"
        );
        assert!(
            !t.has_property(goog_scope_type, "bar"),
            "bar property present on goog type"
        );

        // goog type on the VAR node
        let var_node = t.first(p.root);
        assert_eq!(t.token(var_node), Token::VAR);
        let goog_node_type = t.jstype(t.first(var_node));
        assert!(t.is_object_type(goog_node_type));

        // goog scope type and goog type on VAR node must be the same
        assert_eq!(goog_scope_type, goog_node_type);

        // goog type on the left of the GETPROP node (under fist ASSIGN)
        let getprop_foo1 = t.first_first(t.next(var_node));
        assert_eq!(t.token(getprop_foo1), Token::GETPROP);
        assert_eq!(
            t.first(getprop_foo1)
                .get_string(&t.compiler())
                .to_string_lossy(),
            "goog"
        );
        let goog_getprop_foo1_type = t.jstype(t.first(getprop_foo1));
        assert!(t.is_object_type(goog_getprop_foo1_type));

        // still the same type as the one on the variable
        assert_eq!(goog_getprop_foo1_type, goog_scope_type);

        // the foo property should be defined on goog
        let goog_foo_type = t.property_type(goog_scope_type, "foo");
        assert!(t.is_object_type(goog_foo_type));

        // goog type on the left of the GETPROP lower level node
        // (under second ASSIGN)
        let getprop_foo2 = t.first(t.first_first(t.next(t.next(var_node))));
        assert_eq!(t.token(getprop_foo2), Token::GETPROP);
        assert_eq!(
            t.first(getprop_foo2)
                .get_string(&t.compiler())
                .to_string_lossy(),
            "goog"
        );
        let goog_getprop_foo2_type = t.jstype(t.first(getprop_foo2));
        assert!(t.is_object_type(goog_getprop_foo2_type));

        // still the same type as the one on the variable
        assert_eq!(goog_getprop_foo2_type, goog_scope_type);

        // goog.foo type on the left of the top-level GETPROP node
        // (under second ASSIGN)
        let goog_foo_getprop2_type = t.jstype(getprop_foo2);
        assert!(
            t.is_object_type(goog_foo_getprop2_type),
            "goog.foo incorrectly annotated in goog.foo.bar selection"
        );
        assert!(
            !t.has_property(goog_foo_getprop2_type, "foo"),
            "foo property present on goog.foo type"
        );
        assert!(
            t.has_property(goog_foo_getprop2_type, "bar"),
            "bar property not present on goog.foo type"
        );
        t.assert_type_equals_message(
            "bar property on goog.foo type incorrectly inferred",
            t.native(NUMBER_TYPE),
            t.property_type(goog_foo_getprop2_type, "bar"),
        );
    });
}

#[test]
fn test_constructor_type7() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope("/** @constructor */function A(){};");

        let ty = t.var_type(p.scope, "A");
        assert!(t.is_function_type(ty));
        assert_eq!(t.reference_name(ty).as_deref(), Some("A"));
    });
}

#[test]
fn test_declare_built_in_constructor() {
    run(|t| {
        // Built-in prototype properties should be accessible
        // even if the built-in constructor is declared.
        let node = t.parse_and_type_check_externs(
            TestExternsBuilder::new().add_string().build(),
            "/** @constructor */ var String = function(opt_str) {};\n\
             (new String(\"x\")).charAt(0)\n",
        );
        t.assert_type_equals(t.native(STRING_TYPE), t.jstype(t.first(t.last(node))));
    });
}

#[test]
fn test_declared_native_type_equality() {
    run(|t| {
        let n = t.parse_and_type_check("/** @constructor */ function Object() {};");
        t.assert_type_equals(t.native(OBJECT_FUNCTION_TYPE), t.jstype(t.first(n)));
    });
}

#[test]
fn test_dont_add_methods_if_no_constructor() {
    run(|t| {
        let js1_node = t.parse_and_type_check(
            "function A() {}\n\
             A.prototype = {m1: 5, m2: true}\n",
        );
        let function_a_type = t.jstype(t.first(js1_node));
        assert_eq!(t.type_string(function_a_type), "function(): undefined");
        let function_type = t.native(FUNCTION_TYPE);
        t.assert_type_equals(t.native(UNKNOWN_TYPE), t.property_type(function_type, "m1"));
        t.assert_type_equals(t.native(UNKNOWN_TYPE), t.property_type(function_type, "m2"));
    });
}

#[test]
fn test_enum21() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/** @enum {string} */ var E = {A : 'a', B : 'b'};\n\
             /** @param {!E} x\n\
             @return {!E} */ function f(x) { return x; }\n",
        );
        let node_x = t.last(t.last(t.last(t.last(n))));
        let type_e = t.jstype(node_x);
        assert!(!t.with(|reg, ast| type_e.is_object(reg, ast)));
        assert!(!t.with(|reg, ast| type_e.is_nullable(reg, ast)));
    });
}

const STRING_SUBSTR_EXTERNS: &str = "/** @constructor */ var String = function(opt_str) {};\n\
     /**\n\
     * @param {number} start\n\
     * @param {number} opt_length\n\
     * @return {string}\n\
     */\n\
     String.prototype.substr = function(start, opt_length) {};\n";

#[test]
fn test_extend_built_in_type1() {
    run(|t| {
        let n1 = t.parse_and_type_check(&format!(
            "{STRING_SUBSTR_EXTERNS}(new String(\"x\")).substr(0,1);"
        ));
        t.assert_type_equals(t.native(STRING_TYPE), t.jstype(t.first(t.last(n1))));
    });
}

#[test]
fn test_extend_built_in_type2() {
    run(|t| {
        let n2 = t.parse_and_type_check(&format!("{STRING_SUBSTR_EXTERNS}\"x\".substr(0,1);"));
        t.assert_type_equals(t.native(STRING_TYPE), t.jstype(t.first(t.last(n2))));
    });
}

#[test]
fn test_extend_function1() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/**@return {number}*/Function.prototype.f = function() { return 1; };\n\
             (new Function()).f();\n",
        );
        let ty = t.jstype(t.last(t.last(n)));
        t.assert_type_equals(t.native(NUMBER_TYPE), ty);
    });
}

#[test]
fn test_extend_function2() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/**@return {number}*/Function.prototype.f = function() { return 1; };\n\
             (function() {}).f();\n",
        );
        let ty = t.jstype(t.last(t.last(n)));
        t.assert_type_equals(t.native(NUMBER_TYPE), ty);
    });
}

#[test]
fn test_flow_scope_bug1() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/** @param {number} a\n\
             * @param {number} b */\n\
             function f(a, b) {\n\
             /** @type {number} */\n\
             var i = 0;\n\
             for (; (i + a) < b; ++i) {}}\n",
        );

        // check the type of the add node for i + f
        let add = t.first(t.next(t.first(t.last(t.last(t.first(n))))));
        t.assert_type_equals(t.native(NUMBER_TYPE), t.jstype(add));
    });
}

#[test]
fn test_flow_scope_bug2() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/** @constructor */ function Foo() {};\n\
             Foo.prototype.hi = false;\n\
             function foo(a, b) {\n  \
               /** @type {Array} */\n  \
               var arr;\n  \
               /** @type {number} */\n  \
               var iter;\n  \
               for (iter = 0; iter < arr.length; ++ iter) {\n    \
                 /** @type {Foo} */\n    \
                 var afoo = arr[iter];\n    \
                 afoo;\n  \
               }\n\
             }\n",
        );

        // check the type of afoo when referenced
        let expected = t.with(|reg, ast| {
            let foo = reg.get_global_type(ast, "Foo").unwrap();
            reg.create_nullable_type(ast, foo)
        });
        let afoo = t.last(t.last(t.last(t.last(t.last(t.last(n))))));
        t.assert_type_equals(expected, t.jstype(afoo));
    });
}

#[test]
fn test_gather_propery_without_annotation1() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/** @constructor */ var T = function() {};\n\
             /** @type {!T} */var t; t.x; t;\n",
        );
        let ty = t.jstype(t.last(t.last(n)));
        assert!(!t.is_unknown_type(ty));
        assert!(t.is_object_type(ty));
        assert!(!t.has_property(ty, "x"));
    });
}

#[test]
fn test_gather_propery_without_annotation2() {
    run(|t| {
        let ns = t.parse_and_type_check_with_scope("/** @type {!Object} */var t; t.x; t;");
        let n = ns.root;
        let ty = t.jstype(t.last(t.last(n)));
        assert!(!t.is_unknown_type(ty));
        t.assert_type_equals(ty, t.native(OBJECT_TYPE));
        assert!(t.is_object_type(ty));
        assert!(!t.has_property(ty, "x"));
    });
}

#[test]
fn test_good_extends4() {
    run(|t| {
        // Ensure that @extends actually sets the base type of a constructor
        // correctly. Because this isn't part of the human-readable Function
        // definition, we need to crawl the prototype chain (eww).
        let n = t.parse_and_type_check(
            "var goog = {};\n\
             /** @constructor */goog.Base = function(){};\n\
             /** @constructor\n  \
               * @extends {goog.Base} */goog.Derived = function(){};\n",
        );
        let sub_type_name = t.first(t.last(t.last(n)));
        assert_eq!(
            sub_type_name
                .get_qualified_name(&t.compiler())
                .map(|s| s.to_string_lossy())
                .as_deref(),
            Some("goog.Derived")
        );

        let sub_ctor_type = t.jstype(t.next(sub_type_name));
        assert!(t.is_function_type(sub_ctor_type));
        assert_eq!(
            t.type_string(t.instance_type_of(sub_ctor_type)),
            "goog.Derived"
        );

        let super_type = t
            .with(|reg, ast| {
                let prototype = sub_ctor_type.get_prototype(reg, ast);
                prototype.get_implicit_prototype(reg, ast)
            })
            .unwrap();
        assert_eq!(t.type_string(super_type), "goog.Base");
    });
}

#[test]
fn test_namespaced_constructor() {
    run(|t| {
        let root = t.parse_and_type_check(
            "var goog = {};\n\
             /** @constructor */ goog.MyClass = function() {};\n\
             /** @return {!goog.MyClass} */\n\
             function foo() { return new goog.MyClass(); }\n",
        );

        let type_of_foo = t.jstype(t.last(root));
        assert!(t.is_function_type(type_of_foo));

        let ret_type = t.with(|reg, _| type_of_foo.get_return_type(reg));
        assert!(t.is_object_type(ret_type));
        assert_eq!(t.reference_name(ret_type).as_deref(), Some("goog.MyClass"));
    });
}

#[test]
fn test_new12() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope("var a = new Array();");
        let a = t.var_type(p.scope, "a");

        t.assert_type_equals(t.native(ARRAY_TYPE), a);
    });
}

#[test]
fn test_new13() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope(
            "/** @constructor */function FooBar(){};\n\
             var a = new FooBar();\n",
        );
        let a = t.var_type(p.scope, "a");

        assert!(t.is_object_type(a));
        assert_eq!(t.type_string(a), "FooBar");
    });
}

#[test]
fn test_new14() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope(
            "/** @constructor */var FooBar = function(){};\n\
             var a = new FooBar();\n",
        );
        let a = t.var_type(p.scope, "a");

        assert!(t.is_object_type(a));
        assert_eq!(t.type_string(a), "FooBar");
    });
}

#[test]
fn test_new15() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope(
            "var goog = {};\n\
             /** @constructor */goog.A = function(){};\n\
             var a = new goog.A();\n",
        );
        let a = t.var_type(p.scope, "a");

        assert!(t.is_object_type(a));
        assert_eq!(t.type_string(a), "goog.A");
    });
}

#[test]
fn test_new6() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope(
            "/** @constructor */function A(){};\n\
             var a = new A();\n",
        );

        let a_type = t.var_type(p.scope, "a");
        assert!(t.is_object_type(a_type));
        let constructor = t.with(|reg, _| a_type.get_constructor(reg)).unwrap();
        assert_eq!(t.reference_name(constructor).as_deref(), Some("A"));
    });
}

#[test]
fn test_object_literal() {
    run(|t| {
        let n = t.parse_and_type_check("var a = {m1: 7, m2: 'hello'}");

        let name_node = t.first_first(n);
        let object_node = t.first(name_node);

        // node extraction
        assert_eq!(t.token(name_node), Token::NAME);
        assert_eq!(t.token(object_node), Token::OBJECTLIT);

        // value's type
        let object_type = t.jstype(object_node);
        t.assert_type_equals(t.native(NUMBER_TYPE), t.property_type(object_type, "m1"));
        t.assert_type_equals(t.native(STRING_TYPE), t.property_type(object_type, "m2"));

        // variable's type
        t.assert_type_equals(object_type, t.jstype(name_node));
    });
}

#[test]
fn test_prototype_property_reference() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope_externs(
            default_externs(),
            "/** @constructor */\n\
             function Foo() {}\n\
             /** @param {number} a */\n\
             Foo.prototype.bar = function(a){};\n\
             /** @param {Foo} f */\n\
             function baz(f) {\n  \
               Foo.prototype.bar.call(f, 3);\n\
             }\n",
        );
        assert_eq!(t.compiler().get_error_count(), 0);
        assert_eq!(t.compiler().get_warning_count(), 0);

        let foo_type = t.var_type(p.scope, "Foo");
        assert!(t.is_function_type(foo_type));
        let prototype = t.with(|reg, ast| foo_type.get_prototype(reg, ast));
        assert_eq!(
            t.type_string(t.property_type(prototype, "bar")),
            "function(this:Foo, number): undefined"
        );
    });
}

#[test]
fn test_prototype_property_types() {
    run(|t| {
        let js1_node = t.parse_and_type_check_externs(
            default_externs(),
            "/** @constructor */function A() {\n  \
               /** @type {string} */ this.m1;\n  \
               /** @type {Object?} */ this.m2 = {};\n  \
               /** @type {boolean} */ this.m3;\n\
             }\n\
             /** @type {string} */ A.prototype.m4;\n\
             /** @type {number} */ A.prototype.m5 = 0;\n\
             /** @type {boolean} */ A.prototype.m6;\n",
        );

        let instance_type = t.get_instance_type(js1_node);
        t.assert_has_x_more_properties_than_native_object(instance_type, 6);
        t.check_object_type(instance_type, "m1", t.native(STRING_TYPE));
        let object_or_null = t.create_union_type(&[t.native(OBJECT_TYPE), t.native(NULL_TYPE)]);
        t.check_object_type(instance_type, "m2", object_or_null);
        t.check_object_type(instance_type, "m3", t.native(BOOLEAN_TYPE));
        t.check_object_type(instance_type, "m4", t.native(STRING_TYPE));
        t.check_object_type(instance_type, "m5", t.native(NUMBER_TYPE));
        t.check_object_type(instance_type, "m6", t.native(BOOLEAN_TYPE));
    });
}

#[test]
fn test_resolution_via_registry5() {
    run(|t| {
        let n = t.parse_and_type_check("/** @constructor */ u.T = function() {}; u.T");
        let ty = t.jstype(t.last(t.last(n)));
        assert!(!t.is_unknown_type(ty));
        assert!(t.is_function_type(ty));
        assert_eq!(
            t.reference_name(t.instance_type_of(ty)).as_deref(),
            Some("u.T")
        );
    });
}

#[test]
fn test_scoping10() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope("var a = function b(){};");

        // a declared, b is not
        {
            let mut compiler = t.compiler();
            assert_scope(&mut compiler, p.scope).declares("a");
            assert_scope(&mut compiler, p.scope).does_not_declare("b");
        }

        // checking that a has the correct assigned type
        assert_eq!(
            t.type_string(t.var_type(p.scope, "a")),
            "function(): undefined"
        );
    });
}

#[test]
fn test_sheq_refined_scope() {
    run(|t| {
        let n = t.parse_and_type_check(
            "/** @constructor */function A() {}\n\
             /** @constructor \n \
             @extends A */ function B() {}\n\
             /** @return {number} */\n\
             B.prototype.p = function() { return 1; }\n\
             /** @param {A} a\n \
             @param {B} b */\n\
             function f(a, b) {\n  \
               b.p();\n  \
               if (a === b) {\n    \
                 b.p();\n  \
               }\n\
             }\n",
        );
        let node_c = t.last(t.last(t.last(t.last(t.last(t.last(n))))));
        let type_c = t.jstype(node_c);
        assert!(t.with(|reg, ast| type_c.is_number(reg, ast)));

        let node_b = t.first_first(node_c);
        let type_b = t.jstype(node_b);
        assert_eq!(t.type_string(type_b), "B");
    });
}

#[test]
fn test_undefined_var() {
    run(|t| {
        let n = t.parse_and_type_check("var undefined;");
        t.assert_type_equals(t.native(VOID_TYPE), t.jstype(t.first_first(n)));
    });
}

#[test]
fn test_value_type_built_in_prototype_property_type() {
    run(|t| {
        let node = t.parse_and_type_check_externs(
            TestExternsBuilder::new().add_string().build(),
            "\"x\".charAt(0)",
        );
        t.assert_type_equals(t.native(STRING_TYPE), t.jstype(t.first_first(node)));
    });
}

#[test]
fn test_var1() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope("/** @type {(string|null)} */var a = null");

        let expected = t.create_union_type(&[t.native(STRING_TYPE), t.native(NULL_TYPE)]);
        t.assert_type_equals(expected, t.var_type(p.scope, "a"));
    });
}

#[test]
fn test_var3() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope("var a = 3;");

        t.assert_type_equals(t.native(NUMBER_TYPE), t.var_type(p.scope, "a"));
    });
}

#[test]
fn test_var4() {
    run(|t| {
        let p = t.parse_and_type_check_with_scope("var a = 3; a = 'string';");

        let expected = t.create_union_type(&[t.native(STRING_TYPE), t.native(NUMBER_TYPE)]);
        t.assert_type_equals(expected, t.var_type(p.scope, "a"));
    });
}

// port: TypeCheckTest#testBackwardsInferenceGoogArrayFilter1
#[test]
fn test_backwards_inference_goog_array_filter1() {
    run(|t| {
        t.test_closure_types("/** @type {Array<string>} */\nvar arr;\n/** @type {!Array<number>} */\nvar result = goog.array.filter(\n   arr,\n   function(item,index,src) {return false;});\n", Some("initializing variable\nfound   : Array<string>\nrequired: Array<number>\n"));
    });
}

// port: TypeCheckTest#testBackwardsInferenceGoogArrayFilter2
#[test]
fn test_backwards_inference_goog_array_filter2() {
    run(|t| {
        t.test_closure_types("/** @type {number} */\nvar out;\n/** @type {Array<string>} */\nvar arr;\nvar out4 = goog.array.filter(\n   arr,\n   function(item,index,src) {out = item; return false});\n", Some("assignment\nfound   : string\nrequired: number\n"));
    });
}

// port: TypeCheckTest#testBackwardsInferenceGoogArrayFilter3
#[test]
fn test_backwards_inference_goog_array_filter3() {
    run(|t| {
        t.test_closure_types("/** @type {string} */\nvar out;\n/** @type {Array<string>} */ var arr;\nvar result = goog.array.filter(\n   arr,\n   function(item,index,src) {out = index;});\n", Some("assignment\nfound   : number\nrequired: string\n"));
    });
}

// port: TypeCheckTest#testBackwardsInferenceGoogArrayFilter4
#[test]
fn test_backwards_inference_goog_array_filter4() {
    run(|t| {
        t.test_closure_types("/** @type {string} */\nvar out;\n/** @type {Array<string>} */ var arr;\nvar out4 = goog.array.filter(\n   arr,\n   function(item,index,srcArr) {out = srcArr;});\n", Some("assignment\nfound   : (Array|null|{length: number})\nrequired: string\n"));
    });
}

// port: TypeCheckTest#testBadImplements6
#[test]
fn test_bad_implements6() {
    run(|t| {
        t.test_closure_types_multiple_warnings("/** @interface */function Disposable() {}\n/** @type {function()} */ Disposable.prototype.bar = 3;\n", Some(&["assignment to property bar of Disposable.prototype\nfound   : number\nrequired: function(): ?", "interface members can only be empty property declarations, empty functions, or goog.abstractMethod"]));
    });
}

// port: TypeCheckTest#testCast13
#[test]
fn test_cast13() {
    run(|t| {
        // In a typespace world, types and values may collide on the same symbol.
        t.test_closure_types("goog.forwardDeclare('goog.foo');\ngoog.foo = function() {};\nfunction f() { return /** @type {goog.foo} */ (new Object()); }\n", None);
    });
}

// port: TypeCheckTest#testCast14
#[test]
fn test_cast14() {
    run(|t| {
        // Test to make sure that the forward-declaration still prevents
        // some warnings.
        t.test_closure_types("goog.forwardDeclare('goog.bar');\nfunction f() { return /** @type {goog.bar} */ (new Object()); }\n", None);
    });
}

// port: TypeCheckTest#testClosure7
#[test]
fn test_closure7() {
    run(|t| {
        t.test_closure_types("/** @type {string|null|undefined} */ var a = foo();\n/** @type {number} */\nvar b = goog.asserts.assert(a);\n", Some("initializing variable\nfound   : string\nrequired: number\n"));
    });
}

// port: TypeCheckTest#testComparison15
#[test]
fn test_comparison15() {
    run(|t| {
        t.test_closure_types("/** @constructor */ function F() {}\n/**\n * @param {number} x\n * @constructor\n * @extends {F}\n */\nfunction G(x) {}\ngoog.inherits(G, F);\n/**\n * @param {number} x\n * @constructor\n * @extends {G}\n */\nfunction H(x) {}\ngoog.inherits(H, G);\n/** @param {G} x */\nfunction f(x) { return x.constructor === H; }\n", None);
    });
}

// port: TypeCheckTest#testConversionFromInterfaceToRecursiveConstructor
#[test]
fn test_conversion_from_interface_to_recursive_constructor() {
    run(|t| {
        t.test_closure_types_multiple_warnings(&cat(&[&*TypeCheckTestCase::suppress_missing_property(&["foo"]), "/** @interface */ var OtherType = function() {}\n/** @implements {MyType}\n * @constructor */\nvar MyType = function() {}\n/** @type {MyType} */\nvar x = /** @type {!OtherType} */ (new Object());\n"]), Some(&["Cycle detected in inheritance chain of type MyType", "initializing variable\nfound   : OtherType\nrequired: (MyType|null)"]));
    });
}

// port: TypeCheckTest#testDuplicateLocalVarDecl
#[test]
fn test_duplicate_local_var_decl() {
    run(|t| {
        t.test_closure_types_multiple_warnings("/** @param {number} x */\nfunction f(x) { /** @type {string} */ var x = ''; }\n", Some(&["variable x redefined with type string, original definition at testcode:2 with type number", "initializing variable\nfound   : string\nrequired: number"]));
    });
}

// port: TypeCheckTest#testDuplicateStaticPropertyDecl4
#[test]
fn test_duplicate_static_property_decl4() {
    run(|t| {
        t.test_closure_types_multiple_warnings("/** @type {!Foo} */ goog.foo;\n/** @type {string} */ goog.foo = 'x';\n/** @constructor */ function Foo() {}\n", Some(&["assignment to property foo of goog\nfound   : string\nrequired: Foo", "variable goog.foo redefined with type string, original definition at testcode:1 with type Foo"]));
    });
}

// port: TypeCheckTest#testDuplicateStaticPropertyDecl5
#[test]
fn test_duplicate_static_property_decl5() {
    run(|t| {
        t.test_closure_types_multiple_warnings("var goog = goog || {};\n/** @type {!Foo} */ goog.foo;\n/** @type {string}\n * @suppress {duplicate} */ goog.foo = 'x';\n/** @constructor */ function Foo() {}\n", Some(&["assignment to property foo of goog\nfound   : string\nrequired: Foo"]));
    });
}

// port: TypeCheckTest#testEnum8
#[test]
fn test_enum8() {
    run(|t| {
        t.test_closure_types_multiple_warnings(
            "/** @enum */var a=8;",
            Some(&[
                "enum initializer must be an object literal or an enum",
                "initializing variable\nfound   : number\nrequired: enum{a}",
            ]),
        );
    });
}

// port: TypeCheckTest#testEnum9
#[test]
fn test_enum9() {
    run(|t| {
        t.test_closure_types_multiple_warnings(
            "/** @enum */ goog.a=8;",
            Some(&[
                "assignment to property a of goog\nfound   : number\nrequired: enum{goog.a}",
                "enum initializer must be an object literal or an enum",
            ]),
        );
    });
}

// port: TypeCheckTest#testErrorMismatchingPropertyOnInterface6
#[test]
fn test_error_mismatching_property_on_interface6() {
    run(|t| {
        t.test_closure_types_multiple_warnings("/** @interface */ function T() {};\n/** @return {number} */T.prototype.x = 1\n", Some(&["assignment to property x of T.prototype\nfound   : number\nrequired: function(this:T): number", "interface members can only be empty property declarations, empty functions, or goog.abstractMethod"]));
    });
}

// port: TypeCheckTest#testEs6ExtendCannotUseGoogInherits
#[test]
fn test_es6_extend_cannot_use_goog_inherits() {
    run(|t| {
        t.test_closure_types("class Super {}\n/** @extends {Super} */\nclass Sub {}\ngoog.inherits(Sub, Super);\n", Some("Do not use goog.inherits with ES6 classes. Use the ES6 `extends` keyword to inherit instead.\n"));
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration1
#[test]
fn test_forward_type_declaration1() {
    run(|t| {
        let f = "goog.forwardDeclare('MyType');\n/** @param {!MyType} x */ function f(x) { }\n";
        t.test_closure_types(f, None);
        t.test_closure_types(&cat(&[f, "f(3);"]), Some("actual parameter 1 of f does not match formal parameter\nfound   : number\nrequired: MyType\n"));
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration10
#[test]
fn test_forward_type_declaration10() {
    run(|t| {
        let f =
            "goog.forwardDeclare('MyType');\n/** @param {MyType|number} x */ function f(x) { }\n";
        t.test_closure_types(f, None);
        t.test_closure_types(&cat(&[f, "f(3);"]), None);
        t.test_closure_types(&cat(&[f, "f('3');"]), Some("actual parameter 1 of f does not match formal parameter\nfound   : string\nrequired: (MyType|null|number)\n"));
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration12
#[test]
fn test_forward_type_declaration12() {
    run(|t| {
        // We assume that {Function} types can produce anything, and don't
        // want to type-check them.
        t.test_closure_types("goog.forwardDeclare('MyType');\n/**\n * @param {!Function} ctor\n * @return {MyType}\n */\nfunction f(ctor) { return new ctor(); }\n", None);
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration13
#[test]
fn test_forward_type_declaration13() {
    run(|t| {
        // Some projects use {Function} registries to register constructors
        // that aren't in their binaries. We want to make sure we can pass these
        // around, but still do other checks on them.
        t.test_closure_types("goog.forwardDeclare('MyType');\n/**\n * @param {!Function} ctor\n * @return {MyType}\n */\nfunction f(ctor) { return (new ctor()).impossibleProp; }\n", Some(&*cat(&["Property impossibleProp never defined on ?", POSSIBLE_INEXISTENT_PROPERTY_EXPLANATION])));
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration2
#[test]
fn test_forward_type_declaration2() {
    run(|t| {
        let f = "goog.forwardDeclare('MyType');\n/** @param {MyType} x */ function f(x) { }\n";
        t.test_closure_types(f, None);
        t.test_closure_types(&cat(&[f, "f(3);"]), Some("actual parameter 1 of f does not match formal parameter\nfound   : number\nrequired: (MyType|null)\n"));
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration3
#[test]
fn test_forward_type_declaration3() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MyType');\n/** @param {MyType} x */ function f(x) { return x; }\n/** @constructor */ var MyType = function() {};\nf(3);\n", Some("actual parameter 1 of f does not match formal parameter\nfound   : number\nrequired: (MyType|null)\n"));
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration4
#[test]
fn test_forward_type_declaration4() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MyType');\n/** @param {MyType} x */ function f(x) { return x; }\n/** @constructor */ var MyType = function() {};\nf(new MyType());\n", None);
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration5
#[test]
fn test_forward_type_declaration5() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MyType');\n/**\n * @constructor\n * @extends {MyType}\n */ var YourType = function() {};\n/** @override */ YourType.prototype.method = function() {};\n", Some("Could not resolve type in @extends tag of YourType"));
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration6
#[test]
fn test_forward_type_declaration6() {
    run(|t| {
        t.test_closure_types_multiple_warnings("goog.forwardDeclare('MyType');\n/**\n * @constructor\n * @implements {MyType}\n */ var YourType = function() {};\n/** @override */ YourType.prototype.method = function() {};\n", Some(&["Could not resolve type in @implements tag of YourType", "property method not defined on any superclass of YourType"]));
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration7
#[test]
fn test_forward_type_declaration7() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MyType');\n/** @param {MyType=} x */\nfunction f(x) { return x == undefined; }\n", None);
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration8
#[test]
fn test_forward_type_declaration8() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MyType');\n/** @param {MyType} x */\nfunction f(x) { return x.name == undefined; }\n", None);
    });
}

// port: TypeCheckTest#testForwardTypeDeclaration9
#[test]
fn test_forward_type_declaration9() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MyType');\n/** @param {MyType} x */\nfunction f(x) { x.name = 'Bob'; }\n", None);
    });
}

// port: TypeCheckTest#testGoogBind1
#[test]
fn test_goog_bind1() {
    run(|t| {
        t.test_closure_types("goog.bind = function(var_args) {};\n/** @type {function(number): boolean} */\nfunction f(x, y) { return true; }\nf(goog.bind(f, null, 'x')());\n", Some("actual parameter 1 of f does not match formal parameter\nfound   : boolean\nrequired: number\n"));
    });
}

// port: TypeCheckTest#testGoogBind2
#[test]
fn test_goog_bind2() {
    run(|t| {
        // TODO(nicksantos): We do not currently type-check the arguments
        // of the goog.bind.
        t.test_closure_types("goog.bind = function(var_args) {};\n/** @type {function(boolean): boolean} */\nfunction f(x, y) { return true; }\nf(goog.bind(f, null, 'x')());\n", None);
    });
}

// port: TypeCheckTest#testImplementsExtendsLoop
#[test]
fn test_implements_extends_loop() {
    run(|t| {
        t.test_closure_types_multiple_warnings("/**\n * @constructor\n * @implements {F}\n */\nvar G = function() {};\n/**\n * @constructor\n * @extends {G}\n */\nvar F = function() {};\nalert((new F).foo);\n", Some(&["Cycle detected in inheritance chain of type F", "Property foo never defined on F"]));
    });
}

// port: TypeCheckTest#testImplementsLoop
#[test]
fn test_implements_loop() {
    run(|t| {
        t.test_closure_types_multiple_warnings(&"/** @constructor\n * @implements {T} */var T = function() {};\nSUPPRESSION\nalert((new T).foo);\n".replace("SUPPRESSION", &TypeCheckTestCase::suppress_missing_property_for("T", &["foo"])), Some(&["Cycle detected in inheritance chain of type T"]));
    });
}

// port: TypeCheckTest#testInheritanceCheck14
#[test]
fn test_inheritance_check14() {
    run(|t| {
        t.test_closure_types("/** @constructor\n @extends {goog.Missing} */\ngoog.Super = function() {};\n/** @constructor\n @extends {goog.Super} */function Sub() {};\n/** @override */ Sub.prototype.foo = function() {};\n", Some("Bad type annotation. Unknown type goog.Missing\nIt's possible that 'goog.Missing' refers to a value, not a type.\n"));
    });
}

// port: TypeCheckTest#testInnerFunction6NullishCoalesce
#[test]
fn test_inner_function6_nullish_coalesce() {
    run(|t| {
        t.test_closure_types("function f() {\n var x = null ?? function() {};\n function g() { if (goog.isFunction(x)) { x(1); } }\n g();\n}\n", Some("Function x: called with 1 argument(s). Function requires at least 0 argument(s) and no more than 0 argument(s).\n"));
    });
}

// port: TypeCheckTest#testInnerFunction7NullishCoalesce
#[test]
fn test_inner_function7_nullish_coalesce() {
    run(|t| {
        t.test_closure_types("function f() {\n /** @type {function()} */\n var x = null ?? function() {};\n function g() { if (goog.isFunction(x)) { x(1); } }\n g();\n}\n", Some("Function x: called with 1 argument(s). Function requires at least 0 argument(s) and no more than 0 argument(s).\n"));
    });
}

// port: TypeCheckTest#testInnerFunction8
#[test]
fn test_inner_function8() {
    run(|t| {
        t.test_closure_types("function f() {\n  function x() {};\n  function g() { if (goog.isFunction(x)) { x(1); } }\n  g();\n}\n", Some("Function x: called with 1 argument(s). Function requires at least 0 argument(s) and no more than 0 argument(s).\n"));
    });
}

// port: TypeCheckTest#testInterfaceExtendsLoop2
#[test]
fn test_interface_extends_loop2() {
    run(|t| {
        t.test_closure_types_multiple_warnings(&"/** @record\n * @extends {F} */var G = function() {};\n/** @record\n * @extends {G} */var F = function() {};\n/** @constructor\n * @implements {F} */var H = function() {};\nSUPPRESSION\nalert((new H).foo);\n".replace("SUPPRESSION", &TypeCheckTestCase::suppress_missing_property_for("H", &["foo"])), Some(&["Cycle detected in inheritance chain of type F", "Could not resolve type in @extends tag of G"]));
    });
}

// port: TypeCheckTest#testLends9
#[test]
fn test_lends9() {
    run(|t| {
        t.test_closure_types_multiple_warnings("function extend(x, y) {}\n/** @constructor */ function Foo() {}\nextend(Foo, /** @lends {!Foo} */ ({bar: 1}));\n", Some(&[&*cat(&["Bad type annotation. expected closing }", BAD_TYPE_WIKI_LINK]), &*cat(&["Bad type annotation. missing object name in @lends tag.", BAD_TYPE_WIKI_LINK])]));
    });
}

// port: TypeCheckTest#testMissingProperty24
#[test]
fn test_missing_property24() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MissingType');\n/** @param {MissingType} x */\nfunction f(x) { x.impossible(); }\n", None);
    });
}

// port: TypeCheckTest#testMissingProperty27
#[test]
fn test_missing_property27() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MissingType');\n/** @param {?MissingType} x */\nfunction f(x) {\n  for (var parent = x; parent; parent = parent.getParent()) {}\n}\n", None);
    });
}

// port: TypeCheckTest#testMissingProperty40a
#[test]
fn test_missing_property40a() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MissingType');\n/** @param {MissingType} x */\nfunction f(x) { x.impossible(); }\n", None);
    });
}

// port: TypeCheckTest#testMissingProperty40b
#[test]
fn test_missing_property40b() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('MissingType');\n/** @param {(Array|MissingType)} x */\nfunction f(x) { x.impossible(); }\n", Some("Property impossible not defined on all member types of x"));
    });
}

// port: TypeCheckTest#testNoResolvedTypeAndGreatestSubtypeInference
#[test]
fn test_no_resolved_type_and_greatest_subtype_inference() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('Foo');\n/**\n * @param {boolean} pred\n * @param {?Foo} x\n */\nfunction f(pred, x) {\n  var y;\n  if (pred) {\n    y = null;\n  } else {\n    y = x;\n  }\n  var /** number */ z = y;\n}\n", Some("initializing variable\nfound   : (Foo|null)\nrequired: number\n"));
    });
}

// port: TypeCheckTest#testNoResolvedTypeDoesntCauseInfiniteLoop
#[test]
fn test_no_resolved_type_doesnt_cause_infinite_loop() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('Foo');\ngoog.forwardDeclare('Bar');\n\n/** @interface */\nvar Baz = function() {};\n/** @return {!Bar} */\nBaz.prototype.getBar = function() {};\n/** @constructor */\nvar Qux = function() {\n  /** @type {?Foo} */\n  this.jobRuntimeTracker_ = null;\n};\n/** @param {!Baz} job */\nQux.prototype.runRenderJobs_ = function(job) {\n  for (var i = 0; i < 10; i++) {\n    if (this.jobRuntimeTracker_) {\n      goog.asserts.assert(job.getBar, '');\n    }\n  }\n};\n", None);
    });
}

// port: TypeCheckTest#testObjectLiteralDeclaration4
#[test]
fn test_object_literal_declaration4() {
    run(|t| {
        t.test_closure_types("var x = {\n  /** @param {boolean} x */ abc: function(x) {}\n};\n/**\n * @param {string} x\n * @suppress {duplicate}\n */ x.abc = function(x) {};\n", Some("assignment to property abc of x\nfound   : function(string): undefined\nrequired: function(boolean): undefined\n"));
        // TODO(user): suppress {duplicate} currently also silence the
        // redefining type error in the TypeValidator. Maybe it needs
        // a new suppress name instead?
    });
}

// port: TypeCheckTest#testPrototypeLoop
#[test]
fn test_prototype_loop() {
    run(|t| {
        t.test_closure_types_multiple_warnings(
            &cat(&[
                &*TypeCheckTestCase::suppress_missing_property(&["foo"]),
                "/** @constructor\n * @extends {T} */var T = function() {};\nalert((new T).foo);\n",
            ]),
            Some(&[
                "Cycle detected in inheritance chain of type T",
                "Could not resolve type in @extends tag of T",
            ]),
        );
    });
}

// port: TypeCheckTest#testQualifiedNameInference8
#[test]
fn test_qualified_name_inference8() {
    run(|t| {
        t.test_closure_types_multiple_warnings("var ns = {};\n(function() {\n  /** @constructor\n  * @param {number} x */\n  ns.Foo = function(x) {};\n})();\n/** @param {ns.Foo} x */ function f(x) {}\nf(new ns.Foo(true));\n", Some(&["Property Foo never defined on ns", "actual parameter 1 of ns.Foo does not match formal parameter\nfound   : boolean\nrequired: number"]));
    });
}

// port: TypeCheckTest#testRecordType4
#[test]
fn test_record_type4() {
    run(|t| {
        // Notice that we do not do flow-based inference on the object type:
        // We don't try to prove that x.prop may not be string until x
        // gets passed to g.
        t.test_closure_types_multiple_warnings("/** @param {{prop: (number|undefined)}} x */\nfunction f(x) {}\n/** @param {{prop: (string|undefined)}} x */\nfunction g(x) {}\nvar x = {};\nf(x);\ng(x);\n", Some(&["actual parameter 1 of f does not match formal parameter\nfound   : {prop: (number|string|undefined)}\nrequired: {prop: (number|undefined)}\nmissing : []\nmismatch: [prop]", "actual parameter 1 of g does not match formal parameter\nfound   : {prop: (number|string|undefined)}\nrequired: {prop: (string|undefined)}\nmissing : []\nmismatch: [prop]"]));
    });
}

// port: TypeCheckTest#testReflectObject1
#[test]
fn test_reflect_object1() {
    run(|t| {
        t.test_closure_types("goog.reflect = {};\ngoog.reflect.object = function(x, y){};\n/** @constructor */ function A() {}\ngoog.reflect.object(A, {x: 3});\n", None);
    });
}

// port: TypeCheckTest#testReflectObject2
#[test]
fn test_reflect_object2() {
    run(|t| {
        t.test_closure_types("goog.reflect = {};\ngoog.reflect.object = function(x, y){};\n/** @param {string} x */ function f(x) {}\n/** @constructor */ function A() {}\ngoog.reflect.object(A, {x: f(1 + 1)});\n", Some("actual parameter 1 of f does not match formal parameter\nfound   : number\nrequired: string\n"));
    });
}

// port: TypeCheckTest#testTemplateTypeWithUnresolvedType
#[test]
fn test_template_type_with_unresolved_type() {
    run(|t| {
        t.test_closure_types("goog.forwardDeclare('Color');\n/** @interface @template T */ function C() {}\n/** @return {!Color} */ C.prototype.method;\n/** @constructor @implements {C} */ function D() {}\n/** @override */ D.prototype.method = function() {};\n", None);
        // no warning expected.
    });
}

// port: TypeCheckTest#testTypeInferenceWithNoEntry2
#[test]
fn test_type_inference_with_no_entry2() {
    run(|t| {
        t.test_closure_types("/** @param {number} x */ function f(x) {}\n/** @param {!Object} x */ function g(x) {}\n/** @constructor */ function Foo() {}\nFoo.prototype.init = function() {\n  /** @type {?{baz: number}} */ this.bar = {baz: 3};\n};\n/**\n * @extends {Foo}\n * @constructor\n */\nfunction SubFoo() {}\n/** Method */\nSubFoo.prototype.method = function() {\n  for (var i = 0; i < 10; i++) {\n    f(this.bar);\n    goog.asserts.assert(this.bar);\n    g(this.bar);\n  }\n};\n", Some("actual parameter 1 of f does not match formal parameter\nfound   : (null|{baz: number})\nrequired: number\n"));
    });
}

// port: TypeCheckTest#testTypeOfReduction11
#[test]
fn test_type_of_reduction11() {
    run(|t| {
        t.test_closure_types("/** @param {Array|string} x\n@return {Array} */\nfunction f(x) {\nreturn goog.isObject(x) ? x : [];\n}\n", None);
    });
}

// port: TypeCheckTest#testTypeOfReduction13
#[test]
fn test_type_of_reduction13() {
    run(|t| {
        t.test_closure_types("/** @enum {string} */ var E = {A: 'a', B: 'b'};\n/** @param {E|Array} x\n@return {Array} */\nfunction f(x) { return goog.isObject(x) ? x : []; }\n", None);
    });
}

// port: TypeCheckTest#testTypeOfReduction15
#[test]
fn test_type_of_reduction15() {
    run(|t| {
        // Don't do type inference on GETELEMs.
        t.test_closure_types(
            "function f(x) {\n  return typeof arguments[0] == 'string' ? arguments[0] : 0;\n}\n",
            None,
        );
    });
}

// port: TypeCheckTest#testTypeOfReduction16
#[test]
fn test_type_of_reduction16() {
    run(|t| {
        t.test_closure_types("/** @interface */ function I() {}\n/**\n * @param {*} x\n * @return {I}\n */\nfunction f(x) {\n  if(goog.isObject(x)) {\n    return /** @type {I} */ (x);\n  }\n  return null;\n}\n", None);
    });
}

// port: TypeCheckTest#testTypeRedefinition
#[test]
fn test_type_redefinition() {
    run(|t| {
        t.test_closure_types_multiple_warnings("a={};/**@enum {string}*/ a.A = {ZOR:'b'};\n/** @constructor */ a.A = function() {}\n", Some(&["variable a.A redefined with type (typeof a.A), original definition at testcode:1 with type enum{a.A}", "assignment to property A of a\nfound   : (typeof a.A)\nrequired: enum{a.A}"]));
    });
}

// port: TypeCheckTest#testInitialTypingScope
#[test]
fn test_initial_typing_scope() {
    run(|t| {
        let s = {
            let mut compiler = t.compiler();
            let inner1 = compiler.new_node(Token::ROOT);
            let inner2 = compiler.new_node(Token::ROOT);
            let root = compiler.new_node(Token::ROOT);
            root.add_child_to_back(&mut compiler, inner1);
            root.add_child_to_back(&mut compiler, inner2);
            TypedScopeCreator::new_with_coding_convention(
                &mut compiler,
                CodingConventions::get_default(),
            )
            .create_initial_scope(&mut compiler, root)
        };

        for (native, name) in [
            (JSTypeNative::ARRAY_FUNCTION_TYPE, "Array"),
            (JSTypeNative::BOOLEAN_OBJECT_FUNCTION_TYPE, "Boolean"),
            (JSTypeNative::DATE_FUNCTION_TYPE, "Date"),
            (JSTypeNative::NUMBER_OBJECT_FUNCTION_TYPE, "Number"),
            (JSTypeNative::OBJECT_FUNCTION_TYPE, "Object"),
            (JSTypeNative::REGEXP_FUNCTION_TYPE, "RegExp"),
            (JSTypeNative::STRING_OBJECT_FUNCTION_TYPE, "String"),
        ] {
            t.assert_type_equals(t.native(native), t.var_type(s, name));
        }
    });
}

// port: TypeCheckTest#testBooleanNodeFalse
#[test]
fn test_boolean_node_false() {
    run(|t| {
        let false_node = IR::false_node(&mut t.compiler());
        let expr = IR::expr_result(&mut t.compiler(), false_node);
        t.type_check(expr);

        t.assert_type_equals(t.native(BOOLEAN_TYPE), t.jstype(false_node));
    });
}

// port: TypeCheckTest#testBooleanNodeTrue
#[test]
fn test_boolean_node_true() {
    run(|t| {
        let true_node = IR::true_node(&mut t.compiler());
        let expr = IR::expr_result(&mut t.compiler(), true_node);
        t.type_check(expr);

        t.assert_type_equals(t.native(BOOLEAN_TYPE), t.jstype(true_node));
    });
}

// port: TypeCheckTest#testNumberNode
#[test]
fn test_number_node() {
    run(|t| {
        let n = IR::number(&mut t.compiler(), 0.0);
        let expr = IR::expr_result(&mut t.compiler(), n);
        t.type_check(expr);

        t.assert_type_equals(t.native(NUMBER_TYPE), t.jstype(n));
    });
}

// port: TypeCheckTest#testStringNode
#[test]
fn test_string_node() {
    run(|t| {
        let n = IR::string(&mut t.compiler(), "hello");
        let expr = IR::expr_result(&mut t.compiler(), n);
        t.type_check(expr);

        t.assert_type_equals(t.native(STRING_TYPE), t.jstype(n));
    });
}

// port: TypeCheckTest#testTypeCheckStandaloneAST
#[test]
fn test_type_check_standalone_ast() {
    run(|t| {
        let externs = IR::root(&mut t.compiler(), &[]);
        let first_script = t.compiler().parse_test_code("function Foo() { }");
        t.type_check(first_script);
        let root = {
            let mut compiler = t.compiler();
            first_script.detach(&mut compiler);
            let js_root = IR::root(&mut compiler, &[first_script]);
            IR::root(&mut compiler, &[externs, js_root])
        };
        let mut compiler = t.compiler();
        let mut scope_creator = TypedScopeCreator::new(&mut compiler);
        let top_scope = scope_creator.create_scope(&mut compiler, root, None);

        let second_script = compiler.parse_test_code("new Foo");

        first_script.replace_with(&mut compiler, second_script);

        let interpreter = SemanticReverseAbstractInterpreter::new(compiler.get_type_registry());
        let second_parent = second_script.get_parent(&compiler).unwrap();
        TypeCheck::new_with_scope(
            &mut compiler,
            interpreter,
            Some(top_scope),
            Some(scope_creator),
        )
        .process(&mut compiler, externs, second_parent);

        assert_eq!(compiler.get_warning_count(), 1);
        assert_eq!(
            compiler.get_warnings()[0].description(),
            "cannot instantiate non-constructor, found type: function(): undefined"
        );
    });
}

// port: TypeCheckTest#testUndefinedNode
#[test]
fn test_undefined_node() {
    run(|t| {
        let (n, expr) = {
            let mut compiler = t.compiler();
            let p = compiler.new_node(Token::ADD);
            let n = compiler.new_string_with_token(Token::NAME, "undefined");
            p.add_child_to_back(&mut compiler, n);
            let five = compiler.new_number(5.0);
            p.add_child_to_back(&mut compiler, five);
            (n, IR::expr_result(&mut compiler, p))
        };
        t.type_check(expr);

        t.assert_type_equals(t.native(VOID_TYPE), t.jstype(n));
    });
}

// port: TypeCheckTest#testName1
#[test]
fn test_name1() {
    run(|t| {
        t.assert_type_equals(t.native(VOID_TYPE), t.test_name_node("undefined").unwrap());
    });
}

// port: TypeCheckTest#testName2
#[test]
fn test_name2() {
    run(|t| {
        t.assert_type_equals(
            t.native(OBJECT_FUNCTION_TYPE),
            t.test_name_node("Object").unwrap(),
        );
    });
}

// port: TypeCheckTest#testName3
#[test]
fn test_name3() {
    run(|t| {
        t.assert_type_equals(
            t.native(JSTypeNative::ARRAY_FUNCTION_TYPE),
            t.test_name_node("Array").unwrap(),
        );
    });
}

// port: TypeCheckTest#testName4
#[test]
fn test_name4() {
    run(|t| {
        t.assert_type_equals(
            t.native(JSTypeNative::DATE_FUNCTION_TYPE),
            t.test_name_node("Date").unwrap(),
        );
    });
}

// port: TypeCheckTest#testName5
#[test]
fn test_name5() {
    run(|t| {
        t.assert_type_equals(
            t.native(JSTypeNative::REGEXP_FUNCTION_TYPE),
            t.test_name_node("RegExp").unwrap(),
        );
    });
}
