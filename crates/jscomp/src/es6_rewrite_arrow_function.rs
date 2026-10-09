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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/Es6RewriteArrowFunction.java.

//! Port of `Es6RewriteArrowFunction.java`: converts ES6 arrow functions to standard anonymous ES3
//! functions.

use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    compiler_input::CompilerInput,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    this_and_arguments_reference_updater::{
        ThisAndArgumentsContext, ThisAndArgumentsReferenceUpdater,
    },
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{check_not_null, check_state, node::NodeId, token::Token};
use std::sync::LazyLock;

// port: Es6RewriteArrowFunction#transpiledFeatures
static TRANSPILED_FEATURES: LazyLock<FeatureSet> =
    LazyLock::new(|| FeatureSet::BARE_MINIMUM.with(Feature::ARROW_FUNCTIONS));

pub struct Es6RewriteArrowFunction {
    ast_factory: AstFactory,
    /// Java's `Deque` used as a stack: `push`/`peek`/`pop` act on the last element here.
    context_stack: Vec<ThisAndArgumentsContext>,
}

impl Es6RewriteArrowFunction {
    // port: Es6RewriteArrowFunction#Es6RewriteArrowFunction
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            context_stack: Vec::new(),
        }
    }

    /// @return The statement Node that is a child of block and contains n.
    // port: Es6RewriteArrowFunction#getEnclosingStatement
    fn get_enclosing_statement(t: &NodeTraversal<'_>, mut n: NodeId, block: NodeId) -> NodeId {
        while check_not_null!(n.get_parent(t)) != block {
            n = check_not_null!(NodeUtil::get_enclosing_statement(
                t,
                n.get_parent(t).unwrap()
            ));
        }
        n
    }

    // port: Es6RewriteArrowFunction#visitArrowFunction
    fn visit_arrow_function(
        ast_factory: &AstFactory,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        context: &mut ThisAndArgumentsContext,
    ) {
        n.set_is_arrow_function(t, false);

        let body = n.get_last_child(t).unwrap();
        check_state!(
            body.is_block(t),
            "Arrow function body must be a block after normalization"
        );

        let mut updater = ThisAndArgumentsReferenceUpdater::new(context, ast_factory);
        NodeTraversal::traverse(t.get_compiler(), body, &mut updater);

        t.report_code_change();
    }

    // port: Es6RewriteArrowFunction#contextForFunction
    fn context_for_function(
        compiler: &mut AbstractCompiler,
        function_node: NodeId,
        input: Option<CompilerInput>,
    ) -> ThisAndArgumentsContext {
        let scope_body = function_node.get_last_child(compiler).unwrap();
        let is_constructor = NodeUtil::is_es6_constructor(compiler, function_node);
        ThisAndArgumentsContext::new(
            scope_body,
            is_constructor,
            compiler
                .get_unique_id_supplier()
                .get_unique_id(&input.expect("NullPointerException: input")),
        )
    }

    // port: Es6RewriteArrowFunction#contextForScript
    fn context_for_script(
        compiler: &mut AbstractCompiler,
        script_node: NodeId,
        input: Option<CompilerInput>,
    ) -> ThisAndArgumentsContext {
        ThisAndArgumentsContext::new(
            script_node,
            /* is_constructor= */ false,
            compiler
                .get_unique_id_supplier()
                .get_unique_id(&input.expect("NullPointerException: input")),
        )
    }
}

impl CompilerPass for Es6RewriteArrowFunction {
    // port: Es6RewriteArrowFunction#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        TranspilationPasses::process_transpile(compiler, root, *TRANSPILED_FEATURES, &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            *TRANSPILED_FEATURES,
        );
    }
}

impl Callback for Es6RewriteArrowFunction {
    // port: Es6RewriteArrowFunction#shouldTraverse
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::SCRIPT => {
                let input = t.get_input().cloned();
                let context = Self::context_for_script(t.get_compiler(), n, input);
                self.context_stack.push(context);
            }
            Token::FUNCTION => {
                if !n.is_arrow_function(t) {
                    let input = t.get_input().cloned();
                    let context = Self::context_for_function(t.get_compiler(), n, input);
                    self.context_stack.push(context);
                }
            }
            Token::SUPER => {
                let context = check_not_null!(self.context_stack.last_mut());
                let parent = parent.unwrap();
                // super(...) within a constructor.
                if context.is_constructor
                    && parent.is_call(t)
                    && parent.get_first_child(t) == Some(n)
                {
                    context.last_super_statement =
                        Some(Self::get_enclosing_statement(t, parent, context.scope_body));
                }
            }
            _ => {}
        }
        true
    }

    // port: Es6RewriteArrowFunction#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let ast_factory = &self.ast_factory;
        let context = self.context_stack.last_mut();

        if n.is_arrow_function(t) {
            Self::visit_arrow_function(ast_factory, t, n, check_not_null!(context));
        } else if let Some(context) = context
            && context.scope_body == n
        {
            let context = self.context_stack.pop().unwrap();
            let script = t.get_current_script().unwrap();
            context.add_var_declarations(t.get_compiler(), &self.ast_factory, script);
        }
    }
}
