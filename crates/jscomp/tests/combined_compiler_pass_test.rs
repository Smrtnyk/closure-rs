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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CombinedCompilerPassTest.java.

//! Port of `CombinedCompilerPassTest.java`.
use closure_jscomp::combined_compiler_pass::CombinedCompilerPass;
use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_options::CompilerOptions;
use closure_jscomp::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;

/// Returns a Node tree with the post-order traversal a b c d e f g h i j k l m and the in-order
/// traversal m d a b c h e f g l i j k:
///
/// ```text
///                                   m
///                         ,---------|---------.
///                         d         h         l
///                      ,--|--.   ,--|--.   ,--|--.
///                      a  b  c   e  f  g   i  j  k
/// ```
// port: CombinedCompilerPassTest#createPostOrderAlphabet
fn create_post_order_alphabet(ast: &mut Ast) -> NodeId {
    let a = ast.new_string("a");
    let b = ast.new_string("b");
    let c = ast.new_string("c");
    let d = ast.new_string("d");
    let e = ast.new_string("e");
    let f = ast.new_string("f");
    let g = ast.new_string("g");
    let h = ast.new_string("h");
    let i = ast.new_string("i");
    let j = ast.new_string("j");
    let k = ast.new_string("k");
    let l = ast.new_string("l");
    let m = ast.new_string("m");

    d.add_child_to_back(ast, a);
    d.add_child_to_back(ast, b);
    d.add_child_to_back(ast, c);

    h.add_child_to_back(ast, e);
    h.add_child_to_back(ast, f);
    h.add_child_to_back(ast, g);

    l.add_child_to_back(ast, i);
    l.add_child_to_back(ast, j);
    l.add_child_to_back(ast, k);

    m.add_child_to_back(ast, d);
    m.add_child_to_back(ast, h);
    m.add_child_to_back(ast, l);

    m
}

// port: CombinedCompilerPassTest#setUp
fn set_up() -> Compiler {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    compiler
}

/// Concatenates contents of string nodes encountered in pre-order and post-order traversals.
/// Abbreviates traversals by ignoring subtrees rooted with specified strings.
// port: CombinedCompilerPassTest.ConcatTraversal
#[derive(Default)]
struct ConcatTraversal {
    visited: String,
    should_traversed: String,
    ignoring: IndexSet<String>,
}

impl ConcatTraversal {
    // port: CombinedCompilerPassTest.ConcatTraversal#ignore
    fn ignore(mut self, s: &str) -> Self {
        self.ignoring.insert(s.to_string());
        self
    }
    // port: CombinedCompilerPassTest.ConcatTraversal#getVisited
    fn get_visited(&self) -> &str {
        &self.visited
    }
    // port: CombinedCompilerPassTest.ConcatTraversal#getShouldTraversed
    fn get_should_traversed(&self) -> &str {
        &self.should_traversed
    }
    // port: CombinedCompilerPassTest.ConcatTraversal#getIgnoring
    fn get_ignoring(&self) -> &IndexSet<String> {
        &self.ignoring
    }
}

impl Callback for ConcatTraversal {
    // port: CombinedCompilerPassTest.ConcatTraversal#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let ast = t.get_compiler();
        assert_eq!(n.get_token(ast), Token::STRINGLIT);
        let s = n.get_string(ast).to_string();
        self.should_traversed.push_str(&s);
        !self.ignoring.contains(&s)
    }
    // port: CombinedCompilerPassTest.ConcatTraversal#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let ast = t.get_compiler();
        assert_eq!(n.get_token(ast), Token::STRINGLIT);
        self.visited.push_str(&n.get_string(ast).to_string());
    }
}

/// Collection of data for a traversal test. Contains the traversal callback and the expected
/// pre- and post-order traversal results.
// port: CombinedCompilerPassTest.TestHelper
struct TestHelper {
    traversal: ConcatTraversal,
    expected_visited: &'static str,
    should_traverse_expected: &'static str,
}

