/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/NodeTraversalTest.java.

use closure_jscomp::{
    Compiler,
    check_level::CheckLevel,
    compiler_options::CompilerOptions,
    diagnostic_type::DiagnosticType,
    error_handler::ErrorHandler,
    error_manager::ErrorManager,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    scope::ScopeId,
    sorting_error_manager::SortingErrorManager,
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

// port: NodeTraversalTest#parse
fn parse(compiler: &mut Compiler, js: &str) -> NodeId {
    let n = compiler.parse_test_code(js);
    assert!(
        compiler.get_errors().is_empty(),
        "{:?}",
        compiler.get_errors()
    );
    IR::root(compiler, &[n]);
    n
}

struct RecordingErrorManager {
    errors: Arc<Mutex<Vec<JSError>>>,
    sorting_manager: SortingErrorManager,
}
impl ErrorHandler for RecordingErrorManager {
    // port: NodeTraversalTest.testReportWithRange#report
    fn report(&mut self, _level: CheckLevel, error: JSError) {
        self.errors.lock().unwrap().push(error);
    }
}
impl ErrorManager for RecordingErrorManager {
    fn generate_report(&mut self, _ast: &Ast) {}
    fn get_error_count(&self) -> i32 {
        self.sorting_manager.get_error_count()
    }
    fn get_warning_count(&self) -> i32 {
        self.sorting_manager.get_warning_count()
    }
    fn get_errors(&self) -> Vec<JSError> {
        self.sorting_manager.get_errors()
    }
    fn get_warnings(&self) -> Vec<JSError> {
        self.sorting_manager.get_warnings()
    }
    fn set_typed_percent(&mut self, value: f64) {
        self.sorting_manager.set_typed_percent(value);
    }
    fn get_typed_percent(&self) -> f64 {
        self.sorting_manager.get_typed_percent()
    }
}

#[derive(Default)]
struct ExpectNodeOnEnterScope {
    node: Option<NodeId>,
    scope_root: Option<NodeId>,
    entered: bool,
}
impl ExpectNodeOnEnterScope {
    // port: NodeTraversalTest.ExpectNodeOnEnterScope#expect
    fn expect(&mut self, node: NodeId, scope_root: NodeId) {
        self.node = Some(node);
        self.scope_root = Some(scope_root);
        self.entered = false;
    }
    // port: NodeTraversalTest.ExpectNodeOnEnterScope#assertEntered
    fn assert_entered(&self) {
        assert!(self.entered);
    }
}
impl Callback for ExpectNodeOnEnterScope {
    // port: NodeTraversalTest.ExpectNodeOnEnterScope#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !self.entered
    }
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}
impl ScopedCallback for ExpectNodeOnEnterScope {
    // port: NodeTraversalTest.ExpectNodeOnEnterScope#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        assert_eq!(t.get_current_node(), self.node);
        assert_eq!(t.get_scope_root(), self.scope_root);
        let node = self.node.unwrap();
        if node.is_for_in(t) || node.is_for_of(t) {
            self.node = node.get_last_child(t);
            self.scope_root = self.scope_root.unwrap().get_last_child(t);
        }
        self.entered = true;
    }
    // port: NodeTraversalTest.ExpectNodeOnEnterScope#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}

#[derive(Default)]
struct TokenAccumulator {
    tokens: Vec<Token>,
    scope_roots: Vec<Token>,
}
impl Callback for TokenAccumulator {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
    // port: NodeTraversalTest.TokenAccumulator#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        self.tokens.push(n.get_token(t));
    }
    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}
impl ScopedCallback for TokenAccumulator {
    // port: NodeTraversalTest.TokenAccumulator#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        self.scope_roots
            .push(t.get_scope_root().unwrap().get_token(t));
    }
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}

#[derive(Default)]
struct StringAccumulator {
    strings: Vec<JsString>,
}
impl Callback for StringAccumulator {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
    // port: NodeTraversalTest.StringAccumulator#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_string_lit(t) {
            self.strings.push(n.get_string(t));
        }
    }
}

