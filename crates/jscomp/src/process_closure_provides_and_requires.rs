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
//   src/com/google/javascript/jscomp/ProcessClosureProvidesAndRequires.java.

#![allow(clippy::collapsible_match, clippy::collapsible_if)] // Keep Java's nested if statements.
use crate::{
    abstract_compiler::AbstractCompiler,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_chunk::JSChunk,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

// port: ProcessClosureProvidesAndRequires#TYPEDEF_CHILD_OF_PROVIDE
pub static TYPEDEF_CHILD_OF_PROVIDE: DiagnosticType = DiagnosticType::error(
    "JSC_TYPEDEF_CHILD_OF_PROVIDE",
    "invalid @typedef goog.provide {0}\nParent namespace {1} is goog.provided and initialized in the same file",
);

// The root Closure namespace
// port: ProcessClosureProvidesAndRequires#GOOG
const GOOG: &str = "goog";

/// Java's `String.valueOf(Object)` for a nullable node, as Guava's precondition messages print it.
fn node_value_of(compiler: &AbstractCompiler, node: Option<NodeId>) -> String {
    node.map_or_else(|| "null".to_string(), |n| n.to_string(compiler))
}

/// Replaces `goog.provide` calls and removes goog.{require,requireType,forwardDeclare} calls.
///
/// We expect all `goog.modules` and `goog.require`s in modules to have been rewritten. This is
/// why all remaining require/requireType calls must refer to a `goog.provide`, although the
/// original JS code may contain `goog.require`s of a `goog.module`.
///
/// This also annotates all provided namespace definitions `a.b = {};` with `Node#IS_NAMESPACE`
/// so that later passes know they refer to a `goog.provide`d namespace.
///
/// If modules have not been rewritten, this pass also includes legacy Closure module namespaces
/// in the list of `ProvidedName`s.
pub struct ProcessClosureProvidesAndRequires {
    // Use a LinkedHashMap because the goog.provides must be processed in a deterministic order.
    provided_names: IndexMap<JsString, ProvidedName>,

    exported_variables: IndexSet<JsString>,

    // If this is true, rewriting will not remove any goog.provide or goog.require calls
    preserve_goog_provides_and_requires: bool,
    requires_to_be_removed: Vec<NodeId>,
    // Whether this instance has already rewritten goog.provides, which can only happen once
    has_rewriting_occurred: bool,
    forward_declares_to_remove: IndexSet<NodeId>,
    ast_factory: AstFactory,
}

/// Typed borrowing view for java.lang.reflect.Field replay; the instance fields remain private.
pub struct ProcessClosureProvidesAndRequiresReplayFields<'a> {
    pub provided_names: &'a IndexMap<JsString, ProvidedName>,
    pub exported_variables: &'a IndexSet<JsString>,
    pub preserve_goog_provides_and_requires: &'a bool,
    pub requires_to_be_removed: &'a [NodeId],
    pub has_rewriting_occurred: &'a bool,
    pub forward_declares_to_remove: &'a IndexSet<NodeId>,
    pub ast_factory: &'a AstFactory,
}

impl ProcessClosureProvidesAndRequires {
    // port: java.lang.reflect.Field#get (native replay access)
    pub fn replay_fields(&self) -> ProcessClosureProvidesAndRequiresReplayFields<'_> {
        ProcessClosureProvidesAndRequiresReplayFields {
            provided_names: &self.provided_names,
            exported_variables: &self.exported_variables,
            preserve_goog_provides_and_requires: &self.preserve_goog_provides_and_requires,
            requires_to_be_removed: &self.requires_to_be_removed,
            has_rewriting_occurred: &self.has_rewriting_occurred,
            forward_declares_to_remove: &self.forward_declares_to_remove,
            ast_factory: &self.ast_factory,
        }
    }
}

/// Typed borrowing view for java.lang.reflect.Field replay; the instance fields remain private.
pub struct ProvidedNameReplayFields<'a> {
    pub namespace: &'a JsString,
    pub first_node: &'a Option<NodeId>,
    pub first_chunk: &'a Option<JSChunk>,
    pub has_implicit_initialization: &'a bool,
    pub explicit_node: &'a Option<NodeId>,
    pub candidate_definition: &'a Option<NodeId>,
    pub minimum_chunk: &'a Option<JSChunk>,
    pub replacement_node: &'a Option<NodeId>,
    pub from_legacy_module: &'a bool,
}

impl ProvidedName {
    // port: java.lang.reflect.Field#get (native replay access)
    pub fn replay_fields(&self) -> ProvidedNameReplayFields<'_> {
        ProvidedNameReplayFields {
            namespace: &self.namespace,
            first_node: &self.first_node,
            first_chunk: &self.first_chunk,
            has_implicit_initialization: &self.has_implicit_initialization,
            explicit_node: &self.explicit_node,
            candidate_definition: &self.candidate_definition,
            minimum_chunk: &self.minimum_chunk,
            replacement_node: &self.replacement_node,
            from_legacy_module: &self.from_legacy_module,
        }
    }
}

impl ProcessClosureProvidesAndRequires {
    // port: ProcessClosureProvidesAndRequires#ProcessClosureProvidesAndRequires
    pub fn new(compiler: &mut AbstractCompiler, preserve_goog_provides_and_requires: bool) -> Self {
        Self {
            provided_names: IndexMap::new(),
            exported_variables: IndexSet::new(),
            preserve_goog_provides_and_requires,
            requires_to_be_removed: Vec::new(),
            has_rewriting_occurred: false,
            forward_declares_to_remove: IndexSet::new(),
            ast_factory: compiler.create_ast_factory(),
        }
    }

