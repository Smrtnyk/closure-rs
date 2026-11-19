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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   test/com/google/javascript/jscomp/EvaluationOrderTest.java.

//! Port of `EvaluationOrderTest.java`.

use closure_jscomp::compiler::Compiler;
use closure_jscomp::evaluation_order::EvaluationOrder;
use closure_jscomp::node_traversal::NodeTraversal;
use closure_jscomp::scope::ScopeId;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct EvaluationOrderTest {
    compiler: Compiler,
}

/// Java's `record ScopedNode(Node node, Scope scope)`.
#[derive(Clone, Copy)]
struct ScopedNode {
    node: NodeId,
    scope: ScopeId,
}

impl EvaluationOrderTest {
    // port: EvaluationOrderTest#setUp
    fn set_up() -> Self {
        Self {
            compiler: Compiler::new(),
        }
    }

    // port: EvaluationOrderTest#executesAfterInitTestCase
    fn executes_after_init_test_case(&mut self, js: &str) -> bool {
        let script = self.parse(js);
        let nodes = self.get_scoped_name_nodes(script);
        let init = *nodes
            .get(&JsString::from("init"))
            .unwrap_or_else(|| panic!("No 'init' node in: {js}"));
        let r#ref = *nodes
            .get(&JsString::from("ref"))
            .unwrap_or_else(|| panic!("No 'ref' node in: {js}"));
        EvaluationOrder::check_execution_after(&self.compiler, init.node, init.scope)
            .is_executed_after_init(&self.compiler, r#ref.node, r#ref.scope)
    }

    // port: EvaluationOrderTest#parse
    fn parse(&mut self, js: &str) -> NodeId {
        let script = self.compiler.parse_test_code(js);
        assert!(
            self.compiler.get_errors().is_empty(),
            "{:?}",
            self.compiler.get_errors()
        );
        script
    }

    // port: EvaluationOrderTest#parseExpr
    fn parse_expr(&mut self, js_expr: &str) -> NodeId {
        let script = self.parse(&format!("({js_expr});"));
        script.get_first_first_child(&self.compiler).unwrap()
    }

    // port: EvaluationOrderTest#firstOf
    fn first_of(&mut self, js: &str, token: Token) -> NodeId {
        let root = self.parse(js);
        self.find_first(root, token)
    }

    // port: EvaluationOrderTest#findFirst
    fn find_first(&self, root: NodeId, token: Token) -> NodeId {
        let found = self.find_first_or_null(root, token);
        found.unwrap_or_else(|| panic!("No {token} in {}", root.to_string(&self.compiler)))
    }

    // port: EvaluationOrderTest#findFirstOrNull
    fn find_first_or_null(&self, root: NodeId, token: Token) -> Option<NodeId> {
        if root.get_token(&self.compiler) == token {
            return Some(root);
        }
        let mut child = root.get_first_child(&self.compiler);
        while let Some(c) = child {
            let found = self.find_first_or_null(c, token);
            if found.is_some() {
                return found;
            }
            child = c.get_next(&self.compiler);
        }
        None
    }

    // port: EvaluationOrderTest#getScopedNameNodes
    fn get_scoped_name_nodes(&mut self, script: NodeId) -> IndexMap<JsString, ScopedNode> {
        let mut found: IndexMap<JsString, ScopedNode> = IndexMap::default();
        let mut cb = |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| {
            if n.is_name(t) {
                let name = n.get_string(t);
                if !found.contains_key(&name) {
                    let scope = t.get_scope();
                    found.insert(name, ScopedNode { node: n, scope });
                }
            }
        };
        NodeTraversal::builder()
            .set_compiler(&mut self.compiler)
            .set_callback_post_order(&mut cb)
            .traverse(script);
        found
    }
}

// port: EvaluationOrderTest#isConditionalChild_shortCircuitAndHook
#[test]
fn is_conditional_child_short_circuit_and_hook() {
    let mut t = EvaluationOrderTest::set_up();
    for op in ["&&", "||", "??", "&&=", "||=", "??="] {
        let n = t.parse_expr(&format!("a {op} b"));
        let c = &t.compiler;
        assert!(!EvaluationOrder::is_conditional_child(
            c,
            n.get_first_child(c).unwrap()
        ));
        assert!(EvaluationOrder::is_conditional_child(
            c,
            n.get_second_child(c).unwrap()
        ));
    }

    let hook = t.parse_expr("a ? b : c");
    let c = &t.compiler;
    assert!(!EvaluationOrder::is_conditional_child(
        c,
        hook.get_first_child(c).unwrap()
    ));
    assert!(EvaluationOrder::is_conditional_child(
        c,
        hook.get_second_child(c).unwrap()
    ));
    assert!(EvaluationOrder::is_conditional_child(
        c,
        hook.get_last_child(c).unwrap()
    ));
}

// port: EvaluationOrderTest#isConditionalChild_optionalChains
#[test]
fn is_conditional_child_optional_chains() {
    let mut t = EvaluationOrderTest::set_up();
    let call = t.parse_expr("a?.(b, c)");
    let c = &t.compiler;
    assert_eq!(call.get_token(c), Token::OPTCHAIN_CALL);
    assert!(!EvaluationOrder::is_conditional_child(
        c,
        call.get_first_child(c).unwrap()
    ));
    assert!(EvaluationOrder::is_conditional_child(
        c,
        call.get_second_child(c).unwrap()
    ));
    assert!(EvaluationOrder::is_conditional_child(
        c,
        call.get_last_child(c).unwrap()
    ));

    let get_elem = t.parse_expr("a?.[b]");
    let c = &t.compiler;
    assert_eq!(get_elem.get_token(c), Token::OPTCHAIN_GETELEM);
    assert!(!EvaluationOrder::is_conditional_child(
        c,
        get_elem.get_first_child(c).unwrap()
    ));
    assert!(EvaluationOrder::is_conditional_child(
        c,
        get_elem.get_second_child(c).unwrap()
    ));

    let get_prop = t.parse_expr("a?.b");
    let c = &t.compiler;
    assert!(!EvaluationOrder::is_conditional_child(
        c,
        get_prop.get_first_child(c).unwrap()
    ));
}

// port: EvaluationOrderTest#isConditionalChild_defaultValue
#[test]
fn is_conditional_child_default_value() {
    let mut t = EvaluationOrderTest::set_up();
    let root = t.parse("[a = b] = arr;");
    let default_value = t.find_first(root, Token::DEFAULT_VALUE);
    let c = &t.compiler;
    assert!(!EvaluationOrder::is_conditional_child(
        c,
        default_value.get_first_child(c).unwrap()
    ));
    assert!(EvaluationOrder::is_conditional_child(
        c,
        default_value.get_second_child(c).unwrap()
    ));

    let root = t.parse("function f(a = b) {}");
    let param_default = t.find_first(root, Token::DEFAULT_VALUE);
    let c = &t.compiler;
    assert!(EvaluationOrder::is_conditional_child(
        c,
        param_default.get_second_child(c).unwrap()
    ));
}

// port: EvaluationOrderTest#isConditionalChild_unconditionalChildren
#[test]
fn is_conditional_child_unconditional_children() {
    let mut t = EvaluationOrderTest::set_up();
    let call = t.parse_expr("f(a, b)");
    let c = &t.compiler;
    let mut child = call.get_first_child(c);
    while let Some(ch) = child {
        assert!(!EvaluationOrder::is_conditional_child(c, ch));
        child = ch.get_next(c);
    }
    let add = t.parse_expr("a + b");
    let c = &t.compiler;
    assert!(!EvaluationOrder::is_conditional_child(
        c,
        add.get_second_child(c).unwrap()
    ));

    // Statement-level control flow is intentionally out of scope.
    let if_node = t
        .parse("if (a) { b(); } else { c(); }")
        .get_first_child(&t.compiler)
        .unwrap();
    let c = &t.compiler;
    assert!(!EvaluationOrder::is_conditional_child(
        c,
        if_node.get_second_child(c).unwrap()
    ));
    assert!(!EvaluationOrder::is_conditional_child(
        c,
        if_node.get_last_child(c).unwrap()
    ));

    let detached = IR::name(&mut t.compiler, "detached");
    assert!(!EvaluationOrder::is_conditional_child(
        &t.compiler,
        detached
    ));
}

// port: EvaluationOrderTest#evaluatesLastChildFirst
#[test]
fn evaluates_last_child_first() {
    let mut t = EvaluationOrderTest::set_up();
    let mut check = |js: &str, token: Token, expected: bool| {
        let n = t.first_of(js, token);
        assert_eq!(
            EvaluationOrder::evaluates_last_child_first(&t.compiler, n),
            expected,
            "{js}"
        );
    };
    check("const [a] = rhs;", Token::DESTRUCTURING_LHS, true);
    check("const {a} = rhs;", Token::DESTRUCTURING_LHS, true);
    // No initializer.
    check("for (const [a] of arr) {}", Token::DESTRUCTURING_LHS, false);

    check("[a] = rhs;", Token::ASSIGN, true);
    check("({a} = rhs);", Token::ASSIGN, true);
    check("a = rhs;", Token::ASSIGN, false);
    check("o.p = rhs;", Token::ASSIGN, false);

    check("[a = d] = arr;", Token::DEFAULT_VALUE, true);
    check("[[a] = d] = arr;", Token::DEFAULT_VALUE, true);
    check("function f(a = d) {}", Token::DEFAULT_VALUE, true);
    // Property targets evaluate the target reference before the default value.
    check("[o.p = d] = arr;", Token::DEFAULT_VALUE, false);
    check("[o[k] = d] = arr;", Token::DEFAULT_VALUE, false);

    let call = t.parse_expr("f(a, b)");
    assert!(!EvaluationOrder::evaluates_last_child_first(
        &t.compiler,
        call
    ));
}

// port: EvaluationOrderTest#isEvaluatedBeforeSibling
#[test]
fn is_evaluated_before_sibling() {
    let mut t = EvaluationOrderTest::set_up();
    let call = t.parse_expr("f(a, b)");
    let c = &t.compiler;
    let f = call.get_first_child(c).unwrap();
    let a = f.get_next(c).unwrap();
    let b = a.get_next(c).unwrap();
    assert!(EvaluationOrder::is_evaluated_before_sibling(c, f, b));
    assert!(EvaluationOrder::is_evaluated_before_sibling(c, a, b));
    assert!(!EvaluationOrder::is_evaluated_before_sibling(c, b, a));
    assert!(!EvaluationOrder::is_evaluated_before_sibling(c, a, a));

    let assign = t.first_of("[x] = rhs;", Token::ASSIGN);
    let c = &t.compiler;
    assert!(EvaluationOrder::is_evaluated_before_sibling(
        c,
        assign.get_last_child(c).unwrap(),
        assign.get_first_child(c).unwrap()
    ));
    assert!(!EvaluationOrder::is_evaluated_before_sibling(
        c,
        assign.get_first_child(c).unwrap(),
        assign.get_last_child(c).unwrap()
    ));

    let member_default = t.first_of("[o[k] = d] = arr;", Token::DEFAULT_VALUE);
    let c = &t.compiler;
    assert!(EvaluationOrder::is_evaluated_before_sibling(
        c,
        member_default.get_first_child(c).unwrap(),
        member_default.get_last_child(c).unwrap()
    ));

    // assertThrows(IllegalArgumentException.class, ...)
    let c = &t.compiler;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            EvaluationOrder::is_evaluated_before_sibling(c, call, a)
        }))
        .is_err()
    );
}

