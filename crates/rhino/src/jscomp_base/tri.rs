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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/base/Tri.java.

use std::fmt;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Tri {
    FALSE = -1,
    UNKNOWN = 0,
    TRUE = 1,
}
impl Tri {
    // port: Tri#forInt
    fn for_int(x: i32) -> Self {
        [Self::FALSE, Self::UNKNOWN, Self::TRUE][(x + 1) as usize]
    }
    // port: Tri#or
    pub fn or(self, x: Self) -> Self {
        if self as i32 > x as i32 { self } else { x }
    }
    // port: Tri#and
    pub fn and(self, x: Self) -> Self {
        if (self as i32) < x as i32 { self } else { x }
    }
    // port: Tri#xor
    pub fn xor(self, x: Self) -> Self {
        Self::for_int(-(self as i32) * (x as i32))
    }
    // port: Tri#not
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Self {
        Self::for_int(-(self as i32))
    }
    // port: Tri#toBoolean
    pub fn to_boolean(self, x: bool) -> bool {
        match self {
            Self::FALSE => false,
            Self::UNKNOWN => x,
            Self::TRUE => true,
        }
    }
    // port: Tri#forBoolean
    pub fn for_boolean(x: bool) -> Self {
        if x { Self::TRUE } else { Self::FALSE }
    }
}
impl fmt::Display for Tri {
    // port: Tri#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FALSE => "false",
            Self::UNKNOWN => "unknown",
            Self::TRUE => "true",
        })
    }
}
