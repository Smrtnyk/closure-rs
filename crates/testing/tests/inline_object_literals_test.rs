/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/InlineObjectLiteralsTest.java.

//! Port of InlineObjectLiteralsTest (CompilerTestCase over the crates/testing port).
mod optimize_calls_support;

use closure_jscomp::{
    compilation_level::CompilationLevel, inline_object_literals::InlineObjectLiterals,
};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue},
    throwable::Throwable,
};
#[allow(unused_imports)]
use optimize_calls_support::*;

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: InlineObjectLiteralsTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(InlineObjectLiterals::new(
            compiler.borrow().get_unique_name_id_supplier(),
        )))
    }
    // port: InlineObjectLiteralsTest#getNumRepetitions
    fn get_num_repetitions(&self) -> i32 {
        // TODO(b/33104006): remove this override.
        2
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Test = Fixture<Hooks>;

impl Test {
    // port: InlineObjectLiteralsTest#InlineObjectLiteralsTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("InlineObjectLiteralsTest"),
            },
        }
    }
}

// port: InlineObjectLiteralsTest#LOCAL_PREFIX
const LOCAL_PREFIX: &str = "function local(){";
// port: InlineObjectLiteralsTest#LOCAL_POSTFIX
const LOCAL_POSTFIX: &str = "}";

impl Test {
    // port: InlineObjectLiteralsTest#testLocal
    fn test_local(&mut self, code: &str, result: &str) {
        self.test(
            &[LOCAL_PREFIX, code, LOCAL_POSTFIX].concat(),
            &[LOCAL_PREFIX, result, LOCAL_POSTFIX].concat(),
        );
    }

    // port: InlineObjectLiteralsTest#testSameLocal
    fn test_same_local(&mut self, code: &str) {
        self.test_same(&[LOCAL_PREFIX, code, LOCAL_POSTFIX].concat());
    }
}

// port: InlineObjectLiteralsTest#testObject0
#[test]
fn test_object0() {
    let mut t = Test::new();
    // Don't mess with global variables, that is the job of CollapseProperties.
    t.test_same("var a = {x:1}; f(a.x);");
}

