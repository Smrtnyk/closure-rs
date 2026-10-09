/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AngularPass.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `AngularPass.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::jsdoc_info::{JSDocInfo, Visibility};
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null};

/// Compiler pass for AngularJS-specific needs. Generates `$inject` properties for functions
/// (class constructors, wrappers, etc) annotated with `@ngInject`. Without this pass, AngularJS
/// will not work properly if variable renaming is enabled, because the function arguments will be
/// renamed.
///
/// See http://docs.angularjs.org/tutorial/step_05#a-note-on-minification
///
/// For example, the following code:
///
/// ```text
/// /** @ngInject */
/// function Controller(dependency1, dependency2) {
/// // do something
/// }
/// ```
///
/// will be transformed into:
///
/// ```text
/// function Controller(dependency1, dependency2) {
/// // do something
/// }
/// Controller.$inject = ['dependency1', 'dependency2'];
/// ```
///
/// This pass also supports assignments of function expressions to variables like:
///
/// ```text
/// /** @ngInject */
/// var filter = function(a, b) {};
///
/// var ns = {};
/// /** @ngInject */
/// ns.method = function(a,b,c) {};
///
/// /** @ngInject */
/// var shorthand = ns.method2 = function(a,b,c,) {}
/// ```
pub struct AngularPass {
    /// Nodes annotated with @ngInject
    injectables: Vec<NodeContext>,
}

// port: AngularPass#INJECT_PROPERTY_NAME
pub const INJECT_PROPERTY_NAME: &str = "$inject";

// port: AngularPass#INJECT_IN_NON_GLOBAL_OR_BLOCK_ERROR
pub static INJECT_IN_NON_GLOBAL_OR_BLOCK_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INJECT_IN_NON_GLOBAL_OR_BLOCK_ERROR",
    "@ngInject only applies to functions defined in blocks or global scope.",
);

// port: AngularPass#INJECT_NON_FUNCTION_ERROR
pub static INJECT_NON_FUNCTION_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INJECT_NON_FUNCTION_ERROR",
    "@ngInject can only be used when defining a function or assigning a function expression.",
);

// port: AngularPass#INJECTED_FUNCTION_HAS_DESTRUCTURED_PARAM
pub static INJECTED_FUNCTION_HAS_DESTRUCTURED_PARAM: DiagnosticType = DiagnosticType::error(
    "JSC_INJECTED_FUNCTION_HAS_DESTRUCTURED_PARAM",
    "@ngInject cannot be used on functions containing destructured parameter.",
);

// port: AngularPass#INJECTED_FUNCTION_HAS_DEFAULT_VALUE
pub static INJECTED_FUNCTION_HAS_DEFAULT_VALUE: DiagnosticType = DiagnosticType::error(
    "JSC_INJECTED_FUNCTION_HAS_DEFAULT_VALUE",
    "@ngInject cannot be used on functions containing default value.",
);

// port: AngularPass#INJECTED_FUNCTION_ON_NON_QNAME
pub static INJECTED_FUNCTION_ON_NON_QNAME: DiagnosticType = DiagnosticType::error(
    "JSC_INJECTED_FUNCTION_ON_NON_QNAME",
    "@ngInject can only be used on qualified names.",
);

