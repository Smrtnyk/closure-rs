/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/InlineProperties.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    colors::{Color, ColorRegistry, standard_colors},
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{check_state, ir::IR, js_string::JsString, node::Ast, node::NodeId};
use std::sync::Arc;

/// InlineProperties attempts to find references to properties that are known to be constants and
/// inline the known value.
///
/// This pass relies on type information to find these property references and properties are
/// assumed to be constant if they are assigned exactly once, unconditionally, in either of the
/// following contexts: (1) statically on a constructor, or (2) on a class's prototype.
///
/// The current implementation only inlines immutable values (as defined by
/// NodeUtil.isImmutableValue).
pub struct InlineProperties {
    registry: Arc<ColorRegistry>,
    // Java: Map<String, PropertyInfo> with the shared INVALIDATED instance; `None` here is
    // INVALIDATED (the only PropertyInfo whose color and value are null).
    props: IndexMap<JsString, Option<PropertyInfo>>,
}

// port: InlineProperties.PropertyInfo
struct PropertyInfo {
    color: Color,
    value: NodeId,
}

impl InlineProperties {
    // port: InlineProperties#InlineProperties
    pub fn new(compiler: &AbstractCompiler) -> Self {
        let mut this = Self {
            registry: compiler.get_color_registry().clone(),
            props: IndexMap::<_, _>::default(),
        };
        this.invalidate_extern_properties(compiler);
        this
    }

    // port: InlineProperties#invalidateExternProperties
    fn invalidate_extern_properties(&mut self, compiler: &AbstractCompiler) {
        // Invalidate properties defined in externs.
        for name in compiler.get_extern_properties().expect("") {
            self.props.insert(JsString::from(name.as_str()), None);
        }
    }

    /// This method gets the JSType from the Node argument and verifies that it is present.
    // port: InlineProperties#getColor
    fn get_color(ast: &Ast, n: NodeId) -> Color {
        let color = n.get_color(ast);

        match color {
            None => standard_colors::UNKNOWN.clone(),
            Some(color) => color,
        }
    }

