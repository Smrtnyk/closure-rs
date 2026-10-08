/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CodePrinterTest.java.

#![allow(unused_mut)]
#[path = "support/code_printer_test_base.rs"]
mod code_printer_test_base;
use closure_jscomp::compiler_options::LanguageMode;
use closure_rhino::js_string::JsString;
use code_printer_test_base::CodePrinterTestBase;
// port: CodePrinterTest#testBigInt
#[test]
fn test_big_int() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::UNSUPPORTED;
    t.assert_print_same(JsString::from("1n"));
    t.assert_print(JsString::from("0b10n"), JsString::from("2n"));
    t.assert_print(JsString::from("0o3n"), JsString::from("3n"));
    t.assert_print(JsString::from("0x4n"), JsString::from("4n"));
    t.assert_print_same(JsString::from("-5n"));
    t.assert_print(JsString::from("-0b110n"), JsString::from("-6n"));
    t.assert_print(JsString::from("-0o7n"), JsString::from("-7n"));
    t.assert_print(JsString::from("-0x8n"), JsString::from("-8n"));
    t.assert_print_same(JsString::from("1000000000n"));
    t.assert_print_same(JsString::from("-10000000000n"));
    t.assert_print(
        JsString::from(
            "7182582809266874004429817360811601465721521679690913024666851123813995272999n",
        ),
        JsString::from("0xfe132a356ec5f9279946c8fdf29780abd0387a8277e811069d174660b385b27n"),
    );
    t.assert_print(
        JsString::from("0x000000fe132a356ec5f9279946c8fdf29780abd0387a8277e811069d174660b385b27n"),
        JsString::from("0xfe132a356ec5f9279946c8fdf29780abd0387a8277e811069d174660b385b27n"),
    );
    t.assert_print(
        JsString::from(
            "-7182582809266874004429817360811601465721521679690913024666851123813995272999n",
        ),
        JsString::from("-0xfe132a356ec5f9279946c8fdf29780abd0387a8277e811069d174660b385b27n"),
    );
    t.assert_print(JsString::from("-0o00774114521526730576223631215443757451360052750070365011677201040647213506301316055447n"), JsString::from("-0xfe132a356ec5f9279946c8fdf29780abd0387a8277e811069d174660b385b27n"));
    t.assert_print(JsString::from("0o774114521526730576223631215443757451360052750070365011677201040647213506301316055447n"), JsString::from("0xfe132a356ec5f9279946c8fdf29780abd0387a8277e811069d174660b385b27n"));
    t.assert_print(JsString::from("0b111111100001001100101010001101010110111011000101111110010010011110011001010001101100100011111101111100101001011110000000101010111101000000111000011110101000001001110111111010000001000100000110100111010001011101000110011000001011001110000101101100100111n"), JsString::from("0xfe132a356ec5f9279946c8fdf29780abd0387a8277e811069d174660b385b27n"));
}
// port: CodePrinterTest#testTrailingCommaInArrayAndObjectWithPrettyPrint
#[test]
fn test_trailing_comma_in_array_and_object_with_pretty_print() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("({a:1, b:2,});\n"));
    t.assert_pretty_print_same(JsString::from("[1, 2, 3,];\n"));
    t.assert_pretty_print_same(JsString::from("[, ];\n"));
}
// port: CodePrinterTest#testTrailingCommaInArrayAndObjectWithoutPrettyPrint
#[test]
fn test_trailing_comma_in_array_and_object_without_pretty_print() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("({a:1, b:2,})"),
        JsString::from("({a:1,b:2})"),
    );
    t.assert_print(JsString::from("[1, 2, 3,]"), JsString::from("[1,2,3]"));
    t.assert_print_same(JsString::from("[,]"));
    t.assert_print_same(JsString::from("[a,,]"));
}
// port: CodePrinterTest#testTrailingCommaInParameterListWithPrettyPrint
#[test]
fn test_trailing_comma_in_parameter_list_with_pretty_print() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::UNSUPPORTED;
    t.assert_pretty_print_same(JsString::from("function f(a, b,) {\n}\n"));
    t.assert_pretty_print_same(JsString::from("f(1, 2,);\n"));
    t.assert_pretty_print_same(JsString::from("f?.(1, 2,);\n"));
    t.assert_pretty_print_same(JsString::from("let x = new Number(1,);\n"));
}
// port: CodePrinterTest#testTrailingCommaInParameterListWithoutPrettyPrint
#[test]
fn test_trailing_comma_in_parameter_list_without_pretty_print() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::UNSUPPORTED;
    t.assert_print(
        JsString::from("function f(a, b,) {}"),
        JsString::from("function f(a,b){}"),
    );
    t.assert_print(JsString::from("f(1, 2,);"), JsString::from("f(1,2)"));
    t.assert_print(JsString::from("f?.(1, 2,);"), JsString::from("f?.(1,2)"));
    t.assert_print(
        JsString::from("let x = new Number(1,);"),
        JsString::from("let x=new Number(1)"),
    );
}
// port: CodePrinterTest#optChain
#[test]
fn opt_chain() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::UNSUPPORTED;
    t.assert_print_same(JsString::from("a.b?.c"));
    t.assert_print_same(JsString::from("a.b?.[\"c\"]"));
    t.assert_print_same(JsString::from("a.b?.()"));
    t.assert_print_same(JsString::from("a?.b.c?.d"));
    t.assert_print_same(JsString::from("(a?.b).c"));
    t.assert_print_same(JsString::from("(a.b?.c.d).e"));
    t.assert_print_same(JsString::from("(a?.[b])[c]"));
    t.assert_print_same(JsString::from("(a.b?.())()"));
}
// port: CodePrinterTest#testUnescapedUnicodeLineSeparator_2018
#[test]
fn test_unescaped_unicode_line_separator_2018() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_2018;
    t.assert_print_same(JsString::from("` `"));
    t.assert_print(JsString::from("' '"), JsString::from("\"\\u2028\""));
    t.assert_print(JsString::from("\" \""), JsString::from("\"\\u2028\""));
    t.output_charset = Some("UTF-8".into());
    t.assert_print(JsString::from("' '"), JsString::from("\"\\u2028\""));
    t.assert_print(JsString::from("\" \""), JsString::from("\"\\u2028\""));
}
// port: CodePrinterTest#testUnescapedUnicodeLineSeparator_2019
#[test]
fn test_unescaped_unicode_line_separator_2019() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_2019;
    t.assert_print(JsString::from("' '"), JsString::from("\"\\u2028\""));
    t.assert_print(JsString::from("\" \""), JsString::from("\"\\u2028\""));
    t.output_charset = Some("UTF-8".into());
    t.assert_print(JsString::from("' '"), JsString::from("\" \""));
    t.assert_print(JsString::from("\" \""), JsString::from("\" \""));
}
// port: CodePrinterTest#testUnescapedUnicodeParagraphSeparator_2018
#[test]
fn test_unescaped_unicode_paragraph_separator_2018() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_2018;
    t.assert_print_same(JsString::from("` `"));
    t.assert_print(JsString::from("' '"), JsString::from("\"\\u2029\""));
    t.assert_print(JsString::from("\" \""), JsString::from("\"\\u2029\""));
    t.output_charset = Some("UTF-8".into());
    t.assert_print(JsString::from("' '"), JsString::from("\"\\u2029\""));
    t.assert_print(JsString::from("\" \""), JsString::from("\"\\u2029\""));
}
// port: CodePrinterTest#testUnescapedUnicodeParagraphSeparator_2019
#[test]
fn test_unescaped_unicode_paragraph_separator_2019() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_2019;
    t.assert_print(JsString::from("' '"), JsString::from("\"\\u2029\""));
    t.assert_print(JsString::from("\" \""), JsString::from("\"\\u2029\""));
    t.output_charset = Some("UTF-8".into());
    t.assert_print(JsString::from("' '"), JsString::from("\" \""));
    t.assert_print(JsString::from("\" \""), JsString::from("\" \""));
}
// port: CodePrinterTest#testOptionalCatchBlock
#[test]
fn test_optional_catch_block() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("try{}catch{}"));
    t.assert_print_same(JsString::from("try{}catch{}finally{}"));
}
// port: CodePrinterTest#testExponentiationOperator
#[test]
fn test_exponentiation_operator() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_2016;
    t.assert_print_same(JsString::from("x**y"));
    t.assert_print(JsString::from("x**(y**z)"), JsString::from("x**y**z"));
    t.assert_print_same(JsString::from("(x**y)**z"));
    t.assert_print_same(JsString::from("(-x)**y"));
    t.assert_print_same(JsString::from("-(x**y)"));
    t.assert_print(JsString::from("x**(-y)"), JsString::from("x**-y"));
    t.assert_print(JsString::from("x/(y**z)"), JsString::from("x/y**z"));
    t.assert_print_same(JsString::from("(x/y)**z"));
}
// port: CodePrinterTest#testExponentiationAssignmentOperator
#[test]
fn test_exponentiation_assignment_operator() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_2016;
    t.assert_print_same(JsString::from("x**=y"));
}
// port: CodePrinterTest#testNullishCoalesceOperator
#[test]
fn test_nullish_coalesce_operator() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("x??y??z"));
    t.assert_print_same(JsString::from("x??(y??z)"));
    t.assert_print(JsString::from("(x??y)??z"), JsString::from("x??y??z"));
    t.assert_print_same(JsString::from("(x&&y)??z"));
    t.assert_print_same(JsString::from("(x??y)||z"));
    t.assert_print_same(JsString::from("x??(y||z)"));
}
// port: CodePrinterTest#testNullishCoalesceOperator2
#[test]
fn test_nullish_coalesce_operator2() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("(a|b)??c"), JsString::from("a|b??c"));
    t.assert_print_same(JsString::from("(a??b)|c"));
    t.assert_print_same(JsString::from("a|(b??c)"));
    t.assert_print(JsString::from("a??(b|c)"), JsString::from("a??b|c"));
    t.assert_print(
        JsString::from("(a??b)?(c??d):(e??f)"),
        JsString::from("a??b?c??d:e??f"),
    );
    t.assert_print_same(JsString::from("a??(b?c:d)"));
    t.assert_print_same(JsString::from("(a?b:c)??d"));
}
// port: CodePrinterTest#testLogicalAssignmentOperator
#[test]
fn test_logical_assignment_operator() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("x||=y"));
    t.assert_print_same(JsString::from("x&&=y"));
    t.assert_print_same(JsString::from("x??=y"));
}
// port: CodePrinterTest#testObjectLiteralWithSpread
#[test]
fn test_object_literal_with_spread() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("({...{}})"));
    t.assert_print_same(JsString::from("({...x})"));
    t.assert_print_same(JsString::from("({...x,a:1})"));
    t.assert_print_same(JsString::from("({a:1,...x})"));
    t.assert_print_same(JsString::from("({a:1,...x,b:1})"));
    t.assert_print_same(JsString::from("({...x,...y})"));
    t.assert_print_same(JsString::from("({...x,...f()})"));
    t.assert_print_same(JsString::from("({...{...{}}})"));
}
// port: CodePrinterTest#testObjectLiteralWithComma
#[test]
fn test_object_literal_with_comma() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("({[(a,b)]:c})"));
    t.assert_print_same(JsString::from("({a:(b,c)})"));
    t.assert_print_same(JsString::from("({[(a,b)]:(c,d)})"));
    t.assert_print_same(JsString::from("({[(a,b)]:c,[d]:(e,f)})"));
}
// port: CodePrinterTest#testPrint
#[test]
fn test_print() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("10 + a + b"), JsString::from("10+a+b"));
    t.assert_print(JsString::from("10 + (30*50)"), JsString::from("10+30*50"));
    t.assert_print(
        JsString::from("with(x) { x + 3; }"),
        JsString::from("with(x)x+3"),
    );
    t.assert_print(JsString::from("\"aa'a\""), JsString::from("\"aa'a\""));
    t.assert_print(JsString::from("\"aa\\\"a\""), JsString::from("'aa\"a'"));
    t.assert_print(
        JsString::from("function foo()\n{return 10;}"),
        JsString::from("function foo(){return 10}"),
    );
    t.assert_print(
        JsString::from("a instanceof b"),
        JsString::from("a instanceof b"),
    );
    t.assert_print(JsString::from("typeof(a)"), JsString::from("typeof a"));
    t.assert_print(
        JsString::from("var foo = x ? { a : 1 } : {a: 3, b:4, \"default\": 5, \"foo-bar\": 6}"),
        JsString::from("var foo=x?{a:1}:{a:3,b:4,\"default\":5,\"foo-bar\":6}"),
    );
    t.assert_print(
        JsString::from("function foo(){throw 'error';}"),
        JsString::from("function foo(){throw\"error\";}"),
    );
    t.assert_print(
        JsString::from("var x = 10; { var y = 20; }"),
        JsString::from("var x=10;{var y=20}"),
    );
    t.assert_print(
        JsString::from("while (x-- > 0);"),
        JsString::from("while(x-- >0);"),
    );
    t.assert_print(JsString::from("x-- >> 1"), JsString::from("x-- >>1"));
    t.assert_print(
        JsString::from("(function () {})(); "),
        JsString::from("(function(){})()"),
    );
    t.assert_print(
        JsString::from("var a,b,c,d;a || (b&& c) && (a || d)"),
        JsString::from("var a,b,c,d;a||b&&c&&(a||d)"),
    );
    t.assert_print(
        JsString::from("var a,b,c; a || (b || c); a * (b * c); a | (b | c)"),
        JsString::from("var a,b,c;a||(b||c);a*(b*c);a|(b|c)"),
    );
    t.assert_print(
        JsString::from("var a,b,c; a / b / c;a / (b / c); a - (b - c);"),
        JsString::from("var a,b,c;a/b/c;a/(b/c);a-(b-c)"),
    );
    t.assert_print(
        JsString::from("var a,b; a = b = 3;"),
        JsString::from("var a,b;a=b=3"),
    );
    t.assert_print(
        JsString::from("var a,b,c,d; a = (b = c = (d = 3));"),
        JsString::from("var a,b,c,d;a=b=c=d=3"),
    );
    t.assert_print(
        JsString::from("var a,b,c; a += (b = c += 3);"),
        JsString::from("var a,b,c;a+=b=c+=3"),
    );
    t.assert_print(
        JsString::from("var a,b,c; a *= (b -= c);"),
        JsString::from("var a,b,c;a*=b-=c"),
    );
    t.assert_print(
        JsString::from("a ? delete b[0] : 3"),
        JsString::from("a?delete b[0]:3"),
    );
    t.assert_print(
        JsString::from("(delete a[0])/10"),
        JsString::from("delete a[0]/10"),
    );
    t.assert_print(JsString::from("new A"), JsString::from("new A"));
    t.assert_print(JsString::from("new A()"), JsString::from("new A"));
    t.assert_print(JsString::from("new A('x')"), JsString::from("new A(\"x\")"));
    t.assert_print(JsString::from("new A().a()"), JsString::from("(new A).a()"));
    t.assert_print(JsString::from("(new A).a()"), JsString::from("(new A).a()"));
    t.assert_print(
        JsString::from("new A('y').a()"),
        JsString::from("(new A(\"y\")).a()"),
    );
    t.assert_print(JsString::from("new A.B"), JsString::from("new A.B"));
    t.assert_print(JsString::from("new A.B()"), JsString::from("new A.B"));
    t.assert_print(
        JsString::from("new A.B('z')"),
        JsString::from("new A.B(\"z\")"),
    );
    t.assert_print(
        JsString::from("(new A.B).a()"),
        JsString::from("(new A.B).a()"),
    );
    t.assert_print(
        JsString::from("new A.B().a()"),
        JsString::from("(new A.B).a()"),
    );
    t.assert_print(
        JsString::from("new A.B('w').a()"),
        JsString::from("(new A.B(\"w\")).a()"),
    );
    t.assert_print_same(JsString::from("new (a())"));
    t.assert_print(JsString::from("new (a())()"), JsString::from("new (a())"));
    t.assert_print_same(JsString::from("new (a.b())"));
    t.assert_print(
        JsString::from("new (a.b())()"),
        JsString::from("new (a.b())"),
    );
    t.assert_print(JsString::from("x + +y"), JsString::from("x+ +y"));
    t.assert_print(JsString::from("x - (-y)"), JsString::from("x- -y"));
    t.assert_print(JsString::from("x++ +y"), JsString::from("x++ +y"));
    t.assert_print(JsString::from("x-- -y"), JsString::from("x-- -y"));
    t.assert_print(JsString::from("x++ -y"), JsString::from("x++-y"));
    t.assert_print(
        JsString::from("foo:for(;;){break foo;}"),
        JsString::from("foo:for(;;)break foo"),
    );
    t.assert_print(
        JsString::from("foo:while(1){continue foo;}"),
        JsString::from("foo:while(1)continue foo"),
    );
    t.assert_print_same(JsString::from("foo:;"));
    t.assert_print(JsString::from("foo: {}"), JsString::from("foo:;"));
    t.assert_print(JsString::from("({})"), JsString::from("({})"));
    t.assert_print(JsString::from("var x = {};"), JsString::from("var x={}"));
    t.assert_print(JsString::from("({}).x"), JsString::from("({}).x"));
    t.assert_print(JsString::from("({})['x']"), JsString::from("({})[\"x\"]"));
    t.assert_print(
        JsString::from("({}) instanceof Object"),
        JsString::from("({})instanceof Object"),
    );
    t.assert_print(JsString::from("({}) || 1"), JsString::from("({})||1"));
    t.assert_print(JsString::from("1 || ({})"), JsString::from("1||{}"));
    t.assert_print(JsString::from("({}) ? 1 : 2"), JsString::from("({})?1:2"));
    t.assert_print(JsString::from("0 ? ({}) : 2"), JsString::from("0?{}:2"));
    t.assert_print(JsString::from("0 ? 1 : ({})"), JsString::from("0?1:{}"));
    t.assert_print(JsString::from("typeof ({})"), JsString::from("typeof{}"));
    t.assert_print(JsString::from("f({})"), JsString::from("f({})"));
    t.assert_print(
        JsString::from("(function(){})"),
        JsString::from("(function(){})"),
    );
    t.assert_print(
        JsString::from("(function(){})()"),
        JsString::from("(function(){})()"),
    );
    t.assert_print(
        JsString::from("(function(){})instanceof Object"),
        JsString::from("(function(){})instanceof Object"),
    );
    t.assert_print(
        JsString::from("(function(){}).bind().call()"),
        JsString::from("(function(){}).bind().call()"),
    );
    t.assert_print(
        JsString::from("var x = function() { };"),
        JsString::from("var x=function(){}"),
    );
    t.assert_print(
        JsString::from("var x = function() { }();"),
        JsString::from("var x=function(){}()"),
    );
    t.assert_print(
        JsString::from("(function() {}), 2"),
        JsString::from("(function(){}),2"),
    );
    t.assert_print(
        JsString::from("(function f(){})"),
        JsString::from("(function f(){})"),
    );
    t.assert_print(
        JsString::from("function f(){}"),
        JsString::from("function f(){}"),
    );
    t.assert_print(
        JsString::from("({ 'a': 4, '\\u0100': 4 })"),
        JsString::from("({\"a\":4,\"\\u0100\":4})"),
    );
    t.assert_print(
        JsString::from("({ a: 4, '\\u0100': 4 })"),
        JsString::from("({a:4,\"\\u0100\":4})"),
    );
    t.assert_print(
        JsString::from("if (true) { alert();}"),
        JsString::from("if(true)alert()"),
    );
    t.assert_print(
        JsString::from("if (false) {} else {alert(\"a\");}"),
        JsString::from("if(false);else alert(\"a\")"),
    );
    t.assert_print(
        JsString::from("for(;;) { alert();};"),
        JsString::from("for(;;)alert()"),
    );
    t.assert_print(
        JsString::from("do { alert(); } while(true);"),
        JsString::from("do alert();while(true)"),
    );
    t.assert_print(
        JsString::from("myLabel: { alert();}"),
        JsString::from("myLabel:alert()"),
    );
    t.assert_print(
        JsString::from("myLabel: for(;;) continue myLabel;"),
        JsString::from("myLabel:for(;;)continue myLabel"),
    );
    t.assert_print(
        JsString::from("if (true) var x; x = 4;"),
        JsString::from("if(true)var x;x=4"),
    );
    t.assert_print(JsString::from("\\u00fb"), JsString::from("\\u00fb"));
    t.assert_print(JsString::from("\\u00fa=1"), JsString::from("\\u00fa=1"));
    t.assert_print(
        JsString::from("function \\u00f9(){}"),
        JsString::from("function \\u00f9(){}"),
    );
    t.assert_print(JsString::from("x.\\u00f8"), JsString::from("x.\\u00f8"));
    t.assert_print(JsString::from("x.\\u00f8"), JsString::from("x.\\u00f8"));
    t.assert_print(
        JsString::from("abc\\u4e00\\u4e01jkl"),
        JsString::from("abc\\u4e00\\u4e01jkl"),
    );
    t.assert_print(JsString::from("! ! true"), JsString::from("!!true"));
    t.assert_print(JsString::from("!(!(true))"), JsString::from("!!true"));
    t.assert_print(
        JsString::from("typeof(void(0))"),
        JsString::from("typeof void 0"),
    );
    t.assert_print(
        JsString::from("typeof(void(!0))"),
        JsString::from("typeof void!0"),
    );
    t.assert_print(JsString::from("+ - + + - + 3"), JsString::from("+-+ +-+3"));
    t.assert_print(JsString::from("+(--x)"), JsString::from("+--x"));
    t.assert_print(JsString::from("-(++x)"), JsString::from("-++x"));
    t.assert_print(JsString::from("-(--x)"), JsString::from("- --x"));
    t.assert_print(JsString::from("!(~~5)"), JsString::from("!~~5"));
    t.assert_print(JsString::from("~(a/b)"), JsString::from("~(a/b)"));
    t.assert_print(
        JsString::from("new (foo.bar()).factory(baz)"),
        JsString::from("new (foo.bar().factory)(baz)"),
    );
    t.assert_print(
        JsString::from("new (bar()).factory(baz)"),
        JsString::from("new (bar().factory)(baz)"),
    );
    t.assert_print(
        JsString::from("new (new foobar(x)).factory(baz)"),
        JsString::from("new (new foobar(x)).factory(baz)"),
    );
    t.assert_print(
        JsString::from("a ? b : (c ? d : e)"),
        JsString::from("a?b:c?d:e"),
    );
    t.assert_print(
        JsString::from("a ? (b ? c : d) : e"),
        JsString::from("a?b?c:d:e"),
    );
    t.assert_print(
        JsString::from("(a ? b : c) ? d : e"),
        JsString::from("(a?b:c)?d:e"),
    );
    t.assert_print(
        JsString::from("if (x) if (y); else;"),
        JsString::from("if(x)if(y);else;"),
    );
    t.assert_print(JsString::from("a,b,c"), JsString::from("a,b,c"));
    t.assert_print(JsString::from("(a,b),c"), JsString::from("a,b,c"));
    t.assert_print(JsString::from("a,(b,c)"), JsString::from("a,b,c"));
    t.assert_print(JsString::from("x=a,b,c"), JsString::from("x=a,b,c"));
    t.assert_print(JsString::from("x=(a,b),c"), JsString::from("x=(a,b),c"));
    t.assert_print(JsString::from("x=a,(b,c)"), JsString::from("x=a,b,c"));
    t.assert_print(JsString::from("x=a,y=b,z=c"), JsString::from("x=a,y=b,z=c"));
    t.assert_print(
        JsString::from("x=(a,y=b,z=c)"),
        JsString::from("x=(a,y=b,z=c)"),
    );
    t.assert_print(JsString::from("x=[a,b,c,d]"), JsString::from("x=[a,b,c,d]"));
    t.assert_print(
        JsString::from("x=[(a,b,c),d]"),
        JsString::from("x=[(a,b,c),d]"),
    );
    t.assert_print(
        JsString::from("x=[(a,(b,c)),d]"),
        JsString::from("x=[(a,b,c),d]"),
    );
    t.assert_print(
        JsString::from("x=[a,(b,c,d)]"),
        JsString::from("x=[a,(b,c,d)]"),
    );
    t.assert_print(JsString::from("var x=(a,b)"), JsString::from("var x=(a,b)"));
    t.assert_print(JsString::from("var x=a,b,c"), JsString::from("var x=a,b,c"));
    t.assert_print(
        JsString::from("var x=(a,b),c"),
        JsString::from("var x=(a,b),c"),
    );
    t.assert_print(
        JsString::from("var x=a,b=(c,d)"),
        JsString::from("var x=a,b=(c,d)"),
    );
    t.assert_print(
        JsString::from("var x=(a,b)(c);"),
        JsString::from("var x=(a,b)(c)"),
    );
    t.assert_print(
        JsString::from("var x=(a,b)`c`;"),
        JsString::from("var x=(a,b)`c`"),
    );
    t.assert_print(
        JsString::from("foo(a,b,c,d)"),
        JsString::from("foo(a,b,c,d)"),
    );
    t.assert_print(
        JsString::from("foo((a,b,c),d)"),
        JsString::from("foo((a,b,c),d)"),
    );
    t.assert_print(
        JsString::from("foo((a,(b,c)),d)"),
        JsString::from("foo((a,b,c),d)"),
    );
    t.assert_print(
        JsString::from("f(a+b,(c,d,(e,f,g)))"),
        JsString::from("f(a+b,(c,d,e,f,g))"),
    );
    t.assert_print(JsString::from("({}) , 1 , 2"), JsString::from("({}),1,2"));
    t.assert_print(
        JsString::from("({}) , {} , {}"),
        JsString::from("({}),{},{}"),
    );
    t.assert_print_same(JsString::from("var a=(b=c,d)"));
    t.assert_print_same(JsString::from("var a=(b[c]=d,e)"));
    t.assert_print_same(JsString::from("var a=(b[c]=d,e[f]=g,h)"));
    t.assert_print(
        JsString::from("var a = /** @type {?} */ (b=c,d)"),
        JsString::from("var a=(b=c,d)"),
    );
    t.assert_print(
        JsString::from("var a = /** @type {?} */ (b[c]=d,e)"),
        JsString::from("var a=(b[c]=d,e)"),
    );
    t.assert_print(
        JsString::from("var a = /** @type {?} */ (b[c]=d,e[f]=g,h)"),
        JsString::from("var a=(b[c]=d,e[f]=g,h)"),
    );
    t.assert_print(JsString::from("if (x){}"), JsString::from("if(x);"));
    t.assert_print(JsString::from("if(x);"), JsString::from("if(x);"));
    t.assert_print(JsString::from("if(x)if(y);"), JsString::from("if(x)if(y);"));
    t.assert_print(
        JsString::from("if(x){if(y);}"),
        JsString::from("if(x)if(y);"),
    );
    t.assert_print(
        JsString::from("if(x){if(y){};;;}"),
        JsString::from("if(x)if(y);"),
    );
}
// port: CodePrinterTest#testPrintNewVoid
#[test]
fn test_print_new_void() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("new (void 0)"));
}
// port: CodePrinterTest#testPrettyPrintJSDoc
#[test]
fn test_pretty_print_js_doc() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("/** @type {number} */ \nvar x;\n"));
}
// port: CodePrinterTest#testPrintCast1
#[test]
fn test_print_cast1() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x = /** @type {number} */ (0);"),
        JsString::from("var x=0"),
    );
    t.assert_pretty_print_same(JsString::from("var x = /** @type {number} */ (0);\n"));
}
// port: CodePrinterTest#testPrintCast2
#[test]
fn test_print_cast2() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x = (2+3) * 4;"),
        JsString::from("var x=(2+3)*4"),
    );
    t.assert_print(
        JsString::from("var x = /** @type {number} */ (2+3) * 4;"),
        JsString::from("var x=(2+3)*4"),
    );
    t.assert_pretty_print_same(JsString::from(
        "var x = (/** @type {number} */ (2 + 3)) * 4;\n",
    ));
}
// port: CodePrinterTest#testPrintCast3
#[test]
fn test_print_cast3() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x = (2*3) + 4;"),
        JsString::from("var x=2*3+4"),
    );
    t.assert_print(
        JsString::from("var x = /** @type {number} */ (2*3) + 4;"),
        JsString::from("var x=2*3+4"),
    );
    t.assert_pretty_print_same(JsString::from(
        "var x = /** @type {number} */ (2 * 3) + 4;\n",
    ));
}
// port: CodePrinterTest#testLetConstInIf
#[test]
fn test_let_const_in_if() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print(
        JsString::from("if (true) { let x; };"),
        JsString::from("if(true){let x}"),
    );
    t.assert_print(
        JsString::from("if (true) { const x = 0; };"),
        JsString::from("if(true){const x=0}"),
    );
}
// port: CodePrinterTest#testPrintBlockScopedFunctions
#[test]
fn test_print_block_scoped_functions() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("if (true) function foo(){return}"),
        JsString::from("if(true){function foo(){return}}"),
    );
    t.assert_print(
        JsString::from("if(x){;;function y(){};;}"),
        JsString::from("if(x){function y(){}}"),
    );
}
// port: CodePrinterTest#testPrintArrayPatternVar
#[test]
fn test_print_array_pattern_var() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("var []=[]"));
    t.assert_print_same(JsString::from("var [a]=[1]"));
    t.assert_print_same(JsString::from("var [a,b]=[1,2]"));
    t.assert_print_same(JsString::from("var [a,...b]=[1,2]"));
    t.assert_print_same(JsString::from("var [,b]=[1,2]"));
    t.assert_print_same(JsString::from("var [,,,,,,g]=[1,2,3,4,5,6,7]"));
    t.assert_print_same(JsString::from("var [a,,c]=[1,2,3]"));
    t.assert_print_same(JsString::from("var [a,,,d]=[1,2,3,4]"));
    t.assert_print_same(JsString::from("var [a,,c,,e]=[1,2,3,4,5]"));
}
// port: CodePrinterTest#testPrintArrayPatternLet
#[test]
fn test_print_array_pattern_let() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("let []=[]"));
    t.assert_print_same(JsString::from("let [a]=[1]"));
    t.assert_print_same(JsString::from("let [a,b]=[1,2]"));
    t.assert_print_same(JsString::from("let [a,...b]=[1,2]"));
    t.assert_print_same(JsString::from("let [,b]=[1,2]"));
    t.assert_print_same(JsString::from("let [,,,,,,g]=[1,2,3,4,5,6,7]"));
    t.assert_print_same(JsString::from("let [a,,c]=[1,2,3]"));
    t.assert_print_same(JsString::from("let [a,,,d]=[1,2,3,4]"));
    t.assert_print_same(JsString::from("let [a,,c,,e]=[1,2,3,4,5]"));
}
// port: CodePrinterTest#testPrintArrayPatternConst
#[test]
fn test_print_array_pattern_const() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("const []=[]"));
    t.assert_print_same(JsString::from("const [a]=[1]"));
    t.assert_print_same(JsString::from("const [a,b]=[1,2]"));
    t.assert_print_same(JsString::from("const [a,...b]=[1,2]"));
    t.assert_print_same(JsString::from("const [,b]=[1,2]"));
    t.assert_print_same(JsString::from("const [,,,,,,g]=[1,2,3,4,5,6,7]"));
    t.assert_print_same(JsString::from("const [a,,c]=[1,2,3]"));
    t.assert_print_same(JsString::from("const [a,,,d]=[1,2,3,4]"));
    t.assert_print_same(JsString::from("const [a,,c,,e]=[1,2,3,4,5]"));
}
// port: CodePrinterTest#testPrintArrayPatternAssign
#[test]
fn test_print_array_pattern_assign() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("[]=[]"));
    t.assert_print_same(JsString::from("[a]=[1]"));
    t.assert_print_same(JsString::from("[a,b]=[1,2]"));
    t.assert_print_same(JsString::from("[a,...b]=[1,2]"));
    t.assert_print_same(JsString::from("[,b]=[1,2]"));
    t.assert_print_same(JsString::from("[,,,,,,g]=[1,2,3,4,5,6,7]"));
    t.assert_print_same(JsString::from("[a,,c]=[1,2,3]"));
    t.assert_print_same(JsString::from("[a,,,d]=[1,2,3,4]"));
    t.assert_print_same(JsString::from("[a,,c,,e]=[1,2,3,4,5]"));
}
// port: CodePrinterTest#testPrintArrayPatternWithInitializer
#[test]
fn test_print_array_pattern_with_initializer() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("[x=1]=[]"));
    t.assert_print_same(JsString::from("[a,,c=2,,e]=[1,2,3,4,5]"));
    t.assert_print_same(JsString::from("[a=1,b=2,c=3]=foo()"));
    t.assert_print_same(JsString::from("[a=(1,2),b]=foo()"));
    t.assert_print_same(JsString::from("[a=[b=(1,2)]=bar(),c]=foo()"));
}
// port: CodePrinterTest#testPrintNestedArrayPattern
#[test]
fn test_print_nested_array_pattern() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("var [a,[b,c],d]=[1,[2,3],4]"));
    t.assert_print_same(JsString::from("var [[[[a]]]]=[[[[1]]]]"));
    t.assert_print_same(JsString::from("[a,[b,c],d]=[1,[2,3],4]"));
    t.assert_print_same(JsString::from("[[[[a]]]]=[[[[1]]]]"));
}
// port: CodePrinterTest#testPrettyPrintArrayPattern
#[test]
fn test_pretty_print_array_pattern() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("let [a,b,c]=foo();"),
        JsString::from("let [a, b, c] = foo();\n"),
    );
}
// port: CodePrinterTest#testPrintObjectPatternVar
#[test]
fn test_print_object_pattern_var() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("var {a}=foo()"));
    t.assert_print_same(JsString::from("var {a,b}=foo()"));
    t.assert_print_same(JsString::from("var {a:a,b:b}=foo()"));
}
// port: CodePrinterTest#testPrintObjectPatternLet
#[test]
fn test_print_object_pattern_let() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("let {a}=foo()"));
    t.assert_print_same(JsString::from("let {a,b}=foo()"));
    t.assert_print_same(JsString::from("let {a:a,b:b}=foo()"));
}
// port: CodePrinterTest#testPrintObjectPatternConst
#[test]
fn test_print_object_pattern_const() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("const {a}=foo()"));
    t.assert_print_same(JsString::from("const {a,b}=foo()"));
    t.assert_print_same(JsString::from("const {a:a,b:b}=foo()"));
}
// port: CodePrinterTest#testPrintObjectPatternAssign
#[test]
fn test_print_object_pattern_assign() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("({a}=foo())"));
    t.assert_print_same(JsString::from("({a,b}=foo())"));
    t.assert_print_same(JsString::from("({a:a,b:b}=foo())"));
}
// port: CodePrinterTest#testPrintNestedObjectPattern
#[test]
fn test_print_nested_object_pattern() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("({a:{b,c}}=foo())"));
    t.assert_print_same(JsString::from("({a:{b:{c:{d}}}}=foo())"));
}
// port: CodePrinterTest#testPrintObjectPatternInitializer
#[test]
fn test_print_object_pattern_initializer() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("({a=1}=foo())"));
    t.assert_print_same(JsString::from("({a:{b=2}}=foo())"));
    t.assert_print_same(JsString::from("({a:b=2}=foo())"));
    t.assert_print_same(JsString::from("({a,b:{c=2}}=foo())"));
    t.assert_print_same(JsString::from("({a:{b=2},c}=foo())"));
    t.assert_print_same(JsString::from("({a=(1,2),b}=foo())"));
    t.assert_print_same(JsString::from("({a:b=(1,2),c}=foo())"));
}
// port: CodePrinterTest#testPrintObjectPatternWithRest
#[test]
fn test_print_object_pattern_with_rest() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("const {a,...rest}=foo()"));
    t.assert_print_same(JsString::from("var {a,...rest}=foo()"));
    t.assert_print_same(JsString::from("let {a,...rest}=foo()"));
    t.assert_print_same(JsString::from("({a,...rest}=foo())"));
    t.assert_print_same(JsString::from("({a=2,...rest}=foo())"));
    t.assert_print_same(JsString::from("({a:b=2,...rest}=foo())"));
}
// port: CodePrinterTest#testPrettyPrintObjectPattern
#[test]
fn test_pretty_print_object_pattern() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("const {a,b,c}=foo();"),
        JsString::from("const {a, b, c} = foo();\n"),
    );
}
// port: CodePrinterTest#testPrintMixedDestructuring
#[test]
fn test_print_mixed_destructuring() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("({a:[b,c]}=foo())"));
    t.assert_print_same(JsString::from("[a,{b,c}]=foo()"));
}
// port: CodePrinterTest#testPrintDestructuringInParamList
#[test]
fn test_print_destructuring_in_param_list() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("function f([a]){}"));
    t.assert_print_same(JsString::from("function f([a,b]){}"));
    t.assert_print_same(JsString::from("function f([a,b]=c()){}"));
    t.assert_print_same(JsString::from("function f([a=(1,2),b=(3,4)]=c()){}"));
    t.assert_print_same(JsString::from("function f({a}){}"));
    t.assert_print_same(JsString::from("function f({a,b}){}"));
    t.assert_print_same(JsString::from("function f({a,b}=c()){}"));
    t.assert_print_same(JsString::from("function f([a,{b,c}]){}"));
    t.assert_print_same(JsString::from("function f({a,b:[c,d]}){}"));
}
// port: CodePrinterTest#testPrintDestructuringInRestParam
#[test]
fn test_print_destructuring_in_rest_param() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("function f(...[a,b]){}"));
    t.assert_print_same(JsString::from("function f(...{length:num_params}){}"));
}
// port: CodePrinterTest#testDestructuringForInLoops
#[test]
fn test_destructuring_for_in_loops() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("for({a}in b)c"));
    t.assert_print_same(JsString::from("for(var {a}in b)c"));
    t.assert_print_same(JsString::from("for(let {a}in b)c"));
    t.assert_print_same(JsString::from("for(const {a}in b)c"));
    t.assert_print_same(JsString::from("for({a:b}in c)d"));
    t.assert_print_same(JsString::from("for(var {a:b}in c)d"));
    t.assert_print_same(JsString::from("for(let {a:b}in c)d"));
    t.assert_print_same(JsString::from("for(const {a:b}in c)d"));
    t.assert_print_same(JsString::from("for([a]in b)c"));
    t.assert_print_same(JsString::from("for(var [a]in b)c"));
    t.assert_print_same(JsString::from("for(let [a]in b)c"));
    t.assert_print_same(JsString::from("for(const [a]in b)c"));
}
// port: CodePrinterTest#testDestructuringForOfLoops1
#[test]
fn test_destructuring_for_of_loops1() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("for({a}of b)c"));
    t.assert_print_same(JsString::from("for(var {a}of b)c"));
    t.assert_print_same(JsString::from("for(let {a}of b)c"));
    t.assert_print_same(JsString::from("for(const {a}of b)c"));
    t.assert_print_same(JsString::from("for({a:b}of c)d"));
    t.assert_print_same(JsString::from("for(var {a:b}of c)d"));
    t.assert_print_same(JsString::from("for(let {a:b}of c)d"));
    t.assert_print_same(JsString::from("for(const {a:b}of c)d"));
    t.assert_print_same(JsString::from("for([a]of b)c"));
    t.assert_print_same(JsString::from("for(var [a]of b)c"));
    t.assert_print_same(JsString::from("for(let [a]of b)c"));
    t.assert_print_same(JsString::from("for(const [a]of b)c"));
}
// port: CodePrinterTest#testDestructuringForOfLoops2
#[test]
fn test_destructuring_for_of_loops2() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("for(a of b)var {x}=y"));
}
// port: CodePrinterTest#testBreakTrustedStrings
#[test]
fn test_break_trusted_strings() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("'<script>'"), JsString::from("\"<script>\""));
    t.assert_print(
        JsString::from("'</script>'"),
        JsString::from("\"\\x3c/script>\""),
    );
    t.assert_print(
        JsString::from("\"</script> </SCRIPT>\""),
        JsString::from("\"\\x3c/script> \\x3c/SCRIPT>\""),
    );
    t.assert_print(JsString::from("'-->'"), JsString::from("\"--\\x3e\""));
    t.assert_print(JsString::from("']]>'"), JsString::from("\"]]\\x3e\""));
    t.assert_print(
        JsString::from("' --></script>'"),
        JsString::from("\" --\\x3e\\x3c/script>\""),
    );
    t.assert_print(
        JsString::from("/--> <\\/script>/g"),
        JsString::from("/--\\x3e <\\/script>/g"),
    );
    t.assert_print(
        JsString::from("'<!-- I am a string -->'"),
        JsString::from("\"\\x3c!-- I am a string --\\x3e\""),
    );
    t.assert_print(JsString::from("'<=&>'"), JsString::from("\"<=&>\""));
}
// port: CodePrinterTest#testBreakUntrustedStrings
#[test]
fn test_break_untrusted_strings() {
    let mut t = CodePrinterTestBase::new();
    t.trusted_strings = false;
    t.assert_print(
        JsString::from("'<script>'"),
        JsString::from("\"\\x3cscript\\x3e\""),
    );
    t.assert_print(
        JsString::from("'</script>'"),
        JsString::from("\"\\x3c/script\\x3e\""),
    );
    t.assert_print(
        JsString::from("\"</script> </SCRIPT>\""),
        JsString::from("\"\\x3c/script\\x3e \\x3c/SCRIPT\\x3e\""),
    );
    t.assert_print(JsString::from("'-->'"), JsString::from("\"--\\x3e\""));
    t.assert_print(JsString::from("']]>'"), JsString::from("\"]]\\x3e\""));
    t.assert_print(
        JsString::from("' --></script>'"),
        JsString::from("\" --\\x3e\\x3c/script\\x3e\""),
    );
    t.assert_print(
        JsString::from("/--> <\\/script>/g"),
        JsString::from("/--\\x3e <\\/script>/g"),
    );
    t.assert_print(
        JsString::from("'<!-- I am a string -->'"),
        JsString::from("\"\\x3c!-- I am a string --\\x3e\""),
    );
    t.assert_print(
        JsString::from("'<=&>'"),
        JsString::from("\"\\x3c\\x3d\\x26\\x3e\""),
    );
    t.assert_print(JsString::from("/(?=x)/"), JsString::from("/(?=x)/"));
}
// port: CodePrinterTest#testBreakTemplateLiterals
#[test]
fn test_break_template_literals() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print(
        JsString::from("`</script>`"),
        JsString::from("`\\x3c/script>`"),
    );
    t.assert_print(
        JsString::from("`</SCRIPT>`"),
        JsString::from("`\\x3c/SCRIPT>`"),
    );
    t.assert_print(
        JsString::from("`</style>`"),
        JsString::from("`\\x3c/style>`"),
    );
    t.assert_print(
        JsString::from("`</STYLE>`"),
        JsString::from("`\\x3c/STYLE>`"),
    );
    t.assert_print(JsString::from("`-->`"), JsString::from("`--\\x3e`"));
    t.assert_print(JsString::from("`]]>`"), JsString::from("`]]\\x3e`"));
    t.assert_print(
        JsString::from("` --></script>`"),
        JsString::from("` --\\x3e\\x3c/script>`"),
    );
    t.assert_print(
        JsString::from("`<!-- I am a string -->`"),
        JsString::from("`\\x3c!-- I am a string --\\x3e`"),
    );
}
// port: CodePrinterTest#testBreakUntrustedTemplateLiterals
#[test]
fn test_break_untrusted_template_literals() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.trusted_strings = false;
    t.assert_print(
        JsString::from("`</script>`"),
        JsString::from("`\\x3c/script\\x3e`"),
    );
    t.assert_print(
        JsString::from("`<!-- I am a string -->`"),
        JsString::from("`\\x3c!-- I am a string --\\x3e`"),
    );
    t.assert_print(
        JsString::from("`<=&>`"),
        JsString::from("`\\x3c\\x3d\\x26\\x3e`"),
    );
}
// port: CodePrinterTest#testHtmlComments
#[test]
fn test_html_comments() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("3< !(--x)"), JsString::from("3< !--x"));
    t.assert_print(
        JsString::from("while (x-- > 0) {}"),
        JsString::from("while(x-- >0);"),
    );
}
// port: CodePrinterTest#testPrintArray
#[test]
fn test_print_array() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("[void 0, void 0]"),
        JsString::from("[void 0,void 0]"),
    );
    t.assert_print(
        JsString::from("[undefined, undefined]"),
        JsString::from("[undefined,undefined]"),
    );
    t.assert_print(
        JsString::from("[ , , , undefined]"),
        JsString::from("[,,,undefined]"),
    );
    t.assert_print(JsString::from("[ , , , 0]"), JsString::from("[,,,0]"));
}
// port: CodePrinterTest#testHook
#[test]
fn test_hook() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("a ? b = 1 : c = 2"),
        JsString::from("a?b=1:c=2"),
    );
    t.assert_print(
        JsString::from("x = a ? b = 1 : c = 2"),
        JsString::from("x=a?b=1:c=2"),
    );
    t.assert_print(
        JsString::from("(x = a) ? b = 1 : c = 2"),
        JsString::from("(x=a)?b=1:c=2"),
    );
    t.assert_print(
        JsString::from("x, a ? b = 1 : c = 2"),
        JsString::from("x,a?b=1:c=2"),
    );
    t.assert_print(
        JsString::from("x, (a ? b = 1 : c = 2)"),
        JsString::from("x,a?b=1:c=2"),
    );
    t.assert_print(
        JsString::from("(x, a) ? b = 1 : c = 2"),
        JsString::from("(x,a)?b=1:c=2"),
    );
    t.assert_print(
        JsString::from("a ? (x, b) : c = 2"),
        JsString::from("a?(x,b):c=2"),
    );
    t.assert_print(
        JsString::from("a ? b = 1 : (x,c)"),
        JsString::from("a?b=1:(x,c)"),
    );
    t.assert_print(
        JsString::from("a ? b = 1 : c = 2 + x"),
        JsString::from("a?b=1:c=2+x"),
    );
    t.assert_print(
        JsString::from("(a ? b = 1 : c = 2) + x"),
        JsString::from("(a?b=1:c=2)+x"),
    );
    t.assert_print(
        JsString::from("a ? b = 1 : (c = 2) + x"),
        JsString::from("a?b=1:(c=2)+x"),
    );
    t.assert_print(
        JsString::from("a ? (b?1:2) : 3"),
        JsString::from("a?b?1:2:3"),
    );
}
// port: CodePrinterTest#testForIn
#[test]
fn test_for_in() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("for(a in b)c"));
    t.assert_print_same(JsString::from("for(var a in b)c"));
    t.assert_print_same(JsString::from("for(var a in b=c)d"));
    t.assert_print_same(JsString::from("for(var a in b,c)d"));
}
// port: CodePrinterTest#testPrintInOperatorInForLoop
#[test]
fn test_print_in_operator_in_for_loop() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var a={}; for (var i = (\"length\" in a); i;) {}"),
        JsString::from("var a={};for(var i=(\"length\"in a);i;);"),
    );
    t.assert_print(
        JsString::from("var a={}; for (var i = (\"length\" in a) ? 0 : 1; i;) {}"),
        JsString::from("var a={};for(var i=(\"length\"in a)?0:1;i;);"),
    );
    t.assert_print(
        JsString::from("var a={}; for (var i = (\"length\" in a) + 1; i;) {}"),
        JsString::from("var a={};for(var i=(\"length\"in a)+1;i;);"),
    );
    t.assert_print(
        JsString::from("var a={};for (var i = (\"length\" in a|| \"size\" in a);;);"),
        JsString::from("var a={};for(var i=(\"length\"in a)||(\"size\"in a);;);"),
    );
    t.assert_print(
        JsString::from("var a={};for (var i = (a || a) || (\"size\" in a);;);"),
        JsString::from("var a={};for(var i=a||a||(\"size\"in a);;);"),
    );
    t.assert_print(
        JsString::from("var a={}; for (var i = -(\"length\" in a); i;) {}"),
        JsString::from("var a={};for(var i=-(\"length\"in a);i;);"),
    );
    t.assert_print(
        JsString::from(
            "var a={};function b_(p){ return p;};for(var i=1,j=b_(\"length\" in a);;) {}",
        ),
        JsString::from("var a={};function b_(p){return p}for(var i=1,j=b_(\"length\"in a);;);"),
    );
    t.assert_print(
        JsString::from("var a={}; for (;(\"length\" in a);) {}"),
        JsString::from("var a={};for(;\"length\"in a;);"),
    );
    t.assert_print_same(JsString::from("for(x,(y in z);;)foo()"));
    t.assert_print_same(JsString::from("for(var x,w=(y in z);;)foo()"));
    t.assert_print_same(JsString::from("for(a=c?0:(0 in d);;)foo()"));
    t.assert_print(
        JsString::from("var a={}; for(var i = () => (0 in a); i;) {}"),
        JsString::from("var a={};for(var i=()=>(0 in a);i;);"),
    );
    t.assert_print(
        JsString::from("var a={}; for(var i = () => ({} in a); i;) {}"),
        JsString::from("var a={};for(var i=()=>({}in a);i;);"),
    );
    t.assert_print(
        JsString::from("var a={}; for(var {noop} = (\"prop\" in a); noop;) {}"),
        JsString::from("var a={};for(var {noop}=(\"prop\"in a);noop;);"),
    );
}
// port: CodePrinterTest#testForOf
#[test]
fn test_for_of() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("for(a of b)c"));
    t.assert_print_same(JsString::from("for(var a of b)c"));
    t.assert_print_same(JsString::from("for(var a of b=c)d"));
    t.assert_print_same(JsString::from("for(var a of(b,c))d"));
}
// port: CodePrinterTest#testForOfPretty
#[test]
fn test_for_of_pretty() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("for ([x, y] of b) {\n  c;\n}\n"));
    t.assert_pretty_print_same(JsString::from("for (x of [[1, 2]]) {\n  c;\n}\n"));
    t.assert_pretty_print_same(JsString::from("for ([x, y] of [[1, 2]]) {\n  c;\n}\n"));
}
// port: CodePrinterTest#testForAwaitOf
#[test]
fn test_for_await_of() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("async()=>{for await(a of b)c}"));
    t.assert_print_same(JsString::from("async()=>{for await(var a of b)c}"));
    t.assert_print_same(JsString::from("async()=>{for await(var a of b=c)d}"));
    t.assert_print_same(JsString::from("async()=>{for await(var a of(b,c))d}"));
}
// port: CodePrinterTest#testForAwaitOfPretty
#[test]
fn test_for_await_of_pretty() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_pretty_print_same(JsString::from(
        "async() => {\n  for await ([x, y] of b) {\n    c;\n  }\n};\n",
    ));
    t.assert_pretty_print_same(JsString::from(
        "async() => {\n  for await (x of [[1, 2]]) {\n    c;\n  }\n};\n",
    ));
    t.assert_pretty_print_same(JsString::from(
        "async() => {\n  for await ([x, y] of [[1, 2]]) {\n    c;\n  }\n};\n",
    ));
}
// port: CodePrinterTest#testLetFor
#[test]
fn test_let_for() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("for(let a=0;a<5;a++)b"));
    t.assert_print_same(JsString::from("for(let a in b)c"));
    t.assert_print_same(JsString::from("for(let a of b)c"));
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("async()=>{for await(let a of b)c}"));
}
// port: CodePrinterTest#testConstFor
#[test]
fn test_const_for() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("for(const a=5;b<a;b++)c"));
    t.assert_print_same(JsString::from("for(const a in b)c"));
    t.assert_print_same(JsString::from("for(const a of b)c"));
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("async()=>{for await(const a of b)c}"));
}
// port: CodePrinterTest#testLiteralProperty
#[test]
fn test_literal_property() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("(64).toString()"),
        JsString::from("(64).toString()"),
    );
}
// port: CodePrinterTest#testLineBreak
#[test]
fn test_line_break() {
    let mut t = CodePrinterTestBase::new();
    t.assert_line_break(
        JsString::from("function a() {}\nfunction b() {}\n"),
        JsString::from("function a(){}\nfunction b(){}\n"),
    );
    t.assert_line_break(
        JsString::from("var a = {};\na.foo = function () {}\nfunction b() {}\n"),
        JsString::from("var a={};a.foo=function(){};\nfunction b(){}\n"),
    );
    t.assert_line_break(
        JsString::from("var a = {\n  b: function() {},\n  c: function() {}\n};\nalert(a);\n"),
        JsString::from("var a={b:function(){},\nc:function(){}};\nalert(a)"),
    );
}
// port: CodePrinterTest#testPrettyPrinter
#[test]
fn test_pretty_printer() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("(function(){})();"),
        JsString::from("(function() {\n})();\n"),
    );
    t.assert_pretty_print(
        JsString::from("var a = (function() {});alert(a);"),
        JsString::from("var a = function() {\n};\nalert(a);\n"),
    );
    t.assert_pretty_print(JsString::from("if (1) {}"), JsString::from("if (1) {\n}\n"));
    t.assert_pretty_print(
        JsString::from("if (1) {alert(\"\");}"),
        JsString::from("if (1) {\n  alert(\"\");\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("if (1)alert(\"\");"),
        JsString::from("if (1) {\n  alert(\"\");\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("if (1) {alert();alert();}"),
        JsString::from("if (1) {\n  alert();\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("label: alert();"),
        JsString::from("label: alert();\n"),
    );
    t.assert_pretty_print(
        JsString::from("if (1) alert();"),
        JsString::from("if (1) {\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("for (;;) alert();"),
        JsString::from("for (;;) {\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("while (1) alert();"),
        JsString::from("while (1) {\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("if (1) {} else {alert(a);}"),
        JsString::from("if (1) {\n} else {\n  alert(a);\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("if (1) alert(a); else alert(b);"),
        JsString::from("if (1) {\n  alert(a);\n} else {\n  alert(b);\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("for(;;) { alert();}"),
        JsString::from("for (;;) {\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("for(;;) {}"),
        JsString::from("for (;;) {\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("for(;;) { alert(); alert(); }"),
        JsString::from("for (;;) {\n  alert();\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("for(var x=0;x<10;x++) { alert(); alert(); }"),
        JsString::from("for (var x = 0; x < 10; x++) {\n  alert();\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("do { alert(); } while(true);"),
        JsString::from("do {\n  alert();\n} while (true);\n"),
    );
    t.assert_pretty_print(
        JsString::from("myLabel: { alert();}"),
        JsString::from("myLabel: {\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("myLabel: {}"),
        JsString::from("myLabel: {\n}\n"),
    );
    t.assert_pretty_print(JsString::from("myLabel: ;"), JsString::from("myLabel: ;\n"));
    t.assert_pretty_print(
        JsString::from("myLabel: for(;;) continue myLabel;"),
        JsString::from("myLabel: for (;;) {\n  continue myLabel;\n}\n"),
    );
    t.assert_pretty_print(JsString::from("var a;"), JsString::from("var a;\n"));
    t.assert_pretty_print(JsString::from("i--"), JsString::from("i--;\n"));
    t.assert_pretty_print(JsString::from("i++"), JsString::from("i++;\n"));
    t.assert_pretty_print(
        JsString::from("var foo = 3+5;"),
        JsString::from("var foo = 3 + 5;\n"),
    );
    t.assert_pretty_print(
        JsString::from("var foo = bar ? 3 : null;"),
        JsString::from("var foo = bar ? 3 : null;\n"),
    );
    t.assert_pretty_print(
        JsString::from("function foo() { return \"foo\"; }"),
        JsString::from("function foo() {\n  return \"foo\";\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("throw \"foo\";"),
        JsString::from("throw \"foo\";\n"),
    );
    t.assert_pretty_print(
        JsString::from("do{ alert(); } while(true);"),
        JsString::from("do {\n  alert();\n} while (true);\n"),
    );
    t.assert_pretty_print(
        JsString::from("while(true) { alert(); }"),
        JsString::from("while (true) {\n  alert();\n}\n"),
    );
}
// port: CodePrinterTest#testPrettyPrinter2
#[test]
fn test_pretty_printer2() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("if(true) f();"),
        JsString::from("if (true) {\n  f();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("if (true) { f() } else { g() }"),
        JsString::from("if (true) {\n  f();\n} else {\n  g();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("if(true) f(); for(;;) g();"),
        JsString::from("if (true) {\n  f();\n}\nfor (;;) {\n  g();\n}\n"),
    );
}
// port: CodePrinterTest#testPrettyPrinter3
#[test]
fn test_pretty_printer3() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("try {} catch(e) {}if (1) {alert();alert();}"),
        JsString::from("try {\n} catch (e) {\n}\nif (1) {\n  alert();\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("try {} finally {}if (1) {alert();alert();}"),
        JsString::from("try {\n} finally {\n}\nif (1) {\n  alert();\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("try {} catch(e) {} finally {} if (1) {alert();alert();}"),
        JsString::from(
            "try {\n} catch (e) {\n} finally {\n}\nif (1) {\n  alert();\n  alert();\n}\n",
        ),
    );
}
// port: CodePrinterTest#testPrettyPrinter4
#[test]
fn test_pretty_printer4() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("function f() {}if (1) {alert();}"),
        JsString::from("function f() {\n}\nif (1) {\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("var f = function() {};if (1) {alert();}"),
        JsString::from("var f = function() {\n};\nif (1) {\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("(function() {})();if (1) {alert();}"),
        JsString::from("(function() {\n})();\nif (1) {\n  alert();\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("(function() {alert();alert();})();if (1) {alert();}"),
        JsString::from("(function() {\n  alert();\n  alert();\n})();\nif (1) {\n  alert();\n}\n"),
    );
}
// port: CodePrinterTest#testPrettyPrinter_arrow
#[test]
fn test_pretty_printer_arrow() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(JsString::from("(a)=>123;"), JsString::from("a => 123;\n"));
}
// port: CodePrinterTest#testPrettyPrinter_defaultValue
#[test]
fn test_pretty_printer_default_value() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("(a=1)=>123;"),
        JsString::from("(a = 1) => 123;\n"),
    );
    t.assert_pretty_print(
        JsString::from("[a=(1,2)]=[];"),
        JsString::from("[a = (1, 2)] = [];\n"),
    );
}
// port: CodePrinterTest#testPrettyPrinter_spaceBeforeUnaryOperators
#[test]
fn test_pretty_printer_space_before_unary_operators() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("var f = function() { return !b; };"),
        JsString::from("var f = function() {\n  return !b;\n};\n"),
    );
    t.assert_pretty_print(
        JsString::from("var f = function*(){yield -b}"),
        JsString::from("var f = function*() {\n  yield -b;\n};\n"),
    );
    t.assert_pretty_print(
        JsString::from("var f = function() { return +b; };"),
        JsString::from("var f = function() {\n  return +b;\n};\n"),
    );
    t.assert_pretty_print(
        JsString::from("var f = function() { throw ~b; };"),
        JsString::from("var f = function() {\n  throw ~b;\n};\n"),
    );
    t.assert_pretty_print(
        JsString::from("var f = function() { return ++b; };"),
        JsString::from("var f = function() {\n  return ++b;\n};\n"),
    );
    t.assert_pretty_print(
        JsString::from("var f = function() { return --b; };"),
        JsString::from("var f = function() {\n  return --b;\n};\n"),
    );
}
// port: CodePrinterTest#testPrettyPrinter_varLetConst
#[test]
fn test_pretty_printer_var_let_const() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(JsString::from("var x=0;"), JsString::from("var x = 0;\n"));
    t.assert_pretty_print(
        JsString::from("const x=0;"),
        JsString::from("const x = 0;\n"),
    );
    t.assert_pretty_print(JsString::from("let x=0;"), JsString::from("let x = 0;\n"));
}
// port: CodePrinterTest#testReturnWithTypeAnnotation
#[test]
fn test_return_with_type_annotation() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_type_annotations = true;
    t.assert_pretty_print(JsString::from("function f() { return (/** @return {number} */ function() { return 42; }); }"), JsString::from("function f() {\n  return (/**\n   * @return {number}\n   */\n  function() {\n    return 42;\n  });\n}\n"));
}
// port: CodePrinterTest#testDeprecatedAnnotationIncludesNewline
#[test]
fn test_deprecated_annotation_includes_newline() {
    let mut t = CodePrinterTestBase::new();
    let js = JsString::from(
        "/**\n * @type {number}\n * @deprecated See {@link replacementClass} for more details.\n */\nvar x;\n",
    );
    t.assert_pretty_print(js.clone(), js.clone());
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_nonTrailing_blockComment
#[test]
fn test_non_js_doc_comments_printed_non_trailing_block_comment() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("/* testComment */ function Foo(){}"),
        JsString::from("/* testComment */ function Foo() {\n}\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_endOfFile_lineComment
#[test]
fn test_non_js_doc_comments_printed_end_of_file_line_comment() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("function f1() {}\nif (true) {\n// first\nf1();\n}\n// second\n"),
        JsString::from("function f1() {\n}\nif (true) {\n  // first\n  f1();\n}\n // second\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_endOfBlockComment
#[test]
fn test_non_js_doc_comments_printed_end_of_block_comment() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("function f1() {}\nif (true) {\n// first\nf1();\n/* second */\n}\n"),
        JsString::from(
            "function f1() {\n}\nif (true) {\n  // first\n  f1(); \n  /* second */\n}\n",
        ),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_endOfBlock_manyMixedComments
#[test]
fn test_non_js_doc_comments_printed_end_of_block_many_mixed_comments() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(JsString::from("function f1() {}\nif (true) {\n// first\nf1();\n// second\n/* third */\n// fourth\n}\n"), JsString::from("function f1() {\n}\nif (true) {\n  // first\n  f1(); \n  // second\n  /* third */\n  // fourth\n}\n"));
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_lastTrailing
#[test]
fn test_non_js_doc_comments_printed_last_trailing() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("function f1() {}\nif (true) {\n// first\nf1(); // second\n}\n"),
        JsString::from("function f1() {\n}\nif (true) {\n  // first\n  f1(); // second\n}\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_nonTrailing_lineComment
#[test]
fn test_non_js_doc_comments_printed_non_trailing_line_comment() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("// testComment\nfunction Foo(){}"),
        JsString::from("// testComment\nfunction Foo() {\n}\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_betweenCode_sameLine
#[test]
fn test_non_js_doc_comments_printed_between_code_same_line() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("function /* testComment */ Foo(){}"),
        JsString::from("function/* testComment */ Foo() {\n}\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_betweenCode_differentLines
#[test]
fn test_non_js_doc_comments_printed_between_code_different_lines() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("function /* testComment */\nFoo(){}"),
        JsString::from("function/* testComment */\nFoo() {\n}\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_nonTrailing_inlineComments
#[test]
fn test_non_js_doc_comments_printed_non_trailing_inline_comments() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("function Foo(/*first*/ x, /* second*/ y) {}"),
        JsString::from("function Foo(/*first*/ x, /* second*/ y) {\n}\n"),
    );
}
// port: CodePrinterTest#testArgs_noComments_newLines
#[test]
fn test_args_no_comments_new_lines() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from(" var rpcid = new RpcId(a,\n b, \nc);"),
        JsString::from("var rpcid = new RpcId(a, b, c);\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_nonTrailing_inlineComments_newLines
#[test]
fn test_non_js_doc_comments_printed_non_trailing_inline_comments_new_lines() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from(" var rpcid = new RpcId(a,\n /* comment1 */ b, \n/* comment1 */ c);"),
        JsString::from("var rpcid = new RpcId(a, /* comment1 */ b, /* comment1 */ c);\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_trailing_and_nonTrailing_inlineComments
#[test]
fn test_non_js_doc_comments_printed_trailing_and_non_trailing_inline_comments() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("function Foo(x //first\n, /* second*/ y) {}"),
        JsString::from("function Foo(x //first\n, /* second*/ y) {\n}\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_trailing_inlineComments_paramList
#[test]
fn test_non_js_doc_comments_printed_trailing_inline_comments_param_list() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("function Foo(x) {}"),
        JsString::from("function Foo(x) {\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("function Foo(x /*first*/) {}"),
        JsString::from("function Foo(x /*first*/) {\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("function Foo(x //first\n) {}"),
        JsString::from("function Foo(x //first\n) {\n}\n"),
    );
}
// port: CodePrinterTest#testClassExtendsLeftHandSideExpression
#[test]
fn test_class_extends_left_hand_side_expression() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("class A {} class B extends (0, A) {}"),
        JsString::from("class A {\n}\nclass B extends(0, A) {\n}\n"),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_trailing_inlineComments_callArgList
#[test]
fn test_non_js_doc_comments_printed_trailing_inline_comments_call_arg_list() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(JsString::from("foo(x);"), JsString::from("foo(x);\n"));
    t.assert_pretty_print(
        JsString::from("foo(x /*first*/);"),
        JsString::from("foo(x /*first*/);\n"),
    );
    t.assert_pretty_print(
        JsString::from("foo(x //first\n);"),
        JsString::from("foo(x //first\n);\n"),
    );
}
// port: CodePrinterTest#testFunctionWithCall
#[test]
fn test_function_with_call() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var user = new function() {\nalert(\"foo\")}\n"),
        JsString::from("var user=new function(){alert(\"foo\")}"),
    );
    t.assert_print(JsString::from("var user = new function() {\nthis.name = \"foo\";\nthis.local = function(){alert(this.name)};}\n"), JsString::from("var user=new function(){this.name=\"foo\";this.local=function(){alert(this.name)}}"));
}
// port: CodePrinterTest#testLineLength
#[test]
fn test_line_length() {
    let mut t = CodePrinterTestBase::new();
    t.assert_line_length(
        JsString::from("var aba,bcb,cdc"),
        JsString::from("var aba,bcb,\ncdc"),
    );
    t.assert_line_length(
        JsString::from("\"foo\"+\"bar,baz,bomb\"+\"whee\"+\";long-string\"\n+\"aaa\""),
        JsString::from("\"foo\"+\"bar,baz,bomb\"+\n\"whee\"+\";long-string\"+\n\"aaa\""),
    );
    t.assert_line_length(
        JsString::from("var abazaba=1234"),
        JsString::from("var abazaba=\n1234"),
    );
    t.assert_line_length(
        JsString::from("var abab=1;var bab=2"),
        JsString::from("var abab=1;\nvar bab=2"),
    );
    t.assert_line_length(
        JsString::from("var a=/some[reg](ex),with.*we?rd|chars/i;var b=a"),
        JsString::from("var a=/some[reg](ex),with.*we?rd|chars/i;\nvar b=a"),
    );
    t.assert_line_length(
        JsString::from("var a=\"foo,{bar};baz\";var b=a"),
        JsString::from("var a=\"foo,{bar};baz\";\nvar b=a"),
    );
    t.assert_line_length(
        JsString::from("var a=\"a\";a++;var b=\"bbb\";"),
        JsString::from("var a=\"a\";a++;\nvar b=\"bbb\""),
    );
}
// port: CodePrinterTest#testParsePrintParse
#[test]
fn test_parse_print_parse() {
    let mut t = CodePrinterTestBase::new();
    t.test_reparse(JsString::from("3;"));
    t.test_reparse(JsString::from("var a = b;"));
    t.test_reparse(JsString::from("var x, y, z;"));
    t.test_reparse(JsString::from("try { foo() } catch(e) { bar() }"));
    t.test_reparse(JsString::from(
        "try { foo() } catch(e) { bar() } finally { stuff() }",
    ));
    t.test_reparse(JsString::from("try { foo() } finally { stuff() }"));
    t.test_reparse(JsString::from("throw 'me'"));
    t.test_reparse(JsString::from("function foo(a) { return a + 4; }"));
    t.test_reparse(JsString::from("function foo() { return; }"));
    t.test_reparse(JsString::from(
        "var a = function(a, b) { foo(); return a + b; }",
    ));
    t.test_reparse(JsString::from("b = [3, 4, 'paul', \"Buchhe it\",,5];"));
    t.test_reparse(JsString::from("v = (5, 6, 7, 8)"));
    t.test_reparse(JsString::from("d = 34.0; x = 0; y = .3; z = -22"));
    t.test_reparse(JsString::from("d = -x; t = !x + ~y;"));
    t.test_reparse(JsString::from(
        "'hi'; /* just a test */ stuff(a,b) \n foo(); // and another \n bar();\n",
    ));
    t.test_reparse(JsString::from(
        "a = b++ + ++c; a = b++-++c; a = - --b; a = - ++b;",
    ));
    t.test_reparse(JsString::from(
        "a++; b= a++; b = ++a; b = a--; b = --a; a+=2; b-=5",
    ));
    t.test_reparse(JsString::from("a = (2 + 3) * 4;"));
    t.test_reparse(JsString::from("a = 1 + (2 + 3) + 4;"));
    t.test_reparse(JsString::from(
        "x = a ? b : c; x = a ? (b,3,5) : (foo(),bar());",
    ));
    t.test_reparse(JsString::from(
        "a = b | c || d ^ e\n&& f & !g != h << i <= j < k >>> l > m * n % !o\n",
    ));
    t.test_reparse(JsString::from(
        "a == b; a != b; a === b; a == b == a;\n (a == b) == a; a == (b == a);\n",
    ));
    t.test_reparse(JsString::from(
        "if (a > b) a = b; if (b < 3) a = 3; else c = 4;",
    ));
    t.test_reparse(JsString::from(
        "if (a == b) { a++; } if (a == 0) { a++; } else { a --; }",
    ));
    t.test_reparse(JsString::from("for (var i in a) b += i;"));
    t.test_reparse(JsString::from(
        "for (var i = 0; i < 10; i++){ b /= 2;\n if (b == 2)break;else continue;}\n",
    ));
    t.test_reparse(JsString::from("for (x = 0; x < 10; x++) a /= 2;"));
    t.test_reparse(JsString::from("for (;;) a++;"));
    t.test_reparse(JsString::from("while(true) { blah(); }while(true) blah();"));
    t.test_reparse(JsString::from("do stuff(); while(a>b);"));
    t.test_reparse(JsString::from("[0, null, , true, false, this];"));
    t.test_reparse(JsString::from(
        "s.replace(/absc/, 'X').replace(/ab/gi, 'Y');",
    ));
    t.test_reparse(JsString::from("new Foo; new Bar(a, b,c);"));
    t.test_reparse(JsString::from(
        "with(foo()) { x = z; y = t; } with(bar()) a = z;",
    ));
    t.test_reparse(JsString::from("delete foo['bar']; delete foo;"));
    t.test_reparse(JsString::from("var x = { 'a':'paul', 1:'3', 2:(3,4) };"));
    t.test_reparse(JsString::from("switch(a) { case 2: case 3: stuff(); break;\ncase 4: morestuff(); break; default: done();}\n"));
    t.test_reparse(JsString::from(
        "x = foo['bar'] + foo['my stuff'] + foo[bar] + f.stuff;",
    ));
    t.test_reparse(JsString::from("a.v = b.v; x['foo'] = y['zoo'];"));
    t.test_reparse(JsString::from("'test' in x; 3 in x; a in x;"));
    t.test_reparse(JsString::from(
        "'foo\"bar' + \"foo'c\" + 'stuff\\n and \\\\more'",
    ));
    t.test_reparse(JsString::from("x.__proto__;"));
}
// port: CodePrinterTest#testDoLoopIECompatibility
#[test]
fn test_do_loop_ie_compatibility() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("function f(){if(e1){do foo();while(e2)}else foo()}"),
        JsString::from("function f(){if(e1){do foo();while(e2)}else foo()}"),
    );
    t.assert_print(
        JsString::from("function f(){if(e1)do foo();while(e2)else foo()}"),
        JsString::from("function f(){if(e1){do foo();while(e2)}else foo()}"),
    );
    t.assert_print(
        JsString::from("if(x){do{foo()}while(y)}else bar()"),
        JsString::from("if(x){do foo();while(y)}else bar()"),
    );
    t.assert_print(
        JsString::from("if(x)do{foo()}while(y);else bar()"),
        JsString::from("if(x){do foo();while(y)}else bar()"),
    );
    t.assert_print(
        JsString::from("if(x){do{foo()}while(y)}"),
        JsString::from("if(x){do foo();while(y)}"),
    );
    t.assert_print(
        JsString::from("if(x)do{foo()}while(y);"),
        JsString::from("if(x){do foo();while(y)}"),
    );
    t.assert_print(
        JsString::from("if(x)A:do{foo()}while(y);"),
        JsString::from("if(x){A:do foo();while(y)}"),
    );
    t.assert_print(
        JsString::from("var i = 0;a: do{b: do{i++;break b;} while(0);} while(0);"),
        JsString::from("var i=0;a:do{b:do{i++;break b}while(0)}while(0)"),
    );
}
// port: CodePrinterTest#testFunctionSafariCompatibility
#[test]
fn test_function_safari_compatibility() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("function f(){if(e1){function goo(){return true}}else foo()}"),
        JsString::from("function f(){if(e1){function goo(){return true}}else foo()}"),
    );
    t.assert_print(
        JsString::from("function f(){if(e1)function goo(){return true}else foo()}"),
        JsString::from("function f(){if(e1){function goo(){return true}}else foo()}"),
    );
    t.assert_print(
        JsString::from("if(e1){function goo(){return true}}"),
        JsString::from("if(e1){function goo(){return true}}"),
    );
    t.assert_print(
        JsString::from("if(e1)function goo(){return true}"),
        JsString::from("if(e1){function goo(){return true}}"),
    );
}
// port: CodePrinterTest#testBiggerThanMaxLongNumericLiterals
#[test]
fn test_bigger_than_max_long_numeric_literals() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("9223372036854775808"),
        JsString::from("0x7fffffffffffffff"),
    );
    t.assert_print(
        JsString::from("0x8000000000000000"),
        JsString::from("0x7fffffffffffffff"),
    );
    t.assert_print(
        JsString::from("0b1000000000000000000000000000000000000000000000000000000000000000"),
        JsString::from("0x7fffffffffffffff"),
    );
    t.assert_print(
        JsString::from("0o1000000000000000000000"),
        JsString::from("0x7fffffffffffffff"),
    );
}
// port: CodePrinterTest#testDirectEval
#[test]
fn test_direct_eval() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("eval('1');"), JsString::from("eval(\"1\")"));
}
// port: CodePrinterTest#optChainCalleeForNewRequiresParentheses
#[test]
fn opt_chain_callee_for_new_requires_parentheses() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("new (a?.b)"));
}
// port: CodePrinterTest#testFreeCall1
#[test]
fn test_free_call1() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("foo(a);"), JsString::from("foo(a)"));
    t.assert_print(JsString::from("x.foo(a);"), JsString::from("x.foo(a)"));
}
// port: CodePrinterTest#testObjectLit
#[test]
fn test_object_lit() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("({x:1})"), JsString::from("({x:1})"));
    t.assert_print(
        JsString::from("var x=({x:1})"),
        JsString::from("var x={x:1}"),
    );
    t.assert_print(
        JsString::from("var x={'x':1}"),
        JsString::from("var x={\"x\":1}"),
    );
    t.assert_print(JsString::from("var x={1:1}"), JsString::from("var x={1:1}"));
    t.assert_print(JsString::from("({},42)+0"), JsString::from("({},42)+0"));
}
// port: CodePrinterTest#testObjectLit2
#[test]
fn test_object_lit2() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("var x={1:1}"), JsString::from("var x={1:1}"));
    t.assert_print(
        JsString::from("var x={'1':1}"),
        JsString::from("var x={1:1}"),
    );
    t.assert_print(
        JsString::from("var x={'1.0':1}"),
        JsString::from("var x={\"1.0\":1}"),
    );
    t.assert_print(
        JsString::from("var x={1.5:1}"),
        JsString::from("var x={\"1.5\":1}"),
    );
}
// port: CodePrinterTest#testObjectLit3
#[test]
fn test_object_lit3() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x={3E9:1}"),
        JsString::from("var x={3E9:1}"),
    );
    t.assert_print(
        JsString::from("var x={'3000000000':1}"),
        JsString::from("var x={3E9:1}"),
    );
    t.assert_print(
        JsString::from("var x={'3000000001':1}"),
        JsString::from("var x={3000000001:1}"),
    );
    t.assert_print(
        JsString::from("var x={'6000000001':1}"),
        JsString::from("var x={6000000001:1}"),
    );
    t.assert_print(
        JsString::from("var x={\"12345678901234567\":1}"),
        JsString::from("var x={\"12345678901234567\":1}"),
    );
}
// port: CodePrinterTest#testObjectLit4
#[test]
fn test_object_lit4() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x={\"123456789012345671234567890123456712345678901234567\":1}"),
        JsString::from("var x={\"123456789012345671234567890123456712345678901234567\":1}"),
    );
}
// port: CodePrinterTest#testExtendedObjectLit
#[test]
fn test_extended_object_lit() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("var a={b}"));
    t.assert_print_same(JsString::from("var a={b,c}"));
    t.assert_print_same(JsString::from("var a={b,c:d,e}"));
    t.assert_print_same(JsString::from("var a={b,c(){},d,e:f}"));
}
// port: CodePrinterTest#testComputedProperties
#[test]
fn test_computed_properties() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("var a={[b]:c}"));
    t.assert_print_same(JsString::from("var a={[b+3]:c}"));
    t.assert_print_same(JsString::from("var a={[b](){}}"));
    t.assert_print_same(JsString::from("var a={[b](){alert(foo)}}"));
    t.assert_print_same(JsString::from("var a={*[b](){yield\"foo\"}}"));
    t.assert_print_same(JsString::from("var a={[b]:()=>c}"));
    t.assert_print_same(JsString::from("var a={get [b](){return null}}"));
    t.assert_print_same(JsString::from("var a={set [b](val){window.b=val}}"));
}
// port: CodePrinterTest#testComputedPropertiesClassMethods
#[test]
fn test_computed_properties_class_methods() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("class C{[m](){}}"));
    t.assert_print_same(JsString::from("class C{[\"foo\"+bar](){alert(1)}}"));
}
// port: CodePrinterTest#testGetter
#[test]
fn test_getter() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("var x = {}"), JsString::from("var x={}"));
    t.assert_print(
        JsString::from("var x = {get a() {return 1}}"),
        JsString::from("var x={get a(){return 1}}"),
    );
    t.assert_print(
        JsString::from("var x = {get a() {}, get b(){}}"),
        JsString::from("var x={get a(){},get b(){}}"),
    );
    t.assert_print(
        JsString::from("var x = {get 'a'() {return 1}}"),
        JsString::from("var x={get \"a\"(){return 1}}"),
    );
    t.assert_print(
        JsString::from("var x = {get 1() {return 1}}"),
        JsString::from("var x={get 1(){return 1}}"),
    );
    t.assert_print(
        JsString::from("var x = {get \"()\"() {return 1}}"),
        JsString::from("var x={get \"()\"(){return 1}}"),
    );
    t.language_mode = LanguageMode::ECMASCRIPT5;
    t.assert_print_same(JsString::from("var x={get function(){return 1}}"));
}
// port: CodePrinterTest#testSetter
#[test]
fn test_setter() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("var x = {}"), JsString::from("var x={}"));
    t.assert_print(
        JsString::from("var x = {set a(y) {return 1}}"),
        JsString::from("var x={set a(y){return 1}}"),
    );
    t.assert_print(
        JsString::from("var x = {get 'a'() {return 1}}"),
        JsString::from("var x={get \"a\"(){return 1}}"),
    );
    t.assert_print(
        JsString::from("var x = {set 1(y) {return 1}}"),
        JsString::from("var x={set 1(y){return 1}}"),
    );
    t.assert_print(
        JsString::from("var x = {set \"(x)\"(y) {return 1}}"),
        JsString::from("var x={set \"(x)\"(y){return 1}}"),
    );
    t.language_mode = LanguageMode::ECMASCRIPT5;
    t.assert_print_same(JsString::from("var x={set function(x){}}"));
}
// port: CodePrinterTest#testNegNoCollapse
#[test]
fn test_neg_no_collapse() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x = - - 2;"),
        JsString::from("var x=- -2"),
    );
    t.assert_print(JsString::from("var x = - (2);"), JsString::from("var x=-2"));
}
// port: CodePrinterTest#testIjsWithProvideAlreadyProvided
#[test]
fn test_ijs_with_provide_already_provided() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from(
        "/** @provideAlreadyProvided */ \ngoog.provide(\"a.b.c\");\n",
    ));
}
// port: CodePrinterTest#testArrayLiteral
#[test]
fn test_array_literal() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("var x = [,];"), JsString::from("var x=[,]"));
    t.assert_print(
        JsString::from("var x = [,,];"),
        JsString::from("var x=[,,]"),
    );
    t.assert_print(
        JsString::from("var x = [,s,,];"),
        JsString::from("var x=[,s,,]"),
    );
    t.assert_print(
        JsString::from("var x = [,s];"),
        JsString::from("var x=[,s]"),
    );
    t.assert_print(JsString::from("var x = [s,];"), JsString::from("var x=[s]"));
}
// port: CodePrinterTest#testZero
#[test]
fn test_zero() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x ='\\0';"),
        JsString::from("var x=\"\\x00\""),
    );
    t.assert_print(
        JsString::from("var x ='\\x00';"),
        JsString::from("var x=\"\\x00\""),
    );
    t.assert_print(
        JsString::from("var x ='\\u0000';"),
        JsString::from("var x=\"\\x00\""),
    );
    t.assert_print(
        JsString::from("var x ='\\u00003';"),
        JsString::from("var x=\"\\x003\""),
    );
}
// port: CodePrinterTest#testOctalInString
#[test]
fn test_octal_in_string() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT5;
    t.assert_print(
        JsString::from("var x ='\\0';"),
        JsString::from("var x=\"\\x00\""),
    );
    t.assert_print(
        JsString::from("var x ='\\07';"),
        JsString::from("var x=\"\\u0007\""),
    );
    t.assert_print(
        JsString::from("var x ='\\012';"),
        JsString::from("var x=\"\\n\""),
    );
    t.assert_print(
        JsString::from("var x ='\\013';"),
        JsString::from("var x=\"\\v\""),
    );
    t.assert_print(
        JsString::from("var x ='\\034';"),
        JsString::from("var x=\"\\u001c\""),
    );
    t.assert_print(
        JsString::from("var x ='\\08';"),
        JsString::from("var x=\"\\x008\""),
    );
    t.assert_print(
        JsString::from("var x ='\\09';"),
        JsString::from("var x=\"\\x009\""),
    );
    t.assert_print(
        JsString::from("var x ='\\01234';"),
        JsString::from("var x=\"\\n34\""),
    );
}
// port: CodePrinterTest#testOctalInStringNoLeadingZero
#[test]
fn test_octal_in_string_no_leading_zero() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT5;
    t.assert_print(
        JsString::from("var x ='\\7';"),
        JsString::from("var x=\"\\u0007\""),
    );
    t.assert_print(
        JsString::from("var x ='\\12';"),
        JsString::from("var x=\"\\n\""),
    );
    t.assert_print(
        JsString::from("var x ='\\13';"),
        JsString::from("var x=\"\\v\""),
    );
    t.assert_print(
        JsString::from("var x ='\\34';"),
        JsString::from("var x=\"\\u001c\""),
    );
    t.assert_print(
        JsString::from("var x ='\\240';"),
        JsString::from("var x=\"\\u00a0\""),
    );
    t.assert_print(
        JsString::from("var x ='\\2400';"),
        JsString::from("var x=\"\\u00a00\""),
    );
    t.assert_print(
        JsString::from("var x ='\\6789';"),
        JsString::from("var x=\"789\""),
    );
    t.assert_print(
        JsString::from("var x ='\\8';"),
        JsString::from("var x=\"8\""),
    );
    t.assert_print(
        JsString::from("var x ='\\9';"),
        JsString::from("var x=\"9\""),
    );
    t.assert_print(
        JsString::from("var x ='\\1234';"),
        JsString::from("var x=\"S4\""),
    );
}
// port: CodePrinterTest#testUnicode
#[test]
fn test_unicode() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x ='\\x0f';"),
        JsString::from("var x=\"\\u000f\""),
    );
    t.assert_print(
        JsString::from("var x ='\\x68';"),
        JsString::from("var x=\"h\""),
    );
    t.assert_print(
        JsString::from("var x ='\\x7f';"),
        JsString::from("var x=\"\\u007f\""),
    );
}
// port: CodePrinterTest#testOctalNumericKey
#[test]
fn test_octal_numeric_key() {
    let mut t = CodePrinterTestBase::new();
    t.allow_warnings = true;
    t.language_mode = LanguageMode::ECMASCRIPT5;
    t.assert_print(
        JsString::from("var x = {010: 1};"),
        JsString::from("var x={8:1}"),
    );
}
// port: CodePrinterTest#testNumericKeys
#[test]
fn test_numeric_keys() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x = {'010': 1};"),
        JsString::from("var x={\"010\":1}"),
    );
    t.assert_print(
        JsString::from("var x = {0x10: 1};"),
        JsString::from("var x={16:1}"),
    );
    t.assert_print(
        JsString::from("var x = {'0x10': 1};"),
        JsString::from("var x={\"0x10\":1}"),
    );
    t.assert_print(
        JsString::from("var x = {.2: 1};"),
        JsString::from("var x={\"0.2\":1}"),
    );
    t.assert_print(
        JsString::from("var x = {'.2': 1};"),
        JsString::from("var x={\".2\":1}"),
    );
    t.assert_print(
        JsString::from("var x = {0.2: 1};"),
        JsString::from("var x={\"0.2\":1}"),
    );
    t.assert_print(
        JsString::from("var x = {'0.2': 1};"),
        JsString::from("var x={\"0.2\":1}"),
    );
}
// port: CodePrinterTest#testIssue582
#[test]
fn test_issue582() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("var x = -0.0;"), JsString::from("var x=-0"));
}
// port: CodePrinterTest#testIssue942
#[test]
fn test_issue942() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var x = {0: 1};"),
        JsString::from("var x={0:1}"),
    );
}
// port: CodePrinterTest#testIssue601
#[test]
fn test_issue601() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("'\\v' == 'v'"),
        JsString::from("\"\\v\"==\"v\""),
    );
    t.assert_print(
        JsString::from("'\\u000B' == '\\v'"),
        JsString::from("\"\\v\"==\"\\v\""),
    );
    t.assert_print(
        JsString::from("'\\x0B' == '\\v'"),
        JsString::from("\"\\v\"==\"\\v\""),
    );
}
// port: CodePrinterTest#testIssue620
#[test]
fn test_issue620() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("alert(/ / / / /);"),
        JsString::from("alert(/ // / /)"),
    );
    t.assert_print(
        JsString::from("alert(/ // / /);"),
        JsString::from("alert(/ // / /)"),
    );
}
// port: CodePrinterTest#testIssue5746867
#[test]
fn test_issue5746867() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var a = { '$\\\\' : 5 };"),
        JsString::from("var a={\"$\\\\\":5}"),
    );
}
// port: CodePrinterTest#testCommaSpacing
#[test]
fn test_comma_spacing() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("var a = (b = 5, c = 5);"),
        JsString::from("var a=(b=5,c=5)"),
    );
    t.assert_pretty_print(
        JsString::from("var a = (b = 5, c = 5);"),
        JsString::from("var a = (b = 5, c = 5);\n"),
    );
}
// port: CodePrinterTest#testMinusNegativeZero
#[test]
fn test_minus_negative_zero() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("x- -0"), JsString::from("x- -0"));
}
// port: CodePrinterTest#testStringEscapeSequences
#[test]
fn test_string_escape_sequences() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("var x=\"\\b\""));
    t.assert_print_same(JsString::from("var x=\"\\f\""));
    t.assert_print_same(JsString::from("var x=\"\\n\""));
    t.assert_print_same(JsString::from("var x=\"\\r\""));
    t.assert_print_same(JsString::from("var x=\"\\t\""));
    t.assert_print_same(JsString::from("var x=\"\\v\""));
    t.assert_print(
        JsString::from("var x=\"\\\"\""),
        JsString::from("var x='\"'"),
    );
    t.assert_print(
        JsString::from("var x=\"\\'\""),
        JsString::from("var x=\"'\""),
    );
    t.assert_print(
        JsString::from("var x=\"\\u000A\""),
        JsString::from("var x=\"\\n\""),
    );
    t.assert_print(
        JsString::from("var x=\"\\u000D\""),
        JsString::from("var x=\"\\r\""),
    );
    t.assert_print_same(JsString::from("var x=\"\\u2028\""));
    t.assert_print_same(JsString::from("var x=\"\\u2029\""));
    t.assert_print_same(JsString::from("var x=/\\b/"));
    t.assert_print_same(JsString::from("var x=/\\f/"));
    t.assert_print_same(JsString::from("var x=/\\n/"));
    t.assert_print_same(JsString::from("var x=/\\r/"));
    t.assert_print_same(JsString::from("var x=/\\t/"));
    t.assert_print_same(JsString::from("var x=/\\v/"));
    t.assert_print_same(JsString::from("var x=/\\u000A/"));
    t.assert_print_same(JsString::from("var x=/\\u000D/"));
    t.assert_print_same(JsString::from("var x=/\\u2028/"));
    t.assert_print_same(JsString::from("var x=/\\u2029/"));
}
// port: CodePrinterTest#testRegexp_escape
#[test]
fn test_regexp_escape() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("/\\bword\\b/"));
    t.assert_print_same(JsString::from("/Java\\BScript/"));
    t.assert_print_same(JsString::from("/\\ca/"));
    t.assert_print_same(JsString::from("/\\cb/"));
    t.assert_print_same(JsString::from("/\\cc/"));
    t.assert_print_same(JsString::from("/\\cA/"));
    t.assert_print_same(JsString::from("/\\cB/"));
    t.assert_print_same(JsString::from("/\\cC/"));
    t.assert_print_same(JsString::from("/\\d/"));
    t.assert_print_same(JsString::from("/\\D/"));
    t.assert_print_same(JsString::from("/\\0/"));
    t.assert_print_same(JsString::from("/\\\\/"));
    t.assert_print_same(JsString::from("/(.)\\1/"));
    t.assert_print_same(JsString::from("/\\k<group>/"));
    t.assert_print_same(JsString::from("/(?<group>a)\\k<group>/"));
    t.assert_print_same(JsString::from("/\\x0B/"));
}
// port: CodePrinterTest#testRegexp_unnecessaryEscape
#[test]
fn test_regexp_unnecessary_escape() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("/\\a/"), JsString::from("/a/"));
    t.assert_print(JsString::from("/\\e/"), JsString::from("/e/"));
    t.assert_print(JsString::from("/\\g/"), JsString::from("/g/"));
    t.assert_print(JsString::from("/\\h/"), JsString::from("/h/"));
    t.assert_print(JsString::from("/\\i/"), JsString::from("/i/"));
    t.assert_print(JsString::from("/\\¡/"), JsString::from("/\\u00a1/"));
}
// port: CodePrinterTest#testRegexp_afterLt
#[test]
fn test_regexp_after_lt() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("x < /script/"),
        JsString::from("x< /script/"),
    );
    t.assert_print(
        JsString::from("x << /script/"),
        JsString::from("x<< /script/"),
    );
}
// port: CodePrinterTest#testKeywordProperties1
#[test]
fn test_keyword_properties1() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT5;
    t.assert_print_same(JsString::from("x.foo=2"));
    t.assert_print_same(JsString::from("x.function=2"));
    t.language_mode = LanguageMode::ECMASCRIPT3;
    t.assert_print_same(JsString::from("x.foo=2"));
}
// port: CodePrinterTest#testKeywordProperties2
#[test]
fn test_keyword_properties2() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT5;
    t.assert_print_same(JsString::from("x={foo:2}"));
    t.assert_print_same(JsString::from("x={function:2}"));
    t.language_mode = LanguageMode::ECMASCRIPT3;
    t.assert_print_same(JsString::from("x={foo:2}"));
}
// port: CodePrinterTest#testIssue1062
#[test]
fn test_issue1062() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("3*(4%3*5)"));
}
// port: CodePrinterTest#testPreserveTypeAnnotations
#[test]
fn test_preserve_type_annotations() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_type_annotations = true;
    t.assert_print_same(JsString::from("/** @type {foo} */ var bar"));
    t.assert_print_same(JsString::from(
        "function/** void */ f(/** string */ s,/** number */ n){}",
    ));
    t.preserve_type_annotations = false;
    t.assert_print(
        JsString::from("/** @type {foo} */ var bar;"),
        JsString::from("var bar"),
    );
}
// port: CodePrinterTest#testPreserveTypeAnnotations2
#[test]
fn test_preserve_type_annotations2() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_type_annotations = true;
    t.assert_print_same(JsString::from("/** @const */ var ns={}"));
    t.assert_print_same(JsString::from(
        "/**\n * @const\n * @suppress {const,duplicate}\n */\nvar ns={}\n",
    ));
}
// port: CodePrinterTest#testDefaultParameters
#[test]
fn test_default_parameters() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("function f(a=0){}"));
    t.assert_print_same(JsString::from("function f(a,b=0){}"));
    t.assert_print_same(JsString::from("function f(a=(1,2),b){}"));
    t.assert_print_same(JsString::from("function f(a,b=(1,2)){}"));
}
// port: CodePrinterTest#testRestParameters
#[test]
fn test_rest_parameters() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("function f(...args){}"));
    t.assert_print_same(JsString::from("function f(first,...rest){}"));
}
// port: CodePrinterTest#testDefaultParametersWithRestParameters
#[test]
fn test_default_parameters_with_rest_parameters() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("function f(first=0,...args){}"));
    t.assert_print_same(JsString::from("function f(first,second=0,...rest){}"));
}
// port: CodePrinterTest#testSpreadExpression
#[test]
fn test_spread_expression() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("f(...args)"));
    t.assert_print_same(JsString::from("f(...arrayOfArrays[0])"));
    t.assert_print_same(JsString::from("f(...[1,2,3])"));
    t.assert_print_same(JsString::from("f(...([1],[2]))"));
}
// port: CodePrinterTest#testClass
#[test]
fn test_class() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("class C{}"));
    t.assert_print_same(JsString::from("(class C{})"));
    t.assert_print_same(JsString::from("class C extends D{}"));
    t.assert_print_same(JsString::from("class C{static member(){}}"));
    t.assert_print_same(JsString::from("class C{member(){}get f(){}}"));
    t.assert_print_same(JsString::from("var x=class C{}"));
}
// port: CodePrinterTest#testClassComputedProperties
#[test]
fn test_class_computed_properties() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("class C{[x](){}}"));
    t.assert_print_same(JsString::from("class C{get [x](){}}"));
    t.assert_print_same(JsString::from("class C{set [x](val){}}"));
    t.assert_print_same(JsString::from("class C{static [x](){}}"));
    t.assert_print_same(JsString::from("class C{static get [x](){}}"));
    t.assert_print_same(JsString::from("class C{static set [x](val){}}"));
}
// port: CodePrinterTest#testClassPretty
#[test]
fn test_class_pretty() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("class C{}"),
        JsString::from("class C {\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("class C{member(){}get f(){}}"),
        JsString::from("class C {\n  member() {\n  }\n  get f() {\n  }\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("var x=class C{}"),
        JsString::from("var x = class C {\n};\n"),
    );
}
// port: CodePrinterTest#testClassField
#[test]
fn test_class_field() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("class C {\n  x;\n}\n"));
    t.assert_pretty_print_same(JsString::from("class C {\n  x = 2;\n}\n"));
    t.assert_pretty_print_same(JsString::from("class C {\n  x = 2;\n  y = 3;\n}\n"));
    t.assert_pretty_print(
        JsString::from("class C {\n  x = 2\n  y = 3\n}\n"),
        JsString::from("class C {\n  x = 2;\n  y = 3;\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("class C {x=2;y=3}"),
        JsString::from("class C {\n  x = 2;\n  y = 3;\n}\n"),
    );
}
// port: CodePrinterTest#testClassFieldCheckState
#[test]
fn test_class_field_check_state() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("/** @interface */ \nclass C {\n  x;\n}\n"));
    t.assert_pretty_print_same(JsString::from("/** @record */ \nclass C {\n  x;\n}\n"));
}
// port: CodePrinterTest#testClassFieldStatic
#[test]
fn test_class_field_static() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("class C {\n  static x;\n}\n"));
    t.assert_pretty_print_same(JsString::from("class C {\n  static x = 2;\n}\n"));
    t.assert_pretty_print_same(JsString::from(
        "class C {\n  static x = 2;\n  static y = 3;\n}\n",
    ));
    t.assert_pretty_print_same(JsString::from(
        "/** @interface */ \nclass C {\n  static x;\n}\n",
    ));
    t.assert_pretty_print_same(JsString::from(
        "/** @record */ \nclass C {\n  static x;\n}\n",
    ));
}
// port: CodePrinterTest#testComputedClassFieldLiteralStringNumber
#[test]
fn test_computed_class_field_literal_string_number() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("class C { 'str' = 2;}"),
        JsString::from("class C {\n  [\"str\"] = 2;\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("class C { 1 = 2;}"),
        JsString::from("class C {\n  [1] = 2;\n}\n"),
    );
}
// port: CodePrinterTest#testComputedClassField
#[test]
fn test_computed_class_field() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("class C {\n  [x];\n}\n"));
    t.assert_pretty_print_same(JsString::from("class C {\n  [x] = 2;\n}\n"));
    t.assert_pretty_print_same(JsString::from("class C {\n  [x] = 2;\n  y = 3;\n}\n"));
    t.assert_pretty_print_same(JsString::from("class C {\n  [x] = 2;\n  [y] = 3;\n}\n"));
}
// port: CodePrinterTest#testComputedClassFieldStatic
#[test]
fn test_computed_class_field_static() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("class C {\n  static [x];\n}\n"));
    t.assert_pretty_print_same(JsString::from("class C {\n  static [x] = 2;\n}\n"));
    t.assert_pretty_print_same(JsString::from(
        "class C {\n  static [x] = 2;\n  static y = 3;\n}\n",
    ));
    t.assert_pretty_print_same(JsString::from(
        "class C {\n  static [x] = 2;\n  static [y] = 3;\n}\n",
    ));
}
// port: CodePrinterTest#testPrivateClassProperties_definition
#[test]
fn test_private_class_properties_definition() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("class C{#f;}"));
    t.assert_print_same(JsString::from("class C{#m(){}}"));
    t.assert_print_same(JsString::from("class C{*#g(){}}"));
    t.assert_print_same(JsString::from("class C{get #g(){}}"));
    t.assert_print_same(JsString::from("class C{set #s(x){}}"));
    t.assert_print_same(JsString::from("class C{get #p(){}set #p(x){}}"));
    t.assert_print_same(JsString::from("class C{async #a(){}}"));
    t.assert_print_same(JsString::from("class C{async *#ag(){}}"));
    t.assert_print_same(JsString::from("class C{static #sf;}"));
    t.assert_print_same(JsString::from("class C{static #sm(){}}"));
    t.assert_print_same(JsString::from("class C{static *#sg(){}}"));
    t.assert_print_same(JsString::from("class C{static get #sg(){}}"));
    t.assert_print_same(JsString::from("class C{static set #ss(x){}}"));
    t.assert_print_same(JsString::from(
        "class C{static get #sp(){}static set #sp(x){}}",
    ));
    t.assert_print_same(JsString::from("class C{static async #sa(){}}"));
    t.assert_print_same(JsString::from("class C{static async *#sag(){}}"));
}
// port: CodePrinterTest#testPrivateClassProperties_usage
#[test]
fn test_private_class_properties_usage() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("class C{#f;#g=this.#f;}"));
    t.assert_print_same(JsString::from("class C{#f;#g=this?.#f;}"));
    t.assert_print_same(JsString::from("class C{#m(){this.#m()}}"));
    t.assert_print_same(JsString::from("class C{#m(){this?.#m()}}"));
    t.assert_print_same(JsString::from(
        "class C{static #f;static #m(){const t=this;t.#f}}",
    ));
    t.assert_print_same(JsString::from(
        "class C{static #f;static #m(){const t=this;t?.#f}}",
    ));
    t.assert_print_same(JsString::from(
        "class C{static #f;static #m(){const t=this;#f in t}}",
    ));
}
// port: CodePrinterTest#testSuper
#[test]
fn test_super() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("class C extends foo(){}"));
    t.assert_print_same(JsString::from("class C extends m.foo(){}"));
    t.assert_print_same(JsString::from("class C extends D{member(){super.foo()}}"));
}
// port: CodePrinterTest#testNewTarget
#[test]
fn test_new_target() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("function f(){new.target}"));
    t.assert_print(
        JsString::from("function f() {\nnew\n.\ntarget;\n}"),
        JsString::from("function f(){new.target}"),
    );
}
// port: CodePrinterTest#testImportMeta
#[test]
fn test_import_meta() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("import.meta"));
    t.assert_print_same(JsString::from("import.meta.url"));
    t.assert_print_same(JsString::from("console.log(import.meta.url)"));
}
// port: CodePrinterTest#testGeneratorYield
#[test]
fn test_generator_yield() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("function*f(){yield 1}"));
    t.assert_print_same(JsString::from("function*f(){yield}"));
    t.assert_print_same(JsString::from("function*f(){yield 1?0:2}"));
    t.assert_print_same(JsString::from("function*f(){yield 1,0}"));
    t.assert_print_same(JsString::from("function*f(){1,yield 0}"));
    t.assert_print_same(JsString::from("function*f(){yield(a=0)}"));
    t.assert_print_same(JsString::from("function*f(){a=yield 0}"));
    t.assert_print_same(JsString::from("function*f(){(yield 1)+(yield 1)}"));
    t.assert_print_same(JsString::from("function*f(){yield(()=>({}))}"));
}
// port: CodePrinterTest#testGeneratorYieldPretty
#[test]
fn test_generator_yield_pretty() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("function *f() {yield 1}"),
        JsString::from("function* f() {\n  yield 1;\n}\n"),
    );
    t.assert_pretty_print(
        JsString::from("function *f() {yield}"),
        JsString::from("function* f() {\n  yield;\n}\n"),
    );
}
// port: CodePrinterTest#testMemberGeneratorYield1
#[test]
fn test_member_generator_yield1() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("class C{*member(){(yield 1)+(yield 1)}}"));
    t.assert_print_same(JsString::from("class C{*[0](){(yield 1)+(yield 1)}}"));
    t.assert_print_same(JsString::from("var obj={*member(){(yield 1)+(yield 1)}}"));
    t.assert_print_same(JsString::from("var obj={*[0](){(yield 1)+(yield 1)}}"));
    t.assert_print_same(JsString::from(
        "var obj={[0]:function*(){(yield 1)+(yield 1)}}",
    ));
}
// port: CodePrinterTest#testArrowFunction_zeroParams
#[test]
fn test_arrow_function_zero_params() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("()=>1"));
    t.assert_print(JsString::from("(()=>1)"), JsString::from("()=>1"));
    t.assert_print_same(JsString::from("()=>{}"));
    t.assert_print(JsString::from("(()=>a),b"), JsString::from("()=>a,b"));
    t.assert_print(JsString::from("()=>(a=b)"), JsString::from("()=>a=b"));
}
// port: CodePrinterTest#testArrowFunction_oneParam
#[test]
fn test_arrow_function_one_param() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("a=>b"));
    t.assert_print_same(JsString::from("([a])=>b"));
    t.assert_print_same(JsString::from("(...a)=>b"));
    t.assert_print_same(JsString::from("(a=0)=>b"));
    t.assert_print_same(JsString::from("(a=>b)(1)"));
    t.assert_print_same(JsString::from("fn?.(a=>a)"));
    t.assert_print(
        JsString::from("(a=>a)?.['length']"),
        JsString::from("(a=>a)?.[\"length\"]"),
    );
    t.assert_print_same(JsString::from("(a=>a)?.(1)"));
    t.assert_print_same(JsString::from("(a=>a)?.length"));
    t.assert_print_same(JsString::from("a=>a?.length"));
    t.assert_print_same(JsString::from("var z={x:a=>1}"));
    t.assert_print_same(JsString::from("[1,2].forEach(x=>y)"));
}
// port: CodePrinterTest#testArrowFunction_manyParams
#[test]
fn test_arrow_function_many_params() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("(a,b)=>b"));
}
// port: CodePrinterTest#testArrowFunction_bodyEdgeCases
#[test]
fn test_arrow_function_body_edge_cases() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("()=>(a,b)"));
    t.assert_print_same(JsString::from("()=>({a:1})"));
    t.assert_print_same(JsString::from("()=>{return 1}"));
}
// port: CodePrinterTest#testAsyncFunction
#[test]
fn test_async_function() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("async function f(){}"));
    t.assert_print_same(JsString::from("let f=async function f(){}"));
    t.assert_print_same(JsString::from("let f=async function(){}"));
    t.assert_print(
        JsString::from("async\nfunction f(){}"),
        JsString::from("async;function f(){}"),
    );
    t.assert_print(
        JsString::from("let f=async\nfunction f(){}"),
        JsString::from("let f=async;function f(){}"),
    );
}
// port: CodePrinterTest#testAsyncGeneratorFunction
#[test]
fn test_async_generator_function() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("async function*f(){}"));
    t.assert_print_same(JsString::from("let f=async function*f(){}"));
    t.assert_print_same(JsString::from("let f=async function*(){}"));
    t.assert_print(
        JsString::from("async\nfunction*f(){}"),
        JsString::from("async;function*f(){}"),
    );
    t.assert_print(
        JsString::from("let f=async\nfunction*f(){}"),
        JsString::from("let f=async;function*f(){}"),
    );
}
// port: CodePrinterTest#testAsyncArrowFunction
#[test]
fn test_async_arrow_function() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("async()=>1"));
    t.assert_print(
        JsString::from("async (a) => 1"),
        JsString::from("async a=>1"),
    );
    t.assert_print(
        JsString::from("f=async\n()=>1"),
        JsString::from("f=async;()=>1"),
    );
}
// port: CodePrinterTest#testAsyncMethod
#[test]
fn test_async_method() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("o={async m(){}}"));
    t.assert_print_same(JsString::from("o={async[a+b](){}}"));
    t.assert_print_same(JsString::from("o={[0]:async function(){}}"));
    t.assert_print_same(JsString::from("class C{async m(){}}"));
    t.assert_print_same(JsString::from("class C{async[a+b](){}}"));
    t.assert_print_same(JsString::from("class C{static async m(){}}"));
    t.assert_print_same(JsString::from("class C{static async[a+b](){}}"));
}
// port: CodePrinterTest#testAsyncGeneratorMethod
#[test]
fn test_async_generator_method() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("o={async *m(){}}"));
    t.assert_print_same(JsString::from("o={async*[a+b](){}}"));
    t.assert_print_same(JsString::from("o={[0]:async*function(){}}"));
    t.assert_print_same(JsString::from("class C{async *m(){}}"));
    t.assert_print_same(JsString::from("class C{async*[a+b](){}}"));
    t.assert_print_same(JsString::from("class C{static async *m(){}}"));
    t.assert_print_same(JsString::from("class C{static async*[a+b](){}}"));
}
// port: CodePrinterTest#testAwaitExpression
#[test]
fn test_await_expression() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from(
        "async function f(promise){return await promise}",
    ));
    t.assert_print_same(JsString::from(
        "pwait=async function(promise){return await promise}",
    ));
    t.assert_print_same(JsString::from(
        "class C{async pwait(promise){await promise}}",
    ));
    t.assert_print_same(JsString::from("o={async pwait(promise){await promise}}"));
    t.assert_print_same(JsString::from("pwait=async promise=>await promise"));
}
// port: CodePrinterTest#testParensAroundArrow
#[test]
fn test_parens_around_arrow() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("x||((_)=>true)"),
        JsString::from("x||(_=>true)"),
    );
    t.assert_print(
        JsString::from("void((e)=>e*5)"),
        JsString::from("void(e=>e*5)"),
    );
    t.assert_print(
        JsString::from("((_) => true), ((_) => false)"),
        JsString::from("_=>true,_=>false"),
    );
    t.assert_print(
        JsString::from("x = ((_) => _ + 1)"),
        JsString::from("x=_=>_+1"),
    );
    t.assert_print(
        JsString::from("((_)=>\"\")`template`"),
        JsString::from("(_=>\"\")`template`"),
    );
    t.assert_print_same(JsString::from("((a,b,c)=>a+b+c).length"));
    t.assert_print_same(JsString::from("((a,b,c)=>a+b+c)[\"length\"]"));
    t.assert_print(JsString::from("x[((_)=>0)]"), JsString::from("x[_=>0]"));
    t.assert_print(
        JsString::from("((x)=>x*5)(10)"),
        JsString::from("(x=>x*5)(10)"),
    );
    t.assert_print(
        JsString::from("x(((_) => true), ((_) => false))"),
        JsString::from("x(_=>true,_=>false)"),
    );
    t.assert_print(JsString::from("((x)=>1)?a:b"), JsString::from("(x=>1)?a:b"));
    t.assert_print(
        JsString::from("x?((x)=>0):((x)=>1)"),
        JsString::from("x?x=>0:x=>1"),
    );
    t.assert_print(JsString::from("new ((x)=>x)"), JsString::from("new (x=>x)"));
    t.assert_print_same(JsString::from("new C(x=>x)"));
}
// port: CodePrinterTest#testParensAroundArrowFnInCast
#[test]
fn test_parens_around_arrow_fn_in_cast() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_type_annotations = false;
    t.assert_print(
        JsString::from("x(/** @type {?} */ (()=>{x}))"),
        JsString::from("x(()=>{x})"),
    );
    t.assert_print(
        JsString::from("x(/** @type {?} */ (()=>{x})())"),
        JsString::from("x((()=>{x})())"),
    );
    t.assert_print(
        JsString::from("x(/** @type {string} */ (/** @type {?} */ (()=>{x}))())"),
        JsString::from("x((()=>{x})())"),
    );
    t.preserve_type_annotations = true;
    t.assert_print_same(JsString::from("x(/** @type {?} */ (()=>{x}))"));
    t.assert_print_same(JsString::from("x(/** @type {?} */ (()=>{x})())"));
    t.assert_print_same(JsString::from(
        "x(/** @type {string} */ (/** @type {?} */ (()=>{x}))())",
    ));
}
// port: CodePrinterTest#testParensAroundVariableDeclarator
#[test]
fn test_parens_around_variable_declarator() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("var o=(test++,{one:1})"));
    t.assert_print_same(JsString::from("({one}=(test++,{one:1}))"));
    t.assert_print_same(JsString::from("[one]=(test++,[1])"));
    t.assert_print_same(JsString::from("var {one}=(test++,{one:1})"));
    t.assert_print_same(JsString::from("var [one]=(test++,[1])"));
    t.assert_print(
        JsString::from("var {one}=/** @type {{one: number}} */(test++,{one:1})"),
        JsString::from("var {one}=(test++,{one:1})"),
    );
    t.assert_print(
        JsString::from("var [one]=/** @type {!Array<number>} */(test++,[1])"),
        JsString::from("var [one]=(test++,[1])"),
    );
}
// port: CodePrinterTest#testParensAroundArrowReturnValue
#[test]
fn test_parens_around_arrow_return_value() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("()=>({})"));
    t.assert_print_same(JsString::from("()=>({a:1})"));
    t.assert_print_same(JsString::from("()=>({a:1,b:2})"));
    t.assert_print(
        JsString::from("()=>/** @type {Object} */({})"),
        JsString::from("()=>({})"),
    );
    t.assert_print(
        JsString::from("()=>/** @type {Object} */({a:1})"),
        JsString::from("()=>({a:1})"),
    );
    t.assert_print(
        JsString::from("()=>/** @type {Object} */({a:1,b:2})"),
        JsString::from("()=>({a:1,b:2})"),
    );
    t.assert_print(
        JsString::from("()=>/** @type {number} */(3)"),
        JsString::from("()=>3"),
    );
    t.assert_print(
        JsString::from("()=>/** @type {Object} */ ({}={})"),
        JsString::from("()=>({}={})"),
    );
    t.assert_print_same(JsString::from("()=>(1,2)"));
    t.assert_print_same(JsString::from("()=>({},2)"));
    t.assert_print_same(JsString::from("()=>(1,{})"));
    t.assert_print(
        JsString::from("()=>/** @type {?} */(1,2)"),
        JsString::from("()=>(1,2)"),
    );
    t.assert_print(
        JsString::from("()=>/** @type {?} */({},2)"),
        JsString::from("()=>({},2)"),
    );
    t.assert_print(
        JsString::from("()=>/** @type {?} */(1,{})"),
        JsString::from("()=>(1,{})"),
    );
    t.assert_print_same(JsString::from("fn=()=>({})||3"));
    t.assert_print_same(JsString::from("fn=()=>3||{}"));
    t.assert_print_same(JsString::from("fn=()=>({}={})"));
    t.assert_print_same(JsString::from("()=>function(){}"));
    t.assert_print_same(JsString::from("for(var i=()=>({});;);"));
    t.preserve_type_annotations = true;
    t.assert_print_same(JsString::from("()=>/** @type {Object} */ ({})"));
}
// port: CodePrinterTest#testPrettyArrowFunction
#[test]
fn test_pretty_arrow_function() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(
        JsString::from("if (x) {var f = ()=>{alert(1); alert(2)}}"),
        JsString::from("if (x) {\n  var f = () => {\n    alert(1);\n    alert(2);\n  };\n}\n"),
    );
}
// port: CodePrinterTest#testPrettyPrint_switch
#[test]
fn test_pretty_print_switch() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print(JsString::from("switch(something){case 0:alert(0);break;case 1:alert(1);break}"), JsString::from("switch(something) {\n  case 0:\n    alert(0);\n    break;\n  case 1:\n    alert(1);\n    break;\n}\n"));
}
// port: CodePrinterTest#testBlocksInCaseArePreserved
#[test]
fn test_blocks_in_case_are_preserved() {
    let mut t = CodePrinterTestBase::new();
    let js = JsString::from(
        "switch(something) {\n  case 0:\n    {\n      const x = 1;\n      break;\n    }\n  case 1:\n    break;\n  case 2:\n    console.log(`case 2!`);\n    {\n      const x = 2;\n      break;\n    }\n}\n",
    );
    t.assert_pretty_print(js.clone(), js.clone());
}
// port: CodePrinterTest#testBlocksArePreserved
#[test]
fn test_blocks_are_preserved() {
    let mut t = CodePrinterTestBase::new();
    let js =
        JsString::from("console.log(0);\n{\n  let x = 1;\n  console.log(x);\n}\nconsole.log(x);\n");
    t.assert_pretty_print(js.clone(), js.clone());
}
// port: CodePrinterTest#testBlocksNotPreserved
#[test]
fn test_blocks_not_preserved() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(JsString::from("if (x) {};"), JsString::from("if(x);"));
    t.assert_print(JsString::from("while (x) {};"), JsString::from("while(x);"));
}
// port: CodePrinterTest#testEmptyClassStaticBlock
#[test]
fn test_empty_class_static_block() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("class C {\n  static {\n  }\n}\n"));
    t.assert_print(
        JsString::from("class C {\n  static {\n  }\n}\n"),
        JsString::from("class C{static{}}"),
    );
    t.assert_pretty_print_same(JsString::from("let a = class {\n  static {\n  }\n};\n"));
}
// port: CodePrinterTest#testClassStaticBlock
#[test]
fn test_class_static_block() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("class C {\n  static field1 = 1;\n  static field2 = 2;\n  static {\n    let x = this.field1;\n    let y = this.field2;\n  }\n}\n"));
    t.assert_pretty_print_same(JsString::from(
        "class C {\n  static {\n    this.field1 = 1;\n    this.field2 = 2;\n  }\n}\n",
    ));
    t.assert_pretty_print_same(JsString::from("let a = class {\n  static field1 = 1;\n  static field2 = 2;\n  static {\n    let x = this.field1;\n    let y = this.field2;\n  }\n};\n"));
    t.assert_pretty_print_same(JsString::from(
        "let a = class {\n  static {\n    this.field1 = 1;\n    this.field2 = 2;\n  }\n};\n",
    ));
}
// port: CodePrinterTest#testMultipleClassStaticBlocks
#[test]
fn test_multiple_class_static_blocks() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from(
        "class C {\n  static {\n  }\n  static {\n  }\n}\n",
    ));
    t.assert_pretty_print_same(JsString::from(
        "let a = class {\n  static {\n  }\n  static {\n  }\n};\n",
    ));
    t.assert_pretty_print_same(JsString::from("class C {\n  static field1 = 1;\n  static field2 = 2;\n  static {\n    let x = this.field1;\n  }\n  static {\n    let y = this.field2;\n  }\n}\n"));
    t.assert_pretty_print_same(JsString::from("class C {\n  static {\n    this.field1 = 1;\n  }\n  static {\n    this.field2 = 2;\n  }\n}\n"));
    t.assert_pretty_print_same(JsString::from("let a = class {\n  static field1 = 1;\n  static field2 = 2;\n  static {\n    let x = this.field1;\n  }\n  static {\n    let y = this.field2;\n  }\n};\n"));
    t.assert_pretty_print_same(JsString::from("let a = class {\n  static {\n    this.field1 = 1;\n  }\n  static {\n    this.field2 = 2;\n  }\n};\n"));
}
// port: CodePrinterTest#testDeclarations
#[test]
fn test_declarations() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from("let x"));
    t.assert_print_same(JsString::from("let x,y"));
    t.assert_print_same(JsString::from("let x=1"));
    t.assert_print_same(JsString::from("let x=1,y=2"));
    t.assert_print_same(JsString::from("if(a){let x}"));
    t.assert_print_same(JsString::from("const x=1"));
    t.assert_print_same(JsString::from("const x=1,y=2"));
    t.assert_print_same(JsString::from("if(a){const x=1}"));
    t.assert_print_same(JsString::from("function f(){}"));
    t.assert_print_same(JsString::from("if(a){function f(){}}"));
    t.assert_print_same(JsString::from("if(a)(function(){})"));
    t.assert_print_same(JsString::from("class f{}"));
    t.assert_print_same(JsString::from("if(a){class f{}}"));
    t.assert_print_same(JsString::from("if(a)(class{})"));
    t.assert_print_same(JsString::from("class P extends(_Component=Component){}"));
}
// port: CodePrinterTest#testExportAsyncFunction
#[test]
fn test_export_async_function() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_2017;
    t.assert_print_same(JsString::from("export async function f(){}"));
}
// port: CodePrinterTest#testTemplateLiteral
#[test]
fn test_template_literal() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("`hello`"));
    t.assert_print_same(JsString::from("`\\\\bhello`"));
    t.assert_print_same(JsString::from("`hel\rlo`"));
    t.assert_print_same(JsString::from("`hel\\rlo`"));
    t.assert_print_same(JsString::from("`hel\r\nlo`"));
    t.assert_print_same(JsString::from("`hel\\r\\nlo`"));
    t.assert_print(
        JsString::from("`hello`\n'world'"),
        JsString::from("`hello`;\"world\""),
    );
    t.assert_print(
        JsString::from("`hello`\n`world`"),
        JsString::from("`hello``world`"),
    );
    t.assert_print(
        JsString::from("var x=`TestA`\n`TemplateB`"),
        JsString::from("var x=`TestA``TemplateB`"),
    );
    t.assert_print_same(JsString::from("`hello``world`"));
    t.assert_print_same(JsString::from("`hello${world}!`"));
    t.assert_print_same(JsString::from("`hello${world} ${name}!`"));
    t.assert_print_same(JsString::from("`hello${(function(){let x=3})()}`"));
    t.assert_print_same(JsString::from("(function(){})()`${(function(){})()}`"));
    t.assert_print_same(JsString::from("url`hello`"));
    t.assert_print_same(JsString::from("url(`hello`)"));
    t.assert_print_same(JsString::from("`\\u{2026}`"));
    t.assert_print_same(JsString::from("`start\\u{2026}end`"));
    t.assert_print_same(JsString::from("`\\u{1f42a}`"));
    t.assert_print_same(JsString::from("`start\\u{1f42a}end`"));
    t.assert_print_same(JsString::from("`\\u2026`"));
    t.assert_print_same(JsString::from("`start\\u2026end`"));
    t.assert_print_same(JsString::from("`\"`"));
    t.assert_print_same(JsString::from("`'`"));
    t.assert_print_same(JsString::from("`\\\"`"));
    t.assert_print_same(JsString::from("`\\'`"));
    t.assert_print_same(JsString::from("`\\``"));
    t.assert_print_same(JsString::from("foo`\\unicode`"));
    t.assert_print_same(JsString::from("String.raw`a\\ b`"));
    t.assert_print_same(JsString::from("`Hello ${x?`Alice`:`Bob`}?`"));
    t.assert_print_same(JsString::from("`Hello ${x?`Alice ${y(`Kitten`)}`:`Bob`}?`"));
    t.assert_print_same(JsString::from("`Unbroken${x}string`"));
    t.assert_print_same(JsString::from("let a;`a`"));
}
// port: CodePrinterTest#testMultiLineTemplateLiteral_preservesInteralNewAndBlankLines
#[test]
fn test_multi_line_template_literal_preserves_interal_new_and_blank_lines() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print_same(JsString::from("var y=`hello // Line break (0 blank lines).\nworld\n // Single blank line.\nfoo\n // Multiple blank lines.\n\n\nbar`\n"));
    t.assert_pretty_print_same(JsString::from("var y = `hello // Line break (0 blank lines).\nworld\n // Single blank line.\nfoo\n // Multiple blank lines.\n\n\nbar`;\n"));
}
// port: CodePrinterTest#testMultiLineTemplateLiteral_doesNotPreserveNewLines_inSubstituions
#[test]
fn test_multi_line_template_literal_does_not_preserve_new_lines_in_substituions() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_print(
        JsString::from("var y=`Hello ${x\n+\nz\n}`\n"),
        JsString::from("var y=`Hello ${x+z}`"),
    );
    t.assert_pretty_print(
        JsString::from("var y=`Hello ${x\n+\nz\n}`\n"),
        JsString::from("var y = `Hello ${x + z}`;\n"),
    );
}
// port: CodePrinterTest#testMultiLineTemplateLiteral_notIndented_byPrettyPrint
#[test]
fn test_multi_line_template_literal_not_indented_by_pretty_print() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT_NEXT;
    t.assert_pretty_print(JsString::from("function indentScope() {\n  var y =\n// Open backtick.\n`hello\nworld\nfoo\n${ // Open substituion.\nbing\n// Close substitution.\n}\nbar\n`; // Close backtick.\n}\n"), JsString::from("function indentScope() {\n  var y = `hello\nworld\nfoo\n${bing}\nbar\n`;\n}\n"));
}
// port: CodePrinterTest#testEs6NewTargetBare
#[test]
fn test_es6_new_target_bare() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from(
        "class C{constructor(){new.target.prototype}}",
    ));
}
// port: CodePrinterTest#testEs6NewTargetPrototype
#[test]
fn test_es6_new_target_prototype() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_same(JsString::from(
        "class C{constructor(){var callable=Object.setPrototypeOf(obj,new.target.prototype)}}",
    ));
}
// port: CodePrinterTest#testEs6NewTargetConditional
#[test]
fn test_es6_new_target_conditional() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from("function f() {\n  if (!new.target) throw 'Must be called with new!';\n}\n"),
        JsString::from("function f(){if(!new.target)throw\"Must be called with new!\";}"),
    );
}
// port: CodePrinterTest#testPrettyPrinterIfElseIfAddedBlock
#[test]
fn test_pretty_printer_if_else_if_added_block() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from(
        "if (0) {\n  0;\n} else if (1) {\n  if (2) {\n    2;\n  }\n} else if (3) {\n  3;\n}\n",
    ));
    t.assert_pretty_print(
        JsString::from("if(0)if(1)1;else 2;else 3;"),
        JsString::from(
            "if (0) {\n  if (1) {\n    1;\n  } else {\n    2;\n  }\n} else {\n  3;\n}\n",
        ),
    );
}
// port: CodePrinterTest#testNonJSDocCommentsPrinted_getProp
#[test]
fn test_non_js_doc_comments_printed_get_prop() {
    let mut t = CodePrinterTestBase::new();
    t.preserve_non_jsdoc_comments = true;
    t.assert_pretty_print(
        JsString::from("a.// testComment\nb"),
        JsString::from("// testComment\na.b;\n"),
    );
}