impl AngularPass {
    // port: AngularPass#AngularPass
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            injectables: Vec::new(),
        }
    }

    /// Given a FUNCTION node returns array of STRING nodes representing function parameters.
    // port: AngularPass#createDependenciesList
    fn create_dependencies_list(&self, compiler: &mut AbstractCompiler, n: NodeId) -> Vec<NodeId> {
        check_argument!(n.is_function(compiler));
        let params = NodeUtil::get_function_parameters(compiler, n);
        self.create_strings_from_param_list(compiler, params)
    }

    /// Given a PARAM_LIST node creates an array of corresponding STRING nodes.
    // port: AngularPass#createStringsFromParamList
    fn create_strings_from_param_list(
        &self,
        compiler: &mut AbstractCompiler,
        params: NodeId,
    ) -> Vec<NodeId> {
        let mut param = params.get_first_child(compiler);
        let mut names = Vec::new();
        while let Some(p) = param {
            if p.is_name(compiler) {
                let s = p.get_string(compiler);
                names.push(IR::string(compiler, s).srcref(compiler, p));
            } else if p.is_destructuring_pattern(compiler) {
                compiler.report(JSError::make(
                    compiler,
                    p,
                    &INJECTED_FUNCTION_HAS_DESTRUCTURED_PARAM,
                    &[],
                ));
                return Vec::new();
            } else if p.is_default_value(compiler) {
                compiler.report(JSError::make(
                    compiler,
                    p,
                    &INJECTED_FUNCTION_HAS_DEFAULT_VALUE,
                    &[],
                ));
                return Vec::new();
            }
            param = p.get_next(compiler);
        }
        names
    }

    /// Add node to the list of injectables.
    // port: AngularPass#addNode
    fn add_node(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let mut inject_after: Option<NodeId> = None;
        let mut fn_: Option<NodeId> = None;
        let mut name: Option<JsString> = None;

        match n.get_token(compiler) {
            Token::ASSIGN => {
                // handles assignment cases like:
                // a = function() {}
                // a = b = c = function() {}
                if !n
                    .get_first_child(compiler)
                    .unwrap()
                    .is_qualified_name(compiler)
                {
                    compiler.report(JSError::make(
                        compiler,
                        n,
                        &INJECTED_FUNCTION_ON_NON_QNAME,
                        &[],
                    ));
                    return;
                }
                name = n
                    .get_first_child(compiler)
                    .unwrap()
                    .get_qualified_name(compiler);
                // last node of chained assignment.
                let mut f = n;
                while f.is_assign(compiler) {
                    f = f.get_last_child(compiler).unwrap();
                }
                fn_ = Some(f);
                inject_after = n.get_parent(compiler);
            }
            Token::FUNCTION => {
                // handles function case:
                // function fnName() {}
                name = NodeUtil::get_name(compiler, n);
                fn_ = Some(n);
                inject_after = Some(n);
                let parent = n.get_parent(compiler).unwrap();
                if parent.is_assign(compiler)
                    && parent
                        .get_jsdoc_info(compiler)
                        .expect("AngularPass: JSDoc of the parent ASSIGN")
                        .is_ng_inject()
                {
                    // This is a function assigned into a symbol, e.g. a regular function
                    // declaration in a goog.module or goog.scope.
                    // Skip in this traversal, it is handled when visiting the assign.
                    return;
                }
            }
            Token::VAR | Token::LET | Token::CONST => {
                // handles var declaration cases like:
                // var a = function() {}
                // var a = b = function() {}
                name = Some(n.get_first_child(compiler).unwrap().get_string(compiler));
                // looks for a function node.
                fn_ = Self::get_declaration_r_value(compiler, n);
                inject_after = Some(n);
            }
            Token::MEMBER_FUNCTION_DEF => {
                // handles class method case:
                // class clName(){
                //   constructor(){}
                //   someMethod(){} <===
                // }
                let parent = n.get_parent(compiler).unwrap();
                if parent.is_class_members(compiler) {
                    let class_node = parent.get_parent(compiler).unwrap();
                    let mid_part = if n.is_static_member(compiler) {
                        "."
                    } else {
                        ".prototype."
                    };
                    // Java's string concatenation renders a null name as "null".
                    let class_name = NodeUtil::get_name(compiler, class_node)
                        .unwrap_or_else(|| JsString::from("null"));
                    name = Some(
                        class_name
                            .concat(&JsString::from(mid_part))
                            .concat(&n.get_string(compiler)),
                    );
                    if NodeUtil::is_es6_constructor_member_function_def(compiler, n) {
                        name = NodeUtil::get_name(compiler, class_node);
                    }
                    fn_ = n.get_first_child(compiler);
                    inject_after = NodeUtil::get_enclosing_statement(compiler, class_node);
                }
            }
            _ => {}
        }

        let Some(fn_) = fn_.filter(|f| f.is_function(compiler)) else {
            compiler.report(JSError::make(compiler, n, &INJECT_NON_FUNCTION_ERROR, &[]));
            return;
        };
        let mut inject_after = inject_after.unwrap();
        if inject_after
            .get_parent(compiler)
            .unwrap()
            .is_export(compiler)
        {
            // handle `export class Foo {` or `export function(`
            inject_after = inject_after.get_parent(compiler).unwrap();
        }
        // report an error if the function declaration did not take place in the root of a
        // statement for example, `fn(/** @inject */ function(x) {});` is forbidden
        if !NodeUtil::is_statement_block(compiler, inject_after.get_parent(compiler).unwrap()) {
            compiler.report(JSError::make(
                compiler,
                n,
                &INJECT_IN_NON_GLOBAL_OR_BLOCK_ERROR,
                &[],
            ));
            return;
        }
        // checks that name is present, which must always be the case unless the
        // compiler allowed a syntax error or a dangling anonymous function
        // expression.
        let name = check_not_null!(name);
        // registers the node.
        self.injectables
            .push(NodeContext::new(name, n, fn_, inject_after));
    }

    /// Given a VAR node (variable declaration) returns the node of initial value.
    ///
    /// ```text
    /// var x;  // null
    /// var y = "value"; // STRING "value" node
    /// var z = x = y = function() {}; // FUNCTION node
    /// ```
    ///
    /// Returns the assigned initial value, or the rightmost rvalue of an assignment chain, or
    /// null.
    // port: AngularPass#getDeclarationRValue
    fn get_declaration_r_value(compiler: &AbstractCompiler, n: NodeId) -> Option<NodeId> {
        check_argument!(NodeUtil::is_name_declaration(compiler, Some(n)));
        let mut n = n.get_first_first_child(compiler)?;
        while n.is_assign(compiler) {
            n = n.get_last_child(compiler).unwrap();
        }
        Some(n)
    }
}

