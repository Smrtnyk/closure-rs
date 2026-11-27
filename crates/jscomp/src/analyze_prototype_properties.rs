/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AnalyzePrototypeProperties.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of AnalyzePrototypeProperties.java.
//!
//! Analyzes properties on prototypes.
//!
//! Uses a reference graph to analyze prototype properties. Each unique property name is
//! represented by a node in this graph. An edge from property A to property B means that there's
//! a GETPROP access of a property B on some object inside of a method named A.
//!
//! Global functions are also represented by nodes in this graph, with similar semantics.
//!
//! Rust shape: Java's NameInfo objects are graph node values compared by identity; they live in
//! the `name_infos` arena and the graph holds their `NameInfoId`. Java's `chunkGraph` field is
//! `compiler.getChunkGraph()` (its only caller, CrossChunkMethodMotion, passes that); the pass
//! reads it from the compiler after construction.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::graph::fixed_point_graph_traversal::{EdgeCallback, FixedPointGraphTraversal};
use crate::graph::graph::Graph;
use crate::graph::linked_directed_graph::LinkedDirectedGraph;
use crate::js_chunk::JSChunk;
use crate::js_chunk_graph::JSChunkGraph;
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::scope::ScopeId;
use crate::syntactic_scope_cache::SyntacticScopeCache;
use crate::var::VarId;
use closure_rhino::check_state;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId, Prop};
use closure_rhino::token::Token;
use std::collections::VecDeque;
use std::sync::Arc;

/// Handle of a Java NameInfo object (identity).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NameInfoId(usize);

pub struct AnalyzePrototypeProperties {
    can_modify_externs: bool,
    anchor_unused_vars: bool,
    root_scope_uses_are_global: bool,

    first_chunk: Option<JSChunk>,

    // A graph where the nodes are property names or variable names,
    // and the edges signify the chunks where the property is referenced.
    // For example, if we had the code:
    //
    // Foo.prototype.bar = function(x) { x.baz(); }; // in chunk 2.;
    //
    // then this would be represented in the graph by a node representing
    // "bar", a node representing "baz", and an edge between them representing
    // chunk #2.
    //
    // Similarly, if we had:
    //
    // var scotch = function(f) { return f.age(); };
    //
    // then there would be a node for "scotch", a node for "age", and an edge
    // from scotch to age.
    symbol_graph: LinkedDirectedGraph<NameInfoId, Option<ChunkIndex>>,

    /// Owns every NameInfo (Rust-only).
    name_infos: Vec<NameInfo>,

    // A dummy node for representing global references.
    global_node: NameInfoId,

    // A dummy node for representing extern references.
    extern_node: NameInfoId,

    // A dummy node for representing all anonymous functions with no names.
    anonymous_node: NameInfoId,

    // All the real NameInfo for prototype properties, hashed by the name
    // of the property that they represent.
    property_name_info: IndexMap<JsString, NameInfoId>,

    // All the NameInfo for global functions, hashed by the name of the
    // global variable that it's assigned to.
    var_name_info: IndexMap<JsString, NameInfoId>,
}

impl AnalyzePrototypeProperties {
    // Constants for symbol types, for easier readability.
    // port: AnalyzePrototypeProperties#PROPERTY
    const PROPERTY: SymbolType = SymbolType::PROPERTY;
    // port: AnalyzePrototypeProperties#VAR
    const VAR: SymbolType = SymbolType::VAR;

    // Properties that are implicitly used as part of the JS language.
    // port: AnalyzePrototypeProperties#IMPLICITLY_USED_PROPERTIES
    const IMPLICITLY_USED_PROPERTIES: [&'static str; 3] = ["length", "toString", "valueOf"];

    /// Creates a new pass for analyzing prototype properties.
    ///
    /// @param chunk_graph The graph for resolving chunk dependencies.
    /// @param can_modify_externs If true, then we can move prototype properties that are declared
    ///     in the externs file.
    /// @param anchor_unused_vars If true, then we must mark all vars as referenced, even if they
    ///     are never used.
    /// @param root_scope_uses_are_global If true, all uses in root level scope are treated as
    ///     references from '[global]', even if they are assignments to a property.
    // port: AnalyzePrototypeProperties#AnalyzePrototypeProperties
    pub fn new(
        chunk_graph: Option<&JSChunkGraph>,
        can_modify_externs: bool,
        anchor_unused_vars: bool,
        root_scope_uses_are_global: bool,
    ) -> Self {
        let first_chunk = if chunk_graph.unwrap().get_chunk_count() > 1 {
            Some(chunk_graph.unwrap().get_root_chunk())
        } else {
            None
        };

        let mut this = Self {
            can_modify_externs,
            anchor_unused_vars,
            root_scope_uses_are_global,
            first_chunk,
            // LinkedDirectedGraph.createWithoutAnnotations(); the graph holds NameInfo handles, so
            // Java's String.valueOf(NameInfo) is not available to it (debug output only).
            symbol_graph: LinkedDirectedGraph::new_with_value_to_strings(
                false,
                false,
                |n| format!("NameInfo#{}", n.0),
                // Rust-only: the edge holds the chunk's index, not its name (debug output only).
                |e| {
                    e.map_or_else(
                        || "null".to_string(),
                        |chunk| format!("JSChunk#{}", chunk.0),
                    )
                },
            ),
            name_infos: Vec::new(),
            global_node: NameInfoId(0),
            extern_node: NameInfoId(1),
            anonymous_node: NameInfoId(2),
            property_name_info: IndexMap::<_, _>::default(),
            var_name_info: IndexMap::<_, _>::default(),
        };
        this.global_node = this.new_name_info("[global]");
        this.extern_node = this.new_name_info("[extern]");
        this.anonymous_node = this.new_name_info("[anonymous]");

        let global_node = this.global_node;
        let extern_node = this.extern_node;
        this.name_infos[global_node.0].mark_reference(chunk_graph, None);
        this.name_infos[extern_node.0].mark_reference(chunk_graph, None);
        this.symbol_graph.create_node(global_node);
        this.symbol_graph.create_node(extern_node);

        for property in Self::IMPLICITLY_USED_PROPERTIES {
            let name_info = this.get_name_info_for_name(&JsString::from(property), Self::PROPERTY);
            match chunk_graph {
                None => {
                    this.symbol_graph.connect(extern_node, None, name_info);
                }
                Some(chunk_graph) => {
                    for chunk in chunk_graph.get_all_chunks() {
                        this.symbol_graph.connect(
                            extern_node,
                            Some(ChunkIndex::of(chunk)),
                            name_info,
                        );
                    }
                }
            }
        }
        this
    }

    fn new_name_info(&mut self, name: &str) -> NameInfoId {
        self.name_infos
            .push(NameInfo::new(JsString::from(name), Self::PROPERTY));
        NameInfoId(self.name_infos.len() - 1)
    }

    /// Returns the NameInfo object behind a handle (Rust-only accessor).
    pub fn get_name_info(&self, id: NameInfoId) -> &NameInfo {
        &self.name_infos[id.0]
    }

    /// Returns information on all prototype properties.
    // port: AnalyzePrototypeProperties#getAllNameInfo
    pub fn get_all_name_info(&self) -> Vec<NameInfoId> {
        let mut result: Vec<NameInfoId> = self.property_name_info.values().copied().collect();
        result.extend(self.var_name_info.values().copied());
        result
    }

    /// Gets the name info for the property or variable of a given name, and creates a new one if
    /// necessary.
    ///
    /// @param name The name of the symbol.
    /// @param type The type of symbol.
    // port: AnalyzePrototypeProperties#getNameInfoForName
    fn get_name_info_for_name(&mut self, name: &JsString, r#type: SymbolType) -> NameInfoId {
        let map = if r#type == Self::PROPERTY {
            &self.property_name_info
        } else {
            &self.var_name_info
        };
        if let Some(&name_info) = map.get(name) {
            name_info
        } else {
            self.name_infos.push(NameInfo::new(name.clone(), r#type));
            let name_info = NameInfoId(self.name_infos.len() - 1);
            let map = if r#type == Self::PROPERTY {
                &mut self.property_name_info
            } else {
                &mut self.var_name_info
            };
            map.insert(name.clone(), name_info);
            self.symbol_graph.create_node(name_info);
            name_info
        }
    }
}

impl CompilerPass for AnalyzePrototypeProperties {
    // port: AnalyzePrototypeProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, extern_root: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        if !self.can_modify_externs {
            NodeTraversal::traverse(
                compiler,
                extern_root,
                &mut ProcessExternProperties { outer: self },
            );
        }

