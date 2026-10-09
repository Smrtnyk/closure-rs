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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/testing/Asserts.java.

use super::type_subject::TypeSubject;
use crate::TypeId;
use crate::js_type::JSType;
use crate::js_type_registry::JSTypeRegistry;
use closure_rhino::node::Ast;

pub struct Asserts;
impl Asserts {
    // port: Asserts#Asserts
    fn new() -> Self {
        Self
    }
    // port: Asserts#assertResolvesToSame
    pub fn assert_resolves_to_same(reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId {
        assert_eq!(type_, Self::assert_valid_resolve(reg, ast, type_));
        type_
    }
    // port: Asserts#assertValidResolve
    pub fn assert_valid_resolve(reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId {
        let mut reporter = closure_rhino::testing::test_error_reporter::TestErrorReporter::new();
        let resolved_type = type_.resolve_with_reporter(reg, ast, &mut reporter);
        TypeSubject::assert_type(resolved_type).is_equal_to(reg, ast, type_);
        reporter.verify_has_encountered_all_warnings_and_errors();
        resolved_type
    }
    // port: Asserts#assertTypeCollectionEquals
    pub fn assert_type_collection_equals(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        a: &[TypeId],
        b: &[TypeId],
    ) {
        assert_eq!(b.len(), a.len());
        for (&actual, &expected) in b.iter().zip(a) {
            TypeSubject::assert_type(actual).is_equal_to(reg, ast, expected);
        }
    }
    // port: Asserts#assertEquivalenceOperations
    pub fn assert_equivalence_operations(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        a: TypeId,
        b: TypeId,
    ) {
        let _ = Self::new();
        TypeSubject::assert_type(a).is_equal_to(reg, ast, a);
        TypeSubject::assert_type(b).is_equal_to(reg, ast, a);
        TypeSubject::assert_type(b).is_equal_to(reg, ast, b);
        assert!(a.is_subtype_of(reg, ast, b));
        assert!(a.is_subtype_of(reg, ast, a));
        assert!(b.is_subtype_of(reg, ast, b));
        assert!(b.is_subtype_of(reg, ast, a));
        for (left, right) in [(a, b), (a, a), (b, b), (b, a)] {
            let greatest = left.get_greatest_subtype(reg, ast, right);
            TypeSubject::assert_type(greatest).is_equal_to(reg, ast, a);
        }
        for (left, right) in [(a, b), (a, a), (b, b), (b, a)] {
            let least = left.get_least_supertype(reg, ast, right);
            TypeSubject::assert_type(least).is_equal_to(reg, ast, a);
        }
        assert!(a.can_cast_to(reg, ast, b));
        assert!(a.can_cast_to(reg, ast, a));
        assert!(b.can_cast_to(reg, ast, b));
        assert!(b.can_cast_to(reg, ast, a));
    }
}
pub use Asserts as TypeAsserts;

/// Guava EqualsTester's equality groups, using Java structural type equality and hashes.
// port: EqualsTester#testEquals
pub fn assert_equality_groups(reg: &mut JSTypeRegistry, ast: &Ast, groups: &[Vec<TypeId>]) {
    for (group_index, group) in groups.iter().enumerate() {
        for &actual in group {
            TypeSubject::assert_type(actual).is_equal_to(reg, ast, actual);
            TypeSubject::assert_type(actual).is_not_equal_to(reg, ast, None);
            for (other_index, other_group) in groups.iter().enumerate() {
                for &expected in other_group {
                    if group_index == other_index {
                        TypeSubject::assert_type(actual).is_equal_to(reg, ast, expected);
                    } else {
                        TypeSubject::assert_type(actual).is_not_equal_to(reg, ast, expected);
                    }
                }
            }
        }
    }
}
