/*
 * Copyright 2026 The closure-rs Authors.
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

// Java declarations retain enum constant names and the original constructor argument order.
#![allow(clippy::upper_case_acronyms, clippy::too_many_arguments)]

use crate::parser::{
    token::Token,
    token_type::TokenType,
    util::{SourcePosition, SourceRange},
};

pub mod argument_list_tree;
pub mod array_literal_expression_tree;
pub mod array_pattern_tree;
pub mod await_expression_tree;
pub mod binary_operator_tree;
pub mod block_tree;
pub mod break_statement_tree;
pub mod call_expression_tree;
pub mod case_clause_tree;
pub mod catch_tree;
pub mod class_declaration_tree;
pub mod comma_expression_tree;
pub mod comment;
pub mod comprehension_for_tree;
pub mod comprehension_if_tree;
pub mod comprehension_tree;
pub mod computed_property_definition_tree;
pub mod computed_property_field_tree;
pub mod computed_property_getter_tree;
pub mod computed_property_method_tree;
pub mod computed_property_setter_tree;
pub mod conditional_expression_tree;
pub mod continue_statement_tree;
pub mod debugger_statement_tree;
pub mod default_clause_tree;
pub mod default_parameter_tree;
pub mod do_while_statement_tree;
pub mod dynamic_import_tree;
pub mod empty_statement_tree;
pub mod export_declaration_tree;
pub mod export_specifier_tree;
pub mod expression_statement_tree;
pub mod field_declaration_tree;
pub mod finally_tree;
pub mod for_await_of_statement_tree;
pub mod for_in_statement_tree;
pub mod for_of_statement_tree;
pub mod for_statement_tree;
pub mod formal_parameter_list_tree;
pub mod function_declaration_tree;
pub mod get_accessor_tree;
pub mod identifier_expression_tree;
pub mod if_statement_tree;
pub mod import_declaration_tree;
pub mod import_meta_expression_tree;
pub mod import_specifier_tree;
pub mod iter_rest_tree;
pub mod iter_spread_tree;
pub mod labelled_statement_tree;
pub mod literal_expression_tree;
pub mod member_expression_tree;
pub mod member_lookup_expression_tree;
pub mod missing_primary_expression_tree;
pub mod new_expression_tree;
pub mod new_target_expression_tree;
pub mod null_tree;
pub mod object_literal_expression_tree;
pub mod object_pattern_tree;
pub mod object_rest_tree;
pub mod object_spread_tree;
pub mod opt_chain_call_expression_tree;
pub mod optional_member_expression_tree;
pub mod optional_member_lookup_expression_tree;
pub mod paren_expression_tree;
pub mod parse_tree;
pub mod parse_tree_type;
pub mod program_tree;
pub mod property_name_assignment_tree;
pub mod return_statement_tree;
pub mod set_accessor_tree;
pub mod super_expression_tree;
pub mod switch_statement_tree;
pub mod template_literal_expression_tree;
pub mod template_literal_portion_tree;
pub mod template_substitution_tree;
pub mod this_expression_tree;
pub mod throw_statement_tree;
pub mod try_statement_tree;
pub mod unary_expression_tree;
pub mod update_expression_tree;
pub mod variable_declaration_list_tree;
pub mod variable_declaration_tree;
pub mod variable_statement_tree;
pub mod while_statement_tree;
pub mod with_statement_tree;
pub mod yield_expression_tree;

pub use argument_list_tree::*;
pub use array_literal_expression_tree::*;
pub use array_pattern_tree::*;
pub use await_expression_tree::*;
pub use binary_operator_tree::*;
pub use block_tree::*;
pub use break_statement_tree::*;
pub use call_expression_tree::*;
pub use case_clause_tree::*;
pub use catch_tree::*;
pub use class_declaration_tree::*;
pub use comma_expression_tree::*;
pub use comment::Comment;
pub use comprehension_for_tree::*;
pub use comprehension_if_tree::*;
pub use comprehension_tree::ComprehensionTree;
pub use computed_property_definition_tree::*;
pub use computed_property_field_tree::*;
pub use computed_property_getter_tree::*;
pub use computed_property_method_tree::*;
pub use computed_property_setter_tree::*;
pub use conditional_expression_tree::*;
pub use continue_statement_tree::*;
pub use debugger_statement_tree::*;
pub use default_clause_tree::*;
pub use default_parameter_tree::*;
pub use do_while_statement_tree::*;
pub use dynamic_import_tree::*;
pub use empty_statement_tree::*;
pub use export_declaration_tree::*;
pub use export_specifier_tree::*;
pub use expression_statement_tree::*;
pub use field_declaration_tree::*;
pub use finally_tree::*;
pub use for_await_of_statement_tree::*;
pub use for_in_statement_tree::*;
pub use for_of_statement_tree::*;
pub use for_statement_tree::*;
pub use formal_parameter_list_tree::*;
pub use function_declaration_tree::FunctionDeclarationTree;
pub use get_accessor_tree::*;
pub use identifier_expression_tree::*;
pub use if_statement_tree::*;
pub use import_declaration_tree::*;
pub use import_meta_expression_tree::*;
pub use import_specifier_tree::*;
pub use iter_rest_tree::*;
pub use iter_spread_tree::*;
pub use labelled_statement_tree::*;
pub use literal_expression_tree::*;
pub use member_expression_tree::*;
pub use member_lookup_expression_tree::*;
pub use missing_primary_expression_tree::*;
pub use new_expression_tree::*;
pub use new_target_expression_tree::*;
pub use null_tree::*;
pub use object_literal_expression_tree::*;
pub use object_pattern_tree::*;
pub use object_rest_tree::*;
pub use object_spread_tree::*;
pub use opt_chain_call_expression_tree::*;
pub use optional_member_expression_tree::*;
pub use optional_member_lookup_expression_tree::*;
pub use paren_expression_tree::*;
pub use parse_tree::*;
pub use parse_tree_type::*;
pub use program_tree::*;
pub use property_name_assignment_tree::*;
pub use return_statement_tree::*;
pub use set_accessor_tree::*;
pub use super_expression_tree::*;
pub use switch_statement_tree::*;
pub use template_literal_expression_tree::*;
pub use template_literal_portion_tree::*;
pub use template_substitution_tree::*;
pub use this_expression_tree::*;
pub use throw_statement_tree::*;
pub use try_statement_tree::*;
pub use unary_expression_tree::*;
pub use update_expression_tree::UpdateExpressionTree;
pub use variable_declaration_list_tree::*;
pub use variable_declaration_tree::*;
pub use variable_statement_tree::*;
pub use while_statement_tree::*;
pub use with_statement_tree::*;
pub use yield_expression_tree::*;
