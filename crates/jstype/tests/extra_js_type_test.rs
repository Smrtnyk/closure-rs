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

use closure_jstype::{
    js_type::{UnitTestingJSType, UnitTestingJSTypeData},
    named_type::{NamedTypeBuilder, ResolutionKind},
    prelude::*,
    testing::base_js_type_test_case::BaseJSTypeTestCase,
};

#[test]
fn extra_hash_recursion_flag_remains_set_after_exception() {
    let mut f = BaseJSTypeTestCase::new();
    let type_ = UnitTestingJSType::with_data(&mut f.reg, UnitTestingJSTypeData::default());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| type_.hash_code(&f.reg))).is_err()
    );
    assert_eq!(type_.hash_code(&f.reg), -1);
}

#[test]
fn extra_resolved_singleton_union_collapses_to_null() {
    let mut f = BaseJSTypeTestCase::new();
    let mut closer = f.reg.get_resolver().open_for_definition();
    let number_proxy = NamedTypeBuilder::new(&f.reg, "NumberProxy")
        .set_resolution_kind(ResolutionKind::NONE)
        .set_referenced_type(f.number_type)
        .build(&mut f.reg, &f.ast);
    let union = f
        .reg
        .create_union_type(&f.ast, &[f.number_type, number_proxy]);
    closer.close(&mut f.reg, &f.ast);
    assert_eq!(union.get_alternates(&mut f.reg, &f.ast).len(), 1);
    assert!(union.collapse_union(&mut f.reg, &f.ast).is_none());
}
