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
//   test/com/google/javascript/jscomp/ControlFlowAnalysisTest.java.

#![allow(dead_code)]
use closure_jscomp::{
    Compiler,
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::{Branch, ControlFlowGraph},
    dot_formatter::DotFormatter,
    graph::{
        adjacency_graph::AdjacencyGraph,
        di_graph::{DiGraph, DiGraphEdge},
        graph::GraphEdge,
        graph_node::GraphNode,
    },
};
use closure_rhino::{node::NodeId, token::Token};
// port: ControlFlowAnalysisTest#testCfg(String, String)
fn test_cfg(compiler: &mut Compiler, input: &str, expected: &str) {
    test_cfg_with_traverse_functions(compiler, input, expected, true);
}
// port: ControlFlowAnalysisTest#testCfg(String, String, boolean)
fn test_cfg_with_traverse_functions(
    compiler: &mut Compiler,
    input: &str,
    expected: &str,
    traverse: bool,
) {
    let root = compiler.parse_synthetic_code("cfgtest", input);
    let cfg = ControlFlowAnalysis::builder()
        .set_cfg_root(root)
        .set_traverse_functions(traverse)
        .set_include_edge_annotations(true)
        .compute_cfg(compiler);
    assert_eq!(
        DotFormatter::to_dot_with_cfg(compiler, root, Some(&cfg)),
        expected
    );
}
// port: ControlFlowAnalysisTest#createCfg(String, boolean)
fn create_cfg_with_syn_block_pass(
    compiler: &mut Compiler,
    input: &str,
    run_syn_block_pass: bool,
) -> ControlFlowGraph<NodeId> {
    let root = compiler.parse_synthetic_code("cfgtest", input);
    if run_syn_block_pass {
        let mut pass = closure_jscomp::create_synthetic_blocks::CreateSyntheticBlocks::new(
            "START",
            Some("END".into()),
        );
        // Java: pass.process(null, root). The externs argument is never read by
        // CreateSyntheticBlocks#process and a Rust NodeId cannot be null, so root stands in.
        pass.process(compiler, root, root);
    }
    ControlFlowAnalysis::builder()
        .set_cfg_root(root)
        .set_traverse_functions(true)
        .set_include_edge_annotations(true)
        .compute_cfg(compiler)
}
// port: ControlFlowAnalysisTest#createCfg(String)
fn create_cfg(compiler: &mut Compiler, input: &str) -> ControlFlowGraph<NodeId> {
    create_cfg_with_syn_block_pass(compiler, input, false)
}
// port: ControlFlowAnalysisTest#getAllEdges(ControlFlowGraph)
fn get_all_edges(cfg: &ControlFlowGraph<NodeId>) -> Vec<DiGraphEdge> {
    let mut edges = vec![];
    for n in cfg.get_nodes() {
        edges.extend(cfg.get_out_edges(n.get_value(cfg)));
    }
    edges
}
// port: ControlFlowAnalysisTest#getAllEdges(ControlFlowGraph, Token, Token)
fn get_all_edges_tokens(
    cfg: &ControlFlowGraph<NodeId>,
    start: Token,
    end: Token,
    compiler: &Compiler,
) -> Vec<DiGraphEdge> {
    get_all_edges(cfg)
        .into_iter()
        .filter(|e| {
            e.get_source(cfg)
                .get_value(cfg)
                .is_some_and(|n| n.get_token(compiler) == start)
                && e.get_destination(cfg)
                    .get_value(cfg)
                    .is_some_and(|n| n.get_token(compiler) == end)
        })
        .collect()
}
// port: ControlFlowAnalysisTest#getAllEdges(ControlFlowGraph, Token, Token, Branch)
fn get_all_edges_branch(
    cfg: &ControlFlowGraph<NodeId>,
    start: Token,
    end: Token,
    branch: Branch,
    compiler: &Compiler,
) -> Vec<DiGraphEdge> {
    get_all_edges_tokens(cfg, start, end, compiler)
        .into_iter()
        .filter(|e| *e.get_value(cfg) == branch)
        .collect()
}
// port: ControlFlowAnalysisTest#isAncestor
fn is_ancestor(n: NodeId, descendant: NodeId, compiler: &Compiler) -> bool {
    for current in n.children(compiler) {
        if current == descendant || is_ancestor(current, descendant, compiler) {
            return true;
        }
    }
    false
}
// port: ControlFlowAnalysisTest#getAllDownEdges
fn get_all_down_edges(
    cfg: &ControlFlowGraph<NodeId>,
    start: Token,
    end: Token,
    branch: Branch,
    compiler: &Compiler,
) -> Vec<DiGraphEdge> {
    get_all_edges_branch(cfg, start, end, branch, compiler)
        .into_iter()
        .filter(|e| {
            is_ancestor(
                e.get_source(cfg).get_value(cfg).unwrap(),
                e.get_destination(cfg).get_value(cfg).unwrap(),
                compiler,
            )
        })
        .collect()
}
// port: ControlFlowAnalysisTest#assertNoEdge(ControlFlowGraph, Token, Token)
fn assert_no_edge(cfg: &ControlFlowGraph<NodeId>, start: Token, end: Token, compiler: &Compiler) {
    assert!(get_all_edges_tokens(cfg, start, end, compiler).is_empty());
}
// port: ControlFlowAnalysisTest#assertNoEdge(ControlFlowGraph, Token, Token, Branch)
fn assert_no_edge_branch(
    cfg: &ControlFlowGraph<NodeId>,
    start: Token,
    end: Token,
    branch: Branch,
    compiler: &Compiler,
) {
    assert!(get_all_edges_branch(cfg, start, end, branch, compiler).is_empty());
}
// port: ControlFlowAnalysisTest#assertDownEdge
fn assert_down_edge(
    cfg: &ControlFlowGraph<NodeId>,
    start: Token,
    end: Token,
    branch: Branch,
    compiler: &Compiler,
) {
    assert!(
        !get_all_down_edges(cfg, start, end, branch, compiler).is_empty(),
        "No down edge found"
    );
}
// port: ControlFlowAnalysisTest#assertEdge
fn assert_edge(
    cfg: &ControlFlowGraph<NodeId>,
    start: Token,
    end: Token,
    branch: Branch,
    compiler: &Compiler,
) {
    assert!(
        !get_all_edges_branch(cfg, start, end, branch, compiler).is_empty(),
        "No up edge found."
    );
}
// port: ControlFlowAnalysisTest#assertUpEdge
fn assert_up_edge(
    cfg: &ControlFlowGraph<NodeId>,
    start: Token,
    end: Token,
    branch: Branch,
    compiler: &Compiler,
) {
    assert!(
        !get_all_down_edges(cfg, end, start, branch, compiler).is_empty(),
        "No up edge found."
    );
}
// port: ControlFlowAnalysisTest#assertCrossEdge
fn assert_cross_edge(
    cfg: &ControlFlowGraph<NodeId>,
    start: Token,
    end: Token,
    branch: Branch,
    compiler: &Compiler,
) {
    let num_down = get_all_down_edges(cfg, start, end, branch, compiler).len();
    let num_up = get_all_down_edges(cfg, end, start, branch, compiler).len();
    let num_edges = get_all_edges_branch(cfg, start, end, branch, compiler).len();
    assert!(num_down + num_up < num_edges, "No cross edges found");
}
// port: ControlFlowAnalysisTest#assertReturnEdge
fn assert_return_edge(cfg: &ControlFlowGraph<NodeId>, start: Token, compiler: &Compiler) {
    for edge in get_all_edges(cfg) {
        let source = edge.get_source(cfg).get_value(cfg).unwrap();
        if source.get_token(compiler) == start && cfg.is_implicit_return(edge.get_destination(cfg))
        {
            return;
        }
    }
    panic!("No return edge found");
}
// port: ControlFlowAnalysisTest#assertNoReturnEdge
fn assert_no_return_edge(cfg: &ControlFlowGraph<NodeId>, start: Token, compiler: &Compiler) {
    for edge in get_all_edges(cfg) {
        let source = edge.get_source(cfg).get_value(cfg).unwrap();
        if source.get_token(compiler) == start {
            assert!(
                !cfg.is_implicit_return(edge.get_destination(cfg)),
                "Token {start} should not have an out going edge to the implicit return"
            );
            return;
        }
    }
}
// port: ControlFlowAnalysisTest#assertNodeOrder
fn assert_node_order(cfg: ControlFlowGraph<NodeId>, node_types: &[Token], compiler: &Compiler) {
    let mut nodes = cfg.get_nodes();
    let compare = cfg.get_optional_node_comparator(true).unwrap();
    nodes.sort_by(|&a, &b| compare(a, b));
    let implicit_return = nodes.pop().unwrap().get_value(&cfg);
    assert!(
        implicit_return.is_none(),
        "{}",
        implicit_return
            .map(|n| n.to_string_tree(compiler))
            .unwrap_or_else(|| "null".into())
    );
    let tokens: Vec<_> = nodes
        .iter()
        .map(|n| n.get_value(&cfg).unwrap().get_token(compiler))
        .collect();
    assert_eq!(tokens, node_types);
}

