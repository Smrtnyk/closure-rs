/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Es6RewriteDestructuring.java.

//! Port of `Es6RewriteDestructuring.java`.
//!
//! Rewrites destructuring patterns and default parameters to valid ES3 code or to a different form
//! of destructuring.
#![allow(clippy::needless_return, clippy::needless_late_init)] // Preserve Java statement order and declarations.

use crate::{
    AbstractCompiler,
    ast_factory::{AstFactory, Type},
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
};
use closure_jstype::js_type_native::JSTypeNative;
use closure_parsing::{
    parser::feature_set::{Feature, FeatureSet},
    parsing_util::ParsingUtil,
};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    jscomp_colors::standard_colors,
    jsdoc_info::JSDocInfo,
    node::{NodeId, Prop},
    token::Token,
};
use std::collections::VecDeque;

// port: Es6RewriteDestructuring.ObjectDestructuringRewriteMode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectDestructuringRewriteMode {
    /// Rewrite all object destructuring patterns. This is the default mode used if no
    /// ObjectDestructuringRewriteMode is provided to the Builder.
    ///
    /// Used to transpile ES2018 -> ES5
    REWRITE_ALL_OBJECT_PATTERNS,

    /// Rewrite only destructuring patterns that contain object pattern rest properties (whether
    /// the rest is on the top level or nested within a property).
    ///
    /// Used to transpile ES2018 -> ES2017
    REWRITE_OBJECT_REST,
}

pub struct Es6RewriteDestructuring {
    rewrite_mode: ObjectDestructuringRewriteMode,
    ast_factory: AstFactory,
    namespace: TranspilationNamespace,

    features_to_trigger_running_pass: FeatureSet,
    features_to_mark_as_removed: FeatureSet,

    pattern_nesting_stack: VecDeque<PatternNestingLevel>,

    destructuring_var_counter: i32,
}

// port: Es6RewriteDestructuring#DESTRUCTURING_TEMP_VAR
pub const DESTRUCTURING_TEMP_VAR: &str = "$jscomp$destructuring$var";

// port: Es6RewriteDestructuring.Builder
pub struct Builder<'a> {
    compiler: &'a mut AbstractCompiler,
    rewrite_mode: ObjectDestructuringRewriteMode,
}

impl<'a> Builder<'a> {
    // port: Es6RewriteDestructuring.Builder#Builder
    pub fn new(compiler: &'a mut AbstractCompiler) -> Self {
        Self {
            compiler,
            rewrite_mode: ObjectDestructuringRewriteMode::REWRITE_ALL_OBJECT_PATTERNS,
        }
    }

    // port: Es6RewriteDestructuring.Builder#setDestructuringRewriteMode
    pub fn set_destructuring_rewrite_mode(
        mut self,
        rewrite_mode: ObjectDestructuringRewriteMode,
    ) -> Self {
        self.rewrite_mode = rewrite_mode;
        self
    }

    // port: Es6RewriteDestructuring.Builder#build
    pub fn build(self) -> Es6RewriteDestructuring {
        Es6RewriteDestructuring::new(self)
    }
}

// port: Es6RewriteDestructuring.PatternNestingLevel
struct PatternNestingLevel {
    pattern: NodeId,
    has_nested_object_rest: bool,
}

impl PatternNestingLevel {
    // port: Es6RewriteDestructuring.PatternNestingLevel#PatternNestingLevel
    fn new(pattern: NodeId, has_nested_rest: bool) -> Self {
        Self {
            pattern,
            has_nested_object_rest: has_nested_rest,
        }
    }
}

impl Es6RewriteDestructuring {
    // port: Es6RewriteDestructuring#Es6RewriteDestructuring
    fn new(builder: Builder<'_>) -> Self {
        let compiler = builder.compiler;
        let rewrite_mode = builder.rewrite_mode;
        let ast_factory = compiler.create_ast_factory();
        let namespace = TranspilationNamespace::get(compiler);

        let features_to_trigger_running_pass;
        let features_to_mark_as_removed;
        match rewrite_mode {
            ObjectDestructuringRewriteMode::REWRITE_ALL_OBJECT_PATTERNS => {
                features_to_trigger_running_pass = FeatureSet::BARE_MINIMUM.with_features(&[
                    Feature::DEFAULT_PARAMETERS,
                    Feature::ARRAY_DESTRUCTURING,
                    Feature::ARRAY_PATTERN_REST,
                    Feature::OBJECT_DESTRUCTURING,
                ]);

                // If OBJECT_PATTERN_REST were to be present in featuresToTriggerRunningPass and
                // not the input language featureSet (such as ES6=>ES5) the pass would be skipped.
                features_to_mark_as_removed =
                    features_to_trigger_running_pass.with(Feature::OBJECT_PATTERN_REST);
            }
            ObjectDestructuringRewriteMode::REWRITE_OBJECT_REST => {
                // TODO(bradfordcsmith): We shouldn't really need to remove default parameters for
                // this case.
                features_to_trigger_running_pass =
                    FeatureSet::BARE_MINIMUM.with(Feature::OBJECT_PATTERN_REST);
                features_to_mark_as_removed = features_to_trigger_running_pass;
            }
        }
        Self {
            rewrite_mode,
            ast_factory,
            namespace,
            features_to_trigger_running_pass,
            features_to_mark_as_removed,
            pattern_nesting_stack: VecDeque::new(),
            destructuring_var_counter: 0,
        }
    }