    // port: InlineProperties#removeNullAndUndefinedIfUnion
    fn remove_null_and_undefined_if_union(original: Color) -> Color {
        if original.is_union() {
            original.subtract_null_or_void()
        } else {
            original
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectReplaceMode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_INLINE_PROPERTIES_DIRECT=off` replaces in a second traversal as Java does;
/// `=verify` does too and checks that it visits the recorded GETPROPs in order.
static DIRECT_REPLACE_MODE: std::sync::LazyLock<DirectReplaceMode> =
    std::sync::LazyLock::new(|| {
        match std::env::var("CLOSURE_RS_INLINE_PROPERTIES_DIRECT").as_deref() {
            Ok("off") => DirectReplaceMode::Off,
            Ok("verify") => DirectReplaceMode::Verify,
            _ => DirectReplaceMode::On,
        }
    });

#[derive(Clone, Copy, PartialEq, Eq)]
enum GatherCacheMode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_INLINE_PROPERTIES_CACHE=off` traverses every top-level statement in every run;
/// `=verify` traverses them all and compares the log of each one the cache would have replayed.
static GATHER_CACHE_MODE: std::sync::LazyLock<GatherCacheMode> = std::sync::LazyLock::new(|| {
    match std::env::var("CLOSURE_RS_INLINE_PROPERTIES_CACHE").as_deref() {
        Ok("off") => GatherCacheMode::Off,
        Ok("verify") => GatherCacheMode::Verify,
        _ => GatherCacheMode::On,
    }
});

/// Rust-only (performance divergence): what the gathering traversal of each top-level statement
/// of a script found in the last run, kept in the compiler for the next run (the optimization
/// loop runs InlineProperties several times; Java traverses the whole program in every run).
///
/// The traversal of a top-level statement starts outside every function and hoist scope, and
/// what it does to the candidate map is a sequence of updates, each a property name with either
/// nothing (an invalidation) or a definition that would be stored if the name were not in the
/// map yet; whether a definition qualifies depends only on the statement's subtree (its value, its
/// place, the enclosing function's JSDoc and form) and the colors read from nodes of the subtree.
/// So the log keeps the updates and the GETPROPs in traversal order, and is replayed (the updates
/// applied as the traversal applies them, the GETPROPs appended to the replacement list) while
/// the statement and every function in it keep their change stamps with flags (structure and
/// in-place JSDoc), and every color and static-member flag the traversal read is the same.
#[derive(Default)]
pub(crate) struct InlinePropertiesCache {
    units: IndexMap<NodeId, Arc<GatherLog>>,
}

#[derive(Debug, PartialEq)]
struct GatherLog {
    /// The statement and every function in it with `cached_subtree_stamp_with_flags`.
    stamps: Vec<(NodeId, (u32, u32))>,
    /// Every color the traversal read, by node.
    colors: Vec<(NodeId, Option<Color>)>,
    /// Every static-member flag the traversal read, by node.
    statics: Vec<(NodeId, bool)>,
    updates: Vec<(JsString, Option<(Color, NodeId)>)>,
    get_props: Vec<NodeId>,
}

impl GatherLog {
    fn is_current(&self, compiler: &AbstractCompiler) -> bool {
        use crate::syntactic_scope_cache::SyntacticScopeCache;
        self.stamps.iter().all(|&(n, stamp)| {
            SyntacticScopeCache::cached_subtree_stamp_with_flags(compiler, n) == stamp
        }) && self
            .colors
            .iter()
            .all(|(n, color)| n.get_color(compiler) == *color)
            && self
                .statics
                .iter()
                .all(|&(n, is_static)| n.is_static_member(compiler) == is_static)
    }
}

/// The log of the statement being traversed (see `InlinePropertiesCache`).
struct GatherRecording {
    unit: NodeId,
    functions: Vec<NodeId>,
    colors: Vec<(NodeId, Option<Color>)>,
    statics: Vec<(NodeId, bool)>,
    updates: Vec<(JsString, Option<(Color, NodeId)>)>,
    /// Where the statement's GETPROPs start in `GatherCandidates::get_props`.
    get_props_start: usize,
}

impl CompilerPass for InlineProperties {
    // port: InlineProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mode = *DIRECT_REPLACE_MODE;
        let cache_mode = *GATHER_CACHE_MODE;
        // The log keeps no JSType reads: with a registry, `inConstructor` reads them.
        let cache = (mode != DirectReplaceMode::Off
            && cache_mode != GatherCacheMode::Off
            && compiler.get_type_registry_field_and_ast().0.is_none())
        .then(|| {
            // Brings the change stamps up to date; the gathering traversal changes nothing.
            crate::syntactic_scope_cache::SyntacticScopeCache::subtree_stamp(compiler, root);
            std::mem::take(&mut compiler.inline_properties_cache)
        });
        // Find and replace the properties in non-extern AST.
        let mut gather = GatherCandidates {
            outer: self,
            get_props: (mode != DirectReplaceMode::Off).then(Vec::new),
            cache,
            fresh: IndexMap::default(),
            recording: None,
            verify: cache_mode == GatherCacheMode::Verify,
        };
        NodeTraversal::traverse(compiler, root, &mut gather);
        if gather.cache.take().is_some() {
            compiler.inline_properties_cache = InlinePropertiesCache {
                units: std::mem::take(&mut gather.fresh),
            };
        }
        let get_props = gather.get_props.take();
        let mut replace = ReplaceCandidates {
            outer: self,
            has_in_supertypes_list_seen_set: IndexSet::<_>::default(),
            expected_get_props: None,
        };
        match (mode, get_props) {
            (DirectReplaceMode::On, Some(get_props)) => {
                replace.replace_in_traversal_order(compiler, &get_props);
            }
            (_, get_props) => {
                replace.expected_get_props = get_props.map(|nodes| (nodes, 0));
                NodeTraversal::traverse(compiler, root, &mut replace);
                if let Some((nodes, next)) = &replace.expected_get_props {
                    assert_eq!(
                        *next,
                        nodes.len(),
                        "CLOSURE_RS_INLINE_PROPERTIES_DIRECT=verify: the replacing traversal \
                         visited {next} GETPROPs, the gathering traversal {}",
                        nodes.len()
                    );
                }
            }
        }
    }
}

struct GatherCandidates<'a> {
    outer: &'a mut InlineProperties,
    /// Rust-only (performance divergence): every GETPROP the traversal visits, in order
    /// (`ReplaceCandidates::replace_in_traversal_order`).
    get_props: Option<Vec<NodeId>>,
    /// Rust-only (performance divergence): the logs of the last run (`InlinePropertiesCache`),
    /// the logs of this run, and the log of the statement being traversed.
    cache: Option<InlinePropertiesCache>,
    fresh: IndexMap<NodeId, Arc<GatherLog>>,
    recording: Option<GatherRecording>,
    verify: bool,
}

/// Rust-only: a definition `maybeStoreCandidateValue` would store if the name were not in the
/// map yet (its color and value), or `None`.
type Definition = Option<(Color, NodeId)>;

impl GatherCandidates<'_> {
    /// `InlineProperties#getColor`, recorded for the log.
    fn color(&mut self, ast: &Ast, n: NodeId) -> Color {
        let color = n.get_color(ast);
        if let Some(recording) = &mut self.recording {
            recording.colors.push((n, color.clone()));
        }
        color.unwrap_or_else(|| standard_colors::UNKNOWN.clone())
    }

