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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/GlobalNamespace.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `GlobalNamespace.java`.
//!
//! Builds a namespace of all qualified names whose root is in the global scope or a module, plus
//! an index of all references to those global names.
//!
//! Java's `Name` (an inner class) and `Ref` objects are arena entries owned by the
//! `GlobalNamespace`; [`Name`] and [`Ref`] are `Copy` handles whose equality is Java object
//! identity (DESIGN.md §3 and the Scope/Var precedent). Their methods take the namespace right
//! after the receiver (`name.get_full_name(gn)`), then the compiler where Java's inner class used
//! `GlobalNamespace.this.compiler` or a node. As for `Scope`/`Var`, Rhino's `StaticScope`,
//! `StaticSlot`, `StaticRef` and `StaticSymbolTable` traits return trait-object references that an
//! arena handle cannot provide; the same methods exist on the handles with the Java names.
//!
//! Java shares one `LogFile` between `InlineAndCollapseProperties` and its `GlobalNamespace`; the
//! Rust namespace holds it as a shared [`DecisionsLog`]. The spread-sibling cache is a pure cache
//! (DESIGN.md §8) behind a `Mutex`, so the read-only inlinability queries take `&GlobalNamespace`.
use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_scope::AbstractScope,
    abstract_var::AbstractVar,
    diagnostic::log_file::LogFile,
    js_chunk::JSChunk,
    js_chunk_graph::JSChunkGraph,
    module_import_resolver::ModuleImportResolver,
    modules::module_metadata_map::ModuleMetadata,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    scope::ScopeId,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId, Prop},
    qualified_name::QualifiedName,
    static_source_file::StaticSourceFile,
    token::Token,
};
use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::{Arc, LazyLock, Mutex},
};

/// `Predicate<Node>` limiting the traversal to some scripts.
pub type ScriptPredicate = dyn Fn(&AbstractCompiler, NodeId) -> bool + Send + Sync;

/// The `LogFile` Java shares between the creating pass and the namespace.
pub type DecisionsLog = Arc<Mutex<Box<dyn LogFile>>>;

static GOOG_PROVIDE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.provide"));

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceKind {
    EXTERN,
    TYPE_SUMMARY,
    CODE,
}

impl SourceKind {
    // port: GlobalNamespace.SourceKind#fromScriptNode
    pub fn from_script_node(ast: &Ast, n: NodeId) -> SourceKind {
        if !n.is_from_externs(ast) {
            SourceKind::CODE
        } else if NodeUtil::is_from_type_summary(ast, n) {
            SourceKind::TYPE_SUMMARY
        } else {
            SourceKind::EXTERN
        }
    }
}

/// Java's `ModuleMetadata` uses reference equality (`equals` is `Object#equals`).
#[derive(Clone)]
struct ModuleMetadataKey(Arc<ModuleMetadata>);

impl PartialEq for ModuleMetadataKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ModuleMetadataKey {}
impl Hash for ModuleMetadataKey {
    fn hash<H: Hasher>(&self, h: &mut H) {
        Arc::as_ptr(&self.0).hash(h);
    }
}

/// A handle to a `GlobalNamespace.Name`. Equality is Java object identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(u32);

/// A handle to a `GlobalNamespace.Ref`. Equality is Java object identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ref(u32);

/// Java's `Object refsForNode`: null, a singleton `Ref` or a `LinkedHashMap<Node, Ref>`.
enum RefsForNode {
    Null,
    Single(Ref),
    // A Java LinkedHashMap accepts the null key a singleton Ref with a null node brings along.
    Map(IndexMap<Option<NodeId>, Ref>),
}

struct NameData {
    base_name: JsString,
    parent: Option<Name>,
    // The children of this name. Must be null if there are no children.
    props: Option<Vec<Name>>,
    // The declaration of a name.
    declaration: Option<Ref>,
    // The first global assignment to a name.
    initialization: Option<Ref>,
    refs_for_node: RefsForNode,
    global_sets: i32,
    local_sets: i32,
    local_sets_with_no_collapse: i32,
    aliasing_gets: i32,
    total_gets: i32,
    call_gets: i32,
    delete_props: i32,
    subclassing_gets: i32,
    // Bitset containing NameProps
    property_bit_set: i32,
    // Will be set to the JSDocInfo associated with the first SET_FROM_GLOBAL reference added
    // that has JSDocInfo.
    first_declaration_jsdoc_info: Option<Arc<JSDocInfo>>,
    // Will be set to the JSDocInfo associated with the first get reference that is a statement
    // by itself.
    first_qname_declaration_without_assignment_jsdoc_info: Option<Arc<JSDocInfo>>,
}

struct RefData {
    // Not final because CollapseProperties needs to update the namespace in-place.
    node: Option<NodeId>,
    r#type: RefType,
    chunk: Option<JSChunk>,
    // The scope in which the reference is resolved. Note that for ALIASING_GETS like
    // "var x = ns;" this scope may not be the correct hoist scope of the aliasing VAR.
    scope: Option<ScopeId>,
}

pub struct GlobalNamespace {
    enable_implicitly_aliased_values: bool,
    root: NodeId,
    externs_root: Option<NodeId>,
    global_root: NodeId,
    spread_sibling_cache: Mutex<IndexMap<NodeId, bool>>,
    source_kind: Option<SourceKind>,
    generated: bool,
    // Records decisions made by this class.
    decisions_log: Option<DecisionsLog>,
    // Global namespace tree
    global_names: Vec<Name>,
    // Maps names (e.g. "a.b.c") to nodes in the global namespace tree
    name_map: IndexMap<JsString, Name>,
    // Maps names (e.g. "a.b.c") and MODULE_BODY nodes to Names in that module
    name_map_by_module: IndexMap<(ModuleMetadataKey, JsString), Name>,
    // Limits traversal to scripts matching the given predicate.
    should_traverse_script: Arc<ScriptPredicate>,
    // Rust-only arenas backing the Name and Ref handles.
    names: Vec<NameData>,
    refs: Vec<RefData>,
}

impl GlobalNamespace {
    // port: GlobalNamespace#GlobalNamespace(LogFile,AbstractCompiler,Node)
    pub fn new_with_decisions_log(
        decisions_log: Option<DecisionsLog>,
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> Self {
        Self::new_full(decisions_log, compiler, None, root)
    }

    // port: GlobalNamespace#GlobalNamespace(AbstractCompiler,Node)
    pub fn new_without_externs(compiler: &mut AbstractCompiler, root: NodeId) -> Self {
        Self::new_full(None, compiler, None, root)
    }

    // port: GlobalNamespace#GlobalNamespace(AbstractCompiler,Node,Node)
    pub fn new(compiler: &mut AbstractCompiler, externs_root: NodeId, root: NodeId) -> Self {
        Self::new_full(None, compiler, Some(externs_root), root)
    }

    // port: GlobalNamespace#GlobalNamespace(LogFile,AbstractCompiler,Node,Node)
    pub fn new_full(
        decisions_log: Option<DecisionsLog>,
        compiler: &mut AbstractCompiler,
        externs_root: Option<NodeId>,
        root: NodeId,
    ) -> Self {
        let global_root = IR::root(compiler, &[]);
        let enable_implicitly_aliased_values = !compiler
            .get_options()
            .get_assume_static_inheritance_is_not_used();
        Self {
            enable_implicitly_aliased_values,
            root,
            externs_root,
            global_root,
            spread_sibling_cache: Mutex::new(IndexMap::<_, _>::default()),
            source_kind: None,
            generated: false,
            decisions_log,
            global_names: Vec::new(),
            name_map: IndexMap::<_, _>::default(),
            name_map_by_module: IndexMap::<_, _>::default(),
            should_traverse_script: Arc::new(|_, _| true),
            names: Vec::new(),
            refs: Vec::new(),
        }
    }

    // port: GlobalNamespace#setShouldTraverseScriptPredicate
    pub fn set_should_traverse_script_predicate(
        &mut self,
        should_traverse_script: Arc<ScriptPredicate>,
    ) {
        self.should_traverse_script = should_traverse_script;
    }

    // port: GlobalNamespace#hasExternsRoot
    pub fn has_externs_root(&self) -> bool {
        self.externs_root.is_some()
    }

    // port: GlobalNamespace#getRootNode()
    pub fn get_root_node(&self, ast: &Ast) -> Option<NodeId> {
        self.root.get_parent(ast)
    }

    /// Returns the root node of the scope in which the root of a qualified name is declared, or
    /// null.
    // port: GlobalNamespace#getRootNode(String,Scope)
    fn get_root_node_for_name(
        &self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
        s: Option<ScopeId>,
    ) -> Option<NodeId> {
        let name = Self::get_top_var_name(name);
        let v = check_not_null!(s).get_var(compiler, &name);
        match v {
            None => {
                let provided_name = self.name_map.get(&name).copied();
                if provided_name
                    .is_some_and(|p| p.get_boolean_property(self, NameProp::IS_PROVIDED))
                {
                    Some(self.global_root)
                } else {
                    None
                }
            }
            Some(v) => {
                if v.is_local(compiler) {
                    Some(AbstractVar::get_scope_root(v, compiler))
                } else {
                    Some(self.global_root)
                }
            }
        }
    }

    // port: GlobalNamespace#getParentScope
    pub fn get_parent_scope(&self) -> Option<&GlobalNamespace> {
        None
    }

    // port: GlobalNamespace#getSlot
    pub fn get_slot(&mut self, compiler: &mut AbstractCompiler, name: &JsString) -> Option<Name> {
        self.get_own_slot(compiler, name)
    }

    // port: GlobalNamespace#getOwnSlot
    pub fn get_own_slot(
        &mut self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
    ) -> Option<Name> {
        self.ensure_generated(compiler);
        self.name_map.get(name).copied()
    }

    // port: GlobalNamespace#getReferences
    pub fn get_references(&mut self, compiler: &mut AbstractCompiler, slot: Name) -> Vec<Ref> {
        self.ensure_generated(compiler);
        slot.get_refs(self)
    }

    // port: GlobalNamespace#getScope
    pub fn get_scope(&self, _slot: Name) -> &GlobalNamespace {
        self
    }

    // port: GlobalNamespace#getAllSymbols
    pub fn get_all_symbols(&mut self, compiler: &mut AbstractCompiler) -> Vec<Name> {
        self.ensure_generated(compiler);
        self.get_name_index(compiler).values().copied().collect()
    }

    // port: GlobalNamespace#ensureGenerated
    fn ensure_generated(&mut self, compiler: &mut AbstractCompiler) {
        if !self.generated {
            self.process(compiler);
        }
    }

    /// Gets a list of the roots of the forest of the global names, where the roots are the
    /// top-level names.
    // port: GlobalNamespace#getNameForest
    pub fn get_name_forest(&mut self, compiler: &mut AbstractCompiler) -> &[Name] {
        self.ensure_generated(compiler);
        &self.global_names
    }

    /// Gets an index of all the global names, indexed by full qualified name (as in "a", "a.b.c",
    /// etc.).
    // port: GlobalNamespace#getNameIndex
    pub fn get_name_index(&mut self, compiler: &mut AbstractCompiler) -> &IndexMap<JsString, Name> {
        self.ensure_generated(compiler);
        &self.name_map
    }

    /// If the client adds new nodes to the AST, scan these new nodes to see if they've added any
    /// references to the global namespace.
    // port: GlobalNamespace#scanNewNodes
    pub fn scan_new_nodes(
        &mut self,
        compiler: &mut AbstractCompiler,
        new_nodes: &IndexSet<AstChange>,
    ) {
        let mut builder = BuildGlobalNamespace::new(self);

        for info in new_nodes {
            if !info.node().is_qualified_name(compiler)
                && !NodeUtil::may_be_object_lit_key(compiler, info.node())
            {
                continue;
            }
            let scope = info.scope(builder.gn);
            let chunk = info.chunk(builder.gn);
            Self::scan_from_node(&mut builder, compiler, scope, info.node(), chunk);
        }
    }

    // port: GlobalNamespace#scanFromNode
    fn scan_from_node(
        builder: &mut BuildGlobalNamespace<'_>,
        compiler: &mut AbstractCompiler,
        scope: Option<ScopeId>,
        n: NodeId,
        chunk: Option<JSChunk>,
    ) {
        // Check affected parent nodes first.
        let parent = n.get_parent(compiler);
        if (n.is_name(compiler) || n.is_get_prop(compiler))
            && check_not_null!(parent).is_get_prop(compiler)
        {
            // e.g. when replacing "my.alias.prop" with "foo.bar.prop"
            // we want also want to visit "foo.bar.prop", since that's a new global qname we are
            // now referencing.
            let p = n.get_parent(compiler).unwrap();
            Self::scan_from_node(builder, compiler, scope, p, chunk.clone());
        } else if n
            .get_previous(compiler)
            .is_some_and(|p| p.is_object_pattern(compiler))
        {
            // e.g. if we change `const {x} = bar` to `const {x} = foo`, add a new reference to
            // `foo.x` attached to the STRING_KEY `x`
            let pattern = n.get_previous(compiler).unwrap();
            let mut key = pattern.get_first_child(compiler);
            while let Some(k) = key {
                if k.is_string_key(compiler) {
                    Self::scan_from_node(builder, compiler, scope, k, chunk.clone());
                }
                key = k.get_next(compiler);
            }
        }
        builder.collect(compiler, scope, chunk, n);
    }

    /// Builds the namespace lazily.
    // port: GlobalNamespace#process
    fn process(&mut self, compiler: &mut AbstractCompiler) {
        let externs_root = self.externs_root;
        let root = self.root;
        {
            let mut callback = BuildGlobalNamespace::new(self);
            let mut traversal = NodeTraversal::builder();
            traversal.set_compiler(compiler).set_callback(&mut callback);

            if let Some(externs_root) = externs_root {
                traversal.traverse_roots(externs_root, root);
            } else {
                traversal.traverse(root);
            }
        }

        self.generated = true;
    }

    /// Gets the top variable name from a possibly namespaced name.
    // port: GlobalNamespace#getTopVarName
    fn get_top_var_name(name: &JsString) -> JsString {
        let first_dot_index = name.index_of_char(u16::from(b'.'));
        if first_dot_index == -1 {
            name.clone()
        } else {
            name.substring(0, first_dot_index as usize)
        }
    }

    // port: GlobalNamespace#getNameFromModule
    pub fn get_name_from_module(
        &mut self,
        compiler: &mut AbstractCompiler,
        module_metadata: &Arc<ModuleMetadata>,
        name: &JsString,
    ) -> Option<Name> {
        self.ensure_generated(compiler);
        self.name_map_by_module
            .get(&(ModuleMetadataKey(module_metadata.clone()), name.clone()))
            .copied()
    }

    /// Returns whether a declaration node, inside an object-literal, has a following
    /// OBJECT_SPREAD sibling.
    ///
    /// This check is implemented using a cache because otherwise it has aggregate O(n^2)
    /// performance in terms of the size of an OBJECTLIT.
    // port: GlobalNamespace#declarationHasFollowingObjectSpreadSibling
    fn declaration_has_following_object_spread_sibling(
        &self,
        ast: &Ast,
        declaration: NodeId,
    ) -> bool {
        check_state!(
            declaration.get_parent(ast).unwrap().is_object_lit(ast),
            "%s",
            declaration.to_string(ast)
        );

        let mut spread_sibling_cache = self.spread_sibling_cache.lock().unwrap();
        let cached = spread_sibling_cache.get(&declaration).copied();
        if let Some(cached) = cached {
            return cached;
        }

        // Iterate backward over all children of the object-literal, filling in the cache.
        //
        // We iterate the entire literal because we expect to eventually need the result for each
        // of them. Additionally, it makes the loop conditions simpler.
        let mut to_cache = false;
        let mut sibling = declaration.get_parent(ast).unwrap().get_last_child(ast);
        while let Some(s) = sibling {
            if s.is_spread(ast) {
                to_cache = true;
            }
            spread_sibling_cache.insert(s, to_cache);
            sibling = s.get_previous(ast);
        }

        *spread_sibling_cache.get(&declaration).unwrap()
    }

    // port: GlobalNamespace#createNameForTesting
    pub fn create_name_for_testing(&mut self, name: impl Into<JsString>) -> Name {
        self.new_name(name.into(), None, Some(SourceKind::CODE))
    }

    /// Given something like the `'c'` STRING_KEY node in `x = {a: {b: {c: 0}}};`, return the Node
    /// for the outermost object literal.
    // port: GlobalNamespace#getOutermostObjectLit
    fn get_outermost_object_lit(&self, ast: &Ast, obj_lit_key: NodeId) -> NodeId {
        let mut outermost_object_lit = obj_lit_key.get_parent(ast).unwrap();
        check_state!(
            outermost_object_lit.is_object_lit(ast),
            "%s",
            outermost_object_lit.to_string(ast)
        );
        loop {
            let obj_lit_grandparent = outermost_object_lit.get_grandparent(ast);
            match obj_lit_grandparent {
                Some(g) if g.is_object_lit(ast) => {
                    outermost_object_lit = g;
                }
                _ => return outermost_object_lit,
            }
        }
    }

    /// True if the given Node is the GETPROP in a statement like `some.q.name;`
    // port: GlobalNamespace#isQnameDeclarationWithoutAssignment
    fn is_qname_declaration_without_assignment(ast: &Ast, node: Option<NodeId>) -> bool {
        node.is_some_and(|node| {
            node.is_get_prop(ast) && node.get_parent(ast).unwrap().is_expr_result(ast)
        })
    }

    // port: GlobalNamespace.Name#Name
    fn new_name(
        &mut self,
        name: JsString,
        parent: Option<Name>,
        source_kind: Option<SourceKind>,
    ) -> Name {
        let id = Name(u32::try_from(self.names.len()).unwrap());
        self.names.push(NameData {
            base_name: name,
            parent,
            props: None,
            declaration: None,
            initialization: None,
            refs_for_node: RefsForNode::Null,
            global_sets: 0,
            local_sets: 0,
            local_sets_with_no_collapse: 0,
            aliasing_gets: 0,
            total_gets: 0,
            call_gets: 0,
            delete_props: 0,
            subclassing_gets: 0,
            property_bit_set: 0,
            first_declaration_jsdoc_info: None,
            first_qname_declaration_without_assignment_jsdoc_info: None,
        });
        id.set_boolean_property(self, NameProp::NOT_A_TYPE);
        id.set_boolean_property(self, NameProp::OTHER_OBJECT);
        if check_not_null!(source_kind) == SourceKind::EXTERN {
            id.set_boolean_property(self, NameProp::IS_EXTERN);
        }
        id
    }

    // port: GlobalNamespace.Ref#Ref
    fn new_ref(
        &mut self,
        chunk: Option<JSChunk>,
        scope: Option<ScopeId>,
        node: Option<NodeId>,
        r#type: RefType,
    ) -> Ref {
        let id = Ref(u32::try_from(self.refs.len()).unwrap());
        self.refs.push(RefData {
            node,
            r#type,
            chunk,
            scope,
        });
        id
    }

    fn name_data(&self, name: Name) -> &NameData {
        &self.names[name.0 as usize]
    }

    fn name_data_mut(&mut self, name: Name) -> &mut NameData {
        &mut self.names[name.0 as usize]
    }

    fn ref_data(&self, r: Ref) -> &RefData {
        &self.refs[r.0 as usize]
    }

    fn ref_data_mut(&mut self, r: Ref) -> &mut RefData {
        &mut self.refs[r.0 as usize]
    }

    // Rust-only: the `decisionsLog != null && decisionsLog.isLogging()` guard shared by
    // Name#logDecision and Name#logChildNamesDecision.
    fn with_decisions_log(&self, f: impl FnOnce(&mut dyn LogFile)) {
        if let Some(decisions_log) = &self.decisions_log {
            let mut decisions_log = decisions_log.lock().unwrap();
            if decisions_log.is_logging() {
                f(&mut **decisions_log);
            }
        }
    }
}

/// `GlobalNamespace.AstChange`: a node to inspect for changes to the global namespace.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum AstChange {
    SimpleAstChange(SimpleAstChange),
    RefBasedAstChange(RefBasedAstChange),
}