use closure_rhino::{ir::IR, node::NodeId, token::Token};
// port: CodePrinterTest#testNoTrailingCommaInEmptyArrayLiteral
#[test]
fn test_no_trailing_comma_in_empty_array_literal() {
    let mut t = CodePrinterTestBase::new();
    let node1 = IR::arraylit(&mut t.last_compiler, &[]);
    let arr_lit = node1;
    arr_lit.set_trailing_comma(&mut t.last_compiler, true);
    t.assert_pretty_print_node(JsString::from("[]"), arr_lit);
}
// port: CodePrinterTest#testNoTrailingCommaInEmptyObjectLiteral
#[test]
fn test_no_trailing_comma_in_empty_object_literal() {
    let mut t = CodePrinterTestBase::new();
    let node1 = IR::objectlit(&mut t.last_compiler, &[]);
    let obj_lit = node1;
    obj_lit.set_trailing_comma(&mut t.last_compiler, true);
    t.assert_pretty_print_node(JsString::from("{}"), obj_lit);
}
// port: CodePrinterTest#testNoTrailingCommaInEmptyParamList
#[test]
fn test_no_trailing_comma_in_empty_param_list() {
    let mut t = CodePrinterTestBase::new();
    let node1 = IR::param_list(&mut t.last_compiler, &[]);
    let param_list = node1;
    let node2 = IR::name(&mut t.last_compiler, JsString::from("f"));
    let node3 = IR::block(&mut t.last_compiler);
    let _node4 = IR::function(&mut t.last_compiler, node2, param_list, node3);
    param_list.set_trailing_comma(&mut t.last_compiler, true);
    t.assert_pretty_print_node(JsString::from("()"), param_list);
}
// port: CodePrinterTest#testNoTrailingCommaInEmptyCall
#[test]
fn test_no_trailing_comma_in_empty_call() {
    let mut t = CodePrinterTestBase::new();
    let node1 = IR::name(&mut t.last_compiler, JsString::from("f"));
    let node2 = IR::call(&mut t.last_compiler, node1, &[]);
    let call = node2;
    call.set_trailing_comma(&mut t.last_compiler, true);
    t.assert_pretty_print_node(JsString::from("f()"), call);
}
// port: CodePrinterTest#testNoTrailingCommaInEmptyOptChainCall
#[test]
fn test_no_trailing_comma_in_empty_opt_chain_call() {
    let mut t = CodePrinterTestBase::new();
    let node1 = IR::name(&mut t.last_compiler, JsString::from("f"));
    let node2 = IR::start_opt_chain_call(&mut t.last_compiler, node1, &[]);
    let opt_chain_call = node2;
    opt_chain_call.set_trailing_comma(&mut t.last_compiler, true);
    t.assert_pretty_print_node(JsString::from("f?.()"), opt_chain_call);
}
// port: CodePrinterTest#testNoTrailingCommaInEmptyNew
#[test]
fn test_no_trailing_comma_in_empty_new() {
    let mut t = CodePrinterTestBase::new();
    let node1 = IR::name(&mut t.last_compiler, JsString::from("f"));
    let node2 = IR::new_node(&mut t.last_compiler, node1, &[]);
    let new_node = node2;
    new_node.set_trailing_comma(&mut t.last_compiler, true);
    t.assert_pretty_print_node(JsString::from("new f()"), new_node);
}
// port: CodePrinterTest#testPrintComma1
#[test]
fn test_print_comma1() {
    let mut t = CodePrinterTestBase::new();
    let node1 = IR::name(&mut t.last_compiler, JsString::from("a"));
    let node2 = IR::name(&mut t.last_compiler, JsString::from("b"));
    let node3 = IR::name(&mut t.last_compiler, JsString::from("c"));
    let node4 = IR::comma(&mut t.last_compiler, node2, node3);
    let node5 = IR::name(&mut t.last_compiler, JsString::from("d"));
    let node6 = IR::comma(&mut t.last_compiler, node4, node5);
    let node7 = IR::var_with_value(&mut t.last_compiler, node1, node6);
    let node = node7;
    t.assert_print_node(JsString::from("var a=(b,c,d)"), node);
}
// port: CodePrinterTest#testPrintComma2
#[test]
fn test_print_comma2() {
    let mut t = CodePrinterTestBase::new();
    let node1 = IR::name(&mut t.last_compiler, JsString::from("a"));
    let node2 = IR::name(&mut t.last_compiler, JsString::from("b"));
    let node3 = IR::name(&mut t.last_compiler, JsString::from("c"));
    let node4 = IR::name(&mut t.last_compiler, JsString::from("d"));
    let node5 = IR::comma(&mut t.last_compiler, node3, node4);
    let node6 = IR::comma(&mut t.last_compiler, node2, node5);
    let node7 = IR::var_with_value(&mut t.last_compiler, node1, node6);
    let node = node7;
    t.assert_print_node(JsString::from("var a=(b,c,d)"), node);
}
// port: CodePrinterTest#testAmbiguousElseClauses
#[test]
fn test_ambiguous_else_clauses() {
    let mut t = CodePrinterTestBase::new();
    let node1 = t
        .last_compiler
        .new_string_with_token(Token::NAME, JsString::from("x"));
    let node2 = t
        .last_compiler
        .new_string_with_token(Token::NAME, JsString::from("y"));
    let node3 = t.last_compiler.new_node(Token::BLOCK);
    let node4 = t.last_compiler.new_node(Token::BLOCK);
    let node5 = t
        .last_compiler
        .new_node_with_children3(Token::IF, node2, node3, node4);
    let node6 = t.last_compiler.new_node_with_child(Token::BLOCK, node5);
    let node7 = t
        .last_compiler
        .new_node_with_children2(Token::IF, node1, node6);
    t.assert_print_node(JsString::from("if(x)if(y);else;"), node7);
    let node8 = t
        .last_compiler
        .new_string_with_token(Token::NAME, JsString::from("x"));
    let node9 = t
        .last_compiler
        .new_string_with_token(Token::NAME, JsString::from("y"));
    let node10 = t.last_compiler.new_node(Token::BLOCK);
    let node11 = t
        .last_compiler
        .new_node_with_children2(Token::IF, node9, node10);
    let node12 = t.last_compiler.new_node_with_child(Token::BLOCK, node11);
    let node13 = t.last_compiler.new_node(Token::BLOCK);
    let node14 = t
        .last_compiler
        .new_node_with_children3(Token::IF, node8, node12, node13);
    t.assert_print_node(JsString::from("if(x){if(y);}else;"), node14);
    let node15 = t
        .last_compiler
        .new_string_with_token(Token::NAME, JsString::from("x"));
    let node16 = t
        .last_compiler
        .new_string_with_token(Token::NAME, JsString::from("y"));
    let node17 = t.last_compiler.new_node(Token::BLOCK);
    let node18 = t
        .last_compiler
        .new_string_with_token(Token::NAME, JsString::from("z"));
    let node19 = t.last_compiler.new_node(Token::BLOCK);
    let node20 = t
        .last_compiler
        .new_node_with_children2(Token::IF, node18, node19);
    let node21 = t.last_compiler.new_node_with_child(Token::BLOCK, node20);
    let node22 = t
        .last_compiler
        .new_node_with_children3(Token::IF, node16, node17, node21);
    let node23 = t.last_compiler.new_node_with_child(Token::BLOCK, node22);
    let node24 = t.last_compiler.new_node(Token::BLOCK);
    let node25 = t
        .last_compiler
        .new_node_with_children3(Token::IF, node15, node23, node24);
    t.assert_print_node(JsString::from("if(x)if(y);else{if(z);}else;"), node25);
}
// port: CodePrinterTest#testPrettyPrinter_spaceBeforeSingleQuote
#[test]
fn test_pretty_printer_space_before_single_quote() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_options(
        JsString::from("var f = function() { return 'hello'; };"),
        JsString::from("var f = function() {\n  return 'hello';\n};\n"),
        |options| options.set_prefer_single_quotes(true),
    );
}
// port: CodePrinterTest#testPrettyPrinter_number
#[test]
fn test_pretty_printer_number() {
    let mut t = CodePrinterTestBase::new();
    t.assert_pretty_print_same(JsString::from("var x = 10;\n"));
    t.assert_pretty_print_same(JsString::from("var x = 1.;\n"));
    t.assert_pretty_print(
        JsString::from("var x = 0xFE;"),
        JsString::from("var x = 254;\n"),
    );
    t.assert_pretty_print_same(JsString::from("var x = 1").concat(&JsString::from("0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000")).concat(&JsString::from(";\n")));
    t.assert_pretty_print_same(JsString::from("f(10000);\n"));
    t.assert_pretty_print(
        JsString::from("var x = -10000;\n"),
        JsString::from("var x = -10000;\n"),
    );
    t.assert_pretty_print(
        JsString::from("var x = y - -10000;\n"),
        JsString::from("var x = y - -10000;\n"),
    );
    t.assert_pretty_print(
        JsString::from("f(-10000);\n"),
        JsString::from("f(-10000);\n"),
    );
    t.assert_pretty_print_same(JsString::from("x < 2592000;\n"));
    t.assert_pretty_print_same(JsString::from("x < 1000.000;\n"));
    t.assert_pretty_print_same(JsString::from("x < 1000.912;\n"));
    t.assert_pretty_print_same(JsString::from("var x = 1E20;\n"));
    t.assert_pretty_print_same(JsString::from("var x = 1E1;\n"));
    t.assert_pretty_print_same(JsString::from("var x = void 0;\n"));
    t.assert_pretty_print_same(JsString::from("foo(-0);\n"));
    t.assert_pretty_print(
        JsString::from("var x = 4-1000;"),
        JsString::from("var x = 4 - 1000;\n"),
    );
}
// port: CodePrinterTest#testSubtraction
#[test]
fn test_subtraction() {
    let mut t = CodePrinterTestBase::new();
    let n = t.parse("x - -4");
    assert!(t.last_compiler.get_errors().is_empty());
    assert_eq!(t.print_node(n), JsString::from("x- -4"));
}
// port: CodePrinterTest#testExponents
#[test]
fn test_exponents() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print_number_int("1", 1);
    t.assert_print_number_int("10", 10);
    t.assert_print_number_int("100", 100);
    t.assert_print_number_int("1E3", 1000);
    t.assert_print_number_int("1E4", 10000);
    t.assert_print_number_int("1E5", 100000);
    t.assert_print_number("1E18", 1000000000000000000.0);
    t.assert_print_number("1E5", 100000.0);
    t.assert_print_number("100000.1", 100000.1);
    t.assert_print_number("1E-6", 0.000001);
    t.assert_print_number("0x38d7ea4c68001", 0x38d7ea4c68001u64 as f64);
    t.assert_print_number("0x7fffffffffffffff", 0x7fffffffffffffffu64 as f64);
    t.assert_print_number(".01", 0.01);
    t.assert_print_number("1.01", 1.01);
}
// port: CodePrinterTest#testIndirectEval
#[test]
fn test_indirect_eval() {
    let mut t = CodePrinterTestBase::new();
    let node1 = t.parse(JsString::from("eval('1');"));
    let n = node1;
    t.assert_print_node(JsString::from("eval(\"1\")"), n);
    n.get_first_first_child(&t.last_compiler)
        .unwrap()
        .get_first_child(&t.last_compiler)
        .unwrap()
        .put_boolean_prop(&mut t.last_compiler, NodeId::DIRECT_EVAL, false);
    t.assert_print_node(JsString::from("(0,eval)(\"1\")"), n);
}
// port: CodePrinterTest#freeCallTaggedTemplate
#[test]
fn free_call_tagged_template() {
    let mut t = CodePrinterTestBase::new();
    let node1 = t.parse(JsString::from("a.b`xyz`"));
    let n = node1;
    let call = n.get_first_first_child(&t.last_compiler).unwrap();
    assert!(call.is_tagged_template_lit(&t.last_compiler));
    call.put_boolean_prop(&mut t.last_compiler, NodeId::FREE_CALL, true);
    t.assert_print_node(JsString::from("(0,a.b)`xyz`"), n);
}
// port: CodePrinterTest#freeCallOptChain
#[test]
fn free_call_opt_chain() {
    let mut t = CodePrinterTestBase::new();
    let node1 = t.parse(JsString::from("(a?.b)()"));
    let n = node1;
    let call = n.get_first_first_child(&t.last_compiler).unwrap();
    assert!(call.is_call(&t.last_compiler));
    call.put_boolean_prop(&mut t.last_compiler, NodeId::FREE_CALL, true);
    t.assert_print_node(JsString::from("(0,a?.b)()"), n);
}
// port: CodePrinterTest#freeCallOptChainOptChainCall
#[test]
fn free_call_opt_chain_opt_chain_call() {
    let mut t = CodePrinterTestBase::new();
    let node1 = t.parse(JsString::from("(a?.b)?.()"));
    let n = node1;
    let call = n.get_first_first_child(&t.last_compiler).unwrap();
    assert!(call.is_opt_chain_call(&t.last_compiler));
    call.put_boolean_prop(&mut t.last_compiler, NodeId::FREE_CALL, true);
    t.assert_print_node(JsString::from("(0,a?.b)?.()"), n);
}
// port: CodePrinterTest#testFreeCall2
#[test]
fn test_free_call2() {
    let mut t = CodePrinterTestBase::new();
    let node1 = t.parse(JsString::from("foo(a);"));
    let n = node1;
    t.assert_print_node(JsString::from("foo(a)"), n);
    let call = n.get_first_first_child(&t.last_compiler).unwrap();
    assert!(call.is_call(&t.last_compiler));
    call.put_boolean_prop(&mut t.last_compiler, NodeId::FREE_CALL, true);
    t.assert_print_node(JsString::from("foo(a)"), n);
}
// port: CodePrinterTest#testFreeCall3
#[test]
fn test_free_call3() {
    let mut t = CodePrinterTestBase::new();
    let node1 = t.parse(JsString::from("x.foo(a);"));
    let n = node1;
    t.assert_print_node(JsString::from("x.foo(a)"), n);
    let call = n.get_first_first_child(&t.last_compiler).unwrap();
    assert!(call.is_call(&t.last_compiler));
    call.put_boolean_prop(&mut t.last_compiler, NodeId::FREE_CALL, true);
    t.assert_print_node(JsString::from("(0,x.foo)(a)"), n);
}
// port: CodePrinterTest#testPrintScript
#[test]
fn test_print_script() {
    let mut t = CodePrinterTestBase::new();
    let node1 = t.last_compiler.new_string(JsString::from("f"));
    let node2 = t
        .last_compiler
        .new_node_with_child(Token::EXPR_RESULT, node1);
    let node3 = t.last_compiler.new_string(JsString::from("g"));
    let node4 = t
        .last_compiler
        .new_node_with_child(Token::EXPR_RESULT, node3);
    let node5 = t
        .last_compiler
        .new_node_with_children2(Token::SCRIPT, node2, node4);
    let ast = node5;
    let result = closure_jscomp::code_printer::Builder::new(ast)
        .set_pretty_print(true)
        .build(&t.last_compiler);
    assert_eq!(result, JsString::from("\"f\";\n\"g\";\n"));
}
// port: CodePrinterTest#testGetterInEs3
#[test]
fn test_getter_in_es3() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT3;
    let node1 = t
        .last_compiler
        .new_string_with_token(Token::GETTER_DEF, JsString::from("f"));
    let getter = node1;
    let node2 = IR::name(&mut t.last_compiler, JsString::from(""));
    let node3 = IR::param_list(&mut t.last_compiler, &[]);
    let node4 = IR::block(&mut t.last_compiler);
    let node5 = IR::function(&mut t.last_compiler, node2, node3, node4);
    getter.add_child_to_back(&mut t.last_compiler, node5);
    let node6 = IR::objectlit(&mut t.last_compiler, &[getter]);
    let node7 = IR::expr_result(&mut t.last_compiler, node6);
    t.assert_print_node(JsString::from("({get f(){}})"), node7);
}
// port: CodePrinterTest#testSetterInEs3
#[test]
fn test_setter_in_es3() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT3;
    let node1 = t
        .last_compiler
        .new_string_with_token(Token::SETTER_DEF, JsString::from("f"));
    let getter = node1;
    let node2 = IR::name(&mut t.last_compiler, JsString::from(""));
    let node3 = IR::name(&mut t.last_compiler, JsString::from("a"));
    let node4 = IR::param_list(&mut t.last_compiler, &[node3]);
    let node5 = IR::block(&mut t.last_compiler);
    let node6 = IR::function(&mut t.last_compiler, node2, node4, node5);
    getter.add_child_to_back(&mut t.last_compiler, node6);
    let node7 = IR::objectlit(&mut t.last_compiler, &[getter]);
    let node8 = IR::expr_result(&mut t.last_compiler, node7);
    t.assert_print_node(JsString::from("({set f(a){}})"), node8);
}
// port: CodePrinterTest#testManyCommas
#[test]
fn test_many_commas() {
    let mut t = CodePrinterTestBase::new();
    let mut numbers = vec!["0".to_owned(), "1".to_owned()];
    let zero = t.last_compiler.new_number(0.0);
    let one = t.last_compiler.new_number(1.0);
    let mut current = t
        .last_compiler
        .new_node_with_children2(Token::COMMA, zero, one);
    for i in 2..10000 {
        current = t.last_compiler.new_node_with_child(Token::COMMA, current);
        let num = i % 1000;
        numbers.push(num.to_string());
        let n = t.last_compiler.new_number(num as f64);
        current.add_child_to_back(&mut t.last_compiler, n);
    }
    assert_eq!(
        t.print_node(current).to_string_lossy().replace("\n", ""),
        numbers.join(",")
    );
}
// port: CodePrinterTest#testManyAdds
#[test]
fn test_many_adds() {
    let mut t = CodePrinterTestBase::new();
    let mut numbers = vec!["0".to_owned(), "1".to_owned()];
    let zero = t.last_compiler.new_number(0.0);
    let one = t.last_compiler.new_number(1.0);
    let mut current = t
        .last_compiler
        .new_node_with_children2(Token::ADD, zero, one);
    for i in 2..10000 {
        current = t.last_compiler.new_node_with_child(Token::ADD, current);
        let num = i % 1000;
        numbers.push(num.to_string());
        let n = t.last_compiler.new_number(num as f64);
        current.add_child_to_back(&mut t.last_compiler, n);
    }
    assert_eq!(
        t.print_node(current).to_string_lossy().replace("\n", ""),
        numbers.join("+")
    );
}
// port: CodePrinterTest#testKeywordProperties1a
#[test]
fn test_keyword_properties1a() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT5;
    let node1 = t.parse(JsString::from("x.function=2"));
    let nodes = node1;
    t.language_mode = LanguageMode::ECMASCRIPT3;
    t.assert_print_node(JsString::from("x[\"function\"]=2"), nodes);
}
// port: CodePrinterTest#testKeywordProperties2a
#[test]
fn test_keyword_properties2a() {
    let mut t = CodePrinterTestBase::new();
    t.language_mode = LanguageMode::ECMASCRIPT5;
    let node1 = t.parse(JsString::from("x={function:2}"));
    let nodes = node1;
    t.language_mode = LanguageMode::ECMASCRIPT3;
    t.assert_print_node(JsString::from("x={\"function\":2}"), nodes);
}
// port: CodePrinterTest#testParansAroudAwaitArrowFunction
#[test]
fn test_parans_aroud_await_arrow_function() {
    let mut t = CodePrinterTestBase::new();
    t.assert_print(
        JsString::from(
            "async function f(){return await (()=>new Promise((resolve)=>setTimeout(resolve,0)));}",
        ),
        JsString::from(
            "async function f(){return await (()=>new Promise(resolve=>setTimeout(resolve,0)))}",
        ),
    );
    t.assert_print(
        JsString::from(
            "async function f(){return await new Promise((resolve)=>setTimeout(resolve,0));}",
        ),
        JsString::from(
            "async function f(){return await new Promise(resolve=>setTimeout(resolve,0))}",
        ),
    );
}
// port: CodePrinterTest#testImports
#[test]
fn test_imports() {
    let mut t = CodePrinterTestBase::new();
    t.diagnostics_to_ignore = vec![closure_jscomp::diagnostic_groups::MODULE_LOAD.clone()];
    t.assert_print_same(JsString::from("import x from\"./foo\""));
    t.assert_print_same(JsString::from("import\"./foo\""));
    t.assert_print_same(JsString::from("import x,{a as b}from\"./foo\""));
    t.assert_print_same(JsString::from("import{a as b,c as d}from\"./foo\""));
    t.assert_print_same(JsString::from("import x,{a}from\"./foo\""));
    t.assert_print_same(JsString::from("import{a,c}from\"./foo\""));
    t.assert_print_same(JsString::from("import x,*as f from\"./foo\""));
    t.assert_print_same(JsString::from("import*as f from\"./foo\""));
}
// port: CodePrinterTest#testExports
#[test]
fn test_exports() {
    let mut t = CodePrinterTestBase::new();
    t.diagnostics_to_ignore = vec![closure_jscomp::diagnostic_groups::MODULE_LOAD.clone()];
    t.assert_print_same(JsString::from("export var x=1"));
    t.assert_print_same(JsString::from("export var x;export var y"));
    t.assert_print_same(JsString::from("export let x=1"));
    t.assert_print_same(JsString::from("export const x=1"));
    t.assert_print_same(JsString::from("export function f(){}"));
    t.assert_print_same(JsString::from("export class f{}"));
    t.assert_print_same(JsString::from("export class f{}export class b{}"));
    t.assert_print(
        JsString::from("export * from './a.b.c'"),
        JsString::from("export*from\"./a.b.c\""),
    );
    t.assert_print_same(JsString::from("export{a}from\"./a.b.c\""));
    t.assert_print_same(JsString::from("export{a as x}from\"./a.b.c\""));
    t.assert_print_same(JsString::from("export{a,b}from\"./a.b.c\""));
    t.assert_print_same(JsString::from("export{a as x,b as y}from\"./a.b.c\""));
    t.assert_print_same(JsString::from("export{a}"));
    t.assert_print_same(JsString::from("export{a as x}"));
    t.assert_print_same(JsString::from("export{a,b}"));
    t.assert_print_same(JsString::from("export{a as x,b as y}"));
    t.assert_print_same(JsString::from("export default x"));
    t.assert_print_same(JsString::from("export default 1"));
    t.assert_print_same(JsString::from(
        "export default class Foo{}export function f(){}",
    ));
    t.assert_print_same(JsString::from(
        "export function f(){}export default class Foo{}",
    ));
}
// port: CodePrinterTest#testMultiLineTemplateLiteral_brokenOntoLastLine_isNotCollapsed
#[test]
fn test_multi_line_template_literal_broken_onto_last_line_is_not_collapsed() {
    let mut t = CodePrinterTestBase::new();
    let mut options = closure_jscomp::compiler_options::CompilerOptions::new();
    options.set_line_length_threshold(30);
    let node = t.parse(JsString::from("`hello\nworld\nfoo\nbar`;\n"));
    let actual = closure_jscomp::code_printer::Builder::new(node)
        .set_compiler_options(&options)
        .set_pretty_print(false)
        .build(&t.last_compiler);
    assert_eq!(actual, JsString::from("`hello\nworld\nfoo\nbar`"));
}

