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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/NodeUtilTest.java.

use super::*;
use closure_jscomp::{
    node_util::GoogRequire, scope::Scope, syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_parsing::parser::feature_set::Feature;
use std::panic::{AssertUnwindSafe, catch_unwind};

use closure_jscomp::abstract_compiler::LifeCycleStage;

fn panic_message(error: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = error.downcast_ref::<String>() {
        return message.clone();
    }
    error
        .downcast_ref::<&str>()
        .expect("string panic payload")
        .to_string()
}

mod assorted_tests {
    use super::*;

    // port: NodeUtilTest.AssortedTests#testTypedefNamespace
    #[test]
    fn test_typedef_namespace() {
        let mut compiler = Compiler::new();
        let statements = parse(
            &mut compiler,
            "/** @typedef */ const obj = {};\n/** @const */ obj.a = {};\n/** @typedef @const */ obj.b = {};\n/** @typedef */ obj.c = {};\n/** @typedef @const */ obj.d = function() {};\n",
        );
        let mut namespace_names = closure_rhino::fast_hash::IndexSet::<_>::default();
        let mut non_namespace_names = closure_rhino::fast_hash::IndexSet::<_>::default();
        for statement in statements.children(&compiler) {
            let name = if statement.is_expr_result(&compiler) {
                statement.get_first_first_child(&compiler).unwrap()
            } else {
                statement.get_first_child(&compiler).unwrap()
            };
            assert!(name.is_name(&compiler) || name.is_get_prop(&compiler));
            if NodeUtil::is_namespace_decl(&compiler, name) {
                namespace_names.insert(name.get_qualified_name(&compiler).unwrap());
            } else {
                non_namespace_names.insert(name.get_qualified_name(&compiler).unwrap());
            }
        }
        assert_contents(
            namespace_names.into_iter().collect(),
            vec!["obj".into(), "obj.a".into(), "obj.b".into()],
        );
        assert_contents(
            non_namespace_names.into_iter().collect(),
            vec!["obj.c".into(), "obj.d".into()],
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBestLValueName
    #[test]
    fn test_get_best_l_value_name() {
        let mut compiler = Compiler::new();
        let mut get_best_name = |js| {
            let n = parse_first(&mut compiler, Token::FUNCTION, js);
            NodeUtil::get_best_l_value_name(&compiler, NodeUtil::get_best_l_value(&compiler, n))
        };
        assert_eq!(get_best_name("var x = function() {};"), Some("x".into()));
        assert_eq!(get_best_name("x = function() {};"), Some("x".into()));
        assert_eq!(get_best_name("function x() {};"), Some("x".into()));
        assert_eq!(
            get_best_name("var x = y ? z : function() {};"),
            Some("x".into())
        );
        assert_eq!(
            get_best_name("var x = y ? function() {} : z;"),
            Some("x".into())
        );
        assert_eq!(
            get_best_name("var x = y && function() {};"),
            Some("x".into())
        );
        assert_eq!(
            get_best_name("var x = y || function() {};"),
            Some("x".into())
        );
        assert_eq!(
            get_best_name("var x = (y, function() {});"),
            Some("x".into())
        );
        assert_eq!(
            get_best_name("C.prototype.d = function() {};"),
            Some("C.prototype.d".into())
        );
        assert_eq!(
            get_best_name("class C { d() {} };"),
            Some("C.prototype.d".into())
        );
        assert_eq!(get_best_name("C.d = function() {};"), Some("C.d".into()));
        assert_eq!(
            get_best_name("class C { static d() {} };"),
            Some("C.d".into())
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetBestJsDocInfoNodeStrict_rhsUnnamedClassExpression_throws
    #[test]
    fn test_get_best_js_doc_info_node_strict_rhs_unnamed_class_expression_throws() {
        let mut compiler = Compiler::new();
        let const_node = parse(&mut compiler, "/** some */ const x = class {}")
            .get_first_child(&compiler)
            .unwrap();
        let x_name = const_node.get_first_child(&compiler).unwrap();
        let class_node = x_name.get_first_child(&compiler).unwrap();
        assert!(class_node.is_class(&compiler));
        let best_js_doc_info_node = NodeUtil::get_best_jsdoc_info_node(&compiler, class_node);
        assert!(best_js_doc_info_node.is_some());
        assert_eq!(best_js_doc_info_node, Some(const_node));
        let e = catch_unwind(AssertUnwindSafe(|| {
            NodeUtil::get_best_js_doc_info_node_strict(&compiler, class_node)
        }))
        .expect_err("IllegalStateException");
        assert!(panic_message(e).contains("Not allowed to get JSDocInfo node for node"));
    }

    // port: NodeUtilTest.AssortedTests#testIsConstantDeclaration_throwsOnNonDeclarationReferences
    #[test]
    fn test_is_constant_declaration_throws_on_non_declaration_references() {
        let mut compiler = Compiler::new();
        let y = get_name_node_from(&mut compiler, "const x = y;", "y");
        assert!(
            catch_unwind(AssertUnwindSafe(|| NodeUtil::is_constant_declaration(
                &compiler, None, y
            )))
            .is_err(),
            "IllegalStateException"
        );
        let x = get_name_node_from(&mut compiler, "x;", "x");
        assert!(
            catch_unwind(AssertUnwindSafe(|| NodeUtil::is_constant_declaration(
                &compiler, None, x
            )))
            .is_err(),
            "IllegalStateException"
        );
        let x = get_name_node_from(&mut compiler, "const ns = {y: x};", "x");
        assert!(
            catch_unwind(AssertUnwindSafe(|| NodeUtil::is_constant_declaration(
                &compiler, None, x
            )))
            .is_err(),
            "IllegalStateException"
        );
        let const_assignment = parse(&mut compiler, "/** @const */ x.y = a.b;");
        let rhs = const_assignment
            .get_first_first_child(&compiler)
            .unwrap()
            .get_second_child(&compiler)
            .unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(|| NodeUtil::is_constant_declaration(
                &compiler,
                NodeUtil::get_best_jsdoc_info(&compiler, rhs).as_deref(),
                rhs
            )))
            .is_err(),
            "IllegalArgumentException"
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetGoogRequireInfo_returnsNullForShadowedRequires
    #[test]
    fn test_get_goog_require_info_returns_null_for_shadowed_requires() {
        let src = "goog.module('a.b.c'); const Foo = goog.require('d.Foo'); { const Foo = 0; }";
        let mut compiler = Compiler::new();
        let first = parse(&mut compiler, src);
        let mut scope_creator = SyntacticScopeCreator::new();
        let global_scope = scope_creator.create_scope(&mut compiler, first, None);
        let module = get_node(&compiler, first, Token::MODULE_BODY);
        let module_scope = scope_creator.create_scope(&mut compiler, module, Some(global_scope));
        let block = get_node(&compiler, first, Token::BLOCK);
        let block_scope = scope_creator.create_scope(&mut compiler, block, Some(module_scope));
        assert_eq!(
            NodeUtil::get_goog_require_info_name(&mut compiler, "Foo", module_scope),
            Some(GoogRequire::from_namespace(JsString::from("d.Foo"), true))
        );
        assert_eq!(
            NodeUtil::get_goog_require_info_name(&mut compiler, "Foo", block_scope),
            None
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetAllModuleVars
    #[test]
    fn test_get_all_module_vars() {
        let js = "goog.module('m'); var h =2; function g(x, y) {var z; {let a; const b = 1} let c}";
        let mut compiler = Compiler::new();
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
        let mut scope_creator = SyntacticScopeCreator::new();
        let ast = parse(&mut compiler, js);
        let module_node = parse_first(&mut compiler, Token::MODULE_BODY, js);
        let global_scope = Scope::create_global_scope(&mut compiler, ast);
        let all_variables = NodeUtil::get_all_var_names_declared_in_module(
            &mut compiler,
            module_node,
            &mut scope_creator,
            global_scope,
        );
        assert_contents(
            all_variables.into_iter().collect(),
            vec!["g".into(), "h".into()],
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetAllModuleVars2
    #[test]
    fn test_get_all_module_vars2() {
        let js = "var glob = 3;";
        let mut compiler = Compiler::new();
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
        let mut scope_creator = SyntacticScopeCreator::new();
        let ast = parse(&mut compiler, js);
        let global_scope = Scope::create_global_scope(&mut compiler, ast);
        let ex = catch_unwind(AssertUnwindSafe(|| {
            NodeUtil::get_all_var_names_declared_in_module(
                &mut compiler,
                ast,
                &mut scope_creator,
                global_scope,
            )
        }))
        .expect_err("IllegalStateException");
        assert_eq!(
            panic_message(ex),
            "getAllVarsDeclaredInModule expects a module body node"
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetAllVars1
    #[test]
    fn test_get_all_vars1() {
        let fn_string = "var h; function g(x, y) {var z; h = 2; {let a; const b = 1} let c}";
        let mut compiler = Compiler::new();
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
        let mut scope_creator = SyntacticScopeCreator::new();
        let ast = parse(&mut compiler, fn_string);
        let function_node = parse_first(&mut compiler, Token::FUNCTION, fn_string);
        let global_scope = Scope::create_global_scope(&mut compiler, ast);
        let function_scope =
            scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
        let all_vars_declared_in_function = NodeUtil::get_all_vars_declared_in_function(
            &mut compiler,
            &mut scope_creator,
            function_scope,
        );
        let all_variables = all_vars_declared_in_function.get_all_variables();
        assert_contents(
            all_variables.keys().cloned().collect(),
            vec![
                "a".into(),
                "b".into(),
                "c".into(),
                "z".into(),
                "x".into(),
                "y".into(),
            ],
        );
    }

    // port: NodeUtilTest.AssortedTests#testGetAllVars2
    #[test]
    fn test_get_all_vars2() {
        let fn_string = "function g(x, y)\n{var z;\n{let a = (no1, no2) => { let no6, no7; };\nconst b = 1}\nlet c}\nfunction u(h) {let e}\n";
        let mut compiler = Compiler::new();
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
        let mut scope_creator = SyntacticScopeCreator::new();
        let ast = parse(&mut compiler, fn_string);
        let function_node = parse_first(&mut compiler, Token::FUNCTION, fn_string);
        let global_scope = Scope::create_global_scope(&mut compiler, ast);
        let function_scope =
            scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
        let all_vars_declared_in_function = NodeUtil::get_all_vars_declared_in_function(
            &mut compiler,
            &mut scope_creator,
            function_scope,
        );
        let all_variables = all_vars_declared_in_function.get_all_variables();
        assert_contents(
            all_variables.keys().cloned().collect(),
            vec![
                "x".into(),
                "y".into(),
                "z".into(),
                "a".into(),
                "b".into(),
                "c".into(),
            ],
        );
    }
}

mod node_traversal_tests {
    use super::*;
    // port: NodeUtilTest.NodeTraversalTests#prohibitAddingNonAllowableFeatureToScriptExceptModules
    #[test]
    fn prohibit_adding_non_allowable_feature_to_script_except_modules() {
        let mut compiler = Compiler::new();
        let script_node = parse(&mut compiler, "");
        compiler.set_allowable_features(FeatureSet::BARE_MINIMUM);
        NodeUtil::add_feature_to_script(&mut compiler, script_node, Feature::MODULES);
        assert_eq!(
            NodeUtil::get_feature_set_of_script(&compiler, script_node),
            Some(FeatureSet::BARE_MINIMUM.with(Feature::MODULES))
        );
        assert_eq!(
            compiler.get_allowable_features(),
            FeatureSet::BARE_MINIMUM.with(Feature::MODULES)
        );
        compiler.set_allowable_features(FeatureSet::BARE_MINIMUM);
        assert!(
            catch_unwind(AssertUnwindSafe(|| NodeUtil::add_feature_to_script(
                &mut compiler,
                script_node,
                Feature::LET_DECLARATIONS
            )))
            .is_err(),
            "IllegalStateException"
        );
    }

    // port: NodeUtilTest.NodeTraversalTests#removeFeatureFromAllScriptUpdatesCompilerFeatureSet
    #[test]
    fn remove_feature_from_all_script_updates_compiler_feature_set() {
        let mut compiler = Compiler::new();
        let root_node = IR::root(&mut compiler, &[]);
        let script = parse(&mut compiler, "");
        root_node.add_child_to_front(&mut compiler, script);
        compiler.set_allowable_features(FeatureSet::ES2020);
        assert_eq!(compiler.get_allowable_features(), FeatureSet::ES2020);
        NodeUtil::remove_feature_from_all_scripts(
            &mut compiler,
            root_node,
            Feature::LET_DECLARATIONS,
        );
        assert_eq!(
            compiler.get_allowable_features(),
            FeatureSet::ES2020.without(Feature::LET_DECLARATIONS)
        );
    }

    // port: NodeUtilTest.NodeTraversalTests#removeFeatureSetFromAllScriptUpdatesCompilerFeatureSet
    #[test]
    fn remove_feature_set_from_all_script_updates_compiler_feature_set() {
        let mut compiler = Compiler::new();
        let root_node = IR::root(&mut compiler, &[]);
        let script = parse(&mut compiler, "");
        root_node.add_child_to_front(&mut compiler, script);
        compiler.set_allowable_features(FeatureSet::ES2020);
        assert_eq!(compiler.get_allowable_features(), FeatureSet::ES2020);
        NodeUtil::remove_features_from_all_scripts(
            &mut compiler,
            root_node,
            FeatureSet::BARE_MINIMUM
                .with_features(&[Feature::LET_DECLARATIONS, Feature::CONST_DECLARATIONS]),
        );
        assert_eq!(
            compiler.get_allowable_features(),
            FeatureSet::ES2020
                .without(Feature::LET_DECLARATIONS)
                .without(Feature::CONST_DECLARATIONS)
        );
    }
}