        let mode = *UNIT_CACHE_MODE;
        let cache = (mode != UnitCacheMode::Off).then(|| {
            // Brings the change stamps up to date; the traversal changes nothing.
            SyntacticScopeCache::subtree_stamp(compiler, root);
            let mut cache = std::mem::take(&mut compiler.prototype_properties_cache);
            let config = (self.anchor_unused_vars, self.root_scope_uses_are_global);
            if cache.config != Some(config) {
                cache.units.clear();
                cache.config = Some(config);
            }
            cache
        });
        let mut process_properties = ProcessProperties {
            outer: self,
            symbol_stack: VecDeque::new(),
            cache,
            fresh: IndexMap::default(),
            recording: None,
            verify: mode == UnitCacheMode::Verify,
            prototype_assign_nodes: Vec::new(),
            run_to_global: Vec::new(),
            global_to_run: Vec::new(),
        };
        NodeTraversal::traverse(compiler, root, &mut process_properties);
        if let Some(mut cache) = process_properties.cache.take() {
            cache.units = std::mem::take(&mut process_properties.fresh);
            compiler.prototype_properties_cache = cache;
        }

        let entry = [self.extern_node, self.global_node];
        let mut t = FixedPointGraphTraversal::new_traversal(PropagateReferences {
            chunk_graph: compiler.get_chunk_graph(),
            name_infos: &mut self.name_infos,
        });
        t.compute_fixed_point_from_set(&mut self.symbol_graph, entry);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UnitCacheMode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_PROTOTYPE_PROPERTIES_CACHE=off` traverses every top-level statement in every run;
/// `=verify` traverses them all and compares the log of each one the cache would have replayed.
static UNIT_CACHE_MODE: std::sync::LazyLock<UnitCacheMode> = std::sync::LazyLock::new(|| {
    match std::env::var("CLOSURE_RS_PROTOTYPE_PROPERTIES_CACHE").as_deref() {
        Ok("off") => UnitCacheMode::Off,
        Ok("verify") => UnitCacheMode::Verify,
        _ => UnitCacheMode::On,
    }
});

/// Rust-only (performance divergence): what the traversal of each top-level statement of a
/// script did to the analysis in the last run, kept in the compiler for the next run
/// (`CrossChunkMethodMotion` runs in the chunk motion loop, and between two runs only the
/// statements the motion passes touched change). Java traverses the whole program in every run.
///
/// A top-level statement starts with the same traversal state in every run: the symbol stack
/// holds only the `[global]` context, the scope is the global scope, and the chunk is that of
/// its script. Everything its traversal does to the analysis is a call of one of the `op_*`
/// operations below, whose arguments the log keeps, a name info by its name and kind in `names`
/// (made, at its first use in a replay, by the same operation that made it in the traversal: a
/// name info on the symbol stack was made by an earlier operation of the same statement, or is a
/// dummy node). What depends on more than the statement's subtree is kept unresolved and resolved
/// again when the log is replayed: the chunk (from the script), a NAME that does not resolve to a
/// local var (looked up in the global scope then: whether it is a global function and which var
/// it is), and a property root var that is not local. A local var is kept by its declaring name node,
/// which identifies it as the var would (`RootVar::Local`). The log is replayed while the
/// statement and every function in it have their change stamps, it is in the same script, and
/// the flags its traversal can read in place (quoted keys, static members, computed method kinds)
/// are the same.
#[derive(Default)]
pub(crate) struct PrototypePropertiesCache {
    /// The `anchor_unused_vars` and `root_scope_uses_are_global` the logs were made with.
    config: Option<(bool, bool)>,
    units: IndexMap<NodeId, Arc<UnitLog>>,
    /// The name and kind of every name info a log refers to, by index.
    names: IndexSet<(JsString, SymbolType)>,
}

#[derive(Debug, PartialEq)]
struct UnitLog {
    script: NodeId,
    /// The statement and every function in it, with their change stamps
    /// (`SyntacticScopeCache::cached_subtree_stamp`).
    stamps: Vec<(NodeId, u32)>,
    /// Every key or class member in the statement, with the flags `flag_bits` reads.
    flags: Vec<(NodeId, u8)>,
    ops: Vec<Op>,
    symbols: Vec<SymbolSpec>,
}

impl UnitLog {
    fn is_current(&self, compiler: &AbstractCompiler, script: NodeId) -> bool {
        self.script == script
            && self
                .stamps
                .iter()
                .all(|&(n, stamp)| SyntacticScopeCache::cached_subtree_stamp(compiler, n) == stamp)
            && self
                .flags
                .iter()
                .all(|&(n, bits)| flag_bits(compiler, n) == bits)
    }
}

/// The flags of a key or class member that the traversal may read in place: whether it is a
/// quoted key, a static member, or a computed method, getter or setter.
fn flag_bits(ast: &Ast, n: NodeId) -> u8 {
    u8::from(n.is_quoted_string_key(ast))
        | (u8::from(n.is_static_member(ast)) << 1)
        | (u8::from(n.get_boolean_prop(ast, Prop::COMPUTED_PROP_METHOD)) << 2)
        | (u8::from(n.get_boolean_prop(ast, Prop::COMPUTED_PROP_GETTER)) << 3)
        | (u8::from(n.get_boolean_prop(ast, Prop::COMPUTED_PROP_SETTER)) << 4)
}

fn has_flags(ast: &Ast, n: NodeId) -> bool {
    matches!(
        n.get_token(ast),
        Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::MEMBER_VARIABLE_DEF
            | Token::COMPUTED_PROP
            | Token::COMPUTED_FIELD_DEF
    )
}

/// A name info in a log: a dummy node or an entry of `PrototypePropertiesCache::names` (while the
/// statement is traversed, a `NameInfoId` of the run).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ref(u32);

impl Ref {
    const GLOBAL: Ref = Ref(u32::MAX);
    const EXTERN: Ref = Ref(u32::MAX - 1);
    const ANONYMOUS: Ref = Ref(u32::MAX - 2);

    fn is_dummy(self) -> bool {
        self.0 >= Self::ANONYMOUS.0
    }
}

/// How a property's root var was found, for a log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RootVarSpec {
    /// The root is not a NAME.
    None,
    /// A local var, by its declaring name node.
    Local(NodeId),
    /// The NAME resolved to no local var: looked up in the global scope.
    Global(NodeId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymbolSpec {
    ClassMember {
        node: NodeId,
        var: RootVarSpec,
    },
    Assignment {
        expr: NodeId,
        root_var: RootVarSpec,
    },
    Literal {
        value: NodeId,
        assign: NodeId,
        root_var: RootVarSpec,
    },
}

/// One call of an `op_*` operation; a name info that is never a dummy node is a bare `Ref` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    NameInfo(u32),
    SymbolUse {
        info: u32,
        def: Ref,
    },
    GlobalUse(u32),
    SetReferencesSuper(Ref),
    SetReadsClosureVariables(Ref),
    /// `symbol` is an index of `UnitLog::symbols`.
    Declare {
        info: u32,
        symbol: u32,
    },
    GlobalName {
        node: NodeId,
        in_global_hoist_scope: bool,
        is_declaration: bool,
        def: Ref,
    },
}

/// The log of the statement being traversed (see `PrototypePropertiesCache`).
struct UnitRecording {
    unit: NodeId,
    script: NodeId,
    functions: Vec<NodeId>,
    flags: Vec<(NodeId, u8)>,
    ops: Vec<Op>,
    symbols: Vec<SymbolSpec>,
    /// False once the statement's symbols use a local var without a declaring name node.
    cacheable: bool,
}

struct ProcessProperties<'a> {
    outer: &'a mut AnalyzePrototypeProperties,
    // There are two types of context information on this stack:
    // 1) Every scope has a NameContext corresponding to its scope.
    //    Variables are given VAR contexts.
    //    Prototype properties are given PROPERTY contexts.
    //    The global scope is given the special [global] context.
    //    And function expressions that we aren't able to give a reasonable
    //    name are given a special [anonymous] context.
    // 2) Every assignment of a prototype property of a non-function is
    //    given a name context. These contexts do not have scopes.
    symbol_stack: VecDeque<NameContext>,
    /// Rust-only (performance divergence): the logs of the last run (`PrototypePropertiesCache`),
    /// the logs of this run, and the log of the statement being traversed.
    cache: Option<PrototypePropertiesCache>,
    fresh: IndexMap<NodeId, Arc<UnitLog>>,
    recording: Option<UnitRecording>,
    verify: bool,
    /// Rust-only (performance divergence): the nodes whose `shouldTraverse` pushed a non-function
    /// prototype assignment context, innermost last: `visit` pops the context of such a node
    /// instead of computing `processNonFunctionPrototypeAssign` again (the traversal changes
    /// nothing, so it would find the same name).
    prototype_assign_nodes: Vec<NodeId>,
    /// The entry of `PrototypePropertiesCache::names` of each name info of this run, and the
    /// reverse (`u32::MAX`: not known yet).
    run_to_global: Vec<u32>,
    global_to_run: Vec<u32>,
}

impl ScopedCallback for ProcessProperties<'_> {
    // port: AnalyzePrototypeProperties.ProcessProperties#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let n = t.get_current_node().unwrap();
        let scope = t.get_scope();
        let root = scope.get_root_node(t.get_compiler());
        if root.is_function(t) {
            let prop_name = self.get_prototype_property_name_from_r_value(t, n);
            if let Some(prop_name) = prop_name {
                let name = self.op_name_info(&prop_name, AnalyzePrototypeProperties::PROPERTY);
                self.symbol_stack
                    .push_front(NameContext::new(name, Some(scope)));
            } else if self.is_global_function_declaration(t, n) {
                let parent = n.get_parent(t).unwrap();
                let name = if parent.is_name(t) {
                    parent.get_string(t) /* VAR */
                } else {
                    n.get_first_child(t).unwrap().get_string(t) /* named function */
                };
                let name_info = self.op_name_info(&name, AnalyzePrototypeProperties::VAR);
                let hoist_scope = scope.get_closest_hoist_scope(t.get_compiler());
                self.symbol_stack
                    .push_front(NameContext::new(name_info, hoist_scope));
            } else {
                // NOTE(nicksantos): We use the same anonymous node for all
                // functions that do not have reasonable names. I can't remember
                // at the moment why we do this. I think it's because anonymous
                // nodes can never have in-edges. They're just there as a placeholder
                // for scope information, and do not matter in the edge propagation.
                self.symbol_stack
                    .push_front(NameContext::new(self.outer.anonymous_node, Some(scope)));
            }
        } else if t.in_global_scope() {
            self.symbol_stack
                .push_front(NameContext::new(self.outer.global_node, Some(scope)));
        } else {
            // TODO(moz): It's not yet clear if we need another kind of NameContext for block
            // scopes in ES6, use anonymous node for now and investigate later.
            // TODO(b/189993301): Test effects of pushing anonymousNode for member and computed
            // field defs in CrossChunkMethodMotion (and test class fields there as well)
            check_state!(
                NodeUtil::creates_block_scope(t, root)
                    || root.is_module_body(t)
                    || root.is_computed_field_def(t)
                    || root.is_member_field_def(t),
                "%s",
                scope.to_string(t.get_compiler())
            );
            self.symbol_stack
                .push_front(NameContext::new(self.outer.anonymous_node, Some(scope)));
        }
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {
        self.symbol_stack.pop_front();
    }
}

impl Callback for ProcessProperties<'_> {
    // port: AnalyzePrototypeProperties.ProcessProperties#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // Rust-only (performance divergence): a top-level statement of a script is replayed from
        // its log or traversed with one recorded (`PrototypePropertiesCache`).
        if self.cache.is_some()
            && self.recording.is_none()
            && let Some(script) = parent.filter(|p| p.is_script(t))
            && !n.is_module_body(t)
        {
            if !self.verify
                && let Some(log) = self
                    .cache
                    .as_ref()
                    .unwrap()
                    .units
                    .get(&n)
                    .filter(|log| log.is_current(t.get_compiler(), script))
                    .cloned()
            {
                self.replay(t, &log);
                self.fresh.insert(n, log);
                return false;
            }
            self.recording = Some(UnitRecording {
                unit: n,
                script,
                functions: Vec::new(),
                flags: Vec::new(),
                ops: Vec::new(),
                symbols: Vec::new(),
                cacheable: true,
            });
        }
        if let Some(recording) = &mut self.recording {
            if n.is_function(t) {
                recording.functions.push(n);
            } else if has_flags(t, n) {
                recording.flags.push((n, flag_bits(t, n)));
            }
        }

        if !self.outer.root_scope_uses_are_global {
            // Process assignment of a non-function expression to a prototype property.
            let prop_name = self.process_non_function_prototype_assign(t, n, parent);
            if let Some(prop_name) = prop_name {
                let name = self.op_name_info(&prop_name, AnalyzePrototypeProperties::PROPERTY);
                self.symbol_stack.push_front(NameContext::new(name, None));
                self.prototype_assign_nodes.push(n);
            }
        }
        true
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let reached_end = self.visit_node(t, n, parent);
        // Process prototype assignments to non-functions.
        if self.prototype_assign_nodes.last() == Some(&n) {
            debug_assert!(
                !self.outer.root_scope_uses_are_global
                    && self
                        .process_non_function_prototype_assign(t, n, parent)
                        .is_some()
            );
            self.prototype_assign_nodes.pop();
            if reached_end {
                self.symbol_stack.pop_front();
            }
        }
        if self
            .recording
            .as_ref()
            .is_some_and(|recording| recording.unit == n)
        {
            self.finish_recording(t);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ProcessProperties<'_> {
    // port: AnalyzePrototypeProperties.ProcessProperties#visit
    /// Rust-only shape: returns false where Java's `visit` returns early (before popping a
    /// non-function prototype assignment context).
    fn visit_node(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) -> bool {
        let mut fall_through_to_object_pattern = false;
        match n.get_token(t) {
            Token::SUPER => {
                // Example:
                // class X extends Y {
                //   method() {
                //     return () => super.x;
                //   }
                // }
                // Names associated with the arrow function, the method body, and the method
                // itself should be marked as referencing super, but not the class definition or
                // anything containing it.
                for i in 0..self.symbol_stack.len() {
                    let context = self.symbol_stack[i];
                    self.op_set_references_super(context.name);
                    let scope_root = context.scope.unwrap().get_root_node(t.get_compiler());
                    if NodeUtil::is_method_declaration(t, scope_root) {
                        break;
                    }
                }
            }

            Token::OPTCHAIN_GETPROP => {
                let chunk = t.get_compiler_and_chunk().1.map(ChunkIndex::of);
                self.op_symbol_use(
                    &n.get_string(t),
                    chunk,
                    AnalyzePrototypeProperties::PROPERTY,
                );
            }

            Token::GETPROP => {
                let prop_name = n.get_string(t);

                if n.is_qualified_name(t) {
                    if prop_name == "prototype" {
                        if self.handle_possible_assignment_to_prototype(t, n) {
                            // The reference is being assigned to not read from, so don't record
                            // this as a reference.
                            return false;
                        }
                    } else if t
                        .get_compiler()
                        .get_coding_convention()
                        .is_exported(&prop_name, /* local= */ false)
                    {
                        // TODO(bradfordcsmith): We don't seem to have any tests that cover this
                        // case. This class has no unit tests of its own and it is only used by
                        // CrossChunkMethodMotion.
                        let chunk = t.get_compiler_and_chunk().1.map(ChunkIndex::of);
                        self.op_global_use(&prop_name, chunk, AnalyzePrototypeProperties::PROPERTY);
                        return false;
                    } else {
                        let parent = parent.unwrap();
                        if parent.is_assign(t) && Some(n) == parent.get_first_child(t) {
                            let r_value_name = self.get_prototype_property_name_from_r_value(t, n);
                            if r_value_name.is_some() {
                                // e.g. `Foo.prototype.bar = something`
                                // We record the declaration of `bar` when we look at the
                                // `Foo.prototype` Node via the call to
                                // handlePossibleAssignmentToPrototype() above.
                                // Now that we're looking at the whole `Foo.prototype.bar` node,
                                // we just need to make sure we don't record it as a read
                                // reference.
                                return false;
                            }
                        }
                    }
                }

                let chunk = t.get_compiler_and_chunk().1.map(ChunkIndex::of);
                self.op_symbol_use(&prop_name, chunk, AnalyzePrototypeProperties::PROPERTY);
            }

            Token::OBJECTLIT => {
                // Make sure that we're not handling object literals being
                // assigned to a prototype, as in:
                // Foo.prototype = {bar: 3, baz: 5};
                let l_value_name =
                    NodeUtil::get_best_l_value_name(t, NodeUtil::get_best_l_value(t, n));
                if l_value_name
                    .as_ref()
                    .is_some_and(|name| name.ends_with(".prototype"))
                {
                    return false;
                }

                // Fall through.
                fall_through_to_object_pattern = true;
            }

            Token::OBJECT_PATTERN => {
                fall_through_to_object_pattern = true;
            }

            Token::CLASS => {
                let class_members = n.get_last_child(t).unwrap();
                let mut child = class_members.get_first_child(t);
                while let Some(c) = child {
                    if c.is_member_function_def(t) || c.is_setter_def(t) || c.is_getter_def(t) {
                        self.process_member_def(t, c);
                    }
                    child = c.get_next(t);
                }
            }

            Token::NAME => {
                let name = n.get_string(t);

                let scope = t.get_scope();
                let var = scope.get_var(t.get_compiler(), name.clone());
                match var {
                    // If it is not a global, it might be accessing a local of the outer
                    // scope. If that's the case the functions between the variable's
                    // declaring scope and the variable reference scope cannot be moved.
                    Some(var) if !var.is_global(t.get_compiler()) => {
                        if var.get_scope(t.get_compiler()) != t.get_scope() {
                            let var_scope = var.get_scope(t.get_compiler());
                            for i in 0..self.symbol_stack.len() {
                                let context = self.symbol_stack[i];
                                if context.scope == Some(var_scope) {
                                    break;
                                }

                                self.op_set_reads_closure_variables(context.name);
                            }
                        }
                    }
                    // Only process global functions.
                    _ => {
                        let in_global_hoist_scope = t.in_global_hoist_scope();
                        let is_declaration = in_global_hoist_scope && {
                            // processGlobalFunctionDeclaration's test, which reads only the
                            // tree in the global hoist scope.
                            let first_child = n.get_first_child(t);
                            let parent = n.get_parent(t).unwrap();
                            // Check for a named FUNCTION.
                            self.is_global_function_declaration(t, parent)
                                // Check for a VAR declaration.
                                || first_child.is_some_and(|first_child| {
                                    self.is_global_function_declaration(t, first_child)
                                })
                        };
                        self.op_global_name(
                            t,
                            var,
                            &name,
                            n,
                            in_global_hoist_scope,
                            is_declaration,
                        );
                    }
                }
            }

            _ => {}
        }
        if fall_through_to_object_pattern {
            // `var x = {a: 1, b: 2}` and `var {a: x, b: y} = obj;`
            // should count as a use of property a and b.
            let mut prop_node = n.get_first_child(t);
            while let Some(p) = prop_node {
                match p.get_token(t) {
                    Token::COMPUTED_PROP
                    | Token::ITER_REST
                    | Token::OBJECT_REST
                    | Token::ITER_SPREAD
                    | Token::OBJECT_SPREAD => {}

                    Token::STRING_KEY
                    | Token::GETTER_DEF
                    | Token::SETTER_DEF
                    | Token::MEMBER_FUNCTION_DEF => {
                        if !p.is_quoted_string_key(t) {
                            // May be STRING, GET, or SET, but NUMBER isn't interesting.
                            let chunk = t.get_compiler_and_chunk().1.map(ChunkIndex::of);
                            self.op_symbol_use(
                                &p.get_string(t),
                                chunk,
                                AnalyzePrototypeProperties::PROPERTY,
                            );
                        }
                    }

                    _ => {
                        panic!(
                            "Unexpected child of {}: {}",
                            n.get_token(t),
                            p.to_string_tree(t)
                        );
                    }
                }
                prop_node = p.get_next(t);
            }
        }
        // Process prototype assignments to non-functions (in `visit`).
        true
    }

    /// Rust-only: the name info a symbol use from here is attributed to (`addSymbolUse`): the
    /// innermost context that is not anonymous.
    fn current_def(&self) -> NameInfoId {
        let mut def = None;
        // Skip all anonymous nodes. We care only about symbols with names.
        for context in &self.symbol_stack {
            def = Some(context.name);
            if context.name != self.outer.anonymous_node {
                break;
            }
        }
        def.unwrap()
    }

    /// Rust-only: a name info as a log refers to it while the statement is traversed.
    fn ref_of(&self, id: NameInfoId) -> Ref {
        if id == self.outer.global_node {
            Ref::GLOBAL
        } else if id == self.outer.extern_node {
            Ref::EXTERN
        } else if id == self.outer.anonymous_node {
            Ref::ANONYMOUS
        } else {
            Ref(u32::try_from(id.0).unwrap())
        }
    }

    fn record(&mut self, op: impl FnOnce(&Self) -> Op) {
        if self.recording.is_some() {
            let op = op(self);
            if let Some(recording) = &mut self.recording {
                recording.ops.push(op);
            }
        }
    }

    fn op_name_info(&mut self, name: &JsString, r#type: SymbolType) -> NameInfoId {
        let info = self.outer.get_name_info_for_name(name, r#type);
        self.record(|_| Op::NameInfo(u32::try_from(info.0).unwrap()));
        info
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#addSymbolUse
    fn op_symbol_use(&mut self, name: &JsString, chunk: Option<ChunkIndex>, r#type: SymbolType) {
        let def = self.current_def();
        let info = self.outer.add_symbol_use(name, chunk, r#type, def);
        self.record(|this| Op::SymbolUse {
            info: u32::try_from(info.0).unwrap(),
            def: this.ref_of(def),
        });
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#addGlobalUseOfSymbol
    fn op_global_use(&mut self, name: &JsString, chunk: Option<ChunkIndex>, r#type: SymbolType) {
        let info = self.outer.add_global_use_of_symbol(name, chunk, r#type);
        self.record(|_| Op::GlobalUse(u32::try_from(info.0).unwrap()));
    }

    fn op_set_references_super(&mut self, info: NameInfoId) {
        self.record(|this| Op::SetReferencesSuper(this.ref_of(info)));
        self.outer.name_infos[info.0].references_super = true;
    }

    fn op_set_reads_closure_variables(&mut self, info: NameInfoId) {
        self.record(|this| Op::SetReadsClosureVariables(this.ref_of(info)));
        self.outer.name_infos[info.0].read_closure_variables = true;
    }

    /// A property declaration of `name` (PROPERTY); `spec` is how the log keeps `symbol`.
    fn op_declare(&mut self, name: &JsString, spec: SymbolSpec, symbol: Symbol) {
        let info = self
            .outer
            .declare(name, AnalyzePrototypeProperties::PROPERTY, symbol);
        if let Some(recording) = &mut self.recording {
            let symbol = u32::try_from(recording.symbols.len()).unwrap();
            recording.symbols.push(spec);
            recording.ops.push(Op::Declare {
                info: u32::try_from(info.0).unwrap(),
                symbol,
            });
        }
    }

    /// The NAME branch of `visit` for a NAME that resolves to a global var or to none.
    fn op_global_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        var: Option<VarId>,
        name: &JsString,
        node: NodeId,
        in_global_hoist_scope: bool,
        is_declaration: bool,
    ) {
        let def = self.current_def();
        self.record(|this| Op::GlobalName {
            node,
            in_global_hoist_scope,
            is_declaration,
            def: this.ref_of(def),
        });
        let (compiler, chunk) = t.get_compiler_and_chunk();
        self.outer.global_name(
            compiler,
            chunk,
            var,
            name,
            node,
            in_global_hoist_scope,
            is_declaration,
            def,
        );
    }

    /// The root var of a property declaration, and how a log keeps it (a var without a declaring
    /// name node makes the statement's log unusable).
    fn root_var_of(
        &mut self,
        compiler: &AbstractCompiler,
        name_node: NodeId,
        var: Option<VarId>,
    ) -> (Option<RootVar>, RootVarSpec) {
        match var {
            Some(var) if !var.is_global(compiler) => match var.get_name_node(compiler) {
                Some(name_node) => (
                    Some(RootVar::Local(name_node)),
                    RootVarSpec::Local(name_node),
                ),
                None => {
                    if let Some(recording) = &mut self.recording {
                        recording.cacheable = false;
                    }
                    (Some(RootVar::Implicit(var)), RootVarSpec::None)
                }
            },
            _ => (var.map(RootVar::Global), RootVarSpec::Global(name_node)),
        }
    }

    /// Rust-only: the entry of `PrototypePropertiesCache::names` of a name info of this run.
    fn global_index_of(&mut self, id: u32) -> u32 {
        let index = id as usize;
        if self.run_to_global.len() <= index {
            self.run_to_global
                .resize(self.outer.name_infos.len().max(index + 1), u32::MAX);
        }
        if self.run_to_global[index] == u32::MAX {
            let info = &self.outer.name_infos[index];
            let names = &mut self.cache.as_mut().unwrap().names;
            let (global, _) = names.insert_full((info.name.clone(), info.symbol_type));
            let global = u32::try_from(global).unwrap();
            self.run_to_global[index] = global;
            if self.global_to_run.len() <= global as usize {
                self.global_to_run.resize(global as usize + 1, u32::MAX);
            }
            self.global_to_run[global as usize] = id;
        }
        self.run_to_global[index]
    }

    fn ref_to_global(&mut self, r: Ref) -> Ref {
        if r.is_dummy() {
            r
        } else {
            Ref(self.global_index_of(r.0))
        }
    }

    /// Rust-only: ends the log of the statement just visited and keeps it.
    fn finish_recording(&mut self, t: &mut NodeTraversal<'_>) {
        let mut recording = self.recording.take().unwrap();
        if !recording.cacheable {
            return;
        }
        for op in &mut recording.ops {
            *op = match *op {
                Op::NameInfo(info) => Op::NameInfo(self.global_index_of(info)),
                Op::SymbolUse { info, def } => Op::SymbolUse {
                    info: self.global_index_of(info),
                    def: self.ref_to_global(def),
                },
                Op::GlobalUse(info) => Op::GlobalUse(self.global_index_of(info)),
                Op::SetReferencesSuper(r) => Op::SetReferencesSuper(self.ref_to_global(r)),
                Op::SetReadsClosureVariables(r) => {
                    Op::SetReadsClosureVariables(self.ref_to_global(r))
                }
                Op::Declare { info, symbol } => Op::Declare {
                    info: self.global_index_of(info),
                    symbol,
                },
                Op::GlobalName {
                    node,
                    in_global_hoist_scope,
                    is_declaration,
                    def,
                } => Op::GlobalName {
                    node,
                    in_global_hoist_scope,
                    is_declaration,
                    def: self.ref_to_global(def),
                },
            };
        }
        let compiler = t.get_compiler();
        let stamps = std::iter::once(recording.unit)
            .chain(recording.functions)
            .map(|n| (n, SyntacticScopeCache::cached_subtree_stamp(compiler, n)))
            .collect();
        let log = UnitLog {
            script: recording.script,
            stamps,
            flags: recording.flags,
            ops: recording.ops,
            symbols: recording.symbols,
        };
        if self.verify
            && let Some(cached) = self.cache.as_ref().unwrap().units.get(&recording.unit)
            && cached.is_current(compiler, recording.script)
        {
            assert!(
                cached.ops == log.ops && cached.symbols == log.symbols,
                "CLOSURE_RS_PROTOTYPE_PROPERTIES_CACHE=verify: the kept log of {} differs from a \
                 traversal;\nkept ops: {:?}\nkept symbols: {:?}\nnew ops: {:?}\nnew symbols: {:?}",
                recording.unit.to_string(compiler),
                cached.ops,
                cached.symbols,
                log.ops,
                log.symbols
            );
        }
        self.fresh.insert(recording.unit, Arc::new(log));
    }

    /// Rust-only: the name info of this run of an entry of `PrototypePropertiesCache::names`,
    /// made by `getNameInfoForName` at its first use.
    fn name_info_of_global(&mut self, global: u32) -> NameInfoId {
        let index = global as usize;
        if self.global_to_run.len() <= index {
            self.global_to_run.resize(index + 1, u32::MAX);
        }
        if self.global_to_run[index] == u32::MAX {
            let (name, r#type) = self.cache.as_ref().unwrap().names.get_index(index).unwrap();
            let id = self.outer.get_name_info_for_name(name, *r#type);
            let run = u32::try_from(id.0).unwrap();
            self.global_to_run[index] = run;
            if self.run_to_global.len() <= id.0 {
                self.run_to_global
                    .resize(self.outer.name_infos.len().max(id.0 + 1), u32::MAX);
            }
            self.run_to_global[id.0] = global;
        }
        NameInfoId(self.global_to_run[index] as usize)
    }

    fn resolve(&mut self, r: Ref) -> NameInfoId {
        match r {
            Ref::GLOBAL => self.outer.global_node,
            Ref::EXTERN => self.outer.extern_node,
            Ref::ANONYMOUS => self.outer.anonymous_node,
            Ref(global) => self.name_info_of_global(global),
        }
    }

    /// Rust-only (performance divergence): does to the analysis what the traversal of the
    /// statement of `log` did when the log was made (see `PrototypePropertiesCache`).
    fn replay(&mut self, t: &mut NodeTraversal<'_>, log: &UnitLog) {
        let global = t.get_scope();
        let chunk = t.get_chunk();
        let chunk_index = chunk.as_ref().map(ChunkIndex::of);
        let compiler = t.get_compiler();
        let root_var = |compiler: &mut AbstractCompiler, spec: RootVarSpec| match spec {
            RootVarSpec::None => None,
            RootVarSpec::Local(name_node) => Some(RootVar::Local(name_node)),
            RootVarSpec::Global(name_node) => {
                let name = name_node.get_string(compiler);
                global.get_var(compiler, name).map(RootVar::Global)
            }
        };
        for &op in &log.ops {
            match op {
                Op::NameInfo(info) => {
                    self.name_info_of_global(info);
                }
                Op::SymbolUse { info, def } => {
                    let def = self.resolve(def);
                    let info = self.name_info_of_global(info);
                    if def != info {
                        self.outer.symbol_graph.connect(def, chunk_index, info);
                    }
                }
                Op::GlobalUse(info) => {
                    let info = self.name_info_of_global(info);
                    let global_node = self.outer.global_node;
                    self.outer
                        .symbol_graph
                        .connect(global_node, chunk_index, info);
                }
                Op::SetReferencesSuper(r) => {
                    let info = self.resolve(r);
                    self.outer.name_infos[info.0].references_super = true;
                }
                Op::SetReadsClosureVariables(r) => {
                    let info = self.resolve(r);
                    self.outer.name_infos[info.0].read_closure_variables = true;
                }
                Op::Declare { info, symbol } => {
                    let symbol = match log.symbols[symbol as usize] {
                        SymbolSpec::ClassMember { node, var } => {
                            let var = root_var(compiler, var);
                            Symbol::ClassMemberFunction(ClassMemberFunction::new(
                                compiler,
                                node,
                                var,
                                chunk.clone(),
                            ))
                        }
                        SymbolSpec::Assignment {
                            expr,
                            root_var: spec,
                        } => Symbol::AssignmentPrototypeProperty(AssignmentPrototypeProperty::new(
                            expr,
                            root_var(compiler, spec),
                            chunk.clone(),
                        )),
                        SymbolSpec::Literal {
                            value,
                            assign,
                            root_var: spec,
                        } => Symbol::LiteralPrototypeProperty(LiteralPrototypeProperty::new(
                            value,
                            assign,
                            root_var(compiler, spec),
                            chunk.clone(),
                        )),
                    };
                    let info = self.name_info_of_global(info);
                    self.outer.name_infos[info.0]
                        .get_declarations_mut()
                        .push_back(symbol);
                }
                Op::GlobalName {
                    node,
                    in_global_hoist_scope,
                    is_declaration,
                    def,
                } => {
                    let def = self.resolve(def);
                    let name = node.get_string(compiler);
                    let var = global.get_var(compiler, name.clone());
                    self.outer.global_name(
                        compiler,
                        chunk.as_ref(),
                        var,
                        &name,
                        node,
                        in_global_hoist_scope,
                        is_declaration,
                        def,
                    );
                }
            }
        }
    }

    /// If this is a non-function prototype assign, return the prop name. Otherwise, return null.
    // port: AnalyzePrototypeProperties.ProcessProperties#processNonFunctionPrototypeAssign
    fn process_non_function_prototype_assign(
        &mut self,
        ast: &Ast,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> Option<JsString> {
        if Self::is_assign_r_value(ast, n, parent) && !n.is_function(ast) {
            return self.get_prototype_property_name_from_r_value(ast, n);
        }
        None
    }

    /// Determines whether `n` is the FUNCTION node in a global function declaration.
    // port: AnalyzePrototypeProperties.ProcessProperties#isGlobalFunctionDeclaration
    fn is_global_function_declaration(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        // Make sure we're not in a function scope, or if we are then the function we're looking
        // at is defined in the global scope.
        if !(t.in_global_hoist_scope()
            || (n.is_function(t) && t.get_scope_root() == Some(n) && {
                let scope = t.get_scope();
                let compiler = t.get_compiler();
                scope
                    .get_parent(compiler)
                    .unwrap()
                    .get_closest_hoist_scope(compiler)
                    .unwrap()
                    .is_global(compiler)
            }))
        {
            return false;
        }

        NodeUtil::is_function_declaration(t, n)
            || (n.is_function(t) && n.get_parent(t).unwrap().is_name(t))
    }

    /// Returns true if this is the r-value of an assignment.
    // port: AnalyzePrototypeProperties.ProcessProperties#isAssignRValue
    fn is_assign_r_value(ast: &Ast, n: NodeId, parent: Option<NodeId>) -> bool {
        parent.is_some_and(|parent| parent.is_assign(ast) && parent.get_first_child(ast) != Some(n))
    }

    /// Returns the name of a prototype property being assigned to this r-value.
    ///
    /// Returns null if this is not the R-value of a prototype property, or if the R-value is used
    /// in multiple expressions (i.e., if there's a prototype property assignment in a more complex
    /// expression).
    // port: AnalyzePrototypeProperties.ProcessProperties#getPrototypePropertyNameFromRValue
    fn get_prototype_property_name_from_r_value(
        &mut self,
        ast: &Ast,
        r_value: NodeId,
    ) -> Option<JsString> {
        let l_value = NodeUtil::get_best_l_value(ast, r_value);
        let l_value = l_value?;
        if !((NodeUtil::may_be_object_lit_key(ast, l_value) && !l_value.is_quoted_string_key(ast))
            || NodeUtil::is_expr_assign(ast, l_value.get_grandparent(ast).unwrap()))
        {
            return None;
        }
        // Rust-only (performance divergence): the name of a GETPROP l-value is its qualified name,
        // whose part before the last dot is the qualified name of its owner (a property name has
        // no dot); that part ends with `.prototype` only when the owner is a GETPROP `prototype`.
        // Other GETPROPs are rejected here, before the name is built.
        if l_value.is_get_prop(ast)
            && !l_value.get_first_child(ast).is_some_and(|owner| {
                owner.is_get_prop(ast) && owner.get_string_ref(ast) == "prototype"
            })
        {
            return None;
        }

        let l_value_name = NodeUtil::get_best_l_value_name(ast, Some(l_value))?;

        let last_dot = l_value_name.last_index_of_char(u16::from(b'.'));
        if last_dot == -1 {
            return None;
        }
        let last_dot = last_dot as usize;

        let first_part = l_value_name.substring(0, last_dot);
        if !first_part.ends_with(".prototype") {
            return None;
        }

        Some(l_value_name.substring(last_dot + 1, l_value_name.length()))
    }

    /// Examines a qualified name ending in `.prototype`.
    ///
    /// If it is part of an assignment like `foo.prototype = {}` or `foo.prototype.bar = x`,
    /// record this reference as the definition of one or more prototype properties and return
    /// `true`.
    ///
    /// @param ref A reference to some qualified name that ends with `.prototype`
    /// @return True if a declaration was added.
    // port: AnalyzePrototypeProperties.ProcessProperties#handlePossibleAssignmentToPrototype
    fn handle_possible_assignment_to_prototype(
        &mut self,
        t: &mut NodeTraversal<'_>,
        r#ref: NodeId,
    ) -> bool {
        let root = NodeUtil::get_root_of_qualified_name(t, r#ref);

        let n = r#ref.get_parent(t).unwrap();
        match n.get_token(t) {
            Token::GETPROP => {
                // Foo.prototype.getBar = function() { ... }
                let parent = n.get_parent(t).unwrap();
                let grand_parent = parent.get_parent(t).unwrap();

                if NodeUtil::is_expr_assign(t, grand_parent)
                    && NodeUtil::is_name_decl_or_simple_assign_lhs(t, n, parent)
                {
                    let (root_var, spec) = self.maybe_get_var(t, root);
                    let chunk = t.get_chunk();
                    let prop = Symbol::AssignmentPrototypeProperty(
                        AssignmentPrototypeProperty::new(grand_parent, root_var, chunk),
                    );
                    self.op_declare(
                        &n.get_string(t),
                        SymbolSpec::Assignment {
                            expr: grand_parent,
                            root_var: spec,
                        },
                        prop,
                    );
                    return true;
                }
            }
            Token::ASSIGN => {
                // Foo.prototype = { "getBar" : function() { ... } }
                let map = n.get_second_child(t).unwrap();
                if map.is_object_lit(t) {
                    let mut key = map.get_first_child(t);
                    while let Some(k) = key {
                        if !k.is_quoted_string_key(t) && !k.is_computed_prop(t) {
                            // We won't consider quoted or computed properties for any kind of
                            // modification, so key may be STRING_KEY, GETTER_DEF, SETTER_DEF, or
                            // MEMBER_FUNCTION_DEF
                            let name = k.get_string(t);
                            let (root_var, spec) = self.maybe_get_var(t, root);
                            let chunk = t.get_chunk();
                            let value = k.get_first_child(t).unwrap();
                            let prop = Symbol::LiteralPrototypeProperty(
                                LiteralPrototypeProperty::new(value, n, root_var, chunk),
                            );
                            self.op_declare(
                                &name,
                                SymbolSpec::Literal {
                                    value,
                                    assign: n,
                                    root_var: spec,
                                },
                                prop,
                            );
                        }
                        key = k.get_next(t);
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#processMemberDef
    fn process_member_def(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        check_state!(n.is_member_function_def(t) || n.is_getter_def(t) || n.is_setter_def(t));
        let name = n.get_string(t);
        // Don't want to add a declaration for constructors and static members
        // so they aren't removed
        if NodeUtil::is_es6_constructor_member_function_def(t, n) || n.is_static_member(t) {
            return;
        }

        let class_name_node = NodeUtil::get_name_node(t, n.get_grandparent(t).unwrap());
        let (var, spec) = match class_name_node {
            Some(class_name_node) if class_name_node.is_name(t) => {
                let scope = t.get_scope();
                let class_name = class_name_node.get_string(t);
                let var = scope.get_var(t.get_compiler(), class_name);
                self.root_var_of(t.get_compiler(), class_name_node, var)
            }
            _ => (None, RootVarSpec::None),
        };
        let chunk = t.get_chunk();
        let symbol = Symbol::ClassMemberFunction(ClassMemberFunction::new(t, n, var, chunk));
        self.op_declare(
            &name,
            SymbolSpec::ClassMember { node: n, var: spec },
            symbol,
        );
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#maybeGetVar
    fn maybe_get_var(
        &mut self,
        t: &mut NodeTraversal<'_>,
        maybe_name: NodeId,
    ) -> (Option<RootVar>, RootVarSpec) {
        if maybe_name.is_name(t) {
            let scope = t.get_scope();
            let name = maybe_name.get_string(t);
            let var = scope.get_var(t.get_compiler(), name);
            self.root_var_of(t.get_compiler(), maybe_name, var)
        } else {
            (None, RootVarSpec::None)
        }
    }
}

impl AnalyzePrototypeProperties {
    // port: AnalyzePrototypeProperties.ProcessProperties#addSymbolUse
    /// Rust-only parameter: `def`, the name info the use is attributed to (`current_def`).
    fn add_symbol_use(
        &mut self,
        name: &JsString,
        chunk: Option<ChunkIndex>,
        r#type: SymbolType,
        def: NameInfoId,
    ) -> NameInfoId {
        let info = self.get_name_info_for_name(name, r#type);
        if def != info {
            self.symbol_graph.connect(def, chunk, info);
        }
        info
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#addGlobalUseOfSymbol
    fn add_global_use_of_symbol(
        &mut self,
        name: &JsString,
        chunk: Option<ChunkIndex>,
        r#type: SymbolType,
    ) -> NameInfoId {
        let global_node = self.global_node;
        let name_info = self.get_name_info_for_name(name, r#type);
        self.symbol_graph.connect(global_node, chunk, name_info);
        name_info
    }

    /// Rust-only: adds a declaration of the property or var `name`.
    fn declare(&mut self, name: &JsString, r#type: SymbolType, symbol: Symbol) -> NameInfoId {
        let name_info = self.get_name_info_for_name(name, r#type);
        self.name_infos[name_info.0]
            .get_declarations_mut()
            .push_back(symbol);
        name_info
    }

    /// The NAME branch of `ProcessProperties#visit` for a NAME `node` that resolves to the global
    /// var `var` or to none (Rust-only shape: `is_declaration` is the test of
    /// `processGlobalFunctionDeclaration`, `def` the name info a use is attributed to).
    #[allow(clippy::too_many_arguments)]
    fn global_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        chunk: Option<&JSChunk>,
        var: Option<VarId>,
        name: &JsString,
        node: NodeId,
        in_global_hoist_scope: bool,
        is_declaration: bool,
        def: NameInfoId,
    ) {
        let Some(var) = var else {
            return;
        };
        // Only process global functions.
        if !var.is_global(compiler) {
            return;
        }
        let initial_value = var.get_initial_value(compiler);
        if !initial_value.is_some_and(|value| value.is_function(compiler)) {
            return;
        }
        let chunk_index = chunk.map(ChunkIndex::of);
        if in_global_hoist_scope {
            // port: AnalyzePrototypeProperties.ProcessProperties#processGlobalFunctionDeclaration
            if is_declaration {
                let symbol = Symbol::GlobalFunction(GlobalFunction::new(
                    compiler,
                    node,
                    var,
                    chunk.cloned(),
                ));
                self.declare(name, Self::VAR, symbol);

                // If the function name is exported, we should create an edge here
                // so that it's never removed.
                if compiler
                    .get_coding_convention()
                    .is_exported(name, /* local= */ false)
                    || self.anchor_unused_vars
                {
                    self.add_global_use_of_symbol(name, chunk_index, Self::VAR);
                }
            } else {
                self.add_global_use_of_symbol(name, chunk_index, Self::VAR);
            }
        } else {
            self.add_symbol_use(name, chunk_index, Self::VAR, def);
        }
    }
}