    /// Returns whether this is a valid definition for a candidate class field.
    // port: InlineProperties.GatherCandidates#maybeRecordCandidateClassFieldDefinition
    /// Rust-only shape: returns the definition, which the caller stores or invalidates.
    fn maybe_record_candidate_class_field_definition(
        &mut self,
        ast: &Ast,
        n: NodeId,
    ) -> Definition {
        check_state!(n.is_member_field_def(ast), "%s", n.to_string(ast));
        let value = n.get_first_child(ast);
        let class_node = n.get_grandparent(ast).unwrap();
        let is_static = n.is_static_member(ast);
        if let Some(recording) = &mut self.recording {
            recording.statics.push((n, is_static));
        }
        let c: Color = if is_static {
            self.color(ast, class_node)
        } else {
            let class_color = self.color(ast, class_node);
            let possible_instances = class_color.get_instance_colors();
            if possible_instances.is_empty() {
                standard_colors::UNKNOWN.clone()
            } else {
                Color::create_union(possible_instances)
            }
        };

        Self::candidate_value(ast, c, value)
    }

    /// Returns whether this is a valid definition for a candidate property.
    // port: InlineProperties.GatherCandidates#maybeRecordCandidateGetpropDefinition
    /// Rust-only shape: returns the definition, which the caller stores or invalidates.
    fn maybe_record_candidate_getprop_definition(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: NodeId,
    ) -> Definition {
        check_state!(
            n.is_get_prop(t) && parent.is_assign(t),
            "%s",
            n.to_string(t)
        );
        let src = n.get_first_child(t).unwrap();
        let value = parent.get_last_child(t);

        if src.is_this(t) {
            // This is a simple assignment like:
            //    this.foo = 1;
            if self.in_constructor(t) {
                // This may be a valid assignment.
                let color = self.color(t, src);
                return Self::candidate_value(t, color, value);
            }
            return None;
        } else if t.in_global_hoist_scope()
            && src.is_get_prop(t)
            && src.get_string_ref(t) == "prototype"
        {
            // This is a prototype assignment like:
            //    x.prototype.foo = 1;
            let instance_type = self.color(t, src);
            return Self::candidate_value(t, instance_type, value);
        } else if t.in_global_hoist_scope() {
            // This is a static assignment like:
            //    x.foo = 1;
            let target_type = self.color(t, src);
            if target_type.is_constructor() {
                return Self::candidate_value(t, target_type, value);
            }
        }
        None
    }

    /// Rust-only: the map update of a visited node, as Java makes it: a definition is stored when
    /// the name is not in the map yet (`maybeStoreCandidateValue`), and anything else invalidates
    /// the name (`invalidateProperty`). It is recorded for the log.
    fn update(&mut self, prop_name: JsString, definition: Definition) {
        if let Some(recording) = &mut self.recording {
            recording
                .updates
                .push((prop_name.clone(), definition.clone()));
        }
        self.outer.apply_update(prop_name, definition);
    }

    /// Adds the candidate property to the map if it meets all constness and immutability
    /// criteria, and is not already present in the map. If the property was already present, it
    /// is invalidated. Returns true if the property was successfully added.
    // port: InlineProperties.GatherCandidates#maybeStoreCandidateValue
    /// Rust-only shape: the criteria that do not read the map; `update` reads it.
    fn candidate_value(ast: &Ast, color: Color, value: Option<NodeId>) -> Definition {
        let value = closure_rhino::check_not_null!(value);
        (!color.is_invalidating()
            && NodeUtil::is_immutable_value(ast, value)
            && NodeUtil::is_executed_exactly_once(ast, value))
        .then_some((color, value))
    }

