/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/SubstituteEs6Syntax.java.

//! Port of SubstituteEs6Syntax.java: an optimization that does peephole optimizations of ES6 code.
#![allow(clippy::collapsible_match)] // Retain Java control flow.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{check_argument, ir::IR, node::NodeId, token::Token};

/// An optimization that does peephole optimizations of ES6 code.
pub struct SubstituteEs6Syntax {
    object_literal_shorthand_was_added: bool,
}

impl SubstituteEs6Syntax {
    // port: SubstituteEs6Syntax#SubstituteEs6Syntax
    pub fn new() -> Self {
        Self {
            object_literal_shorthand_was_added: false,
        }
    }

    // port: SubstituteEs6Syntax#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }

    /// If possible, replace functions of the form ()=>{ return x; } with ()=>x
    // port: SubstituteEs6Syntax#maybeSimplifyArrowFunctionBody
    fn maybe_simplify_arrow_function_body(
        compiler: &mut AbstractCompiler,
        arrow_function: NodeId,
        body: NodeId,
    ) {
        check_argument!(arrow_function.is_arrow_function(compiler));
        if !body.is_block(compiler)
            || !body.has_one_child(compiler)
            || !body.get_first_child(compiler).unwrap().is_return(compiler)
        {
            return;
        }
        let return_value = body
            .get_first_child(compiler)
            .unwrap()
            .remove_first_child(compiler);
        let replacement = match return_value {
            Some(return_value) => return_value,
            None => IR::name(compiler, "undefined"),
        };
        body.replace_with(compiler, replacement);
        compiler.report_change_to_enclosing_scope(replacement);
    }
}

impl Default for SubstituteEs6Syntax {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for SubstituteEs6Syntax {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        SubstituteEs6Syntax::process(self, compiler, externs, root);
    }
}

impl Callback for SubstituteEs6Syntax {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: SubstituteEs6Syntax#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::FUNCTION => {
                if n.is_arrow_function(t) {
                    let body = n.get_last_child(t).unwrap();
                    Self::maybe_simplify_arrow_function_body(t.get_compiler(), n, body);
                }
            }
            Token::STRING_KEY => {
                let first = n.get_first_child(t).unwrap();
                if first.is_name(t) && first.get_string(t) == n.get_string(t) {
                    n.set_shorthand_property(t, true);
                    self.object_literal_shorthand_was_added = true;
                }
            }
            Token::SCRIPT => {
                if self.object_literal_shorthand_was_added {
                    NodeUtil::add_feature_to_script(
                        t.get_compiler(),
                        n,
                        Feature::SHORTHAND_OBJECT_PROPERTIES,
                    );
                    self.object_literal_shorthand_was_added = false; // false for the next script
                }
            }
            _ => {}
        }
    }
}
