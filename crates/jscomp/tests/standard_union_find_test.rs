/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/graph/StandardUnionFindTest.java.

use closure_jscomp::graph::{standard_union_find::StandardUnionFind, union_find::UnionFind};
use indexmap::IndexSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
// port: StandardUnionFindTest#setUp
fn set_up() -> StandardUnionFind<&'static str> {
    StandardUnionFind::new()
}
// port: StandardUnionFindTest#testEmpty
#[test]
fn test_empty() {
    let mut union = set_up();
    assert!(union.all_equivalence_classes().is_empty());
}
// port: StandardUnionFindTest#testAdd
#[test]
fn test_add() {
    let mut union = set_up();
    union.add("foo");
    union.add("bar");
    assert!(!union.find(&"foo").is_empty());
    assert_eq!(union.all_equivalence_classes().len(), 2);
}
// port: StandardUnionFindTest#testUnion
#[test]
fn test_union() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("C", "D");
    assert_eq!(union.find(&"B"), union.find(&"A"));
    assert_eq!(union.find(&"D"), union.find(&"C"));
    assert_ne!(union.find(&"A"), union.find(&"D"));
}
// port: StandardUnionFindTest#testSetSize
#[test]
fn test_set_size() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("B", "C");
    union.union("D", "E");
    union.union("F", "F");

    assert_eq!(union.find_all("A").size(&mut union), 3);
    assert_eq!(union.find_all("B").size(&mut union), 3);
    assert_eq!(union.find_all("C").size(&mut union), 3);
    assert_eq!(union.find_all("D").size(&mut union), 2);
    assert_eq!(union.find_all("F").size(&mut union), 1);
}
// port: StandardUnionFindTest#testFind
#[test]
fn test_find() {
    let mut union = set_up();
    union.add("A");
    union.add("B");
    assert_eq!(union.find(&"A"), "A");
    assert_eq!(union.find(&"B"), "B");

    union.union("A", "B");
    assert_eq!(union.find(&"B"), union.find(&"A"));

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            union.find(&"Z");
        }))
        .is_err()
    );
}
// port: StandardUnionFindTest#testAllEquivalenceClasses
#[test]
fn test_all_equivalence_classes() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("A", "B");
    union.union("B", "A");
    union.union("B", "C");
    union.union("D", "E");
    union.union("F", "F");

    let classes = union.all_equivalence_classes();
    assert_eq!(classes.len(), 3);
    assert!(classes.contains(&IndexSet::from(["A", "B", "C"])));
    assert!(classes.contains(&IndexSet::from(["D", "E"])));
    assert!(classes.contains(&IndexSet::from(["F"])));
}
// port: StandardUnionFindTest#testFindAll
#[test]
fn test_find_all() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("A", "B");
    union.union("B", "A");
    union.union("D", "E");
    union.union("F", "F");

    let a_set = union.find_all("A");
    assert_eq!(
        a_set.iterator(&mut union).collect::<IndexSet<_>>(),
        IndexSet::from(["A", "B"])
    );

    union.union("B", "C");
    assert!(a_set.contains(&mut union, &"C"));
    assert_eq!(a_set.size(&mut union), 3);

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            union.find_all("Z");
        }))
        .is_err()
    );
}
// port: StandardUnionFindTest#testFindAllIterator
#[test]
fn test_find_all_iterator() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("B", "C");
    union.union("A", "B");
    union.union("D", "E");

    let a_set = union.find_all("A");
    let mut a_iter = a_set.iterator(&mut union).peekable();
    assert!(a_iter.peek().is_some());
    assert_eq!(a_iter.next(), Some("A"));
    assert_eq!(a_iter.next(), Some("B"));
    assert_eq!(a_iter.next(), Some("C"));
    assert!(a_iter.peek().is_none());

    let d_set = union.find_all("D");
    let mut d_iter = d_set.iterator(&mut union).peekable();
    assert!(d_iter.peek().is_some());
    assert_eq!(d_iter.next(), Some("D"));
    assert_eq!(d_iter.next(), Some("E"));
    assert!(d_iter.peek().is_none());
}
// port: StandardUnionFindTest#testFindAllSize
#[test]
fn test_find_all_size() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("B", "C");
    assert_eq!(union.find_all("A").size(&mut union), 3);
    assert_eq!(union.find_all("B").size(&mut union), 3);
    assert_eq!(union.find_all("C").size(&mut union), 3);
    union.union("D", "E");
    assert_eq!(union.find_all("C").size(&mut union), 3);
    assert_eq!(union.find_all("D").size(&mut union), 2);
    union.union("B", "E");
    assert_eq!(union.find_all("C").size(&mut union), 5);
    assert_eq!(union.find_all("D").size(&mut union), 5);
}
// port: StandardUnionFindTest#testElements
#[test]
fn test_elements() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("B", "C");
    union.union("A", "B");
    union.union("D", "E");

    let elements = union.elements();
    assert_eq!(elements, IndexSet::from(["A", "B", "C", "D", "E"]));
    assert!(!elements.contains("F"));
}
// port: StandardUnionFindTest#testCopy
#[test]
fn test_copy() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("B", "Z");
    union.union("X", "Y");
    let mut copy = StandardUnionFind::from_union_find(&mut union);
    assert_eq!(
        copy.find_all("Z")
            .iterator(&mut copy)
            .collect::<IndexSet<_>>(),
        IndexSet::from(["A", "B", "Z"])
    );
    assert_eq!(
        copy.find_all("X")
            .iterator(&mut copy)
            .collect::<IndexSet<_>>(),
        IndexSet::from(["X", "Y"])
    );
}
// port: StandardUnionFindTest#testChangesToCopyDontAffectOriginal
#[test]
fn test_changes_to_copy_dont_affect_original() {
    let mut union = set_up();
    union.union("A", "B");
    union.union("X", "Y");
    union.union("A", "C");
    let mut copy = StandardUnionFind::from_union_find(&mut union);
    copy.union("A", "D");
    assert_eq!(
        copy.find_all("D")
            .iterator(&mut copy)
            .collect::<IndexSet<_>>(),
        IndexSet::from(["A", "B", "C", "D"])
    );
    assert_eq!(
        union
            .find_all("A")
            .iterator(&mut union)
            .collect::<IndexSet<_>>(),
        IndexSet::from(["A", "B", "C"])
    );
    assert_eq!(
        copy.find_all("X")
            .iterator(&mut copy)
            .collect::<IndexSet<_>>(),
        IndexSet::from(["X", "Y"])
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            union.find_all("D");
        }))
        .is_err()
    );
}
// port: StandardUnionFindTest#testCheckEquivalent
#[test]
fn test_check_equivalent() {
    let mut union = set_up();
    union.union("A", "B");
    union.add("C");
    assert!(union.are_equivalent(&"A", &"B"));
    assert!(!union.are_equivalent(&"C", &"A"));
    assert!(!union.are_equivalent(&"C", &"B"));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            union.are_equivalent(&"A", &"F");
        }))
        .is_err()
    );
}
// Rust-only (no Java counterpart): Java allows null elements (@Nullable E) and
// allRepresentatives has no Java test.
#[test]
fn rust_only_nullable_elements_and_representatives() {
    let mut nullable = StandardUnionFind::new_with_value_to_string(|e: &Option<&str>| {
        e.unwrap_or("null").to_owned()
    });
    nullable.add(None);
    nullable.union(None, Some("x"));
    assert_eq!(nullable.find(&Some("x")), None);
    assert_eq!(nullable.elements(), IndexSet::from([None, Some("x")]));
    assert_eq!(nullable.all_representatives(), IndexSet::from([None]));
    assert_eq!(
        nullable
            .find_all(None)
            .iterator(&mut nullable)
            .collect::<Vec<_>>(),
        [None, Some("x")]
    );
}