// port: EvaluationOrderTest#isExecutedAfterInit_sameNodeReturnsFalse
#[test]
fn is_executed_after_init_same_node_returns_false() {
    let mut t = EvaluationOrderTest::set_up();
    let script = t.parse("if (c) { init; }");
    let init = *t
        .get_scoped_name_nodes(script)
        .get(&JsString::from("init"))
        .unwrap();

    assert!(
        !EvaluationOrder::check_execution_after(&t.compiler, init.node, init.scope)
            .is_executed_after_init(&t.compiler, init.node, init.scope)
    );
}

// port: EvaluationOrderTest#isExecutedAfterInit_sequentialStatements
#[test]
fn is_executed_after_init_sequential_statements() {
    let mut t = EvaluationOrderTest::set_up();
    assert!(t.executes_after_init_test_case("init; ref;"));
    assert!(!t.executes_after_init_test_case("ref; init;"));
    assert!(t.executes_after_init_test_case("if (c) { init; alert(1); use(ref); }"));
    assert!(!t.executes_after_init_test_case("if (c) { use(ref); alert(1); init; }"));
    assert!(t.executes_after_init_test_case("if (c) { init; if (inner) { use(ref); } }"));
    assert!(!t.executes_after_init_test_case("if (c) { inner && init; alert(1); use(ref); }"));
    assert!(!t.executes_after_init_test_case("if (c) { if (inner) { init; } use(ref); }"));
}

