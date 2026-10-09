/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AstValidator.java.

//! Port of `AstValidator.java`.
//!
//! Every `validate*` method takes the compiler after the receiver (DESIGN §6): the type checks
//! call `compiler.toSource(..)` and read the compiler's options, allowable features and type
//! registry.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_util::NodeUtil;
use closure_jstype::js_type::Nullability;
use closure_jstype::prelude::*;
use closure_jstype::{JSTypeRegistry, TypeId};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::jscomp_base::tri::Tri;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null, check_state};
use std::panic::{AssertUnwindSafe, catch_unwind};

// Possible enhancements:
// * verify NAME, LABEL_NAME, GETPROP property name and unquoted
// object-literal keys are valid JavaScript identifiers.
// * optionally verify every node has source location information.

/// Violation handler
pub trait ViolationHandler {
    // port: AstValidator.ViolationHandler#handleViolation
    fn handle_violation(&mut self, ast: &Ast, message: &str, n: Option<NodeId>);
}

// Rust-only forwarding so a caller can lend a handler it inspects afterwards.
impl<T: ViolationHandler + ?Sized> ViolationHandler for &mut T {
    fn handle_violation(&mut self, ast: &Ast, message: &str, n: Option<NodeId>) {
        (**self).handle_violation(ast, message, n);
    }
}

/// The anonymous `ViolationHandler` of `AstValidator(AbstractCompiler, boolean)`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThrowingViolationHandler;

impl ViolationHandler for ThrowingViolationHandler {
    // port: AstValidator#AstValidator(AbstractCompiler,boolean) (ViolationHandler#handleViolation)
    fn handle_violation(&mut self, ast: &Ast, message: &str, n: Option<NodeId>) {
        // Java dereferences a null node (NullPointerException).
        let n = n.expect("NullPointerException");
        // Java: throw new IllegalStateException(...)
        panic!(
            "{}",
            message.to_string()
                + ". Reference node:\n"
                + &n.to_string_tree(ast)
                + "\n Parent node:\n"
                + &(if n.has_parent(ast) {
                    n.get_parent(ast).unwrap().to_string_tree(ast)
                } else {
                    " no parent ".to_string()
                })
        );
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeInfoValidation {
    JSTYPE,
    COLOR,
    NONE,
}

pub struct AstValidator<'h> {
    violation_handler: Box<dyn ViolationHandler + 'h>,

    current_script: Option<NodeId>,

    /// Perform type validation if this is enabled.
    type_validation_mode: TypeInfoValidation,

    /// Validate that a SCRIPT's FeatureSet property includes all features if this is enabled.
    is_script_feature_validation_enabled: bool,

    // TODO: varomodt - make this the default.
    /// Validate that all required inlinings were performed.
    should_validate_required_inlinings: bool,
}

const AWAIT_NOT_WITHIN_ASYNC_FUNCTION: &str = "'await' expression is not within an async function";

const AWAIT_NOT_ALLOWED_IN_PARAMETER_LIST: &str =
    "'await' expression is not allowed in a parameter list";

impl<'h> AstValidator<'h> {
    // port: AstValidator#AstValidator(AbstractCompiler,ViolationHandler,boolean,boolean)
    pub fn new_with_handler(
        _compiler: &AbstractCompiler,
        handler: impl ViolationHandler + 'h,
        validate_script_features: bool,
        should_validate_required_inlinings: bool,
    ) -> Self {
        Self {
            violation_handler: Box::new(handler),
            current_script: None,
            type_validation_mode: TypeInfoValidation::NONE,
            is_script_feature_validation_enabled: validate_script_features,
            should_validate_required_inlinings,
        }
    }

    // port: AstValidator#AstValidator(AbstractCompiler)
    pub fn new(compiler: &AbstractCompiler) -> Self {
        Self::new_with_script_features(compiler, /* validateScriptFeatures= */ false)
    }

    // port: AstValidator#AstValidator(AbstractCompiler,boolean)
    pub fn new_with_script_features(
        compiler: &AbstractCompiler,
        validate_script_features: bool,
    ) -> Self {
        Self::new_with_handler(
            compiler,
            ThrowingViolationHandler,
            validate_script_features,
            compiler
                .get_options()
                .get_should_validate_required_inlinings()
                == Tri::TRUE,
        )
    }

    /// Enable or disable validation of type information.
    ///
    /// TODO(b/74537281): Currently only expressions are checked for type information. Do we need
    /// to do more?
    // port: AstValidator#setTypeValidationMode
    pub fn set_type_validation_mode(&mut self, mode: TypeInfoValidation) -> &mut Self {
        self.type_validation_mode = mode;
        self
    }

    // port: AstValidator#validateRoot
    pub fn validate_root(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::ROOT, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 2);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_code_root(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_code_root(compiler, last);
    }