    /// Pulls all default and destructuring parameters out of function parameters.
    // TODO(bradfordcsmith): Ideally if we're only removing OBJECT_REST, we should only do this when
    // the parameter list contains a usage of OBJECT_REST.
    // port: Es6RewriteDestructuring#pullDestructuringOutOfParams
    fn pull_destructuring_out_of_params(
        &mut self,
        compiler: &mut AbstractCompiler,
        param_list: NodeId,
        function: NodeId,
    ) {
        let mut insert_spot: Option<NodeId> = None;
        let body = function.get_last_child(compiler).unwrap();
        let mut next;
        let mut param = param_list.get_first_child(compiler);
        while let Some(p) = param {
            next = p.get_next(compiler);
            if p.is_default_value(compiler) {
                let name_or_pattern = p.remove_first_child(compiler).unwrap();
                // We'll be cloning nameOrPattern below, and we don't want to clone the JSDoc info
                // with it
                let js_doc = name_or_pattern.get_jsdoc_info(compiler);
                name_or_pattern.set_jsdoc_info(compiler, None);
                let default_value = p.remove_first_child(compiler).unwrap();
                let new_param;

                // Treat name=undefined (and equivalent) as if it was just name.  There
                // is no need to generate a (name===void 0?void 0:name) statement for
                // such arguments.
                let mut is_noop = false;
                if !name_or_pattern.is_name(compiler) {
                    // Do not try to optimize unless nameOrPattern is a simple name.
                } else if default_value.is_name(compiler) {
                    is_noop = default_value.get_string(compiler) == "undefined";
                } else if default_value.is_void(compiler) {
                    // Any kind of 'void literal' is fine, but 'void fun()' or anything
                    // else with side effects isn't.  We're not trying to be particularly
                    // smart here and treat 'void {}' for example as if it could cause side
                    // effects.
                    is_noop = NodeUtil::is_immutable_value(
                        compiler,
                        default_value.get_first_child(compiler).unwrap(),
                    );
                }

                if is_noop {
                    new_param = name_or_pattern.clone_tree(compiler);
                } else {
                    new_param = if name_or_pattern.is_name(compiler) {
                        name_or_pattern
                    } else {
                        let name = self.get_temp_variable_name();
                        self.create_temp_var_name_node(
                            compiler,
                            name,
                            AstFactory::type_node(name_or_pattern),
                        )
                    };
                    let lhs = name_or_pattern.clone_tree(compiler);
                    let new_param_clone = new_param.clone_tree(compiler);
                    let rhs = self.default_value_hook(compiler, new_param_clone, default_value);
                    let new_statement = if name_or_pattern.is_name(compiler) {
                        let assign = self.ast_factory.create_assign(compiler, lhs, rhs);
                        IR::expr_result(compiler, assign)
                    } else {
                        IR::var_with_value(compiler, lhs, rhs)
                    };
                    new_statement.srcref_tree_if_missing(compiler, p);
                    if let Some(spot) = insert_spot {
                        new_statement.insert_after(compiler, spot);
                    } else {
                        // insert new declarations only after all inner function declarations in
                        // that function body to preserve normalization
                        insert_spot =
                            NodeUtil::get_insertion_point_after_all_inner_function_declarations(
                                compiler, body,
                            );
                        if let Some(spot) = insert_spot {
                            new_statement.insert_before(compiler, spot);
                        } else {
                            // functionBody only contains hoisted function declarations
                            body.add_child_to_back(compiler, new_statement);
                        }
                    }
                    insert_spot = Some(new_statement);
                    let first = new_statement.get_first_child(compiler).unwrap();
                    if first.is_destructuring_lhs(compiler) {
                        insert_spot =
                            Some(self.rewrite_var_destructuring_declaration(compiler, first));
                    }
                }

                p.replace_with(compiler, new_param);
                new_param.set_jsdoc_info(compiler, js_doc);

                compiler.report_change_to_change_scope(function);
            } else if p.is_destructuring_pattern(compiler) {
                let name = self.get_temp_variable_name();
                insert_spot = Some(self.replace_pattern_param_with_temp_var(
                    compiler,
                    function,
                    insert_spot,
                    p,
                    name,
                ));
                compiler.report_change_to_change_scope(function);
            } else if p.is_rest(compiler)
                && p.get_first_child(compiler)
                    .unwrap()
                    .is_destructuring_pattern(compiler)
            {
                let first = p.get_first_child(compiler).unwrap();
                let name = self.get_temp_variable_name();
                insert_spot = Some(self.replace_pattern_param_with_temp_var(
                    compiler,
                    function,
                    insert_spot,
                    first,
                    name,
                ));
                compiler.report_change_to_change_scope(function);
            }
            param = next;
        }
    }

    /// Replace a destructuring pattern parameter with a a temporary parameter name. To do this,
    /// this function adds a new local assignment in the function body assigning the temporary
    /// parameter to the pattern and creates stub declarations for the individual names in the
    /// destructuring pattern.
    ///
    /// For example, given the function `function f([a, b]) {}`, this function will convert it to
    /// `function f(tempVar) { var a; var b; [a, b] = tempVar; }`
    ///
    /// Note: Rewrites of the destructuring pattern will happen later to rewrite this destructuring
    /// pattern as non-destructured code.
    ///
    /// Returns the last stub declaration that was generated for the local variable.
    // port: Es6RewriteDestructuring#replacePatternParamWithTempVar
    fn replace_pattern_param_with_temp_var(
        &mut self,
        compiler: &mut AbstractCompiler,
        function: NodeId,
        mut insert_spot: Option<NodeId>,
        pattern_param: NodeId,
        temp_var_name: String,
    ) -> NodeId {
        // Convert `function f([a, b]) {}` to `function f(tempVar) { var [a, b] = tempVar; }`
        let param_type = AstFactory::type_node(pattern_param);
        let new_param =
            self.create_temp_var_name_node(compiler, temp_var_name.clone(), param_type.clone());
        let info = pattern_param.get_jsdoc_info(compiler);
        new_param.set_jsdoc_info(compiler, info);
        pattern_param.replace_with(compiler, new_param);
        let temp_name = self.create_temp_var_name_node(compiler, temp_var_name, param_type);
        let new_decl = IR::var_with_value(compiler, pattern_param, temp_name);
        new_decl.srcref_tree_if_missing(compiler, pattern_param);

        if let Some(spot) = insert_spot {
            new_decl.insert_after(compiler, spot);
        } else {
            // insert new declarations only after all inner function declarations in that function
            // body to preserve normalization
            let function_body = function.get_last_child(compiler).unwrap();
            insert_spot = NodeUtil::get_insertion_point_after_all_inner_function_declarations(
                compiler,
                function_body,
            );
            if let Some(spot) = insert_spot {
                new_decl.insert_before(compiler, spot);
            } else {
                // functionBody only contains hoisted function declarations
                function_body.add_child_to_back(compiler, new_decl);
            }
        }

        let first = new_decl.get_first_child(compiler).unwrap();
        self.rewrite_var_destructuring_declaration(compiler, first)
    }

