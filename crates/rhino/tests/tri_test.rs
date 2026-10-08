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
//   test/com/google/javascript/jscomp/base/TriTest.java.

use closure_rhino::jscomp_base::Tri::{self, FALSE, TRUE, UNKNOWN};
// port: TriTest#or
#[test]
fn or() {
    let all_expected = [
        [FALSE, UNKNOWN, TRUE],
        [UNKNOWN, UNKNOWN, TRUE],
        [TRUE, TRUE, TRUE],
    ];
    for (i, row) in [FALSE, UNKNOWN, TRUE].into_iter().enumerate() {
        for (j, col) in [FALSE, UNKNOWN, TRUE].into_iter().enumerate() {
            assert_eq!(row.or(col), all_expected[i][j]);
            assert_eq!(col.or(row), all_expected[i][j]);
        }
    }
}
// port: TriTest#and
#[test]
fn and() {
    let all_expected = [
        [FALSE, FALSE, FALSE],
        [FALSE, UNKNOWN, UNKNOWN],
        [FALSE, UNKNOWN, TRUE],
    ];
    for (i, row) in [FALSE, UNKNOWN, TRUE].into_iter().enumerate() {
        for (j, col) in [FALSE, UNKNOWN, TRUE].into_iter().enumerate() {
            assert_eq!(row.and(col), all_expected[i][j]);
            assert_eq!(col.and(row), all_expected[i][j]);
        }
    }
}
// port: TriTest#xor
#[test]
fn xor() {
    let all_expected = [
        [FALSE, UNKNOWN, TRUE],
        [UNKNOWN, UNKNOWN, UNKNOWN],
        [TRUE, UNKNOWN, FALSE],
    ];
    for (i, row) in [FALSE, UNKNOWN, TRUE].into_iter().enumerate() {
        for (j, col) in [FALSE, UNKNOWN, TRUE].into_iter().enumerate() {
            assert_eq!(row.xor(col), all_expected[i][j]);
            assert_eq!(col.xor(row), all_expected[i][j]);
        }
    }
}
// port: TriTest#not
#[test]
fn not() {
    assert_eq!(FALSE.not(), TRUE);
    assert_eq!(UNKNOWN.not(), UNKNOWN);
    assert_eq!(TRUE.not(), FALSE);
}
// port: TriTest#toBoolean
#[test]
fn to_boolean() {
    let all_expected = [[false, false], [false, true], [true, true]];
    for (i, row) in [FALSE, UNKNOWN, TRUE].into_iter().enumerate() {
        assert_eq!(row.to_boolean(false), all_expected[i][0]);
        assert_eq!(row.to_boolean(true), all_expected[i][1]);
    }
}
// port: TriTest#forBoolean
#[test]
fn for_boolean() {
    assert_eq!(Tri::for_boolean(false), FALSE);
    assert_eq!(Tri::for_boolean(true), TRUE);
}