impl AstChange {
    // port: GlobalNamespace.AstChange#node
    pub fn node(&self) -> NodeId {
        match self {
            AstChange::SimpleAstChange(c) => c.node,
            AstChange::RefBasedAstChange(c) => c.node,
        }
    }

    // port: GlobalNamespace.AstChange#chunk
    pub fn chunk(&self, gn: &GlobalNamespace) -> Option<JSChunk> {
        match self {
            AstChange::SimpleAstChange(c) => c.chunk.clone(),
            AstChange::RefBasedAstChange(c) => c.chunk(gn),
        }
    }

    // port: GlobalNamespace.AstChange#scope
    pub fn scope(&self, gn: &GlobalNamespace) -> Option<ScopeId> {
        match self {
            AstChange::SimpleAstChange(c) => c.scope,
            AstChange::RefBasedAstChange(c) => c.scope(gn),
        }
    }
}

/// `record SimpleAstChange(Node node, JSChunk chunk, Scope scope)`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SimpleAstChange {
    pub node: NodeId,
    pub chunk: Option<JSChunk>,
    pub scope: Option<ScopeId>,
}

impl SimpleAstChange {
    // port: GlobalNamespace.SimpleAstChange#SimpleAstChange
    pub fn new(node: NodeId, chunk: Option<JSChunk>, scope: Option<ScopeId>) -> Self {
        Self { node, chunk, scope }
    }
}

/// `record RefBasedAstChange(Ref ref, Node node)`: the information necessary to inspect a node
/// for changes to the global namespace.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RefBasedAstChange {
    pub r#ref: Ref,
    pub node: NodeId,
}

impl RefBasedAstChange {
    // port: GlobalNamespace.RefBasedAstChange#RefBasedAstChange
    pub fn new(r#ref: Ref, node: NodeId) -> Self {
        Self { r#ref, node }
    }

    // port: GlobalNamespace.RefBasedAstChange#chunk
    pub fn chunk(&self, gn: &GlobalNamespace) -> Option<JSChunk> {
        self.r#ref.get_chunk(gn)
    }

    // port: GlobalNamespace.RefBasedAstChange#scope
    pub fn scope(&self, gn: &GlobalNamespace) -> Option<ScopeId> {
        self.r#ref.scope(gn)
    }
}

// -------------------------------------------------------------------------

/// Builds a tree representation of the global namespace. Omits prototypes.
struct BuildGlobalNamespace<'g> {
    gn: &'g mut GlobalNamespace,
    cur_module_root: Option<NodeId>,
    cur_metadata: Option<Arc<ModuleMetadata>>,
}

impl Callback for BuildGlobalNamespace<'_> {
    /// Collect the references in pre-order.
    // port: GlobalNamespace.BuildGlobalNamespace#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) && !(self.gn.should_traverse_script)(t.get_compiler(), n) {
            return false;
        }
        if self.gn.has_externs_root() && n.is_script(t) {
            // When checking type-summary files, we want to consider them like normal code
            // for some things (like alias inlining) but like externs for other things.
            self.gn.source_kind = Some(SourceKind::from_script_node(t, n));
        } else if n == self.gn.root {
            self.gn.source_kind = Some(SourceKind::CODE);
        }
        if n.is_module_body(t) || NodeUtil::is_bundled_goog_module_scope_root(t, n) {
            self.setup_module_metadata(t.get_compiler(), n);
        } else if n.is_script(t) || NodeUtil::is_bundled_goog_module_call(t, n) {
            self.cur_module_root = None;
            self.cur_metadata = None;
        }

        let scope = t.get_scope();
        let chunk = t.get_chunk();
        self.collect(t.get_compiler(), Some(scope), chunk, n);

        true
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

impl<'g> BuildGlobalNamespace<'g> {
    // port: GlobalNamespace.BuildGlobalNamespace#BuildGlobalNamespace
    fn new(gn: &'g mut GlobalNamespace) -> Self {
        Self {
            gn,
            cur_module_root: None,
            cur_metadata: None,
        }
    }

    /// Initializes the ModuleMetadata for a goog;.module or ES module
    // port: GlobalNamespace.BuildGlobalNamespace#setupModuleMetadata
    fn setup_module_metadata(&mut self, compiler: &mut AbstractCompiler, module_root: NodeId) {
        let Some(module_map) = compiler.get_module_map().cloned() else {
            return;
        };
        self.cur_module_root = Some(module_root);

        let cur_metadata = check_not_null!(
            ModuleImportResolver::get_module_from_scope_root(
                Some(&module_map),
                compiler,
                module_root
            )
            .map(|module| module.metadata().clone())
        );
        let is_goog_module = cur_metadata.is_goog_module();
        self.cur_metadata = Some(cur_metadata);
        if is_goog_module {
            let metadata = self.cur_metadata.clone();
            self.get_or_create_name(&JsString::from("exports"), metadata.as_ref());
        }
    }