    // port: ProcessClosureProvidesAndRequires#getExportedVariableNames
    pub fn get_exported_variable_names(&self) -> &IndexSet<JsString> {
        &self.exported_variables
    }

    /// Collects all `goog.provide`s in the given namespace and warns on invalid code
    // port: ProcessClosureProvidesAndRequires#collectProvidedNames
    pub fn collect_provided_names(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) -> &IndexMap<JsString, ProvidedName> {
        if self.provided_names.is_empty() {
            // goog is special-cased because it is provided in Closure's base library.
            let goog = ProvidedNameBuilder::new()
                .set_namespace(GOOG.into())
                .set_node(None)
                .set_chunk(None)
                .build(compiler);
            self.provided_names.insert(GOOG.into(), goog);
            NodeTraversal::traverse_roots(
                compiler,
                &mut CollectDefinitions { pass: self },
                externs,
                root,
            );
        }
        &self.provided_names
    }

    /// Rewrites all provides and requires in the given namespace.
    ///
    /// Call this instead of `collectProvidedNames(Node, Node)` if you want rewriting.
    // port: ProcessClosureProvidesAndRequires#rewriteProvidesAndRequires
    pub fn rewrite_provides_and_requires(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        check_state!(
            !self.has_rewriting_occurred,
            "Cannot call rewriteProvidesAndRequires twice per instance"
        );
        self.has_rewriting_occurred = true;

        self.collect_provided_names(compiler, externs, root);

        // Java mutates each ProvidedName in place while replace() reads the replacement node of
        // its parent namespace (an earlier entry) from the same map.
        for i in 0..self.provided_names.len() {
            let mut pn = self.provided_names[i].clone();
            pn.replace(
                compiler,
                &self.ast_factory,
                self.preserve_goog_provides_and_requires,
                &self.provided_names,
            );
            self.provided_names[i] = pn;
        }

        for &closure_require in &self.requires_to_be_removed {
            compiler.report_change_to_enclosing_scope(closure_require);
            closure_require.detach(compiler);
        }
        for &forward_declare in &self.forward_declares_to_remove {
            NodeUtil::delete_node(compiler, forward_declare);
        }
    }

    // port: ProcessClosureProvidesAndRequires#visitGoogMethodCall
    fn visit_goog_method_call(
        &mut self,
        t: &mut NodeTraversal<'_>,
        parent: NodeId,
        n: NodeId,
        method_name: &str,
    ) {
        // For the sake of simplicity, we report code changes
        // when we see a provides/requires, and don't worry about
        // reporting the change when we actually do the replacement.
        match method_name {
            "exportSymbol" => {
                // Note: exportSymbol is allowed in local scope
                let arg = n.get_second_child(t).unwrap();
                if arg.is_string_lit(t) {
                    let arg_string = arg.get_string(t);
                    let dot = arg_string.index_of_char(u16::from(b'.'));
                    if dot == -1 {
                        self.exported_variables.insert(arg_string);
                    } else {
                        self.exported_variables
                            .insert(arg_string.substring(0, dot as usize));
                    }
                }
            }
            "require" | "requireType" => {
                if self.is_valid_primitive_call(t, n) {
                    self.process_require_call(t.get_compiler(), n, parent);
                }
            }
            "provide" => {
                if self.is_valid_primitive_call(t, n) {
                    self.process_provide_call(t, n, parent);
                }
            }
            "forwardDeclare" => {
                if self.is_valid_primitive_call(t, n) {
                    self.process_forward_declare(t.get_compiler(), n, parent);
                }
            }
            _ => {}
        }
    }

    // port: ProcessClosureProvidesAndRequires#isValidPrimitiveCall
    fn is_valid_primitive_call(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        // Ignore invalid primitives if we didn't strip module sugar.
        if t.get_compiler().get_options().should_preserve_goog_module() {
            return true;
        }

        t.in_global_hoist_scope() && n.get_parent(t).unwrap().is_expr_result(t)
    }

    /// Handles a goog.require or goog.requireType call.
    // port: ProcessClosureProvidesAndRequires#processRequireCall
    fn process_require_call(&mut self, compiler: &AbstractCompiler, call: NodeId, parent: NodeId) {
        if !Self::verify_only_argument_is_string(compiler, call) {
            return;
        }

        if !self.preserve_goog_provides_and_requires {
            self.requires_to_be_removed.push(parent);
        }
    }

    /// Handles a goog.module that is a legacy namespace.
    // port: ProcessClosureProvidesAndRequires#processLegacyModuleCall
    fn process_legacy_module_call(
        &mut self,
        compiler: &AbstractCompiler,
        namespace: JsString,
        goog_module_call: NodeId,
        chunk: Option<JSChunk>,
    ) {
        self.register_any_provided_prefixes(compiler, &namespace, goog_module_call, chunk.clone());
        let provided_name = ProvidedNameBuilder::new()
            .set_namespace(namespace.clone())
            .set_node(Some(goog_module_call))
            .set_chunk(chunk)
            .set_explicit(true)
            .set_from_legacy_module(true)
            .build(compiler);
        self.provided_names.insert(namespace, provided_name);
    }