#[derive(Default)]
struct LexicallyScopedVarsAccumulator {
    var_names: IndexSet<JsString>,
}
impl Callback for LexicallyScopedVarsAccumulator {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
    // port: NodeTraversalTest.LexicallyScopedVarsAccumulator#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {
        let first_scope = t.get_scope();
        let mut scope = Some(first_scope);
        while let Some(current) = scope {
            for var in current.get_var_iterable(t.get_compiler()) {
                self.var_names.insert(var.get_name(t.get_compiler()));
            }
            scope = current.get_parent(t.get_compiler());
        }
    }
}

#[derive(Default)]
struct AccessibleCallback {
    num_accessible: usize,
}
impl AccessibleCallback {
    // port: NodeTraversalTest.AccessibleCallback#expect
    fn expect(&mut self, accessible: usize) {
        self.num_accessible = accessible;
    }
    // port: NodeTraversalTest.AccessibleCallback#assertAccessible
    fn assert_accessible(&self, compiler: &Compiler, s: ScopeId) {
        assert_eq!(
            s.get_all_accessible_variables(compiler).len(),
            self.num_accessible
        );
    }
}
impl Callback for AccessibleCallback {
    // port: NodeTraversalTest.AccessibleCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}
impl ScopedCallback for AccessibleCallback {
    // port: NodeTraversalTest.AccessibleCallback#enterScope
    fn enter_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
    // port: NodeTraversalTest.AccessibleCallback#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}

fn assert_contents(actual: impl IntoIterator<Item = JsString>, expected: &[&str]) {
    let mut actual: Vec<_> = actual.into_iter().collect();
    let mut expected: Vec<_> = expected.iter().map(|s| JsString::from(*s)).collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

// port: NodeTraversalTest#testReportWithRange
#[test]
fn test_report_with_range() {
    let errors = Arc::new(Mutex::new(Vec::new()));
    static DT: DiagnosticType = DiagnosticType::warning("FOO", "{0}, {1} - {2}");
    struct TestCallback;
    impl Callback for TestCallback {
        // port: NodeTraversalTest.testReportWithRange#shouldTraverse
        fn should_traverse(
            &mut self,
            t: &mut NodeTraversal<'_>,
            n: NodeId,
            parent: Option<NodeId>,
        ) -> bool {
            if n.is_get_prop(t) && parent.unwrap().is_expr_result(t) {
                t.report_with_range(Self::get_root_node(t, n), n, &DT, &["Foo", "Bar", "Hello"]);
            }
            true
        }
        // port: NodeTraversalTest.testReportWithRange#visit
        fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
    }
    impl TestCallback {
        // port: NodeTraversalTest.testReportWithRange#getRootNode
        fn get_root_node(t: &NodeTraversal<'_>, mut n: NodeId) -> NodeId {
            while n.is_get_prop(t) {
                n = n.get_first_child(t).unwrap();
            }
            n
        }
    }
    let mut compiler = Compiler::new_with_error_manager(Box::new(RecordingErrorManager {
        errors: Arc::clone(&errors),
        sorting_manager: SortingErrorManager::new(Vec::new()),
    }));
    compiler.init_compiler_options_if_testing();
    let code = "a.b.c;";
    let tree = parse(&mut compiler, code);
    let mut callback = TestCallback;
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .traverse(tree);
    let errors = errors.lock().unwrap();
    assert_eq!(errors.len(), 1);
    let error = &errors[0];
    assert_eq!(error.description(), "Foo, Bar - Hello");
    assert_eq!(error.get_node_source_offset(&compiler), 0);
    assert_eq!(error.length(), 5);
}

const TEST_EXCEPTION: &str = "test me";

// port: NodeTraversalTest#testUnexpectedException
#[test]
fn test_unexpected_exception() {
    let mut cb = |_t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>| {
        panic!("{TEST_EXCEPTION}");
    };
    let mut compiler = Compiler::new();
    let e = catch_unwind(AssertUnwindSafe(|| {
        let code = "function foo() {}";
        let tree = parse(&mut compiler, code);
        NodeTraversal::builder()
            .set_compiler(&mut compiler)
            .set_callback_post_order(&mut cb)
            .traverse(tree);
    }))
    .expect_err("Expected RuntimeException");
    let message = e
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| e.downcast_ref::<&str>().copied())
        .expect("string panic payload");
    assert!(
        message.starts_with("INTERNAL COMPILER ERROR.\nPlease report this problem.\n\ntest me")
    );
}

// port: NodeTraversalTest#testGetScopeRoot
#[test]
fn test_get_scope_root() {
    let mut compiler = Compiler::new();
    let code = "var a;\nfunction foo() {\n  var b\n}\n";
    let tree = parse(&mut compiler, code);
    struct TestCallback;
    impl Callback for TestCallback {
        // port: NodeTraversalTest.testGetScopeRoot#shouldTraverse
        fn should_traverse(
            &mut self,
            _t: &mut NodeTraversal<'_>,
            _n: NodeId,
            _parent: Option<NodeId>,
        ) -> bool {
            true
        }
        // port: NodeTraversalTest.testGetScopeRoot#visit
        fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
        fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
            Some(self)
        }
    }
    impl ScopedCallback for TestCallback {
        // port: NodeTraversalTest.testGetScopeRoot#enterScope
        fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
            let root1 = t.get_scope_root();
            let scope2 = t.get_scope();
            let root2 = scope2.get_root_node(t.get_compiler());
            assert_eq!(Some(root2), root1);
        }
        // port: NodeTraversalTest.testGetScopeRoot#exitScope
        fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
    }
    NodeTraversal::traverse(&mut compiler, tree, &mut TestCallback);
}

