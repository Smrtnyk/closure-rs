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
//   src/com/google/javascript/jscomp/disambiguate/ColorGraphNode.java.

//! Port of `disambiguate/ColorGraphNode.java`, plus the Rust-only arena that owns the
//! package's `ColorGraphNode` and `PropertyClustering` objects.

use super::property_clustering::PropertyClustering;
use crate::colors::{Color, standard_colors};
use closure_rhino::java_util::bit_set::BitSet;
use closure_rhino::{check_argument, check_not_null};
use indexmap::IndexMap;
use std::fmt;

/// Rust-only owner of the disambiguate package's object web. Java's `ColorGraphNode` and
/// `PropertyClustering` objects reference each other (a node's associated properties, a
/// property's clusters and use sites); here both live in this arena and are addressed by
/// identity handles, so `==` on a handle is Java's reference equality.
#[derive(Default)]
pub struct DisambiguateArena {
    pub(crate) nodes: Vec<ColorGraphNode>,
    pub(crate) props: Vec<PropertyClustering>,
}

impl DisambiguateArena {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rust-only: `a.getSubtypeIndices()` and `b.getSubtypeIndices()` at once, for
    /// `b.getSubtypeIndices().or(a.getSubtypeIndices())`. `None` when `a == b` (Java's `BitSet#or`
    /// then returns at once).
    pub fn subtype_indices_pair_mut(
        &mut self,
        a: ColorGraphNodeId,
        b: ColorGraphNodeId,
    ) -> (&BitSet, Option<&mut BitSet>) {
        let (a, b) = (a.0 as usize, b.0 as usize);
        if a == b {
            return (&self.nodes[a].subtype_indices, None);
        }
        if a < b {
            let (low, high) = self.nodes.split_at_mut(b);
            (&low[a].subtype_indices, Some(&mut high[0].subtype_indices))
        } else {
            let (low, high) = self.nodes.split_at_mut(a);
            (&high[0].subtype_indices, Some(&mut low[b].subtype_indices))
        }
    }
}

/// Java object identity of a [`ColorGraphNode`] in a [`DisambiguateArena`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct ColorGraphNodeId(u32);

/// Java object identity of a [`PropertyClustering`] in a [`DisambiguateArena`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct PropertyClusteringId(pub(crate) u32);

impl fmt::Display for ColorGraphNodeId {
    /// Graph value strings for error messages; Java prints `ColorGraphNode#toString`, which
    /// needs the arena (see [`ColorGraphNodeId::to_string_in`]).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ColorGraphNode#{}", self.0)
    }
}

/// A struct representing a {@link Color} for use in ambiguation.
///
/// Each instance pairs a Color with additional information computed by/for optimizations.
///
/// Note: this design now depends on the implementation of Color to preserve invariants about
/// recursive Colors. The node factory can't be depended on for post-processing Colors without
/// losing type safety.
pub struct ColorGraphNode {
    color: Color,
    associated_props: IndexMap<PropertyClusteringId, PropAssociation>,
    index: i32,
    subtype_indices: BitSet,
}

/// Reasons a property name became associated with a type.
///
/// This information is only used for debugging. It doesn't affect the behaviour of the pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropAssociation {
    /// Because of an access in the AST (e.g. foo.prop)
    AST,
    /// Because the type system recorded such an association, without any reason visible in the
    /// AST. Usually this means the relevant AST segment has been optimized away by an earlier pass.
    TYPE_SYSTEM,
    /// Because it was inherited from a supertype in the type graph
    SUPERTYPE,
}

impl PropAssociation {
    // port: ColorGraphNode.PropAssociation#name
    pub fn name(self) -> &'static str {
        match self {
            PropAssociation::AST => "AST",
            PropAssociation::TYPE_SYSTEM => "TYPE_SYSTEM",
            PropAssociation::SUPERTYPE => "SUPERTYPE",
        }
    }
}

