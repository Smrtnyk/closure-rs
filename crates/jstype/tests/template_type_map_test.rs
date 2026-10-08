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
//   test/com/google/javascript/rhino/jstype/TemplateTypeMapTest.java.

use closure_jstype::{
    TypeId,
    template_type_map::TemplateTypeMap,
    testing::{
        base_js_type_test_case::BaseJSTypeTestCase,
        template_type_map_subject::TemplateTypeMapSubject, type_subject::TypeSubject,
    },
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

struct Fixture {
    base: BaseJSTypeTestCase,
    empty_map: Arc<TemplateTypeMap>,
}
impl Fixture {
    // port: TemplateTypeMapTest#setUp
    fn set_up() -> Self {
        let base = BaseJSTypeTestCase::new();
        let empty_map = base.reg.get_empty_template_type_map();
        Self { base, empty_map }
    }
    // port: TemplateTypeMapTest#createMap
    fn create_map(&mut self, keys: &[TypeId], values: &[TypeId]) -> Arc<TemplateTypeMap> {
        self.empty_map
            .copy_with_extension(&mut self.base.reg, &self.base.ast, keys, values)
    }
    // port: TemplateTypeMapTest#key
    fn key(&mut self, name: &str) -> TypeId {
        self.base.reg.create_template_type(&self.base.ast, name)
    }
}

// port: TemplateTypeMapTest#testCreateEmptyMap_throwsWhenDuplicateRequested
#[test]
fn test_create_empty_map_throws_when_duplicate_requested() {
    let f = Fixture::set_up();

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            TemplateTypeMap::create_empty(&f.base.reg);
        }))
        .is_err()
    );
}

// port: TemplateTypeMapTest#testEmptyMap_hasNoKeysValuesOrSubmaps
#[test]
fn test_empty_map_has_no_keys_values_or_submaps() {
    let mut f = Fixture::set_up();

    TemplateTypeMapSubject::assert_that(f.empty_map.clone()).has_keys_and_values(
        &mut f.base.reg,
        &f.base.ast,
        &[],
        &[],
    );
    TemplateTypeMapSubject::assert_that(f.empty_map.clone()).has_submaps_at(&[]);
}

// port: TemplateTypeMapTest#testCopyExtend_createsNewMap
#[test]
fn test_copy_extend_creates_new_map() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let result = f.empty_map.copy_with_extension(
        &mut f.base.reg,
        &f.base.ast,
        &[key_a],
        &[f.base.number_type],
    );

    TemplateTypeMapSubject::assert_that(result.clone()).is_not_same_instance_as(&f.empty_map);
    TemplateTypeMapSubject::assert_that(result.clone()).has_submaps_at(&[0]);
}

// port: TemplateTypeMapTest#testCopyExtend_validatesCountOfKeysVsValues
#[test]
fn test_copy_extend_validates_count_of_keys_vs_values() {
    let mut f = Fixture::set_up();

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            f.empty_map.copy_with_extension(
                &mut f.base.reg,
                &f.base.ast,
                &[],
                &[f.base.number_type],
            );
        }))
        .is_err()
    );
}

// port: TemplateTypeMapTest#testCopyExtend_emptyMap
#[test]
fn test_copy_extend_empty_map() {
    let mut f = Fixture::set_up();

    let key_t = f.key("T");
    let key_u = f.key("U");

    let result = f.empty_map.copy_with_extension(
        &mut f.base.reg,
        &f.base.ast,
        &[key_t, key_u],
        &[f.base.number_type],
    );

    TemplateTypeMapSubject::assert_that(result.clone()).has_keys_and_values(
        &mut f.base.reg,
        &f.base.ast,
        &[key_t, key_u],
        &[f.base.number_type],
    );
    TemplateTypeMapSubject::assert_that(result.clone()).has_submaps_at(&[0]);
}

