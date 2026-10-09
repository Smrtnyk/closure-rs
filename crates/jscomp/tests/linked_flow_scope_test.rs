/*
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
//   test/com/google/javascript/jscomp/LinkedFlowScopeTest.java.

//! Port of LinkedFlowScopeTest (a CompilerTypeTestCase without JUnit records).
use closure_jscomp::{
    compiler::Compiler,
    flow_scope::FlowScope,
    linked_flow_scope::{FlowScopeJoinOp, LinkedFlowScope},
    typed_scope::TypedScope,
};
use closure_jstype::{TypeId, js_type_native::JSTypeNative, testing::type_subject::TypeSubject};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};
use closure_testing::compiler_type_test_case::CompilerTypeTestCase;
use std::sync::Arc;

// port: LinkedFlowScopeTest#LONG_CHAIN_LENGTH
const LONG_CHAIN_LENGTH: i32 = 1050;

struct LinkedFlowScopeTest {
    compiler: Compiler,
    local_scope: TypedScope,
    #[allow(dead_code)] // Java: @SuppressWarnings("unused")
    global_entry: Arc<dyn FlowScope>,
    local_entry: Arc<dyn FlowScope>,
}

impl LinkedFlowScopeTest {
    // port: LinkedFlowScopeTest#setUp
    fn set_up() -> Self {
        // super.setUp(): CompilerTypeTestCase#initializeNewCompiler
        let mut compiler = Compiler::new();
        compiler.init_options(CompilerTypeTestCase::default_options().unwrap());
        compiler.mark_feature_not_allowed(Feature::MODULES);
        compiler.get_type_registry();

        // Fields: functionNode = new Node(FUNCTION); rootNode = new Node(ROOT, functionNode)
        let function_node: NodeId = compiler.new_node(Token::FUNCTION);
        let root_node = compiler.new_node_with_child(Token::ROOT, function_node);

        let global_scope = TypedScope::create_global_scope(&mut compiler, root_node);
        global_scope.declare(&mut compiler, "globalA", None, None, None, true);
        global_scope.declare(&mut compiler, "globalB", None, None, None, true);

        let local_scope = TypedScope::new(&mut compiler, global_scope, function_node);
        local_scope.declare(&mut compiler, "localA", None, None, None, true);
        local_scope.declare(&mut compiler, "localB", None, None, None, true);

        let global_entry: Arc<dyn FlowScope> = LinkedFlowScope::create_entry_lattice(global_scope);
        let local_entry: Arc<dyn FlowScope> = LinkedFlowScope::create_entry_lattice(local_scope);
        Self {
            compiler,
            local_scope,
            global_entry,
            local_entry,
        }
    }

    fn native(&mut self, native: JSTypeNative) -> TypeId {
        self.compiler.get_type_registry().get_native_type(native)
    }

    // port: CompilerTypeTestCase#getNativeNumberType
    fn get_native_number_type(&mut self) -> TypeId {
        self.native(JSTypeNative::NUMBER_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeStringType
    fn get_native_string_type(&mut self) -> TypeId {
        self.native(JSTypeNative::STRING_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeBooleanType
    fn get_native_boolean_type(&mut self) -> TypeId {
        self.native(JSTypeNative::BOOLEAN_TYPE)
    }

    // port: CompilerTypeTestCase#getNativeNoType
    fn get_native_no_type(&mut self) -> TypeId {
        self.native(JSTypeNative::NO_TYPE)
    }

    // port: CompilerTypeTestCase#createUnionType
    fn create_union_type(&mut self, variants: &[TypeId]) -> TypeId {
        let (reg, ast) = self.compiler.get_type_registry_and_ast();
        reg.create_union_type(ast, variants)
    }

    // port: CompilerTypeTestCase#assertTypeEquals
    fn assert_type_equals(&mut self, a: TypeId, b: Option<TypeId>) {
        let b = b.expect("expected a non-null type");
        let (reg, ast) = self.compiler.get_type_registry_and_ast();
        TypeSubject::assert_type(b).is_equal_to(reg, ast, a);
    }

    fn infer(
        &mut self,
        scope: &Arc<dyn FlowScope>,
        name: &str,
        type_: TypeId,
    ) -> Arc<dyn FlowScope> {
        Arc::clone(scope).infer_slot_type(&mut self.compiler, &JsString::from(name), Some(type_))
    }

    fn slot_type(&mut self, scope: &Arc<dyn FlowScope>, name: &str) -> Option<TypeId> {
        let slot = scope
            .get_slot(&mut self.compiler, &JsString::from(name))
            .expect("NullPointerException");
        slot.get_type(&self.compiler)
    }

    /// Java `a.equals(b)` (LinkedFlowScope#equals).
    fn flow_equals(&mut self, a: &Arc<dyn FlowScope>, b: &Arc<dyn FlowScope>) -> bool {
        a.as_any()
            .downcast_ref::<LinkedFlowScope>()
            .unwrap()
            .equals(&mut self.compiler, &**b)
    }

    // port: LinkedFlowScopeTest#assertScopesDiffer
    fn assert_scopes_differ(&mut self, a: &Arc<dyn FlowScope>, b: &Arc<dyn FlowScope>) {
        assert!(!self.flow_equals(a, b));
        assert!(!self.flow_equals(b, a));
    }

    // port: LinkedFlowScopeTest#assertScopesSame
    fn assert_scopes_same(&mut self, a: &Arc<dyn FlowScope>, b: &Arc<dyn FlowScope>) {
        assert!(self.flow_equals(b, a));
        assert!(self.flow_equals(a, b));
    }

    // port: LinkedFlowScopeTest#join
    fn join(&mut self, a: &Arc<dyn FlowScope>, b: &Arc<dyn FlowScope>) -> Arc<dyn FlowScope> {
        let mut joiner = FlowScopeJoinOp::new();
        joiner.join_flow(&mut self.compiler, Arc::clone(a));
        joiner.join_flow(&mut self.compiler, Arc::clone(b));
        joiner.finish().expect("NullPointerException")
    }
}

// port: LinkedFlowScopeTest#testJoin1
#[test]
fn test_join1() {
    let mut t = LinkedFlowScopeTest::set_up();
    let local_entry = Arc::clone(&t.local_entry);
    let number = t.get_native_number_type();
    let string = t.get_native_string_type();
    let boolean = t.get_native_boolean_type();
    let child_a = t.infer(&local_entry, "localB", number);
    let child_ab = t.infer(&child_a, "localB", string);
    let child_b = t.infer(&local_entry, "localB", boolean);

    let ty = t.slot_type(&child_ab, "localB");
    t.assert_type_equals(string, ty);
    let ty = t.slot_type(&child_b, "localB");
    t.assert_type_equals(boolean, ty);
    assert_eq!(t.slot_type(&child_b, "localA"), None);

    let mut joined = t.join(&child_b, &child_ab);
    let union = t.create_union_type(&[string, boolean]);
    let ty = t.slot_type(&joined, "localB");
    t.assert_type_equals(union, ty);
    assert_eq!(t.slot_type(&joined, "localA"), None);

    joined = t.join(&child_ab, &child_b);
    let union = t.create_union_type(&[string, boolean]);
    let ty = t.slot_type(&joined, "localB");
    t.assert_type_equals(union, ty);
    assert_eq!(t.slot_type(&joined, "localA"), None);

    let left = t.join(&child_ab, &child_b);
    let right = t.join(&child_b, &child_ab);
    assert!(t.flow_equals(&left, &right), "Join should be symmetric");
}

// port: LinkedFlowScopeTest#testJoin2
#[test]
fn test_join2() {
    let mut t = LinkedFlowScopeTest::set_up();
    let local_entry = Arc::clone(&t.local_entry);
    let string = t.get_native_string_type();
    let boolean = t.get_native_boolean_type();
    let child_a = t.infer(&local_entry, "localA", string);
    let child_b = t.infer(&local_entry, "globalB", boolean);

    let ty = t.slot_type(&child_a, "localA");
    t.assert_type_equals(string, ty);
    let ty = t.slot_type(&child_b, "globalB");
    t.assert_type_equals(boolean, ty);
    assert_eq!(t.slot_type(&child_b, "localB"), None);

    let mut joined = t.join(&child_b, &child_a);
    let ty = t.slot_type(&joined, "localA");
    t.assert_type_equals(string, ty);
    let ty = t.slot_type(&joined, "globalB");
    t.assert_type_equals(boolean, ty);

    joined = t.join(&child_a, &child_b);
    let ty = t.slot_type(&joined, "localA");
    t.assert_type_equals(string, ty);
    let ty = t.slot_type(&joined, "globalB");
    t.assert_type_equals(boolean, ty);

    let left = t.join(&child_a, &child_b);
    let right = t.join(&child_b, &child_a);
    assert!(t.flow_equals(&left, &right), "Join should be symmetric");
}

// port: LinkedFlowScopeTest#testJoin3
#[test]
fn test_join3() {
    let mut t = LinkedFlowScopeTest::set_up();
    let string = t.get_native_string_type();
    let number = t.get_native_number_type();
    let boolean = t.get_native_boolean_type();
    let local_scope = t.local_scope;
    local_scope.declare(&mut t.compiler, "localC", None, Some(string), None, true);
    local_scope.declare(&mut t.compiler, "localD", None, Some(string), None, true);

    let local_entry = Arc::clone(&t.local_entry);
    let child_a = t.infer(&local_entry, "localC", number);
    let child_b = t.infer(&local_entry, "localD", boolean);

    let mut joined = t.join(&child_b, &child_a);
    let union_c = t.create_union_type(&[string, number]);
    let ty = t.slot_type(&joined, "localC");
    t.assert_type_equals(union_c, ty);
    let union_d = t.create_union_type(&[string, boolean]);
    let ty = t.slot_type(&joined, "localD");
    t.assert_type_equals(union_d, ty);

    joined = t.join(&child_a, &child_b);
    let union_c = t.create_union_type(&[string, number]);
    let ty = t.slot_type(&joined, "localC");
    t.assert_type_equals(union_c, ty);
    let union_d = t.create_union_type(&[string, boolean]);
    let ty = t.slot_type(&joined, "localD");
    t.assert_type_equals(union_d, ty);

    let left = t.join(&child_a, &child_b);
    let right = t.join(&child_b, &child_a);
    assert!(t.flow_equals(&left, &right), "Join should be symmetric");
}

// port: LinkedFlowScopeTest#testLongChain
/// Create a long chain of flow scopes.
#[test]
fn test_long_chain() {
    let mut t = LinkedFlowScopeTest::set_up();
    let number = t.get_native_number_type();
    let string = t.get_native_string_type();
    let boolean = t.get_native_boolean_type();
    let local_scope = t.local_scope;
    let mut chain_a = Arc::clone(&t.local_entry);
    let mut chain_b = Arc::clone(&t.local_entry);
    for i in 0..LONG_CHAIN_LENGTH {
        local_scope.declare(&mut t.compiler, format!("local{i}"), None, None, None, true);
        chain_a = t.infer(
            &chain_a,
            &format!("local{i}"),
            if i % 2 == 0 { number } else { boolean },
        );
        chain_b = t.infer(
            &chain_b,
            &format!("local{i}"),
            if i % 3 == 0 { string } else { boolean },
        );
    }

    let joined = t.join(&chain_a, &chain_b);
    for i in 0..LONG_CHAIN_LENGTH {
        let ty = t.slot_type(&chain_a, &format!("local{i}"));
        t.assert_type_equals(if i % 2 == 0 { number } else { boolean }, ty);
        let ty = t.slot_type(&chain_b, &format!("local{i}"));
        t.assert_type_equals(if i % 3 == 0 { string } else { boolean }, ty);

        let joined_slot_type = t.slot_type(&joined, &format!("local{i}"));
        if i % 6 == 0 {
            let union = t.create_union_type(&[string, number]);
            t.assert_type_equals(union, joined_slot_type);
        } else if i % 2 == 0 {
            let union = t.create_union_type(&[number, boolean]);
            t.assert_type_equals(union, joined_slot_type);
        } else if i % 3 == 0 {
            let union = t.create_union_type(&[string, boolean]);
            t.assert_type_equals(union, joined_slot_type);
        } else {
            t.assert_type_equals(boolean, joined_slot_type);
        }
    }

    t.assert_scopes_differ(&chain_a, &chain_b);
    t.assert_scopes_differ(&chain_a, &joined);
    t.assert_scopes_differ(&chain_b, &joined);
}

// port: LinkedFlowScopeTest#testDiffer1
#[test]
fn test_differ1() {
    let mut t = LinkedFlowScopeTest::set_up();
    let local_entry = Arc::clone(&t.local_entry);
    let number = t.get_native_number_type();
    let string = t.get_native_string_type();
    let boolean = t.get_native_boolean_type();
    let no_type = t.get_native_no_type();
    let child_a = t.infer(&local_entry, "localB", number);
    let child_ab = t.infer(&child_a, "localB", string);
    let child_abc = t.infer(&child_ab, "localA", boolean);
    let child_b = t.infer(&child_ab, "localB", string);
    let child_bc = t.infer(&child_b, "localA", no_type);

    t.assert_scopes_same(&child_ab, &child_b);
    t.assert_scopes_differ(&child_abc, &child_bc);

    t.assert_scopes_differ(&child_abc, &child_b);
    t.assert_scopes_differ(&child_ab, &child_bc);

    t.assert_scopes_differ(&child_a, &child_ab);
    t.assert_scopes_differ(&child_a, &child_abc);
    t.assert_scopes_differ(&child_a, &child_b);
    t.assert_scopes_differ(&child_a, &child_bc);
}

// port: LinkedFlowScopeTest#testDiffer2
#[test]
fn test_differ2() {
    let mut t = LinkedFlowScopeTest::set_up();
    let local_entry = Arc::clone(&t.local_entry);
    let number = t.get_native_number_type();
    let no_type = t.get_native_no_type();
    let child_a = t.infer(&local_entry, "localA", number);
    let child_b = t.infer(&local_entry, "localA", no_type);

    t.assert_scopes_differ(&child_a, &child_b);
}
