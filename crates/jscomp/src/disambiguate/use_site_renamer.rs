/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/disambiguate/UseSiteRenamer.java.

//! Port of `disambiguate/UseSiteRenamer.java`.

use super::color_graph_node::{ColorGraphNodeId, DisambiguateArena, PropertyClusteringId};
use crate::{graph::union_find::UnionFind, node_util::AstContext};
use closure_rhino::{js_string::JsString, node::NodeId};
use indexmap::{IndexMap, IndexSet};

// port: UseSiteRenamer#INVALIDATED_NAME_VALUE
const INVALIDATED_NAME_VALUE: &str = "<INVALIDATED>";

/// The mutation callback (Java `Consumer<Node>`). It receives the arena owner the renamer
/// mutates, so a compiler can report the change (`compiler::reportChangeToEnclosingScope`).
pub type MutationCb<'a, C> = Box<dyn FnMut(&mut C, NodeId) + 'a>;

/// Java `ImmutableSetMultimap<String, String>` (keys and values in first-insertion order).
pub type RenamingIndex = IndexMap<JsString, IndexSet<JsString>>;

/// Applies renaming to property use sites following cluster computation.
pub struct UseSiteRenamer<'a, C: AstContext + ?Sized> {
    mutation_cb: MutationCb<'a, C>,
    renaming_index: RenamingIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameUsesResult {
    INVALIDATED,
    ONLY_ONE_CLUSTER,
    DISAMBIGUATED,
}

impl<'a, C: AstContext + ?Sized> UseSiteRenamer<'a, C> {
    // port: UseSiteRenamer#UseSiteRenamer
    pub fn new(mutation_cb: MutationCb<'a, C>) -> Self {
        Self {
            mutation_cb,
            renaming_index: IndexMap::new(),
        }
    }

    // Java: ImmutableSetMultimap.Builder#put
    fn put_renaming(&mut self, key: JsString, value: JsString) {
        self.renaming_index.entry(key).or_default().insert(value);
    }

    /// Renames all references to {@code prop}.
    ///
    /// If {@code prop} is invalid or should otherwise not be renamed, the AST will not be
    /// changed.
    // port: UseSiteRenamer#renameUses
    pub fn rename_uses(
        &mut self,
        ctx: &mut C,
        arena: &mut DisambiguateArena,
        prop: PropertyClusteringId,
    ) -> RenameUsesResult {
        if prop.is_invalidated(arena) {
            self.put_renaming(
                prop.get_name(arena).clone(),
                JsString::from(INVALIDATED_NAME_VALUE),
            );
            return RenameUsesResult::INVALIDATED;
        }

        let cluster_names = Self::create_all_cluster_names(arena, prop);

        if cluster_names.len() <= 1 {
            /*
             * Don't bother renaming clusters with a single element. Renaming won't actaully
             * disambiguate anything in this case, so skip the work.
             */
            let name = prop.get_name(arena).clone();
            self.put_renaming(name.clone(), name);
            return RenameUsesResult::ONLY_ONE_CLUSTER;
        }

        for value in cluster_names.values() {
            self.put_renaming(prop.get_name(arena).clone(), value.clone());
        }
        let use_sites = prop
            .get_use_sites(arena)
            .iter()
            .map(|(site, flat)| (*site, *flat))
            .collect::<Vec<_>>();
        for (site, usage_value) in use_sites {
            let flat_rep = prop.get_clusters_mut(arena).find(&usage_value);
            let new_name = cluster_names.get(&flat_rep);
            if new_name != Some(&site.get_string(ctx.ast())) {
                site.set_string(ctx.ast_mut(), new_name.unwrap().clone());
                (self.mutation_cb)(ctx, site);
            }
        }
        RenameUsesResult::DISAMBIGUATED
    }

    // port: UseSiteRenamer#getRenamingIndex
    pub fn get_renaming_index(&self) -> RenamingIndex {
        self.renaming_index.clone()
    }

    /// Creates a unique name for each cluster in {@code prop} and maps it to the cluster
    /// representative.
    // port: UseSiteRenamer#createAllClusterNames
    fn create_all_cluster_names(
        arena: &mut DisambiguateArena,
        prop: PropertyClusteringId,
    ) -> IndexMap<ColorGraphNodeId, JsString> {
        let mut result = IndexMap::new();
        for r in prop.get_clusters(arena).all_representatives() {
            let name = Self::create_cluster_name(arena, prop, r);
            // toImmutableMap rejects duplicate keys; allRepresentatives is a set.
            result.insert(r, name);
        }
        result
    }

    // port: UseSiteRenamer#createClusterName
    fn create_cluster_name(
        arena: &mut DisambiguateArena,
        prop: PropertyClusteringId,
        rep: ColorGraphNodeId,
    ) -> JsString {
        if prop.get_original_name_cluster_rep(arena) == Some(rep) {
            return prop.get_name(arena).clone();
        }

        JsString::from(format!("JSC${}_", rep.get_index(arena))).concat(prop.get_name(arena))
    }
}