// port: TemplateTypeMapTest#testCopyExtend_partialMap_fillsExitingUnfilledKeysWithUnknown
#[test]
fn test_copy_extend_partial_map_fills_exiting_unfilled_keys_with_unknown() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let key_b = f.key("B");
    let key_c = f.key("C");
    let existing = f.create_map(&[key_a, key_b, key_c], &[f.base.number_type]);

    let key_x = f.key("X");
    let key_y = f.key("Y");
    let key_z = f.key("Z");

    let result = existing.copy_with_extension(
        &mut f.base.reg,
        &f.base.ast,
        &[key_x, key_y, key_z],
        &[f.base.string_type],
    );

    TemplateTypeMapSubject::assert_that(result.clone()).has_keys_and_values(
        &mut f.base.reg,
        &f.base.ast,
        &[key_a, key_b, key_c, key_x, key_y, key_z],
        &[
            f.base.number_type,
            f.base.unknown_type,
            f.base.unknown_type,
            f.base.string_type,
        ],
    );
    TemplateTypeMapSubject::assert_that(result.clone()).has_submaps_at(&[0, 3]);
}

// port: TemplateTypeMapTest#testCopyExtend_partialMap_emptyExtension_fillsExitingUnfilledKeysWithUnknown
#[test]
fn test_copy_extend_partial_map_empty_extension_fills_exiting_unfilled_keys_with_unknown() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let key_b = f.key("B");
    let key_c = f.key("C");
    let existing = f.create_map(&[key_a, key_b, key_c], &[f.base.number_type]);

    let result = existing.copy_with_extension(&mut f.base.reg, &f.base.ast, &[], &[]);

    TemplateTypeMapSubject::assert_that(result.clone()).has_keys_and_values(
        &mut f.base.reg,
        &f.base.ast,
        &[key_a, key_b, key_c],
        &[f.base.number_type, f.base.unknown_type, f.base.unknown_type],
    );
    TemplateTypeMapSubject::assert_that(result.clone()).has_submaps_at(&[0]);
}

// port: TemplateTypeMapTest#testCopyExtend_ifMapFilled_ifExtensionEmpty_returnsSelf
#[test]
fn test_copy_extend_if_map_filled_if_extension_empty_returns_self() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let existing = f.create_map(&[key_a], &[f.base.number_type]);

    let result = f
        .empty_map
        .copy_with_extension(&mut f.base.reg, &f.base.ast, &[], &[]);

    TemplateTypeMapSubject::assert_that(result.clone()).is_not_same_instance_as(&existing);
}

// port: TemplateTypeMapTest#testCopyFill_validatesValueCount_againstUnfilledKeys
#[test]
fn test_copy_fill_validates_value_count_against_unfilled_keys() {
    let mut f = Fixture::set_up();

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            f.empty_map.copy_filled_with_values(
                &mut f.base.reg,
                &f.base.ast,
                &[f.base.number_type],
            );
        }))
        .is_err()
    );
}

// port: TemplateTypeMapTest#testCopyFill_ifNoSlotsEmpty_returnsSelf
#[test]
fn test_copy_fill_if_no_slots_empty_returns_self() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let key_b = f.key("B");
    let existing = f.create_map(&[key_a, key_b], &[f.base.number_type, f.base.string_type]);

    let result = existing.copy_filled_with_values(&mut f.base.reg, &f.base.ast, &[]);

    TemplateTypeMapSubject::assert_that(result.clone()).is_same_instance_as(&existing);
}

// port: TemplateTypeMapTest#testCopyFill_partialMap_fillsExtraKeysWithUnknown
#[test]
fn test_copy_fill_partial_map_fills_extra_keys_with_unknown() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let key_b = f.key("B");
    let key_c = f.key("C");
    let existing = f.create_map(&[key_a, key_b, key_c], &[f.base.number_type]);

    let result =
        existing.copy_filled_with_values(&mut f.base.reg, &f.base.ast, &[f.base.string_type]);

    TemplateTypeMapSubject::assert_that(result.clone()).has_keys_and_values(
        &mut f.base.reg,
        &f.base.ast,
        &[key_a, key_b, key_c],
        &[f.base.number_type, f.base.string_type, f.base.unknown_type],
    );
    TemplateTypeMapSubject::assert_that(result.clone()).has_submaps_at(&[0]);
}