    // port: GlobalNamespace.BuildGlobalNamespace#collect
    fn collect(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope: Option<ScopeId>,
        chunk: Option<JSChunk>,
        n: NodeId,
    ) {
        let parent = n.get_parent(compiler);

        let name: Option<JsString>;
        let mut is_set = false;
        let mut r#type = NameProp::OTHER_OBJECT;

        match n.get_token(compiler) {
            Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF => {
                if check_not_null!(parent).is_class_members(compiler)
                    && !n.is_static_member(compiler)
                {
                    return; // Within a class, only static members define global names.
                }
                name = NodeUtil::get_best_l_value_name(compiler, Some(n));
                is_set = true;

                if n.is_member_function_def(compiler) {
                    r#type = NameProp::FUNCTION;
                } else if n.is_member_field_def(compiler) {
                    r#type = NameProp::OTHER_OBJECT;
                } else {
                    r#type = NameProp::GET_SET;
                }
            }
            Token::STRING_KEY => {
                let mut name_string = None;
                let parent = check_not_null!(parent);
                if parent.is_object_lit(compiler) {
                    let analysis = self.create_obj_lit_string_key_analysis(compiler, n);
                    name_string = analysis.name_string;
                    r#type = analysis.name_type;
                    is_set = true;
                } else if parent.is_object_pattern(compiler) {
                    name_string = self.get_name_for_object_pattern_key(compiler, n);
                    r#type = self.get_value_type(compiler, n.get_first_child(compiler).unwrap());
                    // not a set
                }
                // else not a reference we should record
                name = name_string;
            }
            Token::NAME | Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                // "a.b?.c" is not a reference to the global name "a.b.c" for the
                // purposes of GlobalNamespace, but we include OPTCHAIN_GETPROP to
                // detect optional chain hasOwnProperty guards.
                // TODO(b/127505242): CAST parents may indicate a set.
                // This may be a variable get or set.
                let parent = check_not_null!(parent);
                match parent.get_token(compiler) {
                    Token::VAR | Token::LET | Token::CONST => {
                        is_set = true;
                        let rvalue = n.get_first_child(compiler);
                        r#type = match rvalue {
                            None => NameProp::OTHER_OBJECT,
                            Some(rvalue) => self.get_value_type(compiler, rvalue),
                        };
                    }
                    Token::ASSIGN => {
                        if parent.get_first_child(compiler) == Some(n) {
                            is_set = true;
                            r#type = self.get_value_type(compiler, n.get_next(compiler).unwrap());
                        }
                    }
                    Token::GETPROP => {
                        // This name is nested in a getprop. Return and only create a Ref for the
                        // outermost getprop in the chain.
                        return;
                    }
                    Token::FUNCTION => {
                        let grandparent = parent.get_parent(compiler);
                        if grandparent.is_none()
                            || NodeUtil::is_function_expression(compiler, parent)
                        {
                            return;
                        }
                        is_set = true;
                        r#type = NameProp::FUNCTION;
                    }
                    Token::CATCH | Token::INC | Token::DEC => {
                        is_set = true;
                        r#type = NameProp::OTHER_OBJECT;
                    }
                    Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                        if parent.get_first_child(compiler) == Some(n) {
                            is_set = true;
                            r#type = NameProp::OTHER_OBJECT;
                        }
                    }
                    Token::CLASS => {
                        // The first child is the class name, and the second child is the
                        // superclass name.
                        if parent.get_first_child(compiler) == Some(n) {
                            is_set = true;
                            r#type = NameProp::CLASS;
                        }
                    }
                    Token::STRING_KEY
                    | Token::ARRAY_PATTERN
                    | Token::DEFAULT_VALUE
                    | Token::COMPUTED_PROP
                    | Token::ITER_REST
                    | Token::OBJECT_REST => {
                        // This may be a set.
                        // TODO(b/120303257): this should extend to qnames too, but doing
                        // so causes invalid output. Covered in CollapsePropertiesTest
                        if n.is_name(compiler) && NodeUtil::is_lhs_by_destructuring(compiler, n) {
                            is_set = true;
                            r#type = NameProp::OTHER_OBJECT;
                        }
                    }
                    Token::ITER_SPREAD | Token::OBJECT_SPREAD => {} // isSet = false, type = OTHER.
                    Token::CALL | Token::OPTCHAIN_CALL => {
                        if n.is_first_child_of(compiler, Some(parent))
                            && self.is_object_has_own_property_call(compiler, parent)
                        {
                            let qname = n
                                .get_first_child(compiler)
                                .unwrap()
                                .get_qualified_name(compiler)
                                .unwrap();
                            let metadata = self.cur_metadata.clone();
                            let global_name = self.get_or_create_name(&qname, metadata.as_ref());
                            global_name
                                .set_boolean_property(self.gn, NameProp::IS_USED_HAS_OWN_PROPERTY);
                        }
                    }
                    _ => {
                        if NodeUtil::is_assignment_op(compiler, parent)
                            && parent.get_first_child(compiler) == Some(n)
                        {
                            is_set = true;
                            r#type = NameProp::OTHER_OBJECT;
                        }
                    }
                }
                if !n.is_qualified_name(compiler) {
                    return;
                }
                name = n.get_qualified_name(compiler);
            }
            Token::CALL => {
                if check_not_null!(parent).is_expr_result(compiler)
                    && GOOG_PROVIDE.matches(compiler, n.get_first_child(compiler).unwrap())
                    && n.get_second_child(compiler)
                        .unwrap()
                        .is_string_lit(compiler)
                {
                    // goog.provide goes through a different code path than regular sets because
                    // it can create multiple names, e.g. `goog.provide('a.b.c');` creates the
                    // global names a, a.b, and a.b.c. Other sets only create a single global name.
                    let namespace = n.get_second_child(compiler).unwrap().get_string(compiler);
                    self.create_names_from_provide(&namespace);
                    return;
                }
                return;
            }
            _ => {
                return;
            }
        }

        let Some(name) = name else {
            return;
        };

        let root = self.gn.get_root_node_for_name(compiler, &name, scope);
        // We are only interested in global and module names.
        if !self.is_top_level_scope_root(compiler, root) {
            return;
        }

        let name_metadata = if root == Some(self.gn.global_root) {
            None
        } else {
            self.cur_metadata.clone()
        };
        if is_set {
            // Use the closest hoist scope to select handleSetFromGlobal or handleSetFromLocal
            // because they use the term 'global' in an ES5, pre-block-scoping sense.
            let hoist_scope = check_not_null!(scope)
                .get_closest_hoist_scope(compiler)
                .unwrap();
            // Consider a set to be 'global' if it is in the hoist scope in which the name is
            // defined. For example, a global name set in a module scope is a 'local' set, but a
            // module-level name set in a module scope is a 'global' set.
            if hoist_scope.is_global(compiler)
                || (root != Some(self.gn.global_root)
                    && Some(hoist_scope.get_root_node(compiler)) == self.cur_module_root)
            {
                self.handle_set_from_global(
                    compiler,
                    chunk,
                    scope,
                    n,
                    &name,
                    r#type,
                    name_metadata.as_ref(),
                );
            } else {
                self.handle_set_from_local(
                    compiler,
                    chunk,
                    scope,
                    n,
                    &name,
                    name_metadata.as_ref(),
                );
            }
        } else {
            self.handle_get(compiler, chunk, scope, n, &name, name_metadata.as_ref());
        }
    }

    // port: GlobalNamespace.BuildGlobalNamespace#createObjLitStringKeyAnalysis
    fn create_obj_lit_string_key_analysis(
        &self,
        ast: &Ast,
        string_key_node: NodeId,
    ) -> ObjLitStringKeyAnalysis {
        let name_string = NodeUtil::get_best_l_value_name(ast, Some(string_key_node));
        if let Some(name_string) = name_string {
            // `parent.qname = { myPropName: myValue }`;
            // `NodeUtil.getBestLValueName()` finds the name being assigned for this case.
            ObjLitStringKeyAnalysis::for_obj_lit_assignment(
                name_string,
                self.get_value_type(ast, string_key_node.get_only_child(ast)),
            )
        } else {
            // maybe we have a case like
            // `Object.defineProperties(parentName, { myPropName: { get: ..., set: ..., ... })`
            let obj_lit_node = string_key_node.get_parent(ast).unwrap();
            check_argument!(
                obj_lit_node.is_object_lit(ast),
                "%s",
                obj_lit_node.to_string(ast)
            );
            let obj_lit_parent_node = obj_lit_node.get_parent(ast).unwrap();
            if NodeUtil::is_object_define_properties_definition(ast, obj_lit_parent_node) {
                let receiver_node = obj_lit_parent_node.get_second_child(ast).unwrap();
                if receiver_node.is_qualified_name(ast) {
                    check_state!(
                        Some(obj_lit_node) == receiver_node.get_next(ast),
                        "%s",
                        obj_lit_parent_node.to_string(ast)
                    );
                    let name_string = receiver_node
                        .get_qualified_name(ast)
                        .unwrap()
                        .concat(&JsString::from("."))
                        .concat(&string_key_node.get_string(ast));
                    return ObjLitStringKeyAnalysis::for_object_define_property(name_string);
                }
            }
            ObjLitStringKeyAnalysis::for_non_reference()
        }
    }

    /// Declares all subnamespaces from `goog.provide('some.long.namespace')` globally.
    // port: GlobalNamespace.BuildGlobalNamespace#createNamesFromProvide
    fn create_names_from_provide(&mut self, namespace: &JsString) {
        let dot_string = JsString::from(".");
        let mut dot: i32 = 0;

        while dot >= 0 {
            dot = namespace.index_of_from(&dot_string, dot + 1);
            let sub_namespace = if dot < 0 {
                namespace.clone()
            } else {
                namespace.substring(0, dot as usize)
            };
            check_state!(!sub_namespace.is_empty());
            let name = self.get_or_create_name(&sub_namespace, None);
            name.set_boolean_property(self.gn, NameProp::IS_PROVIDED);
        }

        let new_name = self.get_or_create_name(namespace, None);
        new_name.set_boolean_property(self.gn, NameProp::IS_PROVIDED);
    }

    /// Whether the given name root represents a global or module-level name
    ///
    /// This method will return false for functions and blocks and true for module bodies and the
    /// `globalRoot`. The one exception is if the function or block is from a goog.loadModule
    /// argument, as those functions/blocks are treated as module roots.
    // port: GlobalNamespace.BuildGlobalNamespace#isTopLevelScopeRoot
    #[allow(clippy::if_same_then_else)] // Java control flow.
    fn is_top_level_scope_root(&self, ast: &Ast, root: Option<NodeId>) -> bool {
        let Some(root) = root else {
            return false;
        };
        if root == self.gn.global_root {
            return true;
        } else if Some(root) == self.cur_module_root {
            return true;
        }
        // Given
        //   goog.loadModule(function(exports) {
        // pretend that assignments to `exports` or `exports.x = ...` are scoped to the function
        // body, although `exports` is really in the enclosing function parameter scope.
        self.cur_module_root
            .is_some_and(|m| m.is_block(ast) && Some(root) == m.get_parent(ast))
    }

    /// Gets the fully qualified name corresponding to an object pattern key, as long as it is not
    /// in a nested pattern and is destructuring an qualified name.
    // port: GlobalNamespace.BuildGlobalNamespace#getNameForObjectPatternKey
    fn get_name_for_object_pattern_key(&self, ast: &Ast, string_key: NodeId) -> Option<JsString> {
        let parent = string_key.get_parent(ast).unwrap();
        check_state!(parent.is_object_pattern(ast));

        let pattern_parent = parent.get_parent(ast).unwrap();
        if pattern_parent.is_assign(ast) || pattern_parent.is_destructuring_lhs(ast) {
            // this is a top-level string key. we find the name.
            let rhs = pattern_parent.get_second_child(ast);
            match rhs {
                Some(rhs) if rhs.is_qualified_name(ast) => Some(
                    rhs.get_qualified_name(ast)
                        .unwrap()
                        .concat(&JsString::from("."))
                        .concat(&string_key.get_string(ast)),
                ),
                // The rhs is null for patterns in parameter lists, enhanced for loops, and catch
                // exprs
                _ => None,
            }
        } else {
            // skip this step for nested patterns for now
            None
        }
    }

    /// Gets the type of a value or simple expression.
    // port: GlobalNamespace.BuildGlobalNamespace#getValueType
    fn get_value_type(&self, ast: &Ast, n: NodeId) -> NameProp {
        match n.get_token(ast) {
            Token::CLASS => {
                return NameProp::CLASS;
            }
            Token::OBJECTLIT => {
                return NameProp::OBJECTLIT;
            }
            Token::FUNCTION => {
                return NameProp::FUNCTION;
            }
            Token::OR => {
                // Recurse on the second value. If the first value were an object
                // literal or function, then the OR would be meaningless and the
                // second value would be dead code. Assume that if the second value
                // is an object literal or function, then the first value will also
                // evaluate to one when it doesn't evaluate to false.
                return self.get_value_type(ast, n.get_last_child(ast).unwrap());
            }
            Token::HOOK => {
                // The same line of reasoning used for the OR case applies here.
                let second = n.get_second_child(ast).unwrap();
                let t = self.get_value_type(ast, second);
                if t != NameProp::OTHER_OBJECT {
                    return t;
                }
                let third = second.get_next(ast).unwrap();
                return self.get_value_type(ast, third);
            }
            _ => {}
        }
        NameProp::OTHER_OBJECT
    }

    /// Updates our representation of the global namespace to reflect an assignment to a global
    /// name in any scope where variables are hoisted to the global scope (i.e. the global scope in
    /// an ES5 sense).
    // port: GlobalNamespace.BuildGlobalNamespace#handleSetFromGlobal
    #[allow(clippy::too_many_arguments)] // Java signature.
    fn handle_set_from_global(
        &mut self,
        compiler: &mut AbstractCompiler,
        chunk: Option<JSChunk>,
        scope: Option<ScopeId>,
        n: NodeId,
        name: &JsString,
        r#type: NameProp,
        metadata: Option<&Arc<ModuleMetadata>>,
    ) {
        if self.maybe_handle_prototype_prefix(compiler, chunk.clone(), scope, n, name, metadata) {
            return;
        }

        let name_obj = self.get_or_create_name(name, metadata);
        if !name_obj.is_get_or_set_definition(self.gn) {
            // Don't change the type of a getter or setter. This is because given:
            //   var a = {set b(item) {}}; a.b = class {};
            // `a.b = class {};` does not change the runtime value of a.b, and we do not want to
            // change the 'type' of a.b to Type.CLASS.
            // TODO(lharker): for non-setter cases, do we really want to just treat the last set
            // of a name as canonical? e.g. what if a name is first set to a class, then an object
            // literal?
            name_obj.set_name_type(self.gn, r#type);
        }
        if n.get_boolean_prop(compiler, Prop::MODULE_EXPORT) {
            name_obj.set_boolean_property(self.gn, NameProp::IS_MODULE_PROP);
        }

        if self.is_nested_assign(compiler, n.get_parent(compiler).unwrap()) {
            // This assignment is both a set and a get that creates an alias.
            let ref_type = RefType::GET_AND_SET_FROM_GLOBAL;
            self.add_or_confirm_ref(compiler, name_obj, n, ref_type, scope, chunk);
        } else {
            self.add_or_confirm_ref(
                compiler,
                name_obj,
                n,
                RefType::SET_FROM_GLOBAL,
                scope,
                chunk,
            );
            let declared_type = self.get_declared_type_kind(compiler, n);
            name_obj.set_declared_type_kind(self.gn, declared_type);
        }
    }

    /// Determines whether a set operation is a constructor or enumeration or interface
    /// declaration. The set operation may either be an assignment to a name, a variable
    /// declaration, or an object literal key mapping.
    // port: GlobalNamespace.BuildGlobalNamespace#getDeclaredTypeKind
    fn get_declared_type_kind(&self, ast: &Ast, n: NodeId) -> NameProp {
        let value_node = NodeUtil::get_r_value_of_l_value(ast, n);
        let kind;
        match value_node {
            None => {
                kind = NameProp::NOT_A_TYPE;
            }
            Some(value_node) if value_node.is_class(ast) => {
                // Always treat classes as having a declared type. (Transpiled classes are
                // annotated @constructor)
                kind = NameProp::CONSTRUCTOR_TYPE;
            }
            Some(value_node) => {
                let info = NodeUtil::get_best_jsdoc_info(ast, n);
                // Heed the annotations only if they're sensibly used.
                match info {
                    None => {
                        kind = NameProp::NOT_A_TYPE;
                    }
                    Some(info) => {
                        if info.is_constructor() && value_node.is_function(ast) {
                            kind = NameProp::CONSTRUCTOR_TYPE;
                        } else if info.is_interface() && value_node.is_function(ast) {
                            kind = NameProp::INTERFACE_TYPE;
                        } else if info.has_enum_parameter_type() && value_node.is_object_lit(ast) {
                            kind = NameProp::ENUM_TYPE;
                        } else {
                            kind = NameProp::NOT_A_TYPE;
                        }
                    }
                }
            }
        }
        kind
    }

    /// Updates our representation of the global namespace to reflect an assignment to a global
    /// name in a local scope.
    // port: GlobalNamespace.BuildGlobalNamespace#handleSetFromLocal
    fn handle_set_from_local(
        &mut self,
        compiler: &mut AbstractCompiler,
        chunk: Option<JSChunk>,
        scope: Option<ScopeId>,
        n: NodeId,
        name: &JsString,
        metadata: Option<&Arc<ModuleMetadata>>,
    ) {
        if self.maybe_handle_prototype_prefix(compiler, chunk.clone(), scope, n, name, metadata) {
            return;
        }

        let name_obj = self.get_or_create_name(name, metadata);
        if n.get_boolean_prop(compiler, Prop::MODULE_EXPORT) {
            name_obj.set_boolean_property(self.gn, NameProp::IS_MODULE_PROP);
        }

        if self.is_nested_assign(compiler, n.get_parent(compiler).unwrap()) {
            // This assignment is both a set and a get that creates an alias.
            self.add_or_confirm_ref(
                compiler,
                name_obj,
                n,
                RefType::GET_AND_SET_FROM_LOCAL,
                scope,
                chunk,
            );
        } else {
            self.add_or_confirm_ref(compiler, name_obj, n, RefType::SET_FROM_LOCAL, scope, chunk);
        }
    }

    /// Updates our representation of the global namespace to reflect a read of a global name.
    // port: GlobalNamespace.BuildGlobalNamespace#handleGet
    fn handle_get(
        &mut self,
        compiler: &mut AbstractCompiler,
        chunk: Option<JSChunk>,
        scope: Option<ScopeId>,
        n: NodeId,
        name: &JsString,
        metadata: Option<&Arc<ModuleMetadata>>,
    ) {
        if self.maybe_handle_prototype_prefix(compiler, chunk.clone(), scope, n, name, metadata) {
            return;
        }
        let r#type = self.determine_ref_type_for_get(compiler, n, n, name);

        let name_obj = self.get_or_create_name(name, metadata);
        self.add_or_confirm_ref(compiler, name_obj, n, r#type, scope, chunk);
    }

    /// Determine the Ref.Type for referenceNode by inspecting parents and recusively ascending as
    /// necessary, where n represents.
    // port: GlobalNamespace.BuildGlobalNamespace#determineRefTypeForGet
    fn determine_ref_type_for_get(
        &self,
        compiler: &AbstractCompiler,
        n: NodeId,
        reference_node: NodeId,
        name: &JsString,
    ) -> RefType {
        let r#type;
        let parent = n.get_parent(compiler).unwrap();
        match parent.get_token(compiler) {
            Token::EXPR_RESULT
            | Token::IF
            | Token::WHILE
            | Token::FOR
            | Token::INSTANCEOF
            | Token::TYPEOF
            | Token::VOID
            | Token::NOT
            | Token::BITNOT
            | Token::POS
            | Token::NEG
            | Token::SHEQ
            | Token::EQ
            | Token::SHNE
            | Token::NE
            | Token::LT
            | Token::LE
            | Token::GT
            | Token::GE
            | Token::ADD
            | Token::SUB
            | Token::MUL
            | Token::DIV
            | Token::MOD
            | Token::EXPONENT
            | Token::BITAND
            | Token::BITOR
            | Token::BITXOR
            | Token::LSH
            | Token::RSH
            | Token::URSH => {
                r#type = RefType::DIRECT_GET;
            }
            Token::OPTCHAIN_CALL | Token::CALL => {
                if Some(n) == parent.get_first_child(compiler) {
                    // It is a call target
                    r#type = RefType::CALL_GET;
                } else if self.is_class_defining_call(compiler, parent) {
                    r#type = RefType::DIRECT_GET;
                } else {
                    r#type = RefType::ALIASING_GET;
                }
            }
            Token::NEW => {
                r#type = if Some(n) == parent.get_first_child(compiler) {
                    RefType::DIRECT_GET
                } else {
                    RefType::ALIASING_GET
                };
            }
            Token::CAST | Token::OR | Token::AND | Token::COALESCE => {
                // This node is x or y in (x||y), (x&&y), or (x??y). We only know that an
                // alias is not getting created for this name if the result is used
                // in a boolean context or assigned to the same name
                // (e.g. var a = a || {}).
                r#type = self.determine_ref_type_for_get(compiler, parent, reference_node, name);
            }
            Token::NAME => {
                // Only LET, CONST, VAR declarations have NAME nodes
                // with children.
                // Of particular interest is "var n = n || {}"
                if n != reference_node && *name == parent.get_string(compiler) {
                    r#type = RefType::DIRECT_GET;
                } else {
                    r#type = RefType::ALIASING_GET;
                }
            }
            Token::COMMA | Token::HOOK => {
                if Some(n) != parent.get_first_child(compiler) {
                    // This node is y or z in (x?y:z) or (x,y). We only know that an alias is
                    // not getting created for this name if the result is assigned to
                    // the same name (e.g. var a = a ? a : {}).
                    r#type =
                        self.determine_ref_type_for_get(compiler, parent, reference_node, name);
                } else {
                    r#type = RefType::DIRECT_GET;
                }
            }
            Token::DELPROP => {
                r#type = RefType::DELETE_PROP;
            }
            Token::CLASS => {
                // This node is the superclass in an extends clause.
                r#type = RefType::SUBCLASSING_GET;
            }
            Token::DESTRUCTURING_LHS | Token::ASSIGN => {
                let lhs = n.get_previous(compiler);
                let Some(mut lhs) = lhs else {
                    // TODO(b/127505242): CAST confused the "is this a get or set?"
                    // logic and "handleGet" should not have been called.
                    return RefType::ALIASING_GET;
                };
                while lhs.is_cast(compiler) {
                    // Case: `/** @type {!Foo} */ (x) = ...`; or multiple casts like
                    // `(cast(cast(x)) =`
                    lhs = lhs.get_only_child(compiler);
                }

                // This is a recursive ascent check if this an assignment
                // to itself.  This handles cases like: "a.b = a.b || {}"
                if n != reference_node && lhs.matches_qualified_name(compiler, name.clone()) {
                    return RefType::DIRECT_GET;
                }

                match lhs.get_token(compiler) {
                    // The rhs of an assign or a name declaration is escaped if it's assigned to a
                    // name directly ...
                    Token::NAME | Token::GETPROP | Token::GETELEM
                    // ... or referenced through numeric/object keys.
                    | Token::ARRAY_PATTERN
                    | Token::OBJECT_PATTERN => {
                        r#type = RefType::ALIASING_GET;
                    }
                    _ => {
                        panic!(
                            "Unexpected previous sibling of {}: {}",
                            n.get_token(compiler),
                            n.get_previous(compiler).unwrap().to_string(compiler)
                        );
                    }
                }
            }
            // OBJECT_PATTERN (STRING_KEYS in object patterns), ITER_SPREAD, OBJECT_SPREAD,
            // RETURN, THROW and the rest:
            _ => {
                // NOTE: There are likely more cases where we should be returning
                // DIRECT_GET.
                r#type = RefType::ALIASING_GET;
            }
        }
        r#type
    }

    /// If there is already a Ref for the given name & node, confirm it matches what we would
    /// create. Otherwise add a new one.
    // port: GlobalNamespace.BuildGlobalNamespace#addOrConfirmRef
    fn add_or_confirm_ref(
        &mut self,
        compiler: &AbstractCompiler,
        name_obj: Name,
        node: NodeId,
        ref_type: RefType,
        scope: Option<ScopeId>,
        chunk: Option<JSChunk>,
    ) {
        let existing_ref = name_obj.get_ref_for_node(self.gn, node);
        match existing_ref {
            None => {
                name_obj.add_ref(self.gn, compiler, chunk, scope, node, ref_type);
            }
            Some(existing_ref) => {
                // module and scope are dependent on Node, so not much point in checking them
                let existing_ref_type = existing_ref.r#type(self.gn);
                check_state!(
                    existing_ref_type == ref_type,
                    "existing ref type: %s expected: %s",
                    existing_ref_type,
                    ref_type
                );
            }
        }
    }

    // port: GlobalNamespace.BuildGlobalNamespace#isClassDefiningCall
    fn is_class_defining_call(&self, compiler: &AbstractCompiler, call_node: NodeId) -> bool {
        let convention = compiler.get_coding_convention();
        // Look for goog.inherits and J2CL mixin calls
        let classes = convention.get_classes_defined_by_call(compiler, call_node);
        if classes.is_some() {
            return true;
        }

        // Look for calls to goog.addSingletonGetter calls.
        let class_name = convention.get_singleton_getter_class_name(compiler, call_node);
        class_name.is_some()
    }

    /// Detect calls of the form a.b.hasOwnProperty(c); that prevent property collapsing on a.b
    // port: GlobalNamespace.BuildGlobalNamespace#isObjectHasOwnPropertyCall
    fn is_object_has_own_property_call(&self, ast: &Ast, call_node: NodeId) -> bool {
        check_argument!(
            call_node.is_call(ast) || call_node.is_opt_chain_call(ast),
            "%s",
            call_node.to_string(ast)
        );
        if !call_node.has_two_children(ast) {
            return false;
        }
        let callee = call_node.get_first_child(ast).unwrap();
        if !callee.is_get_prop(ast) && !callee.is_opt_chain_get_prop(ast) {
            return false;
        }
        let receiver = callee.get_first_child(ast).unwrap();
        let prop_name = callee.get_string(ast);
        (prop_name == "hasOwnProperty" || prop_name == "propertyIsEnumerable")
            && receiver.is_qualified_name(ast)
    }

    /// Updates our representation of the global namespace to reflect a read of a global name's
    /// longest prefix before the "prototype" property if the name includes the "prototype"
    /// property. Does nothing otherwise.
    // port: GlobalNamespace.BuildGlobalNamespace#maybeHandlePrototypePrefix
    fn maybe_handle_prototype_prefix(
        &mut self,
        compiler: &AbstractCompiler,
        chunk: Option<JSChunk>,
        scope: Option<ScopeId>,
        n: NodeId,
        name: &JsString,
        metadata: Option<&Arc<ModuleMetadata>>,
    ) -> bool {
        // We use a string-based approach instead of inspecting the parse tree
        // to avoid complexities with object literals, possibly nested, beneath
        // assignments.

        let mut num_levels_to_remove: i32;
        let prefix: JsString;
        if name.ends_with(".prototype") {
            num_levels_to_remove = 1;
            prefix = name.substring(0, name.length() - 10);
        } else {
            let mut i = name.index_of(".prototype.");
            if i == -1 {
                return false;
            }
            prefix = name.substring(0, i as usize);
            num_levels_to_remove = 2;
            let dot = JsString::from(".");
            i = name.index_of_from(&dot, i + 11);
            while i >= 0 {
                num_levels_to_remove += 1;
                i = name.index_of_from(&dot, i + 1);
            }
        }

        if NodeUtil::may_be_object_lit_key(compiler, n) {
            // Object literal keys have no prefix that's referenced directly per
            // key, so we're done.
            return true;
        }

        let mut n = n;
        for _ in 0..num_levels_to_remove {
            n = n.get_first_child(compiler).unwrap();
        }

        let name_obj = self.get_or_create_name(&prefix, metadata);
        self.add_or_confirm_ref(compiler, name_obj, n, RefType::PROTOTYPE_GET, scope, chunk);
        true
    }

    /// Determines whether an assignment is nested (i.e. whether its return value is used).
    // port: GlobalNamespace.BuildGlobalNamespace#isNestedAssign
    fn is_nested_assign(&self, ast: &Ast, parent: NodeId) -> bool {
        parent.is_assign(ast) && !parent.get_parent(ast).unwrap().is_expr_result(ast)
    }

    /// Gets a Name instance for a global name. Creates it if necessary, as well as instances for
    /// any of its prefixes that are not yet defined.
    // port: GlobalNamespace.BuildGlobalNamespace#getOrCreateName
    fn get_or_create_name(
        &mut self,
        name: &JsString,
        metadata: Option<&Arc<ModuleMetadata>>,
    ) -> Name {
        let mut node = match metadata {
            None => self.gn.name_map.get(name).copied(),
            Some(metadata) => self
                .gn
                .name_map_by_module
                .get(&(ModuleMetadataKey(metadata.clone()), name.clone()))
                .copied(),
        };
        if node.is_none() {
            let i = name.last_index_of_char(u16::from(b'.'));
            if i >= 0 {
                let parent_name = name.substring(0, i as usize);
                let parent = self.get_or_create_name(&parent_name, metadata);
                let source_kind = self.gn.source_kind;
                let new_node =
                    parent.add_property(self.gn, name.substring_from(i as usize + 1), source_kind);
                match metadata {
                    None => {
                        self.gn.name_map.insert(name.clone(), new_node);
                    }
                    Some(metadata) => {
                        self.gn.name_map_by_module.insert(
                            (ModuleMetadataKey(metadata.clone()), name.clone()),
                            new_node,
                        );
                    }
                }
                node = Some(new_node);
            } else {
                let source_kind = self.gn.source_kind;
                let new_node = self.gn.new_name(name.clone(), None, source_kind);
                match metadata {
                    None => {
                        self.gn.global_names.push(new_node);
                        self.gn.name_map.insert(name.clone(), new_node);
                    }
                    Some(metadata) => {
                        self.gn.name_map_by_module.insert(
                            (ModuleMetadataKey(metadata.clone()), name.clone()),
                            new_node,
                        );
                    }
                }
                node = Some(new_node);
            }
        }
        node.unwrap()
    }
}

