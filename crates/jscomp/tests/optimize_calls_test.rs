/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ReferenceMap.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/OptimizeCallsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of OptimizeCallsTest. The tests run through the native CompilerTestCase port of
//! crates/testing (`test(externs(..), srcs(..))` with Normalize and GatherExternProperties
//! enabled, as the Java setUp does). Java's test keeps the collected `ReferenceMap` and its
//! Nodes in the field `references`; the Nodes live in the compiler's arena here, so the
//! capturing pass records, while the compiler is at hand, each key with the token of every
//! reference Node and what `OptimizeCalls.isAllowedReference` says about it (the only Node
//! facts the Java assertions read).
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    optimize_calls::{OptimizeCalls, ReferenceMap},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

/// One reference Node of the ReferenceMap: its token and `OptimizeCalls.isAllowedReference`.
#[derive(Clone, Debug)]
struct Ref {
    token: Token,
    allowed: bool,
}

/// The `references` field: the name and property references, keys in the map's order.
#[derive(Clone, Debug, Default)]
struct References {
    names: Vec<(String, Vec<Ref>)>,
    props: Vec<(String, Vec<Ref>)>,
}

impl References {
    fn of(
        compiler: &AbstractCompiler,
        map: &IndexMap<JsString, Vec<NodeId>>,
    ) -> Vec<(String, Vec<Ref>)> {
        map.iter()
            .map(|(key, nodes)| {
                (
                    key.to_string(),
                    nodes
                        .iter()
                        .map(|&n| Ref {
                            token: n.get_token(compiler),
                            allowed: OptimizeCalls::is_allowed_reference(compiler, n),
                        })
                        .collect(),
                )
            })
            .collect()
    }

    // port: ReferenceMap#getNameReferences
    fn get_name_references(&self) -> &[(String, Vec<Ref>)] {
        &self.names
    }

    // port: ReferenceMap#getPropReferences
    fn get_prop_references(&self) -> &[(String, Vec<Ref>)] {
        &self.props
    }
}

struct OptimizeCallsTest {
    ctx: Ctx,
    // Will be assigned with the references collected from the most recent pass.
    references: Rc<RefCell<Option<References>>>,
    // Whether to consider externs during the next collection. Must be explicitly set.
    consider_externs: Option<bool>,
}

impl CompilerTestCaseHooks for OptimizeCallsTest {
    // port: OptimizeCallsTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let references = self.references.clone();
        let pass = OptimizeCalls::builder()
            .set_compiler(&compiler.borrow())
            .set_consider_externs(
                self.consider_externs
                    .expect("considerExterns must be explicitly set"),
            )
            .add_pass(Box::new(
                move |compiler: &mut AbstractCompiler,
                      _externs: NodeId,
                      _root: NodeId,
                      map: &mut ReferenceMap| {
                    *references.borrow_mut() = Some(References {
                        names: References::of(compiler, map.get_name_references()),
                        props: References::of(compiler, map.get_prop_references()),
                    });
                },
            ))
            .build();
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
    }

    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl OptimizeCallsTest {
    // port: OptimizeCallsTest#setUp
    fn set_up() -> (CompilerTestCase, Self) {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_normalize().unwrap(); // Required for `OptimizeCalls`.
        // Even if we're ignoring externs we need to know their names to do that.
        harness.enable_gather_extern_properties().unwrap();
        let test = Self {
            ctx: Ctx::new(
                "OptimizeCallsTest".into(),
                object([]),
                IndexMap::<_, _>::default(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            references: Rc::new(RefCell::new(None)),
            consider_externs: None,
        };
        (harness, test)
    }

    fn references(&self) -> References {
        self.references
            .borrow()
            .clone()
            .expect("the call graph pass ran")
    }
}

// port: CompilerTestCase#externs(String)
fn externs(code: &str) -> TestPart {
    TestPart::Externs(CompilerTestCase::externs(code))
}

// port: CompilerTestCase#srcs(String)
fn srcs(code: &str) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs(code))
}

