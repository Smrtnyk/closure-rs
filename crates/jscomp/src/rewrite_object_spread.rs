/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteObjectSpread.java.

//! Port of `RewriteObjectSpread.java`.
use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{check_argument, check_not_null, node::NodeId, token::Token};

// port: RewriteObjectSpread#transpiledFeatures
fn transpiled_features() -> FeatureSet {
    FeatureSet::BARE_MINIMUM.with(Feature::OBJECT_LITERALS_WITH_SPREAD)
}

/// Converts object spread to valid ES2017 code.
///
/// Currently this class converts Object spread properties as documented in tc39.
/// https://github.com/tc39/proposal-object-rest-spread. For example:
///
/// `const bar = {a: 1, ...foo};`
///
/// Note that object rest is handled by `Es6RewriteDestructuring`.
pub struct RewriteObjectSpread {
    ast_factory: AstFactory,
    namespace: TranspilationNamespace,
}

impl RewriteObjectSpread {
    // port: RewriteObjectSpread#RewriteObjectSpread
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            namespace: TranspilationNamespace::get(compiler),
        }
    }

    // port: RewriteObjectSpread#visitObject
    fn visit_object(&self, compiler: &mut AbstractCompiler, obj: NodeId) {
        let mut child = obj.get_first_child(compiler);
        while let Some(c) = child {
            if c.is_spread(compiler) {
                self.visit_object_with_spread(compiler, obj);
                return;
            }
            child = c.get_next(compiler);
        }
    }

    /// Convert '{first: b, c, ...spread, d: e, last}' to:
    ///
    /// Object.assign({}, {first:b, c}, spread, {d:e, last});
    // port: RewriteObjectSpread#visitObjectWithSpread
    fn visit_object_with_spread(&self, compiler: &mut AbstractCompiler, obj: NodeId) {
        check_argument!(obj.is_object_lit(compiler));

        // Add an empty target object literal so changes made by Object.assign will not affect any
        // other variables.
        let target = self.ast_factory.create_object_lit(compiler, &[]);
        let result = self.ast_factory.create_object_dot_assign_call(
            compiler,
            &self.namespace,
            AstFactory::type_node(obj),
            &[target],
        );

        // An indicator whether the current last thing in the param list is an object literal to
        // which properties may be added.  Initialized to null since nothing should be added to the
        // empty object literal in first position of the param list.
        let mut trailing_object_literal: Option<NodeId> = None;

        let mut child = obj.get_first_child(compiler);
        while let Some(c) = child {
            let next = c.get_next(compiler);
            if c.is_spread(compiler) {
                // Add the object directly to the param list.
                let spreaded = check_not_null!(c.remove_first_child(compiler));
                result.add_child_to_back(compiler, spreaded);

                // Properties should not be added to the trailing object.
                trailing_object_literal = None;
            } else {
                let trailing = match trailing_object_literal {
                    Some(trailing) => trailing,
                    None => {
                        // Add a new object to which properties may be added.
                        let trailing = self.ast_factory.create_object_lit(compiler, &[]);
                        result.add_child_to_back(compiler, trailing);
                        trailing_object_literal = Some(trailing);
                        trailing
                    }
                };
                // Add the property to the object literal.
                let detached = c.detach(compiler);
                trailing.add_child_to_back(compiler, detached);
            }
            child = next;
        }

        result.srcref_tree_if_missing(compiler, obj);
        obj.replace_with(compiler, result);
        compiler.report_change_to_enclosing_scope(result);
    }
}

impl CompilerPass for RewriteObjectSpread {
    // port: RewriteObjectSpread#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        TranspilationPasses::process_transpile(compiler, root, transpiled_features(), &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            transpiled_features(),
        );
    }
}

impl Callback for RewriteObjectSpread {
    // port: RewriteObjectSpread#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RewriteObjectSpread#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.get_token(t) == Token::OBJECTLIT {
            self.visit_object(t.get_compiler(), n);
        }
    }
}
