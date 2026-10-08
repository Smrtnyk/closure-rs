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
//   src/com/google/javascript/jscomp/serialization/TypePointers.java.

//! Port of serialization/TypePointers.java: TypePointer utilities.
use closure_rhino::check_state;
use closure_rhino::jscomp_colors::color::Color;
use closure_rhino::jscomp_colors::standard_colors;
use std::sync::LazyLock;

/// port: TypePointers
pub struct TypePointers;

/// The first N TypePointer pool offsets correspond to axiomatic colors in this order.
///
/// These colors are never serialized because all their information is constant.
pub static OFFSET_TO_AXIOMATIC_COLOR: LazyLock<Vec<Color>> = LazyLock::new(|| {
    vec![
        standard_colors::UNKNOWN.clone(),
        standard_colors::BOOLEAN.clone(),
        standard_colors::STRING.clone(),
        standard_colors::NUMBER.clone(),
        standard_colors::NULL_OR_VOID.clone(),
        standard_colors::SYMBOL.clone(),
        standard_colors::BIGINT.clone(),
        standard_colors::TOP_OBJECT.clone(),
        standard_colors::TOP_FUNCTION.clone(),
        standard_colors::GBIGINT.clone(),
    ]
});

/// `OFFSET_TO_AXIOMATIC_COLOR.size()`
pub const AXIOMATIC_COLOR_COUNT: i32 = 10;

impl TypePointers {
    // port: TypePointers#trimOffset
    pub fn trim_offset(x: i32) -> i32 {
        check_state!(x >= AXIOMATIC_COLOR_COUNT, "%s", x);
        x - AXIOMATIC_COLOR_COUNT
    }

    // port: TypePointers#untrimOffset
    pub fn untrim_offset(x: i32) -> i32 {
        check_state!(x >= 0, "%s", x);
        x + AXIOMATIC_COLOR_COUNT
    }

    // port: TypePointers#isAxiomatic
    pub fn is_axiomatic(x: i32) -> bool {
        x < AXIOMATIC_COLOR_COUNT
    }
}
