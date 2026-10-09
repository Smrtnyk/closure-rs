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
//   test/com/google/javascript/jscomp/colors/ColorIdTest.java.

use closure_jscomp::colors::ColorId;
use closure_rhino::fast_hash::IndexSet;
use std::{panic::catch_unwind, sync::LazyLock};

static A: LazyLock<ColorId> = LazyLock::new(|| ColorId::from_ascii("a"));
static B: LazyLock<ColorId> = LazyLock::new(|| ColorId::from_ascii("b"));
static C: LazyLock<ColorId> = LazyLock::new(|| ColorId::from_ascii("c"));
const ZERO: ColorId = ColorId::from_unsigned_int(0);
const ONE: ColorId = ColorId::from_unsigned_int(1);

// port: ColorIdTest#equals_sameInputTrue
#[test]
fn equals_same_input_true() {
    assert_equals_and_related_methods(*A, ColorId::from_ascii("a"));
}
// port: ColorIdTest#equals_differentInputFalse
#[test]
fn equals_different_input_false() {
    assert_not_equals_and_related_methods(*A, *B);
}
// port: ColorIdTest#equals_leadingZerosIgnored
#[test]
fn equals_leading_zeros_ignored() {
    assert_equals_and_related_methods(
        ColorId::from_bytes(&bytes(&[0, 0, 13])),
        ColorId::from_bytes(&bytes(&[13])),
    );
}
// port: ColorIdTest#equals_leadingZerosIgnored_negativeBytes
#[test]
fn equals_leading_zeros_ignored_negative_bytes() {
    assert_equals_and_related_methods(
        ColorId::from_bytes(&bytes(&[0, 0, -13])),
        ColorId::from_bytes(&bytes(&[-13])),
    );
}
// port: ColorIdTest#equals_fromByteArrayVsFromAscii
#[test]
fn equals_from_byte_array_vs_from_ascii() {
    assert_equals_and_related_methods(*A, ColorId::from_bytes(&bytes(&[97])));
}
// port: ColorIdTest#equals_fromByteStringVsFromAscii
#[test]
fn equals_from_byte_string_vs_from_ascii() {
    assert_equals_and_related_methods(*A, ColorId::from_byte_string(&[97]));
}
// port: ColorIdTest#equals_maxSize
#[test]
fn equals_max_size() {
    assert_equals_and_related_methods(
        ColorId::from_ascii("12345678"),
        ColorId::from_ascii("12345678"),
    );
}
// port: ColorIdTest#maxSize64Bits
#[test]
fn max_size64_bits() {
    ColorId::from_ascii("12345678");
    assert!(catch_unwind(|| ColorId::from_ascii("123456789")).is_err());
}
// port: ColorIdTest#union_commutativity
#[test]
fn union_commutativity() {
    let abc = ColorId::union(&IndexSet::<_>::from_iter([*A, *B, *C]));
    let acb = ColorId::union(&IndexSet::<_>::from_iter([*A, *C, *B]));
    let cab = ColorId::union(&IndexSet::<_>::from_iter([*C, *B, *A]));
    assert_equals_and_related_methods(abc, acb);
    assert_equals_and_related_methods(abc, cab);
    assert_equals_and_related_methods(acb, cab);
}
// port: ColorIdTest#union_identity
#[test]
fn union_identity() {
    assert_eq!(ColorId::union(&IndexSet::<_>::from_iter([*A])), *A);
}
// port: ColorIdTest#union_empty
#[test]
fn union_empty() {
    assert!(catch_unwind(|| ColorId::union(&IndexSet::<_>::default())).is_err());
}
// port: ColorIdTest#union_zeroAffectsResult
#[test]
fn union_zero_affects_result() {
    assert_not_equals_and_related_methods(
        ColorId::union(&IndexSet::<_>::from_iter([*A, *B])),
        ColorId::union(&IndexSet::<_>::from_iter([*A, *B, ZERO])),
    );
}
// port: ColorIdTest#union_oneAffectsResult
#[test]
fn union_one_affects_result() {
    assert_not_equals_and_related_methods(
        ColorId::union(&IndexSet::<_>::from_iter([*A, *B])),
        ColorId::union(&IndexSet::<_>::from_iter([*A, *B, ONE])),
    );
}
// port: ColorIdTest#fromUnsigned_noSignExtension
#[test]
fn from_unsigned_no_sign_extension() {
    assert_eq!(ColorId::from_unsigned_byte(-1).to_string(), "ff");
    assert_eq!(ColorId::from_unsigned_int(-1).to_string(), "ffffffff");
    assert_eq!(ColorId::from_unsigned(-1).to_string(), "ffffffffffffffff");
}
// port: ColorIdTest#roundtrip_throughByteString
#[test]
fn roundtrip_through_byte_string() {
    assert_equals_and_related_methods(*A, ColorId::from_byte_string(&A.as_byte_string()));
}
// port: ColorIdTest#asByteString_exactBytes
#[test]
fn as_byte_string_exact_bytes() {
    assert_eq!(A.as_byte_string(), vec![0, 0, 0, 0, 0, 0, 0, 97]);
}

// port: ColorIdTest#assertEqualsAndRelatedMethods
fn assert_equals_and_related_methods(actual: ColorId, expected: ColorId) {
    assert_eq!(actual, expected);
    assert_eq!(actual.hash_code(), expected.hash_code());
    assert_eq!(actual.to_string(), expected.to_string());
}
// port: ColorIdTest#assertNotEqualsAndRelatedMethods
fn assert_not_equals_and_related_methods(actual: ColorId, expected: ColorId) {
    assert_ne!(actual, expected);
    assert_ne!(actual.to_string(), expected.to_string());
}
// port: ColorIdTest#bytes
fn bytes(input: &[i32]) -> Vec<i8> {
    input.iter().map(|x| *x as i8).collect()
}
