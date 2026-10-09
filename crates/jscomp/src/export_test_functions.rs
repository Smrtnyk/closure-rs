/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ExportTestFunctions.java.

//! Port of `ExportTestFunctions.java`.
//!
//! Generates goog.exportSymbol for test functions, so they can be recognized by the test runner,
//! even if the code is compiled.

use crate::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::NodeTraversal;
use crate::node_traversal::{AbstractPostOrderCallbackInterface, AbstractShallowCallback};
use crate::node_util::NodeUtil;
use closure_rhino::check_state;
use closure_rhino::ir::IR;
use closure_rhino::java_lang::regex::Pattern;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId, Prop};
use std::sync::LazyLock;

// port: ExportTestFunctions#TEST_FUNCTIONS_NAME_PATTERN
static TEST_FUNCTIONS_NAME_PATTERN: LazyLock<Pattern> = LazyLock::new(|| {
    Pattern::compile(concat!(
        r"^(?:((\w+\.)+prototype\.||window\.)*",
        r"(setUpPage|setUp|shouldRunTests|tearDown|tearDownPage|test[\w\$]+))$"
    ))
});
// port: ExportTestFunctions#GOOG_TESTING_TEST_SUITE
const GOOG_TESTING_TEST_SUITE: &str = "goog.testing.testSuite";

pub struct ExportTestFunctions {
    export_symbol_function: JsString,
    export_property_function: Option<JsString>,
}

impl ExportTestFunctions {
    /// Creates a new export test functions compiler pass.
    ///
    /// `export_symbol_function`: The function name used to export symbols in JS.
    /// `export_property_function`: The function name used to export properties in JS.
    // port: ExportTestFunctions#ExportTestFunctions
    pub fn new(
        export_symbol_function: impl Into<JsString>,
        export_property_function: Option<JsString>,
    ) -> Self {
        Self {
            export_symbol_function: export_symbol_function.into(),
            export_property_function,
        }
    }

