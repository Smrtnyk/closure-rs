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
//   src/com/google/javascript/jscomp/serialization/TypedAstSerializer.java.

//! Port of serialization/TypedAstSerializer.java.
use super::color_serializer::ColorSerializer;
use super::jsdoc_serializer::JSDocSerializer;
use super::protobuf::Message;
use super::serialization_options::SerializationOptions;
use super::serialize_types_to_pointers::SerializeTypesToPointers;
use super::source_file_proto::SourceFilePool;
use super::string_pool::{StringPool, StringPoolBuilder};
use super::typed_ast_proto::{
    AstNode, ExternsSummary, LazyAst, NodeKind, TemplateStringValue, TypedAst,
};
use super::types_proto::TypePool;
use crate::abstract_compiler::AbstractCompiler;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::source_file::SourceFile;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_colors::color::Color;
use closure_rhino::jscomp_colors::color_registry::ColorRegistry;
use closure_rhino::jstype::TypeId;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::static_source_file::StaticSourceFile;
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null, check_state};
use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

/// Rust-only: Java keys `sourceFilePointers` (and compares `subtreeSourceFiles` entries) by the
/// SourceFile object's identity; this is that identity (the `Arc` data address).
fn source_file_identity(file: &Arc<dyn StaticSourceFile>) -> usize {
    Arc::as_ptr(file).cast::<()>() as usize
}

/// port: TypedAstSerializer
///
/// Transforms a compiler AST into a serialized TypedAst object.
pub struct TypedAstSerializer<'c> {
    compiler: &'c mut AbstractCompiler,
    serialization_mode: SerializationOptions,
    /// Java shares this one `StringPool.Builder` with the type serializers (`stringPool::put`).
    string_pool: Rc<RefCell<StringPoolBuilder>>,
    previous_line: i32,
    previous_column: i32,
    subtree_source_files: VecDeque<Arc<dyn StaticSourceFile>>,
    /// Java's `LinkedHashMap<SourceFile, Integer>` (SourceFile has identity equality).
    source_file_pointers: IndexMap<usize, (Arc<dyn StaticSourceFile>, i32)>,

    type_serializer: Option<Box<dyn TypeSerializer>>,
}

impl<'c> TypedAstSerializer<'c> {
    // port: TypedAstSerializer#<init>
    pub fn new(
        compiler: &'c mut AbstractCompiler,
        serialization_mode: SerializationOptions,
    ) -> Self {
        Self {
            compiler,
            serialization_mode,
            string_pool: Rc::new(RefCell::new(StringPool::builder())),
            previous_line: 0,
            previous_column: 0,
            subtree_source_files: VecDeque::new(),
            source_file_pointers: IndexMap::<_, _>::default(),
            type_serializer: None,
        }
    }

    /// Transforms the given compiler AST root nodes into into a serialized TypedAst object
    // port: TypedAstSerializer#serializeRoots
    pub fn serialize_roots(&mut self, externs_root: NodeId, js_root: NodeId) -> TypedAst {
        check_argument!(externs_root.is_root(self.compiler));
        check_argument!(js_root.is_root(self.compiler));

        if self.compiler.has_optimization_colors() {
            self.type_serializer = Some(Box::new(Self::create_color_type_serializer(
                self.compiler,
                &self.serialization_mode,
                &self.string_pool,
            )));
        } else if self.compiler.has_type_checking_run() {
            // The AST has JSTypes, but we want to serialize colors instead.
            // TODO(bradfordcsmith): Change this branch to throw an error and delete the related
            //     logic. Nothing should be using it anymore.
            self.type_serializer = Some(Box::new(Self::create_js_type_serializer(
                self.compiler,
                &self.serialization_mode,
                &self.string_pool,
            )));
        } else {
            self.type_serializer = Some(Box::new(NoOpTypeSerializer));
        }

        let mut builder = TypedAst::new_builder();
        let mut script = externs_root.get_first_child(self.compiler);
        while let Some(s) = script {
            if !NodeUtil::is_from_type_summary(self.compiler, s) {
                let lazy_ast = self.serialize_script_node(s);
                builder = builder.add_extern_ast(lazy_ast);
            }
            script = s.get_next(self.compiler);
        }
        let mut script = js_root.get_first_child(self.compiler);
        while let Some(s) = script {
            if !NodeUtil::is_from_type_summary(self.compiler, s) {
                let lazy_ast = self.serialize_script_node(s);
                builder = builder.add_code_ast(lazy_ast);
            }
            script = s.get_next(self.compiler);
        }

        let source_files = SourceFilePool::new_builder()
            .add_all_source_file(
                self.source_file_pointers
                    .values()
                    .map(|(file, _)| as_source_file(file).get_proto())
                    .collect::<Vec<_>>(),
            )
            .build();

        if let Some(extern_properties) = self.compiler.get_extern_properties() {
            let mut externs_summary = ExternsSummary::new_builder();
            for prop in extern_properties {
                externs_summary = externs_summary
                    .add_prop_name_ptr(self.string_pool.borrow_mut().put(prop.as_str()));
            }
            builder = builder.set_externs_summary(externs_summary);
        }

        builder = builder.add_all_runtime_library_to_inject(
            self.serialization_mode.runtime_libraries().to_vec(),
        );
        let type_pool = self.type_serializer.as_mut().unwrap().generate_type_pool();
        let string_pool = self.string_pool.borrow().build().to_proto();
        builder
            .set_type_pool(type_pool)
            .set_string_pool(string_pool)
            .set_source_file_pool(source_files)
            .build()
    }