struct ProcessExternProperties<'a> {
    outer: &'a mut AnalyzePrototypeProperties,
}

impl ProcessExternProperties<'_> {
    fn mark_extern_property(&mut self, name: &JsString) {
        let extern_node = self.outer.extern_node;
        let first_chunk = self.outer.first_chunk.as_ref().map(ChunkIndex::of);
        let name_info = self
            .outer
            .get_name_info_for_name(name, AnalyzePrototypeProperties::PROPERTY);
        self.outer
            .symbol_graph
            .connect(extern_node, first_chunk, name_info);
    }
}

// port: NodeTraversal.AbstractPostOrderCallback
impl Callback for ProcessExternProperties<'_> {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    /// Rust-only (performance divergence): Java visits every GETPROP of the bundled externs. In
    /// an unchanged `normalize_clean` bundled prefix the only nodes this callback marks are
    /// GETPROPs (no class members or object literal keys), so its `properties` summary lists
    /// their names in traversal order, each once. Marking a name again only adds another edge
    /// from `[extern]` with the same chunk, which changes no NameInfo in the fixed point; the
    /// NameInfos are made in the same order. Statements appended to the script are visited.
    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if *crate::variable_reference_check::SKIP_BUNDLED_EXTERNS
            && let Some((facts, first)) =
                t.bundled_script_prefix(script, closure_rhino::node::SnapshotUse::Syntax)
            && facts.normalize_clean
        {
            for &name in facts.properties {
                self.mark_extern_property(&JsString::from(name));
            }
            t.traverse_script_suffix(script, parent, first, self);
            return false;
        }
        true
    }

    // port: AnalyzePrototypeProperties.ProcessExternProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_get_prop(t) {
            self.mark_extern_property(&n.get_string(t));
        } else if n.is_member_function_def(t) || n.is_getter_def(t) || n.is_setter_def(t) {
            // As of 2019-08-29 the only user of this class is CrossChunkMethodMotion, which never
            // moves static methods, but that could change. So, we're intentionally including
            // static methods, static getters, and static setters here, because there are cases
            // where they could act like prototype properties.
            //
            // e.g.
            // // externs.js
            // class Foo {
            //   static foo() {}
            // }
            //
            // // src.js
            // /** @record */
            // class ObjWithFooMethod {
            //   foo() {}
            // }
            // /** @type {!ObjWithFooMethod} */
            // let objWithFooMethod = Foo; // yes, this is valid
            //
            let extern_node = self.outer.extern_node;
            let first_chunk = self.outer.first_chunk.as_ref().map(ChunkIndex::of);
            let name_info = self
                .outer
                .get_name_info_for_name(&n.get_string(t), AnalyzePrototypeProperties::PROPERTY);
            self.outer
                .symbol_graph
                .connect(extern_node, first_chunk, name_info);
        }
    }
}