    /// Rewrites a destructuring declaration to a destructuring assignment.
    ///
    /// For example, given the code `var [a, b] = ...;`, this function will convert it to `var a;
    /// var b; [a, b] = ...;`
    ///
    /// Returns the expression result containing the assignment.
    // port: Es6RewriteDestructuring#rewriteVarDestructuringDeclaration
    fn rewrite_var_destructuring_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        destructuring_lhs: NodeId,
    ) -> NodeId {
        check_state!(
            destructuring_lhs.is_destructuring_lhs(compiler),
            "%s",
            destructuring_lhs.to_string(compiler)
        );
        check_state!(
            !destructuring_lhs
                .get_grandparent(compiler)
                .unwrap()
                .is_export(compiler),
            "Export destructuring declarations not expected inside a function body."
        );
        let var = destructuring_lhs.get_parent(compiler).unwrap();

        // create a stub declaration for each name in the destructuring pattern
        // e.g. generate `var a; var b;` from `var [a, b] = ...;`
        NodeUtil::visit_lhs_nodes_in_node(compiler, destructuring_lhs, &mut |compiler, name| {
            // Add a declaration outside the destructuring pattern for the given name.
            check_state!(
                name.is_name(compiler),
                "lhs in destructuring declaration should be a simple name. (%s)",
                name.to_string(compiler)
            );
            let name_string = name.get_string(compiler);
            let new_name = IR::name(compiler, name_string).srcref(compiler, name);
            if name.get_boolean_prop(compiler, Prop::IS_CONSTANT_NAME) {
                // if old name was a const, new name should be too
                // e.g. when rewriting `{VALUE} = ...` the `VALUE` is const by coding convention
                new_name.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
            }
            let new_var = IR::var(compiler, new_name).srcref(compiler, name);
            new_var.insert_before(compiler, var);
        });

        // Transform destructuring var declaration to assignment. That is, `var [a, b] = ...` to
        // `[a, b] = ...` and `var {a, b} = ...` to `({a, b} = ...);`
        let destructuring_pattern = destructuring_lhs.remove_first_child(compiler).unwrap();
        check_state!(
            destructuring_pattern.is_destructuring_pattern(compiler),
            "Expected destructuring pattern but got %s",
            destructuring_pattern.to_string_tree(compiler)
        );

        let rhs = destructuring_lhs.remove_first_child(compiler).unwrap();
        let assign = self
            .ast_factory
            .create_assign(compiler, destructuring_pattern, rhs);
        assign.srcref(compiler, var);
        let expr = self.ast_factory.expr_result(compiler, assign);
        expr.srcref(compiler, var);

        var.replace_with(compiler, expr);
        expr
    }

    /// Creates a new name node and adds the constant name property because a constant variable is
    /// used (the compiler only assigns $jscomp$destructuring$var[num] once)
    // port: Es6RewriteDestructuring#createTempVarNameNode
    fn create_temp_var_name_node(
        &self,
        compiler: &mut AbstractCompiler,
        name: String,
        type_: Type,
    ) -> NodeId {
        // NOTE: This does not really create a constant node as this pass runs before
        // normalization. See b/322009741.
        self.ast_factory.create_constant_name(compiler, name, type_)
    }

    /// Creates a new unique name to use for a pattern we need to rewrite.
    // port: Es6RewriteDestructuring#getTempVariableName
    fn get_temp_variable_name(&mut self) -> String {
        let counter = self.destructuring_var_counter;
        self.destructuring_var_counter += 1;
        format!("{DESTRUCTURING_TEMP_VAR}{counter}")
    }

    // port: Es6RewriteDestructuring#visitPattern
    fn visit_pattern(&mut self, t: &mut NodeTraversal<'_>, pattern: NodeId) {
        let parent = pattern.get_parent(t).unwrap();

        match parent.get_token(t) {
            Token::DESTRUCTURING_LHS => {
                let declaration = parent.get_parent(t).unwrap();
                let declaration_parent = declaration.get_parent(t).unwrap();
                if declaration_parent.is_vanilla_for(t) {
                    self.visit_destructuring_pattern_in_vanilla_for_inner_vars(t, pattern);
                } else if NodeUtil::is_enhanced_for(t, declaration_parent) {
                    self.visit_destructuring_pattern_in_enhanced_for_inner_vars(
                        t.get_compiler(),
                        pattern,
                    );
                } else {
                    let rhs = pattern.get_next(t).unwrap();
                    self.replace_pattern(t, pattern, rhs, declaration, declaration);
                }
            }
            Token::ASSIGN => {
                let grandparent = parent.get_parent(t).unwrap();
                if grandparent.is_expr_result(t) {
                    let rhs = pattern.get_next(t).unwrap();
                    self.replace_pattern(t, pattern, rhs, parent, grandparent);
                } else {
                    self.wrap_assign_or_destructuring_in_call_to_arrow(t, parent);
                }
            }
            Token::OBJECT_REST
            | Token::ITER_REST
            | Token::STRING_KEY
            | Token::ARRAY_PATTERN
            | Token::DEFAULT_VALUE
            | Token::COMPUTED_PROP => {
                // Nested pattern; do nothing. We will visit it after rewriting the parent.
            }
            Token::FOR_OF | Token::FOR_IN | Token::FOR_AWAIT_OF => {
                self.visit_destructuring_pattern_in_enhanced_for_with_outer_vars(
                    t.get_compiler(),
                    pattern,
                );
            }
            Token::CATCH => self.visit_destructuring_pattern_in_catch(t, pattern),
            _ => panic!("unexpected parent"),
        }
    }

    /// Transpiles a destructuring pattern in a declaration or assignment to ES5
    ///
    /// `node_to_detach`: a statement node containing the pattern. This method will replace the
    /// node with one or more other statements.
    // port: Es6RewriteDestructuring#replacePattern
    fn replace_pattern(
        &mut self,
        t: &mut NodeTraversal<'_>,
        pattern: NodeId,
        rhs: NodeId,
        parent: NodeId,
        node_to_detach: NodeId,
    ) {
        check_argument!(
            NodeUtil::is_statement(t, node_to_detach),
            "%s",
            node_to_detach.to_string(t)
        );
        match pattern.get_token(t) {
            Token::ARRAY_PATTERN => {
                self.replace_array_pattern(t, pattern, rhs, parent, node_to_detach)
            }
            Token::OBJECT_PATTERN => {
                self.replace_object_pattern(t, pattern, rhs, parent, node_to_detach)
            }
            _ => panic!("unexpected"),
        }
    }

    /// Convert 'var {a: b, c: d} = rhs' to:
    ///
    /// @const var temp = rhs; var b = temp.a; var d = temp.c;
    // port: Es6RewriteDestructuring#replaceObjectPattern
    fn replace_object_pattern(
        &mut self,
        t: &mut NodeTraversal<'_>,
        object_pattern: NodeId,
        rhs: NodeId,
        parent: NodeId,
        node_to_detach: NodeId,
    ) {
        let temp_var_name = self.get_temp_variable_name();
        let temp_var_type = AstFactory::type_node(object_pattern);

        let mut rest_temp_var_name: Option<String> = None;
        // If the last child is a rest node we will want a list of the stated properties so we can
        // exclude them from being written to the rest variable.
        let mut props_to_delete_for_rest: Option<Vec<NodeId>> = None;
        if object_pattern.has_children(t) && object_pattern.get_last_child(t).unwrap().is_rest(t) {
            props_to_delete_for_rest = Some(Vec::new());
            rest_temp_var_name = Some(self.get_temp_variable_name());
        } else if self.rewrite_mode == ObjectDestructuringRewriteMode::REWRITE_OBJECT_REST {
            // We are configured to only break object pattern rest, but this destructure has none.
            if !self
                .pattern_nesting_stack
                .back()
                .unwrap()
                .has_nested_object_rest
            {
                // Replacement is performed after the post-order visit has reached the root pattern
                // node, so peeking last represents if there is a rest property anywhere in the
                // entire pattern. All nesting levels of lower levels have already been popped.
                self.destructuring_var_counter -= 1;
                return;
            }
        }

        let compiler = t.get_compiler();
        // create the declaration `var temp = rhs;`
        let temp_name =
            self.create_temp_var_name_node(compiler, temp_var_name.clone(), temp_var_type.clone());
        rhs.detach(compiler);
        let temp_decl = IR::var_with_value(compiler, temp_name, rhs)
            .srcref_tree_if_missing(compiler, object_pattern);
        // TODO(tbreisacher): Remove the "if" and add this JSDoc unconditionally.
        if parent.is_const(compiler) {
            let mut js_doc = JSDocInfo::builder();
            js_doc.record_constancy();
            let built = js_doc.build();
            temp_decl.set_jsdoc_info(compiler, built);
        }
        temp_decl.insert_before(compiler, node_to_detach);

        let mut child = object_pattern.get_first_child(t);
        while let Some(c) = child {
            let next = c.get_next(t);

            let new_lhs: NodeId;
            let new_rhs: NodeId;
            if c.is_string_key(t) {
                let compiler = t.get_compiler();
                // const {a: b} = obj;
                let temp_var_name_node = self.create_temp_var_name_node(
                    compiler,
                    temp_var_name.clone(),
                    temp_var_type.clone(),
                );
                let getprop = if c.is_quoted_string_key(compiler) {
                    let key = c.get_string(compiler);
                    let key_string = self.ast_factory.create_string(compiler, key);
                    self.ast_factory
                        .create_get_elem(compiler, temp_var_name_node, key_string)
                } else {
                    let key = c.get_string(compiler);
                    self.ast_factory.create_get_prop(
                        compiler,
                        temp_var_name_node,
                        key,
                        temp_var_type.clone(),
                    )
                };

                let value = c.remove_first_child(compiler).unwrap();
                if !value.is_default_value(compiler) {
                    new_lhs = value;
                    new_rhs = getprop;
                } else {
                    new_lhs = value.remove_first_child(compiler).unwrap();
                    let default_value = value.remove_first_child(compiler).unwrap();
                    let intermediate_temp_var_name = self.get_temp_variable_name();
                    let intermediate_name = self.create_temp_var_name_node(
                        compiler,
                        intermediate_temp_var_name.clone(),
                        AstFactory::type_node(getprop),
                    );
                    let intermediate_decl =
                        IR::var_with_value(compiler, intermediate_name, getprop);
                    intermediate_decl.srcref_tree_if_missing(compiler, c);
                    intermediate_decl.insert_before(compiler, node_to_detach);
                    let intermediate_ref = self.create_temp_var_name_node(
                        compiler,
                        intermediate_temp_var_name,
                        AstFactory::type_node(getprop),
                    );
                    new_rhs = self.default_value_hook(compiler, intermediate_ref, default_value);
                }
                if let Some(props) = props_to_delete_for_rest.as_mut() {
                    props.push(c);
                }
            } else if c.is_computed_prop(t) {
                let compiler = t.get_compiler();
                // const {[propExpr]: newLHS = defaultValue} = newRHS;
                let has_default = c
                    .get_last_child(compiler)
                    .unwrap()
                    .is_default_value(compiler);
                let default_value: Option<NodeId>;
                let mut prop_expr = c.remove_first_child(compiler).unwrap();
                if has_default {
                    let default_node = c.get_last_child(compiler).unwrap();
                    new_lhs = default_node.remove_first_child(compiler).unwrap();
                    default_value = default_node.remove_first_child(compiler);
                } else {
                    new_lhs = c.remove_first_child(compiler).unwrap();
                    default_value = None;
                }
                if let Some(props) = props_to_delete_for_rest.as_mut() {
                    // A "...rest" variable is present and result of computation must be cached
                    let expr_eval_temp_var_name = self.get_temp_variable_name();
                    let expr_eval_temp_var_model = self.create_temp_var_name_node(
                        compiler,
                        expr_eval_temp_var_name,
                        AstFactory::type_node(prop_expr),
                    ); // clone this node
                    let model_clone = expr_eval_temp_var_model.clone_node(compiler);
                    let expr_eval_decl = IR::var_with_value(compiler, model_clone, prop_expr);
                    expr_eval_decl.srcref_tree_if_missing(compiler, c);
                    expr_eval_decl.insert_before(compiler, node_to_detach);
                    prop_expr = expr_eval_temp_var_model.clone_node(compiler);
                    props.push(expr_eval_temp_var_model.clone_node(compiler));
                }
                if has_default {
                    // tempVarName[propExpr]
                    let temp_ref = self.create_temp_var_name_node(
                        compiler,
                        temp_var_name.clone(),
                        temp_var_type.clone(),
                    );
                    let getelem = self
                        .ast_factory
                        .create_get_elem(compiler, temp_ref, prop_expr);

                    // var tempVarName2 = tempVarName1[propExpr]
                    let intermediate_temp_var_name = self.get_temp_variable_name();
                    let intermediate_name = self.create_temp_var_name_node(
                        compiler,
                        intermediate_temp_var_name.clone(),
                        AstFactory::type_node(getelem),
                    );
                    let intermediate_decl =
                        IR::var_with_value(compiler, intermediate_name, getelem);
                    intermediate_decl.srcref_tree_if_missing(compiler, c);
                    intermediate_decl.insert_before(compiler, node_to_detach);

                    // tempVarName2 === undefined ? defaultValue : tempVarName2
                    let intermediate_ref = self.create_temp_var_name_node(
                        compiler,
                        intermediate_temp_var_name,
                        AstFactory::type_node(getelem),
                    );
                    new_rhs =
                        self.default_value_hook(compiler, intermediate_ref, default_value.unwrap());
                } else {
                    let temp_ref = self.create_temp_var_name_node(
                        compiler,
                        temp_var_name.clone(),
                        AstFactory::type_node(new_lhs),
                    );
                    new_rhs = self
                        .ast_factory
                        .create_get_elem(compiler, temp_ref, prop_expr);
                }
            } else if c.is_rest(t) {
                let compiler = t.get_compiler();
                if next.is_some() {
                    panic!("object rest may not be followed by any properties");
                }
                // Emit Object.assign here after preceding property bindings to preserve
                // evaluation side-effect ordering (e.g. default value evaluations, property
                // extractions).
                // TODO(b/538156291): Properties already bound are still copied during
                // Object.assign and then deleted, so getters for those properties may still be
                // invoked during Object.assign.
                // TODO(b/116532470): see if casting this to a more specific type fixes
                // disambiguation
                let object_assign =
                    self.ast_factory
                        .create_qname(compiler, &self.namespace, "Object.assign");
                let assign_call = self.ast_factory.create_call(
                    compiler,
                    object_assign,
                    AstFactory::type_(standard_colors::TOP_OBJECT.clone()),
                    &[],
                );
                let object_lit = self.ast_factory.create_object_lit(compiler, &[]);
                assign_call.add_child_to_back(compiler, object_lit);
                let temp_ref = self.create_temp_var_name_node(
                    compiler,
                    temp_var_name.clone(),
                    temp_var_type.clone(),
                );
                assign_call.add_child_to_back(compiler, temp_ref);

                let rest_temp_var_name = rest_temp_var_name.clone().unwrap();
                let rest_temp_name = self.create_temp_var_name_node(
                    compiler,
                    rest_temp_var_name.clone(),
                    temp_var_type.clone(),
                );
                let rest_temp_decl = IR::var_with_value(compiler, rest_temp_name, assign_call);
                rest_temp_decl.srcref_tree_if_missing(compiler, object_pattern);
                rest_temp_decl.insert_before(compiler, node_to_detach);

                let rest_name = c.get_only_child(compiler); // e.g. get `rest` from `const {...rest} = {};`
                let rest_name_string = rest_name.get_string(compiler);
                if rest_name_string.starts_with(&DESTRUCTURING_TEMP_VAR.into()) {
                    check_state!(
                        rest_name.is_name(compiler),
                        "%s",
                        rest_name.to_string(compiler)
                    );
                    new_lhs = self.create_temp_var_name_node(
                        compiler,
                        rest_name_string.to_string(),
                        AstFactory::type_node(rest_name),
                    );
                } else {
                    rest_name.detach(compiler);
                    new_lhs = rest_name;
                }
                new_rhs = self.object_pattern_rest_rhs(
                    compiler,
                    object_pattern,
                    c,
                    rest_temp_var_name,
                    props_to_delete_for_rest.as_ref().unwrap(),
                );
            } else {
                panic!("unexpected child");
            }

            let compiler = t.get_compiler();
            let new_node;
            if NodeUtil::is_name_declaration(compiler, Some(parent)) {
                if parent.is_const(compiler) {
                    new_lhs.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
                }
                let token = parent.get_token(compiler);
                new_node = IR::declaration_with_value(compiler, new_lhs, new_rhs, token);
            } else if parent.is_assign(compiler) {
                let assign = self.ast_factory.create_assign(compiler, new_lhs, new_rhs);
                new_node = IR::expr_result(compiler, assign);
            } else {
                panic!("not reached");
            }
            new_node.srcref_tree_if_missing(compiler, c);

            new_node.insert_before(compiler, node_to_detach);

            // Explicitly visit the LHS of the new node since it may be a nested
            // destructuring pattern.
            let new_lhs_parent = new_lhs.get_parent(t);
            self.visit(t, new_lhs, new_lhs_parent);
            child = next;
        }

        node_to_detach.detach(t);
        t.report_code_change();
    }

    /// Convert "rest" of object destructuring lhs by making a clone and deleting any properties
    /// that were stated in the original object pattern.
    ///
    /// Nodes in statedProperties that are a stringKey will be used in a getprop when deleting. All
    /// other types will be used in a getelem such as what is done for computed properties.
    ///
    /// ```text
    ///   {a, [foo()]:b, ...x} = rhs;
    /// becomes
    ///   var temp = rhs;
    ///   a = temp.a;
    ///   var temp2 = foo();
    ///   b = temp[temp2];
    ///   var temp1 = Object.assign({}, temp);
    ///   x = (delete temp1.a, delete temp1[temp2], temp1);
    /// ```
    ///
    /// `rest`: node representing the "...rest" of objectPattern; `rest_temp_var_name`: name of var
    /// containing clone of result of rhs evaluation; `stated_properties`: list of properties to
    /// delete from the clone
    // port: Es6RewriteDestructuring#objectPatternRestRHS
    fn object_pattern_rest_rhs(
        &self,
        compiler: &mut AbstractCompiler,
        object_pattern: NodeId,
        rest: NodeId,
        rest_temp_var_name: String,
        stated_properties: &[NodeId],
    ) -> NodeId {
        check_argument!(object_pattern.get_last_child(compiler) == Some(rest));
        let rest_temp_var_model = self.create_temp_var_name_node(
            compiler,
            rest_temp_var_name,
            AstFactory::type_node(object_pattern),
        );
        let mut result = rest_temp_var_model.clone_node(compiler);
        if !stated_properties.is_empty() {
            let mut prop_itr = stated_properties.iter();
            let model_clone = rest_temp_var_model.clone_node(compiler);
            let mut comma = self.deletion_node_for_rest_property(
                compiler,
                model_clone,
                *prop_itr.next().unwrap(),
            );
            for &prop in prop_itr {
                let model_clone = rest_temp_var_model.clone_node(compiler);
                let deletion = self.deletion_node_for_rest_property(compiler, model_clone, prop);
                comma = self.ast_factory.create_comma(compiler, comma, deletion);
            }
            result = self.ast_factory.create_comma(compiler, comma, result);
        }
        result.srcref_tree_if_missing(compiler, rest);
        result
    }

    // port: Es6RewriteDestructuring#deletionNodeForRestProperty
    fn deletion_node_for_rest_property(
        &self,
        compiler: &mut AbstractCompiler,
        rest_temp_var_name_node: NodeId,
        property: NodeId,
    ) -> NodeId {
        let get = match property.get_token(compiler) {
            Token::STRING_KEY => {
                if property.is_quoted_string_key(compiler) {
                    let key = property.get_string(compiler);
                    let key_string = self.ast_factory.create_string(compiler, key);
                    self.ast_factory
                        .create_get_elem(compiler, rest_temp_var_name_node, key_string)
                } else {
                    let key = property.get_string(compiler);
                    self.ast_factory.create_get_prop_with_unknown_type(
                        compiler,
                        rest_temp_var_name_node,
                        key,
                    )
                }
            }
            Token::NAME => {
                self.ast_factory
                    .create_get_elem(compiler, rest_temp_var_name_node, property)
            }
            _ => panic!(
                "Unexpected property to delete node: {}",
                property.to_string_tree(compiler)
            ),
        };

        self.ast_factory.create_del_prop(compiler, get)
    }

    /// Convert `var [x, y] = rhs` to:
    ///
    /// ```text
    ///   var temp = $jscomp.makeIterator(rhs);
    ///   var x = temp.next().value;
    ///   var y = temp.next().value;
    /// ```
    // port: Es6RewriteDestructuring#replaceArrayPattern
    fn replace_array_pattern(
        &mut self,
        t: &mut NodeTraversal<'_>,
        array_pattern: NodeId,
        rhs: NodeId,
        parent: NodeId,
        node_to_detach: NodeId,
    ) {
        if self.rewrite_mode == ObjectDestructuringRewriteMode::REWRITE_OBJECT_REST
            && (self.pattern_nesting_stack.is_empty()
                || !self
                    .pattern_nesting_stack
                    .back()
                    .unwrap()
                    .has_nested_object_rest)
        {
            return;
        }

        let temp_var_name = self.get_temp_variable_name();
        let compiler = t.get_compiler();
        rhs.detach(compiler);
        let make_iterator_call =
            self.ast_factory
                .create_jscomp_make_iterator_call(compiler, rhs, &self.namespace);
        let temp_decl = self
            .ast_factory
            .create_single_var_name_declaration_with_value(
                compiler,
                temp_var_name,
                make_iterator_call,
            );
        let temp_var_model = temp_decl.get_first_child(compiler).unwrap();
        temp_decl.srcref_tree_if_missing(compiler, array_pattern);
        temp_decl.insert_before(compiler, node_to_detach);

        let mut child = array_pattern.get_first_child(t);
        while let Some(c) = child {
            let next = c.get_next(t);
            let compiler = t.get_compiler();
            if c.is_empty(compiler) {
                // Just call the next() method to advance the iterator, but throw away the value.
                let model_clone = temp_var_model.clone_node(compiler);
                let next_prop = self.ast_factory.create_get_prop(
                    compiler,
                    model_clone,
                    "next",
                    AstFactory::type_(standard_colors::TOP_OBJECT.clone()),
                );
                let call = self
                    .ast_factory
                    .create_call_with_unknown_type(compiler, next_prop, &[]);
                let next_call = IR::expr_result(compiler, call);
                next_call.srcref_tree_if_missing(compiler, c);
                next_call.insert_before(compiler, node_to_detach);
                child = next;
                continue;
            }

            let new_lhs: NodeId;
            let new_rhs: NodeId;
            if c.is_default_value(compiler) {
                //   [x = defaultValue] = rhs;
                // becomes
                //   var temp0 = $jscomp.makeIterator(rhs);
                //   var temp1 = temp.next().value
                //   x = (temp1 === undefined) ? defaultValue : temp1;
                let next_var_name = self.get_temp_variable_name();
                // `temp.next().value`
                let model_clone = temp_var_model.clone_node(compiler);
                let next_prop = self.ast_factory.create_get_prop_with_unknown_type(
                    compiler,
                    model_clone,
                    "next",
                );
                let next_call =
                    self.ast_factory
                        .create_call_with_unknown_type(compiler, next_prop, &[]);
                let next_call_dot_value = self
                    .ast_factory
                    .create_get_prop_with_unknown_type(compiler, next_call, "value");
                let next_var_type = AstFactory::type_node(next_call_dot_value);
                // `var temp1 = temp.next().value`
                let next_var_name_node = self.create_temp_var_name_node(
                    compiler,
                    next_var_name.clone(),
                    next_var_type.clone(),
                );
                let var = IR::var_with_value(compiler, next_var_name_node, next_call_dot_value);
                var.srcref_tree_if_missing(compiler, c);
                var.insert_before(compiler, node_to_detach);

                // `x`
                new_lhs = c.remove_first_child(compiler).unwrap();
                // `(temp1 === undefined) ? defaultValue : temp1;
                let next_var_ref =
                    self.create_temp_var_name_node(compiler, next_var_name, next_var_type);
                let default_value = c.get_last_child(compiler).unwrap();
                default_value.detach(compiler);
                new_rhs = self.default_value_hook(compiler, next_var_ref, default_value);
            } else if c.is_rest(compiler) {
                //   [...x] = rhs;
                // becomes
                //   var temp = $jscomp.makeIterator(rhs);
                //   x = $jscomp.arrayFromIterator(temp);
                new_lhs = c.remove_first_child(compiler).unwrap();
                let model_clone = temp_var_model.clone_node(compiler);
                new_rhs = self.ast_factory.create_jscomp_array_from_iterator_call(
                    compiler,
                    model_clone,
                    &self.namespace,
                );
            } else {
                // LHS is just a name (or a nested pattern).
                //   var [x] = rhs;
                // becomes
                //   var temp = $jscomp.makeIterator(rhs);
                //   var x = temp.next().value;
                c.detach(compiler);
                new_lhs = c;
                let model_clone = temp_var_model.clone_node(compiler);
                let next_prop = self.ast_factory.create_get_prop_with_unknown_type(
                    compiler,
                    model_clone,
                    "next",
                );
                let next_call =
                    self.ast_factory
                        .create_call_with_unknown_type(compiler, next_prop, &[]);
                new_rhs = self.ast_factory.create_get_prop(
                    compiler,
                    next_call,
                    "value",
                    AstFactory::type_node(c),
                );
            }
            let new_node;
            if parent.is_assign(compiler) {
                let assignment = self.ast_factory.create_assign(compiler, new_lhs, new_rhs);
                new_node = IR::expr_result(compiler, assignment);
            } else {
                let token = parent.get_token(compiler);
                new_node = IR::declaration_with_value(compiler, new_lhs, new_rhs, token);
            }
            new_node.srcref_tree_if_missing(compiler, array_pattern);

            new_node.insert_before(compiler, node_to_detach);
            // Explicitly visit the LHS of the new node since it may be a nested
            // destructuring pattern.
            let new_lhs_parent = new_lhs.get_parent(t);
            self.visit(t, new_lhs, new_lhs_parent);
            child = next;
        }

        node_to_detach.detach(t);
        t.report_code_change();
    }

    /// Replace ASSIGN or DESTRUCTURING_LHS with a IIFE that contains the transpiled destructuring.
    ///
    /// ```text
    /// Transform
    ///   [x, y] = rhs
    /// into
    ///   ((temp0) => {
    ///     var temp1 = $jscomp.makeIterator(temp0);
    ///     var x = temp0.next().value;
    ///     var y = temp0.next().value;
    ///     return temp0;
    ///   })(rhs)
    ///
    /// Transform
    ///   {x: a, y: b} = rhs
    /// into
    ///   ((temp0) => {
    ///     var temp1 = temp0;
    ///     var a = temp0.x;
    ///     var b = temp0.y;
    ///     return temp0;
    ///   })(rhs)
    /// ```
    // port: Es6RewriteDestructuring#wrapAssignOrDestructuringInCallToArrow
    fn wrap_assign_or_destructuring_in_call_to_arrow(
        &mut self,
        t: &mut NodeTraversal<'_>,
        assignment: NodeId,
    ) {
        let lhs = assignment.get_first_child(t).unwrap();
        let rhs = assignment.get_last_child(t).unwrap();
        rhs.detach(t);
        // NOTE: we do not presently support await/yield in the LHS of nested destructuring
        // assignments. See b/475296868.
        Self::validate_no_await_or_yield_in_nested_assignment_pattern(t, lhs);

        let temp_var_name = self.get_temp_variable_name();

        let compiler = t.get_compiler();
        let af = &self.ast_factory;
        let temp_var_model =
            self.create_temp_var_name_node(compiler, temp_var_name, AstFactory::type_node(rhs));
        //  ((temp0) => {...})
        let model_clone = temp_var_model.clone_node(compiler);
        let param_list = IR::param_list(compiler, &[model_clone]);
        // [x, y] = temp0;
        let assignment_lhs = assignment.remove_first_child(compiler).unwrap();
        let model_clone = temp_var_model.clone_node(compiler);
        let replacement_expr = af.create_assign(compiler, assignment_lhs, model_clone);
        let expr_result = IR::expr_result(compiler, replacement_expr);
        // return temp0;
        let model_clone = temp_var_model.clone_node(compiler);
        let return_node = IR::return_node_with_expression(compiler, model_clone);

        // Create a function to hold these assignments:
        let block = IR::block_with_children(compiler, &[expr_result, return_node]);
        let arrow_fn = af.create_function(
            compiler,
            /* name= */ "",
            param_list,
            block,
            AstFactory::type_native_and_color(
                JSTypeNative::UNKNOWN_TYPE,
                standard_colors::UNKNOWN.clone(),
            ),
        );
        arrow_fn.set_is_arrow_function(compiler, true);

        // Create a call to the function, and replace the pattern with the call.
        let call = af.create_call(compiler, arrow_fn, AstFactory::type_node(rhs), &[rhs]);
        let script = t.get_current_script().unwrap();
        let compiler = t.get_compiler();
        NodeUtil::add_feature_to_script(compiler, script, Feature::ARROW_FUNCTIONS);
        call.srcref_tree_if_missing(compiler, assignment);
        call.put_boolean_prop(compiler, Prop::FREE_CALL, true);
        assignment.replace_with(compiler, call);
        NodeUtil::mark_new_scopes_changed(compiler, call);

        let pattern = replacement_expr.get_first_child(compiler).unwrap();
        let pattern_rhs = replacement_expr.get_last_child(compiler).unwrap();
        self.replace_pattern(t, pattern, pattern_rhs, replacement_expr, expr_result);
    }

    // port: Es6RewriteDestructuring#validateNoAwaitOrYieldInNestedAssignmentPattern
    fn validate_no_await_or_yield_in_nested_assignment_pattern(
        t: &mut NodeTraversal<'_>,
        pattern: NodeId,
    ) {
        let has_await_or_yield_in_lhs = NodeUtil::find_preorder(
            t,
            pattern,
            &|ast, n| n.is_await(ast) || n.is_yield(ast),
            &NodeUtil::MATCH_NOT_FUNCTION,
        )
        .is_some();
        check_state!(
            !has_await_or_yield_in_lhs,
            "Cannot transpile yet: destructuring assignment referencing await or yield in lhs, in nested sub-expression: %s",
            pattern.to_string(t)
        );
    }

    /// for (let [a, b, c] = arr; a < b; a++)
    // port: Es6RewriteDestructuring#visitDestructuringPatternInVanillaForInnerVars
    fn visit_destructuring_pattern_in_vanilla_for_inner_vars(
        &mut self,
        t: &mut NodeTraversal<'_>,
        pattern: NodeId,
    ) {
        check_argument!(pattern.is_destructuring_pattern(t));
        let destructuring_lhs = pattern.get_parent(t).unwrap();
        let declaration = destructuring_lhs.get_parent(t).unwrap();

        let mut insertion_point = declaration.get_parent(t).unwrap();
        while insertion_point.get_parent(t).unwrap().is_label(t) {
            insertion_point = insertion_point.get_parent(t).unwrap();
        }

        let token = declaration.get_token(t);
        match token {
            Token::CONST | Token::VAR => {
                if token == Token::CONST {
                    let compiler = t.get_compiler();
                    let block = IR::block(compiler).srcref(compiler, insertion_point);
                    insertion_point.replace_with(compiler, block);
                    block.add_child_to_back(compiler, insertion_point);
                }
                // Fall through

                let compiler = t.get_compiler();
                // Move any earlier variables out of the loop initializer
                let mut c = declaration.get_first_child(compiler).unwrap();
                while c != destructuring_lhs {
                    let new_declaration = declaration.clone_node(compiler);
                    c.detach(compiler);
                    new_declaration.add_child_to_back(compiler, c);
                    new_declaration.insert_before(compiler, insertion_point);
                    c = declaration.get_first_child(compiler).unwrap();
                }

                // Move the pattern out of the initializer and transpile it
                let new_declaration = declaration.clone_node(compiler);
                destructuring_lhs.detach(compiler);
                new_declaration.add_child_to_back(compiler, destructuring_lhs);
                new_declaration.insert_before(compiler, insertion_point);
                let pattern_rhs = pattern.get_next(compiler).unwrap();
                self.replace_pattern(t, pattern, pattern_rhs, new_declaration, new_declaration);

                let compiler = t.get_compiler();
                if !declaration.has_children(compiler) {
                    let empty = IR::empty(compiler);
                    declaration.replace_with(compiler, empty);
                }
            }
            Token::LET => {
                // See https://tc39.es/ecma262/#sec-createperiterationenvironment
                // for (let a, b, c, unusedTmp = (() => [a, b, c] = arr); a < b; a++)
                let compiler = t.get_compiler();
                let mut names = Vec::new();
                ParsingUtil::get_param_or_pattern_names(compiler, pattern, &mut |name| {
                    names.push(name);
                });
                for name in names {
                    name.clone_node(compiler)
                        .insert_before(compiler, destructuring_lhs);
                }

                let unused_name = format!("{}$unused", self.get_temp_variable_name());
                let unused_var = self
                    .ast_factory
                    .create_name_with_unknown_type(compiler, unused_name)
                    .srcref(compiler, destructuring_lhs);
                destructuring_lhs.replace_with(compiler, unused_var);
                unused_var.add_child_to_back(compiler, destructuring_lhs);
                self.wrap_assign_or_destructuring_in_call_to_arrow(t, destructuring_lhs);
            }
            _ => panic!("{}", declaration.to_string(t)),
        }

        t.get_compiler()
            .report_change_to_enclosing_scope(insertion_point);
    }

    /// for (const [a, b, c] of arr)
    // port: Es6RewriteDestructuring#visitDestructuringPatternInEnhancedForInnerVars
    fn visit_destructuring_pattern_in_enhanced_for_inner_vars(
        &mut self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
    ) {
        check_argument!(pattern.is_destructuring_pattern(compiler));
        let temp_var_name = self.get_temp_variable_name();

        let destructuring_lhs = pattern.get_parent(compiler).unwrap();
        check_state!(destructuring_lhs.is_destructuring_lhs(compiler));
        let declaration_node = destructuring_lhs.get_parent(compiler).unwrap();
        let for_node = declaration_node.get_parent(compiler).unwrap();
        check_state!(NodeUtil::is_enhanced_for(compiler, for_node));
        let block = for_node.get_last_child(compiler).unwrap();

        let temp_name = self
            .create_temp_var_name_node(
                compiler,
                temp_var_name.clone(),
                AstFactory::type_node(pattern),
            )
            .srcref(compiler, pattern);
        destructuring_lhs.replace_with(compiler, temp_name);
        let declaration_type = declaration_node.get_token(compiler);
        pattern.detach(compiler);
        let temp_ref =
            self.create_temp_var_name_node(compiler, temp_var_name, AstFactory::type_node(pattern));
        let decl = IR::declaration_with_value(compiler, pattern, temp_ref, declaration_type);
        decl.srcref_tree_if_missing(compiler, pattern);
        // Move the body into an inner block to handle cases where declared variables in the for
        // loop initializer are shadowed by variables in the for loop body. e.g.
        //   for (const [value] of []) { const value = 1; }
        let new_block = IR::block_with_child(compiler, decl);
        block.replace_with(compiler, new_block);
        new_block.add_child_to_back(compiler, block);
    }

    /// for ([a, b, c] of arr)
    ///
    /// ```text
    /// Transform
    ///   for ({x} of y) {}
    /// into
    ///   var TEMP_VAR0;
    ///   for (TEMP_VAR0 of y) {
    ///     var TEMP_VAR1 = TEMP_VAR0;
    ///     x = TEMP_VAR1.x;
    ///   }
    /// ```
    // port: Es6RewriteDestructuring#visitDestructuringPatternInEnhancedForWithOuterVars
    fn visit_destructuring_pattern_in_enhanced_for_with_outer_vars(
        &mut self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
    ) {
        check_argument!(pattern.is_destructuring_pattern(compiler));
        let temp_var_name = self.get_temp_variable_name();

        let for_node = pattern.get_parent(compiler).unwrap();
        let block = for_node.get_last_child(compiler).unwrap();

        let name = self.create_temp_var_name_node(
            compiler,
            temp_var_name.clone(),
            AstFactory::type_node(pattern),
        );
        let decl = IR::var(compiler, name);
        decl.srcref_tree_if_missing(compiler, pattern);
        decl.insert_before(compiler, for_node);
        let cloned_name = name.clone_node(compiler);
        cloned_name.srcref_tree_if_missing(compiler, pattern);
        pattern.replace_with(compiler, cloned_name);
        let temp_ref =
            self.create_temp_var_name_node(compiler, temp_var_name, AstFactory::type_node(pattern));
        let assign = self.ast_factory.create_assign(compiler, pattern, temp_ref);
        let expr_result = IR::expr_result(compiler, assign);
        expr_result.srcref_tree_if_missing(compiler, pattern);
        block.add_child_to_front(compiler, expr_result);
    }

    // port: Es6RewriteDestructuring#visitDestructuringPatternInCatch
    fn visit_destructuring_pattern_in_catch(&mut self, t: &mut NodeTraversal<'_>, pattern: NodeId) {
        let temp_var_name = self.get_temp_variable_name();
        let catch_block = pattern.get_next(t).unwrap();
        let pattern_type = AstFactory::type_node(pattern);

        let compiler = t.get_compiler();
        let temp_name =
            self.create_temp_var_name_node(compiler, temp_var_name.clone(), pattern_type.clone());
        pattern.replace_with(compiler, temp_name);
        let temp_ref = self.create_temp_var_name_node(compiler, temp_var_name, pattern_type);
        let declaration = IR::declaration_with_value(compiler, pattern, temp_ref, Token::LET);
        catch_block.add_child_to_front(compiler, declaration);
        let script = t.get_current_script().unwrap();
        NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::LET_DECLARATIONS);
    }

    /// Helper for transpiling DEFAULT_VALUE trees.
    // port: Es6RewriteDestructuring#defaultValueHook
    fn default_value_hook(
        &self,
        compiler: &mut AbstractCompiler,
        getprop: NodeId,
        default_value: NodeId,
    ) -> NodeId {
        let undefined = self.ast_factory.create_undefined_value(compiler);
        undefined.make_non_indexable(compiler);
        let color = getprop.get_color(compiler);
        let getprop_clone = getprop.clone_tree(compiler).set_color(compiler, color);
        let sheq = self.ast_factory.create_sheq(compiler, getprop, undefined);
        self.ast_factory
            .create_hook(compiler, sheq, default_value, getprop_clone)
    }
}