// port: OptimizeCallsTest#KEY_EQUALITY (Truth containsExactly: same keys, any order)
fn assert_keys_contain_exactly(entries: &[(String, Vec<Ref>)], expected: &[&str]) {
    let mut got = entries.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>();
    let mut want = expected.to_vec();
    got.sort_unstable();
    want.sort_unstable();
    assert_eq!(got, want);
}

// port: OptimizeCallsTest#HAS_TOKEN (Truth containsExactly(..).inOrder())
fn assert_tokens_in_order(entries: &[(String, Vec<Ref>)], key: &str, expected: &[Token]) {
    let refs = &entries
        .iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("no references for {key}"))
        .1;
    assert_eq!(
        refs.iter().map(|r| r.token).collect::<Vec<_>>(),
        expected.to_vec(),
        "tokens of {key}"
    );
}

const EXTERNS: &str = r#"function externFoo(dummyParam1, dummyParam2) {
  deadRef1;
}

var externSpace = {};

externSpace.externProp = {
  externChildProp: 0,

  externChildMethod(dummyParam3) { }
};

class ExternBar { }
"#;

#[test]
fn test_reference_collection_ignoring_externs() {
    let (mut harness, mut t) = OptimizeCallsTest::set_up();
    t.consider_externs = Some(false);

    harness
        .test(
            &mut t,
            vec![
                externs(EXTERNS),
                srcs(
                    r#"function foo(param1, param2) {
  var liveRef1;
  new ExternBar();
}

var space = {};

space.prop = {
  childProp: foo,

  childMethod(param3, externFoo) { }
};

class Bar { }

new Bar()
space.prop.childMethod(externSpace);
"#,
                ),
            ],
        )
        .unwrap();

    let references = t.references();
    // Only global names should be collected.
    assert_keys_contain_exactly(references.get_name_references(), &["Bar", "foo", "space"]);

    assert_keys_contain_exactly(
        references.get_prop_references(),
        &["childMethod", "childProp", "prop"],
    );
}

#[test]
fn test_reference_collection_considering_externs() {
    let (mut harness, mut t) = OptimizeCallsTest::set_up();
    t.consider_externs = Some(true);

    harness
        .test(
            &mut t,
            vec![
                externs(EXTERNS),
                srcs(
                    r#"function foo(param1, param2) {
  liveRef1;
  new ExternBar();
}

var space = {};

space.prop = {
  childProp: foo,

  childMethod(param3, externFoo) { }
};

class Bar { }

new Bar()
space.prop.childMethod(externSpace);
"#,
                ),
            ],
        )
        .unwrap();

    let references = t.references();
    // Only global names should be collected.
    assert_keys_contain_exactly(
        references.get_name_references(),
        &[
            "Bar",
            "ExternBar",
            "externFoo",
            "externSpace",
            "foo",
            "space",
        ],
    );

    assert_keys_contain_exactly(
        references.get_prop_references(),
        &[
            "childMethod",
            "childProp",
            "externChildMethod",
            "externChildProp",
            "externProp",
            "prop",
        ],
    );
}

#[test]
fn test_reference_collection_does_not_collect_the_empty_name() {
    let (mut harness, mut t) = OptimizeCallsTest::set_up();
    t.consider_externs = Some(true);

    harness
        .test(&mut t, vec![srcs("var f = function() { }")])
        .unwrap();

    // Anonymous functions have the empty string as a name.
    assert!(
        !t.references()
            .get_name_references()
            .iter()
            .any(|(k, _)| k.is_empty())
    );
}

#[test]
fn test_reference_collection_finds_super_calls_simple_names() {
    let (mut harness, mut t) = OptimizeCallsTest::set_up();
    t.consider_externs = Some(true);

    harness
        .test(
            &mut t,
            vec![srcs(
                r#"class SuperClass {
  constructor() {}
}
class SubClass extends SuperClass {
  constructor() {
    super();
  }
}
new SuperClass();
new SubClass();
"#,
            )],
        )
        .unwrap();

    let name_to_refs = t.references().names;
    assert_keys_contain_exactly(&name_to_refs, &["SuperClass", "SubClass"]);

    assert_tokens_in_order(
        &name_to_refs,
        "SuperClass",
        &[Token::NAME, Token::NAME, Token::SUPER, Token::NAME],
    );

    assert_tokens_in_order(&name_to_refs, "SubClass", &[Token::NAME, Token::NAME]);
}