/// Rust-only (performance divergence): a chunk in the symbol graph's edges and in a `NameInfo`,
/// by its index in the chunk graph (a chunk's index is its position in
/// `JSChunkGraph#getAllChunks`), so the fixed-point propagation copies it without a reference
/// count (Java shares the object). Within the chunk graph, equal indices are the same chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkIndex(i32);

impl ChunkIndex {
    fn of(chunk: &JSChunk) -> Self {
        Self(chunk.get_index())
    }

    fn get(self, chunk_graph: &JSChunkGraph) -> &JSChunk {
        &chunk_graph.get_all_chunks()[usize::try_from(self.0).expect("a chunk of the chunk graph")]
    }
}

struct PropagateReferences<'a> {
    chunk_graph: Option<&'a JSChunkGraph>,
    name_infos: &'a mut Vec<NameInfo>,
}

impl<G> EdgeCallback<NameInfoId, Option<ChunkIndex>, G> for PropagateReferences<'_> {
    // port: AnalyzePrototypeProperties.PropagateReferences#traverseEdge
    fn traverse_edge(
        &mut self,
        _graph: &mut G,
        start: NameInfoId,
        edge: Option<ChunkIndex>,
        dest: NameInfoId,
    ) -> bool {
        if self.name_infos[start.0].is_referenced() {
            let start_chunk = self.name_infos[start.0].deepest_common_chunk_ref;
            if let Some(start_chunk) = start_chunk.filter(|start_chunk| {
                let chunk_graph = self.chunk_graph.unwrap();
                chunk_graph.depends_on(start_chunk.get(chunk_graph), edge.unwrap().get(chunk_graph))
            }) {
                self.name_infos[dest.0].mark_reference(self.chunk_graph, Some(start_chunk))
            } else {
                self.name_infos[dest.0].mark_reference(self.chunk_graph, edge)
            }
        } else {
            false
        }
    }
}

