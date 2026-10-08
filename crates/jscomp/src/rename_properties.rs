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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/RenameProperties.java.

//! Port of RenameProperties.java.
//!
//! RenameProperties renames properties (including methods) of all JavaScript objects. This
//! includes prototypes, functions, object literals, etc.
//!
//! If provided a VariableMap of previously used names, it tries to reuse those names.
//!
//! To prevent a property from getting renamed you may extern it (add it to your externs file) or
//! put it in quotes.
//!
//! To avoid run-time JavaScript errors, use quotes when accessing properties that are defined
//! using quotes.
//!
//! ```text
//!   var a = {'myprop': 0}, b = a['myprop'];  // correct
//!   var x = {'myprop': 0}, y = x.myprop;     // incorrect
//! ```
//!
//! This pass also recognizes and replaces special renaming functions. They supply a property name
//! as the string literal for the first argument. This pass will replace them as though they were
//! JS property references. Here are two examples: JSCompiler_renameProperty('propertyName') ->
//! 'jYq' JSCompiler_renameProperty('myProp.nestedProp.innerProp') -> 'e4.sW.C$'
//!
//! This class is not thread-safe.
use crate::{
    AbstractCompiler,
    abstract_compiler::LifeCycleStage,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    gather_getter_and_setter_properties::GatherGetterAndSetterProperties,
    name_generator::{NameGenerator, ReservedNames},
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    variable_map::VariableMap,
};
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
    token_stream::TokenStream,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::{Arc, RwLock};

/// Filter function for property names. The function takes a node as input and returns true if the
/// node should be renamed.
pub type PropertyRenameEligibilityFilter = Box<dyn Fn(&Ast, NodeId) -> bool>;

// port: RenameProperties#BAD_CALL
pub static BAD_CALL: DiagnosticType = DiagnosticType::error(
    "JSC_BAD_RENAME_PROPERTY_FUNCTION_NAME_CALL",
    "Bad {0} call - the first argument must be a string literal",
);

// port: RenameProperties#BAD_ARG
pub static BAD_ARG: DiagnosticType = DiagnosticType::error(
    "JSC_BAD_RENAME_PROPERTY_FUNCTION_NAME_ARG",
    "Bad {0} argument - ''{1}'' is not a valid JavaScript identifier",
);

/// Rust-only: the field values the test harness reads reflectively (UnitRecorder walks the pass
/// object's fields; see RenameVars#replay_fields).
pub struct RenamePropertiesReplayFields<'a> {
    pub generate_pseudo_names: bool,
    pub prev_used_property_map: Option<&'a Arc<VariableMap>>,
    pub reserved_first_characters: &'a IndexSet<u16>,
    pub reserved_non_first_characters: &'a IndexSet<u16>,
    pub externed_names: &'a IndexSet<JsString>,
    pub quoted_names: &'a IndexSet<JsString>,
    pub name_generator: &'a dyn NameGenerator,
}

pub struct RenameProperties {
    generate_pseudo_names: bool,
    /// Property renaming map from a previous compilation.
    prev_used_property_map: Option<Arc<VariableMap>>,
    to_remove: Vec<NodeId>,
    string_nodes_to_rename: Vec<NodeId>,
    call_node_to_parent_map: IndexMap<NodeId, NodeId>,
    reserved_first_characters: IndexSet<u16>,
    reserved_non_first_characters: IndexSet<u16>,
    // Map from property name to Property object
    property_map: IndexMap<JsString, Property>,
    // Property names that don't get renamed
    externed_names: IndexSet<JsString>,
    // Names to which properties shouldn't be renamed, to avoid name conflicts
    quoted_names: IndexSet<JsString>,
    // Shared name generator
    name_generator: Box<dyn NameGenerator>,
    // Filter function for property names. The function takes a node as input and returns true if
    // the node should be renamed.
    property_rename_eligibility_filter: PropertyRenameEligibilityFilter,
    // Property names that should not be renamed, based on the propertyNameFilter. Unlike
    // externedNames, this set is dynamically constructed during the pass.
    filtered_out_names: IndexSet<JsString>,
}

