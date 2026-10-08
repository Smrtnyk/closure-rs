/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/FeatureCollector.java.

//! Port of com.google.javascript.jscomp.parsing.FeatureCollector.
use crate::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_state,
    node::{Ast, NodeId},
    token::Token,
};

pub struct FeatureCollector {
    script_features: FeatureSet,
}
impl Default for FeatureCollector {
    // port: FeatureCollector#<init>
    fn default() -> Self {
        Self {
            script_features: FeatureSet::BARE_MINIMUM,
        }
    }
}
impl FeatureCollector {
    // port: FeatureCollector#allFeatures
    pub fn all_features(&self) -> FeatureSet {
        self.script_features
    }
    // port: FeatureCollector#visitScript
    pub fn visit_script(&mut self, ast: &Ast, script: NodeId) {
        let mut empty_context = vec![FeatureContext::NONE];
        self.visit_recursive(ast, &mut empty_context, script);
    }
    // port: FeatureCollector#visitRecursive
    fn visit_recursive(
        &mut self,
        ast: &Ast,
        context_stack: &mut Vec<FeatureContext>,
        node: NodeId,
    ) {
        let new_context = self.visit_single_node(ast, context_stack, node);
        context_stack.push(new_context);
        for child in node.children(ast) {
            self.visit_recursive(ast, context_stack, child);
        }
        context_stack.pop();
    }
    // port: FeatureCollector#visitSingleNode
    pub fn visit_single_node(
        &mut self,
        ast: &Ast,
        context_stack: &[FeatureContext],
        node: NodeId,
    ) -> FeatureContext {
        check_state!(
            !context_stack.is_empty(),
            "The provided feature context stack was empty. It must have at least one element."
        );
        self.record_script_features(ast, context_stack, node);
        Self::context_for(ast, context_stack, node)
    }
    // port: FeatureCollector#recordScriptFeatures
    #[allow(clippy::collapsible_match)] // Keep Java switch/if control flow.
    fn record_script_features(
        &mut self,
        ast: &Ast,
        context_stack: &[FeatureContext],
        node: NodeId,
    ) {
        use Feature::*;
        let parent_context = *context_stack.last().unwrap();
        match node.get_token(ast) {
            Token::FUNCTION => {
                if node.is_async_generator_function(ast) {
                    self.add_script_feature(ASYNC_GENERATORS);
                }
                if node.is_arrow_function(ast) {
                    self.add_script_feature(ARROW_FUNCTIONS);
                }
                if node.is_async_function(ast) {
                    self.add_script_feature(ASYNC_FUNCTIONS);
                }
                if node.is_generator_function(ast) {
                    self.add_script_feature(GENERATORS);
                }
                if parent_context == FeatureContext::BLOCK_SCOPE {
                    self.script_features =
                        self.script_features.with(BLOCK_SCOPED_FUNCTION_DECLARATION);
                }
            }
            Token::PARAM_LIST | Token::CALL | Token::NEW => {}
            Token::STRING_KEY => {
                if node.is_shorthand_property(ast)
                    && parent_context == FeatureContext::OBJECT_LITERAL
                {
                    self.add_script_feature(SHORTHAND_OBJECT_PROPERTIES);
                }
            }
            Token::DEFAULT_VALUE => {
                if parent_context == FeatureContext::PARAM_LIST {
                    self.add_script_feature(DEFAULT_PARAMETERS);
                }
            }
            Token::GETTER_DEF => {
                self.add_script_feature(GETTER);
                if parent_context == FeatureContext::CLASS_MEMBERS {
                    self.add_script_feature(CLASS_GETTER_SETTER);
                }
                if node.is_private_identifier(ast) {
                    self.add_script_feature(PRIVATE_ELEMENTS);
                }
            }
            Token::REGEXP => self.add_script_feature(REGEXP_SYNTAX),
            Token::SETTER_DEF => {
                self.add_script_feature(SETTER);
                if parent_context == FeatureContext::CLASS_MEMBERS {
                    self.add_script_feature(CLASS_GETTER_SETTER);
                }
                if node.is_private_identifier(ast) {
                    self.add_script_feature(PRIVATE_ELEMENTS);
                }
            }
            Token::BLOCK => {
                if parent_context == FeatureContext::CLASS_MEMBERS {
                    self.add_script_feature(CLASS_STATIC_BLOCK);
                }
            }
            Token::EMPTY => {
                if parent_context == FeatureContext::CATCH {
                    self.add_script_feature(OPTIONAL_CATCH_BINDING);
                }
            }
            Token::ITER_REST => {
                self.add_script_feature(if parent_context == FeatureContext::PARAM_LIST {
                    REST_PARAMETERS
                } else {
                    ARRAY_PATTERN_REST
                })
            }
            Token::ITER_SPREAD => self.add_script_feature(SPREAD_EXPRESSIONS),
            Token::OBJECT_REST => self.add_script_feature(OBJECT_PATTERN_REST),
            Token::OBJECT_SPREAD => self.add_script_feature(OBJECT_LITERALS_WITH_SPREAD),
            Token::BIGINT => self.add_script_feature(BIGINT),
            Token::EXPONENT | Token::ASSIGN_EXPONENT => self.add_script_feature(EXPONENT_OP),
            Token::TAGGED_TEMPLATELIT | Token::TEMPLATELIT => {
                self.add_script_feature(TEMPLATE_LITERALS)
            }
            Token::NEW_TARGET => self.add_script_feature(NEW_TARGET),
            Token::COMPUTED_PROP => {
                self.add_script_feature(COMPUTED_PROPERTIES);
                let is_getter = node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_GETTER);
                let is_setter = node.get_boolean_prop(ast, NodeId::COMPUTED_PROP_SETTER);
                let is_class_member = parent_context == FeatureContext::CLASS_MEMBERS;
                if is_getter {
                    self.add_script_feature(GETTER);
                } else if is_setter {
                    self.add_script_feature(SETTER);
                }
                if (is_getter || is_setter) && is_class_member {
                    self.add_script_feature(CLASS_GETTER_SETTER);
                }
            }
            Token::OPTCHAIN_GETPROP | Token::OPTCHAIN_CALL | Token::OPTCHAIN_GETELEM => {
                self.add_script_feature(OPTIONAL_CHAINING)
            }
            Token::COALESCE => self.add_script_feature(NULL_COALESCE_OP),
            Token::DYNAMIC_IMPORT => self.add_script_feature(DYNAMIC_IMPORT),
            Token::ASSIGN_OR | Token::ASSIGN_AND => self.add_script_feature(LOGICAL_ASSIGNMENT),
            Token::ASSIGN_COALESCE => {
                self.add_script_feature(NULL_COALESCE_OP);
                self.add_script_feature(LOGICAL_ASSIGNMENT);
            }
            Token::FOR_OF => self.add_script_feature(FOR_OF),
            Token::FOR_AWAIT_OF => {
                self.add_script_feature(FOR_AWAIT_OF);
                self.maybe_add_top_level_await(context_stack);
            }
            Token::IMPORT | Token::EXPORT => self.add_script_feature(MODULES),
            Token::CONST => self.add_script_feature(CONST_DECLARATIONS),
            Token::LET => self.add_script_feature(LET_DECLARATIONS),
            Token::CLASS => self.add_script_feature(CLASSES),
            Token::MEMBER_FUNCTION_DEF => {
                self.add_script_feature(MEMBER_DECLARATIONS);
                if node.is_private_identifier(ast) {
                    self.add_script_feature(PRIVATE_ELEMENTS);
                }
            }
            Token::MEMBER_FIELD_DEF => {
                self.add_script_feature(if node.is_private_identifier(ast) {
                    PRIVATE_ELEMENTS
                } else {
                    PUBLIC_CLASS_FIELDS
                })
            }
            Token::IN => {
                if node
                    .get_first_child(ast)
                    .is_some_and(|c| c.is_private_identifier(ast))
                {
                    self.add_script_feature(PRIVATE_ELEMENTS);
                }
            }
            Token::COMPUTED_FIELD_DEF => self.add_script_feature(PUBLIC_CLASS_FIELDS),
            Token::SUPER => self.add_script_feature(SUPER),
            Token::ARRAY_PATTERN => self.add_script_feature(ARRAY_DESTRUCTURING),
            Token::OBJECT_PATTERN => self.add_script_feature(OBJECT_DESTRUCTURING),
            Token::AWAIT => {
                self.add_script_feature(ASYNC_FUNCTIONS);
                self.maybe_add_top_level_await(context_stack);
            }
            _ => {}
        }
    }
    // port: FeatureCollector#maybeAddTopLevelAwait
    fn maybe_add_top_level_await(&mut self, context_stack: &[FeatureContext]) {
        if !context_stack.contains(&FeatureContext::FUNCTION) {
            self.add_script_feature(Feature::TOP_LEVEL_AWAIT);
        }
    }
    // port: FeatureCollector#addScriptFeature
    fn add_script_feature(&mut self, x: Feature) {
        self.script_features = self.script_features.with(x);
    }
    // port: FeatureCollector#contextFor
    fn context_for(ast: &Ast, context_stack: &[FeatureContext], node: NodeId) -> FeatureContext {
        match node.get_token(ast) {
            Token::PARAM_LIST => FeatureContext::PARAM_LIST,
            Token::CLASS_MEMBERS => FeatureContext::CLASS_MEMBERS,
            Token::CLASS => FeatureContext::CLASS,
            Token::CATCH => FeatureContext::CATCH,
            Token::BLOCK => {
                if context_stack.last() == Some(&FeatureContext::FUNCTION) {
                    FeatureContext::NONE
                } else {
                    FeatureContext::BLOCK_SCOPE
                }
            }
            Token::FUNCTION => FeatureContext::FUNCTION,
            Token::OBJECTLIT => FeatureContext::OBJECT_LITERAL,
            _ => FeatureContext::NONE,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureContext {
    PARAM_LIST,
    CLASS_MEMBERS,
    CLASS,
    CATCH,
    BLOCK_SCOPE,
    FUNCTION,
    OBJECT_LITERAL,
    NONE,
}
