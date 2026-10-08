/*
 * Copyright 2004 The Closure Compiler Authors.
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
/*
 * Copyright 2026 The closure-rs Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   test/com/google/javascript/jscomp/PureFunctionIdentifierTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of PureFunctionIdentifierTest. The tests run through the native CompilerTestCase port of
//! crates/testing (`testSame(srcs(..))` with Normalize, GatherExternProperties and TypeCheck
//! enabled, as the Java setUp does). Java's test keeps the calls marked as side-effect free in
//! the field `noSideEffectCalls` and checks them in a postcondition; the Nodes live in the last
//! compiler's arena here, so the check runs on `getLastCompiler()` right after `testSame`.
//!
//! Only the tests that call `disableTypeCheck()` are here: every other test needs TypeCheck
//! and replays from corpus/unit/records.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    pure_function_identifier,
};
use closure_rhino::{js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{
        CompilerTestCase, CompilerTestCaseHooks, DEFAULT_EXTERNS, TestPart, contains_exactly_by,
    },
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    testing::js_comp_correspondences::equality_when_parsed_as_expression,
    throwable::Throwable,
};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

// port: PureFunctionIdentifierTest#TEST_EXTERNS
const TEST_EXTERNS_SUFFIX: &str = r#"var window; window.setTimeout;
/**@nosideeffects*/ function externSENone(){}
/**@modifies{this}*/ function externSEThis(){}
/**@constructor
 * @modifies{this}*/
function externObjSEThis(){}
/**
 * @param {string} s id.
 * @return {string}
 * @modifies{this}
 */
externObjSEThis.prototype.externObjSEThisMethod = function(s) {};
/**
 * @param {string} s id.
 * @return {string}
 * @modifies{arguments}
 */
externObjSEThis.prototype.externObjSEThisMethod2 = function(s) {};
/**@nosideeffects*/function Error(){}
function externSef1(){}
/**@nosideeffects*/function externNsef1(){}
var externSef2 = function(){};
/**@nosideeffects*/var externNsef2 = function(){};
var externNsef3 = /**@nosideeffects*/function(){};
var externObj;
externObj.sef1 = function(){};
/**@nosideeffects*/externObj.nsef1 = function(){};
externObj.nsef2 = /**@nosideeffects*/function(){};
externObj.partialFn = function(){};
externObj.partialSharedFn;
var externObj2;
externObj2.partialSharedFn = /**@nosideeffects*/function(){};
/**@constructor*/function externSefConstructor(){}
externSefConstructor.prototype.sefFnOfSefObj = function(){};
externSefConstructor.prototype.nsefFnOfSefObj =
  /**@nosideeffects*/function(){};
externSefConstructor.prototype.externShared = function(){};
/**@constructor@nosideeffects*/function externNsefConstructor(){}
externNsefConstructor.prototype.sefFnOfNsefObj = function(){};
externNsefConstructor.prototype.nsefFnOfNsefObj =
  /**@nosideeffects*/function(){};
externNsefConstructor.prototype.externShared =
  /**@nosideeffects*/function(){};
/**@constructor @nosideeffects*/function externNsefConstructor2(){}
externNsefConstructor2.prototype.externShared =
  /**@nosideeffects*/function(){};
externNsefConstructor.prototype.sharedPartialSef;
/**@nosideeffects*/externNsefConstructor.prototype.sharedPartialNsef;
// An externs definition with a stub before.
/**@constructor*/function externObj3(){}
externObj3.prototype.propWithStubBefore;
/**
 * @param {string} s id.
 * @return {string}
 * @nosideeffects
 */
externObj3.prototype.propWithStubBefore = function(s) {};
// useless JsDoc
/**
 * @see {foo}
 */
externObj3.prototype.propWithStubBeforeWithJSDoc;
/**
 * @param {string} s id.
 * @return {string}
 * @nosideeffects
 */
externObj3.prototype.propWithStubBeforeWithJSDoc = function(s) {};
// An externs definition with a stub after.
/**@constructor*/function externObj4(){}
/**
 * @param {string} s id.
 * @return {string}
 * @nosideeffects
 */
externObj4.prototype.propWithStubAfter = function(s) {};
externObj4.prototype.propWithStubAfter;
/**
 * @param {string} s id.
 * @return {string}
 * @nosideeffects
 */
externObj4.prototype.propWithStubAfterWithJSDoc = function(s) {};
// useless JsDoc
/**
 * @see {foo}
 */