    // port: TypedAstSerializer#serializeScriptNode
    fn serialize_script_node(&mut self, script: NodeId) -> LazyAst {
        check_state!(script.is_script(self.compiler));
        self.previous_column = 0;
        self.previous_line = 0;

        let source_file = self.get_source_file_pointer(script);
        let script_proto = self.visit(script);
        self.subtree_source_files.clear();

        let source_file_name = script.get_source_file_name(self.compiler);
        let encoded_source_map = self
            .compiler
            .get_base64_source_map_contents(source_file_name.as_deref().unwrap_or_default());

        let mut lazy_ast_builder = LazyAst::new_builder()
            .set_script(script_proto.to_byte_array())
            .set_source_file(source_file);

        if let Some(encoded_source_map) = encoded_source_map {
            // This is the encoded source map taken from the inline sourcemap comment. It does not
            // include the base64 prefix.
            // E.g. We serialize "eyJ2ZXJzaW9uI..." from the "//# sourceMappingURL=
            // data:application/json;base64,eyJ2ZXJzaW9uI..." comment.
            lazy_ast_builder = lazy_ast_builder.set_source_mapping_url(encoded_source_map);
        }

        lazy_ast_builder.build()
    }

    // port: TypedAstSerializer#createWithPositionInfo
    fn create_with_position_info(&mut self, n: NodeId) -> AstNode {
        let ast: &Ast = self.compiler;
        check_state!(n.get_length(ast) >= 0);
        let current_line = n.get_lineno(ast);
        let current_column = n.get_charno(ast);
        let builder = AstNode::new_builder()
            .set_relative_line(current_line - self.previous_line)
            .set_relative_column(current_column - self.previous_column);
        self.previous_line = current_line;
        self.previous_column = current_column;
        builder
    }

    // port: TypedAstSerializer#visit
    fn visit(&mut self, n: NodeId) -> AstNode {
        let mut builder = self.create_with_position_info(n);
        builder = self.add_type(n, builder);
        let jsdoc = n.get_jsdoc_info(self.compiler);
        let serialized_jsdoc =
            JSDocSerializer::serialize_jsdoc(jsdoc.as_deref(), &mut self.string_pool.borrow_mut());
        if let Some(serialized_jsdoc) = serialized_jsdoc {
            builder = builder.set_jsdoc(serialized_jsdoc);
        }
        builder = builder.set_kind(Self::kind_translator(self.compiler, n));
        builder = self.value_translator(builder, n);
        builder = builder.set_boolean_properties(n.serialize_properties(self.compiler));
        let source_file = self.get_source_file_pointer(n);
        builder = builder.set_source_file(source_file);

        let mut child = n.get_first_child(self.compiler);
        while let Some(c) = child {
            let child_proto = self.visit(c);
            builder = builder.add_child(child_proto);
            child = c.get_next(self.compiler);
        }
        let shadowed_code = n.get_closure_unaware_shadow(self.compiler);
        if let Some(shadowed_code) = shadowed_code {
            // For Closure shadow hosts, the ASTNode will get a boolean property bit set
            // (CLOSURE_UNAWARE_SHADOW) that indicates this child ASTNode is not a normal child, but
            // is the contents of the shadow.
            // Shadow roots are structured as
            // ROOT -> SCRIPT -> EXPR_RESULT -> FUNCTION
            // We avoid including the ROOT -> SCRIPT -> EXPR_RESULT structure in the serialization
            // as it is all synthetic code that would unnecessarily bloat the TypedAST and is
            // instead recreated upon deserialization.
            // The child ASTNode is just the FUNCTION.
            let function = shadowed_code
                .get_first_first_child(self.compiler)
                .unwrap()
                .get_first_child(self.compiler)
                .unwrap();
            let child_proto = self.visit(function);
            builder = builder.add_child(child_proto);
        }

        if source_file != 0 {
            self.subtree_source_files.pop_back();
        }
        builder = self.set_original_name(builder, n);

        builder.build()
    }

