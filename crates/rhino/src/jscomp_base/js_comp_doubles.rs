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
//   src/com/google/javascript/jscomp/base/JSCompDoubles.java,
//   test/com/google/javascript/jscomp/base/JSCompDoublesTest.java.

pub struct JSCompDoubles;
impl JSCompDoubles {
    // port: JSCompDoubles#isExactInt32
    pub fn is_exact_int32(x: f64) -> bool {
        !x.is_nan() && f64::from(x as i32) == x
    }
    // port: JSCompDoubles#isExactInt64
    pub fn is_exact_int64(x: f64) -> bool {
        !x.is_nan() && (x as i64) as f64 == x
    }
    // port: JSCompDoubles#isMathematicalInteger
    pub fn is_mathematical_integer(x: f64) -> bool {
        x % 1.0 == 0.0
    }
    // port: JSCompDoubles#isAtLeastIntegerPrecision
    pub fn is_at_least_integer_precision(x: f64) -> bool {
        x.abs() < Self::POW_2_53
    }
    // port: JSCompDoubles#isNegative
    pub fn is_negative(x: f64) -> bool {
        crate::check_state!(!x.is_nan());
        x.is_sign_negative()
    }
    // port: JSCompDoubles#isPositive
    pub fn is_positive(x: f64) -> bool {
        !Self::is_negative(x)
    }
    // port: JSCompDoubles#isEitherZero
    pub fn is_either_zero(x: f64) -> bool {
        x == 0.0
    }
    // port: JSCompDoubles#ecmascriptToInt32
    pub fn ecmascript_to_int32(number: f64) -> i32 {
        if Self::is_exact_int32(number) {
            return number as i32;
        }
        if number.is_nan() || number.is_infinite() {
            return 0;
        }
        let pos_int = if number >= 0.0 {
            number.floor()
        } else {
            number.ceil()
        };
        let int32bit = (pos_int % Self::POW_2_32) as i64;
        int32bit as i32
    }
    // port: JSCompDoubles#ecmascriptToUint32
    pub fn ecmascript_to_uint32(number: f64) -> i32 {
        Self::ecmascript_to_int32(number)
    }
    pub const POW_2_32: f64 = 4294967296.0;
    pub const POW_2_53: f64 = 9007199254740992.0;
}

#[cfg(test)]
mod tests {
    use super::JSCompDoubles;
    // port: JSCompDoublesTest#testIsExactInt32
    #[test]
    fn test_is_exact_int32() {
        for value in [
            0.0,
            -0.0,
            1.0,
            -1.0,
            3.1e5,
            -3.1e5,
            i32::MIN as f64,
            i32::MAX as f64,
        ] {
            assert!(JSCompDoubles::is_exact_int32(value));
        }
        for value in [
            3.1e-5,
            -3.1e-5,
            3.1e12,
            -3.1e12,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            i32::MIN as f64 - 1.0,
            i32::MAX as f64 + 1.0,
        ] {
            assert!(!JSCompDoubles::is_exact_int32(value));
        }
    }
}
