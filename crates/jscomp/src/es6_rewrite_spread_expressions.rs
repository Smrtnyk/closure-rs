/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Es6RewriteSpreadExpressions.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `Es6RewriteSpreadExpressions.java`: converts ES6 spread expressions in array
//! literals, calls and `new` expressions to ES3 code.

use crate::{
    AbstractCompiler,
    ast_factory::{AstFactory, Type},
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
    transpilation_util::TranspilationUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    jscomp_colors::standard_colors,
    node::{NodeId, Prop},
    token::Token,
};
use std::sync::LazyLock;

// port: Es6RewriteSpreadExpressions#TRANSPILED_FEATURES
static TRANSPILED_FEATURES: LazyLock<FeatureSet> =
    LazyLock::new(|| FeatureSet::BARE_MINIMUM.with(Feature::SPREAD_EXPRESSIONS));

// port: Es6RewriteSpreadExpressions#FRESH_SPREAD_VAR
const FRESH_SPREAD_VAR: &str = "$jscomp$spread$args";

// port: Es6RewriteSpreadExpressions#arrayType
static ARRAY_TYPE: LazyLock<Type> =
    LazyLock::new(|| AstFactory::type_color_id(standard_colors::ARRAY_ID));
// port: Es6RewriteSpreadExpressions#concatFnType
static CONCAT_FN_TYPE: LazyLock<Type> =
    LazyLock::new(|| AstFactory::type_(standard_colors::TOP_OBJECT.clone()));

pub struct Es6RewriteSpreadExpressions {
    ast_factory: AstFactory,
    namespace: TranspilationNamespace,
}

