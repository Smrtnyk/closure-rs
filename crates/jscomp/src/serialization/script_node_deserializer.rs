/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/ScriptNodeDeserializer.java.

//! Port of serialization/ScriptNodeDeserializer.java.
use super::color_pool::ShardView;
use super::jsdoc_serializer::JSDocSerializer;
use super::malformed_typed_ast_exception::MalformedTypedAstException;
use super::protobuf::{CodedInputStream, Message};
use super::string_pool::StringPool;
use super::typed_ast_proto::{AstNode, LazyAst, NodeKind, NodeProperty};
use crate::source_file::SourceFile;
use closure_parsing::feature_collector::{FeatureCollector, FeatureContext};
use closure_rhino::ir::IR;
use closure_rhino::node::{Ast, NodeId, ObjectProp};
use closure_rhino::static_source_file::StaticSourceFile;
use closure_rhino::token::Token;
use num_bigint::BigInt;
use std::sync::Arc;

/// port: ScriptNodeDeserializer
///
/// Class that deserializes an AstNode-tree representing a SCRIPT into a Node-tree.
///
/// This process depends on other information from the TypedAST format, but the output it limited
/// to only a single SCRIPT. The other deserialized content must be provided beforehand.
pub struct ScriptNodeDeserializer {
    source_file: Arc<SourceFile>,
    script_bytes: Arc<[u8]>,
    source_mapping_url: String,
    color_pool_shard: Option<Arc<ShardView>>,
    string_pool: Arc<StringPool>,
    file_pool: Arc<Vec<Arc<SourceFile>>>,
}

impl ScriptNodeDeserializer {
    // port: ScriptNodeDeserializer#<init>
    pub fn new(
        ast: &LazyAst,
        string_pool: Arc<StringPool>,
        color_pool_shard: Option<Arc<ShardView>>,
        file_pool: Arc<Vec<Arc<SourceFile>>>,
    ) -> Self {
        Self {
            script_bytes: Arc::from(ast.get_script()),
            source_file: file_pool[(ast.get_source_file() - 1) as usize].clone(),
            source_mapping_url: ast.get_source_mapping_url().to_string(),
            color_pool_shard,
            string_pool,
            file_pool,
        }
    }

    // port: ScriptNodeDeserializer#deserializeNew
    pub fn deserialize_new(&self, ast: &mut Ast) -> NodeId {
        Runner {
            owner: self,
            feature_collector: FeatureCollector::default(),
            previous_line: 0,
            previous_column: 0,
        }
        .run(ast)
    }

    // port: ScriptNodeDeserializer#getSourceMappingURL
    pub fn get_source_mapping_url(&self) -> &str {
        &self.source_mapping_url
    }

    // port: ScriptNodeDeserializer#getSourceFile
    pub fn get_source_file(&self) -> &Arc<SourceFile> {
        &self.source_file
    }

    /// Create a template node to use as a source of common attributes.
    ///
    /// This allows the prop structure to be shared among all the node from this source file.
    /// This reduces the cost of these properties to O(nodes) to O(files).
    // port: ScriptNodeDeserializer#createSourceInfoTemplate
    fn create_source_info_template(&self, ast: &mut Ast, file: &Arc<SourceFile>) -> NodeId {
        // The Node type choice is arbitrary.
        let source_info_template = ast.new_node(Token::SCRIPT);
        let file: Arc<dyn StaticSourceFile> = file.clone();
        source_info_template.set_static_source_file(ast, Some(file));
        source_info_template
    }

    // port: ScriptNodeDeserializer#setOriginalNameIfPresent
    fn set_original_name_if_present(&self, ast: &mut Ast, ast_node: &AstNode, n: NodeId) {
        if ast_node.get_original_name_pointer() != 0 {
            n.set_original_name_from_string_pool(
                ast,
                self.string_pool.get_interned_strings(),
                ast_node.get_original_name_pointer(),
            );
        }
    }

    /// Creates a new string node with the given token & string value of the AstNode
    ///
    /// Prefer calling this method over calling a regular Node.* or IR.* method when possible.
    /// This method integrates with `RhinoStringPool` to cache String interning results.
    // port: ScriptNodeDeserializer#stringNode
    fn string_node(
        &self,
        ast: &mut Ast,
        token: Token,
        n: &AstNode,
        template_node: NodeId,
    ) -> NodeId {
        let str = ast.new_string_from_string_pool(
            token,
            self.string_pool.get_interned_strings(),
            n.get_string_value_pointer(),
        );
        str.clone_props_from(ast, template_node);
        str
    }

