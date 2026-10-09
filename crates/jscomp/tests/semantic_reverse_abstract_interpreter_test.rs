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
//   test/com/google/javascript/jscomp/CompilerTypeTestCase.java,
//   test/com/google/javascript/jscomp/SemanticReverseAbstractInterpreterTest.java.

//! Port of SemanticReverseAbstractInterpreterTest (a CompilerTypeTestCase without JUnit records).
use closure_jscomp::{
    compiler::Compiler, flow_scope::FlowScope, linked_flow_scope::LinkedFlowScope,
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
    semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter,
    typed_scope::TypedScope,
};
use closure_jstype::{TypeId, js_type_native::JSTypeNative, testing::type_subject::TypeSubject};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{js_string::JsString, node::NodeId, outcome::Outcome, token::Token};
use closure_testing::compiler_type_test_case::CompilerTypeTestCase;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Arc;

type Blind = [Arc<dyn FlowScope>; 1];

struct SemanticReverseAbstractInterpreterTest {
    compiler: Compiler,
    interpreter: Arc<dyn ReverseAbstractInterpreter>,
    function_scope: Option<TypedScope>,
}

/// Java's `TypedName` (name, type).
// port: SemanticReverseAbstractInterpreterTest.TypedName#TypedName
type TypedName = (&'static str, TypeId);

impl SemanticReverseAbstractInterpreterTest {
    // port: SemanticReverseAbstractInterpreterTest#setUp
    fn set_up() -> Self {
        // super.setUp(): CompilerTypeTestCase#initializeNewCompiler
        let mut compiler = Compiler::new();
        compiler.init_options(CompilerTypeTestCase::default_options().unwrap());
        compiler.mark_feature_not_allowed(Feature::MODULES);
        let interpreter: Arc<dyn ReverseAbstractInterpreter> =
            SemanticReverseAbstractInterpreter::new(compiler.get_type_registry());
        Self {
            compiler,
            interpreter,
            function_scope: None,
        }
    }

    // port: SemanticReverseAbstractInterpreterTest#newScope
    fn new_scope(&mut self) -> Blind {
        let root = self.compiler.new_node(Token::ROOT);
        let global_scope = TypedScope::create_global_scope(&mut self.compiler, root);
        let function = self.compiler.new_node(Token::FUNCTION);
        let function_scope = TypedScope::new(&mut self.compiler, global_scope, function);
        self.function_scope = Some(function_scope);
        [LinkedFlowScope::create_entry_lattice(function_scope)]
    }

    fn native(&mut self, native: JSTypeNative) -> TypeId {
        self.compiler.get_type_registry().get_native_type(native)
    }

    fn native_object(&mut self, native: JSTypeNative) -> TypeId {
        self.compiler
            .get_type_registry()
            .get_native_object_type(native)
    }

    // port: CompilerTypeTestCase#getNativeStringType
    fn get_native_string_type(&mut self) -> TypeId {
        self.native(JSTypeNative::STRING_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeNumberType
    fn get_native_number_type(&mut self) -> TypeId {
        self.native(JSTypeNative::NUMBER_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeBooleanType
    fn get_native_boolean_type(&mut self) -> TypeId {
        self.native(JSTypeNative::BOOLEAN_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeVoidType
    fn get_native_void_type(&mut self) -> TypeId {
        self.native(JSTypeNative::VOID_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeNullType
    fn get_native_null_type(&mut self) -> TypeId {
        self.native(JSTypeNative::NULL_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeNoType
    fn get_native_no_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::NO_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeAllType
    fn get_native_all_type(&mut self) -> TypeId {
        self.native(JSTypeNative::ALL_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeObjectType
    fn get_native_object_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::OBJECT_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeUnknownType
    fn get_native_unknown_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::UNKNOWN_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeFunctionType
    fn get_native_function_type(&mut self) -> TypeId {
        self.native(JSTypeNative::FUNCTION_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeStringObjectType
    fn get_native_string_object_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::STRING_OBJECT_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeNumberObjectType
    fn get_native_number_object_type(&mut self) -> TypeId {
        self.native_object(JSTypeNative::NUMBER_OBJECT_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeStringObjectConstructorType
    fn get_native_string_object_constructor_type(&mut self) -> TypeId {
        self.native(JSTypeNative::STRING_OBJECT_FUNCTION_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeNumberStringBooleanType
    fn get_native_number_string_boolean_type(&mut self) -> TypeId {
        self.native(JSTypeNative::NUMBER_STRING_BOOLEAN)
    }

    // port: CompilerTypeTestCase#getNativeObjectNumberStringBooleanType
    fn get_native_object_number_string_boolean_type(&mut self) -> TypeId {
        let variants = [
            self.native(JSTypeNative::OBJECT_TYPE),
            self.native(JSTypeNative::NUMBER_TYPE),
            self.native(JSTypeNative::STRING_TYPE),
            self.native(JSTypeNative::BOOLEAN_TYPE),
        ];
        self.create_union_type(&variants)
    }

    // port: CompilerTypeTestCase#createUnionType
    fn create_union_type(&mut self, variants: &[TypeId]) -> TypeId {
        let (reg, ast) = self.compiler.get_type_registry_and_ast();
        reg.create_union_type(ast, variants)
    }

    // port: CompilerTypeTestCase#createNullableType
    fn create_nullable_type(&mut self, type_: TypeId) -> TypeId {
        let (reg, ast) = self.compiler.get_type_registry_and_ast();
        reg.create_nullable_type(ast, type_)
    }

    // port: CompilerTypeTestCase#assertTypeEquals(JSType,JSType)
    fn assert_type_equals(&mut self, a: TypeId, b: Option<TypeId>) {
        let (reg, ast) = self.compiler.get_type_registry_and_ast();
        TypeSubject::assert_type(b).is_equal_to(reg, ast, a);
    }

    // port: CompilerTypeTestCase#assertTypeEquals(String,JSType,JSType)
    fn assert_type_equals_message(&mut self, msg: &str, a: TypeId, b: Option<TypeId>) {
        // assertWithMessage(msg): the failure message starts with msg.
        let result = catch_unwind(AssertUnwindSafe(|| self.assert_type_equals(a, b)));
        if let Err(payload) = result {
            let detail = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()));
            match detail {
                Some(detail) => panic!("{msg}\n{detail}"),
                None => resume_unwind(payload),
            }
        }
    }

    fn get_preciser(
        &mut self,
        condition: NodeId,
        blind: &Blind,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope> {
        let interpreter = Arc::clone(&self.interpreter);
        interpreter.get_preciser_scope_knowing_condition_outcome(
            &mut self.compiler,
            condition,
            Arc::clone(&blind[0]),
            outcome,
        )
    }

    // port: SemanticReverseAbstractInterpreterTest#testBinop
    fn test_binop(
        &mut self,
        blind: &Blind,
        binop: Token,
        left: NodeId,
        right: NodeId,
        true_outcome: &[TypedName],
        false_outcome: &[TypedName],
    ) {
        let condition = self.compiler.new_node(binop);
        condition.add_child_to_back(&mut self.compiler, left);
        condition.add_child_to_back(&mut self.compiler, right);

        // true outcome.
        let informed_true = self.get_preciser(condition, blind, Outcome::TRUE);
        for &(name, type_) in true_outcome {
            let actual = self.get_var_type(&informed_true, name);
            self.assert_type_equals_message(name, type_, actual);
        }

        // false outcome.
        let informed_false = self.get_preciser(condition, blind, Outcome::FALSE);
        for &(name, type_) in false_outcome {
            let actual = self.get_var_type(&informed_false, name);
            self.assert_type_equals(type_, actual);
        }
    }

    // port: SemanticReverseAbstractInterpreterTest#createNull
    fn create_null(&mut self) -> NodeId {
        let n = self.compiler.new_node(Token::NULL);
        let null_type = self.get_native_null_type();
        n.set_jstype(&mut self.compiler, Some(null_type));
        n
    }

    // port: SemanticReverseAbstractInterpreterTest#createNumber
    fn create_number(&mut self, n: i32) -> NodeId {
        let number = self.create_untyped_number(n);
        let number_type = self.get_native_number_type();
        number.set_jstype(&mut self.compiler, Some(number_type));
        number
    }

    // port: SemanticReverseAbstractInterpreterTest#createUntypedNumber
    fn create_untyped_number(&mut self, n: i32) -> NodeId {
        self.compiler.new_number(f64::from(n))
    }

    // port: SemanticReverseAbstractInterpreterTest#getVarType
    fn get_var_type(&mut self, scope: &Arc<dyn FlowScope>, name: &str) -> Option<TypeId> {
        let slot = scope
            .get_slot(&mut self.compiler, &JsString::from(name))
            .expect("NullPointerException");
        slot.get_type(&self.compiler)
    }

    // port: SemanticReverseAbstractInterpreterTest#createVar
    fn create_var(&mut self, scope: &mut Blind, name: &str, type_: TypeId) -> NodeId {
        let n = self.compiler.new_string_with_token(Token::NAME, name);
        self.function_scope.expect("NullPointerException").declare(
            &mut self.compiler,
            name,
            Some(n),
            None,
            None,
            true,
        );
        scope[0] = Arc::clone(&scope[0]).infer_slot_type(
            &mut self.compiler,
            &JsString::from(name),
            Some(type_),
        );
        n.set_jstype(&mut self.compiler, Some(type_));
        n
    }
}

/// Tests reverse interpretation of a NAME expression.
// port: SemanticReverseAbstractInterpreterTest#testNameCondition
#[test]
fn test_name_condition() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let nullable_string = t.create_nullable_type(string);
    let condition = t.create_var(&mut blind, "a", nullable_string);

    // true outcome.
    let informed_true = t.get_preciser(condition, &blind, Outcome::TRUE);
    let actual = t.get_var_type(&informed_true, "a");
    t.assert_type_equals(string, actual);

    // false outcome.
    let informed_false = t.get_preciser(condition, &blind, Outcome::FALSE);
    let actual = t.get_var_type(&informed_false, "a");
    let expected = t.create_nullable_type(string);
    t.assert_type_equals(expected, actual);
}

/// Tests reverse interpretation of a NOT(NAME) expression.
// port: SemanticReverseAbstractInterpreterTest#testNegatedNameCondition
#[test]
fn test_negated_name_condition() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let nullable_string = t.create_nullable_type(string);
    let a = t.create_var(&mut blind, "a", nullable_string);
    let condition = t.compiler.new_node(Token::NOT);
    condition.add_child_to_back(&mut t.compiler, a);

    // true outcome.
    let informed_true = t.get_preciser(condition, &blind, Outcome::TRUE);
    let actual = t.get_var_type(&informed_true, "a");
    let expected = t.create_nullable_type(string);
    t.assert_type_equals(expected, actual);

    // false outcome.
    let informed_false = t.get_preciser(condition, &blind, Outcome::FALSE);
    let actual = t.get_var_type(&informed_false, "a");
    t.assert_type_equals(string, actual);
}

/// Tests reverse interpretation of a ASSIGN expression.
// port: SemanticReverseAbstractInterpreterTest#testAssignCondition1
#[test]
fn test_assign_condition1() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let object = t.get_native_object_type();
    let null = t.get_native_null_type();
    let nullable_object = t.create_nullable_type(object);
    let left = t.create_var(&mut blind, "a", nullable_object);
    let nullable_object = t.create_nullable_type(object);
    let right = t.create_var(&mut blind, "b", nullable_object);
    t.test_binop(
        &blind,
        Token::ASSIGN,
        left,
        right,
        &[("a", object), ("b", object)],
        &[("a", null), ("b", null)],
    );
}

/// Tests reverse interpretation of a SHEQ(NAME, NUMBER) expression.
// port: SemanticReverseAbstractInterpreterTest#testSheqCondition1
#[test]
fn test_sheq_condition1() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let string_number = t.create_union_type(&[string, number]);
    let left = t.create_var(&mut blind, "a", string_number);
    let right = t.create_number(56);
    let string_number = t.create_union_type(&[string, number]);
    t.test_binop(
        &blind,
        Token::SHEQ,
        left,
        right,
        &[("a", number)],
        &[("a", string_number)],
    );
}

/// Tests reverse interpretation of a SHEQ(NUMBER, NAME) expression.
// port: SemanticReverseAbstractInterpreterTest#testSheqCondition2
#[test]
fn test_sheq_condition2() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let left = t.create_number(56);
    let string_number = t.create_union_type(&[string, number]);
    let right = t.create_var(&mut blind, "a", string_number);
    let string_number = t.create_union_type(&[string, number]);
    t.test_binop(
        &blind,
        Token::SHEQ,
        left,
        right,
        &[("a", number)],
        &[("a", string_number)],
    );
}

/// Tests reverse interpretation of a SHEQ(NAME, NAME) expression.
// port: SemanticReverseAbstractInterpreterTest#testSheqCondition3
#[test]
fn test_sheq_condition3() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let boolean = t.get_native_boolean_type();
    let string_boolean = t.create_union_type(&[string, boolean]);
    let left = t.create_var(&mut blind, "b", string_boolean);
    let string_number = t.create_union_type(&[string, number]);
    let right = t.create_var(&mut blind, "a", string_number);
    let string_number = t.create_union_type(&[string, number]);
    let string_boolean = t.create_union_type(&[string, boolean]);
    t.test_binop(
        &blind,
        Token::SHEQ,
        left,
        right,
        &[("a", string), ("b", string)],
        &[("a", string_number), ("b", string_boolean)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testSheqCondition4
#[test]
fn test_sheq_condition4() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let void = t.get_native_void_type();
    let string_void = t.create_union_type(&[string, void]);
    let left = t.create_var(&mut blind, "a", string_void);
    let void_union = t.create_union_type(&[void]);
    let right = t.create_var(&mut blind, "b", void_union);
    t.test_binop(
        &blind,
        Token::SHEQ,
        left,
        right,
        &[("a", void), ("b", void)],
        &[("a", string), ("b", void)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testSheqCondition5
#[test]
fn test_sheq_condition5() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let null = t.get_native_null_type();
    let void = t.get_native_void_type();
    let null_void = t.create_union_type(&[null, void]);
    let left = t.create_var(&mut blind, "a", null_void);
    let void_union = t.create_union_type(&[void]);
    let right = t.create_var(&mut blind, "b", void_union);
    t.test_binop(
        &blind,
        Token::SHEQ,
        left,
        right,
        &[("a", void), ("b", void)],
        &[("a", null), ("b", void)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testSheqCondition6
#[test]
fn test_sheq_condition6() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let void = t.get_native_void_type();
    let string_void = t.create_union_type(&[string, void]);
    let left = t.create_var(&mut blind, "a", string_void);
    let number_void = t.create_union_type(&[number, void]);
    let right = t.create_var(&mut blind, "b", number_void);
    let string_void = t.create_union_type(&[string, void]);
    let number_void = t.create_union_type(&[number, void]);
    t.test_binop(
        &blind,
        Token::SHEQ,
        left,
        right,
        &[("a", void), ("b", void)],
        &[("a", string_void), ("b", number_void)],
    );
}

/// Tests reverse interpretation of a SHNE(NAME, NUMBER) expression.
// port: SemanticReverseAbstractInterpreterTest#testShneCondition1
#[test]
fn test_shne_condition1() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let string_number = t.create_union_type(&[string, number]);
    let left = t.create_var(&mut blind, "a", string_number);
    let right = t.create_number(56);
    let string_number = t.create_union_type(&[string, number]);
    t.test_binop(
        &blind,
        Token::SHNE,
        left,
        right,
        &[("a", string_number)],
        &[("a", number)],
    );
}

/// Tests reverse interpretation of a SHNE(NUMBER, NAME) expression.
// port: SemanticReverseAbstractInterpreterTest#testShneCondition2
#[test]
fn test_shne_condition2() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let left = t.create_number(56);
    let string_number = t.create_union_type(&[string, number]);
    let right = t.create_var(&mut blind, "a", string_number);
    let string_number = t.create_union_type(&[string, number]);
    t.test_binop(
        &blind,
        Token::SHNE,
        left,
        right,
        &[("a", string_number)],
        &[("a", number)],
    );
}

/// Tests reverse interpretation of a SHNE(NAME, NAME) expression.
// port: SemanticReverseAbstractInterpreterTest#testShneCondition3
#[test]
fn test_shne_condition3() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let boolean = t.get_native_boolean_type();
    let string_boolean = t.create_union_type(&[string, boolean]);
    let left = t.create_var(&mut blind, "b", string_boolean);
    let string_number = t.create_union_type(&[string, number]);
    let right = t.create_var(&mut blind, "a", string_number);
    let string_number = t.create_union_type(&[string, number]);
    let string_boolean = t.create_union_type(&[string, boolean]);
    t.test_binop(
        &blind,
        Token::SHNE,
        left,
        right,
        &[("a", string_number), ("b", string_boolean)],
        &[("a", string), ("b", string)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testShneCondition4
#[test]
fn test_shne_condition4() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let void = t.get_native_void_type();
    let string_void = t.create_union_type(&[string, void]);
    let left = t.create_var(&mut blind, "a", string_void);
    let void_union = t.create_union_type(&[void]);
    let right = t.create_var(&mut blind, "b", void_union);
    t.test_binop(
        &blind,
        Token::SHNE,
        left,
        right,
        &[("a", string), ("b", void)],
        &[("a", void), ("b", void)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testShneCondition5
#[test]
fn test_shne_condition5() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let null = t.get_native_null_type();
    let void = t.get_native_void_type();
    let null_void = t.create_union_type(&[null, void]);
    let left = t.create_var(&mut blind, "a", null_void);
    let null_union = t.create_union_type(&[null]);
    let right = t.create_var(&mut blind, "b", null_union);
    t.test_binop(
        &blind,
        Token::SHNE,
        left,
        right,
        &[("a", void), ("b", null)],
        &[("a", null), ("b", null)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testShneCondition6
#[test]
fn test_shne_condition6() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let void = t.get_native_void_type();
    let string_void = t.create_union_type(&[string, void]);
    let left = t.create_var(&mut blind, "a", string_void);
    let number_void = t.create_union_type(&[number, void]);
    let right = t.create_var(&mut blind, "b", number_void);
    let string_void = t.create_union_type(&[string, void]);
    let number_void = t.create_union_type(&[number, void]);
    t.test_binop(
        &blind,
        Token::SHNE,
        left,
        right,
        &[("a", string_void), ("b", number_void)],
        &[("a", void), ("b", void)],
    );
}

/// Tests reverse interpretation of a EQ(NAME, NULL) expression.
// port: SemanticReverseAbstractInterpreterTest#testEqCondition1
#[test]
fn test_eq_condition1() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let boolean = t.get_native_boolean_type();
    let void = t.get_native_void_type();
    let boolean_void = t.create_union_type(&[boolean, void]);
    let left = t.create_var(&mut blind, "a", boolean_void);
    let right = t.create_null();
    t.test_binop(
        &blind,
        Token::EQ,
        left,
        right,
        &[("a", void)],
        &[("a", boolean)],
    );
}

/// Tests reverse interpretation of a NE(NULL, NAME) expression.
// port: SemanticReverseAbstractInterpreterTest#testEqCondition2
#[test]
fn test_eq_condition2() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let boolean = t.get_native_boolean_type();
    let void = t.get_native_void_type();
    let left = t.create_null();
    let boolean_void = t.create_union_type(&[boolean, void]);
    let right = t.create_var(&mut blind, "a", boolean_void);
    t.test_binop(
        &blind,
        Token::NE,
        left,
        right,
        &[("a", boolean)],
        &[("a", void)],
    );
}

/// Tests reverse interpretation of a EQ(NAME, NULL) expression.
// port: SemanticReverseAbstractInterpreterTest#testEqCondition3
#[test]
fn test_eq_condition3() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let null = t.get_native_null_type();
    let void = t.get_native_void_type();
    let number = t.get_native_number_type();
    // (number,undefined,null)
    let nullable_optional_number = t.create_union_type(&[null, void, number]);
    // (null,undefined)
    let null_undefined = t.create_union_type(&[void, null]);
    let left = t.create_var(&mut blind, "a", nullable_optional_number);
    let right = t.create_null();
    t.test_binop(
        &blind,
        Token::EQ,
        left,
        right,
        &[("a", null_undefined)],
        &[("a", number)],
    );
}

/// Tests reverse interpretation of two undefineds.
// port: SemanticReverseAbstractInterpreterTest#testEqCondition4
#[test]
fn test_eq_condition4() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let void = t.get_native_void_type();
    let no_type = t.get_native_no_type();
    let left = t.create_var(&mut blind, "a", void);
    let right = t.create_var(&mut blind, "b", void);
    t.test_binop(
        &blind,
        Token::EQ,
        left,
        right,
        &[("a", void), ("b", void)],
        &[("a", no_type), ("b", no_type)],
    );
}

/// Tests reverse interpretation of a COMPARE(NAME, NUMBER) expression, where COMPARE can be LE,
/// LT, GE or GT.
// port: SemanticReverseAbstractInterpreterTest#testInequalitiesCondition1
#[test]
fn test_inequalities_condition1() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    for op in [Token::LT, Token::GT, Token::LE, Token::GE] {
        let mut blind = t.new_scope();
        let string = t.get_native_string_type();
        let void = t.get_native_void_type();
        let string_void = t.create_union_type(&[string, void]);
        let left = t.create_var(&mut blind, "a", string_void);
        let right = t.create_number(8);
        let string_void = t.create_union_type(&[string, void]);
        t.test_binop(
            &blind,
            op,
            left,
            right,
            &[("a", string)],
            &[("a", string_void)],
        );
    }
}

/// Tests reverse interpretation of a COMPARE(NAME, NAME) expression, where COMPARE can be LE, LT,
/// GE or GT.
// port: SemanticReverseAbstractInterpreterTest#testInequalitiesCondition2
#[test]
fn test_inequalities_condition2() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    for op in [Token::LT, Token::GT, Token::LE, Token::GE] {
        let mut blind = t.new_scope();
        let string = t.get_native_string_type();
        let number = t.get_native_number_type();
        let void = t.get_native_void_type();
        let null = t.get_native_null_type();
        let string_number_void = t.create_union_type(&[string, number, void]);
        let left = t.create_var(&mut blind, "a", string_number_void);
        let number_null = t.create_union_type(&[number, null]);
        let right = t.create_var(&mut blind, "b", number_null);
        let string_number = t.create_union_type(&[string, number]);
        let number_null_true = t.create_union_type(&[number, null]);
        let string_number_void = t.create_union_type(&[string, number, void]);
        let number_null_false = t.create_union_type(&[number, null]);
        t.test_binop(
            &blind,
            op,
            left,
            right,
            &[("a", string_number), ("b", number_null_true)],
            &[("a", string_number_void), ("b", number_null_false)],
        );
    }
}

/// Tests reverse interpretation of a COMPARE(NUMBER-untyped, NAME) expression, where COMPARE can
/// be LE, LT, GE or GT.
// port: SemanticReverseAbstractInterpreterTest#testInequalitiesCondition3
#[test]
fn test_inequalities_condition3() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    for op in [Token::LT, Token::GT, Token::LE, Token::GE] {
        let mut blind = t.new_scope();
        let string = t.get_native_string_type();
        let void = t.get_native_void_type();
        let left = t.create_untyped_number(8);
        let string_void = t.create_union_type(&[string, void]);
        let right = t.create_var(&mut blind, "a", string_void);
        let string_void = t.create_union_type(&[string, void]);
        t.test_binop(
            &blind,
            op,
            left,
            right,
            &[("a", string)],
            &[("a", string_void)],
        );
    }
}

// port: SemanticReverseAbstractInterpreterTest#testAnd
#[test]
fn test_and() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let null = t.get_native_null_type();
    let void = t.get_native_void_type();
    let string_null = t.create_union_type(&[string, null]);
    let left = t.create_var(&mut blind, "b", string_null);
    let number_void = t.create_union_type(&[number, void]);
    let right = t.create_var(&mut blind, "a", number_void);
    let number_void = t.create_union_type(&[number, void]);
    let string_null = t.create_union_type(&[string, null]);
    t.test_binop(
        &blind,
        Token::AND,
        left,
        right,
        &[("a", number), ("b", string)],
        &[("a", number_void), ("b", string_null)],
    );
}

/// `new Node(Token.TYPEOF, createVar(blind, name, type))`.
fn typeof_var(
    t: &mut SemanticReverseAbstractInterpreterTest,
    blind: &mut Blind,
    name: &str,
    type_: TypeId,
) -> NodeId {
    let var = t.create_var(blind, name, type_);
    t.compiler.new_node_with_child(Token::TYPEOF, var)
}

// port: SemanticReverseAbstractInterpreterTest#testTypeof1
#[test]
fn test_typeof1() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let object = t.get_native_object_type();
    let function = t.get_native_function_type();
    let left = typeof_var(&mut t, &mut blind, "a", object);
    let right = t.compiler.new_string("function");
    t.test_binop(
        &blind,
        Token::EQ,
        left,
        right,
        &[("a", function)],
        &[("a", object)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testTypeof2
#[test]
fn test_typeof2() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let all = t.get_native_all_type();
    let function = t.get_native_function_type();
    let left = typeof_var(&mut t, &mut blind, "a", all);
    let right = t.compiler.new_string("function");
    t.test_binop(
        &blind,
        Token::EQ,
        left,
        right,
        &[("a", function)],
        &[("a", all)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testTypeof3
#[test]
fn test_typeof3() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let object_number_string_boolean = t.get_native_object_number_string_boolean_type();
    let left = typeof_var(&mut t, &mut blind, "a", object_number_string_boolean);
    let right = t.compiler.new_string("function");
    let function = t.get_native_function_type();
    let object_number_string_boolean = t.get_native_object_number_string_boolean_type();
    t.test_binop(
        &blind,
        Token::EQ,
        left,
        right,
        &[("a", function)],
        &[("a", object_number_string_boolean)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testTypeof4
#[test]
fn test_typeof4() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let function = t.get_native_function_type();
    let number_string_boolean = t.get_native_number_string_boolean_type();
    let function_number_string_boolean = t.create_union_type(&[function, number_string_boolean]);
    let left = typeof_var(&mut t, &mut blind, "a", function_number_string_boolean);
    let right = t.compiler.new_string("function");
    t.test_binop(
        &blind,
        Token::EQ,
        left,
        right,
        &[("a", function)],
        &[("a", number_string_boolean)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testInstanceOf
#[test]
fn test_instance_of() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let unknown = t.get_native_unknown_type();
    let string_object = t.get_native_string_object_type();
    let string_object_constructor = t.get_native_string_object_constructor_type();
    let left = t.create_var(&mut blind, "x", unknown);
    let right = t.create_var(&mut blind, "s", string_object_constructor);
    t.test_binop(
        &blind,
        Token::INSTANCEOF,
        left,
        right,
        &[("x", string_object), ("s", string_object_constructor)],
        &[("s", string_object_constructor)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testInstanceOf2
#[test]
fn test_instance_of2() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let string_object = t.get_native_string_object_type();
    let number_object = t.get_native_number_object_type();
    let string_object_constructor = t.get_native_string_object_constructor_type();
    let string_number_object = t.create_union_type(&[string_object, number_object]);
    let left = t.create_var(&mut blind, "x", string_number_object);
    let right = t.create_var(&mut blind, "s", string_object_constructor);
    t.test_binop(
        &blind,
        Token::INSTANCEOF,
        left,
        right,
        &[("x", string_object), ("s", string_object_constructor)],
        &[("x", number_object), ("s", string_object_constructor)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testInstanceOf3
#[test]
fn test_instance_of3() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let object = t.get_native_object_type();
    let string_object = t.get_native_string_object_type();
    let string_object_constructor = t.get_native_string_object_constructor_type();
    let left = t.create_var(&mut blind, "x", object);
    let right = t.create_var(&mut blind, "s", string_object_constructor);
    t.test_binop(
        &blind,
        Token::INSTANCEOF,
        left,
        right,
        &[("x", string_object), ("s", string_object_constructor)],
        &[("x", object), ("s", string_object_constructor)],
    );
}

// port: SemanticReverseAbstractInterpreterTest#testInstanceOf4
#[test]
fn test_instance_of4() {
    let mut t = SemanticReverseAbstractInterpreterTest::set_up();
    let mut blind = t.new_scope();
    let all = t.get_native_all_type();
    let string_object = t.get_native_string_object_type();
    let string_object_constructor = t.get_native_string_object_constructor_type();
    let left = t.create_var(&mut blind, "x", all);
    let right = t.create_var(&mut blind, "s", string_object_constructor);
    t.test_binop(
        &blind,
        Token::INSTANCEOF,
        left,
        right,
        &[("x", string_object), ("s", string_object_constructor)],
        &[("s", string_object_constructor)],
    );
}