// port: InlineObjectLiteralsTest#testObject1
#[test]
fn test_object1() {
    let mut t = Test::new();
    t.test_local(
        "var a = {x:x(), y:y()}; f(a.x, a.y);",
        concat!(
            "var JSCompiler_object_inline_x_0=x();\n",
            "var JSCompiler_object_inline_y_1=y();\n",
            "f(JSCompiler_object_inline_x_0, JSCompiler_object_inline_y_1);\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject1a
#[test]
fn test_object1a() {
    let mut t = Test::new();
    t.test_local(
        "var a; a = {x:x, y:y}; f(a.x, a.y);",
        concat!(
            "var JSCompiler_object_inline_x_0;\n",
            "var JSCompiler_object_inline_y_1;\n",
            "(JSCompiler_object_inline_x_0=x,\n",
            "JSCompiler_object_inline_y_1=y, true);\n",
            "f(JSCompiler_object_inline_x_0, JSCompiler_object_inline_y_1);\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject2
#[test]
fn test_object2() {
    let mut t = Test::new();
    t.test_local(
        "var a = {y:y}; a.x = z; f(a.x, a.y);",
        concat!(
            "var JSCompiler_object_inline_y_0 = y;\n",
            "var JSCompiler_object_inline_x_1;\n",
            "JSCompiler_object_inline_x_1=z;\n",
            "f(JSCompiler_object_inline_x_1, JSCompiler_object_inline_y_0);\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject3
#[test]
fn test_object3() {
    let mut t = Test::new();
    // Inlining the 'y' would cause the 'this' to be different in the
    // target function.
    t.test_same_local("var a = {y:y,x:x}; a.y(); f(a.x);");
    t.test_same_local("var a; a = {y:y,x:x}; a.y(); f(a.x);");
    t.test_same_local("var a = {y:y,x:x}; a.y?.(); f(a.x);");
    t.test_same_local("var a = {y:y,x:x}; a?.y(); f(a.x);");
    t.test_same_local("var a = {y:y,x:x}; a?.y?.(); f(a.x);");
    t.test_same_local("var a = {y:y,x:x}; a.y`hi`; f(a.x);");
}

// port: InlineObjectLiteralsTest#testObject4
#[test]
fn test_object4() {
    let mut t = Test::new();
    // Object literal is escaped.
    t.test_same_local("var a = {y:y}; a.x = z; f(a.x, a.y); g(a);");
    t.test_same_local("var a; a = {y:y}; a.x = z; f(a.x, a.y); g(a);");
}

// port: InlineObjectLiteralsTest#testObject5
#[test]
fn test_object5() {
    let mut t = Test::new();
    t.test_local(
        "var a = {x:x, y:y}; var b = {a:a}; f(b.a.x, b.a.y);",
        concat!(
            "var a = {x:x, y:y};\n",
            "var JSCompiler_object_inline_a_0=a;\n",
            "f(JSCompiler_object_inline_a_0.x, JSCompiler_object_inline_a_0.y);\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testNoCrashInliningObjectLiteral_conditionalAssignmentToObject_es2017
#[test]
fn test_no_crash_inlining_object_literal_conditional_assignment_to_object_es2017() {
    let mut t = Test::new();
    CompilationLevel::ADVANCED_OPTIMIZATIONS
        .set_options_for_compilation_level(&mut t.harness.get_options().unwrap());
    let src = "{let a;if(Math.random()){a={x:''};console.log(a.x)}};";
    let expected = concat!(
        "{\n",
        "  var JSCompiler_object_inline_x_0;\n",
        "  if (Math.random()) {\n",
        "    JSCompiler_object_inline_x_0 = '', true;\n",
        "    console.log(JSCompiler_object_inline_x_0);\n",
        "  }\n",
        "};\n"
    );
    t.test(src, expected);
}

// port: InlineObjectLiteralsTest#testNoInlining_propertyAccessOnVar_unconditionalAssignmentToObject_es2017
#[test]
fn test_no_inlining_property_access_on_var_unconditional_assignment_to_object_es2017() {
    let mut t = Test::new();
    CompilationLevel::ADVANCED_OPTIMIZATIONS
        .set_options_for_compilation_level(&mut t.harness.get_options().unwrap());
    // `var` declaration corresponds to global scope: ineligible for inlining object literal
    let src = "{var a; a={x:\"\"};console.log(a.x);}";
    t.test_same(src);
}

// port: InlineObjectLiteralsTest#testNoCrashInliningObjectLiteral_unconditionalAssignmentToObject_es2017
#[test]
fn test_no_crash_inlining_object_literal_unconditional_assignment_to_object_es2017() {
    let mut t = Test::new();
    CompilationLevel::ADVANCED_OPTIMIZATIONS
        .set_options_for_compilation_level(&mut t.harness.get_options().unwrap());
    let src = "{let a; a={x:\"\"};console.log(a.x);}";
    let expected = concat!(
        "{\n",
        "  var JSCompiler_object_inline_x_0;\n",
        "  JSCompiler_object_inline_x_0 = \"\", true;\n",
        "  console.log(JSCompiler_object_inline_x_0);\n",
        "}\n"
    );
    t.test(src, expected);
}

// port: InlineObjectLiteralsTest#testNoInliningObjectLiteral_conditionalAssignmentToReturnValue_es2017
#[test]
fn test_no_inlining_object_literal_conditional_assignment_to_return_value_es2017() {
    let mut t = Test::new();
    CompilationLevel::ADVANCED_OPTIMIZATIONS
        .set_options_for_compilation_level(&mut t.harness.get_options().unwrap());
    let src = concat!(
        "const b = () => ({ x: '' })\n",
        "function main() {\n",
        "  let a;\n",
        "  if (Math.random()) {\n",
        "// regardless of scope, only direct assignments to object literals get inlined.\n",
        "    a = b();\n",
        "    alert(a.x);\n",
        "  }\n",
        "}\n",
        "main();\n"
    );
    t.test_same_local(src);
}

// port: InlineObjectLiteralsTest#testNoInliningObjectLiteral_unconditionalAssignmentToReturnValue_es2017
#[test]
fn test_no_inlining_object_literal_unconditional_assignment_to_return_value_es2017() {
    let mut t = Test::new();
    CompilationLevel::ADVANCED_OPTIMIZATIONS
        .set_options_for_compilation_level(&mut t.harness.get_options().unwrap());
    let src = concat!(
        "const b = () => ({ x: '' })\n",
        "function main() {\n",
        "  let a;\n",
        "// regardless of scope, only direct assignments to object literals get inlined.\n",
        "  a = b();\n",
        "  alert(a.x);\n",
        "}\n",
        "main();\n"
    );
    t.test_same_local(src);
}

// port: InlineObjectLiteralsTest#testObject6
#[test]
fn test_object6() {
    let mut t = Test::new();
    t.test_local(
        "for (var i = 0; i < 5; i++) { var a = {i:i,x:x}; f(a.i, a.x); }",
        concat!(
            "for (var i = 0; i < 5; i++) {\n",
            "  var JSCompiler_object_inline_i_0=i;\n",
            "  var JSCompiler_object_inline_x_1=x;\n",
            "  f(JSCompiler_object_inline_i_0,JSCompiler_object_inline_x_1)\n",
            "}\n"
        ),
    );
    t.test_local(
        "if (c) { var a = {i:i,x:x}; f(a.i, a.x); }",
        concat!(
            "if (c) {\n",
            "  var JSCompiler_object_inline_i_0=i;\n",
            "  var JSCompiler_object_inline_x_1=x;\n",
            "  f(JSCompiler_object_inline_i_0,JSCompiler_object_inline_x_1)\n",
            "}\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject6_let
#[test]
fn test_object6_let() {
    let mut t = Test::new();
    t.test_local(
        "for (let i = 0; i < 5; i++) { let a = {i:i,x:x}; f(a.i, a.x); }",
        concat!(
            "for (let i = 0; i < 5; i++) {\n",
            "  var JSCompiler_object_inline_i_0=i;\n",
            "  var JSCompiler_object_inline_x_1=x;\n",
            "  f(JSCompiler_object_inline_i_0,JSCompiler_object_inline_x_1)\n",
            "}\n"
        ),
    );
    t.test_local(
        "if (c) { var a = {i:i,x:x}; f(a.i, a.x); }",
        concat!(
            "if (c) {\n",
            "  var JSCompiler_object_inline_i_0=i;\n",
            "  var JSCompiler_object_inline_x_1=x;\n",
            "  f(JSCompiler_object_inline_i_0,JSCompiler_object_inline_x_1)\n",
            "}\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObjectSwitch
#[test]
fn test_object_switch() {
    let mut t = Test::new();
    t.test_local(
        concat!(
            "let x;\n",
            "switch (x) {\n",
            "  case 'h':\n",
            "    let a;\n",
            "    if (Math.random() > 0.5) {\n",
            "      a = {i:i,x:x};\n",
            "      f(a.i, a.x);\n",
            "      break;\n",
            "    }\n",
            "}\n"
        ),
        concat!(
            "let x;\n",
            "var JSCompiler_object_inline_i_0;\n",
            "var JSCompiler_object_inline_x_1;\n",
            " switch (x) {\n",
            "   case 'h':\n",
            "    if (Math.random() > .5) {\n",
            "      JSCompiler_object_inline_i_0 = i, JSCompiler_object_inline_x_1 = x, true;\n",
            "      f(JSCompiler_object_inline_i_0,JSCompiler_object_inline_x_1);\n",
            "      break;\n",
            "    }\n",
            "}\n"
        ),
    );
    t.test_local(
        concat!(
            "let x;\n",
            " switch (x) {\n",
            "   case 'h':\n",
            "   case 'i':\n",
            "     let a;\n",
            "     if (true) {\n",
            "       a = {i:i,x:x};\n",
            "       f(a.i, a.x);\n",
            "     }\n",
            "     break;\n",
            "}\n"
        ),
        concat!(
            "let x;\n",
            "var JSCompiler_object_inline_i_0;\n",
            "var JSCompiler_object_inline_x_1;\n",
            " switch (x) {\n",
            " case 'h':\n",
            " case 'i':\n",
            "  if (true) {\n",
            "    JSCompiler_object_inline_i_0 = i, JSCompiler_object_inline_x_1 = x, true;\n",
            "    f(JSCompiler_object_inline_i_0,JSCompiler_object_inline_x_1);\n",
            "  }\n",
            "break;\n",
            "}\n"
        ),
    );
    t.test_local(
        concat!(
            "let x;\n",
            "switch (x) {\n",
            "  case 'h':\n",
            "    let a;\n",
            "    if (Math.random() > 0.5) {\n",
            "      a = {i:i,x:x};\n",
            "      f(a.i, a.x);\n",
            "      break;\n",
            "    }\n",
            "}\n"
        ),
        concat!(
            "let x;\n",
            "var JSCompiler_object_inline_i_0;\n",
            "var JSCompiler_object_inline_x_1;\n",
            " switch (x) {\n",
            " case 'h':\n",
            "  if (Math.random() > .5) {\n",
            "    JSCompiler_object_inline_i_0 = i, JSCompiler_object_inline_x_1 = x, true;\n",
            "    f(JSCompiler_object_inline_i_0,JSCompiler_object_inline_x_1);\n",
            "    break;\n",
            "  }\n",
            "}\n"
        ),
    );
    t.test_local(
        concat!(
            "let x;\n",
            "let z;\n",
            "switch (x) {\n",
            "  case 'h':\n",
            "    if (Math.random() > 0.5) {\n",
            "      z = {i:i,x:x};\n",
            "      f(z.i, z.x);\n",
            "    }\n",
            "  case 'i':\n",
            "    ({x:x} = f(z.i, z.x));\n",
            "    break;\n",
            "}\n"
        ),
        concat!(
            "var JSCompiler_object_inline_i_0;\n",
            "var JSCompiler_object_inline_x_1;\n",
            "let x;\n",
            " switch (x) {\n",
            " case 'h':\n",
            "  if (Math.random() > .5) {\n",
            "    JSCompiler_object_inline_i_0 = i, JSCompiler_object_inline_x_1 = x, true;\n",
            "    f(JSCompiler_object_inline_i_0,JSCompiler_object_inline_x_1);\n",
            "  }\n",
            "  case 'i':\n",
            "    ({x:x} = f(JSCompiler_object_inline_i_0, JSCompiler_object_inline_x_1));\n",
            "    break;\n",
            "}\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject7
#[test]
fn test_object7() {
    let mut t = Test::new();
    t.test_local(
        "var a = {x:x, y:f()}; g(a.x);",
        concat!(
            "var JSCompiler_object_inline_x_0=x;\n",
            "var JSCompiler_object_inline_y_1=f();\n",
            "g(JSCompiler_object_inline_x_0)\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject8
#[test]
fn test_object8() {
    let mut t = Test::new();
    t.test_same_local("var a = {x:x,y:y}; var b = {x:y}; f((c?a:b).x);");
    t.test_local(
        "var a; if(c) { a={x:x, y:y}; } else { a={x:y}; } f(a.x);",
        concat!(
            "var JSCompiler_object_inline_x_0;\n",
            "var JSCompiler_object_inline_y_1;\n",
            "if(c) JSCompiler_object_inline_x_0=x,\n",
            "      JSCompiler_object_inline_y_1=y,\n",
            "      true;\n",
            "else JSCompiler_object_inline_x_0=y,\n",
            "     JSCompiler_object_inline_y_1=void 0,\n",
            "     true;\n",
            "f(JSCompiler_object_inline_x_0)\n"
        ),
    );
    t.test_local(
        "var a = {x:x,y:y}; var b = {x:y}; c ? f(a.x) : f(b.x);",
        concat!(
            "var JSCompiler_object_inline_x_0 = x;\n",
            "var JSCompiler_object_inline_y_1 = y;\n",
            "var JSCompiler_object_inline_x_2 = y;\n",
            "c ? f(JSCompiler_object_inline_x_0):f(JSCompiler_object_inline_x_2)\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject9
#[test]
fn test_object9() {
    let mut t = Test::new();
    // There is a call, so no inlining
    t.test_same_local(concat!(
        "function f(a,b) {\n",
        "  var x = {a:a,b:b}; x.a(); return x.b;\n",
        "}\n"
    ));
    t.test_local(
        concat!(
            "function f(a,b) {\n",
            "  var x = {a:a,b:b}; g(x.a); x = {a:a,b:2}; return x.b;\n",
            "}\n"
        ),
        concat!(
            "function f(a,b) {\n",
            "  var JSCompiler_object_inline_a_0 = a;\n",
            "  var JSCompiler_object_inline_b_1 = b;\n",
            "  g(JSCompiler_object_inline_a_0);\n",
            "  JSCompiler_object_inline_a_0 = a,\n",
            "  JSCompiler_object_inline_b_1=2,\n",
            "  true;\n",
            "  return JSCompiler_object_inline_b_1\n",
            "}\n"
        ),
    );
    t.test_local(
        concat!(
            "function f(a,b) {\n",
            "  var x = {a:a,b:b}; g(x.a); x.b = x.c = 2; return x.b;\n",
            "}\n"
        ),
        concat!(
            "function f(a,b) {\n",
            "  var JSCompiler_object_inline_a_0=a;\n",
            "  var JSCompiler_object_inline_b_1=b;\n",
            "  var JSCompiler_object_inline_c_2;\n",
            "  g(JSCompiler_object_inline_a_0);\n",
            "  JSCompiler_object_inline_b_1=JSCompiler_object_inline_c_2=2;\n",
            "  return JSCompiler_object_inline_b_1\n",
            "}\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject10
#[test]
fn test_object10() {
    let mut t = Test::new();
    t.test_local(
        "var x; var b = f(); x = {a:a, b:b}; if(x.a) g(x.b);",
        concat!(
            "var JSCompiler_object_inline_a_0;\n",
            "var JSCompiler_object_inline_b_1;\n",
            "var b = f();\n",
            "JSCompiler_object_inline_a_0=a,JSCompiler_object_inline_b_1=b,true;\n",
            "if(JSCompiler_object_inline_a_0) g(JSCompiler_object_inline_b_1)\n"
        ),
    );
    t.test_same_local("var x = {}; var b = f(); x = {a:a, b:b}; if(x.a) g(x.b) + x.c");
    t.test_local(
        "var x; var b = f(); x = {a:a, b:b}; x.c = c; if(x.a) g(x.b) + x.c",
        concat!(
            "var JSCompiler_object_inline_a_0;\n",
            "var JSCompiler_object_inline_b_1;\n",
            "var JSCompiler_object_inline_c_2;\n",
            "var b = f();\n",
            "JSCompiler_object_inline_a_0 = a,JSCompiler_object_inline_b_1 = b,\n",
            "  JSCompiler_object_inline_c_2=void 0,true;\n",
            "JSCompiler_object_inline_c_2 = c;\n",
            "if (JSCompiler_object_inline_a_0)\n",
            "  g(JSCompiler_object_inline_b_1) + JSCompiler_object_inline_c_2;\n"
        ),
    );
    t.test_local(
        "var x = {a:a}; if (b) x={b:b}; f(x.a||x.b);",
        concat!(
            "var JSCompiler_object_inline_a_0 = a;\n",
            "var JSCompiler_object_inline_b_1;\n",
            "if(b) JSCompiler_object_inline_b_1 = b,\n",
            "      JSCompiler_object_inline_a_0 = void 0,\n",
            "      true;\n",
            "f(JSCompiler_object_inline_a_0 || JSCompiler_object_inline_b_1)\n"
        ),
    );
    t.test_local(
        "var x; var y = 5; x = {a:a, b:b, c:c}; if (b) x={b:b}; f(x.a||x.b);",
        concat!(
            "var JSCompiler_object_inline_a_0;\n",
            "var JSCompiler_object_inline_b_1;\n",
            "var JSCompiler_object_inline_c_2;\n",
            "var y=5;\n",
            "JSCompiler_object_inline_a_0=a,\n",
            "JSCompiler_object_inline_b_1=b,\n",
            "JSCompiler_object_inline_c_2=c,\n",
            "true;\n",
            "if (b) JSCompiler_object_inline_b_1=b,\n",
            "       JSCompiler_object_inline_a_0=void 0,\n",
            "       JSCompiler_object_inline_c_2=void 0,\n",
            "       true;\n",
            "f(JSCompiler_object_inline_a_0||JSCompiler_object_inline_b_1)\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject11
#[test]
fn test_object11() {
    let mut t = Test::new();
    t.test_same_local("var x = {a:b}; (x = {a:a}).c = 5; f(x.a);");
    t.test_same_local("var x = {a:a}; f(x[a]); g(x[a]);");
}

// port: InlineObjectLiteralsTest#testObject12
#[test]
fn test_object12() {
    let mut t = Test::new();
    t.test_same_local("var a; a = {x:1, y:2}; f(a.x, a.y2);");
}

// port: InlineObjectLiteralsTest#testObject13
#[test]
fn test_object13() {
    let mut t = Test::new();
    t.test_same_local("var x = {a:1, b:2}; x = {a:3, b:x.a};");
}

// port: InlineObjectLiteralsTest#testObject14
#[test]
fn test_object14() {
    let mut t = Test::new();
    t.test_same_local("var x = {a:1}; if ('a' in x) { f(); }");
    t.test_same_local("var x = {a:1}; for (var y in x) { f(y); }");
}

// port: InlineObjectLiteralsTest#testObject15
#[test]
fn test_object15() {
    let mut t = Test::new();
    t.test_same_local("x = x || {}; f(x.a);");
}

// port: InlineObjectLiteralsTest#testObject16
#[test]
fn test_object16() {
    let mut t = Test::new();
    t.test_local(
        "function f(e) { bar(); x = {a: foo()}; var x; print(x.a); }",
        concat!(
            "function f(e) {\n",
            "  var JSCompiler_object_inline_a_0;\n",
            "  bar();\n",
            "  JSCompiler_object_inline_a_0 = foo(), true;\n",
            "  print(JSCompiler_object_inline_a_0);\n",
            "}\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject17
#[test]
fn test_object17() {
    let mut t = Test::new();
    // Note: Some day, with careful analysis, these two uses could be
    // disambiguated, and the second assignment could be inlined.
    t.test_same_local(concat!(
        "var a = {a: function(){}};\n",
        "a.a();\n",
        "a = {a1: 100};\n",
        "print(a.a1);\n"
    ));
}

// port: InlineObjectLiteralsTest#testObject18
#[test]
fn test_object18() {
    let mut t = Test::new();
    t.test_same_local("var a,b; b=a={x:x, y:y}; f(b.x);");
}

// port: InlineObjectLiteralsTest#testObject19
#[test]
fn test_object19() {
    let mut t = Test::new();
    t.test_same_local("var a,b; if(c) { b=a={x:x, y:y}; } else { b=a={x:y}; } f(b.x);");
}

// port: InlineObjectLiteralsTest#testObject20
#[test]
fn test_object20() {
    let mut t = Test::new();
    t.test_same_local("var a,b; if(c) { b=a={x:x, y:y}; } else { b=a={x:y}; } f(a.x);");
}

// port: InlineObjectLiteralsTest#testObject21
#[test]
fn test_object21() {
    let mut t = Test::new();
    t.test_same_local("var a,b; b=a={x:x, y:y};");
    t.test_same_local(concat!(
        "var a,b; if(c) { b=a={x:x, y:y}; }\n",
        "else { b=a={x:y}; } f(a.x); f(b.x)\n"
    ));
    t.test_same_local(concat!(
        "var a, b; if(c) { if (a={x:x, y:y}) f(); }\n",
        "else { b=a={x:y}; } f(a.x);\n"
    ));
    t.test_same_local("var a,b; b = (a = {x:x, y:x});");
    t.test_same_local("var a,b; a = {x:x, y:x}; b = a");
    t.test_same_local("var a,b; a = {x:x, y:x}; b = x || a");
    t.test_same_local("var a,b; a = {x:x, y:x}; b = y && a");
    t.test_same_local("var a,b; a = {x:x, y:x}; b = y ? a : a");
    t.test_same_local("var a,b; a = {x:x, y:x}; b = y , a");
    t.test_same_local("b = x || (a = {x:1, y:2});");
}

// port: InlineObjectLiteralsTest#testObject22
#[test]
fn test_object22() {
    let mut t = Test::new();
    t.test_local(
        "while(1) { var a = {y:1}; if (b) a.x = 2; f(a.y, a.x);}",
        concat!(
            "for(;1;){\n",
            " var JSCompiler_object_inline_y_0=1;\n",
            " var JSCompiler_object_inline_x_1;\n",
            " if(b) JSCompiler_object_inline_x_1=2;\n",
            " f(JSCompiler_object_inline_y_0,JSCompiler_object_inline_x_1)\n",
            "}\n"
        ),
    );
    t.test_same_local("var a; while (1) { f(a.x, a.y); a = {x:1, y:1};}");
}

// port: InlineObjectLiteralsTest#testObject23
#[test]
fn test_object23() {
    let mut t = Test::new();
    t.test_local(
        concat!(
            "function f() {\n",
            "  var templateData = {\n",
            "    linkIds: {\n",
            "      CHROME: 'cl',\n",
            "      DISMISS: 'd'\n",
            "    }\n",
            "  };\n",
            "  var html = templateData.linkIds.CHROME \n",
            "       + \":\" + templateData.linkIds.DISMISS;\n",
            "}\n"
        ),
        concat!(
            "function f(){\n",
            "var JSCompiler_object_inline_CHROME_1='cl';\n",
            "var JSCompiler_object_inline_DISMISS_2='d';\n",
            "var html=JSCompiler_object_inline_CHROME_1 +\n",
            " ':' +JSCompiler_object_inline_DISMISS_2}\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject24
#[test]
fn test_object24() {
    let mut t = Test::new();
    t.test_local(
        concat!(
            "function f() {\n",
            "  var linkIds = {\n",
            "      CHROME: 1,\n",
            "  };\n",
            "  var g = function () {var o = {a: linkIds};}\n",
            "}\n"
        ),
        concat!(
            "function f(){var linkIds={CHROME:1};\n",
            "var g=function(){var JSCompiler_object_inline_a_0=linkIds}}\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject25
#[test]
fn test_object25() {
    let mut t = Test::new();
    t.test_local(
        "var a = {x:f(), y:g()}; a = {y:g(), x:f()}; f(a.x, a.y);",
        concat!(
            "var JSCompiler_object_inline_x_0=f();\n",
            "var JSCompiler_object_inline_y_1=g();\n",
            "JSCompiler_object_inline_y_1=g(),\n",
            "  JSCompiler_object_inline_x_0=f(),\n",
            "  true;\n",
            "f(JSCompiler_object_inline_x_0,JSCompiler_object_inline_y_1)\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testObject26
#[test]
fn test_object26() {
    let mut t = Test::new();
    t.test_local(
        "var a = {}; a.b = function() {}; new a.b.c",
        concat!(
            "var JSCompiler_object_inline_b_0;\n",
            "JSCompiler_object_inline_b_0=function(){};\n",
            "new JSCompiler_object_inline_b_0.c\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testInlineObjectWithLet
#[test]
fn test_inline_object_with_let() {
    let mut t = Test::new();
    t.test_local(
        "let a = {x:x(), y:y()}; f(a.x, a.y);",
        concat!(
            "var JSCompiler_object_inline_x_0=x();\n",
            "var JSCompiler_object_inline_y_1=y();\n",
            "f(JSCompiler_object_inline_x_0, JSCompiler_object_inline_y_1);\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testInlineObjectWithConst
#[test]
fn test_inline_object_with_const() {
    let mut t = Test::new();
    t.test_local(
        "const a = {x:x(), y:y()}; f(a.x, a.y);",
        concat!(
            "var JSCompiler_object_inline_x_0=x();\n",
            "var JSCompiler_object_inline_y_1=y();\n",
            "f(JSCompiler_object_inline_x_0, JSCompiler_object_inline_y_1);\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testDontInlineLetInForLoopInit
#[test]
fn test_dont_inline_let_in_for_loop_init() {
    let mut t = Test::new();
    // Handling this case should be possible, but we don't currently have that logic in place.
    t.test_same_local("var i; for(let a = {x:x(), y:y()}; i < 0; i++) { f(a.x, a.y); }");
}

// port: InlineObjectLiteralsTest#testDontInlineConstInForLoopInit
#[test]
fn test_dont_inline_const_in_for_loop_init() {
    let mut t = Test::new();
    t.test_same_local("var i; for(const a = {x:x(), y:y()}; i < 0; i++) { f(a.x, a.y); }");
}

// port: InlineObjectLiteralsTest#testDoInlineObjectWithVarInForLoop
#[test]
fn test_do_inline_object_with_var_in_for_loop() {
    let mut t = Test::new();
    // The Normalize pass actually moves "var a = {x: x(), y: y()}" out of the for loop initializer
    // before this pass runs, which is why this works.
    t.test_local(
        "var i; for(var a = {x:x(), y:y()}; i < 0; i++) { f(a.x, a.y); }",
        concat!(
            "var i;\n",
            "var JSCompiler_object_inline_x_0=x();\n",
            "var JSCompiler_object_inline_y_1=y();\n",
            "for(; i < 0; i++) {\n",
            "f(JSCompiler_object_inline_x_0, JSCompiler_object_inline_y_1);\n",
            "}\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testBug545a
#[test]
fn test_bug545a() {
    let mut t = Test::new();
    t.test_local("var a = {}", "");
}

// port: InlineObjectLiteralsTest#testBug545b
#[test]
fn test_bug545b() {
    let mut t = Test::new();
    t.test_local("var a; a = {}", "true");
}

// port: InlineObjectLiteralsTest#testIssue724
#[test]
fn test_issue724() {
    let mut t = Test::new();
    t.test_same_local(concat!(
        "var getType; getType = {};\n",
        "return functionToCheck &&\n",
        "   getType.toString.apply(functionToCheck) ===\n",
        "   '[object Function]';\n"
    ));
}

// port: InlineObjectLiteralsTest#testNoInlineDeletedProperties
#[test]
fn test_no_inline_deleted_properties() {
    let mut t = Test::new();
    t.test_same_local(concat!(
        "var foo = {bar:1};\n",
        "delete foo.bar;\n",
        "return foo.bar;\n"
    ));
}

// port: InlineObjectLiteralsTest#testProto
#[test]
fn test_proto() {
    let mut t = Test::new();
    t.test_same_local(concat!(
        "var protoObject = {\n",
        "  f: function() {\n",
        "    return 1;\n",
        "  }\n",
        "};\n",
        "var object = {\n",
        "  __proto__: protoObject,\n",
        "};\n",
        "g(object.f);\n"
    ));
    t.test_same_parts(vec![
        externs(concat!(
            "var protoObject = {\n",
            "  f: function() {\n",
            "    return 1;\n",
            "  }\n",
            "};\n",
            "var object = {\n",
            "  __proto__: protoObject,\n",
            "  g: false\n",
            "};\n",
            "g(object.g);\n"
        )),
        srcs(concat!(
            "var protoObject = {\n",
            "  f: function() {\n",
            "    return 1;\n",
            "  }\n",
            "};\n",
            "var JSCompiler_object_inline___proto___0=protoObject;\n",
            "var JSCompiler_object_inline_g_1=false;\n",
            "g(JSCompiler_object_inline_g_1)\n"
        )),
    ]);
}

// port: InlineObjectLiteralsTest#testSuper
#[test]
fn test_super() {
    let mut t = Test::new();
    t.test_same_local(concat!(
        "var superObject = {\n",
        "  f() {\n",
        "    return 1;\n",
        "  }\n",
        "};\n",
        "var object = {\n",
        "  __proto__: superObject,\n",
        "  f() {\n",
        "    return super.f();\n",
        "  }\n",
        "}\n",
        "g(object.f());\n"
    ));
}

// port: InlineObjectLiteralsTest#testShorthandFunctions
#[test]
fn test_shorthand_functions() {
    let mut t = Test::new();
    t.test_same_local(concat!(
        "var object = {\n",
        "  items: [],\n",
        "  add(item) {\n",
        "    this.items.push(item);\n",
        "  },\n",
        "};\n",
        "object.add(1);\n"
    ));
    t.test_same_local(concat!(
        "var object = {\n",
        "  one() {\n",
        "    return 1\n",
        "  },\n",
        "};\n",
        "object.one();\n"
    ));
}

// port: InlineObjectLiteralsTest#testShorthandAssignments
#[test]
fn test_shorthand_assignments() {
    let mut t = Test::new();
    t.test_local(
        concat!(
            "var object = {\n",
            "  x,\n",
            "  y\n",
            "};\n",
            "f(object.x, object.y);\n"
        ),
        concat!(
            "var JSCompiler_object_inline_x_0=x;\n",
            "var JSCompiler_object_inline_y_1=y;\n",
            "f(JSCompiler_object_inline_x_0,JSCompiler_object_inline_y_1)\n"
        ),
    );
    t.test_local(
        concat!(
            "var object = {\n",
            "  x,\n",
            "};\n",
            "object.y = y\n",
            "f(object.x, object.y);\n"
        ),
        concat!(
            "var JSCompiler_object_inline_x_0=x;\n",
            "var JSCompiler_object_inline_y_1;\n",
            "var JSCompiler_object_inline_y_1=y;\n",
            "f(JSCompiler_object_inline_x_0,JSCompiler_object_inline_y_1)\n"
        ),
    );
}

// port: InlineObjectLiteralsTest#testComputedPropertyName
#[test]
fn test_computed_property_name() {
    let mut t = Test::new();
    t.test_same_local(concat!(
        "function addBar(name) {\n",
        "  return name + 'Bar'\n",
        "}\n",
        "var object = {\n",
        "  [addBar(\"foo\")]: 1\n",
        "};\n"
    ));
    t.test_same_local(concat!(
        "var sym = Symbol('key');\n",
        "var object = {\n",
        "  [sym]: 1,\n",
        "  x: true\n",
        "}\n",
        "use(object[sym]);\n"
    ));
}

// port: InlineObjectLiteralsTest#testQuotedKeyThatIsNotRead
#[test]
fn test_quoted_key_that_is_not_read() {
    let mut t = Test::new();
    t.test_local(
        "var obj = {'a.b.c': 'd'};",
        "var JSCompiler_object_inline_string_key_0 = 'd';",
    );
    t.test_local(
        "var obj = {'@': 5, '!': 4, 'foo': 3};",
        concat!(
            "var JSCompiler_object_inline_string_key_0 = 5;\n",
            "var JSCompiler_object_inline_string_key_1 = 4;\n",
            "var JSCompiler_object_inline_foo_2 = 3;\n"
        ),
    );
    t.test_same_local("var obj = {}; obj['@'] = 3;");
}

// port: InlineObjectLiteralsTest#testQuotedKeyThatIsRead
#[test]
fn test_quoted_key_that_is_read() {
    let mut t = Test::new();
    t.test_same_local("var obj = {'a.b.c': 'd'}; use(obj['a.b.c']);");
    t.test_same_local("var obj = {}; obj['a'] = 3; use(obj['a']);");
}

// port: InlineObjectLiteralsTest#testObjectSpread_readingFromSpreadAssignment
#[test]
fn test_object_spread_reading_from_spread_assignment() {
    let mut t = Test::new();
    t.test_same_local(concat!("var obj = {...foo};\n", "use(obj.bar);\n"));
}

// port: InlineObjectLiteralsTest#testObjectSpread_readingFromOverwrittenProp
#[test]
fn test_object_spread_reading_from_overwritten_prop() {
    let mut t = Test::new();
    t.test_same_local(concat!(
        "var foo = {prop: 7};\n",
        "var obj = {prop: 6, ...foo};\n",
        "use(obj.prop);\n"
    ));
}
