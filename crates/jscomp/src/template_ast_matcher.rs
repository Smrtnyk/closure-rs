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
//   src/com/google/javascript/jscomp/TemplateAstMatcher.java.

//! Port of `TemplateAstMatcher.java`.
//!
//! A matcher that can take an arbitrary AST and use it as a template to find matches in another.
//! As this matcher potentially matches against every node in the AST it is tuned to avoid
//! generating GC garbage. It first checks the AST shape without types and then successful checks
//! the associated types.

use crate::{
    AbstractCompiler, node_util::NodeUtil, type_matching_strategy::TypeMatchingStrategy,
    typed_scope::TypedScope,
};
use closure_jstype::prelude::*;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

// Custom Token types for to use as placeholders in the template AST.
// port: TemplateAstMatcher#TEMPLATE_TYPE_PARAM
const TEMPLATE_TYPE_PARAM: Token = Token::PLACEHOLDER1;
// port: TemplateAstMatcher#TEMPLATE_LOCAL_NAME
const TEMPLATE_LOCAL_NAME: Token = Token::PLACEHOLDER2;
// port: TemplateAstMatcher#TEMPLATE_STRING_LITERAL
const TEMPLATE_STRING_LITERAL: Token = Token::PLACEHOLDER3;

// port: TemplateAstMatcher
pub struct TemplateAstMatcher {
    #[allow(dead_code)] // Java keeps it as a field; only the constructor reads it
    top_scope: Option<TypedScope>,

    /// The head of the Node list that should be used to start the matching process.
    template_start: Option<NodeId>,

    /// The params declared in the template (in order)
    template_params: Vec<JsString>,

    /// Record the first Node to match a template parameter, only valid for the last match if it
    /// was successful.
    param_node_matches: Vec<Option<NodeId>>,

    /// The locals declared in the template (in order)
    template_locals: Vec<JsString>,

    /// Record the first name to match a template local variable, only valid for the last match if
    /// it was successful.
    local_var_matches: Vec<Option<JsString>>,

    /// The names of all matched string literals, in order.
    ///
    /// This re-uses strings already present in the AST, which is faster and simpler than keeping
    /// an additional layer of indirection.
    string_literal_matches: IndexMap<JsString, NodeId>,

    /// Record whether the last successful was a loosely matched type, only valid for the last
    /// match if it was successful.
    is_loose_match: bool,

    /// The strategy to use when matching the `JSType` of nodes.
    type_matching_strategy: TypeMatchingStrategy,
}

impl TemplateAstMatcher {
    /// Constructs this matcher with a Function node that serves as the template to match all
    /// other nodes against. The body of the function will be used to match against.
    // port: TemplateAstMatcher#TemplateAstMatcher
    pub fn new(
        compiler: &mut AbstractCompiler,
        template_function_node: NodeId,
        type_matching_strategy: TypeMatchingStrategy,
    ) -> Self {
        check_state!(
            template_function_node.is_function(compiler),
            "Template node must be a function node. Received: %s",
            template_function_node.to_string(compiler)
        );

        // TopScope may be null if the template is used before type checking, which is useful if
        // just check code structure.
        let mut this = Self {
            top_scope: compiler.get_top_scope(),
            template_start: None,
            template_params: Vec::new(),
            param_node_matches: Vec::new(),
            template_locals: Vec::new(),
            local_var_matches: Vec::new(),
            string_literal_matches: IndexMap::<_, _>::default(),
            is_loose_match: false,
            type_matching_strategy,
        };
        this.template_start = this.init_template(compiler, template_function_node);
        this
    }

    /// @param n The node to check.
    /// @return Whether the node is matches the template.
    // port: TemplateAstMatcher#matches
    pub fn matches(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        if self.matches_template_shape(compiler, self.template_start, Some(n)) {
            if self.param_node_matches.is_empty() && self.local_var_matches.is_empty() {
                // If there are no parameters or locals to match against, this
                // has been a successful match and there is no reason to traverse
                // the AST again.
                return true;
            }
            self.reset();
            return self.matches_template(compiler, self.template_start, Some(n));
        }
        false
    }

