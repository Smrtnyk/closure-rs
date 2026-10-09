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
//   src/com/google/javascript/jscomp/colors/ColorRegistry.java.

use super::{Color, ColorId, standard_colors};
use crate::fx_hash::{IndexMap, IndexSet};
use crate::{check_not_null, check_state};
use std::sync::LazyLock;

pub struct ColorRegistry {
    native_colors: IndexMap<ColorId, Color>,
    color_to_disambiguation_supertype_graph: IndexMap<Color, IndexSet<Color>>,
    mismatch_locations: IndexMap<ColorId, IndexSet<String>>,
}

// port: ColorRegistry#REQUIRED_IDS
pub static REQUIRED_IDS: LazyLock<IndexSet<ColorId>> =
    LazyLock::new(|| standard_colors::STANDARD_OBJECT_IDS.clone());
static EMPTY_SUPERTYPES: LazyLock<IndexSet<Color>> = LazyLock::new(IndexSet::<_>::default);

impl ColorRegistry {
    // port: ColorRegistry#ColorRegistry
    fn new(builder: &Builder) -> Self {
        let result = Self {
            native_colors: builder.native_colors.clone(),
            color_to_disambiguation_supertype_graph: builder
                .color_to_disambiguation_supertype_graph
                .clone(),
            mismatch_locations: builder.mismatch_locations.clone(),
        };
        check_state!(
            result
                .native_colors
                .keys()
                .copied()
                .collect::<IndexSet<_>>()
                == *REQUIRED_IDS
        );
        result
    }
    // port: ColorRegistry#get
    pub fn get(&self, id: ColorId) -> Color {
        check_not_null!(self.native_colors.get(&id), "Missing Color for %s", id).clone()
    }
    // port: ColorRegistry#getDisambiguationSupertypes
    pub fn get_disambiguation_supertypes(&self, x: &Color) -> &IndexSet<Color> {
        self.color_to_disambiguation_supertype_graph
            .get(x)
            .unwrap_or(&EMPTY_SUPERTYPES)
    }
    // port: ColorRegistry#getMismatchLocationsForDebugging
    pub fn get_mismatch_locations_for_debugging(&self) -> &IndexMap<ColorId, IndexSet<String>> {
        &self.mismatch_locations
    }
    /// Rust-only: the private fields `nativeColors` and `colorToDisambiguationSupertypeGraph`,
    /// read by the unit-record replay's field dumps.
    pub fn replay_fields(&self) -> (&IndexMap<ColorId, Color>, &IndexMap<Color, IndexSet<Color>>) {
        (
            &self.native_colors,
            &self.color_to_disambiguation_supertype_graph,
        )
    }
    // port: ColorRegistry#builder
    pub fn builder() -> Builder {
        Builder::new()
    }
}

pub struct Builder {
    native_colors: IndexMap<ColorId, Color>,
    color_to_disambiguation_supertype_graph: IndexMap<Color, IndexSet<Color>>,
    mismatch_locations: IndexMap<ColorId, IndexSet<String>>,
}
impl Builder {
    // port: ColorRegistry.Builder#Builder
    fn new() -> Self {
        Self {
            native_colors: IndexMap::<_, _>::default(),
            color_to_disambiguation_supertype_graph: IndexMap::<_, _>::default(),
            mismatch_locations: IndexMap::<_, _>::default(),
        }
    }
    // port: ColorRegistry.Builder#setNativeColor
    pub fn set_native_color(&mut self, x: Color) -> &mut Self {
        check_state!(REQUIRED_IDS.contains(&x.get_id()), "%s", x);
        self.native_colors.insert(x.get_id(), x);
        self
    }
    // port: ColorRegistry.Builder#addDisambiguationEdge
    pub fn add_disambiguation_edge(&mut self, subtype: Color, supertype: Color) -> &mut Self {
        self.color_to_disambiguation_supertype_graph
            .entry(subtype)
            .or_default()
            .insert(supertype);
        self
    }
    // port: ColorRegistry.Builder#addMismatchLocation
    pub fn add_mismatch_location(&mut self, id: ColorId, location: String) -> &mut Self {
        self.mismatch_locations
            .entry(id)
            .or_default()
            .insert(location);
        self
    }
    // port: ColorRegistry.Builder#setDefaultNativeColorsForTesting
    pub fn set_default_native_colors_for_testing(&mut self) -> &mut Self {
        for id in REQUIRED_IDS.iter() {
            self.set_native_color(Color::single_builder().set_id(*id).build());
        }
        self
    }
    // port: ColorRegistry.Builder#build
    pub fn build(&self) -> ColorRegistry {
        ColorRegistry::new(self)
    }
}
