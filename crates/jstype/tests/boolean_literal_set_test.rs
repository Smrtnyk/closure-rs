/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Nick Santos
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/rhino/jstype/BooleanLiteralSetTest.java.

use closure_jstype::boolean_literal_set::BooleanLiteralSet::{self, BOTH, EMPTY, FALSE, TRUE};

// port: BooleanLiteralSetTest#testIntersection
#[test]
fn test_intersection() {
    assert_eq!(EMPTY.intersection(EMPTY), EMPTY);
    assert_eq!(EMPTY.intersection(TRUE), EMPTY);
    assert_eq!(EMPTY.intersection(FALSE), EMPTY);
    assert_eq!(EMPTY.intersection(BOTH), EMPTY);
    assert_eq!(TRUE.intersection(EMPTY), EMPTY);
    assert_eq!(TRUE.intersection(TRUE), TRUE);
    assert_eq!(TRUE.intersection(FALSE), EMPTY);
    assert_eq!(TRUE.intersection(BOTH), TRUE);
    assert_eq!(FALSE.intersection(EMPTY), EMPTY);
    assert_eq!(FALSE.intersection(TRUE), EMPTY);
    assert_eq!(FALSE.intersection(FALSE), FALSE);
    assert_eq!(FALSE.intersection(BOTH), FALSE);
    assert_eq!(BOTH.intersection(EMPTY), EMPTY);
    assert_eq!(BOTH.intersection(TRUE), TRUE);
    assert_eq!(BOTH.intersection(FALSE), FALSE);
    assert_eq!(BOTH.intersection(BOTH), BOTH);
}

// port: BooleanLiteralSetTest#testUnion
#[test]
fn test_union() {
    assert_eq!(EMPTY.union(EMPTY), EMPTY);
    assert_eq!(EMPTY.union(TRUE), TRUE);
    assert_eq!(EMPTY.union(FALSE), FALSE);
    assert_eq!(EMPTY.union(BOTH), BOTH);
    assert_eq!(TRUE.union(EMPTY), TRUE);
    assert_eq!(TRUE.union(TRUE), TRUE);
    assert_eq!(TRUE.union(FALSE), BOTH);
    assert_eq!(TRUE.union(BOTH), BOTH);
    assert_eq!(FALSE.union(EMPTY), FALSE);
    assert_eq!(FALSE.union(TRUE), BOTH);
    assert_eq!(FALSE.union(FALSE), FALSE);
    assert_eq!(FALSE.union(BOTH), BOTH);
    assert_eq!(BOTH.union(EMPTY), BOTH);
    assert_eq!(BOTH.union(TRUE), BOTH);
    assert_eq!(BOTH.union(FALSE), BOTH);
    assert_eq!(BOTH.union(BOTH), BOTH);
}

// port: BooleanLiteralSetTest#testGet
#[test]
fn test_get() {
    assert_eq!(BooleanLiteralSet::get(true), TRUE);
    assert_eq!(BooleanLiteralSet::get(false), FALSE);
}

// port: BooleanLiteralSetTest#testContains
#[test]
fn test_contains() {
    assert!(!EMPTY.contains(true));
    assert!(!EMPTY.contains(false));
    assert!(TRUE.contains(true));
    assert!(!TRUE.contains(false));
    assert!(!FALSE.contains(true));
    assert!(FALSE.contains(false));
    assert!(BOTH.contains(true));
    assert!(BOTH.contains(false));
}
