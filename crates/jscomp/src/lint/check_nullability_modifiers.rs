/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/lint/CheckNullabilityModifiers.java.

//! Checks for missing or redundant nullability modifiers.
//!
//! Primitive and literal types should not be preceded by a `!` modifier. Reference types must be
//! preceded by a `?` or `!` modifier.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    check_state,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    node::{Ast, NodeId},
    token::Token,
};

/// The diagnostic for a missing nullability modifier.
// port: CheckNullabilityModifiers#MISSING_NULLABILITY_MODIFIER_JSDOC
pub static MISSING_NULLABILITY_MODIFIER_JSDOC: DiagnosticType = DiagnosticType::disabled(
    "JSC_MISSING_NULLABILITY_MODIFIER_JSDOC",
    "{0} is a reference type with no nullability modifier, which is disallowed by the style guide.\nPlease add a '!' to make it explicitly non-nullable, or a '?' to make it explicitly nullable.",
);

/// The diagnostic for a missing nullability modifier, where the value is clearly nullable.
// port: CheckNullabilityModifiers#NULL_MISSING_NULLABILITY_MODIFIER_JSDOC
pub static NULL_MISSING_NULLABILITY_MODIFIER_JSDOC: DiagnosticType = DiagnosticType::disabled(
    "JSC_NULL_MISSING_NULLABILITY_MODIFIER_JSDOC",
    "{0} is a reference type with no nullability modifier that is explicitly set to null.\nAdd a '?' to make it explicitly nullable.",
);

/// The diagnostic for a redundant nullability modifier.
// port: CheckNullabilityModifiers#REDUNDANT_NULLABILITY_MODIFIER_JSDOC
pub static REDUNDANT_NULLABILITY_MODIFIER_JSDOC: DiagnosticType = DiagnosticType::disabled(
    "JSC_REDUNDANT_NULLABILITY_MODIFIER_JSDOC",
    "{0} is a non-reference type which is already non-nullable.\nPlease remove the redundant '!', which is disallowed by the style guide.",
);

/// The set of primitive type names. Note that `void` is a synonym for `undefined`.
// port: CheckNullabilityModifiers#PRIMITIVE_TYPE_NAMES
const PRIMITIVE_TYPE_NAMES: [&str; 8] = [
    "boolean",
    "number",
    "bigint",
    "string",
    "symbol",
    "undefined",
    "void",
    "null",
];

fn is_primitive_type_name(name: &JsString) -> bool {
    PRIMITIVE_TYPE_NAMES.iter().any(|p| name == *p)
}

pub struct CheckNullabilityModifiers {
    // Store the candidate warnings and template types found while traversing a single script
    // node.
    redundant_candidates: IndexSet<NodeId>,
    missing_candidates: IndexSet<NodeId>,
    null_missing_candidates: IndexSet<NodeId>,
    template_type_names: IndexSet<JsString>,
}

