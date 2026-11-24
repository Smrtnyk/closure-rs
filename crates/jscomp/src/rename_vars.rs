/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/RenameVars.java.

//! Port of RenameVars.java: renames all the variables names into short names, to reduce code size
//! and also to obfuscate the code.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    make_declared_names_unique::ContextualRenameInverter,
    name_generator::{NameGenerator, ReservedNames},
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    scope::ScopeId,
    var::VarId,
    variable_map::VariableMap,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_state, js_string::JsString, node::NodeId, rhino_string_pool::RhinoStringPool,
};
use std::{
    cmp::Ordering,
    sync::{Arc, RwLock},
};

/// RenameVars renames all the variables names into short names, to reduce code size and also to
/// obfuscate the code.
pub struct RenameVars {
    /// List of global NAME nodes and their assignment indices.
    global_name_nodes: Vec<(NodeId, u32)>,
    /// List of local NAME nodes and their temporary assignment indices.
    /// Rust-only (performance divergence): avoids writing temporary `"L <n>"` strings to the AST
    /// and avoids a separate per-node `original_name_by_node` hash map.
    local_name_nodes: Vec<(NodeId, u32)>,
    /// Cache mapping temporary local index `i` to its slot in `assignments`.
    local_temp_assignments: Vec<Option<usize>>,
    /// Maps a name node to its pseudo name, null if we are not generating so there will be no
    /// overhead unless we are debugging.
    pseudo_name_map: Option<IndexMap<NodeId, JsString>>,
    /// Set of extern variable names
    extern_names: IndexSet<JsString>,
    /// Set of reserved variable names
    reserved_names: ReservedNames,
    /// The renaming map
    rename_map: IndexMap<JsString, JsString>,
    /// The previously used rename map.
    prev_used_rename_map: Option<Arc<VariableMap>>,
    /// The global name prefix
    prefix: JsString,
    /// Counter for each assignment
    assignment_count: i32,
    // Logic for bleeding functions, where the name leaks into the outer
    // scope on IE but not on other browsers.
    local_bleeding_functions: IndexSet<VarId>,
    local_bleeding_functions_per_scope: IndexMap<ScopeId, Vec<VarId>>,
    /// Maps an old name to a new name assignment
    assignments: IndexMap<JsString, Assignment>,
    /// Whether renaming should apply to local variables only.
    local_renaming_only: bool,
    prefer_stable_names: bool,
    /// Characters that shouldn't be used in variable names.
    reserved_characters: IndexSet<u16>,
    // Shared name generator
    name_generator: Box<dyn NameGenerator>,
}

/// Rust-only: borrowed `RenameVars` fields for the test harness (Java reflection).
pub struct RenameVarsReplayFields<'a> {
    pub prefix: &'a JsString,
    pub assignment_count: i32,
    pub local_renaming_only: bool,
    pub prefer_stable_names: bool,
    pub name_generator: &'a dyn NameGenerator,
}

pub struct Assignment {
    is_local: bool,
    old_name: JsString,
    order_of_occurrence: i32,
    new_name: Option<JsString>,
    count: i32, // Number of times this is referenced
}

impl Assignment {
    // port: RenameVars.Assignment#Assignment
    fn new(name: JsString, assignment_count: &mut i32) -> Self {
        let is_local = name.starts_with(JsString::from(RenameVars::LOCAL_VAR_PREFIX));
        // Represents the order at which a symbol appears in the source.
        let order_of_occurrence = *assignment_count;
        *assignment_count += 1;
        Self {
            is_local,
            old_name: name,
            new_name: None,
            count: 0,
            order_of_occurrence,
        }
    }

    /// Assigns the new name.
    // port: RenameVars.Assignment#setNewName
    fn set_new_name(&mut self, new_name: JsString) {
        check_state!(self.new_name.is_none());
        self.new_name = Some(new_name);
    }
}

impl RenameVars {
    /// Limit on number of locals in a scope for temporary local renaming when
    /// `preferStableNames` is true.
    // port: RenameVars#MAX_LOCALS_IN_SCOPE_TO_TEMP_RENAME
    const MAX_LOCALS_IN_SCOPE_TO_TEMP_RENAME: i32 = 1000;

