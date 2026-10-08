/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SynthesizeExplicitConstructors.java.

//! Port of `SynthesizeExplicitConstructors.java`.
use crate::{
    AbstractCompiler, ast_factory::AstFactory, node_traversal::NodeTraversal, node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{check_argument, check_state, ir::IR, node::NodeId};

/// Adds explicit constructors to classes that lack them.
pub struct SynthesizeExplicitConstructors {
    ast_factory: AstFactory,
}

impl SynthesizeExplicitConstructors {
    // port: SynthesizeExplicitConstructors#SynthesizeExplicitConstructors
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
        }
    }

    /// Add a synthetic constructor to the given class if no constructor is present
    // port: SynthesizeExplicitConstructors#synthesizeClassConstructorIfMissing
    pub fn synthesize_class_constructor_if_missing(
        &self,
        t: &mut NodeTraversal<'_>,
        class_node: NodeId,
    ) {
        check_argument!(class_node.is_class(t));
        if NodeUtil::get_es6_class_constructor_member_function_def(t, class_node).is_none() {
            self.add_synthetic_constructor(t, class_node);
        }
    }

    // port: SynthesizeExplicitConstructors#addSyntheticConstructor
    #[allow(clippy::needless_late_init)] // Java declares memberDef before the branches.
    fn add_synthetic_constructor(&self, t: &mut NodeTraversal<'_>, class_node: NodeId) {
        let super_class = class_node.get_second_child(t).unwrap();
        let class_members = class_node.get_last_child(t).unwrap();
        let member_def;
        if super_class.is_empty(t) {
            let function = self
                .ast_factory
                .create_empty_function(t.get_compiler(), AstFactory::type_node(class_node));
            t.get_compiler().report_change_to_change_scope(function);
            member_def = self.ast_factory.create_member_function_def(
                t.get_compiler(),
                "constructor",
                function,
            );
        } else {
            check_state!(
                super_class.is_qualified_name(t),
                "Expected Es6NormalizeClasses to make all extends clauses into qualified names, found %s",
                super_class.to_string(t)
            );
            let body = IR::block(t);

            // If a class is defined in an externs file or as an interface, it's only a stub, not
            // an implementation that should be instantiated.
            // A call to super() shouldn't actually exist for these cases and is problematic to
            // transpile, so don't generate it.
            if !class_node.is_from_externs(t) && !Self::is_interface(t, class_node) {
                // Generate required call to super()
                // `super(...arguments);`
                // Note that transpilation of spread args must occur after this pass for this to
                // work.
                let super_node = self
                    .ast_factory
                    .create_super(t.get_compiler(), AstFactory::type_node(super_class));
                let arguments = self
                    .ast_factory
                    .create_arguments_reference(t.get_compiler());
                let spread = IR::iter_spread(t, arguments);
                let call = self.ast_factory.create_constructor_call(
                    t.get_compiler(),
                    AstFactory::type_node(class_node), // returned type is the subclass
                    super_node,
                    &[spread],
                );
                let expr_result = IR::expr_result(t, call);
                body.add_child_to_front(t, expr_result);
                let script = t.get_current_script().unwrap();
                NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::SUPER);
                NodeUtil::add_feature_to_script(
                    t.get_compiler(),
                    script,
                    Feature::SPREAD_EXPRESSIONS,
                );
            }
            let param_list = IR::param_list(t, &[]);
            let constructor = self.ast_factory.create_function(
                t.get_compiler(),
                "",
                param_list,
                body,
                AstFactory::type_node(class_node),
            );
            member_def = self.ast_factory.create_member_function_def(
                t.get_compiler(),
                "constructor",
                constructor,
            );
        }
        member_def.srcref_tree_if_missing(t, class_node);
        member_def.make_non_indexable_recursive(t);
        class_members.add_child_to_front(t, member_def);
        let script = t.get_current_script().unwrap();
        NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::MEMBER_DECLARATIONS);
        // report newly created constructor
        let only_child = member_def.get_only_child(t);
        t.get_compiler().report_change_to_change_scope(only_child);
        // report change to scope containing the class
        t.get_compiler()
            .report_change_to_enclosing_scope(member_def);
    }

    // port: SynthesizeExplicitConstructors#isInterface
    fn is_interface(ast: &closure_rhino::node::Ast, class_node: NodeId) -> bool {
        let class_js_doc_info = NodeUtil::get_best_jsdoc_info(ast, class_node);
        class_js_doc_info.is_some_and(|info| info.is_interface())
    }
}