    // port: ExportTestFunctions#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractShallowCallback::new(ExportTestFunctionsNodes { pass: self }),
        );
    }

    // Adds exportSymbol(testFunctionName, testFunction);
    // port: ExportTestFunctions#exportTestFunctionAsSymbol
    fn export_test_function_as_symbol(
        &self,
        compiler: &mut AbstractCompiler,
        test_function_name: &JsString,
        node: NodeId,
    ) {
        let export_call_target = NodeUtil::new_qname_with_basis(
            compiler,
            self.export_symbol_function.clone(),
            node,
            test_function_name.clone(),
        );
        let call = IR::call(compiler, export_call_target, &[]);
        if export_call_target.is_name(compiler) {
            call.put_boolean_prop(compiler, Prop::FREE_CALL, true);
        }
        let s = IR::string(compiler, test_function_name.clone());
        call.add_child_to_back(compiler, s);
        let q = NodeUtil::new_qname_with_basis(
            compiler,
            test_function_name.clone(),
            node,
            test_function_name.clone(),
        );
        call.add_child_to_back(compiler, q);

        let expression = IR::expr_result(compiler, call).srcref_tree_if_missing(compiler, node);

        expression.insert_after(compiler, node);
        compiler.report_change_to_enclosing_scope(expression);
    }

    // Adds exportProperty() of the test function name on the prototype object
    // port: ExportTestFunctions#exportTestFunctionAsProperty
    fn export_test_function_as_property(
        &self,
        compiler: &mut AbstractCompiler,
        fully_qualified_function_name: NodeId,
        node: NodeId,
    ) {
        check_state!(
            fully_qualified_function_name.is_get_prop(compiler),
            "%s",
            fully_qualified_function_name.to_string(compiler)
        );

        let mut test_function_name = NodeUtil::get_prototype_property_name(
            compiler,
            node.get_first_child(compiler).unwrap(),
        );
        let first_qname = node
            .get_first_child(compiler)
            .unwrap()
            .get_qualified_name(compiler)
            .unwrap();
        if first_qname.starts_with("window.") {
            test_function_name = first_qname.substring_from("window.".len());
        }

        let target = NodeUtil::new_qname(compiler, self.export_property_function.clone().unwrap());
        let obj = fully_qualified_function_name
            .get_only_child(compiler)
            .clone_tree(compiler);
        let name = IR::string(compiler, test_function_name);
        let fn_ref = fully_qualified_function_name.clone_tree(compiler);
        let export_call = IR::call(compiler, target, &[obj, name, fn_ref]);
        let is_name = export_call
            .get_first_child(compiler)
            .unwrap()
            .is_name(compiler);
        export_call.put_boolean_prop(compiler, Prop::FREE_CALL, is_name);

        let export = IR::expr_result(compiler, export_call).srcref_tree(compiler, node);

        let node_parent = node.get_parent(compiler).unwrap();
        export.insert_after(compiler, node_parent);
        compiler.report_change_to_enclosing_scope(export);
    }

    /// Whether this is an object literal containing test methods passed to goog.testing.testSuite
    ///
    /// `t`: We only need the scope from this NodeTraversal but want to lazily create the scope.
    // port: ExportTestFunctions#isTestSuiteArgument
    fn is_test_suite_argument(n: NodeId, t: &mut NodeTraversal<'_>) -> bool {
        n.is_object_lit(t)
            && n.get_parent(t).unwrap().is_call(t)
            && n.is_second_child_of(t, n.get_parent(t))
            && {
                let previous = n.get_previous(t).unwrap();
                Self::is_goog_testing_test_suite(t, previous)
            }
    }

    /// Whether this node is a reference to goog.testing.testSuite, either through a goog.require
    /// or by its fully qualified name.
    ///
    /// If this becomes more broadly useful consider branching it into its own class and making
    /// more robust to handle forwardDeclares, destructuring requires, etc.
    // port: ExportTestFunctions#isGoogTestingTestSuite
    fn is_goog_testing_test_suite(t: &mut NodeTraversal<'_>, qname: NodeId) -> bool {
        let qname =
            NodeUtil::get_call_target_resolving_indirect_calls(t, qname.get_parent(t).unwrap());
        if !qname.is_qualified_name(t) {
            return false;
        }
        let root = NodeUtil::get_root_of_qualified_name(t, qname);
        let root_name = root.get_string(t);
        let scope = t.get_scope();
        let root_var = scope.get_slot(t.get_compiler(), &root_name);
        let compiler = t.get_compiler();
        match root_var {
            None => qname.matches_qualified_name(compiler, GOOG_TESTING_TEST_SUITE),
            Some(root_var) if root_var.is_global(compiler) => {
                qname.matches_qualified_name(compiler, GOOG_TESTING_TEST_SUITE)
            }
            Some(root_var) => {
                if root_var.get_scope(compiler).is_module_scope(compiler) {
                    let original_value = root_var.get_initial_value(compiler);
                    original_value.is_some_and(|original_value| {
                        NodeUtil::is_goog_require_call(compiler, original_value)
                            && original_value.has_two_children(compiler)
                            && original_value
                                .get_second_child(compiler)
                                .unwrap()
                                .get_string(compiler)
                                == GOOG_TESTING_TEST_SUITE
                    })
                } else {
                    false
                }
            }
        }
    }

    /// Whether a function is recognized as a test function. We follow the JsUnit convention for
    /// naming (functions should start with "test"), and we also check if it has no parameters
    /// declared.
    ///
    /// `function_name`: The name of the function
    ///
    /// Returns `true` if the function is recognized as a test function.
    // port: ExportTestFunctions#isTestFunction
    pub fn is_test_function(function_name: Option<&JsString>) -> bool {
        function_name.is_some_and(|function_name| {
            TEST_FUNCTIONS_NAME_PATTERN
                .matcher(function_name.clone())
                .matches()
        })
    }
}

impl CompilerPass for ExportTestFunctions {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::process(self, compiler, externs, root);
    }
}

// port: ExportTestFunctions.ExportTestFunctionsNodes
struct ExportTestFunctionsNodes<'a> {
    pass: &'a ExportTestFunctions,
}