    /// A prefix to distinguish temporary local names from global names
    // port: RenameVars#LOCAL_VAR_PREFIX
    const LOCAL_VAR_PREFIX: &'static str = "L ";

    /// nameGenerator is a shared NameGenerator that this instance can use; the instance may reset
    /// or reconfigure it, so the caller should not expect any state to be preserved.
    // port: RenameVars#RenameVars
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        prefix: Option<JsString>,
        local_renaming_only: bool,
        generate_pseudo_names: bool,
        prefer_stable_names: bool,
        prev_used_rename_map: Option<Arc<VariableMap>>,
        reserved_characters: IndexSet<u16>,
        reserved_names: Option<IndexSet<JsString>>,
        name_generator: Box<dyn NameGenerator>,
    ) -> Self {
        let pseudo_name_map = if generate_pseudo_names {
            Some(IndexMap::<_, _>::default())
        } else {
            None
        };
        let reserved_names = reserved_names.unwrap_or_default();
        Self {
            global_name_nodes: Vec::new(),
            local_name_nodes: Vec::new(),
            local_temp_assignments: Vec::new(),
            pseudo_name_map,
            extern_names: IndexSet::<_>::default(),
            reserved_names: Arc::new(RwLock::new(reserved_names)),
            rename_map: IndexMap::<_, _>::default(),
            prev_used_rename_map,
            prefix: prefix.unwrap_or_else(|| JsString::from("")),
            assignment_count: 0,
            local_bleeding_functions: IndexSet::<_>::default(),
            local_bleeding_functions_per_scope: IndexMap::<_, _>::default(),
            assignments: IndexMap::<_, _>::default(),
            local_renaming_only,
            prefer_stable_names,
            reserved_characters,
            name_generator,
        }
    }

    /// Sorts Assignment objects by their count, breaking ties by their order of occurrence in the
    /// source to ensure a deterministic total ordering.
    // port: RenameVars#frequencyComparator
    fn frequency_comparator(a1: &Assignment, a2: &Assignment) -> i32 {
        if a1.count != a2.count {
            return a2.count.wrapping_sub(a1.count);
        }
        // Break a tie using the order in which the variable first appears in
        // the source.
        Self::order_of_occurrence_comparator(a1, a2)
    }

    /// Sorts Assignment objects by the order the variable name first appears in the source.
    // port: RenameVars#ORDER_OF_OCCURRENCE_COMPARATOR
    fn order_of_occurrence_comparator(a1: &Assignment, a2: &Assignment) -> i32 {
        match a1.order_of_occurrence.cmp(&a2.order_of_occurrence) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }

    /// Builds the iteration order of a `TreeSet<Assignment>` with the given comparator over the
    /// given assignments (by index into `assignments`).
    fn tree_set(
        assignments: &IndexMap<JsString, Assignment>,
        indices: impl IntoIterator<Item = usize>,
        comparator: fn(&Assignment, &Assignment) -> i32,
    ) -> Vec<usize> {
        // Rust-only (performance divergence): O(N log N) sort + dedup instead of O(N^2) Vec::insert.
        let mut set: Vec<usize> = indices.into_iter().collect();
        set.sort_by(|&a, &b| comparator(&assignments[a], &assignments[b]).cmp(&0));
        set.dedup_by(|&mut b, &mut a| comparator(&assignments[a], &assignments[b]) == 0);
        set
    }

    // port: RenameVars#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.extern_names = NodeUtil::collect_extern_variable_names(compiler, externs);

        // Do variable reference counting.
        {
            let mut reserved_names = self.reserved_names.write().unwrap();
            let mut visitor = ProcessVars {
                global_name_nodes: &mut self.global_name_nodes,
                local_name_nodes: &mut self.local_name_nodes,
                local_temp_assignments: &mut self.local_temp_assignments,
                pseudo_name_map: &mut self.pseudo_name_map,
                reserved_names: &mut reserved_names,
                assignment_count: &mut self.assignment_count,
                local_bleeding_functions: &mut self.local_bleeding_functions,
                local_bleeding_functions_per_scope: &mut self.local_bleeding_functions_per_scope,
                assignments: &mut self.assignments,
                local_renaming_only: self.local_renaming_only,
                prefer_stable_names: self.prefer_stable_names,
            };
            NodeTraversal::traverse(compiler, root, &mut visitor);

            // Make sure that new names don't overlap with extern names.
            reserved_names.extend(self.extern_names.iter().cloned());
        }

        // Rename vars, sorted by frequency of occurrence to minimize code size.
        let vars_by_frequency = Self::tree_set(
            &self.assignments,
            0..self.assignments.len(),
            Self::frequency_comparator,
        );

        // First try to reuse names from an earlier compilation.
        if self.prev_used_rename_map.is_some() {
            self.reuse_previously_used_variable_map(&vars_by_frequency);
        }

        // Assign names, sorted by descending frequency to minimize code size.
        self.assign_names(&vars_by_frequency);

        // Rename the globals!
        for i in 0..self.global_name_nodes.len() {
            let (n, assign_idx) = self.global_name_nodes[i];
            let new_name = self.get_new_global_name(n, assign_idx as usize);
            self.set_name_and_report(compiler, n, new_name);
        }

        // Rename the locals!
        for i in 0..self.local_name_nodes.len() {
            let (n, assign_idx) = self.local_name_nodes[i];
            let new_name = self.get_new_local_name(n, assign_idx as usize);
            if new_name.is_some() {
                self.set_name_and_report(compiler, n, new_name);
            } else {
                // Edge case: `prev_used_rename_map` mapped the temp name to itself;
                // Java leaves the temporary name on the node without reporting a change.
                let temp_name = self.assignments[assign_idx as usize].old_name.clone();
                if temp_name != *n.get_string_ref(compiler) {
                    n.set_string(compiler, temp_name);
                }
            }
        }
    }

    // port: RenameVars#setNameAndReport
    fn set_name_and_report(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        new_name: Option<JsString>,
    ) {
        // A null newName, indicates it should not be renamed.
        // Because local nodes keep their original spelling until final renaming,
        // `new_name != *n.get_string_ref(compiler)` checks both whether the node
        // needs updating and whether the final name differs from the original.
        if let Some(new_name) = new_name
            && new_name != *n.get_string_ref(compiler)
        {
            n.set_string(compiler, new_name);
            compiler.report_change_to_enclosing_scope(n);
            let parent = n.get_parent(compiler).unwrap();
            if parent.is_function(compiler) && NodeUtil::is_function_declaration(compiler, parent) {
                // If we are renaming a function declaration, make sure the containing scope
                // has the opportunity to act on the change.
                compiler.report_change_to_enclosing_scope(parent);
            }
        }
    }

    // port: RenameVars#getNewGlobalName
    fn get_new_global_name(&self, n: NodeId, assign_idx: usize) -> Option<JsString> {
        let a = &self.assignments[assign_idx];
        if let Some(new_name) = &a.new_name
            && *new_name != a.old_name
        {
            if let Some(pseudo_name_map) = &self.pseudo_name_map {
                return pseudo_name_map.get(&n).cloned();
            }
            Some(new_name.clone())
        } else {
            None
        }
    }

    // port: RenameVars#getNewLocalName
    fn get_new_local_name(&self, n: NodeId, assign_idx: usize) -> Option<JsString> {
        let a = &self.assignments[assign_idx];
        if *a.new_name.as_ref().unwrap() != a.old_name {
            if let Some(pseudo_name_map) = &self.pseudo_name_map {
                return pseudo_name_map.get(&n).cloned();
            }
            return a.new_name.clone();
        }
        None
    }

    // port: RenameVars#recordPseudoName
    fn record_pseudo_name(
        pseudo_name_map: &mut IndexMap<NodeId, JsString>,
        compiler: &AbstractCompiler,
        n: NodeId,
    ) {
        // Variable names should be in a different name space than
        // property pseudo names.
        let pseudo_name = JsString::from("$")
            .concat(n.get_string_ref(compiler))
            .concat(&JsString::from("$$"));
        pseudo_name_map.insert(n, pseudo_name);
    }

    /// Runs through the assignments and reuses as many names as possible from the previously used
    /// variable map. Updates reservedNames with the set of names that were reused.
    // port: RenameVars#reusePreviouslyUsedVariableMap
    fn reuse_previously_used_variable_map(&mut self, vars_to_rename: &[usize]) {
        let prev_used_rename_map = self.prev_used_rename_map.clone().unwrap();
        // If prevUsedRenameMap had duplicate values then this pass would be
        // non-deterministic.
        // In such a case, the following will throw an IllegalArgumentException.
        prev_used_rename_map.get_new_name_to_original_name_map();
        for &a in vars_to_rename {
            let prev_new_name = prev_used_rename_map.lookup_new_name(&self.assignments[a].old_name);
            let Some(prev_new_name) = prev_new_name else {
                continue;
            };
            if self.reserved_names.read().unwrap().contains(&prev_new_name) {
                continue;
            }

            if self.assignments[a].is_local
                || (!self.extern_names.contains(&self.assignments[a].old_name)
                    && prev_new_name.starts_with(&self.prefix))
            {
                self.reserved_names
                    .write()
                    .unwrap()
                    .insert(prev_new_name.clone());
                self.finalize_name_assignment(a, prev_new_name);
            }
        }
    }

    /// Determines which new names to substitute for the original names.
    // port: RenameVars#assignNames
    fn assign_names(&mut self, vars_to_rename: &[usize]) {
        self.name_generator.reset(
            self.reserved_names.clone(),
            self.prefix.clone(),
            &self.reserved_characters,
        );

        // Local variables never need a prefix.
        // Also, we need to avoid conflicts between global and local variable
        // names; we do this by having using the same generator (not two
        // instances). The case where global variables have a prefix (and
        // therefore we use two different generators) but a local variable name
        // might nevertheless conflict with a global one is not handled.
        // (`None` stands for "the same generator as globalNameGenerator".)
        let mut local_name_generator: Option<Box<dyn NameGenerator>> = if self.prefix.is_empty() {
            None
        } else {
            Some(self.name_generator.clone(
                self.reserved_names.clone(),
                JsString::from(""),
                &self.reserved_characters,
            ))
        };

        // Generated names and the assignments for non-local vars.
        let mut pending_assignments: Vec<usize> = Vec::new();
        let mut generated_names_for_assignments: Vec<JsString> = Vec::new();

        for &a in vars_to_rename {
            if self.assignments[a].new_name.is_some() {
                continue;
            }

            if self.extern_names.contains(&self.assignments[a].old_name) {
                continue;
            }

            let new_name;
            if self.assignments[a].is_local {
                // For local variable, we make the assignment right away.
                new_name = match &mut local_name_generator {
                    Some(local_name_generator) => local_name_generator.generate_next_name(),
                    None => self.name_generator.generate_next_name(),
                };
                self.finalize_name_assignment(a, new_name.clone());
            } else {
                // For non-local variable, delay finalizing the name assignment
                // until we know how many new names we'll have of length 2, 3, etc.
                new_name = self.name_generator.generate_next_name();
                pending_assignments.push(a);
                generated_names_for_assignments.push(new_name.clone());
            }
            self.reserved_names.write().unwrap().insert(new_name);
        }

        // Now that we have a list of generated names, and a list of variable
        // Assignment objects, we assign the generated names to the vars as
        // follows:
        // 1) The most frequent vars get the shorter names.
        // 2) If N number of vars are going to be assigned names of the same
        //    length, we assign the N names based on the order at which the vars
        //    first appear in the source. This makes the output somewhat less
        //    random, because symbols declared close together are assigned names
        //    that are quite similar. With this heuristic, the output is more
        //    compressible.
        //    For instance, the output may look like:
        //    var da = "..", ea = "..";
        //    function fa() { .. } function ga() { .. }

        let num_pending_assignments = generated_names_for_assignments.len();
        let mut i = 0;
        while i < num_pending_assignments {
            // Add k number of Assignment to the set, where k is the number of
            // generated names of the same length.
            let len = generated_names_for_assignments[i].length();
            let mut same_length = Vec::new();
            let mut j = i;
            while j < num_pending_assignments && generated_names_for_assignments[j].length() == len
            {
                same_length.push(pending_assignments[j]);
                j += 1;
            }
            let vars_by_order_of_occurrence = Self::tree_set(
                &self.assignments,
                same_length,
                Self::order_of_occurrence_comparator,
            );

            // Now, make the assignments
            for a in vars_by_order_of_occurrence {
                self.finalize_name_assignment(a, generated_names_for_assignments[i].clone());
                i += 1;
            }
        }
    }

    /// Makes a final name assignment.
    // port: RenameVars#finalizeNameAssignment
    fn finalize_name_assignment(&mut self, a: usize, new_name: JsString) {
        // Rust-only (performance divergence): intern once so every renamed node shares the handle.
        let new_name = RhinoStringPool::add_or_get(new_name);
        self.assignments[a].set_new_name(new_name.clone());

        // Keep track of the mapping
        let old_name = self.assignments[a].old_name.clone();
        self.rename_map.insert(old_name, new_name);
    }

    /// Gets the variable map.
    // port: RenameVars#getVariableMap
    pub fn get_variable_map(&self) -> VariableMap {
        VariableMap::new(&self.rename_map)
    }

    /// Rust-only: the field values the test harness reads reflectively (Java tests share the
    /// `nameGenerator` object with the pass and inspect it after the run).
    pub fn replay_fields(&self) -> RenameVarsReplayFields<'_> {
        RenameVarsReplayFields {
            prefix: &self.prefix,
            assignment_count: self.assignment_count,
            local_renaming_only: self.local_renaming_only,
            prefer_stable_names: self.prefer_stable_names,
            name_generator: &*self.name_generator,
        }
    }

    /// Determines whether a variable name is okay to rename.
    // port: RenameVars#okToRenameVar
    fn ok_to_rename_var(compiler: &AbstractCompiler, name: &JsString, is_local: bool) -> bool {
        !compiler
            .get_coding_convention()
            .is_exported(name, /* local= */ is_local)
    }

    /// Returns the index within the scope stack.
    /// e.g. function Foo(a) { var b; function c(d) { } }
    /// a = 0, b = 1, c = 2, d = 3
    // port: RenameVars#getLocalVarIndex
    fn get_local_var_index(
        local_bleeding_functions: &IndexSet<VarId>,
        local_bleeding_functions_per_scope: &IndexMap<ScopeId, Vec<VarId>>,
        prefer_stable_names: bool,
        compiler: &AbstractCompiler,
        v: VarId,
    ) -> i32 {
        let mut num = v.get_index(compiler);
        let Some(mut s) = v.get_scope(compiler).get_parent(compiler) else {
            panic!("Var is not local");
        };

        let mut is_bleeding_into_scope =
            s.get_parent(compiler).is_some() && local_bleeding_functions.contains(&v);

        while s.get_parent(compiler).is_some() {
            let bleeding_functions = local_bleeding_functions_per_scope
                .get(&s)
                .map(Vec::as_slice)
                .unwrap_or_default();
            if is_bleeding_into_scope {
                let index_of = bleeding_functions
                    .iter()
                    .position(|f| *f == v)
                    .map_or(-1, |i| i as i32);
                num += index_of + 1;
                is_bleeding_into_scope = false;
            } else {
                num += bleeding_functions.len() as i32;
            }
            if Self::should_temporarily_rename_locals_in_scope(prefer_stable_names, compiler, s) {
                num += s.get_var_count(compiler);
            }
            s = s.get_parent(compiler).unwrap();
        }
        num
    }

    /// Returns true if the local variables in a scope should be given temporary names (eg, 'L 123')
    /// prior to renaming to allow reuse of names across scopes. With `preferStableNames`,
    /// temporary renaming is disabled if the number of locals in the scope is above a heuristic
    /// threshold to allow effective reuse of rename maps (see `prevUsedRenameMap`). In scopes with
    /// many variables the temporary name given to a variable is unlikely to be the same temporary
    /// name used when the rename map was created.
    // port: RenameVars#shouldTemporarilyRenameLocalsInScope
    fn should_temporarily_rename_locals_in_scope(
        prefer_stable_names: bool,
        compiler: &AbstractCompiler,
        s: ScopeId,
    ) -> bool {
        !prefer_stable_names
            || s.get_var_count(compiler) <= Self::MAX_LOCALS_IN_SCOPE_TO_TEMP_RENAME
    }

    // Increment count of an assignment
    // port: RenameVars.ProcessVars#incCount
    fn inc_count_ref(
        assignments: &mut IndexMap<JsString, Assignment>,
        assignment_count: &mut i32,
        name: &JsString,
    ) -> usize {
        if let Some((idx, _, s)) = assignments.get_full_mut(name) {
            s.count += 1;
            return idx;
        }
        let owned = name.clone();
        let (idx, _) =
            assignments.insert_full(owned.clone(), Assignment::new(owned, assignment_count));
        assignments[idx].count += 1;
        idx
    }

    /// Rust-only (performance divergence): cache temporary `"L <index>"` assignment slots by
    /// integer index so local references avoid formatting and hashing a string per node.
    fn inc_local_temp_count(
        assignments: &mut IndexMap<JsString, Assignment>,
        assignment_count: &mut i32,
        local_temp_assignments: &mut Vec<Option<usize>>,
        index: i32,
    ) -> usize {
        if let Ok(slot) = usize::try_from(index) {
            if let Some(Some(assign_idx)) = local_temp_assignments.get(slot).copied() {
                assignments[assign_idx].count += 1;
                return assign_idx;
            }
            let temp_name = JsString::from(format!("{}{}", Self::LOCAL_VAR_PREFIX, index));
            let assign_idx = Self::inc_count_ref(assignments, assignment_count, &temp_name);
            if local_temp_assignments.len() <= slot {
                local_temp_assignments.resize(slot + 1, None);
            }
            local_temp_assignments[slot] = Some(assign_idx);
            return assign_idx;
        }
        let temp_name = JsString::from(format!("{}{}", Self::LOCAL_VAR_PREFIX, index));
        Self::inc_count_ref(assignments, assignment_count, &temp_name)
    }
}