    // port: ScriptNodeDeserializer#newNodeFromTemplate
    fn new_node_from_template(&self, ast: &mut Ast, token: Token, template_node: NodeId) -> NodeId {
        let n = template_node.clone_tree(ast);
        n.set_token(ast, token);
        n
    }

    // port: ScriptNodeDeserializer#deserializeSingleNode
    fn deserialize_single_node(&self, ast: &mut Ast, n: &AstNode, template_node: NodeId) -> NodeId {
        match n.get_kind() {
            NodeKind::SOURCE_FILE => self.new_node_from_template(ast, Token::SCRIPT, template_node),

            NodeKind::NUMBER_LITERAL => {
                let number_node = IR::number(ast, n.get_double_value());
                number_node.clone_props_from(ast, template_node);
                number_node
            }
            NodeKind::BIGINT_LITERAL => {
                let bigint_string = self.string_pool.get(n.get_string_value_pointer());
                let bi = IR::bigint(
                    ast,
                    BigInt::parse_bytes(bigint_string.to_string_lossy().as_bytes(), 10)
                        .unwrap_or_else(|| {
                            panic!(
                                "java.lang.NumberFormatException: For input string: \"{}\"",
                                bigint_string.to_string_lossy()
                            )
                        }),
                );
                bi.clone_props_from(ast, template_node);
                bi
            }
            NodeKind::POST_INCREMENT => {
                let post_inc = self.new_node_from_template(ast, Token::INC, template_node);
                post_inc.put_boolean_prop(ast, NodeId::INCRDECR_PROP, true);
                post_inc
            }
            NodeKind::POST_DECREMENT => {
                let post_dec = self.new_node_from_template(ast, Token::DEC, template_node);
                post_dec.put_boolean_prop(ast, NodeId::INCRDECR_PROP, true);
                post_dec
            }
            NodeKind::TEMPLATELIT_STRING => {
                let template_string_value = n.get_template_string_value();
                let template_lit_string = ast.new_template_lit_string_from_string_pool(
                    self.string_pool.get_interned_strings(),
                    template_string_value.get_cooked_string_pointer(),
                    template_string_value.get_raw_string_pointer(),
                );
                template_lit_string.clone_props_from(ast, template_node);
                template_lit_string
            }
            NodeKind::QUOTED_STRING_KEY => {
                let quoted_string_key = self.string_node(ast, Token::STRING_KEY, n, template_node);
                quoted_string_key.set_quoted_string_key(ast);
                quoted_string_key
            }
            NodeKind::RENAMABLE_GETTER_DEF | NodeKind::QUOTED_GETTER_DEF => {
                let getter_def = self.string_node(ast, Token::GETTER_DEF, n, template_node);
                if n.get_kind() == NodeKind::QUOTED_GETTER_DEF {
                    getter_def.set_quoted_string_key(ast);
                }
                getter_def
            }
            NodeKind::RENAMABLE_SETTER_DEF | NodeKind::QUOTED_SETTER_DEF => {
                let setter_def = self.string_node(ast, Token::SETTER_DEF, n, template_node);
                if n.get_kind() == NodeKind::QUOTED_SETTER_DEF {
                    setter_def.set_quoted_string_key(ast);
                }
                setter_def
            }
            NodeKind::STRING_LITERAL => self.string_node(ast, Token::STRINGLIT, n, template_node),
            NodeKind::IDENTIFIER => self.string_node(ast, Token::NAME, n, template_node),
            NodeKind::FALSE => self.new_node_from_template(ast, Token::FALSE, template_node),
            NodeKind::TRUE => self.new_node_from_template(ast, Token::TRUE, template_node),
            NodeKind::NULL => self.new_node_from_template(ast, Token::NULL, template_node),
            NodeKind::THIS => self.new_node_from_template(ast, Token::THIS, template_node),
            NodeKind::VOID => self.new_node_from_template(ast, Token::VOID, template_node),
            NodeKind::REGEX_LITERAL => {
                self.new_node_from_template(ast, Token::REGEXP, template_node)
            }
            NodeKind::ARRAY_LITERAL => {
                self.new_node_from_template(ast, Token::ARRAYLIT, template_node)
            }
            NodeKind::OBJECT_LITERAL => {
                self.new_node_from_template(ast, Token::OBJECTLIT, template_node)
            }
            NodeKind::ASSIGNMENT => self.new_node_from_template(ast, Token::ASSIGN, template_node),
            NodeKind::CALL => self.new_node_from_template(ast, Token::CALL, template_node),
            NodeKind::NEW => self.new_node_from_template(ast, Token::NEW, template_node),
            NodeKind::PROPERTY_ACCESS => self.string_node(ast, Token::GETPROP, n, template_node),
            NodeKind::ELEMENT_ACCESS => {
                self.new_node_from_template(ast, Token::GETELEM, template_node)
            }
            NodeKind::COMMA => self.new_node_from_template(ast, Token::COMMA, template_node),
            NodeKind::BOOLEAN_OR => self.new_node_from_template(ast, Token::OR, template_node),
            NodeKind::BOOLEAN_AND => self.new_node_from_template(ast, Token::AND, template_node),
            NodeKind::HOOK => self.new_node_from_template(ast, Token::HOOK, template_node),
            NodeKind::EQUAL => self.new_node_from_template(ast, Token::EQ, template_node),
            NodeKind::NOT_EQUAL => self.new_node_from_template(ast, Token::NE, template_node),
            NodeKind::LESS_THAN => self.new_node_from_template(ast, Token::LT, template_node),
            NodeKind::LESS_THAN_EQUAL => self.new_node_from_template(ast, Token::LE, template_node),
            NodeKind::GREATER_THAN => self.new_node_from_template(ast, Token::GT, template_node),
            NodeKind::GREATER_THAN_EQUAL => {
                self.new_node_from_template(ast, Token::GE, template_node)
            }
            NodeKind::TRIPLE_EQUAL => self.new_node_from_template(ast, Token::SHEQ, template_node),
            NodeKind::NOT_TRIPLE_EQUAL => {
                self.new_node_from_template(ast, Token::SHNE, template_node)
            }
            NodeKind::NOT => self.new_node_from_template(ast, Token::NOT, template_node),
            NodeKind::POSITIVE => self.new_node_from_template(ast, Token::POS, template_node),
            NodeKind::NEGATIVE => self.new_node_from_template(ast, Token::NEG, template_node),
            NodeKind::TYPEOF => self.new_node_from_template(ast, Token::TYPEOF, template_node),
            NodeKind::INSTANCEOF => {
                self.new_node_from_template(ast, Token::INSTANCEOF, template_node)
            }
            NodeKind::IN => self.new_node_from_template(ast, Token::IN, template_node),
            NodeKind::ADD => self.new_node_from_template(ast, Token::ADD, template_node),
            NodeKind::SUBTRACT => self.new_node_from_template(ast, Token::SUB, template_node),
            NodeKind::MULTIPLY => self.new_node_from_template(ast, Token::MUL, template_node),
            NodeKind::DIVIDE => self.new_node_from_template(ast, Token::DIV, template_node),
            NodeKind::MODULO => self.new_node_from_template(ast, Token::MOD, template_node),
            NodeKind::EXPONENT => self.new_node_from_template(ast, Token::EXPONENT, template_node),
            NodeKind::BITWISE_NOT => self.new_node_from_template(ast, Token::BITNOT, template_node),
            NodeKind::BITWISE_OR => self.new_node_from_template(ast, Token::BITOR, template_node),
            NodeKind::BITWISE_AND => self.new_node_from_template(ast, Token::BITAND, template_node),
            NodeKind::BITWISE_XOR => self.new_node_from_template(ast, Token::BITXOR, template_node),
            NodeKind::LEFT_SHIFT => self.new_node_from_template(ast, Token::LSH, template_node),
            NodeKind::RIGHT_SHIFT => self.new_node_from_template(ast, Token::RSH, template_node),
            NodeKind::UNSIGNED_RIGHT_SHIFT => {
                self.new_node_from_template(ast, Token::URSH, template_node)
            }
            NodeKind::PRE_INCREMENT => self.new_node_from_template(ast, Token::INC, template_node),
            NodeKind::PRE_DECREMENT => self.new_node_from_template(ast, Token::DEC, template_node),
            NodeKind::ASSIGN_ADD => {
                self.new_node_from_template(ast, Token::ASSIGN_ADD, template_node)
            }
            NodeKind::ASSIGN_SUBTRACT => {
                self.new_node_from_template(ast, Token::ASSIGN_SUB, template_node)
            }
            NodeKind::ASSIGN_MULTIPLY => {
                self.new_node_from_template(ast, Token::ASSIGN_MUL, template_node)
            }
            NodeKind::ASSIGN_DIVIDE => {
                self.new_node_from_template(ast, Token::ASSIGN_DIV, template_node)
            }
            NodeKind::ASSIGN_MODULO => {
                self.new_node_from_template(ast, Token::ASSIGN_MOD, template_node)
            }
            NodeKind::ASSIGN_EXPONENT => {
                self.new_node_from_template(ast, Token::ASSIGN_EXPONENT, template_node)
            }
            NodeKind::ASSIGN_BITWISE_OR => {
                self.new_node_from_template(ast, Token::ASSIGN_BITOR, template_node)
            }
            NodeKind::ASSIGN_BITWISE_AND => {
                self.new_node_from_template(ast, Token::ASSIGN_BITAND, template_node)
            }
            NodeKind::ASSIGN_BITWISE_XOR => {
                self.new_node_from_template(ast, Token::ASSIGN_BITXOR, template_node)
            }
            NodeKind::ASSIGN_LEFT_SHIFT => {
                self.new_node_from_template(ast, Token::ASSIGN_LSH, template_node)
            }
            NodeKind::ASSIGN_RIGHT_SHIFT => {
                self.new_node_from_template(ast, Token::ASSIGN_RSH, template_node)
            }
            NodeKind::ASSIGN_UNSIGNED_RIGHT_SHIFT => {
                self.new_node_from_template(ast, Token::ASSIGN_URSH, template_node)
            }
            NodeKind::YIELD => self.new_node_from_template(ast, Token::YIELD, template_node),
            NodeKind::AWAIT => self.new_node_from_template(ast, Token::AWAIT, template_node),
            NodeKind::DELETE => self.new_node_from_template(ast, Token::DELPROP, template_node),
            NodeKind::TAGGED_TEMPLATELIT => {
                self.new_node_from_template(ast, Token::TAGGED_TEMPLATELIT, template_node)
            }
            NodeKind::TEMPLATELIT => {
                self.new_node_from_template(ast, Token::TEMPLATELIT, template_node)
            }
            NodeKind::TEMPLATELIT_SUB => {
                self.new_node_from_template(ast, Token::TEMPLATELIT_SUB, template_node)
            }
            NodeKind::NEW_TARGET => {
                self.new_node_from_template(ast, Token::NEW_TARGET, template_node)
            }
            NodeKind::COMPUTED_PROP => {
                self.new_node_from_template(ast, Token::COMPUTED_PROP, template_node)
            }
            NodeKind::IMPORT_META => {
                self.new_node_from_template(ast, Token::IMPORT_META, template_node)
            }
            NodeKind::OPTCHAIN_PROPERTY_ACCESS => {
                self.string_node(ast, Token::OPTCHAIN_GETPROP, n, template_node)
            }
            NodeKind::OPTCHAIN_CALL => {
                self.new_node_from_template(ast, Token::OPTCHAIN_CALL, template_node)
            }
            NodeKind::OPTCHAIN_ELEMENT_ACCESS => {
                self.new_node_from_template(ast, Token::OPTCHAIN_GETELEM, template_node)
            }
            NodeKind::COALESCE => self.new_node_from_template(ast, Token::COALESCE, template_node),
            NodeKind::DYNAMIC_IMPORT => {
                self.new_node_from_template(ast, Token::DYNAMIC_IMPORT, template_node)
            }
            NodeKind::ASSIGN_OR => {
                self.new_node_from_template(ast, Token::ASSIGN_OR, template_node)
            }
            NodeKind::ASSIGN_AND => {
                self.new_node_from_template(ast, Token::ASSIGN_AND, template_node)
            }
            NodeKind::ASSIGN_COALESCE => {
                self.new_node_from_template(ast, Token::ASSIGN_COALESCE, template_node)
            }
            NodeKind::EXPRESSION_STATEMENT => {
                self.new_node_from_template(ast, Token::EXPR_RESULT, template_node)
            }
            NodeKind::BREAK_STATEMENT => {
                self.new_node_from_template(ast, Token::BREAK, template_node)
            }
            NodeKind::CONTINUE_STATEMENT => {
                self.new_node_from_template(ast, Token::CONTINUE, template_node)
            }
            NodeKind::DEBUGGER_STATEMENT => {
                self.new_node_from_template(ast, Token::DEBUGGER, template_node)
            }
            NodeKind::DO_STATEMENT => self.new_node_from_template(ast, Token::DO, template_node),
            NodeKind::FOR_STATEMENT => self.new_node_from_template(ast, Token::FOR, template_node),
            NodeKind::FOR_IN_STATEMENT => {
                self.new_node_from_template(ast, Token::FOR_IN, template_node)
            }
            NodeKind::FOR_OF_STATEMENT => {
                self.new_node_from_template(ast, Token::FOR_OF, template_node)
            }
            NodeKind::FOR_AWAIT_OF_STATEMENT => {
                self.new_node_from_template(ast, Token::FOR_AWAIT_OF, template_node)
            }
            NodeKind::IF_STATEMENT => self.new_node_from_template(ast, Token::IF, template_node),
            NodeKind::RETURN_STATEMENT => {
                self.new_node_from_template(ast, Token::RETURN, template_node)
            }
            NodeKind::SWITCH_STATEMENT => {
                self.new_node_from_template(ast, Token::SWITCH, template_node)
            }
            NodeKind::SWITCH_BODY => {
                self.new_node_from_template(ast, Token::SWITCH_BODY, template_node)
            }
            NodeKind::THROW_STATEMENT => {
                self.new_node_from_template(ast, Token::THROW, template_node)
            }
            NodeKind::TRY_STATEMENT => self.new_node_from_template(ast, Token::TRY, template_node),
            NodeKind::WHILE_STATEMENT => {
                self.new_node_from_template(ast, Token::WHILE, template_node)
            }
            NodeKind::EMPTY => self.new_node_from_template(ast, Token::EMPTY, template_node),
            NodeKind::WITH => self.new_node_from_template(ast, Token::WITH, template_node),
            NodeKind::IMPORT => self.new_node_from_template(ast, Token::IMPORT, template_node),
            NodeKind::EXPORT => self.new_node_from_template(ast, Token::EXPORT, template_node),
            NodeKind::VAR_DECLARATION => {
                self.new_node_from_template(ast, Token::VAR, template_node)
            }
            NodeKind::CONST_DECLARATION => {
                self.new_node_from_template(ast, Token::CONST, template_node)
            }
            NodeKind::LET_DECLARATION => {
                self.new_node_from_template(ast, Token::LET, template_node)
            }
            NodeKind::FUNCTION_LITERAL => {
                self.new_node_from_template(ast, Token::FUNCTION, template_node)
            }
            NodeKind::CLASS_LITERAL => {
                self.new_node_from_template(ast, Token::CLASS, template_node)
            }
            NodeKind::BLOCK => self.new_node_from_template(ast, Token::BLOCK, template_node),
            NodeKind::LABELED_STATEMENT => {
                self.new_node_from_template(ast, Token::LABEL, template_node)
            }
            NodeKind::LABELED_NAME => self.string_node(ast, Token::LABEL_NAME, n, template_node),
            NodeKind::CLASS_MEMBERS => {
                self.new_node_from_template(ast, Token::CLASS_MEMBERS, template_node)
            }
            NodeKind::METHOD_DECLARATION => {
                self.string_node(ast, Token::MEMBER_FUNCTION_DEF, n, template_node)
            }
            NodeKind::FIELD_DECLARATION => {
                self.string_node(ast, Token::MEMBER_FIELD_DEF, n, template_node)
            }
            NodeKind::COMPUTED_PROP_FIELD => {
                self.new_node_from_template(ast, Token::COMPUTED_FIELD_DEF, template_node)
            }
            NodeKind::PARAMETER_LIST => {
                self.new_node_from_template(ast, Token::PARAM_LIST, template_node)
            }
            NodeKind::RENAMABLE_STRING_KEY => {
                self.string_node(ast, Token::STRING_KEY, n, template_node)
            }
            NodeKind::CASE => self.new_node_from_template(ast, Token::CASE, template_node),
            NodeKind::DEFAULT_CASE => {
                self.new_node_from_template(ast, Token::DEFAULT_CASE, template_node)
            }
            NodeKind::CATCH => self.new_node_from_template(ast, Token::CATCH, template_node),
            NodeKind::SUPER => self.new_node_from_template(ast, Token::SUPER, template_node),
            NodeKind::ARRAY_PATTERN => {
                self.new_node_from_template(ast, Token::ARRAY_PATTERN, template_node)
            }
            NodeKind::OBJECT_PATTERN => {
                self.new_node_from_template(ast, Token::OBJECT_PATTERN, template_node)
            }
            NodeKind::DESTRUCTURING_LHS => {
                self.new_node_from_template(ast, Token::DESTRUCTURING_LHS, template_node)
            }
            NodeKind::DEFAULT_VALUE => {
                self.new_node_from_template(ast, Token::DEFAULT_VALUE, template_node)
            }
            NodeKind::IMPORT_SPECS => {
                self.new_node_from_template(ast, Token::IMPORT_SPECS, template_node)
            }
            NodeKind::IMPORT_SPEC => {
                self.new_node_from_template(ast, Token::IMPORT_SPEC, template_node)
            }
            NodeKind::IMPORT_STAR => self.string_node(ast, Token::IMPORT_STAR, n, template_node),
            NodeKind::EXPORT_SPECS => {
                self.new_node_from_template(ast, Token::EXPORT_SPECS, template_node)
            }
            NodeKind::EXPORT_SPEC => {
                self.new_node_from_template(ast, Token::EXPORT_SPEC, template_node)
            }
            NodeKind::MODULE_BODY => {
                self.new_node_from_template(ast, Token::MODULE_BODY, template_node)
            }
            NodeKind::ITER_REST => {
                self.new_node_from_template(ast, Token::ITER_REST, template_node)
            }
            NodeKind::ITER_SPREAD => {
                self.new_node_from_template(ast, Token::ITER_SPREAD, template_node)
            }
            NodeKind::OBJECT_REST => {
                self.new_node_from_template(ast, Token::OBJECT_REST, template_node)
            }
            NodeKind::OBJECT_SPREAD => {
                self.new_node_from_template(ast, Token::OBJECT_SPREAD, template_node)
            }

            NodeKind::NODE_KIND_UNSPECIFIED | NodeKind::UNRECOGNIZED => {
                panic!("Unexpected serialized kind for AstNode: {n:?}")
            }
        }
    }

