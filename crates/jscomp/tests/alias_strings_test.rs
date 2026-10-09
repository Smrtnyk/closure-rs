/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/AliasStringsTest.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

//! Port of `AliasStringsTest.java`: tests for `AliasStrings`.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler, alias_strings::AliasStrings,
    compiler_options::AliasStringsMode, compiler_pass::CompilerPass, js_chunk::JSChunk,
    replace_messages_constants,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    testing::js_chunk_graph_builder::JSChunkGraphBuilder,
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

// port: AliasStringsTest#EXTERNS
const EXTERNS: &str = "alert";

struct AliasStringsTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    hash_reduction: bool,
    alias_strings_mode: AliasStringsMode,
}

impl CompilerTestCaseHooks for Hooks {
    // port: AliasStringsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let mut pass = AliasStrings::new(false, self.alias_strings_mode);
        if self.hash_reduction {
            pass.unit_test_hash_reduction_mask = 0;
        }
        let pass: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                pass.process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "AliasStringsTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl AliasStringsTest {
    // port: AliasStringsTest#AliasStringsTest
    // port: AliasStringsTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(EXTERNS);
        harness.set_up();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "AliasStringsTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                hash_reduction: false,
                alias_strings_mode: AliasStringsMode::ALL,
            },
        }
    }

    fn test(&mut self, js: &str, expected: &str) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_same(&mut self, js: &str) {
        self.harness
            .test_same_string(&mut self.hooks, js)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_chunks(&mut self, chunks: Vec<JSChunk>, expected: &[&str]) {
        let expected: Vec<JsString> = expected.iter().map(|s| JsString::from(*s)).collect();
        self.harness
            .test(
                &mut self.hooks,
                vec![
                    TestPart::Sources(CompilerTestCase::srcs_chunks(chunks)),
                    TestPart::Expected(CompilerTestCase::expected_strings(&expected)),
                ],
            )
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

// port: AliasStringsTest#testTemplateLiteral
#[test]
fn test_template_literal() {
    let mut t = AliasStringsTest::new();
    // TODO(bradfordcsmith): Maybe implement using aliases in template literals?
    t.test(
        "const A = 'aliasable string';\nconst B = 'aliasable string';\nconst AB = `${A}aliasable string${B}`;\n",
        "var $$S_aliasable$20string = 'aliasable string';\nconst A = $$S_aliasable$20string;\nconst B = $$S_aliasable$20string;\nconst AB = `${A}aliasable string${B}`\n",
    );
}

// port: AliasStringsTest#testAliasAggressively
#[test]
fn test_alias_aggressively() {
    let mut t = AliasStringsTest::new();
    t.test_same("function f() { return 'aliasable string'; }");

    t.hooks.alias_strings_mode = AliasStringsMode::ALL_AGGRESSIVE;
    t.test(
        "function f() { return 'aliasable string'; }",
        "var $$S_aliasable$20string = 'aliasable string';\nfunction f() { return $$S_aliasable$20string; }\n",
    );
}

// port: AliasStringsTest#testProtectedMessage
#[test]
fn test_protected_message() {
    let mut t = AliasStringsTest::new();
    t.test(
        &"const A = 'aliasable string';\nconst B = 'aliasable string';\nvar MSG_A =\n    DEFINE_MSG_CALLEE(\n        {\n          \"key\":    \"MSG_A\",\n          \"msg_text\":\"aliasable string\",\n        });\n\n"
            .replace("DEFINE_MSG_CALLEE", replace_messages_constants::DEFINE_MSG_CALLEE),
        // msg_text's string is left unchanged instead of using an alias,
        // because `ReplaceMessages` needs the literal string here.
        &"var $$S_aliasable$20string = 'aliasable string';\nconst A = $$S_aliasable$20string;\nconst B = $$S_aliasable$20string;\nvar MSG_A =\n    DEFINE_MSG_CALLEE(\n        {\n          \"key\":    \"MSG_A\",\n          \"msg_text\":\"aliasable string\",\n        });\n\n"
            .replace("DEFINE_MSG_CALLEE", replace_messages_constants::DEFINE_MSG_CALLEE),
    );
}

// port: AliasStringsTest#testLongStableAlias
#[test]
fn test_long_stable_alias() {
    let mut t = AliasStringsTest::new();
    // Check long strings get a hash code

    t.test(
        "a='Antidisestablishmentarianism';\nb='Antidisestablishmentarianism';\n",
        "var $$S_Antidisestablishment_e428eaa9=\n  'Antidisestablishmentarianism';\na=$$S_Antidisestablishment_e428eaa9;\nb=$$S_Antidisestablishment_e428eaa9\n",
    );

    // Check that small changes give different hash codes

    t.test(
        "a='AntidisestablishmentarianIsm';\nb='AntidisestablishmentarianIsm';\n",
        "var $$S_Antidisestablishment_e4287289=\n  'AntidisestablishmentarianIsm';\na=$$S_Antidisestablishment_e4287289;\nb=$$S_Antidisestablishment_e4287289\n",
    );

    // TODO(user): check that hash code collisions are handled.
}

// port: AliasStringsTest#testLongStableAliasHashCollision
#[test]
fn test_long_stable_alias_hash_collision() {
    let mut t = AliasStringsTest::new();
    t.hooks.hash_reduction = true;

    // Check that hash code collisions generate different alias
    // variable names

    t.test(
        "f('Antidisestablishmentarianism');\nf('Antidisestablishmentarianism');\nf('Antidisestablishmentarianismo');\nf('Antidisestablishmentarianismo');\n",
        "var $$S_Antidisestablishment_0=\n  'Antidisestablishmentarianism';\nvar $$S_Antidisestablishment_0_1=\n  'Antidisestablishmentarianismo';\nf($$S_Antidisestablishment_0);\nf($$S_Antidisestablishment_0);\nf($$S_Antidisestablishment_0_1);\nf($$S_Antidisestablishment_0_1);\n",
    );
}

// port: AliasStringsTest#testStringsThatAreGlobalVarValues
#[test]
fn test_strings_that_are_global_var_values() {
    let mut t = AliasStringsTest::new();

    t.test_same("var foo='foo'; var bar='';");

    // Regular array
    t.test_same("var foo=['foo','bar'];");

    // Nested array
    t.test_same("var foo=['foo',['bar']];");

    // Same string is in a global array and a local in a function
    t.test_same("var foo=['foo', 'bar'];function bar() {return 'foo';}");

    // Regular object literal
    t.test_same("var foo={'foo': 'bar'};");

    // Nested object literal
    t.test_same("var foo={'foo': {'bar': 'baz'}};");

    // Same string is in a global object literal (as key) and local in a
    // function
    t.test_same("var foo={'foo': 'bar'};function bar() {return 'foo';}");

    // Same string is in a global object literal (as value) and local in a
    // function
    t.test_same("var foo={'foo': 'foo'};function bar() {return 'foo';}");
}

// port: AliasStringsTest#testStringsInChunks
#[test]
fn test_strings_in_chunks() {
    let mut t = AliasStringsTest::new();

    // Aliases must be placed in the correct chunk. The alias for
    // '------adios------' must be lifted from m2 and m3 and go in the
    // common parent chunks m1

    let chunks = JSChunkGraphBuilder::for_bush()
        .add_chunk(
            "function f(a) { alert('ffffffffffffffffffff' + 'ffffffffffffffffffff' + a); }\nfunction g() { alert('ciaociaociaociaociao'); }\n",
        )
        .add_chunk(
            "f('---------hi---------');f('bye');function h(a) { alert('hhhhhhhhhhhhhhhhhhhh'\n + 'hhhhhhhhhhhhhhhhhhhh' + a); }\n",
        )
        .add_chunk(
            "f('---------hi---------');h('ciaociaociaociaociao' +\n '--------adios-------');(function() { alert('zzzzzzzzzzzzzzzzzzzz' +\n 'zzzzzzzzzzzzzzzzzzzz'); })();\n",
        )
        .add_chunk(
            "f('---------hi---------'); alert('--------adios-------');\nh('-------peaches------'); h('-------peaches------');\n",
        )
        .build();

    t.test_chunks(
        chunks,
        &[
            // m1
            "var $$S_ciaociaociaociaociao = 'ciaociaociaociaociao';\nvar $$S_ffffffffffffffffffff = 'ffffffffffffffffffff';\nfunction f(a) {\n  alert($$S_ffffffffffffffffffff + $$S_ffffffffffffffffffff + a);\n}\nfunction g() { alert($$S_ciaociaociaociaociao); }\n",
            // m2
            "var $$S_$2d$2d$2d$2d$2d$2d$2d$2d$2dhi$2d$2d$2d$2d$2d$2d$2d$2d$2d\n = '---------hi---------';\nvar $$S_$2d$2d$2d$2d$2d$2d$2d$2d_adios$2d$2d$2d$2d$2d$2d$2d\n = '--------adios-------';\nvar $$S_hhhhhhhhhhhhhhhhhhhh = 'hhhhhhhhhhhhhhhhhhhh';\nf($$S_$2d$2d$2d$2d$2d$2d$2d$2d$2dhi$2d$2d$2d$2d$2d$2d$2d$2d$2d);\nf('bye');\nfunction h(a) {\n  alert($$S_hhhhhhhhhhhhhhhhhhhh + $$S_hhhhhhhhhhhhhhhhhhhh + a);\n}\n",
            // m3
            "var $$S_zzzzzzzzzzzzzzzzzzzz = 'zzzzzzzzzzzzzzzzzzzz';\nf($$S_$2d$2d$2d$2d$2d$2d$2d$2d$2dhi$2d$2d$2d$2d$2d$2d$2d$2d$2d);\nh($$S_ciaociaociaociaociao + \n$$S_$2d$2d$2d$2d$2d$2d$2d$2d_adios$2d$2d$2d$2d$2d$2d$2d);\n(function() { alert($$S_zzzzzzzzzzzzzzzzzzzz + $$S_zzzzzzzzzzzzzzzzzzzz)\n })();\n",
            // m4
            "var $$S_$2d$2d$2d$2d$2d$2d$2dpeaches$2d$2d$2d$2d$2d$2d\n = '-------peaches------';\nf($$S_$2d$2d$2d$2d$2d$2d$2d$2d$2dhi$2d$2d$2d$2d$2d$2d$2d$2d$2d);\nalert($$S_$2d$2d$2d$2d$2d$2d$2d$2d_adios$2d$2d$2d$2d$2d$2d$2d);\nh($$S_$2d$2d$2d$2d$2d$2d$2dpeaches$2d$2d$2d$2d$2d$2d);\nh($$S_$2d$2d$2d$2d$2d$2d$2dpeaches$2d$2d$2d$2d$2d$2d);\n",
        ],
    );
}

// port: AliasStringsTest#testStringsInChunks2
#[test]
fn test_strings_in_chunks2() {
    let mut t = AliasStringsTest::new();

    // Aliases must be placed in the correct chunk. The alias for
    // '------adios------' must be lifted from m2 and m3 and go in the
    // common parent chunk m1

    let chunks = JSChunkGraphBuilder::for_bush()
        .add_chunk("function g() { alert('ciaociaociaociaociao'); }")
        .add_chunk(
            "function h(a) {\n  alert('hhhhhhhhhhhhhhhhhhh:' + a);\n  alert('hhhhhhhhhhhhhhhhhhh:' + a);\n}\n",
        )
        .add_chunk("h('ciaociaociaociaociao' + 'adios');")
        .add_chunk("g();")
        .build();

    t.test_chunks(
        chunks,
        &[
            // m1
            "var $$S_ciaociaociaociaociao = 'ciaociaociaociaociao';\nfunction g() { alert($$S_ciaociaociaociaociao); }\n",
            // m2
            "var $$S_hhhhhhhhhhhhhhhhhhh$3a = 'hhhhhhhhhhhhhhhhhhh:';\nfunction h(a) {\n  alert($$S_hhhhhhhhhhhhhhhhhhh$3a + a);\n  alert($$S_hhhhhhhhhhhhhhhhhhh$3a + a);\n}\n",
            // m3
            "h($$S_ciaociaociaociaociao + 'adios');",
            // m4
            "g();",
        ],
    );
}

// port: AliasStringsTest#testAliasInCommonChunkInclusive
#[test]
fn test_alias_in_common_chunk_inclusive() {
    let mut t = AliasStringsTest::new();

    let chunks = JSChunkGraphBuilder::for_bush()
        .add_chunk("")
        .add_chunk("function g() { alert('ciaociaociaociaociao'); }")
        .add_chunk("h('ciaociaociaociaociao' + 'adios');")
        .add_chunk("g();")
        .build();

    // The "ciao" string is used in m1 and m2.
    // Since m2 depends on m1, we should create the alias there and not force it into m0.
    t.test_chunks(
        chunks,
        &[
            // m0
            "",
            // m1
            "var $$S_ciaociaociaociaociao = 'ciaociaociaociaociao';\nfunction g() { alert($$S_ciaociaociaociaociao); }\n",
            // m2
            "h($$S_ciaociaociaociaociao + 'adios');",
            // m3
            "g();",
        ],
    );
}

// port: AliasStringsTest#testEmptyChunks
#[test]
fn test_empty_chunks() {
    let mut t = AliasStringsTest::new();
    let chunks = JSChunkGraphBuilder::for_star()
        .add_chunk("")
        .add_chunk("function foo() { f('goodgoodgoodgoodgood') }")
        .add_chunk("function foo() { f('goodgoodgoodgoodgood') }")
        .build();

    t.test_chunks(
        chunks,
        &[
            // m0
            "var $$S_goodgoodgoodgoodgood='goodgoodgoodgoodgood'",
            // m1
            "function foo() {f($$S_goodgoodgoodgoodgood)}",
            // m2
            "function foo() {f($$S_goodgoodgoodgoodgood)}",
        ],
    );
}

// port: AliasStringsTest#testOnlyAliasLargeStrings
#[test]
fn test_only_alias_large_strings() {
    let mut t = AliasStringsTest::new();
    t.hooks.alias_strings_mode = AliasStringsMode::LARGE;

    t.test(
        "const A = 'non aliasable string with length <= 100 characters';\nconst B = 'non aliasable string with length <= 100 characters';\nconst C = 'aliasable large string largestringlargestringlargestringlargestringlargestringlargestringlargestring!';\nconst D = 'aliasable large string largestringlargestringlargestringlargestringlargestringlargestringlargestring!';\n",
        "var $$S_aliasable$20large$20stri_6c7cf169 = 'aliasable large string largestringlargestringlargestringlargestringlargestringlargestringlargestring!';\nconst A = 'non aliasable string with length <= 100 characters';\nconst B = 'non aliasable string with length <= 100 characters';\nconst C = $$S_aliasable$20large$20stri_6c7cf169;\nconst D = $$S_aliasable$20large$20stri_6c7cf169;\n",
    );
}
