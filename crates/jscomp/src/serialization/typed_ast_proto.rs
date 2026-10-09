/*
 * Copyright The Closure Compiler Authors.
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
//   src/com/google/javascript/rhino/typed_ast/typed_ast.proto.

// Generated from src/com/google/javascript/rhino/typed_ast/typed_ast.proto (protoc's Java API shape; D-003, no
// protoc on the host). Wire format and defaults follow proto3; see protobuf.rs.
#![allow(clippy::all, unused_imports, unused_variables, dead_code)]
use super::optimization_jsdoc_proto::*;
use super::protobuf::{
    CodedInputStream, CodedOutputStream, Descriptor, EnumDescriptor, FieldDescriptor, FieldType,
    Message, ProtoEnum, ProtoResult, WireFormat,
};
use super::source_file_proto::*;
use super::types_proto::*;
use closure_rhino::js_string::JsString;
pub use closure_rhino::jscomp_serialization::node_property::NodeProperty;
use std::fmt;
use std::sync::LazyLock;
// port: NodeKind (proto enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum NodeKind {
    #[default]
    NODE_KIND_UNSPECIFIED = 0,
    NUMBER_LITERAL = 201,
    STRING_LITERAL = 202,
    BIGINT_LITERAL = 203,
    TRUE = 204,
    FALSE = 205,
    NULL = 206,
    REGEX_LITERAL = 207,
    ARRAY_LITERAL = 208,
    OBJECT_LITERAL = 209,
    TEMPLATELIT = 210,
    TAGGED_TEMPLATELIT = 211,
    IDENTIFIER = 214,
    THIS = 215,
    ASSIGNMENT = 216,
    COMMA = 217,
    CALL = 218,
    NEW = 219,
    YIELD = 220,
    AWAIT = 221,
    BOOLEAN_OR = 223,
    BOOLEAN_AND = 224,
    HOOK = 225,
    PROPERTY_ACCESS = 226,
    ELEMENT_ACCESS = 227,
    LESS_THAN = 228,
    LESS_THAN_EQUAL = 229,
    GREATER_THAN = 230,
    GREATER_THAN_EQUAL = 231,
    EQUAL = 232,
    TRIPLE_EQUAL = 233,
    NOT_EQUAL = 234,
    NOT_TRIPLE_EQUAL = 235,
    NOT = 236,
    TYPEOF = 237,
    INSTANCEOF = 238,
    IN = 239,
    LEFT_SHIFT = 240,
    RIGHT_SHIFT = 241,
    UNSIGNED_RIGHT_SHIFT = 242,
    ADD = 243,
    SUBTRACT = 244,
    MULTIPLY = 245,
    DIVIDE = 246,
    MODULO = 247,
    EXPONENT = 248,
    ASSIGN_ADD = 249,
    ASSIGN_SUBTRACT = 250,
    ASSIGN_MULTIPLY = 251,
    ASSIGN_DIVIDE = 252,
    ASSIGN_MODULO = 253,
    ASSIGN_EXPONENT = 254,
    ASSIGN_BITWISE_OR = 255,
    ASSIGN_BITWISE_AND = 256,
    ASSIGN_BITWISE_XOR = 257,
    ASSIGN_LEFT_SHIFT = 258,
    ASSIGN_RIGHT_SHIFT = 259,
    ASSIGN_UNSIGNED_RIGHT_SHIFT = 260,
    PRE_INCREMENT = 261,
    POST_INCREMENT = 262,
    PRE_DECREMENT = 263,
    POST_DECREMENT = 264,
    POSITIVE = 265,
    NEGATIVE = 266,
    BITWISE_OR = 267,
    BITWISE_AND = 268,
    BITWISE_XOR = 269,
    BITWISE_NOT = 270,
    VOID = 271,
    DELETE = 272,
    NEW_TARGET = 273,
    COMPUTED_PROP = 274,
    IMPORT_META = 275,
    OPTCHAIN_PROPERTY_ACCESS = 276,
    OPTCHAIN_CALL = 277,
    OPTCHAIN_ELEMENT_ACCESS = 278,
    COALESCE = 279,
    DYNAMIC_IMPORT = 280,
    ASSIGN_OR = 281,
    ASSIGN_AND = 282,
    ASSIGN_COALESCE = 283,
    BREAK_STATEMENT = 400,
    CONTINUE_STATEMENT = 401,
    DEBUGGER_STATEMENT = 402,
    DO_STATEMENT = 403,
    EXPRESSION_STATEMENT = 404,
    FOR_AWAIT_OF_STATEMENT = 405,
    FOR_IN_STATEMENT = 406,
    FOR_OF_STATEMENT = 407,
    FOR_STATEMENT = 408,
    IF_STATEMENT = 409,
    LABELED_STATEMENT = 410,
    RETURN_STATEMENT = 411,
    SWITCH_STATEMENT = 412,
    THROW_STATEMENT = 413,
    TRY_STATEMENT = 414,
    WHILE_STATEMENT = 415,
    BLOCK = 416,
    EMPTY = 417,
    IMPORT = 418,
    EXPORT = 419,
    WITH = 420,
    VAR_DECLARATION = 500,
    LET_DECLARATION = 501,
    CONST_DECLARATION = 502,
    FUNCTION_LITERAL = 503,
    CLASS_LITERAL = 504,
    SOURCE_FILE = 600,
    CASE = 601,
    DEFAULT_CASE = 602,
    CATCH = 603,
    CLASS_MEMBERS = 604,
    METHOD_DECLARATION = 605,
    PARAMETER_LIST = 606,
    RENAMABLE_STRING_KEY = 607,
    QUOTED_STRING_KEY = 608,
    LABELED_NAME = 609,
    ARRAY_PATTERN = 610,
    OBJECT_PATTERN = 611,
    DESTRUCTURING_LHS = 612,
    TEMPLATELIT_SUB = 613,
    TEMPLATELIT_STRING = 614,
    SUPER = 615,
    DEFAULT_VALUE = 616,
    IMPORT_SPECS = 619,
    IMPORT_SPEC = 620,
    IMPORT_STAR = 621,
    EXPORT_SPECS = 622,
    EXPORT_SPEC = 623,
    ITER_REST = 624,
    ITER_SPREAD = 625,
    OBJECT_REST = 626,
    OBJECT_SPREAD = 627,
    RENAMABLE_GETTER_DEF = 628,
    QUOTED_GETTER_DEF = 629,
    RENAMABLE_SETTER_DEF = 630,
    QUOTED_SETTER_DEF = 631,
    MODULE_BODY = 632,
    FIELD_DECLARATION = 633,
    COMPUTED_PROP_FIELD = 634,
    SWITCH_BODY = 635,
    UNRECOGNIZED = -1,
}
impl NodeKind {
    pub const VALUES: [Self; 142] = [
        Self::NODE_KIND_UNSPECIFIED,
        Self::NUMBER_LITERAL,
        Self::STRING_LITERAL,
        Self::BIGINT_LITERAL,
        Self::TRUE,
        Self::FALSE,
        Self::NULL,
        Self::REGEX_LITERAL,
        Self::ARRAY_LITERAL,
        Self::OBJECT_LITERAL,
        Self::TEMPLATELIT,
        Self::TAGGED_TEMPLATELIT,
        Self::IDENTIFIER,
        Self::THIS,
        Self::ASSIGNMENT,
        Self::COMMA,
        Self::CALL,
        Self::NEW,
        Self::YIELD,
        Self::AWAIT,
        Self::BOOLEAN_OR,
        Self::BOOLEAN_AND,
        Self::HOOK,
        Self::PROPERTY_ACCESS,
        Self::ELEMENT_ACCESS,
        Self::LESS_THAN,
        Self::LESS_THAN_EQUAL,
        Self::GREATER_THAN,
        Self::GREATER_THAN_EQUAL,
        Self::EQUAL,
        Self::TRIPLE_EQUAL,
        Self::NOT_EQUAL,
        Self::NOT_TRIPLE_EQUAL,
        Self::NOT,
        Self::TYPEOF,
        Self::INSTANCEOF,
        Self::IN,
        Self::LEFT_SHIFT,
        Self::RIGHT_SHIFT,
        Self::UNSIGNED_RIGHT_SHIFT,
        Self::ADD,
        Self::SUBTRACT,
        Self::MULTIPLY,
        Self::DIVIDE,
        Self::MODULO,
        Self::EXPONENT,
        Self::ASSIGN_ADD,
        Self::ASSIGN_SUBTRACT,
        Self::ASSIGN_MULTIPLY,
        Self::ASSIGN_DIVIDE,
        Self::ASSIGN_MODULO,
        Self::ASSIGN_EXPONENT,
        Self::ASSIGN_BITWISE_OR,
        Self::ASSIGN_BITWISE_AND,
        Self::ASSIGN_BITWISE_XOR,
        Self::ASSIGN_LEFT_SHIFT,
        Self::ASSIGN_RIGHT_SHIFT,
        Self::ASSIGN_UNSIGNED_RIGHT_SHIFT,
        Self::PRE_INCREMENT,
        Self::POST_INCREMENT,
        Self::PRE_DECREMENT,
        Self::POST_DECREMENT,
        Self::POSITIVE,
        Self::NEGATIVE,
        Self::BITWISE_OR,
        Self::BITWISE_AND,
        Self::BITWISE_XOR,
        Self::BITWISE_NOT,
        Self::VOID,
        Self::DELETE,
        Self::NEW_TARGET,
        Self::COMPUTED_PROP,
        Self::IMPORT_META,
        Self::OPTCHAIN_PROPERTY_ACCESS,
        Self::OPTCHAIN_CALL,
        Self::OPTCHAIN_ELEMENT_ACCESS,
        Self::COALESCE,
        Self::DYNAMIC_IMPORT,
        Self::ASSIGN_OR,
        Self::ASSIGN_AND,
        Self::ASSIGN_COALESCE,
        Self::BREAK_STATEMENT,
        Self::CONTINUE_STATEMENT,
        Self::DEBUGGER_STATEMENT,
        Self::DO_STATEMENT,
        Self::EXPRESSION_STATEMENT,
        Self::FOR_AWAIT_OF_STATEMENT,
        Self::FOR_IN_STATEMENT,
        Self::FOR_OF_STATEMENT,
        Self::FOR_STATEMENT,
        Self::IF_STATEMENT,
        Self::LABELED_STATEMENT,
        Self::RETURN_STATEMENT,
        Self::SWITCH_STATEMENT,
        Self::THROW_STATEMENT,
        Self::TRY_STATEMENT,
        Self::WHILE_STATEMENT,
        Self::BLOCK,
        Self::EMPTY,
        Self::IMPORT,
        Self::EXPORT,
        Self::WITH,
        Self::VAR_DECLARATION,
        Self::LET_DECLARATION,
        Self::CONST_DECLARATION,
        Self::FUNCTION_LITERAL,
        Self::CLASS_LITERAL,
        Self::SOURCE_FILE,
        Self::CASE,
        Self::DEFAULT_CASE,
        Self::CATCH,
        Self::CLASS_MEMBERS,
        Self::METHOD_DECLARATION,
        Self::PARAMETER_LIST,
        Self::RENAMABLE_STRING_KEY,
        Self::QUOTED_STRING_KEY,
        Self::LABELED_NAME,
        Self::ARRAY_PATTERN,
        Self::OBJECT_PATTERN,
        Self::DESTRUCTURING_LHS,
        Self::TEMPLATELIT_SUB,
        Self::TEMPLATELIT_STRING,
        Self::SUPER,
        Self::DEFAULT_VALUE,
        Self::IMPORT_SPECS,
        Self::IMPORT_SPEC,
        Self::IMPORT_STAR,
        Self::EXPORT_SPECS,
        Self::EXPORT_SPEC,
        Self::ITER_REST,
        Self::ITER_SPREAD,
        Self::OBJECT_REST,
        Self::OBJECT_SPREAD,
        Self::RENAMABLE_GETTER_DEF,
        Self::QUOTED_GETTER_DEF,
        Self::RENAMABLE_SETTER_DEF,
        Self::QUOTED_SETTER_DEF,
        Self::MODULE_BODY,
        Self::FIELD_DECLARATION,
        Self::COMPUTED_PROP_FIELD,
        Self::SWITCH_BODY,
        Self::UNRECOGNIZED,
    ];
    // port: Enum#ordinal
    pub fn ordinal(self) -> usize {
        Self::VALUES
            .iter()
            .position(|value| *value == self)
            .unwrap()
    }
    // port: NodeKind#getNumber
    pub fn get_number(self) -> i32 {
        assert!(
            self != Self::UNRECOGNIZED,
            "Can't get the number of an unknown enum value."
        );
        self as i32
    }
    // port: Enum#name
    pub fn name(self) -> &'static str {
        match self {
            Self::NODE_KIND_UNSPECIFIED => "NODE_KIND_UNSPECIFIED",
            Self::NUMBER_LITERAL => "NUMBER_LITERAL",
            Self::STRING_LITERAL => "STRING_LITERAL",
            Self::BIGINT_LITERAL => "BIGINT_LITERAL",
            Self::TRUE => "TRUE",
            Self::FALSE => "FALSE",
            Self::NULL => "NULL",
            Self::REGEX_LITERAL => "REGEX_LITERAL",
            Self::ARRAY_LITERAL => "ARRAY_LITERAL",
            Self::OBJECT_LITERAL => "OBJECT_LITERAL",
            Self::TEMPLATELIT => "TEMPLATELIT",
            Self::TAGGED_TEMPLATELIT => "TAGGED_TEMPLATELIT",
            Self::IDENTIFIER => "IDENTIFIER",
            Self::THIS => "THIS",
            Self::ASSIGNMENT => "ASSIGNMENT",
            Self::COMMA => "COMMA",
            Self::CALL => "CALL",
            Self::NEW => "NEW",
            Self::YIELD => "YIELD",
            Self::AWAIT => "AWAIT",
            Self::BOOLEAN_OR => "BOOLEAN_OR",
            Self::BOOLEAN_AND => "BOOLEAN_AND",
            Self::HOOK => "HOOK",
            Self::PROPERTY_ACCESS => "PROPERTY_ACCESS",
            Self::ELEMENT_ACCESS => "ELEMENT_ACCESS",
            Self::LESS_THAN => "LESS_THAN",
            Self::LESS_THAN_EQUAL => "LESS_THAN_EQUAL",
            Self::GREATER_THAN => "GREATER_THAN",
            Self::GREATER_THAN_EQUAL => "GREATER_THAN_EQUAL",
            Self::EQUAL => "EQUAL",
            Self::TRIPLE_EQUAL => "TRIPLE_EQUAL",
            Self::NOT_EQUAL => "NOT_EQUAL",
            Self::NOT_TRIPLE_EQUAL => "NOT_TRIPLE_EQUAL",
            Self::NOT => "NOT",
            Self::TYPEOF => "TYPEOF",
            Self::INSTANCEOF => "INSTANCEOF",
            Self::IN => "IN",
            Self::LEFT_SHIFT => "LEFT_SHIFT",
            Self::RIGHT_SHIFT => "RIGHT_SHIFT",
            Self::UNSIGNED_RIGHT_SHIFT => "UNSIGNED_RIGHT_SHIFT",
            Self::ADD => "ADD",
            Self::SUBTRACT => "SUBTRACT",
            Self::MULTIPLY => "MULTIPLY",
            Self::DIVIDE => "DIVIDE",
            Self::MODULO => "MODULO",
            Self::EXPONENT => "EXPONENT",
            Self::ASSIGN_ADD => "ASSIGN_ADD",
            Self::ASSIGN_SUBTRACT => "ASSIGN_SUBTRACT",
            Self::ASSIGN_MULTIPLY => "ASSIGN_MULTIPLY",
            Self::ASSIGN_DIVIDE => "ASSIGN_DIVIDE",
            Self::ASSIGN_MODULO => "ASSIGN_MODULO",
            Self::ASSIGN_EXPONENT => "ASSIGN_EXPONENT",
            Self::ASSIGN_BITWISE_OR => "ASSIGN_BITWISE_OR",
            Self::ASSIGN_BITWISE_AND => "ASSIGN_BITWISE_AND",
            Self::ASSIGN_BITWISE_XOR => "ASSIGN_BITWISE_XOR",
            Self::ASSIGN_LEFT_SHIFT => "ASSIGN_LEFT_SHIFT",
            Self::ASSIGN_RIGHT_SHIFT => "ASSIGN_RIGHT_SHIFT",
            Self::ASSIGN_UNSIGNED_RIGHT_SHIFT => "ASSIGN_UNSIGNED_RIGHT_SHIFT",
            Self::PRE_INCREMENT => "PRE_INCREMENT",
            Self::POST_INCREMENT => "POST_INCREMENT",
            Self::PRE_DECREMENT => "PRE_DECREMENT",
            Self::POST_DECREMENT => "POST_DECREMENT",
            Self::POSITIVE => "POSITIVE",
            Self::NEGATIVE => "NEGATIVE",
            Self::BITWISE_OR => "BITWISE_OR",
            Self::BITWISE_AND => "BITWISE_AND",
            Self::BITWISE_XOR => "BITWISE_XOR",
            Self::BITWISE_NOT => "BITWISE_NOT",
            Self::VOID => "VOID",
            Self::DELETE => "DELETE",
            Self::NEW_TARGET => "NEW_TARGET",
            Self::COMPUTED_PROP => "COMPUTED_PROP",
            Self::IMPORT_META => "IMPORT_META",
            Self::OPTCHAIN_PROPERTY_ACCESS => "OPTCHAIN_PROPERTY_ACCESS",
            Self::OPTCHAIN_CALL => "OPTCHAIN_CALL",
            Self::OPTCHAIN_ELEMENT_ACCESS => "OPTCHAIN_ELEMENT_ACCESS",
            Self::COALESCE => "COALESCE",
            Self::DYNAMIC_IMPORT => "DYNAMIC_IMPORT",
            Self::ASSIGN_OR => "ASSIGN_OR",
            Self::ASSIGN_AND => "ASSIGN_AND",
            Self::ASSIGN_COALESCE => "ASSIGN_COALESCE",
            Self::BREAK_STATEMENT => "BREAK_STATEMENT",
            Self::CONTINUE_STATEMENT => "CONTINUE_STATEMENT",
            Self::DEBUGGER_STATEMENT => "DEBUGGER_STATEMENT",
            Self::DO_STATEMENT => "DO_STATEMENT",
            Self::EXPRESSION_STATEMENT => "EXPRESSION_STATEMENT",
            Self::FOR_AWAIT_OF_STATEMENT => "FOR_AWAIT_OF_STATEMENT",
            Self::FOR_IN_STATEMENT => "FOR_IN_STATEMENT",
            Self::FOR_OF_STATEMENT => "FOR_OF_STATEMENT",
            Self::FOR_STATEMENT => "FOR_STATEMENT",
            Self::IF_STATEMENT => "IF_STATEMENT",
            Self::LABELED_STATEMENT => "LABELED_STATEMENT",
            Self::RETURN_STATEMENT => "RETURN_STATEMENT",
            Self::SWITCH_STATEMENT => "SWITCH_STATEMENT",
            Self::THROW_STATEMENT => "THROW_STATEMENT",
            Self::TRY_STATEMENT => "TRY_STATEMENT",
            Self::WHILE_STATEMENT => "WHILE_STATEMENT",
            Self::BLOCK => "BLOCK",
            Self::EMPTY => "EMPTY",
            Self::IMPORT => "IMPORT",
            Self::EXPORT => "EXPORT",
            Self::WITH => "WITH",
            Self::VAR_DECLARATION => "VAR_DECLARATION",
            Self::LET_DECLARATION => "LET_DECLARATION",
            Self::CONST_DECLARATION => "CONST_DECLARATION",
            Self::FUNCTION_LITERAL => "FUNCTION_LITERAL",
            Self::CLASS_LITERAL => "CLASS_LITERAL",
            Self::SOURCE_FILE => "SOURCE_FILE",
            Self::CASE => "CASE",
            Self::DEFAULT_CASE => "DEFAULT_CASE",
            Self::CATCH => "CATCH",
            Self::CLASS_MEMBERS => "CLASS_MEMBERS",
            Self::METHOD_DECLARATION => "METHOD_DECLARATION",
            Self::PARAMETER_LIST => "PARAMETER_LIST",
            Self::RENAMABLE_STRING_KEY => "RENAMABLE_STRING_KEY",
            Self::QUOTED_STRING_KEY => "QUOTED_STRING_KEY",
            Self::LABELED_NAME => "LABELED_NAME",
            Self::ARRAY_PATTERN => "ARRAY_PATTERN",
            Self::OBJECT_PATTERN => "OBJECT_PATTERN",
            Self::DESTRUCTURING_LHS => "DESTRUCTURING_LHS",
            Self::TEMPLATELIT_SUB => "TEMPLATELIT_SUB",
            Self::TEMPLATELIT_STRING => "TEMPLATELIT_STRING",
            Self::SUPER => "SUPER",
            Self::DEFAULT_VALUE => "DEFAULT_VALUE",
            Self::IMPORT_SPECS => "IMPORT_SPECS",
            Self::IMPORT_SPEC => "IMPORT_SPEC",
            Self::IMPORT_STAR => "IMPORT_STAR",
            Self::EXPORT_SPECS => "EXPORT_SPECS",
            Self::EXPORT_SPEC => "EXPORT_SPEC",
            Self::ITER_REST => "ITER_REST",
            Self::ITER_SPREAD => "ITER_SPREAD",
            Self::OBJECT_REST => "OBJECT_REST",
            Self::OBJECT_SPREAD => "OBJECT_SPREAD",
            Self::RENAMABLE_GETTER_DEF => "RENAMABLE_GETTER_DEF",
            Self::QUOTED_GETTER_DEF => "QUOTED_GETTER_DEF",
            Self::RENAMABLE_SETTER_DEF => "RENAMABLE_SETTER_DEF",
            Self::QUOTED_SETTER_DEF => "QUOTED_SETTER_DEF",
            Self::MODULE_BODY => "MODULE_BODY",
            Self::FIELD_DECLARATION => "FIELD_DECLARATION",
            Self::COMPUTED_PROP_FIELD => "COMPUTED_PROP_FIELD",
            Self::SWITCH_BODY => "SWITCH_BODY",
            Self::UNRECOGNIZED => "UNRECOGNIZED",
        }
    }
    // port: NodeKind#valueOf(String) (`None` where Java throws IllegalArgumentException)
    pub fn value_of(name: &str) -> Option<Self> {
        match name {
            "NODE_KIND_UNSPECIFIED" => Some(Self::NODE_KIND_UNSPECIFIED),
            "NUMBER_LITERAL" => Some(Self::NUMBER_LITERAL),
            "STRING_LITERAL" => Some(Self::STRING_LITERAL),
            "BIGINT_LITERAL" => Some(Self::BIGINT_LITERAL),
            "TRUE" => Some(Self::TRUE),
            "FALSE" => Some(Self::FALSE),
            "NULL" => Some(Self::NULL),
            "REGEX_LITERAL" => Some(Self::REGEX_LITERAL),
            "ARRAY_LITERAL" => Some(Self::ARRAY_LITERAL),
            "OBJECT_LITERAL" => Some(Self::OBJECT_LITERAL),
            "TEMPLATELIT" => Some(Self::TEMPLATELIT),
            "TAGGED_TEMPLATELIT" => Some(Self::TAGGED_TEMPLATELIT),
            "IDENTIFIER" => Some(Self::IDENTIFIER),
            "THIS" => Some(Self::THIS),
            "ASSIGNMENT" => Some(Self::ASSIGNMENT),
            "COMMA" => Some(Self::COMMA),
            "CALL" => Some(Self::CALL),
            "NEW" => Some(Self::NEW),
            "YIELD" => Some(Self::YIELD),
            "AWAIT" => Some(Self::AWAIT),
            "BOOLEAN_OR" => Some(Self::BOOLEAN_OR),
            "BOOLEAN_AND" => Some(Self::BOOLEAN_AND),
            "HOOK" => Some(Self::HOOK),
            "PROPERTY_ACCESS" => Some(Self::PROPERTY_ACCESS),
            "ELEMENT_ACCESS" => Some(Self::ELEMENT_ACCESS),
            "LESS_THAN" => Some(Self::LESS_THAN),
            "LESS_THAN_EQUAL" => Some(Self::LESS_THAN_EQUAL),
            "GREATER_THAN" => Some(Self::GREATER_THAN),
            "GREATER_THAN_EQUAL" => Some(Self::GREATER_THAN_EQUAL),
            "EQUAL" => Some(Self::EQUAL),
            "TRIPLE_EQUAL" => Some(Self::TRIPLE_EQUAL),
            "NOT_EQUAL" => Some(Self::NOT_EQUAL),
            "NOT_TRIPLE_EQUAL" => Some(Self::NOT_TRIPLE_EQUAL),
            "NOT" => Some(Self::NOT),
            "TYPEOF" => Some(Self::TYPEOF),
            "INSTANCEOF" => Some(Self::INSTANCEOF),
            "IN" => Some(Self::IN),
            "LEFT_SHIFT" => Some(Self::LEFT_SHIFT),
            "RIGHT_SHIFT" => Some(Self::RIGHT_SHIFT),
            "UNSIGNED_RIGHT_SHIFT" => Some(Self::UNSIGNED_RIGHT_SHIFT),
            "ADD" => Some(Self::ADD),
            "SUBTRACT" => Some(Self::SUBTRACT),
            "MULTIPLY" => Some(Self::MULTIPLY),
            "DIVIDE" => Some(Self::DIVIDE),
            "MODULO" => Some(Self::MODULO),
            "EXPONENT" => Some(Self::EXPONENT),
            "ASSIGN_ADD" => Some(Self::ASSIGN_ADD),
            "ASSIGN_SUBTRACT" => Some(Self::ASSIGN_SUBTRACT),
            "ASSIGN_MULTIPLY" => Some(Self::ASSIGN_MULTIPLY),
            "ASSIGN_DIVIDE" => Some(Self::ASSIGN_DIVIDE),
            "ASSIGN_MODULO" => Some(Self::ASSIGN_MODULO),
            "ASSIGN_EXPONENT" => Some(Self::ASSIGN_EXPONENT),
            "ASSIGN_BITWISE_OR" => Some(Self::ASSIGN_BITWISE_OR),
            "ASSIGN_BITWISE_AND" => Some(Self::ASSIGN_BITWISE_AND),
            "ASSIGN_BITWISE_XOR" => Some(Self::ASSIGN_BITWISE_XOR),
            "ASSIGN_LEFT_SHIFT" => Some(Self::ASSIGN_LEFT_SHIFT),
            "ASSIGN_RIGHT_SHIFT" => Some(Self::ASSIGN_RIGHT_SHIFT),
            "ASSIGN_UNSIGNED_RIGHT_SHIFT" => Some(Self::ASSIGN_UNSIGNED_RIGHT_SHIFT),
            "PRE_INCREMENT" => Some(Self::PRE_INCREMENT),
            "POST_INCREMENT" => Some(Self::POST_INCREMENT),
            "PRE_DECREMENT" => Some(Self::PRE_DECREMENT),
            "POST_DECREMENT" => Some(Self::POST_DECREMENT),
            "POSITIVE" => Some(Self::POSITIVE),
            "NEGATIVE" => Some(Self::NEGATIVE),
            "BITWISE_OR" => Some(Self::BITWISE_OR),
            "BITWISE_AND" => Some(Self::BITWISE_AND),
            "BITWISE_XOR" => Some(Self::BITWISE_XOR),
            "BITWISE_NOT" => Some(Self::BITWISE_NOT),
            "VOID" => Some(Self::VOID),
            "DELETE" => Some(Self::DELETE),
            "NEW_TARGET" => Some(Self::NEW_TARGET),
            "COMPUTED_PROP" => Some(Self::COMPUTED_PROP),
            "IMPORT_META" => Some(Self::IMPORT_META),
            "OPTCHAIN_PROPERTY_ACCESS" => Some(Self::OPTCHAIN_PROPERTY_ACCESS),
            "OPTCHAIN_CALL" => Some(Self::OPTCHAIN_CALL),
            "OPTCHAIN_ELEMENT_ACCESS" => Some(Self::OPTCHAIN_ELEMENT_ACCESS),
            "COALESCE" => Some(Self::COALESCE),
            "DYNAMIC_IMPORT" => Some(Self::DYNAMIC_IMPORT),
            "ASSIGN_OR" => Some(Self::ASSIGN_OR),
            "ASSIGN_AND" => Some(Self::ASSIGN_AND),
            "ASSIGN_COALESCE" => Some(Self::ASSIGN_COALESCE),
            "BREAK_STATEMENT" => Some(Self::BREAK_STATEMENT),
            "CONTINUE_STATEMENT" => Some(Self::CONTINUE_STATEMENT),
            "DEBUGGER_STATEMENT" => Some(Self::DEBUGGER_STATEMENT),
            "DO_STATEMENT" => Some(Self::DO_STATEMENT),
            "EXPRESSION_STATEMENT" => Some(Self::EXPRESSION_STATEMENT),
            "FOR_AWAIT_OF_STATEMENT" => Some(Self::FOR_AWAIT_OF_STATEMENT),
            "FOR_IN_STATEMENT" => Some(Self::FOR_IN_STATEMENT),
            "FOR_OF_STATEMENT" => Some(Self::FOR_OF_STATEMENT),
            "FOR_STATEMENT" => Some(Self::FOR_STATEMENT),
            "IF_STATEMENT" => Some(Self::IF_STATEMENT),
            "LABELED_STATEMENT" => Some(Self::LABELED_STATEMENT),
            "RETURN_STATEMENT" => Some(Self::RETURN_STATEMENT),
            "SWITCH_STATEMENT" => Some(Self::SWITCH_STATEMENT),
            "THROW_STATEMENT" => Some(Self::THROW_STATEMENT),
            "TRY_STATEMENT" => Some(Self::TRY_STATEMENT),
            "WHILE_STATEMENT" => Some(Self::WHILE_STATEMENT),
            "BLOCK" => Some(Self::BLOCK),
            "EMPTY" => Some(Self::EMPTY),
            "IMPORT" => Some(Self::IMPORT),
            "EXPORT" => Some(Self::EXPORT),
            "WITH" => Some(Self::WITH),
            "VAR_DECLARATION" => Some(Self::VAR_DECLARATION),
            "LET_DECLARATION" => Some(Self::LET_DECLARATION),
            "CONST_DECLARATION" => Some(Self::CONST_DECLARATION),
            "FUNCTION_LITERAL" => Some(Self::FUNCTION_LITERAL),
            "CLASS_LITERAL" => Some(Self::CLASS_LITERAL),
            "SOURCE_FILE" => Some(Self::SOURCE_FILE),
            "CASE" => Some(Self::CASE),
            "DEFAULT_CASE" => Some(Self::DEFAULT_CASE),
            "CATCH" => Some(Self::CATCH),
            "CLASS_MEMBERS" => Some(Self::CLASS_MEMBERS),
            "METHOD_DECLARATION" => Some(Self::METHOD_DECLARATION),
            "PARAMETER_LIST" => Some(Self::PARAMETER_LIST),
            "RENAMABLE_STRING_KEY" => Some(Self::RENAMABLE_STRING_KEY),
            "QUOTED_STRING_KEY" => Some(Self::QUOTED_STRING_KEY),
            "LABELED_NAME" => Some(Self::LABELED_NAME),
            "ARRAY_PATTERN" => Some(Self::ARRAY_PATTERN),
            "OBJECT_PATTERN" => Some(Self::OBJECT_PATTERN),
            "DESTRUCTURING_LHS" => Some(Self::DESTRUCTURING_LHS),
            "TEMPLATELIT_SUB" => Some(Self::TEMPLATELIT_SUB),
            "TEMPLATELIT_STRING" => Some(Self::TEMPLATELIT_STRING),
            "SUPER" => Some(Self::SUPER),
            "DEFAULT_VALUE" => Some(Self::DEFAULT_VALUE),
            "IMPORT_SPECS" => Some(Self::IMPORT_SPECS),
            "IMPORT_SPEC" => Some(Self::IMPORT_SPEC),
            "IMPORT_STAR" => Some(Self::IMPORT_STAR),
            "EXPORT_SPECS" => Some(Self::EXPORT_SPECS),
            "EXPORT_SPEC" => Some(Self::EXPORT_SPEC),
            "ITER_REST" => Some(Self::ITER_REST),
            "ITER_SPREAD" => Some(Self::ITER_SPREAD),
            "OBJECT_REST" => Some(Self::OBJECT_REST),
            "OBJECT_SPREAD" => Some(Self::OBJECT_SPREAD),
            "RENAMABLE_GETTER_DEF" => Some(Self::RENAMABLE_GETTER_DEF),
            "QUOTED_GETTER_DEF" => Some(Self::QUOTED_GETTER_DEF),
            "RENAMABLE_SETTER_DEF" => Some(Self::RENAMABLE_SETTER_DEF),
            "QUOTED_SETTER_DEF" => Some(Self::QUOTED_SETTER_DEF),
            "MODULE_BODY" => Some(Self::MODULE_BODY),
            "FIELD_DECLARATION" => Some(Self::FIELD_DECLARATION),
            "COMPUTED_PROP_FIELD" => Some(Self::COMPUTED_PROP_FIELD),
            "SWITCH_BODY" => Some(Self::SWITCH_BODY),
            "UNRECOGNIZED" => Some(Self::UNRECOGNIZED),
            _ => None,
        }
    }
    // port: NodeKind#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::NODE_KIND_UNSPECIFIED),
            201 => Some(Self::NUMBER_LITERAL),
            202 => Some(Self::STRING_LITERAL),
            203 => Some(Self::BIGINT_LITERAL),
            204 => Some(Self::TRUE),
            205 => Some(Self::FALSE),
            206 => Some(Self::NULL),
            207 => Some(Self::REGEX_LITERAL),
            208 => Some(Self::ARRAY_LITERAL),
            209 => Some(Self::OBJECT_LITERAL),
            210 => Some(Self::TEMPLATELIT),
            211 => Some(Self::TAGGED_TEMPLATELIT),
            214 => Some(Self::IDENTIFIER),
            215 => Some(Self::THIS),
            216 => Some(Self::ASSIGNMENT),
            217 => Some(Self::COMMA),
            218 => Some(Self::CALL),
            219 => Some(Self::NEW),
            220 => Some(Self::YIELD),
            221 => Some(Self::AWAIT),
            223 => Some(Self::BOOLEAN_OR),
            224 => Some(Self::BOOLEAN_AND),
            225 => Some(Self::HOOK),
            226 => Some(Self::PROPERTY_ACCESS),
            227 => Some(Self::ELEMENT_ACCESS),
            228 => Some(Self::LESS_THAN),
            229 => Some(Self::LESS_THAN_EQUAL),
            230 => Some(Self::GREATER_THAN),
            231 => Some(Self::GREATER_THAN_EQUAL),
            232 => Some(Self::EQUAL),
            233 => Some(Self::TRIPLE_EQUAL),
            234 => Some(Self::NOT_EQUAL),
            235 => Some(Self::NOT_TRIPLE_EQUAL),
            236 => Some(Self::NOT),
            237 => Some(Self::TYPEOF),
            238 => Some(Self::INSTANCEOF),
            239 => Some(Self::IN),
            240 => Some(Self::LEFT_SHIFT),
            241 => Some(Self::RIGHT_SHIFT),
            242 => Some(Self::UNSIGNED_RIGHT_SHIFT),
            243 => Some(Self::ADD),
            244 => Some(Self::SUBTRACT),
            245 => Some(Self::MULTIPLY),
            246 => Some(Self::DIVIDE),
            247 => Some(Self::MODULO),
            248 => Some(Self::EXPONENT),
            249 => Some(Self::ASSIGN_ADD),
            250 => Some(Self::ASSIGN_SUBTRACT),
            251 => Some(Self::ASSIGN_MULTIPLY),
            252 => Some(Self::ASSIGN_DIVIDE),
            253 => Some(Self::ASSIGN_MODULO),
            254 => Some(Self::ASSIGN_EXPONENT),
            255 => Some(Self::ASSIGN_BITWISE_OR),
            256 => Some(Self::ASSIGN_BITWISE_AND),
            257 => Some(Self::ASSIGN_BITWISE_XOR),
            258 => Some(Self::ASSIGN_LEFT_SHIFT),
            259 => Some(Self::ASSIGN_RIGHT_SHIFT),
            260 => Some(Self::ASSIGN_UNSIGNED_RIGHT_SHIFT),
            261 => Some(Self::PRE_INCREMENT),
            262 => Some(Self::POST_INCREMENT),
            263 => Some(Self::PRE_DECREMENT),
            264 => Some(Self::POST_DECREMENT),
            265 => Some(Self::POSITIVE),
            266 => Some(Self::NEGATIVE),
            267 => Some(Self::BITWISE_OR),
            268 => Some(Self::BITWISE_AND),
            269 => Some(Self::BITWISE_XOR),
            270 => Some(Self::BITWISE_NOT),
            271 => Some(Self::VOID),
            272 => Some(Self::DELETE),
            273 => Some(Self::NEW_TARGET),
            274 => Some(Self::COMPUTED_PROP),
            275 => Some(Self::IMPORT_META),
            276 => Some(Self::OPTCHAIN_PROPERTY_ACCESS),
            277 => Some(Self::OPTCHAIN_CALL),
            278 => Some(Self::OPTCHAIN_ELEMENT_ACCESS),
            279 => Some(Self::COALESCE),
            280 => Some(Self::DYNAMIC_IMPORT),
            281 => Some(Self::ASSIGN_OR),
            282 => Some(Self::ASSIGN_AND),
            283 => Some(Self::ASSIGN_COALESCE),
            400 => Some(Self::BREAK_STATEMENT),
            401 => Some(Self::CONTINUE_STATEMENT),
            402 => Some(Self::DEBUGGER_STATEMENT),
            403 => Some(Self::DO_STATEMENT),
            404 => Some(Self::EXPRESSION_STATEMENT),
            405 => Some(Self::FOR_AWAIT_OF_STATEMENT),
            406 => Some(Self::FOR_IN_STATEMENT),
            407 => Some(Self::FOR_OF_STATEMENT),
            408 => Some(Self::FOR_STATEMENT),
            409 => Some(Self::IF_STATEMENT),
            410 => Some(Self::LABELED_STATEMENT),
            411 => Some(Self::RETURN_STATEMENT),
            412 => Some(Self::SWITCH_STATEMENT),
            413 => Some(Self::THROW_STATEMENT),
            414 => Some(Self::TRY_STATEMENT),
            415 => Some(Self::WHILE_STATEMENT),
            416 => Some(Self::BLOCK),
            417 => Some(Self::EMPTY),
            418 => Some(Self::IMPORT),
            419 => Some(Self::EXPORT),
            420 => Some(Self::WITH),
            500 => Some(Self::VAR_DECLARATION),
            501 => Some(Self::LET_DECLARATION),
            502 => Some(Self::CONST_DECLARATION),
            503 => Some(Self::FUNCTION_LITERAL),
            504 => Some(Self::CLASS_LITERAL),
            600 => Some(Self::SOURCE_FILE),
            601 => Some(Self::CASE),
            602 => Some(Self::DEFAULT_CASE),
            603 => Some(Self::CATCH),
            604 => Some(Self::CLASS_MEMBERS),
            605 => Some(Self::METHOD_DECLARATION),
            606 => Some(Self::PARAMETER_LIST),
            607 => Some(Self::RENAMABLE_STRING_KEY),
            608 => Some(Self::QUOTED_STRING_KEY),
            609 => Some(Self::LABELED_NAME),
            610 => Some(Self::ARRAY_PATTERN),
            611 => Some(Self::OBJECT_PATTERN),
            612 => Some(Self::DESTRUCTURING_LHS),
            613 => Some(Self::TEMPLATELIT_SUB),
            614 => Some(Self::TEMPLATELIT_STRING),
            615 => Some(Self::SUPER),
            616 => Some(Self::DEFAULT_VALUE),
            619 => Some(Self::IMPORT_SPECS),
            620 => Some(Self::IMPORT_SPEC),
            621 => Some(Self::IMPORT_STAR),
            622 => Some(Self::EXPORT_SPECS),
            623 => Some(Self::EXPORT_SPEC),
            624 => Some(Self::ITER_REST),
            625 => Some(Self::ITER_SPREAD),
            626 => Some(Self::OBJECT_REST),
            627 => Some(Self::OBJECT_SPREAD),
            628 => Some(Self::RENAMABLE_GETTER_DEF),
            629 => Some(Self::QUOTED_GETTER_DEF),
            630 => Some(Self::RENAMABLE_SETTER_DEF),
            631 => Some(Self::QUOTED_SETTER_DEF),
            632 => Some(Self::MODULE_BODY),
            633 => Some(Self::FIELD_DECLARATION),
            634 => Some(Self::COMPUTED_PROP_FIELD),
            635 => Some(Self::SWITCH_BODY),
            _ => None,
        }
    }
}
impl ProtoEnum for NodeKind {
    fn get_number(self) -> i32 {
        self.get_number()
    }
    fn for_number_or_unrecognized(value: i32) -> Self {
        Self::for_number(value).unwrap_or(Self::UNRECOGNIZED)
    }
}
impl fmt::Display for NodeKind {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: TypedAst (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TypedAst {
    pub type_pool: Option<TypePool>,
    pub string_pool: Option<StringPoolProto>,
    pub extern_ast: Vec<LazyAst>,
    pub code_ast: Vec<LazyAst>,
    pub source_file_pool: Option<SourceFilePool>,
    pub externs_summary: Option<ExternsSummary>,
    pub runtime_library_to_inject: Vec<String>,
}
static TYPEDAST_DEFAULT_INSTANCE: LazyLock<TypedAst> = LazyLock::new(TypedAst::default);
impl TypedAst {
    pub const TYPE_POOL_FIELD_NUMBER: i32 = 1;
    pub const STRING_POOL_FIELD_NUMBER: i32 = 2;
    pub const EXTERN_AST_FIELD_NUMBER: i32 = 3;
    pub const CODE_AST_FIELD_NUMBER: i32 = 4;
    pub const SOURCE_FILE_POOL_FIELD_NUMBER: i32 = 5;
    pub const EXTERNS_SUMMARY_FIELD_NUMBER: i32 = 6;
    pub const RUNTIME_LIBRARY_TO_INJECT_FIELD_NUMBER: i32 = 7;
    // port: TypedAst#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: TypedAst#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &TYPEDAST_DEFAULT_INSTANCE
    }
    // port: TypedAst#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: TypedAst.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: TypedAst.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: TypedAst#hasTypePool
    pub fn has_type_pool(&self) -> bool {
        self.type_pool.is_some()
    }
    // port: TypedAst#getTypePool
    pub fn get_type_pool(&self) -> &TypePool {
        self.type_pool
            .as_ref()
            .unwrap_or_else(|| TypePool::default_instance_ref())
    }
    // port: TypedAst.Builder#setTypePool
    pub fn set_type_pool(mut self, value: TypePool) -> Self {
        self.type_pool = Some(value);
        self
    }
    // port: TypedAst.Builder#clearTypePool
    pub fn clear_type_pool(mut self) -> Self {
        self.type_pool = None;
        self
    }
    // port: TypedAst#hasStringPool
    pub fn has_string_pool(&self) -> bool {
        self.string_pool.is_some()
    }
    // port: TypedAst#getStringPool
    pub fn get_string_pool(&self) -> &StringPoolProto {
        self.string_pool
            .as_ref()
            .unwrap_or_else(|| StringPoolProto::default_instance_ref())
    }
    // port: TypedAst.Builder#setStringPool
    pub fn set_string_pool(mut self, value: StringPoolProto) -> Self {
        self.string_pool = Some(value);
        self
    }
    // port: TypedAst.Builder#clearStringPool
    pub fn clear_string_pool(mut self) -> Self {
        self.string_pool = None;
        self
    }
    // port: TypedAst#getExternAstList
    pub fn get_extern_ast_list(&self) -> &[LazyAst] {
        &self.extern_ast
    }
    // port: TypedAst#getExternAstCount
    pub fn get_extern_ast_count(&self) -> i32 {
        self.extern_ast.len() as i32
    }
    // port: TypedAst#getExternAst(int)
    pub fn get_extern_ast(&self, index: i32) -> &LazyAst {
        &self.extern_ast[index as usize]
    }
    // port: TypedAst.Builder#addExternAst
    pub fn add_extern_ast(mut self, value: LazyAst) -> Self {
        self.extern_ast.push(value);
        self
    }
    // port: TypedAst.Builder#addAllExternAst
    pub fn add_all_extern_ast(mut self, values: impl IntoIterator<Item = LazyAst>) -> Self {
        self.extern_ast.extend(values);
        self
    }
    // port: TypedAst.Builder#clearExternAst
    pub fn clear_extern_ast(mut self) -> Self {
        self.extern_ast.clear();
        self
    }
    // port: TypedAst#getCodeAstList
    pub fn get_code_ast_list(&self) -> &[LazyAst] {
        &self.code_ast
    }
    // port: TypedAst#getCodeAstCount
    pub fn get_code_ast_count(&self) -> i32 {
        self.code_ast.len() as i32
    }
    // port: TypedAst#getCodeAst(int)
    pub fn get_code_ast(&self, index: i32) -> &LazyAst {
        &self.code_ast[index as usize]
    }
    // port: TypedAst.Builder#addCodeAst
    pub fn add_code_ast(mut self, value: LazyAst) -> Self {
        self.code_ast.push(value);
        self
    }
    // port: TypedAst.Builder#addAllCodeAst
    pub fn add_all_code_ast(mut self, values: impl IntoIterator<Item = LazyAst>) -> Self {
        self.code_ast.extend(values);
        self
    }
    // port: TypedAst.Builder#clearCodeAst
    pub fn clear_code_ast(mut self) -> Self {
        self.code_ast.clear();
        self
    }
    // port: TypedAst#hasSourceFilePool
    pub fn has_source_file_pool(&self) -> bool {
        self.source_file_pool.is_some()
    }
    // port: TypedAst#getSourceFilePool
    pub fn get_source_file_pool(&self) -> &SourceFilePool {
        self.source_file_pool
            .as_ref()
            .unwrap_or_else(|| SourceFilePool::default_instance_ref())
    }
    // port: TypedAst.Builder#setSourceFilePool
    pub fn set_source_file_pool(mut self, value: SourceFilePool) -> Self {
        self.source_file_pool = Some(value);
        self
    }
    // port: TypedAst.Builder#clearSourceFilePool
    pub fn clear_source_file_pool(mut self) -> Self {
        self.source_file_pool = None;
        self
    }
    // port: TypedAst#hasExternsSummary
    pub fn has_externs_summary(&self) -> bool {
        self.externs_summary.is_some()
    }
    // port: TypedAst#getExternsSummary
    pub fn get_externs_summary(&self) -> &ExternsSummary {
        self.externs_summary
            .as_ref()
            .unwrap_or_else(|| ExternsSummary::default_instance_ref())
    }
    // port: TypedAst.Builder#setExternsSummary
    pub fn set_externs_summary(mut self, value: ExternsSummary) -> Self {
        self.externs_summary = Some(value);
        self
    }
    // port: TypedAst.Builder#clearExternsSummary
    pub fn clear_externs_summary(mut self) -> Self {
        self.externs_summary = None;
        self
    }
    // port: TypedAst#getRuntimeLibraryToInjectList
    pub fn get_runtime_library_to_inject_list(&self) -> &[String] {
        &self.runtime_library_to_inject
    }
    // port: TypedAst#getRuntimeLibraryToInjectCount
    pub fn get_runtime_library_to_inject_count(&self) -> i32 {
        self.runtime_library_to_inject.len() as i32
    }
    // port: TypedAst#getRuntimeLibraryToInject(int)
    pub fn get_runtime_library_to_inject(&self, index: i32) -> &String {
        &self.runtime_library_to_inject[index as usize]
    }
    // port: TypedAst.Builder#addRuntimeLibraryToInject
    pub fn add_runtime_library_to_inject(mut self, value: impl Into<String>) -> Self {
        self.runtime_library_to_inject.push(value.into());
        self
    }
    // port: TypedAst.Builder#addAllRuntimeLibraryToInject
    pub fn add_all_runtime_library_to_inject(
        mut self,
        values: impl IntoIterator<Item = String>,
    ) -> Self {
        self.runtime_library_to_inject.extend(values);
        self
    }
    // port: TypedAst.Builder#clearRuntimeLibraryToInject
    pub fn clear_runtime_library_to_inject(mut self) -> Self {
        self.runtime_library_to_inject.clear();
        self
    }
}
impl Message for TypedAst {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = self.type_pool.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.type_pool = Some(v);
                }
                18 => {
                    let mut v = self.string_pool.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.string_pool = Some(v);
                }
                26 => {
                    let mut v = LazyAst::default();
                    input.read_message(&mut v)?;
                    self.extern_ast.push(v);
                }
                34 => {
                    let mut v = LazyAst::default();
                    input.read_message(&mut v)?;
                    self.code_ast.push(v);
                }
                42 => {
                    let mut v = self.source_file_pool.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.source_file_pool = Some(v);
                }
                50 => {
                    let mut v = self.externs_summary.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.externs_summary = Some(v);
                }
                58 => {
                    self.runtime_library_to_inject
                        .push(input.read_string_require_utf8()?);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if let Some(v) = &self.type_pool {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.string_pool {
            output.write_tag(2, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.extern_ast {
            output.write_tag(3, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.code_ast {
            output.write_tag(4, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.source_file_pool {
            output.write_tag(5, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.externs_summary {
            output.write_tag(6, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.runtime_library_to_inject {
            output.write_tag(7, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if let Some(v) = &self.type_pool {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.string_pool {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.extern_ast {
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.code_ast {
            size += CodedOutputStream::compute_tag_size(4)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.source_file_pool {
            size += CodedOutputStream::compute_tag_size(5)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.externs_summary {
            size += CodedOutputStream::compute_tag_size(6)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.runtime_library_to_inject {
            size += CodedOutputStream::compute_tag_size(7)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        size
    }
}
// port: TypedAst.List (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TypedAstList {
    pub typed_asts: Vec<TypedAst>,
}
static TYPEDASTLIST_DEFAULT_INSTANCE: LazyLock<TypedAstList> = LazyLock::new(TypedAstList::default);
impl TypedAstList {
    pub const TYPED_ASTS_FIELD_NUMBER: i32 = 1;
    // port: TypedAstList#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: TypedAstList#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &TYPEDASTLIST_DEFAULT_INSTANCE
    }
    // port: TypedAstList#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: TypedAstList.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: TypedAstList.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: TypedAstList#getTypedAstsList
    pub fn get_typed_asts_list(&self) -> &[TypedAst] {
        &self.typed_asts
    }
    // port: TypedAstList#getTypedAstsCount
    pub fn get_typed_asts_count(&self) -> i32 {
        self.typed_asts.len() as i32
    }
    // port: TypedAstList#getTypedAsts(int)
    pub fn get_typed_asts(&self, index: i32) -> &TypedAst {
        &self.typed_asts[index as usize]
    }
    // port: TypedAstList.Builder#addTypedAsts
    pub fn add_typed_asts(mut self, value: TypedAst) -> Self {
        self.typed_asts.push(value);
        self
    }
    // port: TypedAstList.Builder#addAllTypedAsts
    pub fn add_all_typed_asts(mut self, values: impl IntoIterator<Item = TypedAst>) -> Self {
        self.typed_asts.extend(values);
        self
    }
    // port: TypedAstList.Builder#clearTypedAsts
    pub fn clear_typed_asts(mut self) -> Self {
        self.typed_asts.clear();
        self
    }
}
impl Message for TypedAstList {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = TypedAst::default();
                    input.read_message(&mut v)?;
                    self.typed_asts.push(v);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        for v in &self.typed_asts {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        for v in &self.typed_asts {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: LazyAst (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LazyAst {
    pub script: Vec<u8>,
    pub source_file: i32,
    pub source_mapping_url: String,
}
static LAZYAST_DEFAULT_INSTANCE: LazyLock<LazyAst> = LazyLock::new(LazyAst::default);
impl LazyAst {
    pub const SCRIPT_FIELD_NUMBER: i32 = 1;
    pub const SOURCE_FILE_FIELD_NUMBER: i32 = 2;
    pub const SOURCE_MAPPING_URL_FIELD_NUMBER: i32 = 3;
    // port: LazyAst#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: LazyAst#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &LAZYAST_DEFAULT_INSTANCE
    }
    // port: LazyAst#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: LazyAst.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: LazyAst.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: LazyAst#getScript
    pub fn get_script(&self) -> &[u8] {
        &self.script
    }
    // port: LazyAst.Builder#setScript
    pub fn set_script(mut self, value: Vec<u8>) -> Self {
        self.script = value;
        self
    }
    // port: LazyAst#getSourceFile
    pub fn get_source_file(&self) -> i32 {
        self.source_file
    }
    // port: LazyAst.Builder#setSourceFile
    pub fn set_source_file(mut self, value: i32) -> Self {
        self.source_file = value;
        self
    }
    // port: LazyAst#getSourceMappingUrl
    pub fn get_source_mapping_url(&self) -> &str {
        &self.source_mapping_url
    }
    // port: LazyAst.Builder#setSourceMappingUrl
    pub fn set_source_mapping_url(mut self, value: impl Into<String>) -> Self {
        self.source_mapping_url = value.into();
        self
    }
}
impl Message for LazyAst {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.script = input.read_bytes()?;
                }
                16 => {
                    self.source_file = input.read_uint32()?;
                }
                26 => {
                    self.source_mapping_url = input.read_string_require_utf8()?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if !self.script.is_empty() {
            let v = &self.script;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v);
        }
        if self.source_file != 0 {
            let v = &self.source_file;
            output.write_tag(2, 0);
            output.write_uint32_no_tag(*v);
        }
        if !self.source_mapping_url.is_empty() {
            let v = &self.source_mapping_url;
            output.write_tag(3, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.script.is_empty() {
            let v = &self.script;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v);
        }
        if self.source_file != 0 {
            let v = &self.source_file;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_uint32_size_no_tag(*v);
        }
        if !self.source_mapping_url.is_empty() {
            let v = &self.source_mapping_url;
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        size
    }
}
// port: NonLazyTypedAst (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NonLazyTypedAst {
    pub type_pool: Option<TypePool>,
    pub string_pool: Option<StringPoolProto>,
    pub extern_ast: Vec<NonLazyAst>,
    pub code_ast: Vec<NonLazyAst>,
    pub source_file_pool: Option<SourceFilePool>,
    pub externs_summary: Option<ExternsSummary>,
}
static NONLAZYTYPEDAST_DEFAULT_INSTANCE: LazyLock<NonLazyTypedAst> =
    LazyLock::new(NonLazyTypedAst::default);
impl NonLazyTypedAst {
    pub const TYPE_POOL_FIELD_NUMBER: i32 = 1;
    pub const STRING_POOL_FIELD_NUMBER: i32 = 2;
    pub const EXTERN_AST_FIELD_NUMBER: i32 = 3;
    pub const CODE_AST_FIELD_NUMBER: i32 = 4;
    pub const SOURCE_FILE_POOL_FIELD_NUMBER: i32 = 5;
    pub const EXTERNS_SUMMARY_FIELD_NUMBER: i32 = 6;
    // port: NonLazyTypedAst#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: NonLazyTypedAst#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &NONLAZYTYPEDAST_DEFAULT_INSTANCE
    }
    // port: NonLazyTypedAst#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: NonLazyTypedAst.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: NonLazyTypedAst.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: NonLazyTypedAst#hasTypePool
    pub fn has_type_pool(&self) -> bool {
        self.type_pool.is_some()
    }
    // port: NonLazyTypedAst#getTypePool
    pub fn get_type_pool(&self) -> &TypePool {
        self.type_pool
            .as_ref()
            .unwrap_or_else(|| TypePool::default_instance_ref())
    }
    // port: NonLazyTypedAst.Builder#setTypePool
    pub fn set_type_pool(mut self, value: TypePool) -> Self {
        self.type_pool = Some(value);
        self
    }
    // port: NonLazyTypedAst.Builder#clearTypePool
    pub fn clear_type_pool(mut self) -> Self {
        self.type_pool = None;
        self
    }
    // port: NonLazyTypedAst#hasStringPool
    pub fn has_string_pool(&self) -> bool {
        self.string_pool.is_some()
    }
    // port: NonLazyTypedAst#getStringPool
    pub fn get_string_pool(&self) -> &StringPoolProto {
        self.string_pool
            .as_ref()
            .unwrap_or_else(|| StringPoolProto::default_instance_ref())
    }
    // port: NonLazyTypedAst.Builder#setStringPool
    pub fn set_string_pool(mut self, value: StringPoolProto) -> Self {
        self.string_pool = Some(value);
        self
    }
    // port: NonLazyTypedAst.Builder#clearStringPool
    pub fn clear_string_pool(mut self) -> Self {
        self.string_pool = None;
        self
    }
    // port: NonLazyTypedAst#getExternAstList
    pub fn get_extern_ast_list(&self) -> &[NonLazyAst] {
        &self.extern_ast
    }
    // port: NonLazyTypedAst#getExternAstCount
    pub fn get_extern_ast_count(&self) -> i32 {
        self.extern_ast.len() as i32
    }
    // port: NonLazyTypedAst#getExternAst(int)
    pub fn get_extern_ast(&self, index: i32) -> &NonLazyAst {
        &self.extern_ast[index as usize]
    }
    // port: NonLazyTypedAst.Builder#addExternAst
    pub fn add_extern_ast(mut self, value: NonLazyAst) -> Self {
        self.extern_ast.push(value);
        self
    }
    // port: NonLazyTypedAst.Builder#addAllExternAst
    pub fn add_all_extern_ast(mut self, values: impl IntoIterator<Item = NonLazyAst>) -> Self {
        self.extern_ast.extend(values);
        self
    }
    // port: NonLazyTypedAst.Builder#clearExternAst
    pub fn clear_extern_ast(mut self) -> Self {
        self.extern_ast.clear();
        self
    }
    // port: NonLazyTypedAst#getCodeAstList
    pub fn get_code_ast_list(&self) -> &[NonLazyAst] {
        &self.code_ast
    }
    // port: NonLazyTypedAst#getCodeAstCount
    pub fn get_code_ast_count(&self) -> i32 {
        self.code_ast.len() as i32
    }
    // port: NonLazyTypedAst#getCodeAst(int)
    pub fn get_code_ast(&self, index: i32) -> &NonLazyAst {
        &self.code_ast[index as usize]
    }
    // port: NonLazyTypedAst.Builder#addCodeAst
    pub fn add_code_ast(mut self, value: NonLazyAst) -> Self {
        self.code_ast.push(value);
        self
    }
    // port: NonLazyTypedAst.Builder#addAllCodeAst
    pub fn add_all_code_ast(mut self, values: impl IntoIterator<Item = NonLazyAst>) -> Self {
        self.code_ast.extend(values);
        self
    }
    // port: NonLazyTypedAst.Builder#clearCodeAst
    pub fn clear_code_ast(mut self) -> Self {
        self.code_ast.clear();
        self
    }
    // port: NonLazyTypedAst#hasSourceFilePool
    pub fn has_source_file_pool(&self) -> bool {
        self.source_file_pool.is_some()
    }
    // port: NonLazyTypedAst#getSourceFilePool
    pub fn get_source_file_pool(&self) -> &SourceFilePool {
        self.source_file_pool
            .as_ref()
            .unwrap_or_else(|| SourceFilePool::default_instance_ref())
    }
    // port: NonLazyTypedAst.Builder#setSourceFilePool
    pub fn set_source_file_pool(mut self, value: SourceFilePool) -> Self {
        self.source_file_pool = Some(value);
        self
    }
    // port: NonLazyTypedAst.Builder#clearSourceFilePool
    pub fn clear_source_file_pool(mut self) -> Self {
        self.source_file_pool = None;
        self
    }
    // port: NonLazyTypedAst#hasExternsSummary
    pub fn has_externs_summary(&self) -> bool {
        self.externs_summary.is_some()
    }
    // port: NonLazyTypedAst#getExternsSummary
    pub fn get_externs_summary(&self) -> &ExternsSummary {
        self.externs_summary
            .as_ref()
            .unwrap_or_else(|| ExternsSummary::default_instance_ref())
    }
    // port: NonLazyTypedAst.Builder#setExternsSummary
    pub fn set_externs_summary(mut self, value: ExternsSummary) -> Self {
        self.externs_summary = Some(value);
        self
    }
    // port: NonLazyTypedAst.Builder#clearExternsSummary
    pub fn clear_externs_summary(mut self) -> Self {
        self.externs_summary = None;
        self
    }
}
impl Message for NonLazyTypedAst {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = self.type_pool.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.type_pool = Some(v);
                }
                18 => {
                    let mut v = self.string_pool.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.string_pool = Some(v);
                }
                26 => {
                    let mut v = NonLazyAst::default();
                    input.read_message(&mut v)?;
                    self.extern_ast.push(v);
                }
                34 => {
                    let mut v = NonLazyAst::default();
                    input.read_message(&mut v)?;
                    self.code_ast.push(v);
                }
                42 => {
                    let mut v = self.source_file_pool.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.source_file_pool = Some(v);
                }
                50 => {
                    let mut v = self.externs_summary.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.externs_summary = Some(v);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if let Some(v) = &self.type_pool {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.string_pool {
            output.write_tag(2, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.extern_ast {
            output.write_tag(3, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.code_ast {
            output.write_tag(4, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.source_file_pool {
            output.write_tag(5, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.externs_summary {
            output.write_tag(6, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if let Some(v) = &self.type_pool {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.string_pool {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.extern_ast {
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.code_ast {
            size += CodedOutputStream::compute_tag_size(4)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.source_file_pool {
            size += CodedOutputStream::compute_tag_size(5)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.externs_summary {
            size += CodedOutputStream::compute_tag_size(6)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: NonLazyTypedAst.List (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NonLazyTypedAstList {
    pub typed_asts: Vec<NonLazyTypedAst>,
}
static NONLAZYTYPEDASTLIST_DEFAULT_INSTANCE: LazyLock<NonLazyTypedAstList> =
    LazyLock::new(NonLazyTypedAstList::default);
impl NonLazyTypedAstList {
    pub const TYPED_ASTS_FIELD_NUMBER: i32 = 1;
    // port: NonLazyTypedAstList#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: NonLazyTypedAstList#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &NONLAZYTYPEDASTLIST_DEFAULT_INSTANCE
    }
    // port: NonLazyTypedAstList#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: NonLazyTypedAstList.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: NonLazyTypedAstList.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: NonLazyTypedAstList#getTypedAstsList
    pub fn get_typed_asts_list(&self) -> &[NonLazyTypedAst] {
        &self.typed_asts
    }
    // port: NonLazyTypedAstList#getTypedAstsCount
    pub fn get_typed_asts_count(&self) -> i32 {
        self.typed_asts.len() as i32
    }
    // port: NonLazyTypedAstList#getTypedAsts(int)
    pub fn get_typed_asts(&self, index: i32) -> &NonLazyTypedAst {
        &self.typed_asts[index as usize]
    }
    // port: NonLazyTypedAstList.Builder#addTypedAsts
    pub fn add_typed_asts(mut self, value: NonLazyTypedAst) -> Self {
        self.typed_asts.push(value);
        self
    }
    // port: NonLazyTypedAstList.Builder#addAllTypedAsts
    pub fn add_all_typed_asts(mut self, values: impl IntoIterator<Item = NonLazyTypedAst>) -> Self {
        self.typed_asts.extend(values);
        self
    }
    // port: NonLazyTypedAstList.Builder#clearTypedAsts
    pub fn clear_typed_asts(mut self) -> Self {
        self.typed_asts.clear();
        self
    }
}
impl Message for NonLazyTypedAstList {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = NonLazyTypedAst::default();
                    input.read_message(&mut v)?;
                    self.typed_asts.push(v);
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        for v in &self.typed_asts {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        for v in &self.typed_asts {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: NonLazyAst (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NonLazyAst {
    pub script: Option<AstNode>,
    pub source_file: i32,
    pub source_mapping_url: String,
}
static NONLAZYAST_DEFAULT_INSTANCE: LazyLock<NonLazyAst> = LazyLock::new(NonLazyAst::default);
impl NonLazyAst {
    pub const SCRIPT_FIELD_NUMBER: i32 = 1;
    pub const SOURCE_FILE_FIELD_NUMBER: i32 = 2;
    pub const SOURCE_MAPPING_URL_FIELD_NUMBER: i32 = 3;
    // port: NonLazyAst#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: NonLazyAst#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &NONLAZYAST_DEFAULT_INSTANCE
    }
    // port: NonLazyAst#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: NonLazyAst.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: NonLazyAst.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: NonLazyAst#hasScript
    pub fn has_script(&self) -> bool {
        self.script.is_some()
    }
    // port: NonLazyAst#getScript
    pub fn get_script(&self) -> &AstNode {
        self.script
            .as_ref()
            .unwrap_or_else(|| AstNode::default_instance_ref())
    }
    // port: NonLazyAst.Builder#setScript
    pub fn set_script(mut self, value: AstNode) -> Self {
        self.script = Some(value);
        self
    }
    // port: NonLazyAst.Builder#clearScript
    pub fn clear_script(mut self) -> Self {
        self.script = None;
        self
    }
    // port: NonLazyAst#getSourceFile
    pub fn get_source_file(&self) -> i32 {
        self.source_file
    }
    // port: NonLazyAst.Builder#setSourceFile
    pub fn set_source_file(mut self, value: i32) -> Self {
        self.source_file = value;
        self
    }
    // port: NonLazyAst#getSourceMappingUrl
    pub fn get_source_mapping_url(&self) -> &str {
        &self.source_mapping_url
    }
    // port: NonLazyAst.Builder#setSourceMappingUrl
    pub fn set_source_mapping_url(mut self, value: impl Into<String>) -> Self {
        self.source_mapping_url = value.into();
        self
    }
}
impl Message for NonLazyAst {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = self.script.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.script = Some(v);
                }
                16 => {
                    self.source_file = input.read_uint32()?;
                }
                26 => {
                    self.source_mapping_url = input.read_string_require_utf8()?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if let Some(v) = &self.script {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
        if self.source_file != 0 {
            let v = &self.source_file;
            output.write_tag(2, 0);
            output.write_uint32_no_tag(*v);
        }
        if !self.source_mapping_url.is_empty() {
            let v = &self.source_mapping_url;
            output.write_tag(3, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if let Some(v) = &self.script {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if self.source_file != 0 {
            let v = &self.source_file;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_uint32_size_no_tag(*v);
        }
        if !self.source_mapping_url.is_empty() {
            let v = &self.source_mapping_url;
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        size
    }
}
// port: StringPoolProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StringPoolProto {
    pub strings: Vec<Vec<u8>>,
    pub max_length: i32,
}
static STRINGPOOLPROTO_DEFAULT_INSTANCE: LazyLock<StringPoolProto> =
    LazyLock::new(StringPoolProto::default);
impl StringPoolProto {
    pub const STRINGS_FIELD_NUMBER: i32 = 1;
    pub const MAX_LENGTH_FIELD_NUMBER: i32 = 2;
    // port: StringPoolProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: StringPoolProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &STRINGPOOLPROTO_DEFAULT_INSTANCE
    }
    // port: StringPoolProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: StringPoolProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: StringPoolProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: StringPoolProto#getStringsList
    pub fn get_strings_list(&self) -> &[Vec<u8>] {
        &self.strings
    }
    // port: StringPoolProto#getStringsCount
    pub fn get_strings_count(&self) -> i32 {
        self.strings.len() as i32
    }
    // port: StringPoolProto#getStrings(int)
    pub fn get_strings(&self, index: i32) -> &Vec<u8> {
        &self.strings[index as usize]
    }
    // port: StringPoolProto.Builder#addStrings
    pub fn add_strings(mut self, value: Vec<u8>) -> Self {
        self.strings.push(value);
        self
    }
    // port: StringPoolProto.Builder#addAllStrings
    pub fn add_all_strings(mut self, values: impl IntoIterator<Item = Vec<u8>>) -> Self {
        self.strings.extend(values);
        self
    }
    // port: StringPoolProto.Builder#clearStrings
    pub fn clear_strings(mut self) -> Self {
        self.strings.clear();
        self
    }
    // port: StringPoolProto#getMaxLength
    pub fn get_max_length(&self) -> i32 {
        self.max_length
    }
    // port: StringPoolProto.Builder#setMaxLength
    pub fn set_max_length(mut self, value: i32) -> Self {
        self.max_length = value;
        self
    }
}
impl Message for StringPoolProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.strings.push(input.read_bytes()?);
                }
                16 => {
                    self.max_length = input.read_int32()?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        for v in &self.strings {
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v);
        }
        if self.max_length != 0 {
            let v = &self.max_length;
            output.write_tag(2, 0);
            output.write_int32_no_tag(*v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        for v in &self.strings {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v);
        }
        if self.max_length != 0 {
            let v = &self.max_length;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        size
    }
}
// port: ExternsSummary (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ExternsSummary {
    pub prop_name_ptr: Vec<i32>,
}
static EXTERNSSUMMARY_DEFAULT_INSTANCE: LazyLock<ExternsSummary> =
    LazyLock::new(ExternsSummary::default);
impl ExternsSummary {
    pub const PROP_NAME_PTR_FIELD_NUMBER: i32 = 1;
    // port: ExternsSummary#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: ExternsSummary#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &EXTERNSSUMMARY_DEFAULT_INSTANCE
    }
    // port: ExternsSummary#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: ExternsSummary.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: ExternsSummary.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: ExternsSummary#getPropNamePtrList
    pub fn get_prop_name_ptr_list(&self) -> &[i32] {
        &self.prop_name_ptr
    }
    // port: ExternsSummary#getPropNamePtrCount
    pub fn get_prop_name_ptr_count(&self) -> i32 {
        self.prop_name_ptr.len() as i32
    }
    // port: ExternsSummary#getPropNamePtr(int)
    pub fn get_prop_name_ptr(&self, index: i32) -> i32 {
        self.prop_name_ptr[index as usize]
    }
    // port: ExternsSummary.Builder#addPropNamePtr
    pub fn add_prop_name_ptr(mut self, value: i32) -> Self {
        self.prop_name_ptr.push(value);
        self
    }
    // port: ExternsSummary.Builder#addAllPropNamePtr
    pub fn add_all_prop_name_ptr(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.prop_name_ptr.extend(values);
        self
    }
    // port: ExternsSummary.Builder#clearPropNamePtr
    pub fn clear_prop_name_ptr(mut self) -> Self {
        self.prop_name_ptr.clear();
        self
    }
}
impl Message for ExternsSummary {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 | 10 => {
                    input.read_repeated(tag, &mut self.prop_name_ptr, |input| {
                        Ok(input.read_int32()?)
                    })?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if !self.prop_name_ptr.is_empty() {
            let data_size: usize = self
                .prop_name_ptr
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            output.write_tag(1, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.prop_name_ptr {
                output.write_int32_no_tag(*v);
            }
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.prop_name_ptr.is_empty() {
            let data_size: usize = self
                .prop_name_ptr
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        size
    }
}
// port: AstNode#value (oneof)
#[derive(Clone, Debug, PartialEq, Default)]
pub enum AstNodeValue {
    STRING_VALUE_POINTER(i32),
    DOUBLE_VALUE(f64),
    TEMPLATE_STRING_VALUE(TemplateStringValue),
    #[default]
    VALUE_NOT_SET,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AstNodeValueCase {
    STRING_VALUE_POINTER,
    DOUBLE_VALUE,
    TEMPLATE_STRING_VALUE,
    VALUE_NOT_SET,
}
// port: AstNode (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AstNode {
    pub kind: NodeKind,
    pub child: Vec<AstNode>,
    pub relative_line: i32,
    pub relative_column: i32,
    pub jsdoc: Option<OptimizationJsdoc>,
    pub original_name_pointer: i32,
    pub boolean_properties: i64,
    pub type_: Option<i32>,
    pub source_file: i32,
    pub value: AstNodeValue,
}
static ASTNODE_DEFAULT_INSTANCE: LazyLock<AstNode> = LazyLock::new(AstNode::default);
impl AstNode {
    pub const KIND_FIELD_NUMBER: i32 = 1;
    pub const CHILD_FIELD_NUMBER: i32 = 2;
    pub const STRING_VALUE_POINTER_FIELD_NUMBER: i32 = 3;
    pub const DOUBLE_VALUE_FIELD_NUMBER: i32 = 4;
    pub const TEMPLATE_STRING_VALUE_FIELD_NUMBER: i32 = 8;
    pub const RELATIVE_LINE_FIELD_NUMBER: i32 = 5;
    pub const RELATIVE_COLUMN_FIELD_NUMBER: i32 = 6;
    pub const JSDOC_FIELD_NUMBER: i32 = 7;
    pub const ORIGINAL_NAME_POINTER_FIELD_NUMBER: i32 = 9;
    pub const BOOLEAN_PROPERTIES_FIELD_NUMBER: i32 = 14;
    pub const TYPE_FIELD_NUMBER: i32 = 15;
    pub const SOURCE_FILE_FIELD_NUMBER: i32 = 16;
    // port: AstNode#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: AstNode#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &ASTNODE_DEFAULT_INSTANCE
    }
    // port: AstNode#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: AstNode.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: AstNode.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: AstNode#getKind
    pub fn get_kind(&self) -> NodeKind {
        self.kind
    }
    // port: AstNode.Builder#setKind
    pub fn set_kind(mut self, value: NodeKind) -> Self {
        self.kind = value;
        self
    }
    // port: AstNode#getKindValue
    pub fn get_kind_value(&self) -> i32 {
        self.kind.get_number()
    }
    // port: AstNode#getChildList
    pub fn get_child_list(&self) -> &[AstNode] {
        &self.child
    }
    // port: AstNode#getChildCount
    pub fn get_child_count(&self) -> i32 {
        self.child.len() as i32
    }
    // port: AstNode#getChild(int)
    pub fn get_child(&self, index: i32) -> &AstNode {
        &self.child[index as usize]
    }
    // port: AstNode.Builder#addChild
    pub fn add_child(mut self, value: AstNode) -> Self {
        self.child.push(value);
        self
    }
    // port: AstNode.Builder#addAllChild
    pub fn add_all_child(mut self, values: impl IntoIterator<Item = AstNode>) -> Self {
        self.child.extend(values);
        self
    }
    // port: AstNode.Builder#clearChild
    pub fn clear_child(mut self) -> Self {
        self.child.clear();
        self
    }
    // port: AstNode#hasStringValuePointer
    pub fn has_string_value_pointer(&self) -> bool {
        matches!(self.value, AstNodeValue::STRING_VALUE_POINTER(_))
    }
    // port: AstNode#getStringValuePointer
    pub fn get_string_value_pointer(&self) -> i32 {
        if let AstNodeValue::STRING_VALUE_POINTER(v) = &self.value {
            *v
        } else {
            i32::default()
        }
    }
    // port: AstNode.Builder#setStringValuePointer
    pub fn set_string_value_pointer(mut self, value: i32) -> Self {
        self.value = AstNodeValue::STRING_VALUE_POINTER(value);
        self
    }
    // port: AstNode#hasDoubleValue
    pub fn has_double_value(&self) -> bool {
        matches!(self.value, AstNodeValue::DOUBLE_VALUE(_))
    }
    // port: AstNode#getDoubleValue
    pub fn get_double_value(&self) -> f64 {
        if let AstNodeValue::DOUBLE_VALUE(v) = &self.value {
            *v
        } else {
            f64::default()
        }
    }
    // port: AstNode.Builder#setDoubleValue
    pub fn set_double_value(mut self, value: f64) -> Self {
        self.value = AstNodeValue::DOUBLE_VALUE(value);
        self
    }
    // port: AstNode#hasTemplateStringValue
    pub fn has_template_string_value(&self) -> bool {
        matches!(self.value, AstNodeValue::TEMPLATE_STRING_VALUE(_))
    }
    // port: AstNode#getTemplateStringValue
    pub fn get_template_string_value(&self) -> &TemplateStringValue {
        if let AstNodeValue::TEMPLATE_STRING_VALUE(v) = &self.value {
            v
        } else {
            TemplateStringValue::default_instance_ref()
        }
    }
    // port: AstNode.Builder#setTemplateStringValue
    pub fn set_template_string_value(mut self, value: TemplateStringValue) -> Self {
        self.value = AstNodeValue::TEMPLATE_STRING_VALUE(value);
        self
    }
    // port: AstNode#getRelativeLine
    pub fn get_relative_line(&self) -> i32 {
        self.relative_line
    }
    // port: AstNode.Builder#setRelativeLine
    pub fn set_relative_line(mut self, value: i32) -> Self {
        self.relative_line = value;
        self
    }
    // port: AstNode#getRelativeColumn
    pub fn get_relative_column(&self) -> i32 {
        self.relative_column
    }
    // port: AstNode.Builder#setRelativeColumn
    pub fn set_relative_column(mut self, value: i32) -> Self {
        self.relative_column = value;
        self
    }
    // port: AstNode#hasJsdoc
    pub fn has_jsdoc(&self) -> bool {
        self.jsdoc.is_some()
    }
    // port: AstNode#getJsdoc
    pub fn get_jsdoc(&self) -> &OptimizationJsdoc {
        self.jsdoc
            .as_ref()
            .unwrap_or_else(|| OptimizationJsdoc::default_instance_ref())
    }
    // port: AstNode.Builder#setJsdoc
    pub fn set_jsdoc(mut self, value: OptimizationJsdoc) -> Self {
        self.jsdoc = Some(value);
        self
    }
    // port: AstNode.Builder#clearJsdoc
    pub fn clear_jsdoc(mut self) -> Self {
        self.jsdoc = None;
        self
    }
    // port: AstNode#getOriginalNamePointer
    pub fn get_original_name_pointer(&self) -> i32 {
        self.original_name_pointer
    }
    // port: AstNode.Builder#setOriginalNamePointer
    pub fn set_original_name_pointer(mut self, value: i32) -> Self {
        self.original_name_pointer = value;
        self
    }
    // port: AstNode#getBooleanProperties
    pub fn get_boolean_properties(&self) -> i64 {
        self.boolean_properties
    }
    // port: AstNode.Builder#setBooleanProperties
    pub fn set_boolean_properties(mut self, value: i64) -> Self {
        self.boolean_properties = value;
        self
    }
    // port: AstNode#hasType
    pub fn has_type(&self) -> bool {
        self.type_.is_some()
    }
    // port: AstNode#getType
    pub fn get_type(&self) -> i32 {
        self.type_.unwrap_or_default()
    }
    // port: AstNode.Builder#setType
    pub fn set_type(mut self, value: i32) -> Self {
        self.type_ = Some(value);
        self
    }
    // port: AstNode.Builder#clearType
    pub fn clear_type(mut self) -> Self {
        self.type_ = None;
        self
    }
    // port: AstNode#getSourceFile
    pub fn get_source_file(&self) -> i32 {
        self.source_file
    }
    // port: AstNode.Builder#setSourceFile
    pub fn set_source_file(mut self, value: i32) -> Self {
        self.source_file = value;
        self
    }
    // port: AstNode#getValueCase
    pub fn get_value_case(&self) -> AstNodeValueCase {
        match self.value {
            AstNodeValue::STRING_VALUE_POINTER(_) => AstNodeValueCase::STRING_VALUE_POINTER,
            AstNodeValue::DOUBLE_VALUE(_) => AstNodeValueCase::DOUBLE_VALUE,
            AstNodeValue::TEMPLATE_STRING_VALUE(_) => AstNodeValueCase::TEMPLATE_STRING_VALUE,
            AstNodeValue::VALUE_NOT_SET => AstNodeValueCase::VALUE_NOT_SET,
        }
    }
}
impl Message for AstNode {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 => {
                    self.kind = NodeKind::for_number_or_unrecognized(input.read_enum()?);
                }
                18 => {
                    let mut v = AstNode::default();
                    input.read_message(&mut v)?;
                    self.child.push(v);
                }
                24 => {
                    self.value = AstNodeValue::STRING_VALUE_POINTER(input.read_uint32()?);
                }
                33 => {
                    self.value = AstNodeValue::DOUBLE_VALUE(input.read_double()?);
                }
                66 => {
                    let mut v = if let AstNodeValue::TEMPLATE_STRING_VALUE(v) =
                        std::mem::take(&mut self.value)
                    {
                        v
                    } else {
                        TemplateStringValue::default()
                    };
                    input.read_message(&mut v)?;
                    self.value = AstNodeValue::TEMPLATE_STRING_VALUE(v);
                }
                40 => {
                    self.relative_line = input.read_sint32()?;
                }
                48 => {
                    self.relative_column = input.read_sint32()?;
                }
                58 => {
                    let mut v = self.jsdoc.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.jsdoc = Some(v);
                }
                72 => {
                    self.original_name_pointer = input.read_uint32()?;
                }
                112 => {
                    self.boolean_properties = input.read_int64()?;
                }
                120 => {
                    self.type_ = Some(input.read_int32()?);
                }
                128 => {
                    self.source_file = input.read_uint32()?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if self.kind.get_number() != 0 {
            let v = &self.kind;
            output.write_tag(1, 0);
            output.write_int32_no_tag(v.get_number());
        }
        for v in &self.child {
            output.write_tag(2, 2);
            output.write_message_no_tag(v);
        }
        if let AstNodeValue::STRING_VALUE_POINTER(v) = &self.value {
            output.write_tag(3, 0);
            output.write_uint32_no_tag(*v);
        }
        if let AstNodeValue::DOUBLE_VALUE(v) = &self.value {
            output.write_tag(4, 1);
            output.write_double_no_tag(*v);
        }
        if self.relative_line != 0 {
            let v = &self.relative_line;
            output.write_tag(5, 0);
            output.write_sint32_no_tag(*v);
        }
        if self.relative_column != 0 {
            let v = &self.relative_column;
            output.write_tag(6, 0);
            output.write_sint32_no_tag(*v);
        }
        if let Some(v) = &self.jsdoc {
            output.write_tag(7, 2);
            output.write_message_no_tag(v);
        }
        if let AstNodeValue::TEMPLATE_STRING_VALUE(v) = &self.value {
            output.write_tag(8, 2);
            output.write_message_no_tag(v);
        }
        if self.original_name_pointer != 0 {
            let v = &self.original_name_pointer;
            output.write_tag(9, 0);
            output.write_uint32_no_tag(*v);
        }
        if self.boolean_properties != 0 {
            let v = &self.boolean_properties;
            output.write_tag(14, 0);
            output.write_uint64_no_tag(*v);
        }
        if let Some(v) = &self.type_ {
            output.write_tag(15, 0);
            output.write_int32_no_tag(*v);
        }
        if self.source_file != 0 {
            let v = &self.source_file;
            output.write_tag(16, 0);
            output.write_uint32_no_tag(*v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if self.kind.get_number() != 0 {
            let v = &self.kind;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_int32_size_no_tag(v.get_number());
        }
        for v in &self.child {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let AstNodeValue::STRING_VALUE_POINTER(v) = &self.value {
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_uint32_size_no_tag(*v);
        }
        if let AstNodeValue::DOUBLE_VALUE(v) = &self.value {
            size += CodedOutputStream::compute_tag_size(4) + 8;
        }
        if self.relative_line != 0 {
            let v = &self.relative_line;
            size += CodedOutputStream::compute_tag_size(5)
                + CodedOutputStream::compute_sint32_size_no_tag(*v);
        }
        if self.relative_column != 0 {
            let v = &self.relative_column;
            size += CodedOutputStream::compute_tag_size(6)
                + CodedOutputStream::compute_sint32_size_no_tag(*v);
        }
        if let Some(v) = &self.jsdoc {
            size += CodedOutputStream::compute_tag_size(7)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let AstNodeValue::TEMPLATE_STRING_VALUE(v) = &self.value {
            size += CodedOutputStream::compute_tag_size(8)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if self.original_name_pointer != 0 {
            let v = &self.original_name_pointer;
            size += CodedOutputStream::compute_tag_size(9)
                + CodedOutputStream::compute_uint32_size_no_tag(*v);
        }
        if self.boolean_properties != 0 {
            let v = &self.boolean_properties;
            size += CodedOutputStream::compute_tag_size(14)
                + CodedOutputStream::compute_uint64_size_no_tag(*v);
        }
        if let Some(v) = &self.type_ {
            size += CodedOutputStream::compute_tag_size(15)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.source_file != 0 {
            let v = &self.source_file;
            size += CodedOutputStream::compute_tag_size(16)
                + CodedOutputStream::compute_uint32_size_no_tag(*v);
        }
        size
    }
}
// port: TemplateStringValue (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TemplateStringValue {
    pub raw_string_pointer: i32,
    pub cooked_string_pointer: i32,
}
static TEMPLATESTRINGVALUE_DEFAULT_INSTANCE: LazyLock<TemplateStringValue> =
    LazyLock::new(TemplateStringValue::default);
impl TemplateStringValue {
    pub const RAW_STRING_POINTER_FIELD_NUMBER: i32 = 1;
    pub const COOKED_STRING_POINTER_FIELD_NUMBER: i32 = 2;
    // port: TemplateStringValue#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: TemplateStringValue#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &TEMPLATESTRINGVALUE_DEFAULT_INSTANCE
    }
    // port: TemplateStringValue#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: TemplateStringValue.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: TemplateStringValue.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: TemplateStringValue#getRawStringPointer
    pub fn get_raw_string_pointer(&self) -> i32 {
        self.raw_string_pointer
    }
    // port: TemplateStringValue.Builder#setRawStringPointer
    pub fn set_raw_string_pointer(mut self, value: i32) -> Self {
        self.raw_string_pointer = value;
        self
    }
    // port: TemplateStringValue#getCookedStringPointer
    pub fn get_cooked_string_pointer(&self) -> i32 {
        self.cooked_string_pointer
    }
    // port: TemplateStringValue.Builder#setCookedStringPointer
    pub fn set_cooked_string_pointer(mut self, value: i32) -> Self {
        self.cooked_string_pointer = value;
        self
    }
}
impl Message for TemplateStringValue {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 => {
                    self.raw_string_pointer = input.read_uint32()?;
                }
                16 => {
                    self.cooked_string_pointer = input.read_int32()?;
                }
                _ => {
                    if !input.skip_field(tag)? {
                        return Ok(());
                    }
                }
            }
        }
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        if self.raw_string_pointer != 0 {
            let v = &self.raw_string_pointer;
            output.write_tag(1, 0);
            output.write_uint32_no_tag(*v);
        }
        if self.cooked_string_pointer != 0 {
            let v = &self.cooked_string_pointer;
            output.write_tag(2, 0);
            output.write_int32_no_tag(*v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if self.raw_string_pointer != 0 {
            let v = &self.raw_string_pointer;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_uint32_size_no_tag(*v);
        }
        if self.cooked_string_pointer != 0 {
            let v = &self.cooked_string_pointer;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        size
    }
}
// port: Descriptors.FileDescriptor of typed_ast.proto (the message descriptors protoc embeds)
pub static MESSAGE_DESCRIPTORS: &[Descriptor] = &[
    Descriptor {
        full_name: "jscomp.TypedAst",
        fields: &[
            FieldDescriptor {
                name: "type_pool",
                number: 1,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.TypePool",
            },
            FieldDescriptor {
                name: "string_pool",
                number: 2,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.StringPoolProto",
            },
            FieldDescriptor {
                name: "extern_ast",
                number: 3,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.LazyAst",
            },
            FieldDescriptor {
                name: "code_ast",
                number: 4,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.LazyAst",
            },
            FieldDescriptor {
                name: "source_file_pool",
                number: 5,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.SourceFilePool",
            },
            FieldDescriptor {
                name: "externs_summary",
                number: 6,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.ExternsSummary",
            },
            FieldDescriptor {
                name: "runtime_library_to_inject",
                number: 7,
                field_type: FieldType::STRING,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.TypedAst.List",
        fields: &[FieldDescriptor {
            name: "typed_asts",
            number: 1,
            field_type: FieldType::MESSAGE,
            repeated: true,
            containing_oneof: None,
            type_name: "jscomp.TypedAst",
        }],
    },
    Descriptor {
        full_name: "jscomp.LazyAst",
        fields: &[
            FieldDescriptor {
                name: "script",
                number: 1,
                field_type: FieldType::BYTES,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "source_file",
                number: 2,
                field_type: FieldType::UINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "source_mapping_url",
                number: 3,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.NonLazyTypedAst",
        fields: &[
            FieldDescriptor {
                name: "type_pool",
                number: 1,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.TypePool",
            },
            FieldDescriptor {
                name: "string_pool",
                number: 2,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.StringPoolProto",
            },
            FieldDescriptor {
                name: "extern_ast",
                number: 3,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.NonLazyAst",
            },
            FieldDescriptor {
                name: "code_ast",
                number: 4,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.NonLazyAst",
            },
            FieldDescriptor {
                name: "source_file_pool",
                number: 5,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.SourceFilePool",
            },
            FieldDescriptor {
                name: "externs_summary",
                number: 6,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.ExternsSummary",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.NonLazyTypedAst.List",
        fields: &[FieldDescriptor {
            name: "typed_asts",
            number: 1,
            field_type: FieldType::MESSAGE,
            repeated: true,
            containing_oneof: None,
            type_name: "jscomp.NonLazyTypedAst",
        }],
    },
    Descriptor {
        full_name: "jscomp.NonLazyAst",
        fields: &[
            FieldDescriptor {
                name: "script",
                number: 1,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.AstNode",
            },
            FieldDescriptor {
                name: "source_file",
                number: 2,
                field_type: FieldType::UINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "source_mapping_url",
                number: 3,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.StringPoolProto",
        fields: &[
            FieldDescriptor {
                name: "strings",
                number: 1,
                field_type: FieldType::BYTES,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "max_length",
                number: 2,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.ExternsSummary",
        fields: &[FieldDescriptor {
            name: "prop_name_ptr",
            number: 1,
            field_type: FieldType::INT32,
            repeated: true,
            containing_oneof: None,
            type_name: "",
        }],
    },
    Descriptor {
        full_name: "jscomp.AstNode",
        fields: &[
            FieldDescriptor {
                name: "kind",
                number: 1,
                field_type: FieldType::ENUM,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.NodeKind",
            },
            FieldDescriptor {
                name: "child",
                number: 2,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.AstNode",
            },
            FieldDescriptor {
                name: "string_value_pointer",
                number: 3,
                field_type: FieldType::UINT32,
                repeated: false,
                containing_oneof: Some("value"),
                type_name: "",
            },
            FieldDescriptor {
                name: "double_value",
                number: 4,
                field_type: FieldType::DOUBLE,
                repeated: false,
                containing_oneof: Some("value"),
                type_name: "",
            },
            FieldDescriptor {
                name: "template_string_value",
                number: 8,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: Some("value"),
                type_name: "jscomp.TemplateStringValue",
            },
            FieldDescriptor {
                name: "relative_line",
                number: 5,
                field_type: FieldType::SINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "relative_column",
                number: 6,
                field_type: FieldType::SINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "jsdoc",
                number: 7,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.OptimizationJsdoc",
            },
            FieldDescriptor {
                name: "original_name_pointer",
                number: 9,
                field_type: FieldType::UINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "boolean_properties",
                number: 14,
                field_type: FieldType::INT64,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "type",
                number: 15,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "source_file",
                number: 16,
                field_type: FieldType::UINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.TemplateStringValue",
        fields: &[
            FieldDescriptor {
                name: "raw_string_pointer",
                number: 1,
                field_type: FieldType::UINT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "cooked_string_pointer",
                number: 2,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
];
// port: Descriptors.FileDescriptor of typed_ast.proto (the enum descriptors protoc embeds)
pub static ENUM_DESCRIPTORS: &[EnumDescriptor] = &[
    EnumDescriptor {
        full_name: "jscomp.NodeKind",
        name: "NodeKind",
        values: &[
            ("NODE_KIND_UNSPECIFIED", 0),
            ("NUMBER_LITERAL", 201),
            ("STRING_LITERAL", 202),
            ("BIGINT_LITERAL", 203),
            ("TRUE", 204),
            ("FALSE", 205),
            ("NULL", 206),
            ("REGEX_LITERAL", 207),
            ("ARRAY_LITERAL", 208),
            ("OBJECT_LITERAL", 209),
            ("TEMPLATELIT", 210),
            ("TAGGED_TEMPLATELIT", 211),
            ("IDENTIFIER", 214),
            ("THIS", 215),
            ("ASSIGNMENT", 216),
            ("COMMA", 217),
            ("CALL", 218),
            ("NEW", 219),
            ("YIELD", 220),
            ("AWAIT", 221),
            ("BOOLEAN_OR", 223),
            ("BOOLEAN_AND", 224),
            ("HOOK", 225),
            ("PROPERTY_ACCESS", 226),
            ("ELEMENT_ACCESS", 227),
            ("LESS_THAN", 228),
            ("LESS_THAN_EQUAL", 229),
            ("GREATER_THAN", 230),
            ("GREATER_THAN_EQUAL", 231),
            ("EQUAL", 232),
            ("TRIPLE_EQUAL", 233),
            ("NOT_EQUAL", 234),
            ("NOT_TRIPLE_EQUAL", 235),
            ("NOT", 236),
            ("TYPEOF", 237),
            ("INSTANCEOF", 238),
            ("IN", 239),
            ("LEFT_SHIFT", 240),
            ("RIGHT_SHIFT", 241),
            ("UNSIGNED_RIGHT_SHIFT", 242),
            ("ADD", 243),
            ("SUBTRACT", 244),
            ("MULTIPLY", 245),
            ("DIVIDE", 246),
            ("MODULO", 247),
            ("EXPONENT", 248),
            ("ASSIGN_ADD", 249),
            ("ASSIGN_SUBTRACT", 250),
            ("ASSIGN_MULTIPLY", 251),
            ("ASSIGN_DIVIDE", 252),
            ("ASSIGN_MODULO", 253),
            ("ASSIGN_EXPONENT", 254),
            ("ASSIGN_BITWISE_OR", 255),
            ("ASSIGN_BITWISE_AND", 256),
            ("ASSIGN_BITWISE_XOR", 257),
            ("ASSIGN_LEFT_SHIFT", 258),
            ("ASSIGN_RIGHT_SHIFT", 259),
            ("ASSIGN_UNSIGNED_RIGHT_SHIFT", 260),
            ("PRE_INCREMENT", 261),
            ("POST_INCREMENT", 262),
            ("PRE_DECREMENT", 263),
            ("POST_DECREMENT", 264),
            ("POSITIVE", 265),
            ("NEGATIVE", 266),
            ("BITWISE_OR", 267),
            ("BITWISE_AND", 268),
            ("BITWISE_XOR", 269),
            ("BITWISE_NOT", 270),
            ("VOID", 271),
            ("DELETE", 272),
            ("NEW_TARGET", 273),
            ("COMPUTED_PROP", 274),
            ("IMPORT_META", 275),
            ("OPTCHAIN_PROPERTY_ACCESS", 276),
            ("OPTCHAIN_CALL", 277),
            ("OPTCHAIN_ELEMENT_ACCESS", 278),
            ("COALESCE", 279),
            ("DYNAMIC_IMPORT", 280),
            ("ASSIGN_OR", 281),
            ("ASSIGN_AND", 282),
            ("ASSIGN_COALESCE", 283),
            ("BREAK_STATEMENT", 400),
            ("CONTINUE_STATEMENT", 401),
            ("DEBUGGER_STATEMENT", 402),
            ("DO_STATEMENT", 403),
            ("EXPRESSION_STATEMENT", 404),
            ("FOR_AWAIT_OF_STATEMENT", 405),
            ("FOR_IN_STATEMENT", 406),
            ("FOR_OF_STATEMENT", 407),
            ("FOR_STATEMENT", 408),
            ("IF_STATEMENT", 409),
            ("LABELED_STATEMENT", 410),
            ("RETURN_STATEMENT", 411),
            ("SWITCH_STATEMENT", 412),
            ("THROW_STATEMENT", 413),
            ("TRY_STATEMENT", 414),
            ("WHILE_STATEMENT", 415),
            ("BLOCK", 416),
            ("EMPTY", 417),
            ("IMPORT", 418),
            ("EXPORT", 419),
            ("WITH", 420),
            ("VAR_DECLARATION", 500),
            ("LET_DECLARATION", 501),
            ("CONST_DECLARATION", 502),
            ("FUNCTION_LITERAL", 503),
            ("CLASS_LITERAL", 504),
            ("SOURCE_FILE", 600),
            ("CASE", 601),
            ("DEFAULT_CASE", 602),
            ("CATCH", 603),
            ("CLASS_MEMBERS", 604),
            ("METHOD_DECLARATION", 605),
            ("PARAMETER_LIST", 606),
            ("RENAMABLE_STRING_KEY", 607),
            ("QUOTED_STRING_KEY", 608),
            ("LABELED_NAME", 609),
            ("ARRAY_PATTERN", 610),
            ("OBJECT_PATTERN", 611),
            ("DESTRUCTURING_LHS", 612),
            ("TEMPLATELIT_SUB", 613),
            ("TEMPLATELIT_STRING", 614),
            ("SUPER", 615),
            ("DEFAULT_VALUE", 616),
            ("IMPORT_SPECS", 619),
            ("IMPORT_SPEC", 620),
            ("IMPORT_STAR", 621),
            ("EXPORT_SPECS", 622),
            ("EXPORT_SPEC", 623),
            ("ITER_REST", 624),
            ("ITER_SPREAD", 625),
            ("OBJECT_REST", 626),
            ("OBJECT_SPREAD", 627),
            ("RENAMABLE_GETTER_DEF", 628),
            ("QUOTED_GETTER_DEF", 629),
            ("RENAMABLE_SETTER_DEF", 630),
            ("QUOTED_SETTER_DEF", 631),
            ("MODULE_BODY", 632),
            ("FIELD_DECLARATION", 633),
            ("COMPUTED_PROP_FIELD", 634),
            ("SWITCH_BODY", 635),
        ],
    },
    EnumDescriptor {
        full_name: "jscomp.NodeProperty",
        name: "NodeProperty",
        values: &[
            ("NODE_PROPERTY_UNSPECIFIED", 0),
            ("IS_PARENTHESIZED", 1),
            ("SYNTHETIC", 3),
            ("ADDED_BLOCK", 4),
            ("IS_CONSTANT_NAME", 6),
            ("IS_NAMESPACE", 7),
            ("DIRECT_EVAL", 9),
            ("FREE_CALL", 10),
            ("UNUSED_11", 11),
            ("REFLECTED_OBJECT", 12),
            ("STATIC_MEMBER", 13),
            ("GENERATOR_FN", 14),
            ("ARROW_FN", 15),
            ("ASYNC_FN", 16),
            ("YIELD_ALL", 17),
            ("EXPORT_DEFAULT", 18),
            ("EXPORT_ALL_FROM", 19),
            ("CONSTANT_VAR_FLAGS", 20),
            ("IS_GENERATOR_MARKER", 21),
            ("IS_GENERATOR_SAFE", 22),
            ("COMPUTED_PROP_METHOD", 23),
            ("COMPUTED_PROP_GETTER", 24),
            ("COMPUTED_PROP_SETTER", 25),
            ("COMPUTED_PROP_VARIABLE", 26),
            ("COLOR_FROM_CAST", 28),
            ("NON_INDEXABLE", 29),
            ("GOOG_MODULE", 30),
            ("DELETED", 35),
            ("MODULE_ALIAS", 36),
            ("IS_UNUSED_PARAMETER", 37),
            ("MODULE_EXPORT", 38),
            ("IS_SHORTHAND_PROPERTY", 39),
            ("ES6_MODULE", 40),
            ("START_OF_OPT_CHAIN", 41),
            ("TRAILING_COMMA", 42),
            ("IS_INFERRED_CONSTANT", 43),
            ("IS_DECLARED_CONSTANT", 44),
            ("SYNTHESIZED_UNFULFILLED_NAME_DECLARATION", 45),
            ("MUTATES_GLOBAL_STATE", 46),
            ("MUTATES_THIS", 47),
            ("MUTATES_ARGUMENTS", 48),
            ("THROWS", 49),
            ("CLOSURE_UNAWARE_SHADOW", 50),
            ("PRIVATE_IDENTIFIER", 51),
        ],
    },
];