struct ModuleScopeRootCallback;
impl Callback for ModuleScopeRootCallback {
    // port: NodeTraversalTest.testGetScopeRoot_inEsModule#shouldTraverse
    // port: NodeTraversalTest.testGetScopeRoot_inGoogModule#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if n.is_module_body(t) || parent.is_some_and(|p| p.is_module_body(t)) {
            assert_eq!(t.get_scope_root().unwrap().get_token(t), Token::MODULE_BODY);
        }
        true
    }
    // port: NodeTraversalTest.testGetScopeRoot_inEsModule#visit
    // port: NodeTraversalTest.testGetScopeRoot_inGoogModule#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_module_body(t) || parent.is_some_and(|p| p.is_module_body(t)) {
            assert_eq!(t.get_scope_root().unwrap().get_token(t), Token::MODULE_BODY);
        }
    }
}

// port: NodeTraversalTest#testGetScopeRoot_inEsModule
#[test]
fn test_get_scope_root_in_es_module() {
    let mut compiler = Compiler::new();
    let code = "const x = 0;\nexport {x};\n";
    let tree = parse(&mut compiler, code);
    NodeTraversal::traverse(&mut compiler, tree, &mut ModuleScopeRootCallback);
}

// port: NodeTraversalTest#testGetScopeRoot_inGoogModule
#[test]
fn test_get_scope_root_in_goog_module() {
    let mut compiler = Compiler::new();
    let code = "goog.module('a.b');\nfunction foo() {\n  var b\n}\n";
    let tree = parse(&mut compiler, code);
    NodeTraversal::traverse(&mut compiler, tree, &mut ModuleScopeRootCallback);
}

// port: NodeTraversalTest#testGetHoistScopeRoot
#[test]
fn test_get_hoist_scope_root() {
    let mut compiler = Compiler::new();
    let code = "function foo() {\n  if (true) { var XXX; }\n}\n";
    let tree = parse(&mut compiler, code);
    struct TestCallback;
    impl Callback for TestCallback {
        // port: NodeTraversalTest.testGetHoistScopeRoot#shouldTraverse
        fn should_traverse(
            &mut self,
            _t: &mut NodeTraversal<'_>,
            _n: NodeId,
            _parent: Option<NodeId>,
        ) -> bool {
            true
        }
        // port: NodeTraversalTest.testGetHoistScopeRoot#visit
        fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
            if n.is_name(t) && n.get_string(t) == "XXX" {
                let root = t.get_closest_hoist_scope_root().unwrap();
                assert!(NodeUtil::is_function_block(t, root));
                t.get_scope();
                let root = t.get_closest_hoist_scope_root().unwrap();
                assert!(NodeUtil::is_function_block(t, root));
            }
        }
    }
    NodeTraversal::traverse(&mut compiler, tree, &mut TestCallback);
}