    // port: TypedAstSerializer#getSourceFilePointer
    fn get_source_file_pointer(&mut self, n: NodeId) -> i32 {
        let Some(source_file) = n.get_static_source_file(self.compiler) else {
            // TODO(b/186056977): enforce that SourceFile is not null in externs as well as code.
            check_state!(
                self.subtree_source_files.back().unwrap().is_extern(),
                "Unexpected null SourceFile for node %s with parent %s",
                n.to_string_tree(self.compiler),
                n.get_parent(self.compiler)
                    .map_or_else(|| "null".to_string(), |p| p.to_string(self.compiler))
            );
            return 0; // not set
        };
        // Java casts `n.getStaticSourceFile()` to `SourceFile`.
        as_source_file(&source_file);

        if self
            .subtree_source_files
            .back()
            .is_some_and(|last| source_file_identity(last) == source_file_identity(&source_file))
        {
            // To save space, only serialize a SourceFile for a node if it is different than the
            // parent's source.
            return 0; // not set
        }

        self.subtree_source_files.push_back(source_file.clone());
        let next = 1 + self.source_file_pointers.len() as i32;
        self.source_file_pointers
            .entry(source_file_identity(&source_file))
            .or_insert((source_file, next))
            .1
    }

    // port: TypedAstSerializer#addType
    fn add_type(&mut self, n: NodeId, builder: AstNode) -> AstNode {
        self.type_serializer
            .as_mut()
            .unwrap()
            .add_type_for_node(self.compiler, n, builder)
    }

    // port: TypedAstSerializer#valueTranslator
    fn value_translator(&mut self, builder: AstNode, n: NodeId) -> AstNode {
        let ast: &Ast = self.compiler;
        match n.get_token(ast) {
            Token::GETPROP
            | Token::OPTCHAIN_GETPROP
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::NAME
            | Token::STRINGLIT
            | Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::LABEL_NAME
            | Token::IMPORT_STAR => builder
                .set_string_value_pointer(self.string_pool.borrow_mut().put(n.get_string(ast))),
            Token::TEMPLATELIT_STRING => {
                let raw_string_pointer = self.string_pool.borrow_mut().put(n.get_raw_string(ast));
                let cooked_string_pointer = match n.get_cooked_string(ast) {
                    None => -1,
                    Some(cooked) => self.string_pool.borrow_mut().put(cooked),
                };
                builder.set_template_string_value(
                    TemplateStringValue::new_builder()
                        .set_raw_string_pointer(raw_string_pointer)
                        .set_cooked_string_pointer(cooked_string_pointer)
                        .build(),
                )
            }
            Token::NUMBER => builder.set_double_value(n.get_double(ast)),
            Token::BIGINT => builder.set_string_value_pointer(
                self.string_pool
                    .borrow_mut()
                    .put(n.get_big_int(ast).to_string().as_str()),
            ),
            _ => {
                // No value
                builder
            }
        }
    }

    // port: TypedAstSerializer#setOriginalName
    fn set_original_name(&mut self, builder: AstNode, n: NodeId) -> AstNode {
        let original_name = n.get_original_name(self.compiler);
        let Some(original_name) = original_name else {
            return builder.set_original_name_pointer(0); // equivalent to 'not set'
        };

        builder.set_original_name_pointer(self.string_pool.borrow_mut().put(original_name))
    }