    /// @return Whether the last match succeeded due to loose type information.
    // port: TemplateAstMatcher#isLooseMatch
    pub fn is_loose_match(&self) -> bool {
        self.is_loose_match
    }

    /// Returns a map from named template Nodes (such as parameters or local variables) to Nodes
    /// that were matches from the last matched template.
    // port: TemplateAstMatcher#getTemplateNodeToMatchMap
    pub fn get_template_node_to_match_map(
        &self,
        ast: &mut Ast,
    ) -> IndexMap<JsString, Option<NodeId>> {
        let mut map: IndexMap<JsString, Option<NodeId>> = self
            .string_literal_matches
            .iter()
            .map(|(name, node)| (name.clone(), Some(*node)))
            .collect();

        for i in 0..self.template_params.len() {
            let name = self.template_params[i].clone();
            map.insert(name, self.param_node_matches[i]);
        }

        for i in 0..self.template_locals.len() {
            let name = self.template_locals[i].clone();
            // IR.name(null) throws in Java.
            let local = self.local_var_matches[i]
                .clone()
                .expect("NullPointerException");
            map.insert(name, Some(IR::name(ast, local)));
        }

        map
    }

    /// Prepare an template AST to use when performing matches.
    ///
    /// @param templateFunctionNode The template declaration function to extract the template AST
    ///     from.
    /// @return The first node of the template AST sequence to use when matching.
    // port: TemplateAstMatcher#initTemplate
    fn init_template(
        &mut self,
        compiler: &mut AbstractCompiler,
        template_function_node: NodeId,
    ) -> Option<NodeId> {
        let prepped = template_function_node.clone_tree(compiler);
        self.prep_template_placeholders(compiler, prepped);

        let body = prepped.get_last_child(compiler).unwrap();
        let start_node = if body.has_one_child(compiler)
            && body
                .get_first_child(compiler)
                .unwrap()
                .is_expr_result(compiler)
        {
            // When matching an expression, don't require it to be a complete
            // statement.
            body.get_first_first_child(compiler)
        } else {
            body.get_first_child(compiler)
        };

        for _ in 0..self.template_locals.len() {
            // reserve space in the locals array.
            self.local_var_matches.push(None);
        }
        for _ in 0..self.template_params.len() {
            // reserve space in the params array.
            self.param_node_matches.push(None);
        }

        start_node
    }

    /// Build parameter and local information for the template and replace the references in the
    /// template 'fn' with placeholder nodes use to facility matching.
    // port: TemplateAstMatcher#prepTemplatePlaceholders
    fn prep_template_placeholders(&mut self, compiler: &mut AbstractCompiler, fn_: NodeId) {
        let mut param_types: IndexMap<JsString, TypeId> = IndexMap::<_, _>::default();

        // drop the function name so it isn't include in the name maps
        let fn_name_node = fn_.get_first_child(compiler).unwrap();
        let fn_name = fn_name_node.get_string(compiler);
        fn_name_node.set_string(compiler, "");

        // Build a list of parameter names and types.
        let template_parameters_node = fn_.get_second_child(compiler).unwrap();
        let info = NodeUtil::get_best_jsdoc_info(compiler, fn_);
        if template_parameters_node.has_children(compiler) {
            check_not_null!(
                info.as_ref(),
                "Missing JSDoc declaration for template function %s",
                fn_name
            );
        }
        let mut param_node = template_parameters_node.get_first_child(compiler);
        while let Some(p) = param_node {
            let name = p.get_string(compiler);
            let expression = info
                .as_ref()
                .expect("NullPointerException")
                .get_parameter_type(name.clone());
            let expression = check_not_null!(
                expression,
                "Missing JSDoc for parameter %s of template function %s",
                name,
                fn_name
            );
            let scope = self
                .top_scope
                .map(|scope| scope.as_static_typed_scope_arc(compiler));
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let type_ = registry.evaluate_type_expression(ast, &expression, scope);
            self.template_params.push(name.clone());
            param_types.insert(name, type_);
            param_node = p.get_next(compiler);
        }

        // Find references to string literals, local variables and parameters and replace them.
        self.traverse(compiler, fn_, &param_types);
    }