/// `Splitter.on('.').split(s)`: every '.' separates, empty strings are kept.
// port: RenameProperties#DOT_SPLITTER
fn dot_splitter_split(s: &JsString) -> Vec<JsString> {
    s.as_units()
        .split(|&c| c == u16::from(b'.'))
        .map(JsString::from_units)
        .collect()
}

impl RenameProperties {
    // port: RenameProperties#FREQUENCY_COMPARATOR
    fn frequency_comparator(p1: &Property, p2: &Property) -> i32 {
        // First a frequently used names would always be picked first.
        if p1.num_occurrences != p2.num_occurrences {
            return p2.num_occurrences.wrapping_sub(p1.num_occurrences);
        }

        // Finally, for determinism, we compare them based on the old name.
        p1.old_name.compare_to(&p2.old_name)
    }

    /// Creates an instance.
    ///
    /// * `compiler` The JSCompiler.
    /// * `generate_pseudo_names` Generate pseudo names. e.g foo -> $foo$ instead of compact
    ///   obfuscated names. This is used for debugging.
    /// * `prev_used_property_map` The property renaming map used in a previous compilation.
    /// * `reserved_first_characters` If specified these characters won't be used in generated
    ///   names for the first character
    /// * `reserved_non_first_characters` If specified these characters won't be used in generated
    ///   names for characters after the first
    /// * `name_generator` a shared NameGenerator that this instance can use; the instance may
    ///   reset or reconfigure it, so the caller should not expect any state to be preserved
    // port: RenameProperties#RenameProperties(AbstractCompiler,boolean,VariableMap,Set,Set,NameGenerator)
    pub fn new(
        compiler: &AbstractCompiler,
        generate_pseudo_names: bool,
        prev_used_property_map: Option<Arc<VariableMap>>,
        reserved_first_characters: IndexSet<u16>,
        reserved_non_first_characters: IndexSet<u16>,
        name_generator: Box<dyn NameGenerator>,
    ) -> Self {
        Self::new_with_filter(
            compiler,
            generate_pseudo_names,
            prev_used_property_map,
            reserved_first_characters,
            reserved_non_first_characters,
            name_generator,
            Box::new(|_: &Ast, _: NodeId| true),
        )
    }

    /// Creates an instance.
    ///
    /// `property_rename_eligibility_filter` limits the nodes that are renamed using a predicate
    /// function. When this returns true, the property name is renamed if and only if all other
    /// property references for the same name match the predicate. If this returns false for any
    /// instance of a property, then no property references anywhere in the AST to that property
    /// are renamed
    // port: RenameProperties#RenameProperties(AbstractCompiler,boolean,VariableMap,Set,Set,NameGenerator,Predicate)
    pub fn new_with_filter(
        compiler: &AbstractCompiler,
        generate_pseudo_names: bool,
        prev_used_property_map: Option<Arc<VariableMap>>,
        reserved_first_characters: IndexSet<u16>,
        reserved_non_first_characters: IndexSet<u16>,
        name_generator: Box<dyn NameGenerator>,
        property_rename_eligibility_filter: PropertyRenameEligibilityFilter,
    ) -> Self {
        let mut externed_names = IndexSet::new();
        externed_names.insert(JsString::from("prototype"));
        externed_names.extend(
            check_not_null!(compiler.get_extern_properties())
                .iter()
                .map(|name| JsString::from(name.as_str())),
        );
        Self {
            generate_pseudo_names,
            prev_used_property_map,
            to_remove: Vec::new(),
            string_nodes_to_rename: Vec::new(),
            call_node_to_parent_map: IndexMap::new(),
            reserved_first_characters,
            reserved_non_first_characters,
            property_map: IndexMap::new(),
            externed_names,
            quoted_names: IndexSet::new(),
            name_generator,
            property_rename_eligibility_filter,
            filtered_out_names: IndexSet::new(),
        }
    }

    // port: RenameProperties#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());

        NodeTraversal::traverse(compiler, root, &mut ProcessProperties { this: self });