#[test]
fn test_reference_collection_finds_super_calls_qualified_names() {
    let (mut harness, mut t) = OptimizeCallsTest::set_up();
    t.consider_externs = Some(true);

    harness
        .test(
            &mut t,
            vec![srcs(
                r#"const ns = {};
ns.SuperClass = class {
  constructor() {}
}
ns.SubClass = class extends ns.SuperClass {
  constructor() {
    super();
  }
}
new ns.SuperClass();
new ns.SubClass();
"#,
            )],
        )
        .unwrap();

    let name_to_refs = t.references().props;
    assert_keys_contain_exactly(&name_to_refs, &["SuperClass", "SubClass", "constructor"]);

    assert_tokens_in_order(
        &name_to_refs,
        "SuperClass",
        &[Token::GETPROP, Token::GETPROP, Token::SUPER, Token::GETPROP],
    );

    assert_tokens_in_order(&name_to_refs, "SubClass", &[Token::GETPROP, Token::GETPROP]);
}

#[test]
fn test_reference_collection_finds_class_fields() {
    let (mut harness, mut t) = OptimizeCallsTest::set_up();
    t.consider_externs = Some(true);

    harness
        .test(
            &mut t,
            vec![srcs(
                r#"class C {
  a = 2;
  b;
  ['c'] = 'hi';
  'd' = 5;
  1 = 2;
  e = function() { return 1; };
  ['f'] = function z() { return 2; };
}
new C();
"#,
            )],
        )
        .unwrap();

    let references = t.references();
    assert_keys_contain_exactly(references.get_name_references(), &["C"]);

    assert_keys_contain_exactly(references.get_prop_references(), &["a", "b", "e"]);

    let name_to_refs = references.props;
    assert_keys_contain_exactly(&name_to_refs, &["a", "b", "e"]);

    assert_tokens_in_order(&name_to_refs, "a", &[Token::MEMBER_FIELD_DEF]);

    assert_tokens_in_order(&name_to_refs, "b", &[Token::MEMBER_FIELD_DEF]);

    assert_tokens_in_order(&name_to_refs, "e", &[Token::MEMBER_FIELD_DEF]);
}

#[test]
fn test_reference_collection_reflected_props() {
    let (mut harness, mut t) = OptimizeCallsTest::set_up();
    t.consider_externs = Some(false);

    harness
        .test(
            &mut t,
            vec![srcs(
                r#"class C {
  m() {
    console.log('hello');
  }
}
C.prototype[goog.reflect.objectProperty('m', C.prototype)]();
"#,
            )],
        )
        .unwrap();

    let name_to_refs = t.references().props;
    assert_tokens_in_order(
        &name_to_refs,
        "m",
        &[Token::MEMBER_FUNCTION_DEF, Token::CALL],
    );
}

#[test]
fn test_is_allowed_reference_tagged_template_literal() {
    let (mut harness, mut t) = OptimizeCallsTest::set_up();
    t.consider_externs = Some(false);

    harness
        .test(
            &mut t,
            vec![srcs(
                r#"var obj = {
  tag() {}
};
obj.tag`template`;
"#,
            )],
        )
        .unwrap();

    let name_to_refs = t.references().names;
    let obj_refs = name_to_refs
        .iter()
        .find(|(k, _)| k == "obj")
        .map(|(_, refs)| refs);
    assert!(obj_refs.is_some());
    let mut has_disallowed_ref = false;
    for r in obj_refs.unwrap() {
        if !r.allowed {
            has_disallowed_ref = true;
        }
    }
    assert!(has_disallowed_ref);
}
