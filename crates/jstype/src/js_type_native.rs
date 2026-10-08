/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Bob Jervis
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/jstype/JSTypeNative.java.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JSTypeNative {
    ARGUMENTS_TYPE,
    ARGUMENTS_FUNCTION_TYPE,
    ARRAY_TYPE,
    ARRAY_FUNCTION_TYPE,
    ASYNC_ITERABLE_FUNCTION_TYPE,
    ASYNC_ITERABLE_TYPE,
    ASYNC_ITERATOR_FUNCTION_TYPE,
    ASYNC_ITERATOR_TYPE,
    ASYNC_ITERATOR_ITERABLE_FUNCTION_TYPE,
    ASYNC_ITERATOR_ITERABLE_TYPE,
    ASYNC_GENERATOR_FUNCTION_TYPE,
    ASYNC_GENERATOR_TYPE,
    BIGINT_TYPE,
    BIGINT_OBJECT_TYPE,
    BIGINT_OBJECT_FUNCTION_TYPE,
    BOOLEAN_TYPE,
    BOOLEAN_OBJECT_TYPE,
    BOOLEAN_OBJECT_FUNCTION_TYPE,
    CHECKED_UNKNOWN_TYPE,
    DATE_TYPE,
    DATE_FUNCTION_TYPE,
    FUNCTION_TYPE,
    FUNCTION_FUNCTION_TYPE,
    FUNCTION_PROTOTYPE,
    FUNCTION_INSTANCE_PROTOTYPE,
    GBIGINT_TYPE,
    GENERATOR_FUNCTION_TYPE,
    GENERATOR_TYPE,
    I_ITERABLE_RESULT_FUNCTION_TYPE,
    I_ITERABLE_RESULT_TYPE,
    ITERABLE_FUNCTION_TYPE,
    ITERABLE_TYPE,
    ITERATOR_LIKE_FUNCTION_TYPE,
    ITERATOR_LIKE_TYPE,
    ITERATOR_FUNCTION_TYPE,
    ITERATOR_TYPE,
    ITERATOR_ITERABLE_FUNCTION_TYPE,
    ITERATOR_ITERABLE_TYPE,
    I_ARRAY_LIKE_FUNCTION_TYPE,
    I_ARRAY_LIKE_TYPE,
    I_TEMPLATE_ARRAY_TYPE,
    I_OBJECT_FUNCTION_TYPE,
    I_OBJECT_TYPE,
    I_THENABLE_FUNCTION_TYPE,
    I_THENABLE_TYPE,
    NULL_TYPE,
    NUMBER_TYPE,
    NUMBER_OBJECT_TYPE,
    NUMBER_OBJECT_FUNCTION_TYPE,
    PROMISE_TYPE,
    PROMISE_FUNCTION_TYPE,
    OBJECT_TYPE,
    OBJECT_FUNCTION_TYPE,
    OBJECT_PROTOTYPE,
    READONLY_ARRAY_TYPE,
    READONLY_ARRAY_FUNCTION_TYPE,
    READONLY_MAP_TYPE,
    READONLY_MAP_FUNCTION_TYPE,
    MAP_TYPE,
    MAP_FUNCTION_TYPE,
    REGEXP_TYPE,
    REGEXP_FUNCTION_TYPE,
    STRING_OBJECT_TYPE,
    STRING_OBJECT_FUNCTION_TYPE,
    STRING_TYPE,
    SYMBOL_OBJECT_TYPE,
    SYMBOL_OBJECT_FUNCTION_TYPE,
    SYMBOL_TYPE,
    THENABLE_TYPE,
    UNKNOWN_TYPE,
    VOID_TYPE,
    ALL_TYPE,
    NO_TYPE,
    NO_OBJECT_TYPE,
    GLOBAL_THIS,
    LEAST_FUNCTION_TYPE,
    GREATEST_FUNCTION_TYPE,
    NULL_VOID,
    NUMBER_STRING_BOOLEAN,
    VALUE_TYPES,
    NUMBER_SYMBOL,
    STRING_SYMBOL,
    NUMBER_STRING,
    NUMBER_STRING_SYMBOL,
    BIGINT_NUMBER,
    BIGINT_NUMBER_OBJECT,
    BIGINT_NUMBER_STRING,
    BIGINT_NUMBER_STRING_OBJECT,
    NUMBER_ADDITION_SUPERTYPE,
}
impl JSTypeNative {
    pub const COUNT: usize = 89;
    pub const VALUES: [Self; Self::COUNT] = [
        Self::ARGUMENTS_TYPE,
        Self::ARGUMENTS_FUNCTION_TYPE,
        Self::ARRAY_TYPE,
        Self::ARRAY_FUNCTION_TYPE,
        Self::ASYNC_ITERABLE_FUNCTION_TYPE,
        Self::ASYNC_ITERABLE_TYPE,
        Self::ASYNC_ITERATOR_FUNCTION_TYPE,
        Self::ASYNC_ITERATOR_TYPE,
        Self::ASYNC_ITERATOR_ITERABLE_FUNCTION_TYPE,
        Self::ASYNC_ITERATOR_ITERABLE_TYPE,
        Self::ASYNC_GENERATOR_FUNCTION_TYPE,
        Self::ASYNC_GENERATOR_TYPE,
        Self::BIGINT_TYPE,
        Self::BIGINT_OBJECT_TYPE,
        Self::BIGINT_OBJECT_FUNCTION_TYPE,
        Self::BOOLEAN_TYPE,
        Self::BOOLEAN_OBJECT_TYPE,
        Self::BOOLEAN_OBJECT_FUNCTION_TYPE,
        Self::CHECKED_UNKNOWN_TYPE,
        Self::DATE_TYPE,
        Self::DATE_FUNCTION_TYPE,
        Self::FUNCTION_TYPE,
        Self::FUNCTION_FUNCTION_TYPE,
        Self::FUNCTION_PROTOTYPE,
        Self::FUNCTION_INSTANCE_PROTOTYPE,
        Self::GBIGINT_TYPE,
        Self::GENERATOR_FUNCTION_TYPE,
        Self::GENERATOR_TYPE,
        Self::I_ITERABLE_RESULT_FUNCTION_TYPE,
        Self::I_ITERABLE_RESULT_TYPE,
        Self::ITERABLE_FUNCTION_TYPE,
        Self::ITERABLE_TYPE,
        Self::ITERATOR_LIKE_FUNCTION_TYPE,
        Self::ITERATOR_LIKE_TYPE,
        Self::ITERATOR_FUNCTION_TYPE,
        Self::ITERATOR_TYPE,
        Self::ITERATOR_ITERABLE_FUNCTION_TYPE,
        Self::ITERATOR_ITERABLE_TYPE,
        Self::I_ARRAY_LIKE_FUNCTION_TYPE,
        Self::I_ARRAY_LIKE_TYPE,
        Self::I_TEMPLATE_ARRAY_TYPE,
        Self::I_OBJECT_FUNCTION_TYPE,
        Self::I_OBJECT_TYPE,
        Self::I_THENABLE_FUNCTION_TYPE,
        Self::I_THENABLE_TYPE,
        Self::NULL_TYPE,
        Self::NUMBER_TYPE,
        Self::NUMBER_OBJECT_TYPE,
        Self::NUMBER_OBJECT_FUNCTION_TYPE,
        Self::PROMISE_TYPE,
        Self::PROMISE_FUNCTION_TYPE,
        Self::OBJECT_TYPE,
        Self::OBJECT_FUNCTION_TYPE,
        Self::OBJECT_PROTOTYPE,
        Self::READONLY_ARRAY_TYPE,
        Self::READONLY_ARRAY_FUNCTION_TYPE,
        Self::READONLY_MAP_TYPE,
        Self::READONLY_MAP_FUNCTION_TYPE,
        Self::MAP_TYPE,
        Self::MAP_FUNCTION_TYPE,
        Self::REGEXP_TYPE,
        Self::REGEXP_FUNCTION_TYPE,
        Self::STRING_OBJECT_TYPE,
        Self::STRING_OBJECT_FUNCTION_TYPE,
        Self::STRING_TYPE,
        Self::SYMBOL_OBJECT_TYPE,
        Self::SYMBOL_OBJECT_FUNCTION_TYPE,
        Self::SYMBOL_TYPE,
        Self::THENABLE_TYPE,
        Self::UNKNOWN_TYPE,
        Self::VOID_TYPE,
        Self::ALL_TYPE,
        Self::NO_TYPE,
        Self::NO_OBJECT_TYPE,
        Self::GLOBAL_THIS,
        Self::LEAST_FUNCTION_TYPE,
        Self::GREATEST_FUNCTION_TYPE,
        Self::NULL_VOID,
        Self::NUMBER_STRING_BOOLEAN,
        Self::VALUE_TYPES,
        Self::NUMBER_SYMBOL,
        Self::STRING_SYMBOL,
        Self::NUMBER_STRING,
        Self::NUMBER_STRING_SYMBOL,
        Self::BIGINT_NUMBER,
        Self::BIGINT_NUMBER_OBJECT,
        Self::BIGINT_NUMBER_STRING,
        Self::BIGINT_NUMBER_STRING_OBJECT,
        Self::NUMBER_ADDITION_SUPERTYPE,
    ];
}
