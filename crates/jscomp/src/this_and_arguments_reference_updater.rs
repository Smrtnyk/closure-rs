/*
 * Copyright 2024 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ThisAndArgumentsReferenceUpdater.java.

//! Port of `ThisAndArgumentsReferenceUpdater.java`.
//!
//! Rewrites references to `this` and `arguments` in a single function to instead refer to local
//! variables.
//!
//! When systematically transpiling away all arrow functions, an instance is generated for each
//! arrow function in order of *decreasing* depth. This isn't too inefficient, because instances
//! don't traverse into non-arrow functions and all nested functions will already have been
//! "de-arrowed". This class is also used for one-off cases (e.g. instrumenting generators for
//! `AsyncContext`), in which case there may be nested arrow functions that need to be recursed
//! into.

use crate::{
    AbstractCompiler,
    ast_factory::{AstFactory, Type},
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{ir::IR, js_string::JsString, node::NodeId};

// The name of the vars that capture 'this' and 'arguments' for converting arrow functions. Note
// that these names can be reused (once per scope) because declarations in nested scopes will
// shadow one another, which results in the intended behaviour.
// port: ThisAndArgumentsReferenceUpdater#ARGUMENTS_VAR
const ARGUMENTS_VAR: &str = "$jscomp$arguments";
// port: ThisAndArgumentsReferenceUpdater#THIS_VAR
const THIS_VAR: &str = "$jscomp$this";

pub struct ThisAndArgumentsReferenceUpdater<'a> {
    context: &'a mut ThisAndArgumentsContext,
    ast_factory: &'a AstFactory,
}

impl<'a> ThisAndArgumentsReferenceUpdater<'a> {
    // port: ThisAndArgumentsReferenceUpdater#ThisAndArgumentsReferenceUpdater
    pub fn new(context: &'a mut ThisAndArgumentsContext, ast_factory: &'a AstFactory) -> Self {
        Self {
            context,
            ast_factory,
        }
    }
}

impl Callback for ThisAndArgumentsReferenceUpdater<'_> {
    // port: ThisAndArgumentsReferenceUpdater#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_this(t) {
            self.context
                .set_needs_this_var_with_type(AstFactory::type_node(n));

            let name = self
                .ast_factory
                .create_name(
                    t.get_compiler(),
                    format!("{}${}", THIS_VAR, self.context.unique_id),
                    self.context
                        .get_this_type()
                        .expect("NullPointerException: thisType"),
                )
                .srcref(t, n);
            name.make_non_indexable(t);
            if t.get_compiler()
                .get_options()
                .preserves_detailed_source_info()
            {
                name.set_original_name(t, Some(JsString::from("this")));
            }

            n.replace_with(t, name);
        } else if n.is_name(t) && n.get_string_ref(t) == "arguments" {
            self.context.set_needs_arguments_var();

            let name = self
                .ast_factory
                .create_name(
                    t.get_compiler(),
                    format!("{}${}", ARGUMENTS_VAR, self.context.unique_id),
                    AstFactory::type_node(n),
                )
                .srcref(t, n);
            if t.get_compiler()
                .get_options()
                .preserves_detailed_source_info()
            {
                name.set_original_name(t, Some(JsString::from("arguments")));
            }

            n.replace_with(t, name);
        }
    }

    // port: ThisAndArgumentsReferenceUpdater#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if n.is_member_field_def(t) {
            return false;
        }
        if let Some(parent) = parent
            && parent.is_computed_field_def(t)
            && Some(n) != parent.get_first_child(t)
        {
            return false;
        }
        !n.is_function(t) || n.is_arrow_function(t)
    }
}

/// Accumulates information about a scope in which `this` and `arguments` are consistent.
///
/// Instances are maintained in a DFS stack during traversal of `Es6RewriteArrowFunction`. They
/// can't be immutable because a context isn't fully defined by a single node (`super()` makes this
/// hard).
// port: ThisAndArgumentsReferenceUpdater.ThisAndArgumentsContext
pub struct ThisAndArgumentsContext {
    pub scope_body: NodeId,
    pub is_constructor: bool,
    /// Last statement in the body that refers to super().
    pub last_super_statement: Option<NodeId>,
    // An object of ThisAndArgumentsContext exists for each Script and each Function. Store a
    // uniqueId string based on the script node's filename's hash. This gets used to generate a
    // unique name when rewriting `this` to `$jscomp$this`.
    pub unique_id: String,

    pub needs_this_var: bool,
    pub this_type: Option<Type>,

    pub needs_arguments_var: bool,
}

impl ThisAndArgumentsContext {
    // port: ThisAndArgumentsReferenceUpdater.ThisAndArgumentsContext#ThisAndArgumentsContext
    pub fn new(scope_body: NodeId, is_constructor: bool, unique_id: String) -> Self {
        Self {
            scope_body,
            is_constructor,
            last_super_statement: None,
            unique_id,
            needs_this_var: false,
            this_type: None,
            needs_arguments_var: false,
        }
    }

    // port: ThisAndArgumentsReferenceUpdater.ThisAndArgumentsContext#getThisType
    pub fn get_this_type(&self) -> Option<Type> {
        self.this_type.clone()
    }

    // port: ThisAndArgumentsReferenceUpdater.ThisAndArgumentsContext#setNeedsThisVarWithType
    pub fn set_needs_this_var_with_type(&mut self, type_: Type) -> &mut Self {
        self.this_type = Some(type_);
        self.needs_this_var = true;
        self
    }

    // port: ThisAndArgumentsReferenceUpdater.ThisAndArgumentsContext#setNeedsArgumentsVar
    pub fn set_needs_arguments_var(&mut self) -> &mut Self {
        self.needs_arguments_var = true;
        self
    }

    // port: ThisAndArgumentsReferenceUpdater.ThisAndArgumentsContext#addVarDeclarations
    pub fn add_var_declarations(
        &self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
        script: NodeId,
    ) {
        if self.needs_this_var {
            let name = ast_factory.create_name(
                compiler,
                format!("{}${}", THIS_VAR, self.unique_id),
                self.get_this_type()
                    .expect("NullPointerException: thisType"),
            );
            let this_type = self
                .get_this_type()
                .expect("NullPointerException: thisType");
            let this_node = ast_factory.create_this(compiler, this_type);
            let this_var = IR::const_node(compiler, name, this_node);
            NodeUtil::add_feature_to_script(compiler, script, Feature::CONST_DECLARATIONS);
            this_var.srcref_tree_if_missing(compiler, self.scope_body);
            Self::make_tree_non_indexable(compiler, this_var);

            match self.last_super_statement {
                None => Self::insert_var_declaration(compiler, this_var, self.scope_body),
                Some(last_super_statement) => {
                    // Not safe to reference `this` until after super() has been called.
                    // TODO(bradfordcsmith): Some complex cases still aren't covered, like
                    //     if (...) { super(); arrow function } else { super(); }
                    this_var.insert_after(compiler, last_super_statement);
                }
            }
            compiler.report_change_to_enclosing_scope(this_var);
        }

        if self.needs_arguments_var {
            let arguments_var = ast_factory.create_arguments_alias_declaration(
                compiler,
                format!("{}${}", ARGUMENTS_VAR, self.unique_id),
            );
            NodeUtil::add_feature_to_script(compiler, script, Feature::CONST_DECLARATIONS);

            Self::insert_var_declaration(compiler, arguments_var, self.scope_body);

            arguments_var.srcref_tree_if_missing(compiler, self.scope_body);
            compiler.report_change_to_enclosing_scope(arguments_var);
        }
    }

    // port: ThisAndArgumentsReferenceUpdater.ThisAndArgumentsContext#insertVarDeclaration
    fn insert_var_declaration(
        compiler: &mut AbstractCompiler,
        var_declaration: NodeId,
        scope_body: NodeId,
    ) {
        if scope_body
            .get_parent(compiler)
            .unwrap()
            .is_function(compiler)
        {
            // for functions, we must find the correct insertion point to preserve normalization
            let insert_before_point =
                NodeUtil::get_insertion_point_after_all_inner_function_declarations(
                    compiler, scope_body,
                );
            if let Some(insert_before_point) = insert_before_point {
                var_declaration.insert_before(compiler, insert_before_point);
            } else {
                // functionBody only contains hoisted function declarations
                scope_body.add_child_to_back(compiler, var_declaration);
            }
        } else {
            scope_body.add_child_to_front(compiler, var_declaration);
        }
    }

    // port: ThisAndArgumentsReferenceUpdater.ThisAndArgumentsContext#makeTreeNonIndexable
    fn make_tree_non_indexable(compiler: &mut AbstractCompiler, n: NodeId) {
        n.make_non_indexable(compiler);
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            Self::make_tree_non_indexable(compiler, c);
            child = c.get_next(compiler);
        }
    }
}
