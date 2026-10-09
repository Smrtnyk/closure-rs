/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Es6ForOfConverter.java.

//! Port of `Es6ForOfConverter.java`.
use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    colors::standard_colors,
    compiler_pass::CompilerPass,
    default_name_generator::DefaultNameGenerator,
    name_generator::NameGenerator,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    node::{NodeId, Prop},
    token::Token,
};

// port: Es6ForOfConverter#transpiledFeatures
fn transpiled_features() -> FeatureSet {
    FeatureSet::BARE_MINIMUM.with(Feature::FOR_OF)
}

const ITER_BASE: &str = "$jscomp$iter$";

const ITER_RESULT: &str = "$jscomp$key$";

/// Converts ES6 "for of" loops to ES5.
///
/// Java keeps the compiler; here it is reached through the compiler argument or the traversal
/// (DESIGN §6).
pub struct Es6ForOfConverter {
    namer: DefaultNameGenerator,
    ast_factory: AstFactory,
    namespace: TranspilationNamespace,
}

impl Es6ForOfConverter {
    // port: Es6ForOfConverter#Es6ForOfConverter
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            namer: DefaultNameGenerator::new(),
            ast_factory: compiler.create_ast_factory(),
            namespace: TranspilationNamespace::get(compiler),
        }
    }

    // TODO(lharker): break up this method
    // port: Es6ForOfConverter#visitForOf
    fn visit_for_of(&mut self, t: &mut NodeTraversal<'_>, node: NodeId) {
        // `var v` or `let v` or v any valid lhs
        let variable = check_not_null!(node.remove_first_child(t));
        let iterable = check_not_null!(node.remove_first_child(t));
        let body = check_not_null!(node.remove_first_child(t));
        let var_jsdoc_info = variable.get_jsdoc_info(t);
        let input = check_not_null!(t.get_input().cloned());
        // `$jscomp$iter$0`
        let iter_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        let iter_name = self.ast_factory.create_name(
            t.get_compiler(),
            format!("{ITER_BASE}{iter_id}").as_str(),
            AstFactory::type_color_id(standard_colors::ITERATOR_ID),
        );
        iter_name.make_non_indexable(t);
        // `$jscomp$iter$0.next()`
        let iter_name_clone = iter_name.clone_tree(t);
        let get_next_prop = self.ast_factory.create_get_prop_with_unknown_type(
            t.get_compiler(),
            iter_name_clone,
            "next",
        );
        let get_next =
            self.ast_factory
                .create_call_with_unknown_type(t.get_compiler(), get_next_prop, &[]);
        // generate a unique iterator result name for every for-of loop getting rewritten to avoid
        // conflicts
        let result_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        let mut iterator_result_name = format!("{ITER_RESULT}{result_id}$");

        if NodeUtil::is_name_declaration(t, Some(variable)) {
            let first = check_not_null!(variable.get_first_child(t));
            iterator_result_name.push_str(&first.get_string(t).to_string());
        } else if variable.is_name(t) {
            iterator_result_name.push_str(&variable.get_string(t).to_string());
        } else {
            // give arbitrary lhs expressions an arbitrary name
            iterator_result_name.push_str(&self.namer.generate_next_name().to_string());
        }
        // `$jscomp$key$extraName`
        let iter_result = self
            .ast_factory
            .create_name_with_unknown_type(t.get_compiler(), iterator_result_name.as_str());
        iter_result.make_non_indexable(t);

        // `$jscomp.makeIterator(iterable)`
        let call_make_iterator = self
            .ast_factory
            .create_jscomp_make_iterator_call(t.get_compiler(), iterable, &self.namespace)
            .srcref_tree_if_missing(t, iterable);
        // `var $jscomp$iter$0 = $jscomp.makeIterator(iterable)`
        let iter_name_clone = iter_name.clone_tree(t);
        let init_iter = IR::var_with_value(t, iter_name_clone, call_make_iterator)
            .srcref_tree_if_missing(t, iterable);
        // var $jscomp$key$extraName = $jscomp$iter$0.next();
        let iter_result_clone = iter_result.clone_tree(t);
        let get_next_clone = get_next.clone_tree(t);
        let init_iter_result = IR::var_with_value(t, iter_result_clone, get_next_clone)
            .srcref_tree_if_missing(t, iterable);

        // !$jscomp$key$extraName.done
        let iter_result_clone = iter_result.clone_tree(t);
        let done = self.ast_factory.create_get_prop(
            t.get_compiler(),
            iter_result_clone,
            "done",
            AstFactory::type_(standard_colors::BOOLEAN.clone()),
        );
        let cond = self.ast_factory.create_not(t.get_compiler(), done);
        // $jscomp$key$extraName = $jscomp$iter$0.next()
        let iter_result_clone = iter_result.clone_tree(t);
        let get_next_clone = get_next.clone_tree(t);
        let incr =
            self.ast_factory
                .create_assign(t.get_compiler(), iter_result_clone, get_next_clone);

        let declaration_or_assign;
        if !NodeUtil::is_name_declaration(t, Some(variable)) {
            // e.g. `for(a.b of []) {}`
            let lhs = variable.clone_tree(t).set_jsdoc_info(t, None);
            let iter_result_clone = iter_result.clone_tree(t);
            let value = self.ast_factory.create_get_prop(
                t.get_compiler(),
                iter_result_clone,
                "value",
                AstFactory::type_node(variable),
            );
            let assign = self.ast_factory.create_assign(t.get_compiler(), lhs, value);
            assign.set_jsdoc_info(t, var_jsdoc_info);
            declaration_or_assign = IR::expr_result(t, assign);
        } else {
            // `for(let a of []) {}` or `for(const a of []) {}`
            let first = check_not_null!(variable.get_first_child(t));
            let type_ = AstFactory::type_node(first);
            let declaration_type = variable.get_token(t); // i.e. VAR, CONST, or LET.
            check_state!(
                declaration_type != Token::VAR,
                "var initializers must've gotten moved out of the loop during normalize"
            );
            let iter_result_clone = iter_result.clone_tree(t);
            let value = self.ast_factory.create_get_prop(
                t.get_compiler(),
                iter_result_clone,
                "value",
                type_,
            );
            let name = first.get_string(t);
            declaration_or_assign = self.ast_factory.create_single_name_declaration(
                t.get_compiler(),
                declaration_type,
                name,
                value,
            );
            if first.get_boolean_prop(t, Prop::IS_CONSTANT_NAME) {
                // if the original name was const, then the new name should be too
                // e.g. `for(let CID of []) {}` where `CID` was originally marked constant by coding
                // convention
                let decl_name = check_not_null!(declaration_or_assign.get_first_child(t));
                decl_name.put_boolean_prop(t, Prop::IS_CONSTANT_NAME, true);
            }
            declaration_or_assign.set_jsdoc_info(t, var_jsdoc_info);
        }
        let new_body = IR::block_with_children(t, &[declaration_or_assign, body]).srcref(t, body);
        let empty = self.ast_factory.create_empty(t.get_compiler());
        let new_for = IR::for_node(t, empty, cond, incr, new_body).srcref_tree_if_missing(t, node);

        // Build finally block:
        // (0, $jscomp.iteratorClose)($jscomp$iter$0, $jscomp$key$extraName);
        let iter_name_clone = iter_name.clone_tree(t);
        let iter_result_clone = iter_result.clone_tree(t);
        let call_iterator_close = self
            .ast_factory
            .create_jscomp_iterator_close_call(
                t.get_compiler(),
                iter_name_clone,
                iter_result_clone,
                &self.namespace,
            )
            .srcref_tree_if_missing(t, node);
        let call_stmt = self
            .ast_factory
            .expr_result(t.get_compiler(), call_iterator_close);
        let finally_block = self
            .ast_factory
            .create_block(t.get_compiler(), &[call_stmt]);

        // Check if the for loop has a parent that is a label i.e. `loop1: for(...of ...)`
        let mut label_names: Vec<NodeId> = Vec::new();
        let mut insertion_point = node;
        while let Some(parent) = insertion_point.get_parent(t)
            && parent.is_label(t)
        {
            insertion_point = parent;
            let label = check_not_null!(insertion_point.get_first_child(t));
            label_names.push(label.clone_node(t));
        }

        let mut inner_loop = new_for;
        for label_name in label_names {
            inner_loop = self
                .ast_factory
                .create_label(t.get_compiler(), label_name, inner_loop);
        }
        let try_block = self
            .ast_factory
            .create_block(t.get_compiler(), &[inner_loop]);
        let try_finally = self
            .ast_factory
            .create_try_finally(t.get_compiler(), try_block, finally_block)
            .srcref_tree_if_missing(t, node);

        insertion_point.replace_with(t, try_finally);

        init_iter.insert_before(t, try_finally);
        init_iter_result.insert_after(t, init_iter);
        t.get_compiler()
            .report_change_to_enclosing_scope(try_finally);
    }
}

impl CompilerPass for Es6ForOfConverter {
    // port: Es6ForOfConverter#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        TranspilationPasses::process_transpile(compiler, root, transpiled_features(), &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            transpiled_features(),
        );
    }
}

impl Callback for Es6ForOfConverter {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6ForOfConverter#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_for_of(t) {
            self.visit_for_of(t, n);
        }
    }
}
