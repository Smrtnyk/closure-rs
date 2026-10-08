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
//   src/com/google/javascript/jscomp/colors/Color.java.

use super::{ColorId, standard_colors};
use crate::{check_state, js_string::JsString};
use indexmap::IndexSet;
use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::{Arc, OnceLock, atomic::Ordering},
};

#[derive(Clone)]
pub struct Color(Arc<ColorValue>);

struct ColorValue {
    id: ColorId,
    prototypes: IndexSet<Color>,
    instance_colors: IndexSet<Color>,
    invalidating: bool,
    properties_keep_original_name: bool,
    constructor: bool,
    own_properties: IndexSet<JsString>,
    box_id: Option<ColorId>,
    closure_assert: bool,
    union_elements: IndexSet<Color>,
    subtract_null_or_void: OnceLock<Color>,
    // Not in Java (AutoValue recomputes hashCode, but String caches its own hash): the value is
    // immutable, so its hash is computed once.
    hash_code: OnceLock<i32>,
}

impl Color {
    // port: Color#getId
    pub fn get_id(&self) -> ColorId {
        self.0.id
    }
    // port: Color#getPrototypes
    pub fn get_prototypes(&self) -> &IndexSet<Color> {
        &self.0.prototypes
    }
    // port: Color#getInstanceColors
    pub fn get_instance_colors(&self) -> &IndexSet<Color> {
        &self.0.instance_colors
    }
    // port: Color#isInvalidating
    pub fn is_invalidating(&self) -> bool {
        self.0.invalidating
    }
    // port: Color#getPropertiesKeepOriginalName
    pub fn get_properties_keep_original_name(&self) -> bool {
        self.0.properties_keep_original_name
    }
    // port: Color#isConstructor
    pub fn is_constructor(&self) -> bool {
        self.0.constructor
    }
    // port: Color#getOwnProperties
    pub fn get_own_properties(&self) -> &IndexSet<JsString> {
        &self.0.own_properties
    }
    // port: Color#getBoxId
    pub fn get_box_id(&self) -> Option<ColorId> {
        self.0.box_id
    }
    // port: Color#isClosureAssert
    pub fn is_closure_assert(&self) -> bool {
        self.0.closure_assert
    }
    // port: Color#getUnionElements
    pub fn get_union_elements(&self) -> &IndexSet<Color> {
        &self.0.union_elements
    }
    /// Java reference identity, used by Truth's isSameInstanceAs.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    // port: Color#singleBuilder
    pub fn single_builder() -> Builder {
        Builder::new()
            .set_closure_assert(false)
            .set_constructor(false)
            .set_instance_colors(IndexSet::new())
            .set_invalidating(false)
            .set_own_properties(IndexSet::new())
            .set_properties_keep_original_name(false)
            .set_prototypes(IndexSet::new())
            .set_union_elements(IndexSet::new())
    }

    // port: Color#createUnion
    pub fn create_union(elements: &IndexSet<Color>) -> Color {
        match elements.len() {
            0 => panic!(""),
            1 => return elements.first().unwrap().clone(),
            _ => {}
        }
        let mut instance_colors = IndexSet::new();
        let mut prototypes = IndexSet::new();
        let mut new_elements = IndexSet::new();
        let mut ids = IndexSet::new();
        let mut own_properties = IndexSet::new();
        let mut is_closure_assert = true;
        let mut is_constructor = true;
        let mut is_invalidating = false;
        let mut properties_keep_original_name = false;
        for element in elements {
            if element.is_union() {
                for nested_element in element.get_union_elements() {
                    new_elements.insert(nested_element.clone());
                    ids.insert(nested_element.get_id());
                }
            } else {
                new_elements.insert(element.clone());
                ids.insert(element.get_id());
            }
            instance_colors.extend(element.get_instance_colors().iter().cloned());
            is_closure_assert &= element.is_closure_assert();
            is_constructor &= element.is_constructor();
            is_invalidating |= element.is_invalidating();
            own_properties.extend(element.get_own_properties().iter().cloned());
            properties_keep_original_name |= element.get_properties_keep_original_name();
            prototypes.extend(element.get_prototypes().iter().cloned());
        }
        Builder::new()
            .set_closure_assert(is_closure_assert)
            .set_constructor(is_constructor)
            .set_id(ColorId::union(&ids))
            .set_instance_colors(instance_colors)
            .set_invalidating(is_invalidating)
            .set_own_properties(own_properties)
            .set_properties_keep_original_name(properties_keep_original_name)
            .set_prototypes(prototypes)
            .set_union_elements(new_elements)
            .build_union()
    }

    // port: Color#isPrimitive
    pub fn is_primitive(&self) -> bool {
        check_state!(!self.is_union(), "%s", self);
        standard_colors::PRIMITIVE_COLORS.contains_key(&self.get_id())
    }

    // port: Color#isUnion
    pub fn is_union(&self) -> bool {
        !self.get_union_elements().is_empty()
    }

