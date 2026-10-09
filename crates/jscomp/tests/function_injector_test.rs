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
//   test/com/google/javascript/jscomp/FunctionInjectorTest.java.

//! Port of `FunctionInjectorTest.java`: inline function tests.
//!
//! The 8 Java tests marked `@Ignore` are not ported.

use closure_jscomp::{
    Compiler,
    abstract_compiler::LifeCycleStage,
    change_verifier::ChangeVerifier,
    code_printer,
    compiler_options::CompilerOptions,
    compiler_pass::CompilerPass,
    function_argument_injector::FunctionArgumentInjector,
    function_injector::{self, CanInlineResult, FunctionInjector, InliningMode, Reference},
    gather_getter_and_setter_properties::GatherGetterAndSetterProperties,
    google_coding_convention::GoogleCodingConvention,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    normalize::Normalize,
    pure_function_identifier::Driver,
    source_file::SourceFile,
    source_info_check::SourceInfoCheck,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
    testing::node_subject::assert_node,
};
use std::{rc::Rc, sync::Arc};

const INLINE_DIRECT: InliningMode = InliningMode::DIRECT;
const INLINE_BLOCK: InliningMode = InliningMode::BLOCK;
const ASSUME_MINIMUM_CAPTURE: bool = false;

// TODO(johnlenz): remove this constant once this has been proven in
// production code.
const NEW_VARS_IN_GLOBAL_SCOPE: CanInlineResult = CanInlineResult::YES;

struct FunctionInjectorTest {
    assume_strict_this: bool,
    allow_decomposition: bool,
}

impl FunctionInjectorTest {
    // port: FunctionInjectorTest#setUp
    fn set_up() -> Self {
        Self {
            assume_strict_this: false,
            allow_decomposition: false,
        }
    }

    /// Test case
    ///
    /// var a = {}, b = {} a.test = "a", b.test = "b" c = a; foo() { c=b; return "a" } c.teste
    // port: FunctionInjectorTest#helperCanInlineReferenceToFunction
    fn helper_can_inline_reference_to_function(
        &self,
        expected_result: CanInlineResult,
        code: &str,
        fn_name: &str,
        mode: InliningMode,
    ) {
        let mut compiler = Compiler::new();
        compiler.init_options(CompilerOptions::new());
        let function_argument_injector =
            Rc::new(FunctionArgumentInjector::new(compiler.get_ast_analyzer()));
        let mut injector = function_injector::Builder::new(&compiler)
            .allow_decomposition(self.allow_decomposition)
            .assume_strict_this(self.assume_strict_this)
            .assume_minimum_capture(ASSUME_MINIMUM_CAPTURE)
            .function_argument_injector(function_argument_injector.clone())
            .build(&compiler);
        let tree = parse(&mut compiler, code);

        let fn_node = find_function(&compiler, tree, fn_name).unwrap();
        let r#unsafe = function_argument_injector.find_modified_parameters(&compiler, fn_node);

        // can-inline tester
        let tester = move |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| -> bool {
            let scope = t.get_scope();
            let chunk = t.get_chunk();
            let r#ref = Reference::new(n, scope, chunk, mode);
            let fn_body = NodeUtil::get_function_body(t, fn_node);
            let references_own_receiver = NodeUtil::references_own_receiver(t, fn_node);
            let has_function =
                NodeUtil::has(t, fn_body, &|ast, n| n.is_function(ast), &|_, _| true);
            let result = injector.can_inline_reference_to_function(
                t.get_compiler(),
                &r#ref,
                fn_node,
                &r#unsafe,
                references_own_receiver,
                has_function,
            );
            assert_eq!(result, expected_result);
            true
        };

        let mut test = TestCallback::new(JsString::from(fn_name), Box::new(tester));
        NodeTraversal::traverse(&mut compiler, tree, &mut test);
    }

    // port: FunctionInjectorTest#helperInlineReferenceToFunction
    fn helper_inline_reference_to_function(
        &self,
        code: &str,
        expected_result: &str,
        fn_name: &str,
        mode: InliningMode,
    ) {
        let mut compiler = Compiler::new();
        let externs_inputs = vec![Arc::new(SourceFile::from_code("externs", ""))];

        let mut options = CompilerOptions::new();
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        compiler.init(
            &externs_inputs,
            &[Arc::new(SourceFile::from_code("code", code))],
            options,
        );

        let function_argument_injector =
            Rc::new(FunctionArgumentInjector::new(compiler.get_ast_analyzer()));
        let mut injector = function_injector::Builder::new(&compiler)
            .allow_decomposition(self.allow_decomposition)
            .assume_strict_this(self.assume_strict_this)
            .assume_minimum_capture(ASSUME_MINIMUM_CAPTURE)
            .function_argument_injector(function_argument_injector.clone())
            .build(&compiler);

        let parse_root = compiler.parse_inputs().unwrap();
        let externs_root = parse_root.get_first_child(&compiler).unwrap();
        let tree = parse_root.get_last_child(&compiler).unwrap();
        assert_ne!(tree, externs_root);

        let mut expected_compiler = Compiler::new();
        let expected_root = parse_expected(&mut expected_compiler, expected_result);

        let main_root = tree;
        Normalize::create_normalize_for_optimizations(&mut compiler).process(
            &mut compiler,
            externs_root,
            main_root,
        );
        GatherGetterAndSetterProperties::update(&mut compiler, externs_root, main_root);
        Driver::new().process(&mut compiler, externs_root, main_root);

        let mut normalize = Normalize::create_normalize_for_optimizations(&mut compiler);
        normalize.process(&mut compiler, externs_root, main_root);
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);

        let fn_node = find_function(&compiler, tree, fn_name);
        assert!(fn_node.is_some());
        let fn_node = fn_node.unwrap();
        let r#unsafe = function_argument_injector.find_modified_parameters(&compiler, fn_node);