impl ColorGraphNode {
    // port: ColorGraphNode#create
    pub fn create(arena: &mut DisambiguateArena, single: Color, index: i32) -> ColorGraphNodeId {
        check_argument!(index >= 0);
        Self::new(arena, single, index)
    }

    // port: ColorGraphNode#createForTesting(int)
    pub fn create_for_testing(arena: &mut DisambiguateArena, index: i32) -> ColorGraphNodeId {
        check_argument!(index < 0); // All test nodes have negative indexs to differentiate them.
        Self::new(arena, standard_colors::UNKNOWN.clone(), index)
    }

    // port: ColorGraphNode#createForTesting(Color,int)
    pub fn create_for_testing_with_color(
        arena: &mut DisambiguateArena,
        single: Color,
        index: i32,
    ) -> ColorGraphNodeId {
        check_argument!(index < 0); // All test nodes have negative indexs to differentiate them.
        Self::new(arena, single, index)
    }

    // port: ColorGraphNode#ColorGraphNode
    #[allow(clippy::new_ret_no_self)] // The arena owns the object; the handle is its identity.
    fn new(arena: &mut DisambiguateArena, single: Color, index: i32) -> ColorGraphNodeId {
        let id = ColorGraphNodeId(arena.nodes.len() as u32);
        arena.nodes.push(ColorGraphNode {
            color: single,
            associated_props: IndexMap::new(),
            index,
            subtype_indices: BitSet::new_default(),
        });
        id
    }
}

impl ColorGraphNodeId {
    // port: ColorGraphNode#getColor
    pub fn get_color(self, arena: &DisambiguateArena) -> &Color {
        check_not_null!(Some(&arena.nodes[self.0 as usize].color))
    }

    /// An ID used to efficiently construct a unique name for any cluster this node becomes the
    /// represenative of.
    // port: ColorGraphNode#getIndex
    pub fn get_index(self, arena: &DisambiguateArena) -> i32 {
        arena.nodes[self.0 as usize].index
    }

    /// The set of properties that that might be accessed from this type.
    // port: ColorGraphNode#getAssociatedProps
    pub fn get_associated_props(
        self,
        arena: &DisambiguateArena,
    ) -> &IndexMap<PropertyClusteringId, PropAssociation> {
        &arena.nodes[self.0 as usize].associated_props
    }

    // port: ColorGraphNode#getAssociatedProps
    pub fn get_associated_props_mut(
        self,
        arena: &mut DisambiguateArena,
    ) -> &mut IndexMap<PropertyClusteringId, PropAssociation> {
        &mut arena.nodes[self.0 as usize].associated_props
    }

    /// The IDs of other ColorGraphNodes that have been found to be subtypes of this type.
    // port: ColorGraphNode#getSubtypeIndices
    pub fn get_subtype_indices(self, arena: &DisambiguateArena) -> &BitSet {
        &arena.nodes[self.0 as usize].subtype_indices
    }

    // port: ColorGraphNode#getSubtypeIndices
    pub fn get_subtype_indices_mut(self, arena: &mut DisambiguateArena) -> &mut BitSet {
        &mut arena.nodes[self.0 as usize].subtype_indices
    }

    /// For debugging only.
    // port: ColorGraphNode#toString
    pub fn to_string_in(self, arena: &DisambiguateArena) -> String {
        // Just report a few important properties of color instead of the whole thing.
        // The string generated here will become the label of the `.dot` graph node
        // if this graph is logged for debugging. Including the entire color, recursively
        // includes all the colors it extends also, making the labels unmanageably long
        // to display and repeating information available from other nodes.
        let node = &arena.nodes[self.0 as usize];
        let own_properties = node
            .color
            .get_own_properties()
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "ColorGraphNode{{index={}, color.id={}, color.ownProperties=[{}]}}",
            node.index,
            node.color.get_id(),
            own_properties
        )
    }
}