    // port: TypedAstSerializer#kindTranslator
    fn kind_translator(ast: &Ast, n: NodeId) -> NodeKind {
        match n.get_token(ast) {
            Token::SCRIPT => {
                return NodeKind::SOURCE_FILE;
            }
            Token::NUMBER => {
                return NodeKind::NUMBER_LITERAL;
            }
            Token::STRINGLIT => {
                return NodeKind::STRING_LITERAL;
            }
            Token::BIGINT => {
                return NodeKind::BIGINT_LITERAL;
            }
            Token::REGEXP => {
                return NodeKind::REGEX_LITERAL;
            }
            Token::FALSE => {
                return NodeKind::FALSE;
            }
            Token::TRUE => {
                return NodeKind::TRUE;
            }
            Token::NULL => {
                return NodeKind::NULL;
            }
            Token::THIS => {
                return NodeKind::THIS;
            }
            Token::VOID => {
                return NodeKind::VOID;
            }
            Token::ARRAYLIT => {
                return NodeKind::ARRAY_LITERAL;
            }
            Token::OBJECTLIT => {
                return NodeKind::OBJECT_LITERAL;
            }
            Token::NAME => {
                return NodeKind::IDENTIFIER;
            }
            Token::ASSIGN => {
                return NodeKind::ASSIGNMENT;
            }
            Token::CALL => {
                return NodeKind::CALL;
            }
            Token::NEW => {
                return NodeKind::NEW;
            }
            Token::GETPROP => {
                return NodeKind::PROPERTY_ACCESS;
            }
            Token::GETELEM => {
                return NodeKind::ELEMENT_ACCESS;
            }
            Token::COMMA => {
                return NodeKind::COMMA;
            }
            Token::OR => {
                return NodeKind::BOOLEAN_OR;
            }
            Token::AND => {
                return NodeKind::BOOLEAN_AND;
            }
            Token::HOOK => {
                return NodeKind::HOOK;
            }
            Token::EQ => {
                return NodeKind::EQUAL;
            }
            Token::NE => {
                return NodeKind::NOT_EQUAL;
            }
            Token::LT => {
                return NodeKind::LESS_THAN;
            }
            Token::LE => {
                return NodeKind::LESS_THAN_EQUAL;
            }
            Token::GT => {
                return NodeKind::GREATER_THAN;
            }
            Token::GE => {
                return NodeKind::GREATER_THAN_EQUAL;
            }
            Token::SHEQ => {
                return NodeKind::TRIPLE_EQUAL;
            }
            Token::SHNE => {
                return NodeKind::NOT_TRIPLE_EQUAL;
            }
            Token::NOT => {
                return NodeKind::NOT;
            }
            Token::POS => {
                return NodeKind::POSITIVE;
            }
            Token::NEG => {
                return NodeKind::NEGATIVE;
            }
            Token::TYPEOF => {
                return NodeKind::TYPEOF;
            }
            Token::INSTANCEOF => {
                return NodeKind::INSTANCEOF;
            }
            Token::IN => {
                return NodeKind::IN;
            }
            Token::ADD => {
                return NodeKind::ADD;
            }
            Token::SUB => {
                return NodeKind::SUBTRACT;
            }
            Token::MUL => {
                return NodeKind::MULTIPLY;
            }
            Token::DIV => {
                return NodeKind::DIVIDE;
            }
            Token::MOD => {
                return NodeKind::MODULO;
            }
            Token::EXPONENT => {
                return NodeKind::EXPONENT;
            }
            Token::BITNOT => {
                return NodeKind::BITWISE_NOT;
            }
            Token::BITOR => {
                return NodeKind::BITWISE_OR;
            }
            Token::BITAND => {
                return NodeKind::BITWISE_AND;
            }
            Token::BITXOR => {
                return NodeKind::BITWISE_XOR;
            }
            Token::LSH => {
                return NodeKind::LEFT_SHIFT;
            }
            Token::RSH => {
                return NodeKind::RIGHT_SHIFT;
            }
            Token::URSH => {
                return NodeKind::UNSIGNED_RIGHT_SHIFT;
            }
            Token::INC => {
                return if n.get_boolean_prop(ast, NodeId::INCRDECR_PROP) {
                    NodeKind::POST_INCREMENT
                } else {
                    NodeKind::PRE_INCREMENT
                };
            }
            Token::DEC => {
                return if n.get_boolean_prop(ast, NodeId::INCRDECR_PROP) {
                    NodeKind::POST_DECREMENT
                } else {
                    NodeKind::PRE_DECREMENT
                };
            }
            Token::ASSIGN_ADD => {
                return NodeKind::ASSIGN_ADD;
            }
            Token::ASSIGN_SUB => {
                return NodeKind::ASSIGN_SUBTRACT;
            }
            Token::ASSIGN_MUL => {
                return NodeKind::ASSIGN_MULTIPLY;
            }
            Token::ASSIGN_DIV => {
                return NodeKind::ASSIGN_DIVIDE;
            }
            Token::ASSIGN_MOD => {
                return NodeKind::ASSIGN_MODULO;
            }
            Token::ASSIGN_EXPONENT => {
                return NodeKind::ASSIGN_EXPONENT;
            }
            Token::ASSIGN_BITOR => {
                return NodeKind::ASSIGN_BITWISE_OR;
            }
            Token::ASSIGN_BITAND => {
                return NodeKind::ASSIGN_BITWISE_AND;
            }
            Token::ASSIGN_BITXOR => {
                return NodeKind::ASSIGN_BITWISE_XOR;
            }
            Token::ASSIGN_LSH => {
                return NodeKind::ASSIGN_LEFT_SHIFT;
            }
            Token::ASSIGN_RSH => {
                return NodeKind::ASSIGN_RIGHT_SHIFT;
            }
            Token::ASSIGN_URSH => {
                return NodeKind::ASSIGN_UNSIGNED_RIGHT_SHIFT;
            }
            Token::YIELD => {
                return NodeKind::YIELD;
            }
            Token::AWAIT => {
                return NodeKind::AWAIT;
            }
            Token::DELPROP => {
                return NodeKind::DELETE;
            }
            Token::TAGGED_TEMPLATELIT => {
                return NodeKind::TAGGED_TEMPLATELIT;
            }
            Token::TEMPLATELIT => {
                return NodeKind::TEMPLATELIT;
            }
            Token::TEMPLATELIT_SUB => {
                return NodeKind::TEMPLATELIT_SUB;
            }
            Token::TEMPLATELIT_STRING => {
                return NodeKind::TEMPLATELIT_STRING;
            }
            Token::NEW_TARGET => {
                return NodeKind::NEW_TARGET;
            }
            Token::COMPUTED_PROP => {
                return NodeKind::COMPUTED_PROP;
            }
            Token::IMPORT_META => {
                return NodeKind::IMPORT_META;
            }
            Token::OPTCHAIN_GETPROP => {
                return NodeKind::OPTCHAIN_PROPERTY_ACCESS;
            }
            Token::OPTCHAIN_CALL => {
                return NodeKind::OPTCHAIN_CALL;
            }
            Token::OPTCHAIN_GETELEM => {
                return NodeKind::OPTCHAIN_ELEMENT_ACCESS;
            }
            Token::COALESCE => {
                return NodeKind::COALESCE;
            }
            Token::DYNAMIC_IMPORT => {
                return NodeKind::DYNAMIC_IMPORT;
            }
            Token::ASSIGN_OR => {
                return NodeKind::ASSIGN_OR;
            }
            Token::ASSIGN_AND => {
                return NodeKind::ASSIGN_AND;
            }
            Token::ASSIGN_COALESCE => {
                return NodeKind::ASSIGN_COALESCE;
            }
            Token::EXPR_RESULT => {
                return NodeKind::EXPRESSION_STATEMENT;
            }
            Token::BREAK => {
                return NodeKind::BREAK_STATEMENT;
            }
            Token::CONTINUE => {
                return NodeKind::CONTINUE_STATEMENT;
            }
            Token::DEBUGGER => {
                return NodeKind::DEBUGGER_STATEMENT;
            }
            Token::DO => {
                return NodeKind::DO_STATEMENT;
            }
            Token::FOR => {
                return NodeKind::FOR_STATEMENT;
            }
            Token::FOR_IN => {
                return NodeKind::FOR_IN_STATEMENT;
            }
            Token::FOR_OF => {
                return NodeKind::FOR_OF_STATEMENT;
            }
            Token::FOR_AWAIT_OF => {
                return NodeKind::FOR_AWAIT_OF_STATEMENT;
            }
            Token::IF => {
                return NodeKind::IF_STATEMENT;
            }
            Token::RETURN => {
                return NodeKind::RETURN_STATEMENT;
            }
            Token::SWITCH => {
                return NodeKind::SWITCH_STATEMENT;
            }
            Token::THROW => {
                return NodeKind::THROW_STATEMENT;
            }
            Token::TRY => {
                return NodeKind::TRY_STATEMENT;
            }
            Token::WHILE => {
                return NodeKind::WHILE_STATEMENT;
            }
            Token::EMPTY => {
                return NodeKind::EMPTY;
            }
            Token::WITH => {
                return NodeKind::WITH;
            }
            Token::IMPORT => {
                return NodeKind::IMPORT;
            }
            Token::EXPORT => {
                return NodeKind::EXPORT;
            }
            Token::VAR => {
                return NodeKind::VAR_DECLARATION;
            }
            Token::CONST => {
                return NodeKind::CONST_DECLARATION;
            }
            Token::LET => {
                return NodeKind::LET_DECLARATION;
            }
            Token::FUNCTION => {
                return NodeKind::FUNCTION_LITERAL;
            }
            Token::CLASS => {
                return NodeKind::CLASS_LITERAL;
            }
            Token::BLOCK => {
                return NodeKind::BLOCK;
            }
            Token::LABEL => {
                return NodeKind::LABELED_STATEMENT;
            }
            Token::LABEL_NAME => {
                return NodeKind::LABELED_NAME;
            }
            Token::CLASS_MEMBERS => {
                return NodeKind::CLASS_MEMBERS;
            }
            Token::MEMBER_FUNCTION_DEF => {
                return NodeKind::METHOD_DECLARATION;
            }
            Token::MEMBER_FIELD_DEF => {
                return NodeKind::FIELD_DECLARATION;
            }
            Token::COMPUTED_FIELD_DEF => {
                return NodeKind::COMPUTED_PROP_FIELD;
            }
            Token::PARAM_LIST => {
                return NodeKind::PARAMETER_LIST;
            }
            Token::STRING_KEY => {
                return if n.is_quoted_string_key(ast) {
                    NodeKind::QUOTED_STRING_KEY
                } else {
                    NodeKind::RENAMABLE_STRING_KEY
                };
            }
            Token::CASE => {
                return NodeKind::CASE;
            }
            Token::DEFAULT_CASE => {
                return NodeKind::DEFAULT_CASE;
            }
            Token::CATCH => {
                return NodeKind::CATCH;
            }
            Token::SUPER => {
                return NodeKind::SUPER;
            }
            Token::ARRAY_PATTERN => {
                return NodeKind::ARRAY_PATTERN;
            }
            Token::OBJECT_PATTERN => {
                return NodeKind::OBJECT_PATTERN;
            }
            Token::DESTRUCTURING_LHS => {
                return NodeKind::DESTRUCTURING_LHS;
            }
            Token::DEFAULT_VALUE => {
                return NodeKind::DEFAULT_VALUE;
            }
            Token::GETTER_DEF => {
                return if n.is_quoted_string_key(ast) {
                    NodeKind::QUOTED_GETTER_DEF
                } else {
                    NodeKind::RENAMABLE_GETTER_DEF
                };
            }
            Token::SETTER_DEF => {
                return if n.is_quoted_string_key(ast) {
                    NodeKind::QUOTED_SETTER_DEF
                } else {
                    NodeKind::RENAMABLE_SETTER_DEF
                };
            }
            Token::IMPORT_SPECS => {
                return NodeKind::IMPORT_SPECS;
            }
            Token::IMPORT_SPEC => {
                return NodeKind::IMPORT_SPEC;
            }
            Token::IMPORT_STAR => {
                return NodeKind::IMPORT_STAR;
            }
            Token::EXPORT_SPECS => {
                return NodeKind::EXPORT_SPECS;
            }
            Token::EXPORT_SPEC => {
                return NodeKind::EXPORT_SPEC;
            }
            Token::MODULE_BODY => {
                return NodeKind::MODULE_BODY;
            }
            Token::ITER_REST => {
                return NodeKind::ITER_REST;
            }
            Token::ITER_SPREAD => {
                return NodeKind::ITER_SPREAD;
            }
            Token::OBJECT_REST => {
                return NodeKind::OBJECT_REST;
            }
            Token::OBJECT_SPREAD => {
                return NodeKind::OBJECT_SPREAD;
            }
            Token::SWITCH_BODY => {
                return NodeKind::SWITCH_BODY;
            }
            // Explicitly unsupported token types. Not serialized: ROOT, the TS type tokens, the
            // JSDoc tokens, CAST (Closure-type-system-specific) and the unused placeholder tokens.
            _ => {}
        }
        panic!("Unserializable token for node: {}", n.to_string(ast));
    }