impl CompilerPass for Es6RewriteDestructuring {
    // port: Es6RewriteDestructuring#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        check_state!(self.pattern_nesting_stack.is_empty());
        NodeTraversal::traverse(compiler, root, self);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            self.features_to_mark_as_removed,
        );
        check_state!(self.pattern_nesting_stack.is_empty());
    }
}

impl Callback for Es6RewriteDestructuring {
    // port: Es6RewriteDestructuring#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            let script_features = NodeUtil::get_feature_set_of_script(t, n)
                .expect("NullPointerException: getFeatureSetOfScript");
            // This will ensure that we run this only when features exist in the script
            return script_features.contains_at_least_one_of(self.features_to_trigger_running_pass);
        }
        match n.get_token(t) {
            Token::PARAM_LIST => {
                self.pull_destructuring_out_of_params(t.get_compiler(), n, parent.unwrap())
            }
            Token::ARRAY_PATTERN | Token::OBJECT_PATTERN => {
                let has_rest = n.is_object_pattern(t)
                    && n.has_children(t)
                    && n.get_last_child(t).unwrap().is_rest(t);
                if !self.pattern_nesting_stack.is_empty() && has_rest {
                    for level in self.pattern_nesting_stack.iter_mut() {
                        if level.has_nested_object_rest {
                            break;
                        }
                        level.has_nested_object_rest = true;
                    }
                    self.pattern_nesting_stack
                        .back_mut()
                        .unwrap()
                        .has_nested_object_rest = true;
                }
                self.pattern_nesting_stack
                    .push_back(PatternNestingLevel::new(n, has_rest));
            }
            _ => {}
        }
        true
    }

    // port: Es6RewriteDestructuring#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::ARRAY_PATTERN | Token::OBJECT_PATTERN => {
                self.visit_pattern(t, n);
                if n == self.pattern_nesting_stack.back().unwrap().pattern {
                    self.pattern_nesting_stack.pop_back();
                }
            }
            _ => {}
        }
    }
}
