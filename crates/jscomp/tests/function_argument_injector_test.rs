/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/FunctionArgumentInjectorTest.java.

//! Port of `FunctionArgumentInjectorTest.java`: tests for the static methods in
//! FunctionArgumentInjector.

use closure_jscomp::{
    Compiler,
    closure_coding_convention::ClosureCodingConvention,
    compiler_options::CompilerOptions,
    function_argument_injector::{FunctionArgumentInjector, ParamArgPair},
    node_util::NodeUtil,
    source_info_check::SourceInfoCheck,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, js_string::JsString, node::NodeId, testing::node_subject::assert_node,
};
use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

struct FunctionArgumentInjectorTest {
    compiler: Compiler,
    function_argument_injector: FunctionArgumentInjector,
}

impl FunctionArgumentInjectorTest {
    // port: FunctionArgumentInjectorTest#setUp
    fn set_up() -> Self {
        let mut compiler = Compiler::new();
        let options = CompilerOptions::new();

        compiler.init_options(options);
        let function_argument_injector = FunctionArgumentInjector::new(compiler.get_ast_analyzer());
        Self {
            compiler,
            function_argument_injector,
        }
    }

    // port: FunctionArgumentInjectorTest#assertArgMapHasKeys
    fn assert_arg_map_has_keys(&mut self, code: &str, fn_name: &str, expected_keys: &[&str]) {
        let n = self.parse(code);
        let r#fn = find_function(&self.compiler, n, fn_name);
        assert!(r#fn.is_some());
        let call = find_call(&self.compiler, n, fn_name);
        assert!(call.is_some());
        let actual_map =
            self.get_and_validate_function_call_parameter_map(r#fn.unwrap(), call.unwrap());
        let actual_keys: IndexSet<JsString> = actual_map.keys().cloned().collect();
        let expected_keys: IndexSet<JsString> =
            expected_keys.iter().map(|k| JsString::from(*k)).collect();
        assert_eq!(actual_keys, expected_keys);
    }

    // port: FunctionArgumentInjectorTest#testNeededTemps
    fn test_needed_temps(&mut self, code: &str, fn_name: &str, expected_temps: &[&str]) {
        let n = self.parse(code);
        let r#fn = find_function(&self.compiler, n, fn_name);
        assert!(r#fn.is_some());
        let call = find_call(&self.compiler, n, fn_name);
        assert!(call.is_some());
        let r#fn = r#fn.unwrap();
        let args = self.get_and_validate_function_call_parameter_map(r#fn, call.unwrap());

        let actual_temps = self
            .function_argument_injector
            .gather_call_arguments_needing_temps(
                &mut self.compiler,
                r#fn,
                &args,
                &IndexSet::<_>::default(),
                Some(&ClosureCodingConvention::new()),
            );

        let expected_temps: IndexSet<JsString> =
            expected_temps.iter().map(|k| JsString::from(*k)).collect();
        assert_eq!(actual_temps, expected_temps);
    }

    // port: FunctionArgumentInjectorTest#getAndValidateFunctionCallParameterMap
    fn get_and_validate_function_call_parameter_map(
        &mut self,
        r#fn: NodeId,
        call: NodeId,
    ) -> IndexMap<JsString, ParamArgPair> {
        let map = self
            .function_argument_injector
            .get_function_call_parameter_map(&mut self.compiler, r#fn, call, &*get_name_supplier());
        // Verify that all nodes in the map have source info, so they are valid to add to the AST
        for (name, pair) in &map {
            let node = pair.arg();

            SourceInfoCheck::new(&self.compiler).set_check_sub_tree(&mut self.compiler, node);
            assert!(
                self.compiler.get_errors().is_empty(),
                "errors for name: {name}"
            );
        }

        map
    }

    // port: FunctionArgumentInjectorTest#parseFunction
    fn parse_function(&mut self, js: &str) -> NodeId {
        let n = self.parse(js);
        n.get_first_child(&self.compiler).unwrap()
    }

    // port: FunctionArgumentInjectorTest#parse
    fn parse(&mut self, js: &str) -> NodeId {
        let n = self.compiler.parse_test_code(js);
        assert!(
            self.compiler.get_errors().is_empty(),
            "{:?}",
            self.compiler.get_errors()
        );
        n
    }

    /// The `inject` tests: `inject(compiler, getFunctionBody(parseFunction(code)), null,
    /// ImmutableMap.of(name, parse(value).getFirstFirstChild(), ...))`.
    // Rust-only: shared by the testInject* methods.
    fn inject(&mut self, code: &str, replacements: &[(&str, &str)]) -> NodeId {
        let function = self.parse_function(code);
        let body = NodeUtil::get_function_body(&self.compiler, function);
        let mut replacements: IndexMap<JsString, NodeId> = replacements
            .iter()
            .map(|(name, value)| {
                let value = self.parse(value);
                (
                    JsString::from(*name),
                    value.get_first_first_child(&self.compiler).unwrap(),
                )
            })
            .collect();
        self.function_argument_injector
            .inject(&mut self.compiler, body, None, &mut replacements)
    }

    /// `assertNode(result).isEqualTo(getFunctionBody(parseFunction(expected)))`.
    // Rust-only: shared by the testInject* methods.
    fn assert_body_is(&mut self, result: NodeId, expected: &str) {
        let expected_function = self.parse_function(expected);
        let expected_body = NodeUtil::get_function_body(&self.compiler, expected_function);
        assert_node(result).is_equal_to(&self.compiler, expected_body);
    }
}

// port: FunctionArgumentInjectorTest#getNameSupplier
fn get_name_supplier() -> Arc<dyn Fn() -> String + Send + Sync> {
    let i = AtomicI32::new(0);
    Arc::new(move || i.fetch_add(1, Ordering::SeqCst).to_string())
}

// port: FunctionArgumentInjectorTest#findCall
fn find_call(compiler: &Compiler, n: NodeId, name: &str) -> Option<NodeId> {
    if NodeUtil::is_normal_or_opt_chain_call(compiler, n) {
        let callee;
        if NodeUtil::is_normal_or_opt_chain_get_prop(compiler, n.get_first_child(compiler).unwrap())
        {
            callee = n.get_first_first_child(compiler).unwrap();
            // Only "call" is supported at this point.
            check_argument!(callee.get_parent(compiler).unwrap().get_string(compiler) == "call");
        } else {
            callee = n.get_first_child(compiler).unwrap();
        }

        if callee.is_name(compiler) && callee.get_string(compiler) == name {
            return Some(n);
        }
    }

    let mut c = n.get_first_child(compiler);
    while let Some(child) = c {
        let result = find_call(compiler, child, name);
        if result.is_some() {
            return result;
        }
        c = child.get_next(compiler);
    }

    None
}

// port: FunctionArgumentInjectorTest#findFunction
fn find_function(compiler: &Compiler, n: NodeId, name: &str) -> Option<NodeId> {
    if n.is_function(compiler) && n.get_first_child(compiler).unwrap().get_string(compiler) == name
    {
        return Some(n);
    }

    let mut c = n.get_first_child(compiler);
    while let Some(child) = c {
        let result = find_function(compiler, child, name);
        if result.is_some() {
            return result;
        }
        c = child.get_next(compiler);
    }

    None
}

/// Truth's `assertThat(set).containsExactly(..)`.
// Rust-only: the Truth assertion.
fn assert_contains_exactly(actual: &IndexSet<JsString>, expected: &[&str]) {
    let expected: IndexSet<JsString> = expected.iter().map(|k| JsString::from(*k)).collect();
    assert_eq!(*actual, expected);
}

// port: FunctionArgumentInjectorTest#testInject0
#[test]
fn test_inject0() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let result = t.inject("function f(x) { alert(x); }", &[("x", "null")]);
    t.assert_body_is(result, "function f(x) { alert(null); }");
}

// port: FunctionArgumentInjectorTest#testInject1_thisRef
#[test]
fn test_inject1_this_ref() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let result = t.inject("function f() { alert(this); }", &[("this", "null")]);
    t.assert_body_is(result, "function f() { alert(null); }");
}

// port: FunctionArgumentInjectorTest#testInject_thisRef_classField_instance
#[test]
fn test_inject_this_ref_class_field_instance() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let result = t.inject(
        "function f() { return class { field = this; }; }",
        &[("this", "null")],
    );

    t.assert_body_is(result, "function f() { return class { field = this; }; }");
}

// port: FunctionArgumentInjectorTest#testInject_thisRef_classField_static
#[test]
fn test_inject_this_ref_class_field_static() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let result = t.inject(
        "function f() { return class { static field = this; }; }",
        &[("this", "null")],
    );

    t.assert_body_is(
        result,
        "function f() { return class { static field = this; }; }",
    );
}

// port: FunctionArgumentInjectorTest#testInject_thisRef_classField_computed
#[test]
fn test_inject_this_ref_class_field_computed() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let result = t.inject(
        "function f() { return class { [this.k] = this.v; }; }",
        &[("this", "x")],
    );