impl CompilerPass for RenameVars {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        RenameVars::process(self, compiler, externs, root);
    }
}

/// Iterate through the nodes, collect all the NAME nodes that need to be renamed, and count how
/// many times each variable name is referenced.
///
/// Keep track of all name references in globalNameNodes, and localNameNodes.
///
/// To get shorter local variable renaming, we rename local variables to a temporary name
/// "LOCAL_VAR_PREFIX + index" where index is the index of the variable declared in the local scope
/// stack. e.g.
/// Foo(fa, fb) {
///   var c = function(d, e) { return fa; }
/// }
/// The indexes are: fa:0, fb:1, c:2, d:3, e:4
///
/// In that way, local variable names are reused in each global function. e.g. the final code might
/// look like
/// function x(a,b) { ... }
/// function y(a,b,c) { ... }
struct ProcessVars<'a> {
    global_name_nodes: &'a mut Vec<(NodeId, u32)>,
    local_name_nodes: &'a mut Vec<(NodeId, u32)>,
    local_temp_assignments: &'a mut Vec<Option<usize>>,
    pseudo_name_map: &'a mut Option<IndexMap<NodeId, JsString>>,
    reserved_names: &'a mut IndexSet<JsString>,
    assignment_count: &'a mut i32,
    local_bleeding_functions: &'a mut IndexSet<VarId>,
    local_bleeding_functions_per_scope: &'a mut IndexMap<ScopeId, Vec<VarId>>,
    assignments: &'a mut IndexMap<JsString, Assignment>,
    local_renaming_only: bool,
    prefer_stable_names: bool,
}

