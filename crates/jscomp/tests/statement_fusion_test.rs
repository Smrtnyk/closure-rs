/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/StatementFusionTest.java.

//! Port of `StatementFusionTest.java`: unit tests for StatementFusion.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    peephole_optimizations_pass::PeepholeOptimizationsPass, statement_fusion::StatementFusion,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct StatementFusionTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: StatementFusionTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let name = self.get_name();
        let pass: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> =
                    vec![Box::new(StatementFusion::new())];
                let mut peephole_pass = PeepholeOptimizationsPass::new(name.clone(), optimizations);
                peephole_pass.process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "StatementFusionTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl StatementFusionTest {
    // port: CompilerTestCase#CompilerTestCase()
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "StatementFusionTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
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

    // port: StatementFusionTest#fuse
    fn fuse(&mut self, before: &str, after: &str) {
        self.test(
            &format!("function F(){{if(CONDITION){{{before}}}}}"),
            &format!("function F(){{if(CONDITION){{{after}}}}}"),
        );
    }

    // port: StatementFusionTest#fuseSame
    fn fuse_same(&mut self, code: &str) {
        self.test_same(&format!("function F(){{if(CONDITION){{{code}}}}}"));
    }
}

fn language_mode(name: &str) -> DslValue {
    DslValue::Enum {
        class: "com.google.javascript.jscomp.CompilerOptions$LanguageMode".into(),
        name: name.into(),
    }
}

// port: StatementFusionTest#testNothingToDo
#[test]
fn test_nothing_to_do() {
    let mut t = StatementFusionTest::new();
    t.fuse_same("");
    t.fuse_same("a");
    t.fuse_same("a()");
    t.fuse_same("if(a()){}");
}

// port: StatementFusionTest#testFoldBlockWithStatements
#[test]
fn test_fold_block_with_statements() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c", "a,b,c");
    t.fuse("a();b();c();", "a(),b(),c()");
    t.fuse("a(),b();c(),d()", "a(),b(),c(),d()");
    t.fuse("a();b(),c(),d()", "a(),b(),c(),d()");
    t.fuse("a(),b(),c();d()", "a(),b(),c(),d()");
}

// port: StatementFusionTest#testFoldBlockIntoIf
#[test]
fn test_fold_block_into_if() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c;if(x){}", "if(a,b,c,x){}");
    t.fuse("a;b;c;if(x,y){}else{}", "if(a,b,c,x,y){}else{}");
    t.fuse("a;b;c;if(x,y){}", "if(a,b,c,x,y){}");
    t.fuse("a;b;c;if(x,y,z){}", "if(a,b,c,x,y,z){}");
    // Can't fuse if there are statements after the IF.
    t.fuse_same("a();if(a()){}a()");
}

// port: StatementFusionTest#testFoldBlockReturn
#[test]
fn test_fold_block_return() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c;return x", "return a,b,c,x");
    t.fuse("a;b;c;return x+y", "return a,b,c,x+y");
    // DeadAssignmentElimination would have cleaned it up anyways.
    t.fuse_same("a;b;c;return x;a;b;c");
}

// port: StatementFusionTest#testFoldBlockThrow
#[test]
fn test_fold_block_throw() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c;throw x", "throw a,b,c,x");
    t.fuse("a;b;c;throw x+y", "throw a,b,c,x+y");
    t.fuse_same("a;b;c;throw x;a;b;c");
}

// port: StatementFusionTest#testFoldSwitch
#[test]
fn test_fold_switch() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c;switch(x){}", "switch(a,b,c,x){}");
}

// port: StatementFusionTest#testFuseIntoForIn1
#[test]
fn test_fuse_into_for_in1() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c;for(x in y){}", "for(x in a,b,c,y){}");
}

// port: StatementFusionTest#testFuseIntoForIn2
#[test]
fn test_fuse_into_for_in2() {
    let mut t = StatementFusionTest::new();
    // This test case causes a parse warning in ES5 strict out, but is a parse error in ES6+ out.
    t.harness
        .set_accepted_language(language_mode("ECMASCRIPT5_STRICT"))
        .unwrap();
    t.harness.set_expect_parse_warnings_in_this_test().unwrap();
    t.fuse_same("a();for(var x = b() in y){}");
}

