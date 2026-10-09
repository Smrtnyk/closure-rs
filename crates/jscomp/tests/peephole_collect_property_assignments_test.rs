/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/PeepholeCollectPropertyAssignmentsTest.java.

//! Port of `PeepholeCollectPropertyAssignmentsTest.java`.
use closure_jscomp::{
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    peephole_collect_property_assignments::PeepholeCollectPropertyAssignments,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
};
use closure_rhino::fast_hash::IndexMap;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct PeepholeCollectPropertyAssignmentsTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: PeepholeCollectPropertyAssignmentsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> =
            vec![Box::new(PeepholeCollectPropertyAssignments::new())];
        let pass: Box<dyn CompilerPass> = Box::new(PeepholeOptimizationsPass::new(
            self.get_name(),
            optimizations,
        ));
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "PeepholeCollectPropertyAssignmentsTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl PeepholeCollectPropertyAssignmentsTest {
    // port: PeepholeCollectPropertyAssignmentsTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "PeepholeCollectPropertyAssignmentsTest".into(),
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
}

// port: PeepholeCollectPropertyAssignmentsTest#test36122565a
#[test]
fn test36122565a() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var foo = { bar: g(), baz: 4 };\nfoo.bar = 3;\nfoo.baz = 3;\nconsole.log(foo.bar);\nconsole.log(foo.baz);\n");
    t.test("var foo = { bar: g(), baz: 4 };\nfoo.baz = 3;\nfoo.bar = 3;\nconsole.log(foo.bar);\nconsole.log(foo.baz);\n", "var foo = { bar: g(), baz: 3 };\nfoo.bar = 3;\nconsole.log(foo.bar);\nconsole.log(foo.baz);\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimization1
#[test]
fn test_array_optimization1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = []; a[0] = 1; a[1] = 2; a[2] = 3;",
        "var a = [1, 2, 3];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArray_nonConsecutiveAssignments
#[test]
fn test_array_non_consecutive_assignments() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = []; a[2] = 1; a[0] = 2; a[1] = 3;",
        "var a = [2, 3, 1];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimization2
#[test]
fn test_array_optimization2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a; a = []; a[0] = 1; a[1] = 2; a[2] = 3;",
        "var a; a = [1, 2, 3];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimization3
#[test]
fn test_array_optimization3() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a; a.b = []; a.b[0] = 1; a.b[1] = 2; a.b[2] = 3;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimizationWithLet
#[test]
fn test_array_optimization_with_let() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "let a = []; a[0] = 1; a[1] = 2; a[2] = 3;",
        "let a = [1, 2, 3];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimizationWithConst
#[test]
fn test_array_optimization_with_const() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "const a = []; a[0] = 1; a[1] = 2; a[2] = 3;",
        "const a = [1, 2, 3];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testCompoundAssignment
#[test]
fn test_compound_assignment() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var x, a; a = []; a[0] *= x;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testNegativeArrayIndex1
#[test]
fn test_negative_array_index1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = []; a[-1] = 1;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testNegativeArrayIndex2
#[test]
fn test_negative_array_index2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a; a = []; a[-1] = 1;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testFractionalArrayIndex1
#[test]
fn test_fractional_array_index1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = []; a[0.5] = 1;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testFractionalArrayIndex2
#[test]
fn test_fractional_array_index2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a; a = []; a[0.5] = 1;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimizationOfPartiallyBuiltArray1
#[test]
fn test_array_optimization_of_partially_built_array1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test("var a = [1, 2]; a[2] = 3;", "var a = [1, 2, 3];");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimizationOfPartiallyBuiltArray2
#[test]
fn test_array_optimization_of_partially_built_array2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test("var a; a = [1, 2]; a[2] = 3;", "var a; a = [1, 2, 3];");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimizationWithAHole1
#[test]
fn test_array_optimization_with_a_hole1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = []; a[0] = 1; a[1] = 2; a[3] = 4;",
        "var a = [1, 2, , 4];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOptimizationWithAHole2
#[test]
fn test_array_optimization_with_a_hole2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a; a = []; a[0] = 1; a[1] = 2; a[3] = 4;",
        "var a; a = [1, 2, , 4];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testEarlyUsage1
#[test]
fn test_early_usage1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("function c() {return sum(a)};\nvar a = [1,2,3];\na[4] = c();\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testEarlyUsage2
#[test]
fn test_early_usage2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("function c() {return sum(a)};\nvar a; a = [1,2,3];\na[4] = c();\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayTooSparseOptimization1
#[test]
fn test_array_too_sparse_optimization1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = []; a[0] = 1; a[1] = 2; a[100] = 4;",
        "var a = [1, 2]; a[100] = 4;",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayTooSparseOptimization2
#[test]
fn test_array_too_sparse_optimization2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a; a = []; a[0] = 1; a[1] = 2; a[100] = 4;",
        "var a; a = [1, 2]; a[100] = 4;",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayOutOfOrder
#[test]
fn test_array_out_of_order() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test("var a = []; a[1] = 1; a[0] = 0;", "var a = [0, 1];");
    t.test("var a; a = []; a[1] = 1; a[0] = 0;", "var a; a = [0, 1];");
    // We cannot change the order of side-effects.
    // The below should not be
    //   var x = 0; var a = [x++, x++]
    // since that would produce
    //   var a = [0, 1], x = 2;
    // instead of
    //   var a = [1, 0], x = 2;
    t.test_same("var x = 0; var a = []; a[1] = x++; a[0] = x++;");
    t.test_same("var x; x = 0; var a = []; a[1] = x++; a[0] = x++;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testMultipleNames1
#[test]
fn test_multiple_names1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var b = []; b[0] = 2; var a = []; a[0] = 1;",
        "var b = [2]; var a = [1];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testMultipleNames2
#[test]
fn test_multiple_names2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var b; b = []; b[0] = 2; var a = []; a[0] = 1;",
        "var b; b = [2]; var a = [1];",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayReassignedInValue1
#[test]
fn test_array_reassigned_in_value1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = []; a[0] = 1; a[1] = (a = []); a[3] = 4;",
        "var a = [1]; a[1] = (a = []); a[3] = 4;",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayReassignedInValue2
#[test]
fn test_array_reassigned_in_value2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a; a = []; a[0] = 1; a[1] = (a = []); a[3] = 4;",
        "var a; a = [1]; a[1] = (a = []); a[3] = 4;",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayReassignedInSubsequentVar1
#[test]
fn test_array_reassigned_in_subsequent_var1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = []; a[0] = a = []; a[1] = 2;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArrayReassignedInSubsequentVar2
#[test]
fn test_array_reassigned_in_subsequent_var2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a; a = []; a[0] = a = []; a[1] = 2;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testForwardReference1
#[test]
fn test_forward_reference1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a; a = []; a[0] = 1; a[1] = a;",
        "var a; a = [1]; a[1] = a;",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testForwardReference2
#[test]
fn test_forward_reference2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a; a = []; a[0] = 1; a[1] = a;",
        "var a; a = [1]; a[1] = a;",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testArray_usingSpread_diablesOptimization_leadingSpread
#[test]
fn test_array_using_spread_diables_optimization_leading_spread() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    // TODO(nickreid): A future improvement could generate `var a = [3, ...b, 2];`
    t.test_same("var a = [1, ...b, 2]; a[0] = 3;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArray_usingSpread_diablesOptimization_withinSpread
#[test]
fn test_array_using_spread_diables_optimization_within_spread() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = [1, ...b, 2]; a[1] = 3;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArray_usingSpread_diablesOptimization_followingSpread
#[test]
fn test_array_using_spread_diables_optimization_following_spread() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = [1, ...b, 2]; a[2] = 3;");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectOptimization1
#[test]
fn test_object_optimization1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var o = {}; o.x = 0; o['y'] = 1; o[2] = 2;",
        "var o = { x: 0, \"y\": 1, \"2\": 2 };",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectOptimization2
#[test]
fn test_object_optimization2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var o; o = {}; o.x = 0; o['y'] = 1; o[2] = 2;",
        "var o; o = { x: 0, \"y\": 1, \"2\": 2 };",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectOptimizationWithLet
#[test]
fn test_object_optimization_with_let() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "let o = {}; o.x = 0; o['y'] = 1; o[2] = 2;",
        "let o = { x: 0, 'y': 1, '2': 2 };",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectOptimizationWithConst
#[test]
fn test_object_optimization_with_const() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "const o = {}; o.x = 0; o['y'] = 1; o[2] = 2;",
        "const o = { x: 0, 'y': 1, '2': 2 };",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectReassignedInValue1
#[test]
fn test_object_reassigned_in_value1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var o = {}; o.x = 1; o.y = (o = {}); o.z = 4;",
        "var o = {x:1}; o.y = (o = {}); o.z = 4;",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectReassignedInValue2
#[test]
fn test_object_reassigned_in_value2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var o; o = {}; o.x = 1; o.y = (o = {}); o.z = 4;",
        "var o; o = {x:1}; o.y = (o = {}); o.z = 4;",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectFunctionRollup1
#[test]
fn test_object_function_rollup1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var o; o = {};\no.x = function() {};\n",
        "var o; o = {x:function () {}};",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectFunctionRollup2
#[test]
fn test_object_function_rollup2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var o; o = {};\no.x = (function() {return o})();\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectFunctionRollup3
#[test]
fn test_object_function_rollup3() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var o; o = {};\no.x = function() {return o};\n",
        "var o; o = {x:function () {return o}};",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectFunctionRollup4
#[test]
fn test_object_function_rollup4() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("function f() {return o};\nvar o; o = {};\no.x = f();\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectFunctionRollup5
#[test]
fn test_object_function_rollup5() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test("var o; o = {};\no.x = function() {return o};\no.y = [function() {return o}];\no.z = {a:function() {return o}};\n", "var o; o = {\nx:function () {return o},\ny:[function () {return o}],\nz:{a:function () {return o}}};\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectPropertyReassigned
#[test]
fn test_object_property_reassigned() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test("var a = {b:''};\na.b='c';\n", "var a={b:'c'};");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectPropertyReassigned2
#[test]
fn test_object_property_reassigned2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test("var a = {b:'', x:10};\na.b='c';\n", "var a={x:10, b:'c'};");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectPropertyReassigned3
#[test]
fn test_object_property_reassigned3() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test("var a = {x:10};\na.b = 'c';\n", "var a = {x:10, b:'c'};");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectPropertyReassigned4
#[test]
fn test_object_property_reassigned4() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = {b:10};\nvar x = 1;\na.b = x+10;\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObject_usingSpread_diablesOptimization_leadingSpread
#[test]
fn test_object_using_spread_diables_optimization_leading_spread() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = {a: 1, ...b, c: 2}; a.a = 3;",
        "var a = {...b, c: 2, a: 3};",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObject_usingSpread_diablesOptimization_withinSpread
#[test]
fn test_object_using_spread_diables_optimization_within_spread() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = {a: 1, ...b, c: 2, b: 3};");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObject_usingSpread_diablesOptimization_followingSpread
#[test]
fn test_object_using_spread_diables_optimization_following_spread() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = {a: 1, ...b, c: 3};");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectComputedProp1
#[test]
fn test_object_computed_prop1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same(
        "var a = {['computed']: 10};\nvar alsoComputed = 'someValue';\na[alsoComputed] = 20;\n",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectComputedProp2
#[test]
fn test_object_computed_prop2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = {['computed']: 10};\na.prop = 20;\n",
        "var a = {\n  ['computed']: 10,\n  prop: 20,\n};\n",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectMemberFunction1
#[test]
fn test_object_member_function1() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = { member() {} };\na.prop = 20;\n",
        "var a = {\n  member() {},\n  prop: 20,\n};\n",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectMemberFunction2
#[test]
fn test_object_member_function2() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test(
        "var a = { member() {} };\na.member = 20;\n",
        "var a = {\n  member: 20,\n};\n",
    );
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectGetter
#[test]
fn test_object_getter() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = { get x() {} };\na.x = 20;\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testObjectSetter
#[test]
fn test_object_setter() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = { set x(value) {} };\na.x = 20;\n");
}

// port: PeepholeCollectPropertyAssignmentsTest#testArraySpread
#[test]
fn test_array_spread() {
    let mut t = PeepholeCollectPropertyAssignmentsTest::new();
    t.test_same("var a = [1, ...spread]; a[2] = 3;");
    t.test_same("var a = [...spread]; a[0] = 1;");
    t.test_same("var a = [1, ...spread]; a[0] = 2;");
}