// -------------------------------------------------------------------------

/// How much to inline a Name.
///
/// The `Inlinability#INLINE_BUT_KEEP_DECLARATION_*` cass are really an indicator that something
/// 'unsafe' is happening in order to not break CollapseProperties as badly. Sadly
/// `Inlinability#INLINE_COMPLETELY` may *also* be unsafe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Inlinability {
    INLINE_COMPLETELY,
    INLINE_BUT_KEEP_DECLARATION,
    INLINE_UNLESS_INVALID_CROSS_CHUNK_DEPENDENCY,
    DO_NOT_INLINE,
}

impl Inlinability {
    // port: GlobalNamespace.Inlinability#Inlinability
    const fn fields(self) -> (bool, bool, bool) {
        // (shouldInlineUsages, shouldRemoveDeclaration, canCollapse)
        match self {
            Inlinability::INLINE_COMPLETELY => (
                /* shouldInlineUsages= */ true, /* shouldRemoveDeclaration= */ true,
                /* canCollapse= */ true,
            ),
            Inlinability::INLINE_BUT_KEEP_DECLARATION => (
                /* shouldInlineUsages= */ true, /* shouldRemoveDeclaration= */ false,
                /* canCollapse= */ true,
            ),
            Inlinability::INLINE_UNLESS_INVALID_CROSS_CHUNK_DEPENDENCY => (
                /* shouldInlineUsages= */ true, /* shouldRemoveDeclaration= */ false,
                /* canCollapse= */ true,
            ),
            Inlinability::DO_NOT_INLINE => (
                /* shouldInlineUsages= */ false, /* shouldRemoveDeclaration= */ false,
                /* canCollapse= */ false,
            ),
        }
    }

    // port: GlobalNamespace.Inlinability#shouldInlineUsages
    pub fn should_inline_usages(self) -> bool {
        self.fields().0
    }

    // port: GlobalNamespace.Inlinability#shouldRemoveDeclaration
    pub fn should_remove_declaration(self) -> bool {
        self.fields().1
    }

    // port: GlobalNamespace.Inlinability#canCollapse
    pub fn can_collapse(self) -> bool {
        self.fields().2
    }
}

impl fmt::Display for Inlinability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A name defined in global scope (e.g. "a" or "a.b.c.d").
///
/// Instances form a tree describing the "Closure namespaces" in the program. As the parse tree
/// traversal proceeds, we'll discover that some names correspond to JavaScript objects whose
/// properties we should consider collapsing.
impl Name {
    /// Java's package-private `props` field: the children of this name, null if there are none.
    pub fn props(self, gn: &GlobalNamespace) -> Option<&[Name]> {
        gn.name_data(self).props.as_deref()
    }

    // port: GlobalNamespace.Name#addProperty
    pub fn add_property(
        self,
        gn: &mut GlobalNamespace,
        name: JsString,
        source_kind: Option<SourceKind>,
    ) -> Name {
        if gn.name_data(self).props.is_none() {
            gn.name_data_mut(self).props = Some(Vec::new());
        }
        let node = gn.new_name(name, Some(self), source_kind);
        gn.name_data_mut(self).props.as_mut().unwrap().push(node);
        node
    }

    // port: GlobalNamespace.Name#getBaseName
    pub fn get_base_name(self, gn: &GlobalNamespace) -> JsString {
        gn.name_data(self).base_name.clone()
    }

    // port: GlobalNamespace.Name#inExterns
    pub fn in_externs(self, gn: &GlobalNamespace) -> bool {
        self.get_boolean_property(gn, NameProp::IS_EXTERN)
    }

    // port: GlobalNamespace.Name#subclassingGetCount
    pub fn subclassing_get_count(self, gn: &GlobalNamespace) -> i32 {
        gn.name_data(self).subclassing_gets
    }

    // port: GlobalNamespace.Name#getName
    pub fn get_name(self, gn: &GlobalNamespace) -> JsString {
        self.get_full_name(gn)
    }

    // port: GlobalNamespace.Name#getFullName
    pub fn get_full_name(self, gn: &GlobalNamespace) -> JsString {
        let data = gn.name_data(self);
        match data.parent {
            None => data.base_name.clone(),
            Some(parent) => parent
                .get_full_name(gn)
                .concat(&JsString::from("."))
                .concat(&data.base_name),
        }
    }

    // port: GlobalNamespace.Name#usesHasOwnProperty
    pub fn uses_has_own_property(self, gn: &GlobalNamespace) -> bool {
        self.get_boolean_property(gn, NameProp::IS_USED_HAS_OWN_PROPERTY)
    }

    // port: GlobalNamespace.Name#getDeclaration
    pub fn get_declaration(self, gn: &GlobalNamespace) -> Option<Ref> {
        gn.name_data(self).declaration
    }

    // port: GlobalNamespace.Name#getInitialization
    pub fn get_initialization(self, gn: &GlobalNamespace) -> Option<Ref> {
        gn.name_data(self).initialization
    }

    // port: GlobalNamespace.Name#isFunction
    pub fn is_function(self, gn: &GlobalNamespace) -> bool {
        self.get_boolean_property(gn, NameProp::FUNCTION)
    }

    // port: GlobalNamespace.Name#isClass
    pub fn is_class(self, gn: &GlobalNamespace) -> bool {
        self.get_boolean_property(gn, NameProp::CLASS)
    }

    // port: GlobalNamespace.Name#isObjectLiteral
    pub fn is_object_literal(self, gn: &GlobalNamespace) -> bool {
        self.get_boolean_property(gn, NameProp::OBJECTLIT)
    }

    // port: GlobalNamespace.Name#getAliasingGets
    pub fn get_aliasing_gets(self, gn: &GlobalNamespace) -> i32 {
        gn.name_data(self).aliasing_gets
    }

    // port: GlobalNamespace.Name#getSubclassingGets
    pub fn get_subclassing_gets(self, gn: &GlobalNamespace) -> i32 {
        gn.name_data(self).subclassing_gets
    }

    // port: GlobalNamespace.Name#getLocalSets
    pub fn get_local_sets(self, gn: &GlobalNamespace) -> i32 {
        gn.name_data(self).local_sets
    }

    // port: GlobalNamespace.Name#getGlobalSets
    pub fn get_global_sets(self, gn: &GlobalNamespace) -> i32 {
        gn.name_data(self).global_sets
    }

    // port: GlobalNamespace.Name#getTotalSets
    pub fn get_total_sets(self, gn: &GlobalNamespace) -> i32 {
        let data = gn.name_data(self);
        data.global_sets + data.local_sets
    }

    // port: GlobalNamespace.Name#getCallGets
    pub fn get_call_gets(self, gn: &GlobalNamespace) -> i32 {
        gn.name_data(self).call_gets
    }

    // port: GlobalNamespace.Name#getTotalGets
    pub fn get_total_gets(self, gn: &GlobalNamespace) -> i32 {
        gn.name_data(self).total_gets
    }

    // port: GlobalNamespace.Name#getDeleteProps
    pub fn get_delete_props(self, gn: &GlobalNamespace) -> i32 {
        gn.name_data(self).delete_props
    }

    // port: GlobalNamespace.Name#getParent
    pub fn get_parent(self, gn: &GlobalNamespace) -> Option<Name> {
        gn.name_data(self).parent
    }

    /// Returns the JSChunk that is an ancestor (or equal to) all the chunks in which this name is
    /// referenced, inclusive of the actual chunks in the refs.
    // port: GlobalNamespace.Name#getDeepestCommonAncestorChunk
    pub fn get_deepest_common_ancestor_chunk(
        self,
        gn: &GlobalNamespace,
        chunk_graph: &JSChunkGraph,
    ) -> Option<JSChunk> {
        let mut common_ancestor = check_not_null!(self.get_declaration(gn)).get_chunk(gn);
        for r in self.get_refs(gn) {
            let r_chunk = r.get_chunk(gn);
            common_ancestor = chunk_graph.get_deepest_common_dependency_inclusive(
                check_not_null!(r_chunk.as_ref()),
                check_not_null!(common_ancestor.as_ref()),
            );
        }
        common_ancestor
    }