    // port: AstValidator#validateCodeRoot
    pub fn validate_code_root(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::ROOT, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_script(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateScript
    pub fn validate_script(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::SCRIPT, n);
        self.validate_has_source_name(compiler, n);
        self.validate_has_input_id(compiler, n);
        self.current_script = Some(n);
        if n.has_children(compiler)
            && n.get_first_child(compiler)
                .unwrap()
                .is_module_body(compiler)
        {
            self.validate_properties(compiler, n);
            self.validate_child_count_expected(compiler, n, 1);
            let first = n.get_first_child(compiler).unwrap();
            self.validate_module_contents(compiler, first);
        } else {
            let first = n.get_first_child(compiler);
            self.validate_statements(compiler, first);
        }
        if self.is_script_feature_validation_enabled {
            self.validate_script_feature_set(compiler, n);
        }
    }

    /// Confirm that every SCRIPT node’s FEATURE_SET <= compiler's allowable featureSet. This is
    /// possbile because with go/accurately-maintain-script-node-featureSet, each transpiler pass
    /// updates script features anytime it updates the compiler's allowable features.
    // port: AstValidator#validateScriptFeatureSet
    fn validate_script_feature_set(
        &mut self,
        compiler: &mut AbstractCompiler,
        script_node: NodeId,
    ) {
        if !script_node.is_script(compiler) {
            self.violation(compiler, "Not a script node", script_node);
            // unit tests for this pass perform "Negaive Testing" (i.e pass non-script nodes to
            // {@code validateScript}) and expect a violation {@code expectInvalid(n,
            // Check.SCRIPT);} report violation and return here instead of crashing below in
            // {@code NodeUtil.getFeatureSetofScript} for test to complete
            return;
        }
        let current_script = self.current_script.unwrap();
        let script_features = NodeUtil::get_feature_set_of_script(compiler, current_script);
        let allowable_features = Some(compiler.get_allowable_features());
        let (Some(script_features), Some(allowable_features)) =
            (script_features, allowable_features)
        else {
            return;
        };

        #[allow(clippy::collapsible_if)] // Retain Java control flow.
        if !allowable_features.contains(script_features) {
            if !script_node.is_from_externs(compiler) {
                // Skip this check for externs because we don't need to complete transpilation on
                // externs, and currently only transpile externs so that we can typecheck ES6+
                // features in externs.
                let different_features = script_features.without_set(allowable_features);
                let message = format!(
                    "SCRIPT node contains these unallowable features:[{}]",
                    different_features
                        .get_features()
                        .iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                self.violation(compiler, &message, current_script);
            }
        }
    }

    // port: AstValidator#validateModuleContents
    pub fn validate_module_contents(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::MODULE_BODY, n);
        let first = n.get_first_child(compiler);
        self.validate_statements(compiler, first);
    }

    // port: AstValidator#validateStatements
    pub fn validate_statements(&mut self, compiler: &mut AbstractCompiler, mut n: Option<NodeId>) {
        while let Some(cur) = n {
            self.validate_statement(compiler, cur);
            n = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateStatement(Node)
    pub fn validate_statement(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_statement_ambient(compiler, n, false);
    }

    /// Validates a statement node and its children.
    ///
    /// `is_ambient`: whether this statement comes from TS ambient `declare [...]`
    // port: AstValidator#validateStatement(Node,boolean)
    pub fn validate_statement_ambient(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_ambient: bool,
    ) {
        match n.get_token(compiler) {
            Token::LABEL => self.validate_label(compiler, n),
            Token::BLOCK => self.validate_block(compiler, n),
            Token::FUNCTION => {
                if is_ambient {
                    self.validate_function_signature(compiler, n);
                } else {
                    self.validate_function_statement(compiler, n);
                }
            }
            Token::WITH => self.validate_with(compiler, n),
            Token::FOR => self.validate_for(compiler, n),
            Token::FOR_IN => self.validate_for_in(compiler, n),
            Token::FOR_OF => self.validate_for_of(compiler, n),
            Token::FOR_AWAIT_OF => self.validate_for_await_of(compiler, n),
            Token::WHILE => self.validate_while(compiler, n),
            Token::DO => self.validate_do(compiler, n),
            Token::SWITCH => self.validate_switch(compiler, n),
            Token::IF => self.validate_if(compiler, n),
            Token::CONST | Token::VAR | Token::LET => {
                let token = n.get_token(compiler);
                self.validate_name_declaration_helper(compiler, n, token, n);
            }
            Token::EXPR_RESULT => self.validate_expr_stmt(compiler, n),
            Token::RETURN => self.validate_return(compiler, n),
            Token::THROW => self.validate_throw(compiler, n),
            Token::TRY => self.validate_try(compiler, n),
            Token::BREAK => self.validate_break(compiler, n),
            Token::CONTINUE => self.validate_continue(compiler, n),
            Token::EMPTY | Token::DEBUGGER => {
                self.validate_properties(compiler, n);
                self.validate_childless(compiler, n);
            }
            Token::CLASS => self.validate_class_declaration(compiler, n, is_ambient),
            Token::IMPORT => self.validate_import(compiler, n),
            Token::EXPORT => self.validate_export(compiler, n, is_ambient),
            Token::INTERFACE => self.validate_interface(compiler, n),
            Token::ENUM => self.validate_enum(compiler, n),
            Token::TYPE_ALIAS => self.validate_type_alias(compiler, n),
            Token::DECLARE => self.validate_ambient_declaration(compiler, n),
            Token::NAMESPACE => self.validate_namespace(compiler, n, is_ambient),
            Token::MODULE_BODY => {
                // Uncommon case where a module body is not the first child of a script. This may
                // happen in a specific circumstance where the {@code LateEs6ToEs3Rewriter} pass
                // injects code above a module body. Valid only when skipNonTranspilationPasses=true
                // and setWrapGoogModulesForWhitespaceOnly=false
                // TODO: b/294420383 Ideally the LateEs6ToEs3Rewriter pass should not inject code
                // above the module body node
                #[allow(clippy::collapsible_if)] // Retain Java control flow.
                if compiler.get_options().get_skip_non_transpilation_passes() {
                    if !compiler
                        .get_options()
                        .should_wrap_goog_modules_for_whitespace_only()
                    {
                        self.validate_module_contents(compiler, n);
                        return;
                    }
                }
                let message = format!("Expected statement but was {}.", n.get_token(compiler));
                self.violation(compiler, &message, n);
            }
            _ => {
                if n.is_module_body(compiler)
                    && compiler.get_options().get_skip_non_transpilation_passes()
                {
                    check_state!(
                        !compiler
                            .get_options()
                            .should_wrap_goog_modules_for_whitespace_only(),
                        "Modules can exist in transpiler only if setWrapGoogModulesForWhitespaceOnly \
                         is false"
                    );
                    self.validate_module_contents(compiler, n);
                    return;
                }
                let message = format!("Expected statement but was {}.", n.get_token(compiler));
                self.violation(compiler, &message, n);
            }
        }
    }

    // port: AstValidator#validateExpression
    pub fn validate_expression(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_type_information(compiler, n);
        self.validate_required_inlinings(compiler, n);

        match n.get_token(compiler) {
            // Childless expressions
            Token::NEW_TARGET => {
                self.validate_feature(compiler, Feature::NEW_TARGET, n);
                self.validate_properties(compiler, n);
                self.validate_childless(compiler, n);
            }
            Token::IMPORT_META => {
                self.validate_feature(compiler, Feature::IMPORT_META, n);
                self.validate_properties(compiler, n);
                self.validate_childless(compiler, n);
            }
            Token::FALSE | Token::NULL | Token::THIS | Token::TRUE => {
                self.validate_properties(compiler, n);
                self.validate_childless(compiler, n);
            }

            // General unary ops
            Token::DELPROP
            | Token::POS
            | Token::NEG
            | Token::NOT
            | Token::TYPEOF
            | Token::VOID
            | Token::BITNOT
            | Token::CAST => self.validate_unary_op(compiler, n),
            Token::INC | Token::DEC => self.validate_inc_dec_op(compiler, n),

            // Assignments
            Token::ASSIGN => self.validate_assignment_expression(compiler, n),
            Token::ASSIGN_EXPONENT => {
                self.validate_feature(compiler, Feature::EXPONENT_OP, n);
                self.validate_compound_assignment_expression(compiler, n);
            }
            Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD => self.validate_compound_assignment_expression(compiler, n),
            Token::ASSIGN_COALESCE => {
                self.validate_feature(compiler, Feature::NULL_COALESCE_OP, n);
                self.validate_feature(compiler, Feature::LOGICAL_ASSIGNMENT, n);
                self.validate_compound_assignment_expression(compiler, n);
            }
            Token::ASSIGN_OR | Token::ASSIGN_AND => {
                self.validate_feature(compiler, Feature::LOGICAL_ASSIGNMENT, n);
                self.validate_compound_assignment_expression(compiler, n);
            }

            Token::HOOK => self.validate_trinary_op(compiler, n),

            // Node types that require special handling
            Token::STRINGLIT => self.validate_string_lit(compiler, n),
            Token::NUMBER => self.validate_number(compiler, n),
            Token::BIGINT => self.validate_big_int(compiler, n),
            Token::NAME => self.validate_name(compiler, n),

            // General binary ops
            Token::EXPONENT => {
                self.validate_feature(compiler, Feature::EXPONENT_OP, n);
                self.validate_binary_op(compiler, n);
            }
            Token::COALESCE => {
                self.validate_feature(compiler, Feature::NULL_COALESCE_OP, n);
                self.validate_binary_op(compiler, n);
            }
            Token::COMMA
            | Token::OR
            | Token::AND
            | Token::BITOR
            | Token::BITXOR
            | Token::BITAND
            | Token::EQ
            | Token::NE
            | Token::SHEQ
            | Token::SHNE
            | Token::LT
            | Token::GT
            | Token::LE
            | Token::GE
            | Token::INSTANCEOF
            | Token::IN
            | Token::LSH
            | Token::RSH
            | Token::URSH
            | Token::SUB
            | Token::ADD
            | Token::MUL
            | Token::MOD
            | Token::DIV => self.validate_binary_op(compiler, n),

            Token::GETELEM => self.validate_get_elem(compiler, n),
            Token::OPTCHAIN_GETELEM => self.validate_opt_chain_get_elem(compiler, n),

            Token::GETPROP => self.validate_get_prop(compiler, n),
            Token::OPTCHAIN_GETPROP => self.validate_opt_chain_get_prop(compiler, n),

            Token::ARRAYLIT => self.validate_array_lit(compiler, n),
            Token::OBJECTLIT => self.validate_object_lit(compiler, n),
            Token::REGEXP => self.validate_reg_exp_lit(compiler, n),

            Token::CALL => self.validate_call(compiler, n),
            Token::OPTCHAIN_CALL => self.validate_opt_chain_call(compiler, n),
            Token::NEW => self.validate_new(compiler, n),

            Token::FUNCTION => {
                self.validate_required_inlinings(compiler, n);
                self.validate_function_expression(compiler, n);
            }
            Token::CLASS => self.validate_class(compiler, n),

            Token::TEMPLATELIT => self.validate_template_lit(compiler, n),
            Token::TAGGED_TEMPLATELIT => self.validate_tagged_template_lit(compiler, n),

            Token::YIELD => self.validate_yield(compiler, n),
            Token::AWAIT => self.validate_await(compiler, n),
            Token::DYNAMIC_IMPORT => {
                self.validate_feature(compiler, Feature::DYNAMIC_IMPORT, n);
                self.validate_unary_op(compiler, n);
            }

            _ => {
                let message = format!("Expected expression but was {}", n.get_token(compiler));
                self.violation(compiler, &message, n);
            }
        }
    }

    /// Validate an expression or expresison-like construct.
    ///
    /// An expression-like construct (pseudoexpression) is an AST fragment that is valid in some,
    /// but not all, of the same contexts as true expressions. For example, a VANILLA_FOR permits
    /// EMPTY as its condition and increment expressions, even though EMPTY is not valid as an
    /// expression in general.
    ///
    /// `allowed_pseudoexpressions` allows the caller to specify which pseudoexpressions are valid
    /// for their context. If `n` is a pseudoexpression, it will be considered invalid unless its
    /// token is in this set.
    // port: AstValidator#validatePseudoExpression
    fn validate_pseudo_expression(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        allowed_pseudoexpressions: &[Token],
    ) {
        let allowed_tokens_set = allowed_pseudoexpressions;
        match n.get_token(compiler) {
            Token::EMPTY => {
                check_argument!(
                    allowed_tokens_set.contains(&Token::EMPTY),
                    "Unexpected pseudoexpression %s",
                    n.to_string(compiler)
                );
                self.validate_properties(compiler, n);
                self.validate_childless(compiler, n);
            }
            Token::ITER_SPREAD => {
                check_argument!(
                    allowed_tokens_set.contains(&Token::ITER_SPREAD),
                    "Unexpected pseudoexpression %s",
                    n.to_string(compiler)
                );
                self.validate_properties(compiler, n);
                self.validate_child_count(compiler, n);
                self.validate_feature(compiler, Feature::SPREAD_EXPRESSIONS, n);
                let first = n.get_first_child(compiler).unwrap();
                self.validate_expression(compiler, first);
            }
            // The only kinds of potential pseudo-expressions we recognize are EMPTY and
            // ITER_SPREAD. So if the given node is neither, validate that it's a (non-pseudo)
            // legitimate expression
            _ => self.validate_expression(compiler, n),
        }
    }

    /// Enforces the given node has a type if we are validating JSTypes or Colors
    ///
    /// `n`: a Node which we expect to have a type attached (i.e. not a control-flow-only node like
    /// a BLOCK or IF)
    // port: AstValidator#validateTypeInformation
    fn validate_type_information(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.get_is_in_closure_unaware_subtree(compiler) {
            // We don't expect closure-unaware code to have type information.
            // TODO: b/321233583 - Maybe this should be a separate validation step, to ensure that
            // nothing tries to infer type information where we are mostly unsure of it?

            return;
        }
        if self.type_validation_mode == TypeInfoValidation::NONE {
            return;
        }

        if self.type_validation_mode == TypeInfoValidation::JSTYPE {
            let type_ = n.get_jstype(compiler);

            if let Some(type_) = type_ {
                // null types are checked in the switch statement
                let (reg, ast) = type_registry(compiler);
                if !type_.is_resolved(reg) {
                    let message = format!("Found unresolved type {}", type_.to_string(reg, ast));
                    self.violation(compiler, &message, n);
                }
            }
        }

        match n.get_token(compiler) {
            Token::CALL => {
                if !n.get_first_child(compiler).unwrap().is_super(compiler) {
                    // TODO(sdh): need to validate super() using validateNewType() instead, if it
                    // existed
                    self.validate_call_type(compiler, n);
                }
            }
            _ => self.expect_some_type_information(compiler, n),
        }
    }

    // port: AstValidator#validateCallType
    fn validate_call_type(&mut self, compiler: &mut AbstractCompiler, call_node: NodeId) {
        match self.type_validation_mode {
            TypeInfoValidation::JSTYPE => {
                // TODO(b/74537281): Shouldn't CALL nodes always have a type, even if it is
                // unknown?
                let callee = call_node.get_first_child(compiler).unwrap();
                let callee_type: TypeId = check_not_null!(
                    callee.get_jstype(compiler),
                    "Callee of\n\n%s\nhas no type.",
                    call_node.to_string_tree(compiler)
                );

                let (reg, ast) = type_registry(compiler);
                if callee_type.is_function_type(reg) {
                    let callee_function_type = callee_type.to_maybe_function_type(reg).unwrap();
                    let return_type = callee_function_type.get_return_type(reg);
                    // Skip this check if the call node was originally in a cast, because the cast
                    // type may be narrower than the return type. Also skip the check if the
                    // function's return type is the any (formerly unknown) type, since we may
                    // have inferred a better type.
                    if call_node.get_jstype_before_cast(ast).is_none()
                        && !return_type.is_unknown_type(reg, ast)
                    {
                        self.expect_matching_type_information(compiler, call_node, return_type);
                    }
                }
                // TODO(b/74537281): What other cases should be covered?
            }
            TypeInfoValidation::COLOR => {
                let callee = call_node.get_first_child(compiler).unwrap();
                check_not_null!(
                    callee.get_color(compiler),
                    "Callee of\n\n%s\nhas no color.",
                    call_node.to_string_tree(compiler)
                );
                // skip additional validation of return types, since optimization colors don't
                // include call signature types
            }
            TypeInfoValidation::NONE => panic!("AssertionError"),
        }
    }

    // port: AstValidator#expectSomeTypeInformation
    fn expect_some_type_information(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        match self.type_validation_mode {
            TypeInfoValidation::JSTYPE => {
                if n.get_jstype(compiler).is_none() {
                    let statement = NodeUtil::get_enclosing_statement(compiler, n).unwrap();
                    let message = format!(
                        "Type information missing\n{}",
                        compiler.to_source_for_node(statement)
                    );
                    self.violation(compiler, &message, n);
                }
            }
            TypeInfoValidation::COLOR => {
                if n.get_color(compiler).is_none() {
                    let statement = NodeUtil::get_enclosing_statement(compiler, n).unwrap();
                    let message = format!(
                        "Color information missing\n{}",
                        compiler.to_source_for_node(statement)
                    );
                    self.violation(compiler, &message, n);
                }
            }
            TypeInfoValidation::NONE => panic!("AssertionError"),
        }
    }

    // port: AstValidator#expectMatchingTypeInformation
    fn expect_matching_type_information(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        expected_type_i: TypeId,
    ) {
        let type_i = n.get_jstype(compiler);
        let (reg, ast) = type_registry(compiler);
        // Objects.equals(expectedTypeI, typeI) with a non-null expectedTypeI
        if !expected_type_i.equals(reg, ast, type_i) {
            let message = format!(
                "Expected type: {} Actual type: {}",
                Self::get_type_annotation_string(reg, ast, Some(expected_type_i)),
                Self::get_type_annotation_string(reg, ast, type_i)
            );
            self.violation(compiler, &message, n);
        }
    }

    // port: AstValidator#getTypeAnnotationString
    fn get_type_annotation_string(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_i: Option<TypeId>,
    ) -> String {
        match type_i {
            None => "NO TYPE INFORMATION".to_string(),
            Some(type_i) => {
                format!(
                    "{{{}}}",
                    type_i.to_annotation_string(reg, ast, Nullability::EXPLICIT)
                )
            }
        }
    }

    // port: AstValidator#validateYield
    fn validate_yield(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::GENERATORS, n);
        self.validate_node_type(compiler, Token::YIELD, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_in(compiler, n, 0, 1);
        if n.has_children(compiler) {
            let first = n.get_first_child(compiler).unwrap();
            self.validate_expression(compiler, first);
        }
        self.validate_yield_within_generator_function(compiler, n);
    }

    // port: AstValidator#validateYieldWithinGeneratorFunction
    fn validate_yield_within_generator_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) {
        let parent_function = NodeUtil::get_enclosing_function(compiler, n);
        if parent_function.is_none_or(|f| !f.is_generator_function(compiler)) {
            self.violation(
                compiler,
                "'yield' expression is not within a generator function",
                n,
            );
        } else if self.is_in_parameter_list_of_function(compiler, n, parent_function.unwrap()) {
            self.violation(
                compiler,
                "'yield' expression is not allowed in a parameter list",
                n,
            );
        }
    }

    // port: AstValidator#validateAwait
    fn validate_await(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::ASYNC_FUNCTIONS, n);
        self.validate_node_type(compiler, Token::AWAIT, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        self.validate_await_within_async_function(compiler, n);
    }

    // port: AstValidator#validateAwaitWithinAsyncFunction
    fn validate_await_within_async_function(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let Some(parent_function) = NodeUtil::get_enclosing_function(compiler, n) else {
            // Top-level await is only allowed in modules.
            if !NodeUtil::get_enclosing_script(compiler, n)
                .unwrap()
                .get_boolean_prop(compiler, NodeId::ES6_MODULE)
            {
                self.violation(compiler, AWAIT_NOT_WITHIN_ASYNC_FUNCTION, n);
            }
            return;
        };

        if !parent_function.is_async_function(compiler) {
            self.violation(compiler, AWAIT_NOT_WITHIN_ASYNC_FUNCTION, n);
        } else if self.is_in_parameter_list_of_function(compiler, n, parent_function) {
            self.violation(compiler, AWAIT_NOT_ALLOWED_IN_PARAMETER_LIST, n);
        }
    }

    // port: AstValidator#isInParameterListOfFunction
    fn is_in_parameter_list_of_function(
        &self,
        ast: &Ast,
        child: NodeId,
        function_node: NodeId,
    ) -> bool {
        let param_list = check_not_null!(
            function_node.get_second_child(ast),
            &function_node.to_string(ast)
        );
        let mut parent = child.get_parent(ast);
        while parent != Some(function_node) {
            let p = check_not_null!(
                parent,
                "%s not contained in function %s",
                child.to_string(ast),
                function_node.to_string(ast)
            );
            if p == param_list {
                return true;
            }
            parent = p.get_parent(ast);
        }
        false
    }

    // port: AstValidator#validateImport
    fn validate_import(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::MODULES, n);
        self.validate_node_type(compiler, Token::IMPORT, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);

        let first = n.get_first_child(compiler).unwrap();
        if first.is_name(compiler) {
            self.validate_name(compiler, first);
        } else {
            self.validate_node_type(compiler, Token::EMPTY, first);
        }

        let second_child = n.get_second_child(compiler).unwrap();
        match second_child.get_token(compiler) {
            Token::IMPORT_SPECS => self.validate_import_specifiers(compiler, second_child),
            Token::IMPORT_STAR => self.validate_non_empty_string(compiler, second_child),
            _ => self.validate_node_type(compiler, Token::EMPTY, second_child),
        }

        let third = n.get_child_at_index(compiler, 2).unwrap();
        self.validate_string_lit(compiler, third);
    }

    // port: AstValidator#validateImportSpecifiers
    fn validate_import_specifiers(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::IMPORT_SPECS, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_import_specifier(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateImportSpecifier
    fn validate_import_specifier(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::IMPORT_SPEC, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 2);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_name(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateExport
    fn validate_export(&mut self, compiler: &mut AbstractCompiler, n: NodeId, is_ambient: bool) {
        self.validate_feature(compiler, Feature::MODULES, n);
        self.validate_node_type(compiler, Token::EXPORT, n);
        if n.get_boolean_prop(compiler, NodeId::EXPORT_ALL_FROM) {
            // export * from "mod"
            self.validate_properties(compiler, n);
            self.validate_child_count_expected(compiler, n, 2);
            let first = n.get_first_child(compiler).unwrap();
            self.validate_node_type(compiler, Token::EMPTY, first);
            let second = n.get_second_child(compiler).unwrap();
            self.validate_string_lit(compiler, second);
        } else if n.get_boolean_prop(compiler, NodeId::EXPORT_DEFAULT) {
            // export default foo = 2
            self.validate_properties(compiler, n);
            self.validate_child_count_expected(compiler, n, 1);
            let first = n.get_first_child(compiler).unwrap();
            self.validate_expression(compiler, first);
        } else {
            self.validate_properties(compiler, n);
            self.validate_child_count_in(compiler, n, 1, 2);
            let first = n.get_first_child(compiler).unwrap();
            if first.is_export_specs(compiler) {
                self.validate_export_specifiers(compiler, first);
            } else {
                self.validate_statement_ambient(compiler, first, is_ambient);
            }
            if n.has_two_children(compiler) {
                let second = n.get_second_child(compiler).unwrap();
                self.validate_string_lit(compiler, second);
            }
        }
    }

    // port: AstValidator#validateExportSpecifiers
    fn validate_export_specifiers(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::EXPORT_SPECS, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_export_specifier(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateExportSpecifier
    fn validate_export_specifier(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::EXPORT_SPEC, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 2);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_name(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateTaggedTemplateLit
    fn validate_tagged_template_lit(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::TEMPLATE_LITERALS, n);
        self.validate_node_type(compiler, Token::TAGGED_TEMPLATELIT, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_template_lit(compiler, last);
    }

    // port: AstValidator#validateTemplateLit
    fn validate_template_lit(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::TEMPLATE_LITERALS, n);
        self.validate_node_type(compiler, Token::TEMPLATELIT, n);
        let mut child = n.get_first_child(compiler);
        while let Some(cur) = child {
            if cur.is_template_lit_string(compiler) {
                self.validate_template_lit_string(compiler, cur);
            } else {
                self.validate_template_lit_sub(compiler, cur);
            }
            child = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateTemplateLitString
    fn validate_template_lit_string(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::TEMPLATELIT_STRING, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        // Validate that getRawString doesn't throw (Java catches UnsupportedOperationException;
        // the Rust Node panics like Java's exception).
        let ast: &Ast = compiler;
        if catch_unwind(AssertUnwindSafe(|| n.get_raw_string(ast))).is_err() {
            self.violation(compiler, "Invalid TEMPLATELIT_STRING node.", n);
        }
    }

    // port: AstValidator#validateTemplateLitSub
    fn validate_template_lit_sub(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::TEMPLATELIT_SUB, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
    }

    // port: AstValidator#validateInterface
    fn validate_interface(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::INTERFACE, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let name = n.get_first_child(compiler).unwrap();
        self.validate_name(compiler, name);
        let super_types = name.get_next(compiler).unwrap();
        if super_types.is_empty(compiler) {
            self.validate_properties(compiler, super_types);
            self.validate_childless(compiler, super_types);
        } else {
            self.validate_interface_extends(compiler, super_types);
        }
        let last = n.get_last_child(compiler).unwrap();
        self.validate_interface_members(compiler, last);
    }

    // port: AstValidator#validateInterfaceExtends
    fn validate_interface_extends(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::INTERFACE_EXTENDS, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_named_type(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateInterfaceMembers
    fn validate_interface_members(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::INTERFACE_MEMBERS, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_interface_member(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateInterfaceMember
    fn validate_interface_member(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::MEMBER_FUNCTION_DEF => {
                self.validate_properties(compiler, n);
                self.validate_child_count(compiler, n);
                let first = n.get_first_child(compiler).unwrap();
                self.validate_function_signature(compiler, first);
            }
            Token::MEMBER_VARIABLE_DEF => {
                self.validate_properties(compiler, n);
                self.validate_childless(compiler, n);
            }
            Token::INDEX_SIGNATURE => {
                self.validate_properties(compiler, n);
                self.validate_child_count(compiler, n);
                let child = n.get_first_child(compiler).unwrap();
                self.validate_properties(compiler, child);
                self.validate_childless(compiler, child);
            }
            Token::CALL_SIGNATURE => {
                self.validate_properties(compiler, n);
                self.validate_child_count(compiler, n);
            }
            _ => {
                let message = format!(
                    "Interface contained member of invalid type {}",
                    n.get_token(compiler)
                );
                self.violation(compiler, &message, n);
            }
        }
    }

    // port: AstValidator#validateEnum
    fn validate_enum(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::ENUM, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_name(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_enum_members(compiler, last);
    }

    // port: AstValidator#validateEnumMembers
    fn validate_enum_members(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::ENUM_MEMBERS, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_enum_string_key(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateEnumStringKey
    fn validate_enum_string_key(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::STRING_KEY, n);
        self.validate_object_literal_key_name(compiler, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 0);
    }

    /// In a class declaration, unlike a class expression, the class name is required.
    // port: AstValidator#validateClassDeclaration
    fn validate_class_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_ambient: bool,
    ) {
        self.validate_class_helper(compiler, n, is_ambient);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_name(compiler, first);
    }

    // port: AstValidator#validateClass
    fn validate_class(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_class_helper(compiler, n, false);
    }

    // port: AstValidator#validateClassHelper
    fn validate_class_helper(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_ambient: bool,
    ) {
        self.validate_feature(compiler, Feature::CLASSES, n);
        self.validate_node_type(compiler, Token::CLASS, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);

        let name = n.get_first_child(compiler).unwrap();
        if name.is_empty(compiler) {
            self.validate_properties(compiler, name);
            self.validate_childless(compiler, name);
        } else {
            self.validate_name(compiler, name);
        }

        let super_class = name.get_next(compiler).unwrap();
        if super_class.is_empty(compiler) {
            self.validate_properties(compiler, super_class);
            self.validate_childless(compiler, super_class);
        } else {
            self.validate_expression(compiler, super_class);
        }

        let last = n.get_last_child(compiler).unwrap();
        self.validate_class_members(compiler, last, is_ambient);
    }

    // port: AstValidator#validateClassMembers
    fn validate_class_members(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_ambient: bool,
    ) {
        self.validate_node_type(compiler, Token::CLASS_MEMBERS, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_class_member(compiler, cur, is_ambient);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateClassMember
    fn validate_class_member(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_ambient: bool,
    ) {
        match n.get_token(compiler) {
            Token::MEMBER_FUNCTION_DEF => {
                self.validate_feature(compiler, Feature::MEMBER_DECLARATIONS, n);
                if n.is_private_identifier(compiler) {
                    self.validate_feature(compiler, Feature::PRIVATE_ELEMENTS, n);
                }
                self.validate_object_literal_key_name(compiler, n);
                self.validate_properties(compiler, n);
                self.validate_child_count(compiler, n);
                self.validate_member_function(compiler, n, is_ambient);
            }
            Token::GETTER_DEF | Token::SETTER_DEF => {
                self.validate_feature(compiler, Feature::CLASS_GETTER_SETTER, n);
                if n.is_private_identifier(compiler) {
                    self.validate_feature(compiler, Feature::PRIVATE_ELEMENTS, n);
                }
                self.validate_object_literal_key_name(compiler, n);
                self.validate_object_lit_key(compiler, n);
                self.validate_properties(compiler, n);
                self.validate_child_count(compiler, n);
                self.validate_member_function(compiler, n, is_ambient);
            }
            Token::MEMBER_VARIABLE_DEF => {
                self.validate_properties(compiler, n);
                self.validate_childless(compiler, n);
            }
            Token::COMPUTED_PROP => self.validate_computed_prop_class_method(compiler, n),
            Token::MEMBER_FIELD_DEF => self.validate_class_field(compiler, n),
            Token::COMPUTED_FIELD_DEF => self.validate_computed_prop_class_field(compiler, n),
            Token::INDEX_SIGNATURE => {
                self.validate_properties(compiler, n);
                self.validate_child_count(compiler, n);
                let child = n.get_first_child(compiler).unwrap();
                self.validate_properties(compiler, child);
                self.validate_childless(compiler, child);
            }
            Token::CALL_SIGNATURE => {
                self.validate_properties(compiler, n);
                self.validate_child_count(compiler, n);
            }
            Token::BLOCK => {
                self.validate_feature(compiler, Feature::CLASS_STATIC_BLOCK, n);
                self.validate_block(compiler, n);
            }
            Token::EMPTY => {
                // Empty is allowed too.
            }
            _ => {
                let message = format!(
                    "Class contained member of invalid type {}",
                    n.get_token(compiler)
                );
                self.violation(compiler, &message, n);
            }
        }
    }

    // port: AstValidator#validateMemberFunction
    fn validate_member_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_ambient: bool,
    ) {
        let function = n.get_first_child(compiler).unwrap();
        if is_ambient {
            self.validate_function_signature(compiler, function);
        } else {
            self.validate_function_expression(compiler, function);
        }
    }

    // port: AstValidator#validateClassField
    fn validate_class_field(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.is_private_identifier(compiler) {
            self.validate_feature(compiler, Feature::PRIVATE_ELEMENTS, n);
        } else {
            self.validate_feature(compiler, Feature::PUBLIC_CLASS_FIELDS, n);
        }
        self.validate_non_empty_string(compiler, n);
        if n.has_children(compiler) {
            let first = n.get_first_child(compiler).unwrap();
            self.validate_expression(compiler, first);
        }
    }

    // port: AstValidator#validateComputedPropClassField
    fn validate_computed_prop_class_field(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::PUBLIC_CLASS_FIELDS, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        if let Some(second) = n.get_second_child(compiler) {
            self.validate_expression(compiler, second);
        }
    }

    // port: AstValidator#validateBlock
    fn validate_block(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::BLOCK, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_statement(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateHasSourceName
    fn validate_has_source_name(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let source_name = n.get_source_file_name(compiler);
        if source_name.is_none_or(|s| s.is_empty()) {
            self.violation(compiler, "Missing 'source name' annotation.", n);
        }
    }

    // port: AstValidator#validateHasInputId
    fn validate_has_input_id(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let input_id = n.get_input_id(compiler);
        if input_id.is_none() {
            self.violation(compiler, "Missing 'input id' annotation.", n);
        }
    }

    // port: AstValidator#validateLabel
    fn validate_label(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::LABEL, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_label_name(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_statement(compiler, last);
    }

    // port: AstValidator#validateLabelName
    fn validate_label_name(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::LABEL_NAME, n);
        self.validate_non_empty_string(compiler, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
    }

    // port: AstValidator#validateNonEmptyString
    fn validate_non_empty_string(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if self.validate_non_null_string(compiler, n) && n.get_string_ref(compiler).is_empty() {
            self.violation(compiler, "Expected non-empty string.", n);
        }
    }

    // port: AstValidator#validateEmptyString
    fn validate_empty_string(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if self.validate_non_null_string(compiler, n) && !n.get_string_ref(compiler).is_empty() {
            self.violation(compiler, "Expected empty string.", n);
        }
    }

    // port: AstValidator#validateNonNullString
    fn validate_non_null_string(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        // Java catches the RuntimeException Node#getString throws for a non-string node; the Rust
        // Node panics there. A Rust string node never holds null.
        let ast: &Ast = compiler;
        if catch_unwind(AssertUnwindSafe(|| n.get_string(ast))).is_err() {
            self.violation(compiler, "Expected non-null string.", n);
            return false;
        }
        true
    }

    // port: AstValidator#validateName
    fn validate_name(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::NAME, n);
        self.validate_non_empty_string(compiler, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        self.validate_type_information(compiler, n);
        self.validate_shadow_content_if_present(compiler, n);
    }

    // port: AstValidator#validateOptionalName
    fn validate_optional_name(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::NAME, n);
        self.validate_non_null_string(compiler, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        // A Rust string node never holds null.
        let is_empty = n.get_string_ref(compiler).is_empty();
        if !is_empty {
            self.validate_type_information(compiler, n);
        }
    }

    // port: AstValidator#validateShadowContentIfPresent
    fn validate_shadow_content_if_present(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let Some(shadow) = n.get_closure_unaware_shadow(compiler) else {
            return;
        };
        if !shadow.is_root(compiler) {
            self.violation(compiler, "Shadow reference node is not a ROOT node", shadow);
            return;
        }
        let shadow_script = shadow.get_first_child(compiler);
        if shadow_script.is_none_or(|s| !s.is_script(compiler)) {
            self.violation(
                compiler,
                "Shadow root node's child is not a script node",
                shadow_script,
            );
            return;
        }
        let shadow_script = shadow_script.unwrap();
        if shadow_script.get_child_count(compiler) != 1 {
            self.violation(
                compiler,
                "Shadow SCRIPT node child has more than one child",
                shadow_script,
            );
            return;
        }
        let expr_result = shadow_script.get_first_child(compiler);
        if expr_result.is_none_or(|e| !e.is_expr_result(compiler)) {
            self.violation(
                compiler,
                "Shadow SCRIPT node child is not an expr result node",
                expr_result,
            );
            return;
        }
        let expr_result = expr_result.unwrap();
        if expr_result.get_child_count(compiler) != 1 {
            self.violation(
                compiler,
                "Shadow EXPR_RESULT node should have exactly one child",
                expr_result,
            );
            return;
        }
        let shadow_js_call = expr_result.get_only_child(compiler);
        if !shadow_js_call.is_call(compiler) {
            self.violation(
                compiler,
                "Shadow node EXPR_RESULT child is not a call",
                shadow_js_call,
            );
            return;
        }
        let shadow_js_function = shadow_js_call.get_last_child(compiler).unwrap();
        if !shadow_js_function.is_function(compiler) {
            self.violation(
                compiler,
                "Shadow node CALL child is not a function",
                shadow_js_function,
            );
            return;
        }
        self.validate_function_expression(compiler, shadow_js_function);
    }

    // port: AstValidator#validateEmptyName
    fn validate_empty_name(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::NAME, n);
        self.validate_empty_string(compiler, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
    }

    // port: AstValidator#validateFunctionStatement
    fn validate_function_statement(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::FUNCTION, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_name(compiler, first);
        let second = n.get_second_child(compiler).unwrap();
        self.validate_parameters(compiler, second);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_function_body(compiler, last, false);
        self.validate_function_features(compiler, n);
        if n.get_parent(compiler).unwrap().is_block(compiler)
            && !n.get_grandparent(compiler).unwrap().is_function(compiler)
        {
            // e.g. if (true) { function f() {} }
            self.validate_feature(compiler, Feature::BLOCK_SCOPED_FUNCTION_DECLARATION, n);
        }
    }

    // port: AstValidator#validateFunctionExpression
    fn validate_function_expression(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_function_expression_helper(compiler, n, false);
    }

    // port: AstValidator#validateFunctionSignature
    fn validate_function_signature(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_function_expression_helper(compiler, n, true);
    }

    // port: AstValidator#validateFunctionExpressionHelper
    fn validate_function_expression_helper(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_ambient: bool,
    ) {
        self.validate_node_type(compiler, Token::FUNCTION, n);
        self.validate_properties(compiler, n);

        self.validate_child_count(compiler, n);

        let second = n.get_second_child(compiler).unwrap();
        self.validate_parameters(compiler, second);

        let name = n.get_first_child(compiler).unwrap();
        let body = n.get_last_child(compiler).unwrap();
        if n.is_arrow_function(compiler) {
            self.validate_empty_name(compiler, name);
            if body.is_block(compiler) {
                self.validate_block(compiler, body);
            } else {
                self.validate_expression(compiler, body);
            }
        } else {
            self.validate_optional_name(compiler, name);
            self.validate_function_body(compiler, body, is_ambient);
        }
        self.validate_function_features(compiler, n);
    }

    // port: AstValidator#validateFunctionFeatures
    fn validate_function_features(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.is_arrow_function(compiler) {
            self.validate_feature(compiler, Feature::ARROW_FUNCTIONS, n);
        }
        if n.is_generator_function(compiler) {
            self.validate_feature(compiler, Feature::GENERATORS, n);
        }
        if n.is_async_function(compiler) {
            self.validate_feature(compiler, Feature::ASYNC_FUNCTIONS, n);
        }
        if n.is_async_function(compiler) && n.is_generator_function(compiler) {
            self.validate_feature(compiler, Feature::ASYNC_GENERATORS, n);
        }
    }

    // port: AstValidator#validateFunctionBody
    fn validate_function_body(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        no_block: bool,
    ) {
        if no_block {
            self.validate_node_type(compiler, Token::EMPTY, n);
        } else {
            self.validate_block(compiler, n);
        }
    }

    // port: AstValidator#validateParameters
    fn validate_parameters(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::PARAM_LIST, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            if cur.is_rest(compiler) {
                self.validate_rest_parameters(compiler, Token::PARAM_LIST, cur);
            } else if cur.is_default_value(compiler) {
                self.validate_feature(compiler, Feature::DEFAULT_PARAMETERS, cur);
                self.validate_default_value(compiler, Token::PARAM_LIST, cur);
            } else if cur.is_name(compiler) {
                self.validate_name(compiler, cur);
            } else if cur.is_array_pattern(compiler) {
                self.validate_array_pattern(compiler, Token::PARAM_LIST, cur);
            } else {
                self.validate_object_pattern(compiler, Token::PARAM_LIST, cur);
            }
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateDefaultValue
    fn validate_default_value(
        &mut self,
        compiler: &mut AbstractCompiler,
        context_type: Token,
        n: NodeId,
    ) {
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_lhs(compiler, context_type, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
    }

    // port: AstValidator#validateCall
    fn validate_call(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::CALL, n);
        self.validate_properties(compiler, n);
        self.validate_minimum_child_count(compiler, n, 1);

        let callee = n.get_first_child(compiler).unwrap();
        if callee.is_super(compiler) {
            self.validate_super(compiler, callee);
        } else {
            self.validate_expression(compiler, callee);
        }

        let mut c = callee.get_next(compiler);
        while let Some(cur) = c {
            self.validate_pseudo_expression(compiler, cur, &[Token::ITER_SPREAD]);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateOptChainCall
    fn validate_opt_chain_call(&mut self, compiler: &mut AbstractCompiler, node: NodeId) {
        self.validate_feature(compiler, Feature::OPTIONAL_CHAINING, node);
        self.validate_node_type(compiler, Token::OPTCHAIN_CALL, node);
        self.validate_properties(compiler, node);
        self.validate_minimum_child_count(compiler, node, 1);

        let callee = node.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, callee);

        let mut argument = callee.get_next(compiler);
        while let Some(cur) = argument {
            self.validate_pseudo_expression(compiler, cur, &[Token::ITER_SPREAD]);
            argument = cur.get_next(compiler);
        }
        self.validate_first_node_of_opt_chain(compiler, node);
    }

    // port: AstValidator#validateSuper
    fn validate_super(&mut self, compiler: &mut AbstractCompiler, super_node: NodeId) {
        self.validate_feature(compiler, Feature::SUPER, super_node);
        self.validate_properties(compiler, super_node);
        self.validate_childless(compiler, super_node);
        self.validate_type_information(compiler, super_node);
        let super_parent = super_node.get_parent(compiler).unwrap();
        let method_node = NodeUtil::get_enclosing_non_arrow_function(compiler, super_parent);

        if NodeUtil::is_normal_get(compiler, super_parent)
            && super_node.is_first_child_of(compiler, Some(super_parent))
        {
            // `super.prop` or `super['prop']`
            if !self.allows_super_property_reference(compiler, super_parent) {
                self.violation(
                    compiler,
                    "super property references are only allowed in methods, class static blocks \
                     and class fields.",
                    super_node,
                );
            }
        } else if super_parent.is_call(compiler)
            && super_node.is_first_child_of(compiler, Some(super_parent))
        {
            // super() constructor call
            if method_node.is_none_or(|m| !NodeUtil::is_es6_constructor(compiler, m)) {
                self.violation(
                    compiler,
                    "super constructor call is only allowed in a constructor method",
                    super_node,
                );
            } else {
                let extends_node = method_node
                    .unwrap()
                    .get_parent(compiler) // MEMBER_FUNCTION_DEF
                    .unwrap()
                    .get_parent(compiler) // CLASS_METHODS
                    .unwrap()
                    .get_parent(compiler) // CLASS
                    .unwrap()
                    .get_second_child(compiler) // extends clause
                    .unwrap();
                if extends_node.is_empty(compiler) {
                    self.violation(
                        compiler,
                        "super constructor call in a class that extends nothing",
                        super_node,
                    );
                }
            }
        } else {
            self.violation(compiler, "`super` is a syntax error here", super_node);
        }
    }

    // Check if a super property reference is in a method, class static block or class field.
    // port: AstValidator#allowsSuperPropertyReference
    #[allow(clippy::only_used_in_recursion)]
    fn allows_super_property_reference(&self, ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::SCRIPT => false,
            Token::MEMBER_FIELD_DEF | Token::COMPUTED_FIELD_DEF => true,
            Token::FUNCTION => {
                if NodeUtil::is_method_declaration(ast, n) {
                    return true;
                } else if !n.is_arrow_function(ast) {
                    return false;
                }
                self.allows_super_property_reference(ast, n.get_parent(ast).unwrap())
            }
            Token::BLOCK => {
                if NodeUtil::is_class_static_block(ast, n) {
                    return true;
                }
                self.allows_super_property_reference(ast, n.get_parent(ast).unwrap())
            }
            _ => self.allows_super_property_reference(ast, n.get_parent(ast).unwrap()),
        }
    }

    // port: AstValidator#validateRestParameters
    fn validate_rest_parameters(
        &mut self,
        compiler: &mut AbstractCompiler,
        context_type: Token,
        n: NodeId,
    ) {
        self.validate_feature(compiler, Feature::REST_PARAMETERS, n);
        self.validate_rest(compiler, context_type, n);
    }

    // port: AstValidator#validateArrayPatternRest
    fn validate_array_pattern_rest(
        &mut self,
        compiler: &mut AbstractCompiler,
        context_type: Token,
        n: NodeId,
    ) {
        self.validate_feature(compiler, Feature::ARRAY_PATTERN_REST, n);
        self.validate_rest(compiler, context_type, n);
    }

    // port: AstValidator#validateObjectPatternRest
    fn validate_object_pattern_rest(
        &mut self,
        compiler: &mut AbstractCompiler,
        context_type: Token,
        n: NodeId,
    ) {
        self.validate_feature(compiler, Feature::OBJECT_PATTERN_REST, n);
        self.validate_rest(compiler, context_type, n);
    }

    /// `context_type`: A [`Token`] constant value indicating that `n` should be validated
    /// appropriately for a descendant of a Node of this type.
    // port: AstValidator#validateRest
    fn validate_rest(&mut self, compiler: &mut AbstractCompiler, context_type: Token, n: NodeId) {
        match n.get_token(compiler) {
            Token::ITER_REST | Token::OBJECT_REST => {}
            _ => {
                self.violation(compiler, "Unexpected node type.", n);
                return;
            }
        }
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_lhs(compiler, context_type, first);
        if n.get_next(compiler).is_some() {
            self.violation(
                compiler,
                "Rest parameters must come after all other parameters.",
                n,
            );
        }
    }

    // port: AstValidator#validateObjectSpread
    fn validate_object_spread(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        self.validate_feature(compiler, Feature::OBJECT_LITERALS_WITH_SPREAD, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
    }

    // port: AstValidator#validateNew
    fn validate_new(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::NEW, n);
        self.validate_properties(compiler, n);
        self.validate_minimum_child_count(compiler, n, 1);

        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let mut c = n.get_second_child(compiler);
        while let Some(cur) = c {
            self.validate_pseudo_expression(compiler, cur, &[Token::ITER_SPREAD]);
            c = cur.get_next(compiler);
        }
    }

    /// `statement`: the enclosing statement. Will not always match the declaration Token.
    // port: AstValidator#validateNameDeclarationHelper
    fn validate_name_declaration_helper(
        &mut self,
        compiler: &mut AbstractCompiler,
        statement: NodeId,
        declaration: Token,
        n: NodeId,
    ) {
        self.validate_properties(compiler, n);
        self.validate_minimum_child_count(compiler, n, 1);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_name_declaration_child(compiler, statement, declaration, cur);
            c = cur.get_next(compiler);
        }
        if declaration == Token::LET {
            self.validate_feature(compiler, Feature::LET_DECLARATIONS, n);
        } else if declaration == Token::CONST {
            self.validate_feature(compiler, Feature::CONST_DECLARATIONS, n);
        }
    }

    // port: AstValidator#validateNameDeclarationChild
    fn validate_name_declaration_child(
        &mut self,
        compiler: &mut AbstractCompiler,
        statement: NodeId,
        declaration: Token,
        n: NodeId,
    ) {
        let in_enhanced_for = NodeUtil::is_enhanced_for(compiler, statement);
        let in_for_in = statement.is_for_in(compiler);
        let min_values: i32;
        let max_values: i32;
        if in_for_in && declaration == Token::VAR {
            // ECMASCRIPT5 sloppy mode allows for-in initializers.
            min_values = 0;
            max_values = 1;
        } else if in_enhanced_for {
            min_values = 0;
            max_values = 0;
        } else if n.is_destructuring_lhs(compiler) || declaration == Token::CONST {
            min_values = 1;
            max_values = 1;
        } else {
            min_values = 0;
            max_values = 1;
        }

        if n.is_name(compiler) {
            // Don't use validateName here since this NAME node may have a child.
            self.validate_non_empty_string(compiler, n);
            self.validate_properties(compiler, n);
            self.validate_child_count_in(compiler, n, min_values, max_values);
            if n.has_children(compiler) {
                let first = n.get_first_child(compiler).unwrap();
                self.validate_expression(compiler, first);
            }
        } else if n.is_destructuring_lhs(compiler) {
            self.validate_properties(compiler, n);
            self.validate_child_count_in(compiler, n, 1 + min_values, 1 + max_values);
            let c = n.get_first_child(compiler).unwrap();
            match c.get_token(compiler) {
                Token::ARRAY_PATTERN => self.validate_array_pattern(compiler, declaration, c),
                Token::OBJECT_PATTERN => self.validate_object_pattern(compiler, declaration, c),
                _ => {
                    let message =
                        format!("Invalid destructuring lhs first child for {declaration} node");
                    self.violation(compiler, &message, n);
                }
            }
            if n.has_two_children(compiler) {
                let second = n.get_second_child(compiler).unwrap();
                self.validate_expression(compiler, second);
            }
        } else {
            let message = format!("Invalid child for {declaration} node");
            self.violation(compiler, &message, n);
        }
    }

    /// `context_type`: A [`Token`] constant value indicating that `n` should be validated
    /// appropriately for a descendant of a Node of this type.
    // port: AstValidator#validateLHS
    fn validate_lhs(&mut self, compiler: &mut AbstractCompiler, context_type: Token, n: NodeId) {
        match n.get_token(compiler) {
            Token::NAME => self.validate_name(compiler, n),
            Token::ARRAY_PATTERN => self.validate_array_pattern(compiler, context_type, n),
            Token::OBJECT_PATTERN => self.validate_object_pattern(compiler, context_type, n),
            Token::GETPROP | Token::GETELEM => {
                self.validate_get_prop_get_elem_in_lhs(compiler, context_type, n)
            }
            Token::CAST => {
                let only = n.get_only_child(compiler);
                self.validate_lhs(compiler, context_type, only);
            }
            _ => {
                let message = format!("Invalid child for {context_type} node");
                self.violation(compiler, &message, n);
            }
        }
    }

    // port: AstValidator#validateGetPropGetElemInLHS
    fn validate_get_prop_get_elem_in_lhs(
        &mut self,
        compiler: &mut AbstractCompiler,
        context_type: Token,
        n: NodeId,
    ) {
        if context_type == Token::CONST
            || context_type == Token::LET
            || context_type == Token::VAR
            || context_type == Token::PARAM_LIST
        {
            let message = format!("Invalid child for {context_type} node");
            self.violation(compiler, &message, n);
            return;
        }
        match n.get_token(compiler) {
            Token::GETPROP => self.validate_get_prop(compiler, n),
            Token::GETELEM => self.validate_get_elem(compiler, n),
            _ => panic!(
                "Expected GETPROP or GETELEM but instead got node {}",
                n.get_token(compiler)
            ),
        }
    }

    // port: AstValidator#validateArrayPattern
    fn validate_array_pattern(&mut self, compiler: &mut AbstractCompiler, type_: Token, n: NodeId) {
        self.validate_feature(compiler, Feature::ARRAY_DESTRUCTURING, n);
        self.validate_node_type(compiler, Token::ARRAY_PATTERN, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            match cur.get_token(compiler) {
                Token::DEFAULT_VALUE => self.validate_default_value(compiler, type_, cur),
                Token::ITER_REST => self.validate_array_pattern_rest(compiler, type_, cur),
                Token::EMPTY => {
                    self.validate_properties(compiler, cur);
                    self.validate_childless(compiler, cur);
                }
                _ => self.validate_lhs(compiler, type_, cur),
            }
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateObjectPattern
    fn validate_object_pattern(
        &mut self,
        compiler: &mut AbstractCompiler,
        type_: Token,
        n: NodeId,
    ) {
        self.validate_feature(compiler, Feature::OBJECT_DESTRUCTURING, n);
        self.validate_node_type(compiler, Token::OBJECT_PATTERN, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            match cur.get_token(compiler) {
                Token::STRING_KEY => self.validate_object_pattern_string_key(compiler, type_, cur),
                Token::OBJECT_REST => self.validate_object_pattern_rest(compiler, type_, cur),
                Token::COMPUTED_PROP => {
                    self.validate_object_pattern_computed_prop_key(compiler, type_, cur)
                }
                _ => {
                    let message = format!("Invalid object pattern child for {type_} node");
                    self.violation(compiler, &message, n);
                }
            }
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateFor
    fn validate_for(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::FOR, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 4);
        let target = n.get_first_child(compiler).unwrap();
        if NodeUtil::is_name_declaration(compiler, Some(target)) {
            let token = target.get_token(compiler);
            self.validate_name_declaration_helper(compiler, n, token, target);
        } else {
            self.validate_pseudo_expression(compiler, target, &[Token::EMPTY]);
        }
        let second = n.get_second_child(compiler).unwrap();
        self.validate_pseudo_expression(compiler, second, &[Token::EMPTY]);
        let third = n.get_child_at_index(compiler, 2).unwrap();
        self.validate_pseudo_expression(compiler, third, &[Token::EMPTY]);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
    }

    // port: AstValidator#validateForIn
    fn validate_for_in(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::FOR_IN, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_enhanced_for_var_or_assignment_target(compiler, n, first);
        let second = n.get_second_child(compiler).unwrap();
        self.validate_expression(compiler, second);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
    }

    // port: AstValidator#validateForOf
    fn validate_for_of(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::FOR_OF, n);
        self.validate_node_type(compiler, Token::FOR_OF, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_enhanced_for_var_or_assignment_target(compiler, n, first);
        let second = n.get_second_child(compiler).unwrap();
        self.validate_expression(compiler, second);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
    }

    // port: AstValidator#validateForAwaitOf
    fn validate_for_await_of(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::FOR_AWAIT_OF, n);
        self.validate_node_type(compiler, Token::FOR_AWAIT_OF, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_enhanced_for_var_or_assignment_target(compiler, n, first);
        let second = n.get_second_child(compiler).unwrap();
        self.validate_expression(compiler, second);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
        self.validate_await_within_async_function(compiler, n);
    }

    // port: AstValidator#validateEnhancedForVarOrAssignmentTarget
    fn validate_enhanced_for_var_or_assignment_target(
        &mut self,
        compiler: &mut AbstractCompiler,
        for_node: NodeId,
        n: NodeId,
    ) {
        if NodeUtil::is_name_declaration(compiler, Some(n)) {
            // Only one NAME can be declared for FOR-IN and FOR_OF expressions.
            self.validate_properties(compiler, n);
            self.validate_child_count_expected(compiler, n, 1);
            let token = n.get_token(compiler);
            self.validate_name_declaration_helper(compiler, for_node, token, n);
        } else {
            let token = n.get_parent(compiler).unwrap().get_token(compiler);
            self.validate_lhs(compiler, token, n);
        }
    }

    // port: AstValidator#validateWith
    fn validate_with(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::WITH, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
    }

    // port: AstValidator#validateWhile
    fn validate_while(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::WHILE, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
    }

    // port: AstValidator#validateDo
    fn validate_do(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::DO, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_block(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
    }

    // port: AstValidator#validateIf
    fn validate_if(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::IF, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_in(compiler, n, 2, 3);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let second = n.get_second_child(compiler).unwrap();
        self.validate_block(compiler, second);
        if n.has_x_children(compiler, 3) {
            let last = n.get_last_child(compiler).unwrap();
            self.validate_block(compiler, last);
        }
    }

    // port: AstValidator#validateExprStmt
    fn validate_expr_stmt(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::EXPR_RESULT, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
    }

    // port: AstValidator#validateReturn
    fn validate_return(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::RETURN, n);
        self.validate_properties(compiler, n);
        self.validate_maximum_child_count(compiler, n, 1);
        if n.has_children(compiler) {
            let first = n.get_first_child(compiler).unwrap();
            self.validate_expression(compiler, first);
        }
    }

    // port: AstValidator#validateThrow
    fn validate_throw(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::THROW, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
    }

    // port: AstValidator#validateBreak
    fn validate_break(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::BREAK, n);
        self.validate_properties(compiler, n);
        self.validate_maximum_child_count(compiler, n, 1);
        if n.has_children(compiler) {
            let first = n.get_first_child(compiler).unwrap();
            self.validate_label_name(compiler, first);
        }
    }

    // port: AstValidator#validateContinue
    fn validate_continue(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::CONTINUE, n);
        self.validate_properties(compiler, n);
        self.validate_maximum_child_count(compiler, n, 1);
        if n.has_children(compiler) {
            let first = n.get_first_child(compiler).unwrap();
            self.validate_label_name(compiler, first);
        }
    }

    // port: AstValidator#validateTry
    fn validate_try(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::TRY, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_in(compiler, n, 2, 3);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_block(compiler, first);

        let mut seen_catch_or_finally = false;

        // Validate catch
        let catches = n.get_second_child(compiler).unwrap();
        self.validate_node_type(compiler, Token::BLOCK, catches);
        self.validate_properties(compiler, catches);
        self.validate_maximum_child_count(compiler, catches, 1);
        if catches.has_children(compiler) {
            let catch = catches.get_first_child(compiler).unwrap();
            self.validate_catch(compiler, catch);
            seen_catch_or_finally = true;
        }

        // Validate finally
        if n.has_x_children(compiler, 3) {
            let last = n.get_last_child(compiler).unwrap();
            self.validate_block(compiler, last);
            seen_catch_or_finally = true;
        }

        if !seen_catch_or_finally {
            self.violation(compiler, "Missing catch or finally for try statement.", n);
        }
    }

    // port: AstValidator#validateCatch
    fn validate_catch(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::CATCH, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let caught = n.get_first_child(compiler).unwrap();
        if caught.is_name(compiler) {
            self.validate_name(compiler, caught);
        } else if caught.is_array_pattern(compiler) {
            self.validate_array_pattern(compiler, Token::CATCH, caught);
        } else if caught.is_object_pattern(compiler) {
            self.validate_object_pattern(compiler, Token::CATCH, caught);
        } else if caught.is_empty(compiler) {
            self.validate_no_catch_binding(compiler, caught);
        } else {
            let message = format!("Unexpected catch binding: {}", caught.to_string(compiler));
            self.violation(compiler, &message, n);
        }
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
    }

    // port: AstValidator#validateNoCatchBinding
    fn validate_no_catch_binding(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::OPTIONAL_CATCH_BINDING, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
    }

    // port: AstValidator#validateSwitch
    fn validate_switch(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::SWITCH, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 2);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let second = n.get_second_child(compiler).unwrap();
        self.validate_node_type(compiler, Token::SWITCH_BODY, second);
        let cases = n.get_second_child(compiler).unwrap();
        let mut defaults = 0;
        let mut c = cases.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_switch_member(compiler, cur);
            if cur.is_default_case(compiler) {
                defaults += 1;
            }
            c = cur.get_next(compiler);
        }
        if defaults > 1 {
            let message = format!("Expected at most 1 'default' in switch but was {defaults}");
            self.violation(compiler, &message, n);
        }
    }

    // port: AstValidator#validateSwitchMember
    fn validate_switch_member(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::CASE => self.validate_case(compiler, n),
            Token::DEFAULT_CASE => self.validate_default_case(compiler, n),
            _ => {
                let message = format!("Expected switch member but was {}", n.get_token(compiler));
                self.violation(compiler, &message, n);
            }
        }
    }

    // port: AstValidator#validateDefaultCase
    fn validate_default_case(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::DEFAULT_CASE, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
    }

    // port: AstValidator#validateCase
    fn validate_case(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::CASE, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_block(compiler, last);
    }

    // port: AstValidator#validateChildless
    fn validate_childless(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_child_count_expected(compiler, n, 0);
    }

    // port: AstValidator#validateAssignmentExpression
    fn validate_assignment_expression(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let token = n.get_token(compiler);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_lhs(compiler, token, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
    }

    // port: AstValidator#validateCompoundAssignmentExpression
    fn validate_compound_assignment_expression(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) {
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let context_type = n.get_token(compiler);
        let lhs = n.get_first_child(compiler).unwrap();
        self.validate_assignment_op_target(compiler, lhs, context_type);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
    }

    /// Validates the lhs of a compound assignment op, inc, or dec
    ///
    /// This check is stricter than validateLhs.
    // port: AstValidator#validateAssignmentOpTarget
    fn validate_assignment_op_target(
        &mut self,
        compiler: &mut AbstractCompiler,
        lhs: NodeId,
        context_type: Token,
    ) {
        match lhs.get_token(compiler) {
            Token::NAME => self.validate_name(compiler, lhs),
            Token::GETPROP | Token::GETELEM => {
                self.validate_get_prop_get_elem_in_lhs(compiler, context_type, lhs)
            }
            Token::CAST => {
                self.validate_properties(compiler, lhs);
                self.validate_child_count_expected(compiler, lhs, 1);
                let first = lhs.get_first_child(compiler).unwrap();
                self.validate_assignment_op_target(compiler, first, context_type);
            }
            _ => {
                let message = format!("Invalid child for {context_type} node");
                self.violation(compiler, &message, lhs);
            }
        }
    }

    // port: AstValidator#validateGetElem
    fn validate_get_elem(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        check_argument!(n.is_get_elem(compiler), &n.to_string(compiler));
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 2);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_property_reference_target(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
    }

    // port: AstValidator#validateOptChainGetElem
    fn validate_opt_chain_get_elem(&mut self, compiler: &mut AbstractCompiler, node: NodeId) {
        self.validate_feature(compiler, Feature::OPTIONAL_CHAINING, node);
        check_argument!(
            node.is_opt_chain_get_elem(compiler),
            &node.to_string(compiler)
        );
        self.validate_properties(compiler, node);
        self.validate_child_count_expected(compiler, node, 2);
        let first = node.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let last = node.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
        self.validate_first_node_of_opt_chain(compiler, node);
    }

    // port: AstValidator#validateGetProp
    fn validate_get_prop(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::GETPROP, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_property_reference_target(compiler, first);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        self.validate_non_empty_string(compiler, n);
    }

    // port: AstValidator#validateOptChainGetProp
    fn validate_opt_chain_get_prop(&mut self, compiler: &mut AbstractCompiler, node: NodeId) {
        self.validate_feature(compiler, Feature::OPTIONAL_CHAINING, node);
        self.validate_node_type(compiler, Token::OPTCHAIN_GETPROP, node);
        let first = node.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        self.validate_first_node_of_opt_chain(compiler, node);
        self.validate_properties(compiler, node);
        self.validate_child_count(compiler, node);
        self.validate_non_empty_string(compiler, node);
    }

    // port: AstValidator#validatePropertyReferenceTarget
    fn validate_property_reference_target(
        &mut self,
        compiler: &mut AbstractCompiler,
        object_node: NodeId,
    ) {
        if object_node.is_super(compiler) {
            self.validate_super(compiler, object_node);
        } else {
            self.validate_expression(compiler, object_node);
        }
    }

    // port: AstValidator#validateRegExpLit
    fn validate_reg_exp_lit(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::REGEXP_SYNTAX, n);
        self.validate_node_type(compiler, Token::REGEXP, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_in(compiler, n, 1, 2);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_string_lit(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateStringLit
    fn validate_string_lit(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::STRINGLIT, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        // Validate that getString doesn't throw (Java catches UnsupportedOperationException; the
        // Rust Node panics like Java's exception).
        let ast: &Ast = compiler;
        if catch_unwind(AssertUnwindSafe(|| n.get_string(ast))).is_err() {
            self.violation(compiler, "Invalid STRING node.", n);
        }
    }

    // port: AstValidator#validateNumber
    fn validate_number(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::NUMBER, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        // Validate that getDouble doesn't throw (Java catches UnsupportedOperationException; the
        // Rust Node panics like Java's exception).
        let ast: &Ast = compiler;
        if catch_unwind(AssertUnwindSafe(|| n.get_double(ast))).is_err() {
            self.violation(compiler, "Invalid NUMBER node.", n);
        }
    }

    // port: AstValidator#validateBigInt
    fn validate_big_int(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::BIGINT, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        // Validate that getBigInt doesn't throw (Java catches UnsupportedOperationException; the
        // Rust Node panics like Java's exception).
        let ast: &Ast = compiler;
        if catch_unwind(AssertUnwindSafe(|| n.get_big_int(ast))).is_err() {
            self.violation(compiler, "Invalid BIGINT node.", n);
        }
    }

    // port: AstValidator#validateArrayLit
    fn validate_array_lit(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::ARRAYLIT, n);
        // Java's for loop over the children breaks after its first iteration.
        if let Some(c) = n.get_first_child(compiler) {
            // Array-literals may have empty slots.
            self.validate_pseudo_expression(compiler, c, &[Token::EMPTY, Token::ITER_SPREAD]);
        }
    }

    // port: AstValidator#validateObjectLit
    fn validate_object_lit(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::OBJECTLIT, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            self.validate_object_lit_key(compiler, cur);
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#validateObjectLitKey
    fn validate_object_lit_key(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::GETTER_DEF => self.validate_object_lit_get_key(compiler, n),
            Token::SETTER_DEF => self.validate_object_lit_set_key(compiler, n),
            Token::STRING_KEY => self.validate_object_lit_string_key(compiler, n),
            Token::MEMBER_FUNCTION_DEF => {
                self.validate_class_member(compiler, n, false);
                if n.is_static_member(compiler) {
                    self.violation(
                        compiler,
                        "Keys in an object literal should not be static.",
                        n,
                    );
                }
            }
            Token::COMPUTED_PROP => self.validate_object_lit_computed_prop_key(compiler, n),
            Token::OBJECT_SPREAD => self.validate_object_spread(compiler, n),
            _ => {
                let message = format!(
                    "Expected object literal key expression but was {}",
                    n.get_token(compiler)
                );
                self.violation(compiler, &message, n);
            }
        }
    }

    // port: AstValidator#validateObjectLitGetKey
    fn validate_object_lit_get_key(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::GETTER, n);
        self.validate_node_type(compiler, Token::GETTER_DEF, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        self.validate_object_literal_key_name(compiler, n);
        let function = n.get_first_child(compiler).unwrap();
        self.validate_function_expression(compiler, function);
        // objlit get functions must be nameless, and must have zero parameters.
        if !function
            .get_first_child(compiler)
            .unwrap()
            .get_string(compiler)
            .is_empty()
        {
            self.violation(compiler, "Expected unnamed function expression.", n);
        }
        let function_params = function.get_second_child(compiler).unwrap();
        if function_params.has_children(compiler) {
            self.violation(compiler, "get methods must not have parameters.", n);
        }
    }

    // port: AstValidator#validateObjectLitSetKey
    fn validate_object_lit_set_key(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::SETTER, n);
        self.validate_node_type(compiler, Token::SETTER_DEF, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        self.validate_object_literal_key_name(compiler, n);
        let function = n.get_first_child(compiler).unwrap();
        self.validate_function_expression(compiler, function);
        // objlit set functions must be nameless, and must have 1 parameter.
        if !function
            .get_first_child(compiler)
            .unwrap()
            .get_string(compiler)
            .is_empty()
        {
            self.violation(compiler, "Expected unnamed function expression.", n);
        }
        let function_params = function.get_second_child(compiler).unwrap();
        if !function_params.has_one_child(compiler) {
            self.violation(compiler, "set methods must have exactly one parameter.", n);
        }
    }

    // port: AstValidator#validateObjectLitStringKey
    fn validate_object_lit_string_key(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::STRING_KEY, n);
        self.validate_object_literal_key_name(compiler, n);

        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 1);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);

        if n.get_boolean_prop(compiler, NodeId::IS_SHORTHAND_PROPERTY) {
            self.validate_feature(compiler, Feature::SHORTHAND_OBJECT_PROPERTIES, n);
        }
    }

    // port: AstValidator#validateObjectPatternStringKey
    fn validate_object_pattern_string_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        type_: Token,
        n: NodeId,
    ) {
        self.validate_node_type(compiler, Token::STRING_KEY, n);
        self.validate_object_literal_key_name(compiler, n);
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 1);

        let c = n.get_first_child(compiler).unwrap();
        match c.get_token(compiler) {
            Token::DEFAULT_VALUE => self.validate_default_value(compiler, type_, c),
            _ => self.validate_lhs(compiler, type_, c),
        }
    }

    // port: AstValidator#validateObjectLitComputedPropKey
    fn validate_object_lit_computed_prop_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) {
        // TODO: b/499356003 - clean up this hack. The right fix is to:
        // 1) model computed prop getter/setters with a separate Feature from COMPUTED_PROPERTIES
        // 2) modify the transpilation passes to *not* remove the computed prop getter/setter in
        // object literals feature from the compiler featureset because it can't actually
        // transpile them. (The transpilation passes report a suppressible error instead).
        if !(n.get_is_in_closure_unaware_subtree(compiler)
            && NodeUtil::is_get_or_set_key(compiler, n))
        {
            self.validate_feature(compiler, Feature::COMPUTED_PROPERTIES, n);
        }
        self.validate_node_type(compiler, Token::COMPUTED_PROP, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
    }

    // port: AstValidator#validateObjectPatternComputedPropKey
    fn validate_object_pattern_computed_prop_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        type_: Token,
        n: NodeId,
    ) {
        self.validate_feature(compiler, Feature::COMPUTED_PROPERTIES, n);
        self.validate_node_type(compiler, Token::COMPUTED_PROP, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        if last.is_default_value(compiler) {
            self.validate_default_value(compiler, type_, last);
        } else {
            let token = last.get_token(compiler);
            self.validate_lhs(compiler, token, last);
        }
    }

    // port: AstValidator#validateComputedPropClassMethod
    fn validate_computed_prop_class_method(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_feature(compiler, Feature::COMPUTED_PROPERTIES, n);
        self.validate_node_type(compiler, Token::COMPUTED_PROP, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        if n.get_boolean_prop(compiler, NodeId::COMPUTED_PROP_VARIABLE) {
            self.validate_properties(compiler, n);
            self.validate_child_count_expected(compiler, n, 1);
        } else {
            self.validate_properties(compiler, n);
            self.validate_child_count_expected(compiler, n, 2);
            let last = n.get_last_child(compiler).unwrap();
            self.validate_function_expression(compiler, last);
            if n.get_boolean_prop(compiler, NodeId::COMPUTED_PROP_GETTER) {
                self.validate_object_lit_computed_prop_get_key(compiler, n);
                self.validate_feature(compiler, Feature::CLASS_GETTER_SETTER, n);
            } else if n.get_boolean_prop(compiler, NodeId::COMPUTED_PROP_SETTER) {
                self.validate_object_lit_computed_prop_set_key(compiler, n);
                self.validate_feature(compiler, Feature::CLASS_GETTER_SETTER, n);
            }
        }
    }

    // port: AstValidator#validateObjectLitComputedPropGetKey
    fn validate_object_lit_computed_prop_get_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) {
        self.validate_feature(compiler, Feature::COMPUTED_PROPERTIES, n);
        self.validate_feature(compiler, Feature::GETTER, n);
        self.validate_node_type(compiler, Token::COMPUTED_PROP, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let function = n.get_last_child(compiler).unwrap();
        self.validate_function_expression(compiler, function);
        // objlit get functions must be nameless, and must have zero parameters.
        if !function
            .get_first_child(compiler)
            .unwrap()
            .get_string(compiler)
            .is_empty()
        {
            self.violation(compiler, "Expected unnamed function expression.", n);
        }
        let function_params = function.get_second_child(compiler).unwrap();
        if function_params.has_children(compiler) {
            self.violation(compiler, "get methods must not have parameters.", n);
        }
    }

    // port: AstValidator#validateObjectLitComputedPropSetKey
    fn validate_object_lit_computed_prop_set_key(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) {
        self.validate_feature(compiler, Feature::COMPUTED_PROPERTIES, n);
        self.validate_feature(compiler, Feature::SETTER, n);
        self.validate_node_type(compiler, Token::COMPUTED_PROP, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let function = n.get_last_child(compiler).unwrap();
        self.validate_function_expression(compiler, function);
        // objlit set functions must be nameless, and must have 1 parameter.
        if !function
            .get_first_child(compiler)
            .unwrap()
            .get_string(compiler)
            .is_empty()
        {
            self.violation(compiler, "Expected unnamed function expression.", n);
        }
        let function_params = function.get_second_child(compiler).unwrap();
        if !function_params.has_one_child(compiler) {
            self.violation(compiler, "set methods must have exactly one parameter.", n);
        }
    }

    // port: AstValidator#validateObjectLiteralKeyName
    fn validate_object_literal_key_name(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.is_quoted_string_key(compiler) {
            // Validate that getString doesn't throw (Java catches UnsupportedOperationException;
            // the Rust Node panics like Java's exception).
            let ast: &Ast = compiler;
            if catch_unwind(AssertUnwindSafe(|| n.get_string(ast))).is_err() {
                let message = format!("getString failed for{}", n.get_token(compiler));
                self.violation(compiler, &message, n);
            }
        } else {
            self.validate_non_empty_string(compiler, n);
        }
    }

    // port: AstValidator#validateIncDecOp
    fn validate_inc_dec_op(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 1);
        let first = n.get_first_child(compiler).unwrap();
        let token = n.get_token(compiler);
        self.validate_assignment_op_target(compiler, first, token);
    }

    // port: AstValidator#validateUnaryOp
    fn validate_unary_op(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 1);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
    }

    // port: AstValidator#validateBinaryOp
    fn validate_binary_op(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 2);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
    }

    // port: AstValidator#validateTrinaryOp
    fn validate_trinary_op(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_properties(compiler, n);
        self.validate_child_count_expected(compiler, n, 3);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_expression(compiler, first);
        let second = first.get_next(compiler).unwrap();
        self.validate_expression(compiler, second);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_expression(compiler, last);
    }

    // port: AstValidator#validateNamedType
    fn validate_named_type(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::NAMED_TYPE, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_name(compiler, first);
    }

    // port: AstValidator#validateTypeAlias
    fn validate_type_alias(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::TYPE_ALIAS, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
    }

    // port: AstValidator#validateAmbientDeclaration
    fn validate_ambient_declaration(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.validate_node_type(compiler, Token::DECLARE, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_ambient_declaration_helper(compiler, first);
    }

    // port: AstValidator#validateAmbientDeclarationHelper
    fn validate_ambient_declaration_helper(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::VAR | Token::LET | Token::CONST => {
                let parent = n.get_parent(compiler).unwrap();
                let token = n.get_token(compiler);
                self.validate_name_declaration_helper(compiler, parent, token, n);
            }
            Token::FUNCTION => self.validate_function_signature(compiler, n),
            Token::CLASS => self.validate_class_declaration(compiler, n, true),
            Token::ENUM => self.validate_enum(compiler, n),
            Token::NAMESPACE => self.validate_namespace(compiler, n, true),
            Token::TYPE_ALIAS => self.validate_type_alias(compiler, n),
            Token::EXPORT => self.validate_export(compiler, n, true),
            _ => {}
        }
    }

    // port: AstValidator#validateNamespace
    fn validate_namespace(&mut self, compiler: &mut AbstractCompiler, n: NodeId, is_ambient: bool) {
        self.validate_node_type(compiler, Token::NAMESPACE, n);
        self.validate_properties(compiler, n);
        self.validate_child_count(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        self.validate_namespace_name(compiler, first);
        let last = n.get_last_child(compiler).unwrap();
        self.validate_namespace_elements(compiler, last, is_ambient);
    }

    // port: AstValidator#validateNamespaceName
    fn validate_namespace_name(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::NAME => self.validate_name(compiler, n),
            Token::GETPROP => self.validate_get_prop(compiler, n),
            _ => {}
        }
    }

    // port: AstValidator#validateNamespaceElements
    fn validate_namespace_elements(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        is_ambient: bool,
    ) {
        self.validate_node_type(compiler, Token::NAMESPACE_ELEMENTS, n);
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            if is_ambient {
                self.validate_ambient_declaration_helper(compiler, cur);
            } else {
                self.validate_statement(compiler, cur);
            }
            c = cur.get_next(compiler);
        }
    }

    // port: AstValidator#violation
    fn violation(&mut self, ast: &Ast, message: &str, n: impl Into<Option<NodeId>>) {
        self.violation_handler
            .handle_violation(ast, message, n.into());
    }

    // the first node of an opt chain must be marked with Prop.START_OF_OPT_CHAIN
    // port: AstValidator#validateFirstNodeOfOptChain
    fn validate_first_node_of_opt_chain(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        #[allow(clippy::collapsible_if)] // Retain Java control flow.
        if !NodeUtil::is_opt_chain_node(compiler, n.get_first_child(compiler).unwrap()) {
            // if the first child of an opt chain node is not an opt chain node then it is the
            // start of an opt chain
            if !n.is_optional_chain_start(compiler) {
                let message = format!(
                    "Start of optional chain node {} is not marked as the start.",
                    n.get_token(compiler)
                );
                self.violation(compiler, &message, n);
            }
        }
    }

    // port: AstValidator#validateNodeType
    fn validate_node_type(&mut self, compiler: &mut AbstractCompiler, type_: Token, n: NodeId) {
        if n.get_token(compiler) != type_ {
            let message = format!("Expected {type_} but was {}", n.get_token(compiler));
            self.violation(compiler, &message, n);
        }
    }

    // port: AstValidator#validateChildCount(Node)
    fn validate_child_count(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let expected_arity = Token::arity(n.get_token(compiler));
        if expected_arity != -1 {
            self.validate_child_count_expected(compiler, n, expected_arity);
        }
    }

    // port: AstValidator#validateChildCount(Node,int)
    fn validate_child_count_expected(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        expected: i32,
    ) {
        let count = n.get_child_count(compiler);
        if expected != count {
            let message = format!("Expected {expected} children, but was {count}");
            self.violation(compiler, &message, n);
        }
    }

    // port: AstValidator#validateChildCountIn
    fn validate_child_count_in(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        min: i32,
        max: i32,
    ) {
        if max == min {
            self.validate_child_count_expected(compiler, n, min);
            return;
        }
        let count = n.get_child_count(compiler);
        if count < min || count > max {
            let message = format!("Expected child count in [{min}, {max}], but was {count}");
            self.violation(compiler, &message, n);
        }
    }

    // port: AstValidator#validateMinimumChildCount
    fn validate_minimum_child_count(&mut self, compiler: &mut AbstractCompiler, n: NodeId, i: i32) {
        let valid = if i == 1 {
            n.has_children(compiler)
        } else if i == 2 {
            n.has_more_than_one_child(compiler)
        } else {
            n.get_child_count(compiler) >= i
        };

        if !valid {
            let message = format!(
                "Expected at least {i} children, but was {}",
                n.get_child_count(compiler)
            );
            self.violation(compiler, &message, n);
        }
    }

    // port: AstValidator#validateMaximumChildCount
    fn validate_maximum_child_count(&mut self, compiler: &mut AbstractCompiler, n: NodeId, i: i32) {
        let valid = if i == 1 {
            !n.has_more_than_one_child(compiler)
        } else if i == -1 {
            true // Varying number of children.
        } else {
            n.get_child_count(compiler) <= i
        };
        if !valid {
            let message = format!(
                "Expected no more than {i} children, but was {}",
                n.get_child_count(compiler)
            );
            self.violation(compiler, &message, n);
        }
    }

    // port: AstValidator#validateFeature
    fn validate_feature(&mut self, compiler: &mut AbstractCompiler, feature: Feature, n: NodeId) {
        let allowble_features = compiler.get_allowable_features();
        // Checks that feature present in the AST is recorded in the compiler's featureSet.
        if !n.is_from_externs(compiler) && !allowble_features.has(feature) {
            // Skip this check for externs because we don't need to complete transpilation on
            // externs, and currently only transpile externs so that we can typecheck ES6+
            // features in externs.
            let message = format!("AST should not contain {feature}");
            self.violation(compiler, &message, n);
        }
        // Note: currentScript may be null if someone called validateStatement or
        // validateExpression
        let Some(current_script) = self.current_script else {
            return;
        };
        if !self.is_script_feature_validation_enabled {
            return;
        }
        let script_features = NodeUtil::get_feature_set_of_script(compiler, current_script);
        // Checks that feature present in the AST is recorded in the SCRIPT node's featureSet.
        if script_features.is_none()
            || !NodeUtil::get_feature_set_of_script(compiler, current_script)
                .unwrap()
                .has(feature)
        {
            let message = format!("SCRIPT node should be marked as containing feature {feature}");
            self.violation(compiler, &message, current_script);
        }
    }

    // port: AstValidator#validateProperties
    fn validate_properties(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let mut error_messages: Vec<String> = Vec::new();
        n.validate_properties(compiler, |error_message| error_messages.push(error_message));
        // The consumer only lends the arena; report in the same order.
        for error_message in error_messages {
            self.violation(compiler, &error_message, n);
        }
    }

    // port: AstValidator#validateRequiredInlinings
    fn validate_required_inlinings(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if !self.should_validate_required_inlinings {
            return;
        }
        // Any node with JSDoc that says that it was required to be inlined is an error.
        let jsdoc_info = n.get_jsdoc_info(compiler);
        if jsdoc_info.is_some_and(|jsdoc_info| jsdoc_info.is_require_inlining()) {
            self.violation(compiler, "@requireInlining node failed to be inlined.", n);
        }
    }
}

/// Rust-only: the compiler's JSTypeRegistry with the arena (Java reads types through the node).
fn type_registry(compiler: &mut AbstractCompiler) -> (&mut JSTypeRegistry, &Ast) {
    let (reg, ast) = compiler.get_type_registry_field_and_ast();
    (
        reg.expect("AstValidator: a node has a JSType but the compiler has no JSTypeRegistry"),
        ast,
    )
}

impl CompilerPass for AstValidator<'_> {
    // port: AstValidator#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Java checks externs != null and root != null; NodeId is never null.
        self.validate_code_root(compiler, externs);
        self.validate_code_root(compiler, root);
    }
}