    /// Rust-only: ends the log of the statement just visited and keeps it.
    fn finish_recording(&mut self, compiler: &AbstractCompiler) {
        use crate::syntactic_scope_cache::SyntacticScopeCache;
        let recording = self.recording.take().unwrap();
        let stamps = std::iter::once(recording.unit)
            .chain(recording.functions)
            .map(|n| {
                (
                    n,
                    SyntacticScopeCache::cached_subtree_stamp_with_flags(compiler, n),
                )
            })
            .collect();
        let log = GatherLog {
            stamps,
            colors: recording.colors,
            statics: recording.statics,
            updates: recording.updates,
            get_props: self.get_props.as_ref().unwrap()[recording.get_props_start..].to_vec(),
        };
        if self.verify
            && let Some(cached) = self.cache.as_ref().unwrap().units.get(&recording.unit)
            && cached.is_current(compiler)
        {
            assert!(
                cached.updates == log.updates && cached.get_props == log.get_props,
                "CLOSURE_RS_INLINE_PROPERTIES_CACHE=verify: the kept log of {} differs from a \
                 traversal;\nkept updates: {:?}\nkept GETPROPs: {:?}\nnew updates: {:?}\nnew \
                 GETPROPs: {:?}",
                recording.unit.to_string(compiler),
                cached.updates,
                cached.get_props,
                log.updates,
                log.get_props
            );
        }
        self.fresh.insert(recording.unit, Arc::new(log));
    }

    /// Returns whether the traversal is directly in an ES6 class constructor or an @constructor
    /// function
    ///
    /// This returns false for nested functions inside ctors, including arrow functions (even
    /// though the `this` is the same). This pass only cares about property definitions executed
    /// once per ctor invocation, and in general we don't know how many times an arrow fn will be
    /// executed. In the future, we could special case arrow fn IIFEs in this pass if it becomes
    /// useful.
    // port: InlineProperties.GatherCandidates#inConstructor
    fn in_constructor(&mut self, t: &mut NodeTraversal<'_>) -> bool {
        let root = t.get_enclosing_function();
        if root.is_none() {
            // we might be in the global scope
            return false;
        }
        let (types, ast) = t.get_compiler().get_type_registry_field_and_ast();
        match types {
            Some(types) => NodeUtil::is_constructor(ast, root, types),
            None => {
                if let Some(recording) = &mut self.recording {
                    recording
                        .colors
                        .push((root.unwrap(), root.unwrap().get_color(ast)));
                }
                is_constructor_without_type_registry(ast, root.unwrap())
            }
        }
    }
}

impl Callback for GatherCandidates<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // Rust-only (performance divergence): a top-level statement of a script is replayed from
        // its log or traversed with one recorded (`InlinePropertiesCache`).
        if self.recording.is_none()
            && let Some(cache) = &self.cache
            && parent.is_some_and(|p| p.is_script(t))
            && !n.is_module_body(t)
        {
            let hit = if self.verify {
                None
            } else {
                cache
                    .units
                    .get(&n)
                    .filter(|log| log.is_current(t.get_compiler()))
                    .cloned()
            };
            if let Some(log) = hit {
                self.get_props
                    .as_mut()
                    .unwrap()
                    .extend_from_slice(&log.get_props);
                for (prop_name, definition) in &log.updates {
                    self.outer
                        .apply_update(prop_name.clone(), definition.clone());
                }
                self.fresh.insert(n, log);
                return false;
            }
            self.recording = Some(GatherRecording {
                unit: n,
                functions: Vec::new(),
                colors: Vec::new(),
                statics: Vec::new(),
                updates: Vec::new(),
                get_props_start: self.get_props.as_ref().unwrap().len(),
            });
        }
        if n.is_function(t)
            && let Some(recording) = &mut self.recording
        {
            recording.functions.push(n);
        }
        true
    }

    // port: InlineProperties.GatherCandidates#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.visit_node(t, n, parent);
        if self
            .recording
            .as_ref()
            .is_some_and(|recording| recording.unit == n)
        {
            self.finish_recording(t.get_compiler());
        }
    }
}