    // port: GlobalNamespace.Name#getScope
    pub fn get_scope(self) -> &'static GlobalNamespace {
        panic!("java.lang.UnsupportedOperationException")
    }

    // port: GlobalNamespace.Name#setBooleanProperty
    fn set_boolean_property(self, gn: &mut GlobalNamespace, property: NameProp) {
        let data = gn.name_data_mut(self);
        data.property_bit_set |= property.bit();
    }

    // port: GlobalNamespace.Name#getBooleanProperty
    fn get_boolean_property(self, gn: &GlobalNamespace, property: NameProp) -> bool {
        (gn.name_data(self).property_bit_set & property.bit()) != 0
    }

    // port: GlobalNamespace.Name#addRef
    fn add_ref(
        self,
        gn: &mut GlobalNamespace,
        compiler: &AbstractCompiler,
        chunk: Option<JSChunk>,
        scope: Option<ScopeId>,
        node: NodeId,
        r#type: RefType,
    ) {
        self.check_no_existing_refs_for_node(gn, compiler, node);
        let r = self.create_new_ref(gn, chunk, scope, node, r#type);
        self.put_ref(gn, node, r);
        self.update_state_for_added_ref(gn, compiler, r);
    }

    // port: GlobalNamespace.Name#checkNoExistingRefsForNode
    fn check_no_existing_refs_for_node(
        self,
        gn: &GlobalNamespace,
        compiler: &AbstractCompiler,
        node: NodeId,
    ) {
        match &gn.name_data(self).refs_for_node {
            RefsForNode::Null => {}
            RefsForNode::Single(r) => {
                let r = *r;
                check_state!(
                    r.get_node(gn) != Some(node),
                    "Ref already exists for node: %s",
                    r.to_string(gn, compiler)
                );
            }
            RefsForNode::Map(_) => {
                let ref_for_node = self.cast_refs_for_node_map(gn).get(&Some(node)).copied();
                check_state!(
                    ref_for_node.is_none(),
                    "Ref already exists for node: %s",
                    ref_for_node.unwrap().to_string(gn, compiler)
                );
            }
        }
    }

    // port: GlobalNamespace.Name#castRefsForNodeMap
    fn cast_refs_for_node_map(self, gn: &GlobalNamespace) -> &IndexMap<Option<NodeId>, Ref> {
        match &gn.name_data(self).refs_for_node {
            RefsForNode::Map(map) => map,
            _ => panic!("java.lang.ClassCastException"),
        }
    }

    // Rust-only mutable counterpart of castRefsForNodeMap.
    fn cast_refs_for_node_map_mut(
        self,
        gn: &mut GlobalNamespace,
    ) -> &mut IndexMap<Option<NodeId>, Ref> {
        match &mut gn.name_data_mut(self).refs_for_node {
            RefsForNode::Map(map) => map,
            _ => panic!("java.lang.ClassCastException"),
        }
    }

    // port: GlobalNamespace.Name#createNewRef
    fn create_new_ref(
        self,
        gn: &mut GlobalNamespace,
        chunk: Option<JSChunk>,
        scope: Option<ScopeId>,
        node: NodeId,
        r#type: RefType,
    ) -> Ref {
        gn.new_ref(
            chunk,
            Some(check_not_null!(scope)),
            Some(node), // may be null later, but not on creation
            r#type,
        )
    }

    // port: GlobalNamespace.Name#putRef
    fn put_ref(self, gn: &mut GlobalNamespace, node: NodeId, r: Ref) {
        let data = gn.name_data(self);
        if let RefsForNode::Null = data.refs_for_node {
            gn.name_data_mut(self).refs_for_node = RefsForNode::Single(r);
            return;
        }
        if let RefsForNode::Single(existing_ref) = data.refs_for_node {
            // Convert the singleton Ref object into a map, so that we can store a second Ref.
            let mut refs_for_node_map = IndexMap::<_, _>::default();
            refs_for_node_map.insert(existing_ref.get_node(gn), existing_ref);
            gn.name_data_mut(self).refs_for_node = RefsForNode::Map(refs_for_node_map);
        }
        self.cast_refs_for_node_map_mut(gn).insert(Some(node), r);
    }

    // port: GlobalNamespace.Name#addSingleRefForTesting
    pub fn add_single_ref_for_testing(
        self,
        gn: &mut GlobalNamespace,
        compiler: &AbstractCompiler,
        node: NodeId,
        r#type: RefType,
    ) -> Ref {
        let r = gn.new_ref(
            /* chunk= */ None,
            /* scope= */ None,
            /* node= */ Some(node),
            r#type,
        );
        self.put_ref(gn, node, r);
        self.update_state_for_added_ref(gn, compiler, r);
        r
    }

    /// Add an ALIASING_GET Ref for the given Node using the same Ref properties as the
    /// declaration Ref, which must exist.
    ///
    /// Only for use by CollapseProperties.
    // port: GlobalNamespace.Name#addAliasingGetClonedFromDeclaration
    pub fn add_aliasing_get_cloned_from_declaration(
        self,
        gn: &mut GlobalNamespace,
        compiler: &AbstractCompiler,
        new_ref_node: NodeId,
    ) {
        // TODO(bradfordcsmith): It would be good to add checks that the scope is correct.
        let decl_ref = check_not_null!(gn.name_data(self).declaration);
        let chunk = gn.ref_data(decl_ref).chunk.clone();
        let scope = gn.ref_data(decl_ref).scope;
        self.add_ref(
            gn,
            compiler,
            chunk,
            scope,
            new_ref_node,
            RefType::ALIASING_GET,
        );
    }

    /// Updates counters and JSDocInfo recorded for the name to include a newly added Ref.
    ///
    /// Must be called exactly once when a new Ref is added.
    // port: GlobalNamespace.Name#updateStateForAddedRef
    fn update_state_for_added_ref(self, gn: &mut GlobalNamespace, ast: &Ast, r: Ref) {
        let ref_type = r.r#type(gn);
        match ref_type {
            RefType::GET_AND_SET_FROM_GLOBAL | RefType::SET_FROM_GLOBAL => {
                if gn.name_data(self).declaration.is_none() {
                    gn.name_data_mut(self).declaration = Some(r);
                }
                if gn.name_data(self).initialization.is_none()
                    && !r.is_uninitialized_declaration(gn, ast)
                {
                    // Record the reference where the first value is actually assigned.
                    // Do not include the automatically-assigned `undefined` value case.
                    // e.g. `var name;`
                    gn.name_data_mut(self).initialization = Some(r);
                }
                if gn.name_data(self).first_declaration_jsdoc_info.is_none() {
                    // JSDocInfo from the first SET_FROM_GLOBAL will be assumed to be canonical
                    // Note that this will not change if the first declaration is later removed
                    // by optimizations.
                    let info = self.get_doc_info_for_declaration(gn, ast, r);
                    gn.name_data_mut(self).first_declaration_jsdoc_info = info;
                }
                let data = gn.name_data_mut(self);
                data.global_sets += 1;
                if ref_type == RefType::GET_AND_SET_FROM_GLOBAL {
                    data.aliasing_gets += 1;
                    data.total_gets += 1;
                }
            }
            RefType::GET_AND_SET_FROM_LOCAL | RefType::SET_FROM_LOCAL => {
                gn.name_data_mut(self).local_sets += 1;
                let info = r
                    .get_node(gn)
                    .and_then(|node| NodeUtil::get_best_jsdoc_info(ast, node));
                let data = gn.name_data_mut(self);
                if info.is_some_and(|info| info.is_no_collapse()) {
                    data.local_sets_with_no_collapse += 1;
                }
                if ref_type == RefType::GET_AND_SET_FROM_LOCAL {
                    data.aliasing_gets += 1;
                    data.total_gets += 1;
                }
            }
            RefType::PROTOTYPE_GET | RefType::DIRECT_GET => {
                let node = r.get_node(gn);
                if gn
                    .name_data(self)
                    .first_qname_declaration_without_assignment_jsdoc_info
                    .is_none()
                    && GlobalNamespace::is_qname_declaration_without_assignment(ast, node)
                {
                    // /** @type {sometype} */
                    // some.qname.ref;
                    gn.name_data_mut(self)
                        .first_qname_declaration_without_assignment_jsdoc_info =
                        node.unwrap().get_jsdoc_info(ast);
                }
                gn.name_data_mut(self).total_gets += 1;
            }
            RefType::ALIASING_GET => {
                let data = gn.name_data_mut(self);
                data.aliasing_gets += 1;
                data.total_gets += 1;
            }
            RefType::CALL_GET => {
                let data = gn.name_data_mut(self);
                data.call_gets += 1;
                data.total_gets += 1;
            }
            RefType::DELETE_PROP => gn.name_data_mut(self).delete_props += 1,
            RefType::SUBCLASSING_GET => {
                let data = gn.name_data_mut(self);
                data.subclassing_gets += 1;
                data.total_gets += 1;
            }
        }
    }

    /// This is the only safe way to update the Node belonging to a Ref once it is added to a
    /// Name.
    ///
    /// This is a specialized method that exists only for use by CollapseProperties.
    // port: GlobalNamespace.Name#updateRefNode
    pub fn update_ref_node(
        self,
        gn: &mut GlobalNamespace,
        compiler: &AbstractCompiler,
        r: Ref,
        new_node: Option<NodeId>,
    ) {
        check_argument!(
            r.get_node(gn) != new_node,
            "redundant update to Ref node: %s",
            r.to_string(gn, compiler)
        );

        // Once a Ref's node is set to null, it shouldn't ever be set to anything else.
        // TODO(bradfordcsmith): Document here what it means when we set the node to null.
        //     Seems to be a way to keep name.getDeclaration() returning the original declaration
        //     Ref even though its node is no longer in the AST.
        let old_node = r.get_node(gn);
        check_state!(
            old_node.is_some(),
            "Ref's node is already null: %s",
            r.to_string(gn, compiler)
        );
        gn.ref_data_mut(r).node = new_node;

        match &gn.name_data(self).refs_for_node {
            RefsForNode::Null => {
                gn.name_data_mut(self).refs_for_node = RefsForNode::Single(r);
            }
            RefsForNode::Single(single) => {
                // No update needed, since refsForNode is a singleton.
                check_state!(*single == r);
            }
            RefsForNode::Map(_) => {
                let refs_for_node_map = self.cast_refs_for_node_map_mut(gn);
                refs_for_node_map.shift_remove(&old_node);
                if let Some(new_node) = new_node {
                    let existing_ref_for_new_node = refs_for_node_map.get(&Some(new_node)).copied();
                    check_argument!(
                        existing_ref_for_new_node.is_none(),
                        "refs already exist: %s",
                        existing_ref_for_new_node.unwrap().to_string(gn, compiler)
                    );
                    self.cast_refs_for_node_map_mut(gn)
                        .insert(Some(new_node), r);
                }
            }
        }
    }

    /// Removes the given Ref, which must belong to this Name.
    ///
    /// NOTE: if this is a twin ref, i.e. both a get and a set of this Name, this removes both the
    /// get and the set.
    // port: GlobalNamespace.Name#removeRef
    pub fn remove_ref(self, gn: &mut GlobalNamespace, compiler: &AbstractCompiler, r: Ref) {
        match &gn.name_data(self).refs_for_node {
            RefsForNode::Null => {
                panic!("removeRef({}): unknown ref", r.to_string(gn, compiler));
            }
            RefsForNode::Single(single) => {
                check_state!(
                    *single == r,
                    "removeRef(%s): unknown ref",
                    r.to_string(gn, compiler)
                );
                gn.name_data_mut(self).refs_for_node = RefsForNode::Null;
            }
            RefsForNode::Map(map) => {
                check_state!(
                    map.contains_key(&r.get_node(gn)),
                    "removeRef(%s): unknown ref",
                    r.to_string(gn, compiler)
                );
                let ref_node = r.get_node(gn);
                if ref_node.is_some() {
                    self.remove_ref_from_node_map(gn, compiler, r);
                }
            }
        }
        self.remove_ref_and_update_state(gn, compiler, r);
    }

    /// Update counts, declaration, and JSDoc to reflect removal of the given Ref.
    // port: GlobalNamespace.Name#removeRefAndUpdateState
    fn remove_ref_and_update_state(self, gn: &mut GlobalNamespace, ast: &Ast, r: Ref) {
        if Some(r) == gn.name_data(self).declaration {
            gn.name_data_mut(self).declaration = None;
            for maybe_new_decl in self.get_refs(gn) {
                if maybe_new_decl.r#type(gn) == RefType::SET_FROM_GLOBAL {
                    gn.name_data_mut(self).declaration = Some(maybe_new_decl);
                    break;
                }
            }
        }

        let info: Option<Arc<JSDocInfo>>;
        let ref_type = r.r#type(gn);
        match ref_type {
            RefType::SET_FROM_GLOBAL => gn.name_data_mut(self).global_sets -= 1,
            RefType::GET_AND_SET_FROM_GLOBAL => {
                let data = gn.name_data_mut(self);
                data.aliasing_gets -= 1;
                data.total_gets -= 1;
                data.global_sets -= 1;
            }
            RefType::SET_FROM_LOCAL | RefType::GET_AND_SET_FROM_LOCAL => {
                gn.name_data_mut(self).local_sets -= 1;
                info = r
                    .get_node(gn)
                    .and_then(|node| NodeUtil::get_best_jsdoc_info(ast, node));
                let data = gn.name_data_mut(self);
                if info.is_some_and(|info| info.is_no_collapse()) {
                    data.local_sets_with_no_collapse -= 1;
                }
                if ref_type == RefType::GET_AND_SET_FROM_LOCAL {
                    data.aliasing_gets -= 1;
                    data.total_gets -= 1;
                }
            }
            RefType::PROTOTYPE_GET | RefType::DIRECT_GET => gn.name_data_mut(self).total_gets -= 1,
            RefType::ALIASING_GET => {
                let data = gn.name_data_mut(self);
                data.aliasing_gets -= 1;
                data.total_gets -= 1;
            }
            RefType::CALL_GET => {
                let data = gn.name_data_mut(self);
                data.call_gets -= 1;
                data.total_gets -= 1;
            }
            RefType::DELETE_PROP => gn.name_data_mut(self).delete_props -= 1,
            RefType::SUBCLASSING_GET => {
                let data = gn.name_data_mut(self);
                data.subclassing_gets -= 1;
                data.total_gets -= 1;
                // Leaving off default: allows compile-time enforcement that all values are
                // covered
            }
        }
    }

    // port: GlobalNamespace.Name#removeRefFromNodeMap
    fn remove_ref_from_node_map(
        self,
        gn: &mut GlobalNamespace,
        compiler: &AbstractCompiler,
        r: Ref,
    ) {
        let ref_node = check_not_null!(r.get_node(gn), "%s", r.to_string(gn, compiler));
        match &gn.name_data(self).refs_for_node {
            RefsForNode::Null => {
                panic!(
                    "Missing ref when trying to remove it: {}",
                    r.to_string(gn, compiler)
                );
            }
            RefsForNode::Single(_) => {
                gn.name_data_mut(self).refs_for_node = RefsForNode::Null;
            }
            RefsForNode::Map(map) => {
                let refs_for_node = map.get(&Some(ref_node)).copied();
                check_state!(
                    refs_for_node == Some(r),
                    "Unexpected Refs for Node: %s: when removing Ref: %s",
                    refs_for_node.map_or_else(|| "null".to_string(), |x| x.to_string(gn, compiler)),
                    r.to_string(gn, compiler)
                );
                self.cast_refs_for_node_map_mut(gn)
                    .shift_remove(&Some(ref_node));
            }
        }
    }

    // port: GlobalNamespace.Name#getRefs
    pub fn get_refs(self, gn: &GlobalNamespace) -> Vec<Ref> {
        match &gn.name_data(self).refs_for_node {
            RefsForNode::Null => Vec::new(),
            RefsForNode::Single(r) => vec![*r],
            RefsForNode::Map(map) => map.values().copied().collect(),
        }
    }

    /// Get the Ref for this name that belongs to the given node.
    ///
    /// Returns null if there are no Refs corresponding to the node.
    // port: GlobalNamespace.Name#getRefForNode
    pub fn get_ref_for_node(self, gn: &GlobalNamespace, node: NodeId) -> Option<Ref> {
        match &gn.name_data(self).refs_for_node {
            RefsForNode::Null => None,
            RefsForNode::Single(r) => {
                if r.get_node(gn) == Some(node) {
                    Some(*r)
                } else {
                    None
                }
            }
            RefsForNode::Map(map) => map.get(&Some(node)).copied(),
        }
    }

    // port: GlobalNamespace.Name#getFirstRef
    pub fn get_first_ref(self, gn: &GlobalNamespace) -> Ref {
        match &gn.name_data(self).refs_for_node {
            RefsForNode::Null => panic!("no first Ref to get"),
            RefsForNode::Single(r) => *r,
            RefsForNode::Map(map) => *map
                .values()
                .next()
                .expect("java.util.NoSuchElementException"),
        }
    }

    // port: GlobalNamespace.Name#canEliminate
    pub fn can_eliminate(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        if !self.can_collapse_unannotated_child_names(gn, ast) || gn.name_data(self).total_gets > 0
        {
            return false;
        }

        if let Some(props) = self.props(gn) {
            for n in props {
                if !n.can_collapse(gn, ast) {
                    return false;
                }
            }
        }
        true
    }

    // port: GlobalNamespace.Name#isSimpleStubDeclaration
    pub fn is_simple_stub_declaration(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        if self.get_refs(gn).len() == 1 {
            let r = self.get_first_ref(gn);
            if r.get_node(gn)
                .unwrap()
                .get_parent(ast)
                .unwrap()
                .is_expr_result(ast)
            {
                return true;
            }
        }
        false
    }

    // port: GlobalNamespace.Name#isCollapsingExplicitlyDenied
    pub fn is_collapsing_explicitly_denied(self, gn: &GlobalNamespace) -> bool {
        let doc_info = self.get_jsdoc_info(gn);
        doc_info.is_some_and(|doc_info| doc_info.is_no_collapse())
    }

    /// Returns whether to treat this alias as completely inlineable or to keep the aliasing
    /// assignment
    // port: GlobalNamespace.Name#calculateInlinability
    pub fn calculate_inlinability(
        self,
        gn: &GlobalNamespace,
        compiler: &AbstractCompiler,
    ) -> Inlinability {
        // Only simple aliases with direct usage are inlineable.
        // Exactly 2 global sets could be OK, if the first is the declaration with no initializer.
        if self.in_externs(gn)
            || !self.has_one_real_global_set(gn, compiler)
            || gn.name_data(self).local_sets != 0
        {
            return Inlinability::DO_NOT_INLINE;
        }

        // TODO(lharker): consider separating canCollapseOrInline() into this method, since it
        // duplicates some logic here
        let mut inlinability = self.can_collapse_or_inline(gn, compiler);
        if !inlinability.should_inline_usages() {
            // if you can't even inline the usages, do nothing.
            return Inlinability::DO_NOT_INLINE;
        }
        let initial_chunk = match self.get_initialization(gn) {
            Some(initialization) => initialization.get_chunk(gn),
            None => check_not_null!(self.get_declaration(gn)).get_chunk(gn),
        };

        // Only allow inlining of simple references.
        for r in self.get_refs(gn) {
            match r.r#type(gn) {
                RefType::SET_FROM_GLOBAL | RefType::GET_AND_SET_FROM_GLOBAL => {
                    // Expect one global set
                    check_state!(
                        r.is_uninitialized_declaration(gn, compiler)
                            || Some(check_not_null!(r.get_chunk(gn))) == initial_chunk,
                        "%s",
                        r.to_string(gn, compiler)
                    );
                    continue;
                }
                RefType::SET_FROM_LOCAL | RefType::GET_AND_SET_FROM_LOCAL => {
                    panic!("java.lang.IllegalStateException")
                }
                RefType::ALIASING_GET
                | RefType::DIRECT_GET
                | RefType::PROTOTYPE_GET
                | RefType::CALL_GET
                | RefType::SUBCLASSING_GET => {
                    // This name has a reference in a different chunk that is not guaranteed to be
                    // loaded before the alias is initialized. In an ideal world, we might just
                    // back off inlining this alias entirely. Unfortunately, that causes new
                    // runtime errors in practice, because property collapsing is intentionally
                    // unsafe and will collapse properties even when it will break code, and so
                    // it's safer to still inling usages of this alias except for the specific
                    // invalid cross-chunk dependency(s).
                    let ref_chunk = r.get_chunk(gn);
                    if inlinability != Inlinability::INLINE_UNLESS_INVALID_CROSS_CHUNK_DEPENDENCY
                        && ref_chunk != initial_chunk
                        && !compiler.get_chunk_graph().unwrap().depends_on(
                            check_not_null!(ref_chunk.as_ref()),
                            check_not_null!(initial_chunk.as_ref()),
                        )
                    {
                        inlinability = Inlinability::INLINE_UNLESS_INVALID_CROSS_CHUNK_DEPENDENCY;
                    }
                    continue;
                }
                RefType::DELETE_PROP => {
                    return Inlinability::DO_NOT_INLINE;
                }
            }
        }
        inlinability
    }

    // port: GlobalNamespace.Name#hasOneRealGlobalSet
    fn has_one_real_global_set(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        let data = gn.name_data(self);
        data.global_sets == 1
            || (data.global_sets == 2
                && check_not_null!(data.declaration).is_uninitialized_declaration(gn, ast))
    }

    // port: GlobalNamespace.Name#canCollapse
    pub fn can_collapse(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        self.can_collapse_or_inline(gn, ast).can_collapse()
    }

    /// Determines whether it's safe to collapse properties on an objects
    ///
    /// For legacy reasons, both CollapseProperties and AggressiveInlineAliases share the same
    /// logic when deciding whether to inline properties or to collapse them.
    // port: GlobalNamespace.Name#canCollapseOrInline
    pub fn can_collapse_or_inline(self, gn: &GlobalNamespace, ast: &Ast) -> Inlinability {
        if self.in_externs(gn) {
            // condition (d)
            self.log_decision(gn, Inlinability::DO_NOT_INLINE, "declared in externs");
            return Inlinability::DO_NOT_INLINE;
        }
        if self.is_get_or_set_definition(gn) {
            // condition (e)
            self.log_decision(gn, Inlinability::DO_NOT_INLINE, "getter / setter");
            return Inlinability::DO_NOT_INLINE;
        }
        if self.is_collapsing_explicitly_denied(gn) {
            // condition (c)
            self.log_decision(gn, Inlinability::DO_NOT_INLINE, "@nocollapse");
            return Inlinability::DO_NOT_INLINE;
        }

        if self.references_super_or_inner_class_name(gn, ast) {
            // condition (f)
            self.log_decision(
                gn,
                Inlinability::DO_NOT_INLINE,
                "references super or inner class name",
            );
            return Inlinability::DO_NOT_INLINE;
        }

        if self.is_to_string_value_of_in_object_literal(gn) {
            self.log_decision(
                gn,
                Inlinability::DO_NOT_INLINE,
                concat!(
                    "references explicit definition of toString/valueOf functions used ",
                    "implicitly in the JS",
                    " language"
                ),
            );
            return Inlinability::DO_NOT_INLINE;
        }

        let data = gn.name_data(self);
        if data.delete_props > 0 {
            // If we inline or collapse, then the delete operation will be incorrect.
            self.log_decision(
                gn,
                Inlinability::DO_NOT_INLINE,
                "delete operator is used on this property",
            );
            return Inlinability::DO_NOT_INLINE;
        }

        if let Some(decl) = self.get_declaration(gn) {
            let declaration = decl.get_node(gn).unwrap();
            if declaration.get_parent(ast).unwrap().is_object_lit(ast) {
                if gn.declaration_has_following_object_spread_sibling(ast, declaration) {
                    // Case: `var x = {a: 0, ...b, c: 2}` where declaration is `a` but not `c`.
                    // Following spreads may overwrite the declaration.
                    self.log_decision(
                        gn,
                        Inlinability::DO_NOT_INLINE,
                        "obj lit property followed by spread",
                    );
                    return Inlinability::DO_NOT_INLINE;
                }
                // We may be in a deeply nested object literal like, `{a: {b: {c: 1}}}`, so find
                // the outermost object literal node and check its ancestor expressions to
                // determine whether it is used conditionally.
                let mut child = gn.get_outermost_object_lit(ast, declaration);
                let mut p = child.get_parent(ast);
                while let Some(pp) = p
                    && pp.get_parent(ast).is_some()
                    && !NodeUtil::is_statement(ast, pp)
                {
                    if (pp.is_or(ast) && pp.get_first_child(ast) != Some(child))
                        || (pp.is_hook(ast) && pp.get_first_child(ast) != Some(child))
                        || (pp.is_and(ast) && pp.get_first_child(ast) != Some(child))
                    {
                        // Case: `var x = y || {a: b}` or `var x = cond ? y : {a: b}` or
                        // `var x = cond && {a: b}`.
                        self.log_decision(
                            gn,
                            Inlinability::DO_NOT_INLINE,
                            "conditional definition",
                        );
                        return Inlinability::DO_NOT_INLINE;
                    }
                    child = pp;
                    p = pp.get_parent(ast);
                }
            }
        }

        // condition (a)
        let is_unchanged_through_full_name = (data.global_sets > 0 || data.local_sets > 0)
            && data.local_sets_with_no_collapse == 0
            && data.delete_props == 0;
        // additional information about condition (b)
        let parent_inlinability = match data.parent {
            None => Inlinability::INLINE_COMPLETELY,
            Some(parent) => parent.can_collapse_or_inline_child_names(gn, ast),
        };

        // if condition (a) or condition (b) is not true, but this is a declared name, we may need
        // to allow inlining usages of a variable but keep the declaration.
        match parent_inlinability {
            Inlinability::INLINE_COMPLETELY => {
                if is_unchanged_through_full_name {
                    self.log_decision(
                        gn,
                        Inlinability::INLINE_COMPLETELY,
                        "parent inlineable: unchanged through full name",
                    );
                    Inlinability::INLINE_COMPLETELY
                } else {
                    // maybe inline usages of this name, but only if a declared type.
                    // non-declared-types just back off and don't inline at all
                    let unsafe_inlinablility =
                        self.get_unsafe_inlinability_based_on_declared_type(gn);
                    self.log_decision(
                        gn,
                        unsafe_inlinablility,
                        "parent inlineable: changed through full name",
                    );
                    unsafe_inlinablility
                }
            }
            Inlinability::INLINE_BUT_KEEP_DECLARATION
            | Inlinability::INLINE_UNLESS_INVALID_CROSS_CHUNK_DEPENDENCY => {
                // this is definitely not safe to completely inline/collapse of its parent
                // if it's a declared type, we should still partially inline it and completely
                // collapse it if not a declared type we should partially inline it iff the other
                // conditions hold
                if self.is_declared_type(gn) {
                    let unsafe_inlinability =
                        self.get_unsafe_inlinability_based_on_declared_type(gn);
                    self.log_decision(
                        gn,
                        unsafe_inlinability,
                        "parent unsafely inlineable & is declared type",
                    );
                    unsafe_inlinability
                } else if is_unchanged_through_full_name {
                    self.log_decision(
                        gn,
                        parent_inlinability,
                        "parent unsafely inlineable & unchanged through full name",
                    );
                    parent_inlinability
                } else {
                    // Not a declared type. We may still 'partially' inline it because it must be
                    // a property on an @enum or @constructor, but only if it actually matches
                    // conditions (a) and (b)
                    self.log_decision(
                        gn,
                        Inlinability::DO_NOT_INLINE,
                        "parent unsafely inlineable & changed through full name",
                    );
                    Inlinability::DO_NOT_INLINE
                }
            }
            Inlinability::DO_NOT_INLINE => {
                // If the parent is unsafely to collapse/inline, we will still inline it if it's
                // on a declaredType (i.e. @constructor or @enum), but we propagate the
                // information that the parent is unsafe. If this is not a declared type, return
                // DO_NOT_INLINE.
                let unsafe_inlinability = self.get_unsafe_inlinability_based_on_declared_type(gn);
                self.log_decision(gn, unsafe_inlinability, "parent cannot be inlined");
                unsafe_inlinability
            }
        }
    }

    // port: GlobalNamespace.Name#logDecision
    fn log_decision(self, gn: &GlobalNamespace, inlinability: Inlinability, reason: &str) {
        gn.with_decisions_log(|decisions_log| {
            decisions_log.log_format(format_args!(
                "{}: {}: {}",
                self.get_full_name(gn),
                inlinability,
                reason
            ));
        });
    }

    // port: GlobalNamespace.Name#getUnsafeInlinabilityBasedOnDeclaredType
    #[allow(clippy::if_same_then_else)] // Java control flow.
    fn get_unsafe_inlinability_based_on_declared_type(self, gn: &GlobalNamespace) -> Inlinability {
        if self.get_boolean_property(gn, NameProp::CONSTRUCTOR_TYPE) {
            Inlinability::INLINE_BUT_KEEP_DECLARATION
        } else if self.get_boolean_property(gn, NameProp::INTERFACE_TYPE) {
            Inlinability::INLINE_BUT_KEEP_DECLARATION
        } else if self.get_boolean_property(gn, NameProp::ENUM_TYPE) {
            Inlinability::INLINE_BUT_KEEP_DECLARATION
        } else if self.get_boolean_property(gn, NameProp::NOT_A_TYPE) {
            Inlinability::DO_NOT_INLINE
        } else {
            panic!(
                "{}",
                format!("name missing declaredType value: {}", self.to_string(gn))
            )
        }
    }

    /// Examines ES6 class members for some syntax that blocks collapsing
    ///
    /// Specifically, this looks for super references and references to inner class names. These
    /// are unique to ES6 static class members so we don't need more general handling.
    // port: GlobalNamespace.Name#referencesSuperOrInnerClassName
    pub fn references_super_or_inner_class_name(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        let Some(r) = self.get_declaration(gn) else {
            return false;
        };
        let member = r.get_node(gn);
        let Some(member) = member else {
            return false;
        };
        if !(member.is_static_member(ast) && member.get_parent(ast).unwrap().is_class_members(ast))
        {
            return false;
        }
        // Get either the function body or the member field initializer (which may be null if
        // there is no initializer).
        let body = if member.is_member_field_def(ast) {
            member.get_first_child(ast)
        } else {
            Some(NodeUtil::get_function_body(
                ast,
                member.get_first_child(ast).unwrap(),
            ))
        };
        if body.is_some_and(|body| NodeUtil::references_super(ast, body)) {
            return true;
        }

        let class_node = member.get_grandparent(ast).unwrap();
        if NodeUtil::is_class_declaration(ast, class_node) {
            return false; // e.g. class C {}
        }

        let inner_name_node = class_node.get_first_child(ast).unwrap();
        !inner_name_node.is_empty(ast) // e.g. const C = class {};
            && NodeUtil::is_name_referenced(ast, member, inner_name_node.get_string(ast))
    }

    // port: GlobalNamespace.Name#isSetInLoop
    fn is_set_in_loop(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        let r = self.get_declaration(gn);
        if let Some(r) = r {
            let n = r.get_node(gn);
            if let Some(n) = n {
                return NodeUtil::is_within_loop(ast, n);
            }
        }
        false
    }

    // port: GlobalNamespace.Name#isGetOrSetDefinition
    pub fn is_get_or_set_definition(self, gn: &GlobalNamespace) -> bool {
        self.get_boolean_property(gn, NameProp::GET_SET)
    }

    // port: GlobalNamespace.Name#canCollapseUnannotatedChildNames
    pub fn can_collapse_unannotated_child_names(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        self.can_collapse_or_inline_child_names(gn, ast)
            .can_collapse()
    }

    // toString/valueOf are implicitly used as part of the JS language should not be collapsed.
    // port: GlobalNamespace.Name#isToStringValueOfInObjectLiteral
    pub fn is_to_string_value_of_in_object_literal(self, gn: &GlobalNamespace) -> bool {
        let parent = self.get_parent(gn);
        let base_name = self.get_base_name(gn);
        self.is_function(gn)
            && parent.is_some_and(|parent| parent.is_object_literal(gn))
            && (base_name == "toString" || base_name == "valueOf")
    }

    /// Returns whether to assume that child properties of this name are collapsible/inlineable
    ///
    /// For legacy reasons, both CollapseProperties and AggressiveInlineAliases share the same
    /// logic when deciding whether to inline properties or to collapse them.
    // port: GlobalNamespace.Name#canCollapseOrInlineChildNames
    pub fn can_collapse_or_inline_child_names(
        self,
        gn: &GlobalNamespace,
        ast: &Ast,
    ) -> Inlinability {
        let data = gn.name_data(self);
        // condition (a) and (b)
        if self.get_boolean_property(gn, NameProp::OTHER_OBJECT) {
            self.log_child_names_decision(gn, Inlinability::DO_NOT_INLINE, "NameProp.OTHER_OBJECT");
            return Inlinability::DO_NOT_INLINE;
        } else if self.is_get_or_set_definition(gn) {
            self.log_child_names_decision(gn, Inlinability::DO_NOT_INLINE, "getter/setter");
            return Inlinability::DO_NOT_INLINE;
        } else if data.global_sets != 1 {
            self.log_child_names_decision_supplier(gn, Inlinability::DO_NOT_INLINE, || {
                format!("set {} times globally", data.global_sets)
            });
            return Inlinability::DO_NOT_INLINE;
        } else if data.local_sets != 0 {
            self.log_child_names_decision_supplier(gn, Inlinability::DO_NOT_INLINE, || {
                format!("set {} times locally", data.local_sets)
            });
            return Inlinability::DO_NOT_INLINE;
        } else if data.delete_props != 0 {
            self.log_child_names_decision_supplier(gn, Inlinability::DO_NOT_INLINE, || {
                format!("properties are deleted {} times", data.delete_props)
            });
            return Inlinability::DO_NOT_INLINE;
        }

        // Don't try to collapse if the one global set is a twin reference.
        // We could theoretically handle this case in CollapseProperties, but
        // it's probably not worth the effort.
        let declaration = check_not_null!(data.declaration);
        if declaration.is_twin(gn) {
            self.log_child_names_decision(gn, Inlinability::DO_NOT_INLINE, "twinned declaration");
            return Inlinability::DO_NOT_INLINE;
        }

        if self.is_collapsing_explicitly_denied(gn) {
            // condition (d)
            self.log_child_names_decision(gn, Inlinability::DO_NOT_INLINE, "@nocollapse");
            return Inlinability::DO_NOT_INLINE;
        }

        if self.is_set_in_loop(gn, ast) {
            // condition (a)
            self.log_child_names_decision(gn, Inlinability::DO_NOT_INLINE, "set in a loop");
            return Inlinability::DO_NOT_INLINE;
        }

        if self.uses_has_own_property(gn) {
            // condition (b)
            self.log_child_names_decision(
                gn,
                Inlinability::DO_NOT_INLINE,
                "hasOwnProperty() call exists",
            );
            return Inlinability::DO_NOT_INLINE;
        }

        if self.value_implicitly_supports_aliasing(gn) {
            // condition (f)
            self.log_child_names_decision(
                gn,
                Inlinability::DO_NOT_INLINE,
                "value implicitly supports aliasing",
            );
            return Inlinability::DO_NOT_INLINE;
        }

        // If this is a key of an aliased object literal, then it will be aliased
        // later. So we won't be able to collapse its properties.
        // condition (b)
        if data
            .parent
            .is_some_and(|parent| parent.should_keep_keys(gn))
        {
            let unsafe_inlinability = self.get_unsafe_inlinability_based_on_declared_type(gn);
            self.log_child_names_decision(gn, unsafe_inlinability, "parent.shouldKeepKeys()");
            return unsafe_inlinability;
        }

        // If this is aliased, then its properties can't be collapsed either. but we may do so
        // anyway if it's a declared type.
        // condition (b)
        if data.aliasing_gets > 0 {
            let unsafe_inlinability = self.get_unsafe_inlinability_based_on_declared_type(gn);
            self.log_child_names_decision_supplier(gn, unsafe_inlinability, || {
                format!("{} aliasing gets exist", data.aliasing_gets)
            });
            return unsafe_inlinability;
        }

        let Some(parent) = data.parent else {
            // this is completely safe to inline! yay
            self.log_child_names_decision(
                gn,
                Inlinability::INLINE_COMPLETELY,
                "no reason not to inline",
            );
            return Inlinability::INLINE_COMPLETELY;
        };

        // Cases are:
        //  - parent is safe to completely inline. then same for this name
        //  - parent is unsafe but should still be inlined. then same for this name
        //  - parent is unsafe, should not be inlined at all. then return either DO_NOT_INLINE,
        //    or maybe unsafely inline if this is a ctor property
        let parent_inlinability = parent.can_collapse_or_inline_child_names(gn, ast);
        if parent_inlinability == Inlinability::DO_NOT_INLINE {
            // the parent name is used in a way making this unsafe to inline, but we might want
            // to inline usages of this name
            let unsafe_inlinability = self.get_unsafe_inlinability_based_on_declared_type(gn);
            self.log_child_names_decision(gn, unsafe_inlinability, "parent is not inlineable");
            return unsafe_inlinability;
        }
        self.log_child_names_decision(gn, parent_inlinability, "inherited from parent");
        parent_inlinability
    }

    // port: GlobalNamespace.Name#logChildNamesDecision(Inlinability,String)
    fn log_child_names_decision(
        self,
        gn: &GlobalNamespace,
        inlinability: Inlinability,
        reason: &str,
    ) {
        gn.with_decisions_log(|decisions_log| {
            decisions_log.log_format(format_args!(
                "{}: children: {}: {}",
                self.get_full_name(gn),
                inlinability,
                reason
            ));
        });
    }

    // port: GlobalNamespace.Name#logChildNamesDecision(Inlinability,Supplier)
    fn log_child_names_decision_supplier(
        self,
        gn: &GlobalNamespace,
        inlinability: Inlinability,
        reason_supplier: impl Fn() -> String,
    ) {
        gn.with_decisions_log(|decisions_log| {
            decisions_log.log(&mut || {
                format!(
                    "{}: children: {}: {}",
                    self.get_full_name(gn),
                    inlinability,
                    reason_supplier()
                )
            });
        });
    }

    // port: GlobalNamespace.Name#valueImplicitlySupportsAliasing
    fn value_implicitly_supports_aliasing(self, gn: &GlobalNamespace) -> bool {
        if !gn.enable_implicitly_aliased_values {
            return false;
        }

        if self.get_boolean_property(gn, NameProp::CLASS) {
            // Properties on classes may be referenced via `this` in static methods.
            return true;
        }
        if self.get_boolean_property(gn, NameProp::FUNCTION) {
            // We want ES5 ctors/interfaces to behave consistently with ES6 because:
            // - transpilation should not change behaviour
            // - updating code shouldn't be hindered by behaviour changes
            let jsdoc = self.get_jsdoc_info(gn);
            return jsdoc.is_some_and(|jsdoc| jsdoc.is_constructor_or_interface());
        }
        false
    }

    /// Whether this is an object literal that needs to keep its keys.
    // port: GlobalNamespace.Name#shouldKeepKeys
    pub fn should_keep_keys(self, gn: &GlobalNamespace) -> bool {
        self.is_object_literal(gn)
            && (gn.name_data(self).aliasing_gets > 0 || self.is_collapsing_explicitly_denied(gn))
    }

    // port: GlobalNamespace.Name#needsToBeStubbed
    pub fn needs_to_be_stubbed(self, gn: &GlobalNamespace) -> bool {
        let data = gn.name_data(self);
        data.global_sets == 0
            && data.local_sets > 0
            && data.local_sets_with_no_collapse == 0
            && !self.is_collapsing_explicitly_denied(gn)
    }

    // port: GlobalNamespace.Name#setDeclaredTypeKind
    fn set_declared_type_kind(self, gn: &mut GlobalNamespace, declared_type: NameProp) {
        check_argument!(
            (declared_type.bit() & NameProp::DECLARED_TYPE_KIND_MASK) != 0,
            "Unexpected NameProp for declaredType %s",
            declared_type
        );
        let data = gn.name_data_mut(self);
        data.property_bit_set =
            (data.property_bit_set & !NameProp::DECLARED_TYPE_KIND_MASK) | declared_type.bit();
        if declared_type != NameProp::NOT_A_TYPE {
            let mut ancestor = gn.name_data(self).parent;
            while let Some(a) = ancestor {
                a.set_boolean_property(gn, NameProp::IS_DECLARED);
                ancestor = gn.name_data(a).parent;
            }
        }
    }

    // port: GlobalNamespace.Name#setNameType
    fn set_name_type(self, gn: &mut GlobalNamespace, r#type: NameProp) {
        check_argument!(
            (r#type.bit() & NameProp::NAME_KIND_MASK) != 0,
            "Unexpected NameProp for nameType %s",
            r#type
        );
        let data = gn.name_data_mut(self);
        data.property_bit_set = (data.property_bit_set & !NameProp::NAME_KIND_MASK) | r#type.bit();
    }

    // port: GlobalNamespace.Name#isDeclaredType
    pub fn is_declared_type(self, gn: &GlobalNamespace) -> bool {
        let property_bit_set = gn.name_data(self).property_bit_set;
        check_state!(
            (property_bit_set & NameProp::DECLARED_TYPE_KIND_MASK) != 0,
            &property_bit_set.to_string()
        );
        !self.get_boolean_property(gn, NameProp::NOT_A_TYPE)
    }

    // port: GlobalNamespace.Name#isConstructor
    pub fn is_constructor(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        let decl_node = check_not_null!(gn.name_data(self).declaration)
            .get_node(gn)
            .unwrap();
        let rvalue_node = NodeUtil::get_r_value_of_l_value(ast, decl_node);
        let jsdoc = NodeUtil::get_best_jsdoc_info(ast, decl_node);
        rvalue_node.is_some_and(|rvalue_node| rvalue_node.is_function(ast))
            && jsdoc.is_some_and(|jsdoc| jsdoc.is_constructor())
    }

    /// Determines whether this name is a prefix of at least one class or enum name. Because
    /// classes and enums are always collapsed, the namespace will have different properties in
    /// compiled code than in uncompiled code.
    // port: GlobalNamespace.Name#isNamespaceObjectLit
    pub fn is_namespace_object_lit(self, gn: &GlobalNamespace) -> bool {
        self.get_boolean_property(gn, NameProp::IS_DECLARED) && self.is_object_literal(gn)
    }

    /// Determines whether this is a simple name (as opposed to a qualified name).
    // port: GlobalNamespace.Name#isSimpleName
    pub fn is_simple_name(self, gn: &GlobalNamespace) -> bool {
        gn.name_data(self).parent.is_none()
    }

    // port: GlobalNamespace.Name#getTypeDebugString
    pub fn get_type_debug_string(self, gn: &GlobalNamespace) -> &'static str {
        if self.is_class(gn) {
            "CLASS"
        } else if self.is_function(gn) {
            "FUNCTION"
        } else if self.is_object_literal(gn) {
            "OBJECTLIT"
        } else if self.is_get_or_set_definition(gn) {
            "GET_SET"
        } else if self.get_boolean_property(gn, NameProp::OTHER_OBJECT) {
            "OTHER"
        } else {
            panic!(
                "Missing NameProp for name kind in {}",
                gn.name_data(self).property_bit_set
            )
        }
    }

    // port: GlobalNamespace.Name#toString
    pub fn to_string(self, gn: &GlobalNamespace) -> String {
        let data = gn.name_data(self);
        format!(
            "{} ({}): {}",
            self.get_full_name(gn),
            self.get_type_debug_string(gn),
            [
                format!("globalSets={}", data.global_sets),
                format!("localSets={}", data.local_sets),
                format!("totalGets={}", data.total_gets),
                format!("aliasingGets={}", data.aliasing_gets),
                format!("callGets={}", data.call_gets),
                format!("subclassingGets={}", data.subclassing_gets),
            ]
            .join(", ")
        )
    }

    // port: GlobalNamespace.Name#getJSDocInfo
    pub fn get_jsdoc_info(self, gn: &GlobalNamespace) -> Option<Arc<JSDocInfo>> {
        // e.g.
        // /** @type {string} */ X.numProp;     // could be a declaration, but...
        // /** @type {number} */ X.numProp = 3; // assignment wins
        let data = gn.name_data(self);
        if data.first_declaration_jsdoc_info.is_some() {
            data.first_declaration_jsdoc_info.clone()
        } else {
            data.first_qname_declaration_without_assignment_jsdoc_info
                .clone()
        }
    }

    /// Tries to get the doc info for a given declaration ref.
    // port: GlobalNamespace.Name#getDocInfoForDeclaration
    fn get_doc_info_for_declaration(
        self,
        gn: &GlobalNamespace,
        ast: &Ast,
        r: Ref,
    ) -> Option<Arc<JSDocInfo>> {
        if let Some(ref_node) = r.get_node(gn) {
            let ref_parent = ref_node.get_parent(ast);
            let Some(ref_parent) = ref_parent else {
                // May happen when inlineAliases removes refs from the AST.
                return None;
            };
            match ref_parent.get_token(ast) {
                Token::FUNCTION | Token::ASSIGN | Token::CLASS => {
                    return ref_parent.get_jsdoc_info(ast);
                }
                Token::VAR | Token::LET | Token::CONST => {
                    return if Some(ref_node) == ref_parent.get_first_child(ast) {
                        ref_parent.get_jsdoc_info(ast)
                    } else {
                        ref_node.get_jsdoc_info(ast)
                    };
                }
                Token::OBJECTLIT | Token::CLASS_MEMBERS => {
                    return ref_node.get_jsdoc_info(ast);
                }
                _ => {}
            }
        }

        None
    }

    // port: GlobalNamespace.Name#isModuleExport
    pub fn is_module_export(self, gn: &GlobalNamespace) -> bool {
        self.get_boolean_property(gn, NameProp::IS_MODULE_PROP)
    }
}

