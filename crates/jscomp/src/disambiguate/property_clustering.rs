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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/disambiguate/PropertyClustering.java.

//! Port of `disambiguate/PropertyClustering.java`.

use super::{
    color_graph_node::{ColorGraphNodeId, DisambiguateArena, PropertyClusteringId},
    invalidation::Invalidation,
};
use crate::graph::{standard_union_find::StandardUnionFind, union_find::UnionFind};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{check_not_null, check_state, js_string::JsString, node::NodeId};

/// The disambiguation clusters for a given property name.
///
/// This is a struct used to aggregate information to be processed by other classes in this
/// package. It is intentionally mutable and doesn't attempt to enforce invariants in the contents
/// of its datastructures. Instances trust that sibling classes make mutations correctly.
pub struct PropertyClustering {
    name: JsString,
    use_sites: IndexMap<NodeId, ColorGraphNodeId>,
    clusters: StandardUnionFind<ColorGraphNodeId>,
    original_name_cluster_rep: Option<ColorGraphNodeId>,
    last_invalidation: Option<Invalidation>,
}

impl PropertyClustering {
    // port: PropertyClustering#PropertyClustering
    #[allow(clippy::new_ret_no_self)] // The arena owns the object; the handle is its identity.
    pub fn new(arena: &mut DisambiguateArena, name: JsString) -> PropertyClusteringId {
        let id = PropertyClusteringId(arena.props.len() as u32);
        arena.props.push(PropertyClustering {
            name,
            use_sites: IndexMap::<_, _>::default(),
            clusters: StandardUnionFind::new(),
            original_name_cluster_rep: None,
            last_invalidation: None,
        });
        id
    }
}

impl PropertyClusteringId {
    fn data(self, arena: &DisambiguateArena) -> &PropertyClustering {
        &arena.props[self.0 as usize]
    }

    fn data_mut(self, arena: &mut DisambiguateArena) -> &mut PropertyClustering {
        &mut arena.props[self.0 as usize]
    }

    // port: PropertyClustering#getName
    pub fn get_name(self, arena: &DisambiguateArena) -> &JsString {
        &self.data(arena).name
    }

    /// The locations properties with this name were used in the program, mapping to their
    /// receiver type.
    ///
    /// This index allows property references to be efficiently renamed once all clusters have
    /// been found. It prevents us from re-traversing the code.
    // port: PropertyClustering#getUseSites
    pub fn get_use_sites(self, arena: &DisambiguateArena) -> &IndexMap<NodeId, ColorGraphNodeId> {
        &self.data(arena).use_sites
    }

    // port: PropertyClustering#getUseSites
    pub fn get_use_sites_mut(
        self,
        arena: &mut DisambiguateArena,
    ) -> &mut IndexMap<NodeId, ColorGraphNodeId> {
        &mut self.data_mut(arena).use_sites
    }

    // port: PropertyClustering#getClusters
    pub fn get_clusters(self, arena: &DisambiguateArena) -> &StandardUnionFind<ColorGraphNodeId> {
        &self.data(arena).clusters
    }

    // port: PropertyClustering#getClusters
    pub fn get_clusters_mut(
        self,
        arena: &mut DisambiguateArena,
    ) -> &mut StandardUnionFind<ColorGraphNodeId> {
        &mut self.data_mut(arena).clusters
    }

    /// The current representative of the cluster of types whose properties must keep their
    /// original name.
    ///
    /// The following refers to all computations with respect to a single property name. Since
    /// extern and enum properties cannot be renamed, all other types in a cluster with an extern
    /// type or enum cannot rename their property either. In theory, there could be many such
    /// clusters containing an extern or enum; however, in practice we conflate them into one.
    /// This is equivalent because all of those clusters would end up using the same, unchanged,
    /// property name. This representation also simplifies tracking of such clusters.
    ///
    /// In practice the types in this cluster include
    ///
    /// - externs, whose properties cannot be renamed without breaking references to external code
    /// - enums, e.g. for const Actions = {STOP: 0, GO: 1}; the disambiguator will not rename STOP
    ///   or GO
    /// - boxable scalars, like string and number
    ///
    /// Note that enum properties could probably be safely renamed, but this would require cleaning
    /// up code depending on the legacy no-renaming behavior.
    ///
    /// `&mut` because `StandardUnionFind#find` compresses paths.
    // port: PropertyClustering#getOriginalNameClusterRep
    pub fn get_original_name_cluster_rep(
        self,
        arena: &mut DisambiguateArena,
    ) -> Option<ColorGraphNodeId> {
        check_state!(!self.is_invalidated(arena));
        let data = self.data_mut(arena);
        match data.original_name_cluster_rep {
            None => None,
            Some(rep) => Some(data.clusters.find(&rep)),
        }
    }

    /// Rust-only: the private fields `originalNameClusterRep` and `lastInvalidation` as stored
    /// (no `find`, no precondition), read by the unit-record replay's field dumps.
    pub fn replay_fields(
        self,
        arena: &DisambiguateArena,
    ) -> (Option<ColorGraphNodeId>, Option<Invalidation>) {
        let data = self.data(arena);
        (data.original_name_cluster_rep, data.last_invalidation)
    }

    // port: PropertyClustering#isInvalidated
    pub fn is_invalidated(self, arena: &DisambiguateArena) -> bool {
        self.data(arena).last_invalidation.is_some()
    }

    // port: PropertyClustering#invalidate
    pub fn invalidate(self, arena: &mut DisambiguateArena, invalidation: Invalidation) {
        self.data_mut(arena).last_invalidation = Some(check_not_null!(Some(invalidation)));
    }

    // port: PropertyClustering#getLastInvalidation
    pub fn get_last_invalidation(self, arena: &DisambiguateArena) -> Invalidation {
        check_state!(self.is_invalidated(arena));
        self.data(arena).last_invalidation.unwrap()
    }

    /// Indicate that all property references off this {@link ColorGraphNode} must keep their
    /// original name.
    ///
    /// See {@link #getOriginalNameClusterRep()} for more details.
    // port: PropertyClustering#registerOriginalNameType
    pub fn register_original_name_type(
        self,
        arena: &mut DisambiguateArena,
        r#type: ColorGraphNodeId,
    ) {
        check_state!(!self.is_invalidated(arena));
        let data = self.data_mut(arena);
        if data.original_name_cluster_rep.is_none() {
            data.original_name_cluster_rep = Some(r#type);
        }
        data.clusters
            .union(data.original_name_cluster_rep.unwrap(), r#type);
    }

    /// For debugging only.
    // port: PropertyClustering#toString
    pub fn to_string_in(self, arena: &DisambiguateArena) -> String {
        format!("PropertyClustering{{name={}}}", self.get_name(arena))
    }
}