    t.assert_body_is(result, "function f() { return class { [x.k] = this.v; }; }");
}

// port: FunctionArgumentInjectorTest#testInject_thisRef_classField_staticComputed
#[test]
fn test_inject_this_ref_class_field_static_computed() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let result = t.inject(
        "function f() { return class { static [this.k] = this.v; }; }",
        &[("this", "x")],
    );

    t.assert_body_is(
        result,
        "function f() { return class { static [x.k] = this.v; }; }",
    );
}

// port: FunctionArgumentInjectorTest#testInject_thisRef_classStaticBlock
#[test]
fn test_inject_this_ref_class_static_block() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let result = t.inject(
        "function f() { return class { static { alert(this); } }; }",
        &[("this", "null")],
    );

    t.assert_body_is(
        result,
        "function f() { return class { static { alert(this); } }; }",
    );
}

// port: FunctionArgumentInjectorTest#testInject_thisRef_classField_paramSubstitution
#[test]
fn test_inject_this_ref_class_field_param_substitution() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let result = t.inject(
        "function f(val) { return class { field = val; [val] = this; }; }",
        &[("val", "123"), ("this", "null")],
    );

    t.assert_body_is(
        result,
        "function f(val) { return class { field = 123; [123] = this; }; }",
    );
}

// TODO(johnlenz): Add more unit tests for "inject"