    // port: Color#subtractNullOrVoid
    pub fn subtract_null_or_void(&self) -> Color {
        check_state!(self.is_union());
        if !self
            .get_union_elements()
            .contains(&*standard_colors::NULL_OR_VOID)
        {
            // Returning this must not store an Arc to itself in its memoized slot.
            return self.clone();
        }
        self.0
            .subtract_null_or_void
            .get_or_init(|| {
                let mut elements = self.get_union_elements().clone();
                elements.shift_remove(&*standard_colors::NULL_OR_VOID);
                Self::create_union(&elements)
            })
            .clone()
    }

    /// Rust-only: the `@Memoized` slot of `subtractNullOrVoid` as AutoValue's `AutoValue_Color`
    /// stores it (`None` until a union containing NULL_OR_VOID computed it), read by the
    /// unit-record replay's field dumps.
    pub fn replay_memoized_subtract_null_or_void(&self) -> Option<&Color> {
        self.0.subtract_null_or_void.get()
    }

    // port: AutoValue_Color#hashCode
    pub fn hash_code(&self) -> i32 {
        *self.0.hash_code.get_or_init(|| self.compute_hash_code())
    }
    fn compute_hash_code(&self) -> i32 {
        let mut h = 1i32;
        h = h.wrapping_mul(1000003) ^ self.get_id().hash_code();
        h = h.wrapping_mul(1000003)
            ^ self
                .get_prototypes()
                .iter()
                .fold(0i32, |h, x| h.wrapping_add(x.hash_code()));
        h = h.wrapping_mul(1000003)
            ^ self
                .get_instance_colors()
                .iter()
                .fold(0i32, |h, x| h.wrapping_add(x.hash_code()));
        h = h.wrapping_mul(1000003) ^ if self.is_invalidating() { 1231 } else { 1237 };
        h = h.wrapping_mul(1000003)
            ^ if self.get_properties_keep_original_name() {
                1231
            } else {
                1237
            };
        h = h.wrapping_mul(1000003) ^ if self.is_constructor() { 1231 } else { 1237 };
        h = h.wrapping_mul(1000003)
            ^ self
                .get_own_properties()
                .iter()
                .fold(0i32, |h, x| h.wrapping_add(x.hash_code()));
        h = h.wrapping_mul(1000003) ^ self.get_box_id().map_or(0, |id| id.hash_code());
        h = h.wrapping_mul(1000003) ^ if self.is_closure_assert() { 1231 } else { 1237 };
        h = h.wrapping_mul(1000003)
            ^ self
                .get_union_elements()
                .iter()
                .fold(0i32, |h, x| h.wrapping_add(x.hash_code()));
        h
    }
}

impl PartialEq for Color {
    // port: AutoValue_Color#equals
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
            || (self.0.id == other.0.id
                && self.0.prototypes == other.0.prototypes
                && self.0.instance_colors == other.0.instance_colors
                && self.0.invalidating == other.0.invalidating
                && self.0.properties_keep_original_name == other.0.properties_keep_original_name
                && self.0.constructor == other.0.constructor
                && self.0.own_properties == other.0.own_properties
                && self.0.box_id == other.0.box_id
                && self.0.closure_assert == other.0.closure_assert
                && self.0.union_elements == other.0.union_elements)
    }
}
impl Eq for Color {}
impl Hash for Color {
    // port: AutoValue_Color#hashCode
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_code().hash(state);
    }
}