// port: NodeTraversalTest#testGetCurrentNode
#[test]
fn test_get_current_node() {
    let mut compiler = Compiler::new();
    let mut creator = SyntacticScopeCreator::new();
    let mut callback = ExpectNodeOnEnterScope::default();
    let code = "var a;\nfunction foo() {\n  var b;\n}\n";
    let tree = parse(&mut compiler, code);
    let top_scope = creator.create_scope(&mut compiler, tree, None);
    let first = tree.get_first_child(&compiler).unwrap();
    callback.expect(first, tree);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_with_scope(first, top_scope);
    callback.assert_entered();
    callback.expect(first, first);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse(first);
    callback.assert_entered();
    let function = tree.get_second_child(&compiler).unwrap();
    let fn_scope = creator.create_scope(&mut compiler, function, Some(top_scope));
    callback.expect(function, function);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(fn_scope);
    callback.assert_entered();
}

// port: NodeTraversalTest#testTraverseAtScopeWithBlockScope
#[test]
fn test_traverse_at_scope_with_block_scope() {
    let mut compiler = Compiler::new();
    let options = CompilerOptions::new();
    compiler.init_options(options);
    let mut creator = SyntacticScopeCreator::new();
    let mut callback = ExpectNodeOnEnterScope::default();
    let code = "function foo() {\n  if (bar) {\n    let x;\n  }\n}\n";
    let tree = parse(&mut compiler, code);
    let top_scope = creator.create_scope(&mut compiler, tree, None);
    let inner_block = tree
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let block_scope = creator.create_scope(&mut compiler, inner_block, Some(top_scope));
    callback.expect(inner_block, inner_block);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(block_scope);
    callback.assert_entered();
}

// port: NodeTraversalTest#testTraverseAtScopeWithForScope
#[test]
fn test_traverse_at_scope_with_for_scope() {
    let mut compiler = Compiler::new();
    let options = CompilerOptions::new();
    compiler.init_options(options);
    let mut creator = SyntacticScopeCreator::new();
    let mut callback = ExpectNodeOnEnterScope::default();
    let code = "function foo() {\n  var b = [0];\n  for (let a of b) {\n    let x;\n  }\n}\n";
    let tree = parse(&mut compiler, code);
    let top_scope = creator.create_scope(&mut compiler, tree, None);
    let for_node = tree
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_second_child(&compiler)
        .unwrap();
    let inner_block = for_node.get_last_child(&compiler).unwrap();
    let for_scope = creator.create_scope(&mut compiler, for_node, Some(top_scope));
    creator.create_scope(&mut compiler, inner_block, Some(for_scope));
    callback.expect(for_node, for_node);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(for_scope);
    callback.assert_entered();
}

// port: NodeTraversalTest#testTraverseAtScopeWithSwitchScope
#[test]
fn test_traverse_at_scope_with_switch_scope() {
    let mut compiler = Compiler::new();
    let options = CompilerOptions::new();
    compiler.init_options(options);
    let mut creator = SyntacticScopeCreator::new();
    let mut callback = ExpectNodeOnEnterScope::default();
    let code = "function foo() {\n  var b = [0];\n  switch(b) {\n    case 1:\n       return b;\n    case 2:\n  }\n}\n";
    let tree = parse(&mut compiler, code);
    let top_scope = creator.create_scope(&mut compiler, tree, None);
    let inner_block = tree
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_second_child(&compiler)
        .unwrap()
        .get_second_child(&compiler)
        .unwrap();
    let block_scope = creator.create_scope(&mut compiler, inner_block, Some(top_scope));
    callback.expect(inner_block, inner_block);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(block_scope);
    callback.assert_entered();
}

// port: NodeTraversalTest#testTraverseAtScopeWithModuleScope
#[test]
fn test_traverse_at_scope_with_module_scope() {
    let mut compiler = Compiler::new();
    let options = CompilerOptions::new();
    compiler.init_options(options);
    let mut creator = SyntacticScopeCreator::new();
    let mut callback = ExpectNodeOnEnterScope::default();
    let code = "goog.module('example.module');\n\nvar x;\n";
    let tree = parse(&mut compiler, code);
    let global_scope = creator.create_scope(&mut compiler, tree, None);
    let module_body = tree.get_first_child(&compiler).unwrap();
    let module_scope = creator.create_scope(&mut compiler, module_body, Some(global_scope));
    callback.expect(module_body, module_body);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(module_scope);
    callback.assert_entered();
}