/// The root var of a symbol (Java keeps the `Var`). Rust-only shape (performance divergence): a
/// var of a function or block scope is kept by its declaring name node, which identifies the var
/// as its handle would within one run (one scope per function or block, one var per declaring
/// name node), so that a symbol replayed from a log (`PrototypePropertiesCache`) can name the
/// var without the scope; a local var without a declaring name node (an implicit var) keeps its
/// handle. Java reads the var only to ask whether it is global, for the name node of a global
/// var, and to compare two root vars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootVar {
    Global(VarId),
    Local(NodeId),
    Implicit(VarId),
}

impl RootVar {
    /// `var.isGlobal()`.
    pub fn is_global(self) -> bool {
        matches!(self, RootVar::Global(_))
    }

    /// The var, when it is global.
    pub fn global_var(self) -> Option<VarId> {
        match self {
            RootVar::Global(var) => Some(var),
            _ => None,
        }
    }
}

/// The declaration of an abstract symbol (Java interfaces Symbol, Property and PrototypeProperty
/// with their four implementations).
pub enum Symbol {
    GlobalFunction(GlobalFunction),
    ClassMemberFunction(ClassMemberFunction),
    AssignmentPrototypeProperty(AssignmentPrototypeProperty),
    LiteralPrototypeProperty(LiteralPrototypeProperty),
}

