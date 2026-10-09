// Copyright 2026 The Closure Compiler Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/compiler_state/compiler_state.proto.

// Generated from src/com/google/javascript/jscomp/compiler_state/compiler_state.proto (protoc's Java API shape; D-003, no
// protoc on the host). Wire format and defaults follow proto3; see protobuf.rs.
#![allow(clippy::all, unused_imports, unused_variables, dead_code)]
use crate::conformance_config::{Requirement, RequirementScopeEntry};
use crate::serialization::protobuf::{
    CodedInputStream, CodedOutputStream, Descriptor, EnumDescriptor, FieldDescriptor, FieldType,
    Message, ProtoEnum, ProtoResult, WireFormat,
};
use closure_rhino::js_string::JsString;
use std::fmt;
use std::sync::LazyLock;
// port: FeatureProto (proto enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum FeatureProto {
    #[default]
    FEATURE_UNKNOWN = 0,
    FEATURE_REGEXP_SYNTAX = 1,
    FEATURE_ES3_KEYWORDS_AS_IDENTIFIERS = 2,
    FEATURE_GETTER = 3,
    FEATURE_KEYWORDS_AS_PROPERTIES = 4,
    FEATURE_SETTER = 5,
    FEATURE_STRING_CONTINUATION = 6,
    FEATURE_TRAILING_COMMA = 7,
    FEATURE_ARRAY_DESTRUCTURING = 8,
    FEATURE_ARRAY_PATTERN_REST = 9,
    FEATURE_ARROW_FUNCTIONS = 10,
    FEATURE_BINARY_LITERALS = 11,
    FEATURE_BLOCK_SCOPED_FUNCTION_DECLARATION = 12,
    FEATURE_CLASSES = 13,
    FEATURE_CLASS_GETTER_SETTER = 14,
    FEATURE_COMPUTED_PROPERTIES = 15,
    FEATURE_CONST_DECLARATIONS = 16,
    FEATURE_DEFAULT_PARAMETERS = 17,
    FEATURE_FOR_OF = 18,
    FEATURE_GENERATORS = 19,
    FEATURE_LET_DECLARATIONS = 20,
    FEATURE_MEMBER_DECLARATIONS = 21,
    FEATURE_NEW_TARGET = 22,
    FEATURE_OBJECT_DESTRUCTURING = 23,
    FEATURE_OCTAL_LITERALS = 24,
    FEATURE_REGEXP_FLAG_U = 25,
    FEATURE_REGEXP_FLAG_Y = 26,
    FEATURE_REST_PARAMETERS = 27,
    FEATURE_SHORTHAND_OBJECT_PROPERTIES = 28,
    FEATURE_SPREAD_EXPRESSIONS = 29,
    FEATURE_SUPER = 30,
    FEATURE_TEMPLATE_LITERALS = 31,
    FEATURE_MODULES = 32,
    FEATURE_EXPONENT_OP = 33,
    FEATURE_ASYNC_FUNCTIONS = 34,
    FEATURE_OBJECT_LITERALS_WITH_SPREAD = 35,
    FEATURE_OBJECT_PATTERN_REST = 36,
    FEATURE_ASYNC_GENERATORS = 37,
    FEATURE_FOR_AWAIT_OF = 38,
    FEATURE_REGEXP_FLAG_S = 39,
    FEATURE_REGEXP_NAMED_GROUPS = 40,
    FEATURE_REGEXP_UNICODE_PROPERTY_ESCAPE = 41,
    FEATURE_REGEXP_LOOKBEHIND = 42,
    FEATURE_UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP = 43,
    FEATURE_OPTIONAL_CATCH_BINDING = 44,
    FEATURE_DYNAMIC_IMPORT = 45,
    FEATURE_BIGINT = 46,
    FEATURE_IMPORT_META = 47,
    FEATURE_NULL_COALESCE_OP = 48,
    FEATURE_OPTIONAL_CHAINING = 49,
    FEATURE_NUMERIC_SEPARATOR = 50,
    FEATURE_LOGICAL_ASSIGNMENT = 51,
    FEATURE_PUBLIC_CLASS_FIELDS = 52,
    FEATURE_CLASS_STATIC_BLOCK = 53,
    FEATURE_REGEXP_FLAG_D = 54,
    FEATURE_TOP_LEVEL_AWAIT = 55,
    FEATURE_ES_NEXT_RUNTIME = 56,
    FEATURE_ES_UNSTABLE_RUNTIME = 57,
    FEATURE_PRIVATE_ELEMENTS = 58,
    FEATURE_TYPE_ANNOTATION = 59,
    UNRECOGNIZED = -1,
}
impl FeatureProto {
    pub const VALUES: [Self; 61] = [
        Self::FEATURE_UNKNOWN,
        Self::FEATURE_REGEXP_SYNTAX,
        Self::FEATURE_ES3_KEYWORDS_AS_IDENTIFIERS,
        Self::FEATURE_GETTER,
        Self::FEATURE_KEYWORDS_AS_PROPERTIES,
        Self::FEATURE_SETTER,
        Self::FEATURE_STRING_CONTINUATION,
        Self::FEATURE_TRAILING_COMMA,
        Self::FEATURE_ARRAY_DESTRUCTURING,
        Self::FEATURE_ARRAY_PATTERN_REST,
        Self::FEATURE_ARROW_FUNCTIONS,
        Self::FEATURE_BINARY_LITERALS,
        Self::FEATURE_BLOCK_SCOPED_FUNCTION_DECLARATION,
        Self::FEATURE_CLASSES,
        Self::FEATURE_CLASS_GETTER_SETTER,
        Self::FEATURE_COMPUTED_PROPERTIES,
        Self::FEATURE_CONST_DECLARATIONS,
        Self::FEATURE_DEFAULT_PARAMETERS,
        Self::FEATURE_FOR_OF,
        Self::FEATURE_GENERATORS,
        Self::FEATURE_LET_DECLARATIONS,
        Self::FEATURE_MEMBER_DECLARATIONS,
        Self::FEATURE_NEW_TARGET,
        Self::FEATURE_OBJECT_DESTRUCTURING,
        Self::FEATURE_OCTAL_LITERALS,
        Self::FEATURE_REGEXP_FLAG_U,
        Self::FEATURE_REGEXP_FLAG_Y,
        Self::FEATURE_REST_PARAMETERS,
        Self::FEATURE_SHORTHAND_OBJECT_PROPERTIES,
        Self::FEATURE_SPREAD_EXPRESSIONS,
        Self::FEATURE_SUPER,
        Self::FEATURE_TEMPLATE_LITERALS,
        Self::FEATURE_MODULES,
        Self::FEATURE_EXPONENT_OP,
        Self::FEATURE_ASYNC_FUNCTIONS,
        Self::FEATURE_OBJECT_LITERALS_WITH_SPREAD,
        Self::FEATURE_OBJECT_PATTERN_REST,
        Self::FEATURE_ASYNC_GENERATORS,
        Self::FEATURE_FOR_AWAIT_OF,
        Self::FEATURE_REGEXP_FLAG_S,
        Self::FEATURE_REGEXP_NAMED_GROUPS,
        Self::FEATURE_REGEXP_UNICODE_PROPERTY_ESCAPE,
        Self::FEATURE_REGEXP_LOOKBEHIND,
        Self::FEATURE_UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP,
        Self::FEATURE_OPTIONAL_CATCH_BINDING,
        Self::FEATURE_DYNAMIC_IMPORT,
        Self::FEATURE_BIGINT,
        Self::FEATURE_IMPORT_META,
        Self::FEATURE_NULL_COALESCE_OP,
        Self::FEATURE_OPTIONAL_CHAINING,
        Self::FEATURE_NUMERIC_SEPARATOR,
        Self::FEATURE_LOGICAL_ASSIGNMENT,
        Self::FEATURE_PUBLIC_CLASS_FIELDS,
        Self::FEATURE_CLASS_STATIC_BLOCK,
        Self::FEATURE_REGEXP_FLAG_D,
        Self::FEATURE_TOP_LEVEL_AWAIT,
        Self::FEATURE_ES_NEXT_RUNTIME,
        Self::FEATURE_ES_UNSTABLE_RUNTIME,
        Self::FEATURE_PRIVATE_ELEMENTS,
        Self::FEATURE_TYPE_ANNOTATION,
        Self::UNRECOGNIZED,
    ];
    // port: Enum#ordinal
    pub fn ordinal(self) -> usize {
        Self::VALUES
            .iter()
            .position(|value| *value == self)
            .unwrap()
    }
    // port: FeatureProto#getNumber
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
            Self::FEATURE_UNKNOWN => "FEATURE_UNKNOWN",
            Self::FEATURE_REGEXP_SYNTAX => "FEATURE_REGEXP_SYNTAX",
            Self::FEATURE_ES3_KEYWORDS_AS_IDENTIFIERS => "FEATURE_ES3_KEYWORDS_AS_IDENTIFIERS",
            Self::FEATURE_GETTER => "FEATURE_GETTER",
            Self::FEATURE_KEYWORDS_AS_PROPERTIES => "FEATURE_KEYWORDS_AS_PROPERTIES",
            Self::FEATURE_SETTER => "FEATURE_SETTER",
            Self::FEATURE_STRING_CONTINUATION => "FEATURE_STRING_CONTINUATION",
            Self::FEATURE_TRAILING_COMMA => "FEATURE_TRAILING_COMMA",
            Self::FEATURE_ARRAY_DESTRUCTURING => "FEATURE_ARRAY_DESTRUCTURING",
            Self::FEATURE_ARRAY_PATTERN_REST => "FEATURE_ARRAY_PATTERN_REST",
            Self::FEATURE_ARROW_FUNCTIONS => "FEATURE_ARROW_FUNCTIONS",
            Self::FEATURE_BINARY_LITERALS => "FEATURE_BINARY_LITERALS",
            Self::FEATURE_BLOCK_SCOPED_FUNCTION_DECLARATION => {
                "FEATURE_BLOCK_SCOPED_FUNCTION_DECLARATION"
            }
            Self::FEATURE_CLASSES => "FEATURE_CLASSES",
            Self::FEATURE_CLASS_GETTER_SETTER => "FEATURE_CLASS_GETTER_SETTER",
            Self::FEATURE_COMPUTED_PROPERTIES => "FEATURE_COMPUTED_PROPERTIES",
            Self::FEATURE_CONST_DECLARATIONS => "FEATURE_CONST_DECLARATIONS",
            Self::FEATURE_DEFAULT_PARAMETERS => "FEATURE_DEFAULT_PARAMETERS",
            Self::FEATURE_FOR_OF => "FEATURE_FOR_OF",
            Self::FEATURE_GENERATORS => "FEATURE_GENERATORS",
            Self::FEATURE_LET_DECLARATIONS => "FEATURE_LET_DECLARATIONS",
            Self::FEATURE_MEMBER_DECLARATIONS => "FEATURE_MEMBER_DECLARATIONS",
            Self::FEATURE_NEW_TARGET => "FEATURE_NEW_TARGET",
            Self::FEATURE_OBJECT_DESTRUCTURING => "FEATURE_OBJECT_DESTRUCTURING",
            Self::FEATURE_OCTAL_LITERALS => "FEATURE_OCTAL_LITERALS",
            Self::FEATURE_REGEXP_FLAG_U => "FEATURE_REGEXP_FLAG_U",
            Self::FEATURE_REGEXP_FLAG_Y => "FEATURE_REGEXP_FLAG_Y",
            Self::FEATURE_REST_PARAMETERS => "FEATURE_REST_PARAMETERS",
            Self::FEATURE_SHORTHAND_OBJECT_PROPERTIES => "FEATURE_SHORTHAND_OBJECT_PROPERTIES",
            Self::FEATURE_SPREAD_EXPRESSIONS => "FEATURE_SPREAD_EXPRESSIONS",
            Self::FEATURE_SUPER => "FEATURE_SUPER",
            Self::FEATURE_TEMPLATE_LITERALS => "FEATURE_TEMPLATE_LITERALS",
            Self::FEATURE_MODULES => "FEATURE_MODULES",
            Self::FEATURE_EXPONENT_OP => "FEATURE_EXPONENT_OP",
            Self::FEATURE_ASYNC_FUNCTIONS => "FEATURE_ASYNC_FUNCTIONS",
            Self::FEATURE_OBJECT_LITERALS_WITH_SPREAD => "FEATURE_OBJECT_LITERALS_WITH_SPREAD",
            Self::FEATURE_OBJECT_PATTERN_REST => "FEATURE_OBJECT_PATTERN_REST",
            Self::FEATURE_ASYNC_GENERATORS => "FEATURE_ASYNC_GENERATORS",
            Self::FEATURE_FOR_AWAIT_OF => "FEATURE_FOR_AWAIT_OF",
            Self::FEATURE_REGEXP_FLAG_S => "FEATURE_REGEXP_FLAG_S",
            Self::FEATURE_REGEXP_NAMED_GROUPS => "FEATURE_REGEXP_NAMED_GROUPS",
            Self::FEATURE_REGEXP_UNICODE_PROPERTY_ESCAPE => {
                "FEATURE_REGEXP_UNICODE_PROPERTY_ESCAPE"
            }
            Self::FEATURE_REGEXP_LOOKBEHIND => "FEATURE_REGEXP_LOOKBEHIND",
            Self::FEATURE_UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP => {
                "FEATURE_UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP"
            }
            Self::FEATURE_OPTIONAL_CATCH_BINDING => "FEATURE_OPTIONAL_CATCH_BINDING",
            Self::FEATURE_DYNAMIC_IMPORT => "FEATURE_DYNAMIC_IMPORT",
            Self::FEATURE_BIGINT => "FEATURE_BIGINT",
            Self::FEATURE_IMPORT_META => "FEATURE_IMPORT_META",
            Self::FEATURE_NULL_COALESCE_OP => "FEATURE_NULL_COALESCE_OP",
            Self::FEATURE_OPTIONAL_CHAINING => "FEATURE_OPTIONAL_CHAINING",
            Self::FEATURE_NUMERIC_SEPARATOR => "FEATURE_NUMERIC_SEPARATOR",
            Self::FEATURE_LOGICAL_ASSIGNMENT => "FEATURE_LOGICAL_ASSIGNMENT",
            Self::FEATURE_PUBLIC_CLASS_FIELDS => "FEATURE_PUBLIC_CLASS_FIELDS",
            Self::FEATURE_CLASS_STATIC_BLOCK => "FEATURE_CLASS_STATIC_BLOCK",
            Self::FEATURE_REGEXP_FLAG_D => "FEATURE_REGEXP_FLAG_D",
            Self::FEATURE_TOP_LEVEL_AWAIT => "FEATURE_TOP_LEVEL_AWAIT",
            Self::FEATURE_ES_NEXT_RUNTIME => "FEATURE_ES_NEXT_RUNTIME",
            Self::FEATURE_ES_UNSTABLE_RUNTIME => "FEATURE_ES_UNSTABLE_RUNTIME",
            Self::FEATURE_PRIVATE_ELEMENTS => "FEATURE_PRIVATE_ELEMENTS",
            Self::FEATURE_TYPE_ANNOTATION => "FEATURE_TYPE_ANNOTATION",
            Self::UNRECOGNIZED => "UNRECOGNIZED",
        }
    }
    // port: FeatureProto#valueOf(String) (`None` where Java throws IllegalArgumentException)
    pub fn value_of(name: &str) -> Option<Self> {
        match name {
            "FEATURE_UNKNOWN" => Some(Self::FEATURE_UNKNOWN),
            "FEATURE_REGEXP_SYNTAX" => Some(Self::FEATURE_REGEXP_SYNTAX),
            "FEATURE_ES3_KEYWORDS_AS_IDENTIFIERS" => {
                Some(Self::FEATURE_ES3_KEYWORDS_AS_IDENTIFIERS)
            }
            "FEATURE_GETTER" => Some(Self::FEATURE_GETTER),
            "FEATURE_KEYWORDS_AS_PROPERTIES" => Some(Self::FEATURE_KEYWORDS_AS_PROPERTIES),
            "FEATURE_SETTER" => Some(Self::FEATURE_SETTER),
            "FEATURE_STRING_CONTINUATION" => Some(Self::FEATURE_STRING_CONTINUATION),
            "FEATURE_TRAILING_COMMA" => Some(Self::FEATURE_TRAILING_COMMA),
            "FEATURE_ARRAY_DESTRUCTURING" => Some(Self::FEATURE_ARRAY_DESTRUCTURING),
            "FEATURE_ARRAY_PATTERN_REST" => Some(Self::FEATURE_ARRAY_PATTERN_REST),
            "FEATURE_ARROW_FUNCTIONS" => Some(Self::FEATURE_ARROW_FUNCTIONS),
            "FEATURE_BINARY_LITERALS" => Some(Self::FEATURE_BINARY_LITERALS),
            "FEATURE_BLOCK_SCOPED_FUNCTION_DECLARATION" => {
                Some(Self::FEATURE_BLOCK_SCOPED_FUNCTION_DECLARATION)
            }
            "FEATURE_CLASSES" => Some(Self::FEATURE_CLASSES),
            "FEATURE_CLASS_GETTER_SETTER" => Some(Self::FEATURE_CLASS_GETTER_SETTER),
            "FEATURE_COMPUTED_PROPERTIES" => Some(Self::FEATURE_COMPUTED_PROPERTIES),
            "FEATURE_CONST_DECLARATIONS" => Some(Self::FEATURE_CONST_DECLARATIONS),
            "FEATURE_DEFAULT_PARAMETERS" => Some(Self::FEATURE_DEFAULT_PARAMETERS),
            "FEATURE_FOR_OF" => Some(Self::FEATURE_FOR_OF),
            "FEATURE_GENERATORS" => Some(Self::FEATURE_GENERATORS),
            "FEATURE_LET_DECLARATIONS" => Some(Self::FEATURE_LET_DECLARATIONS),
            "FEATURE_MEMBER_DECLARATIONS" => Some(Self::FEATURE_MEMBER_DECLARATIONS),
            "FEATURE_NEW_TARGET" => Some(Self::FEATURE_NEW_TARGET),
            "FEATURE_OBJECT_DESTRUCTURING" => Some(Self::FEATURE_OBJECT_DESTRUCTURING),
            "FEATURE_OCTAL_LITERALS" => Some(Self::FEATURE_OCTAL_LITERALS),
            "FEATURE_REGEXP_FLAG_U" => Some(Self::FEATURE_REGEXP_FLAG_U),
            "FEATURE_REGEXP_FLAG_Y" => Some(Self::FEATURE_REGEXP_FLAG_Y),
            "FEATURE_REST_PARAMETERS" => Some(Self::FEATURE_REST_PARAMETERS),
            "FEATURE_SHORTHAND_OBJECT_PROPERTIES" => {
                Some(Self::FEATURE_SHORTHAND_OBJECT_PROPERTIES)
            }
            "FEATURE_SPREAD_EXPRESSIONS" => Some(Self::FEATURE_SPREAD_EXPRESSIONS),
            "FEATURE_SUPER" => Some(Self::FEATURE_SUPER),
            "FEATURE_TEMPLATE_LITERALS" => Some(Self::FEATURE_TEMPLATE_LITERALS),
            "FEATURE_MODULES" => Some(Self::FEATURE_MODULES),
            "FEATURE_EXPONENT_OP" => Some(Self::FEATURE_EXPONENT_OP),
            "FEATURE_ASYNC_FUNCTIONS" => Some(Self::FEATURE_ASYNC_FUNCTIONS),
            "FEATURE_OBJECT_LITERALS_WITH_SPREAD" => {
                Some(Self::FEATURE_OBJECT_LITERALS_WITH_SPREAD)
            }
            "FEATURE_OBJECT_PATTERN_REST" => Some(Self::FEATURE_OBJECT_PATTERN_REST),
            "FEATURE_ASYNC_GENERATORS" => Some(Self::FEATURE_ASYNC_GENERATORS),
            "FEATURE_FOR_AWAIT_OF" => Some(Self::FEATURE_FOR_AWAIT_OF),
            "FEATURE_REGEXP_FLAG_S" => Some(Self::FEATURE_REGEXP_FLAG_S),
            "FEATURE_REGEXP_NAMED_GROUPS" => Some(Self::FEATURE_REGEXP_NAMED_GROUPS),
            "FEATURE_REGEXP_UNICODE_PROPERTY_ESCAPE" => {
                Some(Self::FEATURE_REGEXP_UNICODE_PROPERTY_ESCAPE)
            }
            "FEATURE_REGEXP_LOOKBEHIND" => Some(Self::FEATURE_REGEXP_LOOKBEHIND),
            "FEATURE_UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP" => {
                Some(Self::FEATURE_UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP)
            }
            "FEATURE_OPTIONAL_CATCH_BINDING" => Some(Self::FEATURE_OPTIONAL_CATCH_BINDING),
            "FEATURE_DYNAMIC_IMPORT" => Some(Self::FEATURE_DYNAMIC_IMPORT),
            "FEATURE_BIGINT" => Some(Self::FEATURE_BIGINT),
            "FEATURE_IMPORT_META" => Some(Self::FEATURE_IMPORT_META),
            "FEATURE_NULL_COALESCE_OP" => Some(Self::FEATURE_NULL_COALESCE_OP),
            "FEATURE_OPTIONAL_CHAINING" => Some(Self::FEATURE_OPTIONAL_CHAINING),
            "FEATURE_NUMERIC_SEPARATOR" => Some(Self::FEATURE_NUMERIC_SEPARATOR),
            "FEATURE_LOGICAL_ASSIGNMENT" => Some(Self::FEATURE_LOGICAL_ASSIGNMENT),
            "FEATURE_PUBLIC_CLASS_FIELDS" => Some(Self::FEATURE_PUBLIC_CLASS_FIELDS),
            "FEATURE_CLASS_STATIC_BLOCK" => Some(Self::FEATURE_CLASS_STATIC_BLOCK),
            "FEATURE_REGEXP_FLAG_D" => Some(Self::FEATURE_REGEXP_FLAG_D),
            "FEATURE_TOP_LEVEL_AWAIT" => Some(Self::FEATURE_TOP_LEVEL_AWAIT),
            "FEATURE_ES_NEXT_RUNTIME" => Some(Self::FEATURE_ES_NEXT_RUNTIME),
            "FEATURE_ES_UNSTABLE_RUNTIME" => Some(Self::FEATURE_ES_UNSTABLE_RUNTIME),
            "FEATURE_PRIVATE_ELEMENTS" => Some(Self::FEATURE_PRIVATE_ELEMENTS),
            "FEATURE_TYPE_ANNOTATION" => Some(Self::FEATURE_TYPE_ANNOTATION),
            "UNRECOGNIZED" => Some(Self::UNRECOGNIZED),
            _ => None,
        }
    }
    // port: FeatureProto#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::FEATURE_UNKNOWN),
            1 => Some(Self::FEATURE_REGEXP_SYNTAX),
            2 => Some(Self::FEATURE_ES3_KEYWORDS_AS_IDENTIFIERS),
            3 => Some(Self::FEATURE_GETTER),
            4 => Some(Self::FEATURE_KEYWORDS_AS_PROPERTIES),
            5 => Some(Self::FEATURE_SETTER),
            6 => Some(Self::FEATURE_STRING_CONTINUATION),
            7 => Some(Self::FEATURE_TRAILING_COMMA),
            8 => Some(Self::FEATURE_ARRAY_DESTRUCTURING),
            9 => Some(Self::FEATURE_ARRAY_PATTERN_REST),
            10 => Some(Self::FEATURE_ARROW_FUNCTIONS),
            11 => Some(Self::FEATURE_BINARY_LITERALS),
            12 => Some(Self::FEATURE_BLOCK_SCOPED_FUNCTION_DECLARATION),
            13 => Some(Self::FEATURE_CLASSES),
            14 => Some(Self::FEATURE_CLASS_GETTER_SETTER),
            15 => Some(Self::FEATURE_COMPUTED_PROPERTIES),
            16 => Some(Self::FEATURE_CONST_DECLARATIONS),
            17 => Some(Self::FEATURE_DEFAULT_PARAMETERS),
            18 => Some(Self::FEATURE_FOR_OF),
            19 => Some(Self::FEATURE_GENERATORS),
            20 => Some(Self::FEATURE_LET_DECLARATIONS),
            21 => Some(Self::FEATURE_MEMBER_DECLARATIONS),
            22 => Some(Self::FEATURE_NEW_TARGET),
            23 => Some(Self::FEATURE_OBJECT_DESTRUCTURING),
            24 => Some(Self::FEATURE_OCTAL_LITERALS),
            25 => Some(Self::FEATURE_REGEXP_FLAG_U),
            26 => Some(Self::FEATURE_REGEXP_FLAG_Y),
            27 => Some(Self::FEATURE_REST_PARAMETERS),
            28 => Some(Self::FEATURE_SHORTHAND_OBJECT_PROPERTIES),
            29 => Some(Self::FEATURE_SPREAD_EXPRESSIONS),
            30 => Some(Self::FEATURE_SUPER),
            31 => Some(Self::FEATURE_TEMPLATE_LITERALS),
            32 => Some(Self::FEATURE_MODULES),
            33 => Some(Self::FEATURE_EXPONENT_OP),
            34 => Some(Self::FEATURE_ASYNC_FUNCTIONS),
            35 => Some(Self::FEATURE_OBJECT_LITERALS_WITH_SPREAD),
            36 => Some(Self::FEATURE_OBJECT_PATTERN_REST),
            37 => Some(Self::FEATURE_ASYNC_GENERATORS),
            38 => Some(Self::FEATURE_FOR_AWAIT_OF),
            39 => Some(Self::FEATURE_REGEXP_FLAG_S),
            40 => Some(Self::FEATURE_REGEXP_NAMED_GROUPS),
            41 => Some(Self::FEATURE_REGEXP_UNICODE_PROPERTY_ESCAPE),
            42 => Some(Self::FEATURE_REGEXP_LOOKBEHIND),
            43 => Some(Self::FEATURE_UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP),
            44 => Some(Self::FEATURE_OPTIONAL_CATCH_BINDING),
            45 => Some(Self::FEATURE_DYNAMIC_IMPORT),
            46 => Some(Self::FEATURE_BIGINT),
            47 => Some(Self::FEATURE_IMPORT_META),
            48 => Some(Self::FEATURE_NULL_COALESCE_OP),
            49 => Some(Self::FEATURE_OPTIONAL_CHAINING),
            50 => Some(Self::FEATURE_NUMERIC_SEPARATOR),
            51 => Some(Self::FEATURE_LOGICAL_ASSIGNMENT),
            52 => Some(Self::FEATURE_PUBLIC_CLASS_FIELDS),
            53 => Some(Self::FEATURE_CLASS_STATIC_BLOCK),
            54 => Some(Self::FEATURE_REGEXP_FLAG_D),
            55 => Some(Self::FEATURE_TOP_LEVEL_AWAIT),
            56 => Some(Self::FEATURE_ES_NEXT_RUNTIME),
            57 => Some(Self::FEATURE_ES_UNSTABLE_RUNTIME),
            58 => Some(Self::FEATURE_PRIVATE_ELEMENTS),
            59 => Some(Self::FEATURE_TYPE_ANNOTATION),
            _ => None,
        }
    }
}
impl ProtoEnum for FeatureProto {
    fn get_number(self) -> i32 {
        self.get_number()
    }
    fn for_number_or_unrecognized(value: i32) -> Self {
        Self::for_number(value).unwrap_or(Self::UNRECOGNIZED)
    }
}
impl fmt::Display for FeatureProto {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: LifeCycleStageProto (proto enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum LifeCycleStageProto {
    #[default]
    LIFE_CYCLE_STAGE_UNKNOWN = 0,
    LIFE_CYCLE_STAGE_RAW = 1,
    LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC = 2,
    LIFE_CYCLE_STAGE_NORMALIZED = 3,
    LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED = 4,
    UNRECOGNIZED = -1,
}
impl LifeCycleStageProto {
    pub const VALUES: [Self; 6] = [
        Self::LIFE_CYCLE_STAGE_UNKNOWN,
        Self::LIFE_CYCLE_STAGE_RAW,
        Self::LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC,
        Self::LIFE_CYCLE_STAGE_NORMALIZED,
        Self::LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED,
        Self::UNRECOGNIZED,
    ];
    // port: Enum#ordinal
    pub fn ordinal(self) -> usize {
        Self::VALUES
            .iter()
            .position(|value| *value == self)
            .unwrap()
    }
    // port: LifeCycleStageProto#getNumber
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
            Self::LIFE_CYCLE_STAGE_UNKNOWN => "LIFE_CYCLE_STAGE_UNKNOWN",
            Self::LIFE_CYCLE_STAGE_RAW => "LIFE_CYCLE_STAGE_RAW",
            Self::LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC => {
                "LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC"
            }
            Self::LIFE_CYCLE_STAGE_NORMALIZED => "LIFE_CYCLE_STAGE_NORMALIZED",
            Self::LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED => {
                "LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED"
            }
            Self::UNRECOGNIZED => "UNRECOGNIZED",
        }
    }
    // port: LifeCycleStageProto#valueOf(String) (`None` where Java throws IllegalArgumentException)
    pub fn value_of(name: &str) -> Option<Self> {
        match name {
            "LIFE_CYCLE_STAGE_UNKNOWN" => Some(Self::LIFE_CYCLE_STAGE_UNKNOWN),
            "LIFE_CYCLE_STAGE_RAW" => Some(Self::LIFE_CYCLE_STAGE_RAW),
            "LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC" => {
                Some(Self::LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC)
            }
            "LIFE_CYCLE_STAGE_NORMALIZED" => Some(Self::LIFE_CYCLE_STAGE_NORMALIZED),
            "LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED" => {
                Some(Self::LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED)
            }
            "UNRECOGNIZED" => Some(Self::UNRECOGNIZED),
            _ => None,
        }
    }
    // port: LifeCycleStageProto#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::LIFE_CYCLE_STAGE_UNKNOWN),
            1 => Some(Self::LIFE_CYCLE_STAGE_RAW),
            2 => Some(Self::LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC),
            3 => Some(Self::LIFE_CYCLE_STAGE_NORMALIZED),
            4 => Some(Self::LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED),
            _ => None,
        }
    }
}
impl ProtoEnum for LifeCycleStageProto {
    fn get_number(self) -> i32 {
        self.get_number()
    }
    fn for_number_or_unrecognized(value: i32) -> Self {
        Self::for_number(value).unwrap_or(Self::UNRECOGNIZED)
    }
}
impl fmt::Display for LifeCycleStageProto {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: PropertyAccessKindProto (proto enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum PropertyAccessKindProto {
    #[default]
    KIND_UNKNOWN = 0,
    KIND_NORMAL = 1,
    KIND_GETTER_ONLY = 2,
    KIND_SETTER_ONLY = 3,
    KIND_GETTER_AND_SETTER = 4,
    UNRECOGNIZED = -1,
}
impl PropertyAccessKindProto {
    pub const VALUES: [Self; 6] = [
        Self::KIND_UNKNOWN,
        Self::KIND_NORMAL,
        Self::KIND_GETTER_ONLY,
        Self::KIND_SETTER_ONLY,
        Self::KIND_GETTER_AND_SETTER,
        Self::UNRECOGNIZED,
    ];
    // port: Enum#ordinal
    pub fn ordinal(self) -> usize {
        Self::VALUES
            .iter()
            .position(|value| *value == self)
            .unwrap()
    }
    // port: PropertyAccessKindProto#getNumber
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
            Self::KIND_UNKNOWN => "KIND_UNKNOWN",
            Self::KIND_NORMAL => "KIND_NORMAL",
            Self::KIND_GETTER_ONLY => "KIND_GETTER_ONLY",
            Self::KIND_SETTER_ONLY => "KIND_SETTER_ONLY",
            Self::KIND_GETTER_AND_SETTER => "KIND_GETTER_AND_SETTER",
            Self::UNRECOGNIZED => "UNRECOGNIZED",
        }
    }
    // port: PropertyAccessKindProto#valueOf(String) (`None` where Java throws IllegalArgumentException)
    pub fn value_of(name: &str) -> Option<Self> {
        match name {
            "KIND_UNKNOWN" => Some(Self::KIND_UNKNOWN),
            "KIND_NORMAL" => Some(Self::KIND_NORMAL),
            "KIND_GETTER_ONLY" => Some(Self::KIND_GETTER_ONLY),
            "KIND_SETTER_ONLY" => Some(Self::KIND_SETTER_ONLY),
            "KIND_GETTER_AND_SETTER" => Some(Self::KIND_GETTER_AND_SETTER),
            "UNRECOGNIZED" => Some(Self::UNRECOGNIZED),
            _ => None,
        }
    }
    // port: PropertyAccessKindProto#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::KIND_UNKNOWN),
            1 => Some(Self::KIND_NORMAL),
            2 => Some(Self::KIND_GETTER_ONLY),
            3 => Some(Self::KIND_SETTER_ONLY),
            4 => Some(Self::KIND_GETTER_AND_SETTER),
            _ => None,
        }
    }
}
impl ProtoEnum for PropertyAccessKindProto {
    fn get_number(self) -> i32 {
        self.get_number()
    }
    fn for_number_or_unrecognized(value: i32) -> Self {
        Self::for_number(value).unwrap_or(Self::UNRECOGNIZED)
    }
}
impl fmt::Display for PropertyAccessKindProto {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
// port: JSCompilerStateProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct JSCompilerStateProto {
    pub allowable_features: Vec<FeatureProto>,
    pub type_checking_has_run: bool,
    pub has_reg_exp_global_references: bool,
    pub life_cycle_stage: LifeCycleStageProto,
    pub merged_precompiled_libraries: bool,
    pub chunks: Vec<ChunkProto>,
    pub unique_name_id: i32,
    pub unique_id_supplier: Vec<UniqueIdProto>,
    pub exported_names: Vec<String>,
    pub has_css_names: bool,
    pub css_names: Vec<String>,
    pub id_generator_map: Option<String>,
    pub transpiled_files: bool,
    pub id_generator_current_id: i32,
    pub run_j2cl_passes: bool,
    pub externs: Vec<String>,
    pub injected_libraries: Vec<String>,
    pub last_injected_library_index_in_first_script: i32,
    pub accessor_summary: Option<AccessorSummaryProto>,
    pub has_string_map: bool,
    pub string_map: Vec<VariableMapEntryProto>,
    pub has_instrumentation_mapping: bool,
    pub instrumentation_mapping: Vec<VariableMapEntryProto>,
    pub conformance_violations: Vec<ConformanceViolationsProto>,
    pub unattributed_conformance_violations: Vec<ViolationProto>,
}
static JSCOMPILERSTATEPROTO_DEFAULT_INSTANCE: LazyLock<JSCompilerStateProto> =
    LazyLock::new(JSCompilerStateProto::default);