// port: CodePrinterTest#testPrintsContentOfClosureUnawareShadow
#[test]
fn test_prints_content_of_closure_unaware_shadow() {
    use closure_jscomp::{
        Compiler, ast_factory::AstFactory, code_printer::Builder, compiler_options::CompilerOptions,
    };
    use closure_rhino::{ir::IR, jscomp_colors::standard_colors};
    let mut compiler = Compiler::new();
    let shadow_host = IR::name(&mut compiler, "SHADOW");
    let ast_factory = compiler.create_ast_factory_without_types();
    let closure_unaware_fn = ast_factory.create_empty_function(
        &mut compiler,
        AstFactory::type_(standard_colors::UNKNOWN.clone()),
    );
    let target = IR::name(&mut compiler, "PRESERVE");
    let call = IR::call(&mut compiler, target, &[closure_unaware_fn]);
    let expr = IR::expr_result(&mut compiler, call);
    let script = IR::script_with_children(&mut compiler, &[expr]);
    let root = IR::root(&mut compiler, &[script]);
    shadow_host.set_closure_unaware_shadow(&mut compiler, Some(root));
    assert_eq!(
        Builder::new(shadow_host)
            .set_compiler_options(&CompilerOptions::new())
            .build(&compiler),
        JsString::from("function(){}")
    );
}
// port: CodePrinterTest#testTypeAnnotations
#[test]
fn test_type_annotations() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @constructor */ function Foo(){}"),
        JsString::from("/**\n * @constructor\n */\nfunction Foo() {\n}\n"),
    );
}
// port: CodePrinterTest#testNonNullTypes
#[test]
fn test_non_null_types() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @constructor */\nfunction Foo() {}\n/** @return {!Foo} */\nFoo.prototype.f = function() { return new Foo; };\n"),
        JsString::from("/**\n * @constructor\n */\nfunction Foo() {\n}\n/**\n * @return {!Foo}\n */\nFoo.prototype.f = function() {\n  return new Foo();\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsTypeDef
#[test]
fn test_type_annotations_type_def() {
    let mut t = CodePrinterTestBase::new();
    // TODO(johnlenz): It would be nice if there were some way to preserve
    // typedefs but currently they are resolved into the basic types in the
    // type registry.
    t.assert_type_annotations(
        JsString::from("/** @const */ var goog = {};\n/** @const */ goog.java = {};\n/** @typedef {Array<number>} */ goog.java.Long;\n/** @param {!goog.java.Long} a*/\nfunction f(a){};\n"),
        JsString::from("/** @const */ var goog = {};\n/** @const */ goog.java = {};\ngoog.java.Long;\n/**\n * @param {!Array<number>} a\n * @return {undefined}\n */\nfunction f(a) {\n}\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsAssign
#[test]
fn test_type_annotations_assign() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @constructor */ var Foo = function(){}"),
        JsString::from("/**\n * @constructor\n */\nvar Foo = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsNamespace_varWithoutJSDoc
#[test]
fn test_type_annotations_namespace_var_without_js_doc() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("var a = {};\n/** @constructor */ a.Foo = function(){}\n"),
        JsString::from("var a = {};\n/**\n * @constructor\n */\na.Foo = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsNamespace_varWithConstJSDoc
#[test]
fn test_type_annotations_namespace_var_with_const_js_doc() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @const */\nvar a = {};\n/** @constructor */ a.Foo = function(){}\n"),
        JsString::from(
            "/** @const */ var a = {};\n/**\n * @constructor\n */\na.Foo = function() {\n};\n",
        ),
    );
}
// port: CodePrinterTest#testTypeAnnotationsNamespace_constDeclarationWithoutJSDoc
#[test]
fn test_type_annotations_namespace_const_declaration_without_js_doc() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("const a = {};\n/** @constructor */ a.Foo = function(){}\n"),
        JsString::from("const a = {};\n/**\n * @constructor\n */\na.Foo = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsNamespace_constDeclarationWithJSDoc
#[test]
fn test_type_annotations_namespace_const_declaration_with_js_doc() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @export */\nconst a = {};\n/** @constructor */ a.Foo = function(){}\n"),
        JsString::from(
            "/** @export */ const a = {};\n/**\n * @constructor\n */\na.Foo = function() {\n};\n",
        ),
    );
}
// port: CodePrinterTest#testTypeAnnotationsNamespace_qnameWithConstJSDoc
#[test]
fn test_type_annotations_namespace_qname_with_const_js_doc() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @const */\nvar a = {};\n/** @const */\na.b = {};\n/** @constructor */ a.b.Foo = function(){}\n"),
        JsString::from("/** @const */ var a = {};\n/** @const */ a.b = {};\n/**\n * @constructor\n */\na.b.Foo = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsMemberSubclass
#[test]
fn test_type_annotations_member_subclass() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @const */ var a = {};\n/** @constructor */ a.Foo = function(){};\n/** @constructor\n @extends {a.Foo} */ a.Bar = function(){}\n"),
        JsString::from("/** @const */ var a = {};\n/**\n * @constructor\n */\na.Foo = function() {\n};\n/**\n * @extends {a.Foo}\n * @constructor\n */\na.Bar = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsInterface
#[test]
fn test_type_annotations_interface() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @const */ var a = {};\n/** @interface */ a.Foo = function(){};\n/** @interface\n @extends {a.Foo} */ a.Bar = function(){}\n"),
        JsString::from("/** @const */ var a = {};\n/**\n * @interface\n */\na.Foo = function() {\n};\n/**\n * @extends {a.Foo}\n * @interface\n */\na.Bar = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsMultipleInterface
#[test]
fn test_type_annotations_multiple_interface() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @const */ var a = {};\n/** @interface */ a.Foo1 = function(){};\n/** @interface */ a.Foo2 = function(){};\n/** @interface\n @extends {a.Foo1}\n @extends {a.Foo2} */\na.Bar = function(){}\n"),
        JsString::from("/** @const */ var a = {};\n/**\n * @interface\n */\na.Foo1 = function() {\n};\n/**\n * @interface\n */\na.Foo2 = function() {\n};\n/**\n * @extends {a.Foo1}\n * @extends {a.Foo2}\n * @interface\n */\na.Bar = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsMember
#[test]
fn test_type_annotations_member() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("var a = {};\n/** @constructor */ a.Foo = function(){}\n/** @param {string} foo\n  * @return {number} */\na.Foo.prototype.foo = function(foo) { return 3; };\n/** @type {!Array|undefined} */\na.Foo.prototype.bar = [];\n"),
        JsString::from("var a = {};\n/**\n * @constructor\n */\na.Foo = function() {\n};\n/**\n * @param {string} foo\n * @return {number}\n */\na.Foo.prototype.foo = function(foo) {\n  return 3;\n};\n/** @type {!Array<?>} */\na.Foo.prototype.bar = [];\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsMemberStub
#[test]
fn test_type_annotations_member_stub() {
    let mut t = CodePrinterTestBase::new();
    // TODO(blickly): Investigate why the method's type isn't preserved.
    t.assert_type_annotations(
        JsString::from("/** @interface */ function I(){};\n/** @return {undefined} @param {number} x */ I.prototype.method;\n"),
        JsString::from("/**\n * @interface\n */\nfunction I() {\n}\nI.prototype.method;\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationsImplements
#[test]
fn test_type_annotations_implements() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @const */ var a = {};\n/** @constructor */ a.Foo = function(){};\n/** @interface */ a.I = function(){};\n/** @record */ a.I2 = function(){};\n/** @record @extends {a.I2} */ a.I3 = function(){};\n/** @constructor\n @extends {a.Foo}\n * @implements {a.I}\n @implements {a.I2}\n */ a.Bar = function(){}\n"),
        JsString::from("/** @const */ var a = {};\n/**\n * @constructor\n */\na.Foo = function() {\n};\n/**\n * @interface\n */\na.I = function() {\n};\n/**\n * @record\n */\na.I2 = function() {\n};\n/**\n * @extends {a.I2}\n * @record\n */\na.I3 = function() {\n};\n/**\n * @extends {a.Foo}\n * @implements {a.I}\n * @implements {a.I2}\n * @constructor\n */\na.Bar = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationClassImplements
#[test]
fn test_type_annotation_class_implements() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @interface */ class Foo {}\n/** @implements {Foo} */ class Bar {}\n"),
        JsString::from("/**\n * @interface\n */\nclass Foo {\n}\n/**\n * @implements {Foo}\n */\nclass Bar {\n}\n"),
    );
}
// port: CodePrinterTest#testTypeAnnotationClassConstructor
#[test]
fn test_type_annotation_class_constructor() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/**\n * @template T\n */\nclass Foo {\n  /** @param {T} arg */\n  constructor(arg) {}\n}\n"),
        JsString::from("/**\n * @template T\n */\nclass Foo {\n  /**\n   * @param {T} arg\n   */\n  constructor(arg) {\n  }\n}\n"),
    );
}
// port: CodePrinterTest#testRestParameter
#[test]
fn test_rest_parameter() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {...string} args */\nfunction f(...args) {}\n"),
        JsString::from("/**\n * @param {...string} args\n * @return {undefined}\n */\nfunction f(...args) {\n}\n"),
    );
}
// port: CodePrinterTest#testDefaultParameter
#[test]
fn test_default_parameter() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {string=} msg */\nfunction f(msg = 'hi') {}\n"),
        JsString::from("/**\n * @param {string=} msg\n * @return {undefined}\n */\nfunction f(msg = \"hi\") {\n}\n"),
    );
}
// port: CodePrinterTest#testObjectDestructuringParameter
#[test]
fn test_object_destructuring_parameter() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {{a: number, b: number}} ignoredName */\nfunction f({a, b}) {}\n"),
        JsString::from("/**\n * @param {{a: number, b: number}} p0\n * @return {undefined}\n */\nfunction f({a, b}) {\n}\n"),
    );
}
// port: CodePrinterTest#testObjectDestructuringParameterWithDefault
#[test]
fn test_object_destructuring_parameter_with_default() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {{a: number, b: number}=} ignoredName */\nfunction f({a, b} = {a: 1, b: 2}) {}\n"),
        JsString::from("/**\n * @param {{a: number, b: number}=} p0\n * @return {undefined}\n */\nfunction f({a, b} = {a:1, b:2}) {\n}\n"),
    );
}
// port: CodePrinterTest#testArrayDestructuringParameter
#[test]
fn test_array_destructuring_parameter() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {!Iterable<number>} ignoredName */\nfunction f([a, b]) {}\n"),
        JsString::from("/**\n * @param {!Iterable<number,?,?>} p0\n * @return {undefined}\n */\nfunction f([a, b]) {\n}\n"),
    );
}
// port: CodePrinterTest#testArrayDestructuringParameterWithDefault
#[test]
fn test_array_destructuring_parameter_with_default() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {!Iterable<number>=} ignoredName */\nfunction f([a, b] = [1, 2]) {}\n"),
        JsString::from("/**\n * @param {!Iterable<number,?,?>=} p0\n * @return {undefined}\n */\nfunction f([a, b] = [1, 2]) {\n}\n"),
    );
}
// port: CodePrinterTest#testU2UFunctionTypeAnnotation1
#[test]
fn test_u2_u_function_type_annotation1() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @type {!Function} */ var x = function() {}"),
        JsString::from("/** @type {!Function} */\nvar x = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testU2UFunctionTypeAnnotation2
#[test]
fn test_u2_u_function_type_annotation2() {
    let mut t = CodePrinterTestBase::new();
    // TODO(johnlenz): we currently report the type of the RHS which is not
    // correct, we should export the type of the LHS.
    t.assert_type_annotations(
        JsString::from("/** @type {Function} */ var x = function() {}"),
        JsString::from("/** @type {!Function} */\nvar x = function() {\n};\n"),
    );
}
// port: CodePrinterTest#testEmitUnknownParamTypesAsAllType
#[test]
fn test_emit_unknown_param_types_as_all_type() {
    let mut t = CodePrinterTestBase::new();
    // x is unused, so NTI infers that x can be omitted.
    t.assert_type_annotations(
        JsString::from("var a = function(x) {}"),
        JsString::from(
            "/**\n * @param {?} x\n * @return {undefined}\n */\nvar a = function(x) {\n};\n",
        ),
    );
}
// port: CodePrinterTest#testOptionalTypesAnnotation
#[test]
fn test_optional_types_annotation() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {string=} x */ var a = function(x) {}"),
        JsString::from(
            "/**\n * @param {string=} x\n * @return {undefined}\n */\nvar a = function(x) {\n};\n",
        ),
    );
}
// port: CodePrinterTest#testOptionalTypesAnnotation2
#[test]
fn test_optional_types_annotation2() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {undefined=} x */ var a = function(x) {}"),
        JsString::from("/**\n * @param {undefined=} x\n * @return {undefined}\n */\nvar a = function(x) {\n};\n"),
    );
}
// port: CodePrinterTest#testVariableArgumentsTypesAnnotation
#[test]
fn test_variable_arguments_types_annotation() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @param {...string} x */ var a = function(x) {}"),
        JsString::from("/**\n * @param {...string} x\n * @return {undefined}\n */\nvar a = function(x) {\n};\n"),
    );
}
// port: CodePrinterTest#testTempConstructor
#[test]
fn test_temp_constructor() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("var x = function() {\n  /** @constructor */ function t1() {}\n  /** @constructor */ function t2() {}\n  t1.prototype = t2.prototype\n}\n"),
        JsString::from("/**\n * @return {undefined}\n */\nvar x = function() {\n  /**\n   * @constructor\n   */\n  function t1() {\n  }\n  /**\n   * @constructor\n   */\n  function t2() {\n  }\n  t1.prototype = t2.prototype;\n};\n"),
    );
}
// port: CodePrinterTest#testEnumAnnotation1
#[test]
fn test_enum_annotation1() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @enum {string} */ const Enum = {FOO: 'x', BAR: 'y'};"),
        JsString::from("/** @enum {string} */\nconst Enum = {FOO:\"x\", BAR:\"y\"};\n"),
    );
}
// port: CodePrinterTest#testEnumAnnotation2
#[test]
fn test_enum_annotation2() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @const */ var goog = goog || {};\n/** @enum {string} */ goog.Enum = {FOO: 'x', BAR: 'y'};\n/** @const */ goog.Enum2 = goog.x ? {} : goog.Enum;\n"),
        JsString::from("/** @const */ var goog = goog || {};\n/** @enum {string} */\ngoog.Enum = {FOO:\"x\", BAR:\"y\"};\n/** @type {(!Object|{})} */\ngoog.Enum2 = goog.x ? {} : goog.Enum;\n"),
    );
}
// port: CodePrinterTest#testEnumAnnotation3
#[test]
fn test_enum_annotation3() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @enum {!Object} */ var Enum = {FOO: {}};"),
        JsString::from("/** @enum {!Object} */\nvar Enum = {FOO:{}};\n"),
    );
}
// port: CodePrinterTest#testEnumAnnotation4
#[test]
fn test_enum_annotation4() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @enum {number} */ var E = {A:1, B:2};\nfunction f(/** !E */ x) { return x; }\n"),
        JsString::from("/** @enum {number} */\nvar E = {A:1, B:2};\n/**\n * @param {number} x\n * @return {?}\n */\nfunction f(x) {\n  return x;\n}\n"),
    );
}
// port: CodePrinterTest#testClosureLibraryTypeAnnotationExamples
#[test]
fn test_closure_library_type_annotation_examples() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/** @const */ var goog = goog || {};\n/** @param {Object} obj */goog.removeUid = function(obj) {};\n/** @param {Object} obj The object to remove the field from. */\ngoog.removeHashCode = goog.removeUid;\n"),
        JsString::from("/** @const */ var goog = goog || {};\n/**\n * @param {(Object|null)} obj\n * @return {undefined}\n */\ngoog.removeUid = function(obj) {\n};\n/**\n * @param {(Object|null)} p0\n * @return {undefined}\n */\ngoog.removeHashCode = goog.removeUid;\n"),
    );
}
// port: CodePrinterTest#testFunctionTypeAnnotation
#[test]
fn test_function_type_annotation() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/**\n * @param {{foo:number}} arg\n */\nfunction f(arg) {}"),
        JsString::from("/**\n * @param {{foo: number}} arg\n * @return {undefined}\n */\nfunction f(arg) {\n}\n"),
    );
    t.assert_type_annotations(
        JsString::from("/**\n * @param {number} arg\n */\nfunction f(arg) {}"),
        JsString::from(
            "/**\n * @param {number} arg\n * @return {undefined}\n */\nfunction f(arg) {\n}\n",
        ),
    );
    t.assert_type_annotations(
        JsString::from("/**\n * @param {!Array<string>} arg\n */\nfunction f(arg) {}"),
        JsString::from("/**\n * @param {!Array<string>} arg\n * @return {undefined}\n */\nfunction f(arg) {\n}\n"),
    );
}
// port: CodePrinterTest#testFunctionWithThisTypeAnnotation
#[test]
fn test_function_with_this_type_annotation() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("/**\n * @this {{foo:number}}\n */\nfunction foo() {}"),
        JsString::from(
            "/**\n * @return {undefined}\n * @this {{foo: number}}\n */\nfunction foo() {\n}\n",
        ),
    );
    t.assert_type_annotations(
        JsString::from("/**\n * @this {!Array<string>}\n */\nfunction foo() {}"),
        JsString::from(
            "/**\n * @return {undefined}\n * @this {!Array<string>}\n */\nfunction foo() {\n}\n",
        ),
    );
}
// port: CodePrinterTest#testStrict
#[test]
fn test_strict() {
    let mut t = CodePrinterTestBase::new();
    let root = t.parse_type_checked(JsString::from("var x"), /* typeChecked= */ true);
    let result = t.default_builder_build(root, |b| b.set_tag_as_strict(true));
    assert_eq!(result, JsString::from("'use strict';var x"));
}
// port: CodePrinterTest#testStrictPretty
#[test]
fn test_strict_pretty() {
    let mut t = CodePrinterTestBase::new();
    let root = t.parse_type_checked(JsString::from("var x"), /* typeChecked= */ true);
    let result =
        t.default_builder_build(root, |b| b.set_tag_as_strict(true).set_pretty_print(true));
    assert_eq!(result, JsString::from("'use strict';\nvar x;\n"));
}
// port: CodePrinterTest#testIjs
#[test]
fn test_ijs() {
    let mut t = CodePrinterTestBase::new();
    let root = t.parse_type_checked(JsString::from("var x"), /* typeChecked= */ true);
    let result = t.default_builder_build(root, |b| b.set_tag_as_type_summary(true));
    assert_eq!(
        result,
        JsString::from("/** @fileoverview @typeSummary */\nvar x")
    );
}
// port: CodePrinterTest#testTypeAnnotationClassMember
#[test]
fn test_type_annotation_class_member() {
    let mut t = CodePrinterTestBase::new();
    t.assert_type_annotations(
        JsString::from("class Foo {\n  /** @return {number} */ method(/** string */ arg) {}\n}\n"),
        JsString::from("class Foo {\n  /**\n   * @param {string} arg\n   * @return {number}\n   */\n  method(arg) {\n  }\n}\n"),
    );
}