impl GatherCandidates<'_> {
    // port: InlineProperties.GatherCandidates#visit
    fn visit_node(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // These are assigned at most once in the branches below
        // Rust-only shape: a definition, or none for an invalidating reference, replaces Java's
        // `invalidatingPropRef` (`update` makes Java's map update of both).
        let definition: Definition;
        let prop_name: JsString;
        if n.is_get_prop(t) {
            if let Some(get_props) = &mut self.get_props {
                get_props.push(n);
            }
            let parent = parent.unwrap();
            if parent.is_assign(t) {
                prop_name = n.get_string(t);
                definition = self.maybe_record_candidate_getprop_definition(t, n, parent);
            } else if NodeUtil::is_l_value(t, n) {
                // Other LValue references invalidate
                // e.g. in an enhanced for loop or a destructuring statement
                prop_name = n.get_string(t);
                definition = None;
            } else if parent.is_del_prop(t) {
                // Deletes invalidate
                prop_name = n.get_string(t);
                definition = None;
            } else {
                // A property read doesn't invalidate
                return;
            }
        } else if (n.is_string_key(t) && !n.get_parent(t).unwrap().is_object_pattern(t))
            || n.is_getter_def(t)
            || n.is_setter_def(t)
            || n.is_member_function_def(t)
        {
            prop_name = n.get_string(t);
            // For now, any object literal key invalidates
            // TODO(johnlenz): support prototype properties like:
            //   foo.prototype = { a: 1, b: 2 };
            // TODO(johnlenz): Object.create(), Object.createProperty
            // and getter/setter defs and member functions also invalidate
            // since we do not inline functions in this pass
            // Note that string keys in destructuring patterns are fine, since they just access the
            // prop
            definition = None;
        } else if n.is_member_field_def(t) {
            prop_name = n.get_string(t);
            if n.has_children(t) {
                // class field with initialization
                definition = self.maybe_record_candidate_class_field_definition(t, n);
            } else {
                // class field with no initialization
                definition = None;
            }
        } else if n.is_call(t)
            && {
                let compiler = t.get_compiler();
                compiler
                    .get_coding_convention()
                    .is_property_rename_function(compiler, n.get_first_child(compiler).unwrap())
            }
            && n.has_more_than_one_child(t)
            && n.get_second_child(t).unwrap().is_string_lit(t)
        {
            // JSCompiler_renameProperty('foo', obj) and goog.reflect.objectProperty name `foo` for
            // reflective access, so `foo` may be written without a GETPROP, e.g. by
            // `obj[JSCompiler_renameProperty('foo', obj)] = value`.
            prop_name = n.get_second_child(t).unwrap().get_string(t);
            definition = None;
        } else {
            return;
        }

        // checkNotNull(propName): a JsString is never null.
        self.update(prop_name, definition);
    }
}

impl InlineProperties {
    /// Rust-only: Java's update of the candidate map for one visited node: a definition is stored
    /// when its name is not in the map yet (`maybeStoreCandidateValue` returning true); otherwise
    /// the name is invalidated (`invalidateProperty`).
    fn apply_update(&mut self, prop_name: JsString, definition: Definition) {
        match definition {
            Some((color, value)) if !self.props.contains_key(&prop_name) => {
                self.props
                    .insert(prop_name, Some(PropertyInfo { color, value }));
            }
            _ => {
                // port: InlineProperties.GatherCandidates#invalidateProperty
                self.props.insert(prop_name, None);
            }
        }
    }
}

struct ReplaceCandidates<'a> {
    outer: &'a mut InlineProperties,
    has_in_supertypes_list_seen_set: IndexSet<Color>,
    /// Rust-only: under `CLOSURE_RS_INLINE_PROPERTIES_DIRECT=verify`, the GETPROPs the gathering
    /// traversal visited and how many of them this traversal has visited.
    expected_get_props: Option<(Vec<NodeId>, usize)>,
}