impl AbstractPostOrderCallbackInterface for ExportTestFunctionsNodes<'_> {
    // port: ExportTestFunctions.ExportTestFunctionsNodes#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let Some(parent) = parent else {
            return;
        };

        if parent.is_script(t) || parent.is_module_body(t) {
            if NodeUtil::is_function_declaration(t, n) {
                // Check for a test function statement.
                let function_name = NodeUtil::get_name(t, n);
                if ExportTestFunctions::is_test_function(function_name.as_ref()) {
                    self.pass.export_test_function_as_symbol(
                        t.get_compiler(),
                        &function_name.unwrap(),
                        n,
                    );
                }
            } else if self.is_name_declared_function(t, n) {
                // Check for a test function expression.
                let function_node = n.get_first_first_child(t).unwrap();
                let function_name = NodeUtil::get_name(t, function_node);
                if ExportTestFunctions::is_test_function(function_name.as_ref()) {
                    self.pass.export_test_function_as_symbol(
                        t.get_compiler(),
                        &function_name.unwrap(),
                        n,
                    );
                }
            } else if self.is_name_declared_class(t, n) {
                let class_node = n.get_first_first_child(t).unwrap();
                let class_name = NodeUtil::get_name(t, class_node);
                self.export_class_with_name(t.get_compiler(), class_node, class_name, n);
            } else if n.is_class(t) {
                self.export_class(t.get_compiler(), n);
            }
        } else if NodeUtil::is_expr_assign(t, parent) {
            // Check for a test method assignment.
            let grandparent = parent.get_parent(t);
            if let Some(grandparent) = grandparent
                && (grandparent.is_script(t) || grandparent.is_module_body(t))
            {
                //                                    NAME/(GETPROP -> ... -> NAME)
                // SCRIPT -> EXPR_RESULT -> ASSIGN ->
                //                                    FUNCTION/CLASS
                let first_child = n.get_first_child(t).unwrap();
                let last_child = n.get_last_child(t).unwrap();
                let node_name = first_child.get_qualified_name(t);

                if last_child.is_function(t) {
                    if ExportTestFunctions::is_test_function(node_name.as_ref()) {
                        if n.get_first_child(t).unwrap().is_name(t) {
                            self.pass.export_test_function_as_symbol(
                                t.get_compiler(),
                                &node_name.unwrap(),
                                parent,
                            );
                        } else {
                            self.pass.export_test_function_as_property(
                                t.get_compiler(),
                                first_child,
                                n,
                            );
                        }
                    }
                } else if last_child.is_class(t) {
                    self.export_class_with_name(t.get_compiler(), last_child, node_name, parent);
                }
            }
        } else if ExportTestFunctions::is_test_suite_argument(n, t) {
            let mut c = n.get_first_child(t);
            while let Some(cur) = c {
                let next = cur.get_next(t);
                if cur.is_string_key(t) && !cur.is_quoted_string_key(t) {
                    cur.set_quoted_string_key(t);
                    t.get_compiler().report_change_to_enclosing_scope(cur);
                } else if cur.is_member_function_def(t) {
                    self.rewrite_member_def_in_obj_lit(t.get_compiler(), cur, n);
                }
                c = next;
            }
        }
    }
}

