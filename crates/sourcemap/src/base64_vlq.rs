/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/debugging/sourcemap/Base64VLQ.java.

use crate::base64::Base64;
pub trait CharIterator {
    // port: Base64VLQ.CharIterator#hasNext
    fn has_next(&self) -> bool;
    // port: Base64VLQ.CharIterator#next
    fn next(&mut self) -> u16;
}
pub struct Base64VLQ;
impl Base64VLQ {
    const VLQ_BASE_SHIFT: u32 = 5;
    const VLQ_BASE: i32 = 1 << Self::VLQ_BASE_SHIFT;
    const VLQ_BASE_MASK: i32 = Self::VLQ_BASE - 1;
    const VLQ_CONTINUATION_BIT: i32 = Self::VLQ_BASE;

    // port: Base64VLQ#Base64VLQ
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }

    // port: Base64VLQ#toVLQSigned
    fn to_vlq_signed(value: i32) -> i32 {
        if value < 0 {
            value.wrapping_neg().wrapping_shl(1).wrapping_add(1)
        } else {
            value.wrapping_shl(1).wrapping_add(0)
        }
    }
    // port: Base64VLQ#fromVLQSigned
    fn from_vlq_signed(value: i32) -> i32 {
        let negate = (value & 1) == 1;
        let value = ((value as u32) >> 1) as i32;
        if !negate {
            return value;
        }
        value.wrapping_neg() | i32::MIN
    }
    // port: Base64VLQ#encode
    pub fn encode(out: &mut dyn std::fmt::Write, value: i32) -> std::fmt::Result {
        let mut value = Self::to_vlq_signed(value);
        loop {
            let mut digit = value & Self::VLQ_BASE_MASK;
            value = ((value as u32) >> Self::VLQ_BASE_SHIFT) as i32;
            if value > 0 {
                digit |= Self::VLQ_CONTINUATION_BIT;
            }
            out.write_char(char::from_u32(Base64::to_base64(digit) as u32).unwrap())?;
            if value <= 0 {
                break;
            }
        }
        Ok(())
    }
    // port: Base64VLQ#decode
    pub fn decode(input: &mut dyn CharIterator) -> i32 {
        let mut result = 0i32;
        let mut shift = 0u32;
        loop {
            let c = input.next();
            let mut digit = Base64::from_base64(c);
            let continuation = (digit & Self::VLQ_CONTINUATION_BIT) != 0;
            digit &= Self::VLQ_BASE_MASK;
            result = result.wrapping_add(digit.wrapping_shl(shift));
            shift = shift.wrapping_add(Self::VLQ_BASE_SHIFT);
            if !continuation {
                break;
            }
        }
        Self::from_vlq_signed(result)
    }
}