    /// Create the `TypeSerializer` appropriate for an AST that contains JSTypes.
    // port: TypedAstSerializer#createJSTypeSerializer
    fn create_js_type_serializer(
        compiler: &mut AbstractCompiler,
        serialization_mode: &SerializationOptions,
        string_pool_builder: &Rc<RefCell<StringPoolBuilder>>,
    ) -> JSTypeSerializer {
        let mut serialize_types_to_pointers = SerializeTypesToPointers::create(
            compiler,
            string_pool_builder,
            serialization_mode.clone(),
        );
        // Gather and serialize all the types now.
        let root = compiler.get_root().unwrap();
        serialize_types_to_pointers.gather_types_on_ast(compiler, root);
        JSTypeSerializer::new(
            serialize_types_to_pointers
                .get_type_pointers_by_jstype()
                .clone(),
            serialize_types_to_pointers.get_type_pool().unwrap().clone(),
        )
    }

    /// Creates a `TypeSerializer` that knows how to serialize `Color`s from the AST.
    // port: TypedAstSerializer#createColorTypeSerializer
    fn create_color_type_serializer(
        compiler: &mut AbstractCompiler,
        serialization_mode: &SerializationOptions,
        string_pool_builder: &Rc<RefCell<StringPoolBuilder>>,
    ) -> ColorTypeSerializer {
        // Gather all the property names that are actually used in the AST.
        let used_property_names = Self::collect_used_property_names(compiler);
        let string_pool_builder = string_pool_builder.clone();
        let color_serializer = ColorSerializer::new(
            serialization_mode.clone(),
            // lookup / allocate strings from the shared string pool
            move |s: &JsString| string_pool_builder.borrow_mut().put(s.clone()),
            // only include property names known to be used in the AST
            move |s: &JsString| used_property_names.contains(s),
        );
        ColorTypeSerializer::new(color_serializer, compiler.get_color_registry().clone())
    }