// port: EvaluationOrderTest#isExecutedAfterInit_shortCircuitAndTernaryExpressions
#[test]
fn is_executed_after_init_short_circuit_and_ternary_expressions() {
    let mut t = EvaluationOrderTest::set_up();
    assert!(t.executes_after_init_test_case("init && ref;"));
    assert!(t.executes_after_init_test_case("init || ref;"));
    assert!(t.executes_after_init_test_case("init ?? ref;"));
    assert!(t.executes_after_init_test_case("c && (init.prop ?? (ref.prop = 1));"));
    assert!(!t.executes_after_init_test_case("(c && init) || ref;"));

    assert!(t.executes_after_init_test_case("init ? ref : 1;"));
    assert!(t.executes_after_init_test_case("init ? 1 : ref;"));
    assert!(!t.executes_after_init_test_case("c ? init : ref;"));
    assert!(!t.executes_after_init_test_case("c ? ref : init;"));

    assert!(t.executes_after_init_test_case("init.x ||= ref.y;"));
    assert!(t.executes_after_init_test_case("init.x &&= ref.y;"));
    assert!(t.executes_after_init_test_case("init.x ??= ref.y;"));
    assert!(!t.executes_after_init_test_case("if (c) { a ||= init; use(ref); }"));
    assert!(!t.executes_after_init_test_case("if (c) { a &&= init; use(ref); }"));
    assert!(!t.executes_after_init_test_case("if (c) { a ??= init; use(ref); }"));

    assert!(t.executes_after_init_test_case("init?.(ref);"));
    assert!(t.executes_after_init_test_case("init?.[ref];"));
    assert!(!t.executes_after_init_test_case("if (c) { a?.(init); use(ref); }"));
}

