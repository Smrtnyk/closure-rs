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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/typed_ast/typed_ast.proto.

use std::fmt;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum NodeProperty {
    NODE_PROPERTY_UNSPECIFIED = 0,
    IS_PARENTHESIZED = 1,
    SYNTHETIC = 3,
    ADDED_BLOCK = 4,
    IS_CONSTANT_NAME = 6,
    IS_NAMESPACE = 7,
    DIRECT_EVAL = 9,
    FREE_CALL = 10,
    UNUSED_11 = 11,
    REFLECTED_OBJECT = 12,
    STATIC_MEMBER = 13,
    GENERATOR_FN = 14,
    ARROW_FN = 15,
    ASYNC_FN = 16,
    YIELD_ALL = 17,
    EXPORT_DEFAULT = 18,
    EXPORT_ALL_FROM = 19,
    CONSTANT_VAR_FLAGS = 20,
    IS_GENERATOR_MARKER = 21,
    IS_GENERATOR_SAFE = 22,
    COMPUTED_PROP_METHOD = 23,
    COMPUTED_PROP_GETTER = 24,
    COMPUTED_PROP_SETTER = 25,
    COMPUTED_PROP_VARIABLE = 26,
    COLOR_FROM_CAST = 28,
    NON_INDEXABLE = 29,
    GOOG_MODULE = 30,
    DELETED = 35,
    MODULE_ALIAS = 36,
    IS_UNUSED_PARAMETER = 37,
    MODULE_EXPORT = 38,
    IS_SHORTHAND_PROPERTY = 39,
    ES6_MODULE = 40,
    START_OF_OPT_CHAIN = 41,
    TRAILING_COMMA = 42,
    IS_INFERRED_CONSTANT = 43,
    IS_DECLARED_CONSTANT = 44,
    SYNTHESIZED_UNFULFILLED_NAME_DECLARATION = 45,
    MUTATES_GLOBAL_STATE = 46,
    MUTATES_THIS = 47,
    MUTATES_ARGUMENTS = 48,
    THROWS = 49,
    CLOSURE_UNAWARE_SHADOW = 50,
    PRIVATE_IDENTIFIER = 51,
    UNRECOGNIZED = -1,
}
impl NodeProperty {
    pub const VALUES: [Self; 45] = [
        Self::NODE_PROPERTY_UNSPECIFIED,
        Self::IS_PARENTHESIZED,
        Self::SYNTHETIC,
        Self::ADDED_BLOCK,
        Self::IS_CONSTANT_NAME,
        Self::IS_NAMESPACE,
        Self::DIRECT_EVAL,
        Self::FREE_CALL,
        Self::UNUSED_11,
        Self::REFLECTED_OBJECT,
        Self::STATIC_MEMBER,
        Self::GENERATOR_FN,
        Self::ARROW_FN,
        Self::ASYNC_FN,
        Self::YIELD_ALL,
        Self::EXPORT_DEFAULT,
        Self::EXPORT_ALL_FROM,
        Self::CONSTANT_VAR_FLAGS,
        Self::IS_GENERATOR_MARKER,
        Self::IS_GENERATOR_SAFE,
        Self::COMPUTED_PROP_METHOD,
        Self::COMPUTED_PROP_GETTER,
        Self::COMPUTED_PROP_SETTER,
        Self::COMPUTED_PROP_VARIABLE,
        Self::COLOR_FROM_CAST,
        Self::NON_INDEXABLE,
        Self::GOOG_MODULE,
        Self::DELETED,
        Self::MODULE_ALIAS,
        Self::IS_UNUSED_PARAMETER,
        Self::MODULE_EXPORT,
        Self::IS_SHORTHAND_PROPERTY,
        Self::ES6_MODULE,
        Self::START_OF_OPT_CHAIN,
        Self::TRAILING_COMMA,
        Self::IS_INFERRED_CONSTANT,
        Self::IS_DECLARED_CONSTANT,
        Self::SYNTHESIZED_UNFULFILLED_NAME_DECLARATION,
        Self::MUTATES_GLOBAL_STATE,
        Self::MUTATES_THIS,
        Self::MUTATES_ARGUMENTS,
        Self::THROWS,
        Self::CLOSURE_UNAWARE_SHADOW,
        Self::PRIVATE_IDENTIFIER,
        Self::UNRECOGNIZED,
    ];
    // port: Enum#ordinal
    pub fn ordinal(self) -> usize {
        Self::VALUES
            .iter()
            .position(|value| *value == self)
            .unwrap()
    }
    // port: NodeProperty#getNumber
    pub fn get_number(self) -> i32 {
        assert!(
            self != Self::UNRECOGNIZED,
            "Can't get the number of an unknown enum value."
        );
        self as i32
    }
    // port: NodeProperty#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::NODE_PROPERTY_UNSPECIFIED),
            1 => Some(Self::IS_PARENTHESIZED),
            3 => Some(Self::SYNTHETIC),
            4 => Some(Self::ADDED_BLOCK),
            6 => Some(Self::IS_CONSTANT_NAME),
            7 => Some(Self::IS_NAMESPACE),
            9 => Some(Self::DIRECT_EVAL),
            10 => Some(Self::FREE_CALL),
            11 => Some(Self::UNUSED_11),
            12 => Some(Self::REFLECTED_OBJECT),
            13 => Some(Self::STATIC_MEMBER),
            14 => Some(Self::GENERATOR_FN),
            15 => Some(Self::ARROW_FN),
            16 => Some(Self::ASYNC_FN),
            17 => Some(Self::YIELD_ALL),
            18 => Some(Self::EXPORT_DEFAULT),
            19 => Some(Self::EXPORT_ALL_FROM),
            20 => Some(Self::CONSTANT_VAR_FLAGS),
            21 => Some(Self::IS_GENERATOR_MARKER),
            22 => Some(Self::IS_GENERATOR_SAFE),
            23 => Some(Self::COMPUTED_PROP_METHOD),
            24 => Some(Self::COMPUTED_PROP_GETTER),
            25 => Some(Self::COMPUTED_PROP_SETTER),
            26 => Some(Self::COMPUTED_PROP_VARIABLE),
            28 => Some(Self::COLOR_FROM_CAST),
            29 => Some(Self::NON_INDEXABLE),
            30 => Some(Self::GOOG_MODULE),
            35 => Some(Self::DELETED),
            36 => Some(Self::MODULE_ALIAS),
            37 => Some(Self::IS_UNUSED_PARAMETER),
            38 => Some(Self::MODULE_EXPORT),
            39 => Some(Self::IS_SHORTHAND_PROPERTY),
            40 => Some(Self::ES6_MODULE),
            41 => Some(Self::START_OF_OPT_CHAIN),
            42 => Some(Self::TRAILING_COMMA),
            43 => Some(Self::IS_INFERRED_CONSTANT),
            44 => Some(Self::IS_DECLARED_CONSTANT),
            45 => Some(Self::SYNTHESIZED_UNFULFILLED_NAME_DECLARATION),
            46 => Some(Self::MUTATES_GLOBAL_STATE),
            47 => Some(Self::MUTATES_THIS),
            48 => Some(Self::MUTATES_ARGUMENTS),
            49 => Some(Self::THROWS),
            50 => Some(Self::CLOSURE_UNAWARE_SHADOW),
            51 => Some(Self::PRIVATE_IDENTIFIER),
            _ => None,
        }
    }
}
impl fmt::Display for NodeProperty {
    // port: Enum#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