// -------------------------------------------------------------------------

/// `GlobalNamespace.Ref.Type`.
///
/// Note: we are more aggressive about collapsing @enum and @constructor declarations than implied
/// here, see Name#canCollapse
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RefType {
    /// Set in the scope in which a name is declared, either the global scope or a module scope:
    /// `a.b.c = 0;` or `goog.module('mod'); exports.Foo = class {};`
    SET_FROM_GLOBAL,
    /// Set in a local scope: function f() { a.b.c = 0; }
    SET_FROM_LOCAL,
    /// Combined get and set in the scope in which a name is declared, either the global scope or
    /// a module scope: `const c = a.b.c = 0;` or `goog.module('mod'); exports.Foo = class {};`
    GET_AND_SET_FROM_GLOBAL,
    /// Combined get and set in a local scope: function f() { return a.b.c = 0; }
    GET_AND_SET_FROM_LOCAL,
    /// Get a name's prototype: a.b.c.prototype
    PROTOTYPE_GET,
    /// Includes all uses that prevent a name's properties from being collapsed: var x = a.b.c
    /// f(a.b.c) new Foo(a.b.c)
    ALIASING_GET,
    /// Includes all uses that prevent a name from being completely eliminated:
    /// goog.inherits(anotherName, a.b.c) new a.b.c() x instanceof a.b.c void a.b.c if (a.b.c) {}
    DIRECT_GET,
    /// Calling a name: a.b.c(); Prevents a name from being collapsed if never set.
    CALL_GET,
    /// Deletion of a property: delete a.b.c; Prevents a name from being collapsed at all.
    DELETE_PROP,
    /// ES6 subclassing ref: class extends A {}
    SUBCLASSING_GET,
}