// port: EvaluationOrderTest#isExecutedAfterInit_controlStructures
#[test]
fn is_executed_after_init_control_structures() {
    let mut t = EvaluationOrderTest::set_up();
    assert!(t.executes_after_init_test_case("if (init) { use(ref); }"));
    assert!(t.executes_after_init_test_case("if (init) {} else { use(ref); }"));
    assert!(!t.executes_after_init_test_case("if (c) { init; } else { use(ref); }"));

    assert!(t.executes_after_init_test_case("switch (init) { case 1: use(ref); }"));
    assert!(t.executes_after_init_test_case("switch (init) { case ref: break; }"));
    assert!(t.executes_after_init_test_case("switch (x) { case 1: init; use(ref); }"));
    assert!(!t.executes_after_init_test_case("switch (x) { case init: use(ref); }"));
    assert!(!t.executes_after_init_test_case("switch (x) { case 1: case init: use(ref); }"));
    assert!(!t.executes_after_init_test_case("switch (x) { case init: case 2: use(ref); }"));
    assert!(!t.executes_after_init_test_case("switch (x) { case 1: init; case 2: use(ref); }"));

    assert!(t.executes_after_init_test_case("if (c) { init; label: { use(ref); } }"));
    assert!(t.executes_after_init_test_case("if (c) { label: { init; use(ref); } }"));
    assert!(!t.executes_after_init_test_case(
        "if (c) { label: { if (skip) break label; init; } use(ref); }"
    ));

    assert!(t.executes_after_init_test_case("try { init; use(ref); } catch (e) {}"));
    assert!(!t.executes_after_init_test_case("try { init; } catch (e) { use(ref); }"));
    assert!(!t.executes_after_init_test_case("try { init; } finally { use(ref); }"));
}

// port: EvaluationOrderTest#isExecutedAfterInit_destructuringAndDefaultValues
#[test]
fn is_executed_after_init_destructuring_and_default_values() {
    let mut t = EvaluationOrderTest::set_up();
    assert!(t.executes_after_init_test_case("let {a: x = ref} = init;"));
    assert!(t.executes_after_init_test_case("({a: x = ref} = init);"));

    assert!(!t.executes_after_init_test_case("let {[init]: x} = {a: ref};"));
    assert!(!t.executes_after_init_test_case("({[init]: x} = {a: ref});"));
    assert!(!t.executes_after_init_test_case("let [{[init]: x} = {a: ref}] = arr;"));

    // A property target is evaluated before its (conditional) default value.
    assert!(t.executes_after_init_test_case("[o[init] = ref] = arr;"));
    assert!(!t.executes_after_init_test_case("[o[ref] = init] = arr;"));
    // A name target is written after its default value is evaluated.
    assert!(!t.executes_after_init_test_case("[init = ref] = arr;"));
}

// port: EvaluationOrderTest#isExecutedAfterInit_hoistScopesAndClasses
#[test]
fn is_executed_after_init_hoist_scopes_and_classes() {
    let mut t = EvaluationOrderTest::set_up();
    assert!(!t.executes_after_init_test_case("if (c) { init; function f() { return ref; } }"));
    assert!(!t.executes_after_init_test_case("if (c) { init; const f = () => ref; }"));
    assert!(!t.executes_after_init_test_case("if (c) { init; class C { static { use(ref); } } }"));
    assert!(!t.executes_after_init_test_case("if (c) { init; class C { x = ref; } }"));
    assert!(!t.executes_after_init_test_case("if (c) { class C { static x = init; [ref]() {} } }"));
    assert!(!t.executes_after_init_test_case("if (c) { class C { x = init; static y = ref; } }"));
}