    /// Handles a goog.provide call.
    // port: ProcessClosureProvidesAndRequires#processProvideCall
    fn process_provide_call(&mut self, t: &mut NodeTraversal<'_>, call: NodeId, parent: NodeId) {
        check_state!(call.is_call(t));
        if !Self::verify_only_argument_is_string(t, call) {
            return;
        }
        let left = call.get_first_child(t).unwrap();
        let arg = left.get_next(t).unwrap();
        let ns = arg.get_string(t);

        let info = NodeUtil::get_best_jsdoc_info(t, call);
        let is_implicitly_initialized = info.is_some_and(|info| info.is_provide_already_provided());

        if self.provided_names.contains_key(&ns) {
            let chunk = t.get_chunk();
            let compiler = t.get_compiler();
            let previously_provided = self.provided_names.get_mut(&ns).unwrap();
            if !previously_provided.is_explicitly_provided() {
                previously_provided.add_provide(
                    compiler,
                    Some(parent),
                    chunk,
                    /* explicit= */ true,
                );
            }
        } else {
            let chunk = t.get_chunk();
            let compiler = t.get_compiler();
            self.register_any_provided_prefixes(compiler, &ns, parent, chunk.clone());
            let provided_name = ProvidedNameBuilder::new()
                .set_namespace(ns.clone())
                .set_node(Some(parent))
                .set_chunk(chunk)
                .set_explicit(true)
                .set_has_implicit_initialization(is_implicitly_initialized)
                .build(compiler);
            self.provided_names.insert(ns, provided_name);
        }
    }

    /// Handles a stub definition for a goog.provided name (e.g. a @typedef or a definition from
    /// externs)
    ///
    /// @param exprResult EXPR_RESULT node.
    // port: ProcessClosureProvidesAndRequires#handleStubDefinition
    fn handle_stub_definition(&mut self, t: &mut NodeTraversal<'_>, expr_result: NodeId) {
        if !t.in_global_hoist_scope() {
            return;
        }
        let is_extern_stub = expr_result.is_from_externs(t);
        let is_typedef_stub = Self::is_typedef_stub_declaration(t, expr_result);
        // recognize @typedefs only if using this pass for typechecking via
        // collectProvidedNames. We don't want rewriting to depend on @typedef annotations.
        let is_valid_typedef_stub_definition = is_typedef_stub && !self.has_rewriting_occurred;

        if is_valid_typedef_stub_definition || is_extern_stub {
            if expr_result.get_first_child(t).unwrap().is_qualified_name(t) {
                let name = expr_result
                    .get_first_child(t)
                    .unwrap()
                    .get_qualified_name(t)
                    .unwrap();
                if self.provided_names.contains_key(&name) {
                    let chunk = t.get_chunk();
                    let compiler = t.get_compiler();
                    let pn = self.provided_names.get_mut(&name).unwrap();
                    pn.add_definition(compiler, expr_result, chunk);
                }
            }
        }
        if is_typedef_stub {
            self.check_nested_typedef_provide(t.get_compiler(), expr_result);
        }
    }

    /// Checks that code doesn't use goog.provides in a way that will cause broken output code.
    ///
    /// @param exprResult an EXPR_RESULT node with an @typedef type
    // port: ProcessClosureProvidesAndRequires#checkNestedTypedefProvide
    fn check_nested_typedef_provide(
        &mut self,
        compiler: &mut AbstractCompiler,
        expr_result: NodeId,
    ) {
        // forbid this pattern:
        //   goog.provide('my.parent');
        //   goog.provide('my.parent.ChildTypedef');
        //   my.parent = [...]; // some initialization, doesn't matter what
        //   /** @typedef {...} */
        //   my.parent.ChildType;
        // at one point, this pattern was supported. now it would produce code that crashes at
        // runtime because the compiler would initializer `my.parent.ChildType = {}` before
        // `my.parent = [...]`, so report an error.

        let name = expr_result
            .get_first_child(compiler)
            .unwrap()
            .get_qualified_name(compiler);
        let Some(name) = name.filter(|name| name.index_of(".") >= 0) else {
            // @typedefs on simple names are okay.
            return;
        };
        if !self.provided_names.contains_key(&name) {
            // non-provided names don't matter.
            return;
        }
        let parent_name = name.substring(0, name.last_index_of(".") as usize);
        let parent = self.provided_names.get(&parent_name).unwrap();
        let parent_definition = parent.get_candidate_definition();
        let Some(parent_definition) = parent_definition else {
            return;
        };
        let parent_source = parent_definition.get_static_source_file(compiler).unwrap();
        let same_source = expr_result
            .get_static_source_file(compiler)
            .is_some_and(|s| std::ptr::addr_eq(Arc::as_ptr(&parent_source), Arc::as_ptr(&s)));
        if !same_source || Self::is_typedef_stub_declaration(compiler, parent_definition) {
            return;
        }
        compiler.report(JSError::make(
            compiler,
            expr_result,
            &TYPEDEF_CHILD_OF_PROVIDE,
            &[&name.to_string(), &parent_name.to_string()],
        ));
    }

    // port: ProcessClosureProvidesAndRequires#isTypedefStubDeclaration
    fn is_typedef_stub_declaration(ast: &Ast, statement: NodeId) -> bool {
        if !statement.is_expr_result(ast) {
            return false;
        }
        let info = NodeUtil::get_best_jsdoc_info(ast, statement);
        info.is_some_and(|info| info.has_typedef_type())
    }