// port: ControlFlowAnalysisTest#testSimpleStatementsInScript
#[test]
fn test_simple_statements_in_script() {
    let mut compiler = Compiler::new();
    let src = r###"var a; a = a; a = a"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::SCRIPT, Token::VAR, Branch::UNCOND, &compiler);
    assert_cross_edge(
        &cfg,
        Token::VAR,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSimpleStatementsInGoogModule
#[test]
fn test_simple_statements_in_goog_module() {
    let mut compiler = Compiler::new();
    let src = r###"goog.module('myMod'); var a; a = a; a = a"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(
        &cfg,
        Token::SCRIPT,
        Token::MODULE_BODY,
        Branch::UNCOND,
        &compiler,
    );

    assert_down_edge(
        &cfg,
        Token::MODULE_BODY,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::VAR,
        Branch::UNCOND,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSimpleStatementsInEsModule
#[test]
fn test_simple_statements_in_es_module() {
    let mut compiler = Compiler::new();
    let src = r###"var a; a = a; export default a;"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(
        &cfg,
        Token::SCRIPT,
        Token::MODULE_BODY,
        Branch::UNCOND,
        &compiler,
    );
    assert_down_edge(
        &cfg,
        Token::MODULE_BODY,
        Token::VAR,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(
        &cfg,
        Token::VAR,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::EXPORT,
        Branch::UNCOND,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSimpleIf
#[test]
fn test_simple_if() {
    let mut compiler = Compiler::new();
    let src = r###"var x; if (x) { x() } else { x() };"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::SCRIPT, Token::VAR, Branch::UNCOND, &compiler);
    assert_cross_edge(&cfg, Token::VAR, Token::IF, Branch::UNCOND, &compiler);
    assert_down_edge(&cfg, Token::IF, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_down_edge(
        &cfg,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_no_edge(&cfg, Token::EXPR_RESULT, Token::CALL, &compiler);
    assert_down_edge(&cfg, Token::IF, Token::BLOCK, Branch::ON_FALSE, &compiler);
    assert_return_edge(&cfg, Token::EMPTY, &compiler);
}

// port: ControlFlowAnalysisTest#testBreakingBlock
#[test]
fn test_breaking_block() {
    let mut compiler = Compiler::new();
    let src = r###"X: { while(1) { break } }"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_up_edge(&cfg, Token::BREAK, Token::BLOCK, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testBreakToStatement
#[test]
fn test_break_to_statement() {
    let mut compiler = Compiler::new();
    let src = r###"X: break X;"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_return_edge(&cfg, Token::BREAK, &compiler);
}

// port: ControlFlowAnalysisTest#testBreakWithMultipleLabels
#[test]
fn test_break_with_multiple_labels() {
    let mut compiler = Compiler::new();
    let src = r###"La: Lb: break La;"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_return_edge(&cfg, Token::BREAK, &compiler);
}

// port: ControlFlowAnalysisTest#testBreakingWhile
#[test]
fn test_breaking_while() {
    let mut compiler = Compiler::new();
    let src = r###"var x; while(true) { break; } x();"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::WHILE, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_down_edge(&cfg, Token::BLOCK, Token::BREAK, Branch::UNCOND, &compiler);
    assert_cross_edge(
        &cfg,
        Token::BREAK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testInifiteLoopWhile
#[test]
fn test_inifite_loop_while() {
    let mut compiler = Compiler::new();
    let src = r###"var x; while(true) { } x();"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::WHILE, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_no_edge(&cfg, Token::WHILE, Token::EXPR_RESULT, &compiler);
}

// port: ControlFlowAnalysisTest#testInifiteLoopDoWhile
#[test]
fn test_inifite_loop_do_while() {
    let mut compiler = Compiler::new();
    let src = r###"var x; do { } while (true); x();"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::DO, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_no_edge(&cfg, Token::DO, Token::EXPR_RESULT, &compiler);
}

// port: ControlFlowAnalysisTest#testInifiteLoopFor_emptyCond
#[test]
fn test_inifite_loop_for_empty_cond() {
    let mut compiler = Compiler::new();
    let src = r###"var x; for(;;) { } x();"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::FOR, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_no_edge(&cfg, Token::FOR, Token::EXPR_RESULT, &compiler);
}

// port: ControlFlowAnalysisTest#testBreakingFor_emptyCond
#[test]
fn test_breaking_for_empty_cond() {
    let mut compiler = Compiler::new();
    let src = r###"var x; for(;;) { break; } x();"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::FOR, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_down_edge(&cfg, Token::BLOCK, Token::BREAK, Branch::UNCOND, &compiler);
    assert_cross_edge(
        &cfg,
        Token::BREAK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testInifiteLoopFor_trueCond
#[test]
fn test_inifite_loop_for_true_cond() {
    let mut compiler = Compiler::new();
    let src = r###"var x; for(;true;) { } x();"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::FOR, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_no_edge(&cfg, Token::FOR, Token::EXPR_RESULT, &compiler);
}

// port: ControlFlowAnalysisTest#testBreakingFor_trueCond
#[test]
fn test_breaking_for_true_cond() {
    let mut compiler = Compiler::new();
    let src = r###"var x; for(;true;) { break; } x();"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::FOR, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_down_edge(&cfg, Token::BLOCK, Token::BREAK, Branch::UNCOND, &compiler);
    assert_cross_edge(
        &cfg,
        Token::BREAK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testThrowInCatchBlock
#[test]
fn test_throw_in_catch_block() {
    let mut compiler = Compiler::new();
    let src = r###"try { throw ''; } catch (e) { throw e;} finally {}"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="TRY"];
  node0 -> node1 [weight=1];
  node2 [label="BLOCK"];
  node1 -> node2 [weight=1];
  node3 [label="THROW"];
  node2 -> node3 [weight=1];
  node4 [label="STRINGLIT"];
  node3 -> node4 [weight=1];
  node5 [label="BLOCK"];
  node3 -> node5 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node2 -> node3 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node5 [weight=1];
  node6 [label="CATCH"];
  node5 -> node6 [weight=1];
  node7 [label="NAME(e)"];
  node6 -> node7 [weight=1];
  node8 [label="BLOCK"];
  node6 -> node8 [weight=1];
  node9 [label="THROW"];
  node8 -> node9 [weight=1];
  node10 [label="NAME(e)"];
  node9 -> node10 [weight=1];
  node11 [label="BLOCK"];
  node9 -> node11 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node8 -> node9 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node6 -> node8 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node5 -> node6 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node11 [weight=1];
  node11 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node2 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testBreakingTryBlock
#[test]
fn test_breaking_try_block() {
    let mut compiler = Compiler::new();
    let src = r###"a: try { break a; } finally {} if(x) {}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(&cfg, Token::BREAK, Token::IF, Branch::UNCOND, &compiler);

    let src = r###"a: try {} finally {break a;} if(x) {}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(&cfg, Token::BREAK, Token::IF, Branch::UNCOND, &compiler);

    let src = r###"a: try {} catch(e) {break a;} if(x) {}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(&cfg, Token::BREAK, Token::IF, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testWithStatement
#[test]
fn test_with_statement() {
    let mut compiler = Compiler::new();
    let src = r###"var x, y; with(x) { y() }"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::WITH, Token::BLOCK, Branch::UNCOND, &compiler);
    assert_no_edge(&cfg, Token::WITH, Token::NAME, &compiler);
    assert_no_edge(&cfg, Token::NAME, Token::BLOCK, &compiler);
    assert_down_edge(
        &cfg,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_return_edge(&cfg, Token::EXPR_RESULT, &compiler);
}

// port: ControlFlowAnalysisTest#testSimpleWhile
#[test]
fn test_simple_while() {
    let mut compiler = Compiler::new();
    let src = r###"var x; while (x) { x(); if (x) { break; } x() }"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::WHILE, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_down_edge(
        &cfg,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_down_edge(&cfg, Token::IF, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_return_edge(&cfg, Token::BREAK, &compiler);
}

// port: ControlFlowAnalysisTest#testSimpleSwitch
#[test]
fn test_simple_switch() {
    let mut compiler = Compiler::new();
    let src = r###"var x; switch(x){ case(1): x(); case('x'): x(); break; default: x();}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(&cfg, Token::VAR, Token::SWITCH, Branch::UNCOND, &compiler);
    assert_no_edge(&cfg, Token::SWITCH, Token::NAME, &compiler);
    assert_down_edge(
        &cfg,
        Token::SWITCH,
        Token::SWITCH_BODY,
        Branch::UNCOND,
        &compiler,
    );

    assert_down_edge(
        &cfg,
        Token::SWITCH_BODY,
        Token::CASE,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(&cfg, Token::CASE, Token::CASE, Branch::ON_FALSE, &compiler);
    assert_cross_edge(
        &cfg,
        Token::CASE,
        Token::DEFAULT_CASE,
        Branch::ON_FALSE,
        &compiler,
    );

    assert_down_edge(&cfg, Token::CASE, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_down_edge(
        &cfg,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_no_edge(&cfg, Token::EXPR_RESULT, Token::CALL, &compiler);
    assert_no_edge(&cfg, Token::CALL, Token::NAME, &compiler);
}

// port: ControlFlowAnalysisTest#testSimpleNoDefault
#[test]
fn test_simple_no_default() {
    let mut compiler = Compiler::new();
    let src = r###"var x; switch(x){ case(1): break; } x();"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::CASE,
        Token::EXPR_RESULT,
        Branch::ON_FALSE,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSwitchDefaultFirst
#[test]
fn test_switch_default_first() {
    let mut compiler = Compiler::new();

    let src = r###"var x; switch(x){ default: break; case 1: break; }"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(
        &cfg,
        Token::SWITCH_BODY,
        Token::CASE,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(
        &cfg,
        Token::CASE,
        Token::DEFAULT_CASE,
        Branch::ON_FALSE,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSwitchDefaultInMiddle
#[test]
fn test_switch_default_in_middle() {
    let mut compiler = Compiler::new();

    let src = r###"var x; switch(x){ case 1: break; default: break; case 2: break; }"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(
        &cfg,
        Token::SWITCH_BODY,
        Token::CASE,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(&cfg, Token::CASE, Token::CASE, Branch::ON_FALSE, &compiler);
    assert_cross_edge(
        &cfg,
        Token::CASE,
        Token::DEFAULT_CASE,
        Branch::ON_FALSE,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSwitchEmpty
#[test]
fn test_switch_empty() {
    let mut compiler = Compiler::new();

    let src = r###"var x; switch(x){}; x()"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::SWITCH_BODY,
        Token::EMPTY,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(
        &cfg,
        Token::EMPTY,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testReturnThrowingException
#[test]
fn test_return_throwing_exception() {
    let mut compiler = Compiler::new();
    let src = r###"function f() {try { return a(); } catch (e) {e()}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(&cfg, Token::RETURN, Token::BLOCK, Branch::ON_EX, &compiler);
    assert_down_edge(&cfg, Token::BLOCK, Token::CATCH, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testCallThrowingException
#[test]
fn test_call_throwing_exception() {
    let mut compiler = Compiler::new();
    let src = r###"function f() {try { obj.a(); } catch (e) {e()}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::BLOCK,
        Branch::ON_EX,
        &compiler,
    );
    assert_down_edge(&cfg, Token::BLOCK, Token::CATCH, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testOptChainCallThrowingException
#[test]
fn test_opt_chain_call_throwing_exception() {
    let mut compiler = Compiler::new();
    let src = r###"function f() {try { obj?.a(); } catch (e) {e()}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::BLOCK,
        Branch::ON_EX,
        &compiler,
    );
    assert_down_edge(&cfg, Token::BLOCK, Token::CATCH, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testOptChainGetPropThrowingException
#[test]
fn test_opt_chain_get_prop_throwing_exception() {
    let mut compiler = Compiler::new();
    let src = r###"function f() {try { obj?.a; } catch (e) {e()}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::BLOCK,
        Branch::ON_EX,
        &compiler,
    );
    assert_down_edge(&cfg, Token::BLOCK, Token::CATCH, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testOptChainGetElemThrowingException
#[test]
fn test_opt_chain_get_elem_throwing_exception() {
    let mut compiler = Compiler::new();
    let src = r###"function f() {try { obj?.[a]; } catch (e) {e()}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::BLOCK,
        Branch::ON_EX,
        &compiler,
    );
    assert_down_edge(&cfg, Token::BLOCK, Token::CATCH, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testForOfThrowingException
#[test]
fn test_for_of_throwing_exception() {
    let mut compiler = Compiler::new();
    let src = r###"function f() {try { for (var x of y) {} } catch (e) {e()}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(&cfg, Token::FOR_OF, Token::BLOCK, Branch::ON_EX, &compiler);
    assert_down_edge(&cfg, Token::BLOCK, Token::CATCH, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testForAwaitOfThrowingException
#[test]
fn test_for_await_of_throwing_exception() {
    let mut compiler = Compiler::new();
    let src = r###"async function f() {try { for await (var x of y) {} } catch (e) {e()}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::FOR_AWAIT_OF,
        Token::BLOCK,
        Branch::ON_EX,
        &compiler,
    );
    assert_down_edge(&cfg, Token::BLOCK, Token::CATCH, Branch::UNCOND, &compiler);
}

// port: ControlFlowAnalysisTest#testForInNoThrowingException
#[test]
fn test_for_in_no_throwing_exception() {
    let mut compiler = Compiler::new();
    let src = r###"function f() {try { for (var x in y) {} } catch (e) {e()}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_no_edge_branch(&cfg, Token::FOR_IN, Token::BLOCK, Branch::ON_EX, &compiler);
}

// port: ControlFlowAnalysisTest#testSimpleFor
#[test]
fn test_simple_for() {
    let mut compiler = Compiler::new();
    let src = r###"var a; for (var x = 0; x < 100; x++) { a(); }"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="VAR"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(a)"];
  node1 -> node2 [weight=1];
  node3 [label="VAR"];
  node1 -> node3 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 [label="FOR"];
  node0 -> node4 [weight=1];
  node4 -> node3 [weight=1];
  node5 [label="NAME(x)"];
  node3 -> node5 [weight=1];
  node6 [label="NUMBER"];
  node5 -> node6 [weight=1];
  node3 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node7 [label="LT"];
  node4 -> node7 [weight=1];
  node8 [label="NAME(x)"];
  node7 -> node8 [weight=1];
  node9 [label="NUMBER"];
  node7 -> node9 [weight=1];
  node10 [label="INC"];
  node4 -> node10 [weight=1];
  node11 [label="NAME(x)"];
  node10 -> node11 [weight=1];
  node10 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node12 [label="BLOCK"];
  node4 -> node12 [weight=1];
  node13 [label="EXPR_RESULT"];
  node12 -> node13 [weight=1];
  node14 [label="CALL"];
  node13 -> node14 [weight=1];
  node15 [label="NAME(a)"];
  node14 -> node15 [weight=1];
  node13 -> node10 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node12 -> node13 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 -> RETURN [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node4 -> node12 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testSimpleForWithContinue
#[test]
fn test_simple_for_with_continue() {
    let mut compiler = Compiler::new();
    let src = r###"var a; for (var x = 0; x < 100; x++) {a();continue;a()}"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="VAR"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(a)"];
  node1 -> node2 [weight=1];
  node3 [label="VAR"];
  node1 -> node3 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 [label="FOR"];
  node0 -> node4 [weight=1];
  node4 -> node3 [weight=1];
  node5 [label="NAME(x)"];
  node3 -> node5 [weight=1];
  node6 [label="NUMBER"];
  node5 -> node6 [weight=1];
  node3 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node7 [label="LT"];
  node4 -> node7 [weight=1];
  node8 [label="NAME(x)"];
  node7 -> node8 [weight=1];
  node9 [label="NUMBER"];
  node7 -> node9 [weight=1];
  node10 [label="INC"];
  node4 -> node10 [weight=1];
  node11 [label="NAME(x)"];
  node10 -> node11 [weight=1];
  node10 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node12 [label="BLOCK"];
  node4 -> node12 [weight=1];
  node13 [label="EXPR_RESULT"];
  node12 -> node13 [weight=1];
  node14 [label="CALL"];
  node13 -> node14 [weight=1];
  node15 [label="NAME(a)"];
  node14 -> node15 [weight=1];
  node16 [label="CONTINUE"];
  node13 -> node16 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node12 -> node16 [weight=1];
  node16 -> node10 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node17 [label="EXPR_RESULT"];
  node12 -> node17 [weight=1];
  node18 [label="CALL"];
  node17 -> node18 [weight=1];
  node19 [label="NAME(a)"];
  node18 -> node19 [weight=1];
  node17 -> node10 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node12 -> node13 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 -> RETURN [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node4 -> node12 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testNestedFor
#[test]
fn test_nested_for() {
    let mut compiler = Compiler::new();

    let src = r###"var a,b;a();for(var x=0;x<100;x++){for(var y=0;y<100;y++){continue;b();}}"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="VAR"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(a)"];
  node1 -> node2 [weight=1];
  node3 [label="NAME(b)"];
  node1 -> node3 [weight=1];
  node4 [label="EXPR_RESULT"];
  node1 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node4 [weight=1];
  node5 [label="CALL"];
  node4 -> node5 [weight=1];
  node6 [label="NAME(a)"];
  node5 -> node6 [weight=1];
  node7 [label="VAR"];
  node4 -> node7 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node8 [label="FOR"];
  node0 -> node8 [weight=1];
  node8 -> node7 [weight=1];
  node9 [label="NAME(x)"];
  node7 -> node9 [weight=1];
  node10 [label="NUMBER"];
  node9 -> node10 [weight=1];
  node7 -> node8 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node11 [label="LT"];
  node8 -> node11 [weight=1];
  node12 [label="NAME(x)"];
  node11 -> node12 [weight=1];
  node13 [label="NUMBER"];
  node11 -> node13 [weight=1];
  node14 [label="INC"];
  node8 -> node14 [weight=1];
  node15 [label="NAME(x)"];
  node14 -> node15 [weight=1];
  node14 -> node8 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node16 [label="BLOCK"];
  node8 -> node16 [weight=1];
  node17 [label="FOR"];
  node16 -> node17 [weight=1];
  node18 [label="VAR"];
  node17 -> node18 [weight=1];
  node19 [label="NAME(y)"];
  node18 -> node19 [weight=1];
  node20 [label="NUMBER"];
  node19 -> node20 [weight=1];
  node18 -> node17 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node21 [label="LT"];
  node17 -> node21 [weight=1];
  node22 [label="NAME(y)"];
  node21 -> node22 [weight=1];
  node23 [label="NUMBER"];
  node21 -> node23 [weight=1];
  node24 [label="INC"];
  node17 -> node24 [weight=1];
  node25 [label="NAME(y)"];
  node24 -> node25 [weight=1];
  node24 -> node17 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node26 [label="BLOCK"];
  node17 -> node26 [weight=1];
  node27 [label="CONTINUE"];
  node26 -> node27 [weight=1];
  node27 -> node24 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node28 [label="EXPR_RESULT"];
  node26 -> node28 [weight=1];
  node29 [label="CALL"];
  node28 -> node29 [weight=1];
  node30 [label="NAME(b)"];
  node29 -> node30 [weight=1];
  node28 -> node24 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node26 -> node27 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node17 -> node14 [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node17 -> node26 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node16 -> node18 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node8 -> RETURN [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node8 -> node16 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testNestedDoWithBreak
#[test]
fn test_nested_do_with_break() {
    let mut compiler = Compiler::new();

    let src = r###"var a;do{do{break}while(a);do{a()}while(a)}while(a);"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="VAR"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(a)"];
  node1 -> node2 [weight=1];
  node3 [label="BLOCK"];
  node1 -> node3 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 [label="DO"];
  node0 -> node4 [weight=1];
  node4 -> node3 [weight=1];
  node5 [label="DO"];
  node3 -> node5 [weight=1];
  node6 [label="BLOCK"];
  node5 -> node6 [weight=1];
  node7 [label="BREAK"];
  node6 -> node7 [weight=1];
  node8 [label="BLOCK"];
  node7 -> node8 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node6 -> node7 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node9 [label="NAME(a)"];
  node5 -> node9 [weight=1];
  node5 -> node6 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node5 -> node8 [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node10 [label="DO"];
  node3 -> node10 [weight=1];
  node10 -> node8 [weight=1];
  node11 [label="EXPR_RESULT"];
  node8 -> node11 [weight=1];
  node12 [label="CALL"];
  node11 -> node12 [weight=1];
  node13 [label="NAME(a)"];
  node12 -> node13 [weight=1];
  node11 -> node10 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node8 -> node11 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node14 [label="NAME(a)"];
  node10 -> node14 [weight=1];
  node10 -> node4 [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node10 -> node8 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node3 -> node6 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node15 [label="NAME(a)"];
  node4 -> node15 [weight=1];
  node4 -> RETURN [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node4 -> node3 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testForIn
#[test]
fn test_for_in() {
    let mut compiler = Compiler::new();
    let src = r###"var a,b;for(a in b){a()};"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="VAR"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(a)"];
  node1 -> node2 [weight=1];
  node3 [label="NAME(b)"];
  node1 -> node3 [weight=1];
  node4 [label="NAME(b)"];
  node1 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node5 [label="FOR_IN"];
  node0 -> node5 [weight=1];
  node6 [label="NAME(a)"];
  node5 -> node6 [weight=1];
  node5 -> node4 [weight=1];
  node4 -> node5 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node7 [label="BLOCK"];
  node5 -> node7 [weight=1];
  node8 [label="EXPR_RESULT"];
  node7 -> node8 [weight=1];
  node9 [label="CALL"];
  node8 -> node9 [weight=1];
  node10 [label="NAME(a)"];
  node9 -> node10 [weight=1];
  node8 -> node5 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node7 -> node8 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node11 [label="EMPTY"];
  node5 -> node11 [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node5 -> node7 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node0 -> node11 [weight=1];
  node11 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testThrow
#[test]
fn test_throw() {
    let mut compiler = Compiler::new();
    let src = r###"function f() { throw 1; f() }"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="FUNCTION"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(f)"];
  node1 -> node2 [weight=1];
  node3 [label="PARAM_LIST"];
  node1 -> node3 [weight=1];
  node4 [label="BLOCK"];
  node1 -> node4 [weight=1];
  node5 [label="THROW"];
  node4 -> node5 [weight=1];
  node6 [label="NUMBER"];
  node5 -> node6 [weight=1];
  node7 [label="EXPR_RESULT"];
  node4 -> node7 [weight=1];
  node8 [label="CALL"];
  node7 -> node8 [weight=1];
  node9 [label="NAME(f)"];
  node8 -> node9 [weight=1];
  node7 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 -> node5 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testSimpleFunction
#[test]
fn test_simple_function() {
    let mut compiler = Compiler::new();
    let src = r###"function f() { f() } f()"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="FUNCTION"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(f)"];
  node1 -> node2 [weight=1];
  node3 [label="PARAM_LIST"];
  node1 -> node3 [weight=1];
  node4 [label="BLOCK"];
  node1 -> node4 [weight=1];
  node5 [label="EXPR_RESULT"];
  node4 -> node5 [weight=1];
  node6 [label="CALL"];
  node5 -> node6 [weight=1];
  node7 [label="NAME(f)"];
  node6 -> node7 [weight=1];
  node5 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 -> node5 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node8 [label="EXPR_RESULT"];
  node0 -> node8 [weight=1];
  node9 [label="CALL"];
  node8 -> node9 [weight=1];
  node10 [label="NAME(f)"];
  node9 -> node10 [weight=1];
  node8 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node8 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testSimpleClass
#[test]
fn test_simple_class() {
    let mut compiler = Compiler::new();
    let src = r###"class C{} f();"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="CLASS"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(C)"];
  node1 -> node2 [weight=1];
  node3 [label="EMPTY"];
  node1 -> node3 [weight=1];
  node4 [label="CLASS_MEMBERS"];
  node1 -> node4 [weight=1];
  node5 [label="EXPR_RESULT"];
  node1 -> node5 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node5 [weight=1];
  node6 [label="CALL"];
  node5 -> node6 [weight=1];
  node7 [label="NAME(f)"];
  node6 -> node7 [weight=1];
  node5 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testClass_withPublicFieldsAndMethod
#[test]
fn test_class_with_public_fields_and_method() {
    let mut compiler = Compiler::new();
    let src = r###"class C{ x; y; foo() {}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_edge(
        &cfg,
        Token::FUNCTION,
        Token::BLOCK,
        Branch::UNCOND,
        &compiler,
    );
    assert_return_edge(&cfg, Token::BLOCK, &compiler);
}

// port: ControlFlowAnalysisTest#testClassWithMemberFunctions
#[test]
fn test_class_with_member_functions() {
    let mut compiler = Compiler::new();
    let src = r###"class C{ f(){} g(){} }"###;
    let expected_without_should_traverse_functions = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="CLASS"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(C)"];
  node1 -> node2 [weight=1];
  node3 [label="EMPTY"];
  node1 -> node3 [weight=1];
  node4 [label="CLASS_MEMBERS"];
  node1 -> node4 [weight=1];
  node5 [label="MEMBER_FUNCTION_DEF"];
  node4 -> node5 [weight=1];
  node6 [label="FUNCTION"];
  node5 -> node6 [weight=1];
  node7 [label="NAME"];
  node6 -> node7 [weight=1];
  node8 [label="PARAM_LIST"];
  node6 -> node8 [weight=1];
  node9 [label="BLOCK"];
  node6 -> node9 [weight=1];
  node10 [label="MEMBER_FUNCTION_DEF"];
  node4 -> node10 [weight=1];
  node11 [label="FUNCTION"];
  node10 -> node11 [weight=1];
  node12 [label="NAME"];
  node11 -> node12 [weight=1];
  node13 [label="PARAM_LIST"];
  node11 -> node13 [weight=1];
  node14 [label="BLOCK"];
  node11 -> node14 [weight=1];
  node1 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    let expected_with_should_traverse_functions = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="CLASS"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(C)"];
  node1 -> node2 [weight=1];
  node3 [label="EMPTY"];
  node1 -> node3 [weight=1];
  node4 [label="CLASS_MEMBERS"];
  node1 -> node4 [weight=1];
  node5 [label="MEMBER_FUNCTION_DEF"];
  node4 -> node5 [weight=1];
  node6 [label="FUNCTION"];
  node5 -> node6 [weight=1];
  node7 [label="NAME"];
  node6 -> node7 [weight=1];
  node8 [label="PARAM_LIST"];
  node6 -> node8 [weight=1];
  node9 [label="BLOCK"];
  node6 -> node9 [weight=1];
  node9 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node6 -> node9 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node10 [label="MEMBER_FUNCTION_DEF"];
  node4 -> node10 [weight=1];
  node11 [label="FUNCTION"];
  node10 -> node11 [weight=1];
  node12 [label="NAME"];
  node11 -> node12 [weight=1];
  node13 [label="PARAM_LIST"];
  node11 -> node13 [weight=1];
  node14 [label="BLOCK"];
  node11 -> node14 [weight=1];
  node14 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node11 -> node14 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected_with_should_traverse_functions);
    test_cfg_with_traverse_functions(
        &mut compiler,
        src,
        expected_without_should_traverse_functions,
        false,
    );
}

// port: ControlFlowAnalysisTest#testSimpleCatch
#[test]
fn test_simple_catch() {
    let mut compiler = Compiler::new();
    let src = r###"try{ throw x; x(); x['stuff']; x.x; x} catch (e) { e() }"###;

    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="TRY"];
  node0 -> node1 [weight=1];
  node2 [label="BLOCK"];
  node1 -> node2 [weight=1];
  node3 [label="THROW"];
  node2 -> node3 [weight=1];
  node4 [label="NAME(x)"];
  node3 -> node4 [weight=1];
  node5 [label="BLOCK"];
  node3 -> node5 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node6 [label="EXPR_RESULT"];
  node2 -> node6 [weight=1];
  node7 [label="CALL"];
  node6 -> node7 [weight=1];
  node8 [label="NAME(x)"];
  node7 -> node8 [weight=1];
  node9 [label="EXPR_RESULT"];
  node6 -> node5 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node6 -> node9 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node2 -> node9 [weight=1];
  node10 [label="GETELEM"];
  node9 -> node10 [weight=1];
  node11 [label="NAME(x)"];
  node10 -> node11 [weight=1];
  node12 [label="STRINGLIT(stuff)"];
  node10 -> node12 [weight=1];
  node13 [label="EXPR_RESULT"];
  node9 -> node13 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node9 -> node5 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node2 -> node13 [weight=1];
  node14 [label="GETPROP(x)"];
  node13 -> node14 [weight=1];
  node15 [label="NAME(x)"];
  node14 -> node15 [weight=1];
  node16 [label="EXPR_RESULT"];
  node13 -> node16 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node13 -> node5 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node2 -> node16 [weight=1];
  node17 [label="NAME(x)"];
  node16 -> node17 [weight=1];
  node16 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node2 -> node3 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node5 [weight=1];
  node18 [label="CATCH"];
  node5 -> node18 [weight=1];
  node19 [label="NAME(e)"];
  node18 -> node19 [weight=1];
  node20 [label="BLOCK"];
  node18 -> node20 [weight=1];
  node21 [label="EXPR_RESULT"];
  node20 -> node21 [weight=1];
  node22 [label="CALL"];
  node21 -> node22 [weight=1];
  node23 [label="NAME(e)"];
  node22 -> node23 [weight=1];
  node21 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node20 -> node21 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node18 -> node20 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node5 -> node18 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node2 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;

    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testFunctionWithinTry
#[test]
fn test_function_within_try() {
    let mut compiler = Compiler::new();

    let src = r###"try { var f = function() {throw 1;} } catch (e) { }"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="TRY"];
  node0 -> node1 [weight=1];
  node2 [label="BLOCK"];
  node1 -> node2 [weight=1];
  node3 [label="VAR"];
  node2 -> node3 [weight=1];
  node4 [label="NAME(f)"];
  node3 -> node4 [weight=1];
  node5 [label="FUNCTION"];
  node4 -> node5 [weight=1];
  node6 [label="NAME"];
  node5 -> node6 [weight=1];
  node7 [label="PARAM_LIST"];
  node5 -> node7 [weight=1];
  node8 [label="BLOCK"];
  node5 -> node8 [weight=1];
  node9 [label="THROW"];
  node8 -> node9 [weight=1];
  node10 [label="NUMBER"];
  node9 -> node10 [weight=1];
  node3 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node2 -> node3 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node11 [label="BLOCK"];
  node1 -> node11 [weight=1];
  node12 [label="CATCH"];
  node11 -> node12 [weight=1];
  node13 [label="NAME(e)"];
  node12 -> node13 [weight=1];
  node14 [label="BLOCK"];
  node12 -> node14 [weight=1];
  node14 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node12 -> node14 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node11 -> node12 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node2 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testNestedCatch
#[test]
fn test_nested_catch() {
    let mut compiler = Compiler::new();

    let src = r###"try{try{throw 1;}catch(e){throw 2}}catch(f){}"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="TRY"];
  node0 -> node1 [weight=1];
  node2 [label="BLOCK"];
  node1 -> node2 [weight=1];
  node3 [label="TRY"];
  node2 -> node3 [weight=1];
  node4 [label="BLOCK"];
  node3 -> node4 [weight=1];
  node5 [label="THROW"];
  node4 -> node5 [weight=1];
  node6 [label="NUMBER"];
  node5 -> node6 [weight=1];
  node7 [label="BLOCK"];
  node5 -> node7 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node4 -> node5 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node3 -> node7 [weight=1];
  node8 [label="CATCH"];
  node7 -> node8 [weight=1];
  node9 [label="NAME(e)"];
  node8 -> node9 [weight=1];
  node10 [label="BLOCK"];
  node8 -> node10 [weight=1];
  node11 [label="THROW"];
  node10 -> node11 [weight=1];
  node12 [label="NUMBER"];
  node11 -> node12 [weight=1];
  node13 [label="BLOCK"];
  node11 -> node13 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node10 -> node11 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node8 -> node10 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node7 -> node8 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node3 -> node4 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node2 -> node3 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node13 [weight=1];
  node14 [label="CATCH"];
  node13 -> node14 [weight=1];
  node15 [label="NAME(f)"];
  node14 -> node15 [weight=1];
  node16 [label="BLOCK"];
  node14 -> node16 [weight=1];
  node16 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node14 -> node16 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node13 -> node14 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node2 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testSimpleFinally
#[test]
fn test_simple_finally() {
    let mut compiler = Compiler::new();
    let src = r###"try{var x; foo()}finally{}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::TRY, Token::BLOCK, Branch::UNCOND, &compiler);
    assert_down_edge(&cfg, Token::BLOCK, Token::VAR, Branch::UNCOND, &compiler);

    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::BLOCK,
        Branch::UNCOND,
        &compiler,
    );

    assert_no_edge(&cfg, Token::BLOCK, Token::BLOCK, &compiler);
}

// port: ControlFlowAnalysisTest#testSimpleCatchFinally
#[test]
fn test_simple_catch_finally() {
    let mut compiler = Compiler::new();

    let src = r###"try{ if(a){throw 1}else{a} } catch(e){a}finally{a}"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="TRY"];
  node0 -> node1 [weight=1];
  node2 [label="BLOCK"];
  node1 -> node2 [weight=1];
  node3 [label="IF"];
  node2 -> node3 [weight=1];
  node4 [label="NAME(a)"];
  node3 -> node4 [weight=1];
  node5 [label="BLOCK"];
  node3 -> node5 [weight=1];
  node6 [label="THROW"];
  node5 -> node6 [weight=1];
  node7 [label="NUMBER"];
  node6 -> node7 [weight=1];
  node8 [label="BLOCK"];
  node6 -> node8 [label="ON_EX", fontcolor="red", weight=0.01, color="red"];
  node5 -> node6 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node9 [label="BLOCK"];
  node3 -> node9 [weight=1];
  node10 [label="EXPR_RESULT"];
  node9 -> node10 [weight=1];
  node11 [label="NAME(a)"];
  node10 -> node11 [weight=1];
  node12 [label="BLOCK"];
  node10 -> node12 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node9 -> node10 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node3 -> node5 [label="ON_TRUE", fontcolor="red", weight=0.01, color="red"];
  node3 -> node9 [label="ON_FALSE", fontcolor="red", weight=0.01, color="red"];
  node2 -> node3 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node8 [weight=1];
  node13 [label="CATCH"];
  node8 -> node13 [weight=1];
  node14 [label="NAME(e)"];
  node13 -> node14 [weight=1];
  node15 [label="BLOCK"];
  node13 -> node15 [weight=1];
  node16 [label="EXPR_RESULT"];
  node15 -> node16 [weight=1];
  node17 [label="NAME(a)"];
  node16 -> node17 [weight=1];
  node16 -> node12 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node15 -> node16 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node13 -> node15 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node8 -> node13 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node12 [weight=1];
  node18 [label="EXPR_RESULT"];
  node12 -> node18 [weight=1];
  node19 [label="NAME(a)"];
  node18 -> node19 [weight=1];
  node18 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node12 -> node18 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node1 -> node2 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testComplicatedFinally2
#[test]
fn test_complicated_finally2() {
    let mut compiler = Compiler::new();

    let src = r###"while(1){try{if(a){a;continue;}else if(b){b;break;} else if(c) throw 1; else a}catch(e){}finally{c()}bar}foo"###;

    let cfg = create_cfg(&mut compiler, src);

    assert_cross_edge(
        &cfg,
        Token::CONTINUE,
        Token::BLOCK,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(&cfg, Token::BREAK, Token::BLOCK, Branch::UNCOND, &compiler);
    assert_cross_edge(&cfg, Token::THROW, Token::BLOCK, Branch::ON_EX, &compiler);
}

// port: ControlFlowAnalysisTest#testDeepNestedBreakwithFinally
#[test]
fn test_deep_nested_breakwith_finally() {
    let mut compiler = Compiler::new();
    let src = r###"X:while(1){try{while(2){try{var a;break X;}finally{}}}finally{}}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_down_edge(&cfg, Token::WHILE, Token::BLOCK, Branch::ON_TRUE, &compiler);
    assert_down_edge(&cfg, Token::BLOCK, Token::TRY, Branch::UNCOND, &compiler);
    assert_down_edge(&cfg, Token::BLOCK, Token::VAR, Branch::UNCOND, &compiler);

    assert_cross_edge(&cfg, Token::BREAK, Token::BLOCK, Branch::UNCOND, &compiler);

    assert_cross_edge(&cfg, Token::BLOCK, Token::BLOCK, Branch::ON_EX, &compiler);
    assert_cross_edge(
        &cfg,
        Token::WHILE,
        Token::BLOCK,
        Branch::ON_FALSE,
        &compiler,
    );
    assert_return_edge(&cfg, Token::BLOCK, &compiler);
}

// port: ControlFlowAnalysisTest#testDeepNestedFinally
#[test]
fn test_deep_nested_finally() {
    let mut compiler = Compiler::new();
    let src = r###"try{try{try{throw 1}finally{1;var a}}finally{2;if(a);}}finally{3;a()}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(&cfg, Token::THROW, Token::BLOCK, Branch::ON_EX, &compiler);
    assert_cross_edge(&cfg, Token::VAR, Token::BLOCK, Branch::UNCOND, &compiler);
    assert_cross_edge(&cfg, Token::IF, Token::BLOCK, Branch::ON_EX, &compiler);
}

// port: ControlFlowAnalysisTest#testReturn
#[test]
fn test_return() {
    let mut compiler = Compiler::new();
    let src = r###"function f() { return; }"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_return_edge(&cfg, Token::RETURN, &compiler);
}

// port: ControlFlowAnalysisTest#testReturnInFinally
#[test]
fn test_return_in_finally() {
    let mut compiler = Compiler::new();
    let src = r###"function f(x){ try{} finally {return x;} }"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_return_edge(&cfg, Token::RETURN, &compiler);
}

// port: ControlFlowAnalysisTest#testReturnInFinally2
#[test]
fn test_return_in_finally2() {
    let mut compiler = Compiler::new();
    let src = r###"function f(x){ try{ try{}finally{var dummy; return x;} } finally {} }"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(&cfg, Token::VAR, Token::RETURN, Branch::UNCOND, &compiler);
    assert_cross_edge(&cfg, Token::RETURN, Token::BLOCK, Branch::UNCOND, &compiler);
    assert_return_edge(&cfg, Token::BLOCK, &compiler);
    assert_no_return_edge(&cfg, Token::RETURN, &compiler);
}

// port: ControlFlowAnalysisTest#testReturnInTry
#[test]
fn test_return_in_try() {
    let mut compiler = Compiler::new();
    let src = r###"function f(x){ try{x; return x()} finally {} var y;}"###;
    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::RETURN,
        Branch::UNCOND,
        &compiler,
    );
    assert_cross_edge(&cfg, Token::RETURN, Token::BLOCK, Branch::UNCOND, &compiler);
    assert_cross_edge(&cfg, Token::BLOCK, Token::VAR, Branch::UNCOND, &compiler);
    assert_return_edge(&cfg, Token::VAR, &compiler);
    assert_return_edge(&cfg, Token::BLOCK, &compiler);
    assert_no_return_edge(&cfg, Token::RETURN, &compiler);
}

// port: ControlFlowAnalysisTest#testOptionNotToTraverseFunctions
#[test]
fn test_option_not_to_traverse_functions() {
    let mut compiler = Compiler::new();
    let src = r###"var x = 1; function f() { x = null; }"###;
    let expected_when_not_traversing_functions = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="VAR"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(x)"];
  node1 -> node2 [weight=1];
  node3 [label="NUMBER"];
  node2 -> node3 [weight=1];
  node1 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 [label="FUNCTION"];
  node0 -> node4 [weight=1];
  node5 [label="NAME(f)"];
  node4 -> node5 [weight=1];
  node6 [label="PARAM_LIST"];
  node4 -> node6 [weight=1];
  node7 [label="BLOCK"];
  node4 -> node7 [weight=1];
  node8 [label="EXPR_RESULT"];
  node7 -> node8 [weight=1];
  node9 [label="ASSIGN"];
  node8 -> node9 [weight=1];
  node10 [label="NAME(x)"];
  node9 -> node10 [weight=1];
  node11 [label="NULL"];
  node9 -> node11 [weight=1];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="VAR"];
  node0 -> node1 [weight=1];
  node2 [label="NAME(x)"];
  node1 -> node2 [weight=1];
  node3 [label="NUMBER"];
  node2 -> node3 [weight=1];
  node1 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 [label="FUNCTION"];
  node0 -> node4 [weight=1];
  node5 [label="NAME(f)"];
  node4 -> node5 [weight=1];
  node6 [label="PARAM_LIST"];
  node4 -> node6 [weight=1];
  node7 [label="BLOCK"];
  node4 -> node7 [weight=1];
  node8 [label="EXPR_RESULT"];
  node7 -> node8 [weight=1];
  node9 [label="ASSIGN"];
  node8 -> node9 [weight=1];
  node10 [label="NAME(x)"];
  node9 -> node10 [weight=1];
  node11 [label="NULL"];
  node9 -> node11 [weight=1];
  node8 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node7 -> node8 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node4 -> node7 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
    test_cfg_with_traverse_functions(
        &mut compiler,
        src,
        expected_when_not_traversing_functions,
        false,
    );
}

// port: ControlFlowAnalysisTest#testInstanceOf
#[test]
fn test_instance_of() {
    let mut compiler = Compiler::new();
    let src = "try { x instanceof 'x' } catch (e) { }";
    let cfg = create_cfg_with_syn_block_pass(&mut compiler, src, true);
    assert_cross_edge(
        &cfg,
        Token::EXPR_RESULT,
        Token::BLOCK,
        Branch::ON_EX,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSynBlock
#[test]
fn test_syn_block() {
    let mut compiler = Compiler::new();
    let src = "START(); var x; END(); var y;";
    let cfg = create_cfg_with_syn_block_pass(&mut compiler, src, true);
    assert_cross_edge(
        &cfg,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::SYN_BLOCK,
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testPartialTraversalOfScope
#[test]
fn test_partial_traversal_of_scope() {
    let mut compiler = Compiler::new();

    let script1 = compiler.parse_synthetic_code(r###"cfgtest"###, r###"var foo;"###);
    let script2 = compiler.parse_synthetic_code(r###"cfgtest2"###, r###"var bar;"###);

    compiler.new_node_with_children2(Token::BLOCK, script1, script2);

    let cfg = ControlFlowAnalysis::builder()
        .set_cfg_root(script1)
        .set_traverse_functions(true)
        .set_include_edge_annotations(true)
        .compute_cfg(&mut compiler);

    assert!(cfg.get_node(&Some(script1)).is_some());
    assert!(cfg.get_node(&Some(script2)).is_none());
}

// port: ControlFlowAnalysisTest#testForLoopOrder
#[test]
fn test_for_loop_order() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"for (var i = 0; i < 5; i++) { var x = 3; } if (true) {}"###,
        ),
        &[
            Token::SCRIPT,
            Token::VAR,
            Token::FOR,
            Token::BLOCK,
            Token::VAR,
            Token::INC,
            Token::IF,
            Token::BLOCK,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testLabelledForInLoopOrder
#[test]
fn test_labelled_for_in_loop_order() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"var i = 0; var y = {}; label: for (var x in y) {     if (x) { break label; } else { i++ } x(); }"###,
        ),
        &[
            Token::SCRIPT,
            Token::VAR,
            Token::VAR,
            Token::NAME,
            Token::FOR_IN,
            Token::BLOCK,
            Token::IF,
            Token::BLOCK,
            Token::BREAK,
            Token::BLOCK,
            Token::EXPR_RESULT,
            Token::EXPR_RESULT,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSimpleExportDeclarationsInEsModule
#[test]
fn test_simple_export_declarations_in_es_module() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"export let a = 0; export default a; export {b} from './mod';"###,
        ),
        &[
            Token::SCRIPT,
            Token::MODULE_BODY,
            Token::EXPORT,
            Token::EXPORT,
            Token::EXPORT,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testSimpleImportDeclarationsInEsModule
#[test]
fn test_simple_import_declarations_in_es_module() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"import x from './mod'; import {y} from './mod'; import * as z from './mod';"###,
        ),
        &[
            Token::SCRIPT,
            Token::MODULE_BODY,
            Token::IMPORT,
            Token::IMPORT,
            Token::IMPORT,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testLocalFunctionOrder
#[test]
fn test_local_function_order() {
    let mut compiler = Compiler::new();
    let cfg = create_cfg(
        &mut compiler,
        r###"function f() { while (x) { x++; } } var x = 3;"###,
    );
    assert_node_order(
        cfg,
        &[
            Token::SCRIPT,
            Token::VAR,
            Token::FUNCTION,
            Token::BLOCK,
            Token::WHILE,
            Token::BLOCK,
            Token::EXPR_RESULT,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testDoWhileOrder
#[test]
fn test_do_while_order() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"do { var x = 3; } while (true); void x;"###,
        ),
        &[
            Token::SCRIPT,
            Token::BLOCK,
            Token::VAR,
            Token::DO,
            Token::EXPR_RESULT,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testForOfOrder
#[test]
fn test_for_of_order() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"async function f() { for (x of y) { z; } return 0; }"###,
        ),
        &[
            Token::SCRIPT,
            Token::FUNCTION,
            Token::BLOCK,
            Token::NAME,
            Token::FOR_OF,
            Token::BLOCK,
            Token::EXPR_RESULT,
            Token::RETURN,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testForAwaitOfOrder
#[test]
fn test_for_await_of_order() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"async function f() { for await (x of y) { z; } return 0; }"###,
        ),
        &[
            Token::SCRIPT,
            Token::FUNCTION,
            Token::BLOCK,
            Token::NAME,
            Token::FOR_AWAIT_OF,
            Token::BLOCK,
            Token::EXPR_RESULT,
            Token::RETURN,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testForAwaitOfOrderBreakAndContinue
#[test]
fn test_for_await_of_order_break_and_continue() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"async function f() {
  outer: for await (let x of y) {
    inner: for await (let z of x) {
      if (z) break inner;
      else continue outer;
    }
  }
  return 0;
}
"###,
        ),
        &[
            Token::SCRIPT,
            Token::FUNCTION,
            Token::BLOCK,
            Token::NAME,
            Token::FOR_AWAIT_OF,
            Token::BLOCK,
            Token::NAME,
            Token::FOR_AWAIT_OF,
            Token::BLOCK,
            Token::IF,
            Token::BLOCK,
            Token::BREAK,
            Token::BLOCK,
            Token::CONTINUE,
            Token::RETURN,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testForAwaitOfOrderBreakAndContinueAndYield
#[test]
fn test_for_await_of_order_break_and_continue_and_yield() {
    let mut compiler = Compiler::new();
    assert_node_order(
        create_cfg(
            &mut compiler,
            r###"async function* f() {
  outer: for await (let x of y) {
    inner: for await (let z of x) {
      if (z > 0) break inner;
      else if (z < 0) continue outer;
      yield z;
    }
  }
}
"###,
        ),
        &[
            Token::SCRIPT,
            Token::FUNCTION,
            Token::BLOCK,
            Token::NAME,
            Token::FOR_AWAIT_OF,
            Token::BLOCK,
            Token::NAME,
            Token::FOR_AWAIT_OF,
            Token::BLOCK,
            Token::IF,
            Token::BLOCK,
            Token::BREAK,
            Token::BLOCK,
            Token::IF,
            Token::BLOCK,
            Token::CONTINUE,
            Token::EXPR_RESULT,
        ],
        &compiler,
    );
}

// port: ControlFlowAnalysisTest#testBreakInFinally1
#[test]
fn test_break_in_finally1() {
    let mut compiler = Compiler::new();
    let src = r###"f = function() {
  var action;
  a: {
    var proto = null;
    try {
      proto = new Proto
    } finally {
      action = proto;
      break a // Remove this...
    }
  }
  alert(action) // but not this.
};
"###;
    let expected = r###"digraph AST {
  node [color=lightblue2, style=filled];
  node0 [label="SCRIPT"];
  node1 [label="EXPR_RESULT"];
  node0 -> node1 [weight=1];
  node2 [label="ASSIGN"];
  node1 -> node2 [weight=1];
  node3 [label="NAME(f)"];
  node2 -> node3 [weight=1];
  node4 [label="FUNCTION"];
  node2 -> node4 [weight=1];
  node5 [label="NAME"];
  node4 -> node5 [weight=1];
  node6 [label="PARAM_LIST"];
  node4 -> node6 [weight=1];
  node7 [label="BLOCK"];
  node4 -> node7 [weight=1];
  node8 [label="VAR"];
  node7 -> node8 [weight=1];
  node9 [label="NAME(action)"];
  node8 -> node9 [weight=1];
  node10 [label="LABEL"];
  node7 -> node10 [weight=1];
  node11 [label="LABEL_NAME"];
  node10 -> node11 [weight=1];
  node12 [label="BLOCK"];
  node10 -> node12 [weight=1];
  node13 [label="VAR"];
  node12 -> node13 [weight=1];
  node14 [label="NAME(proto)"];
  node13 -> node14 [weight=1];
  node15 [label="NULL"];
  node14 -> node15 [weight=1];
  node16 [label="TRY"];
  node12 -> node16 [weight=1];
  node17 [label="BLOCK"];
  node16 -> node17 [weight=1];
  node18 [label="EXPR_RESULT"];
  node17 -> node18 [weight=1];
  node19 [label="ASSIGN"];
  node18 -> node19 [weight=1];
  node20 [label="NAME(proto)"];
  node19 -> node20 [weight=1];
  node21 [label="NEW"];
  node19 -> node21 [weight=1];
  node22 [label="NAME(Proto)"];
  node21 -> node22 [weight=1];
  node23 [label="BLOCK"];
  node16 -> node23 [weight=1];
  node24 [label="BLOCK"];
  node16 -> node24 [weight=1];
  node25 [label="EXPR_RESULT"];
  node24 -> node25 [weight=1];
  node26 [label="ASSIGN"];
  node25 -> node26 [weight=1];
  node27 [label="NAME(action)"];
  node26 -> node27 [weight=1];
  node28 [label="NAME(proto)"];
  node26 -> node28 [weight=1];
  node29 [label="BREAK"];
  node24 -> node29 [weight=1];
  node30 [label="LABEL_NAME"];
  node29 -> node30 [weight=1];
  node31 [label="EXPR_RESULT"];
  node7 -> node31 [weight=1];
  node32 [label="CALL"];
  node31 -> node32 [weight=1];
  node33 [label="NAME(alert)"];
  node32 -> node33 [weight=1];
  node34 [label="NAME(action)"];
  node32 -> node34 [weight=1];
  node1 -> RETURN [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
  node0 -> node1 [label="UNCOND", fontcolor="red", weight=0.01, color="red"];
}
"###;
    test_cfg(&mut compiler, src, expected);
}

// port: ControlFlowAnalysisTest#testBreakInFinally2
#[test]
fn test_break_in_finally2() {
    let mut compiler = Compiler::new();
    let src = r###"var action;
a: {
  var proto = null;
  try {
    proto = new Proto
  } finally {
    action = proto;
    break a
  }
}
alert(action)
"###;

    let cfg = create_cfg(&mut compiler, src);
    assert_cross_edge(
        &cfg,
        Token::BREAK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_no_edge(&cfg, Token::BREAK, Token::BLOCK, &compiler);
}

use closure_jscomp::node_util::NodeUtil;

// port: ControlFlowAnalysisTest#testCfgRootedAtEmptyFunctionDeclaration
#[test]
fn test_cfg_rooted_at_empty_function_declaration() {
    let mut compiler = Compiler::new();
    let src = r###"function fn() {}"###;

    let global_root = compiler.parse_synthetic_code(r###"cfgtest"###, src);
    let fn_root = closure_jscomp::testing::code_sub_tree::CodeSubTree::find_first_node(
        &mut compiler,
        global_root,
        |ast, n| n.is_function(ast),
    );

    let cfg = ControlFlowAnalysis::builder()
        .set_cfg_root(fn_root)
        .compute_cfg(&mut compiler);

    assert_down_edge(
        &cfg,
        Token::FUNCTION,
        Token::BLOCK,
        Branch::UNCOND,
        &compiler,
    );
    assert_return_edge(&cfg, Token::BLOCK, &compiler);
}

// port: ControlFlowAnalysisTest#testCfgRootedAtFunctionDeclarationWithSingleStatement
#[test]
fn test_cfg_rooted_at_function_declaration_with_single_statement() {
    let mut compiler = Compiler::new();
    let src = r###"function fn() {  alert(3); }"###;

    let global_root = compiler.parse_synthetic_code(r###"cfgtest"###, src);
    let fn_root = closure_jscomp::testing::code_sub_tree::CodeSubTree::find_first_node(
        &mut compiler,
        global_root,
        |ast, n| n.is_function(ast),
    );

    let cfg = ControlFlowAnalysis::builder()
        .set_cfg_root(fn_root)
        .compute_cfg(&mut compiler);

    assert_down_edge(
        &cfg,
        Token::FUNCTION,
        Token::BLOCK,
        Branch::UNCOND,
        &compiler,
    );
    assert_down_edge(
        &cfg,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_return_edge(&cfg, Token::EXPR_RESULT, &compiler);
}

// port: ControlFlowAnalysisTest#testCfgRootedAtEmptyClassStaticBlock
#[test]
fn test_cfg_rooted_at_empty_class_static_block() {
    let mut compiler = Compiler::new();
    let src = r###"class C { static {} }"###;

    let global_root = compiler.parse_synthetic_code(r###"cfgtest"###, src);
    let static_block = closure_jscomp::testing::code_sub_tree::CodeSubTree::find_first_node(
        &mut compiler,
        global_root,
        |ast, n| n.is_block(ast),
    );

    let cfg = ControlFlowAnalysis::builder()
        .set_cfg_root(static_block)
        .compute_cfg(&mut compiler);

    assert_return_edge(&cfg, Token::BLOCK, &compiler);
}

// port: ControlFlowAnalysisTest#testCfgRootedAtClassStaticBlockSingleStatementAndSubsequentMembers
#[test]
fn test_cfg_rooted_at_class_static_block_single_statement_and_subsequent_members() {
    let mut compiler = Compiler::new();
    let src = r###"class C { static { alert(0); } x = 0; fn() {} }"###;

    let global_root = compiler.parse_synthetic_code(r###"cfgtest"###, src);
    let static_block = closure_jscomp::testing::code_sub_tree::CodeSubTree::find_first_node(
        &mut compiler,
        global_root,
        |ast, n| n.is_block(ast),
    );

    let cfg = ControlFlowAnalysis::builder()
        .set_cfg_root(static_block)
        .compute_cfg(&mut compiler);

    assert_down_edge(
        &cfg,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );

    assert_no_edge(&cfg, Token::EXPR_RESULT, Token::MEMBER_FIELD_DEF, &compiler);
    assert_return_edge(&cfg, Token::EXPR_RESULT, &compiler);
}

// port: ControlFlowAnalysisTest#testCfgRootedAtClassStaticBlockMultipleStatementAndSubsequentMembers
#[test]
fn test_cfg_rooted_at_class_static_block_multiple_statement_and_subsequent_members() {
    let mut compiler = Compiler::new();
    let src = r###"class C {
  static { alert(0); }
  x = 0;
  [0+1]() {}
  static { alert(1); }
  fn(){}
}
"###;

    let global_root = compiler.parse_synthetic_code(r###"cfgtest"###, src);
    let static_blocks = closure_jscomp::testing::code_sub_tree::CodeSubTree::find_nodes_non_empty(
        &mut compiler,
        global_root,
        NodeUtil::is_class_static_block,
    );
    assert_eq!(static_blocks.len(), 2);
    let static_block1 = static_blocks[0];
    let static_block2 = static_blocks[1];

    let cfg1 = ControlFlowAnalysis::builder()
        .set_cfg_root(static_block1)
        .compute_cfg(&mut compiler);

    assert_down_edge(
        &cfg1,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_no_edge(
        &cfg1,
        Token::EXPR_RESULT,
        Token::MEMBER_FIELD_DEF,
        &compiler,
    );
    assert_return_edge(&cfg1, Token::EXPR_RESULT, &compiler);

    assert_no_edge(&cfg1, Token::COMPUTED_PROP, Token::BLOCK, &compiler);

    let cfg2 = ControlFlowAnalysis::builder()
        .set_cfg_root(static_block2)
        .compute_cfg(&mut compiler);

    assert_down_edge(
        &cfg2,
        Token::BLOCK,
        Token::EXPR_RESULT,
        Branch::UNCOND,
        &compiler,
    );
    assert_no_edge(
        &cfg2,
        Token::EXPR_RESULT,
        Token::MEMBER_FIELD_DEF,
        &compiler,
    );
    assert_return_edge(&cfg2, Token::EXPR_RESULT, &compiler);
}

// Rust-only regression (type-inference-cfg-prioritize-positions): TypeInferencePass computes the
// CFG of an arrow whose expression body is another arrow without traversing functions. The inner
// FUNCTION is a CFG node (handleFunction's edge to the body) but has no astPosition; Java's
// PriorityQueue never compares it (it is always alone in the worklist), so no checkNotNull fires.
fn outer_arrow_cfg(compiler: &mut Compiler, input: &str) -> (NodeId, ControlFlowGraph<NodeId>) {
    let root = compiler.parse_synthetic_code("cfgtest", input);
    let mut outer = root;
    while !outer.is_function(compiler) {
        outer = outer.get_first_child(compiler).unwrap();
    }
    let cfg = ControlFlowAnalysis::builder()
        .set_cfg_root(outer)
        .set_include_edge_annotations(true)
        .compute_cfg(compiler);
    (outer, cfg)
}

#[test]
fn test_nesting_arrow_cfg_root() {
    let mut compiler = Compiler::new();
    let (outer, cfg) = outer_arrow_cfg(&mut compiler, "var f = x =>\n y => x+y;");
    assert_down_edge(
        &cfg,
        Token::FUNCTION,
        Token::FUNCTION,
        Branch::UNCOND,
        &compiler,
    );
    let inner = outer.get_last_child(&compiler).unwrap();
    assert!(inner.is_arrow_function(&compiler));
    let outer_node = cfg.get_node(&Some(outer)).unwrap();
    let inner_node = cfg.get_node(&Some(inner)).unwrap();
    assert_eq!(outer_node.get_priority(&cfg), 1);
    assert_eq!(inner_node.get_priority(&cfg), 2);
    assert_eq!(cfg.get_implicit_return().get_priority(&cfg), 4);
}

#[test]
fn test_async_arrow_returning_arrow_cfg_root() {
    for src in [
        "class X { m() { return async () => (() => this); } }",
        "const f = async () => (() => new.target);",
    ] {
        let mut compiler = Compiler::new();
        let root = compiler.parse_synthetic_code("cfgtest", src);
        let mut arrows = Vec::new();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if n.is_arrow_function(&compiler) && n.is_async_function(&compiler) {
                arrows.push(n);
            }
            let mut c = n.get_first_child(&compiler);
            while let Some(child) = c {
                stack.push(child);
                c = child.get_next(&compiler);
            }
        }
        assert_eq!(arrows.len(), 1, "{src}");
        let cfg = ControlFlowAnalysis::builder()
            .set_cfg_root(arrows[0])
            .set_include_edge_annotations(true)
            .compute_cfg(&mut compiler);
        let inner = arrows[0].get_last_child(&compiler).unwrap();
        assert!(
            cfg.get_node(&Some(inner)).unwrap().get_priority(&cfg) == 2,
            "{src}"
        );
    }
}