impl TestHelper {
    // port: CombinedCompilerPassTest.TestHelper#TestHelper
    fn new(
        traversal: ConcatTraversal,
        expected_visited: &'static str,
        should_traverse_expected: &'static str,
    ) -> Self {
        Self {
            traversal,
            expected_visited,
            should_traverse_expected,
        }
    }
    // port: CombinedCompilerPassTest.TestHelper#getTraversal
    fn get_traversal(&mut self) -> &mut ConcatTraversal {
        &mut self.traversal
    }
    // port: CombinedCompilerPassTest.TestHelper#checkResults
    fn check_results(&self) {
        assert_eq!(
            self.traversal.get_visited(),
            self.expected_visited,
            "ConcatTraversal ignoring {:?} has unexpected visiting order",
            self.traversal.get_ignoring()
        );
        assert_eq!(
            self.traversal.get_should_traversed(),
            self.should_traverse_expected,
            "ConcatTraversal ignoring {:?} has unexpected traversal order",
            self.traversal.get_ignoring()
        );
    }
}

// port: CombinedCompilerPassTest#createStringTests
fn create_string_tests() -> Vec<TestHelper> {
    vec![
        TestHelper::new(ConcatTraversal::default(), "abcdefghijklm", "mdabchefglijk"),
        TestHelper::new(
            ConcatTraversal::default().ignore("d"),
            "efghijklm",
            "mdhefglijk",
        ),
        TestHelper::new(
            ConcatTraversal::default().ignore("f"),
            "abcdeghijklm",
            "mdabchefglijk",
        ),
        TestHelper::new(ConcatTraversal::default().ignore("m"), "", "m"),
    ]
}

// port: CombinedCompilerPassTest#testIndividualPasses
#[test]
fn test_individual_passes() {
    let mut compiler = set_up();
    for mut test in create_string_tests() {
        let root = create_post_order_alphabet(&mut compiler);
        let mut pass = CombinedCompilerPass::new(&compiler, vec![test.get_traversal()]);
        pass.process(&mut compiler, None, root);
        drop(pass);
        test.check_results();
    }
}

// port: CombinedCompilerPassTest#testCombinedPasses
#[test]
fn test_combined_passes() {
    let mut compiler = set_up();
    let mut tests = create_string_tests();
    let callbacks: Vec<&mut dyn Callback> = tests
        .iter_mut()
        .map(|test| test.get_traversal() as &mut dyn Callback)
        .collect();
    let root = create_post_order_alphabet(&mut compiler);
    let mut pass = CombinedCompilerPass::new(&compiler, callbacks);
    pass.process(&mut compiler, None, root);
    drop(pass);
    for test in &tests {
        test.check_results();
    }
}

/// Records the scopes visited during an AST traversal. Abbreviates traversals by ignoring
/// subtrees rooted with specified NAME nodes.
// port: CombinedCompilerPassTest.ScopeRecordingCallback
#[derive(Default)]
struct ScopeRecordingCallback {
    visited_scopes: IndexSet<NodeId>,
    ignoring: IndexSet<String>,
}

impl ScopeRecordingCallback {
    // port: CombinedCompilerPassTest.ScopeRecordingCallback#ignore
    fn ignore(&mut self, name: &str) {
        self.ignoring.insert(name.to_string());
    }
    // port: CombinedCompilerPassTest.ScopeRecordingCallback#getVisitedScopes
    fn get_visited_scopes(&self) -> &IndexSet<NodeId> {
        &self.visited_scopes
    }
}

impl Callback for ScopeRecordingCallback {
    // port: CombinedCompilerPassTest.ScopeRecordingCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let ast = t.get_compiler();
        !n.is_name(ast) || !self.ignoring.contains(&n.get_string(ast).to_string())
    }
    // port: CombinedCompilerPassTest.ScopeRecordingCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for ScopeRecordingCallback {
    // port: CombinedCompilerPassTest.ScopeRecordingCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        self.visited_scopes.insert(t.get_scope_root().unwrap());
    }
    // port: CombinedCompilerPassTest.ScopeRecordingCallback#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}

// port: CombinedCompilerPassTest#testScopes
#[test]
fn test_scopes() {
    let mut compiler = set_up();
    let root = compiler.parse_test_code("var y = function() { var x = function() { };}");

    let mut c1 = ScopeRecordingCallback::default();
    c1.ignore("y");
    let mut c2 = ScopeRecordingCallback::default();
    c2.ignore("x");
    let mut c3 = ScopeRecordingCallback::default();

    let mut pass = CombinedCompilerPass::new(&compiler, vec![&mut c1, &mut c2, &mut c3]);
    pass.process(&mut compiler, None, root);
    drop(pass);

    assert_eq!(c1.get_visited_scopes().len(), 1);
    assert_eq!(c2.get_visited_scopes().len(), 3);
    assert_eq!(c3.get_visited_scopes().len(), 5);
}