impl Callback for ProcessVars<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RenameVars.ProcessVars#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if !(n.is_name(t) || n.is_import_star(t)) {
            return;
        }

        // Ignore anonymous functions and classes.
        if n.get_string_ref(t).is_empty() {
            return;
        }

        // "import {x as y} from 'm';"
        // Skip x because it's not a variable in this scope.
        let parent = parent.unwrap();
        if parent.is_import_spec(t)
            && parent.has_two_children(t)
            && parent.get_first_child(t) == Some(n)
        {
            return;
        }

        // Is this local or Global?
        // Bleeding functions should be treated as part of their outer
        // scope, because IE has bugs in how it handles bleeding
        // functions.
        let scope = t.get_scope();
        let var = scope.get_var_of_node(t.get_compiler(), n);
        let local = {
            let compiler = t.get_compiler();
            var.is_some_and(|var| {
                var.is_local(compiler)
                    && (var
                        .get_scope(compiler)
                        .get_parent(compiler)
                        .unwrap()
                        .is_local(compiler)
                        || !var.is_bleeding_function(compiler))
            })
        };

        // Never rename references to the arguments array
        if var.is_some_and(|var| var.is_arguments(t.get_compiler())) {
            let name = n.get_string_ref(t);
            if !self.reserved_names.contains(name) {
                self.reserved_names.insert(name.clone());
            }
            return;
        }

        // Are we renaming global variables?
        if !local && self.local_renaming_only {
            let name = n.get_string_ref(t);
            if !self.reserved_names.contains(name) {
                self.reserved_names.insert(name.clone());
            }
            return;
        }

        // Check if we can rename this.
        let ok_to_rename = {
            let compiler = t.get_compiler();
            let name = n.get_string_ref(compiler);
            RenameVars::ok_to_rename_var(compiler, name, local)
        };
        if !ok_to_rename {
            if local {
                // Blindly de-uniquify for the Prototype library for
                // http://blickly.github.io/closure-compiler-issues/#103
                let name = n.get_string(t);
                let new_name = ContextualRenameInverter::get_original_name(&name);
                if new_name != name {
                    n.set_string(t, new_name);
                }
            }
            return;
        }

        if let Some(pseudo_name_map) = self.pseudo_name_map.as_mut() {
            RenameVars::record_pseudo_name(pseudo_name_map, t.get_compiler(), n);
        }

        if local && {
            let compiler = t.get_compiler();
            let var_scope = var.unwrap().get_scope(compiler);
            RenameVars::should_temporarily_rename_locals_in_scope(
                self.prefer_stable_names,
                compiler,
                var_scope,
            )
        } {
            // Give local variables a temporary name based on the
            // variable's index in the scope to enable name reuse across
            // locals in independent scopes.
            let index = RenameVars::get_local_var_index(
                self.local_bleeding_functions,
                self.local_bleeding_functions_per_scope,
                self.prefer_stable_names,
                t.get_compiler(),
                var.unwrap(),
            );
            let assign_idx = RenameVars::inc_local_temp_count(
                self.assignments,
                self.assignment_count,
                self.local_temp_assignments,
                index,
            );
            self.local_name_nodes.push((n, assign_idx as u32));
        } else if var.is_some() {
            // Not an extern
            // If it's global, increment global count
            let assign_idx = RenameVars::inc_count_ref(
                self.assignments,
                self.assignment_count,
                n.get_string_ref(t),
            );
            self.global_name_nodes.push((n, assign_idx as u32));
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for ProcessVars<'_> {
    // port: RenameVars.ProcessVars#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_global_hoist_scope() || {
            let scope = t.get_scope();
            !RenameVars::should_temporarily_rename_locals_in_scope(
                self.prefer_stable_names,
                t.get_compiler(),
                scope,
            )
        } {
            return;
        }
        let scope = t.get_scope();
        for current in scope.get_var_iterable(t.get_compiler()) {
            if current.is_bleeding_function(t.get_compiler()) {
                self.local_bleeding_functions.insert(current);
                let parent = scope.get_parent(t.get_compiler()).unwrap();
                self.local_bleeding_functions_per_scope
                    .entry(parent)
                    .or_default()
                    .push(current);
            }
        }
    }

    // port: RenameVars.ProcessVars#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}