impl CompilerPass for AngularPass {
    // port: AngularPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // Traverses AST looking for nodes annotated with @ngInject.
        NodeTraversal::traverse(compiler, root, self);
        // iterates through annotated nodes adding $inject property to elements.
        for entry in &self.injectables {
            let name = entry.get_name().clone();
            let fn_ = entry.get_function_node();
            let dependencies = self.create_dependencies_list(compiler, fn_);
            // skips entry if it does not have any dependencies.
            if dependencies.is_empty() {
                continue;
            }
            let dependencies_array = IR::arraylit(compiler, &dependencies);
            // creates `something.$inject = ['param1', 'param2']` node.
            let qname = NodeUtil::new_qname(compiler, name.clone());
            let inject = IR::string(compiler, INJECT_PROPERTY_NAME);
            let getelem = IR::getelem(compiler, qname, inject);
            let assign = IR::assign(compiler, getelem, dependencies_array);
            let statement = IR::expr_result(compiler, assign);
            statement.srcref_tree(compiler, entry.get_node());
            statement.set_original_name(compiler, Some(name));
            // Set the visibility of the newly created property.
            let mut new_property_doc = JSDocInfo::builder();
            new_property_doc.record_visibility(Visibility::PUBLIC);
            let doc = new_property_doc.build();
            statement
                .get_first_child(compiler)
                .unwrap()
                .set_jsdoc_info(compiler, doc);

            // adds `something.$inject = [...]` node after the annotated node or the following
            // goog.inherits call.
            let mut insertion_point = entry.get_target();
            let mut next = insertion_point.get_next(compiler);
            while let Some(nx) = next {
                if !(NodeUtil::is_expr_call(compiler, nx)
                    && compiler
                        .get_coding_convention()
                        .get_classes_defined_by_call(
                            compiler,
                            nx.get_first_child(compiler).unwrap(),
                        )
                        .is_some())
                {
                    break;
                }
                insertion_point = nx;
                next = insertion_point.get_next(compiler);
            }

            statement.insert_after(compiler, insertion_point);
            compiler.report_change_to_enclosing_scope(statement);
        }
    }
}

impl Callback for AngularPass {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: AngularPass#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let doc_info = n.get_jsdoc_info(t);
        if doc_info.is_some_and(|doc_info| doc_info.is_ng_inject()) {
            self.add_node(t.get_compiler(), n);
        }
    }
}

// port: AngularPass.NodeContext
pub struct NodeContext {
    /// Name of the function/object.
    name: JsString,
    /// Node jsDoc is attached to.
    node: NodeId,
    /// Function node
    function_node: NodeId,
    /// Node after which to inject the new code
    target: NodeId,
}

impl NodeContext {
    // port: AngularPass.NodeContext#NodeContext
    pub fn new(name: JsString, node: NodeId, function_node: NodeId, target: NodeId) -> Self {
        Self {
            name,
            node,
            function_node,
            target,
        }
    }

    /// Returns the name.
    // port: AngularPass.NodeContext#getName
    pub fn get_name(&self) -> &JsString {
        &self.name
    }

    /// Returns the node.
    // port: AngularPass.NodeContext#getNode
    pub fn get_node(&self) -> NodeId {
        self.node
    }

    /// Returns the context.
    // port: AngularPass.NodeContext#getFunctionNode
    pub fn get_function_node(&self) -> NodeId {
        self.function_node
    }

    /// Returns the context.
    // port: AngularPass.NodeContext#getTarget
    pub fn get_target(&self) -> NodeId {
        self.target
    }
}
