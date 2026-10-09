/*
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
//   src/com/google/javascript/jscomp/Es7RewriteExponentialOperator.java.

//! Port of `Es7RewriteExponentialOperator.java`.
use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    colors::standard_colors,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{check_argument, check_not_null, check_state, node::NodeId, token::Token};

// port: Es7RewriteExponentialOperator#TRANSPILE_EXPONENT_USING_BIGINT
pub static TRANSPILE_EXPONENT_USING_BIGINT: DiagnosticType = DiagnosticType::error(
    "JSC_TRANSPILE_EXPONENT_USING_BIGINT",
    "Cannot transpile `**` operator applied to BigInt operands.",
);

const TEMP_VAR_NAME_PREFIX: &str = "$jscomp$exp$assign$tmp";
const TEMP_INDEX_VAR_NAME_PREFIX: &str = "$jscomp$exp$assign$tmpindex";

/// Replaces the ES7 `**` and `**=` operators to calls to `Math.pow`.
///
/// Java keeps the compiler and `compiler.getUniqueIdSupplier()`; here both are reached through the
/// compiler argument or the traversal (DESIGN §6).
pub struct Es7RewriteExponentialOperator {
    ast_factory: AstFactory,
    // This node should only ever be cloned, not directly inserted.
    // (Java: `Suppliers.memoize(this::createMathPowCall)`.)
    math_pow_call: Option<NodeId>,
}

impl Es7RewriteExponentialOperator {
    // port: Es7RewriteExponentialOperator#Es7RewriteExponentialOperator
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            math_pow_call: None,
        }
    }

    /// `mathPowCall.get()`: the memoized supplier.
    fn math_pow_call_get(&mut self, compiler: &mut AbstractCompiler) -> NodeId {
        if self.math_pow_call.is_none() {
            self.math_pow_call = Some(self.create_math_pow_call(compiler));
        }
        self.math_pow_call.unwrap()
    }

    // port: Es7RewriteExponentialOperator#visitExponentiationOperator
    fn visit_exponentiation_operator(&mut self, compiler: &mut AbstractCompiler, operator: NodeId) {
        let call_clone = self.math_pow_call_get(compiler).clone_tree(compiler);
        let base = check_not_null!(operator.remove_first_child(compiler));
        call_clone.add_child_to_back(compiler, base); // Base argument.
        let exponent = check_not_null!(operator.remove_first_child(compiler));
        call_clone.add_child_to_back(compiler, exponent); // Exponent argument.

        call_clone.srcref_tree_if_missing(compiler, operator);
        operator.replace_with(compiler, call_clone);

        compiler.report_change_to_enclosing_scope(call_clone);
    }

    // port: Es7RewriteExponentialOperator#visitExponentiationAssignmentOperator
    fn visit_exponentiation_assignment_operator(
        &mut self,
        t: &mut NodeTraversal<'_>,
        operator: NodeId,
    ) {
        let enclosing_statement = check_not_null!(NodeUtil::get_enclosing_statement(t, operator));

        let left = check_not_null!(operator.remove_first_child(t));

        let replacement = if left.is_name(t) {
            self.handle_lhs_name(t.get_compiler(), operator, left)
        } else {
            check_state!(
                left.is_get_prop(t) || left.is_get_elem(t),
                "%s",
                left.to_string(t)
            );
            self.handle_lhs_property_reference(t, operator, left, enclosing_statement)
        };

        operator.replace_with(t, replacement);

        t.get_compiler()
            .report_change_to_enclosing_scope(enclosing_statement);
    }

    // port: Es7RewriteExponentialOperator#handleLHSName
    fn handle_lhs_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        operator: NodeId,
        left: NodeId,
    ) -> NodeId {
        // handle name case
        // e.g. convert `name **= value` to
        // `name = Math.pow(name, value)`
        let call_clone = self.math_pow_call_get(compiler).clone_tree(compiler);
        let base = left.clone_tree(compiler);
        call_clone.add_child_to_back(compiler, base); // Base argument.
        let exponent = check_not_null!(operator.remove_first_child(compiler));
        call_clone.add_child_to_back(compiler, exponent); // Exponent argument.

        self.ast_factory
            .create_assign(compiler, left, call_clone)
            .srcref_tree_if_missing(compiler, operator)
    }

    // port: Es7RewriteExponentialOperator#handleLHSPropertyReference
    fn handle_lhs_property_reference(
        &mut self,
        t: &mut NodeTraversal<'_>,
        operator: NodeId,
        left: NodeId,
        enclosing_statement: NodeId,
    ) -> NodeId {
        // handle getprop case and getelem case
        let input = check_not_null!(t.get_input().cloned());
        let unique_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        let temp_var_name = format!("{TEMP_VAR_NAME_PREFIX}{unique_id}");

        let new_lhs;
        let temp_prop_or_elem;

        let object_node = check_not_null!(left.remove_first_child(t));

        let r#let = self
            .ast_factory
            .create_single_let_name_declaration(t.get_compiler(), temp_var_name.as_str())
            .srcref_tree(t, operator);
        r#let.insert_before(t, enclosing_statement);

        let temp_name = self
            .ast_factory
            .create_name(
                t.get_compiler(),
                temp_var_name.as_str(),
                AstFactory::type_node(object_node),
            )
            .srcref(t, object_node);
        let assign_temp = self
            .ast_factory
            .create_assign(t.get_compiler(), temp_name, object_node)
            .srcref(t, object_node); // (tmp = someExpression)

        if left.is_get_prop(t) {
            // handle getprop case
            // e.g. convert `(someExpression).property **= value` to
            // let tmp;
            // (tmp = someExpression).property = Math.pow(tmp.property, value);
            let property_name = left.get_string(t);

            let temp_name_clone = temp_name.clone_node(t);
            temp_prop_or_elem = self
                .ast_factory
                .create_get_prop(
                    t.get_compiler(),
                    temp_name_clone,
                    property_name.clone(),
                    AstFactory::type_node(left),
                )
                .srcref(t, left); // (tmp.property)
            new_lhs = self
                .ast_factory
                .create_get_prop(
                    t.get_compiler(),
                    assign_temp,
                    property_name,
                    AstFactory::type_node(left),
                )
                .srcref(t, left); // ((tmp = someExpression).property)
        } else {
            // handle getelem case
            // e.g. convert `someExpression[indexExpression] **= value` to
            // let tmp;
            // let tmpIndex;
            // `(tmp = someExpression)[(tmpIndex = indexExpression)] = Math.pow(tmp[tmpIndex], value);`
            check_state!(left.is_get_elem(t), "%s", left.to_string(t));
            let temp_index_var_name = format!("{TEMP_INDEX_VAR_NAME_PREFIX}{unique_id}");

            let index_expr_node = left.get_last_child(t).unwrap().detach(t);

            let let_index = self
                .ast_factory
                .create_single_let_name_declaration(t.get_compiler(), temp_index_var_name.as_str())
                .srcref_tree(t, operator);
            let_index.insert_before(t, enclosing_statement);

            let temp_index_name = self
                .ast_factory
                .create_name(
                    t.get_compiler(),
                    temp_index_var_name.as_str(),
                    AstFactory::type_node(index_expr_node),
                )
                .srcref(t, index_expr_node); // tmpIndex

            let assign_temp_index = self
                .ast_factory
                .create_assign(t.get_compiler(), temp_index_name, index_expr_node)
                .srcref(t, index_expr_node); // [tmpIndex = indexExpression]

            let temp_name_clone = temp_name.clone_node(t);
            let temp_index_name_clone = temp_index_name.clone_node(t);
            temp_prop_or_elem = self
                .ast_factory
                .create_get_elem(t.get_compiler(), temp_name_clone, temp_index_name_clone)
                .copy_type_from(t, left)
                .srcref(t, left); // (tmp[tmpIndex])

            new_lhs = self
                .ast_factory
                .create_get_elem(t.get_compiler(), assign_temp, assign_temp_index)
                .copy_type_from(t, left)
                .srcref(t, left); // (tmp = someExpression)[tmpIndex = indexExpression]
        }
        let call_clone = self.math_pow_call_get(t.get_compiler()).clone_tree(t);
        let base = temp_prop_or_elem.clone_tree(t);
        call_clone.add_child_to_back(t, base); // Base argument.
        let exponent = check_not_null!(operator.remove_first_child(t));
        call_clone.add_child_to_back(t, exponent); // Exponent argument.

        let current_script = check_not_null!(t.get_current_script());
        NodeUtil::add_feature_to_script(
            t.get_compiler(),
            current_script,
            Feature::LET_DECLARATIONS,
        );

        self.ast_factory
            .create_assign(t.get_compiler(), new_lhs, call_clone)
            .srcref_tree_if_missing(t, operator)
    }

    // port: Es7RewriteExponentialOperator#createMathPowCall
    fn create_math_pow_call(&self, compiler: &mut AbstractCompiler) -> NodeId {
        let namespace = TranspilationNamespace::get(compiler);
        let callee = self
            .ast_factory
            .create_qname(compiler, &namespace, "Math.pow");
        self.ast_factory.create_call(
            compiler,
            callee,
            AstFactory::type_(standard_colors::NUMBER.clone()),
            &[],
        )
    }

    // Report an error if the `**` getting transpiled to `Math.pow()`is of BIGINT type
    // port: Es7RewriteExponentialOperator#checkOperatorType
    fn check_operator_type(&self, compiler: &mut AbstractCompiler, operator: NodeId) -> bool {
        check_argument!(
            operator.is_exponent(compiler) || operator.is_assign_exponent(compiler),
            "%s",
            operator.to_string(compiler)
        );
        if compiler.has_type_checking_run()
            && operator.get_color(compiler).expect("NullPointerException")
                == *standard_colors::BIGINT
        {
            let error = JSError::make(compiler, operator, &TRANSPILE_EXPONENT_USING_BIGINT, &[]);
            compiler.report(error);
            return false;
        }
        true
    }
}

impl CompilerPass for Es7RewriteExponentialOperator {
    // port: Es7RewriteExponentialOperator#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        TranspilationPasses::maybe_mark_feature_as_transpiled_away(
            compiler,
            root,
            Feature::EXPONENT_OP,
        );
    }
}

impl Callback for Es7RewriteExponentialOperator {
    // port: Es7RewriteExponentialOperator#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            let script_features = NodeUtil::get_feature_set_of_script(t, n);
            // Runs only if features exist in the script
            return script_features.is_none()
                || script_features.unwrap().contains(Feature::EXPONENT_OP);
        }
        true
    }

    // port: Es7RewriteExponentialOperator#visit
    #[allow(clippy::collapsible_match)] // Java's switch arms with nested ifs.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::EXPONENT => {
                if self.check_operator_type(t.get_compiler(), n) {
                    self.visit_exponentiation_operator(t.get_compiler(), n);
                }
            }
            Token::ASSIGN_EXPONENT => {
                if self.check_operator_type(t.get_compiler(), n) {
                    self.visit_exponentiation_assignment_operator(t, n);
                }
            }
            _ => {}
        }
    }
}
