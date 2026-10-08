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
//   test/com/google/javascript/jscomp/base/JSCompDoublesTest.java.

use closure_rhino::jscomp_base::JSCompDoubles as D;
const POW_2_31: f64 = 2147483648.0;
const POW_2_32: f64 = 4294967296.0;
const POW_2_52: f64 = 4503599627370496.0;
const POW_2_53: f64 = 9007199254740992.0;
const POW_2_64: f64 = 1.8446744073709552e+19;
const POW_2_200: f64 = 1.6069380442589903e+60;
// port: JSCompDoublesTest#testIsExactInt32
#[test]
fn test_is_exact_int32() {
    assert!(D::is_exact_int32(0.0));
    assert!(D::is_exact_int32(-0.0));
    assert!(D::is_exact_int32(1.0));
    assert!(D::is_exact_int32(-1.0));
    assert!(D::is_exact_int32(3.1e5));
    assert!(D::is_exact_int32(-3.1e5));
    assert!(D::is_exact_int32(i32::MIN as f64));
    assert!(D::is_exact_int32(i32::MAX as f64));

    assert!(!D::is_exact_int32(3.1e-5));
    assert!(!D::is_exact_int32(-3.1e-5));
    assert!(!D::is_exact_int32(3.1e12));
    assert!(!D::is_exact_int32(-3.1e12));
    assert!(!D::is_exact_int32(f64::INFINITY));
    assert!(!D::is_exact_int32(f64::NEG_INFINITY));
    assert!(!D::is_exact_int32(f64::NAN));
    assert!(!D::is_exact_int32((i32::MIN as f64) - 1.0));
    assert!(!D::is_exact_int32((i32::MAX as f64) + 1.0));
}
// port: JSCompDoublesTest#testIsExactInt64
#[test]
fn test_is_exact_int64() {
    assert!(D::is_exact_int64(0.0));
    assert!(D::is_exact_int64(-0.0));
    assert!(D::is_exact_int64(1.0));
    assert!(D::is_exact_int64(-1.0));
    assert!(D::is_exact_int64(3.1e5));
    assert!(D::is_exact_int64(-3.1e5));
    assert!(D::is_exact_int64(3.1e12));
    assert!(D::is_exact_int64(-3.1e12));
    assert!(D::is_exact_int64(i64::MIN as f64));
    assert!(D::is_exact_int64(i64::MAX as f64));

    assert!(!D::is_exact_int64(3.1e-5));
    assert!(!D::is_exact_int64(-3.1e-5));
    assert!(!D::is_exact_int64(3.1e24));
    assert!(!D::is_exact_int64(-3.1e24));
    assert!(!D::is_exact_int64(f64::INFINITY));
    assert!(!D::is_exact_int64(f64::NEG_INFINITY));
    assert!(!D::is_exact_int64(f64::NAN));
    assert!(!D::is_exact_int64(POW_2_200));
    assert!(!D::is_exact_int64(-POW_2_200));
}
// port: JSCompDoublesTest#testIsAtLeastIntegerPrecision
#[test]
fn test_is_at_least_integer_precision() {
    assert!(!D::is_at_least_integer_precision(f64::INFINITY));
    assert!(!D::is_at_least_integer_precision(f64::NEG_INFINITY));
    assert!(!D::is_at_least_integer_precision(f64::NAN));
    assert!(!D::is_at_least_integer_precision(POW_2_53));
    assert!(!D::is_at_least_integer_precision(-POW_2_53));

    assert!(D::is_at_least_integer_precision(0.0));
    assert!(D::is_at_least_integer_precision(-0.0));
    assert!(D::is_at_least_integer_precision(1.0));
    assert!(D::is_at_least_integer_precision(-1.0));
    assert!(D::is_at_least_integer_precision(3.1e5));
    assert!(D::is_at_least_integer_precision(-3.1e5));
    assert!(D::is_at_least_integer_precision(POW_2_53 - 1.0));
    assert!(D::is_at_least_integer_precision(-(POW_2_53 - 1.0)));
    assert!(D::is_at_least_integer_precision(3.1e-5));
    assert!(D::is_at_least_integer_precision(-3.1e-5));
}
// port: JSCompDoublesTest#testIsMathematicalInteger
#[test]
fn test_is_mathematical_integer() {
    assert!(!D::is_mathematical_integer(0.1));
    assert!(!D::is_mathematical_integer(3.1e-5));
    assert!(!D::is_mathematical_integer(f64::INFINITY));
    assert!(!D::is_mathematical_integer(f64::NAN));

    assert_is_mathematical_integer_with_decimal_precision(0.0);
    assert_is_mathematical_integer_with_decimal_precision(1.0);
    assert_is_mathematical_integer_with_decimal_precision(3.1e5);
    assert_is_mathematical_integer_with_decimal_precision(3.1e12);
    assert_is_mathematical_integer_with_decimal_precision(i32::MIN as f64);
    assert_is_mathematical_integer_with_decimal_precision(i32::MAX as f64);
    assert_is_mathematical_integer_with_decimal_precision((i32::MIN as f64) - 1.0);
    assert_is_mathematical_integer_with_decimal_precision((i32::MAX as f64) + 1.0);

    assert_is_mathematical_integer_without_decimal_precision(POW_2_53 - 1.0);
    assert_is_mathematical_integer_without_decimal_precision(POW_2_53);
    assert_is_mathematical_integer_without_decimal_precision(POW_2_53 + 2.0);
    assert_is_mathematical_integer_without_decimal_precision(POW_2_64);
    assert_is_mathematical_integer_without_decimal_precision(POW_2_200);
}
// port: JSCompDoublesTest#testIsNegative
#[test]
fn test_is_negative() {
    assert!(D::is_negative(-0.0));
    assert!(D::is_negative(-1.0));
    assert!(D::is_negative(-3.1e-5));
    assert!(D::is_negative(-3.1e5));
    assert!(D::is_negative(f64::NEG_INFINITY));
    assert!(D::is_negative(i32::MIN as f64));

    assert!(!D::is_negative(0.0));
    assert!(!D::is_negative(1.0));
    assert!(!D::is_negative(3.1e-5));
    assert!(!D::is_negative(3.1e12));
    assert!(!D::is_negative(3.1e5));
    assert!(!D::is_negative(f64::INFINITY));
    assert!(!D::is_negative(i32::MAX as f64));

    assert!(std::panic::catch_unwind(|| D::is_negative(f64::NAN)).is_err());
}
// port: JSCompDoublesTest#testEcmascriptToInt32
#[test]
fn test_ecmascript_to_int32() {
    // Step 1 special cases
    assert_eq!(D::ecmascript_to_int32(0.0), 0);
    assert_eq!(D::ecmascript_to_int32(-0.0), 0);
    assert_eq!(D::ecmascript_to_int32(f64::NAN), 0);
    assert_eq!(D::ecmascript_to_int32(f64::INFINITY), 0);
    assert_eq!(D::ecmascript_to_int32(f64::NEG_INFINITY), 0);

    assert_ecmascript_to_int32_equals(0.3, 0);
    assert_ecmascript_to_int32_equals(0.0, 0);
    assert_ecmascript_to_int32_equals(1.0, 1);
    assert_ecmascript_to_int32_equals(2.0, 2);
    assert_ecmascript_to_int32_equals(726832.0, 726832);
    assert_ecmascript_to_int32_equals(POW_2_31, i32::MIN);
    assert_ecmascript_to_int32_equals(POW_2_31 + 1.0, i32::MIN.wrapping_add(1));
    assert_ecmascript_to_int32_equals(POW_2_64, 0);
    assert_ecmascript_to_int32_equals(POW_2_64 + 1.0, 0);
}
// port: JSCompDoublesTest#assertIsMathematicalIntegerWithDecimalPrecision
fn assert_is_mathematical_integer_with_decimal_precision(x: f64) {
    for sign in [1.0, -1.0] {
        let value = sign * x;
        assert!(D::is_mathematical_integer(value));
        assert!(!D::is_mathematical_integer(value + 0.1));
        assert!(!D::is_mathematical_integer(value - 0.1));
    }
}
// port: JSCompDoublesTest#assertIsMathematicalIntegerWithoutDecimalPrecision
fn assert_is_mathematical_integer_without_decimal_precision(x: f64) {
    for sign in [1.0, -1.0] {
        let value = sign * x;
        assert!(D::is_mathematical_integer(value));
        assert_eq!(value, value + 0.1);
        assert_eq!(value, value - 0.1);
    }
}
// port: JSCompDoublesTest#assertEcmascriptToInt32Equals
fn assert_ecmascript_to_int32_equals(input: f64, expected: i32) {
    assert!(D::is_positive(input));
    for bias in [0.0, POW_2_32, POW_2_52] {
        for frac in [0.0, 0.1] {
            for sign in [1_i32, -1] {
                let value = sign as f64 * (input + bias + frac);
                assert_eq!(
                    D::ecmascript_to_int32(value),
                    sign.wrapping_mul(expected),
                    "{value}"
                );
            }
        }
    }
}