// port: FunctionArgumentInjectorTest#testFindModifiedParameters0
#[test]
fn test_find_modified_parameters0() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ return a; }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert!(modified.is_empty(), "{modified:?}");
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters1
#[test]
fn test_find_modified_parameters1() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ return a==0; }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert!(modified.is_empty(), "{modified:?}");
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters2
#[test]
fn test_find_modified_parameters2() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ b=a }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert!(modified.is_empty(), "{modified:?}");
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters3
#[test]
fn test_find_modified_parameters3() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ a=0 }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["a"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters4
#[test]
fn test_find_modified_parameters4() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a,b){ a=0;b=0 }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["a", "b"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters5
#[test]
fn test_find_modified_parameters5() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a,b){ a; if (a) b=0 }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["b"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters6
#[test]
fn test_find_modified_parameters6() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a,b){ function f(){ a;b; } }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["a", "b"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters7
#[test]
fn test_find_modified_parameters7() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a,b){ a; function f(){ b; } }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["b"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters8
#[test]
fn test_find_modified_parameters8() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a,b){ a; function f(){ function g() { b; } } }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["b"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters9
#[test]
fn test_find_modified_parameters9() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a,b){ (function(){ a;b; }) }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["a", "b"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters10
#[test]
fn test_find_modified_parameters10() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a,b){ a; (function (){ b; }) }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["b"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters11
#[test]
fn test_find_modified_parameters11() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a,b){ a; (function(){ (function () { b; }) }) }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["b"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters12
#[test]
fn test_find_modified_parameters12() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ { let a = 1; } }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert!(modified.is_empty(), "{modified:?}");
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters13
#[test]
fn test_find_modified_parameters13() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ { const a = 1; } }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert!(modified.is_empty(), "{modified:?}");
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters14
#[test]
fn test_find_modified_parameters14() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ for (a in []) {} }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["a"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters15
#[test]
fn test_find_modified_parameters15() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ for (const a in []) {} }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert!(modified.is_empty(), "{modified:?}");
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters16
#[test]
fn test_find_modified_parameters16() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ for (a of []) {} }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["a"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters17
#[test]
fn test_find_modified_parameters17() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ [a] = [2]; }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["a"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters18
#[test]
fn test_find_modified_parameters18() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(a){ var [a] = [2]; }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["a"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters19
#[test]
fn test_find_modified_parameters19() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(p){ const stash = (p = 42); }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["p"]);
}