// port: NodeTraversalTest#testTraverseAtScopeWithMemberFieldDefScope
#[test]
fn test_traverse_at_scope_with_member_field_def_scope() {
    let mut compiler = Compiler::new();
    compiler.init_compiler_options_if_testing();
    let mut creator = SyntacticScopeCreator::new();
    let mut callback = ExpectNodeOnEnterScope::default();
    let code = "class Foo {\n  a = this.a;\n}\nclass Bar extends Foo {\n  b = super.a;\n}\n";
    let tree = parse(&mut compiler, code);
    let global_scope = creator.create_scope(&mut compiler, tree, None);
    let member_field_def_a = tree
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let member_field_def_a_scope =
        creator.create_scope(&mut compiler, member_field_def_a, Some(global_scope));
    callback.expect(member_field_def_a, member_field_def_a);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(member_field_def_a_scope);
    callback.assert_entered();
    let member_field_def_b = tree
        .get_second_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let member_field_def_b_scope =
        creator.create_scope(&mut compiler, member_field_def_b, Some(global_scope));
    callback.expect(member_field_def_b, member_field_def_b);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(member_field_def_b_scope);
    callback.assert_entered();
}

// port: NodeTraversalTest#testTraverseAtScopeWithComputedFieldDefScope
#[test]
fn test_traverse_at_scope_with_computed_field_def_scope() {
    let mut compiler = Compiler::new();
    compiler.init_compiler_options_if_testing();
    let mut creator = SyntacticScopeCreator::new();
    let mut callback = ExpectNodeOnEnterScope::default();
    let code = "class Foo {\n  x = 'hi';\n  [this.x] = this.x;\n}\n";
    let tree = parse(&mut compiler, code);
    let global_scope = creator.create_scope(&mut compiler, tree, None);
    let computed_field_def = tree
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let computed_field_def_scope =
        creator.create_scope(&mut compiler, computed_field_def, Some(global_scope));
    callback.expect(computed_field_def, computed_field_def);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(computed_field_def_scope);
    callback.assert_entered();
}

// port: NodeTraversalTest#testTraverseFieldDefScopeRootsInOrder
#[test]
fn test_traverse_field_def_scope_roots_in_order() {
    let mut compiler = Compiler::new();
    let code = "class Foo {\n  x = 'hi';\n  [this.x] = this.x;\n}\n";
    let tree = parse(&mut compiler, code);
    let mut callback = TokenAccumulator::default();
    NodeTraversal::traverse(&mut compiler, tree, &mut callback);
    assert_eq!(
        callback.scope_roots,
        [
            Token::SCRIPT,
            Token::CLASS,
            Token::MEMBER_FIELD_DEF,
            Token::COMPUTED_FIELD_DEF
        ]
    );
}

// port: NodeTraversalTest#testTraverseComputedFieldsInOrder
#[test]
fn test_traverse_computed_fields_in_order() {
    let mut compiler = Compiler::new();
    compiler.init_compiler_options_if_testing();
    let mut callback = TokenAccumulator::default();
    let mut creator = SyntacticScopeCreator::new();
    let code = "class Foo {\n  [this.x] = true;\n}\n";
    let tree = parse(&mut compiler, code);
    let global_scope = creator.create_scope(&mut compiler, tree, None);
    let computed_field_def = tree
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let computed_field_def_scope =
        creator.create_scope(&mut compiler, computed_field_def, Some(global_scope));
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(computed_field_def_scope);
    assert_eq!(callback.tokens, [Token::TRUE, Token::COMPUTED_FIELD_DEF]);
    callback.tokens.clear();
}