    /// If no colors are being deserialized, filters out any NodeProperty.COLOR_BEFORE_CASTs
    ///
    /// This is because it doesn't make sense to have that property present on nodes that don't
    /// have colors.
    // port: ScriptNodeDeserializer#filterOutCastProp
    fn filter_out_cast_prop(&self, node_properties: i64) -> i64 {
        if self.color_pool_shard.is_some() {
            return node_properties; // we are deserializing colors, so this is fine.
        }
        node_properties & !(1i64 << NodeProperty::COLOR_FROM_CAST.get_number())
    }
}

/// port: ScriptNodeDeserializer.Runner
struct Runner<'a> {
    owner: &'a ScriptNodeDeserializer,
    feature_collector: FeatureCollector,
    previous_line: i32,
    previous_column: i32,
}

impl Runner<'_> {
    // port: ScriptNodeDeserializer.Runner#run
    fn run(mut self, ast: &mut Ast) -> NodeId {
        let mut ast_stream = CodedInputStream::new_instance(&self.owner.script_bytes);
        ast_stream.set_recursion_limit(i32::MAX); // The real limit is stack space.

        let mut context_stack: Vec<FeatureContext> = Vec::new();
        context_stack.push(FeatureContext::NONE);

        let parsed = match AstNode::parse_from_stream(&mut ast_stream) {
            Ok(parsed) => parsed,
            Err(ex) => {
                MalformedTypedAstException::new_with_cause(self.owner.source_file.as_ref(), ex)
                    .throw()
            }
        };
        let template = self
            .owner
            .create_source_info_template(ast, &self.owner.source_file);
        let script_node = self.visit(ast, &parsed, &mut context_stack, Some(template));
        script_node.put_prop(
            ast,
            NodeId::FEATURE_SET,
            Some(ObjectProp::Opaque(Arc::new(
                self.feature_collector.all_features(),
            ))),
        );
        script_node
    }

    /// Visits a single AST node and returns the corresponding Node.
    // port: ScriptNodeDeserializer.Runner#visit
    fn visit(
        &mut self,
        ast: &mut Ast,
        ast_node: &AstNode,
        context_stack: &mut Vec<FeatureContext>,
        source_file_template: Option<NodeId>,
    ) -> NodeId {
        let source_file_template = match source_file_template {
            Some(template) if ast_node.get_source_file() == 0 => template,
            _ => {
                // 0 == 'not set'
                let file = &self.owner.file_pool[(ast_node.get_source_file() - 1) as usize];
                self.owner.create_source_info_template(ast, file)
            }
        };

        let current_line = self.previous_line + ast_node.get_relative_line();
        let current_column = self.previous_column + ast_node.get_relative_column();

        let n = self
            .owner
            .deserialize_single_node(ast, ast_node, source_file_template);
        if ast_node.has_type()
            && let Some(shard) = &self.owner.color_pool_shard
        {
            n.set_color(ast, Some(shard.get_color(ast_node.get_type())));
        }
        let properties = ast_node.get_boolean_properties();
        if properties > 0 {
            let in_closure_unaware_subtree =
                source_file_template.get_is_in_closure_unaware_subtree(ast);
            n.deserialize_properties(
                ast,
                self.owner.filter_out_cast_prop(properties),
                in_closure_unaware_subtree,
            );
        }
        let jsdoc = JSDocSerializer::deserialize_jsdoc(
            ast,
            Some(ast_node.get_jsdoc()),
            &self.owner.string_pool,
        );
        n.set_jsdoc_info(ast, jsdoc);
        n.set_lineno_charno(ast, current_line, current_column);
        self.previous_line = current_line;
        self.previous_column = current_column;
        let context = self
            .feature_collector
            .visit_single_node(ast, context_stack, n);
        context_stack.push(context);

        if NodeId::has_bit_set(
            properties,
            NodeProperty::CLOSURE_UNAWARE_SHADOW.get_number(),
        ) {
            let serialized_shadow_child = ast_node.get_child(0);
            let closure_unaware_subtree_template_node = source_file_template.clone_tree(ast);
            closure_unaware_subtree_template_node.set_is_in_closure_unaware_subtree(ast, true);
            // Note: we intentionally pass `contextStack` to visit() in order to capture any
            // language features present in the shadow AST, like const syntax, into the main AST
            // featureset. We are doing this to align ScriptNodeDeserializer with the existing
            // behavior in IRFactory. (In the future we could consider instead giving each shadow
            // AST script its own separate featureset, but that would be a larger change.)
            let shadowed_code = self.visit(
                ast,
                serialized_shadow_child,
                context_stack,
                Some(closure_unaware_subtree_template_node),
            );
            self.owner
                .set_original_name_if_present(ast, serialized_shadow_child, shadowed_code);
            // The shadowed code is only the "source" parts of the shadow structure, and does not
            // include the synthetic code that is needed for the compiler to consider it a valid
            // standalone AST. We recreate that here.
            // This must be kept in sync with the shadow structure created by
            // TypedAstSerializer.
            let expr_result = IR::expr_result(ast, shadowed_code);
            let script = IR::script_with_children(ast, &[expr_result]);
            let shadow_root = IR::root(ast, &[script]);
            shadow_root.clone_props_from(ast, closure_unaware_subtree_template_node);
            shadow_root
                .get_first_child(ast)
                .unwrap()
                .clone_props_from(ast, closure_unaware_subtree_template_node);
            shadow_root
                .get_first_first_child(ast)
                .unwrap()
                .clone_props_from(ast, closure_unaware_subtree_template_node);

            n.set_closure_unaware_shadow(ast, Some(shadow_root));
            return n;
        }
        let children = ast_node.get_child_count();

        for i in 0..children {
            let child = ast_node.get_child(i);
            let deserialized_child =
                self.visit(ast, child, context_stack, Some(source_file_template));
            n.add_child_to_back(ast, deserialized_child);
            self.owner
                .set_original_name_if_present(ast, child, deserialized_child);
        }

        context_stack.pop();

        n
    }
}