        let mut reserved_names: IndexSet<JsString> =
            IndexSet::with_capacity(self.externed_names.len() + self.quoted_names.len());
        reserved_names.extend(self.externed_names.iter().cloned());
        reserved_names.extend(self.quoted_names.iter().cloned());
        let reserved_names: ReservedNames = Arc::new(RwLock::new(reserved_names));

        // Assign names, sorted by descending frequency to minimize code size.
        let mut props_by_freq: Vec<usize> = Vec::new();
        for i in 0..self.property_map.len() {
            // TreeSet.add: a comparator-equal element is not added again.
            let property_map = &self.property_map;
            if let Err(pos) = props_by_freq.binary_search_by(|&probe| {
                Self::frequency_comparator(&property_map[probe], &property_map[i]).cmp(&0)
            }) {
                props_by_freq.insert(pos, i);
            }
        }

        // First, try and reuse as many property names from the previous compilation
        // as possible.
        if self.prev_used_property_map.is_some() {
            self.reuse_property_names(&reserved_names, &props_by_freq);
        }

        self.generate_names(&props_by_freq, &reserved_names);

        // Update the string nodes.
        for n in self.string_nodes_to_rename.clone() {
            let old_name = n.get_string(compiler);
            if self.filtered_out_names.contains(&old_name) {
                continue;
            }
            let p = self.property_map.get(&old_name);
            if let Some(p) = p
                && let Some(new_name) = &p.new_name
            {
                check_state!(old_name == p.old_name);
                n.set_string(compiler, new_name.clone());
                if *new_name != old_name {
                    compiler.report_change_to_enclosing_scope(n);
                }
            }
        }

        // Update the call nodes.
        for (call, parent) in self.call_node_to_parent_map.clone() {
            let first_arg = call.get_second_child(compiler).unwrap();
            let mut sb: Vec<u16> = Vec::new();
            for old_name in dot_splitter_split(&first_arg.get_string(compiler)) {
                let p = self.property_map.get(&old_name);
                let replacement;
                if let Some(p) = p
                    && let Some(new_name) = &p.new_name
                {
                    check_state!(old_name == p.old_name);
                    if self.filtered_out_names.contains(&old_name) {
                        replacement = old_name;
                    } else {
                        replacement = new_name.clone();
                    }
                } else {
                    replacement = old_name;
                }
                if !sb.is_empty() {
                    sb.push(u16::from(b'.'));
                }
                sb.extend_from_slice(replacement.as_units());
            }
            let string = IR::string(compiler, JsString::from_units(sb));
            call.replace_with(compiler, string);
            compiler.report_change_to_enclosing_scope(parent);
        }