impl JSCompilerStateProto {
    pub const ALLOWABLE_FEATURES_FIELD_NUMBER: i32 = 1;
    pub const TYPE_CHECKING_HAS_RUN_FIELD_NUMBER: i32 = 2;
    pub const HAS_REG_EXP_GLOBAL_REFERENCES_FIELD_NUMBER: i32 = 3;
    pub const LIFE_CYCLE_STAGE_FIELD_NUMBER: i32 = 4;
    pub const MERGED_PRECOMPILED_LIBRARIES_FIELD_NUMBER: i32 = 5;
    pub const CHUNKS_FIELD_NUMBER: i32 = 6;
    pub const UNIQUE_NAME_ID_FIELD_NUMBER: i32 = 7;
    pub const UNIQUE_ID_SUPPLIER_FIELD_NUMBER: i32 = 8;
    pub const EXPORTED_NAMES_FIELD_NUMBER: i32 = 9;
    pub const HAS_CSS_NAMES_FIELD_NUMBER: i32 = 10;
    pub const CSS_NAMES_FIELD_NUMBER: i32 = 11;
    pub const ID_GENERATOR_MAP_FIELD_NUMBER: i32 = 12;
    pub const TRANSPILED_FILES_FIELD_NUMBER: i32 = 13;
    pub const ID_GENERATOR_CURRENT_ID_FIELD_NUMBER: i32 = 14;
    pub const RUN_J2CL_PASSES_FIELD_NUMBER: i32 = 15;
    pub const EXTERNS_FIELD_NUMBER: i32 = 16;
    pub const INJECTED_LIBRARIES_FIELD_NUMBER: i32 = 17;
    pub const LAST_INJECTED_LIBRARY_INDEX_IN_FIRST_SCRIPT_FIELD_NUMBER: i32 = 18;
    pub const ACCESSOR_SUMMARY_FIELD_NUMBER: i32 = 19;
    pub const HAS_STRING_MAP_FIELD_NUMBER: i32 = 20;
    pub const STRING_MAP_FIELD_NUMBER: i32 = 21;
    pub const HAS_INSTRUMENTATION_MAPPING_FIELD_NUMBER: i32 = 22;
    pub const INSTRUMENTATION_MAPPING_FIELD_NUMBER: i32 = 23;
    pub const CONFORMANCE_VIOLATIONS_FIELD_NUMBER: i32 = 24;
    pub const UNATTRIBUTED_CONFORMANCE_VIOLATIONS_FIELD_NUMBER: i32 = 25;
    // port: JSCompilerStateProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: JSCompilerStateProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &JSCOMPILERSTATEPROTO_DEFAULT_INSTANCE
    }
    // port: JSCompilerStateProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: JSCompilerStateProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: JSCompilerStateProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: JSCompilerStateProto#getAllowableFeaturesList
    pub fn get_allowable_features_list(&self) -> &[FeatureProto] {
        &self.allowable_features
    }
    // port: JSCompilerStateProto#getAllowableFeaturesCount
    pub fn get_allowable_features_count(&self) -> i32 {
        self.allowable_features.len() as i32
    }
    // port: JSCompilerStateProto#getAllowableFeatures(int)
    pub fn get_allowable_features(&self, index: i32) -> FeatureProto {
        self.allowable_features[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addAllowableFeatures
    pub fn add_allowable_features(mut self, value: FeatureProto) -> Self {
        self.allowable_features.push(value);
        self
    }
    // port: JSCompilerStateProto.Builder#addAllAllowableFeatures
    pub fn add_all_allowable_features(
        mut self,
        values: impl IntoIterator<Item = FeatureProto>,
    ) -> Self {
        self.allowable_features.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearAllowableFeatures
    pub fn clear_allowable_features(mut self) -> Self {
        self.allowable_features.clear();
        self
    }
    // port: JSCompilerStateProto#getTypeCheckingHasRun
    pub fn get_type_checking_has_run(&self) -> bool {
        self.type_checking_has_run
    }
    // port: JSCompilerStateProto.Builder#setTypeCheckingHasRun
    pub fn set_type_checking_has_run(mut self, value: bool) -> Self {
        self.type_checking_has_run = value;
        self
    }
    // port: JSCompilerStateProto#getHasRegExpGlobalReferences
    pub fn get_has_reg_exp_global_references(&self) -> bool {
        self.has_reg_exp_global_references
    }
    // port: JSCompilerStateProto.Builder#setHasRegExpGlobalReferences
    pub fn set_has_reg_exp_global_references(mut self, value: bool) -> Self {
        self.has_reg_exp_global_references = value;
        self
    }
    // port: JSCompilerStateProto#getLifeCycleStage
    pub fn get_life_cycle_stage(&self) -> LifeCycleStageProto {
        self.life_cycle_stage
    }
    // port: JSCompilerStateProto.Builder#setLifeCycleStage
    pub fn set_life_cycle_stage(mut self, value: LifeCycleStageProto) -> Self {
        self.life_cycle_stage = value;
        self
    }
    // port: JSCompilerStateProto#getLifeCycleStageValue
    pub fn get_life_cycle_stage_value(&self) -> i32 {
        self.life_cycle_stage.get_number()
    }
    // port: JSCompilerStateProto#getMergedPrecompiledLibraries
    pub fn get_merged_precompiled_libraries(&self) -> bool {
        self.merged_precompiled_libraries
    }
    // port: JSCompilerStateProto.Builder#setMergedPrecompiledLibraries
    pub fn set_merged_precompiled_libraries(mut self, value: bool) -> Self {
        self.merged_precompiled_libraries = value;
        self
    }
    // port: JSCompilerStateProto#getChunksList
    pub fn get_chunks_list(&self) -> &[ChunkProto] {
        &self.chunks
    }
    // port: JSCompilerStateProto#getChunksCount
    pub fn get_chunks_count(&self) -> i32 {
        self.chunks.len() as i32
    }
    // port: JSCompilerStateProto#getChunks(int)
    pub fn get_chunks(&self, index: i32) -> &ChunkProto {
        &self.chunks[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addChunks
    pub fn add_chunks(mut self, value: ChunkProto) -> Self {
        self.chunks.push(value);
        self
    }
    // port: JSCompilerStateProto.Builder#addAllChunks
    pub fn add_all_chunks(mut self, values: impl IntoIterator<Item = ChunkProto>) -> Self {
        self.chunks.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearChunks
    pub fn clear_chunks(mut self) -> Self {
        self.chunks.clear();
        self
    }
    // port: JSCompilerStateProto#getUniqueNameId
    pub fn get_unique_name_id(&self) -> i32 {
        self.unique_name_id
    }
    // port: JSCompilerStateProto.Builder#setUniqueNameId
    pub fn set_unique_name_id(mut self, value: i32) -> Self {
        self.unique_name_id = value;
        self
    }
    // port: JSCompilerStateProto#getUniqueIdSupplierList
    pub fn get_unique_id_supplier_list(&self) -> &[UniqueIdProto] {
        &self.unique_id_supplier
    }
    // port: JSCompilerStateProto#getUniqueIdSupplierCount
    pub fn get_unique_id_supplier_count(&self) -> i32 {
        self.unique_id_supplier.len() as i32
    }
    // port: JSCompilerStateProto#getUniqueIdSupplier(int)
    pub fn get_unique_id_supplier(&self, index: i32) -> &UniqueIdProto {
        &self.unique_id_supplier[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addUniqueIdSupplier
    pub fn add_unique_id_supplier(mut self, value: UniqueIdProto) -> Self {
        self.unique_id_supplier.push(value);
        self
    }
    // port: JSCompilerStateProto.Builder#addAllUniqueIdSupplier
    pub fn add_all_unique_id_supplier(
        mut self,
        values: impl IntoIterator<Item = UniqueIdProto>,
    ) -> Self {
        self.unique_id_supplier.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearUniqueIdSupplier
    pub fn clear_unique_id_supplier(mut self) -> Self {
        self.unique_id_supplier.clear();
        self
    }
    // port: JSCompilerStateProto#getExportedNamesList
    pub fn get_exported_names_list(&self) -> &[String] {
        &self.exported_names
    }
    // port: JSCompilerStateProto#getExportedNamesCount
    pub fn get_exported_names_count(&self) -> i32 {
        self.exported_names.len() as i32
    }
    // port: JSCompilerStateProto#getExportedNames(int)
    pub fn get_exported_names(&self, index: i32) -> &String {
        &self.exported_names[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addExportedNames
    pub fn add_exported_names(mut self, value: impl Into<String>) -> Self {
        self.exported_names.push(value.into());
        self
    }
    // port: JSCompilerStateProto.Builder#addAllExportedNames
    pub fn add_all_exported_names(mut self, values: impl IntoIterator<Item = String>) -> Self {
        self.exported_names.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearExportedNames
    pub fn clear_exported_names(mut self) -> Self {
        self.exported_names.clear();
        self
    }
    // port: JSCompilerStateProto#getHasCssNames
    pub fn get_has_css_names(&self) -> bool {
        self.has_css_names
    }
    // port: JSCompilerStateProto.Builder#setHasCssNames
    pub fn set_has_css_names(mut self, value: bool) -> Self {
        self.has_css_names = value;
        self
    }
    // port: JSCompilerStateProto#getCssNamesList
    pub fn get_css_names_list(&self) -> &[String] {
        &self.css_names
    }
    // port: JSCompilerStateProto#getCssNamesCount
    pub fn get_css_names_count(&self) -> i32 {
        self.css_names.len() as i32
    }
    // port: JSCompilerStateProto#getCssNames(int)
    pub fn get_css_names(&self, index: i32) -> &String {
        &self.css_names[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addCssNames
    pub fn add_css_names(mut self, value: impl Into<String>) -> Self {
        self.css_names.push(value.into());
        self
    }
    // port: JSCompilerStateProto.Builder#addAllCssNames
    pub fn add_all_css_names(mut self, values: impl IntoIterator<Item = String>) -> Self {
        self.css_names.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearCssNames
    pub fn clear_css_names(mut self) -> Self {
        self.css_names.clear();
        self
    }
    // port: JSCompilerStateProto#hasIdGeneratorMap
    pub fn has_id_generator_map(&self) -> bool {
        self.id_generator_map.is_some()
    }
    // port: JSCompilerStateProto#getIdGeneratorMap
    pub fn get_id_generator_map(&self) -> &str {
        self.id_generator_map.as_deref().unwrap_or("")
    }
    // port: JSCompilerStateProto.Builder#setIdGeneratorMap
    pub fn set_id_generator_map(mut self, value: impl Into<String>) -> Self {
        self.id_generator_map = Some(value.into());
        self
    }
    // port: JSCompilerStateProto.Builder#clearIdGeneratorMap
    pub fn clear_id_generator_map(mut self) -> Self {
        self.id_generator_map = None;
        self
    }
    // port: JSCompilerStateProto#getTranspiledFiles
    pub fn get_transpiled_files(&self) -> bool {
        self.transpiled_files
    }
    // port: JSCompilerStateProto.Builder#setTranspiledFiles
    pub fn set_transpiled_files(mut self, value: bool) -> Self {
        self.transpiled_files = value;
        self
    }
    // port: JSCompilerStateProto#getIdGeneratorCurrentId
    pub fn get_id_generator_current_id(&self) -> i32 {
        self.id_generator_current_id
    }
    // port: JSCompilerStateProto.Builder#setIdGeneratorCurrentId
    pub fn set_id_generator_current_id(mut self, value: i32) -> Self {
        self.id_generator_current_id = value;
        self
    }
    // port: JSCompilerStateProto#getRunJ2clPasses
    pub fn get_run_j2cl_passes(&self) -> bool {
        self.run_j2cl_passes
    }
    // port: JSCompilerStateProto.Builder#setRunJ2clPasses
    pub fn set_run_j2cl_passes(mut self, value: bool) -> Self {
        self.run_j2cl_passes = value;
        self
    }
    // port: JSCompilerStateProto#getExternsList
    pub fn get_externs_list(&self) -> &[String] {
        &self.externs
    }
    // port: JSCompilerStateProto#getExternsCount
    pub fn get_externs_count(&self) -> i32 {
        self.externs.len() as i32
    }
    // port: JSCompilerStateProto#getExterns(int)
    pub fn get_externs(&self, index: i32) -> &String {
        &self.externs[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addExterns
    pub fn add_externs(mut self, value: impl Into<String>) -> Self {
        self.externs.push(value.into());
        self
    }
    // port: JSCompilerStateProto.Builder#addAllExterns
    pub fn add_all_externs(mut self, values: impl IntoIterator<Item = String>) -> Self {
        self.externs.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearExterns
    pub fn clear_externs(mut self) -> Self {
        self.externs.clear();
        self
    }
    // port: JSCompilerStateProto#getInjectedLibrariesList
    pub fn get_injected_libraries_list(&self) -> &[String] {
        &self.injected_libraries
    }
    // port: JSCompilerStateProto#getInjectedLibrariesCount
    pub fn get_injected_libraries_count(&self) -> i32 {
        self.injected_libraries.len() as i32
    }
    // port: JSCompilerStateProto#getInjectedLibraries(int)
    pub fn get_injected_libraries(&self, index: i32) -> &String {
        &self.injected_libraries[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addInjectedLibraries
    pub fn add_injected_libraries(mut self, value: impl Into<String>) -> Self {
        self.injected_libraries.push(value.into());
        self
    }
    // port: JSCompilerStateProto.Builder#addAllInjectedLibraries
    pub fn add_all_injected_libraries(mut self, values: impl IntoIterator<Item = String>) -> Self {
        self.injected_libraries.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearInjectedLibraries
    pub fn clear_injected_libraries(mut self) -> Self {
        self.injected_libraries.clear();
        self
    }
    // port: JSCompilerStateProto#getLastInjectedLibraryIndexInFirstScript
    pub fn get_last_injected_library_index_in_first_script(&self) -> i32 {
        self.last_injected_library_index_in_first_script
    }
    // port: JSCompilerStateProto.Builder#setLastInjectedLibraryIndexInFirstScript
    pub fn set_last_injected_library_index_in_first_script(mut self, value: i32) -> Self {
        self.last_injected_library_index_in_first_script = value;
        self
    }
    // port: JSCompilerStateProto#hasAccessorSummary
    pub fn has_accessor_summary(&self) -> bool {
        self.accessor_summary.is_some()
    }
    // port: JSCompilerStateProto#getAccessorSummary
    pub fn get_accessor_summary(&self) -> &AccessorSummaryProto {
        self.accessor_summary
            .as_ref()
            .unwrap_or_else(|| AccessorSummaryProto::default_instance_ref())
    }
    // port: JSCompilerStateProto.Builder#setAccessorSummary
    pub fn set_accessor_summary(mut self, value: AccessorSummaryProto) -> Self {
        self.accessor_summary = Some(value);
        self
    }
    // port: JSCompilerStateProto.Builder#clearAccessorSummary
    pub fn clear_accessor_summary(mut self) -> Self {
        self.accessor_summary = None;
        self
    }
    // port: JSCompilerStateProto#getHasStringMap
    pub fn get_has_string_map(&self) -> bool {
        self.has_string_map
    }
    // port: JSCompilerStateProto.Builder#setHasStringMap
    pub fn set_has_string_map(mut self, value: bool) -> Self {
        self.has_string_map = value;
        self
    }
    // port: JSCompilerStateProto#getStringMapList
    pub fn get_string_map_list(&self) -> &[VariableMapEntryProto] {
        &self.string_map
    }
    // port: JSCompilerStateProto#getStringMapCount
    pub fn get_string_map_count(&self) -> i32 {
        self.string_map.len() as i32
    }
    // port: JSCompilerStateProto#getStringMap(int)
    pub fn get_string_map(&self, index: i32) -> &VariableMapEntryProto {
        &self.string_map[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addStringMap
    pub fn add_string_map(mut self, value: VariableMapEntryProto) -> Self {
        self.string_map.push(value);
        self
    }
    // port: JSCompilerStateProto.Builder#addAllStringMap
    pub fn add_all_string_map(
        mut self,
        values: impl IntoIterator<Item = VariableMapEntryProto>,
    ) -> Self {
        self.string_map.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearStringMap
    pub fn clear_string_map(mut self) -> Self {
        self.string_map.clear();
        self
    }
    // port: JSCompilerStateProto#getHasInstrumentationMapping
    pub fn get_has_instrumentation_mapping(&self) -> bool {
        self.has_instrumentation_mapping
    }
    // port: JSCompilerStateProto.Builder#setHasInstrumentationMapping
    pub fn set_has_instrumentation_mapping(mut self, value: bool) -> Self {
        self.has_instrumentation_mapping = value;
        self
    }
    // port: JSCompilerStateProto#getInstrumentationMappingList
    pub fn get_instrumentation_mapping_list(&self) -> &[VariableMapEntryProto] {
        &self.instrumentation_mapping
    }
    // port: JSCompilerStateProto#getInstrumentationMappingCount
    pub fn get_instrumentation_mapping_count(&self) -> i32 {
        self.instrumentation_mapping.len() as i32
    }
    // port: JSCompilerStateProto#getInstrumentationMapping(int)
    pub fn get_instrumentation_mapping(&self, index: i32) -> &VariableMapEntryProto {
        &self.instrumentation_mapping[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addInstrumentationMapping
    pub fn add_instrumentation_mapping(mut self, value: VariableMapEntryProto) -> Self {
        self.instrumentation_mapping.push(value);
        self
    }
    // port: JSCompilerStateProto.Builder#addAllInstrumentationMapping
    pub fn add_all_instrumentation_mapping(
        mut self,
        values: impl IntoIterator<Item = VariableMapEntryProto>,
    ) -> Self {
        self.instrumentation_mapping.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearInstrumentationMapping
    pub fn clear_instrumentation_mapping(mut self) -> Self {
        self.instrumentation_mapping.clear();
        self
    }
    // port: JSCompilerStateProto#getConformanceViolationsList
    pub fn get_conformance_violations_list(&self) -> &[ConformanceViolationsProto] {
        &self.conformance_violations
    }
    // port: JSCompilerStateProto#getConformanceViolationsCount
    pub fn get_conformance_violations_count(&self) -> i32 {
        self.conformance_violations.len() as i32
    }
    // port: JSCompilerStateProto#getConformanceViolations(int)
    pub fn get_conformance_violations(&self, index: i32) -> &ConformanceViolationsProto {
        &self.conformance_violations[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addConformanceViolations
    pub fn add_conformance_violations(mut self, value: ConformanceViolationsProto) -> Self {
        self.conformance_violations.push(value);
        self
    }
    // port: JSCompilerStateProto.Builder#addAllConformanceViolations
    pub fn add_all_conformance_violations(
        mut self,
        values: impl IntoIterator<Item = ConformanceViolationsProto>,
    ) -> Self {
        self.conformance_violations.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearConformanceViolations
    pub fn clear_conformance_violations(mut self) -> Self {
        self.conformance_violations.clear();
        self
    }
    // port: JSCompilerStateProto#getUnattributedConformanceViolationsList
    pub fn get_unattributed_conformance_violations_list(&self) -> &[ViolationProto] {
        &self.unattributed_conformance_violations
    }
    // port: JSCompilerStateProto#getUnattributedConformanceViolationsCount
    pub fn get_unattributed_conformance_violations_count(&self) -> i32 {
        self.unattributed_conformance_violations.len() as i32
    }
    // port: JSCompilerStateProto#getUnattributedConformanceViolations(int)
    pub fn get_unattributed_conformance_violations(&self, index: i32) -> &ViolationProto {
        &self.unattributed_conformance_violations[index as usize]
    }
    // port: JSCompilerStateProto.Builder#addUnattributedConformanceViolations
    pub fn add_unattributed_conformance_violations(mut self, value: ViolationProto) -> Self {
        self.unattributed_conformance_violations.push(value);
        self
    }
    // port: JSCompilerStateProto.Builder#addAllUnattributedConformanceViolations
    pub fn add_all_unattributed_conformance_violations(
        mut self,
        values: impl IntoIterator<Item = ViolationProto>,
    ) -> Self {
        self.unattributed_conformance_violations.extend(values);
        self
    }
    // port: JSCompilerStateProto.Builder#clearUnattributedConformanceViolations
    pub fn clear_unattributed_conformance_violations(mut self) -> Self {
        self.unattributed_conformance_violations.clear();
        self
    }
}
impl Message for JSCompilerStateProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 | 10 => {
                    input.read_repeated(tag, &mut self.allowable_features, |input| {
                        Ok(FeatureProto::for_number_or_unrecognized(input.read_enum()?))
                    })?;
                }
                16 => {
                    self.type_checking_has_run = input.read_bool()?;
                }
                24 => {
                    self.has_reg_exp_global_references = input.read_bool()?;
                }
                32 => {
                    self.life_cycle_stage =
                        LifeCycleStageProto::for_number_or_unrecognized(input.read_enum()?);
                }
                40 => {
                    self.merged_precompiled_libraries = input.read_bool()?;
                }
                50 => {
                    let mut v = ChunkProto::default();
                    input.read_message(&mut v)?;
                    self.chunks.push(v);
                }
                56 => {
                    self.unique_name_id = input.read_int32()?;
                }
                66 => {
                    let mut v = UniqueIdProto::default();
                    input.read_message(&mut v)?;
                    self.unique_id_supplier.push(v);
                }
                74 => {
                    self.exported_names.push(input.read_string_require_utf8()?);
                }
                80 => {
                    self.has_css_names = input.read_bool()?;
                }
                90 => {
                    self.css_names.push(input.read_string_require_utf8()?);
                }
                98 => {
                    self.id_generator_map = Some(input.read_string_require_utf8()?);
                }
                104 => {
                    self.transpiled_files = input.read_bool()?;
                }
                112 => {
                    self.id_generator_current_id = input.read_int32()?;
                }
                120 => {
                    self.run_j2cl_passes = input.read_bool()?;
                }
                130 => {
                    self.externs.push(input.read_string_require_utf8()?);
                }
                138 => {
                    self.injected_libraries
                        .push(input.read_string_require_utf8()?);
                }
                144 => {
                    self.last_injected_library_index_in_first_script = input.read_int32()?;
                }
                154 => {
                    let mut v = self.accessor_summary.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.accessor_summary = Some(v);
                }
                160 => {
                    self.has_string_map = input.read_bool()?;
                }
                170 => {
                    let mut v = VariableMapEntryProto::default();
                    input.read_message(&mut v)?;
                    self.string_map.push(v);
                }
                176 => {
                    self.has_instrumentation_mapping = input.read_bool()?;
                }
                186 => {
                    let mut v = VariableMapEntryProto::default();
                    input.read_message(&mut v)?;
                    self.instrumentation_mapping.push(v);
                }
                194 => {
                    let mut v = ConformanceViolationsProto::default();
                    input.read_message(&mut v)?;
                    self.conformance_violations.push(v);
                }
                202 => {
                    let mut v = ViolationProto::default();
                    input.read_message(&mut v)?;
                    self.unattributed_conformance_violations.push(v);
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
        if !self.allowable_features.is_empty() {
            let data_size: usize = self
                .allowable_features
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(v.get_number()))
                .sum();
            output.write_tag(1, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.allowable_features {
                output.write_int32_no_tag(v.get_number());
            }
        }
        if self.type_checking_has_run {
            let v = &self.type_checking_has_run;
            output.write_tag(2, 0);
            output.write_bool_no_tag(*v);
        }
        if self.has_reg_exp_global_references {
            let v = &self.has_reg_exp_global_references;
            output.write_tag(3, 0);
            output.write_bool_no_tag(*v);
        }
        if self.life_cycle_stage.get_number() != 0 {
            let v = &self.life_cycle_stage;
            output.write_tag(4, 0);
            output.write_int32_no_tag(v.get_number());
        }
        if self.merged_precompiled_libraries {
            let v = &self.merged_precompiled_libraries;
            output.write_tag(5, 0);
            output.write_bool_no_tag(*v);
        }
        for v in &self.chunks {
            output.write_tag(6, 2);
            output.write_message_no_tag(v);
        }
        if self.unique_name_id != 0 {
            let v = &self.unique_name_id;
            output.write_tag(7, 0);
            output.write_int32_no_tag(*v);
        }
        for v in &self.unique_id_supplier {
            output.write_tag(8, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.exported_names {
            output.write_tag(9, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if self.has_css_names {
            let v = &self.has_css_names;
            output.write_tag(10, 0);
            output.write_bool_no_tag(*v);
        }
        for v in &self.css_names {
            output.write_tag(11, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if let Some(v) = &self.id_generator_map {
            output.write_tag(12, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if self.transpiled_files {
            let v = &self.transpiled_files;
            output.write_tag(13, 0);
            output.write_bool_no_tag(*v);
        }
        if self.id_generator_current_id != 0 {
            let v = &self.id_generator_current_id;
            output.write_tag(14, 0);
            output.write_int32_no_tag(*v);
        }
        if self.run_j2cl_passes {
            let v = &self.run_j2cl_passes;
            output.write_tag(15, 0);
            output.write_bool_no_tag(*v);
        }
        for v in &self.externs {
            output.write_tag(16, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        for v in &self.injected_libraries {
            output.write_tag(17, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if self.last_injected_library_index_in_first_script != 0 {
            let v = &self.last_injected_library_index_in_first_script;
            output.write_tag(18, 0);
            output.write_int32_no_tag(*v);
        }
        if let Some(v) = &self.accessor_summary {
            output.write_tag(19, 2);
            output.write_message_no_tag(v);
        }
        if self.has_string_map {
            let v = &self.has_string_map;
            output.write_tag(20, 0);
            output.write_bool_no_tag(*v);
        }
        for v in &self.string_map {
            output.write_tag(21, 2);
            output.write_message_no_tag(v);
        }
        if self.has_instrumentation_mapping {
            let v = &self.has_instrumentation_mapping;
            output.write_tag(22, 0);
            output.write_bool_no_tag(*v);
        }
        for v in &self.instrumentation_mapping {
            output.write_tag(23, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.conformance_violations {
            output.write_tag(24, 2);
            output.write_message_no_tag(v);
        }
        for v in &self.unattributed_conformance_violations {
            output.write_tag(25, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.allowable_features.is_empty() {
            let data_size: usize = self
                .allowable_features
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(v.get_number()))
                .sum();
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        if self.type_checking_has_run {
            let v = &self.type_checking_has_run;
            size += CodedOutputStream::compute_tag_size(2) + 1;
        }
        if self.has_reg_exp_global_references {
            let v = &self.has_reg_exp_global_references;
            size += CodedOutputStream::compute_tag_size(3) + 1;
        }
        if self.life_cycle_stage.get_number() != 0 {
            let v = &self.life_cycle_stage;
            size += CodedOutputStream::compute_tag_size(4)
                + CodedOutputStream::compute_int32_size_no_tag(v.get_number());
        }
        if self.merged_precompiled_libraries {
            let v = &self.merged_precompiled_libraries;
            size += CodedOutputStream::compute_tag_size(5) + 1;
        }
        for v in &self.chunks {
            size += CodedOutputStream::compute_tag_size(6)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if self.unique_name_id != 0 {
            let v = &self.unique_name_id;
            size += CodedOutputStream::compute_tag_size(7)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        for v in &self.unique_id_supplier {
            size += CodedOutputStream::compute_tag_size(8)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.exported_names {
            size += CodedOutputStream::compute_tag_size(9)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if self.has_css_names {
            let v = &self.has_css_names;
            size += CodedOutputStream::compute_tag_size(10) + 1;
        }
        for v in &self.css_names {
            size += CodedOutputStream::compute_tag_size(11)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if let Some(v) = &self.id_generator_map {
            size += CodedOutputStream::compute_tag_size(12)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if self.transpiled_files {
            let v = &self.transpiled_files;
            size += CodedOutputStream::compute_tag_size(13) + 1;
        }
        if self.id_generator_current_id != 0 {
            let v = &self.id_generator_current_id;
            size += CodedOutputStream::compute_tag_size(14)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.run_j2cl_passes {
            let v = &self.run_j2cl_passes;
            size += CodedOutputStream::compute_tag_size(15) + 1;
        }
        for v in &self.externs {
            size += CodedOutputStream::compute_tag_size(16)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        for v in &self.injected_libraries {
            size += CodedOutputStream::compute_tag_size(17)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if self.last_injected_library_index_in_first_script != 0 {
            let v = &self.last_injected_library_index_in_first_script;
            size += CodedOutputStream::compute_tag_size(18)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if let Some(v) = &self.accessor_summary {
            size += CodedOutputStream::compute_tag_size(19)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if self.has_string_map {
            let v = &self.has_string_map;
            size += CodedOutputStream::compute_tag_size(20) + 1;
        }
        for v in &self.string_map {
            size += CodedOutputStream::compute_tag_size(21)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if self.has_instrumentation_mapping {
            let v = &self.has_instrumentation_mapping;
            size += CodedOutputStream::compute_tag_size(22) + 1;
        }
        for v in &self.instrumentation_mapping {
            size += CodedOutputStream::compute_tag_size(23)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.conformance_violations {
            size += CodedOutputStream::compute_tag_size(24)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        for v in &self.unattributed_conformance_violations {
            size += CodedOutputStream::compute_tag_size(25)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: ChunkProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ChunkProto {
    pub name: String,
    pub dependencies: Vec<i32>,
    pub input_ids: Vec<String>,
}
static CHUNKPROTO_DEFAULT_INSTANCE: LazyLock<ChunkProto> = LazyLock::new(ChunkProto::default);
impl ChunkProto {
    pub const NAME_FIELD_NUMBER: i32 = 1;
    pub const DEPENDENCIES_FIELD_NUMBER: i32 = 2;
    pub const INPUT_IDS_FIELD_NUMBER: i32 = 3;
    // port: ChunkProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: ChunkProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &CHUNKPROTO_DEFAULT_INSTANCE
    }
    // port: ChunkProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: ChunkProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: ChunkProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: ChunkProto#getName
    pub fn get_name(&self) -> &str {
        &self.name
    }
    // port: ChunkProto.Builder#setName
    pub fn set_name(mut self, value: impl Into<String>) -> Self {
        self.name = value.into();
        self
    }
    // port: ChunkProto#getDependenciesList
    pub fn get_dependencies_list(&self) -> &[i32] {
        &self.dependencies
    }
    // port: ChunkProto#getDependenciesCount
    pub fn get_dependencies_count(&self) -> i32 {
        self.dependencies.len() as i32
    }
    // port: ChunkProto#getDependencies(int)
    pub fn get_dependencies(&self, index: i32) -> i32 {
        self.dependencies[index as usize]
    }
    // port: ChunkProto.Builder#addDependencies
    pub fn add_dependencies(mut self, value: i32) -> Self {
        self.dependencies.push(value);
        self
    }
    // port: ChunkProto.Builder#addAllDependencies
    pub fn add_all_dependencies(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.dependencies.extend(values);
        self
    }
    // port: ChunkProto.Builder#clearDependencies
    pub fn clear_dependencies(mut self) -> Self {
        self.dependencies.clear();
        self
    }
    // port: ChunkProto#getInputIdsList
    pub fn get_input_ids_list(&self) -> &[String] {
        &self.input_ids
    }
    // port: ChunkProto#getInputIdsCount
    pub fn get_input_ids_count(&self) -> i32 {
        self.input_ids.len() as i32
    }
    // port: ChunkProto#getInputIds(int)
    pub fn get_input_ids(&self, index: i32) -> &String {
        &self.input_ids[index as usize]
    }
    // port: ChunkProto.Builder#addInputIds
    pub fn add_input_ids(mut self, value: impl Into<String>) -> Self {
        self.input_ids.push(value.into());
        self
    }
    // port: ChunkProto.Builder#addAllInputIds
    pub fn add_all_input_ids(mut self, values: impl IntoIterator<Item = String>) -> Self {
        self.input_ids.extend(values);
        self
    }
    // port: ChunkProto.Builder#clearInputIds
    pub fn clear_input_ids(mut self) -> Self {
        self.input_ids.clear();
        self
    }
}
impl Message for ChunkProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.name = input.read_string_require_utf8()?;
                }
                16 | 18 => {
                    input.read_repeated(tag, &mut self.dependencies, |input| {
                        Ok(input.read_int32()?)
                    })?;
                }
                26 => {
                    self.input_ids.push(input.read_string_require_utf8()?);
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
        if !self.name.is_empty() {
            let v = &self.name;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if !self.dependencies.is_empty() {
            let data_size: usize = self
                .dependencies
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            output.write_tag(2, 2);
            output.write_uint32_no_tag(data_size as i32);
            for v in &self.dependencies {
                output.write_int32_no_tag(*v);
            }
        }
        for v in &self.input_ids {
            output.write_tag(3, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.name.is_empty() {
            let v = &self.name;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if !self.dependencies.is_empty() {
            let data_size: usize = self
                .dependencies
                .iter()
                .map(|v| CodedOutputStream::compute_int32_size_no_tag(*v))
                .sum();
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_uint32_size_no_tag(data_size as i32)
                + data_size;
        }
        for v in &self.input_ids {
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        size
    }
}
// port: UniqueIdProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UniqueIdProto {
    pub hash: i32,
    pub counter: i32,
}
static UNIQUEIDPROTO_DEFAULT_INSTANCE: LazyLock<UniqueIdProto> =
    LazyLock::new(UniqueIdProto::default);
impl UniqueIdProto {
    pub const HASH_FIELD_NUMBER: i32 = 1;
    pub const COUNTER_FIELD_NUMBER: i32 = 2;
    // port: UniqueIdProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: UniqueIdProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &UNIQUEIDPROTO_DEFAULT_INSTANCE
    }
    // port: UniqueIdProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: UniqueIdProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: UniqueIdProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: UniqueIdProto#getHash
    pub fn get_hash(&self) -> i32 {
        self.hash
    }
    // port: UniqueIdProto.Builder#setHash
    pub fn set_hash(mut self, value: i32) -> Self {
        self.hash = value;
        self
    }
    // port: UniqueIdProto#getCounter
    pub fn get_counter(&self) -> i32 {
        self.counter
    }
    // port: UniqueIdProto.Builder#setCounter
    pub fn set_counter(mut self, value: i32) -> Self {
        self.counter = value;
        self
    }
}
impl Message for UniqueIdProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 => {
                    self.hash = input.read_int32()?;
                }
                16 => {
                    self.counter = input.read_int32()?;
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
        if self.hash != 0 {
            let v = &self.hash;
            output.write_tag(1, 0);
            output.write_int32_no_tag(*v);
        }
        if self.counter != 0 {
            let v = &self.counter;
            output.write_tag(2, 0);
            output.write_int32_no_tag(*v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if self.hash != 0 {
            let v = &self.hash;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.counter != 0 {
            let v = &self.counter;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        size
    }
}
// port: AccessorSummaryEntryProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AccessorSummaryEntryProto {
    pub name: String,
    pub kind: PropertyAccessKindProto,
}
static ACCESSORSUMMARYENTRYPROTO_DEFAULT_INSTANCE: LazyLock<AccessorSummaryEntryProto> =
    LazyLock::new(AccessorSummaryEntryProto::default);
impl AccessorSummaryEntryProto {
    pub const NAME_FIELD_NUMBER: i32 = 1;
    pub const KIND_FIELD_NUMBER: i32 = 2;
    // port: AccessorSummaryEntryProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: AccessorSummaryEntryProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &ACCESSORSUMMARYENTRYPROTO_DEFAULT_INSTANCE
    }
    // port: AccessorSummaryEntryProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: AccessorSummaryEntryProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: AccessorSummaryEntryProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: AccessorSummaryEntryProto#getName
    pub fn get_name(&self) -> &str {
        &self.name
    }
    // port: AccessorSummaryEntryProto.Builder#setName
    pub fn set_name(mut self, value: impl Into<String>) -> Self {
        self.name = value.into();
        self
    }
    // port: AccessorSummaryEntryProto#getKind
    pub fn get_kind(&self) -> PropertyAccessKindProto {
        self.kind
    }
    // port: AccessorSummaryEntryProto.Builder#setKind
    pub fn set_kind(mut self, value: PropertyAccessKindProto) -> Self {
        self.kind = value;
        self
    }
    // port: AccessorSummaryEntryProto#getKindValue
    pub fn get_kind_value(&self) -> i32 {
        self.kind.get_number()
    }
}
impl Message for AccessorSummaryEntryProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.name = input.read_string_require_utf8()?;
                }
                16 => {
                    self.kind =
                        PropertyAccessKindProto::for_number_or_unrecognized(input.read_enum()?);
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
        if !self.name.is_empty() {
            let v = &self.name;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if self.kind.get_number() != 0 {
            let v = &self.kind;
            output.write_tag(2, 0);
            output.write_int32_no_tag(v.get_number());
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.name.is_empty() {
            let v = &self.name;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if self.kind.get_number() != 0 {
            let v = &self.kind;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_int32_size_no_tag(v.get_number());
        }
        size
    }
}
// port: AccessorSummaryProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AccessorSummaryProto {
    pub assume_always_getter_and_setter: bool,
    pub accessors: Vec<AccessorSummaryEntryProto>,
}
static ACCESSORSUMMARYPROTO_DEFAULT_INSTANCE: LazyLock<AccessorSummaryProto> =
    LazyLock::new(AccessorSummaryProto::default);
impl AccessorSummaryProto {
    pub const ASSUME_ALWAYS_GETTER_AND_SETTER_FIELD_NUMBER: i32 = 1;
    pub const ACCESSORS_FIELD_NUMBER: i32 = 2;
    // port: AccessorSummaryProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: AccessorSummaryProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &ACCESSORSUMMARYPROTO_DEFAULT_INSTANCE
    }
    // port: AccessorSummaryProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: AccessorSummaryProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: AccessorSummaryProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: AccessorSummaryProto#getAssumeAlwaysGetterAndSetter
    pub fn get_assume_always_getter_and_setter(&self) -> bool {
        self.assume_always_getter_and_setter
    }
    // port: AccessorSummaryProto.Builder#setAssumeAlwaysGetterAndSetter
    pub fn set_assume_always_getter_and_setter(mut self, value: bool) -> Self {
        self.assume_always_getter_and_setter = value;
        self
    }
    // port: AccessorSummaryProto#getAccessorsList
    pub fn get_accessors_list(&self) -> &[AccessorSummaryEntryProto] {
        &self.accessors
    }
    // port: AccessorSummaryProto#getAccessorsCount
    pub fn get_accessors_count(&self) -> i32 {
        self.accessors.len() as i32
    }
    // port: AccessorSummaryProto#getAccessors(int)
    pub fn get_accessors(&self, index: i32) -> &AccessorSummaryEntryProto {
        &self.accessors[index as usize]
    }
    // port: AccessorSummaryProto.Builder#addAccessors
    pub fn add_accessors(mut self, value: AccessorSummaryEntryProto) -> Self {
        self.accessors.push(value);
        self
    }
    // port: AccessorSummaryProto.Builder#addAllAccessors
    pub fn add_all_accessors(
        mut self,
        values: impl IntoIterator<Item = AccessorSummaryEntryProto>,
    ) -> Self {
        self.accessors.extend(values);
        self
    }
    // port: AccessorSummaryProto.Builder#clearAccessors
    pub fn clear_accessors(mut self) -> Self {
        self.accessors.clear();
        self
    }
}
impl Message for AccessorSummaryProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                8 => {
                    self.assume_always_getter_and_setter = input.read_bool()?;
                }
                18 => {
                    let mut v = AccessorSummaryEntryProto::default();
                    input.read_message(&mut v)?;
                    self.accessors.push(v);
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
        if self.assume_always_getter_and_setter {
            let v = &self.assume_always_getter_and_setter;
            output.write_tag(1, 0);
            output.write_bool_no_tag(*v);
        }
        for v in &self.accessors {
            output.write_tag(2, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if self.assume_always_getter_and_setter {
            let v = &self.assume_always_getter_and_setter;
            size += CodedOutputStream::compute_tag_size(1) + 1;
        }
        for v in &self.accessors {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: VariableMapEntryProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct VariableMapEntryProto {
    pub original_name: String,
    pub new_name: String,
}
static VARIABLEMAPENTRYPROTO_DEFAULT_INSTANCE: LazyLock<VariableMapEntryProto> =
    LazyLock::new(VariableMapEntryProto::default);
impl VariableMapEntryProto {
    pub const ORIGINAL_NAME_FIELD_NUMBER: i32 = 1;
    pub const NEW_NAME_FIELD_NUMBER: i32 = 2;
    // port: VariableMapEntryProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: VariableMapEntryProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &VARIABLEMAPENTRYPROTO_DEFAULT_INSTANCE
    }
    // port: VariableMapEntryProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: VariableMapEntryProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: VariableMapEntryProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: VariableMapEntryProto#getOriginalName
    pub fn get_original_name(&self) -> &str {
        &self.original_name
    }
    // port: VariableMapEntryProto.Builder#setOriginalName
    pub fn set_original_name(mut self, value: impl Into<String>) -> Self {
        self.original_name = value.into();
        self
    }
    // port: VariableMapEntryProto#getNewName
    pub fn get_new_name(&self) -> &str {
        &self.new_name
    }
    // port: VariableMapEntryProto.Builder#setNewName
    pub fn set_new_name(mut self, value: impl Into<String>) -> Self {
        self.new_name = value.into();
        self
    }
}
impl Message for VariableMapEntryProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.original_name = input.read_string_require_utf8()?;
                }
                18 => {
                    self.new_name = input.read_string_require_utf8()?;
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
        if !self.original_name.is_empty() {
            let v = &self.original_name;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if !self.new_name.is_empty() {
            let v = &self.new_name;
            output.write_tag(2, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.original_name.is_empty() {
            let v = &self.original_name;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if !self.new_name.is_empty() {
            let v = &self.new_name;
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        size
    }
}
// port: ConformanceViolationsProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ConformanceViolationsProto {
    pub input_name: String,
    pub violations: Vec<ViolationProto>,
}
static CONFORMANCEVIOLATIONSPROTO_DEFAULT_INSTANCE: LazyLock<ConformanceViolationsProto> =
    LazyLock::new(ConformanceViolationsProto::default);
impl ConformanceViolationsProto {
    pub const INPUT_NAME_FIELD_NUMBER: i32 = 1;
    pub const VIOLATIONS_FIELD_NUMBER: i32 = 2;
    // port: ConformanceViolationsProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: ConformanceViolationsProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &CONFORMANCEVIOLATIONSPROTO_DEFAULT_INSTANCE
    }
    // port: ConformanceViolationsProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: ConformanceViolationsProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: ConformanceViolationsProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: ConformanceViolationsProto#getInputName
    pub fn get_input_name(&self) -> &str {
        &self.input_name
    }
    // port: ConformanceViolationsProto.Builder#setInputName
    pub fn set_input_name(mut self, value: impl Into<String>) -> Self {
        self.input_name = value.into();
        self
    }
    // port: ConformanceViolationsProto#getViolationsList
    pub fn get_violations_list(&self) -> &[ViolationProto] {
        &self.violations
    }
    // port: ConformanceViolationsProto#getViolationsCount
    pub fn get_violations_count(&self) -> i32 {
        self.violations.len() as i32
    }
    // port: ConformanceViolationsProto#getViolations(int)
    pub fn get_violations(&self, index: i32) -> &ViolationProto {
        &self.violations[index as usize]
    }
    // port: ConformanceViolationsProto.Builder#addViolations
    pub fn add_violations(mut self, value: ViolationProto) -> Self {
        self.violations.push(value);
        self
    }
    // port: ConformanceViolationsProto.Builder#addAllViolations
    pub fn add_all_violations(mut self, values: impl IntoIterator<Item = ViolationProto>) -> Self {
        self.violations.extend(values);
        self
    }
    // port: ConformanceViolationsProto.Builder#clearViolations
    pub fn clear_violations(mut self) -> Self {
        self.violations.clear();
        self
    }
}
impl Message for ConformanceViolationsProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    self.input_name = input.read_string_require_utf8()?;
                }
                18 => {
                    let mut v = ViolationProto::default();
                    input.read_message(&mut v)?;
                    self.violations.push(v);
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
        if !self.input_name.is_empty() {
            let v = &self.input_name;
            output.write_tag(1, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        for v in &self.violations {
            output.write_tag(2, 2);
            output.write_message_no_tag(v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if !self.input_name.is_empty() {
            let v = &self.input_name;
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        for v in &self.violations {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        size
    }
}
// port: ViolationProto (proto message; also its Builder)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ViolationProto {
    pub requirement: Option<Requirement>,
    pub allowlist_entry: Option<RequirementScopeEntry>,
    pub source_name: Option<String>,
    pub line_number: i32,
    pub char_number: i32,
    pub should_report_during_library_deps_conformance_checker: bool,
}
static VIOLATIONPROTO_DEFAULT_INSTANCE: LazyLock<ViolationProto> =
    LazyLock::new(ViolationProto::default);
impl ViolationProto {
    pub const REQUIREMENT_FIELD_NUMBER: i32 = 1;
    pub const ALLOWLIST_ENTRY_FIELD_NUMBER: i32 = 2;
    pub const SOURCE_NAME_FIELD_NUMBER: i32 = 3;
    pub const LINE_NUMBER_FIELD_NUMBER: i32 = 4;
    pub const CHAR_NUMBER_FIELD_NUMBER: i32 = 5;
    pub const SHOULD_REPORT_DURING_LIBRARY_DEPS_CONFORMANCE_CHECKER_FIELD_NUMBER: i32 = 6;
    // port: ViolationProto#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: ViolationProto#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    /// Rust-only: the shared default instance that Java's getters return for unset messages.
    pub fn default_instance_ref() -> &'static Self {
        &VIOLATIONPROTO_DEFAULT_INSTANCE
    }
    // port: ViolationProto#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: ViolationProto.Builder#build
    pub fn build(self) -> Self {
        self
    }
    // port: ViolationProto.Builder#clear
    pub fn clear(&mut self) -> &mut Self {
        *self = Self::default();
        self
    }
    // port: ViolationProto#hasRequirement
    pub fn has_requirement(&self) -> bool {
        self.requirement.is_some()
    }
    // port: ViolationProto#getRequirement
    pub fn get_requirement(&self) -> &Requirement {
        self.requirement
            .as_ref()
            .unwrap_or_else(|| Requirement::default_instance_ref())
    }
    // port: ViolationProto.Builder#setRequirement
    pub fn set_requirement(mut self, value: Requirement) -> Self {
        self.requirement = Some(value);
        self
    }
    // port: ViolationProto.Builder#clearRequirement
    pub fn clear_requirement(mut self) -> Self {
        self.requirement = None;
        self
    }
    // port: ViolationProto#hasAllowlistEntry
    pub fn has_allowlist_entry(&self) -> bool {
        self.allowlist_entry.is_some()
    }
    // port: ViolationProto#getAllowlistEntry
    pub fn get_allowlist_entry(&self) -> &RequirementScopeEntry {
        self.allowlist_entry
            .as_ref()
            .unwrap_or_else(|| RequirementScopeEntry::default_instance_ref())
    }
    // port: ViolationProto.Builder#setAllowlistEntry
    pub fn set_allowlist_entry(mut self, value: RequirementScopeEntry) -> Self {
        self.allowlist_entry = Some(value);
        self
    }
    // port: ViolationProto.Builder#clearAllowlistEntry
    pub fn clear_allowlist_entry(mut self) -> Self {
        self.allowlist_entry = None;
        self
    }
    // port: ViolationProto#hasSourceName
    pub fn has_source_name(&self) -> bool {
        self.source_name.is_some()
    }
    // port: ViolationProto#getSourceName
    pub fn get_source_name(&self) -> &str {
        self.source_name.as_deref().unwrap_or("")
    }
    // port: ViolationProto.Builder#setSourceName
    pub fn set_source_name(mut self, value: impl Into<String>) -> Self {
        self.source_name = Some(value.into());
        self
    }
    // port: ViolationProto.Builder#clearSourceName
    pub fn clear_source_name(mut self) -> Self {
        self.source_name = None;
        self
    }
    // port: ViolationProto#getLineNumber
    pub fn get_line_number(&self) -> i32 {
        self.line_number
    }
    // port: ViolationProto.Builder#setLineNumber
    pub fn set_line_number(mut self, value: i32) -> Self {
        self.line_number = value;
        self
    }
    // port: ViolationProto#getCharNumber
    pub fn get_char_number(&self) -> i32 {
        self.char_number
    }
    // port: ViolationProto.Builder#setCharNumber
    pub fn set_char_number(mut self, value: i32) -> Self {
        self.char_number = value;
        self
    }
    // port: ViolationProto#getShouldReportDuringLibraryDepsConformanceChecker
    pub fn get_should_report_during_library_deps_conformance_checker(&self) -> bool {
        self.should_report_during_library_deps_conformance_checker
    }
    // port: ViolationProto.Builder#setShouldReportDuringLibraryDepsConformanceChecker
    pub fn set_should_report_during_library_deps_conformance_checker(
        mut self,
        value: bool,
    ) -> Self {
        self.should_report_during_library_deps_conformance_checker = value;
        self
    }
}
impl Message for ViolationProto {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        loop {
            let tag = input.read_tag()?;
            match tag {
                0 => return Ok(()),
                10 => {
                    let mut v = self.requirement.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.requirement = Some(v);
                }
                18 => {
                    let mut v = self.allowlist_entry.take().unwrap_or_default();
                    input.read_message(&mut v)?;
                    self.allowlist_entry = Some(v);
                }
                26 => {
                    self.source_name = Some(input.read_string_require_utf8()?);
                }
                32 => {
                    self.line_number = input.read_int32()?;
                }
                40 => {
                    self.char_number = input.read_int32()?;
                }
                48 => {
                    self.should_report_during_library_deps_conformance_checker =
                        input.read_bool()?;
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
        if let Some(v) = &self.requirement {
            output.write_tag(1, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.allowlist_entry {
            output.write_tag(2, 2);
            output.write_message_no_tag(v);
        }
        if let Some(v) = &self.source_name {
            output.write_tag(3, 2);
            output.write_byte_array_no_tag(v.as_bytes());
        }
        if self.line_number != 0 {
            let v = &self.line_number;
            output.write_tag(4, 0);
            output.write_int32_no_tag(*v);
        }
        if self.char_number != 0 {
            let v = &self.char_number;
            output.write_tag(5, 0);
            output.write_int32_no_tag(*v);
        }
        if self.should_report_during_library_deps_conformance_checker {
            let v = &self.should_report_during_library_deps_conformance_checker;
            output.write_tag(6, 0);
            output.write_bool_no_tag(*v);
        }
    }
    fn get_serialized_size(&self) -> usize {
        let mut size = 0usize;
        if let Some(v) = &self.requirement {
            size += CodedOutputStream::compute_tag_size(1)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.allowlist_entry {
            size += CodedOutputStream::compute_tag_size(2)
                + CodedOutputStream::compute_message_size_no_tag(v);
        }
        if let Some(v) = &self.source_name {
            size += CodedOutputStream::compute_tag_size(3)
                + CodedOutputStream::compute_byte_array_size_no_tag(v.as_bytes());
        }
        if self.line_number != 0 {
            let v = &self.line_number;
            size += CodedOutputStream::compute_tag_size(4)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.char_number != 0 {
            let v = &self.char_number;
            size += CodedOutputStream::compute_tag_size(5)
                + CodedOutputStream::compute_int32_size_no_tag(*v);
        }
        if self.should_report_during_library_deps_conformance_checker {
            let v = &self.should_report_during_library_deps_conformance_checker;
            size += CodedOutputStream::compute_tag_size(6) + 1;
        }
        size
    }
}
// port: Descriptors.FileDescriptor of compiler_state.proto (the message descriptors protoc embeds)
pub static MESSAGE_DESCRIPTORS: &[Descriptor] = &[
    Descriptor {
        full_name: "jscomp.JSCompilerStateProto",
        fields: &[
            FieldDescriptor {
                name: "allowable_features",
                number: 1,
                field_type: FieldType::ENUM,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.FeatureProto",
            },
            FieldDescriptor {
                name: "type_checking_has_run",
                number: 2,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "has_reg_exp_global_references",
                number: 3,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "life_cycle_stage",
                number: 4,
                field_type: FieldType::ENUM,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.LifeCycleStageProto",
            },
            FieldDescriptor {
                name: "merged_precompiled_libraries",
                number: 5,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "chunks",
                number: 6,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.ChunkProto",
            },
            FieldDescriptor {
                name: "unique_name_id",
                number: 7,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "unique_id_supplier",
                number: 8,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.UniqueIdProto",
            },
            FieldDescriptor {
                name: "exported_names",
                number: 9,
                field_type: FieldType::STRING,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "has_css_names",
                number: 10,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "css_names",
                number: 11,
                field_type: FieldType::STRING,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "id_generator_map",
                number: 12,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "transpiled_files",
                number: 13,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "id_generator_current_id",
                number: 14,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "run_j2cl_passes",
                number: 15,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "externs",
                number: 16,
                field_type: FieldType::STRING,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "injected_libraries",
                number: 17,
                field_type: FieldType::STRING,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "last_injected_library_index_in_first_script",
                number: 18,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "accessor_summary",
                number: 19,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.AccessorSummaryProto",
            },
            FieldDescriptor {
                name: "has_string_map",
                number: 20,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "string_map",
                number: 21,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.VariableMapEntryProto",
            },
            FieldDescriptor {
                name: "has_instrumentation_mapping",
                number: 22,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "instrumentation_mapping",
                number: 23,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.VariableMapEntryProto",
            },
            FieldDescriptor {
                name: "conformance_violations",
                number: 24,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.ConformanceViolationsProto",
            },
            FieldDescriptor {
                name: "unattributed_conformance_violations",
                number: 25,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.ViolationProto",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.ChunkProto",
        fields: &[
            FieldDescriptor {
                name: "name",
                number: 1,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "dependencies",
                number: 2,
                field_type: FieldType::INT32,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "input_ids",
                number: 3,
                field_type: FieldType::STRING,
                repeated: true,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.UniqueIdProto",
        fields: &[
            FieldDescriptor {
                name: "hash",
                number: 1,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "counter",
                number: 2,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.AccessorSummaryEntryProto",
        fields: &[
            FieldDescriptor {
                name: "name",
                number: 1,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "kind",
                number: 2,
                field_type: FieldType::ENUM,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.PropertyAccessKindProto",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.AccessorSummaryProto",
        fields: &[
            FieldDescriptor {
                name: "assume_always_getter_and_setter",
                number: 1,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "accessors",
                number: 2,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.AccessorSummaryEntryProto",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.VariableMapEntryProto",
        fields: &[
            FieldDescriptor {
                name: "original_name",
                number: 1,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "new_name",
                number: 2,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.ConformanceViolationsProto",
        fields: &[
            FieldDescriptor {
                name: "input_name",
                number: 1,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "violations",
                number: 2,
                field_type: FieldType::MESSAGE,
                repeated: true,
                containing_oneof: None,
                type_name: "jscomp.ViolationProto",
            },
        ],
    },
    Descriptor {
        full_name: "jscomp.ViolationProto",
        fields: &[
            FieldDescriptor {
                name: "requirement",
                number: 1,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.Requirement",
            },
            FieldDescriptor {
                name: "allowlist_entry",
                number: 2,
                field_type: FieldType::MESSAGE,
                repeated: false,
                containing_oneof: None,
                type_name: "jscomp.RequirementScopeEntry",
            },
            FieldDescriptor {
                name: "source_name",
                number: 3,
                field_type: FieldType::STRING,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "line_number",
                number: 4,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "char_number",
                number: 5,
                field_type: FieldType::INT32,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
            FieldDescriptor {
                name: "should_report_during_library_deps_conformance_checker",
                number: 6,
                field_type: FieldType::BOOL,
                repeated: false,
                containing_oneof: None,
                type_name: "",
            },
        ],
    },
];
// port: Descriptors.FileDescriptor of compiler_state.proto (the enum descriptors protoc embeds)
pub static ENUM_DESCRIPTORS: &[EnumDescriptor] = &[
    EnumDescriptor {
        full_name: "jscomp.FeatureProto",
        name: "FeatureProto",
        values: &[
            ("FEATURE_UNKNOWN", 0),
            ("FEATURE_REGEXP_SYNTAX", 1),
            ("FEATURE_ES3_KEYWORDS_AS_IDENTIFIERS", 2),
            ("FEATURE_GETTER", 3),
            ("FEATURE_KEYWORDS_AS_PROPERTIES", 4),
            ("FEATURE_SETTER", 5),
            ("FEATURE_STRING_CONTINUATION", 6),
            ("FEATURE_TRAILING_COMMA", 7),
            ("FEATURE_ARRAY_DESTRUCTURING", 8),
            ("FEATURE_ARRAY_PATTERN_REST", 9),
            ("FEATURE_ARROW_FUNCTIONS", 10),
            ("FEATURE_BINARY_LITERALS", 11),
            ("FEATURE_BLOCK_SCOPED_FUNCTION_DECLARATION", 12),
            ("FEATURE_CLASSES", 13),
            ("FEATURE_CLASS_GETTER_SETTER", 14),
            ("FEATURE_COMPUTED_PROPERTIES", 15),
            ("FEATURE_CONST_DECLARATIONS", 16),
            ("FEATURE_DEFAULT_PARAMETERS", 17),
            ("FEATURE_FOR_OF", 18),
            ("FEATURE_GENERATORS", 19),
            ("FEATURE_LET_DECLARATIONS", 20),
            ("FEATURE_MEMBER_DECLARATIONS", 21),
            ("FEATURE_NEW_TARGET", 22),
            ("FEATURE_OBJECT_DESTRUCTURING", 23),
            ("FEATURE_OCTAL_LITERALS", 24),
            ("FEATURE_REGEXP_FLAG_U", 25),
            ("FEATURE_REGEXP_FLAG_Y", 26),
            ("FEATURE_REST_PARAMETERS", 27),
            ("FEATURE_SHORTHAND_OBJECT_PROPERTIES", 28),
            ("FEATURE_SPREAD_EXPRESSIONS", 29),
            ("FEATURE_SUPER", 30),
            ("FEATURE_TEMPLATE_LITERALS", 31),
            ("FEATURE_MODULES", 32),
            ("FEATURE_EXPONENT_OP", 33),
            ("FEATURE_ASYNC_FUNCTIONS", 34),
            ("FEATURE_OBJECT_LITERALS_WITH_SPREAD", 35),
            ("FEATURE_OBJECT_PATTERN_REST", 36),
            ("FEATURE_ASYNC_GENERATORS", 37),
            ("FEATURE_FOR_AWAIT_OF", 38),
            ("FEATURE_REGEXP_FLAG_S", 39),
            ("FEATURE_REGEXP_NAMED_GROUPS", 40),
            ("FEATURE_REGEXP_UNICODE_PROPERTY_ESCAPE", 41),
            ("FEATURE_REGEXP_LOOKBEHIND", 42),
            ("FEATURE_UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP", 43),
            ("FEATURE_OPTIONAL_CATCH_BINDING", 44),
            ("FEATURE_DYNAMIC_IMPORT", 45),
            ("FEATURE_BIGINT", 46),
            ("FEATURE_IMPORT_META", 47),
            ("FEATURE_NULL_COALESCE_OP", 48),
            ("FEATURE_OPTIONAL_CHAINING", 49),
            ("FEATURE_NUMERIC_SEPARATOR", 50),
            ("FEATURE_LOGICAL_ASSIGNMENT", 51),
            ("FEATURE_PUBLIC_CLASS_FIELDS", 52),
            ("FEATURE_CLASS_STATIC_BLOCK", 53),
            ("FEATURE_REGEXP_FLAG_D", 54),
            ("FEATURE_TOP_LEVEL_AWAIT", 55),
            ("FEATURE_ES_NEXT_RUNTIME", 56),
            ("FEATURE_ES_UNSTABLE_RUNTIME", 57),
            ("FEATURE_PRIVATE_ELEMENTS", 58),
            ("FEATURE_TYPE_ANNOTATION", 59),
        ],
    },
    EnumDescriptor {
        full_name: "jscomp.LifeCycleStageProto",
        name: "LifeCycleStageProto",
        values: &[
            ("LIFE_CYCLE_STAGE_UNKNOWN", 0),
            ("LIFE_CYCLE_STAGE_RAW", 1),
            ("LIFE_CYCLE_STAGE_COLORS_AND_SIMPLIFIED_JSDOC", 2),
            ("LIFE_CYCLE_STAGE_NORMALIZED", 3),
            ("LIFE_CYCLE_STAGE_NORMALIZED_OBFUSCATED", 4),
        ],
    },
    EnumDescriptor {
        full_name: "jscomp.PropertyAccessKindProto",
        name: "PropertyAccessKindProto",
        values: &[
            ("KIND_UNKNOWN", 0),
            ("KIND_NORMAL", 1),
            ("KIND_GETTER_ONLY", 2),
            ("KIND_SETTER_ONLY", 3),
            ("KIND_GETTER_AND_SETTER", 4),
        ],
    },
];