// port: NodeTraversalTest#testGetVarAccessible
#[test]
fn test_get_var_accessible() {
    let mut compiler = Compiler::new();
    let options = CompilerOptions::new();
    compiler.init_options(options);
    let mut creator = SyntacticScopeCreator::new();
    let mut callback = AccessibleCallback::default();
    let code = "var varDefinedInScript;\nvar foo = function(param) {\n  var varDefinedInFoo;\n  var baz = function() {\n    var varDefinedInBaz;\n  }\n}\nvar bar = function() {\n  var varDefinedInBar;\n}\n";
    let tree = parse(&mut compiler, code);
    let foo_node = tree
        .get_second_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let top_scope = creator.create_scope(&mut compiler, tree, None);
    let foo_scope = creator.create_scope(&mut compiler, foo_node, Some(top_scope));
    callback.expect(4);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(foo_scope);
    callback.assert_accessible(&compiler, foo_scope);
    let foo_block_node = foo_node.get_last_child(&compiler).unwrap();
    let foo_block_scope = creator.create_scope(&mut compiler, foo_block_node, Some(foo_scope));
    callback.expect(6);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(foo_block_scope);
    callback.assert_accessible(&compiler, foo_block_scope);
    let code = "var foo = function() {\n  var varDefinedInFoo;\n  var baz = function() {\n    var varDefinedInBaz;\n    let varDefinedInFoo; // shadows parent scope\n  }\n  let bar = 1;\n}\n";
    let tree = parse(&mut compiler, code);
    let foo_node = tree
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let foo_block_node = foo_node.get_last_child(&compiler).unwrap();
    let baz_node = foo_block_node
        .get_second_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let baz_block_node = baz_node.get_last_child(&compiler).unwrap();
    let top_scope = creator.create_scope(&mut compiler, tree, None);
    let foo_scope = creator.create_scope(&mut compiler, foo_node, Some(top_scope));
    let foo_block_scope = creator.create_scope(&mut compiler, foo_block_node, Some(foo_scope));
    let baz_scope = creator.create_scope(&mut compiler, baz_node, Some(foo_block_scope));
    let baz_block_scope = creator.create_scope(&mut compiler, baz_block_node, Some(baz_scope));
    callback.expect(5);
    NodeTraversal::builder()
        .set_compiler(&mut compiler)
        .set_callback(&mut callback)
        .set_scope_creator(&mut creator)
        .traverse_at_scope(baz_block_scope);
    callback.assert_accessible(&compiler, baz_block_scope);
}

// port: NodeTraversalTest#testTraverseEs6ScopeRoots_isLimitedToScope
#[test]
fn test_traverse_es6_scope_roots_is_limited_to_scope() {
    let mut compiler = Compiler::new();
    let mut callback = StringAccumulator::default();
    let code = "function foo() {\n  'string in foo';\n  function baz() {\n    'string nested in baz';\n  }\n}\nfunction bar() {\n  'string in bar';\n}\n";
    let tree = parse(&mut compiler, code);
    let foo_function = tree.get_first_child(&compiler).unwrap();
    NodeTraversal::traverse_scope_roots(&mut compiler, &[foo_function], &mut callback, false);
    assert_contents(callback.strings.clone(), &["string in foo"]);
    callback.strings.clear();
    NodeTraversal::traverse_scope_roots(&mut compiler, &[foo_function], &mut callback, true);
    assert_contents(callback.strings, &["string in foo", "string nested in baz"]);
}

// port: NodeTraversalTest#testTraverseEs6ScopeRoots_parentScopesWork
#[test]
fn test_traverse_es6_scope_roots_parent_scopes_work() {
    let mut compiler = Compiler::new();
    let mut callback = LexicallyScopedVarsAccumulator::default();
    let code = "var varDefinedInScript;\nvar foo = function() {\n  var varDefinedInFoo;\n  var baz = function() {\n    var varDefinedInBaz;\n  }\n}\nvar bar = function() {\n  var varDefinedInBar;\n}\n";
    let tree = parse(&mut compiler, code);
    let foo_function = tree
        .get_second_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    NodeTraversal::traverse_scope_roots(&mut compiler, &[foo_function], &mut callback, false);
    assert_contents(
        callback.var_names.clone(),
        &["varDefinedInScript", "foo", "bar", "varDefinedInFoo", "baz"],
    );
    callback.var_names.clear();
    NodeTraversal::traverse_scope_roots(&mut compiler, &[foo_function], &mut callback, true);
    assert_contents(
        callback.var_names,
        &[
            "varDefinedInScript",
            "foo",
            "bar",
            "varDefinedInFoo",
            "baz",
            "varDefinedInBaz",
        ],
    );
}