        // Complete queued removals.
        for n in self.to_remove.clone() {
            let parent = n.get_parent(compiler).unwrap();
            compiler.report_change_to_enclosing_scope(n);
            n.detach(compiler);
            NodeUtil::mark_functions_deleted(compiler, n);
            if !parent.has_children(compiler) && !parent.is_script(compiler) {
                parent.detach(compiler);
            }
        }

        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED_OBFUSCATED);
        // This pass may rename getter or setter properties
        GatherGetterAndSetterProperties::update(compiler, externs, root);
    }

    /// Runs through the list of properties and renames as many as possible with names from the
    /// previous compilation. Also, updates reservedNames with the set of reused names.
    // port: RenameProperties#reusePropertyNames
    fn reuse_property_names(&mut self, reserved_names: &ReservedNames, all_props: &[usize]) {
        let prev_used_property_map = self.prev_used_property_map.clone().unwrap();
        for &prop in all_props {
            // Check if this node can reuse a name from a previous compilation - if
            // it can set the newName for the property too.
            let prev_name =
                prev_used_property_map.lookup_new_name(&self.property_map[prop].old_name);
            if !self.generate_pseudo_names
                && let Some(prev_name) = prev_name
            {
                // We can reuse prevName if it's not reserved.
                if reserved_names.read().unwrap().contains(&prev_name) {
                    continue;
                }

                self.property_map[prop].new_name = Some(prev_name.clone());
                reserved_names.write().unwrap().insert(prev_name);
            }
        }
    }

    /// Generates new names for properties.
    // port: RenameProperties#generateNames
    fn generate_names(&mut self, props: &[usize], reserved_names: &ReservedNames) {
        self.name_generator
            .reset_with_first_and_non_first_characters(
                reserved_names.clone(),
                JsString::from(""),
                &self.reserved_first_characters,
                &self.reserved_non_first_characters,
            );
        for &p in props {
            if self.generate_pseudo_names {
                let new_name = JsString::from("$")
                    .concat(&self.property_map[p].old_name)
                    .concat(&JsString::from("$"));
                self.property_map[p].new_name = Some(new_name);
            } else {
                // If we haven't already given this property a reusable name.
                if self.property_map[p].new_name.is_none() {
                    self.property_map[p].new_name = Some(self.name_generator.generate_next_name());
                }
            }
            reserved_names
                .write()
                .unwrap()
                .insert(self.property_map[p].new_name.clone().unwrap());
        }
    }

    /// Rust-only: the field values the test harness reads reflectively.
    pub fn replay_fields(&self) -> RenamePropertiesReplayFields<'_> {
        RenamePropertiesReplayFields {
            generate_pseudo_names: self.generate_pseudo_names,
            prev_used_property_map: self.prev_used_property_map.as_ref(),
            reserved_first_characters: &self.reserved_first_characters,
            reserved_non_first_characters: &self.reserved_non_first_characters,
            externed_names: &self.externed_names,
            quoted_names: &self.quoted_names,
            name_generator: &*self.name_generator,
        }
    }

    /// Gets the property renaming map (the "answer key").
    ///
    /// Returns a mapping from original names to new names
    // port: RenameProperties#getPropertyMap
    pub fn get_property_map(&self) -> VariableMap {
        let mut map: IndexMap<JsString, JsString> = IndexMap::new();
        for p in self.property_map.values() {
            if let Some(new_name) = &p.new_name {
                // ImmutableMap.Builder#buildOrThrow: duplicate keys throw; property names are
                // unique keys of propertyMap.
                map.insert(p.old_name.clone(), new_name.clone());
            }
        }
        VariableMap::new(&map)
    }

    /// If a property node is eligible for renaming, stashes a reference to it and increments the
    /// property name's access count.
    // port: RenameProperties.ProcessProperties#maybeMarkCandidate
    fn maybe_mark_candidate(&mut self, ast: &Ast, n: NodeId) {
        let name = n.get_string(ast);
        if !self.externed_names.contains(&name) {
            if !(self.property_rename_eligibility_filter)(ast, n) {
                self.filtered_out_names.insert(n.get_string(ast));
            }
            self.string_nodes_to_rename.push(n);
            self.count_property_occurrence(name);
        }
    }

    /// Counts references to property names that occur in a special function call.
    // port: RenameProperties.ProcessProperties#countCallCandidates
    fn count_call_candidates(&mut self, t: &mut NodeTraversal<'_>, call_node: NodeId) {
        let fn_name = call_node.get_first_child(t).unwrap().get_string(t);
        let first_arg = call_node.get_second_child(t).unwrap();
        if !first_arg.is_string_lit(t) {
            t.report(call_node, &BAD_CALL, &[&fn_name.to_string_lossy()]);
            return;
        }

        for name in dot_splitter_split(&first_arg.get_string(t)) {
            if !TokenStream::is_js_identifier(&name) {
                t.report(call_node, &BAD_ARG, &[&fn_name.to_string_lossy()]);
                continue;
            }
            if !self.externed_names.contains(&name) {
                self.count_property_occurrence(name);
            }
        }
    }

    /// Increments the occurrence count for a property name.
    // port: RenameProperties.ProcessProperties#countPropertyOccurrence
    fn count_property_occurrence(&mut self, name: JsString) {
        let prop = self
            .property_map
            .entry(name.clone())
            .or_insert_with(|| Property::new(name));
        prop.num_occurrences += 1;
    }
}

impl CompilerPass for RenameProperties {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        RenameProperties::process(self, compiler, externs, root);
    }
}