impl CheckNullabilityModifiers {
    // port: CheckNullabilityModifiers#CheckNullabilityModifiers
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            redundant_candidates: IndexSet::<_>::default(),
            missing_candidates: IndexSet::<_>::default(),
            null_missing_candidates: IndexSet::<_>::default(),
            template_type_names: IndexSet::<_>::default(),
        }
    }

    // port: CheckNullabilityModifiers#handleHasType
    fn handle_has_type(&mut self, ast: &mut Ast, expr: &JSTypeExpression, n: NodeId) {
        // Check if the type is explicitly set to null, if so use NULL_MISSING_NULLABILITY
        // diagnostic.
        if let Some(first) = n.get_first_child(ast)
            && NodeUtil::is_name_decl_or_simple_assign_lhs(ast, first, n)
        {
            let r_value = NodeUtil::get_r_value_of_l_value(ast, first);
            if let Some(r_value) = r_value
                && r_value.is_null(ast)
            {
                self.visit_type_expression_with_r_value(ast, expr, false, Some(r_value));
                return;
            }
        }
        self.visit_type_expression(ast, expr, false);
    }

    // port: CheckNullabilityModifiers#report
    fn report(&self, t: &mut NodeTraversal<'_>) {
        for &n in &self.missing_candidates {
            if self.should_report(t, n) {
                let name = Self::get_reported_type_name(t, n);
                t.report(n, &MISSING_NULLABILITY_MODIFIER_JSDOC, &[&name]);
            }
        }
        for &n in &self.null_missing_candidates {
            if self.should_report(t, n) {
                let name = Self::get_reported_type_name(t, n);
                t.report(n, &NULL_MISSING_NULLABILITY_MODIFIER_JSDOC, &[&name]);
            }
        }
        for &n in &self.redundant_candidates {
            if self.should_report(t, n) {
                // Report on parent so that modifier is also highlighted.
                let name = Self::get_reported_type_name(t, n);
                let parent = n.get_parent(t).unwrap();
                t.report(parent, &REDUNDANT_NULLABILITY_MODIFIER_JSDOC, &[&name]);
            }
        }
    }

    // port: CheckNullabilityModifiers#shouldReport
    fn should_report(&self, ast: &Ast, n: NodeId) -> bool {
        // Ignore type names that appear in @template clauses in the same source file. In rare
        // cases, this may cause a false negative (if the name is used in a non-template capacity
        // in the same file) or a false positive (if the name is used in a template capacity in a
        // separate file), but it makes this check possible in the absence of type information. If
        // the style guide ever mandates template types (and nothing else) to be all-caps, we can
        // use that assumption to make this check more precise.
        !n.is_string_lit(ast) || !self.template_type_names.contains(&n.get_string(ast))
    }

    // port: CheckNullabilityModifiers#visitTypeExpression(JSTypeExpression,boolean)
    fn visit_type_expression(
        &mut self,
        ast: &mut Ast,
        expr: &JSTypeExpression,
        has_artificial_top_level_bang: bool,
    ) {
        self.visit_type_expression_with_r_value(
            ast,
            expr,
            has_artificial_top_level_bang,
            /* rValue= */ None,
        );
    }

    // port: CheckNullabilityModifiers#visitTypeExpression(JSTypeExpression,boolean,Node)
    fn visit_type_expression_with_r_value(
        &mut self,
        ast: &mut Ast,
        expr: &JSTypeExpression,
        has_artificial_top_level_bang: bool,
        r_value: Option<NodeId>,
    ) {
        let root = expr.get_root();
        let null_missing_candidates = &mut self.null_missing_candidates;
        let missing_candidates = &mut self.missing_candidates;
        let redundant_candidates = &mut self.redundant_candidates;
        NodeUtil::visit_pre_order(ast, root, &mut |ast: &mut Ast, node: NodeId| {
            let parent = node.get_parent(ast);

            let is_primitive_or_literal = Self::is_primitive_type(ast, node)
                || Self::is_function_literal(ast, node)
                || Self::is_record_literal(ast, node);
            let is_reference = Self::is_reference_type(ast, node);

            // Whether the node is preceded by a '!' or '?'.
            let has_bang = parent.is_some_and(|p| p.get_token(ast) == Token::BANG);
            let has_qmark = parent.is_some_and(|p| p.get_token(ast) == Token::QMARK);

            // Whether the node is preceded by a '!' that wasn't artificially added.
            let has_non_artificial_bang =
                has_bang && !(has_artificial_top_level_bang && parent == Some(root));

            // Whether the node is the type in function(new:T) or function(this:T).
            let is_new_or_this = parent.is_some_and(|p| p.is_new(ast) || p.is_this(ast));

            // Whether the node is the type name in a typeof expression.
            let is_type_of_type = parent.is_some_and(|p| p.is_type_of(ast));

            // Whether the node is definitely a possible type for the rValue e.g. 'Type' in @type
            // {Type} or @type {Type|number} but not in @type {!Array<Type>}
            let is_applied_to_r_value =
                node == root || parent.is_some_and(|p| p.get_token(ast) == Token::PIPE);

            if is_reference && !has_bang && !has_qmark && !is_new_or_this && !is_type_of_type {
                if is_applied_to_r_value && r_value.is_some_and(|r| r.is_null(ast)) {
                    null_missing_candidates.insert(node);
                } else {
                    missing_candidates.insert(node);
                }
            } else if is_primitive_or_literal && has_non_artificial_bang {
                redundant_candidates.insert(node);
            }
        });
    }

    // port: CheckNullabilityModifiers#isPrimitiveType
    fn is_primitive_type(ast: &Ast, node: NodeId) -> bool {
        node.is_string_lit(ast) && is_primitive_type_name(&node.get_string(ast))
    }

    // port: CheckNullabilityModifiers#isReferenceType
    fn is_reference_type(ast: &Ast, node: NodeId) -> bool {
        node.is_string_lit(ast) && !is_primitive_type_name(&node.get_string(ast))
    }

    // port: CheckNullabilityModifiers#isFunctionLiteral
    fn is_function_literal(ast: &Ast, node: NodeId) -> bool {
        node.is_function(ast)
    }

    // port: CheckNullabilityModifiers#isRecordLiteral
    fn is_record_literal(ast: &Ast, node: NodeId) -> bool {
        node.get_token(ast) == Token::LC
    }

    // port: CheckNullabilityModifiers#getReportedTypeName
    fn get_reported_type_name(ast: &Ast, node: NodeId) -> String {
        if Self::is_function_literal(ast, node) {
            return "Function".to_string();
        }
        if Self::is_record_literal(ast, node) {
            return "Record literal".to_string();
        }
        check_state!(Self::is_primitive_type(ast, node) || Self::is_reference_type(ast, node));
        node.get_string(ast).to_string_lossy()
    }
}