// port: StatementFusionTest#testFuseIntoVanillaFor1
#[test]
fn test_fuse_into_vanilla_for1() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c;for(;g;){}", "for(a,b,c;g;){}");
    t.fuse("a;b;c;for(d;g;){}", "for(a,b,c,d;g;){}");
    t.fuse("a;b;c;for(d,e;g;){}", "for(a,b,c,d,e;g;){}");
    t.fuse_same("a();for(var x;g;){}");
}

// port: StatementFusionTest#testFuseIntoVanillaFor2
#[test]
fn test_fuse_into_vanilla_for2() {
    let mut t = StatementFusionTest::new();
    t.fuse_same("a;b;c;for(var d;g;){}");
    t.fuse_same("a;b;c;for(let d;g;){}");
    t.fuse_same("a;b;c;for(const d = 5;g;){}");
}

// port: StatementFusionTest#testFuseIntoLabel
#[test]
fn test_fuse_into_label() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c;label:for(x in y){}", "label:for(x in a,b,c,y){}");
    t.fuse("a;b;c;label:for(;g;){}", "label:for(a,b,c;g;){}");
    t.fuse("a;b;c;l1:l2:l3:for(;g;){}", "l1:l2:l3:for(a,b,c;g;){}");
    t.fuse_same("a;b;c;label:while(true){}");
}

// port: StatementFusionTest#testFuseIntoBlock
#[test]
fn test_fuse_into_block() {
    let mut t = StatementFusionTest::new();
    t.fuse("a;b;c;{d;e;f}", "{a,b,c,d,e,f}");
    t.fuse(
        "a;b; label: { if(q) break label; bar(); }",
        "label: { if(a,b,q) break label; bar(); }",
    );
    t.fuse_same("a;b;c;{var x;d;e;}");
    t.fuse_same("a;b;c;label:{break label;d;e;}");
}

// port: StatementFusionTest#testNoFuseIntoWhile
#[test]
fn test_no_fuse_into_while() {
    let mut t = StatementFusionTest::new();
    t.fuse_same("a;b;c;while(x){}");
}

// port: StatementFusionTest#testNoFuseIntoDo
#[test]
fn test_no_fuse_into_do() {
    let mut t = StatementFusionTest::new();
    t.fuse_same("a;b;c;do{}while(x)");
}

// port: StatementFusionTest#testNoFuseIntoBlock
#[test]
fn test_no_fuse_into_block() {
    let mut t = StatementFusionTest::new();
    // Never fuse a statement into a block that contains let/const/class declarations, or you risk
    // colliding variable names. (unless the AST is normalized).
    t.fuse("a; {b;}", "{a,b;}");
    t.fuse("a; {b; var a = 1;}", "{a,b; var a = 1;}");
    t.fuse_same("a; { b; let a = 1; }");
    t.fuse_same("a; { b; const a = 1; }");
    t.fuse_same("a; { b; class a {} }");
    t.fuse_same("a; { b; function a() {} }");
    t.fuse_same("a; { b; const otherVariable = 1; }");
    t.harness.enable_normalize().unwrap();
    t.test(
        "function f(a) { if (COND) { a; { b; let a = 1; } } }",
        "function f(a) { if (COND) { { a,b; let a$jscomp$1 = 1; } } }",
    );
    t.test(
        "function f(a) { if (COND) { a; { b; let otherVariable = 1; } } }",
        "function f(a) { if (COND) {  { a,b; let otherVariable = 1; } } }",
    );
}

// port: StatementFusionTest#testNoGlobalScopeChanges
#[test]
fn test_no_global_scope_changes() {
    let mut t = StatementFusionTest::new();
    t.test_same("a,b,c");
}

// port: StatementFusionTest#testNoFunctionBlockChanges
#[test]
fn test_no_function_block_changes() {
    let mut t = StatementFusionTest::new();
    t.test_same("function foo() { a,b,c }");
}
