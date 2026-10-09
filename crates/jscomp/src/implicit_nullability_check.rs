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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ImplicitNullabilityCheck.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/NodeUtil.java.

//! Port of ImplicitNullabilityCheck.java: warn about types in JSDoc that are implicitly nullable.
use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_jstype::prelude::*;
use closure_rhino::check_argument;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;

// port: ImplicitNullabilityCheck#IMPLICITLY_NULLABLE_JSDOC
pub static IMPLICITLY_NULLABLE_JSDOC: DiagnosticType = DiagnosticType::disabled(
    "JSC_IMPLICITLY_NULLABLE_JSDOC",
    "Name {0} in JSDoc is implicitly nullable, and is discouraged by the style guide.\nPlease add a '!' to make it non-nullable, or a '?' to make it explicitly nullable.",
);

// port: ImplicitNullabilityCheck#IMPLICITLY_NONNULL_JSDOC
pub static IMPLICITLY_NONNULL_JSDOC: DiagnosticType = DiagnosticType::disabled(
    "JSC_IMPLICITLY_NONNULL_JSDOC",
    "Name {0} in JSDoc is implicitly non-null, and is discouraged by the style guide.\nPlease add a '!' to make it explicit.",
);

// port: ImplicitNullabilityCheck#NULLABILITY_OMITTED_TYPES
static NULLABILITY_OMITTED_TYPES: [&str; 10] = [
    "*", //
    "?",
    "bigint",
    "boolean",
    "null",
    "number",
    "string",
    "symbol",
    "undefined",
    "void",
];

// port: ImplicitNullabilityCheck
pub struct ImplicitNullabilityCheck;

impl ImplicitNullabilityCheck {
    // port: ImplicitNullabilityCheck#ImplicitNullabilityCheck
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    /// Finds and returns all the JSDoc nodes inside the given JSDoc object whose nullability is
    /// not explict, using the NodeTraversal the necessary state (current scope, etc.)
    // port: ImplicitNullabilityCheck#findImplicitNullabilityResults
    pub fn find_implicit_nullability_results(
        info: Option<&JSDocInfo>,
        t: &mut NodeTraversal<'_>,
    ) -> Vec<Result> {
        let Some(info) = info else {
            return Vec::new();
        };

        let mut builder: Vec<Result> = Vec::new();
        for type_root in info.get_type_nodes() {
            // The visitor only reads the tree, so the pre-order walk lists the nodes and the
            // visitor body then runs on each of them in that order.
            let mut nodes: Vec<NodeId> = Vec::new();
            NodeUtil::visit_pre_order_with_predicate(
                t.get_compiler(),
                type_root,
                &mut |_: &mut Ast, node: NodeId| nodes.push(node),
                &|_, _| true,
            );
            for node in nodes {
                // port: ImplicitNullabilityCheck#findImplicitNullabilityResults (NodeUtil.Visitor#visit)
                if let Some(result) = Self::visit_type_node(t, node) {
                    builder.push(result);
                }
            }
        }
        builder
    }

    /// The body of the anonymous `NodeUtil.Visitor` in findImplicitNullabilityResults; `None` is
    /// its early `return`.
    fn visit_type_node(t: &mut NodeTraversal<'_>, node: NodeId) -> Option<Result> {
        if !node.is_string_lit(t) {
            return None;
        }
        let parent = node.get_parent(t);
        if let Some(parent) = parent {
            match parent.get_token(t) {
                Token::BANG
                | Token::QMARK
                | Token::THIS // The names inside function(this:Foo) and
                | Token::NEW // function(new:Bar) are already non-null.
                | Token::TYPEOF => {
                    // Names after 'typeof' don't have nullability.
                    return None;
                }
                Token::PIPE => {
                    // Inside a union
                    let gp = parent.get_parent(t);
                    if gp.is_some_and(|gp| gp.get_token(t) == Token::QMARK) {
                        return None; // Inside an explicitly nullable union
                    }
                    let mut child = parent.get_first_child(t);
                    while let Some(c) = child {
                        if (c.is_string_lit(t) && c.get_string_ref(t) == "null")
                            || c.get_token(t) == Token::QMARK
                        {
                            return None; // Inside a union that contains null or nullable type
                        }
                        child = c.get_next(t);
                    }
                }
                _ => {}
            }
        }
        let type_name = node.get_string(t);
        if NULLABILITY_OMITTED_TYPES
            .iter()
            .any(|omitted| type_name == *omitted)
        {
            return None;
        }
        let scope = t.get_scope();
        let compiler = t.get_compiler();
        let static_scope = crate::scope::as_static_scope(compiler, scope);
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let ast: &Ast = ast;
        let type_ = registry.get_type(ast, Some(&*static_scope), type_name.clone())?;
        let is_non_nullable_name =
            registry.is_non_nullable_name(ast, Some(&*static_scope), type_name)
                && !type_.is_nullable(registry, ast);
        let nullability = if is_non_nullable_name {
            Nullability::NONNULL
        } else {
            Nullability::NULLABLE
        };
        Some(Result::create(ast, node, nullability))
    }
}

impl CompilerPass for ImplicitNullabilityCheck {
    // port: ImplicitNullabilityCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse_roots(compiler, self, externs, root);
    }
}

impl Callback for ImplicitNullabilityCheck {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    /// Crawls the JSDoc of the given node to find any names in JSDoc that are implicitly null.
    // port: ImplicitNullabilityCheck#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _p: Option<NodeId>) {
        let info = n.get_jsdoc_info(t);
        for r in Self::find_implicit_nullability_results(info.as_deref(), t) {
            let string_node = r.get_node();
            let dt: &'static DiagnosticType = if r.get_nullability().is_nullable() {
                &IMPLICITLY_NULLABLE_JSDOC
            } else {
                &IMPLICITLY_NONNULL_JSDOC
            };
            let compiler = t.get_compiler();
            let name = string_node.get_string(compiler).to_string();
            compiler.report(JSError::make(compiler, string_node, dt, &[&name]));
        }
    }
}

/// Represents the types of implicit nullability errors caught by this pass: a) implicitly nonnull
/// (missing a "!"), and b) implicitly nullable (missing a "?").
// port: ImplicitNullabilityCheck.Nullability
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nullability {
    NONNULL,
    NULLABLE,
}

impl Nullability {
    // port: ImplicitNullabilityCheck.Nullability#isNullable
    pub fn is_nullable(self) -> bool {
        self == Nullability::NULLABLE
    }
}

/// Information to represent a single "implicit nullability result", including the JSDoc string
/// node that needs a "!" or "?" to be explicit, as well as an enum indicating which of the two
/// nullability cases were found ("!" or "?").
// port: ImplicitNullabilityCheck.Result
#[derive(Clone, Copy, Debug)]
pub struct Result {
    node: NodeId,
    nullability: Nullability,
}

impl Result {
    // port: ImplicitNullabilityCheck.Result#Result
    fn new(ast: &Ast, node: NodeId, nullability: Nullability) -> Self {
        check_argument!(node.is_string_lit(ast));
        Self { node, nullability }
    }

    // port: ImplicitNullabilityCheck.Result#create
    fn create(ast: &Ast, node: NodeId, nullability: Nullability) -> Self {
        Self::new(ast, node, nullability)
    }

    // port: ImplicitNullabilityCheck.Result#getNullability
    pub fn get_nullability(&self) -> Nullability {
        self.nullability
    }

    // port: ImplicitNullabilityCheck.Result#getNode
    pub fn get_node(&self) -> NodeId {
        self.node
    }
}