impl CompilerPass for CheckNullabilityModifiers {
    // port: CheckNullabilityModifiers#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse_roots(compiler, self, externs, root);
    }
}

impl Callback for CheckNullabilityModifiers {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckNullabilityModifiers#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let info = n.get_jsdoc_info(t);
        if let Some(info) = info {
            self.template_type_names
                .extend(info.get_template_type_names());
            self.template_type_names
                .extend(info.get_type_transformations().into_keys());
            if info.has_type() {
                self.handle_has_type(t, &info.get_type().unwrap(), n);
            }
            for param in info.get_parameter_names() {
                if info.has_parameter_type(param.clone()) {
                    self.visit_type_expression(t, &info.get_parameter_type(param).unwrap(), false);
                }
            }
            if info.has_return_type() {
                self.visit_type_expression(t, &info.get_return_type().unwrap(), false);
            }
            if info.has_enum_parameter_type() {
                // JSDocInfoParser wraps the @enum type in an artificial '!' if it's not a
                // primitive type. Thus a missing modifier warning can never trigger, but a
                // redundant modifier warning can.
                self.visit_type_expression(t, &info.get_enum_parameter_type().unwrap(), false);
            }
            if info.has_typedef_type() {
                self.visit_type_expression(t, &info.get_typedef_type().unwrap(), false);
            }
            if info.has_this_type() {
                // JSDocInfoParser wraps the @this type in an artificial '!'. Thus a missing
                // modifier warning can never trigger, but a top-level '!' should be ignored if it
                // precedes a primitive or literal type; otherwise it will trigger a spurious
                // redundant modifier warning.
                self.visit_type_expression(t, &info.get_this_type().unwrap(), true);
            }
            // JSDocInfoParser enforces the @extends and @implements types to be unqualified in
            // the source code, so we don't need to check them.
        }

        if n.is_script(t) {
            // If exiting a script node, report applicable warnings and clear the maps.
            self.report(t);
            self.redundant_candidates.clear();
            self.missing_candidates.clear();
            self.null_missing_candidates.clear();
            self.template_type_names.clear();
        }
    }
}