    /// The visitor of `prepTemplatePlaceholders` (Java's anonymous `Visitor`).
    // port: TemplateAstMatcher#prepTemplatePlaceholders (TemplateAstMatcher.Visitor#visit)
    fn visit(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        param_types: &IndexMap<JsString, TypeId>,
    ) {
        if n.is_name(compiler) {
            let parent = n.get_parent(compiler).unwrap();
            let name = n.get_string(compiler);
            if !name.is_empty() && parent.is_var(compiler) && !self.template_locals.contains(&name)
            {
                self.template_locals.push(n.get_string(compiler));
            }

            if let Some(index) = self.template_params.iter().position(|p| *p == name) {
                let type_ = param_types[&name];
                let is_string_literal = type_.is_string_value_type(compiler.get_type_registry())
                    && name.starts_with("string_literal");
                let replacement =
                    Self::create_template_parameter_node(compiler, index, type_, is_string_literal);
                Self::replace_node_in_place(compiler, n, replacement);
            } else if let Some(index) = self.template_locals.iter().position(|l| *l == name) {
                let replacement = Self::create_template_local_name_node(compiler, index);
                Self::replace_node_in_place(compiler, n, replacement);
            }
        }
    }

    // port: TemplateAstMatcher#replaceNodeInPlace
    fn replace_node_in_place(ast: &mut Ast, n: NodeId, replacement: NodeId) {
        if n.has_children(ast) {
            let children = n.remove_children(ast);
            replacement.add_children_to_front(ast, children);
        }
        n.replace_with(ast, replacement);
    }

    // port: TemplateAstMatcher#traverse
    fn traverse(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        param_types: &IndexMap<JsString, TypeId>,
    ) {
        let mut c = n.get_first_child(compiler);
        while let Some(child) = c {
            let next = child.get_next(compiler); // in case the child is remove, grab the next node now
            self.traverse(compiler, child, param_types);
            c = next;
        }
        self.visit(compiler, n, param_types);
    }

    // port: TemplateAstMatcher#reset
    fn reset(&mut self) {
        self.is_loose_match = false;
        self.local_var_matches.fill(None);
        for i in 0..self.param_node_matches.len() {
            self.param_node_matches[i] = None;
        }
    }

    // port: TemplateAstMatcher#isTemplateParameterNode
    fn is_template_parameter_node(ast: &Ast, n: NodeId) -> bool {
        n.get_token(ast) == TEMPLATE_TYPE_PARAM
    }

    /// Matches parameters (in the refasterJS template) whose names start with 'string_literal_'.
    // port: TemplateAstMatcher#isTemplateParameterStringLiteralNode
    fn is_template_parameter_string_literal_node(ast: &Ast, n: NodeId) -> bool {
        n.get_token(ast) == TEMPLATE_STRING_LITERAL
    }

    /// Creates a template parameter or string literal template node.
    // port: TemplateAstMatcher#createTemplateParameterNode
    fn create_template_parameter_node(
        ast: &mut Ast,
        index: usize,
        type_: TypeId,
        is_string_literal: bool,
    ) -> NodeId {
        let n = ast.new_number(index as f64);
        if is_string_literal {
            n.set_token(ast, TEMPLATE_STRING_LITERAL);
        } else {
            n.set_token(ast, TEMPLATE_TYPE_PARAM);
        }
        n.set_jstype(ast, Some(type_));
        n
    }

    // port: TemplateAstMatcher#isTemplateLocalNameNode
    fn is_template_local_name_node(ast: &Ast, n: NodeId) -> bool {
        n.get_token(ast) == TEMPLATE_LOCAL_NAME
    }

    // port: TemplateAstMatcher#createTemplateLocalNameNode
    fn create_template_local_name_node(ast: &mut Ast, index: usize) -> NodeId {
        let n = ast.new_number(index as f64);
        n.set_token(ast, TEMPLATE_LOCAL_NAME);
        n
    }