    /// Handles a candidate definition for a goog.provided name.
    // port: ProcessClosureProvidesAndRequires#handleCandidateProvideDefinition
    fn handle_candidate_provide_definition(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: NodeId,
    ) {
        if !t.in_global_hoist_scope() {
            return;
        }
        let name = match n.get_parent(t).unwrap().get_token(t) {
            Token::LET | Token::CONST => {
                if t.in_global_scope() {
                    Some(n.get_string(t))
                } else {
                    None
                }
            }
            Token::VAR => Some(n.get_string(t)),
            Token::EXPR_RESULT => {
                if n.is_assign(t) {
                    n.get_first_child(t).unwrap().get_qualified_name(t)
                } else {
                    None
                }
            }
            // Class and function provides are forbidden; see ProcessClosurePrimitives's
            // CLASS_NAMESPACE_ERROR and FUNCTION_NAMESPACE_ERROR.
            Token::CLASS | Token::FUNCTION => None,
            _ => None,
        };

        let Some(name) = name else {
            return;
        };

        if self.provided_names.contains_key(&name) {
            let chunk = t.get_chunk();
            let compiler = t.get_compiler();
            let pn = self.provided_names.get_mut(&name).unwrap();
            pn.add_definition(compiler, parent, chunk);
        }
    }

    /// Marks a goog.forwardDeclare call for removal.
    // port: ProcessClosureProvidesAndRequires#processForwardDeclare
    fn process_forward_declare(&mut self, compiler: &AbstractCompiler, n: NodeId, parent: NodeId) {
        let convention = compiler.get_coding_convention();

        // A Java null list is an empty Vec here.
        let type_declarations = convention.identify_type_declaration_call(compiler, n);

        if !self.preserve_goog_provides_and_requires && type_declarations.len() == 1 {
            // Forward declaration was recorded and we can remove the call.
            let to_remove = if parent.is_expr_result(compiler) {
                parent
            } else {
                parent.get_parent(compiler).unwrap()
            };
            self.forward_declares_to_remove.insert(to_remove);
        }
    }

    /// Verifies that a method call has exactly one argument, and that it's a string literal.
    ///
    /// @return Whether the argument checked out okay
    // port: ProcessClosureProvidesAndRequires#verifyOnlyArgumentIsString
    fn verify_only_argument_is_string(ast: &Ast, call: NodeId) -> bool {
        let arg = call.get_second_child(ast);
        arg.is_some_and(|arg| arg.is_string_lit(ast) && arg.get_next(ast).is_none())
    }

    /// Registers ProvidedNames for prefix namespaces if they haven't already been defined. The
    /// prefix namespaces must be registered in order from shortest to longest.
    ///
    /// @param ns The namespace whose prefixes may need to be provided.
    /// @param node The EXPR of the provide call.
    /// @param chunk The current chunk.
    // port: ProcessClosureProvidesAndRequires#registerAnyProvidedPrefixes
    fn register_any_provided_prefixes(
        &mut self,
        compiler: &AbstractCompiler,
        ns: &JsString,
        node: NodeId,
        chunk: Option<JSChunk>,
    ) {
        let dot = u16::from(b'.');
        let mut pos = ns.index_of_char(dot);
        while pos != -1 {
            let prefix_ns = ns.substring(0, pos as usize);
            pos = index_of_char_from(ns, dot, pos + 1);
            if self.provided_names.contains_key(&prefix_ns) {
                self.provided_names
                    .get_mut(&prefix_ns)
                    .unwrap()
                    .add_provide(
                        compiler,
                        Some(node),
                        chunk.clone(),
                        /* explicit= */ false,
                    );
            } else {
                let provided_name = ProvidedNameBuilder::new()
                    .set_namespace(prefix_ns.clone())
                    .set_node(Some(node))
                    .set_chunk(chunk.clone())
                    .set_explicit(false)
                    .build(compiler);
                self.provided_names.insert(prefix_ns, provided_name);
            }
        }
    }
}

/// Java's `String#indexOf(int, int)`.
fn index_of_char_from(s: &JsString, c: u16, from: i32) -> i32 {
    let units = s.as_units();
    let start = from.max(0) as usize;
    if start >= units.len() {
        return -1;
    }
    units[start..]
        .iter()
        .position(|&u| u == c)
        .map_or(-1, |p| (start + p) as i32)
}

impl CompilerPass for ProcessClosureProvidesAndRequires {
    /// When invoked as compiler pass, we rewrite all provides and requires.
    // port: ProcessClosureProvidesAndRequires#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.rewrite_provides_and_requires(compiler, externs, root);
    }
}

// port: ProcessClosureProvidesAndRequires.CollectDefinitions
struct CollectDefinitions<'a> {
    pass: &'a mut ProcessClosureProvidesAndRequires,
}

impl Callback for CollectDefinitions<'_> {
    // port: ProcessClosureProvidesAndRequires.CollectDefinitions#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // Don't recurse into modules, which cannot have goog.provides.  We do need to handle
        // legacy goog.modules, but we do that quickly here rather than descending all the way into
        // them. We completely ignore ES modules and CommonJS modules.
        if (n.is_module_body(t)
            && n.get_parent(t)
                .unwrap()
                .get_boolean_prop(t, NodeId::GOOG_MODULE))
            || NodeUtil::is_bundled_goog_module_scope_root(t, n)
        {
            let goog_module_call = n.get_first_child(t).unwrap();
            let closure_namespace = goog_module_call
                .get_first_child(t)
                .unwrap()
                .get_second_child(t)
                .unwrap()
                .get_string(t);
            let maybe_legacy_namespace_call = goog_module_call.get_next(t);
            if let Some(maybe_legacy_namespace_call) = maybe_legacy_namespace_call
                && NodeUtil::is_goog_module_declare_legacy_namespace_call(
                    t,
                    maybe_legacy_namespace_call,
                )
            {
                let chunk = t.get_chunk();
                self.pass.process_legacy_module_call(
                    t.get_compiler(),
                    closure_namespace,
                    goog_module_call,
                    chunk,
                );
            }
            return false;
        }
        !n.is_module_body(t)
    }

    // port: ProcessClosureProvidesAndRequires.CollectDefinitions#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::CALL => {
                let left = n.get_first_child(t).unwrap();
                if left.is_get_prop(t) {
                    let name = left.get_first_child(t).unwrap();
                    if name.matches_name(t, GOOG) {
                        let method_name = left.get_string(t).to_string();
                        self.pass
                            .visit_goog_method_call(t, parent.unwrap(), n, &method_name);
                    }
                }
            }
            // If this is an assignment to a provided name, remove the provided object.
            Token::ASSIGN | Token::NAME => {
                self.pass
                    .handle_candidate_provide_definition(t, n, parent.unwrap())
            }
            Token::EXPR_RESULT => self.pass.handle_stub_definition(t, n),
            _ => {}
        }
    }
}