    /// Add all unquoted property names appearing in the AST.
    // port: TypedAstSerializer#collectUsedPropertyNames
    fn collect_used_property_names(compiler: &mut AbstractCompiler) -> IndexSet<JsString> {
        let mut callback = CollectUsedPropertyNamesCallback {
            property_names_builder: IndexSet::<_>::default(),
        };
        let root = compiler.get_root().unwrap();
        NodeTraversal::traverse(compiler, root, &mut callback);
        callback.property_names_builder
    }
}

/// Java casts `n.getStaticSourceFile()` to `SourceFile`.
fn as_source_file(file: &Arc<dyn StaticSourceFile>) -> &SourceFile {
    (file.as_ref() as &dyn Any)
        .downcast_ref::<SourceFile>()
        .expect("ClassCastException: not a SourceFile")
}

/// port: TypedAstSerializer#collectUsedPropertyNames (its anonymous NodeTraversal.Callback)
struct CollectUsedPropertyNamesCallback {
    property_names_builder: IndexSet<JsString>,
}

impl Callback for CollectUsedPropertyNamesCallback {
    // port: TypedAstSerializer#collectUsedPropertyNames$Callback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // We won't serialize type summary files, so we don't care about property names
        // appearing in them.
        !n.is_script(t) || !NodeUtil::is_from_type_summary(t, n)
    }

    // port: TypedAstSerializer#collectUsedPropertyNames$Callback#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            // "name" from (someObject.name)
            // "name" from (someObject?.name)
            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                self.property_names_builder.insert(n.get_string(t));
            }
            // "name" from obj = {name: 0}
            // "name" from class C { name() {} }
            // "name" from class C { name = 0; }
            // "name" from class C { get name() {} }
            // "name" from class C { set name(n) {} }
            Token::STRING_KEY
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::GETTER_DEF
            | Token::SETTER_DEF => {
                if !n.is_quoted_string_key(t) {
                    self.property_names_builder.insert(n.get_string(t));
                }
            }
            _ => {}
        }
    }
}