    /// Returns whether the template matches an AST structure node starting with node, taking into
    /// account the template parameters that were provided to this matcher. Here only the template
    /// shape is checked, template local declarations and parameters are checked later.
    // port: TemplateAstMatcher#matchesTemplateShape
    fn matches_template_shape(
        &self,
        ast: &Ast,
        template: Option<NodeId>,
        ast_node: Option<NodeId>,
    ) -> bool {
        let mut template = template;
        let mut ast_node = ast_node;
        while let Some(t) = template {
            let Some(a) = ast_node else {
                return false;
            };
            if !self.matches_node_shape(ast, t, Some(a)) {
                return false;
            }
            template = t.get_next(ast);
            ast_node = a.get_next(ast);
        }
        true
    }

    // port: TemplateAstMatcher#matchesNodeShape
    fn matches_node_shape(&self, ast: &Ast, template: NodeId, ast_node: Option<NodeId>) -> bool {
        // Java dereferences `ast` unconditionally in every branch but the CALL one.
        if Self::is_template_parameter_node(ast, template) {
            // Match the entire expression but only if it is an expression.
            return !NodeUtil::is_statement(ast, ast_node.expect("NullPointerException"));
        } else if Self::is_template_local_name_node(ast, template) {
            // Match any name. Maybe match locals here.
            if !ast_node.expect("NullPointerException").is_name(ast) {
                return false;
            }
        } else if Self::is_template_parameter_string_literal_node(ast, template) {
            // Matches parameters (in the refasterJS template) whose names start with
            // 'string_literal_'.
            return NodeUtil::is_some_compile_time_const_string_value(
                ast,
                ast_node.expect("NullPointerException"),
            );
        } else if template.is_call(ast) {
            // Loosely match CALL nodes. isEquivalentToShallow checks free calls against non-free
            // calls, but the template should ignore that distinction.
            let Some(a) = ast_node else {
                return false;
            };
            if !a.is_call(ast) || !a.has_x_children(ast, template.get_child_count(ast)) {
                return false;
            }
            // But check any children.
        } else if !template.is_equivalent_to_shallow(ast, ast_node.expect("NullPointerException")) {
            return false;
        }

        // isEquivalentToShallow guarantees the child counts match
        let ast_node = ast_node.unwrap();
        let mut template_child = template.get_first_child(ast);
        let mut ast_child = ast_node.get_first_child(ast);
        while let Some(tc) = template_child {
            if !self.matches_node_shape(ast, tc, ast_child) {
                return false;
            }
            template_child = tc.get_next(ast);
            ast_child = ast_child.expect("NullPointerException").get_next(ast);
        }
        true
    }

    // port: TemplateAstMatcher#matchesTemplate
    fn matches_template(
        &mut self,
        compiler: &mut AbstractCompiler,
        template: Option<NodeId>,
        ast_node: Option<NodeId>,
    ) -> bool {
        let mut template = template;
        let mut ast_node = ast_node;
        while let Some(t) = template {
            let Some(a) = ast_node else {
                return false;
            };
            if !self.matches_node(compiler, t, Some(a)) {
                return false;
            }
            template = t.get_next(compiler);
            ast_node = a.get_next(compiler);
        }
        true
    }