impl fmt::Display for RefType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A global name reference. Contains references to the relevant parse tree node and its ancestors
/// that may be affected.
impl Ref {
    /// Java's package-private `type` field.
    pub fn r#type(self, gn: &GlobalNamespace) -> RefType {
        gn.ref_data(self).r#type
    }

    /// Java's package-private `scope` field: the scope in which the reference is resolved.
    pub fn scope(self, gn: &GlobalNamespace) -> Option<ScopeId> {
        gn.ref_data(self).scope
    }

    // port: GlobalNamespace.Ref#getChunk
    pub fn get_chunk(self, gn: &GlobalNamespace) -> Option<JSChunk> {
        gn.ref_data(self).chunk.clone()
    }

    // port: GlobalNamespace.Ref#getNode
    pub fn get_node(self, gn: &GlobalNamespace) -> Option<NodeId> {
        gn.ref_data(self).node
    }

    // port: GlobalNamespace.Ref#getSourceFile
    pub fn get_source_file(
        self,
        gn: &GlobalNamespace,
        ast: &Ast,
    ) -> Option<Arc<dyn StaticSourceFile>> {
        gn.ref_data(self)
            .node
            .and_then(|node| node.get_static_source_file(ast))
    }

    // port: GlobalNamespace.Ref#getSymbol
    pub fn get_symbol(self) -> Name {
        panic!("java.lang.UnsupportedOperationException")
    }

    // port: GlobalNamespace.Ref#isDeleteProp
    pub fn is_delete_prop(self, gn: &GlobalNamespace) -> bool {
        self.r#type(gn) == RefType::DELETE_PROP
    }

    // port: GlobalNamespace.Ref#isSubclassingGet
    pub fn is_subclassing_get(self, gn: &GlobalNamespace) -> bool {
        self.r#type(gn) == RefType::SUBCLASSING_GET
    }

    /// Whether this is a "twin" ref, i.e. a ref that is both a get and a set.
    ///
    /// Example: `a.b` from `x = a.b = 0;`
    // port: GlobalNamespace.Ref#isTwin
    pub fn is_twin(self, gn: &GlobalNamespace) -> bool {
        matches!(
            self.r#type(gn),
            RefType::GET_AND_SET_FROM_GLOBAL | RefType::GET_AND_SET_FROM_LOCAL
        )
    }

    // port: GlobalNamespace.Ref#isGet
    pub fn is_get(self, gn: &GlobalNamespace) -> bool {
        matches!(
            self.r#type(gn),
            RefType::DIRECT_GET
                | RefType::ALIASING_GET
                | RefType::SUBCLASSING_GET
                | RefType::CALL_GET
                | RefType::GET_AND_SET_FROM_GLOBAL
                | RefType::GET_AND_SET_FROM_LOCAL
                | RefType::PROTOTYPE_GET
        )
    }

    // port: GlobalNamespace.Ref#isAliasingGet
    pub fn is_aliasing_get(self, gn: &GlobalNamespace) -> bool {
        matches!(
            self.r#type(gn),
            RefType::ALIASING_GET
                | RefType::GET_AND_SET_FROM_GLOBAL
                | RefType::GET_AND_SET_FROM_LOCAL
        )
    }

    // port: GlobalNamespace.Ref#isSet
    pub fn is_set(self, gn: &GlobalNamespace) -> bool {
        matches!(
            self.r#type(gn),
            RefType::SET_FROM_GLOBAL
                | RefType::SET_FROM_LOCAL
                | RefType::GET_AND_SET_FROM_GLOBAL
                | RefType::GET_AND_SET_FROM_LOCAL
        )
    }

    // port: GlobalNamespace.Ref#isSetFromGlobal
    pub fn is_set_from_global(self, gn: &GlobalNamespace) -> bool {
        self.r#type(gn) == RefType::SET_FROM_GLOBAL
            || self.r#type(gn) == RefType::GET_AND_SET_FROM_GLOBAL
    }

    /// True for `var name;` and similar cases.
    // port: GlobalNamespace.Ref#isUninitializedDeclaration
    pub fn is_uninitialized_declaration(self, gn: &GlobalNamespace, ast: &Ast) -> bool {
        let node = check_not_null!(self.get_node(gn));
        node.is_name(ast)
            && NodeUtil::is_name_declaration(ast, node.get_parent(ast))
            && !node.has_children(ast)
    }

    // port: GlobalNamespace.Ref#toString
    pub fn to_string(self, gn: &GlobalNamespace, compiler: &AbstractCompiler) -> String {
        // MoreObjects.toStringHelper(this).omitNullValues()
        let data = gn.ref_data(self);
        let mut fields = vec![format!("type={}", data.r#type)];
        if let Some(node) = data.node {
            fields.push(format!("node={}", node.to_string(compiler)));
        }
        if let Some(scope) = data.scope {
            fields.push(format!(
                "scope={}",
                AbstractScope::to_string(scope, compiler)
            ));
        }
        format!("Ref{{{}}}", fields.join(", "))
    }
}

/// Enum of boolean properties of a Name, to be stored in one bit set per name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameProp {
    // Increment the "index" passed to the constructor by 1 for each enum member
    IS_DECLARED,
    IS_MODULE_PROP,
    IS_PROVIDED, // If this name was in any goog.provide() calls.
    IS_USED_HAS_OWN_PROPERTY,
    IS_EXTERN, // corresponds to SourceKind.EXTERN

    // Mutually exclusive properties indicating what kind of Closure type this Name is, if any.
    // Corresponds to DECLARED_TYPE_KIND_MASK.
    CONSTRUCTOR_TYPE, // a non-interface class {} or a `/** @constructor */ function`
    INTERFACE_TYPE,   // a `/** @interface */` or `/** @record */`
    ENUM_TYPE,        // an `/** @enum */`
    NOT_A_TYPE,

    // Mutually exclusive properties indicating what kind of JavaScript entity this Name is.
    // Corresponds to NAME_KIND_MASK.
    CLASS,        // class C {}
    OBJECTLIT,    // var x = {};
    FUNCTION,     // function f() {}
    GET_SET,      // a getter, setter, or both; e.g. `obj.b` in `const obj = {set b(x) {}};`
    OTHER_OBJECT, // anything else, including `var x = 1;`, var x = new Something();`, etc.
}