// port: NodeTraversalTest#testTraverseEs6ScopeRoots_callsEnterScope
#[test]
fn test_traverse_es6_scope_roots_calls_enter_scope() {
    let mut compiler = Compiler::new();
    let mut scopes_entered = Vec::new();
    struct TestCallback<'a> {
        scopes_entered: &'a mut Vec<NodeId>,
    }
    impl Callback for TestCallback<'_> {
        // port: NodeTraversalTest.testTraverseEs6ScopeRoots_callsEnterScope#visit
        fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
        // port: NodeTraversalTest.testTraverseEs6ScopeRoots_callsEnterScope#shouldTraverse
        fn should_traverse(
            &mut self,
            _t: &mut NodeTraversal<'_>,
            _n: NodeId,
            _parent: Option<NodeId>,
        ) -> bool {
            true
        }
        fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
            Some(self)
        }
    }
    impl ScopedCallback for TestCallback<'_> {
        // port: NodeTraversalTest.testTraverseEs6ScopeRoots_callsEnterScope#enterScope
        fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
            self.scopes_entered.push(t.get_scope_root().unwrap());
        }
        // port: NodeTraversalTest.testTraverseEs6ScopeRoots_callsEnterScope#exitScope
        fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
    }
    let code = "function foo() { {} }";
    let tree = parse(&mut compiler, code);
    let foo_function = tree.get_first_child(&compiler).unwrap();
    NodeTraversal::traverse_scope_roots(
        &mut compiler,
        &[foo_function],
        &mut TestCallback {
            scopes_entered: &mut scopes_entered,
        },
        true,
    );
    assert_eq!(scopes_entered.len(), 3);
}

// port: NodeTraversalTest#testTraverseComputedFieldsInClass
#[test]
fn test_traverse_computed_fields_in_class() {
    let mut compiler = Compiler::new();
    compiler.init_compiler_options_if_testing();
    let mut callback = StringAccumulator::default();
    let code = "class Foo {\n  ['in field lhs'] = 'in field rhs';\n  ['in method lhs']() {\n    'nested in method';\n  }\n}\n";
    let tree = parse(&mut compiler, code);
    NodeTraversal::traverse(&mut compiler, tree, &mut callback);
    assert_eq!(
        callback.strings,
        [
            "in field lhs",
            "in method lhs",
            "in field rhs",
            "nested in method"
        ]
        .map(JsString::from)
    );
    callback.strings.clear();
}

struct NameChangingCallback;
impl Callback for NameChangingCallback {
    // port: NodeTraversalTest.NameChangingCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
    // port: NodeTraversalTest.NameChangingCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_name(t) && n.get_string(t) == "change" {
            n.set_string(t, "xx");
            t.report_code_change();
        }
    }
}
// port: NodeTraversalTest#parseRoots
fn parse_roots(compiler: &mut Compiler, externs: &str, js: &str) -> NodeId {
    let externs = parse(compiler, externs).detach(compiler);
    let main = parse(compiler, js).detach(compiler);
    let externs = IR::root(compiler, &[externs]);
    let main = IR::root(compiler, &[main]);
    IR::root(compiler, &[externs, main])
}
// port: NodeTraversalTest#assertChangesRecorded
fn assert_changes_recorded(code: &str, callback: &mut dyn Callback) {
    let mut compiler = Compiler::new();
    let tree = parse_roots(&mut compiler, "", code);
    let verifier = closure_jscomp::change_verifier::ChangeVerifier::new(&compiler)
        .snapshot(&mut compiler, tree);
    let externs = tree.get_first_child(&compiler).unwrap();
    let root = tree.get_second_child(&compiler).unwrap();
    NodeTraversal::traverse_roots(&mut compiler, callback, externs, root);
    verifier.check_recorded_changes(&mut compiler, tree);
}
// port: NodeTraversalTest#testReportChange1
#[test]
fn test_report_change1() {
    assert_changes_recorded(
        "var change;\nfunction foo() {\n  var b\n}\n",
        &mut NameChangingCallback,
    );
}
// port: NodeTraversalTest#testReportChange2
#[test]
fn test_report_change2() {
    assert_changes_recorded(
        "var a;\nfunction foo() {\n  var change\n}\n",
        &mut NameChangingCallback,
    );
}
// port: NodeTraversalTest#testReportChange3
#[test]
fn test_report_change3() {
    assert_changes_recorded(
        "var a;\nfunction foo() {\n  var b\n}\nvar change\n",
        &mut NameChangingCallback,
    );
}
// port: NodeTraversalTest#testReportChange4
#[test]
fn test_report_change4() {
    assert_changes_recorded(
        "function foo() {\n  function bar() {\n    var change\n  }\n}\n",
        &mut NameChangingCallback,
    );
}