externObj4.prototype.propWithStubAfterWithJSDoc;
goog.reflect = {};
goog.reflect.cache = function(a, b, c, opt_d) {};
/** @nosideeffects */
externObj.prototype.duplicateExternFunc = function() {};
externObj2.prototype.duplicateExternFunc = function() {};
externObj.prototype['weirdDefinition'] = function() {};
"#;

struct PureFunctionIdentifierTest {
    ctx: Ctx,
    no_side_effect_calls: Rc<RefCell<Option<Vec<NodeId>>>>,
    reg_exp_have_side_effects: bool,
}

/// Run PureFunctionIdentifier, then gather a list of calls that are marked as having no side
/// effects.
struct NoSideEffectCallEnumerator {
    no_side_effect_calls: Rc<RefCell<Option<Vec<NodeId>>>>,
    reg_exp_have_side_effects: bool,
}

impl CompilerPass for NoSideEffectCallEnumerator {
    // port: PureFunctionIdentifierTest.NoSideEffectCallEnumerator#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut no_side_effect_calls = Vec::new();
        // TODO(nickreid): Move these into 'getOptions' and 'getCompiler' overrides.
        compiler.set_has_reg_exp_global_references(self.reg_exp_have_side_effects);
        compiler
            .get_options_mut()
            .set_use_types_for_local_optimization(true);

        pure_function_identifier::Driver::new().process(compiler, externs, root);
        let mut visitor = Visitor {
            no_side_effect_calls: &mut no_side_effect_calls,
        };
        NodeTraversal::traverse(compiler, externs, &mut visitor);
        NodeTraversal::traverse(compiler, root, &mut visitor);
        *self.no_side_effect_calls.borrow_mut() = Some(no_side_effect_calls);
    }
}

/// The AbstractPostOrderCallback half of NoSideEffectCallEnumerator.
struct Visitor<'a> {
    no_side_effect_calls: &'a mut Vec<NodeId>,
}

impl Callback for Visitor<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: PureFunctionIdentifierTest.NoSideEffectCallEnumerator#visit
    #[allow(clippy::collapsible_if)] // Retain the Java branches.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        if n.is_new(compiler) {
            if !compiler
                .get_ast_analyzer()
                .constructor_call_has_side_effects(compiler, n)
            {
                self.no_side_effect_calls
                    .push(n.get_first_child(compiler).unwrap());
            }
        } else if NodeUtil::is_invocation(compiler, n) {
            if !compiler
                .get_ast_analyzer()
                .function_call_has_side_effects(compiler, n)
            {
                self.no_side_effect_calls
                    .push(n.get_first_child(compiler).unwrap());
            }
        }
    }
}

impl CompilerTestCaseHooks for PureFunctionIdentifierTest {
    // port: PureFunctionIdentifierTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass: Box<dyn CompilerPass> = Box::new(NoSideEffectCallEnumerator {
            no_side_effect_calls: self.no_side_effect_calls.clone(),
            reg_exp_have_side_effects: self.reg_exp_have_side_effects,
        });
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl PureFunctionIdentifierTest {
    // port: PureFunctionIdentifierTest#PureFunctionIdentifierTest
    // port: PureFunctionIdentifierTest#setUp
    fn set_up() -> (CompilerTestCase, Self) {
        let test_externs = DEFAULT_EXTERNS
            .as_ref()
            .expect("DEFAULT_EXTERNS")
            .concat(&JsString::from(TEST_EXTERNS_SUFFIX));
        let mut harness = CompilerTestCase::new(test_externs);
        harness.set_up();

        // Allow testing of features that are not yet fully supported.
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is
        // necessary.
        harness.enable_normalize_expected_output().unwrap();
        harness.disable_compare_js_doc().unwrap();
        harness.enable_gather_extern_properties().unwrap();
        harness.enable_type_check().unwrap();
        let test = Self {
            ctx: Ctx::new(
                "PureFunctionIdentifierTest".into(),
                object([]),
                IndexMap::new(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            no_side_effect_calls: Rc::new(RefCell::new(None)),
            reg_exp_have_side_effects: true,
        };
        (harness, test)
    }

    // port: PureFunctionIdentifierTest#assertNoPureCalls(String)
    fn assert_no_pure_calls(&mut self, harness: &mut CompilerTestCase, source: &str) {
        self.assert_pure_calls_marked(harness, source, &[]);
    }

    // port: PureFunctionIdentifierTest#assertPureCallsMarked(String,List,Postcondition)
    fn assert_pure_calls_marked(
        &mut self,
        harness: &mut CompilerTestCase,
        source: &str,
        expected: &[&str],
    ) {
        harness
            .test_same(
                self,
                vec![TestPart::Sources(CompilerTestCase::srcs(source))],
            )
            .unwrap();
        // postcondition: assertThat(noSideEffectCalls)
        //     .comparingElementsUsing(JSCompCorrespondences.EQUALITY_WHEN_PARSED_AS_EXPRESSION)
        //     .containsExactlyElementsIn(expected);
        let compiler = harness.get_last_compiler().expect("lastCompiler");
        let compiler = compiler.borrow();
        let calls = self
            .no_side_effect_calls
            .borrow()
            .clone()
            .expect("NoSideEffectCallEnumerator ran");
        contains_exactly_by(
            &calls,
            expected,
            false,
            |&n, e| equality_when_parsed_as_expression(&compiler, n, e),
            "noSideEffectCalls",
        )
        .unwrap_or_else(|e| {
            panic!(
                "{e:?}: actual {:?}, expected {expected:?}",
                calls
                    .iter()
                    .map(|&n| n.get_qualified_name(&compiler).map(|q| q.to_string()))
                    .collect::<Vec<_>>()
            )
        });
    }
}

#[test]
fn test_class_computed_field_initializer_side_effects() {
    let (mut harness, mut t) = PureFunctionIdentifierTest::set_up();
    harness.disable_type_check().unwrap();
    t.assert_no_pure_calls(
        &mut harness,
        r#"class C {
  ['x'] = externSef1();
}
new C();
"#,
    );
}

#[test]
fn test_class_field_pure_initializers() {
    let (mut harness, mut t) = PureFunctionIdentifierTest::set_up();
    harness.disable_type_check().unwrap();
    t.assert_pure_calls_marked(
        &mut harness,
        r#"class C {
  x = 1;
  ['y'] = 2;
}
new C();
"#,
        &["C"],
    );
}

