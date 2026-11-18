/*
 * Copyright 2026 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 030d049:
//   test/com/google/javascript/jscomp/BasicBlockTest.java.

//! Port of `BasicBlockTest.java`.

use closure_jscomp::basic_block::BasicBlock;
use closure_jscomp::compiler::Compiler;
use closure_rhino::node::NodeId;

struct BasicBlockTest {
    compiler: Compiler,
}

impl BasicBlockTest {
    // port: BasicBlockTest#setUp
    fn set_up() -> Self {
        Self {
            compiler: Compiler::new(),
        }
    }

    // port: BasicBlockTest#parse
    fn parse(&mut self, js: &str) -> NodeId {
        let script = self.compiler.parse_test_code(js);
        assert!(
            self.compiler.get_errors().is_empty(),
            "{:?}",
            self.compiler.get_errors()
        );
        script
    }

    // port: BasicBlockTest#parseExpr
    fn parse_expr(&mut self, js_expr: &str) -> NodeId {
        let script = self.parse(&format!("({js_expr});"));
        script.get_first_first_child(&self.compiler).unwrap()
    }

    /// `BasicBlock.isBlockBoundary(n, parent)`.
    fn is_block_boundary(&self, n: Option<NodeId>, parent: NodeId) -> bool {
        BasicBlock::is_block_boundary(&self.compiler, n.unwrap(), Some(parent))
    }
}

// port: BasicBlockTest#isBlockBoundary_conditionalAndShortCircuitChildren
#[test]
fn is_block_boundary_conditional_and_short_circuit_children() {
    let mut t = BasicBlockTest::set_up();
    for expr in [
        "a && b", "a || b", "a ?? b", "a ||= b", "a &&= b", "a ??= b", "a?.(b)",
    ] {
        let n = t.parse_expr(expr);
        let c = &t.compiler;
        assert!(!t.is_block_boundary(n.get_first_child(c), n), "{expr}");
        assert!(t.is_block_boundary(n.get_second_child(c), n), "{expr}");
    }

    let hook_node = t.parse_expr("a ? b : c");
    let c = &t.compiler;
    assert!(!t.is_block_boundary(hook_node.get_first_child(c), hook_node));
    assert!(t.is_block_boundary(hook_node.get_second_child(c), hook_node));
    assert!(t.is_block_boundary(hook_node.get_last_child(c), hook_node));
}

// port: BasicBlockTest#isBlockBoundary_controlStatements
#[test]
fn is_block_boundary_control_statements() {
    let mut t = BasicBlockTest::set_up();
    let if_node = t
        .parse("if (a) { b(); } else { c(); }")
        .get_first_child(&t.compiler)
        .unwrap();
    let c = &t.compiler;
    assert!(!t.is_block_boundary(if_node.get_first_child(c), if_node));
    assert!(t.is_block_boundary(if_node.get_second_child(c), if_node));
    assert!(t.is_block_boundary(if_node.get_last_child(c), if_node));

    let for_node = t
        .parse("for (let i = 0; i < 10; i++) { body(); }")
        .get_first_child(&t.compiler)
        .unwrap();
    let c = &t.compiler;
    for child in for_node.children(c) {
        assert!(t.is_block_boundary(Some(child), for_node));
    }

    let try_node = t
        .parse("try { a(); } catch (e) { b(); } finally { c(); }")
        .get_first_child(&t.compiler)
        .unwrap();
    let c = &t.compiler;
    for child in try_node.children(c) {
        assert!(t.is_block_boundary(Some(child), try_node));
    }

    let switch_node = t
        .parse("switch (a) { case 1: b(); }")
        .get_first_child(&t.compiler)
        .unwrap();
    let c = &t.compiler;
    let switch_cond = switch_node.get_first_child(c);
    let switch_body = switch_node.get_second_child(c).unwrap();
    let case_node = switch_body.get_first_child(c).unwrap();
    assert!(!t.is_block_boundary(switch_cond, switch_node));
    assert!(t.is_block_boundary(Some(case_node), switch_body));
    assert!(!t.is_block_boundary(case_node.get_first_child(c), case_node));

    let label_node = t
        .parse("label: { a(); }")
        .get_first_child(&t.compiler)
        .unwrap();
    let c = &t.compiler;
    assert!(!t.is_block_boundary(label_node.get_first_child(c), label_node));
    assert!(t.is_block_boundary(label_node.get_second_child(c), label_node));

    let class_node = t
        .parse("class C extends Base {}")
        .get_first_child(&t.compiler)
        .unwrap();
    let c = &t.compiler;
    // All children of a class are conservatively treated as separate blocks.
    for child in class_node.children(c) {
        assert!(t.is_block_boundary(Some(child), class_node));
    }
}

// port: BasicBlockTest#provablyExecutesBefore_blockHierarchy
#[test]
fn provably_executes_before_block_hierarchy() {
    let mut t = BasicBlockTest::set_up();
    let script = t.parse("if (a) { if (b) {} } else {}");
    let c = &t.compiler;
    let if_node = script.get_first_child(c).unwrap();
    let then_block_node = if_node.get_second_child(c).unwrap();
    let nested_then_block_node = then_block_node
        .get_first_child(c)
        .unwrap()
        .get_second_child(c)
        .unwrap();
    let else_block_node = if_node.get_last_child(c).unwrap();

    let root_block = BasicBlock::new(c, /* parent= */ None, script);
    let then_block = BasicBlock::new(c, Some(root_block.clone()), then_block_node);
    let nested_then_block = BasicBlock::new(c, Some(then_block.clone()), nested_then_block_node);
    let else_block = BasicBlock::new(c, Some(root_block.clone()), else_block_node);

    assert!(root_block.provably_executes_before(&then_block));
    assert!(root_block.provably_executes_before(&nested_then_block));
    assert!(then_block.provably_executes_before(&nested_then_block));
    assert!(!then_block.provably_executes_before(&else_block));
    assert!(!nested_then_block.provably_executes_before(&then_block));
}