impl NameProp {
    const DECLARED_TYPE_KIND_MASK: i32 = NameProp::CONSTRUCTOR_TYPE.bit()
        | NameProp::INTERFACE_TYPE.bit()
        | NameProp::ENUM_TYPE.bit()
        | NameProp::NOT_A_TYPE.bit();
    const NAME_KIND_MASK: i32 = NameProp::CLASS.bit()
        | NameProp::OBJECTLIT.bit()
        | NameProp::FUNCTION.bit()
        | NameProp::GET_SET.bit()
        | NameProp::OTHER_OBJECT.bit();

    // port: GlobalNamespace.NameProp#NameProp
    const fn index(self) -> i32 {
        match self {
            NameProp::IS_DECLARED => 0,
            NameProp::IS_MODULE_PROP => 1,
            NameProp::IS_PROVIDED => 2,
            NameProp::IS_USED_HAS_OWN_PROPERTY => 3,
            NameProp::IS_EXTERN => 4,
            NameProp::CONSTRUCTOR_TYPE => 5,
            NameProp::INTERFACE_TYPE => 6,
            NameProp::ENUM_TYPE => 7,
            NameProp::NOT_A_TYPE => 8,
            NameProp::CLASS => 9,
            NameProp::OBJECTLIT => 10,
            NameProp::FUNCTION => 11,
            NameProp::GET_SET => 12,
            NameProp::OTHER_OBJECT => 13,
        }
    }

    /// Java's `bit` field: some power of 2.
    const fn bit(self) -> i32 {
        1 << self.index()
    }
}

impl fmt::Display for NameProp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// `record ObjLitStringKeyAnalysis(@Nullable String nameString, @Nullable NameProp nameType)`.
struct ObjLitStringKeyAnalysis {
    name_string: Option<JsString>,
    name_type: NameProp,
}

impl ObjLitStringKeyAnalysis {
    /// The object literal key is used to define a property.
    /// `Object.defineProperty(parent.qname, { strKeyName: value, { get: ..., } })`
    // port: GlobalNamespace.ObjLitStringKeyAnalysis#forObjectDefineProperty
    fn for_object_define_property(name_string: JsString) -> ObjLitStringKeyAnalysis {
        // Technically the definition may not have a getter or setter, but we'll just
        // always pretend it does, because we cannot inline and collapse properties defined this
        // way.
        ObjLitStringKeyAnalysis {
            name_string: Some(name_string),
            name_type: NameProp::GET_SET,
        }
    }

    /// The object literal key represents `parent.qname = { strKeyName: value }`
    // port: GlobalNamespace.ObjLitStringKeyAnalysis#forObjLitAssignment
    fn for_obj_lit_assignment(
        name_string: JsString,
        name_type: NameProp,
    ) -> ObjLitStringKeyAnalysis {
        ObjLitStringKeyAnalysis {
            name_string: Some(name_string),
            name_type,
        }
    }

    /// The object literal key does not represent a qualified name assignment.
    // port: GlobalNamespace.ObjLitStringKeyAnalysis#forNonReference
    fn for_non_reference() -> ObjLitStringKeyAnalysis {
        ObjLitStringKeyAnalysis {
            name_string: /* nameString= */ None,
            name_type: NameProp::OTHER_OBJECT,
        }
    }
}

// -------------------------------------------------------------------------
// Rust-only test support (no Java counterpart): a read-only view of the private Java fields of
// GlobalNamespace, Name and Ref, in Java declaration order, for the unit-record field dumps
// (UnitRecorder#collect of a `GlobalNamespace` host field). No behaviour change.

/// The fields of a `GlobalNamespace`, in Java declaration order (`compiler` omitted: it is the
/// caller's compiler).
pub struct GlobalNamespaceUnitDump {
    pub enable_implicitly_aliased_values: bool,
    pub root: NodeId,
    pub externs_root: Option<NodeId>,
    pub global_root: NodeId,
    /// `LinkedHashMap<Node, Boolean>` in insertion order.
    pub spread_sibling_cache: Vec<(NodeId, bool)>,
    /// `null` until `process` has run.
    pub source_kind: Option<SourceKind>,
    pub generated: bool,
    pub decisions_log: Option<DecisionsLog>,
    pub global_names: Vec<Name>,
    /// `LinkedHashMap<String, Name>` in insertion order.
    pub name_map: Vec<(JsString, Name)>,
    /// `HashBasedTable<ModuleMetadata, String, Name>` cells, in Rust insertion order (Java's
    /// iteration order of this table depends on identity hash codes).
    pub name_map_by_module: Vec<(Arc<ModuleMetadata>, JsString, Name)>,
}

/// The shape of a `Name`'s `Object refsForNode` field.
pub enum RefsForNodeUnitDump {
    Null,
    Single(Ref),
    /// `LinkedHashMap<Node, Ref>` in insertion order (Java allows the null key).
    Map(Vec<(Option<NodeId>, Ref)>),
}

/// The fields of a `GlobalNamespace.Name`, in Java declaration order.
pub struct NameUnitDump {
    pub base_name: JsString,
    pub parent: Option<Name>,
    pub props: Option<Vec<Name>>,
    pub declaration: Option<Ref>,
    pub initialization: Option<Ref>,
    pub refs_for_node: RefsForNodeUnitDump,
    pub global_sets: i32,
    pub local_sets: i32,
    pub local_sets_with_no_collapse: i32,
    pub aliasing_gets: i32,
    pub total_gets: i32,
    pub call_gets: i32,
    pub delete_props: i32,
    pub subclassing_gets: i32,
    pub property_bit_set: i32,
    pub first_declaration_jsdoc_info: Option<Arc<JSDocInfo>>,
    pub first_qname_declaration_without_assignment_jsdoc_info: Option<Arc<JSDocInfo>>,
}

/// The fields of a `GlobalNamespace.Ref`, in Java declaration order.
pub struct RefUnitDump {
    pub node: Option<NodeId>,
    pub r#type: RefType,
    pub chunk: Option<JSChunk>,
    pub scope: Option<ScopeId>,
}

impl GlobalNamespace {
    /// Rust-only test support: the Java field values of this namespace.
    pub fn unit_dump_fields(&self) -> GlobalNamespaceUnitDump {
        GlobalNamespaceUnitDump {
            enable_implicitly_aliased_values: self.enable_implicitly_aliased_values,
            root: self.root,
            externs_root: self.externs_root,
            global_root: self.global_root,
            spread_sibling_cache: self
                .spread_sibling_cache
                .lock()
                .unwrap()
                .iter()
                .map(|(&node, &value)| (node, value))
                .collect(),
            source_kind: self.source_kind,
            generated: self.generated,
            decisions_log: self.decisions_log.clone(),
            global_names: self.global_names.clone(),
            name_map: self
                .name_map
                .iter()
                .map(|(key, &name)| (key.clone(), name))
                .collect(),
            name_map_by_module: self
                .name_map_by_module
                .iter()
                .map(|((metadata, key), &name)| (metadata.0.clone(), key.clone(), name))
                .collect(),
        }
    }
}

impl Name {
    /// Rust-only test support: the Java field values of this name.
    pub fn unit_dump_fields(self, gn: &GlobalNamespace) -> NameUnitDump {
        let data = gn.name_data(self);
        NameUnitDump {
            base_name: data.base_name.clone(),
            parent: data.parent,
            props: data.props.clone(),
            declaration: data.declaration,
            initialization: data.initialization,
            refs_for_node: match &data.refs_for_node {
                RefsForNode::Null => RefsForNodeUnitDump::Null,
                RefsForNode::Single(r) => RefsForNodeUnitDump::Single(*r),
                RefsForNode::Map(map) => {
                    RefsForNodeUnitDump::Map(map.iter().map(|(&node, &r)| (node, r)).collect())
                }
            },
            global_sets: data.global_sets,
            local_sets: data.local_sets,
            local_sets_with_no_collapse: data.local_sets_with_no_collapse,
            aliasing_gets: data.aliasing_gets,
            total_gets: data.total_gets,
            call_gets: data.call_gets,
            delete_props: data.delete_props,
            subclassing_gets: data.subclassing_gets,
            property_bit_set: data.property_bit_set,
            first_declaration_jsdoc_info: data.first_declaration_jsdoc_info.clone(),
            first_qname_declaration_without_assignment_jsdoc_info: data
                .first_qname_declaration_without_assignment_jsdoc_info
                .clone(),
        }
    }
}

impl Ref {
    /// Rust-only test support: the Java field values of this ref.
    pub fn unit_dump_fields(self, gn: &GlobalNamespace) -> RefUnitDump {
        let data = gn.ref_data(self);
        RefUnitDump {
            node: data.node,
            r#type: data.r#type,
            chunk: data.chunk.clone(),
            scope: data.scope,
        }
    }
}