impl Es6RewriteSpreadExpressions {
    // port: Es6RewriteSpreadExpressions#Es6RewriteSpreadExpressions
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let namespace = TranspilationNamespace::get(compiler);
        Self {
            ast_factory,
            namespace,
        }
    }

    /// Processes array literals or calls to eliminate spreads.
    ///
    /// Examples:
    ///
    /// - `[1, 2, ...x, 4, 5] => [1, 2].concat($jscomp.arrayFromIterable(x), [4, 5])`
    /// - `f(1, ...arr) => f.apply(null, [1].concat($jscomp.arrayFromIterable(arr)))`
    /// - `new F(...args) => new Function.prototype.bind.apply(F,
    ///   [null].concat($jscomp.arrayFromIterable(args)))`
    // port: Es6RewriteSpreadExpressions#visitArrayLitOrCallWithSpread
    fn visit_array_lit_or_call_with_spread(
        &self,
        t: &mut NodeTraversal<'_>,
        spread_parent: NodeId,
    ) {
        if spread_parent.is_array_lit(t) {
            self.visit_array_lit_containing_spread(t.get_compiler(), spread_parent);
        } else if spread_parent.is_call(t) {
            self.visit_call_containing_spread(t, spread_parent);
        } else {
            check_argument!(spread_parent.is_new(t), "%s", spread_parent.to_string(t));
            self.visit_new_with_spread(t.get_compiler(), spread_parent);
        }
    }

    /// Extracts child nodes from an ARRAYLIT, CALL or NEW node that may contain spread operators
    /// into a list of nodes that may be concatenated with Array.concat() to get an array.
    ///
    /// Example: `[a, b, ...x, c, ...arguments]` returns a list containing
    /// `[ [a, b], $jscomp.arrayFromIterable(x), [c], $jscomp.arrayFromIterable(arguments) ]`
    ///
    /// IMPORTANT: CALL and NEW nodes must have the first, callee, child removed already.
    ///
    /// Note that all elements of the returned list will be one of:
    ///
    /// - array literal
    /// - `$jscomp.arrayFromIterable(spreadExpression)`
    ///
    /// TODO(bradfordcsmith): When this pass moves after type checking, we can use type information
    /// to avoid unnecessary calls to $jscomp.arrayFromIterable().
    ///
    /// TODO(nickreid): Stop mutating `spreadParent`.
    // port: Es6RewriteSpreadExpressions#extractSpreadGroups
    fn extract_spread_groups(
        &self,
        compiler: &mut AbstractCompiler,
        spread_parent: NodeId,
    ) -> Vec<NodeId> {
        check_argument!(
            spread_parent.is_call(compiler)
                || spread_parent.is_array_lit(compiler)
                || spread_parent.is_new(compiler)
        );

        let mut groups: Vec<NodeId> = Vec::new();
        let mut curr_group: Option<NodeId> = None;

        let mut curr_element = spread_parent.remove_first_child(compiler);
        while let Some(element) = curr_element {
            if element.is_spread(compiler) {
                let spread_expression = element.remove_first_child(compiler).unwrap();
                if spread_expression.is_array_lit(compiler) {
                    // We can expand an array literal spread in place.
                    match curr_group {
                        None => {
                            // [...[spread, contents], a, b]
                            // we can use this array lit itself as a group and append following
                            // elements to it
                            curr_group = Some(spread_expression);
                        }
                        Some(group) => {
                            // [ a, b, ...[spread, contents], c]
                            // Just add contents of this array lit to the group we were already
                            // collecting.
                            let children = spread_expression.remove_children(compiler);
                            group.add_children_to_back(compiler, children);
                        }
                    }
                } else {
                    // We need to treat the spread expression as a separate group
                    if let Some(group) = curr_group {
                        // finish off and add the group we were collecting before
                        groups.push(group);
                        curr_group = None;
                    }

                    groups.push(self.ast_factory.create_jscomp_array_from_iterable_call(
                        compiler,
                        spread_expression,
                        &self.namespace,
                    ));
                }
            } else {
                let group = match curr_group {
                    None => {
                        let group = self.ast_factory.create_arraylit(compiler, &[]);
                        curr_group = Some(group);
                        group
                    }
                    Some(group) => group,
                };
                group.add_child_to_back(compiler, element);
            }
            curr_element = spread_parent.remove_first_child(compiler);
        }

        if let Some(group) = curr_group {
            groups.push(group);
        }
        groups
    }

    /// Processes array literals containing spreads.
    ///
    /// Example:
    ///
    /// ```text
    /// [1, 2, ...x, 4, 5] => [1, 2].concat($jscomp.arrayFromIterable(x), [4, 5])
    /// ```
    // port: Es6RewriteSpreadExpressions#visitArrayLitContainingSpread
    fn visit_array_lit_containing_spread(
        &self,
        compiler: &mut AbstractCompiler,
        spread_parent: NodeId,
    ) {
        check_argument!(spread_parent.is_array_lit(compiler));

        let mut groups = self.extract_spread_groups(compiler, spread_parent);

        let base_array_lit = if groups[0].is_array_lit(compiler) {
            // g0.concat(g1, g2, ..., gn)
            groups.remove(0)
        } else {
            // [].concat(g0, g1, g2, ..., gn)
            self.ast_factory.create_arraylit(compiler, &[])
        };

        let joined_groups = if groups.is_empty() {
            base_array_lit
        } else {
            let concat = self.ast_factory.create_get_prop(
                compiler,
                base_array_lit,
                "concat",
                CONCAT_FN_TYPE.clone(),
            );
            self.ast_factory.create_call(
                compiler,
                concat,
                AstFactory::type_node(spread_parent),
                &groups,
            )
        };

        joined_groups.srcref_tree_if_missing(compiler, spread_parent);

        spread_parent.replace_with(compiler, joined_groups);
        compiler.report_change_to_enclosing_scope(joined_groups);
    }

    /// Processes calls containing spreads.
    ///
    /// Examples:
    ///
    /// ```text
    /// f(...arr) => f.apply(null, $jscomp.arrayFromIterable(arr))
    /// f(a, ...arr) => f.apply(null, [a].concat($jscomp.arrayFromIterable(arr)))
    /// f(...arr, b) => f.apply(null, [].concat($jscomp.arrayFromIterable(arr), [b]))
    /// ```
    // port: Es6RewriteSpreadExpressions#visitCallContainingSpread
    fn visit_call_containing_spread(&self, t: &mut NodeTraversal<'_>, spread_parent: NodeId) {
        check_argument!(spread_parent.is_call(t));

        let mut callee = spread_parent.get_first_child(t).unwrap();
        // ES6 classes must all be transpiled away before this pass runs.
        check_state!(!callee.is_super(t), "Cannot spread into super calls");
        // Check if the callee has side effects before removing it from the AST (since some
        // NodeUtil methods assume the node they are passed has a non-null parent).
        let ast_analyzer = t.get_compiler().get_ast_analyzer();
        let callee_may_have_side_effects =
            ast_analyzer.may_have_side_effects(t.get_compiler(), callee);
        // Must remove callee before extracting argument groups.
        callee.detach(t);

        while callee.is_cast(t) {
            // Drop any CAST nodes. They're not needed anymore since this pass runs at the end of
            // the checks phase, and they complicate detecting GETPROP/GETELEM callees.
            callee = callee.remove_first_child(t).unwrap();
        }
        let joined_groups = if spread_parent.has_one_child(t)
            && Self::is_spread_of_arguments(t, spread_parent.get_only_child(t))
        {
            // Check for special case of `foo(...arguments)` and pass `arguments` directly to
            // `foo.apply(null, arguments)`. We want to avoid calling
            // $jscomp.arrayFromIterable(arguments) for this case, because it can have side
            // effects, which prevents code removal.
            let spread = spread_parent.remove_first_child(t).unwrap();
            spread.remove_first_child(t).unwrap()
        } else {
            let mut groups = self.extract_spread_groups(t.get_compiler(), spread_parent);
            check_state!(!groups.is_empty());

            if groups.len() == 1 {
                // A single group can just be passed to `apply()` as-is
                // It could be `arguments`, an array literal, or
                // $jscomp.arrayFromIterable(someExpression).
                groups.remove(0)
            } else {
                // If the first group is an array literal, we can just use that for concatenation,
                // otherwise use an empty array literal.
                //
                // TODO(bradfordcsmith): Now that this pass runs after type checking, it would be
                // nice to skip creating an array literal when when the type of the first element
                // says it is an Array.
                let compiler = t.get_compiler();
                let base_array_lit = if groups[0].is_array_lit(compiler) {
                    groups.remove(0)
                } else {
                    self.ast_factory.create_arraylit(compiler, &[])
                };
                let concat = self.ast_factory.create_get_prop(
                    compiler,
                    base_array_lit,
                    "concat",
                    CONCAT_FN_TYPE.clone(),
                );
                self.ast_factory
                    .create_call(compiler, concat, ARRAY_TYPE.clone(), &groups)
            }
        };
        let is_free_call = spread_parent.get_boolean_prop(t, Prop::FREE_CALL);

        let call_to_apply = if callee_may_have_side_effects
            && (callee.is_get_prop(t) || callee.is_get_elem(t))
            && !is_free_call
        {
            // foo().method(...[a, b, c]) or foo()['method'](...[a, b, c])
            //   must convert to
            // var freshVar;
            // (freshVar = foo()).method.apply(freshVar, [a, b, c])
            let input = t.get_input().cloned().expect("NullPointerException: input");
            let compiler = t.get_compiler();
            let unique_id = compiler.get_unique_id_supplier().get_unique_id(&input);
            let fresh_var = self.ast_factory.create_name(
                compiler,
                format!("{FRESH_SPREAD_VAR}{unique_id}"),
                // Type of `foo()`.
                AstFactory::type_node(callee.get_first_child(compiler).unwrap()),
            );
            let fresh_var_clone = fresh_var.clone_tree(compiler);
            let fresh_var_declaration = IR::var(compiler, fresh_var_clone);

            let statement_containing_spread =
                NodeUtil::get_enclosing_statement(compiler, spread_parent).unwrap();
            fresh_var_declaration.srcref_tree_if_missing(compiler, statement_containing_spread);

            fresh_var_declaration.insert_before(compiler, statement_containing_spread);
            let fresh_var_clone = fresh_var.clone_tree(compiler);
            let receiver = callee.remove_first_child(compiler).unwrap();
            let assign = self
                .ast_factory
                .create_assign(compiler, fresh_var_clone, receiver);
            callee.add_child_to_front(compiler, assign);

            let apply = self
                .ast_factory
                .create_get_prop_with_unknown_type(compiler, callee, "apply");
            let fresh_var_clone = fresh_var.clone_tree(compiler);
            self.ast_factory.create_call_with_unknown_type(
                compiler,
                apply,
                &[fresh_var_clone, joined_groups],
            )
        } else {
            // foo.method(...[a, b, c]) -> foo.method.apply(foo, [a, b, c])
            // foo['method'](...[a, b, c]) -> foo['method'].apply(foo, [a, b, c])
            // or
            // foo(...[a, b, c]) -> foo.apply(null, [a, b, c])
            let compiler = t.get_compiler();
            let context = if (callee.is_get_prop(compiler) || callee.is_get_elem(compiler))
                && !is_free_call
            {
                callee
                    .get_first_child(compiler)
                    .unwrap()
                    .clone_tree(compiler)
            } else {
                self.ast_factory.create_null(compiler)
            };
            let apply = self
                .ast_factory
                .create_get_prop_with_unknown_type(compiler, callee, "apply");
            self.ast_factory.create_call(
                compiler,
                apply,
                AstFactory::type_node(spread_parent),
                &[context, joined_groups],
            )
        };

        let color = spread_parent.get_color(t);
        call_to_apply.set_color(t, color);
        call_to_apply.srcref_tree_if_missing(t, spread_parent);
        spread_parent.replace_with(t, call_to_apply);
        t.get_compiler()
            .report_change_to_enclosing_scope(call_to_apply);
    }

    // port: Es6RewriteSpreadExpressions#isSpreadOfArguments
    fn is_spread_of_arguments(t: &NodeTraversal<'_>, n: NodeId) -> bool {
        n.is_spread(t) && n.get_only_child(t).matches_name(t, "arguments")
    }

    /// Processes new calls containing spreads.
    ///
    /// Example:
    ///
    /// ```text
    /// new F(...args) =>
    ///     new Function.prototype.bind.apply(F, [].concat($jscomp.arrayFromIterable(args)))
    /// ```
    // port: Es6RewriteSpreadExpressions#visitNewWithSpread
    fn visit_new_with_spread(&self, compiler: &mut AbstractCompiler, spread_parent: NodeId) {
        check_argument!(spread_parent.is_new(compiler));

        // Must remove callee before extracting argument groups.
        let callee = spread_parent.remove_first_child(compiler).unwrap();
        let mut groups = self.extract_spread_groups(compiler, spread_parent);

        // We need to generate
        // `new (Function.prototype.bind.apply(callee, [null].concat(other, args))();`.
        // `null` stands in for the 'this' arg to the contructor.
        let base_array_lit = if groups[0].is_array_lit(compiler) {
            groups.remove(0)
        } else {
            self.ast_factory.create_arraylit(compiler, &[])
        };
        let null = self.ast_factory.create_null(compiler);
        base_array_lit.add_child_to_front(compiler, null);
        let joined_groups = if groups.is_empty() {
            base_array_lit
        } else {
            let concat = self.ast_factory.create_get_prop(
                compiler,
                base_array_lit,
                "concat",
                CONCAT_FN_TYPE.clone(),
            );
            self.ast_factory
                .create_call(compiler, concat, ARRAY_TYPE.clone(), &groups)
        };

        if FeatureSet::ES3.contains(compiler.get_options().get_output_feature_set()) {
            // TODO(tbreisacher): Support this in ES3 too by not relying on Function.bind.
            TranspilationUtil::cannot_convert(
                compiler,
                spread_parent,
                "\"...\" passed to a constructor (consider using --language_out=ES5)",
            );
        }

        // Function.prototype.bind =>
        //      function(this:function(new:[spreadParent], ...?), ...?):function(new:[spreadParent])
        // Function.prototype.bind.apply =>
        //      function(function(new:[spreadParent], ...?), !Array<?>):function(new:[spreadParent])
        let function_name =
            self.ast_factory
                .create_name_in_scope(compiler, Some(&self.namespace), "Function");
        let prototype = self
            .ast_factory
            .create_prototype_access(compiler, function_name);
        let bind = self
            .ast_factory
            .create_get_prop_with_unknown_type(compiler, prototype, "bind");
        let bind_apply = self
            .ast_factory
            .create_get_prop_with_unknown_type(compiler, bind, "apply");
        let call = self.ast_factory.create_call_with_unknown_type(
            compiler,
            bind_apply,
            &[
                callee,
                joined_groups, /* function(new:[spreadParent]) */
            ],
        );
        let color = spread_parent.get_color(compiler);
        let result = IR::new_node(compiler, call, &[]).set_color(compiler, color);

        result.srcref_tree_if_missing(compiler, spread_parent);
        spread_parent.replace_with(compiler, result);
        compiler.report_change_to_enclosing_scope(result);
    }
}

impl CompilerPass for Es6RewriteSpreadExpressions {
    // port: Es6RewriteSpreadExpressions#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        TranspilationPasses::process_transpile(compiler, root, *TRANSPILED_FEATURES, &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            *TRANSPILED_FEATURES,
        );
    }
}

impl Callback for Es6RewriteSpreadExpressions {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RewriteSpreadExpressions#visit
    fn visit(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        current: NodeId,
        _parent: Option<NodeId>,
    ) {
        let token = current.get_token(traversal);
        if token == Token::ARRAYLIT || token == Token::NEW || token == Token::CALL {
            let mut child = current.get_first_child(traversal);
            while let Some(c) = child {
                if c.is_spread(traversal) {
                    self.visit_array_lit_or_call_with_spread(traversal, current);
                    break;
                }
                child = c.get_next(traversal);
            }
        }
    }
}
