/*
 * Copyright (c) 2002, 2023, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Character.java.

//! JDK 21 `java.lang.Character` predicates on BMP code units (`char` overloads), table-driven from
//! the pinned JDK (see `character_bmp_data.rs`). Used where Java output depends on them, e.g.
//! `RegExpTree.isIdentifierStart` (`Character.isLetter`) and `Integer.parseInt` (`Character.digit`).

#[path = "character_bmp_data.rs"]
mod character_bmp_data;
#[path = "character_case_data.rs"]
mod character_case_data;

fn in_ranges(ranges: &[(u16, u16)], ch: u16) -> bool {
    let i = ranges.partition_point(|r| r.1 < ch);
    i < ranges.len() && ranges[i].0 <= ch
}

// port: Character#isLetter(char)
pub fn is_letter(ch: u16) -> bool {
    in_ranges(character_bmp_data::IS_LETTER, ch)
}

// port: Character#isDigit(char)
pub fn is_digit(ch: u16) -> bool {
    in_ranges(character_bmp_data::IS_DIGIT, ch)
}

// port: Character#isUpperCase(char)
pub fn is_upper_case(ch: u16) -> bool {
    character_case_data::IS_UPPER_CASE
        .binary_search(&ch)
        .is_ok()
}

// port: Character#toUpperCase(char)
pub fn to_upper_case(ch: u16) -> u16 {
    let table = character_case_data::TO_UPPER_CASE;
    match table.binary_search_by_key(&ch, |&(c, _)| c) {
        Ok(i) => table[i].1,
        Err(_) => ch,
    }
}

// port: Character#digit(char, int)
pub fn digit(ch: u16, radix: i32) -> i32 {
    if !(MIN_RADIX..=MAX_RADIX).contains(&radix) {
        return -1;
    }
    let ranges = character_bmp_data::DIGIT_36;
    let i = ranges.partition_point(|r| r.1 < ch);
    if i < ranges.len() && ranges[i].0 <= ch {
        let value = (ch - ranges[i].2) as i32;
        if value < radix {
            return value;
        }
    }
    -1
}

/// `Character.MIN_RADIX`.
pub const MIN_RADIX: i32 = 2;
/// `Character.MAX_RADIX`.
pub const MAX_RADIX: i32 = 36;
