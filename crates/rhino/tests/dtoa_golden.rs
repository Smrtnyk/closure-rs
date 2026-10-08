/*
 * Copyright 2026 The closure-rs Authors.
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

use closure_rhino::dtoa::d_to_a;
use closure_rhino::java_lang::double_to_string;
#[path = "support/dtoa_records.rs"]
mod dtoa_records;
use dtoa_records::{Outcome, decode, outcome};

#[test]
fn standard_golden() {
    let mut count = 0;
    let mut exceptions = 0;
    let mut surrogate_results = 0;
    for line in include_str!("data/standard.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 3);
        let bits = u64::from_str_radix(fields[0], 16).unwrap();
        let d = f64::from_bits(bits);
        let expected = decode(fields[1]).unwrap();
        if let Outcome::Value(ref units) = expected {
            surrogate_results += usize::from(units.iter().any(|c| (0xd800..=0xdfff).contains(c)));
        }
        exceptions += usize::from(matches!(expected, Outcome::Exception { .. }));
        if !matches!(expected, Outcome::Infeasible(_)) {
            assert_eq!(
                outcome(d_to_a::number_to_string(d)),
                expected,
                "bits={bits:016x}"
            );
        }
        assert_eq!(double_to_string(d), fields[2], "bits={bits:016x}");
        count += 1;
    }
    assert_eq!(count, 8000);
    assert!(exceptions > 0);
    assert!(surrogate_results > 0);
}

#[test]
fn modes_golden() {
    let mut count = 0;
    for line in include_str!("data/modes.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4);
        let d = f64::from_bits(u64::from_str_radix(fields[0], 16).unwrap());
        let mode = fields[1].parse().unwrap();
        let precision = fields[2].parse().unwrap();
        let expected = decode(fields[3]).unwrap();
        if !matches!(expected, Outcome::Infeasible(_)) {
            let mut buffer = Vec::new();
            let actual = d_to_a::js_dtostr(&mut buffer, mode, precision, d).map(|()| buffer);
            assert_eq!(outcome(actual), expected, "input={line}");
        }
        count += 1;
    }
    assert_eq!(count, 3000);
}

#[test]
fn jdk_golden_including_all_subnormal_categories() {
    let mut count = 0;
    for line in include_str!("data/jdk.tsv").lines() {
        let (hex, expected) = line.split_once('\t').unwrap();
        let bits = u64::from_str_radix(hex, 16).unwrap();
        assert_eq!(
            double_to_string(f64::from_bits(bits)),
            expected,
            "bits={bits:016x}"
        );
        count += 1;
    }
    assert_eq!(count, 8000);
}

#[test]
fn all_internal_modes_golden_including_java_exceptions() {
    let mut count = 0;
    let mut exceptions = 0;
    for line in include_str!("data/raw.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert!(fields.len() == 5 || fields.len() == 7);
        let d = f64::from_bits(u64::from_str_radix(fields[0], 16).unwrap());
        let mode = fields[1].parse().unwrap();
        let bias_up = fields[2].parse().unwrap();
        let ndigits = fields[3].parse().unwrap();
        let expected = decode(fields[fields.len() - 1]).unwrap();
        exceptions += usize::from(matches!(expected, Outcome::Exception { .. }));
        if !matches!(expected, Outcome::Infeasible(_)) {
            let mut sign = false;
            let mut buffer = Vec::new();
            let result = d_to_a::js_dtoa(d, mode, bias_up, ndigits, &mut sign, &mut buffer);
            if let Ok(decpt) = result {
                assert_eq!(fields.len(), 7);
                assert_eq!(decpt, fields[4].parse::<i32>().unwrap(), "input={line}");
                assert_eq!(sign, fields[5].parse::<bool>().unwrap(), "input={line}");
            }
            assert_eq!(outcome(result.map(|_| buffer)), expected, "input={line}");
        }
        count += 1;
    }
    assert_eq!(count, 1000);
    assert!(exceptions > 0);
}

#[test]
fn java_big_integer_overflow_for_required_subnormal() {
    // Pinned DToA#numberToString for nextDown(2^-1043), verified by Probe.java.
    assert_eq!(
        d_to_a::number_to_string(f64::from_bits(0x000000007fffffff)),
        Err(d_to_a::DToAError::BigIntegerOverflow)
    );
}

#[test]
fn pinned_subnormal_utf16_quirks() {
    // Exact Java UTF-16 units obtained from CharProbe.java, not Rust's output.
    for (bits, leading, tail) in [
        (0x0000000080000000, 0x009a, ".09978955e-316"),
        (0x0000000180000000, 0x016e, ".29936864e-316"),
        (0x0000010480000000, 0xd81d, ".99035465e-316"),
    ] {
        let mut expected = vec![leading];
        expected.extend(tail.encode_utf16());
        assert_eq!(
            d_to_a::number_to_string(f64::from_bits(bits)).unwrap(),
            expected
        );
    }
}

#[test]
fn pinned_quick_path_array_bounds_exception() {
    let mut buffer = Vec::new();
    let error = d_to_a::js_dtostr(
        &mut buffer,
        d_to_a::DTOSTR_EXPONENTIAL,
        2,
        f64::from_bits(0x0000000000001001),
    )
    .unwrap_err();
    assert_eq!(
        error.java_class(),
        "java.lang.ArrayIndexOutOfBoundsException"
    );
    assert_eq!(error.to_string(), "Index 7 out of bounds for length 5");
}

// Expected strings obtained from the pinned Java harness (picked.tsv.gz); moved here from the
// removed duplicate dtoa/double_to_decimal.rs when it was replaced by java_lang::double_to_string.
#[test]
fn double_to_string_java_hand_picked_cases() {
    for (bits, expected) in [
        (0x0000000000000000u64, "0.0"),
        (0x8000000000000000u64, "-0.0"),
        (0x7ff8000000000000u64, "NaN"),
        (0x7ff0000000000000u64, "Infinity"),
        (0xfff0000000000000u64, "-Infinity"),
        (0x444b1ae4d6e2ef50u64, "1.0E21"),
        (0x3e7ad7f29abcaf48u64, "1.0E-7"),
        (0x441ac53a7e04bcdau64, "1.2345678901234568E20"),
        (0x0000000000000001u64, "4.9E-324"),
        (0x416312d000000000u64, "1.0E7"),
        (0x3f50624dd2f1a9fcu64, "0.001"),
        (0x3f1a36e2eb1c432du64, "1.0E-4"),
        (0x44c52d02c7e14af6u64, "2.0E23"),
        (0x4340000000000000u64, "9.007199254740992E15"),
        (0x0010000000000000u64, "2.2250738585072014E-308"),
        (0x7fefffffffffffffu64, "1.7976931348623157E308"),
        (0x44b52d02c7e14af6u64, "1.0E23"),
        (0x3fb999999999999au64, "0.1"),
        (0x4004000000000000u64, "2.5"),
    ] {
        assert_eq!(
            double_to_string(f64::from_bits(bits)),
            expected,
            "bits={bits:016x}"
        );
    }
}