/// port: TypedAstSerializer.TypeSerializer
///
/// Used to provide TypePointers for serializing Nodes and to generate the TypePool.
pub trait TypeSerializer {
    /// If appropriate for `node` add a `Integer` to `astNodeBuilder`
    // port: TypedAstSerializer.TypeSerializer#addTypeForNode
    fn add_type_for_node(&mut self, ast: &Ast, node: NodeId, ast_node_builder: AstNode) -> AstNode;

    /// Returns a `TypePool` containing the types used by `addTypeForNode()`
    // port: TypedAstSerializer.TypeSerializer#generateTypePool
    fn generate_type_pool(&mut self) -> TypePool;
}

/// port: TypedAstSerializer.NoOpTypeSerializer
///
/// Used when type checking has not been done.
struct NoOpTypeSerializer;

impl TypeSerializer for NoOpTypeSerializer {
    // port: TypedAstSerializer.NoOpTypeSerializer#addTypeForNode
    fn add_type_for_node(
        &mut self,
        _ast: &Ast,
        _node: NodeId,
        ast_node_builder: AstNode,
    ) -> AstNode {
        // Do nothing.
        ast_node_builder
    }

    // port: TypedAstSerializer.NoOpTypeSerializer#generateTypePool
    fn generate_type_pool(&mut self) -> TypePool {
        TypePool::get_default_instance() // empty TypePool
    }
}

