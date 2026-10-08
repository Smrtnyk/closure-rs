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
 *   Google Inc.
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
//   test/com/google/javascript/rhino/RhinoStringPoolTest.java.

use closure_rhino::js_string::JsString;
use closure_rhino::rhino_string_pool::{LazyInternedStringList, RhinoStringPool, WriteOnlyBitset};
// port: RhinoStringPoolTest#returnsDotEqualsString
#[test]
fn returns_dot_equals_string() {
    assert_eq!(RhinoStringPool::add_or_get("foo"), "foo");
}
// port: RhinoStringPoolTest#multipleCallsReturnSameInstance
#[test]
fn multiple_calls_return_same_instance() {
    let foos: Vec<_> = (0..100)
        .map(|_| RhinoStringPool::add_or_get(JsString::from("foo")))
        .collect();
    for foo in &foos[1..] {
        assert!(foo.ptr_eq(&foos[0]));
    }
}
// port: RhinoStringPoolTest#lazyInternedStringList_outOfBounds_throwsException
#[test]
fn lazy_interned_string_list_out_of_bounds_throws_exception() {
    let list = LazyInternedStringList::new(vec!["foo".into()]);
    assert!(std::panic::catch_unwind(|| list.get(100)).is_err());
}
// port: RhinoStringPoolTest#lazyInternedStringList_negativeIndex_throwsException
#[test]
fn lazy_interned_string_list_negative_index_throws_exception() {
    let list = LazyInternedStringList::new(vec![]);
    assert!(std::panic::catch_unwind(|| list.get(-1)).is_err());
}
// port: RhinoStringPoolTest#lazyInternedStringList_getsEqualValuesAsInputList
#[test]
fn lazy_interned_string_list_gets_equal_values_as_input_list() {
    let list = LazyInternedStringList::new(vec!["foo".into(), "bar".into(), "baz".into()]);
    assert_eq!(list.get(0), "foo");
    assert_eq!(list.get(1), "bar");
    assert_eq!(list.get(2), "baz");
}
// port: RhinoStringPoolTest#lazyInternedStringList_getsSameInstanceAsRhinoStringPool
#[test]
fn lazy_interned_string_list_gets_same_instance_as_rhino_string_pool() {
    let strings = vec![
        JsString::from("foo"),
        JsString::from("bar"),
        JsString::from("baz"),
    ];
    let list = LazyInternedStringList::new(strings);
    assert!(list.get(0).ptr_eq(&RhinoStringPool::add_or_get("foo")));
    assert!(list.get(1).ptr_eq(&RhinoStringPool::add_or_get("bar")));
    assert!(list.get(2).ptr_eq(&RhinoStringPool::add_or_get("baz")));
}
// port: RhinoStringPoolTest#lazyInternedStringList_veryLargeListWorks
#[test]
fn lazy_interned_string_list_very_large_list_works() {
    let strings = (0..10000)
        .map(|i| JsString::from(format!("foo{i}")))
        .collect();
    let list = LazyInternedStringList::new(strings);
    for i in 0..10000 {
        assert!(
            list.get(i)
                .ptr_eq(&RhinoStringPool::add_or_get(format!("foo{i}")))
        );
    }
}
// port: RhinoStringPoolTest#writeOnlyBitset_initializedToFalse
#[test]
fn write_only_bitset_initialized_to_false() {
    let bitset = WriteOnlyBitset::new(5);
    for i in 0..5 {
        assert!(!bitset.get(i));
    }
}
// port: RhinoStringPoolTest#writeOnlyBitset_setToTrue_thenGetReturnsTrue
#[test]
fn write_only_bitset_set_to_true_then_get_returns_true() {
    let bitset = WriteOnlyBitset::new(2);
    bitset.set(0);
    assert!(bitset.get(0));
    assert!(!bitset.get(1));
}
// port: RhinoStringPoolTest#writeOnlyBitset_wordBoundary_setToTrue_thenGetReturnsTrue
#[test]
fn write_only_bitset_word_boundary_set_to_true_then_get_returns_true() {
    let bitset = WriteOnlyBitset::new(33);
    bitset.set(32);
    assert!(bitset.get(32));
}
// port: RhinoStringPoolTest#writeOnlyBitset_multipleSetsToTrue_allReturnTrue
#[test]
fn write_only_bitset_multiple_sets_to_true_all_return_true() {
    let bitset = WriteOnlyBitset::new(5);
    bitset.set(1);
    bitset.set(2);
    bitset.set(3);
    assert!(!bitset.get(0));
    assert!(bitset.get(1));
    assert!(bitset.get(2));
    assert!(bitset.get(3));
    assert!(!bitset.get(4));
}