        let allow_decomposition = self.allow_decomposition;
        let fn_name_string = JsString::from(fn_name);
        // inline tester
        let tester = move |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| -> bool {
            let scope = t.get_scope();
            let chunk = t.get_chunk();
            let r#ref = Reference::new(n, scope, chunk, mode);
            let fn_body = NodeUtil::get_function_body(t, fn_node);
            let references_own_receiver = NodeUtil::references_own_receiver(t, fn_node);
            let has_function =
                NodeUtil::has(t, fn_body, &|ast, n| n.is_function(ast), &|_, _| true);
            let can_inline = injector.can_inline_reference_to_function(
                t.get_compiler(),
                &r#ref,
                fn_node,
                &r#unsafe,
                references_own_receiver,
                has_function,
            );
            assert_ne!(
                can_inline,
                CanInlineResult::NO,
                "canInlineReferenceToFunction should not be CAN_NOT_INLINE"
            );
            if allow_decomposition {
                assert_eq!(
                    can_inline,
                    CanInlineResult::AFTER_PREPARATION,
                    "canInlineReferenceToFunction should be AFTER_PREPARATION"
                );

                injector.maybe_prepare_call(t.get_compiler(), &r#ref);

                assert_ne!(
                    can_inline,
                    CanInlineResult::YES,
                    "canInlineReferenceToFunction should be CAN_INLINE"
                );
            }

            let result = injector.inline(t.get_compiler(), &r#ref, &fn_name_string, fn_node);
            validate_source_info(t.get_compiler(), result);

            let actual = tree.get_first_child(t).unwrap();
            if let Err(message) = assert_node(actual)
                .using_serializer(to_source)
                .check_equal_to_across(t, &expected_compiler, expected_root, false)
            {
                panic!("{message}");
            }

            true
        };

        let verifier = ChangeVerifier::new(&compiler).snapshot(&mut compiler, main_root);
        let mut test = TestCallback::new(JsString::from(fn_name), Box::new(tester));
        NodeTraversal::traverse(&mut compiler, tree, &mut test);
        verifier.check_recorded_changes_named(
            &mut compiler,
            "helperInlineReferenceToFunction",
            main_root,
        );
    }

    /// Calls `FunctionInjector#doesFunctionMeetMinimumRequirements(String, Node)`.
    ///
    /// This method is called as a prerequisite to checking if a particular reference is
    /// inlinable.
    // port: FunctionInjectorTest#doesFunctionMeetMinimumRequirements
    fn does_function_meet_minimum_requirements(&self, code: &str, fn_name: &str) -> bool {
        let mut compiler = Compiler::new();
        compiler.init_options(CompilerOptions::new());
        let injector = function_injector::Builder::new(&compiler)
            .allow_decomposition(self.allow_decomposition)
            .assume_strict_this(self.assume_strict_this)
            .assume_minimum_capture(ASSUME_MINIMUM_CAPTURE)
            .build(&compiler);
        let tree = parse(&mut compiler, code);

        let fn_node = find_function(&compiler, tree, fn_name).unwrap();
        injector.does_function_meet_minimum_requirements(
            &compiler,
            &JsString::from(fn_name),
            fn_node,
        )
    }
}

// port: FunctionInjectorTest#validateSourceInfo
fn validate_source_info(compiler: &mut Compiler, subtree: NodeId) {
    SourceInfoCheck::new(compiler).set_check_sub_tree(compiler, subtree);
    // Source information problems are reported as compiler errors.
    if compiler.get_error_count() != 0 {
        let mut msg = String::from("Error encountered: ");
        for err in compiler.get_errors() {
            msg += &format!("{err}\n");
        }
        assert_eq!(compiler.get_error_count(), 0, "{msg}");
    }
}

/// Java's `Method` interface: `boolean call(NodeTraversal t, Node n, Node parent)`.
type Method = Box<dyn FnMut(&mut NodeTraversal<'_>, NodeId, Option<NodeId>) -> bool>;

struct TestCallback {
    callname: JsString,
    method: Method,
    complete: bool,
}

impl TestCallback {
    // port: FunctionInjectorTest.TestCallback#TestCallback
    fn new(callname: JsString, method: Method) -> Self {
        Self {
            callname,
            method,
            complete: false,
        }
    }
}

impl Callback for TestCallback {
    // port: FunctionInjectorTest.TestCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _node_traversal: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !self.complete
    }

    // port: FunctionInjectorTest.TestCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_call(t) {
            let callee = if NodeUtil::is_normal_get(t, n.get_first_child(t).unwrap()) {
                n.get_first_first_child(t).unwrap()
            } else {
                n.get_first_child(t).unwrap()
            };

            if callee.is_name(t) && callee.get_string(t) == self.callname {
                self.complete = (self.method)(t, n, parent);
            }
        }

        if parent.is_none() {
            assert!(self.complete);
        }
    }
}

// port: FunctionInjectorTest#findFunction
fn find_function(ast: &Ast, n: NodeId, name: &str) -> Option<NodeId> {
    if n.is_function(ast) && n.get_first_child(ast).unwrap().get_string(ast) == name {
        return Some(n);
    }

    let mut c = n.get_first_child(ast);
    while let Some(child) = c {
        let result = find_function(ast, child, name);
        if result.is_some() {
            return result;
        }
        c = child.get_next(ast);
    }

    None
}

/// Returns the compiler that owns the parsed node (the arena) with the node.
// port: FunctionInjectorTest#prep
fn prep(js: &str) -> (Compiler, NodeId) {
    let mut compiler = Compiler::new();
    let n = compiler.parse_test_code(js);
    assert_eq!(compiler.get_error_count(), 0);
    let first = n.get_first_child(&compiler).unwrap();
    (compiler, first)
}

// port: FunctionInjectorTest#parse
fn parse(compiler: &mut Compiler, js: &str) -> NodeId {
    let n = compiler.parse_test_code(js);
    assert_eq!(compiler.get_error_count(), 0);
    n
}

// port: FunctionInjectorTest#parseExpected
fn parse_expected(compiler: &mut Compiler, js: &str) -> NodeId {
    let n = compiler.parse_test_code(js);
    let mut message = String::from("Unexpected errors: ");
    let errs = compiler.get_errors();
    for element in &errs {
        message += &format!("\n{element}");
    }
    assert_eq!(compiler.get_error_count(), 0, "{message}");
    n
}

// port: FunctionInjectorTest#toSource
fn to_source(ast: &Ast, n: NodeId) -> String {
    code_printer::Builder::new(n)
        .set_pretty_print(true)
        .set_line_break(true)
        .build(ast)
        .to_string()
}

// port: FunctionInjectorTest#testIsSimpleFunction1
#[test]
fn test_is_simple_function1() {
    let _t = FunctionInjectorTest::set_up();
    let (compiler, n) = prep("function f(){}");
    assert!(FunctionInjector::is_direct_call_node_replacement_possible(
        &compiler, n
    ));
}

// port: FunctionInjectorTest#testIsSimpleFunction2
#[test]
fn test_is_simple_function2() {
    let _t = FunctionInjectorTest::set_up();
    let (compiler, n) = prep("function f(){return 0;}");
    assert!(FunctionInjector::is_direct_call_node_replacement_possible(
        &compiler, n
    ));
}

// port: FunctionInjectorTest#testIsSimpleFunction3
#[test]
fn test_is_simple_function3() {
    let _t = FunctionInjectorTest::set_up();
    let (compiler, n) = prep("function f(){return x ? 0 : 1}");
    assert!(FunctionInjector::is_direct_call_node_replacement_possible(
        &compiler, n
    ));
}

