/*
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/ClosureReverseAbstractInterpreterTest.java,
//   test/com/google/javascript/jscomp/CompilerTypeTestCase.java.

//! Port of ClosureReverseAbstractInterpreterTest (a CompilerTypeTestCase without JUnit records).
use closure_jscomp::{
    closure_reverse_abstract_interpreter::ClosureReverseAbstractInterpreter, compiler::Compiler,
    flow_scope::FlowScope, linked_flow_scope::LinkedFlowScope,
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
    typed_scope_creator::TypedScopeCreator,
};
use closure_jstype::{TypeId, js_type_native::JSTypeNative, testing::type_subject::TypeSubject};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{ir::IR, js_string::JsString, outcome::Outcome, token::Token};
use closure_testing::compiler_type_test_case::CompilerTypeTestCase;
use std::sync::Arc;

struct ClosureReverseAbstractInterpreterTest {
    compiler: Compiler,
}

impl ClosureReverseAbstractInterpreterTest {
    // port: CompilerTypeTestCase#setUp
    fn set_up() -> Self {
        // CompilerTypeTestCase#initializeNewCompiler
        let mut compiler = Compiler::new();
        compiler.init_options(CompilerTypeTestCase::default_options().unwrap());
        compiler.mark_feature_not_allowed(Feature::MODULES);
        compiler.get_type_registry();
        Self { compiler }
    }

    fn native(&mut self, native: JSTypeNative) -> TypeId {
        self.compiler.get_type_registry().get_native_type(native)
    }

    fn native_object(&mut self, native: JSTypeNative) -> TypeId {
        self.compiler
            .get_type_registry()
            .get_native_object_type(native)
    }

    // port: CompilerTypeTestCase#getNativeObjectType
    fn get_native_object_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::OBJECT_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeAllType
    fn get_native_all_type(&mut self) -> TypeId {
        self.native(JSTypeNative::ALL_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeNoObjectType
    fn get_native_no_object_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::NO_OBJECT_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeNumberStringBooleanType
    fn get_native_number_string_boolean_type(&mut self) -> TypeId {
        self.native(JSTypeNative::NUMBER_STRING_BOOLEAN)
    }

    // port: CompilerTypeTestCase#getNativeValueTypes
    fn get_native_value_types(&mut self) -> TypeId {
        self.native(JSTypeNative::VALUE_TYPES)
    }

    // port: CompilerTypeTestCase#getNativeNullType
    fn get_native_null_type(&mut self) -> TypeId {
        self.native(JSTypeNative::NULL_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeVoidType
    fn get_native_void_type(&mut self) -> TypeId {
        self.native(JSTypeNative::VOID_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeUnknownType
    fn get_native_unknown_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::UNKNOWN_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeCheckedUnknownType
    fn get_native_checked_unknown_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::CHECKED_UNKNOWN_TYPE)
    }

    // port: CompilerTypeTestCase#createUnionType
    fn create_union_type(&mut self, variants: &[TypeId]) -> TypeId {
        let (reg, ast) = self.compiler.get_type_registry_and_ast();
        reg.create_union_type(ast, variants)
    }

    fn slot_type(&mut self, scope: &Arc<dyn FlowScope>, name: &str) -> Option<TypeId> {
        let slot = scope
            .get_slot(&mut self.compiler, &JsString::from(name))
            .expect("NullPointerException");
        slot.get_type(&self.compiler)
    }

    // port: ClosureReverseAbstractInterpreterTest#testClosureFunction
    fn test_closure_function(
        &mut self,
        function: &str,
        type_: Option<TypeId>,
        true_type: TypeId,
        false_type: Option<TypeId>,
    ) {
        // function(a) where a : type
        let n = self
            .compiler
            .parse_test_code(format!("var a; {function}(a)").as_str());
        let call = n
            .get_last_child(&self.compiler)
            .unwrap()
            .get_last_child(&self.compiler)
            .unwrap();
        let name = call.get_last_child(&self.compiler).unwrap();

        let externs_root = IR::root(&mut self.compiler, &[]);
        let js_root = IR::root(&mut self.compiler, &[n]);
        let root = IR::root(&mut self.compiler, &[externs_root, js_root]);
        let scope =
            TypedScopeCreator::new(&mut self.compiler).create_scope(&mut self.compiler, root, None);
        let mut flow_scope: Arc<dyn FlowScope> = LinkedFlowScope::create_entry_lattice(scope);

        assert_eq!(call.get_token(&self.compiler), Token::CALL);
        assert_eq!(name.get_token(&self.compiler), Token::NAME);

        flow_scope = flow_scope.infer_slot_type(&mut self.compiler, &JsString::from("a"), type_);
        let rai = ClosureReverseAbstractInterpreter::new(self.compiler.get_type_registry());

        // trueScope
        let true_scope = rai.get_preciser_scope_knowing_condition_outcome(
            &mut self.compiler,
            call,
            Arc::clone(&flow_scope),
            Outcome::TRUE,
        );
        let a_true_type = self.slot_type(&true_scope, "a");
        {
            let (reg, ast) = self.compiler.get_type_registry_and_ast();
            TypeSubject::assert_type(a_true_type).is_equal_to(reg, ast, true_type);
        }

        // falseScope
        let false_scope = rai.get_preciser_scope_knowing_condition_outcome(
            &mut self.compiler,
            call,
            Arc::clone(&flow_scope),
            Outcome::FALSE,
        );
        let a_type = self.slot_type(&false_scope, "a");
        match false_type {
            None => assert_eq!(a_type, None),
            Some(false_type) => {
                let (reg, ast) = self.compiler.get_type_registry_and_ast();
                TypeSubject::assert_type(a_type).is_equal_to(reg, ast, false_type);
            }
        }
    }
}

// port: ClosureReverseAbstractInterpreterTest#testGoogIsObjectOnNull
#[test]
fn test_goog_is_object_on_null() {
    let mut t = ClosureReverseAbstractInterpreterTest::set_up();
    let object = t.get_native_object_type();
    t.test_closure_function("goog.isObject", None, object, None);
}

// port: ClosureReverseAbstractInterpreterTest#testGoogIsObject1
#[test]
fn test_goog_is_object1() {
    let mut t = ClosureReverseAbstractInterpreterTest::set_up();
    let all = t.get_native_all_type();
    let no_object = t.get_native_no_object_type();
    t.test_closure_function("goog.isObject", Some(all), no_object, Some(all));
}

// port: ClosureReverseAbstractInterpreterTest#testGoogIsObject2a
#[test]
fn test_goog_is_object2a() {
    let mut t = ClosureReverseAbstractInterpreterTest::set_up();
    let object = t.get_native_object_type();
    let number_string_boolean = t.get_native_number_string_boolean_type();
    let union = t.create_union_type(&[object, number_string_boolean]);
    t.test_closure_function(
        "goog.isObject",
        Some(union),
        object,
        Some(number_string_boolean),
    );
}

// port: ClosureReverseAbstractInterpreterTest#testGoogIsObject2b
#[test]
fn test_goog_is_object2b() {
    let mut t = ClosureReverseAbstractInterpreterTest::set_up();
    let object = t.get_native_object_type();
    let value_types = t.get_native_value_types();
    let union = t.create_union_type(&[object, value_types]);
    t.test_closure_function("goog.isObject", Some(union), object, Some(value_types));
}

// port: ClosureReverseAbstractInterpreterTest#testGoogIsObject3a
#[test]
fn test_goog_is_object3a() {
    let mut t = ClosureReverseAbstractInterpreterTest::set_up();
    let object = t.get_native_object_type();
    let number_string_boolean = t.get_native_number_string_boolean_type();
    let null = t.get_native_null_type();
    let void = t.get_native_void_type();
    let union = t.create_union_type(&[object, number_string_boolean, null, void]);
    let false_type = t.create_union_type(&[number_string_boolean, null, void]);
    t.test_closure_function("goog.isObject", Some(union), object, Some(false_type));
}

// port: ClosureReverseAbstractInterpreterTest#testGoogIsObject3b
#[test]
fn test_goog_is_object3b() {
    let mut t = ClosureReverseAbstractInterpreterTest::set_up();
    let object = t.get_native_object_type();
    let value_types = t.get_native_value_types();
    let null = t.get_native_null_type();
    let void = t.get_native_void_type();
    let union = t.create_union_type(&[object, value_types, null, void]);
    let false_type = t.create_union_type(&[value_types, null, void]);
    t.test_closure_function("goog.isObject", Some(union), object, Some(false_type));
}

// port: ClosureReverseAbstractInterpreterTest#testGoogIsObject4
#[test]
fn test_goog_is_object4() {
    let mut t = ClosureReverseAbstractInterpreterTest::set_up();
    let unknown = t.get_native_unknown_type();
    let no_object = t.get_native_no_object_type(); // ? Should this be CHECKED_UNKNOWN?
    let checked_unknown = t.get_native_checked_unknown_type();
    t.test_closure_function(
        "goog.isObject",
        Some(unknown),
        no_object,
        Some(checked_unknown),
    );
}
