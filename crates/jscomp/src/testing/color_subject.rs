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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/testing/ColorSubject.java.

use crate::colors::{Color, ColorId, ColorRegistry};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;
use std::{fmt::Debug, hash::Hash, sync::LazyLock};

pub struct ColorSubject {
    actual: Option<Color>,
}
// port: ColorSubject#assertThat
pub fn assert_that(actual: impl Into<Option<Color>>) -> ColorSubject {
    ColorSubject::assert_that(actual)
}
impl ColorSubject {
    // port: ColorSubject#ColorSubject
    fn new(actual: Option<Color>) -> Self {
        Self { actual }
    }
    // port: ColorSubject#colors
    pub fn colors() -> fn(Option<Color>) -> Self {
        Self::new
    }
    // port: ColorSubject#assertThat
    pub fn assert_that(actual: impl Into<Option<Color>>) -> Self {
        Self::colors()(actual.into())
    }
    // port: ColorSubject#actualNonNull
    fn actual_non_null(&self) -> &Color {
        self.actual.as_ref().expect("isNotNull()")
    }
    // port: ColorSubject#hasId
    pub fn has_id(&self, id: ColorId) {
        assert_eq!(self.actual_non_null().get_id(), id, "getId");
    }
    // port: ColorSubject#isUnion
    pub fn is_union(&self) {
        assert!(self.actual_non_null().is_union(), "isUnion()");
    }
    // port: ColorSubject#isPrimitive
    pub fn is_primitive(&self) {
        assert!(self.actual_non_null().is_primitive(), "isPrimitive()");
    }
    // port: ColorSubject#isInvalidating
    pub fn is_invalidating(&self) {
        assert!(self.actual_non_null().is_invalidating(), "isInvalidating");
    }
    // port: ColorSubject#isNotInvalidating
    pub fn is_not_invalidating(&self) {
        assert!(!self.actual_non_null().is_invalidating(), "isInvalidating");
    }
    // port: ColorSubject#isClosureAssert
    pub fn is_closure_assert(&self) {
        assert!(
            self.actual_non_null().is_closure_assert(),
            "isClosureAssert()"
        );
    }
    // port: ColorSubject#isConstructor
    pub fn is_constructor(&self) {
        assert!(self.actual_non_null().is_constructor(), "isConstructor()");
    }
    // port: ColorSubject#isEqualTo
    pub fn is_equal_to_subject(&self, other_subject: &ColorSubject) -> &Self {
        assert_eq!(self.actual, other_subject.actual, "isEqualTo");
        self
    }
    // port: Subject#isEqualTo
    pub fn is_equal_to(&self, other: &Color) -> &Self {
        assert_eq!(self.actual_non_null(), other, "isEqualTo");
        self
    }
    // port: Subject#isNotEqualTo
    pub fn is_not_equal_to(&self, other: &Color) -> &Self {
        assert_ne!(self.actual_non_null(), other, "isNotEqualTo");
        self
    }
    // port: Subject#isSameInstanceAs
    pub fn is_same_instance_as(&self, other: &Color) -> &Self {
        assert!(self.actual_non_null().ptr_eq(other), "isSameInstanceAs");
        self
    }
    // port: ColorSubject#propertiesKeepOriginalName
    pub fn properties_keep_original_name(&self) {
        assert!(
            self.actual_non_null().get_properties_keep_original_name(),
            "propertiesKeepOriginalName"
        );
    }
    // port: ColorSubject#hasOwnPropertiesSetThat
    pub fn has_own_properties_set_that(&self) -> IterableSubject<'_, JsString> {
        IterableSubject {
            actual: self.actual_non_null().get_own_properties(),
            check: "getOwnProperties()".into(),
        }
    }
    // port: ColorSubject#hasAlternates
    pub fn has_alternates(&self, alternates: &[Color]) {
        self.is_union();
        IterableSubject {
            actual: self.actual_non_null().get_union_elements(),
            check: "getAlternates().containsExactly()".into(),
        }
        .contains_exactly(alternates);
    }
    // port: ColorSubject#hasDisambiguationSupertypesThat
    pub fn has_disambiguation_supertypes_that<'a>(
        &self,
        registry: &'a ColorRegistry,
    ) -> IterableSubject<'a, Color> {
        static EMPTY: LazyLock<IndexSet<Color>> = LazyLock::new(IndexSet::<_>::default);
        IterableSubject {
            actual: self.actual.as_ref().map_or(&*EMPTY, |actual| {
                registry.get_disambiguation_supertypes(actual)
            }),
            check: format!(
                "registry.getDisambiguationSupertypes({})",
                self.actual
                    .as_ref()
                    .map_or_else(|| "null".into(), Color::to_string)
            ),
        }
    }
}

pub struct IterableSubject<'a, T> {
    actual: &'a IndexSet<T>,
    check: String,
}
impl<T: Eq + Hash + Debug> IterableSubject<'_, T> {
    // port: IterableSubject#containsExactly
    pub fn contains_exactly(&self, expected: &[T]) -> &Self {
        // Truth checks multiplicity too, so a repeated expected value must fail.
        assert_eq!(self.actual.len(), expected.len(), "{}", self.check);
        assert!(
            expected.iter().all(|x| self.actual.contains(x))
                && self.actual.iter().all(|x| expected.contains(x)),
            "{}",
            self.check
        );
        self
    }
    // port: IterableSubject#isEmpty
    pub fn is_empty(&self) {
        assert!(self.actual.is_empty(), "{}", self.check);
    }
    // port: IterableSubject#hasSize
    pub fn has_size(&self, size: usize) {
        assert_eq!(self.actual.len(), size, "{}", self.check);
    }
}