// port: FunctionInjectorTest#testIsSimpleFunction4
#[test]
fn test_is_simple_function4() {
    let _t = FunctionInjectorTest::set_up();
    let (compiler, n) = prep("function f(){return;}");
    assert!(!FunctionInjector::is_direct_call_node_replacement_possible(
        &compiler, n
    ));
}

// port: FunctionInjectorTest#testIsSimpleFunction5
#[test]
fn test_is_simple_function5() {
    let _t = FunctionInjectorTest::set_up();
    let (compiler, n) = prep("function f(){return 0; return 0;}");
    assert!(!FunctionInjector::is_direct_call_node_replacement_possible(
        &compiler, n
    ));
}

// port: FunctionInjectorTest#testIsSimpleFunction6
#[test]
fn test_is_simple_function6() {
    let _t = FunctionInjectorTest::set_up();
    let (compiler, n) = prep("function f(){var x=true;return x ? 0 : 1}");
    assert!(!FunctionInjector::is_direct_call_node_replacement_possible(
        &compiler, n
    ));
}

// port: FunctionInjectorTest#testIsSimpleFunction7
#[test]
fn test_is_simple_function7() {
    let _t = FunctionInjectorTest::set_up();
    let (compiler, n) = prep("function f(){if (x) return 0; else return 1}");
    assert!(!FunctionInjector::is_direct_call_node_replacement_possible(
        &compiler, n
    ));
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction1
#[test]
fn test_can_inline_reference_to_function1() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){}; foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction2
#[test]
fn test_can_inline_reference_to_function2() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){}; foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction3
#[test]
fn test_can_inline_reference_to_function3() {
    let t = FunctionInjectorTest::set_up();
    // NOTE: FoldConstants will convert this to a empty function,
    // so there is no need to explicitly support it.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(){return;}; foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction4
#[test]
fn test_can_inline_reference_to_function4() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return;}; foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction5
#[test]
fn test_can_inline_reference_to_function5() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return true;}; foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction6
#[test]
fn test_can_inline_reference_to_function6() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return true;}; foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction7
#[test]
fn test_can_inline_reference_to_function7() {
    let t = FunctionInjectorTest::set_up();
    // In var initialization.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return true;}; var x=foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction8
#[test]
fn test_can_inline_reference_to_function8() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return true;}; var x=foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction9
#[test]
fn test_can_inline_reference_to_function9() {
    let t = FunctionInjectorTest::set_up();
    // In assignment.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return true;}; var x; x=foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction10
#[test]
fn test_can_inline_reference_to_function10() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return true;}; var x; x=foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction11
#[test]
fn test_can_inline_reference_to_function11() {
    let t = FunctionInjectorTest::set_up();
    // In expression.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return true;}; var x; x=x+foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction12
#[test]
fn test_can_inline_reference_to_function12() {
    let t = FunctionInjectorTest::set_up();
    // "foo" is not known to be side-effect free, it might change the value
    // of "x", so it can't be inlined.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(){return true;}; var x; x=x+foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction12b
#[test]
fn test_can_inline_reference_to_function12b() {
    let mut t = FunctionInjectorTest::set_up();
    // "foo" is not known to be side-effect free, it might change the value
    // of "x", so it can't be inlined.
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(){return true;}; var x; x=x+foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction14
#[test]
fn test_can_inline_reference_to_function14() {
    let t = FunctionInjectorTest::set_up();
    // Simple call with parameters
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(a){return true;}; foo(x);",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction15
#[test]
fn test_can_inline_reference_to_function15() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(a){return true;}; foo(x);",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction16
#[test]
fn test_can_inline_reference_to_function16() {
    let t = FunctionInjectorTest::set_up();
    // Function "foo" as it contains "var b" which
    // must be brought into the global scope.
    t.helper_can_inline_reference_to_function(
        NEW_VARS_IN_GLOBAL_SCOPE,
        "function foo(a){var b;return a;}; foo(goo());",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction17
#[test]
fn test_can_inline_reference_to_function17() {
    let t = FunctionInjectorTest::set_up();
    // This doesn't bring names into the global name space.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(a){return a;};\nfunction x() { foo(goo()); }\n",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction18
#[test]
fn test_can_inline_reference_to_function18() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has side-effects.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(a){return a;} foo(x++);",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction18b
#[test]
fn test_can_inline_reference_to_function18b() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has side-effects.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a,b){return b,a;} foo(x++,use(x));",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction19
#[test]
fn test_can_inline_reference_to_function19() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has mutable parameter referenced more than once.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return a+a} foo([]);",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction20
#[test]
fn test_can_inline_reference_to_function20() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return a+a} foo({});",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction21
#[test]
fn test_can_inline_reference_to_function21() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return a+a} foo(new Date);",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction22
#[test]
fn test_can_inline_reference_to_function22() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return a+a} foo(true && new Date);",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction23
#[test]
fn test_can_inline_reference_to_function23() {
    let t = FunctionInjectorTest::set_up();
    // variables to global scope.
    t.helper_can_inline_reference_to_function(
        NEW_VARS_IN_GLOBAL_SCOPE,
        "function foo(a){return a;}; foo(x++);",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction24
#[test]
fn test_can_inline_reference_to_function24() {
    let t = FunctionInjectorTest::set_up();
    // ... this is OK, because it doesn't introduce a new global name.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(a){return a;};\nfunction x() { foo(x++); }\n",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction25
#[test]
fn test_can_inline_reference_to_function25() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has side-effects.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return a+a;}; foo(x++);",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction26
#[test]
fn test_can_inline_reference_to_function26() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        NEW_VARS_IN_GLOBAL_SCOPE,
        "function foo(a){return a+a;}; foo(x++);",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction27
#[test]
fn test_can_inline_reference_to_function27() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(a){return a+a;};\nfunction x() { foo(x++); }\n",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction28
#[test]
fn test_can_inline_reference_to_function28() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has side-effects.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return true;}; foo(goo());",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction29
#[test]
fn test_can_inline_reference_to_function29() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        NEW_VARS_IN_GLOBAL_SCOPE,
        "function foo(a){return true;}; foo(goo());",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction30
#[test]
fn test_can_inline_reference_to_function30() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo(goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction31
#[test]
fn test_can_inline_reference_to_function31() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        concat!(
            "function foo(a) {return true;};\n",
            "function x() {foo.call(this, 1);}\n",
        ),
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction32
#[test]
fn test_can_inline_reference_to_function32() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.apply(this, [1]); }\n",
        ),
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction33
#[test]
fn test_can_inline_reference_to_function33() {
    let t = FunctionInjectorTest::set_up();
    // No special handling is required for method calls passing this.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.bar(this, 1); }\n",
        ),
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction34
#[test]
fn test_can_inline_reference_to_function34() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(this, goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction35
#[test]
fn test_can_inline_reference_to_function35() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.apply(this, goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction36
#[test]
fn test_can_inline_reference_to_function36() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.bar(this, goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction37
#[test]
fn test_can_inline_reference_to_function37() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(null, 1); }\n",
        ),
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction38
#[test]
fn test_can_inline_reference_to_function38() {
    let mut t = FunctionInjectorTest::set_up();
    t.assume_strict_this = false;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(null, goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
    t.assume_strict_this = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(null, goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction39
#[test]
fn test_can_inline_reference_to_function39() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(bar, 1); }\n",
        ),
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction40
#[test]
fn test_can_inline_reference_to_function40() {
    let mut t = FunctionInjectorTest::set_up();
    t.assume_strict_this = false;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(bar, goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
    t.assume_strict_this = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(bar, goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction41
#[test]
fn test_can_inline_reference_to_function41() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(new bar(), 1); }\n",
        ),
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction42
#[test]
fn test_can_inline_reference_to_function42() {
    let mut t = FunctionInjectorTest::set_up();
    t.assume_strict_this = false;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(new bar(), goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
    t.assume_strict_this = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        concat!(
            "function foo(a){return true;};\n",
            "function x() { foo.call(new bar(), goo()); }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction43
#[test]
fn test_can_inline_reference_to_function43() {
    let t = FunctionInjectorTest::set_up();
    // Handle the case of a missing 'this' value in a call.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(){return true;};\nfunction x() { foo.call(); }\n",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction44
#[test]
fn test_can_inline_reference_to_function44() {
    let mut t = FunctionInjectorTest::set_up();
    t.assume_strict_this = false;
    // Handle the case of a missing 'this' value in a call.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(){return true;};\nfunction x() { foo.call(); }\n",
        "foo",
        INLINE_BLOCK,
    );
    t.assume_strict_this = true;
    // Handle the case of a missing 'this' value in a call.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return true;};\nfunction x() { foo.call(); }\n",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction45
#[test]
fn test_can_inline_reference_to_function45() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return function() {return true;}}; foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction46
#[test]
fn test_can_inline_reference_to_function46() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return function() {return true;}}; foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction47
#[test]
fn test_can_inline_reference_to_function47() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression and variable decl.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(){var a; return function() {return true;}}; foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction48
#[test]
fn test_can_inline_reference_to_function48() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression and variable decl.
    // TODO(johnlenz): should we validate no values in scope?
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){var a; return function() {return true;}}; foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction49
#[test]
fn test_can_inline_reference_to_function49() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return function() {var a; return true;}}; foo();",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction50
#[test]
fn test_can_inline_reference_to_function50() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return function() {var a; return true;}}; foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction51
#[test]
fn test_can_inline_reference_to_function51() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function statement.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){function x() {var a; return true;} return x}; foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunction52
#[test]
fn test_can_inline_reference_to_function52() {
    let t = FunctionInjectorTest::set_up();
    // Don't inline functions with var declarations into a scope with inner functions
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo() { var a = 3; return a; }\n",
            "function bar() { function baz() {} if (true) { foo(); } }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression1
#[test]
fn test_can_inline_reference_to_function_in_expression1() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in if condition
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() { if (foo(1)) throw 'test'; }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression2
#[test]
fn test_can_inline_reference_to_function_in_expression2() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in return expression
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() { return foo(1); }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression3
#[test]
fn test_can_inline_reference_to_function_in_expression3() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in switch expression
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() { switch(foo(1)) { default:break; } }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression4
#[test]
fn test_can_inline_reference_to_function_in_expression4() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in hook condition
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {foo(1)?0:1 }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression5
#[test]
fn test_can_inline_reference_to_function_in_expression5() {
    let t = FunctionInjectorTest::set_up();
    // Call in hook side-effect free condition
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() {true?foo(1):1 }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression5a
#[test]
fn test_can_inline_reference_to_function_in_expression5a() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in hook side-effect free condition
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {true?foo(1):1 }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression6
#[test]
fn test_can_inline_reference_to_function_in_expression6() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in expression statement "condition"
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {foo(1) && 1 }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression7
#[test]
fn test_can_inline_reference_to_function_in_expression7() {
    let t = FunctionInjectorTest::set_up();
    // Call in expression statement after side-effect free "condition"
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return true;};\nfunction x() {1 && foo(1) }\n",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression7a
#[test]
fn test_can_inline_reference_to_function_in_expression7a() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in expression statement after side-effect free "condition"
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {1 && foo(1) }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression8
#[test]
fn test_can_inline_reference_to_function_in_expression8() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in expression statement after side-effect free operator
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {1 + foo(1) }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression9
#[test]
fn test_can_inline_reference_to_function_in_expression9() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in VAR expression.
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {var b = 1 + foo(1)}",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression10
#[test]
fn test_can_inline_reference_to_function_in_expression10() {
    let t = FunctionInjectorTest::set_up();
    // Call in assignment expression.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(a){return true;};\n",
            "function x() {var b; b += 1 + foo(1) }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression10a
#[test]
fn test_can_inline_reference_to_function_in_expression10a() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in assignment expression.
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {var b; b += 1 + foo(1) }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression12
#[test]
fn test_can_inline_reference_to_function_in_expression12() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {var a,b,c; a = b = c = foo(1) }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression13
#[test]
fn test_can_inline_reference_to_function_in_expression13() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(a){return true;}; function x() {var a,b,c; a = b = c = 1 + foo(1) }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression14
#[test]
fn test_can_inline_reference_to_function_in_expression14() {
    let t = FunctionInjectorTest::set_up();
    // ... foo can not be inlined because of possible changes to "c".
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "var a = {}, b = {}, c;\n",
            "a.test = 'a';\n",
            "b.test = 'b';\n",
            "c = a;\n",
            "function foo(){c = b; return 'foo'};\n",
            "c.test=foo();\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression14a
#[test]
fn test_can_inline_reference_to_function_in_expression14a() {
    let mut t = FunctionInjectorTest::set_up();
    // ... foo can be inlined despite possible changes to "c".
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        concat!(
            "var a = {}, b = {}, c;\n",
            "a.test = 'a';\n",
            "b.test = 'b';\n",
            "c = a;\n",
            "function foo(){c = b; return 'foo'};\n",
            "c.test=foo();\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression18
#[test]
fn test_can_inline_reference_to_function_in_expression18() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in within a call
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(){return _g();}; function x() {1 + foo()() }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression19
#[test]
fn test_can_inline_reference_to_function_in_expression19() {
    let t = FunctionInjectorTest::set_up();
    // ... unless foo is known to be side-effect free, it might actually
    // change the value of "_g" which would unfortunately change the behavior,
    // so we can't inline here.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(){return a;};\nfunction x() {1 + _g(foo()) }\n",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression19a
#[test]
fn test_can_inline_reference_to_function_in_expression19a() {
    let mut t = FunctionInjectorTest::set_up();
    // ... unless foo is known to be side-effect free, it might actually
    // change the value of "_g" which would unfortunately change the behavior,
    // so we can't inline here.
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(){return a;}; function x() {1 + _g(foo()) }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression21
#[test]
fn test_can_inline_reference_to_function_in_expression21() {
    let t = FunctionInjectorTest::set_up();
    // Assignments to object are problematic if the call has side-effects,
    // as the object that is being referred to can change.
    // Note: This could be changed be inlined if we in some way make "z"
    // as not escaping from the local scope.
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() { z.gack = foo(1) }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression21a
#[test]
fn test_can_inline_reference_to_function_in_expression21a() {
    let mut t = FunctionInjectorTest::set_up();
    // Assignments to object are problematic if the call has side-effects,
    // as the object that is being referred to can change.
    // Note: This could be changed be inlined if we in some way make "z"
    // as not escaping from the local scope.
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() { z.gack = foo(1) }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression22
#[test]
fn test_can_inline_reference_to_function_in_expression22() {
    let t = FunctionInjectorTest::set_up();
    // ... foo() is after a side-effect
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(){return a;};\n",
            "function x() {1 + _g(_a(), foo()) }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression22a
#[test]
fn test_can_inline_reference_to_function_in_expression22a() {
    let mut t = FunctionInjectorTest::set_up();
    // ... foo() is after a side-effect
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(){return a;}; function x() {1 + _g(_a(), foo()) }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression23
#[test]
fn test_can_inline_reference_to_function_in_expression23() {
    let t = FunctionInjectorTest::set_up();
    // ... foo() is after a side-effect
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        concat!(
            "function foo(){return a;};\n",
            "function x() {1 + _g(_a(), foo.call(this)) }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInExpression23a
#[test]
fn test_can_inline_reference_to_function_in_expression23a() {
    let mut t = FunctionInjectorTest::set_up();
    // ... foo() is after a side-effect
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::AFTER_PREPARATION,
        "function foo(){return a;}; function x() {1 + _g(_a(), foo.call(this)) }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInLoop1
#[test]
fn test_can_inline_reference_to_function_in_loop1() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){return a;}; while(1) { foo(); }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReferenceToFunctionInLoop2
#[test]
fn test_can_inline_reference_to_function_in_loop2() {
    let mut t = FunctionInjectorTest::set_up();
    // If function contains function, don't inline it into a loop.
    // TODO(johnlenz): this can be improved by looking to see
    // if the inner function contains any references to values defined
    // in the outer function.
    t.allow_decomposition = true;
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(){return function() {};}; while(1) { foo(); }",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineFunctionWithInnerArrowFunction1
#[test]
fn test_can_inline_function_with_inner_arrow_function1() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::YES,
        "function foo(){ () => { alert(1); }; } foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineFunctionWithInnerArrowFunction2
#[test]
fn test_can_inline_function_with_inner_arrow_function2() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(){ () => { this; }; } foo();",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline1
#[test]
fn test_inline1() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(){}; foo();",
        "function foo(){}; void 0",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInline2
#[test]
fn test_inline2() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(){}; foo();",
        "function foo(){}; {}",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline3
#[test]
fn test_inline3() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(){return;}; foo();",
        "function foo(){return;}; {}",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline4
#[test]
fn test_inline4() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(){return true;}; foo();",
        "function foo(){return true;}; true;",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInline5
#[test]
fn test_inline5() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(){return true;}; foo();",
        "function foo(){return true;}; {true;}",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline6
#[test]
fn test_inline6() {
    let t = FunctionInjectorTest::set_up();
    // In var initialization.
    t.helper_inline_reference_to_function(
        "function foo(){return true;}; var x=foo();",
        "function foo(){return true;}; var x=true;",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInline7
#[test]
fn test_inline7() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(){return true;}; var x=foo();",
        "function foo(){return true;}; var x;\n{x=true}\n",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline8
#[test]
fn test_inline8() {
    let t = FunctionInjectorTest::set_up();
    // In assignment.
    t.helper_inline_reference_to_function(
        "function foo(){return true;}; var x; x=foo();",
        "function foo(){return true;}; var x; x=true;",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInline9
#[test]
fn test_inline9() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(){return true;}; var x; x=foo();",
        "function foo(){return true;}; var x;{x=true}",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline10
#[test]
fn test_inline10() {
    let t = FunctionInjectorTest::set_up();
    // In expression.
    t.helper_inline_reference_to_function(
        "function foo(){return true;}; var x; x=x+foo();",
        "function foo(){return true;}; var x; x=x+true;",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInline11
#[test]
fn test_inline11() {
    let t = FunctionInjectorTest::set_up();
    // Simple call with parameters
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; foo(x);",
        "function foo(a){return true;}; true;",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInline12
#[test]
fn test_inline12() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; foo(x);",
        "function foo(a){return true;}; {true}",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline13
#[test]
fn test_inline13() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has side-effects.
    t.helper_inline_reference_to_function(
        "function foo(a){return a;};\nfunction x() { foo(x++); }\n",
        "function foo(a){return a;};\nfunction x(){{x++}}\n",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline14
#[test]
fn test_inline14() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has side-effects.
    t.helper_inline_reference_to_function(
        "function foo(a){return a+a;}; foo(x++);",
        concat!(
            "function foo(a){return a+a;};\n",
            "{var a$jscomp$inline_0=x++;\n",
            " a$jscomp$inline_0+\n",
            "a$jscomp$inline_0;}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline15
#[test]
fn test_inline15() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has mutable, references more than once.
    t.helper_inline_reference_to_function(
        "function foo(a){return a+a;}; foo(new Date());",
        concat!(
            "function foo(a){return a+a;};\n",
            "{var a$jscomp$inline_0=new Date();\n",
            " a$jscomp$inline_0+\n",
            "a$jscomp$inline_0;}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline16
#[test]
fn test_inline16() {
    let t = FunctionInjectorTest::set_up();
    // Parameter is large, references more than once.
    t.helper_inline_reference_to_function(
        "function foo(a){return a+a;}; foo(function(){});",
        concat!(
            "function foo(a){return a+a;};\n",
            "{var a$jscomp$inline_0=function(){};\n",
            " a$jscomp$inline_0+\n",
            "a$jscomp$inline_0;}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline17
#[test]
fn test_inline17() {
    let t = FunctionInjectorTest::set_up();
    // Parameter has side-effects.
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; foo(goo());",
        concat!(
            "function foo(a){return true;};\n",
            "{var a$jscomp$inline_0=goo();true}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline18
#[test]
fn test_inline18() {
    let t = FunctionInjectorTest::set_up();
    // This doesn't bring names into the global name space.
    t.helper_inline_reference_to_function(
        "function foo(a){var b;return a;} function x() { foo(goo()); }",
        concat!(
            "function foo(a) {\n",
            "  var b;\n",
            "  return a;\n",
            "}\n",
            "function x() {\n",
            "  {\n",
            "    var a$jscomp$inline_0 = goo();\n",
            "    var b$jscomp$inline_1;\n",
            "    a$jscomp$inline_0;\n",
            "  }\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline19
#[test]
fn test_inline19() {
    let t = FunctionInjectorTest::set_up();
    // Properly alias.
    t.helper_inline_reference_to_function(
        concat!(
            "var x = 1; var y = 2;\n",
            "function foo(a,b){x = b; y = a;};\n",
            "function bar() { foo(x,y); }\n",
        ),
        concat!(
            "var x = 1; var y = 2;\n",
            "function foo(a,b){x = b; y = a;};\n",
            "function bar() {\n",
            "{var a$jscomp$inline_0=x;\n",
            "x = y;\n",
            "y = a$jscomp$inline_0;}\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline19b
#[test]
fn test_inline19b() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        concat!(
            "var x = 1; var y = 2;\n",
            "function foo(a, b) {\n",
            "  y = a; x = b;\n",
            "};\n",
            "function bar() { foo(x,y); }\n",
        ),
        concat!(
            "var x = 1; var y = 2;\n",
            "function foo(a,b){\n",
            "  y = a; x = b;\n",
            "};\n",
            "function bar() {\n",
            "{\n",
            "  var a$jscomp$inline_0=x;\n",
            "  var b$jscomp$inline_1=y;\n",
            "  y=a$jscomp$inline_0;\n",
            "  x=b$jscomp$inline_1}\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInline20
#[test]
fn test_inline20() {
    let t = FunctionInjectorTest::set_up();
    // cloned FUNCTION node must be reported as being added.
    t.helper_inline_reference_to_function(
        "function foo(a){return a}; foo(function(){});",
        "function foo(a){return a}; (function(){})",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInline21
#[test]
fn test_inline21() {
    let t = FunctionInjectorTest::set_up();
    // cloned FUNCTION node must be reported as being added.
    t.helper_inline_reference_to_function(
        "function foo(a){return a}; foo(function(){});",
        "function foo(a){return a}; {(function(){})}",
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineIntoLoop
#[test]
fn test_inline_into_loop() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(a){var b;return a;}; for(;1;){ foo(1); }",
        "function foo(a){var b;return a;}; for(;1;){ { var b$jscomp$inline_1=void 0;1} }",
        "foo",
        INLINE_BLOCK,
    );
    t.helper_inline_reference_to_function(
        "function foo(a){var b;return a;};\ndo{ foo(1); } while(1)\n",
        concat!(
            "function foo(a){var b;return a;};\n",
            "do{ {\n",
            "var b$jscomp$inline_1=void 0;1}}while(1)\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
    t.helper_inline_reference_to_function(
        concat!(
            "function foo(a){for(var b in c)return a;};\n",
            "for(;1;){ foo(1); }\n",
        ),
        concat!(
            "function foo(a){var b;for(b in c)return a;};\n",
            "for(;1;){ {JSCompiler_inline_label_foo_2:{\n",
            "var b$jscomp$inline_1=void 0;for(b$jscomp$inline_1 in c){\n",
            "1;break JSCompiler_inline_label_foo_2\n",
            "}}}}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineFunctionWithInnerFunction1
#[test]
fn test_inline_function_with_inner_function1() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression.
    t.helper_inline_reference_to_function(
        "function foo(){return function() {return true;}}; foo();",
        concat!(
            "function foo(){return function() {return true;}};\n",
            "(function() {return true;})\n",
        ),
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInlineFunctionWithInnerFunction2
#[test]
fn test_inline_function_with_inner_function2() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression.
    t.helper_inline_reference_to_function(
        "function foo(){return function() {return true;}}; foo();",
        concat!(
            "function foo(){return function() {return true;}};\n",
            "{(function() {return true;})}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineFunctionWithInnerFunction3
#[test]
fn test_inline_function_with_inner_function3() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression.
    t.helper_inline_reference_to_function(
        "function foo(){return function() {var a; return true;}}; foo();",
        concat!(
            "function foo(){return function() {var a; return true;}};\n",
            "(function() {var a; return true;});\n",
        ),
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testInlineFunctionWithInnerFunction4
#[test]
fn test_inline_function_with_inner_function4() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function expression.
    t.helper_inline_reference_to_function(
        "function foo(){return function() {var a; return true;}}; foo();",
        concat!(
            "function foo(){return function() {var a; return true;}};\n",
            "{(function() {var a$jscomp$inline_0; return true;});}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineFunctionWithInnerFunction5
#[test]
fn test_inline_function_with_inner_function5() {
    let t = FunctionInjectorTest::set_up();
    // Call with inner function statement.
    t.helper_inline_reference_to_function(
        "function foo(){function x() {var a; return true;} return x} foo();",
        concat!(
            "function foo() {\n",
            "  function x() { var a; return true; }\n",
            "  return x;\n",
            "}\n",
            "{\n",
            "  var x$jscomp$inline_0 = function(){\n",
            "    var a$jscomp$inline_1;\n",
            "    return true;\n",
            "  };\n",
            "  x$jscomp$inline_0;\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineFunctionWithInnerArrowFunction1
#[test]
fn test_inline_function_with_inner_arrow_function1() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(){ () => { alert(1); }; } foo();",
        concat!(
            "function foo(){ () => { alert(1); }; }\n",
            "{ () => { alert(1); }; }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression1
#[test]
fn test_inline_reference_in_expression1() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in if condition
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() { if (foo(1)) throw 'test'; }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "if (JSCompiler_inline_result$jscomp$0) throw 'test'; }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression2
#[test]
fn test_inline_reference_in_expression2() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in return expression
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() { return foo(1); }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "return JSCompiler_inline_result$jscomp$0; }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression3
#[test]
fn test_inline_reference_in_expression3() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in switch expression
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() { switch(foo(1)) { default:break; } }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "switch(JSCompiler_inline_result$jscomp$0) { default:break; } }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression4
#[test]
fn test_inline_reference_in_expression4() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in hook condition
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() {foo(1)?0:1 }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "JSCompiler_inline_result$jscomp$0?0:1 }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression5
#[test]
fn test_inline_reference_in_expression5() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in expression statement "condition"
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() {foo(1)&&1 }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "JSCompiler_inline_result$jscomp$0&&1 }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression6
#[test]
fn test_inline_reference_in_expression6() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in expression statement after side-effect free "condition"
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() {1 + foo(1) }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "1 + JSCompiler_inline_result$jscomp$0 }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression7
#[test]
fn test_inline_reference_in_expression7() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in expression statement "condition"
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() {foo(1) && 1 }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "JSCompiler_inline_result$jscomp$0&&1 }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression8
#[test]
fn test_inline_reference_in_expression8() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in expression statement after side-effect free operator
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() {1 + foo(1) }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "1 + JSCompiler_inline_result$jscomp$0 }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression9
#[test]
fn test_inline_reference_in_expression9() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in VAR expression.
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() {var b = 1 + foo(1)}",
        concat!(
            "function foo(a){return true;};\n",
            "function x() {\n",
            "var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=true;}\n",
            "var b = 1 + JSCompiler_inline_result$jscomp$0\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression11
#[test]
fn test_inline_reference_in_expression11() {
    let mut t = FunctionInjectorTest::set_up();
    // Call under label
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() {a:foo(1)?0:1 }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() {\n",
            "  a:{\n",
            "    var JSCompiler_inline_result$jscomp$0;\n",
            "    {JSCompiler_inline_result$jscomp$0=true;}\n",
            "    JSCompiler_inline_result$jscomp$0?0:1\n",
            "  }\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression12
#[test]
fn test_inline_reference_in_expression12() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;} function x() { 1?foo(1):1; }",
        concat!(
            "function foo(a){return true}\n",
            "function x() {\n",
            "  if(1) {\n",
            "    {true;}\n",
            "  } else {\n",
            "    1;\n",
            "  }\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression13
#[test]
fn test_inline_reference_in_expression13() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(a){return true;}; function x() { goo() + (1?foo(1):1) }",
        concat!(
            "function foo(a){return true;};\n",
            "function x() { var JSCompiler_temp_const$jscomp$0=goo();\n",
            "var JSCompiler_temp$jscomp$1;\n",
            "if(1) {\n",
            "  {JSCompiler_temp$jscomp$1=true;}\n",
            "} else {\n",
            "  JSCompiler_temp$jscomp$1=1;\n",
            "}\n",
            "JSCompiler_temp_const$jscomp$0 + JSCompiler_temp$jscomp$1\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression14
#[test]
fn test_inline_reference_in_expression14() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() { z.gack = foo(1) }\n",
        ),
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() {\n",
            "var JSCompiler_temp_const$jscomp$0=z;\n",
            "var JSCompiler_inline_result$jscomp$1;\n",
            "{\n",
            "z= {};\n",
            "JSCompiler_inline_result$jscomp$1 = true;\n",
            "}\n",
            "JSCompiler_temp_const$jscomp$0.gack = JSCompiler_inline_result$jscomp$1;\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression15
#[test]
fn test_inline_reference_in_expression15() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() { z.gack = foo.call(this,1) }\n",
        ),
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() {\n",
            "var JSCompiler_temp_const$jscomp$0=z;\n",
            "var JSCompiler_inline_result$jscomp$1;\n",
            "{\n",
            "z= {};\n",
            "JSCompiler_inline_result$jscomp$1 = true;\n",
            "}\n",
            "JSCompiler_temp_const$jscomp$0.gack = JSCompiler_inline_result$jscomp$1;\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression16
#[test]
fn test_inline_reference_in_expression16() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() { z[bar()] = foo(1) }\n",
        ),
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() {\n",
            "var JSCompiler_temp_const$jscomp$1=z;\n",
            "var JSCompiler_temp_const$jscomp$0=bar();\n",
            "var JSCompiler_inline_result$jscomp$2;\n",
            "{\n",
            "z= {};\n",
            "JSCompiler_inline_result$jscomp$2 = true;\n",
            "}\n",
            "JSCompiler_temp_const$jscomp$1[JSCompiler_temp_const$jscomp$0] =\n",
            "JSCompiler_inline_result$jscomp$2;\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceToFunction_passingLiteralThis_andSideEffectfulArg
#[test]
fn test_inline_reference_to_function_passing_literal_this_and_side_effectful_arg() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        concat!(
            "function foo(y) { bar(this, y, y); }\n",
            "function x() {foo.call(this, sideEffects());}\n",
        ),
        concat!(
            "function foo(y) { bar(this, y, y); }\n",
            "function x() {\n",
            "  {\n",
            "    var y$jscomp$inline_0 = sideEffects();\n",
            "    bar(this, y$jscomp$inline_0, y$jscomp$inline_0);\n",
            "  }\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceToFunction_passingSideEffectfulThis
#[test]
fn test_inline_reference_to_function_passing_side_effectful_this() {
    let mut t = FunctionInjectorTest::set_up();
    t.assume_strict_this = true;
    t.helper_inline_reference_to_function(
        concat!(
            "function foo() { bar.call(this, this); }\n",
            "function x() {foo.call(computeThis());}\n",
        ),
        concat!(
            "function foo() { bar.call(this, this); }\n",
            "function x(){\n",
            "  {\n",
            "    var JSCompiler_inline_this_0 = computeThis();\n",
            "    bar.call(JSCompiler_inline_this_0, JSCompiler_inline_this_0);\n",
            "  }\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineReferenceInExpression17
#[test]
fn test_inline_reference_in_expression17() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() { z.y.x.gack = foo(1) }\n",
        ),
        concat!(
            "var z = {};\n",
            "function foo(a){z = {};return true;};\n",
            "function x() {\n",
            "var JSCompiler_temp_const$jscomp$0=z.y.x;\n",
            "var JSCompiler_inline_result$jscomp$1;\n",
            "{\n",
            "z= {};\n",
            "JSCompiler_inline_result$jscomp$1 = true;\n",
            "}\n",
            "JSCompiler_temp_const$jscomp$0.gack = JSCompiler_inline_result$jscomp$1;\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineWithinCalls1
#[test]
fn test_inline_within_calls1() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in within a call
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(){return _g;}; function x() {1 + foo()() }",
        concat!(
            "function foo(){return _g;};\n",
            "function x() { var JSCompiler_inline_result$jscomp$0;\n",
            "{JSCompiler_inline_result$jscomp$0=_g;}\n",
            "1 + JSCompiler_inline_result$jscomp$0() }\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineWithinSuperCall
#[test]
fn test_inline_within_super_call() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in within a call
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        concat!(
            "class A { constructor(g) {} }\n",
            "class B extends A {\n",
            "  constructor() { super(foo()); }\n",
            "}\n",
            "function foo() {\n",
            "  return '';\n",
            "}\n",
        ),
        concat!(
            "class A { constructor(g) {} }\n",
            "class B extends A {\n",
            "  constructor() {\n",
            "    var JSCompiler_inline_result$jscomp$0;\n",
            "    {\n",
            "      JSCompiler_inline_result$jscomp$0 = '';\n",
            "    }\n",
            "    super(JSCompiler_inline_result$jscomp$0)\n",
            "  }\n",
            "}\n",
            "function foo() {\n",
            "  return '';\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineWithinSuperCall_followingCall
#[test]
fn test_inline_within_super_call_following_call() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in within a call
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        concat!(
            "class A { constructor(g) {} }\n",
            "class B extends A {\n",
            "  constructor() { super(goo(), foo()); }\n",
            "}\n",
            "function foo() {\n",
            "  return '';\n",
            "}\n",
        ),
        concat!(
            "class A { constructor(g) {} }\n",
            "class B extends A {\n",
            "  constructor() {\n",
            "    var JSCompiler_temp_const$jscomp$0=goo();\n",
            "    var JSCompiler_inline_result$jscomp$1;\n",
            "    {\n",
            "      JSCompiler_inline_result$jscomp$1 = '';\n",
            "    }\n",
            "    super(JSCompiler_temp_const$jscomp$0, JSCompiler_inline_result$jscomp$1)\n",
            "  }\n",
            "}\n",
            "function foo() {\n",
            "  return '';\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testInlineAssignmentToConstant
#[test]
fn test_inline_assignment_to_constant() {
    let mut t = FunctionInjectorTest::set_up();
    // Call in within a call
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        "function foo(){return _g;}; function x(){var CONSTANT_RESULT = foo(); }",
        concat!(
            "function foo(){return _g;};\n",
            "function x() {\n",
            "  var JSCompiler_inline_result$jscomp$0;\n",
            "  {JSCompiler_inline_result$jscomp$0=_g;}\n",
            "  var CONSTANT_RESULT = JSCompiler_inline_result$jscomp$0;\n",
            "}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testBug1897706
#[test]
fn test_bug1897706() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(a){}; foo(x())",
        "function foo(a){}; {var a$jscomp$inline_0=x()}",
        "foo",
        INLINE_BLOCK,
    );
    t.helper_inline_reference_to_function(
        "function foo(a){bar()}; foo(x())",
        "function foo(a){bar()}; {var a$jscomp$inline_0=x();bar()}",
        "foo",
        INLINE_BLOCK,
    );
    t.helper_inline_reference_to_function(
        "function foo(a,b){bar()}; foo(x(),y())",
        concat!(
            "function foo(a,b){bar()};\n",
            "{var a$jscomp$inline_0=x();var b$jscomp$inline_1=y();bar()}\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testIssue1101a
#[test]
fn test_issue1101a() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return modifiyX() + a;} foo(x);",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testIssue1101b
#[test]
fn test_issue1101b() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a){return (x.prop = 2),a;} foo(x.prop);",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReference_isNo_ifCalledWithSpread
#[test]
fn test_can_inline_reference_is_no_if_called_with_spread() {
    let t = FunctionInjectorTest::set_up();
    t.helper_can_inline_reference_to_function(
        CanInlineResult::NO,
        "function foo(a) { return a; };\nfoo(...b);\n",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReference_direct_ifArgExpression_containsSpread
#[test]
fn test_can_inline_reference_direct_if_arg_expression_contains_spread() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(a) { return a; };\nfoo([...b]);\n",
        "function foo(a) { return a; };\n[...b]\n",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReference_direct_ifResultIsSpread
#[test]
fn test_can_inline_reference_direct_if_result_is_spread() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(b) { return [1, 2, b]; }\nbar(...foo(5));\n",
        "function foo(b) { return [1, 2, b]; }\nbar(...[1, 2, 5]);\n",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReference_block_ifResultIsSpread
#[test]
fn test_can_inline_reference_block_if_result_is_spread() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        concat!(
            "function foo(b) {\n",
            "  return [1, 2, b];\n",
            "}\n",
            "const bar = function() { };\n",
            "\n",
            "bar(...foo(5));\n",
        ),
        concat!(
            "function foo(b) {\n",
            "  return [1, 2, b];\n",
            "}\n",
            "const bar = function() { };\n",
            "\n",
            "var JSCompiler_inline_result$jscomp$0\n",
            "{\n",
            "  JSCompiler_inline_result$jscomp$0 = [1, 2, 5];\n",
            "}\n",
            "bar(...JSCompiler_inline_result$jscomp$0);\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testCanInlineReference_direct_ifPreviousSibling_isSpread
#[test]
fn test_can_inline_reference_direct_if_previous_sibling_is_spread() {
    let t = FunctionInjectorTest::set_up();
    t.helper_inline_reference_to_function(
        "function foo(b) { return b + 1; }\nbar(...qux(), foo(5));\n",
        "function foo(b) { return b + 1; }\nbar(...qux(), 5 + 1);\n",
        "foo",
        INLINE_DIRECT,
    );
}

// port: FunctionInjectorTest#testCanInlineReference_block_ifPreviousSibling_isSpread
#[test]
fn test_can_inline_reference_block_if_previous_sibling_is_spread() {
    let mut t = FunctionInjectorTest::set_up();
    t.allow_decomposition = true;
    t.helper_inline_reference_to_function(
        concat!(
            "function foo(b) {\n",
            "  return [1, 2, b];\n",
            "}\n",
            "const bar = function() { };\n",
            "\n",
            "bar(...qux(), foo(5));\n",
        ),
        concat!(
            "function foo(b) {\n",
            "  return [1, 2, b];\n",
            "}\n",
            "const bar = function() { };\n",
            "\n",
            "var JSCompiler_temp_const$jscomp$0 = [...qux()];\n",
            "var JSCompiler_inline_result$jscomp$1\n",
            "{\n",
            "  JSCompiler_inline_result$jscomp$1 = [1, 2, 5];\n",
            "}\n",
            "bar(...JSCompiler_temp_const$jscomp$0, JSCompiler_inline_result$jscomp$1);\n",
        ),
        "foo",
        INLINE_BLOCK,
    );
}

// port: FunctionInjectorTest#testArgumentsReferenceInArrowFunction
#[test]
fn test_arguments_reference_in_arrow_function() {
    let t = FunctionInjectorTest::set_up();
    assert!(!t.does_function_meet_minimum_requirements(
        "function foo() { return () => arguments; }",
        "foo"
    ));
}

// port: FunctionInjectorTest#testArgumentsReferenceInNestedVanillaFunction
#[test]
fn test_arguments_reference_in_nested_vanilla_function() {
    let t = FunctionInjectorTest::set_up();
    assert!(t.does_function_meet_minimum_requirements(
        "function foo() { return function() { return arguments; }; }",
        "foo"
    ));
}
