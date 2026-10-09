/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteNullishCoalesceOperator.java.

//! Port of `RewriteNullishCoalesceOperator.java`.
use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{check_not_null, node::NodeId};

const TEMP_VAR_NAME_PREFIX: &str = "$jscomp$nullish$tmp";

/// Replaces the ES2020 `??` operator with conditional (?:).
///
/// Java keeps the compiler and `compiler.getUniqueIdSupplier()`; here both are reached through the
/// compiler argument or the traversal (DESIGN §6).
pub struct RewriteNullishCoalesceOperator {
    ast_factory: AstFactory,
}

impl RewriteNullishCoalesceOperator {
    // port: RewriteNullishCoalesceOperator#RewriteNullishCoalesceOperator
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
        }
    }

    // port: RewriteNullishCoalesceOperator#visitNullishCoalesce
    fn visit_nullish_coalesce(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        // a() ?? b()
        // let temp;
        // ((temp = a()) != null) ? temp : b()
        let input = check_not_null!(t.get_input().cloned());
        let unique_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        let temp_var_name = format!("{TEMP_VAR_NAME_PREFIX}{unique_id}");
        let enclosing_statement = check_not_null!(NodeUtil::get_enclosing_statement(t, n));

        let left = check_not_null!(n.remove_first_child(t));
        let right = n.get_last_child(t).unwrap().detach(t);

        let r#let = self
            .ast_factory
            .create_single_let_name_declaration(t.get_compiler(), temp_var_name.as_str());
        let assign_name = self.ast_factory.create_name(
            t.get_compiler(),
            temp_var_name.as_str(),
            AstFactory::type_node(left),
        );
        let assign = self
            .ast_factory
            .create_assign(t.get_compiler(), assign_name, left);
        let null = self.ast_factory.create_null(t.get_compiler());
        let ne = self.ast_factory.create_ne(t.get_compiler(), assign, null);
        let hook_name = self.ast_factory.create_name(
            t.get_compiler(),
            temp_var_name.as_str(),
            AstFactory::type_node(left),
        );
        let hook = self
            .ast_factory
            .create_hook(t.get_compiler(), ne, hook_name, right);

        r#let.srcref_tree_if_missing(t, left);
        assign_name.srcref_tree_if_missing(t, left);
        assign.srcref_tree_if_missing(t, left);
        ne.srcref_tree_if_missing(t, left);
        hook_name.srcref_tree_if_missing(t, left);

        r#let.insert_before(t, enclosing_statement);
        n.replace_with(t, hook);

        let current_script = check_not_null!(t.get_current_script());
        NodeUtil::add_feature_to_script(
            t.get_compiler(),
            current_script,
            Feature::LET_DECLARATIONS,
        );
        t.get_compiler().report_change_to_enclosing_scope(hook);
    }
}

impl CompilerPass for RewriteNullishCoalesceOperator {
    // port: RewriteNullishCoalesceOperator#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        TranspilationPasses::maybe_mark_feature_as_transpiled_away(
            compiler,
            root,
            Feature::NULL_COALESCE_OP,
        );
    }
}

impl Callback for RewriteNullishCoalesceOperator {
    // port: RewriteNullishCoalesceOperator#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            let script_features = NodeUtil::get_feature_set_of_script(t, n);
            return script_features.is_none()
                || script_features.unwrap().contains(Feature::NULL_COALESCE_OP);
        }
        true
    }

    // port: RewriteNullishCoalesceOperator#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_nullish_coalesce(t) {
            self.visit_nullish_coalesce(t, n);
            t.report_code_change();
        }
    }
}