// port: CodePrinterTest#checkWithOriginalName
fn check_with_original_name(
    code: &str,
    expected_code: &str,
    compiler_options: &mut closure_jscomp::compiler_options::CompilerOptions,
) {
    compiler_options.set_check_symbols(true);
    compiler_options.set_check_types(true);
    compiler_options.set_preserve_detailed_source_info(true);
    compiler_options.set_preserve_closure_primitives(true);
    compiler_options.set_closure_pass(true);
    let mut compiler = closure_jscomp::Compiler::new();
    compiler.disable_threads();
    compiler.compile(
        &[], // Externs
        &[std::sync::Arc::new(
            closure_jscomp::source_file::SourceFile::from_code("test", code),
        )],
        compiler_options.clone(),
    );
    let node = compiler
        .get_root()
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();

    let mut code_printer_options = closure_jscomp::compiler_options::CompilerOptions::new();
    code_printer_options.set_prefer_single_quotes(true);
    code_printer_options.set_line_length_threshold(80);
    code_printer_options.set_use_original_names_in_output(true);
    assert_eq!(
        closure_jscomp::code_printer::Builder::new(node)
            .set_compiler_options(&code_printer_options)
            .set_pretty_print(true)
            .set_line_break(true)
            .build(&compiler)
            .to_string(),
        expected_code
    );
}

