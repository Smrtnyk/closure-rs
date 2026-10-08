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
//   test/com/google/javascript/jscomp/NodeUtilTest.java.

use super::*;
mod is_defined_value_tests {
    use super::*;
    // port: NodeUtilTest.IsDefinedValueTests#testIsDefinedValue
    #[test]
    fn test_is_defined_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "null ?? undefined");
        assert!(!NodeUtil::is_defined_value(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "null ?? null");
        assert!(NodeUtil::is_defined_value(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "undefined ?? undefined");
        assert!(!NodeUtil::is_defined_value(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "undefined ?? null");
        assert!(NodeUtil::is_defined_value(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "0 ?? undefined");
        assert!(!NodeUtil::is_defined_value(&compiler, value_5));
    }

    // port: NodeUtilTest.IsDefinedValueTests#isDefinedValueOptionalChain
    #[test]
    fn is_defined_value_optional_chain() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "x?.y");
        assert!(!NodeUtil::is_defined_value(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "x?.[y]");
        assert!(!NodeUtil::is_defined_value(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "x?.()");
        assert!(!NodeUtil::is_defined_value(&compiler, value_3));
    }
}

mod is_from_type_summary_tests {
    use super::*;
    // port: NodeUtilTest.IsFromTypeSummaryTests#identifiesTypeSummaryJSDoc
    #[test]
    fn identifies_type_summary_js_doc() {
        let mut compiler = Compiler::new();
        let script = parse(&mut compiler, "/** @fileoverview @typeSummary */ var x;");
        assert!(NodeUtil::is_from_type_summary(&compiler, script));
        let script_with_externs = parse(
            &mut compiler,
            "// IJS\n/** @fileoverview @typeSummary */\n/** @externs */\n;",
        );
        assert!(NodeUtil::is_from_type_summary(
            &compiler,
            script_with_externs
        ));
        let regular_script = parse(&mut compiler, "var x;");
        assert!(!NodeUtil::is_from_type_summary(&compiler, regular_script));
    }
}

mod is_property_test_tests {
    use super::*;
    // port: NodeUtilTest.IsPropertyTestTests#optChainGetPropIsPropertyTest
    #[test]
    fn opt_chain_get_prop_is_property_test() {
        let mut compiler = Compiler::new();
        let get_prop = parse_expr(&mut compiler, "x.y?.z");
        assert!(NodeUtil::is_property_test(
            &compiler,
            get_prop.get_first_child(&compiler).unwrap()
        ));
    }

    // port: NodeUtilTest.IsPropertyTestTests#optChainGetElemIsPropertyTest
    #[test]
    fn opt_chain_get_elem_is_property_test() {
        let mut compiler = Compiler::new();
        let get_elem = parse_expr(&mut compiler, "x.y?.[z]");
        assert!(NodeUtil::is_property_test(
            &compiler,
            get_elem.get_first_child(&compiler).unwrap()
        ));
    }

    // port: NodeUtilTest.IsPropertyTestTests#optChainCallIsPropertyTest
    #[test]
    fn opt_chain_call_is_property_test() {
        let mut compiler = Compiler::new();
        let call = parse_expr(&mut compiler, "x.y?.(z)");
        assert!(NodeUtil::is_property_test(
            &compiler,
            call.get_first_child(&compiler).unwrap()
        ));
    }

    // port: NodeUtilTest.IsPropertyTestTests#optChainNonStartOfChainIsPropertyTest
    #[test]
    fn opt_chain_non_start_of_chain_is_property_test() {
        let mut compiler = Compiler::new();
        let get_prop = parse_expr(&mut compiler, "x.y?.z.foo.bar");
        assert!(NodeUtil::is_property_test(
            &compiler,
            get_prop.get_first_child(&compiler).unwrap()
        ));
    }
}

mod assorted_tests {
    use super::*;
    // port: NodeUtilTest.AssortedTests#testIsLiteralOrConstValue
    #[test]
    fn test_is_literal_or_const_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "10");
        assert_literal_and_immutable(&compiler, value_1);
        let value_2 = parse_expr(&mut compiler, "-10");
        assert_literal_and_immutable(&compiler, value_2);
        let value_3 = parse_expr(&mut compiler, "[10, 20]");
        assert_literal_but_not_immutable(&compiler, value_3);
        let value_4 = parse_expr(&mut compiler, "{'a': 20}");
        assert_literal_but_not_immutable(&compiler, value_4);
        let value_5 = parse_expr(&mut compiler, "[10, , 1.0, [undefined], 'a']");
        assert_literal_but_not_immutable(&compiler, value_5);
        let value_6 = parse_expr(&mut compiler, "/abc/");
        assert_literal_but_not_immutable(&compiler, value_6);
        let value_7 = parse_expr(&mut compiler, "\"string\"");
        assert_literal_and_immutable(&compiler, value_7);
        let value_8 = parse_expr(&mut compiler, "'aaa'");
        assert_literal_and_immutable(&compiler, value_8);
        let value_9 = parse_expr(&mut compiler, "null");
        assert_literal_and_immutable(&compiler, value_9);
        let value_10 = parse_expr(&mut compiler, "undefined");
        assert_literal_and_immutable(&compiler, value_10);
        let value_11 = parse_expr(&mut compiler, "void 0");
        assert_literal_and_immutable(&compiler, value_11);
        let value_12 = parse_expr(&mut compiler, "abc");
        assert_not_literal(&compiler, value_12);
        let value_13 = parse_expr(&mut compiler, "[10, foo(), 20]");
        assert_not_literal(&compiler, value_13);
        let value_14 = parse_expr(&mut compiler, "foo()");
        assert_not_literal(&compiler, value_14);
        let value_15 = parse_expr(&mut compiler, "c + d");
        assert_not_literal(&compiler, value_15);
        let value_16 = parse_expr(&mut compiler, "{'a': foo()}");
        assert_not_literal(&compiler, value_16);
        let value_17 = parse_expr(&mut compiler, "void foo()");
        assert_not_literal(&compiler, value_17);
    }

    // port: NodeUtilTest.AssortedTests#testObjectLiteralIsLiteralValue
    #[test]
    fn test_object_literal_is_literal_value() {
        let mut compiler = Compiler::new();
        let value_1 = is_literal_value(&mut compiler, "{a: 20}");
        assert!(value_1);
        let value_2 = is_literal_value(&mut compiler, "{'a': 20}");
        assert!(value_2);
        let value_3 = is_literal_value(&mut compiler, "{a: function() {}}");
        assert!(value_3);
        let value_4 = is_literal_value_excluding_functions(&mut compiler, "{a: function() {}}");
        assert!(!value_4);
        let value_5 = is_literal_value(&mut compiler, "{a() {}}");
        assert!(value_5);
        let value_6 = is_literal_value_excluding_functions(&mut compiler, "{a() {}}");
        assert!(!value_6);
        let value_7 = is_literal_value(&mut compiler, "{'a'() {}}");
        assert!(value_7);
        let value_8 = is_literal_value_excluding_functions(&mut compiler, "{'a'() {}}");
        assert!(!value_8);
        let value_9 = is_literal_value(&mut compiler, "{['a']: 20}");
        assert!(value_9);
        let value_10 = is_literal_value(&mut compiler, "{[b]: 20}");
        assert!(!value_10);
        let value_11 = is_literal_value(&mut compiler, "{['a']() {}}");
        assert!(value_11);
        let value_12 = is_literal_value(&mut compiler, "{[b]() {}}");
        assert!(!value_12);
        let value_13 = is_literal_value_excluding_functions(&mut compiler, "{['a']() {}}");
        assert!(!value_13);
        let value_14 = is_literal_value(&mut compiler, "{ get a() { return 0; } }");
        assert!(value_14);
        let value_15 = is_literal_value(&mut compiler, "{ get 'a'() { return 0; } }");
        assert!(value_15);
        let value_16 = is_literal_value(&mut compiler, "{ get ['a']() { return 0; } }");
        assert!(value_16);
        let value_17 = is_literal_value(&mut compiler, "{ get 123() { return 0; } }");
        assert!(value_17);
        let value_18 = is_literal_value(&mut compiler, "{ get [123]() { return 0; } }");
        assert!(value_18);
        let value_19 = is_literal_value(&mut compiler, "{ get [b]() { return 0; } }");
        assert!(!value_19);
        let value_20 = is_literal_value(&mut compiler, "{ set a(x) { } }");
        assert!(value_20);
        let value_21 = is_literal_value(&mut compiler, "{ set 'a'(x) { } }");
        assert!(value_21);
        let value_22 = is_literal_value(&mut compiler, "{ set ['a'](x) { } }");
        assert!(value_22);
        let value_23 = is_literal_value(&mut compiler, "{ set 123(x) { } }");
        assert!(value_23);
        let value_24 = is_literal_value(&mut compiler, "{ set [123](x) { } }");
        assert!(value_24);
        let value_25 = is_literal_value(&mut compiler, "{ set [b](x) { } }");
        assert!(!value_25);
    }

    // port: NodeUtilTest.AssortedTests#testIsStringLiteralStringLiteral
    #[test]
    fn test_is_string_literal_string_literal() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "'b'");
        assert!(NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_1
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsStringLiteralTemplateNoSubst
    #[test]
    fn test_is_string_literal_template_no_subst() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "`b`");
        assert!(NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_1
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsStringLiteralTemplateWithSubstitution
    #[test]
    fn test_is_string_literal_template_with_substitution() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "`foo${bar}`");
        assert!(!NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_1
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsStringLiteralVariable
    #[test]
    fn test_is_string_literal_variable() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "b");
        assert!(!NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_1
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsStringLiteralConcatLiterals
    #[test]
    fn test_is_string_literal_concat_literals() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "'b' + 'c'");
        assert!(NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_1
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsStringLiteralConcatLiteralVariable
    #[test]
    fn test_is_string_literal_concat_literal_variable() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "b + 'c'");
        assert!(!NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_1
        ));
        let value_2 = parse_expr(&mut compiler, "'b' + c");
        assert!(!NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_2
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsStringLiteralTernary
    #[test]
    fn test_is_string_literal_ternary() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "a ? 'b' : 'c'");
        assert!(NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_1
        ));
        let value_2 = parse_expr(&mut compiler, "a ? b : 'c'");
        assert!(!NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_2
        ));
        let value_3 = parse_expr(&mut compiler, "a ? 'b' : c");
        assert!(!NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_3
        ));
        let value_4 = parse_expr(&mut compiler, "a ? b : c");
        assert!(!NodeUtil::is_some_compile_time_const_string_value(
            &compiler, value_4
        ));
    }

    // port: NodeUtilTest.AssortedTests#testGetStringValue
    #[test]
    fn test_get_string_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "true");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_1).unwrap(),
            "true"
        );
        let value_2 = parse_expr(&mut compiler, "10");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_2).unwrap(),
            "10"
        );
        let value_3 = parse_expr(&mut compiler, "1.0");
        assert_eq!(NodeUtil::get_string_value(&compiler, value_3).unwrap(), "1");
        let value_4 = parse_expr(&mut compiler, "-1.0");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_4).unwrap(),
            "-1"
        );
        let value_5 = parse_expr(&mut compiler, "1.2323919403474454e+21");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_5).unwrap(),
            "1.2323919403474454e+21"
        );
        let value_6 = parse_expr(&mut compiler, "'0'");
        assert_eq!(NodeUtil::get_string_value(&compiler, value_6).unwrap(), "0");
        let value_7 = parse_expr(&mut compiler, "/a/");
        assert!(NodeUtil::get_string_value(&compiler, value_7).is_none());
        let value_8 = parse_expr(&mut compiler, "{}");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_8).unwrap(),
            "[object Object]"
        );
        let value_9 = parse_expr(&mut compiler, "[]");
        assert!(
            NodeUtil::get_string_value(&compiler, value_9)
                .unwrap()
                .is_empty()
        );
        let value_10 = parse_expr(&mut compiler, "false");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_10).unwrap(),
            "false"
        );
        let value_11 = parse_expr(&mut compiler, "null");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_11).unwrap(),
            "null"
        );
        let value_12 = parse_expr(&mut compiler, "0");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_12).unwrap(),
            "0"
        );
        let value_13 = parse_expr(&mut compiler, "1n");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_13).unwrap(),
            "1"
        );
        let value_14 = parse_expr(&mut compiler, "''");
        assert!(
            NodeUtil::get_string_value(&compiler, value_14)
                .unwrap()
                .is_empty()
        );
        let value_15 = parse_expr(&mut compiler, "undefined");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_15).unwrap(),
            "undefined"
        );
        let value_16 = parse_expr(&mut compiler, "void 0");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_16).unwrap(),
            "undefined"
        );
        let value_17 = parse_expr(&mut compiler, "void foo()");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_17).unwrap(),
            "undefined"
        );
        let value_18 = parse_expr(&mut compiler, "NaN");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_18).unwrap(),
            "NaN"
        );
        let value_19 = parse_expr(&mut compiler, "Infinity");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_19).unwrap(),
            "Infinity"
        );
        let value_20 = parse_expr(&mut compiler, "x");
        assert!(NodeUtil::get_string_value(&compiler, value_20).is_none());
        let value_21 = parse_expr(&mut compiler, "`Hello`");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_21).unwrap(),
            "Hello"
        );
        let value_22 = parse_expr(&mut compiler, "`Hello ${'foo'}`");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_22).unwrap(),
            "Hello foo"
        );
        let value_23 = parse_expr(&mut compiler, "`Hello ${name}`");
        assert!(NodeUtil::get_string_value(&compiler, value_23).is_none());
        let value_24 = parse_expr(&mut compiler, "`${4} bananas`");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_24).unwrap(),
            "4 bananas"
        );
        let value_25 = parse_expr(&mut compiler, "`This is ${true}.`");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_25).unwrap(),
            "This is true."
        );
        let value_26 = parse_expr(&mut compiler, "`${'hello'} ${name}`");
        assert!(NodeUtil::get_string_value(&compiler, value_26).is_none());
    }

    // port: NodeUtilTest.AssortedTests#testGetArrayStringValue
    #[test]
    fn test_get_array_string_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "[]");
        assert!(
            NodeUtil::get_string_value(&compiler, value_1)
                .unwrap()
                .is_empty()
        );
        let value_2 = parse_expr(&mut compiler, "['']");
        assert!(
            NodeUtil::get_string_value(&compiler, value_2)
                .unwrap()
                .is_empty()
        );
        let value_3 = parse_expr(&mut compiler, "[null]");
        assert!(
            NodeUtil::get_string_value(&compiler, value_3)
                .unwrap()
                .is_empty()
        );
        let value_4 = parse_expr(&mut compiler, "[undefined]");
        assert!(
            NodeUtil::get_string_value(&compiler, value_4)
                .unwrap()
                .is_empty()
        );
        let value_5 = parse_expr(&mut compiler, "[void 0]");
        assert!(
            NodeUtil::get_string_value(&compiler, value_5)
                .unwrap()
                .is_empty()
        );
        let value_6 = parse_expr(&mut compiler, "[NaN]");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_6).unwrap(),
            "NaN"
        );
        let value_7 = parse_expr(&mut compiler, "[,'']");
        assert_eq!(NodeUtil::get_string_value(&compiler, value_7).unwrap(), ",");
        let value_8 = parse_expr(&mut compiler, "[[''],[''],['']]");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_8).unwrap(),
            ",,"
        );
        let value_9 = parse_expr(&mut compiler, "[[1.0],[2.0]]");
        assert_eq!(
            NodeUtil::get_string_value(&compiler, value_9).unwrap(),
            "1,2"
        );
        let value_10 = parse_expr(&mut compiler, "[a]");
        assert!(NodeUtil::get_string_value(&compiler, value_10).is_none());
        let value_11 = parse_expr(&mut compiler, "[1,a]");
        assert!(NodeUtil::get_string_value(&compiler, value_11).is_none());
    }

    // port: NodeUtilTest.AssortedTests#testMayBeObjectLitKey
    #[test]
    fn test_may_be_object_lit_key() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "({})");
        assert_may_be_object_lit_key(&compiler, value_1, false);
        let value_2 = parse_expr(&mut compiler, "a");
        assert_may_be_object_lit_key(&compiler, value_2, false);
        let value_3 = parse_expr(&mut compiler, "'a'");
        assert_may_be_object_lit_key(&compiler, value_3, false);
        let value_4 = parse_expr(&mut compiler, "1");
        assert_may_be_object_lit_key(&compiler, value_4, false);
        let value_5 = parse_expr(&mut compiler, "1n");
        assert_may_be_object_lit_key(&compiler, value_5, false);
        let value_6 = parse_expr(&mut compiler, "({a: 1})");
        assert_may_be_object_lit_key(&compiler, value_6.get_first_child(&compiler).unwrap(), true);
        let value_7 = parse_expr(&mut compiler, "({1: 1})");
        assert_may_be_object_lit_key(&compiler, value_7.get_first_child(&compiler).unwrap(), true);
        let value_8 = parse_expr(&mut compiler, "({get a(){}})");
        assert_may_be_object_lit_key(&compiler, value_8.get_first_child(&compiler).unwrap(), true);
        let value_9 = parse_expr(&mut compiler, "({set a(b){}})");
        assert_may_be_object_lit_key(&compiler, value_9.get_first_child(&compiler).unwrap(), true);
        let value_10 = parse_expr(&mut compiler, "({['a']: 1})");
        assert_may_be_object_lit_key(
            &compiler,
            value_10.get_first_child(&compiler).unwrap(),
            false,
        );
        let value_11 = parse_expr(&mut compiler, "({a} = {})");
        assert_may_be_object_lit_key(
            &compiler,
            value_11.get_first_first_child(&compiler).unwrap(),
            true,
        );
        let value_12 = parse_expr(&mut compiler, "(class { a() {} })");
        assert_may_be_object_lit_key(
            &compiler,
            value_12
                .get_last_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
            true,
        );
    }

    // port: NodeUtilTest.AssortedTests#testIsObjectLitKey
    #[test]
    fn test_is_object_lit_key() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "({a: 1})");
        assert_is_object_lit_key(&compiler, value_1.get_first_child(&compiler).unwrap(), true);
        let value_2 = parse_expr(&mut compiler, "({['a']: 1})");
        assert_may_be_object_lit_key(
            &compiler,
            value_2.get_first_child(&compiler).unwrap(),
            false,
        );
        let value_3 = parse_expr(&mut compiler, "({a} = {})");
        assert_is_object_lit_key(
            &compiler,
            value_3.get_first_first_child(&compiler).unwrap(),
            false,
        );
        let value_4 = parse_expr(&mut compiler, "(class { a() {} })");
        assert_is_object_lit_key(
            &compiler,
            value_4
                .get_last_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
            false,
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetFunctionName1
    #[test]
    fn test_get_function_name1() {
        let mut compiler = Compiler::new();
        let parent = parse(&mut compiler, "function name(){}");
        assert_get_name_result(
            &compiler,
            parent.get_first_child(&compiler).unwrap(),
            "name",
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetFunctionName2
    #[test]
    fn test_get_function_name2() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var name = function(){}");
        let parent = value_1.get_first_first_child(&compiler);
        assert_get_name_result(
            &compiler,
            parent.unwrap().get_first_child(&compiler).unwrap(),
            "name",
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetFunctionName3
    #[test]
    fn test_get_function_name3() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "qualified.name = function(){}");
        let parent = value_1.get_first_first_child(&compiler);
        assert_get_name_result(
            &compiler,
            parent.unwrap().get_last_child(&compiler).unwrap(),
            "qualified.name",
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetFunctionName4
    #[test]
    fn test_get_function_name4() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var name2 = function name1(){}");
        let parent = value_1.get_first_first_child(&compiler);
        assert_get_name_result(
            &compiler,
            parent.unwrap().get_first_child(&compiler).unwrap(),
            "name2",
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetFunctionName5
    #[test]
    fn test_get_function_name5() {
        let mut compiler = Compiler::new();
        let n = parse(&mut compiler, "qualified.name2 = function name1(){}");
        let parent = n.get_first_first_child(&compiler);
        assert_get_name_result(
            &compiler,
            parent.unwrap().get_last_child(&compiler).unwrap(),
            "qualified.name2",
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBestFunctionName1
    #[test]
    fn test_get_best_function_name1() {
        let mut compiler = Compiler::new();
        let parent = parse(&mut compiler, "function func(){}");
        assert_eq!(
            NodeUtil::get_nearest_function_name(
                &compiler,
                parent.get_first_child(&compiler).unwrap()
            )
            .unwrap(),
            "func"
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBestFunctionName2
    #[test]
    fn test_get_best_function_name2() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var obj = {memFunc(){}}");
        let parent = value_1
            .get_first_first_child(&compiler)
            .unwrap()
            .get_first_first_child(&compiler);
        assert_eq!(
            NodeUtil::get_nearest_function_name(
                &compiler,
                parent.unwrap().get_last_child(&compiler).unwrap()
            )
            .unwrap(),
            "memFunc"
        );
    }

    // port: NodeUtilTest.AssortedTests#testConstKeywordNamespace
    #[test]
    fn test_const_keyword_namespace() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "const ns = {};");
        let decl = value_1.get_first_child(&compiler);
        assert!(decl.unwrap().is_const(&compiler));
        let name_node = decl.unwrap().get_first_child(&compiler);
        assert!(name_node.unwrap().is_name(&compiler));
        assert!(NodeUtil::is_namespace_decl(&compiler, name_node.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testIsFunctionDeclaration
    #[test]
    fn test_is_function_declaration() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(&mut compiler, Token::FUNCTION, "function foo(){}");
        assert!(NodeUtil::is_function_declaration(&compiler, value_1));
        let value_2 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "class C { constructor() {} }",
        );
        assert!(!NodeUtil::is_function_declaration(&compiler, value_2));
        let value_3 = parse_first(&mut compiler, Token::FUNCTION, "({ foo() {} })");
        assert!(!NodeUtil::is_function_declaration(&compiler, value_3));
        let value_4 = parse_first(&mut compiler, Token::FUNCTION, "var x = function(){}");
        assert!(!NodeUtil::is_function_declaration(&compiler, value_4));
        let value_5 = parse_first(&mut compiler, Token::FUNCTION, "export function f() {}");
        assert!(NodeUtil::is_function_declaration(&compiler, value_5));
        let value_6 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "export default function() {}",
        );
        assert!(!NodeUtil::is_function_declaration(&compiler, value_6));
        let value_7 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "export default function foo() {}",
        );
        assert!(NodeUtil::is_function_declaration(&compiler, value_7));
        let value_8 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "export default (foo) => { alert(foo); }",
        );
        assert!(!NodeUtil::is_function_declaration(&compiler, value_8));
    }

    // port: NodeUtilTest.AssortedTests#checkParamSideEffects
    fn check_param_side_effects(js: &str) -> bool {
        let mut compiler = Compiler::new();
        let fn_node = parse_first(&mut compiler, Token::FUNCTION, js);
        let ast_analyzer = compiler.get_ast_analyzer();
        NodeUtil::function_parameters_may_have_side_effects(&mut compiler, fn_node, &ast_analyzer)
    }

    // port: NodeUtilTest.AssortedTests#testFunctionParametersMayHaveSideEffects
    #[test]
    fn test_function_parameters_may_have_side_effects() {
        assert!(!check_param_side_effects("function f() {}"));
        assert!(!check_param_side_effects("function f(a, b) {}"));
        assert!(!check_param_side_effects("function f(a = 1) {}"));
        assert!(!check_param_side_effects("function f(a = 'default') {}"));
        assert!(!check_param_side_effects("function f(...a) {}"));

        assert!(check_param_side_effects("function f(a = audit()) {}"));
        assert!(check_param_side_effects("function f({a}) {}"));
        assert!(check_param_side_effects("function f([a]) {}"));
        assert!(check_param_side_effects("function f({a} = {}) {}"));
        assert!(check_param_side_effects("function f(...[a]) {}"));
        assert!(check_param_side_effects("function f(a, {b}, ...c) {}"));
    }

    // port: NodeUtilTest.AssortedTests#testisBlockScopedFunctionDeclaration
    #[test]
    fn testis_block_scoped_function_declaration() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(&mut compiler, Token::FUNCTION, "function foo(){}");
        assert!(!NodeUtil::is_block_scoped_function_declaration(
            &compiler, value_1
        ));
        let value_2 = parse_first(&mut compiler, Token::FUNCTION, "var x = function(){}");
        assert!(!NodeUtil::is_block_scoped_function_declaration(
            &compiler, value_2
        ));
        let value_3 = parse_first(&mut compiler, Token::FUNCTION, "{ function foo(){} }");
        assert!(NodeUtil::is_block_scoped_function_declaration(
            &compiler, value_3
        ));
        let value_4 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "class C { static { function foo(){} } }",
        );
        assert!(NodeUtil::is_block_scoped_function_declaration(
            &compiler, value_4
        ));
        let value_5 = parse_second(
            &mut compiler,
            Token::FUNCTION,
            "function foo(){ class C { static { function a(){} } } }",
        );
        assert!(NodeUtil::is_block_scoped_function_declaration(
            &compiler, value_5
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsMethodDeclaration
    #[test]
    fn test_is_method_declaration() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "class C { constructor() {} }",
        );
        assert!(NodeUtil::is_method_declaration(&compiler, value_1));
        let value_2 = parse_first(&mut compiler, Token::FUNCTION, "class C { a() {} }");
        assert!(NodeUtil::is_method_declaration(&compiler, value_2));
        let value_3 = parse_first(&mut compiler, Token::FUNCTION, "class C { static a() {} }");
        assert!(NodeUtil::is_method_declaration(&compiler, value_3));
        let value_4 = parse_first(&mut compiler, Token::FUNCTION, "({ set foo(v) {} })");
        assert!(NodeUtil::is_method_declaration(&compiler, value_4));
        let value_5 = parse_first(&mut compiler, Token::FUNCTION, "({ get foo() {} })");
        assert!(NodeUtil::is_method_declaration(&compiler, value_5));
        let value_6 = parse_first(&mut compiler, Token::FUNCTION, "({ [foo]() {} })");
        assert!(NodeUtil::is_method_declaration(&compiler, value_6));
    }

    // port: NodeUtilTest.AssortedTests#testIsClassDeclaration
    #[test]
    fn test_is_class_declaration() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(&mut compiler, Token::CLASS, "class Foo {}");
        assert!(NodeUtil::is_class_declaration(&compiler, value_1));
        let value_2 = parse_first(&mut compiler, Token::CLASS, "var Foo = class {}");
        assert!(!NodeUtil::is_class_declaration(&compiler, value_2));
        let value_3 = parse_first(&mut compiler, Token::CLASS, "var Foo = class Foo{}");
        assert!(!NodeUtil::is_class_declaration(&compiler, value_3));
        let value_4 = parse_first(&mut compiler, Token::CLASS, "export default class Foo {}");
        assert!(NodeUtil::is_class_declaration(&compiler, value_4));
        let value_5 = parse_first(&mut compiler, Token::CLASS, "export class Foo {}");
        assert!(NodeUtil::is_class_declaration(&compiler, value_5));
        let value_6 = parse_first(&mut compiler, Token::CLASS, "export default class {}");
        assert!(!NodeUtil::is_class_declaration(&compiler, value_6));
    }

    // port: NodeUtilTest.AssortedTests#testIsFunctionExpression
    #[test]
    fn test_is_function_expression() {
        let mut compiler = Compiler::new();
        assert_contains_anon_func(&mut compiler, true, "(function(){})");
        assert_contains_anon_func(&mut compiler, true, "[function a(){}]");
        assert_contains_anon_func(&mut compiler, true, "({x: function a(){}})");
        assert_contains_anon_func(&mut compiler, true, "({[0]: function a() {}})");
        assert_contains_anon_func(&mut compiler, true, "(function a(){})()");
        assert_contains_anon_func(&mut compiler, true, "x = function a(){};");
        assert_contains_anon_func(&mut compiler, true, "var x = function a(){};");
        assert_contains_anon_func(&mut compiler, true, "if (function a(){});");
        assert_contains_anon_func(&mut compiler, true, "while (function a(){});");
        assert_contains_anon_func(&mut compiler, true, "do; while (function a(){});");
        assert_contains_anon_func(&mut compiler, true, "for (function a(){};;);");
        assert_contains_anon_func(&mut compiler, true, "for (;function a(){};);");
        assert_contains_anon_func(&mut compiler, true, "for (;;function a(){});");
        assert_contains_anon_func(&mut compiler, true, "for (p in function a(){});");
        assert_contains_anon_func(&mut compiler, true, "with (function a(){}) {}");
        assert_contains_anon_func(&mut compiler, false, "function a(){}");
        assert_contains_anon_func(&mut compiler, false, "if (x) function a(){};");
        assert_contains_anon_func(&mut compiler, false, "if (x) { function a(){} }");
        assert_contains_anon_func(&mut compiler, false, "if (x); else function a(){};");
        assert_contains_anon_func(&mut compiler, false, "while (x) function a(){};");
        assert_contains_anon_func(&mut compiler, false, "do function a(){} while (0);");
        assert_contains_anon_func(&mut compiler, false, "for (;;) function a(){}");
        assert_contains_anon_func(&mut compiler, false, "for (p in o) function a(){};");
        assert_contains_anon_func(&mut compiler, false, "with (x) function a(){}");
        assert_contains_anon_func(&mut compiler, true, "export default function() {};");
        assert_contains_anon_func(&mut compiler, false, "export default function a() {};");
        assert_contains_anon_func(&mut compiler, false, "export function a() {};");
        assert_contains_anon_func(&mut compiler, false, "class C { a() {} }");
        assert_contains_anon_func(&mut compiler, false, "class C { static a() {} }");
        assert_contains_anon_func(&mut compiler, false, "x = { a() {} }");
        assert_contains_anon_func(&mut compiler, false, "({[0]() {}})");
        assert_contains_anon_func(&mut compiler, false, "({get [0]() {}})");
        assert_contains_anon_func(&mut compiler, false, "({set [0](x) {}})");
    }

    // port: NodeUtilTest.AssortedTests#testIsClassExpression
    #[test]
    fn test_is_class_expression() {
        let mut compiler = Compiler::new();
        assert_contains_anon_class(&mut compiler, true, "(class {})");
        assert_contains_anon_class(&mut compiler, true, "[class Clazz {}]");
        assert_contains_anon_class(&mut compiler, true, "({x: class Clazz {}})");
        assert_contains_anon_class(&mut compiler, true, "x = class Clazz {};");
        assert_contains_anon_class(&mut compiler, true, "var x = class Clazz {};");
        assert_contains_anon_class(&mut compiler, true, "if (class Clazz {});");
        assert_contains_anon_class(&mut compiler, true, "while (class Clazz {});");
        assert_contains_anon_class(&mut compiler, true, "do; while (class Clazz {});");
        assert_contains_anon_class(&mut compiler, true, "for (class Clazz {};;);");
        assert_contains_anon_class(&mut compiler, true, "for (;class Clazz {};);");
        assert_contains_anon_class(&mut compiler, true, "for (;;class Clazz {});");
        assert_contains_anon_class(&mut compiler, true, "for (p in class Clazz {});");
        assert_contains_anon_class(&mut compiler, true, "with (class Clazz {}) {}");
        assert_contains_anon_class(&mut compiler, false, "class Clazz {}");
        assert_contains_anon_class(&mut compiler, false, "if (x) class Clazz {};");
        assert_contains_anon_class(&mut compiler, false, "if (x) { class Clazz {} }");
        assert_contains_anon_class(&mut compiler, false, "if (x); else class Clazz {};");
        assert_contains_anon_class(&mut compiler, false, "while (x) class Clazz {};");
        assert_contains_anon_class(&mut compiler, false, "do class Clazz {} while (0);");
        assert_contains_anon_class(&mut compiler, false, "for (;;) class Clazz {}");
        assert_contains_anon_class(&mut compiler, false, "for (p in o) class Clazz {};");
        assert_contains_anon_class(&mut compiler, false, "with (x) class Clazz {}");
        assert_contains_anon_class(&mut compiler, true, "export default class {};");
        assert_contains_anon_class(&mut compiler, false, "export default class Clazz {};");
        assert_contains_anon_class(&mut compiler, false, "export class Clazz {};");
    }

    // port: NodeUtilTest.AssortedTests#testGetNodeTypeReferenceCount
    #[test]
    fn test_get_node_type_reference_count() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "function foo(){}");
        assert_eq!(
            NodeUtil::get_node_type_reference_count(&compiler, value_1, Token::THIS, &|_, _| true),
            0
        );
        let value_2 = parse(&mut compiler, "this");
        assert_eq!(
            NodeUtil::get_node_type_reference_count(&compiler, value_2, Token::THIS, &|_, _| true),
            1
        );
        let value_3 = parse(&mut compiler, "this;function foo(){}(this)");
        assert_eq!(
            NodeUtil::get_node_type_reference_count(&compiler, value_3, Token::THIS, &|_, _| true),
            2
        );
    }

    // port: NodeUtilTest.AssortedTests#testIsNameReferenceCount
    #[test]
    fn test_is_name_reference_count() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "function foo(){}");
        assert!(NodeUtil::is_name_referenced(&compiler, value_1, "foo"));
        let value_2 = parse(&mut compiler, "var foo = function(){}");
        assert!(NodeUtil::is_name_referenced(&compiler, value_2, "foo"));
        let value_3 = parse(&mut compiler, "function foo(){}");
        assert!(!NodeUtil::is_name_referenced(
            &compiler,
            value_3,
            "undefined"
        ));
        let value_4 = parse(&mut compiler, "undefined");
        assert!(NodeUtil::is_name_referenced(
            &compiler,
            value_4,
            "undefined"
        ));
        let value_5 = parse(&mut compiler, "undefined;function foo(){}(undefined)");
        assert!(NodeUtil::is_name_referenced(
            &compiler,
            value_5,
            "undefined"
        ));
        let value_6 = parse(&mut compiler, "goo.foo");
        assert!(NodeUtil::is_name_referenced(&compiler, value_6, "goo"));
        let value_7 = parse(&mut compiler, "goo.foo");
        assert!(!NodeUtil::is_name_referenced(&compiler, value_7, "foo"));
    }

    // port: NodeUtilTest.AssortedTests#testGetNameReferenceCount
    #[test]
    fn test_get_name_reference_count() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "function foo(){}");
        assert_eq!(
            NodeUtil::get_name_reference_count(&compiler, value_1, "undefined"),
            0
        );
        let value_2 = parse(&mut compiler, "undefined");
        assert_eq!(
            NodeUtil::get_name_reference_count(&compiler, value_2, "undefined"),
            1
        );
        let value_3 = parse(&mut compiler, "undefined;function foo(){}(undefined)");
        assert_eq!(
            NodeUtil::get_name_reference_count(&compiler, value_3, "undefined"),
            2
        );
        let value_4 = parse(&mut compiler, "goo.foo");
        assert_eq!(
            NodeUtil::get_name_reference_count(&compiler, value_4, "goo"),
            1
        );
        let value_5 = parse(&mut compiler, "goo.foo");
        assert_eq!(
            NodeUtil::get_name_reference_count(&compiler, value_5, "foo"),
            0
        );
        let value_6 = parse(&mut compiler, "function foo(){}");
        assert_eq!(
            NodeUtil::get_name_reference_count(&compiler, value_6, "foo"),
            1
        );
        let value_7 = parse(&mut compiler, "var foo = function(){}");
        assert_eq!(
            NodeUtil::get_name_reference_count(&compiler, value_7, "foo"),
            1
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetVarsDeclaredInBranch
    #[test]
    fn test_get_vars_declared_in_branch() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var foo;");
        let value_2 = NodeUtil::get_vars_declared_in_branch(&mut compiler, value_1);
        assert_node_names(&compiler, vec!["foo"], value_2);
        let value_3 = parse(&mut compiler, "var foo,goo;");
        let value_4 = NodeUtil::get_vars_declared_in_branch(&mut compiler, value_3);
        assert_node_names(&compiler, vec!["foo", "goo"], value_4);
        let value_5 = parse(&mut compiler, "foo();");
        let value_6 = NodeUtil::get_vars_declared_in_branch(&mut compiler, value_5);
        assert_node_names(&compiler, vec![], value_6);
        let value_7 = parse(&mut compiler, "function f(){var foo;}");
        let value_8 = NodeUtil::get_vars_declared_in_branch(&mut compiler, value_7);
        assert_node_names(&compiler, vec![], value_8);
        let value_9 = parse(&mut compiler, "var goo;function f(){var foo;}");
        let value_10 = NodeUtil::get_vars_declared_in_branch(&mut compiler, value_9);
        assert_node_names(&compiler, vec!["goo"], value_10);
    }

    // port: NodeUtilTest.AssortedTests#testIsNonlocalModuleExportNameOnExports1
    #[test]
    fn test_is_nonlocal_module_export_name_on_exports1() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "export {localName as exportName};");
        let module_body = root.get_first_child(&compiler);
        let export_node = module_body.unwrap().get_first_child(&compiler);
        let export_specs = export_node.unwrap().get_first_child(&compiler);
        let export_spec = export_specs.unwrap().get_first_child(&compiler);
        let local_name = export_spec.unwrap().get_first_child(&compiler);
        let export_name = export_spec.unwrap().get_second_child(&compiler);
        assert!(!NodeUtil::is_nonlocal_module_export_name(
            &compiler,
            local_name.unwrap()
        ));
        assert!(NodeUtil::is_nonlocal_module_export_name(
            &compiler,
            export_name.unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsNonlocalModuleExportNameOnExports2
    #[test]
    fn test_is_nonlocal_module_export_name_on_exports2() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "let bar; export {bar};");
        let module_body = root.get_first_child(&compiler);
        let export_node = module_body.unwrap().get_second_child(&compiler);
        let export_specs = export_node.unwrap().get_first_child(&compiler);
        let export_spec = export_specs.unwrap().get_first_child(&compiler);
        let name = export_spec.unwrap().get_first_child(&compiler);
        assert!(!NodeUtil::is_nonlocal_module_export_name(
            &compiler,
            name.unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsNonlocalModuleExportNameOnExportFrom
    #[test]
    fn test_is_nonlocal_module_export_name_on_export_from() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "export {bar as baz} from './foo.js';");
        let module_body = root.get_first_child(&compiler);
        let export_node = module_body.unwrap().get_first_child(&compiler);
        let export_specs = export_node.unwrap().get_first_child(&compiler);
        let export_spec = export_specs.unwrap().get_first_child(&compiler);
        let bar = export_spec.unwrap().get_first_child(&compiler);
        let baz = export_spec.unwrap().get_second_child(&compiler);
        assert!(NodeUtil::is_nonlocal_module_export_name(
            &compiler,
            bar.unwrap()
        ));
        assert!(NodeUtil::is_nonlocal_module_export_name(
            &compiler,
            baz.unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsNonlocalModuleExportNameOnImports1
    #[test]
    fn test_is_nonlocal_module_export_name_on_imports1() {
        let mut compiler = Compiler::new();
        let root = parse(
            &mut compiler,
            "import {exportName as localName} from './foo.js';",
        );
        let module_body = root.get_first_child(&compiler);
        let import_node = module_body.unwrap().get_first_child(&compiler);
        let import_specs = import_node.unwrap().get_second_child(&compiler);
        let import_spec = import_specs.unwrap().get_first_child(&compiler);
        let export_name = import_spec.unwrap().get_first_child(&compiler);
        let local_name = import_spec.unwrap().get_second_child(&compiler);
        assert!(NodeUtil::is_nonlocal_module_export_name(
            &compiler,
            export_name.unwrap()
        ));
        assert!(!NodeUtil::is_nonlocal_module_export_name(
            &compiler,
            local_name.unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsNonlocalModuleExportNameOnImports2
    #[test]
    fn test_is_nonlocal_module_export_name_on_imports2() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "import {bar} from './foo.js';");
        let module_body = root.get_first_child(&compiler);
        let import_node = module_body.unwrap().get_first_child(&compiler);
        let import_specs = import_node.unwrap().get_second_child(&compiler);
        let import_spec = import_specs.unwrap().get_first_child(&compiler);
        let name = import_spec.unwrap().get_second_child(&compiler);
        assert!(!NodeUtil::is_nonlocal_module_export_name(
            &compiler,
            name.unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsControlStructureCodeBlock
    #[test]
    fn test_is_control_structure_code_block() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "if (x) foo(); else boo();");
        let if_node = root.get_first_child(&compiler);
        let if_condition = if_node.unwrap().get_first_child(&compiler);
        let if_case = if_node.unwrap().get_second_child(&compiler);
        let else_case = if_node.unwrap().get_last_child(&compiler);
        assert!(!NodeUtil::is_control_structure_code_block(
            &compiler,
            if_node.unwrap(),
            if_condition.unwrap()
        ));
        assert!(NodeUtil::is_control_structure_code_block(
            &compiler,
            if_node.unwrap(),
            if_case.unwrap()
        ));
        assert!(NodeUtil::is_control_structure_code_block(
            &compiler,
            if_node.unwrap(),
            else_case.unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsFunctionExpression1
    #[test]
    fn test_is_function_expression1() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "(function foo() {})");
        let statement_node = root.get_first_child(&compiler);
        assert!(statement_node.unwrap().is_expr_result(&compiler));
        let function_node = statement_node.unwrap().get_first_child(&compiler);
        assert!(function_node.unwrap().is_function(&compiler));
        assert!(NodeUtil::is_function_expression(
            &compiler,
            function_node.unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsFunctionExpression2
    #[test]
    fn test_is_function_expression2() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "function foo() {}");
        let function_node = root.get_first_child(&compiler);
        assert!(function_node.unwrap().is_function(&compiler));
        assert!(!NodeUtil::is_function_expression(
            &compiler,
            function_node.unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testRemoveChildBlock
    #[test]
    fn test_remove_child_block() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "{{x()}}");
        let outer_block_node = actual.get_first_child(&compiler);
        let inner_block_node = outer_block_node.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(
            &mut compiler,
            outer_block_node.unwrap(),
            inner_block_node.unwrap(),
        );
        let expected = "{{}}";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveTryChild1
    #[test]
    fn test_remove_try_child1() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "try {foo()} catch(e) {} finally {}");
        let try_node = actual.get_first_child(&compiler);
        let finally_block = try_node.unwrap().get_last_child(&compiler);
        NodeUtil::remove_child(&mut compiler, try_node.unwrap(), finally_block.unwrap());
        let expected = "try {foo()} catch(e) {}";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveTryChild2
    #[test]
    fn test_remove_try_child2() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "try {foo()} catch(e) {} finally {}");
        let try_node = actual.get_first_child(&compiler);
        let try_block = try_node.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, try_node.unwrap(), try_block.unwrap());
        let expected = "try {} catch(e) {} finally {}";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveTryChild3
    #[test]
    fn test_remove_try_child3() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "try {foo()} catch(e) {} finally {}");
        let try_node = actual.get_first_child(&compiler);
        let catch_blocks = try_node.unwrap().get_second_child(&compiler);
        let catch_block = catch_blocks.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, catch_blocks.unwrap(), catch_block.unwrap());
        let expected = "try {foo()} finally {}";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveTryChild4
    #[test]
    fn test_remove_try_child4() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "try {foo()} catch(e) {} finally {}");
        let try_node = actual.get_first_child(&compiler);
        let catch_blocks = try_node.unwrap().get_second_child(&compiler);
        NodeUtil::remove_child(&mut compiler, try_node.unwrap(), catch_blocks.unwrap());
        let expected = "try {foo()} finally {}";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveFromImport
    #[test]
    fn test_remove_from_import() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "import foo from './foo';");
        let module_body = actual.get_first_child(&compiler);
        let import_node = module_body.unwrap().get_first_child(&compiler);
        let function_foo = import_node.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, import_node.unwrap(), function_foo.unwrap());
        let expected = "import './foo';";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveParamChild1
    #[test]
    fn test_remove_param_child1() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "function f(p1) {}");
        let function_node = actual.get_first_child(&compiler);
        let param_list = function_node
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_next(&compiler);
        let p1 = param_list.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, param_list.unwrap(), p1.unwrap());
        let expected = "function f() {}";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveParamChild2
    #[test]
    fn test_remove_param_child2() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "function f(p1 = 0, p2) {}");
        let function_node = actual.get_first_child(&compiler);
        let param_list = function_node
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_next(&compiler);
        let p1 = param_list.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, param_list.unwrap(), p1.unwrap());
        let expected = "function f(p2) {}";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveVarChild
    #[test]
    fn test_remove_var_child() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "var foo, goo, hoo");
        let var_node = actual.get_first_child(&compiler);
        let name_node = var_node.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, var_node.unwrap(), name_node.unwrap());
        let expected = "var goo, hoo";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "var foo, goo, hoo");
        let var_node = actual.get_first_child(&compiler);
        let name_node = var_node.unwrap().get_second_child(&compiler);
        NodeUtil::remove_child(&mut compiler, var_node.unwrap(), name_node.unwrap());
        let expected = "var foo, hoo";
        let value_6 = parse(&mut compiler, expected);
        assert!(
            value_6.is_equivalent_to(&compiler, actual),
            "{}",
            value_6.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "var foo, hoo");
        let var_node = actual.get_first_child(&compiler);
        let name_node = var_node.unwrap().get_second_child(&compiler);
        NodeUtil::remove_child(&mut compiler, var_node.unwrap(), name_node.unwrap());
        let expected = "var foo";
        let value_9 = parse(&mut compiler, expected);
        assert!(
            value_9.is_equivalent_to(&compiler, actual),
            "{}",
            value_9.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "var hoo");
        let var_node = actual.get_first_child(&compiler);
        let name_node = var_node.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, var_node.unwrap(), name_node.unwrap());
        let expected = "";
        let value_12 = parse(&mut compiler, expected);
        assert!(
            value_12.is_equivalent_to(&compiler, actual),
            "{}",
            value_12.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveLetChild
    #[test]
    fn test_remove_let_child() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "let foo, goo, hoo");
        let let_node = actual.get_first_child(&compiler);
        let name_node = let_node.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, let_node.unwrap(), name_node.unwrap());
        let expected = "let goo, hoo";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "let foo, goo, hoo");
        let let_node = actual.get_first_child(&compiler);
        let name_node = let_node.unwrap().get_second_child(&compiler);
        NodeUtil::remove_child(&mut compiler, let_node.unwrap(), name_node.unwrap());
        let expected = "let foo, hoo";
        let value_6 = parse(&mut compiler, expected);
        assert!(
            value_6.is_equivalent_to(&compiler, actual),
            "{}",
            value_6.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "let foo, hoo");
        let let_node = actual.get_first_child(&compiler);
        let name_node = let_node.unwrap().get_second_child(&compiler);
        NodeUtil::remove_child(&mut compiler, let_node.unwrap(), name_node.unwrap());
        let expected = "let foo";
        let value_9 = parse(&mut compiler, expected);
        assert!(
            value_9.is_equivalent_to(&compiler, actual),
            "{}",
            value_9.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "let hoo");
        let let_node = actual.get_first_child(&compiler);
        let name_node = let_node.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, let_node.unwrap(), name_node.unwrap());
        let expected = "";
        let value_12 = parse(&mut compiler, expected);
        assert!(
            value_12.is_equivalent_to(&compiler, actual),
            "{}",
            value_12.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveLabelChild1
    #[test]
    fn test_remove_label_child1() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "foo: goo()");
        let label_node = actual.get_first_child(&compiler);
        let call_express_node = label_node.unwrap().get_last_child(&compiler);
        NodeUtil::remove_child(
            &mut compiler,
            label_node.unwrap(),
            call_express_node.unwrap(),
        );
        let expected = "";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveLabelChild2
    #[test]
    fn test_remove_label_child2() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "achoo: foo: goo()");
        let label_node = actual.get_first_child(&compiler);
        let call_express_node = label_node.unwrap().get_last_child(&compiler);
        NodeUtil::remove_child(
            &mut compiler,
            label_node.unwrap(),
            call_express_node.unwrap(),
        );
        let expected = "";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemovePatternChild
    #[test]
    fn test_remove_pattern_child() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "var {a, b, c} = {a:1, b:2, c:3}");
        let var_node = actual.get_first_child(&compiler);
        let destructure = var_node.unwrap().get_first_child(&compiler);
        let pattern = destructure.unwrap().get_first_child(&compiler);
        let a = pattern.unwrap().get_first_child(&compiler);
        let b = a.unwrap().get_next(&compiler);
        let c = b.unwrap().get_next(&compiler);
        NodeUtil::remove_child(&mut compiler, pattern.unwrap(), a.unwrap());
        let expected = "var {b, c} = {a:1, b:2, c:3};";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
        NodeUtil::remove_child(&mut compiler, pattern.unwrap(), b.unwrap());
        NodeUtil::remove_child(&mut compiler, pattern.unwrap(), c.unwrap());
        let expected = "var { } = {a:1, b:2, c:3};";
        let value_6 = parse(&mut compiler, expected);
        assert!(
            value_6.is_equivalent_to(&compiler, actual),
            "{}",
            value_6.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "var [a, b] = [1, 2]");
        let var_node = actual.get_first_child(&compiler);
        let destructure = var_node.unwrap().get_first_child(&compiler);
        let pattern = destructure.unwrap().get_first_child(&compiler);
        let a = pattern.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, pattern.unwrap(), a.unwrap());
        let expected = "var [ , b] = [1, 2];";
        let value_9 = parse(&mut compiler, expected);
        assert!(
            value_9.is_equivalent_to(&compiler, actual),
            "{}",
            value_9.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testRemoveForChild
    #[test]
    fn test_remove_for_child() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "for(var a=0;a<0;a++)foo()");
        let for_node = actual.get_first_child(&compiler);
        let child = for_node.unwrap().get_first_child(&compiler);
        NodeUtil::remove_child(&mut compiler, for_node.unwrap(), child.unwrap());
        let expected = "for(;a<0;a++)foo()";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "for(var a=0;a<0;a++)foo()");
        let for_node = actual.get_first_child(&compiler);
        let child = for_node.unwrap().get_second_child(&compiler);
        NodeUtil::remove_child(&mut compiler, for_node.unwrap(), child.unwrap());
        let expected = "for(var a=0;;a++)foo()";
        let value_6 = parse(&mut compiler, expected);
        assert!(
            value_6.is_equivalent_to(&compiler, actual),
            "{}",
            value_6.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "for(var a=0;a<0;a++)foo()");
        let for_node = actual.get_first_child(&compiler);
        let child = for_node
            .unwrap()
            .get_second_child(&compiler)
            .unwrap()
            .get_next(&compiler);
        NodeUtil::remove_child(&mut compiler, for_node.unwrap(), child.unwrap());
        let expected = "for(var a=0;a<0;)foo()";
        let value_9 = parse(&mut compiler, expected);
        assert!(
            value_9.is_equivalent_to(&compiler, actual),
            "{}",
            value_9.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "for(var a=0;a<0;a++)foo()");
        let for_node = actual.get_first_child(&compiler);
        let child = for_node.unwrap().get_last_child(&compiler);
        NodeUtil::remove_child(&mut compiler, for_node.unwrap(), child.unwrap());
        let expected = "for(var a=0;a<0;a++);";
        let value_12 = parse(&mut compiler, expected);
        assert!(
            value_12.is_equivalent_to(&compiler, actual),
            "{}",
            value_12.to_string_tree(&compiler)
        );
        let actual = parse(&mut compiler, "for(a in ack)foo();");
        let for_node = actual.get_first_child(&compiler);
        let child = for_node.unwrap().get_last_child(&compiler);
        NodeUtil::remove_child(&mut compiler, for_node.unwrap(), child.unwrap());
        let expected = "for(a in ack);";
        let value_15 = parse(&mut compiler, expected);
        assert!(
            value_15.is_equivalent_to(&compiler, actual),
            "{}",
            value_15.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testReplaceDeclarationName
    #[test]
    fn test_replace_declaration_name() {
        let mut compiler = Compiler::new();
        replace_decl_child(&mut compiler, "var x;", 0, "{}");
        replace_decl_child(&mut compiler, "var x, y;", 0, "{} var y;");
        replace_decl_child(&mut compiler, "var x, y;", 1, "var x; {}");
        replace_decl_child(&mut compiler, "let x, y, z;", 0, "{} let y, z;");
        replace_decl_child(&mut compiler, "let x, y, z;", 1, "let x; {} let z;");
        replace_decl_child(&mut compiler, "let x, y, z;", 2, "let x, y; {}");
        replace_decl_child(
            &mut compiler,
            "const x = 1, y = 2, z = 3;",
            1,
            "const x = 1; {} const z = 3;",
        );
        replace_decl_child(
            &mut compiler,
            "const x =1, y = 2, z = 3, w = 4;",
            1,
            "const x = 1; {} const z = 3, w = 4;",
        );
    }

    // port: NodeUtilTest.AssortedTests#testTryMergeBlock1
    #[test]
    fn test_try_merge_block1() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "{{a();b();}}");
        let parent_block = actual.get_first_child(&compiler);
        let child_block = parent_block.unwrap().get_first_child(&compiler);
        let value_2 = NodeUtil::try_merge_block(&mut compiler, child_block.unwrap(), false);
        assert!(value_2);
        let expected = "{a();b();}";
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testTryMergeBlock2
    #[test]
    fn test_try_merge_block2() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "foo:{a();}");
        let parent_label = actual.get_first_child(&compiler);
        let child_block = parent_label.unwrap().get_last_child(&compiler);
        let value_2 = NodeUtil::try_merge_block(&mut compiler, child_block.unwrap(), false);
        assert!(!value_2);
    }

    // port: NodeUtilTest.AssortedTests#testTryMergeBlock3
    #[test]
    fn test_try_merge_block3() {
        let mut compiler = Compiler::new();
        let code = "foo:{a();boo()}";
        let actual = parse(&mut compiler, "foo:{a();boo()}");
        let parent_label = actual.get_first_child(&compiler);
        let child_block = parent_label.unwrap().get_last_child(&compiler);
        let value_2 = NodeUtil::try_merge_block(&mut compiler, child_block.unwrap(), false);
        assert!(!value_2);
        let expected = code;
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testTryMergeBlock4
    #[test]
    fn test_try_merge_block4() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "{const module$exports$Foo=class{}}");
        let expected = "const module$exports$Foo=class{}";
        let block = actual.get_first_child(&compiler);
        let value_2 = NodeUtil::try_merge_block(&mut compiler, block.unwrap(), true);
        assert!(value_2);
        let value_3 = parse(&mut compiler, expected);
        assert!(
            value_3.is_equivalent_to(&compiler, actual),
            "{}",
            value_3.to_string_tree(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testCanMergeBlock1
    #[test]
    fn test_can_merge_block1() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "{a(); let x;}");
        let block = actual.get_first_child(&compiler);
        assert!(!NodeUtil::can_merge_block(&compiler, block.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testCanMergeBlock2
    #[test]
    fn test_can_merge_block2() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "{a(); f(); var x; {const y = 2;}}");
        let parent_block = actual.get_first_child(&compiler);
        let child_block = parent_block.unwrap().get_last_child(&compiler);
        assert!(NodeUtil::can_merge_block(&compiler, parent_block.unwrap()));
        assert!(!NodeUtil::can_merge_block(&compiler, child_block.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testLocalValue1
    #[test]
    fn test_local_value1() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "x");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "x()");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "this");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "arguments");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "new x()");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "(new x()).y");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "(new x())['y']");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "null");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "undefined");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "Infinity");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "NaN");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "1");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_12));
        let value_13 = parse_expr(&mut compiler, "'a'");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_13));
        let value_14 = parse_expr(&mut compiler, "true");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_14));
        let value_15 = parse_expr(&mut compiler, "false");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_15));
        let value_16 = parse_expr(&mut compiler, "[]");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_16));
        let value_17 = parse_expr(&mut compiler, "{}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_17));
        let value_18 = parse_expr(&mut compiler, "[x]");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_18));
        let value_19 = parse_expr(&mut compiler, "{'a':x}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_19));
        let value_20 = parse_expr(&mut compiler, "{'a': {'b': 2}}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_20));
        let value_21 = parse_expr(&mut compiler, "{'a': {'b': global}}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_21));
        let value_22 = parse_expr(&mut compiler, "{get someGetter() { return 1; }}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_22));
        let value_23 = parse_expr(&mut compiler, "{get someGetter() { return global; }}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_23));
        let value_24 = parse_expr(&mut compiler, "{set someSetter(value) {}}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_24));
        let value_25 = parse_expr(&mut compiler, "{[someComputedProperty]: {}}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_25));
        let value_26 = parse_expr(&mut compiler, "{[someComputedProperty]: global}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_26));
        let value_27 = parse_expr(&mut compiler, "{[someComputedProperty]: {'a':x}}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_27));
        let value_28 = parse_expr(&mut compiler, "{[someComputedProperty]: {'a':1}}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_28));
        let value_29 = parse_expr(&mut compiler, "++x");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_29));
        let value_30 = parse_expr(&mut compiler, "--x");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_30));
        let value_31 = parse_expr(&mut compiler, "x++");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_31));
        let value_32 = parse_expr(&mut compiler, "x--");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_32));
        let value_33 = parse_expr(&mut compiler, "x=1");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_33));
        let value_34 = parse_expr(&mut compiler, "x=[]");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_34));
        let value_35 = parse_expr(&mut compiler, "x=y");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_35));
        let value_36 = parse_expr(&mut compiler, "x+=y");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_36));
        let value_37 = parse_expr(&mut compiler, "x*=y");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_37));
        let value_38 = parse_expr(&mut compiler, "x==y");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_38));
        let value_39 = parse_expr(&mut compiler, "x!=y");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_39));
        let value_40 = parse_expr(&mut compiler, "x>y");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_40));
        let value_41 = parse_expr(&mut compiler, "(1,2)");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_41));
        let value_42 = parse_expr(&mut compiler, "(x,1)");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_42));
        let value_43 = parse_expr(&mut compiler, "(x,y)");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_43));
        let value_44 = parse_expr(&mut compiler, "1||2");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_44));
        let value_45 = parse_expr(&mut compiler, "x||1");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_45));
        let value_46 = parse_expr(&mut compiler, "x||y");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_46));
        let value_47 = parse_expr(&mut compiler, "1||y");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_47));
        let value_48 = parse_expr(&mut compiler, "1&&2");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_48));
        let value_49 = parse_expr(&mut compiler, "x&&1");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_49));
        let value_50 = parse_expr(&mut compiler, "x&&y");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_50));
        let value_51 = parse_expr(&mut compiler, "1&&y");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_51));
        let value_52 = parse_expr(&mut compiler, "x||=1");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_52));
        let value_53 = parse_expr(&mut compiler, "x&&=1");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_53));
        let value_54 = parse_expr(&mut compiler, "x??=1");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_54));
        let value_55 = parse_expr(&mut compiler, "x?1:2");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_55));
        let value_56 = parse_expr(&mut compiler, "x?x:2");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_56));
        let value_57 = parse_expr(&mut compiler, "x?1:x");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_57));
        let value_58 = parse_expr(&mut compiler, "x?x:y");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_58));
        let value_59 = parse_expr(&mut compiler, "!y");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_59));
        let value_60 = parse_expr(&mut compiler, "~y");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_60));
        let value_61 = parse_expr(&mut compiler, "y + 1");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_61));
        let value_62 = parse_expr(&mut compiler, "y + z");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_62));
        let value_63 = parse_expr(&mut compiler, "y * z");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_63));
        let value_64 = parse_expr(&mut compiler, "'a' in x");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_64));
        let value_65 = parse_expr(&mut compiler, "typeof x");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_65));
        let value_66 = parse_expr(&mut compiler, "x instanceof y");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_66));
        let value_67 = parse_expr(&mut compiler, "void x");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_67));
        let value_68 = parse_expr(&mut compiler, "void 0");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_68));
        let value_69 = parse_expr(&mut compiler, "{}.x");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_69));
        let value_70 = parse_expr(&mut compiler, "{}.toString()");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_70));
        let value_71 = parse_expr(&mut compiler, "o.toString()");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_71));
        let value_72 = parse_expr(&mut compiler, "o.valueOf()");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_72));
        let value_73 = parse_expr(&mut compiler, "delete a.b");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_73));
        let value_74 = parse_first(
            &mut compiler,
            Token::NEW_TARGET,
            "function f() { new.target; }",
        );
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_74));
        let value_75 = parse_expr(&mut compiler, "import('./foo.js')");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_75));
    }

    // port: NodeUtilTest.AssortedTests#testLocalValueTemplateLit
    #[test]
    fn test_local_value_template_lit() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "`hello`");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "`hello ${name}`");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "`${'name'}`");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "`${'name'}`");
        assert!(NodeUtil::is_literal_value(&compiler, value_4, false));
        let value_5 = parse_expr(&mut compiler, "`${'name'}`");
        assert!(NodeUtil::is_immutable_value(&compiler, value_5));
    }

    // port: NodeUtilTest.AssortedTests#testLocalValueTaggedTemplateLit1
    #[test]
    fn test_local_value_tagged_template_lit1() {
        let mut compiler = Compiler::new();
        let n = parse_expr(&mut compiler, "tag`simple string`");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, n));
        let mut flags = SideEffectFlags::new();
        flags.clear_all_flags();
        n.set_side_effect_flags_from_flags(&mut compiler, &flags);
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, n));
    }

    // port: NodeUtilTest.AssortedTests#testLocalValueTaggedTemplateLit2
    #[test]
    fn test_local_value_tagged_template_lit2() {
        let mut compiler = Compiler::new();
        let n = parse_expr(&mut compiler, "tag`string with ${replacement()}`");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, n));
        let mut flags = SideEffectFlags::new();
        flags.clear_all_flags();
        n.set_side_effect_flags_from_flags(&mut compiler, &flags);
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, n));
    }

    // port: NodeUtilTest.AssortedTests#testLocalValueNewExpr
    #[test]
    fn test_local_value_new_expr() {
        let mut compiler = Compiler::new();
        let new_expr = parse_expr(&mut compiler, "new x()");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, new_expr));
        assert!(new_expr.is_new(&compiler));
        let mut flags = SideEffectFlags::new();
        flags.clear_all_flags();
        new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
        assert!(NodeUtil::evaluates_to_local_value(&compiler, new_expr));
        flags.clear_all_flags();
        flags.set_mutates_this();
        new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
        assert!(NodeUtil::evaluates_to_local_value(&compiler, new_expr));
        flags.clear_all_flags();
        new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
        assert!(NodeUtil::evaluates_to_local_value(&compiler, new_expr));
        flags.clear_all_flags();
        flags.set_throws();
        new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, new_expr));
        flags.clear_all_flags();
        flags.set_mutates_arguments();
        new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, new_expr));
        flags.clear_all_flags();
        flags.set_mutates_global_state();
        new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, new_expr));
    }

    // port: NodeUtilTest.AssortedTests#testLocalValueSpread
    #[test]
    fn test_local_value_spread() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "[...x]");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "[...x]");
        assert!(!NodeUtil::is_literal_value(&compiler, value_2, false));
        let value_3 = parse_expr(&mut compiler, "[...x]");
        assert!(!NodeUtil::is_immutable_value(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "{...x}");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "{...x}");
        assert!(!NodeUtil::is_literal_value(&compiler, value_5, false));
        let value_6 = parse_expr(&mut compiler, "{...x}");
        assert!(!NodeUtil::is_immutable_value(&compiler, value_6));
    }

    // port: NodeUtilTest.AssortedTests#testLocalValueAwait
    #[test]
    fn test_local_value_await() {
        let mut compiler = Compiler::new();
        let expr = parse_first(
            &mut compiler,
            Token::AWAIT,
            "async function f() { await someAsyncAction(); }",
        );
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, expr));
        let expr = parse_first(
            &mut compiler,
            Token::AWAIT,
            "async function f() { await {then:function() { return p }}; }",
        );
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, expr));
        let expr = parse_first(
            &mut compiler,
            Token::AWAIT,
            "async function f() { await 5; }",
        );
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, expr));
    }

    // port: NodeUtilTest.AssortedTests#testLocalValueYield
    #[test]
    fn test_local_value_yield() {
        let mut compiler = Compiler::new();
        let expr = parse_first(&mut compiler, Token::YIELD, "function *f() { yield; }");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, expr));
        let expr = parse_first(
            &mut compiler,
            Token::YIELD,
            "function *f() { yield 'something'; }",
        );
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, expr));
    }

    // port: NodeUtilTest.AssortedTests#localValueOptChainGetProp
    #[test]
    fn local_value_opt_chain_get_prop() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "x?.y");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_1));
    }

    // port: NodeUtilTest.AssortedTests#localValueOptChainGetElem
    #[test]
    fn local_value_opt_chain_get_elem() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "x?.[y]");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_1));
    }

    // port: NodeUtilTest.AssortedTests#localValueOptChainCall
    #[test]
    fn local_value_opt_chain_call() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "x?.()");
        assert!(!NodeUtil::evaluates_to_local_value(&compiler, value_1));
    }

    // port: NodeUtilTest.AssortedTests#localValueOptChainCall_toString
    #[test]
    fn local_value_opt_chain_call_to_string() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "x?.toString()");
        assert!(NodeUtil::evaluates_to_local_value(&compiler, value_1));
    }

    // port: NodeUtilTest.AssortedTests#testGetOctalNumberValue
    #[test]
    fn test_get_octal_number_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "022");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_1)
                .unwrap()
                .to_bits(),
            (18.0_f64).to_bits()
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetNumberValue
    #[test]
    fn test_get_number_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "'\\uFEFF1'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_1)
                .unwrap()
                .to_bits(),
            (1.0_f64).to_bits()
        );
        let value_2 = parse_expr(&mut compiler, "''");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_2)
                .unwrap()
                .to_bits(),
            (0.0_f64).to_bits()
        );
        let value_3 = parse_expr(&mut compiler, "' '");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_3)
                .unwrap()
                .to_bits(),
            (0.0_f64).to_bits()
        );
        let value_4 = parse_expr(&mut compiler, "' \\t'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_4)
                .unwrap()
                .to_bits(),
            (0.0_f64).to_bits()
        );
        let value_5 = parse_expr(&mut compiler, "'+0'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_5)
                .unwrap()
                .to_bits(),
            (0.0_f64).to_bits()
        );
        let value_6 = parse_expr(&mut compiler, "'-0'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_6)
                .unwrap()
                .to_bits(),
            (-0.0_f64).to_bits()
        );
        let value_7 = parse_expr(&mut compiler, "'+2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_7)
                .unwrap()
                .to_bits(),
            (2.0_f64).to_bits()
        );
        let value_8 = parse_expr(&mut compiler, "'-1.6'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_8)
                .unwrap()
                .to_bits(),
            (-1.6_f64).to_bits()
        );
        let value_9 = parse_expr(&mut compiler, "'16'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_9)
                .unwrap()
                .to_bits(),
            (16.0_f64).to_bits()
        );
        let value_10 = parse_expr(&mut compiler, "' 16 '");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_10)
                .unwrap()
                .to_bits(),
            (16.0_f64).to_bits()
        );
        let value_11 = parse_expr(&mut compiler, "'123e2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_11)
                .unwrap()
                .to_bits(),
            (12300.0_f64).to_bits()
        );
        let value_12 = parse_expr(&mut compiler, "'123E2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_12)
                .unwrap()
                .to_bits(),
            (12300.0_f64).to_bits()
        );
        let value_13 = parse_expr(&mut compiler, "'123e-2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_13)
                .unwrap()
                .to_bits(),
            (1.23_f64).to_bits()
        );
        let value_14 = parse_expr(&mut compiler, "'123E-2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_14)
                .unwrap()
                .to_bits(),
            (1.23_f64).to_bits()
        );
        let value_15 = parse_expr(&mut compiler, "'-123e-2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_15)
                .unwrap()
                .to_bits(),
            (-1.23_f64).to_bits()
        );
        let value_16 = parse_expr(&mut compiler, "'-123E-2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_16)
                .unwrap()
                .to_bits(),
            (-1.23_f64).to_bits()
        );
        let value_17 = parse_expr(&mut compiler, "'+123e-2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_17)
                .unwrap()
                .to_bits(),
            (1.23_f64).to_bits()
        );
        let value_18 = parse_expr(&mut compiler, "'+123E-2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_18)
                .unwrap()
                .to_bits(),
            (1.23_f64).to_bits()
        );
        let value_19 = parse_expr(&mut compiler, "'+123e+2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_19)
                .unwrap()
                .to_bits(),
            (12300.0_f64).to_bits()
        );
        let value_20 = parse_expr(&mut compiler, "'+123E+2'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_20)
                .unwrap()
                .to_bits(),
            (12300.0_f64).to_bits()
        );
        let value_21 = parse_expr(&mut compiler, "'0xf'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_21)
                .unwrap()
                .to_bits(),
            (15.0_f64).to_bits()
        );
        let value_22 = parse_expr(&mut compiler, "'0xF'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_22)
                .unwrap()
                .to_bits(),
            (15.0_f64).to_bits()
        );
        let value_23 = parse_expr(&mut compiler, "'-0xf'");
        assert!(NodeUtil::get_number_value(&compiler, value_23).is_none());
        let value_24 = parse_expr(&mut compiler, "'-0xF'");
        assert!(NodeUtil::get_number_value(&compiler, value_24).is_none());
        let value_25 = parse_expr(&mut compiler, "'+0xf'");
        assert!(NodeUtil::get_number_value(&compiler, value_25).is_none());
        let value_26 = parse_expr(&mut compiler, "'+0xF'");
        assert!(NodeUtil::get_number_value(&compiler, value_26).is_none());
        let value_27 = parse_expr(&mut compiler, "'0X10'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_27)
                .unwrap()
                .to_bits(),
            (16.0_f64).to_bits()
        );
        let value_28 = parse_expr(&mut compiler, "'0X10.8'");
        assert!(
            NodeUtil::get_number_value(&compiler, value_28)
                .unwrap()
                .is_nan()
        );
        let value_29 = parse_expr(&mut compiler, "'077'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_29)
                .unwrap()
                .to_bits(),
            (77.0_f64).to_bits()
        );
        let value_30 = parse_expr(&mut compiler, "'-077'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_30)
                .unwrap()
                .to_bits(),
            (-77.0_f64).to_bits()
        );
        let value_31 = parse_expr(&mut compiler, "'-077.5'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_31)
                .unwrap()
                .to_bits(),
            (-77.5_f64).to_bits()
        );
        let value_32 = parse_expr(&mut compiler, "'-Infinity'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_32).unwrap(),
            -f64::INFINITY
        );
        let value_33 = parse_expr(&mut compiler, "'Infinity'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_33).unwrap(),
            f64::INFINITY
        );
        let value_34 = parse_expr(&mut compiler, "'+Infinity'");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_34).unwrap(),
            f64::INFINITY
        );
        let value_35 = parse_expr(&mut compiler, "'-infinity'");
        assert!(NodeUtil::get_number_value(&compiler, value_35).is_none());
        let value_36 = parse_expr(&mut compiler, "'infinity'");
        assert!(NodeUtil::get_number_value(&compiler, value_36).is_none());
        let value_37 = parse_expr(&mut compiler, "'+infinity'");
        assert!(NodeUtil::get_number_value(&compiler, value_37).is_none());
        let value_38 = parse_expr(&mut compiler, "'NaN'");
        assert!(
            NodeUtil::get_number_value(&compiler, value_38)
                .unwrap()
                .is_nan()
        );
        let value_39 = parse_expr(&mut compiler, "'some unknown string'");
        assert!(
            NodeUtil::get_number_value(&compiler, value_39)
                .unwrap()
                .is_nan()
        );
        let value_40 = parse_expr(&mut compiler, "'123 blah'");
        assert!(
            NodeUtil::get_number_value(&compiler, value_40)
                .unwrap()
                .is_nan()
        );
        let value_41 = parse_expr(&mut compiler, "1");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_41)
                .unwrap()
                .to_bits(),
            (1.0_f64).to_bits()
        );
        let value_42 = parse_expr(&mut compiler, "1n");
        assert!(NodeUtil::get_number_value(&compiler, value_42).is_none());
        let value_43 = parse_expr(&mut compiler, "-1");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_43)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_44 = parse_expr(&mut compiler, "+1");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_44)
                .unwrap()
                .to_bits(),
            (1.0_f64).to_bits()
        );
        let value_45 = parse_expr(&mut compiler, "22");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_45)
                .unwrap()
                .to_bits(),
            (22.0_f64).to_bits()
        );
        let value_46 = parse_expr(&mut compiler, "022");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_46)
                .unwrap()
                .to_bits(),
            (18.0_f64).to_bits()
        );
        let value_47 = parse_expr(&mut compiler, "0x22");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_47)
                .unwrap()
                .to_bits(),
            (34.0_f64).to_bits()
        );
        let value_48 = parse_expr(&mut compiler, "-0.1");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_48)
                .unwrap()
                .to_bits(),
            (-0.1_f64).to_bits()
        );
        let value_49 = parse_expr(&mut compiler, "-0.0");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_49)
                .unwrap()
                .to_bits(),
            (-0.0_f64).to_bits()
        );
        let value_50 = parse_expr(&mut compiler, "~1");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_50)
                .unwrap()
                .to_bits(),
            (-2.0_f64).to_bits()
        );
        let value_51 = parse_expr(&mut compiler, "~-1");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_51)
                .unwrap()
                .to_bits(),
            (0.0_f64).to_bits()
        );
        let value_52 = parse_expr(&mut compiler, "~22");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_52)
                .unwrap()
                .to_bits(),
            (-23.0_f64).to_bits()
        );
        let value_53 = parse_expr(&mut compiler, "~022");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_53)
                .unwrap()
                .to_bits(),
            (-19.0_f64).to_bits()
        );
        let value_54 = parse_expr(&mut compiler, "~0.0");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_54)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_55 = parse_expr(&mut compiler, "~0.1");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_55)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_56 = parse_expr(&mut compiler, "~NaN");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_56)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_57 = parse_expr(&mut compiler, "~Infinity");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_57)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_58 = parse_expr(&mut compiler, "true");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_58)
                .unwrap()
                .to_bits(),
            (1.0_f64).to_bits()
        );
        let value_59 = parse_expr(&mut compiler, "false");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_59)
                .unwrap()
                .to_bits(),
            (0.0_f64).to_bits()
        );
        let value_60 = parse_expr(&mut compiler, "null");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_60)
                .unwrap()
                .to_bits(),
            (0.0_f64).to_bits()
        );
        let value_61 = parse_expr(&mut compiler, "void 0");
        assert!(
            NodeUtil::get_number_value(&compiler, value_61)
                .unwrap()
                .is_nan()
        );
        let value_62 = parse_expr(&mut compiler, "void f");
        assert!(
            NodeUtil::get_number_value(&compiler, value_62)
                .unwrap()
                .is_nan()
        );
        let value_63 = parse_expr(&mut compiler, "void f()");
        assert!(
            NodeUtil::get_number_value(&compiler, value_63)
                .unwrap()
                .is_nan()
        );
        let value_64 = parse_expr(&mut compiler, "NaN");
        assert!(
            NodeUtil::get_number_value(&compiler, value_64)
                .unwrap()
                .is_nan()
        );
        let value_65 = parse_expr(&mut compiler, "Infinity");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_65).unwrap(),
            f64::INFINITY
        );
        let value_66 = parse_expr(&mut compiler, "-Infinity");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_66).unwrap(),
            -f64::INFINITY
        );
        let value_67 = parse_expr(&mut compiler, "infinity");
        assert!(NodeUtil::get_number_value(&compiler, value_67).is_none());
        let value_68 = parse_expr(&mut compiler, "-infinity");
        assert!(NodeUtil::get_number_value(&compiler, value_68).is_none());
        let value_69 = parse_expr(&mut compiler, "x");
        assert!(NodeUtil::get_number_value(&compiler, value_69).is_none());
        let value_70 = parse_expr(&mut compiler, "x.y");
        assert!(NodeUtil::get_number_value(&compiler, value_70).is_none());
        let value_71 = parse_expr(&mut compiler, "1/2");
        assert!(NodeUtil::get_number_value(&compiler, value_71).is_none());
        let value_72 = parse_expr(&mut compiler, "1-2");
        assert!(NodeUtil::get_number_value(&compiler, value_72).is_none());
        let value_73 = parse_expr(&mut compiler, "[1]");
        assert_eq!(
            NodeUtil::get_number_value(&compiler, value_73)
                .unwrap()
                .to_bits(),
            (1.0_f64).to_bits()
        );
        let value_74 = parse_expr(&mut compiler, "{}");
        assert!(
            NodeUtil::get_number_value(&compiler, value_74)
                .unwrap()
                .is_nan()
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetNumberValueNoConversions
    #[test]
    fn test_get_number_value_no_conversions() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "''");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_1).is_none());
        let value_2 = parse_expr(&mut compiler, "``");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_2).is_none());
        let value_3 = parse_expr(&mut compiler, "true");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_3).is_none());
        let value_4 = parse_expr(&mut compiler, "false");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_4).is_none());
        let value_5 = parse_expr(&mut compiler, "null");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_5).is_none());
        let value_6 = parse_expr(&mut compiler, "undefined");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_6).is_none());
        let value_7 = parse_expr(&mut compiler, "void 0");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_7).is_none());
        let value_8 = parse_expr(&mut compiler, "[1]");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_8).is_none());
        let value_9 = parse_expr(&mut compiler, "{}");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_9).is_none());
        let value_10 = parse_expr(&mut compiler, "1");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_10)
                .unwrap()
                .to_bits(),
            (1.0_f64).to_bits()
        );
        let value_11 = parse_expr(&mut compiler, "1n");
        assert!(NodeUtil::get_number_value_no_conversions(&compiler, value_11).is_none());
        let value_12 = parse_expr(&mut compiler, "-1");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_12)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_13 = parse_expr(&mut compiler, "+1");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_13)
                .unwrap()
                .to_bits(),
            (1.0_f64).to_bits()
        );
        let value_14 = parse_expr(&mut compiler, "22");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_14)
                .unwrap()
                .to_bits(),
            (22.0_f64).to_bits()
        );
        let value_15 = parse_expr(&mut compiler, "022");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_15)
                .unwrap()
                .to_bits(),
            (18.0_f64).to_bits()
        );
        let value_16 = parse_expr(&mut compiler, "0x22");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_16)
                .unwrap()
                .to_bits(),
            (34.0_f64).to_bits()
        );
        let value_17 = parse_expr(&mut compiler, "-0.1");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_17)
                .unwrap()
                .to_bits(),
            (-0.1_f64).to_bits()
        );
        let value_18 = parse_expr(&mut compiler, "-0.0");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_18)
                .unwrap()
                .to_bits(),
            (-0.0_f64).to_bits()
        );
        let value_19 = parse_expr(&mut compiler, "~1");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_19)
                .unwrap()
                .to_bits(),
            (-2.0_f64).to_bits()
        );
        let value_20 = parse_expr(&mut compiler, "~-1");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_20)
                .unwrap()
                .to_bits(),
            (0.0_f64).to_bits()
        );
        let value_21 = parse_expr(&mut compiler, "~22");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_21)
                .unwrap()
                .to_bits(),
            (-23.0_f64).to_bits()
        );
        let value_22 = parse_expr(&mut compiler, "~022");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_22)
                .unwrap()
                .to_bits(),
            (-19.0_f64).to_bits()
        );
        let value_23 = parse_expr(&mut compiler, "~0.0");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_23)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_24 = parse_expr(&mut compiler, "~0.1");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_24)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_25 = parse_expr(&mut compiler, "~NaN");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_25)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
        let value_26 = parse_expr(&mut compiler, "~Infinity");
        assert_eq!(
            NodeUtil::get_number_value_no_conversions(&compiler, value_26)
                .unwrap()
                .to_bits(),
            (-1.0_f64).to_bits()
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBigIntValue
    #[test]
    fn test_get_big_int_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "'1'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_1).unwrap(),
            BigInt::from(1)
        );
        let value_2 = parse_expr(&mut compiler, "'1n'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_2).is_none());
        let value_3 = parse_expr(&mut compiler, "'\\uFEFF1'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_3).unwrap(),
            BigInt::from(1)
        );
        let value_4 = parse_expr(&mut compiler, "''");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_4).unwrap(),
            BigInt::from(0)
        );
        let value_5 = parse_expr(&mut compiler, "' '");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_5).unwrap(),
            BigInt::from(0)
        );
        let value_6 = parse_expr(&mut compiler, "' \\t'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_6).unwrap(),
            BigInt::from(0)
        );
        let value_7 = parse_expr(&mut compiler, "' 1 '");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_7).unwrap(),
            BigInt::from(1)
        );
        let value_8 = parse_expr(&mut compiler, "'+0'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_8).unwrap(),
            BigInt::from(0)
        );
        let value_9 = parse_expr(&mut compiler, "'-0'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_9).unwrap(),
            BigInt::from(0)
        );
        let value_10 = parse_expr(&mut compiler, "'10'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_10).unwrap(),
            BigInt::from(10)
        );
        let value_11 = parse_expr(&mut compiler, "'+10'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_11).unwrap(),
            BigInt::parse_bytes("+10".as_bytes(), 10).unwrap()
        );
        let value_12 = parse_expr(&mut compiler, "'-10'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_12).unwrap(),
            BigInt::parse_bytes("-10".as_bytes(), 10).unwrap()
        );
        let value_13 = parse_expr(&mut compiler, "'   +10'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_13).unwrap(),
            BigInt::parse_bytes("+10".as_bytes(), 10).unwrap()
        );
        let value_14 = parse_expr(&mut compiler, "'   -10'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_14).unwrap(),
            BigInt::parse_bytes("-10".as_bytes(), 10).unwrap()
        );
        let value_15 = parse_expr(&mut compiler, "'0xa'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_15).unwrap(),
            BigInt::from(10)
        );
        let value_16 = parse_expr(&mut compiler, "'0XA'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_16).unwrap(),
            BigInt::from(10)
        );
        let value_17 = parse_expr(&mut compiler, "'0o12'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_17).unwrap(),
            BigInt::from(10)
        );
        let value_18 = parse_expr(&mut compiler, "'0O12'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_18).unwrap(),
            BigInt::from(10)
        );
        let value_19 = parse_expr(&mut compiler, "'0b1010'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_19).unwrap(),
            BigInt::from(10)
        );
        let value_20 = parse_expr(&mut compiler, "'0B1010'");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_20).unwrap(),
            BigInt::from(10)
        );
        let value_21 = parse_expr(&mut compiler, "'+0xF'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_21).is_none());
        let value_22 = parse_expr(&mut compiler, "'-0xF'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_22).is_none());
        let value_23 = parse_expr(&mut compiler, "'+0oF'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_23).is_none());
        let value_24 = parse_expr(&mut compiler, "'-0oF'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_24).is_none());
        let value_25 = parse_expr(&mut compiler, "'+0bF'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_25).is_none());
        let value_26 = parse_expr(&mut compiler, "'-0bF'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_26).is_none());
        let value_27 = parse_expr(&mut compiler, "'1.5'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_27).is_none());
        let value_28 = parse_expr(&mut compiler, "'-Infinity'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_28).is_none());
        let value_29 = parse_expr(&mut compiler, "'Infinity'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_29).is_none());
        let value_30 = parse_expr(&mut compiler, "'+Infinity'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_30).is_none());
        let value_31 = parse_expr(&mut compiler, "'NaN'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_31).is_none());
        let value_32 = parse_expr(&mut compiler, "'some unknown string'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_32).is_none());
        let value_33 = parse_expr(&mut compiler, "'123 blah'");
        assert!(NodeUtil::get_big_int_value(&compiler, value_33).is_none());
        let value_34 = parse_expr(&mut compiler, "1");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_34).unwrap(),
            BigInt::from(1)
        );
        let value_35 = parse_expr(&mut compiler, "1n");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_35).unwrap(),
            BigInt::from(1)
        );
        let value_36 = parse_expr(&mut compiler, "-1");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_36).unwrap(),
            BigInt::parse_bytes("-1".as_bytes(), 10).unwrap()
        );
        let value_37 = parse_expr(&mut compiler, "+1");
        assert!(NodeUtil::get_big_int_value(&compiler, value_37).is_none());
        let value_38 = parse_expr(&mut compiler, "22");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_38).unwrap(),
            BigInt::parse_bytes("22".as_bytes(), 10).unwrap()
        );
        let value_39 = parse_expr(&mut compiler, "022");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_39).unwrap(),
            BigInt::parse_bytes("18".as_bytes(), 10).unwrap()
        );
        let value_40 = parse_expr(&mut compiler, "0x22");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_40).unwrap(),
            BigInt::parse_bytes("34".as_bytes(), 10).unwrap()
        );
        let max_safe_int: i64 = 9007199254740991;
        let value_41 = parse_expr(&mut compiler, (max_safe_int).to_string());
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_41).unwrap(),
            BigInt::from(max_safe_int)
        );
        let value_42 = parse_expr(&mut compiler, (-max_safe_int).to_string());
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_42).unwrap(),
            BigInt::from(-max_safe_int)
        );
        let max_safe_int_plus_one: i64 = max_safe_int + 1;
        let value_43 = parse_expr(&mut compiler, (max_safe_int_plus_one).to_string());
        assert!(NodeUtil::get_big_int_value(&compiler, value_43).is_none());
        let value_44 = parse_expr(&mut compiler, (-max_safe_int_plus_one).to_string());
        assert!(NodeUtil::get_big_int_value(&compiler, value_44).is_none());
        let value_45 = parse_expr(&mut compiler, "true");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_45).unwrap(),
            BigInt::from(1)
        );
        let value_46 = parse_expr(&mut compiler, "false");
        assert_eq!(
            NodeUtil::get_big_int_value(&compiler, value_46).unwrap(),
            BigInt::from(0)
        );
        let value_47 = parse_expr(&mut compiler, "null");
        assert!(NodeUtil::get_big_int_value(&compiler, value_47).is_none());
        let value_48 = parse_expr(&mut compiler, "void 0");
        assert!(NodeUtil::get_big_int_value(&compiler, value_48).is_none());
        let value_49 = parse_expr(&mut compiler, "void f");
        assert!(NodeUtil::get_big_int_value(&compiler, value_49).is_none());
        let value_50 = parse_expr(&mut compiler, "x");
        assert!(NodeUtil::get_big_int_value(&compiler, value_50).is_none());
        let value_51 = parse_expr(&mut compiler, "x.y");
        assert!(NodeUtil::get_big_int_value(&compiler, value_51).is_none());
        let value_52 = parse_expr(&mut compiler, "1/2");
        assert!(NodeUtil::get_big_int_value(&compiler, value_52).is_none());
        let value_53 = parse_expr(&mut compiler, "1-2");
        assert!(NodeUtil::get_big_int_value(&compiler, value_53).is_none());
        let value_54 = parse_expr(&mut compiler, "+1");
        assert!(NodeUtil::get_big_int_value(&compiler, value_54).is_none());
    }

    // port: NodeUtilTest.AssortedTests#testIsNumericResult
    #[test]
    fn test_is_numeric_result() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "1");
        assert!(NodeUtil::is_numeric_result(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "true");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "+true");
        assert!(NodeUtil::is_numeric_result(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "+1");
        assert!(NodeUtil::is_numeric_result(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "-1");
        assert!(NodeUtil::is_numeric_result(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "-Infinity");
        assert!(NodeUtil::is_numeric_result(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "Infinity");
        assert!(NodeUtil::is_numeric_result(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "NaN");
        assert!(NodeUtil::is_numeric_result(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "undefined");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "void 0");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "a << b");
        assert!(NodeUtil::is_numeric_result(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "a >> b");
        assert!(NodeUtil::is_numeric_result(&compiler, value_12));
        let value_13 = parse_expr(&mut compiler, "a << 2");
        assert!(NodeUtil::is_numeric_result(&compiler, value_13));
        let value_14 = parse_expr(&mut compiler, "a >> 2");
        assert!(NodeUtil::is_numeric_result(&compiler, value_14));
        let value_15 = parse_expr(&mut compiler, "a >>> b");
        assert!(NodeUtil::is_numeric_result(&compiler, value_15));
        let value_16 = parse_expr(&mut compiler, "a == b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_16));
        let value_17 = parse_expr(&mut compiler, "a != b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_17));
        let value_18 = parse_expr(&mut compiler, "a === b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_18));
        let value_19 = parse_expr(&mut compiler, "a !== b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_19));
        let value_20 = parse_expr(&mut compiler, "a < b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_20));
        let value_21 = parse_expr(&mut compiler, "a > b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_21));
        let value_22 = parse_expr(&mut compiler, "a <= b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_22));
        let value_23 = parse_expr(&mut compiler, "a >= b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_23));
        let value_24 = parse_expr(&mut compiler, "a in b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_24));
        let value_25 = parse_expr(&mut compiler, "a instanceof b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_25));
        let value_26 = parse_expr(&mut compiler, "'a'");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_26));
        let value_27 = parse_expr(&mut compiler, "'a'+b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_27));
        let value_28 = parse_expr(&mut compiler, "a+'b'");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_28));
        let value_29 = parse_expr(&mut compiler, "a+b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_29));
        let value_30 = parse_expr(&mut compiler, "a()");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_30));
        let value_31 = parse_expr(&mut compiler, "''.a");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_31));
        let value_32 = parse_expr(&mut compiler, "a.b");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_32));
        let value_33 = parse_expr(&mut compiler, "a.b()");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_33));
        let value_34 = parse_expr(&mut compiler, "a().b()");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_34));
        let value_35 = parse_expr(&mut compiler, "new a()");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_35));
        let value_36 = parse_expr(&mut compiler, "([1,2])");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_36));
        let value_37 = parse_expr(&mut compiler, "({a:1})");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_37));
        let value_38 = parse_expr(&mut compiler, "1 && 2");
        assert!(NodeUtil::is_numeric_result(&compiler, value_38));
        let value_39 = parse_expr(&mut compiler, "1 || 2");
        assert!(NodeUtil::is_numeric_result(&compiler, value_39));
        let value_40 = parse_expr(&mut compiler, "a ? 2 : 3");
        assert!(NodeUtil::is_numeric_result(&compiler, value_40));
        let value_41 = parse_expr(&mut compiler, "a,1");
        assert!(NodeUtil::is_numeric_result(&compiler, value_41));
        let value_42 = parse_expr(&mut compiler, "a=1");
        assert!(NodeUtil::is_numeric_result(&compiler, value_42));
        let value_43 = parse_expr(&mut compiler, "a += 1");
        assert!(!NodeUtil::is_numeric_result(&compiler, value_43));
        let value_44 = parse_expr(&mut compiler, "a -= 1");
        assert!(NodeUtil::is_numeric_result(&compiler, value_44));
        let value_45 = parse_expr(&mut compiler, "a *= 1");
        assert!(NodeUtil::is_numeric_result(&compiler, value_45));
        let value_46 = parse_expr(&mut compiler, "--a");
        assert!(NodeUtil::is_numeric_result(&compiler, value_46));
        let value_47 = parse_expr(&mut compiler, "++a");
        assert!(NodeUtil::is_numeric_result(&compiler, value_47));
        let value_48 = parse_expr(&mut compiler, "a++");
        assert!(NodeUtil::is_numeric_result(&compiler, value_48));
        let value_49 = parse_expr(&mut compiler, "a--");
        assert!(NodeUtil::is_numeric_result(&compiler, value_49));
    }

    // port: NodeUtilTest.AssortedTests#testIsBigIntResult
    #[test]
    fn test_is_big_int_result() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "-1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "a *= 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "a * 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "1n * a");
        assert!(NodeUtil::is_big_int_result(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "a * b");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "'a'+1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "1n+'b'");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "1n + 2n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "a += 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "+a");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "a >>> b");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_12));
        let value_13 = parse_expr(&mut compiler, "a * b + 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_13));
        let value_14 = parse_expr(&mut compiler, "a * b * 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_14));
        let value_15 = parse_expr(&mut compiler, "a * b + c * 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_15));
        let value_16 = parse_expr(&mut compiler, "a * b + c * d");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_16));
        let value_17 = parse_expr(&mut compiler, "a * ~b * -c * d * 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_17));
        let value_18 = parse_expr(&mut compiler, "a * 1 * b * c * d * 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_18));
        let value_19 = parse_expr(&mut compiler, "a * b * +c * d * 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_19));
        let value_20 = parse_expr(&mut compiler, "a * b >> c * d * 1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_20));
        let value_21 = parse_expr(&mut compiler, "a * b >>> c * d * 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_21));
        let value_22 = parse_expr(&mut compiler, "-Infinity");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_22));
        let value_23 = parse_expr(&mut compiler, "Infinity");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_23));
        let value_24 = parse_expr(&mut compiler, "NaN");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_24));
        let value_25 = parse_expr(&mut compiler, "undefined");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_25));
        let value_26 = parse_expr(&mut compiler, "void 0n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_26));
        let value_27 = parse_expr(&mut compiler, "a == 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_27));
        let value_28 = parse_expr(&mut compiler, "a != 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_28));
        let value_29 = parse_expr(&mut compiler, "a === 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_29));
        let value_30 = parse_expr(&mut compiler, "a !== 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_30));
        let value_31 = parse_expr(&mut compiler, "a < 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_31));
        let value_32 = parse_expr(&mut compiler, "a > 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_32));
        let value_33 = parse_expr(&mut compiler, "a <= 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_33));
        let value_34 = parse_expr(&mut compiler, "a >= 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_34));
        let value_35 = parse_expr(&mut compiler, "a in 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_35));
        let value_36 = parse_expr(&mut compiler, "a instanceof 1n");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_36));
        let value_37 = parse_expr(&mut compiler, "a()");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_37));
        let value_38 = parse_expr(&mut compiler, "''.a");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_38));
        let value_39 = parse_expr(&mut compiler, "a.b");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_39));
        let value_40 = parse_expr(&mut compiler, "a.b()");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_40));
        let value_41 = parse_expr(&mut compiler, "a().b()");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_41));
        let value_42 = parse_expr(&mut compiler, "new a()");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_42));
        let value_43 = parse_expr(&mut compiler, "([1n,2n])");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_43));
        let value_44 = parse_expr(&mut compiler, "({a:1n})");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_44));
        let value_45 = parse_expr(&mut compiler, "1n && 2n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_45));
        let value_46 = parse_expr(&mut compiler, "1n || 2n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_46));
        let value_47 = parse_expr(&mut compiler, "a ? 2n : 3n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_47));
        let value_48 = parse_expr(&mut compiler, "a,1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_48));
        let value_49 = parse_expr(&mut compiler, "a=1n");
        assert!(NodeUtil::is_big_int_result(&compiler, value_49));
        let value_50 = parse_expr(&mut compiler, "--a");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_50));
        let value_51 = parse_expr(&mut compiler, "++a");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_51));
        let value_52 = parse_expr(&mut compiler, "a++");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_52));
        let value_53 = parse_expr(&mut compiler, "a--");
        assert!(!NodeUtil::is_big_int_result(&compiler, value_53));
    }

    // port: NodeUtilTest.AssortedTests#testIsBooleanResult
    #[test]
    fn test_is_boolean_result() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "1");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "true");
        assert!(NodeUtil::is_boolean_result(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "+true");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "+1");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "-1");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "-Infinity");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "Infinity");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "NaN");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "undefined");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "void 0");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "a << b");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "a >> b");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_12));
        let value_13 = parse_expr(&mut compiler, "a >>> b");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_13));
        let value_14 = parse_expr(&mut compiler, "a == b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_14));
        let value_15 = parse_expr(&mut compiler, "a != b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_15));
        let value_16 = parse_expr(&mut compiler, "a === b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_16));
        let value_17 = parse_expr(&mut compiler, "a !== b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_17));
        let value_18 = parse_expr(&mut compiler, "a < b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_18));
        let value_19 = parse_expr(&mut compiler, "a > b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_19));
        let value_20 = parse_expr(&mut compiler, "a <= b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_20));
        let value_21 = parse_expr(&mut compiler, "a >= b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_21));
        let value_22 = parse_expr(&mut compiler, "a in b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_22));
        let value_23 = parse_expr(&mut compiler, "a instanceof b");
        assert!(NodeUtil::is_boolean_result(&compiler, value_23));
        let value_24 = parse_expr(&mut compiler, "'a'");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_24));
        let value_25 = parse_expr(&mut compiler, "'a'+b");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_25));
        let value_26 = parse_expr(&mut compiler, "a+'b'");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_26));
        let value_27 = parse_expr(&mut compiler, "a+b");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_27));
        let value_28 = parse_expr(&mut compiler, "a()");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_28));
        let value_29 = parse_expr(&mut compiler, "''.a");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_29));
        let value_30 = parse_expr(&mut compiler, "a.b");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_30));
        let value_31 = parse_expr(&mut compiler, "a.b()");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_31));
        let value_32 = parse_expr(&mut compiler, "a().b()");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_32));
        let value_33 = parse_expr(&mut compiler, "new a()");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_33));
        let value_34 = parse_expr(&mut compiler, "delete a");
        assert!(NodeUtil::is_boolean_result(&compiler, value_34));
        let value_35 = parse_expr(&mut compiler, "([true,false])");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_35));
        let value_36 = parse_expr(&mut compiler, "({a:true})");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_36));
        let value_37 = parse_expr(&mut compiler, "true && false");
        assert!(NodeUtil::is_boolean_result(&compiler, value_37));
        let value_38 = parse_expr(&mut compiler, "true || false");
        assert!(NodeUtil::is_boolean_result(&compiler, value_38));
        let value_39 = parse_expr(&mut compiler, "a ? true : false");
        assert!(NodeUtil::is_boolean_result(&compiler, value_39));
        let value_40 = parse_expr(&mut compiler, "a,true");
        assert!(NodeUtil::is_boolean_result(&compiler, value_40));
        let value_41 = parse_expr(&mut compiler, "a=true");
        assert!(NodeUtil::is_boolean_result(&compiler, value_41));
        let value_42 = parse_expr(&mut compiler, "a=1");
        assert!(!NodeUtil::is_boolean_result(&compiler, value_42));
    }

    // port: NodeUtilTest.AssortedTests#testMayBeString
    #[test]
    fn test_may_be_string() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "1");
        assert!(!NodeUtil::may_be_string(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "1n");
        assert!(!NodeUtil::may_be_string(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "true");
        assert!(!NodeUtil::may_be_string(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "+true");
        assert!(!NodeUtil::may_be_string(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "+1");
        assert!(!NodeUtil::may_be_string(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "-1");
        assert!(!NodeUtil::may_be_string(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "-Infinity");
        assert!(!NodeUtil::may_be_string(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "Infinity");
        assert!(!NodeUtil::may_be_string(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "NaN");
        assert!(!NodeUtil::may_be_string(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "undefined");
        assert!(!NodeUtil::may_be_string(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "void 0");
        assert!(!NodeUtil::may_be_string(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "null");
        assert!(!NodeUtil::may_be_string(&compiler, value_12));
        let value_13 = parse_expr(&mut compiler, "a << b");
        assert!(!NodeUtil::may_be_string(&compiler, value_13));
        let value_14 = parse_expr(&mut compiler, "a >> b");
        assert!(!NodeUtil::may_be_string(&compiler, value_14));
        let value_15 = parse_expr(&mut compiler, "a >>> b");
        assert!(!NodeUtil::may_be_string(&compiler, value_15));
        let value_16 = parse_expr(&mut compiler, "a == b");
        assert!(!NodeUtil::may_be_string(&compiler, value_16));
        let value_17 = parse_expr(&mut compiler, "a != b");
        assert!(!NodeUtil::may_be_string(&compiler, value_17));
        let value_18 = parse_expr(&mut compiler, "a === b");
        assert!(!NodeUtil::may_be_string(&compiler, value_18));
        let value_19 = parse_expr(&mut compiler, "a !== b");
        assert!(!NodeUtil::may_be_string(&compiler, value_19));
        let value_20 = parse_expr(&mut compiler, "a < b");
        assert!(!NodeUtil::may_be_string(&compiler, value_20));
        let value_21 = parse_expr(&mut compiler, "a > b");
        assert!(!NodeUtil::may_be_string(&compiler, value_21));
        let value_22 = parse_expr(&mut compiler, "a <= b");
        assert!(!NodeUtil::may_be_string(&compiler, value_22));
        let value_23 = parse_expr(&mut compiler, "a >= b");
        assert!(!NodeUtil::may_be_string(&compiler, value_23));
        let value_24 = parse_expr(&mut compiler, "a in b");
        assert!(!NodeUtil::may_be_string(&compiler, value_24));
        let value_25 = parse_expr(&mut compiler, "a instanceof b");
        assert!(!NodeUtil::may_be_string(&compiler, value_25));
        let value_26 = parse_expr(&mut compiler, "'a'");
        assert!(NodeUtil::may_be_string(&compiler, value_26));
        let value_27 = parse_expr(&mut compiler, "'a'+b");
        assert!(NodeUtil::may_be_string(&compiler, value_27));
        let value_28 = parse_expr(&mut compiler, "a+'b'");
        assert!(NodeUtil::may_be_string(&compiler, value_28));
        let value_29 = parse_expr(&mut compiler, "a+b");
        assert!(NodeUtil::may_be_string(&compiler, value_29));
        let value_30 = parse_expr(&mut compiler, "a()");
        assert!(NodeUtil::may_be_string(&compiler, value_30));
        let value_31 = parse_expr(&mut compiler, "''.a");
        assert!(NodeUtil::may_be_string(&compiler, value_31));
        let value_32 = parse_expr(&mut compiler, "a.b");
        assert!(NodeUtil::may_be_string(&compiler, value_32));
        let value_33 = parse_expr(&mut compiler, "a.b()");
        assert!(NodeUtil::may_be_string(&compiler, value_33));
        let value_34 = parse_expr(&mut compiler, "a().b()");
        assert!(NodeUtil::may_be_string(&compiler, value_34));
        let value_35 = parse_expr(&mut compiler, "new a()");
        assert!(NodeUtil::may_be_string(&compiler, value_35));
        let value_36 = parse_expr(&mut compiler, "1 && 2");
        assert!(!NodeUtil::may_be_string(&compiler, value_36));
        let value_37 = parse_expr(&mut compiler, "1 || 2");
        assert!(!NodeUtil::may_be_string(&compiler, value_37));
        let value_38 = parse_expr(&mut compiler, "1 ? 2 : 3");
        assert!(!NodeUtil::may_be_string(&compiler, value_38));
        let value_39 = parse_expr(&mut compiler, "1,2");
        assert!(!NodeUtil::may_be_string(&compiler, value_39));
        let value_40 = parse_expr(&mut compiler, "a=1");
        assert!(!NodeUtil::may_be_string(&compiler, value_40));
        let value_41 = parse_expr(&mut compiler, "1+1");
        assert!(!NodeUtil::may_be_string(&compiler, value_41));
        let value_42 = parse_expr(&mut compiler, "true+true");
        assert!(!NodeUtil::may_be_string(&compiler, value_42));
        let value_43 = parse_expr(&mut compiler, "null+null");
        assert!(!NodeUtil::may_be_string(&compiler, value_43));
        let value_44 = parse_expr(&mut compiler, "NaN+NaN");
        assert!(!NodeUtil::may_be_string(&compiler, value_44));
        let value_45 = parse_expr(&mut compiler, "([1,2])");
        assert!(NodeUtil::may_be_string(&compiler, value_45));
        let value_46 = parse_expr(&mut compiler, "({a:1})");
        assert!(NodeUtil::may_be_string(&compiler, value_46));
        let value_47 = parse_expr(&mut compiler, "({}+1)");
        assert!(NodeUtil::may_be_string(&compiler, value_47));
        let value_48 = parse_expr(&mut compiler, "(1+{})");
        assert!(NodeUtil::may_be_string(&compiler, value_48));
        let value_49 = parse_expr(&mut compiler, "([]+1)");
        assert!(NodeUtil::may_be_string(&compiler, value_49));
        let value_50 = parse_expr(&mut compiler, "(1+[])");
        assert!(NodeUtil::may_be_string(&compiler, value_50));
        let value_51 = parse_expr(&mut compiler, "a += 'x'");
        assert!(NodeUtil::may_be_string(&compiler, value_51));
        let value_52 = parse_expr(&mut compiler, "a += 1");
        assert!(NodeUtil::may_be_string(&compiler, value_52));
    }

    // port: NodeUtilTest.AssortedTests#testIsStringResult
    #[test]
    fn test_is_string_result() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "1");
        assert!(!NodeUtil::is_string_result(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "true");
        assert!(!NodeUtil::is_string_result(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "+true");
        assert!(!NodeUtil::is_string_result(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "+1");
        assert!(!NodeUtil::is_string_result(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "-1");
        assert!(!NodeUtil::is_string_result(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "-Infinity");
        assert!(!NodeUtil::is_string_result(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "Infinity");
        assert!(!NodeUtil::is_string_result(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "NaN");
        assert!(!NodeUtil::is_string_result(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "undefined");
        assert!(!NodeUtil::is_string_result(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "void 0");
        assert!(!NodeUtil::is_string_result(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "null");
        assert!(!NodeUtil::is_string_result(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "a << b");
        assert!(!NodeUtil::is_string_result(&compiler, value_12));
        let value_13 = parse_expr(&mut compiler, "a >> b");
        assert!(!NodeUtil::is_string_result(&compiler, value_13));
        let value_14 = parse_expr(&mut compiler, "a >>> b");
        assert!(!NodeUtil::is_string_result(&compiler, value_14));
        let value_15 = parse_expr(&mut compiler, "a == b");
        assert!(!NodeUtil::is_string_result(&compiler, value_15));
        let value_16 = parse_expr(&mut compiler, "a != b");
        assert!(!NodeUtil::is_string_result(&compiler, value_16));
        let value_17 = parse_expr(&mut compiler, "a === b");
        assert!(!NodeUtil::is_string_result(&compiler, value_17));
        let value_18 = parse_expr(&mut compiler, "a !== b");
        assert!(!NodeUtil::is_string_result(&compiler, value_18));
        let value_19 = parse_expr(&mut compiler, "a < b");
        assert!(!NodeUtil::is_string_result(&compiler, value_19));
        let value_20 = parse_expr(&mut compiler, "a > b");
        assert!(!NodeUtil::is_string_result(&compiler, value_20));
        let value_21 = parse_expr(&mut compiler, "a <= b");
        assert!(!NodeUtil::is_string_result(&compiler, value_21));
        let value_22 = parse_expr(&mut compiler, "a >= b");
        assert!(!NodeUtil::is_string_result(&compiler, value_22));
        let value_23 = parse_expr(&mut compiler, "a in b");
        assert!(!NodeUtil::is_string_result(&compiler, value_23));
        let value_24 = parse_expr(&mut compiler, "a instanceof b");
        assert!(!NodeUtil::is_string_result(&compiler, value_24));
        let value_25 = parse_expr(&mut compiler, "'a'");
        assert!(NodeUtil::is_string_result(&compiler, value_25));
        let value_26 = parse_expr(&mut compiler, "'a'+b");
        assert!(NodeUtil::is_string_result(&compiler, value_26));
        let value_27 = parse_expr(&mut compiler, "a+'b'");
        assert!(NodeUtil::is_string_result(&compiler, value_27));
        let value_28 = parse_expr(&mut compiler, "a+b");
        assert!(!NodeUtil::is_string_result(&compiler, value_28));
        let value_29 = parse_expr(&mut compiler, "a()");
        assert!(!NodeUtil::is_string_result(&compiler, value_29));
        let value_30 = parse_expr(&mut compiler, "''.a");
        assert!(!NodeUtil::is_string_result(&compiler, value_30));
        let value_31 = parse_expr(&mut compiler, "a.b");
        assert!(!NodeUtil::is_string_result(&compiler, value_31));
        let value_32 = parse_expr(&mut compiler, "a.b()");
        assert!(!NodeUtil::is_string_result(&compiler, value_32));
        let value_33 = parse_expr(&mut compiler, "a().b()");
        assert!(!NodeUtil::is_string_result(&compiler, value_33));
        let value_34 = parse_expr(&mut compiler, "new a()");
        assert!(!NodeUtil::is_string_result(&compiler, value_34));
        let value_35 = parse_expr(&mut compiler, "1 && 2");
        assert!(!NodeUtil::is_string_result(&compiler, value_35));
        let value_36 = parse_expr(&mut compiler, "1 || 2");
        assert!(!NodeUtil::is_string_result(&compiler, value_36));
        let value_37 = parse_expr(&mut compiler, "1 ? 2 : 3");
        assert!(!NodeUtil::is_string_result(&compiler, value_37));
        let value_38 = parse_expr(&mut compiler, "1,2");
        assert!(!NodeUtil::is_string_result(&compiler, value_38));
        let value_39 = parse_expr(&mut compiler, "a=1");
        assert!(!NodeUtil::is_string_result(&compiler, value_39));
        let value_40 = parse_expr(&mut compiler, "1+1");
        assert!(!NodeUtil::is_string_result(&compiler, value_40));
        let value_41 = parse_expr(&mut compiler, "true+true");
        assert!(!NodeUtil::is_string_result(&compiler, value_41));
        let value_42 = parse_expr(&mut compiler, "null+null");
        assert!(!NodeUtil::is_string_result(&compiler, value_42));
        let value_43 = parse_expr(&mut compiler, "NaN+NaN");
        assert!(!NodeUtil::is_string_result(&compiler, value_43));
        let value_44 = parse_expr(&mut compiler, "([1,2])");
        assert!(!NodeUtil::is_string_result(&compiler, value_44));
        let value_45 = parse_expr(&mut compiler, "({a:1})");
        assert!(!NodeUtil::is_string_result(&compiler, value_45));
        let value_46 = parse_expr(&mut compiler, "({}+1)");
        assert!(!NodeUtil::is_string_result(&compiler, value_46));
        let value_47 = parse_expr(&mut compiler, "(1+{})");
        assert!(!NodeUtil::is_string_result(&compiler, value_47));
        let value_48 = parse_expr(&mut compiler, "([]+1)");
        assert!(!NodeUtil::is_string_result(&compiler, value_48));
        let value_49 = parse_expr(&mut compiler, "(1+[])");
        assert!(!NodeUtil::is_string_result(&compiler, value_49));
        let value_50 = parse_expr(&mut compiler, "a += 'x'");
        assert!(NodeUtil::is_string_result(&compiler, value_50));
        let value_51 = parse_expr(&mut compiler, "`x`");
        assert!(NodeUtil::is_string_result(&compiler, value_51));
        let value_52 = parse_expr(&mut compiler, "`a${b}c`");
        assert!(NodeUtil::is_string_result(&compiler, value_52));
    }

    // port: NodeUtilTest.AssortedTests#testMayBeObject
    #[test]
    fn test_may_be_object() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "1");
        assert!(!NodeUtil::may_be_object(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "1n");
        assert!(!NodeUtil::may_be_object(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "a * b");
        assert!(!NodeUtil::may_be_object(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "true");
        assert!(!NodeUtil::may_be_object(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "+true");
        assert!(!NodeUtil::may_be_object(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "+1");
        assert!(!NodeUtil::may_be_object(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "-1");
        assert!(!NodeUtil::may_be_object(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "-Infinity");
        assert!(!NodeUtil::may_be_object(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "Infinity");
        assert!(!NodeUtil::may_be_object(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "NaN");
        assert!(!NodeUtil::may_be_object(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "undefined");
        assert!(!NodeUtil::may_be_object(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "void 0");
        assert!(!NodeUtil::may_be_object(&compiler, value_12));
        let value_13 = parse_expr(&mut compiler, "null");
        assert!(!NodeUtil::may_be_object(&compiler, value_13));
        let value_14 = parse_expr(&mut compiler, "a << b");
        assert!(!NodeUtil::may_be_object(&compiler, value_14));
        let value_15 = parse_expr(&mut compiler, "a >> b");
        assert!(!NodeUtil::may_be_object(&compiler, value_15));
        let value_16 = parse_expr(&mut compiler, "a >>> b");
        assert!(!NodeUtil::may_be_object(&compiler, value_16));
        let value_17 = parse_expr(&mut compiler, "a == b");
        assert!(!NodeUtil::may_be_object(&compiler, value_17));
        let value_18 = parse_expr(&mut compiler, "a != b");
        assert!(!NodeUtil::may_be_object(&compiler, value_18));
        let value_19 = parse_expr(&mut compiler, "a === b");
        assert!(!NodeUtil::may_be_object(&compiler, value_19));
        let value_20 = parse_expr(&mut compiler, "a !== b");
        assert!(!NodeUtil::may_be_object(&compiler, value_20));
        let value_21 = parse_expr(&mut compiler, "a < b");
        assert!(!NodeUtil::may_be_object(&compiler, value_21));
        let value_22 = parse_expr(&mut compiler, "a > b");
        assert!(!NodeUtil::may_be_object(&compiler, value_22));
        let value_23 = parse_expr(&mut compiler, "a <= b");
        assert!(!NodeUtil::may_be_object(&compiler, value_23));
        let value_24 = parse_expr(&mut compiler, "a >= b");
        assert!(!NodeUtil::may_be_object(&compiler, value_24));
        let value_25 = parse_expr(&mut compiler, "a in b");
        assert!(!NodeUtil::may_be_object(&compiler, value_25));
        let value_26 = parse_expr(&mut compiler, "a instanceof b");
        assert!(!NodeUtil::may_be_object(&compiler, value_26));
        let value_27 = parse_expr(&mut compiler, "'a'");
        assert!(!NodeUtil::may_be_object(&compiler, value_27));
        let value_28 = parse_expr(&mut compiler, "'a'+b");
        assert!(!NodeUtil::may_be_object(&compiler, value_28));
        let value_29 = parse_expr(&mut compiler, "a+'b'");
        assert!(!NodeUtil::may_be_object(&compiler, value_29));
        let value_30 = parse_expr(&mut compiler, "a+b");
        assert!(NodeUtil::may_be_object(&compiler, value_30));
        let value_31 = parse_expr(&mut compiler, "a()");
        assert!(NodeUtil::may_be_object(&compiler, value_31));
        let value_32 = parse_expr(&mut compiler, "''.a");
        assert!(NodeUtil::may_be_object(&compiler, value_32));
        let value_33 = parse_expr(&mut compiler, "a.b");
        assert!(NodeUtil::may_be_object(&compiler, value_33));
        let value_34 = parse_expr(&mut compiler, "a.b()");
        assert!(NodeUtil::may_be_object(&compiler, value_34));
        let value_35 = parse_expr(&mut compiler, "a().b()");
        assert!(NodeUtil::may_be_object(&compiler, value_35));
        let value_36 = parse_expr(&mut compiler, "new a()");
        assert!(NodeUtil::may_be_object(&compiler, value_36));
        let value_37 = parse_expr(&mut compiler, "1 && 2");
        assert!(!NodeUtil::may_be_object(&compiler, value_37));
        let value_38 = parse_expr(&mut compiler, "1 || 2");
        assert!(!NodeUtil::may_be_object(&compiler, value_38));
        let value_39 = parse_expr(&mut compiler, "1 ? 2 : 3");
        assert!(!NodeUtil::may_be_object(&compiler, value_39));
        let value_40 = parse_expr(&mut compiler, "1,2");
        assert!(!NodeUtil::may_be_object(&compiler, value_40));
        let value_41 = parse_expr(&mut compiler, "a=1");
        assert!(!NodeUtil::may_be_object(&compiler, value_41));
        let value_42 = parse_expr(&mut compiler, "1+1");
        assert!(!NodeUtil::may_be_object(&compiler, value_42));
        let value_43 = parse_expr(&mut compiler, "true+true");
        assert!(!NodeUtil::may_be_object(&compiler, value_43));
        let value_44 = parse_expr(&mut compiler, "null+null");
        assert!(!NodeUtil::may_be_object(&compiler, value_44));
        let value_45 = parse_expr(&mut compiler, "NaN+NaN");
        assert!(!NodeUtil::may_be_object(&compiler, value_45));
        let value_46 = parse_expr(&mut compiler, "([1,2])");
        assert!(NodeUtil::may_be_object(&compiler, value_46));
        let value_47 = parse_expr(&mut compiler, "({a:1})");
        assert!(NodeUtil::may_be_object(&compiler, value_47));
        let value_48 = parse_expr(&mut compiler, "({}+1)");
        assert!(NodeUtil::may_be_object(&compiler, value_48));
        let value_49 = parse_expr(&mut compiler, "(1+{})");
        assert!(NodeUtil::may_be_object(&compiler, value_49));
        let value_50 = parse_expr(&mut compiler, "([]+1)");
        assert!(NodeUtil::may_be_object(&compiler, value_50));
        let value_51 = parse_expr(&mut compiler, "(1+[])");
        assert!(NodeUtil::may_be_object(&compiler, value_51));
        let value_52 = parse_expr(&mut compiler, "a += 'x'");
        assert!(!NodeUtil::may_be_object(&compiler, value_52));
        let value_53 = parse_expr(&mut compiler, "a += 1");
        assert!(NodeUtil::may_be_object(&compiler, value_53));
    }

    // port: NodeUtilTest.AssortedTests#testIsObjectResult
    #[test]
    fn test_is_object_result() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "1");
        assert!(!NodeUtil::is_object_result(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "true");
        assert!(!NodeUtil::is_object_result(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "+true");
        assert!(!NodeUtil::is_object_result(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "+1");
        assert!(!NodeUtil::is_object_result(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "-1");
        assert!(!NodeUtil::is_object_result(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "-Infinity");
        assert!(!NodeUtil::is_object_result(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "Infinity");
        assert!(!NodeUtil::is_object_result(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "NaN");
        assert!(!NodeUtil::is_object_result(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "undefined");
        assert!(!NodeUtil::is_object_result(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "void 0");
        assert!(!NodeUtil::is_object_result(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "a << b");
        assert!(!NodeUtil::is_object_result(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "a >> b");
        assert!(!NodeUtil::is_object_result(&compiler, value_12));
        let value_13 = parse_expr(&mut compiler, "a >>> b");
        assert!(!NodeUtil::is_object_result(&compiler, value_13));
        let value_14 = parse_expr(&mut compiler, "a == b");
        assert!(!NodeUtil::is_object_result(&compiler, value_14));
        let value_15 = parse_expr(&mut compiler, "a != b");
        assert!(!NodeUtil::is_object_result(&compiler, value_15));
        let value_16 = parse_expr(&mut compiler, "a === b");
        assert!(!NodeUtil::is_object_result(&compiler, value_16));
        let value_17 = parse_expr(&mut compiler, "a !== b");
        assert!(!NodeUtil::is_object_result(&compiler, value_17));
        let value_18 = parse_expr(&mut compiler, "a < b");
        assert!(!NodeUtil::is_object_result(&compiler, value_18));
        let value_19 = parse_expr(&mut compiler, "a > b");
        assert!(!NodeUtil::is_object_result(&compiler, value_19));
        let value_20 = parse_expr(&mut compiler, "a <= b");
        assert!(!NodeUtil::is_object_result(&compiler, value_20));
        let value_21 = parse_expr(&mut compiler, "a >= b");
        assert!(!NodeUtil::is_object_result(&compiler, value_21));
        let value_22 = parse_expr(&mut compiler, "a in b");
        assert!(!NodeUtil::is_object_result(&compiler, value_22));
        let value_23 = parse_expr(&mut compiler, "a instanceof b");
        assert!(!NodeUtil::is_object_result(&compiler, value_23));
        let value_24 = parse_expr(&mut compiler, "delete a");
        assert!(!NodeUtil::is_object_result(&compiler, value_24));
        let value_25 = parse_expr(&mut compiler, "'a'");
        assert!(!NodeUtil::is_object_result(&compiler, value_25));
        let value_26 = parse_expr(&mut compiler, "'a'+b");
        assert!(!NodeUtil::is_object_result(&compiler, value_26));
        let value_27 = parse_expr(&mut compiler, "a+'b'");
        assert!(!NodeUtil::is_object_result(&compiler, value_27));
        let value_28 = parse_expr(&mut compiler, "a+b");
        assert!(!NodeUtil::is_object_result(&compiler, value_28));
        let value_29 = parse_expr(&mut compiler, "{},true");
        assert!(!NodeUtil::is_object_result(&compiler, value_29));
        let value_30 = parse_expr(&mut compiler, "a()");
        assert!(!NodeUtil::is_object_result(&compiler, value_30));
        let value_31 = parse_expr(&mut compiler, "''.a");
        assert!(!NodeUtil::is_object_result(&compiler, value_31));
        let value_32 = parse_expr(&mut compiler, "a.b");
        assert!(!NodeUtil::is_object_result(&compiler, value_32));
        let value_33 = parse_expr(&mut compiler, "a.b()");
        assert!(!NodeUtil::is_object_result(&compiler, value_33));
        let value_34 = parse_expr(&mut compiler, "a().b()");
        assert!(!NodeUtil::is_object_result(&compiler, value_34));
        let value_35 = parse_expr(&mut compiler, "a ? true : {}");
        assert!(!NodeUtil::is_object_result(&compiler, value_35));
        let value_36 = parse_expr(&mut compiler, "true && {}");
        assert!(!NodeUtil::is_object_result(&compiler, value_36));
        let value_37 = parse_expr(&mut compiler, "true || {}");
        assert!(!NodeUtil::is_object_result(&compiler, value_37));
        let value_38 = parse_expr(&mut compiler, "new a.b()");
        assert!(NodeUtil::is_object_result(&compiler, value_38));
        let value_39 = parse_expr(&mut compiler, "([true,false])");
        assert!(NodeUtil::is_object_result(&compiler, value_39));
        let value_40 = parse_expr(&mut compiler, "({a:true})");
        assert!(NodeUtil::is_object_result(&compiler, value_40));
        let value_41 = parse_expr(&mut compiler, "a={}");
        assert!(NodeUtil::is_object_result(&compiler, value_41));
        let value_42 = parse_expr(&mut compiler, "[] && {}");
        assert!(NodeUtil::is_object_result(&compiler, value_42));
        let value_43 = parse_expr(&mut compiler, "[] || {}");
        assert!(NodeUtil::is_object_result(&compiler, value_43));
        let value_44 = parse_expr(&mut compiler, "a ? [] : {}");
        assert!(NodeUtil::is_object_result(&compiler, value_44));
        let value_45 = parse_expr(&mut compiler, "{},[]");
        assert!(NodeUtil::is_object_result(&compiler, value_45));
        let value_46 = parse_expr(&mut compiler, "/a/g");
        assert!(NodeUtil::is_object_result(&compiler, value_46));
    }

    // port: NodeUtilTest.AssortedTests#testGetNearestFunctionName
    #[test]
    fn test_get_nearest_function_name() {
        let mut compiler = Compiler::new();
        test_function_name(&mut compiler, "(function() {})()", None);
        test_function_name(&mut compiler, "function a() {}", Some("a"));
        test_function_name(&mut compiler, "(function a() {})", Some("a"));
        test_function_name(&mut compiler, "({a:function () {}})", Some("a"));
        test_function_name(&mut compiler, "({get a() {}})", Some("a"));
        test_function_name(&mut compiler, "({set a(b) {}})", Some("a"));
        test_function_name(&mut compiler, "({set a(b) {}})", Some("a"));
        test_function_name(&mut compiler, "({1:function () {}})", Some("1"));
        test_function_name(&mut compiler, "var a = function a() {}", Some("a"));
        test_function_name(&mut compiler, "var a;a = function a() {}", Some("a"));
        test_function_name(&mut compiler, "var o;o.a = function a() {}", Some("o.a"));
        test_function_name(&mut compiler, "this.a = function a() {}", Some("this.a"));
    }

    // port: NodeUtilTest.AssortedTests#testGetBestLValue
    #[test]
    fn test_get_best_l_value() {
        let mut compiler = Compiler::new();
        let value_1 = get_function_l_value(&mut compiler, "var x = function() {};");
        assert_eq!(value_1.unwrap(), "x");
        let value_2 = get_function_l_value(&mut compiler, "x = function() {};");
        assert_eq!(value_2.unwrap(), "x");
        let value_3 = get_function_l_value(&mut compiler, "function x() {};");
        assert_eq!(value_3.unwrap(), "x");
        let value_4 = get_function_l_value(&mut compiler, "var x = y ? z : function() {};");
        assert_eq!(value_4.unwrap(), "x");
        let value_5 = get_function_l_value(&mut compiler, "var x = y ? function() {} : z;");
        assert_eq!(value_5.unwrap(), "x");
        let value_6 = get_function_l_value(&mut compiler, "var x = y && function() {};");
        assert_eq!(value_6.unwrap(), "x");
        let value_7 = get_function_l_value(&mut compiler, "var x = y || function() {};");
        assert_eq!(value_7.unwrap(), "x");
        let value_8 = get_function_l_value(&mut compiler, "var x = (y, function() {});");
        assert_eq!(value_8.unwrap(), "x");
    }

    // port: NodeUtilTest.AssortedTests#testGetRValueOfLValue
    #[test]
    fn test_get_r_value_of_l_value() {
        let mut compiler = Compiler::new();
        let value_1 = function_is_r_value_of_assign(&mut compiler, "x = function() {};");
        assert!(value_1);
        let value_2 = function_is_r_value_of_assign(&mut compiler, "x += function() {};");
        assert!(value_2);
        let value_3 = function_is_r_value_of_assign(&mut compiler, "x -= function() {};");
        assert!(value_3);
        let value_4 = function_is_r_value_of_assign(&mut compiler, "x *= function() {};");
        assert!(value_4);
        let value_5 = function_is_r_value_of_assign(&mut compiler, "x /= function() {};");
        assert!(value_5);
        let value_6 = function_is_r_value_of_assign(&mut compiler, "x <<= function() {};");
        assert!(value_6);
        let value_7 = function_is_r_value_of_assign(&mut compiler, "x >>= function() {};");
        assert!(value_7);
        let value_8 = function_is_r_value_of_assign(&mut compiler, "x >>= function() {};");
        assert!(value_8);
        let value_9 = function_is_r_value_of_assign(&mut compiler, "x >>>= function() {};");
        assert!(value_9);
        let value_10 = function_is_r_value_of_assign(&mut compiler, "x = y ? x : function() {};");
        assert!(!value_10);
    }

    // port: NodeUtilTest.AssortedTests#testGetRValueOfLValueInObjectLiteral
    #[test]
    fn test_get_r_value_of_l_value_in_object_literal() {
        let mut compiler = Compiler::new();
        let script_node = parse(&mut compiler, "var x = {[computedProp] : 5};");
        let computed_prop = script_node
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        assert!(computed_prop.unwrap().is_computed_prop(&compiler));
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, computed_prop.unwrap())
                .unwrap()
                .get_token(&compiler),
            Token::NUMBER
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetRValueOfLValueFromClass
    #[test]
    fn test_get_r_value_of_l_value_from_class() {
        let mut compiler = Compiler::new();
        let script_node = parse(&mut compiler, "class Foo { ['computedField'] = 5}");
        let computed_field = script_node
            .get_first_child(&compiler)
            .unwrap()
            .get_last_child(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        assert!(computed_field.unwrap().is_computed_field_def(&compiler));
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, computed_field.unwrap())
                .unwrap()
                .get_token(&compiler),
            Token::NUMBER
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetRValueOfLValueInDestructuringPattern
    #[test]
    fn test_get_r_value_of_l_value_in_destructuring_pattern() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var [x] = rhs;");
        assert!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_name_node(&compiler, value_1, "x"))
                .is_none()
        );
        let value_2 = parse(&mut compiler, "var [x, y] = rhs;");
        assert!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_name_node(&compiler, value_2, "x"))
                .is_none()
        );
        let value_3 = parse(&mut compiler, "var [y, x] = rhs;");
        assert!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_name_node(&compiler, value_3, "x"))
                .is_none()
        );
        let value_4 = parse(&mut compiler, "var {x: x} = rhs;");
        assert!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_name_node(&compiler, value_4, "x"))
                .is_none()
        );
        let value_5 = parse(&mut compiler, "var {y: x} = rhs;");
        assert!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_name_node(&compiler, value_5, "x"))
                .is_none()
        );
        let ast = parse(&mut compiler, "var {x} = rhs;");
        let x = ast
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        assert!(x.unwrap().is_string_key(&compiler));
        assert!(NodeUtil::get_r_value_of_l_value(&compiler, x.unwrap()).is_none());
    }

    // port: NodeUtilTest.AssortedTests#testGetRValueOfLValueDestructuringPattern
    #[test]
    fn test_get_r_value_of_l_value_destructuring_pattern() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var [x] = 'rhs';");
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_pattern(&compiler, value_1))
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_2 = parse(&mut compiler, "var [x, y] = 'rhs';");
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_pattern(&compiler, value_2))
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_3 = parse(&mut compiler, "var [y, x] = 'rhs';");
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_pattern(&compiler, value_3))
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_4 = parse(&mut compiler, "var {x: x} = 'rhs';");
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_pattern(&compiler, value_4))
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_5 = parse(&mut compiler, "var {y: x} = 'rhs';");
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_pattern(&compiler, value_5))
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_6 = parse(&mut compiler, "var {x} = 'rhs';");
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, get_pattern(&compiler, value_6))
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetRValueOfLValueDestructuringLhs
    #[test]
    fn test_get_r_value_of_l_value_destructuring_lhs() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(&mut compiler, Token::DESTRUCTURING_LHS, "var [x] = 'rhs';");
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, value_1)
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_2 = parse_first(
            &mut compiler,
            Token::DESTRUCTURING_LHS,
            "var [x, y] = 'rhs';",
        );
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, value_2)
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_3 = parse_first(
            &mut compiler,
            Token::DESTRUCTURING_LHS,
            "var [y, x] = 'rhs';",
        );
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, value_3)
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_4 = parse_first(
            &mut compiler,
            Token::DESTRUCTURING_LHS,
            "var {x: x} = 'rhs';",
        );
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, value_4)
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_5 = parse_first(
            &mut compiler,
            Token::DESTRUCTURING_LHS,
            "var {y: x} = 'rhs';",
        );
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, value_5)
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
        let value_6 = parse_first(&mut compiler, Token::DESTRUCTURING_LHS, "var {x} = 'rhs';");
        assert_eq!(
            NodeUtil::get_r_value_of_l_value(&compiler, value_6)
                .unwrap()
                .get_token(&compiler),
            Token::STRINGLIT
        );
    }

    // port: NodeUtilTest.AssortedTests#testIsNaN
    #[test]
    fn test_is_na_n() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "NaN");
        assert!(NodeUtil::is_nan(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "Infinity");
        assert!(!NodeUtil::is_nan(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "x");
        assert!(!NodeUtil::is_nan(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "0/0");
        assert!(NodeUtil::is_nan(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "1/0");
        assert!(!NodeUtil::is_nan(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "0/1");
        assert!(!NodeUtil::is_nan(&compiler, value_6));
        let value_7 = IR::number(&mut compiler, 0.0);
        assert!(!NodeUtil::is_nan(&compiler, value_7));
    }

    // port: NodeUtilTest.AssortedTests#testIsExecutedExactlyOnce
    #[test]
    fn test_is_executed_exactly_once() {
        let mut compiler = Compiler::new();
        let value_1 = executed_once_test_case(&mut compiler, "x;");
        assert!(value_1);
        let value_2 = executed_once_test_case(&mut compiler, "x && 1;");
        assert!(value_2);
        let value_3 = executed_once_test_case(&mut compiler, "1 && x;");
        assert!(!value_3);
        let value_4 = executed_once_test_case(&mut compiler, "1 && (x && 1);");
        assert!(!value_4);
        let value_5 = executed_once_test_case(&mut compiler, "x || 1;");
        assert!(value_5);
        let value_6 = executed_once_test_case(&mut compiler, "1 || x;");
        assert!(!value_6);
        let value_7 = executed_once_test_case(&mut compiler, "1 && (x || 1);");
        assert!(!value_7);
        let value_8 = executed_once_test_case(&mut compiler, "x ||= 1;");
        assert!(value_8);
        let value_9 = executed_once_test_case(&mut compiler, "y ||= x;");
        assert!(!value_9);
        let value_10 = executed_once_test_case(&mut compiler, "x &&= 1;");
        assert!(value_10);
        let value_11 = executed_once_test_case(&mut compiler, "y &&= x;");
        assert!(!value_11);
        let value_12 = executed_once_test_case(&mut compiler, "x ??= 1;");
        assert!(value_12);
        let value_13 = executed_once_test_case(&mut compiler, "y ??= x;");
        assert!(!value_13);
        let value_14 = executed_once_test_case(&mut compiler, "x ? 1 : 2;");
        assert!(value_14);
        let value_15 = executed_once_test_case(&mut compiler, "1 ? 1 : x;");
        assert!(!value_15);
        let value_16 = executed_once_test_case(&mut compiler, "1 ? x : 2;");
        assert!(!value_16);
        let value_17 = executed_once_test_case(&mut compiler, "1 && (x ? 1 : 2);");
        assert!(!value_17);
        let value_18 = executed_once_test_case(&mut compiler, "if (x) {}");
        assert!(value_18);
        let value_19 = executed_once_test_case(&mut compiler, "if (true) {x;}");
        assert!(!value_19);
        let value_20 = executed_once_test_case(&mut compiler, "if (true) {} else {x;}");
        assert!(!value_20);
        let value_21 = executed_once_test_case(&mut compiler, "if (1) { if (x) {} }");
        assert!(!value_21);
        let value_22 = executed_once_test_case(&mut compiler, "for(x;;){}");
        assert!(value_22);
        let value_23 = executed_once_test_case(&mut compiler, "for(;x;){}");
        assert!(!value_23);
        let value_24 = executed_once_test_case(&mut compiler, "for(;;x){}");
        assert!(!value_24);
        let value_25 = executed_once_test_case(&mut compiler, "for(;;){x;}");
        assert!(!value_25);
        let value_26 = executed_once_test_case(&mut compiler, "if (1) { for(x;;){} }");
        assert!(!value_26);
        let value_27 = executed_once_test_case(&mut compiler, "for(x in {}){}");
        assert!(!value_27);
        let value_28 = executed_once_test_case(&mut compiler, "for({}.a in x){}");
        assert!(value_28);
        let value_29 = executed_once_test_case(&mut compiler, "for({}.a in {}){x}");
        assert!(!value_29);
        let value_30 = executed_once_test_case(&mut compiler, "if (1) { for(x in {}){} }");
        assert!(!value_30);
        let value_31 = executed_once_test_case(&mut compiler, "for(x of {}){}");
        assert!(!value_31);
        let value_32 = executed_once_test_case(&mut compiler, "for({}.a of x){}");
        assert!(value_32);
        let value_33 = executed_once_test_case(&mut compiler, "for({}.a of {}){x}");
        assert!(!value_33);
        let value_34 = executed_once_test_case(&mut compiler, "if (1) { for(x of {}){} }");
        assert!(!value_34);
        let value_35 =
            executed_once_test_case(&mut compiler, "async function f() { for await(x of {}){} }");
        assert!(!value_35);
        let value_36 = executed_once_test_case(
            &mut compiler,
            "async function f() { for await({}.a of x){} }",
        );
        assert!(value_36);
        let value_37 = executed_once_test_case(
            &mut compiler,
            "async function f() { for await({}.a of {}){x} }",
        );
        assert!(!value_37);
        let value_38 = executed_once_test_case(
            &mut compiler,
            "async function f() { if (1) { for await(x of {}){} } }",
        );
        assert!(!value_38);
        let value_39 = executed_once_test_case(&mut compiler, "switch (x) {}");
        assert!(value_39);
        let value_40 = executed_once_test_case(&mut compiler, "switch (1) {case x:}");
        assert!(!value_40);
        let value_41 = executed_once_test_case(&mut compiler, "switch (1) {case 1: x}");
        assert!(!value_41);
        let value_42 = executed_once_test_case(&mut compiler, "switch (1) {default: x}");
        assert!(!value_42);
        let value_43 = executed_once_test_case(&mut compiler, "if (1) { switch (x) {} }");
        assert!(!value_43);
        let value_44 = executed_once_test_case(&mut compiler, "while (x) {}");
        assert!(!value_44);
        let value_45 = executed_once_test_case(&mut compiler, "while (1) {x}");
        assert!(!value_45);
        let value_46 = executed_once_test_case(&mut compiler, "do {} while (x)");
        assert!(!value_46);
        let value_47 = executed_once_test_case(&mut compiler, "do {x} while (1)");
        assert!(!value_47);
        let value_48 = executed_once_test_case(&mut compiler, "try {x} catch (e) {}");
        assert!(!value_48);
        let value_49 = executed_once_test_case(&mut compiler, "try {} catch (e) {x}");
        assert!(!value_49);
        let value_50 = executed_once_test_case(&mut compiler, "try {} finally {x}");
        assert!(value_50);
        let value_51 = executed_once_test_case(&mut compiler, "if (1) { try {} finally {x} }");
        assert!(!value_51);
    }

    // port: NodeUtilTest.AssortedTests#testIsLValue
    #[test]
    fn test_is_l_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var x;");
        assert_l_value_named_x(&compiler, value_1.get_first_first_child(&compiler).unwrap());
        let value_2 = parse(&mut compiler, "var w, x;");
        assert_l_value_named_x(
            &compiler,
            value_2
                .get_first_child(&compiler)
                .unwrap()
                .get_last_child(&compiler)
                .unwrap(),
        );
        let value_3 = parse(&mut compiler, "var [...x] = y;");
        assert_l_value_named_x(
            &compiler,
            value_3
                .get_first_first_child(&compiler)
                .unwrap()
                .get_first_first_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
        );
        let value_4 = parse(&mut compiler, "var x = y;");
        assert_l_value_named_x(&compiler, value_4.get_first_first_child(&compiler).unwrap());
        let value_5 = parse(&mut compiler, "x++;");
        assert_l_value_named_x(
            &compiler,
            value_5
                .get_first_first_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
        );
        let value_6 = parse(&mut compiler, "function f(x) {}");
        assert_l_value_named_x(
            &compiler,
            NodeUtil::get_function_parameters(
                &compiler,
                value_6.get_first_child(&compiler).unwrap(),
            )
            .get_first_child(&compiler)
            .unwrap(),
        );
        let value_7 = parse(&mut compiler, "function f(x = 3) {}");
        let x = NodeUtil::get_function_parameters(
            &compiler,
            value_7.get_first_child(&compiler).unwrap(),
        )
        .get_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler);
        assert_l_value_named_x(&compiler, x.unwrap());
        let value_8 = parse(&mut compiler, "({x} = obj)");
        assert_l_value_named_x(
            &compiler,
            value_8
                .get_first_first_child(&compiler)
                .unwrap()
                .get_first_first_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
        );
        let value_9 = parse(&mut compiler, "([x] = obj)");
        assert_l_value_named_x(
            &compiler,
            value_9
                .get_first_first_child(&compiler)
                .unwrap()
                .get_first_first_child(&compiler)
                .unwrap(),
        );
        let value_10 = parse(&mut compiler, "function foo (...x) {}");
        assert_l_value_named_x(
            &compiler,
            value_10
                .get_first_child(&compiler)
                .unwrap()
                .get_second_child(&compiler)
                .unwrap()
                .get_first_first_child(&compiler)
                .unwrap(),
        );
        let value_11 = parse(&mut compiler, "({[0]: x} = obj)");
        assert_l_value_named_x(
            &compiler,
            value_11
                .get_first_first_child(&compiler)
                .unwrap()
                .get_first_first_child(&compiler)
                .unwrap()
                .get_second_child(&compiler)
                .unwrap(),
        );
    }

    // port: NodeUtilTest.AssortedTests#testIsNotLValue
    #[test]
    fn test_is_not_l_value() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var a = x;");
        assert_not_l_value_named_x(
            &compiler,
            value_1
                .get_first_first_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
        );
        let value_2 = parse(&mut compiler, "f(...x);");
        let x = value_2
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_last_child(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        assert_not_l_value_named_x(&compiler, x.unwrap());
        let value_3 = parse(&mut compiler, "var a = [...x];");
        let x = value_3
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        assert_not_l_value_named_x(&compiler, x.unwrap());
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_declaredName_doesNotThrow
    #[test]
    fn test_get_best_js_doc_info_node_strict_declared_name_does_not_throw() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "/** some */ let A;");
        let a_name = value_1.get_first_first_child(&compiler);
        assert!(a_name.unwrap().is_name(&compiler));
        let best_js_doc_info_node = NodeUtil::get_best_jsdoc_info_node(&compiler, a_name.unwrap());
        assert!(best_js_doc_info_node.is_some());
        assert!((best_js_doc_info_node.unwrap() == a_name.unwrap().get_parent(&compiler).unwrap()));
        let best_js_doc_info_node_strict =
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, a_name.unwrap());
        assert!(best_js_doc_info_node_strict.is_some());
        assert!(
            (best_js_doc_info_node_strict.unwrap()
                == a_name.unwrap().get_parent(&compiler).unwrap())
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_classDeclaration_doesNotThrow
    #[test]
    fn test_get_best_js_doc_info_node_strict_class_declaration_does_not_throw() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "/** some */ class A {}");
        let class_node = value_1.get_first_child(&compiler);
        assert!(class_node.unwrap().is_class(&compiler));
        let best_js_doc_info_node =
            NodeUtil::get_best_jsdoc_info_node(&compiler, class_node.unwrap());
        assert!(best_js_doc_info_node.is_some());
        assert!((best_js_doc_info_node.unwrap() == class_node.unwrap()));
        let best_js_doc_info_node_strict =
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, class_node.unwrap());
        assert!(best_js_doc_info_node_strict.is_some());
        assert!((best_js_doc_info_node_strict.unwrap() == class_node.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_rhsNamedClassExpression_doesNotThrow
    #[test]
    fn test_get_best_js_doc_info_node_strict_rhs_named_class_expression_does_not_throw() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "/** some */ const x = class A {}");
        let const_node = value_1.get_first_child(&compiler);
        let x_name = const_node.unwrap().get_first_child(&compiler);
        let class_node = x_name.unwrap().get_first_child(&compiler);
        assert!(class_node.unwrap().is_class(&compiler));
        let best_js_doc_info_node =
            NodeUtil::get_best_jsdoc_info_node(&compiler, class_node.unwrap());
        assert!(best_js_doc_info_node.is_some());
        assert!((best_js_doc_info_node.unwrap() == const_node.unwrap()));
        let best_js_doc_info_node_strict =
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, class_node.unwrap());
        assert!(best_js_doc_info_node_strict.is_some());
        assert!((best_js_doc_info_node_strict.unwrap() == const_node.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_rhsClassName_doesNotThrow
    #[test]
    fn test_get_best_js_doc_info_node_strict_rhs_class_name_does_not_throw() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "/** some */ const x = class A {}");
        let const_node = value_1.get_first_child(&compiler);
        let x_name = const_node.unwrap().get_first_child(&compiler);
        let class_node = x_name.unwrap().get_first_child(&compiler);
        assert!(class_node.unwrap().is_class(&compiler));
        let class_name = class_node.unwrap().get_first_child(&compiler);
        assert!(class_name.unwrap().is_name(&compiler));
        let best_js_doc_info_node =
            NodeUtil::get_best_jsdoc_info_node(&compiler, class_name.unwrap());
        assert!(best_js_doc_info_node.is_some());
        assert!((best_js_doc_info_node.unwrap() == const_node.unwrap()));
        let best_js_doc_info_node_strict =
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, class_name.unwrap());
        assert!(best_js_doc_info_node_strict.is_some());
        assert!((best_js_doc_info_node_strict.unwrap() == const_node.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_rhsNamedFunctionExpression_doesNotThrow
    #[test]
    fn test_get_best_js_doc_info_node_strict_rhs_named_function_expression_does_not_throw() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "/** some */ const x = function A() {}");
        let const_node = value_1.get_first_child(&compiler);
        let x_name = const_node.unwrap().get_first_child(&compiler);
        let function_node = x_name.unwrap().get_first_child(&compiler);
        assert!(function_node.unwrap().is_function(&compiler));
        let best_js_doc_info_node =
            NodeUtil::get_best_jsdoc_info_node(&compiler, function_node.unwrap());
        assert!(best_js_doc_info_node.is_some());
        assert!((best_js_doc_info_node.unwrap() == const_node.unwrap()));
        let best_js_doc_info_node_strict =
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, function_node.unwrap());
        assert!(best_js_doc_info_node_strict.is_some());
        assert!((best_js_doc_info_node_strict.unwrap() == const_node.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_exportedFunction_doesNotThrow
    #[test]
    fn test_get_best_js_doc_info_node_strict_exported_function_does_not_throw() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "/** some */ export function A() {}");
        let export_node = value_1.get_first_first_child(&compiler);
        assert!(export_node.unwrap().is_export(&compiler));
        let function_node = export_node.unwrap().get_first_child(&compiler);
        assert!(function_node.unwrap().is_function(&compiler));
        let best_js_doc_info_node =
            NodeUtil::get_best_jsdoc_info_node(&compiler, function_node.unwrap());
        assert!(best_js_doc_info_node.is_some());
        assert!((best_js_doc_info_node.unwrap() == function_node.unwrap()));
        let best_js_doc_info_node_strict =
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, function_node.unwrap());
        assert!(best_js_doc_info_node_strict.is_some());
        assert!((best_js_doc_info_node_strict.unwrap() == function_node.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_exportNode_doesNotThrow
    #[test]
    fn test_get_best_js_doc_info_node_strict_export_node_does_not_throw() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "/** some */ export function A() {}");
        let export_node = value_1.get_first_first_child(&compiler);
        assert!(export_node.unwrap().is_export(&compiler));
        let function_node = export_node.unwrap().get_first_child(&compiler);
        assert!(function_node.unwrap().is_function(&compiler));
        let best_js_doc_info_node =
            NodeUtil::get_best_jsdoc_info_node(&compiler, export_node.unwrap());
        assert!(best_js_doc_info_node.is_some());
        assert!((best_js_doc_info_node.unwrap() == export_node.unwrap()));
        let best_js_doc_info_node_strict =
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, export_node.unwrap());
        assert!(best_js_doc_info_node_strict.is_some());
        assert!((best_js_doc_info_node_strict.unwrap() == export_node.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_rhsFunctionName_doesNotThrow
    #[test]
    fn test_get_best_js_doc_info_node_strict_rhs_function_name_does_not_throw() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "/** some */ const x = function A() {}");
        let const_node = value_1.get_first_child(&compiler);
        let x_name = const_node.unwrap().get_first_child(&compiler);
        let function_node = x_name.unwrap().get_first_child(&compiler);
        assert!(function_node.unwrap().is_function(&compiler));
        let function_name = function_node.unwrap().get_first_child(&compiler);
        assert!(function_name.unwrap().is_name(&compiler));
        let best_js_doc_info_node =
            NodeUtil::get_best_jsdoc_info_node(&compiler, function_name.unwrap());
        assert!(best_js_doc_info_node.is_some());
        assert!((best_js_doc_info_node.unwrap() == const_node.unwrap()));
        let best_js_doc_info_node_strict =
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, function_name.unwrap());
        assert!(best_js_doc_info_node_strict.is_some());
        assert!((best_js_doc_info_node_strict.unwrap() == const_node.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testIsConstantDeclaration
    #[test]
    fn test_is_constant_declaration() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "var x = 1;");
        assert_is_constant_declaration(
            &compiler,
            false,
            value_1.get_first_first_child(&compiler).unwrap(),
        );
        let value_2 = parse(&mut compiler, "let x = 1;");
        assert_is_constant_declaration(
            &compiler,
            false,
            value_2.get_first_first_child(&compiler).unwrap(),
        );
        let value_3 = parse(&mut compiler, "const x = 1;");
        assert_is_constant_declaration(
            &compiler,
            true,
            value_3.get_first_first_child(&compiler).unwrap(),
        );
        let value_4 = parse(&mut compiler, "/** @const */ var x = 1;");
        assert_is_constant_declaration(
            &compiler,
            true,
            value_4.get_first_first_child(&compiler).unwrap(),
        );
        let value_5 = parse(&mut compiler, "var /** @const */ x = 1;");
        assert_is_constant_declaration(
            &compiler,
            true,
            value_5.get_first_first_child(&compiler).unwrap(),
        );
        let value_6 = parse(&mut compiler, "var x, /** @const */ y = 1;");
        assert_is_constant_declaration(
            &compiler,
            false,
            value_6.get_first_first_child(&compiler).unwrap(),
        );
        let value_7 = get_name_node_from(&mut compiler, "const [a] = [];", "a");
        assert_is_constant_declaration(&compiler, true, value_7);
        let value_8 = get_name_node_from(&mut compiler, "const [[[a]]] = [];", "a");
        assert_is_constant_declaration(&compiler, true, value_8);
        let value_9 = get_name_node_from(&mut compiler, "const [a = 1] = [];", "a");
        assert_is_constant_declaration(&compiler, true, value_9);
        let value_10 = get_name_node_from(&mut compiler, "const [...a] = [];", "a");
        assert_is_constant_declaration(&compiler, true, value_10);
        let value_11 = get_name_node_from(&mut compiler, "const {a} = {};", "a");
        assert_is_constant_declaration(&compiler, true, value_11);
        let value_12 = get_name_node_from(&mut compiler, "const {a = 1} = {};", "a");
        assert_is_constant_declaration(&compiler, true, value_12);
        let value_13 = get_name_node_from(&mut compiler, "const {b: a} = {};", "a");
        assert_is_constant_declaration(&compiler, true, value_13);
        let value_14 = get_name_node_from(&mut compiler, "const {[3]: a} = {};", "a");
        assert_is_constant_declaration(&compiler, true, value_14);
        let value_15 = get_name_node_from(&mut compiler, "const {a: [a]} = {};", "a");
        assert_is_constant_declaration(&compiler, true, value_15);
        let value_16 =
            get_name_node_from(&mut compiler, "/** @const */ var x = function a() {};", "a");
        assert_is_constant_declaration(&compiler, true, value_16);
        let value_17 = get_name_node_from(&mut compiler, "var FOO = 1;", "FOO");
        assert_is_constant_declaration(&compiler, false, value_17);
        let value_18 = construct_inferred_constant_declaration(&mut compiler);
        assert_is_constant_declaration(&compiler, true, value_18);
        let value_19 = parse(&mut compiler, "class C {x = 2;}");
        assert_is_constant_declaration(
            &compiler,
            false,
            value_19
                .get_first_child(&compiler)
                .unwrap()
                .get_last_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
        );
        let value_20 = parse(&mut compiler, "class C {/** @const */ x = 2;}");
        assert_is_constant_declaration(
            &compiler,
            true,
            value_20
                .get_first_child(&compiler)
                .unwrap()
                .get_last_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
        );
        let value_21 = parse(&mut compiler, "class C { [x] = 2;}");
        assert_is_constant_declaration(
            &compiler,
            false,
            value_21
                .get_first_child(&compiler)
                .unwrap()
                .get_last_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
        );
        let value_22 = parse(&mut compiler, "class C { /** @const */ [x] = 2;}");
        assert_is_constant_declaration(
            &compiler,
            true,
            value_22
                .get_first_child(&compiler)
                .unwrap()
                .get_last_child(&compiler)
                .unwrap()
                .get_first_child(&compiler)
                .unwrap(),
        );
    }

    // port: NodeUtilTest.AssortedTests#testIsConstantDeclarations_FunctionClassLiterals
    #[test]
    fn test_is_constant_declarations_function_class_literals() {
        let mut compiler = Compiler::new();
        let value_1 = get_name_node_from(&mut compiler, "function Foo() {}", "Foo");
        assert_is_constant_declaration(&compiler, false, value_1);
        let value_2 = get_name_node_from(&mut compiler, "class Foo {}", "Foo");
        assert_is_constant_declaration(&compiler, false, value_2);
    }

    // port: NodeUtilTest.AssortedTests#testIsConstantDeclaration_qnames
    #[test]
    fn test_is_constant_declaration_qnames() {
        let mut compiler = Compiler::new();
        let const_assignment = parse(&mut compiler, "/** @const */ x.y = a.b;");
        let assign = const_assignment.get_first_first_child(&compiler);
        assert_is_constant_declaration(
            &compiler,
            true,
            assign.unwrap().get_first_child(&compiler).unwrap(),
        );
        let const_by_convention_assignment = parse(&mut compiler, "x.Y = a.b;");
        let assign = const_by_convention_assignment.get_first_first_child(&compiler);
        assert_is_constant_declaration(
            &compiler,
            false,
            assign.unwrap().get_first_child(&compiler).unwrap(),
        );
        let non_const_assignment = parse(&mut compiler, "x.y = a.b;");
        let assign = non_const_assignment.get_first_first_child(&compiler);
        assert_is_constant_declaration(
            &compiler,
            false,
            assign.unwrap().get_first_child(&compiler).unwrap(),
        );
        let expression = parse(&mut compiler, "/** @const */ x.y;");
        assert_is_constant_declaration(
            &compiler,
            true,
            expression.get_first_first_child(&compiler).unwrap(),
        );
    }

    // port: NodeUtilTest.AssortedTests#testIsConstantDeclaration_keys
    #[test]
    fn test_is_constant_declaration_keys() {
        let mut compiler = Compiler::new();
        let value_1 =
            get_string_key_node_from(&mut compiler, "const ns = {/** @const */ x: y};", "x");
        assert_is_constant_declaration(&compiler, true, value_1);
        let value_2 =
            get_string_key_node_from(&mut compiler, "/** @const */ const ns = {x: y};", "x");
        assert_is_constant_declaration(&compiler, false, value_2);
        let value_3 = get_string_key_node_from(&mut compiler, "const ns = {x: y};", "x");
        assert_is_constant_declaration(&compiler, false, value_3);
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring1
    #[test]
    fn test_destructuring1() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var [a, b] = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let name_node_b = name_node_a.unwrap().get_next(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "obj"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_b.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring1b
    #[test]
    fn test_destructuring1b() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {a: c, b: d} = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let str_key_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let str_key_node_b = str_key_node_a.unwrap().get_next(&compiler);
        let name_node_c = str_key_node_a.unwrap().get_first_child(&compiler);
        let name_node_d = str_key_node_b.unwrap().get_first_child(&compiler);
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((str_key_node_b.unwrap().get_string(&compiler) == "b"));
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        assert!((name_node_d.unwrap().get_string(&compiler) == "d"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "obj"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, str_key_node_b.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_d.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_c.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_d.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_c.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_d.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring1c
    #[test]
    fn test_destructuring1c() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {a, b} = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let str_key_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let str_key_node_b = str_key_node_a.unwrap().get_next(&compiler);
        let name_node_a = str_key_node_a.unwrap().get_first_child(&compiler);
        let name_node_b = str_key_node_b.unwrap().get_first_child(&compiler);
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((str_key_node_b.unwrap().get_string(&compiler) == "b"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "obj"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, str_key_node_b.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_b.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring1d
    #[test]
    fn test_destructuring1d() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {a = defaultValue} = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let str_key_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        assert!(str_key_node_a.unwrap().is_string_key(&compiler));
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        let default_node_a = str_key_node_a.unwrap().get_first_child(&compiler);
        assert!(default_node_a.unwrap().is_default_value(&compiler));
        let name_node_a = default_node_a.unwrap().get_first_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        let name_node_default = default_node_a.unwrap().get_second_child(&compiler);
        assert!((name_node_default.unwrap().get_string(&compiler) == "defaultValue"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "obj"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, default_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_default.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring1e
    #[test]
    fn test_destructuring1e() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {a: b = defaultValue} = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let str_key_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        let default_node_a = str_key_node_a.unwrap().get_only_child(&compiler);
        assert!(default_node_a.is_default_value(&compiler));
        let name_node_b = default_node_a.get_first_child(&compiler);
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        let name_node_default_value = default_node_a.get_second_child(&compiler);
        assert!((name_node_default_value.unwrap().get_string(&compiler) == "defaultValue"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "obj"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_default_value.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_b.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring1f
    #[test]
    fn test_destructuring1f() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var [a  = defaultValue] = arr;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let default_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        assert!(default_node_a.unwrap().is_default_value(&compiler));
        let name_node_a = default_node_a.unwrap().get_first_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        let name_node_default = default_node_a.unwrap().get_second_child(&compiler);
        assert!((name_node_default.unwrap().get_string(&compiler) == "defaultValue"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "arr"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_default.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring2
    #[test]
    fn test_destructuring2() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var [a, [b, c]] = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let inner_pat = name_node_a.unwrap().get_next(&compiler);
        let name_node_b = inner_pat.unwrap().get_first_child(&compiler);
        let name_node_c = name_node_b.unwrap().get_next(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "obj"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, inner_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_c.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_b.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_c.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring2b
    #[test]
    fn test_destructuring2b() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {a: e, b: {c: f, d: g}} = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let str_key_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let str_key_node_b = str_key_node_a.unwrap().get_next(&compiler);
        let inner_pat = str_key_node_b.unwrap().get_only_child(&compiler);
        let str_key_node_c = inner_pat.get_first_child(&compiler);
        let str_key_node_d = str_key_node_c.unwrap().get_next(&compiler);
        let name_node_e = str_key_node_a.unwrap().get_first_child(&compiler);
        let name_node_f = str_key_node_c.unwrap().get_first_child(&compiler);
        let name_node_g = str_key_node_d.unwrap().get_first_child(&compiler);
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((str_key_node_b.unwrap().get_string(&compiler) == "b"));
        assert!((str_key_node_c.unwrap().get_string(&compiler) == "c"));
        assert!((str_key_node_d.unwrap().get_string(&compiler) == "d"));
        assert!((name_node_e.unwrap().get_string(&compiler) == "e"));
        assert!((name_node_f.unwrap().get_string(&compiler) == "f"));
        assert!((name_node_g.unwrap().get_string(&compiler) == "g"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "obj"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, str_key_node_b.unwrap());
        assert_not_lhs_by_destructuring(&compiler, str_key_node_c.unwrap());
        assert_not_lhs_by_destructuring(&compiler, str_key_node_d.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_e.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_f.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_g.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_e.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, inner_pat),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_f.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_g.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_e.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_f.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_g.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring3
    #[test]
    fn test_destructuring3() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var [a, b] = [c, d];");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let name_node_b = name_node_a.unwrap().get_next(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        let name_node_c = destruct_lhs
            .unwrap()
            .get_last_child(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        let name_node_d = name_node_c.unwrap().get_next(&compiler);
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        assert!((name_node_d.unwrap().get_string(&compiler) == "d"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_d.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_b.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring3b
    #[test]
    fn test_destructuring3b() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {a: c, b: d} = {a: 1, b: 2};");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let str_key_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let str_key_node_b = str_key_node_a.unwrap().get_next(&compiler);
        let name_node_c = str_key_node_a.unwrap().get_first_child(&compiler);
        let name_node_d = str_key_node_b.unwrap().get_first_child(&compiler);
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((str_key_node_b.unwrap().get_string(&compiler) == "b"));
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        assert!((name_node_d.unwrap().get_string(&compiler) == "d"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, str_key_node_b.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_d.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_c.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_d.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_c.unwrap()),
            var_node.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_d.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring4
    #[test]
    fn test_destructuring4() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for ([a, b] of X){}");
        let destruct_pat = root.get_first_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let name_node_b = destruct_pat.unwrap().get_last_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            destruct_pat.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring5
    #[test]
    fn test_destructuring5() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "function fn([a, b] = [c, d]){}");
        let destruct_pat = root
            .get_first_child(&compiler)
            .unwrap()
            .get_second_child(&compiler)
            .unwrap()
            .get_first_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let name_node_b = destruct_pat.unwrap().get_last_child(&compiler);
        let name_node_c = destruct_pat
            .unwrap()
            .get_next(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        let name_node_d = destruct_pat
            .unwrap()
            .get_next(&compiler)
            .unwrap()
            .get_last_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        assert!((name_node_d.unwrap().get_string(&compiler) == "d"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_d.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            destruct_pat.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testRestParameter
    #[test]
    fn test_rest_parameter() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "function fn(a, b, ...rest){}");
        let parameter_list = root
            .get_first_child(&compiler)
            .unwrap()
            .get_second_child(&compiler);
        let name_node_a = parameter_list.unwrap().get_first_child(&compiler);
        let name_node_b = name_node_a.unwrap().get_next(&compiler);
        let rest_node = name_node_b.unwrap().get_next(&compiler);
        assert!(rest_node.unwrap().is_rest(&compiler));
        let name_node_rest = rest_node.unwrap().get_first_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            name_node_a.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            name_node_b.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_rest.unwrap()),
            name_node_rest.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            parameter_list.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_b.unwrap()),
            parameter_list.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_rest.unwrap()),
            parameter_list.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring6
    #[test]
    fn test_destructuring6() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for ([{a: b}] of c) {}");
        let for_of_node = root.get_first_child(&compiler);
        let for_of_target_pat = for_of_node.unwrap().get_first_child(&compiler);
        let inner_pat = for_of_target_pat.unwrap().get_first_child(&compiler);
        assert!(inner_pat.unwrap().is_object_pattern(&compiler));
        assert!(for_of_target_pat.unwrap().is_array_pattern(&compiler));
        let str_key_node_a = inner_pat.unwrap().get_first_child(&compiler);
        let name_node_b = str_key_node_a.unwrap().get_first_child(&compiler);
        let name_node_c = inner_pat
            .unwrap()
            .get_parent(&compiler)
            .unwrap()
            .get_next(&compiler);
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, for_of_target_pat.unwrap()),
            for_of_target_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, inner_pat.unwrap()),
            for_of_target_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            for_of_target_pat.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring6b
    #[test]
    fn test_destructuring6b() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for ([{a: b}] in c) {}");
        let for_in_node = root.get_first_child(&compiler);
        let for_in_target_pat = for_in_node.unwrap().get_first_child(&compiler);
        let inner_pat = for_in_target_pat.unwrap().get_first_child(&compiler);
        assert!(inner_pat.unwrap().is_object_pattern(&compiler));
        assert!(for_in_target_pat.unwrap().is_array_pattern(&compiler));
        let str_key_node_a = inner_pat.unwrap().get_first_child(&compiler);
        let name_node_b = str_key_node_a.unwrap().get_first_child(&compiler);
        let name_node_c = inner_pat
            .unwrap()
            .get_parent(&compiler)
            .unwrap()
            .get_next(&compiler);
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, for_in_target_pat.unwrap()),
            for_in_target_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, inner_pat.unwrap()),
            for_in_target_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            for_in_target_pat.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring6c
    #[test]
    fn test_destructuring6c() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for (var [{a: b}] = [{a: 1}];;) {}");
        let var_node = root.get_first_first_child(&compiler);
        let destruct_arr = var_node.unwrap().get_first_first_child(&compiler);
        assert!(destruct_arr.unwrap().is_array_pattern(&compiler));
        let destruct_pat = destruct_arr.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let str_key_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let name_node_b = str_key_node_a.unwrap().get_first_child(&compiler);
        assert!((str_key_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_b.unwrap().get_string(&compiler) == "b"));
        assert_not_lhs_by_destructuring(&compiler, str_key_node_a.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_b.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_arr.unwrap()),
            destruct_arr.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_arr.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_b.unwrap()),
            destruct_arr.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_b.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring7
    #[test]
    fn test_destructuring7() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for ([a] of c) {}");
        let destruct_pat = root.get_first_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let name_node_c = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring7b
    #[test]
    fn test_destructuring7b() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for ([a] in c) {}");
        let destruct_pat = root.get_first_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        let name_node_c = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert!((name_node_c.unwrap().get_string(&compiler) == "c"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_c.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring7c
    #[test]
    fn test_destructuring7c() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for (var [a] = [1];;) {}");
        let var_node = root.get_first_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring7d
    #[test]
    fn test_destructuring7d() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for (let [a] = [1];;) {}");
        let let_node = root.get_first_first_child(&compiler);
        let destruct_lhs = let_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            let_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring7e
    #[test]
    fn test_destructuring7e() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "for (const [a] = [1];;) {}");
        let const_node = root.get_first_first_child(&compiler);
        let destruct_lhs = const_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let name_node_a = destruct_pat.unwrap().get_first_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            const_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring8
    #[test]
    fn test_destructuring8() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var [...a] = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let rest_node = destruct_pat.unwrap().get_first_child(&compiler);
        assert!(rest_node.unwrap().is_rest(&compiler));
        let name_node_a = rest_node.unwrap().get_first_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring8b
    #[test]
    fn test_destructuring8b() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "([...this.x] = obj);");
        let assign = root.get_first_first_child(&compiler);
        assert!(assign.unwrap().is_assign(&compiler));
        let destruct_pat = assign.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_array_pattern(&compiler));
        let rest_node = destruct_pat.unwrap().get_first_child(&compiler);
        assert!(rest_node.unwrap().is_rest(&compiler));
        let get_prop = rest_node.unwrap().get_first_child(&compiler);
        assert!(get_prop.unwrap().is_get_prop(&compiler));
        assert_lhs_by_destructuring(&compiler, get_prop.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, get_prop.unwrap()),
            destruct_pat.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuring9
    #[test]
    fn test_destructuring9() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {['a']:a} = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let computed_prop_node = destruct_pat.unwrap().get_first_child(&compiler);
        assert!(computed_prop_node.unwrap().is_computed_prop(&compiler));
        let name_node_a = computed_prop_node.unwrap().get_last_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuringComputedPropertyWithDefault
    #[test]
    fn test_destructuring_computed_property_with_default() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {['a']: a = 1} = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let computed_prop_node = destruct_pat.unwrap().get_first_child(&compiler);
        assert!(computed_prop_node.unwrap().is_computed_prop(&compiler));
        let default_value_node = computed_prop_node.unwrap().get_last_child(&compiler);
        let name_node_a = default_value_node.unwrap().get_first_child(&compiler);
        assert!((name_node_a.unwrap().get_string(&compiler) == "a"));
        assert_lhs_by_destructuring(&compiler, name_node_a.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuringWithArrayPatternInCatch
    #[test]
    fn test_destructuring_with_array_pattern_in_catch() {
        let mut compiler = Compiler::new();
        let actual = parse(&mut compiler, "try {} catch([a]) {}");
        let try_node = actual.get_first_child(&compiler);
        let catch_block = try_node.unwrap().get_second_child(&compiler);
        let catch_node = catch_block.unwrap().get_first_child(&compiler);
        assert_eq!(catch_node.unwrap().get_token(&compiler), Token::CATCH);
        let array_pattern = catch_node.unwrap().get_first_child(&compiler);
        let name_node_a = array_pattern.unwrap().get_only_child(&compiler);
        assert_eq!(name_node_a.get_token(&compiler), Token::NAME);
        assert_eq!(name_node_a.get_string(&compiler), "a");
        assert_lhs_by_destructuring(&compiler, name_node_a);
        assert_eq!(
            NodeUtil::get_root_target(&compiler, array_pattern.unwrap()),
            array_pattern.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_a),
            array_pattern.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_a),
            catch_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testDestructuringObjectRest
    #[test]
    fn test_destructuring_object_rest() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "var {...rest} = obj;");
        let var_node = root.get_first_child(&compiler);
        let destruct_lhs = var_node.unwrap().get_first_child(&compiler);
        assert!(destruct_lhs.unwrap().is_destructuring_lhs(&compiler));
        let destruct_pat = destruct_lhs.unwrap().get_first_child(&compiler);
        assert!(destruct_pat.unwrap().is_object_pattern(&compiler));
        let rest_node = destruct_pat.unwrap().get_first_child(&compiler);
        let name_node_rest = rest_node.unwrap().get_first_child(&compiler);
        assert!((name_node_rest.unwrap().get_string(&compiler) == "rest"));
        let name_node_obj = destruct_pat.unwrap().get_next(&compiler);
        assert!((name_node_obj.unwrap().get_string(&compiler) == "obj"));
        assert_not_lhs_by_destructuring(&compiler, rest_node.unwrap());
        assert_lhs_by_destructuring(&compiler, name_node_rest.unwrap());
        assert_not_lhs_by_destructuring(&compiler, name_node_obj.unwrap());
        assert_eq!(
            NodeUtil::get_root_target(&compiler, destruct_pat.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_root_target(&compiler, name_node_rest.unwrap()),
            destruct_pat.unwrap()
        );
        assert_eq!(
            NodeUtil::get_declaring_parent(&compiler, name_node_rest.unwrap()),
            var_node.unwrap()
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoForClasses
    #[test]
    fn test_get_best_js_doc_info_for_classes() {
        let mut compiler = Compiler::new();
        let class_node = parse_first(&mut compiler, Token::CLASS, "/** @export */ class Foo {}");
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, class_node)
                .unwrap()
                .is_export()
        );
        let class_node = parse_first(
            &mut compiler,
            Token::CLASS,
            "/** @export */ var Foo = class {}",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, class_node)
                .unwrap()
                .is_export()
        );
        let class_node = parse_first(
            &mut compiler,
            Token::CLASS,
            "/** @export */ var Foo = class Bar {}",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, class_node)
                .unwrap()
                .is_export()
        );
        let class_node = parse_first(
            &mut compiler,
            Token::CLASS,
            "var Foo_1; /** @export */ let Foo = Foo_1 = class Foo {}",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, class_node)
                .unwrap()
                .is_export()
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoForMethods
    #[test]
    fn test_get_best_js_doc_info_for_methods() {
        let mut compiler = Compiler::new();
        let function = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "class C { /** @export */ foo() {} }",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, function)
                .unwrap()
                .is_export()
        );
        let function = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "class C { /** @export */ [computedMethod]() {} }",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, function)
                .unwrap()
                .is_export()
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoExport
    #[test]
    fn test_get_best_js_doc_info_export() {
        let mut compiler = Compiler::new();
        let class_node = parse_first(
            &mut compiler,
            Token::CLASS,
            "/** @constructor */ export class Foo {}",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, class_node)
                .unwrap()
                .is_constructor()
        );
        let function = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "/** @constructor */ export function Foo() {}",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, function)
                .unwrap()
                .is_constructor()
        );
        let function = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "/** @constructor */ export var Foo = function() {}",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, function)
                .unwrap()
                .is_constructor()
        );
        let function = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "/** @constructor */ export let Foo = function() {}",
        );
        assert!(
            NodeUtil::get_best_jsdoc_info(&compiler, function)
                .unwrap()
                .is_constructor()
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetDeclaredTypeExpression1
    #[test]
    fn test_get_declared_type_expression1() {
        let mut compiler = Compiler::new();
        let ast = parse(&mut compiler, "function f(/** string */ x) {}");
        let x = get_name_node(&compiler, ast, "x");
        let type_expr = NodeUtil::get_declared_type_expression(&compiler, x);
        assert_eq!(
            type_expr.as_ref().unwrap().get_root().get_string(&compiler),
            "string"
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetDeclaredTypeExpression2
    #[test]
    fn test_get_declared_type_expression2() {
        let mut compiler = Compiler::new();
        let ast = parse(&mut compiler, "/** @param {string} x */ function f(x) {}");
        let x = get_name_node(&compiler, ast, "x");
        let type_expr = NodeUtil::get_declared_type_expression(&compiler, x);
        assert_eq!(
            type_expr.as_ref().unwrap().get_root().get_string(&compiler),
            "string"
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetDeclaredTypeExpression3
    #[test]
    fn test_get_declared_type_expression3() {
        let mut compiler = Compiler::new();
        let ast = parse(
            &mut compiler,
            "/** @param {...number} x */ function f(...x) {}",
        );
        let x = get_name_node(&compiler, ast, "x");
        let type_expr = NodeUtil::get_declared_type_expression(&compiler, x);
        assert_eq!(
            type_expr.as_ref().unwrap().get_root().get_token(&compiler),
            Token::ITER_REST
        );
        assert_eq!(
            type_expr
                .as_ref()
                .unwrap()
                .get_root()
                .get_first_child(&compiler)
                .unwrap()
                .get_string(&compiler),
            "number"
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetDeclaredTypeExpression4
    #[test]
    fn test_get_declared_type_expression4() {
        let mut compiler = Compiler::new();
        let ast = parse(
            &mut compiler,
            "/** @param {number=} x */ function f(x = -1) {}",
        );
        let x = get_name_node(&compiler, ast, "x");
        let type_expr = NodeUtil::get_declared_type_expression(&compiler, x);
        assert_eq!(
            type_expr.as_ref().unwrap().get_root().get_token(&compiler),
            Token::EQUALS
        );
        assert_eq!(
            type_expr
                .as_ref()
                .unwrap()
                .get_root()
                .get_first_child(&compiler)
                .unwrap()
                .get_string(&compiler),
            "number"
        );
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInNodeWithNameDeclaration
    #[test]
    fn test_find_lhs_nodes_in_node_with_name_declaration() {
        let mut compiler = Compiler::new();
        let value_1 = visit_lhs_nodes_in_node(&mut compiler, "var x;");
        assert_eq!(value_1.len(), 1);
        let value_2 = visit_lhs_nodes_in_node(&mut compiler, "var x, y;");
        assert_eq!(value_2.len(), 2);
        let value_3 = visit_lhs_nodes_in_node(&mut compiler, "var f = function(x, y, z) {};");
        assert_eq!(value_3.len(), 1);
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInNodeWithArrayPatternDeclaration
    #[test]
    fn test_find_lhs_nodes_in_node_with_array_pattern_declaration() {
        let mut compiler = Compiler::new();
        let value_1 = visit_lhs_nodes_in_node(&mut compiler, "var [x=a => a, y = b=>b+1] = arr;");
        assert_eq!(value_1.len(), 2);
        let value_2 =
            visit_lhs_nodes_in_node(&mut compiler, "var [x=a => a, y = b=>b+1, ...z] = arr;");
        assert_eq!(value_2.len(), 3);
        let value_3 =
            visit_lhs_nodes_in_node(&mut compiler, "var [ , , , y = b=>b+1, ...z] = arr;");
        assert_eq!(value_3.len(), 2);
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInNodeWithObjectPatternDeclaration
    #[test]
    fn test_find_lhs_nodes_in_node_with_object_pattern_declaration() {
        let mut compiler = Compiler::new();
        let value_1 = visit_lhs_nodes_in_node(&mut compiler, "var {x = a=>a, y = b=>b+1} = obj;");
        assert_eq!(value_1.len(), 2);
        let value_2 =
            visit_lhs_nodes_in_node(&mut compiler, "var {p1: x = a=>a, p2: y = b=>b+1} = obj;");
        assert_eq!(value_2.len(), 2);
        let value_3 =
            visit_lhs_nodes_in_node(&mut compiler, "var {[pname]: x = a=>a, [p2name]: y} = obj;");
        assert_eq!(value_3.len(), 2);
        let value_4 = visit_lhs_nodes_in_node(
            &mut compiler,
            "var {lhs1 = a, p2: [lhs2, lhs3 = b] = [notlhs]} = obj;",
        );
        assert_eq!(value_4.len(), 3);
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInNodeWithCastOnLhs
    #[test]
    fn test_find_lhs_nodes_in_node_with_cast_on_lhs() {
        let mut compiler = Compiler::new();
        let lhs_nodes = visit_lhs_nodes_in_node(&mut compiler, "/** @type {*} */ (a.b) = 3;");
        assert_eq!(lhs_nodes.len(), 1);
        let mut node_iterator = lhs_nodes.into_iter();
        let value_2 = node_iterator.next().unwrap();
        assert!(value_2.matches_qualified_name(&compiler, "a.b"));
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInNodeWithArrayPatternAssign
    #[test]
    fn test_find_lhs_nodes_in_node_with_array_pattern_assign() {
        let mut compiler = Compiler::new();
        let value_1 = visit_lhs_nodes_in_node(&mut compiler, "[this.x] = rhs;");
        assert_eq!(value_1.len(), 1);
        let value_2 = visit_lhs_nodes_in_node(&mut compiler, "[this.x, y] = rhs;");
        assert_eq!(value_2.len(), 2);
        let value_3 = visit_lhs_nodes_in_node(&mut compiler, "[this.x, y, this.z] = rhs;");
        assert_eq!(value_3.len(), 3);
        let value_4 = visit_lhs_nodes_in_node(&mut compiler, "[y, this.z] = rhs;");
        assert_eq!(value_4.len(), 2);
        let value_5 = visit_lhs_nodes_in_node(&mut compiler, "[x[y]] = rhs;");
        assert_eq!(value_5.len(), 1);
        let value_6 = visit_lhs_nodes_in_node(&mut compiler, "[x.y.z] = rhs;");
        assert_eq!(value_6.len(), 1);
        let value_7 = visit_lhs_nodes_in_node(&mut compiler, "[ /** @type {*} */ (x.y.z) ] = rhs;");
        assert_eq!(value_7.len(), 1);
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInNodeWithComplexAssign
    #[test]
    fn test_find_lhs_nodes_in_node_with_complex_assign() {
        let mut compiler = Compiler::new();
        let value_1 = visit_lhs_nodes_in_node(&mut compiler, "x += 1;");
        assert_eq!(value_1.len(), 1);
        let value_2 = visit_lhs_nodes_in_node(&mut compiler, "x.y += 1;");
        assert_eq!(value_2.len(), 1);
        let value_3 = visit_lhs_nodes_in_node(&mut compiler, "x -= 1;");
        assert_eq!(value_3.len(), 1);
        let value_4 = visit_lhs_nodes_in_node(&mut compiler, "x.y -= 1;");
        assert_eq!(value_4.len(), 1);
        let value_5 = visit_lhs_nodes_in_node(&mut compiler, "x *= 2;");
        assert_eq!(value_5.len(), 1);
        let value_6 = visit_lhs_nodes_in_node(&mut compiler, "x.y *= 2;");
        assert_eq!(value_6.len(), 1);
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInForOfWithDeclaration
    #[test]
    fn test_find_lhs_nodes_in_for_of_with_declaration() {
        let mut compiler = Compiler::new();
        let lhs_nodes = visit_lhs_nodes_in_node(&mut compiler, "for (const {x, y} of iterable) {}");
        assert_eq!(lhs_nodes.len(), 2);
        let mut node_iterator = lhs_nodes.into_iter();
        let value_2 = node_iterator.next().unwrap();
        assert_eq!(value_2.get_token(&compiler), Token::NAME);
        assert_eq!(value_2.get_string(&compiler), "x");
        let value_3 = node_iterator.next().unwrap();
        assert_eq!(value_3.get_token(&compiler), Token::NAME);
        assert_eq!(value_3.get_string(&compiler), "y");
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInForOfWithoutDeclaration
    #[test]
    fn test_find_lhs_nodes_in_for_of_without_declaration() {
        let mut compiler = Compiler::new();
        let lhs_nodes = visit_lhs_nodes_in_node(&mut compiler, "for ({x, y: a.b} of iterable) {}");
        assert_eq!(lhs_nodes.len(), 2);
        let mut node_iterator = lhs_nodes.into_iter();
        let value_2 = node_iterator.next().unwrap();
        assert_eq!(value_2.get_token(&compiler), Token::NAME);
        assert_eq!(value_2.get_string(&compiler), "x");
        let value_3 = node_iterator.next().unwrap();
        assert!(value_3.matches_qualified_name(&compiler, "a.b"));
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInForInWithDeclaration
    #[test]
    fn test_find_lhs_nodes_in_for_in_with_declaration() {
        let mut compiler = Compiler::new();
        let lhs_nodes = visit_lhs_nodes_in_node(&mut compiler, "for (const x in obj) {}");
        assert_eq!(lhs_nodes.len(), 1);
        let mut node_iterator = lhs_nodes.into_iter();
        let value_2 = node_iterator.next().unwrap();
        assert_eq!(value_2.get_token(&compiler), Token::NAME);
        assert_eq!(value_2.get_string(&compiler), "x");
    }

    // port: NodeUtilTest.AssortedTests#testFindLhsNodesInForInWithoutDeclaration
    #[test]
    fn test_find_lhs_nodes_in_for_in_without_declaration() {
        let mut compiler = Compiler::new();
        let lhs_nodes = visit_lhs_nodes_in_node(&mut compiler, "for (a.b in iterable) {}");
        assert_eq!(lhs_nodes.len(), 1);
        let mut node_iterator = lhs_nodes.into_iter();
        let value_2 = node_iterator.next().unwrap();
        assert!(value_2.matches_qualified_name(&compiler, "a.b"));
    }

    // port: NodeUtilTest.AssortedTests#testIsConstructor
    #[test]
    fn test_is_constructor() {
        let mut compiler = Compiler::new();
        let registry = JSTypeRegistry::new(&mut compiler, Box::new(NullErrorReporter), Vec::new());
        let value_1 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "/** @constructor */ function Foo() {}",
        );
        assert!(NodeUtil::is_constructor(
            &compiler,
            Some(value_1),
            &registry
        ));
        let value_2 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "/** @constructor */ var Foo = function() {}",
        );
        assert!(NodeUtil::is_constructor(
            &compiler,
            Some(value_2),
            &registry
        ));
        let value_3 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "var x = {}; /** @constructor */ x.Foo = function() {}",
        );
        assert!(NodeUtil::is_constructor(
            &compiler,
            Some(value_3),
            &registry
        ));
        let value_4 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "class Foo { constructor() {} }",
        );
        assert!(NodeUtil::is_constructor(
            &compiler,
            Some(value_4),
            &registry
        ));
        let value_5 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "class Foo { static constructor() {} }",
        );
        assert!(!NodeUtil::is_constructor(
            &compiler,
            Some(value_5),
            &registry
        ));
        let value_6 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "let Foo = { constructor() {} }",
        );
        assert!(!NodeUtil::is_constructor(
            &compiler,
            Some(value_6),
            &registry
        ));
        let value_7 = parse_first(&mut compiler, Token::FUNCTION, "function Foo() {}");
        assert!(!NodeUtil::is_constructor(
            &compiler,
            Some(value_7),
            &registry
        ));
        let value_8 = parse_first(&mut compiler, Token::FUNCTION, "var Foo = function() {}");
        assert!(!NodeUtil::is_constructor(
            &compiler,
            Some(value_8),
            &registry
        ));
        let value_9 = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "var x = {}; x.Foo = function() {};",
        );
        assert!(!NodeUtil::is_constructor(
            &compiler,
            Some(value_9),
            &registry
        ));
        let value_10 = parse_first(&mut compiler, Token::FUNCTION, "function constructor() {}");
        assert!(!NodeUtil::is_constructor(
            &compiler,
            Some(value_10),
            &registry
        ));
        let value_11 = parse_first(&mut compiler, Token::FUNCTION, "class Foo { bar() {} }");
        assert!(!NodeUtil::is_constructor(
            &compiler,
            Some(value_11),
            &registry
        ));
    }

    // port: NodeUtilTest.AssortedTests#testGetEs6ClassConstructorMemberFunctionDef
    #[test]
    fn test_get_es6_class_constructor_member_function_def() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(
            &mut compiler,
            Token::CLASS,
            "class Foo { method() {} constructor() {} static constructor() {} };",
        );
        let constructor_member_function_def =
            NodeUtil::get_es6_class_constructor_member_function_def(&compiler, value_1);
        assert_eq!(
            constructor_member_function_def
                .unwrap()
                .get_token(&compiler),
            Token::MEMBER_FUNCTION_DEF
        );
        assert_eq!(
            constructor_member_function_def
                .unwrap()
                .get_string(&compiler),
            "constructor"
        );
        assert!(
            !constructor_member_function_def
                .unwrap()
                .is_static_member(&compiler)
        );
        let value_2 = parse_first(
            &mut compiler,
            Token::CLASS,
            "class Foo { method() {} static constructor() {} }",
        );
        let constructor_member_function_def =
            NodeUtil::get_es6_class_constructor_member_function_def(&compiler, value_2);
        assert!(constructor_member_function_def.is_none());
    }

    // port: NodeUtilTest.AssortedTests#testIsGetterOrSetter
    #[test]
    fn test_is_getter_or_setter() {
        let mut compiler = Compiler::new();
        let fn_node = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "Object.defineProperty(this, 'bar', {get: function() {}});",
        );
        assert!(NodeUtil::is_getter_or_setter(
            &compiler,
            fn_node.get_parent(&compiler).unwrap()
        ));
        let fn_node = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "Object.defineProperty(this, 'bar', {set: function() {}});",
        );
        assert!(NodeUtil::is_getter_or_setter(
            &compiler,
            fn_node.get_parent(&compiler).unwrap()
        ));
        let fn_node = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "Object.defineProperties(this, {bar: {get: function() {}}});",
        );
        assert!(NodeUtil::is_getter_or_setter(
            &compiler,
            fn_node.get_parent(&compiler).unwrap()
        ));
        let fn_node = parse_first(
            &mut compiler,
            Token::FUNCTION,
            "Object.defineProperties(this, {bar: {set: function() {}}});",
        );
        assert!(NodeUtil::is_getter_or_setter(
            &compiler,
            fn_node.get_parent(&compiler).unwrap()
        ));
        let fn_node = parse_first(&mut compiler, Token::FUNCTION, "var x = {get bar() {}};");
        assert!(NodeUtil::is_getter_or_setter(
            &compiler,
            fn_node.get_parent(&compiler).unwrap()
        ));
        let fn_node = parse_first(&mut compiler, Token::FUNCTION, "var x = {set bar(z) {}};");
        assert!(NodeUtil::is_getter_or_setter(
            &compiler,
            fn_node.get_parent(&compiler).unwrap()
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsObjectDefinePropertiesDefinition
    #[test]
    fn test_is_object_define_properties_definition() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(
            &mut compiler,
            Token::CALL,
            "Object.defineProperties(this, {});",
        );
        assert!(NodeUtil::is_object_define_properties_definition(
            &compiler, value_1
        ));
        let value_2 = parse_first(
            &mut compiler,
            Token::CALL,
            "Object.defineProperties(this, foo);",
        );
        assert!(NodeUtil::is_object_define_properties_definition(
            &compiler, value_2
        ));
        let value_3 = parse_first(
            &mut compiler,
            Token::CALL,
            "$jscomp.global.Object.defineProperties(this, foo);",
        );
        assert!(NodeUtil::is_object_define_properties_definition(
            &compiler, value_3
        ));
        let value_4 = parse_first(
            &mut compiler,
            Token::CALL,
            "$jscomp$global.Object.defineProperties(this, foo);",
        );
        assert!(NodeUtil::is_object_define_properties_definition(
            &compiler, value_4
        ));
        let value_5 = parse_first(
            &mut compiler,
            Token::CALL,
            "Object.defineProperties(this, {}, foo);",
        );
        assert!(!NodeUtil::is_object_define_properties_definition(
            &compiler, value_5
        ));
        let value_6 = parse_first(&mut compiler, Token::CALL, "Object.defineProperties(this);");
        assert!(!NodeUtil::is_object_define_properties_definition(
            &compiler, value_6
        ));
        let value_7 = parse_first(&mut compiler, Token::CALL, "Object.defineProperties();");
        assert!(!NodeUtil::is_object_define_properties_definition(
            &compiler, value_7
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsObjectDefinePropertyDefinition
    #[test]
    fn test_is_object_define_property_definition() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(
            &mut compiler,
            Token::CALL,
            "Object.defineProperty(this, 'foo', {});",
        );
        assert!(NodeUtil::is_object_define_property_definition(
            &compiler, value_1
        ));
        let value_2 = parse_first(
            &mut compiler,
            Token::CALL,
            "Object.defineProperty(this, 'foo', foo);",
        );
        assert!(NodeUtil::is_object_define_property_definition(
            &compiler, value_2
        ));
        let value_3 = parse_first(
            &mut compiler,
            Token::CALL,
            "$jscomp.global.Object.defineProperty(this, 'foo', foo);",
        );
        assert!(NodeUtil::is_object_define_property_definition(
            &compiler, value_3
        ));
        let value_4 = parse_first(
            &mut compiler,
            Token::CALL,
            "$jscomp$global.Object.defineProperty(this, 'foo', foo);",
        );
        assert!(NodeUtil::is_object_define_property_definition(
            &compiler, value_4
        ));
        let value_5 = parse_first(
            &mut compiler,
            Token::CALL,
            "Object.defineProperty(this, {});",
        );
        assert!(!NodeUtil::is_object_define_property_definition(
            &compiler, value_5
        ));
        let value_6 = parse_first(&mut compiler, Token::CALL, "Object.defineProperty(this);");
        assert!(!NodeUtil::is_object_define_property_definition(
            &compiler, value_6
        ));
        let value_7 = parse_first(&mut compiler, Token::CALL, "Object.defineProperty();");
        assert!(!NodeUtil::is_object_define_property_definition(
            &compiler, value_7
        ));
    }

    // port: NodeUtilTest.AssortedTests#testDoesFunctionReferenceOwnArgumentsObject
    #[test]
    fn test_does_function_reference_own_arguments_object() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "function() {return () => arguments}");
        assert!(NodeUtil::does_function_reference_own_arguments_object(
            &compiler, value_1
        ));
        let value_2 = parse_expr(&mut compiler, "function(a = arguments.length) {}");
        assert!(NodeUtil::does_function_reference_own_arguments_object(
            &compiler, value_2
        ));
        let value_3 = parse_expr(&mut compiler, "() => arguments");
        assert!(!NodeUtil::does_function_reference_own_arguments_object(
            &compiler, value_3
        ));
    }

    // port: NodeUtilTest.AssortedTests#testIsExpressionResultUsed
    #[test]
    fn test_is_expression_result_used() {
        let mut compiler = Compiler::new();
        let value_1 = get_name_node_from(&mut compiler, "for (x in y) z", "x");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_1));
        let value_2 = get_name_node_from(&mut compiler, "for (x in y) z", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_2));
        let value_3 = get_name_node_from(&mut compiler, "for (x in y) z", "z");
        assert!(!NodeUtil::is_expression_result_used(&compiler, value_3));
        let value_4 = get_name_node_from(&mut compiler, "for (x of y) z", "x");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_4));
        let value_5 = get_name_node_from(&mut compiler, "for (x of y) z", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_5));
        let value_6 = get_name_node_from(&mut compiler, "for (x of y) z", "z");
        assert!(!NodeUtil::is_expression_result_used(&compiler, value_6));
        let value_7 = get_name_node_from(&mut compiler, "for (x; y; z) a", "x");
        assert!(!NodeUtil::is_expression_result_used(&compiler, value_7));
        let value_8 = get_name_node_from(&mut compiler, "for (x; y; z) a", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_8));
        let value_9 = get_name_node_from(&mut compiler, "for (x; y; z) a", "z");
        assert!(!NodeUtil::is_expression_result_used(&compiler, value_9));
        let value_10 = get_name_node_from(&mut compiler, "function f() { return y }", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_10));
        let value_11 = get_name_node_from(&mut compiler, "function f() { y }", "y");
        assert!(!NodeUtil::is_expression_result_used(&compiler, value_11));
        let value_12 = get_name_node_from(&mut compiler, "var x = ()=> y", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_12));
        let value_13 = get_name_node_from(&mut compiler, "var x = ()=>{ y }", "y");
        assert!(!NodeUtil::is_expression_result_used(&compiler, value_13));
        let value_14 = get_name_node_from(&mut compiler, "({a: x = y} = z)", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_14));
        let value_15 = get_name_node_from(&mut compiler, "[x = y] = z", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_15));
        let value_16 = get_name_node_from(&mut compiler, "({x: y})", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_16));
        let value_17 = get_name_node_from(&mut compiler, "[y]", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_17));
        let value_18 = get_name_node_from(&mut compiler, "y()", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_18));
        let value_19 = get_name_node_from(&mut compiler, "y``", "y");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_19));
        let value_20 = get_string_lit_node_from(&mut compiler, "('',eval)()", "");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_20));
        let value_21 = get_string_lit_node_from(&mut compiler, "('',x.y)()", "");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_21));
        let value_22 = get_string_lit_node_from(&mut compiler, "('',x.y)``", "");
        assert!(NodeUtil::is_expression_result_used(&compiler, value_22));
    }

    // port: NodeUtilTest.AssortedTests#testIsSimpleOperator
    #[test]
    fn test_is_simple_operator() {
        let mut compiler = Compiler::new();
        let value_1 = parse_expr(&mut compiler, "!x");
        assert!(NodeUtil::is_simple_operator(&compiler, value_1));
        let value_2 = parse_expr(&mut compiler, "5 + x");
        assert!(NodeUtil::is_simple_operator(&compiler, value_2));
        let value_3 = parse_expr(&mut compiler, "typeof x");
        assert!(NodeUtil::is_simple_operator(&compiler, value_3));
        let value_4 = parse_expr(&mut compiler, "x instanceof y");
        assert!(NodeUtil::is_simple_operator(&compiler, value_4));
        let value_5 = parse_expr(&mut compiler, "5 && x");
        assert!(!NodeUtil::is_simple_operator(&compiler, value_5));
        let value_6 = parse_expr(&mut compiler, "5 || x");
        assert!(!NodeUtil::is_simple_operator(&compiler, value_6));
        let value_7 = parse_expr(&mut compiler, "x = 5");
        assert!(!NodeUtil::is_simple_operator(&compiler, value_7));
        let value_8 = parse_expr(&mut compiler, "x++");
        assert!(!NodeUtil::is_simple_operator(&compiler, value_8));
        let value_9 = parse_expr(&mut compiler, "--y");
        assert!(!NodeUtil::is_simple_operator(&compiler, value_9));
        let value_10 = parse_expr(&mut compiler, "y in x");
        assert!(NodeUtil::is_simple_operator(&compiler, value_10));
        let value_11 = parse_expr(&mut compiler, "x.y");
        assert!(!NodeUtil::is_simple_operator(&compiler, value_11));
        let value_12 = parse_expr(&mut compiler, "x[y]");
        assert!(!NodeUtil::is_simple_operator(&compiler, value_12));
    }

    // port: NodeUtilTest.AssortedTests#testIsCallToString
    #[test]
    fn test_is_call_to_string() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(&mut compiler, Token::CALL, "foo()");
        assert!(NodeUtil::is_call_to(&compiler, value_1, "foo"));
        let value_2 = parse_first(&mut compiler, Token::CALL, "foo.bar()");
        assert!(NodeUtil::is_call_to(&compiler, value_2, "foo.bar"));
        let value_3 = IR::name(&mut compiler, "foo");
        assert!(!NodeUtil::is_call_to(&compiler, value_3, "foo"));
        let value_4 = IR::name(&mut compiler, "foo");
        let value_5 = IR::getprop(&mut compiler, value_4, "bar");
        assert!(!NodeUtil::is_call_to(&compiler, value_5, "foo.bar"));
        let value_6 = parse_first(&mut compiler, Token::CALL, "foo.bar()");
        assert!(!NodeUtil::is_call_to(&compiler, value_6, "foo"));
        let value_7 = parse_first(&mut compiler, Token::CALL, "foo[0]()");
        assert!(!NodeUtil::is_call_to(&compiler, value_7, "foo"));
    }

    // port: NodeUtilTest.AssortedTests#testIsCallToNode
    #[test]
    fn test_is_call_to_node() {
        let mut compiler = Compiler::new();
        let value_1 = parse_first(&mut compiler, Token::CALL, "foo()");
        let value_2 = IR::name(&mut compiler, "foo");
        assert!(NodeUtil::is_call_to_node(&compiler, value_1, value_2));
        let value_3 = parse_first(&mut compiler, Token::CALL, "foo.bar()");
        let value_4 = IR::name(&mut compiler, "foo");
        let value_5 = IR::getprop(&mut compiler, value_4, "bar");
        assert!(NodeUtil::is_call_to_node(&compiler, value_3, value_5));
        let value_6 = IR::name(&mut compiler, "foo");
        let value_7 = IR::name(&mut compiler, "foo");
        assert!(!NodeUtil::is_call_to_node(&compiler, value_6, value_7));
        let value_8 = IR::name(&mut compiler, "foo");
        let value_9 = IR::getprop(&mut compiler, value_8, "bar");
        let value_10 = IR::name(&mut compiler, "foo");
        let value_11 = IR::getprop(&mut compiler, value_10, "bar");
        assert!(!NodeUtil::is_call_to_node(&compiler, value_9, value_11));
        let value_12 = parse_first(&mut compiler, Token::CALL, "foo.bar()");
        let value_13 = IR::name(&mut compiler, "foo");
        assert!(!NodeUtil::is_call_to_node(&compiler, value_12, value_13));
        let value_14 = parse_first(&mut compiler, Token::CALL, "foo[0]()");
        let value_15 = IR::name(&mut compiler, "foo");
        assert!(!NodeUtil::is_call_to_node(&compiler, value_14, value_15));
    }

    // port: NodeUtilTest.AssortedTests#testCallTargetNodeResolution_qualifiedName
    #[test]
    fn test_call_target_node_resolution_qualified_name() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "foo()");
        assert!(
            NodeUtil::get_call_target_resolving_indirect_calls(
                &compiler,
                value_1.get_first_first_child(&compiler).unwrap()
            )
            .matches_qualified_name(&compiler, "foo")
        );
        let value_2 = parse(&mut compiler, "a.b.c.foo()");
        assert!(
            NodeUtil::get_call_target_resolving_indirect_calls(
                &compiler,
                value_2.get_first_first_child(&compiler).unwrap()
            )
            .matches_qualified_name(&compiler, "a.b.c.foo")
        );
    }

    // port: NodeUtilTest.AssortedTests#testCallTargetNodeResolution_notAQualifiedName
    #[test]
    fn test_call_target_node_resolution_not_a_qualified_name() {
        let mut compiler = Compiler::new();
        let script = parse(&mut compiler, "({valueOf: () => {}}.valueOf());");
        assert!(script.is_script(&compiler));
        let call_node = script.get_first_first_child(&compiler);
        assert!(call_node.unwrap().is_call(&compiler));
        assert!(
            !NodeUtil::get_call_target_resolving_indirect_calls(&compiler, call_node.unwrap())
                .is_qualified_name(&compiler)
        );
    }

    // port: NodeUtilTest.AssortedTests#testIsGoogModuleGetCall
    #[test]
    fn test_is_goog_module_get_call() {
        let mut compiler = Compiler::new();
        let root = parse(&mut compiler, "const Foo = goog.module.get('a.b.c.Foo');");
        let call = root
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        assert!(NodeUtil::is_goog_module_get_call(&compiler, call.unwrap()));
        let root = parse(&mut compiler, "const Foo = goog.require('a.b.c.Foo');");
        let call = root
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap()
            .get_first_child(&compiler);
        assert!(!NodeUtil::is_goog_module_get_call(&compiler, call.unwrap()));
    }

    // port: NodeUtilTest.AssortedTests#testIsBundledGoogModule
    #[test]
    fn test_is_bundled_goog_module() {
        let mut compiler = Compiler::new();
        let goog_load_module = parse(
            &mut compiler,
            "goog.loadModule(function(exports) { goog.module('a.b'); return exports; });",
        );
        let goog_name = get_name_node(&compiler, goog_load_module, "goog");
        let call_node = goog_name.get_grandparent(&compiler);
        assert_eq!(call_node.unwrap().get_token(&compiler), Token::CALL);
        assert!(NodeUtil::is_bundled_goog_module_call(
            &compiler,
            call_node.unwrap()
        ));
    }
}