#[test]
fn test_class_static_field_side_effects_do_not_affect_instantiation() {
    let (mut harness, mut t) = PureFunctionIdentifierTest::set_up();
    harness.disable_type_check().unwrap();
    t.assert_pure_calls_marked(
        &mut harness,
        r#"class C {
  static x = externSef1();
}
new C();
"#,
        &["C"],
    );
}

#[test]
fn test_extern_calls_no_type_checking() {
    let (mut harness, mut t) = PureFunctionIdentifierTest::set_up();
    t.test_extern_calls_for_type_inference_mode(&mut harness, /* typeChecked= */ false);
}

impl PureFunctionIdentifierTest {
    // port: PureFunctionIdentifierTest#testExternCallsForTypeInferenceMode
    fn test_extern_calls_for_type_inference_mode(
        &mut self,
        harness: &mut CompilerTestCase,
        type_checked: bool,
    ) {
        if type_checked {
            harness.enable_type_check().unwrap();
        } else {
            harness.disable_type_check().unwrap();
        }
        let prefix = "function f(){";
        let suffix = "} f()";

        self.assert_pure_calls_marked(
            harness,
            &format!("{prefix}externNsef1(){suffix}"),
            &["externNsef1", "f"],
        );
        self.assert_pure_calls_marked(
            harness,
            &format!("{prefix}externObj.nsef1(){suffix}"),
            &["externObj.nsef1", "f"],
        );

        self.assert_no_pure_calls(harness, &format!("{prefix}externSef1(){suffix}"));
        self.assert_no_pure_calls(harness, &format!("{prefix}externObj.sef1(){suffix}"));
    }
}

#[test]
fn test_mutates_this_in_arrow_function_new_receiver() {
    let (mut harness, mut t) = PureFunctionIdentifierTest::set_up();
    harness.disable_type_check().unwrap();
    // An arrow function's `this` is not its receiver, here it is the global `this`.
    t.assert_pure_calls_marked(
        &mut harness,
        r#"/** @constructor */ function Foo() {}
Foo.prototype.f = () => { this.x = 1; };
new Foo().f();
"#,
        &["Foo"],
    );
    t.assert_pure_calls_marked(
        &mut harness,
        r#"function setX() { this.x = 1; }
/** @constructor */ function Foo() {}
Foo.prototype.f = () => { setX.call(this); };
new Foo().f();
"#,
        &["Foo"],
    );
    t.assert_pure_calls_marked(
        &mut harness,
        r#"/** @constructor */ function Foo() {}
Foo.prototype.f = () => { this.x = 1; };
Foo.prototype.run = function() { this.f(); };
new Foo().run();
"#,
        &["Foo"],
    );
}