// port: CodePrinterTest#testEs6GoogModule
#[test]
fn test_es6_goog_module() {
    let code = "goog.module('foo.bar');
const STR = '3';
function fn() {
  alert(STR);
}
exports.fn = fn;
";
    let expected_code = "goog.module('foo.bar');
var module$exports$foo$bar = {};
const STR = '3';
function fn() {
  alert(STR);
}
exports.fn = fn;
";

    let mut compiler_options = closure_jscomp::compiler_options::CompilerOptions::new();
    compiler_options.set_closure_pass(true);
    compiler_options.set_preserve_detailed_source_info(true);
    compiler_options.set_continue_after_errors(true);
    let mut compiler = closure_jscomp::Compiler::new();
    compiler.disable_threads();
    check_with_original_name(code, expected_code, &mut compiler_options);
}

// port: CodePrinterTest#testGoogScope
#[test]
fn test_goog_scope() {
    // TODO(mknichel): Function declarations need to be rewritten to match the original source
    // instead of being assigned to a local variable with duplicate JS Doc.
    let code = "goog.provide('foo.bar');
goog.require('baz.qux.Quux');
goog.require('foo.ScopedType');

goog.scope(function() {
var Quux = baz.qux.Quux;
var ScopedType = foo.ScopedType;

var STR = '3';
/** @param {ScopedType} obj */
function fn(obj) {
  alert(STR);
  alert(Quux.someProperty);
}
}); // goog.scope
";
    let expected_code = "goog.provide('foo.bar');
goog.require('baz.qux.Quux');
goog.require('foo.ScopedType');
/**
 * @param {ScopedType} obj
 */
var $jscomp$scope$3556498$1$fn = /**
 * @param {ScopedType} obj
 */
function(obj) {
  alert(STR);
  alert(Quux.someProperty);
};
var $jscomp$scope$3556498$0$STR = '3';
";

    let mut compiler_options = closure_jscomp::compiler_options::CompilerOptions::new();
    compiler_options.set_checks_only(true);
    compiler_options.set_closure_pass(true);
    compiler_options.set_preserve_detailed_source_info(true);
    compiler_options.set_check_types(true);
    compiler_options.set_continue_after_errors(true);
    compiler_options.set_preserve_closure_primitives(true);
    let mut compiler = closure_jscomp::Compiler::new();
    compiler.disable_threads();
    compiler.compile(
        &[], // Externs
        &[std::sync::Arc::new(
            closure_jscomp::source_file::SourceFile::from_code("test", code),
        )],
        compiler_options,
    );
    let node = compiler
        .get_root()
        .unwrap()
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();

    let mut code_printer_options = closure_jscomp::compiler_options::CompilerOptions::new();
    code_printer_options.set_prefer_single_quotes(true);
    code_printer_options.set_line_length_threshold(80);
    code_printer_options.set_preserve_type_annotations(true);
    code_printer_options.set_use_original_names_in_output(true);
    assert_eq!(
        closure_jscomp::code_printer::Builder::new(node)
            .set_compiler_options(&code_printer_options)
            .set_pretty_print(true)
            .set_line_break(true)
            .build(&compiler)
            .to_string(),
        expected_code
    );
}