// port: ProcessClosureProvidesAndRequires.ProvidedNameBuilder
struct ProvidedNameBuilder {
    namespace: JsString,
    node: Option<NodeId>,
    chunk: Option<JSChunk>,
    explicit: bool,
    from_legacy_module: bool,
    has_implicit_initialization: bool,
}

impl ProvidedNameBuilder {
    fn new() -> Self {
        Self {
            namespace: JsString::from(""),
            node: None,
            chunk: None,
            explicit: false,
            from_legacy_module: false,
            has_implicit_initialization: false,
        }
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedNameBuilder#setNamespace
    fn set_namespace(mut self, namespace: JsString) -> Self {
        self.namespace = namespace;
        self
    }

    /// @param node Can be null (for GOOG or an implicit name), an EXPR_RESULT for a
    ///     goog.provide, or an EXPR_RESULT or name declaration for a previously provided name.
    // port: ProcessClosureProvidesAndRequires.ProvidedNameBuilder#setNode
    fn set_node(mut self, node: Option<NodeId>) -> Self {
        self.node = node;
        self
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedNameBuilder#setChunk
    fn set_chunk(mut self, chunk: Option<JSChunk>) -> Self {
        self.chunk = chunk;
        self
    }

    /// @param explicit Whether this came from an actual goog.provide('a.b.c'); call
    // port: ProcessClosureProvidesAndRequires.ProvidedNameBuilder#setExplicit
    fn set_explicit(mut self, explicit: bool) -> Self {
        self.explicit = explicit;
        self
    }

    /// @param alreadyInitialized Whether this came from an actual goog.provide('a.b.c'); call
    // port: ProcessClosureProvidesAndRequires.ProvidedNameBuilder#setHasImplicitInitialization
    fn set_has_implicit_initialization(mut self, already_initialized: bool) -> Self {
        self.has_implicit_initialization = already_initialized;
        self
    }

    /// Whether this comes from a legacy goog.module
    // port: ProcessClosureProvidesAndRequires.ProvidedNameBuilder#setFromLegacyModule
    fn set_from_legacy_module(mut self, from_legacy_module: bool) -> Self {
        self.from_legacy_module = from_legacy_module;
        self
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedNameBuilder#build
    fn build(self, compiler: &AbstractCompiler) -> ProvidedName {
        ProvidedName::new(self, compiler)
    }
}

// -------------------------------------------------------------------------

/// Stores information about a Closure namespace created by a goog.provide
///
/// There are three ways that we find these namespaces in the AST:
///
/// - An explicit goog.provide. `goog.provide('a.b.c');` creates a ProvidedName for 'a.b.c'.
/// - An implicit parent namespace. `goog.provide('a.b.c');` creates a ProvidedName for 'a.b'.
/// - A provide definition processed by an earlier run. `a.b.c = {};` when annotated
///   IS_NAMESPACE
///
/// Java's `compiler` and `astFactory` fields are passed to the methods that use them.
#[derive(Clone)]
pub struct ProvidedName {
    // The Closure namespace this name represents, e.g. `a.b` for `goog.provide('a.b');`
    namespace: JsString,

    // The first node in the AST that creates this ProvidedName.
    // This is always a goog.provide('a.b'), null (for implicit namespaces and 'goog'), or an
    // assignment or declaration for a 'previously provided' name or parent namespace of such.
    // This should only be used for source info and a place to hang namespace definitions.
    first_node: Option<NodeId>,
    // The chunk where this namespace was first goog.provided, if chunks exist. */
    first_chunk: Option<JSChunk>,

    // When set the namespace will not have an initialization added, even if the namespace
    // initialization is not otherwise visibile to this pass.  This allows for trivial aliasing
    // of a tree of namespaces.
    has_implicit_initialization: bool,

    // The node where the call was explicitly goog.provided. Null if the namespace is implicit.
    // If this is previously provided, this will instead be the expression or declaration marked
    // as IS_NAMESPACE.
    explicit_node: Option<NodeId>,

    // The candidate definition for this namespace. For example, given
    //      goog.provide('a.b');
    //      /** @constructor * /
    //      a.b = function() {};
    // the 'candidate definition' of 'a.b' is the GETPROP 'a.b' from the constructor declaration.
    candidate_definition: Option<NodeId>,

    // The minimum chunk where the provide namespace definition must appear. If child namespaces
    // of this provide appear in multiple chunks, this chunk must be earlier than all child
    // namespace's chunks.
    minimum_chunk: Option<JSChunk>,

    // The replacement declaration. Null until replace() has been called.
    replacement_node: Option<NodeId>,

    // Whether this comes from a goog.module with declareLegacyNamespace.
    from_legacy_module: bool,
}

impl ProvidedName {
    // port: ProcessClosureProvidesAndRequires.ProvidedName#ProvidedName
    fn new(builder: ProvidedNameBuilder, compiler: &AbstractCompiler) -> Self {
        let node = builder.node;
        check_argument!(
            node.is_none_or(|node| NodeUtil::is_expr_call(compiler, node)
                || (!builder.explicit
                    && (NodeUtil::is_expr_assign(compiler, node)
                        || NodeUtil::is_name_declaration(compiler, Some(node))
                        || (node.is_expr_result(compiler)
                            && node
                                .get_first_child(compiler)
                                .unwrap()
                                .is_qualified_name(compiler))))),
            "%s",
            node_value_of(compiler, node)
        );
        let mut provided_name = Self {
            namespace: builder.namespace,
            first_node: builder.node,
            first_chunk: builder.chunk.clone(),
            has_implicit_initialization: builder.has_implicit_initialization,
            explicit_node: None,
            candidate_definition: None,
            minimum_chunk: None,
            replacement_node: None,
            from_legacy_module: builder.from_legacy_module,
        };

        provided_name.add_provide(compiler, node, builder.chunk, builder.explicit);
        provided_name
    }

    /// Adds an implicit or explicit provide.
    ///
    /// Every provided name can have multiple implicit provides but a maximum of one explicit
    /// provide.
    ///
    /// @param node the EXPR_RESULT representing this provide or possible a VAR for a previously
    ///     provided name. null if implicit.
    // port: ProcessClosureProvidesAndRequires.ProvidedName#addProvide
    fn add_provide(
        &mut self,
        compiler: &AbstractCompiler,
        node: Option<NodeId>,
        chunk: Option<JSChunk>,
        explicit: bool,
    ) {
        if explicit {
            // goog.provide('name.space');
            check_state!(self.explicit_node.is_none());
            let node = node.unwrap();
            check_argument!(
                node.is_expr_result(compiler),
                "%s",
                node.to_string(compiler)
            );
            self.explicit_node = Some(node);
        }
        self.update_minimum_chunk(compiler, chunk);
    }

    /// Whether there existed a `goog.provide('a.b');` for this name 'a.b'
    // port: ProcessClosureProvidesAndRequires.ProvidedName#isExplicitlyProvided
    pub fn is_explicitly_provided(&self) -> bool {
        self.explicit_node.is_some()
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedName#hasImplicitInitialization
    pub fn has_implicit_initialization(&self) -> bool {
        self.has_implicit_initialization
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedName#isFromLegacyModule
    pub fn is_from_legacy_module(&self) -> bool {
        self.from_legacy_module
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedName#hasCandidateDefinition
    fn has_candidate_definition(&self) -> bool {
        self.candidate_definition.is_some()
    }

    /// Returns the `goog.provide` or legacy namespace `goog.module` call that created this name,
    /// if any, or otherwise the first 'previous provide' assignment that created this name.
    // port: ProcessClosureProvidesAndRequires.ProvidedName#getFirstProvideCall
    pub fn get_first_provide_call(&self) -> Option<NodeId> {
        self.first_node
    }

    /// Returns the definition of this provided namespace in the input code, if any, or null.
    ///
    /// For example, this returns `a.b = class {};` given 'a.b' in
    ///
    /// ```text
    ///   goog.provide('a.b');
    ///   a.b = class {};
    /// ```
    ///
    /// Note: this method will only return candidate definitions that count towards provide
    /// rewriting. If a name is defined, then provided, the candidate definition will not be the
    /// early definition. This doesn't completely mimic uncompiled behavior, but supports some
    /// legacy code. Externs definitions never count.
    // port: ProcessClosureProvidesAndRequires.ProvidedName#getCandidateDefinition
    pub fn get_candidate_definition(&self) -> Option<NodeId> {
        self.candidate_definition
    }

    /// Returns the Closure namespace of this provide, e.g. "a.b" for `goog.provide('a.b');`
    // port: ProcessClosureProvidesAndRequires.ProvidedName#getNamespace
    pub fn get_namespace(&self) -> &JsString {
        &self.namespace
    }

    /// Records function declaration, variable declarations, and assignments that refer to this
    /// provided namespace.
    ///
    /// This pass gives preference to declarations. If no declaration exists, records a reference
    /// to an assignment so it can be repurposed later into a declaration.
    // port: ProcessClosureProvidesAndRequires.ProvidedName#addDefinition
    fn add_definition(
        &mut self,
        compiler: &AbstractCompiler,
        node: NodeId,
        chunk: Option<JSChunk>,
    ) {
        check_argument!(
            node.is_expr_result(compiler) // assign
                || node.is_function(compiler)
                || NodeUtil::is_name_declaration(compiler, Some(node))
        );
        check_argument!(self.explicit_node != Some(node));
        if self.candidate_definition.is_none() || !node.is_expr_result(compiler) {
            self.candidate_definition = Some(node);
            self.update_minimum_chunk(compiler, chunk);
        }
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedName#updateMinimumChunk
    fn update_minimum_chunk(&mut self, compiler: &AbstractCompiler, new_chunk: Option<JSChunk>) {
        let chunk_graph = compiler.get_chunk_graph();
        if self.minimum_chunk.is_none() {
            self.minimum_chunk = new_chunk;
        } else if chunk_graph.unwrap().get_chunk_count() > 1 {
            self.minimum_chunk = chunk_graph
                .unwrap()
                .get_deepest_common_dependency_inclusive(
                    self.minimum_chunk.as_ref().unwrap(),
                    new_chunk.as_ref().unwrap(),
                );
        } else {
            // If there is no chunk graph, then there must be exactly one chunk in the program.
            check_state!(new_chunk == self.minimum_chunk, "Missing chunk graph");
        }
    }

    /// Replace the provide statement.
    ///
    /// If we're providing a name with no definition, then create one. If we're providing a name
    /// with a duplicate definition, then make sure that definition becomes a declaration.
    // port: ProcessClosureProvidesAndRequires.ProvidedName#replace
    fn replace(
        &mut self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
        preserve_goog_provides_and_requires: bool,
        provided_names: &IndexMap<JsString, ProvidedName>,
    ) {
        check_state!(
            !self.is_from_legacy_module(),
            "Cannot rewrite provides without having rewritten goog.modules, found %s",
            node_value_of(compiler, self.first_node)
        );
        if self.first_node.is_none() {
            // Don't touch the base case ('goog').
            self.replacement_node = self.candidate_definition;
            return;
        }

        // Handle the case where there is a duplicate definition for an explicitly
        // provided symbol.
        if self.has_candidate_definition() && self.explicit_node.is_some() {
            let candidate_definition = self.candidate_definition.unwrap();
            // Does this need a VAR keyword?
            self.replacement_node = Some(candidate_definition);
            if candidate_definition.is_expr_result(compiler) {
                let expr_node = candidate_definition.get_only_child(compiler);
                if expr_node.is_assign(compiler) {
                    let name_node = expr_node.get_first_child(compiler).unwrap();
                    if name_node.is_name(compiler) {
                        // In the case of a simple name, `name = value;`, we need to ensure the
                        // name is actually declared with `var`.
                        self.convert_provide_assignment_to_var_declaration(
                            compiler, expr_node, name_node,
                        );
                    } else {
                        // `some.provided.namespace = value;`
                        // We don't need to change the definition, but mark it as 'IS_NAMESPACE'
                        // so that future passes know this was originally provided.
                        candidate_definition.put_boolean_prop(compiler, NodeId::IS_NAMESPACE, true);
                    }
                }
            }
        } else {
            // Handle the case where there's not an existing definition.
            if !self.has_implicit_initialization {
                let value = ast_factory.create_object_lit(compiler, &[]);
                let replacement = self.create_declaration_node(compiler, ast_factory, value);
                self.create_namespace_initialization(compiler, replacement, provided_names);
            }
        }

        // Remove the `goog.provide('a.b.c');` call.
        if let Some(explicit_node) = self.explicit_node {
            if preserve_goog_provides_and_requires {
                return;
            }
            compiler.report_change_to_enclosing_scope(explicit_node);
            explicit_node.detach(compiler);
        }
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedName#convertProvideAssignmentToVarDeclaration
    fn convert_provide_assignment_to_var_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        assign_node: NodeId,
        name_node: NodeId,
    ) {
        // Convert `providedName = value;` into `var providedName = value;`.
        check_argument!(
            assign_node.is_assign(compiler),
            "%s",
            assign_node.to_string(compiler)
        );
        check_argument!(
            name_node.is_name(compiler),
            "%s",
            name_node.to_string(compiler)
        );
        let value_node = name_node.get_next(compiler).unwrap();
        name_node.detach(compiler);
        value_node.detach(compiler);

        let candidate_definition = self.candidate_definition.unwrap();
        let var_node = IR::var_with_value(compiler, name_node, value_node)
            .srcref(compiler, candidate_definition);
        let info = assign_node.get_jsdoc_info(compiler);
        var_node.set_jsdoc_info(compiler, info);
        var_node.put_boolean_prop(compiler, NodeId::IS_NAMESPACE, true);

        candidate_definition.replace_with(compiler, var_node);
        self.replacement_node = Some(var_node);
        compiler.report_change_to_enclosing_scope(var_node);
    }

    /// Adds an assignment or declaration to this namespace to the AST, using the provided value
    // port: ProcessClosureProvidesAndRequires.ProvidedName#createNamespaceInitialization
    fn create_namespace_initialization(
        &mut self,
        compiler: &mut AbstractCompiler,
        replacement: NodeId,
        provided_names: &IndexMap<JsString, ProvidedName>,
    ) {
        self.replacement_node = Some(replacement);
        if self.first_chunk == self.minimum_chunk {
            replacement.insert_before(compiler, self.first_node.unwrap());
        } else {
            // In this case, the name was implicitly provided by two independent
            // modules. We need to move this code up to a common module.
            let index_of_dot = self.namespace.last_index_of_char(u16::from(b'.'));
            if index_of_dot == -1 {
                // Any old place is fine.
                let minimum_chunk = self.minimum_chunk.clone();
                compiler
                    .get_node_for_code_insertion(minimum_chunk.as_ref())
                    .add_child_to_back(compiler, replacement);
            } else {
                // Add it after the parent namespace.
                let parent_name =
                    provided_names.get(&self.namespace.substring(0, index_of_dot as usize));
                let parent_name = check_not_null!(parent_name);
                let parent_replacement = check_not_null!(parent_name.replacement_node);
                replacement.insert_after(compiler, parent_replacement);
            }
        }
        compiler.report_change_to_enclosing_scope(replacement);
    }

    /// Create the declaration node for this name, without inserting it into the AST.
    ///
    /// @param value the object literal namespace, possibly in a CAST
    // port: ProcessClosureProvidesAndRequires.ProvidedName#createDeclarationNode
    fn create_declaration_node(
        &self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
        value: NodeId,
    ) -> NodeId {
        check_argument!(
            value.is_object_lit(compiler) || value.is_cast(compiler),
            "%s",
            value.to_string(compiler)
        );
        if self.namespace.index_of_char(u16::from(b'.')) == -1 {
            self.make_var_decl_node(compiler, value)
        } else {
            self.make_assignment_expr_node(compiler, ast_factory, value)
        }
    }

    /// Creates a simple namespace variable declaration (e.g. `var foo = {};`).
    // port: ProcessClosureProvidesAndRequires.ProvidedName#makeVarDeclNode
    fn make_var_decl_node(&self, compiler: &mut AbstractCompiler, value: NodeId) -> NodeId {
        let name = IR::name(compiler, self.namespace.clone());
        name.add_child_to_front(compiler, value);

        let decl = IR::var(compiler, name);
        decl.put_boolean_prop(compiler, NodeId::IS_NAMESPACE, true);

        if compiler
            .get_coding_convention()
            .is_constant(&self.namespace)
        {
            name.put_boolean_prop(compiler, NodeId::IS_CONSTANT_NAME, true);
        }
        if !self.has_candidate_definition() {
            decl.set_jsdoc_info(compiler, Some(NodeUtil::create_constant_js_doc()));
        }

        check_state!(is_namespace_placeholder(compiler, decl));
        self.set_source_info(compiler, decl);
        decl
    }

    /// Creates a dotted namespace assignment expression (e.g. `foo.bar = {};`).
    // port: ProcessClosureProvidesAndRequires.ProvidedName#makeAssignmentExprNode
    fn make_assignment_expr_node(
        &self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
        value: NodeId,
    ) -> NodeId {
        // Note: as of May 2021, using the unknown type vs. the actual inferred type both produced
        // the same optimized JS after type-based optimizations. So the lack of type info is
        // intentional.
        let lhs = ast_factory
            .create_qname_with_unknown_type(compiler, &self.namespace.to_string())
            .srcref_tree(compiler, self.first_node.unwrap());
        let assign = ast_factory.create_assign(compiler, lhs, value);
        let decl = IR::expr_result(compiler, assign);
        decl.put_boolean_prop(compiler, NodeId::IS_NAMESPACE, true);
        if !self.has_candidate_definition() {
            decl.get_first_child(compiler)
                .unwrap()
                .set_jsdoc_info(compiler, Some(NodeUtil::create_constant_js_doc()));
        }
        check_state!(is_namespace_placeholder(compiler, decl));
        self.set_source_info(compiler, decl);
        // This function introduces artifical nodes and we don't need them for indexing.
        // Marking all but the last one as non-indexable. So if this function adds:
        // foo.bar.baz = {};
        // then we mark foo and bar as non-indexable.
        lhs.get_first_child(compiler)
            .unwrap()
            .make_non_indexable_recursive(compiler);
        decl
    }

    /// Copy source info to the new node.
    // port: ProcessClosureProvidesAndRequires.ProvidedName#setSourceInfo
    fn set_source_info(&self, compiler: &mut AbstractCompiler, new_node: NodeId) {
        let mut source_info_node = self.first_node.unwrap();

        let provide_string_node = self.get_provide_string_node(compiler);
        if let Some(provide_string_node) = provide_string_node {
            // Given namespace "foo.bar.baz" we create node for "baz" here and need to calculate
            // length and start of the last component which is "baz".
            let first_char_index = self.namespace.last_index_of_char(u16::from(b'.')) + 1; // If no dots, then 0.

            source_info_node = provide_string_node.clone_node(compiler);
            let lineno = source_info_node.get_lineno(compiler);
            let charno = source_info_node.get_charno(compiler);
            source_info_node.set_lineno_charno(
                compiler,
                lineno,
                charno + first_char_index + 1, // +1 for quote
            );
            source_info_node
                .set_length(compiler, self.namespace.length() as i32 - first_char_index);
        }

        new_node.srcref_tree(compiler, source_info_node);
    }

    // port: ProcessClosureProvidesAndRequires.ProvidedName#getProvideStringNode
    fn get_provide_string_node(&self, compiler: &AbstractCompiler) -> Option<NodeId> {
        let first_node = self.first_node.unwrap();
        if first_node.has_children(compiler) && NodeUtil::is_expr_call(compiler, first_node) {
            first_node
                .get_first_child(compiler)
                .unwrap()
                .get_last_child(compiler)
        } else {
            None
        }
    }
}

impl std::fmt::Display for ProvidedName {
    // port: ProcessClosureProvidesAndRequires.ProvidedName#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let explicit_or_implicit = if self.is_explicitly_provided() {
            "explicit"
        } else {
            "implicit"
        };
        write!(
            f,
            "ProvidedName: {}, {}",
            self.namespace, explicit_or_implicit
        )
    }
}

/// Returns whether the node initializes a goog.provide'd namespace (e.g. `a.b = {};`) with a
/// simple namespace object literal (e.g. not `a.b = class {}`;)
// port: ProcessClosureProvidesAndRequires#isNamespacePlaceholder
fn is_namespace_placeholder(compiler: &AbstractCompiler, n: NodeId) -> bool {
    if !n.get_boolean_prop(compiler, NodeId::IS_NAMESPACE) {
        return false;
    }

    let mut value = None;
    if n.is_expr_result(compiler) {
        let assign = n.get_first_child(compiler).unwrap();
        value = assign.get_last_child(compiler);
    } else if n.is_var(compiler) {
        let name = n.get_first_child(compiler).unwrap();
        value = name.get_first_child(compiler);
    }

    let Some(mut value) = value else {
        return false;
    };
    if value.is_cast(compiler) {
        // There may be a cast to unknown type wrapped around the value.
        value = value.get_only_child(compiler);
    }
    value.is_object_lit(compiler) && !value.has_children(compiler)
}
