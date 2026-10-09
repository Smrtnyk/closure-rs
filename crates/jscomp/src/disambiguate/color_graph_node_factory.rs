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
//   src/com/google/javascript/jscomp/disambiguate/ColorGraphNodeFactory.java.

//! Port of `disambiguate/ColorGraphNodeFactory.java`.

use super::color_graph_node::{ColorGraphNode, ColorGraphNodeId, DisambiguateArena};
use crate::colors::{Color, color_registry::ColorRegistry, standard_colors};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use std::sync::Arc;

/// The overridable instance methods of {@link ColorGraphNodeFactory} (Java virtual dispatch:
/// ColorFindPropertyReferencesTest subclasses the factory with a stub). Callers that Java types
/// as `ColorGraphNodeFactory` but that a subclass may be handed to take `&mut dyn` of this trait.
pub trait ColorGraphNodeFactoryMethods {
    // port: ColorGraphNodeFactory#createNode
    fn create_node(
        &mut self,
        arena: &mut DisambiguateArena,
        color: Option<&Color>,
    ) -> ColorGraphNodeId;

    // port: ColorGraphNodeFactory#getAllKnownTypes
    fn get_all_known_types(&self) -> IndexSet<ColorGraphNodeId>;
}

impl ColorGraphNodeFactoryMethods for ColorGraphNodeFactory {
    // port: ColorGraphNodeFactory#createNode
    fn create_node(
        &mut self,
        arena: &mut DisambiguateArena,
        color: Option<&Color>,
    ) -> ColorGraphNodeId {
        ColorGraphNodeFactory::create_node(self, arena, color)
    }

    // port: ColorGraphNodeFactory#getAllKnownTypes
    fn get_all_known_types(&self) -> IndexSet<ColorGraphNodeId> {
        ColorGraphNodeFactory::get_all_known_types(self)
    }
}

/// A factory and cache for {@link ColorGraphNode} instances.
pub struct ColorGraphNodeFactory {
    type_index: IndexMap<Color, ColorGraphNodeId>,
    registry: Arc<ColorRegistry>,
}

impl ColorGraphNodeFactory {
    // port: ColorGraphNodeFactory#ColorGraphNodeFactory
    pub fn new(
        initial_type_index: IndexMap<Color, ColorGraphNodeId>,
        registry: Arc<ColorRegistry>,
    ) -> Self {
        Self {
            type_index: initial_type_index,
            registry,
        }
    }

    // port: ColorGraphNodeFactory#createFactory
    pub fn create_factory(
        arena: &mut DisambiguateArena,
        color_registry: Arc<ColorRegistry>,
    ) -> ColorGraphNodeFactory {
        let mut type_index = IndexMap::<_, _>::default();
        let unknown_color_node = ColorGraphNode::create(arena, standard_colors::UNKNOWN.clone(), 0);
        type_index.insert(standard_colors::UNKNOWN.clone(), unknown_color_node);
        ColorGraphNodeFactory::new(type_index, color_registry)
    }

    /// Returns the {@link ColorGraphNode} known by this factory for {@code type}.
    ///
    /// For a given {@code type} and factory, this method will always return the same result. The
    /// results are cached.
    // port: ColorGraphNodeFactory#createNode
    pub fn create_node(
        &mut self,
        arena: &mut DisambiguateArena,
        color: Option<&Color>,
    ) -> ColorGraphNodeId {
        let key = self.simplify_color(color);
        if let Some(node) = self.type_index.get(&key) {
            return *node;
        }
        let node = self.new_color_graph_node(arena, key.clone());
        self.type_index.insert(key, node);
        node
    }

    // port: ColorGraphNodeFactory#getAllKnownTypes
    pub fn get_all_known_types(&self) -> IndexSet<ColorGraphNodeId> {
        self.type_index.values().copied().collect()
    }

    /// Rust-only: the private fields `typeIndex` and `registry`, read by the unit-record replay
    /// (UnitRecorder dumps of the factory).
    pub fn replay_fields(&self) -> (&IndexMap<Color, ColorGraphNodeId>, &Arc<ColorRegistry>) {
        (&self.type_index, &self.registry)
    }

    // port: ColorGraphNodeFactory#newColorGraphNode
    fn new_color_graph_node(&self, arena: &mut DisambiguateArena, key: Color) -> ColorGraphNodeId {
        let id = self.type_index.len() as i32;
        ColorGraphNode::create(arena, key, id)
    }

    /// Merges different colors with the same ambiguation-behavior into one
    // port: ColorGraphNodeFactory#simplifyColor
    fn simplify_color(&self, r#type: Option<&Color>) -> Color {
        let Some(r#type) = r#type else {
            return standard_colors::UNKNOWN.clone();
        };

        if r#type.is_union() {
            // First remove null/void, then recursively simplify any primitive components
            let r#type = r#type.subtract_null_or_void();
            if r#type.is_union() {
                Color::create_union(
                    &r#type
                        .get_union_elements()
                        .iter()
                        .map(|e| self.simplify_color(Some(e)))
                        .collect::<IndexSet<_>>(),
                )
            } else {
                self.simplify_color(Some(&r#type))
            }
        } else if let Some(box_id) = r#type.get_box_id() {
            self.registry.get(box_id)
        } else if *r#type == *standard_colors::NULL_OR_VOID {
            standard_colors::UNKNOWN.clone()
        } else {
            r#type.clone()
        }
    }
}