impl ReplaceCandidates<'_> {
    /// Rust-only (performance divergence): Java's second traversal of the sources, which acts
    /// only on GETPROPs, as a loop over the GETPROPs the first traversal visited, in its order.
    /// Nothing changes the tree between the two traversals, so they visit the same nodes in the
    /// same post-order; a replacement changes only the subtree of the GETPROP it replaces, whose
    /// nodes come before it in that order, and puts its replacement in the GETPROP's place, which
    /// the traversal does not visit (it moves on to the next sibling it read before descending).
    /// So the traversal visits exactly the recorded nodes, in order, each in the tree as the
    /// replacements before it left it, as this loop does; `visit` reads nothing of the traversal
    /// but the tree. A GETPROP whose name has no candidate is passed over without the rest of
    /// `visit`, which would do nothing for it.
    fn replace_in_traversal_order(
        &mut self,
        compiler: &mut AbstractCompiler,
        get_props: &[NodeId],
    ) {
        if !self.outer.props.values().any(Option::is_some) {
            return;
        }
        for &n in get_props {
            if matches!(
                self.outer.props.get(n.get_string_ref(compiler)),
                Some(Some(_))
            ) {
                self.replace_if_candidate(compiler, n);
            }
        }
    }

    // port: InlineProperties.ReplaceCandidates#visit
    fn replace_if_candidate(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.is_get_prop(compiler) && !NodeUtil::is_l_value(compiler, n) {
            let target = n.get_first_child(compiler).unwrap();
            let prop_name = n.get_string(compiler);
            let info = match self.outer.props.get(&prop_name) {
                Some(Some(info)) => Some((info.color.clone(), info.value)),
                _ => None,
            };
            if let Some((info_color, info_value)) = info
                && self.is_matching_type(compiler, target, info_color)
            {
                let mut replacement = info_value.clone_tree(compiler);
                if compiler
                    .get_ast_analyzer()
                    .may_have_side_effects(compiler, n.get_first_child(compiler).unwrap())
                {
                    let first = n.remove_first_child(compiler).unwrap();
                    replacement = IR::comma(compiler, first, replacement).srcref(compiler, n);
                }
                n.replace_with(compiler, replacement);
                compiler.report_change_to_enclosing_scope(replacement);
            }
        }
    }

    // port: InlineProperties.ReplaceCandidates#isMatchingType
    fn is_matching_type(&mut self, ast: &Ast, n: NodeId, src: Color) -> bool {
        let src = InlineProperties::remove_null_and_undefined_if_union(src);
        let dest = InlineProperties::remove_null_and_undefined_if_union(
            InlineProperties::get_color(ast, n),
        );
        if dest.is_invalidating() {
            return false;
        }
        if dest.is_union() || src.is_union() {
            return false;
        }
        self.has_in_supertypes_list(&dest, &src)
    }

    // port: InlineProperties.ReplaceCandidates#hasInSupertypesList
    fn has_in_supertypes_list(&mut self, sub_ctor: &Color, super_ctor: &Color) -> bool {
        let result = (|| {
            if !self
                .has_in_supertypes_list_seen_set
                .insert(sub_ctor.clone())
            {
                return false;
            }
            // `subCtor == null || superCtor == null`: neither can be null here (callers pass
            // getColor results, PropertyInfo colors and registry supertypes).
            if sub_ctor == super_ctor {
                return true;
            }

            let registry = self.outer.registry.clone();
            for immediate_supertype in registry.get_disambiguation_supertypes(sub_ctor) {
                if !immediate_supertype.is_union()
                    && self.has_in_supertypes_list(immediate_supertype, super_ctor)
                {
                    return true;
                }
            }
            false
        })();
        // finally
        self.has_in_supertypes_list_seen_set.shift_remove(sub_ctor);
        result
    }
}

impl Callback for ReplaceCandidates<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: InlineProperties.ReplaceCandidates#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if let Some((nodes, next)) = &mut self.expected_get_props
            && n.is_get_prop(t)
        {
            assert_eq!(
                nodes.get(*next).copied(),
                Some(n),
                "CLOSURE_RS_INLINE_PROPERTIES_DIRECT=verify: GETPROP {next} of the replacing \
                 traversal is {} ({}), the gathering traversal recorded {:?}",
                n.to_string(t),
                n.get_string_ref(t),
                nodes.get(*next).map(|m| m.to_string(t))
            );
            *next += 1;
        }
        self.replace_if_candidate(t.get_compiler(), n);
    }
}

/// Rust-only: `NodeUtil#isConstructor(Node)` when the compiler holds no `JSTypeRegistry` (never
/// created, or cleared after the types were converted to colors). The Rust port takes the registry
/// as an argument (DESIGN §4); Java reads it only through a JSType on the node, and a node carries a
/// JSType only while the registry exists, so the JSType clause is false here.
fn is_constructor_without_type_registry(ast: &closure_rhino::node::Ast, fn_node: NodeId) -> bool {
    if !fn_node.is_function(ast) {
        return false;
    }
    assert!(
        fn_node.get_jstype(ast).is_none(),
        "NodeUtil#isConstructor: a node has a JSType but the compiler has no JSTypeRegistry"
    );
    let js_doc_info = NodeUtil::get_best_jsdoc_info(ast, fn_node);
    let color = fn_node.get_color(ast);
    js_doc_info.is_some_and(|info| info.is_constructor())
        || color.is_some_and(|color| color.is_constructor())
        || NodeUtil::is_es6_constructor(ast, fn_node)
}