    /// Returns whether two nodes are equivalent, taking into account the template parameters that
    /// were provided to this matcher. If the template comparison node is a parameter node, then
    /// only the types of the node must match. If the template node is a string literal, only match
    /// string literals. Otherwise, the node must be equal and the child nodes must be equivalent
    /// according to the same function. This differs from the built in Node equivalence function
    /// with the special comparison.
    // port: TemplateAstMatcher#matchesNode
    fn matches_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        template: NodeId,
        ast_node: Option<NodeId>,
    ) -> bool {
        if Self::is_template_parameter_node(compiler, template) {
            let ast_node = ast_node.expect("NullPointerException");
            let param_index = template.get_double(compiler) as i32 as usize;
            let previous_match = self.param_node_matches[param_index];
            if let Some(previous_match) = previous_match {
                // If this named node has already been matched against, make sure all
                // subsequent usages of the same named node are equivalent.
                return ast_node.is_equivalent_to(compiler, previous_match);
            }

            // Only the types need to match for the template parameters, which allows
            // the template function to express arbitrary expressions.
            let template_type = template.get_jstype(compiler);

            let template_type = check_not_null!(template_type, "null template parameter type.");

            // TODO(johnlenz): We shouldn't spend time checking template whose
            // types whose definitions aren't included (NoResolvedType). Alternately
            // we should treat them as "unknown" and perform loose matches.
            let ast_type = ast_node.get_jstype(compiler);
            let (registry, ast) = compiler.get_type_registry_and_ast();
            if Self::is_unresolved_type(registry, ast, template_type) {
                return false;
            }

            let match_result =
                self.type_matching_strategy
                    .r#match(registry, ast, template_type, ast_type);
            self.is_loose_match = match_result.is_loose_match();
            let is_match = match_result.is_match();
            if is_match && previous_match.is_none() {
                self.param_node_matches[param_index] = Some(ast_node);
            }
            return is_match;
        } else if Self::is_template_local_name_node(compiler, template) {
            let ast_node = ast_node.expect("NullPointerException");
            // If this template name node was already matched against, then make sure
            // all subsequent usages of the same template name node are equivalent in
            // the matched code.
            // For example, this code will handle the case:
            // function template() {
            //   var a = 'str';
            //   fn(a);
            // }
            //
            // will only match test code:
            //   var b = 'str';
            //   fn(b);
            //
            // but it will not match:
            //   var b = 'str';
            //   fn('str');
            let param_index = template.get_double(compiler) as i32 as usize;
            let previously_matched = self.local_var_matches[param_index].is_some();
            if previously_matched {
                // If this named node has already been matched against, make sure all
                // subsequent usages of the same named node are equivalent.
                return Some(ast_node.get_string(compiler)) == self.local_var_matches[param_index];
            } else {
                let original_name = ast_node.get_original_name(compiler);
                let name = original_name.unwrap_or_else(|| ast_node.get_string(compiler));
                self.local_var_matches[param_index] = Some(name);
            }
        } else if Self::is_template_parameter_string_literal_node(compiler, template) {
            let ast_node = ast_node.expect("NullPointerException");
            let param_index = template.get_double(compiler) as i32 as usize;
            let previous_match = self.param_node_matches[param_index];
            if let Some(previous_match) = previous_match {
                return ast_node.is_equivalent_to(compiler, previous_match);
            }

            if NodeUtil::is_some_compile_time_const_string_value(compiler, ast_node) {
                self.param_node_matches[param_index] = Some(ast_node);
                return true;
            }
            return false;
        }

        // Template and AST shape has already been checked, but continue look for
        // other template variables (parameters and locals) that must be checked.
        let ast_node = ast_node.expect("NullPointerException");
        let mut template_child = template.get_first_child(compiler);
        let mut ast_child = ast_node.get_first_child(compiler);
        while let Some(tc) = template_child {
            if !self.matches_node(compiler, tc, ast_child) {
                return false;
            }
            template_child = tc.get_next(compiler);
            ast_child = ast_child.expect("NullPointerException").get_next(compiler);
        }

        true
    }

    // port: TemplateAstMatcher#isUnresolvedType
    fn is_unresolved_type(registry: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        // TODO(b/146173738): When types are used in templates that do not appear in the
        // compilation unit being processed, the template type will be a named type
        // that resolves to unknown instead of being a no resolved type. This should
        // be fixed in the compiler such that it resolves to a no resolved type, and
        // then this code can be simplified to use that.
        if type_.is_no_resolved_type(registry)
            || (type_.is_named_type(registry) && type_.is_unknown_type(registry, ast))
        {
            return true;
        }
        if type_.is_union_type(registry) {
            let members = type_
                .get_union_members(registry, ast)
                .expect("NullPointerException");
            for &alternate in members.iter() {
                if Self::is_unresolved_type(registry, ast, alternate) {
                    return true;
                }
            }
        }
        false
    }
}