// port: TemplateTypeMapTest#testCopyWithout_canRemoveUnfilledKeys
#[test]
fn test_copy_without_can_remove_unfilled_keys() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let key_b = f.key("B");
    let key_c = f.key("C");
    let existing = f.create_map(&[key_a, key_b, key_c], &[f.base.number_type]);

    let result = existing.copy_without_keys(&mut f.base.reg, &f.base.ast, &[key_b]);

    TemplateTypeMapSubject::assert_that(result.clone()).has_keys_and_values(
        &mut f.base.reg,
        &f.base.ast,
        &[key_a, key_c],
        &[f.base.number_type],
    );
    TemplateTypeMapSubject::assert_that(result.clone()).has_submaps_at(&[0]);
}

// port: TemplateTypeMapTest#testCopyWithout_preservesSubmapBoundary
#[test]
fn test_copy_without_preserves_submap_boundary() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let key_b = f.key("B");
    let key_c = f.key("C");
    let key_d = f.key("D");
    let key_e = f.key("E");
    let existing = f.create_map(&[key_a, key_b], &[f.base.number_type, f.base.string_type]);
    let extension =
        existing.copy_with_extension(&mut f.base.reg, &f.base.ast, &[key_c, key_d, key_e], &[]);

    let result = extension.copy_without_keys(&mut f.base.reg, &f.base.ast, &[key_c, key_e]);

    TemplateTypeMapSubject::assert_that(result.clone()).has_keys_and_values(
        &mut f.base.reg,
        &f.base.ast,
        &[key_a, key_b, key_d],
        &[f.base.number_type, f.base.string_type],
    );
    TemplateTypeMapSubject::assert_that(result.clone()).has_submaps_at(&[0, 2]);
}

// port: TemplateTypeMapTest#testCopyWithout_retainFilledKeys
#[test]
fn test_copy_without_retain_filled_keys() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let key_b = f.key("B");
    let key_c = f.key("C");
    let existing = f.create_map(&[key_a, key_b, key_c], &[f.base.number_type]);

    let result = existing.copy_without_keys(&mut f.base.reg, &f.base.ast, &[key_a]);

    TemplateTypeMapSubject::assert_that(result.clone()).has_keys_and_values(
        &mut f.base.reg,
        &f.base.ast,
        &[key_a, key_b, key_c],
        &[f.base.number_type],
    );
    TemplateTypeMapSubject::assert_that(result.clone()).has_submaps_at(&[0]);
}

// port: TemplateTypeMapTest#testCopyWithout_ifNothingRemoved_returnsSelf
#[test]
fn test_copy_without_if_nothing_removed_returns_self() {
    let mut f = Fixture::set_up();

    let key_a = f.key("A");
    let key_b = f.key("B");
    let key_c = f.key("C");
    let existing = f.create_map(&[key_a, key_b, key_c], &[f.base.number_type]);

    let result = existing.copy_without_keys(&mut f.base.reg, &f.base.ast, &[key_a]);

    TemplateTypeMapSubject::assert_that(result.clone()).is_same_instance_as(&existing);
}

// port: TemplateTypeMapTest#testGetLastTemplateTypeKeyByName_returnsLastKeyIfDuplicates
#[test]
fn test_get_last_template_type_key_by_name_returns_last_key_if_duplicates() {
    let mut f = Fixture::set_up();

    let key1 = f.key("A");
    let key2 = f.key("A");

    let map = f.create_map(&[key1, key2], &[f.base.number_type]);

    let result = map.get_last_template_type_key_by_name(&f.base.reg, "A");

    TypeSubject::assert_type(result).is_same_instance_as(key2);
}