mod get_end_of_opt_chain_tests {
    use super::*;
    // port: NodeUtilTest.GetEndOfOptChainTests#isEndOfChain_call_innerChain
    #[test]
    fn is_end_of_chain_call_inner_chain() {
        let mut compiler = Compiler::new();
        let opt_chain_call = parse_expr(&mut compiler, "a?.b?.(x?.y)");
        assert!(opt_chain_call.is_opt_chain_call(&compiler));
        assert!(opt_chain_call.is_optional_chain_start(&compiler));
        let inner_opt_chain = opt_chain_call.get_last_child(&compiler);
        assert!(inner_opt_chain.unwrap().is_opt_chain_get_prop(&compiler));
        assert!(inner_opt_chain.unwrap().is_optional_chain_start(&compiler));
        assert!(NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            inner_opt_chain.unwrap()
        ));
        assert!(NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            inner_opt_chain.unwrap()
        ));
    }

    // port: NodeUtilTest.GetEndOfOptChainTests#isEndOfChain_getProp_innerChain
    #[test]
    fn is_end_of_chain_get_prop_inner_chain() {
        let mut compiler = Compiler::new();
        let opt_chain_get_prop = parse_expr(&mut compiler, "a?.b.x?.y");
        assert!(opt_chain_get_prop.is_opt_chain_get_prop(&compiler));
        assert!(opt_chain_get_prop.is_optional_chain_start(&compiler));
        assert!(NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            opt_chain_get_prop
        ));
        assert!(NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            opt_chain_get_prop
        ));
        let inner_opt_chain = opt_chain_get_prop.get_first_child(&compiler);
        assert!(inner_opt_chain.unwrap().is_opt_chain_get_prop(&compiler));
        assert!(!inner_opt_chain.unwrap().is_optional_chain_start(&compiler));
        assert!(NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            inner_opt_chain.unwrap()
        ));
        assert!(!NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            inner_opt_chain.unwrap()
        ));
        let inner_most_opt_chain = inner_opt_chain.unwrap().get_first_child(&compiler);
        assert!(
            inner_most_opt_chain
                .unwrap()
                .is_opt_chain_get_prop(&compiler)
        );
        assert!(
            inner_most_opt_chain
                .unwrap()
                .is_optional_chain_start(&compiler)
        );
        assert!(!NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            inner_most_opt_chain.unwrap()
        ));
        assert!(!NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            inner_most_opt_chain.unwrap()
        ));
    }

    // port: NodeUtilTest.GetEndOfOptChainTests#isEndOfChain_call_innerChain2
    #[test]
    fn is_end_of_chain_call_inner_chain2() {
        let mut compiler = Compiler::new();
        let opt_chain_call = parse_expr(&mut compiler, "a?.b?.(c.x?.y)");
        assert!(opt_chain_call.is_opt_chain_call(&compiler));
        assert!(opt_chain_call.is_optional_chain_start(&compiler));
        let inner_opt_chain = opt_chain_call.get_last_child(&compiler);
        assert!(inner_opt_chain.unwrap().is_opt_chain_get_prop(&compiler));
        assert!(inner_opt_chain.unwrap().is_optional_chain_start(&compiler));
        assert!(NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            inner_opt_chain.unwrap()
        ));
        assert!(NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            inner_opt_chain.unwrap()
        ));
    }

    // port: NodeUtilTest.GetEndOfOptChainTests#isEndOfChain_callAndGetProp_innerChain
    #[test]
    fn is_end_of_chain_call_and_get_prop_inner_chain() {
        let mut compiler = Compiler::new();
        let opt_chain_get_prop = parse_expr(&mut compiler, "a?.b(x?.y).c");
        assert!(opt_chain_get_prop.is_opt_chain_get_prop(&compiler));
        assert!(!opt_chain_get_prop.is_optional_chain_start(&compiler));
        let opt_chain_call = opt_chain_get_prop.get_first_child(&compiler);
        assert!(opt_chain_call.unwrap().is_opt_chain_call(&compiler));
        assert!(!opt_chain_call.unwrap().is_optional_chain_start(&compiler));
        assert!(!NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            opt_chain_call.unwrap()
        ));
        assert!(!NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            opt_chain_call.unwrap()
        ));
        let inner_opt_chain = opt_chain_call.unwrap().get_last_child(&compiler);
        assert!(inner_opt_chain.unwrap().is_opt_chain_get_prop(&compiler));
        assert!(inner_opt_chain.unwrap().is_optional_chain_start(&compiler));
        assert!(NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            inner_opt_chain.unwrap()
        ));
        assert!(NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            inner_opt_chain.unwrap()
        ));
    }

    // port: NodeUtilTest.GetEndOfOptChainTests#isEndOfChain_callAndGetProp_innerChain_multipleInnerArgs
    #[test]
    fn is_end_of_chain_call_and_get_prop_inner_chain_multiple_inner_args() {
        let mut compiler = Compiler::new();
        let opt_chain_get_prop = parse_expr(&mut compiler, "a?.b(x?.y, c, d).e");
        assert!(opt_chain_get_prop.is_opt_chain_get_prop(&compiler));
        assert!(!opt_chain_get_prop.is_optional_chain_start(&compiler));
        let opt_chain_call = opt_chain_get_prop.get_first_child(&compiler);
        assert!(opt_chain_call.unwrap().is_opt_chain_call(&compiler));
        assert!(!opt_chain_call.unwrap().is_optional_chain_start(&compiler));
        assert!(!NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            opt_chain_call.unwrap()
        ));
        assert!(!NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            opt_chain_call.unwrap()
        ));
        let inner_opt_chain = opt_chain_call.unwrap().get_second_child(&compiler);
        assert!(inner_opt_chain.unwrap().is_opt_chain_get_prop(&compiler));
        assert!(inner_opt_chain.unwrap().is_optional_chain_start(&compiler));
        assert!(NodeUtil::is_end_of_opt_chain_segment(
            &compiler,
            inner_opt_chain.unwrap()
        ));
        assert!(NodeUtil::is_end_of_full_opt_chain(
            &compiler,
            inner_opt_chain.unwrap()
        ));
    }
}

mod node_metrics_test {
    use super::*;
    // port: NodeUtilTest.NodeMetricsTest#testEstimateNumLines
    #[test]
    fn test_estimate_num_lines() {
        let mut compiler = Compiler::new();
        let value_1 = parse(&mut compiler, "");
        assert_eq!(NodeUtil::estimate_num_lines(&compiler, value_1), 2);
        let value_2 = parse(&mut compiler, "const x = 1;\nconst y = 2;\n");
        assert_eq!(NodeUtil::estimate_num_lines(&compiler, value_2), 3);
        let value_3 = parse(
            &mut compiler,
            "/* some\nlong\nmulti\nline\ncomment\n*/\nconst x = 1;\nconst y = 2;\n",
        );
        assert_eq!(NodeUtil::estimate_num_lines(&compiler, value_3), 9);
    }
}