/// A traversal callback that collects property names and counts how frequently each property name
/// occurs.
struct ProcessProperties<'a> {
    this: &'a mut RenameProperties,
}

impl ProcessProperties<'_> {
    /// `compiler.getCodingConvention().isPropertyRenameFunction(n)`.
    fn is_property_rename_function(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let compiler = t.get_compiler();
        compiler
            .get_coding_convention()
            .is_property_rename_function(compiler, n)
    }
}

impl Callback for ProcessProperties<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RenameProperties.ProcessProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::GETPROP | Token::OPTCHAIN_GETPROP | Token::MEMBER_FIELD_DEF => {
                self.this.maybe_mark_candidate(t, n)
            }
            Token::GETELEM | Token::OPTCHAIN_GETELEM => {
                // If this is a quoted property access (e.g. x['myprop']), we need to
                // ensure that we never rename some other property in a way that
                // could conflict with this quoted name.
                let child = n.get_last_child(t);
                if let Some(child) = child
                    && child.is_string_lit(t)
                {
                    self.this.quoted_names.insert(child.get_string(t));
                }
            }
            Token::CALL => {
                // We replace property renaming function calls with a string
                // containing the renamed property.
                let fn_name = n.get_first_child(t).unwrap();

                if Self::is_property_rename_function(t, fn_name) {
                    self.this.call_node_to_parent_map.insert(n, parent.unwrap());
                    self.this.count_call_candidates(t, n);
                    let string_argument = n.get_second_child(t).unwrap();
                    if !(self.this.property_rename_eligibility_filter)(t, n) {
                        for name in dot_splitter_split(&string_argument.get_string(t)) {
                            self.this.filtered_out_names.insert(name);
                        }
                    }
                }
            }
            Token::MEMBER_FUNCTION_DEF => {
                check_state!(!n.is_quoted_string_key(t));
                if NodeUtil::is_es6_constructor_member_function_def(t, n) {
                    self.this.externed_names.insert(n.get_string(t));
                } else {
                    self.this.maybe_mark_candidate(t, n);
                }
            }
            Token::GETTER_DEF | Token::SETTER_DEF | Token::STRING_KEY => {
                if n.is_quoted_string_key(t) {
                    // Ensure that we never rename some other property in a way
                    // that could conflict with this quoted key.
                    self.this.quoted_names.insert(n.get_string(t));
                } else {
                    self.this.maybe_mark_candidate(t, n);
                }
            }
            Token::FUNCTION => {
                let parent = parent.unwrap();
                // We eliminate any stub implementations of JSCompiler_renameProperty
                // that we encounter.
                if NodeUtil::is_function_declaration(t, n) {
                    let name = n.get_first_child(t).unwrap().get_string(t);
                    if name == NodeUtil::JSC_PROPERTY_NAME_FN {
                        self.this.to_remove.push(n);
                    }
                } else if parent.is_name(t)
                    && parent.get_string(t) == NodeUtil::JSC_PROPERTY_NAME_FN
                {
                    let var_node = parent.get_parent(t).unwrap();
                    if var_node.is_var(t) {
                        self.this.to_remove.push(parent);
                    }
                } else if NodeUtil::is_function_expression(t, n)
                    && parent.is_assign(t)
                    && parent.get_first_child(t).unwrap().is_get_prop(t) // JSCompiler does not handle optional calls to property rename
                    // function
                    && Self::is_property_rename_function(t, parent.get_first_child(t).unwrap())
                {
                    let expr_result = parent.get_parent(t).unwrap();
                    if expr_result.is_expr_result(t)
                        && NodeUtil::is_statement_block(t, expr_result.get_parent(t).unwrap())
                        && expr_result.get_first_child(t).unwrap().is_assign(t)
                    {
                        self.this.to_remove.push(expr_result);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Encapsulates the information needed for renaming a property.
struct Property {
    old_name: JsString,
    new_name: Option<JsString>,
    num_occurrences: i32,
}

impl Property {
    // port: RenameProperties.Property#Property
    fn new(name: JsString) -> Self {
        Self {
            old_name: name,
            new_name: None,
            num_occurrences: 0,
        }
    }
}
