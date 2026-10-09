/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/colors/StandardColors.java.

use super::{Color, ColorId};
use crate::fast_hash::{IndexMap, IndexSet};
use std::sync::{
    LazyLock,
    atomic::{AtomicBool, Ordering},
};

pub(crate) static AXIOMATIC_COLORS_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub const ARGUMENTS_ID: ColorId = ColorId::from_unsigned_int(0x1939a66du32 as i32);
pub const ARRAY_ID: ColorId = ColorId::from_unsigned_int(0x79d4a603u32 as i32);
pub const READONLY_ARRAY_ID: ColorId = ColorId::from_unsigned_int(0x5627ffb0u32 as i32);
pub const GENERATOR_ID: ColorId = ColorId::from_unsigned_int(0x9bb1303fu32 as i32);
pub const I_TEMPLATE_ARRAY_ID: ColorId = ColorId::from_unsigned_int(0x46ab3f0eu32 as i32);
pub const ITERATOR_ID: ColorId = ColorId::from_unsigned_int(0x417ed2abu32 as i32);
pub const ASYNC_ITERATOR_ITERABLE_ID: ColorId = ColorId::from_unsigned_int(0xcb382e0au32 as i32);
pub const PROMISE_ID: ColorId = ColorId::from_unsigned_int(0x39581abfu32 as i32);
pub const BIGINT_OBJECT_ID: ColorId = ColorId::from_unsigned_int(0xa9d9ad6du32 as i32);
pub const BOOLEAN_OBJECT_ID: ColorId = ColorId::from_unsigned_int(0x9205dc06u32 as i32);
pub const NUMBER_OBJECT_ID: ColorId = ColorId::from_unsigned_int(0x34ba2fb1u32 as i32);
pub const STRING_OBJECT_ID: ColorId = ColorId::from_unsigned_int(0x186008a9u32 as i32);
pub const SYMBOL_OBJECT_ID: ColorId = ColorId::from_unsigned_int(0x5e514f7eu32 as i32);
// port: StandardColors#BIGINT
pub static BIGINT: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned_int(0x234eb61au32 as i32))
        .set_box_id(Some(BIGINT_OBJECT_ID))
        .set_invalidating(false)
        .build_axiomatic()
});

// port: StandardColors#BOOLEAN
pub static BOOLEAN: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned_int(0x126812eeu32 as i32))
        .set_box_id(Some(BOOLEAN_OBJECT_ID))
        .set_invalidating(false)
        .build_axiomatic()
});

// port: StandardColors#NULL_OR_VOID
pub static NULL_OR_VOID: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_box_id(None)
        .set_id(ColorId::from_unsigned_int(0x22b49f69u32 as i32))
        .set_invalidating(false)
        .build_axiomatic()
});

// port: StandardColors#NUMBER
pub static NUMBER: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned_int(0xd081722cu32 as i32))
        .set_box_id(Some(NUMBER_OBJECT_ID))
        .set_invalidating(false)
        .build_axiomatic()
});

// port: StandardColors#STRING
pub static STRING: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned_int(0x8c4d8f65u32 as i32))
        .set_box_id(Some(STRING_OBJECT_ID))
        .set_invalidating(false)
        .build_axiomatic()
});

// port: StandardColors#SYMBOL
pub static SYMBOL: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned_int(0x759f2066u32 as i32))
        .set_box_id(Some(SYMBOL_OBJECT_ID))
        .set_invalidating(false)
        .build_axiomatic()
});

// port: StandardColors#TOP_OBJECT
pub static TOP_OBJECT: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned_int(0x889b6838u32 as i32))
        .set_box_id(None)
        .set_invalidating(true)
        .build_axiomatic()
});

// port: StandardColors#TOP_FUNCTION
pub static TOP_FUNCTION: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned_int(0x283b5838u32 as i32))
        .set_box_id(None)
        .set_invalidating(true)
        .build_axiomatic()
});

// port: StandardColors#UNKNOWN
pub static UNKNOWN: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_box_id(None)
        .set_id(ColorId::from_unsigned_int(0)) // Make UNKNOWN the "default" numerical value.
        .set_invalidating(true)
        .build_axiomatic()
});

// port: StandardColors#GBIGINT
pub static GBIGINT: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned_int(0xaaae3678u32 as i32))
        .set_box_id(None)
        .set_invalidating(false)
        .build_axiomatic()
});

// port: StandardColors#AXIOMATIC_COLORS
pub static AXIOMATIC_COLORS: LazyLock<IndexMap<ColorId, Color>> = LazyLock::new(|| {
    let colors = [
        (BIGINT.get_id(), BIGINT.clone()),
        (BOOLEAN.get_id(), BOOLEAN.clone()),
        (NULL_OR_VOID.get_id(), NULL_OR_VOID.clone()),
        (NUMBER.get_id(), NUMBER.clone()),
        (STRING.get_id(), STRING.clone()),
        (SYMBOL.get_id(), SYMBOL.clone()),
        (TOP_OBJECT.get_id(), TOP_OBJECT.clone()),
        (UNKNOWN.get_id(), UNKNOWN.clone()),
        (GBIGINT.get_id(), GBIGINT.clone()),
        (TOP_FUNCTION.get_id(), TOP_FUNCTION.clone()),
    ]
    .into_iter()
    .collect();
    AXIOMATIC_COLORS_INITIALIZED.store(true, Ordering::Release);
    colors
});

// port: StandardColors#PRIMITIVE_COLORS
pub static PRIMITIVE_COLORS: LazyLock<IndexMap<ColorId, Color>> = LazyLock::new(|| {
    [
        (BIGINT.get_id(), BIGINT.clone()),
        (BOOLEAN.get_id(), BOOLEAN.clone()),
        (NULL_OR_VOID.get_id(), NULL_OR_VOID.clone()),
        (NUMBER.get_id(), NUMBER.clone()),
        (STRING.get_id(), STRING.clone()),
        (SYMBOL.get_id(), SYMBOL.clone()),
        (GBIGINT.get_id(), GBIGINT.clone()),
    ]
    .into_iter()
    .collect()
});

// port: StandardColors#PRIMITIVE_BOX_IDS
pub static PRIMITIVE_BOX_IDS: LazyLock<IndexSet<ColorId>> = LazyLock::new(|| {
    [
        BIGINT_OBJECT_ID,
        BOOLEAN_OBJECT_ID,
        NUMBER_OBJECT_ID,
        STRING_OBJECT_ID,
        SYMBOL_OBJECT_ID,
    ]
    .into_iter()
    .collect()
});

// port: StandardColors#STANDARD_OBJECT_IDS
pub(crate) static STANDARD_OBJECT_IDS: LazyLock<IndexSet<ColorId>> = LazyLock::new(|| {
    let mut ids = PRIMITIVE_BOX_IDS.clone();
    ids.extend([
        ARRAY_ID,
        READONLY_ARRAY_ID,
        ARGUMENTS_ID,
        ASYNC_ITERATOR_ITERABLE_ID,
        GENERATOR_ID,
        I_TEMPLATE_ARRAY_ID,
        ITERATOR_ID,
        PROMISE_ID,
    ]);
    ids
});