/// port: TypedAstSerializer.JSTypeSerializer
///
/// Used when the AST's JSTypes have not been converted to Colors
struct JSTypeSerializer {
    // Everything is pre-calculated with this form of serialization.
    types_to_pointers: IndexMap<TypeId, i32>,
    type_pool: TypePool,
}

impl JSTypeSerializer {
    // port: TypedAstSerializer.JSTypeSerializer#<init>
    fn new(types_to_pointers: IndexMap<TypeId, i32>, type_pool: TypePool) -> Self {
        Self {
            types_to_pointers,
            type_pool,
        }
    }
}

impl TypeSerializer for JSTypeSerializer {
    // port: TypedAstSerializer.JSTypeSerializer#addTypeForNode
    fn add_type_for_node(&mut self, ast: &Ast, node: NodeId, ast_node_builder: AstNode) -> AstNode {
        let jstype = node.get_jstype(ast);
        if let Some(jstype) = jstype {
            return ast_node_builder.set_type(*check_not_null!(
                self.types_to_pointers.get(&jstype),
                "cannot find pointer for %s",
                format!("{jstype:?}")
            ));
        }
        ast_node_builder
    }

    // port: TypedAstSerializer.JSTypeSerializer#generateTypePool
    fn generate_type_pool(&mut self) -> TypePool {
        self.type_pool.clone()
    }
}

/// port: TypedAstSerializer.ColorTypeSerializer
///
/// Used when the AST has `Color`s rather than `JSType`s
struct ColorTypeSerializer {
    color_serializer: ColorSerializer<'static>,
    color_registry: Arc<ColorRegistry>,
}

impl ColorTypeSerializer {
    // port: TypedAstSerializer.ColorTypeSerializer#<init>
    fn new(color_serializer: ColorSerializer<'static>, color_registry: Arc<ColorRegistry>) -> Self {
        Self {
            color_serializer,
            color_registry,
        }
    }
}

impl TypeSerializer for ColorTypeSerializer {
    // port: TypedAstSerializer.ColorTypeSerializer#addTypeForNode
    fn add_type_for_node(&mut self, ast: &Ast, node: NodeId, ast_node_builder: AstNode) -> AstNode {
        let color = node.get_color(ast);
        if let Some(color) = color {
            return ast_node_builder.set_type(self.color_serializer.add_color(&color));
        }
        ast_node_builder
    }

    // port: TypedAstSerializer.ColorTypeSerializer#generateTypePool
    fn generate_type_pool(&mut self) -> TypePool {
        let color_registry = self.color_registry.clone();
        let mismatch_locations_for_debugging =
            color_registry.get_mismatch_locations_for_debugging();
        self.color_serializer.generate_type_pool(
            &|c: &Color| color_registry.get_disambiguation_supertypes(c).clone(),
            Some(&|color: &Color| {
                mismatch_locations_for_debugging
                    .get(&color.get_id())
                    .cloned()
                    .unwrap_or_default()
            }),
        )
    }
}