impl ExportTestFunctionsNodes<'_> {
    // port: ExportTestFunctions.ExportTestFunctionsNodes#exportClass(Node)
    fn export_class(&self, compiler: &mut AbstractCompiler, class_node: NodeId) {
        let class_name = NodeUtil::get_name(compiler, class_node);
        self.export_class_with_name(compiler, class_node, class_name, class_node);
    }

    // port: ExportTestFunctions.ExportTestFunctionsNodes#exportClass(Node,String,Node)
    fn export_class_with_name(
        &self,
        compiler: &mut AbstractCompiler,
        class_node: NodeId,
        class_name: Option<JsString>,
        mut add_after: NodeId,
    ) {
        let class_members = class_node.get_last_child(compiler).unwrap();
        let mut maybe_member_function_def = class_members.get_first_child(compiler);
        while let Some(member) = maybe_member_function_def {
            if member.is_member_function_def(compiler) {
                let method_name = member.get_string(compiler);
                if ExportTestFunctions::is_test_function(Some(&method_name)) {
                    // Java string concatenation renders a null className as "null".
                    let class_name_str =
                        class_name.clone().unwrap_or_else(|| JsString::from("null"));
                    let function_ref = class_name_str
                        .concat(&JsString::from(".prototype."))
                        .concat(&method_name);
                    let class_ref = class_name_str.concat(&JsString::from(".prototype"));

                    let export_call_target = NodeUtil::new_qname_with_basis(
                        compiler,
                        self.pass.export_property_function.clone().unwrap(),
                        member,
                        method_name.clone(),
                    );
                    let call = IR::call(compiler, export_call_target, &[]);
                    if export_call_target.is_name(compiler) {
                        call.put_boolean_prop(compiler, Prop::FREE_CALL, true);
                    }

                    let c1 = NodeUtil::new_qname_with_basis(
                        compiler,
                        class_ref.clone(),
                        member,
                        class_ref,
                    );
                    call.add_child_to_back(compiler, c1);
                    let c2 = IR::string(compiler, method_name);
                    call.add_child_to_back(compiler, c2);
                    let c3 = NodeUtil::new_qname_with_basis(
                        compiler,
                        function_ref.clone(),
                        member,
                        function_ref,
                    );
                    call.add_child_to_back(compiler, c3);

                    let expression =
                        IR::expr_result(compiler, call).srcref_tree_if_missing(compiler, member);

                    expression.insert_after(compiler, add_after);
                    compiler.report_change_to_enclosing_scope(expression);
                    add_after = expression;
                }
            }
            maybe_member_function_def = member.get_next(compiler);
        }
    }

    /// Converts a member function into a quoted string key to avoid property renaming
    // port: ExportTestFunctions.ExportTestFunctionsNodes#rewriteMemberDefInObjLit
    fn rewrite_member_def_in_obj_lit(
        &self,
        compiler: &mut AbstractCompiler,
        member_def: NodeId,
        obj_lit: NodeId,
    ) {
        let name = member_def.get_string(compiler);
        let value = member_def.remove_first_child(compiler).unwrap();
        let string_key = IR::string_key_with_value(compiler, name, value);
        member_def.replace_with(compiler, string_key);
        string_key.set_quoted_string_key(compiler);
        let info = member_def.get_jsdoc_info(compiler);
        string_key.set_jsdoc_info(compiler, info);
        compiler.report_change_to_enclosing_scope(obj_lit);
    }

    /// Get the node that corresponds to an expression declared with var, let or const. This has
    /// the AST structure VAR/LET/CONST -> NAME -> NODE
    // port: ExportTestFunctions.ExportTestFunctionsNodes#getNameDeclaredGrandchild
    fn get_name_declared_grandchild(&self, ast: &Ast, node: NodeId) -> Option<NodeId> {
        if !NodeUtil::is_name_declaration(ast, Some(node)) {
            return None;
        }
        node.get_first_first_child(ast)
    }

    /// Whether node corresponds to a function expression declared with var, let or const which
    /// is of the form `var/let/const functionName = function() { ... };`
    ///
    /// This has the AST structure VAR/LET/CONST -> NAME -> FUNCTION
    // port: ExportTestFunctions.ExportTestFunctionsNodes#isNameDeclaredFunction
    fn is_name_declared_function(&self, ast: &Ast, node: NodeId) -> bool {
        let grandchild = self.get_name_declared_grandchild(ast, node);
        grandchild.is_some_and(|g| g.is_function(ast))
    }

    /// Whether node corresponds to a class declared with var, let or const which is of the form
    /// `var/let/const className = class { ... };`
    ///
    /// This has the AST structure VAR/LET/CONST -> NAME -> CLASS
    // port: ExportTestFunctions.ExportTestFunctionsNodes#isNameDeclaredClass
    fn is_name_declared_class(&self, ast: &Ast, node: NodeId) -> bool {
        let grandchild = self.get_name_declared_grandchild(ast, node);
        grandchild.is_some_and(|g| g.is_class(ast))
    }
}