impl Symbol {
    /// The variable for the root of this symbol.
    // port: AnalyzePrototypeProperties.Symbol#getRootVar
    pub fn get_root_var(&self) -> Option<RootVar> {
        match self {
            Symbol::GlobalFunction(s) => s.get_root_var(),
            Symbol::ClassMemberFunction(s) => s.get_root_var(),
            Symbol::AssignmentPrototypeProperty(s) => s.get_root_var(),
            Symbol::LiteralPrototypeProperty(s) => s.get_root_var(),
        }
    }

    /// Returns the chunk where this appears.
    // port: AnalyzePrototypeProperties.Symbol#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        match self {
            Symbol::GlobalFunction(s) => s.get_chunk(),
            Symbol::ClassMemberFunction(s) => s.get_chunk(),
            Symbol::AssignmentPrototypeProperty(s) => s.get_chunk(),
            Symbol::LiteralPrototypeProperty(s) => s.get_chunk(),
        }
    }

    /// Java `symbol instanceof Property`.
    pub fn is_property(&self) -> bool {
        !matches!(self, Symbol::GlobalFunction(_))
    }

    /// Java `symbol instanceof PrototypeProperty`.
    pub fn is_prototype_property(&self) -> bool {
        matches!(
            self,
            Symbol::AssignmentPrototypeProperty(_) | Symbol::LiteralPrototypeProperty(_)
        )
    }

    /// Returns the GETPROP node that refers to the prototype.
    // port: AnalyzePrototypeProperties.PrototypeProperty#getPrototype
    pub fn get_prototype(&self, ast: &Ast) -> NodeId {
        match self {
            Symbol::AssignmentPrototypeProperty(s) => s.get_prototype(ast),
            Symbol::LiteralPrototypeProperty(s) => s.get_prototype(ast),
            _ => panic!("not a PrototypeProperty"),
        }
    }

    /// Returns the value of this property.
    // port: AnalyzePrototypeProperties.PrototypeProperty#getValue
    pub fn get_value(&self, ast: &Ast) -> NodeId {
        match self {
            Symbol::AssignmentPrototypeProperty(s) => s.get_value(ast),
            Symbol::LiteralPrototypeProperty(s) => s.get_value(),
            _ => panic!("not a PrototypeProperty"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SymbolType {
    PROPERTY,
    VAR,
}

/// A function initialized as a VAR statement or LET AND CONST and global or a function
/// declaration.
pub struct GlobalFunction {
    var: VarId,
    chunk: Option<JSChunk>,
}

impl GlobalFunction {
    // port: AnalyzePrototypeProperties.GlobalFunction#GlobalFunction
    fn new(
        compiler: &AbstractCompiler,
        name_node: NodeId,
        var: VarId,
        chunk: Option<JSChunk>,
    ) -> Self {
        let parent = name_node.get_parent(compiler);
        check_state!(
            (NodeUtil::is_name_declaration(compiler, parent) && var.is_global(compiler))
                || NodeUtil::is_function_declaration(compiler, parent.unwrap())
        );
        Self { var, chunk }
    }

    // port: AnalyzePrototypeProperties.GlobalFunction#getRootVar
    pub fn get_root_var(&self) -> Option<RootVar> {
        // A global function's var is global (`ProcessProperties#visit` makes one only for a
        // global var).
        Some(RootVar::Global(self.var))
    }

    // port: AnalyzePrototypeProperties.GlobalFunction#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }
}

pub struct ClassMemberFunction {
    node: NodeId,
    var: Option<RootVar>,
    chunk: Option<JSChunk>,
}

impl ClassMemberFunction {
    // port: AnalyzePrototypeProperties.ClassMemberFunction#ClassMemberFunction
    fn new(ast: &Ast, node: NodeId, var: Option<RootVar>, chunk: Option<JSChunk>) -> Self {
        check_state!(node.get_parent(ast).unwrap().is_class_members(ast));
        check_state!(
            node.is_member_function_def(ast) || node.is_setter_def(ast) || node.is_getter_def(ast)
        );
        Self { node, var, chunk }
    }

    // port: AnalyzePrototypeProperties.ClassMemberFunction#getRootVar
    pub fn get_root_var(&self) -> Option<RootVar> {
        self.var
    }

    // port: AnalyzePrototypeProperties.ClassMemberFunction#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }

    /// Returns the function node within the definition.
    // port: AnalyzePrototypeProperties.ClassMemberFunction#getFunctionNode
    pub fn get_function_node(&self, ast: &Ast) -> NodeId {
        self.node.get_only_child(ast)
    }

    /// Returns the MEMBER_FUNCTION_DEF, GETTER_DEF, or SETTER_DEF node that defines the property.
    // port: AnalyzePrototypeProperties.ClassMemberFunction#getDefinitionNode
    pub fn get_definition_node(&self) -> NodeId {
        self.node
    }
}

/// Properties created via EXPR assignment:
///
/// ```text
/// function Foo() { ... };
/// Foo.prototype.bar = function() { ... };
/// ```
pub struct AssignmentPrototypeProperty {
    expr_node: NodeId,
    root_var: Option<RootVar>,
    chunk: Option<JSChunk>,
}

impl AssignmentPrototypeProperty {
    /// @param node An EXPR node.
    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#AssignmentPrototypeProperty
    fn new(node: NodeId, root_var: Option<RootVar>, chunk: Option<JSChunk>) -> Self {
        Self {
            expr_node: node,
            root_var,
            chunk,
        }
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getRootVar
    pub fn get_root_var(&self) -> Option<RootVar> {
        self.root_var
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getPrototype
    pub fn get_prototype(&self, ast: &Ast) -> NodeId {
        self.get_assign_node(ast)
            .get_first_first_child(ast)
            .unwrap()
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getValue
    pub fn get_value(&self, ast: &Ast) -> NodeId {
        self.get_assign_node(ast).get_last_child(ast).unwrap()
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getAssignNode
    fn get_assign_node(&self, ast: &Ast) -> NodeId {
        self.expr_node.get_first_child(ast).unwrap()
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }
}

/// Properties created via object literals:
///
/// ```text
/// function Foo() { ... };
/// Foo.prototype = {bar: function() { ... };
/// ```
pub struct LiteralPrototypeProperty {
    value: NodeId,
    assign: NodeId,
    root_var: Option<RootVar>,
    chunk: Option<JSChunk>,
}

impl LiteralPrototypeProperty {
    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#LiteralPrototypeProperty
    fn new(
        value: NodeId,
        assign: NodeId,
        root_var: Option<RootVar>,
        chunk: Option<JSChunk>,
    ) -> Self {
        Self {
            value,
            assign,
            root_var,
            chunk,
        }
    }

    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#getRootVar
    pub fn get_root_var(&self) -> Option<RootVar> {
        self.root_var
    }

    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#getPrototype
    pub fn get_prototype(&self, ast: &Ast) -> NodeId {
        self.assign.get_first_child(ast).unwrap()
    }

    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#getValue
    pub fn get_value(&self) -> NodeId {
        self.value
    }

    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }
}

/// The context of the current name. This includes the NameInfo and the scope if it is a scope
/// defining name (function).
#[derive(Clone, Copy)]
struct NameContext {
    name: NameInfoId,

    // If this is a function context, then scope will be the scope of the
    // corresponding function. Otherwise, it will be null.
    scope: Option<ScopeId>,
}

impl NameContext {
    // port: AnalyzePrototypeProperties.NameContext#NameContext
    fn new(name: NameInfoId, scope: Option<ScopeId>) -> Self {
        Self { name, scope }
    }
}

/// Information on all properties or global variables of a given name.
pub struct NameInfo {
    pub name: JsString,
    /// Rust-only: whether this is the name info of a property or of a var (Java keeps the two
    /// kinds in separate maps), for a log to name it.
    symbol_type: SymbolType,

    referenced: bool,
    declarations: VecDeque<Symbol>,
    deepest_common_chunk_ref: Option<ChunkIndex>,

    // True if this property is a function that reads a variable from an
    // outer scope which isn't the global scope.
    read_closure_variables: bool,

    // does the definition refer to `super`?
    // We cannot move references to `super` outside of the class body.
    references_super: bool,
}

impl NameInfo {
    /// Constructs a new NameInfo.
    ///
    /// @param name The name of the property that this represents. May be null to signify dummy
    ///     nodes in the property graph.
    // port: AnalyzePrototypeProperties.NameInfo#NameInfo
    fn new(name: JsString, symbol_type: SymbolType) -> Self {
        Self {
            name,
            symbol_type,
            referenced: false,
            declarations: VecDeque::new(),
            deepest_common_chunk_ref: None,
            read_closure_variables: false,
            references_super: false,
        }
    }

    /// Determines whether we've marked a reference to this property name.
    // port: AnalyzePrototypeProperties.NameInfo#isReferenced
    pub fn is_referenced(&self) -> bool {
        self.referenced
    }

    /// Determines whether it reads a closure variable.
    // port: AnalyzePrototypeProperties.NameInfo#readsClosureVariables
    pub fn reads_closure_variables(&self) -> bool {
        self.read_closure_variables
    }

    /// Does the definition refer to `super`?
    // port: AnalyzePrototypeProperties.NameInfo#referencesSuper
    pub fn references_super(&self) -> bool {
        self.references_super
    }

    /// Mark a reference in a given chunk to this property name, and record the deepest common
    /// chunk reference.
    ///
    /// @param chunk The chunk where it was referenced.
    /// @return Whether the name info has changed.
    // port: AnalyzePrototypeProperties.NameInfo#markReference
    fn mark_reference(
        &mut self,
        chunk_graph: Option<&JSChunkGraph>,
        chunk: Option<ChunkIndex>,
    ) -> bool {
        let mut has_changed = false;
        if !self.referenced {
            self.referenced = true;
            has_changed = true;
        }

        let original_deepest_common = self.deepest_common_chunk_ref;

        if let Some(deepest_common_chunk_ref) = self.deepest_common_chunk_ref {
            // JSChunkGraph#getDeepestCommonDependencyInclusive, keeping the chunk's index.
            let chunk_graph = chunk_graph.unwrap();
            let m1 = deepest_common_chunk_ref.get(chunk_graph);
            let m2 = chunk.unwrap().get(chunk_graph);
            self.deepest_common_chunk_ref = if m2 == m1 || chunk_graph.depends_on(m2, m1) {
                Some(deepest_common_chunk_ref)
            } else if chunk_graph.depends_on(m1, m2) {
                chunk
            } else {
                chunk_graph
                    .get_deepest_common_dependency(m1, m2)
                    .as_ref()
                    .map(ChunkIndex::of)
            };
        } else {
            self.deepest_common_chunk_ref = chunk;
        }

        if original_deepest_common != self.deepest_common_chunk_ref {
            has_changed = true;
        }

        has_changed
    }

    /// Returns the deepest common chunk of all the references to this property.
    // port: AnalyzePrototypeProperties.NameInfo#getDeepestCommonChunkRef
    /// Rust-only parameter: the chunk graph the analysis ran with (see `ChunkIndex`).
    pub fn get_deepest_common_chunk_ref<'a>(
        &self,
        chunk_graph: &'a JSChunkGraph,
    ) -> Option<&'a JSChunk> {
        self.deepest_common_chunk_ref
            .map(|chunk| chunk.get(chunk_graph))
    }

    /// Returns a mutable collection of all the prototype property declarations of this property
    /// name.
    // port: AnalyzePrototypeProperties.NameInfo#getDeclarations
    pub fn get_declarations(&self) -> &VecDeque<Symbol> {
        &self.declarations
    }

    fn get_declarations_mut(&mut self) -> &mut VecDeque<Symbol> {
        &mut self.declarations
    }
}

impl std::fmt::Display for NameInfo {
    // port: AnalyzePrototypeProperties.NameInfo#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name.to_string_lossy())
    }
}