struct SetDisplay<'a, T>(&'a IndexSet<T>);
impl<T: fmt::Display> fmt::Display for SetDisplay<'_, T> {
    // port: AbstractCollection#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (i, x) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{x}")?;
        }
        f.write_str("]")
    }
}
impl fmt::Display for Color {
    // port: AutoValue_Color#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Color{{id={}, prototypes={}, instanceColors={}, invalidating={}, propertiesKeepOriginalName={}, constructor={}, ownProperties={}, boxId={}, closureAssert={}, unionElements={}}}",
            self.get_id(),
            SetDisplay(self.get_prototypes()),
            SetDisplay(self.get_instance_colors()),
            self.is_invalidating(),
            self.get_properties_keep_original_name(),
            self.is_constructor(),
            SetDisplay(self.get_own_properties()),
            self.get_box_id()
                .map_or_else(|| "null".to_owned(), |id| id.to_string()),
            self.is_closure_assert(),
            SetDisplay(self.get_union_elements())
        )
    }
}
impl fmt::Debug for Color {
    // port: AutoValue_Color#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

pub struct Builder {
    id: Option<ColorId>,
    prototypes: Option<IndexSet<Color>>,
    instance_colors: Option<IndexSet<Color>>,
    invalidating: Option<bool>,
    properties_keep_original_name: Option<bool>,
    constructor: Option<bool>,
    own_properties: Option<IndexSet<JsString>>,
    box_id: Option<ColorId>,
    closure_assert: Option<bool>,
    union_elements: Option<IndexSet<Color>>,
}
impl Builder {
    // port: AutoValue_Color.Builder#Builder
    fn new() -> Self {
        Self {
            id: None,
            prototypes: None,
            instance_colors: None,
            invalidating: None,
            properties_keep_original_name: None,
            constructor: None,
            own_properties: None,
            box_id: None,
            closure_assert: None,
            union_elements: None,
        }
    }
    // port: AutoValue_Color.Builder#setId
    pub fn set_id(mut self, x: ColorId) -> Self {
        self.id = Some(x);
        self
    }
    // port: AutoValue_Color.Builder#setPrototypes
    pub fn set_prototypes(mut self, x: IndexSet<Color>) -> Self {
        self.prototypes = Some(x);
        self
    }
    // port: AutoValue_Color.Builder#setInstanceColors
    pub fn set_instance_colors(mut self, x: IndexSet<Color>) -> Self {
        self.instance_colors = Some(x);
        self
    }
    // port: AutoValue_Color.Builder#setInvalidating
    pub fn set_invalidating(mut self, x: bool) -> Self {
        self.invalidating = Some(x);
        self
    }
    // port: AutoValue_Color.Builder#setPropertiesKeepOriginalName
    pub fn set_properties_keep_original_name(mut self, x: bool) -> Self {
        self.properties_keep_original_name = Some(x);
        self
    }
    // port: AutoValue_Color.Builder#setConstructor
    pub fn set_constructor(mut self, x: bool) -> Self {
        self.constructor = Some(x);
        self
    }
    // port: AutoValue_Color.Builder#setOwnProperties
    pub fn set_own_properties(mut self, x: IndexSet<JsString>) -> Self {
        self.own_properties = Some(x);
        self
    }
    // port: AutoValue_Color.Builder#setBoxId
    pub(crate) fn set_box_id(mut self, x: Option<ColorId>) -> Self {
        self.box_id = x;
        self
    }
    // port: AutoValue_Color.Builder#setClosureAssert
    pub fn set_closure_assert(mut self, x: bool) -> Self {
        self.closure_assert = Some(x);
        self
    }
    // port: AutoValue_Color.Builder#setUnionElements
    pub(crate) fn set_union_elements(mut self, x: IndexSet<Color>) -> Self {
        self.union_elements = Some(x);
        self
    }
    // port: Color.Builder#setPrototype
    pub fn set_prototype(self, x: Option<Color>) -> Self {
        self.set_prototypes(x.into_iter().collect())
    }
    // port: Color.Builder#setInstanceColor
    pub fn set_instance_color(self, x: Option<Color>) -> Self {
        self.set_instance_colors(x.into_iter().collect())
    }
    // port: AutoValue_Color.Builder#buildInternal
    fn build_internal(self) -> Color {
        let mut missing = String::new();
        if self.id.is_none() {
            missing.push_str(" id");
        }
        if self.prototypes.is_none() {
            missing.push_str(" prototypes");
        }
        if self.instance_colors.is_none() {
            missing.push_str(" instanceColors");
        }
        if self.invalidating.is_none() {
            missing.push_str(" invalidating");
        }
        if self.properties_keep_original_name.is_none() {
            missing.push_str(" propertiesKeepOriginalName");
        }
        if self.constructor.is_none() {
            missing.push_str(" constructor");
        }
        if self.own_properties.is_none() {
            missing.push_str(" ownProperties");
        }
        if self.closure_assert.is_none() {
            missing.push_str(" closureAssert");
        }
        if self.union_elements.is_none() {
            missing.push_str(" unionElements");
        }
        check_state!(
            missing.is_empty(),
            "Missing required properties:%s",
            missing
        );
        Color(Arc::new(ColorValue {
            id: self.id.unwrap(),
            prototypes: self.prototypes.unwrap(),
            instance_colors: self.instance_colors.unwrap(),
            invalidating: self.invalidating.unwrap(),
            properties_keep_original_name: self.properties_keep_original_name.unwrap(),
            constructor: self.constructor.unwrap(),
            own_properties: self.own_properties.unwrap(),
            box_id: self.box_id,
            closure_assert: self.closure_assert.unwrap(),
            union_elements: self.union_elements.unwrap(),
            subtract_null_or_void: OnceLock::new(),
            hash_code: OnceLock::new(),
        }))
    }
    // port: Color.Builder#build
    pub fn build(self) -> Color {
        let result = self.build_internal();
        check_state!(result.get_union_elements().is_empty(), "%s", result);
        check_state!(result.get_box_id().is_none(), "%s", result);
        check_state!(
            !standard_colors::AXIOMATIC_COLORS.contains_key(&result.get_id()),
            "%s",
            result
        );
        result
    }
    // port: Color.Builder#buildUnion
    fn build_union(self) -> Color {
        let result = self.build_internal();
        check_state!(result.get_union_elements().len() > 1, "%s", result);
        check_state!(result.get_box_id().is_none(), "%s", result);
        result
    }
    // port: Color.Builder#buildAxiomatic
    pub(crate) fn build_axiomatic(self) -> Color {
        check_state!(
            !standard_colors::AXIOMATIC_COLORS_INITIALIZED.load(Ordering::Acquire),
            "StandardColors are all defined"
        );
        let result = self.build_internal();
        check_state!(result.get_union_elements().is_empty(), "%s", result);
        result
    }
}