// port: FunctionArgumentInjectorTest#testFindModifiedParameters20
#[test]
fn test_find_modified_parameters20() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    let f = t.parse_function("function f(p){ let stash = (p = 42); }");
    let modified = t
        .function_argument_injector
        .find_modified_parameters(&t.compiler, f);
    assert_contains_exactly(&modified, &["p"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps1
#[test]
fn test_gather_call_arguments_needing_temps1() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters with side-effects must be executed
    // even if they aren't referenced.
    t.test_needed_temps("function foo(a,b){}; foo(goo(),goo());", "foo", &["a", "b"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps1_optChain
#[test]
fn test_gather_call_arguments_needing_temps1_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters with side-effects must be executed even if they aren't referenced.
    t.test_needed_temps(
        "function foo(a,b){}; foo?.(goo(),goo());",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps2
#[test]
fn test_gather_call_arguments_needing_temps2() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Unreferenced parameters without side-effects can be ignored.
    t.test_needed_temps("function foo(a,b){}; foo(1,2);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams1
#[test]
fn test_adding_temps_for_modified_params1() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The param is not modified so no need for temps.
    t.test_needed_temps("function foo(a,b,c){a;b;c;}; foo(x,x,x);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams2
#[test]
fn test_adding_temps_for_modified_params2() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The first param is modified, so we add a temp for it.
    t.test_needed_temps("function foo(a,b,c){a;b;c;}; foo(x=2,x,x);", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams3
#[test]
fn test_adding_temps_for_modified_params3() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is modified, with first param using the same var, so we add temps for both.
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; foo(x,x=2,x);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams4
#[test]
fn test_adding_temps_for_modified_params4() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is modified, so we only add temp for both.
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; foo(y,x=2,x);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams5
#[test]
fn test_adding_temps_for_modified_params5() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is modified with a more complex statement, we detect it and add temps for
    // first two params.
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; foo(x,(y=2,x=1,z=6),x);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams6
#[test]
fn test_adding_temps_for_modified_params6() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is modified with a function call, we detect it and add temps for first two
    // params.
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; foo(x,(function(){y=2,x=1})(),x);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams7
#[test]
fn test_adding_temps_for_modified_params7() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is modified with addition, so we add temps for both.
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; foo(x,x+=2,x);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams8
#[test]
fn test_adding_temps_for_modified_params8() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is modified with increment, with first param using the same var, so we add
    // temps for both.
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; foo(x,x++,x);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams9
#[test]
fn test_adding_temps_for_modified_params9() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is used in addition but not modified, no temps needed.
    t.test_needed_temps("function foo(a,b,c){a;b;c;}; foo(x,x+2,x);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testAddingTempsForModifiedParams10
#[test]
fn test_adding_temps_for_modified_params10() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is used in addition, the third param is modified, temp all refs.
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; foo(x,x+2,x=5);",
        "foo",
        &["a", "b", "c"],
    );
}

// port: FunctionArgumentInjectorTest#testAddingTempsForCallParams1
#[test]
fn test_adding_temps_for_call_params1() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is a function call so temp it and previous args with names or calls
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; var x; function incX(){return ++x}; foo(x,incX(),x);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testAddingTempsForCallParams2
#[test]
fn test_adding_temps_for_call_params2() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // The second param is a function call so temp it.
    t.test_needed_temps(
        "function foo(a,b,c){a;b;c;}; var x; function incX(){return ++x}; foo(4,incX(),x);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps2_optChain
#[test]
fn test_gather_call_arguments_needing_temps2_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Unreferenced parameters without side-effects can be ignored.
    t.test_needed_temps("function foo(a,b){}; foo?.(1,2);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps3
#[test]
fn test_gather_call_arguments_needing_temps3() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Referenced parameters without side-effects
    // don't need temps.
    t.test_needed_temps("function foo(a,b){a;b;}; foo(x,y);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps3_optChain
#[test]
fn test_gather_call_arguments_needing_temps3_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Referenced parameters without side-effects don't need temps.
    t.test_needed_temps("function foo(a,b){a;b;}; foo?.(x,y);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps4
#[test]
fn test_gather_call_arguments_needing_temps4() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced after side-effect must be assigned to temps.
    // Since b is getting hoisted as a temp, we must also hoist a.
    t.test_needed_temps(
        "function foo(a,b){a;goo();b;}; foo(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testBodyHasConditionalCode
#[test]
fn test_body_has_conditional_code() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    t.test_needed_temps("function foo(a,b){a;goo?.();b;}; foo(x,y);", "foo", &[]);
    t.test_needed_temps("function foo(a,b){a; p&&q; b;}; foo(x,y);", "foo", &[]);
    t.test_needed_temps("function foo(a,b){a; p?q:r; b;}; foo(x,y);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps4_optChain
#[test]
fn test_gather_call_arguments_needing_temps4_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced after side-effect must be assigned to temps.
    // Any param before that param must also be assigned to a temp.
    t.test_needed_temps(
        "function foo(a,b){a;goo();b;}; foo?.(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps5
#[test]
fn test_gather_call_arguments_needing_temps5() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced after out-of-scope side-effect must
    // be assigned to temps.
    t.test_needed_temps("function foo(a,b){x = b; y = a;}; foo(x,y);", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps5_optChain
#[test]
fn test_gather_call_arguments_needing_temps5_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced after out-of-scope side-effect must be assigned to temps.
    t.test_needed_temps(
        "function foo(a,b){x = b; y = a;}; foo?.(x,y);",
        "foo",
        &["a"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps6
#[test]
fn test_gather_call_arguments_needing_temps6() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameter referenced after a out-of-scope side-effect must
    // be assigned to a temp.
    t.test_needed_temps("function foo(a){x++;a;}; foo(x);", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps6_optChain
#[test]
fn test_gather_call_arguments_needing_temps6_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameter referenced after a out-of-scope side-effect must be assigned to a temp.
    t.test_needed_temps("function foo(a){x++;a;}; foo?.(x);", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps7
#[test]
fn test_gather_call_arguments_needing_temps7() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // No temp needed after local side-effects.
    t.test_needed_temps("function foo(a){var c; c = 0; a;}; foo(x);", "foo", &[]);
    t.test_needed_temps("function foo(a){let c; c = 0; a;}; foo(x);", "foo", &[]);
    t.test_needed_temps("function foo(a){const c = 0; a;}; foo(x);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps8
#[test]
fn test_gather_call_arguments_needing_temps8() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Temp needed for side-effects to object using local name.
    t.test_needed_temps(
        "function foo(a){var c = {}; c.goo=0; a;}; foo(x);",
        "foo",
        &["a"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps8_optChain
#[test]
fn test_gather_call_arguments_needing_temps8_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Temp needed for side-effects to object using local name.
    t.test_needed_temps(
        "function foo(a){var c = {}; c.goo=0; a;}; foo?.(x);",
        "foo",
        &["a"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps9
#[test]
fn test_gather_call_arguments_needing_temps9() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced in a loop with side-effects must
    // be assigned to temps.
    t.test_needed_temps(
        "function foo(a,b){while(true){a;goo();b;}}; foo(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps9_optChain
#[test]
fn test_gather_call_arguments_needing_temps9_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced in a loop with side-effects must be assigned to temps.
    t.test_needed_temps(
        "function foo(a,b){while(true){a;goo();b;}}; foo?.(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps10
#[test]
fn test_gather_call_arguments_needing_temps10() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // No temps for parameters referenced in a loop with no side-effects.
    t.test_needed_temps(
        "function foo(a,b){while(true){a;true;b;}}; foo(x,y);",
        "foo",
        &[],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps11
#[test]
fn test_gather_call_arguments_needing_temps11() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced in a loop with side-effects must
    // be assigned to temps.
    t.test_needed_temps(
        "function foo(a,b){do{a;b;}while(goo());}; foo(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps11_optChain
#[test]
fn test_gather_call_arguments_needing_temps11_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced in a loop with side-effects must be assigned to temps.
    t.test_needed_temps(
        "function foo(a,b){do{a;b;}while(goo());}; foo?.(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps12
#[test]
fn test_gather_call_arguments_needing_temps12() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced in a loop with side-effects must
    // be assigned to temps.
    t.test_needed_temps(
        "function foo(a,b){for(;;){a;b;goo();}}; foo(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps13
#[test]
fn test_gather_call_arguments_needing_temps13() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced in a inner loop without side-effects must
    // be assigned to temps if the outer loop has side-effects.
    t.test_needed_temps(
        "function foo(a,b){for(;;){for(;;){a;b;}goo();}}; foo(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps13_optChain
#[test]
fn test_gather_call_arguments_needing_temps13_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced in a inner loop without side-effects must be assigned to temps if the
    // outer loop has side-effects.
    t.test_needed_temps(
        "function foo(a,b){for(;;){for(;;){a;b;}goo();}}; foo?.(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps14
#[test]
fn test_gather_call_arguments_needing_temps14() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Parameters referenced in a loop must
    // be assigned to temps.
    t.test_needed_temps(
        "function foo(a,b){goo();for(;;){a;b;}}; foo(x,y);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps20
#[test]
fn test_gather_call_arguments_needing_temps20() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A long string referenced more than once should have a temp.
    t.test_needed_temps("function foo(a){a;a;}; foo(\"blah blah\");", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps21
#[test]
fn test_gather_call_arguments_needing_temps21() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A short string referenced once should not have a temp.
    t.test_needed_temps("function foo(a){a;a;}; foo(\"\");", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps22
#[test]
fn test_gather_call_arguments_needing_temps22() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A object literal not referenced.
    t.test_needed_temps("function foo(a){}; foo({x:1});", "foo", &[]);
    // A object literal referenced after side-effect, should have a temp.
    t.test_needed_temps(
        "function foo(a){alert('foo');a;}; foo({x:1});",
        "foo",
        &["a"],
    );
    // A object literal referenced after side-effect, should have a temp.
    t.test_needed_temps(
        "function foo(a,b){b;a;}; foo({x:1},alert('foo'));",
        "foo",
        &["a", "b"],
    );
    // A object literal, referenced more than once, should have a temp.
    t.test_needed_temps("function foo(a){a;a;}; foo({x:1});", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps22b
#[test]
fn test_gather_call_arguments_needing_temps22b() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A object literal not referenced.
    t.test_needed_temps(
        "function foo(a){a(this)}; foo.call(f(),g());",
        "foo",
        &["a", "this"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps22b_optChain
#[test]
fn test_gather_call_arguments_needing_temps22b_opt_chain() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A object literal not referenced.
    t.test_needed_temps(
        "function foo(a){a(this)}; foo?.call(f(),g());",
        "foo",
        &["a", "this"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps23
#[test]
fn test_gather_call_arguments_needing_temps23() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A array literal, not referenced.
    t.test_needed_temps("function foo(a){}; foo([1,2]);", "foo", &[]);
    // A array literal, referenced once after side-effect, should have a temp.
    t.test_needed_temps(
        "function foo(a){alert('foo');a;}; foo([1,2]);",
        "foo",
        &["a"],
    );
    // A array literal, referenced more than once, should have a temp.
    t.test_needed_temps("function foo(a){a;a;}; foo([1,2]);", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps24
#[test]
fn test_gather_call_arguments_needing_temps24() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A regex literal, not referenced.
    t.test_needed_temps("function foo(a){}; foo(/mac/);", "foo", &[]);
    // A regex literal, referenced once after side-effect, should have a temp.
    t.test_needed_temps(
        "function foo(a){alert('foo');a;}; foo(/mac/);",
        "foo",
        &["a"],
    );
    // A regex literal, referenced more than once, should have a temp.
    t.test_needed_temps("function foo(a){a;a;}; foo(/mac/);", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps25
#[test]
fn test_gather_call_arguments_needing_temps25() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A side-effect-less constructor, not referenced.
    t.test_needed_temps("function foo(a){}; foo(new Date());", "foo", &[]);
    // A side-effect-less constructor, referenced once after sideeffect, should have a temp.
    t.test_needed_temps(
        "function foo(a){alert('foo');a;}; foo(new Date());",
        "foo",
        &["a"],
    );
    // A side-effect-less constructor, referenced more than once, should have
    // a temp.
    t.test_needed_temps("function foo(a){a;a;}; foo(new Date());", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps26
#[test]
fn test_gather_call_arguments_needing_temps26() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A constructor, not referenced.
    t.test_needed_temps("function foo(a){}; foo(new Bar());", "foo", &["a"]);
    // A constructor, referenced once after a sideeffect, should have a temp.
    t.test_needed_temps(
        "function foo(a){alert('foo');a;}; foo(new Bar());",
        "foo",
        &["a"],
    );
    // A constructor, referenced more than once, should have a temp.
    t.test_needed_temps("function foo(a){a;a;}; foo(new Bar());", "foo", &["a"]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps27
#[test]
fn test_gather_call_arguments_needing_temps27() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Ensure the correct parameter is given a temp, when there is
    // a this value in the call. Since `goo()` is a call and needs to be evaluated in a temp before
    // inlining, we also evaluate the previous args in temporaries.
    t.test_needed_temps(
        "function foo(a,b,c){}; foo.call(this,1,goo(),2);",
        "foo",
        &["this", "a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps28
#[test]
fn test_gather_call_arguments_needing_temps28() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // true/false are don't need temps
    t.test_needed_temps("function foo(a){a;a;}; foo(true);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps29
#[test]
fn test_gather_call_arguments_needing_temps29() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // true/false are don't need temps
    t.test_needed_temps("function foo(a){a;a;}; foo(false);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps30
#[test]
fn test_gather_call_arguments_needing_temps30() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // true/false are don't need temps
    t.test_needed_temps("function foo(a){a;a;}; foo(!0);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps31
#[test]
fn test_gather_call_arguments_needing_temps31() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // true/false are don't need temps
    t.test_needed_temps("function foo(a){a;a;}; foo(!1);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps32
#[test]
fn test_gather_call_arguments_needing_temps32() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // void 0 doesn't need a temp
    t.test_needed_temps("function foo(a){a;a;}; foo(void 0);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps33
#[test]
fn test_gather_call_arguments_needing_temps33() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // doesn't need a temp
    t.test_needed_temps("function foo(a){return a;}; foo(new X);", "foo", &[]);
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps_argMutatesState
#[test]
fn test_gather_call_arguments_needing_temps_arg_mutates_state() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // `y=z` changes state. We create a temp for it and all previous args that it could affect.
    t.test_needed_temps(
        "function foo(a, b, c){return a+b+c;}; foo(x,y+1,y=z);",
        "foo",
        &["a", "b", "c"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTemps_argMutatesState2
#[test]
fn test_gather_call_arguments_needing_temps_arg_mutates_state2() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // `y=z` happens before `y+1` is read. Safe to inject a temp for evaluating it.
    // Consequently, we also inject a temp for previous arg `a` that it could affect
    t.test_needed_temps(
        "function foo(a, b, c){return a+b+c;}; foo(x,y=z,y+1);",
        "foo",
        &["a", "b"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTempsInLoops
#[test]
fn test_gather_call_arguments_needing_temps_in_loops() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // A mutable parameter referenced in loop needs a
    // temporary.
    t.test_needed_temps("function foo(a){for(;;)a;}; foo(new Bar());", "foo", &["a"]);
    t.test_needed_temps(
        "function foo(a){while(true)a;}; foo(new Bar());",
        "foo",
        &["a"],
    );
    t.test_needed_temps(
        "function foo(a){do{a;}while(true)}; foo(new Bar());",
        "foo",
        &["a"],
    );
}

// port: FunctionArgumentInjectorTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    t.test_needed_temps(
        "function foo(...args) {return args ?? x;} foo(1, 2);",
        "foo",
        &["args"],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTempsRestParam1
#[test]
fn test_gather_call_arguments_needing_temps_rest_param1() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    t.test_needed_temps(
        "function foo(...args) {return args;} foo(1, 2);",
        "foo",
        &[],
    );
}

// port: FunctionArgumentInjectorTest#testGatherCallArgumentsNeedingTempsRestParam2
#[test]
fn test_gather_call_arguments_needing_temps_rest_param2() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    // Since args is hoisted as a temp, we must also hoist the previous argument a
    t.test_needed_temps(
        "function foo(x, ...args) {return args;} foo(1, 2);",
        "foo",
        &["x", "args"],
    );
}

// port: FunctionArgumentInjectorTest#testArgMapWithRestParam1
#[test]
fn test_arg_map_with_rest_param1() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    t.assert_arg_map_has_keys(
        "function foo(...args){return args;} foo(1, 2);",
        "foo",
        &["this", "args"],
    );
}

// port: FunctionArgumentInjectorTest#testArgMapWithRestParam2
#[test]
fn test_arg_map_with_rest_param2() {
    let mut t = FunctionArgumentInjectorTest::set_up();
    t.assert_arg_map_has_keys(
        "function foo(...args){return args;} foo();",
        "foo",
        &["this", "args"],
    );
}
