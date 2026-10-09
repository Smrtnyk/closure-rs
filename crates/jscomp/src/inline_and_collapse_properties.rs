/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/InlineAndCollapseProperties.java.

//! Port of `InlineAndCollapseProperties.java`.
//!
//! Java's inner (non-static) classes read the outer instance's fields. Here the outer fields they
//! read are passed explicitly: the `GlobalNamespace` shared by alias inlining and property
//! collapsing lives in `InlineAndCollapseProperties::namespace`, and the inner passes receive it
//! as `ns` (DESIGN §6: the compiler is never stored).

use crate::{
    abstract_compiler::AbstractCompiler,
    ast_factory::{AstFactory, AstFactoryStaticScope},
    closure_rewrite_module::MODULE_EXPORTS_PREFIX,
    compiler::Compiler,
    compiler_options::{ChunkOutputType, PropertyCollapseLevel},
    compiler_pass::CompilerPass,
    convert_chunks_to_es_modules::ConvertChunksToESModules,
    deps::module_loader::ResolutionMode,
    destructuring_global_name_extractor::DestructuringGlobalNameExtractor,
    diagnostic_type::DiagnosticType,
    global_namespace::{
        AstChange, DecisionsLog, GlobalNamespace, Inlinability, Name, Ref, RefBasedAstChange,
        SimpleAstChange,
    },
    js_chunk::JSChunk,
    js_error::JSError,
    node_traversal::{
        AbstractPostOrderCallbackInterface, Callback, ExternsSkippingCallback, NodeTraversal,
    },
    node_util::NodeUtil,
    process_common_js_modules::ProcessCommonJSModules,
    reference_collection::ReferenceCollection,
    reference_collector::ReferenceCollector,
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{NodeId, Prop},
    token::Token,
    token_stream::TokenStream,
};
use indexmap::{IndexMap, IndexSet};
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::{Arc, Mutex},
};

// Warnings
// port: InlineAndCollapseProperties#PARTIAL_NAMESPACE_WARNING
pub static PARTIAL_NAMESPACE_WARNING: DiagnosticType = DiagnosticType::warning(
    "JSC_PARTIAL_NAMESPACE",
    "Partial alias created for namespace {0}, possibly due to await/yield transpilation.\nThis may prevent optimization of anything nested under this namespace.\nSee https://github.com/google/closure-compiler/wiki/FAQ#i-got-a-partial-alias-created-for-namespace-error--what-do-i-do for more details.",
);

// port: InlineAndCollapseProperties#NAMESPACE_REDEFINED_WARNING
pub static NAMESPACE_REDEFINED_WARNING: DiagnosticType = DiagnosticType::warning(
    "JSC_NAMESPACE_REDEFINED",
    "namespace {0} should not be redefined",
);

// port: InlineAndCollapseProperties#RECEIVER_AFFECTED_BY_COLLAPSE
pub static RECEIVER_AFFECTED_BY_COLLAPSE: DiagnosticType = DiagnosticType::warning(
    "JSC_RECEIVER_AFFECTED_BY_COLLAPSE",
    "Receiver reference in function {0} changes meaning when namespace is collapsed.\n Consider annotating @nocollapse; however, other properties on the receiver may still be collapsed.",
);

// port: InlineAndCollapseProperties#UNSAFE_CTOR_ALIASING
pub static UNSAFE_CTOR_ALIASING: DiagnosticType = DiagnosticType::warning(
    "JSC_UNSAFE_CTOR_ALIASING",
    "Variable {0} aliases a constructor, so it cannot be assigned multiple times",
);

// port: InlineAndCollapseProperties#ALIAS_CYCLE
pub static ALIAS_CYCLE: DiagnosticType =
    DiagnosticType::error("JSC_ALIAS_CYCLE", "Alias path contains a cycle: {0} to {1}");

/// Java's `Consumer<GlobalNamespace>` supplied by `AggressiveInlineAliasesTest`.
pub type GlobalNamespaceTester = Box<dyn FnMut(&mut AbstractCompiler, &mut GlobalNamespace)>;

/// Performs alias inlining and property collapsing (see the inner passes).
pub struct InlineAndCollapseProperties {
    property_collapse_level: Option<PropertyCollapseLevel>,
    chunk_output_type: Option<ChunkOutputType>,
    have_modules_been_rewritten: bool,
    module_resolution_mode: Option<ResolutionMode>,

    /// Used by `AggressiveInlineAliasesTest` to enable execution of the aggressive inlining logic
    /// without doing any collapsing.
    test_aggressive_inlining_only: bool,

    /// Supplied by `AggressiveInlineAliasesTest`.
    ///
    /// The `GlobalNamespace` created by `AggressiveInlineAliases` will be passed to this
    /// `Consumer` for examination.
    optional_global_namespace_tester: Option<GlobalNamespaceTester>,

    /// Records decisions made by this class and related logic.
    ///
    /// This field is allocated and cleaned up by process(). It's a class field to avoid having to
    /// pass it as an extra argument through a lot of methods.
    decisions_log: Option<DecisionsLog>,

    /// A `GlobalNamespace` that is shared by alias inlining and property collapsing code.
    namespace: Option<GlobalNamespace>,
}

impl InlineAndCollapseProperties {
    // port: InlineAndCollapseProperties#InlineAndCollapseProperties
    fn new(builder: Builder) -> Self {
        Self {
            property_collapse_level: builder.property_collapse_level,
            chunk_output_type: builder.chunk_output_type,
            have_modules_been_rewritten: builder.have_modules_been_rewritten,
            module_resolution_mode: builder.module_resolution_mode,
            test_aggressive_inlining_only: builder.test_aggressive_inlining_only,
            optional_global_namespace_tester: builder.optional_global_namespace_tester,
            decisions_log: None,
            namespace: None,
        }
    }

    // port: InlineAndCollapseProperties#builder
    pub fn builder(_compiler: &AbstractCompiler) -> Builder {
        Builder::new()
    }

    // port: InlineAndCollapseProperties#performMinimalInliningAndNoCollapsing
    fn perform_minimal_inlining_and_no_collapsing(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        // TODO(b/124915436): Remove InlineAliases completely after cleaning up the codebase.
        InlineAliases::new(compiler).process(compiler, externs, root);
    }

    // port: InlineAndCollapseProperties#performMinimalInliningAndModuleExportCollapsing
    fn perform_minimal_inlining_and_module_export_collapsing(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        // TODO(b/124915436): Remove InlineAliases completely after cleaning up the codebase.
        InlineAliases::new(compiler).process(compiler, externs, root);
        // CollapseProperties needs this namespace.
        // TODO(bradfordcsmith): Have `InlineAliases` update the namespace it already created
        // and reuse that one instead.
        self.namespace = Some(GlobalNamespace::new_with_decisions_log(
            self.decisions_log.clone(),
            compiler,
            root,
        ));
        CollapseProperties::new().process(self, compiler, externs, root);
    }

    // port: InlineAndCollapseProperties#performAggressiveInliningAndCollapsing
    fn perform_aggressive_inlining_and_collapsing(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        crate::concretize_static_inheritance_for_inlining::ConcretizeStaticInheritanceForInlining::new(
            compiler,
        )
        .process(compiler, externs, root);
        AggressiveInlineAliases::new().process(self, compiler, externs, root);

        CollapseProperties::new().process(self, compiler, externs, root);
    }

    // port: InlineAndCollapseProperties#performAggressiveInliningForTest
    fn perform_aggressive_inlining_for_test(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        let mut aggressive_inline_aliases = AggressiveInlineAliases::new();
        aggressive_inline_aliases.process(self, compiler, externs, root);
        let mut tester = self.optional_global_namespace_tester.take().unwrap();
        tester(
            compiler,
            AggressiveInlineAliases::get_last_used_global_namespace(self),
        );
        self.optional_global_namespace_tester = Some(tester);
    }
}

impl CompilerPass for InlineAndCollapseProperties {
    // port: InlineAndCollapseProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let property_collapse_level = self.property_collapse_level;
        check_state!(
            !self.test_aggressive_inlining_only
                || property_collapse_level == Some(PropertyCollapseLevel::ALL),
            "testAggressiveInlining is invalid for: %s",
            property_collapse_level
                .map(|l| format!("{l:?}"))
                .unwrap_or_else(|| "null".to_owned())
        );
        let log_file: DecisionsLog = Arc::new(Mutex::new(compiler.create_or_reopen_indexed_log(
            "InlineAndCollapseProperties",
            "decisions.log",
            &[],
        )));
        // NOTE: decisionsLog will be a do-nothing proxy object unless the compiler
        // was given an option telling it to generate log files and where to put them.
        self.decisions_log = Some(log_file.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match check_not_null!(property_collapse_level) {
                PropertyCollapseLevel::NONE => {
                    self.perform_minimal_inlining_and_no_collapsing(compiler, externs, root)
                }
                PropertyCollapseLevel::MODULE_EXPORT => self
                    .perform_minimal_inlining_and_module_export_collapsing(compiler, externs, root),
                PropertyCollapseLevel::ALL => {
                    if self.test_aggressive_inlining_only {
                        self.perform_aggressive_inlining_for_test(compiler, externs, root);
                    } else {
                        self.perform_aggressive_inlining_and_collapsing(compiler, externs, root);
                    }
                }
            }
        }));
        // finally / try-with-resources
        self.decisions_log = None;
        log_file.lock().unwrap_or_else(|e| e.into_inner()).close();
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }
}

/// `InlineAndCollapseProperties.Builder`.
pub struct Builder {
    property_collapse_level: Option<PropertyCollapseLevel>,
    chunk_output_type: Option<ChunkOutputType>,
    have_modules_been_rewritten: bool,
    module_resolution_mode: Option<ResolutionMode>,
    test_aggressive_inlining_only: bool,
    optional_global_namespace_tester: Option<GlobalNamespaceTester>,
}

impl Builder {
    // port: InlineAndCollapseProperties.Builder#Builder
    fn new() -> Self {
        Self {
            property_collapse_level: None,
            chunk_output_type: None,
            have_modules_been_rewritten: false,
            module_resolution_mode: None,
            test_aggressive_inlining_only: false,
            optional_global_namespace_tester: None,
        }
    }

    // port: InlineAndCollapseProperties.Builder#setPropertyCollapseLevel
    pub fn set_property_collapse_level(
        mut self,
        property_collapse_level: PropertyCollapseLevel,
    ) -> Self {
        self.property_collapse_level = Some(property_collapse_level);
        self
    }

    // port: InlineAndCollapseProperties.Builder#setChunkOutputType
    pub fn set_chunk_output_type(mut self, chunk_output_type: ChunkOutputType) -> Self {
        self.chunk_output_type = Some(chunk_output_type);
        self
    }

    // port: InlineAndCollapseProperties.Builder#setHaveModulesBeenRewritten
    pub fn set_have_modules_been_rewritten(mut self, have_modules_been_rewritten: bool) -> Self {
        self.have_modules_been_rewritten = have_modules_been_rewritten;
        self
    }

    // port: InlineAndCollapseProperties.Builder#setModuleResolutionMode
    pub fn set_module_resolution_mode(mut self, module_resolution_mode: ResolutionMode) -> Self {
        self.module_resolution_mode = Some(module_resolution_mode);
        self
    }

    // port: InlineAndCollapseProperties.Builder#testAggressiveInliningOnly
    pub fn test_aggressive_inlining_only(
        mut self,
        global_namespace_tester: GlobalNamespaceTester,
    ) -> Self {
        self.test_aggressive_inlining_only = true;
        self.optional_global_namespace_tester = Some(global_namespace_tester);
        self
    }

    // port: InlineAndCollapseProperties.Builder#build
    pub fn build(self) -> InlineAndCollapseProperties {
        InlineAndCollapseProperties::new(self)
    }
}

/// Inlines type aliases if they are explicitly or effectively const. Also inlines inherited static
/// property accesses for ES6 classes.
///
/// This frees subsequent optimization passes from the responsibility of having to reason about
/// alias chains and is a requirement for correct behavior in at least CollapseProperties and
/// J2clPropertyInlinerPass.
///
/// This is designed to be no more unsafe than CollapseProperties. It will in some cases inline
/// properties, possibly past places that change the property value. However, it will only do so
/// in cases where CollapseProperties would unsafely collapse the property anyway.
struct AggressiveInlineAliases {
    code_changed: bool,
}

impl AggressiveInlineAliases {
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#AggressiveInlineAliases
    fn new() -> Self {
        Self { code_changed: true }
    }

    // port: InlineAndCollapseProperties.AggressiveInlineAliases#getLastUsedGlobalNamespace
    fn get_last_used_global_namespace(
        outer: &mut InlineAndCollapseProperties,
    ) -> &mut GlobalNamespace {
        outer.namespace.as_mut().unwrap()
    }

    // port: InlineAndCollapseProperties.AggressiveInlineAliases#process
    fn process(
        &mut self,
        outer: &mut InlineAndCollapseProperties,
        compiler: &mut AbstractCompiler,
        _externs: NodeId,
        root: NodeId,
    ) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut RewriteSimpleDestructuringAliases::callback(),
        );

        // Building the `GlobalNamespace` dominates the cost of this pass, so it is built once and
        // updated as changes are made so it can be reused for the next iteration.
        outer.namespace = Some(GlobalNamespace::new_with_decisions_log(
            outer.decisions_log.clone(),
            compiler,
            root,
        ));
        let namespace = outer.namespace.as_mut().unwrap();
        while self.code_changed {
            self.code_changed = false;
            self.inline_aliases(compiler, namespace);
        }
    }

    /// For each qualified name N in the global scope, we check if: (a) No ancestor of N is ever
    /// aliased or assigned an unknown value type. (If N = "a.b.c", "a" and "a.b" are never
    /// aliased). (b) N has exactly one write, and it lives in the global scope. (c) N is aliased
    /// in a local scope. (d) N is aliased in global scope
    ///
    /// See the Java documentation for the full description of the conditions.
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#inlineAliases
    fn inline_aliases(&mut self, compiler: &mut AbstractCompiler, namespace: &mut GlobalNamespace) {
        // Invariant: All the names in the worklist meet condition (a).
        // adds all top-level names to the worklist, but not any properties on those names.
        let mut work_list: VecDeque<Name> = namespace
            .get_name_forest(compiler)
            .iter()
            .copied()
            .collect();

        while let Some(name) = work_list.pop_front() {
            // Don't attempt to inline a getter or setter property as a variable.
            if name.is_get_or_set_definition(namespace) {
                continue;
            }

            if !name.in_externs(namespace) // not an externs definition
                && name.get_global_sets(namespace) == 1 // set exactly once and only set in the global scope
                && name.get_local_sets(namespace) == 0
            {
                // {@code name} meets condition (b). Find all of its aliases
                // and try to inline them.
                self.maybe_inline_inner_name(compiler, namespace, name);
                if name.get_aliasing_gets(namespace) > 0 || name.get_subclassing_gets(namespace) > 0
                {
                    // condition (c) and/or condition (d) are true
                    self.inline_aliases_for_name(compiler, namespace, name);
                }
            }
            maybe_add_properties_to_work_list(compiler, namespace, name, &mut work_list);
        }
    }

    /// Inlines a global name into all the places where references for aliases to it currently
    /// exist.
    ///
    /// This method only handles aliases created by assignment. In particular, it doesn't handle
    /// aliases created by inner names on class or function expressions. See
    /// (maybeInlineInnerName() for that).
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#inlineAliasesForName
    fn inline_aliases_for_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        name: Name,
    ) {
        let refs: Vec<Ref> = name.get_refs(namespace);
        for r#ref in refs {
            if r#ref.is_aliasing_get(namespace)
                && !may_be_global_alias(compiler, namespace, r#ref)
                && !r#ref.is_twin(namespace)
            {
                // {@code name} meets condition (c). Try to inline it.
                // TODO(johnlenz): consider picking up new aliases at the end
                // of the pass instead of immediately like we do for global
                // inlines.
                self.inline_alias_if_possible(compiler, namespace, name, r#ref);
            } else if r#ref.is_aliasing_get(namespace)
                && r#ref
                    .scope(namespace)
                    .unwrap()
                    .get_closest_hoist_scope(compiler)
                    .unwrap()
                    .is_global(compiler)
                && !r#ref.is_twin(namespace)
            {
                self.inline_global_alias_if_possible(compiler, namespace, name, r#ref);
            } else if name.is_class(namespace)
                && r#ref.is_subclassing_get(namespace)
                && name.props(namespace).is_some()
            {
                let props: Vec<Name> = name.props(namespace).unwrap().to_vec();
                for prop in props {
                    self.rewrite_all_subclass_inherited_accesses(
                        compiler, namespace, name, r#ref, prop,
                    );
                }
            }
        }
    }

    /// If the global name is a function with an inner-scope name, inline references to that name
    /// with the global name. Class inner name rewrites are handled in `Es6NormalizeClasses`.
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#maybeInlineInnerName
    fn maybe_inline_inner_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        global_name: Name,
    ) {
        let global_name_declaration = check_not_null!(
            global_name.get_declaration(namespace),
            "%s",
            global_name.to_string(namespace)
        );
        let global_declaration_node = check_not_null!(
            global_name_declaration.get_node(namespace),
            "%s",
            global_name_declaration.to_string(namespace, compiler)
        );
        let Some(value_node) = NodeUtil::get_r_value_of_l_value(compiler, global_declaration_node)
        else {
            // no function or class expression, so no inner name
            return;
        };
        let Some(inner_name_node) = maybe_get_inner_name_node(compiler, value_node) else {
            // no inner name to require inlining
            return;
        };
        let inner_name = inner_name_node.get_string(compiler);
        let mut syntactic_scope_creator = SyntacticScopeCreator::new();
        let inner_scope = syntactic_scope_creator.create_scope(
            compiler,
            value_node,
            global_name_declaration.scope(namespace),
        );
        let inner_name_var = check_not_null!(inner_scope.get_var(compiler, &inner_name));
        let inner_name_refs: ReferenceCollection = {
            let mut collector = ReferenceCollector::new_with_filter(
                compiler,
                ReferenceCollector::DO_NOTHING_BEHAVIOR,
                &mut syntactic_scope_creator,
                Box::new(move |_, v| v == inner_name_var),
            );
            collector.process_scope(compiler, inner_scope);
            check_not_null!(collector.get_references(inner_name_var)).clone()
        };

        let mut new_nodes: IndexSet<AstChange> = IndexSet::new();

        for inner_name_ref in inner_name_refs.iter() {
            // replace all references to the inner name other than its declaration
            let inner_name_ref_node = inner_name_ref.get_node();
            if NodeUtil::is_normal_get(compiler, inner_name_ref_node.get_parent(compiler).unwrap())
            {
                // Replace `innerName` with `globalName` for `innerName.prop` and
                // `innerName[expr]`
                //
                // TODO(b/148237949): We are intentionally ignoring cases where the inner name
                // escapes to other scopes where properties may be accessed on it (e.g.
                // `use(InnerName)`). This is unsafe, but currently necessary to avoid large code
                // size regressions.
                //
                // NOTE: We also don't want to introduce a global reference for cases like
                // `x instanceof innerName`. It would be safe to inline these, but it also isn't
                // necessary, and the introduction of a reference to a global in a local scope can
                // cause other optimizations to back off.
                new_nodes.insert(self.replace_alias_reference(
                    compiler,
                    namespace,
                    global_name_declaration,
                    inner_name_ref,
                ));
            }
        }
        namespace.scan_new_nodes(compiler, &new_nodes);
    }

    /// Inline all references to inherited static superclass properties from the subclass or any
    /// descendant of the given subclass. Avoids inlining references to inherited methods when
    /// possible, since they may use this or super().
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#rewriteAllSubclassInheritedAccesses
    fn rewrite_all_subclass_inherited_accesses(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        superclass_name_obj: Name,
        superclass_ref: Ref,
        prop: Name,
    ) -> bool {
        if !prop.can_collapse(namespace, compiler) {
            return false; // inlining is a) unnecessary if there is @nocollapse and b) might break
            // usages of `this` in the method
        }
        let subclass =
            get_subclass_for_es6_superclass(compiler, superclass_ref.get_node(namespace).unwrap());
        let Some(subclass) = subclass.filter(|s| s.is_qualified_name(compiler)) else {
            return false;
        };

        let subclass_name = subclass.get_qualified_name(compiler).unwrap();
        let subclass_qualified_prop_name = JsString::from(format!(
            "{}.{}",
            subclass_name.to_string_lossy(),
            prop.get_base_name(namespace).to_string_lossy()
        ));
        let subclass_prop_name_obj =
            namespace.get_own_slot(compiler, &subclass_qualified_prop_name);
        // Don't rewrite if the subclass ever shadows the parent static property.
        // This may also back off on cases where the subclass first accesses the parent property,
        // then shadows it.
        if let Some(p) = subclass_prop_name_obj
            && (p.get_local_sets(namespace) > 0 || p.get_global_sets(namespace) > 0)
        {
            return false;
        }

        // Recurse to find potential sub-subclass accesses of the superclass property.
        let subclass_name_obj = namespace.get_own_slot(compiler, &subclass_name);
        if let Some(subclass_name_obj) = subclass_name_obj
            && subclass_name_obj.subclassing_get_count(namespace) > 0
        {
            for r#ref in subclass_name_obj.get_refs(namespace) {
                if r#ref.is_subclassing_get(namespace) {
                    self.rewrite_all_subclass_inherited_accesses(
                        compiler,
                        namespace,
                        superclass_name_obj,
                        r#ref,
                        prop,
                    );
                }
            }
        }

        if let Some(subclass_prop_name_obj) = subclass_prop_name_obj {
            let mut new_nodes: IndexSet<AstChange> = IndexSet::new();

            // Use this node as a template for rewriteNestedAliasReference.
            let mut superclass_name_node = superclass_name_obj
                .get_declaration(namespace)
                .unwrap()
                .get_node(namespace)
                .unwrap();
            if superclass_name_node.is_name(compiler) {
                superclass_name_node = superclass_name_node.clone_node(compiler);
            } else if superclass_name_node.is_get_prop(compiler) {
                superclass_name_node = superclass_name_node.clone_tree(compiler);
            } else {
                return false;
            }

            self.rewrite_nested_alias_reference(
                compiler,
                namespace,
                superclass_name_node,
                0,
                &mut new_nodes,
                subclass_prop_name_obj,
            );
            namespace.scan_new_nodes(compiler, &new_nodes);
        }
        true
    }

    /// Recognizes aliases for the special global variables representing the `exports` values for
    /// goog modules which are safe to inline.
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#isInlineableModuleExportsAlias
    fn is_inlineable_module_exports_alias(
        &self,
        compiler: &AbstractCompiler,
        namespace: &GlobalNamespace,
        name: Name,
        alias_refs: &ReferenceCollection,
    ) -> bool {
        let aliased_name_str = name.get_full_name(namespace);
        if !aliased_name_str.starts_with(&JsString::from(MODULE_EXPORTS_PREFIX)) {
            return false;
        }
        // the first rhs value Node we find assigned to the name
        let mut first_rhs: Option<NodeId> = None;
        let size = alias_refs.references.len();
        for i in 0..size {
            let r#ref = &alias_refs.references[i];
            if r#ref.is_var_declaration(compiler) || r#ref.get_parent(compiler).is_assign(compiler)
            {
                let rhs = if r#ref.get_parent(compiler).is_assign(compiler) {
                    r#ref.get_parent(compiler).get_second_child(compiler)
                } else {
                    r#ref.get_node().get_first_child(compiler)
                };

                if let Some(rhs) = rhs {
                    // Make sure that every time a value is assigned to the alias, it is
                    // the aliased name we expect. If not, we cannot inline this alias.
                    match first_rhs {
                        None => {
                            // NOTE: comparing with a string is slower, but necessary the first
                            // time.
                            if !rhs.matches_qualified_name(compiler, aliased_name_str.clone()) {
                                return false;
                            }
                            first_rhs = Some(rhs);
                        }
                        Some(first_rhs) => {
                            // comparing nodes is faster than comparing with the string name
                            if !rhs.matches_qualified_name_node(compiler, first_rhs) {
                                return false;
                            }
                        }
                    }
                }
                continue;
            }
            if !r#ref.is_dot_property_access(compiler)
                && !r#ref.is_assigned_to_object_destructuring_pattern(compiler)
            {
                return false;
            }
        }
        true
    }

    /// Attempts to inline a non-global alias of a global name.
    ///
    /// It is assumed that the name for which it is an alias meets conditions (a) and (b).
    ///
    /// The non-global alias is only inlinable if it is well-defined and assigned once, according
    /// to the definitions in `ReferenceCollection`
    ///
    /// If the aliasing name is completely removed, also deletes the aliasing Ref.
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#inlineAliasIfPossible
    fn inline_alias_if_possible(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        name: Name,
        alias: Ref,
    ) {
        // Ensure that the alias is assigned to a local variable at that
        // variable's declaration. If the alias's parent is a NAME,
        // then the NAME must be the child of a VAR, LET, or CONST node, and we must
        // be in a VAR, LET, or CONST assignment.
        // Otherwise if the parent is an assign, we are in a "a = alias" case.
        let alias_parent = alias
            .get_node(namespace)
            .unwrap()
            .get_parent(compiler)
            .unwrap();
        if alias_parent.is_name(compiler) || alias_parent.is_assign(compiler) {
            let alias_lhs_node = if alias_parent.is_name(compiler) {
                alias_parent
            } else {
                alias_parent.get_first_child(compiler).unwrap()
            };
            let alias_var_name = alias_lhs_node.get_string(compiler);

            let alias_var = alias
                .scope(namespace)
                .unwrap()
                .get_var(compiler, &alias_var_name);
            check_state!(
                alias_var.is_some(),
                "Expected variable to be defined in scope (%s)",
                alias_var_name.to_string_lossy()
            );
            let alias_var = alias_var.unwrap();
            let mut syntactic_scope_creator = SyntacticScopeCreator::new();
            let alias_refs: ReferenceCollection = {
                let mut collector = ReferenceCollector::new_with_filter(
                    compiler,
                    ReferenceCollector::DO_NOTHING_BEHAVIOR,
                    &mut syntactic_scope_creator,
                    Box::new(move |_, v| v == alias_var),
                );
                let alias_scope = alias_var.get_scope(compiler);
                collector.process_scope(compiler, alias_scope);

                check_not_null!(collector.get_references(alias_var)).clone()
            };
            let mut new_nodes: IndexSet<AstChange> = IndexSet::new();

            if alias_refs.is_well_defined(compiler)
                && (alias_refs.is_assigned_once_in_lifetime(compiler)
                    || self.is_inlineable_module_exports_alias(
                        compiler,
                        namespace,
                        name,
                        &alias_refs,
                    ))
            {
                // The alias is well-formed, so do the inlining now.
                let size = alias_refs.references.len();
                // It's initialized on either the first or second reference.
                let first_read = if alias_refs.references[0].is_initializing_declaration(compiler) {
                    1
                } else {
                    2
                };
                for i in first_read..size {
                    let alias_ref = &alias_refs.references[i];
                    new_nodes.insert(
                        self.replace_alias_reference(compiler, namespace, alias, alias_ref),
                    );
                }

                // just set the original alias to null.
                self.try_replacing_aliasing_assignment(
                    compiler,
                    namespace,
                    alias,
                    name,
                    alias_lhs_node,
                );

                // Inlining the variable may have introduced new references
                // to descendants of {@code name}. So those need to be collected now.
                namespace.scan_new_nodes(compiler, &new_nodes);
                return;
            }

            if name.is_constructor(namespace, compiler) {
                // TODO(lharker): the main reason this was added is because method decomposition
                // inside generators introduces some constructor aliases that weren't getting
                // inlined. If we find another (safer) way to avoid aliasing in method
                // decomposition, consider removing this.
                if !self.partially_inline_alias(
                    compiler,
                    namespace,
                    alias,
                    name,
                    &alias_refs,
                    alias_lhs_node,
                ) {
                    // If we can't inline all alias references, make sure there are no unsafe
                    // property accesses.
                    if references_collapsible_property(compiler, namespace, &alias_refs, name) {
                        let alias_var_name = alias_var_name.to_string_lossy();
                        compiler.report(JSError::make(
                            compiler,
                            alias_parent,
                            &UNSAFE_CTOR_ALIASING,
                            &[&alias_var_name],
                        ));
                    }
                }
            }
        }
    }

    /// Inlines some references to an alias with its value. This handles cases where the alias is
    /// not declared at initialization. It does nothing if the alias is reassigned after being
    /// initialized, unless the reassignment occurs because of an enclosing function or a loop.
    ///
    /// Returns whether all references to the alias were inlined
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#partiallyInlineAlias
    fn partially_inline_alias(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        alias: Ref,
        aliasing_name: Name,
        alias_refs: &ReferenceCollection,
        alias_lhs_node: NodeId,
    ) -> bool {
        let mut alias_block = None;
        // This initial iteration through all the alias references does two things:
        // a) Find the control flow block in which the alias is assigned.
        // b) See if the alias var is assigned to in multiple places, and return if that's the
        //    case.
        //    NOTE: we still may inline if the alias is assigned in a loop or inner function and
        //    that assignment statement is potentially executed multiple times.
        //    This is more aggressive than what "inlineAliasIfPossible" does.
        for alias_ref in alias_refs.iter() {
            let alias_ref_node = alias_ref.get_node();
            if alias_ref_node == alias_lhs_node {
                alias_block = alias_ref.get_basic_block().cloned();
                continue;
            } else if alias_ref.is_lvalue(compiler) {
                // Don't replace any references if the alias is reassigned
                return false;
            }
        }

        let mut new_nodes: IndexSet<AstChange> = IndexSet::new();
        let mut already_seen_initial_alias = false;
        let mut found_non_replaceable_alias = false;
        // Do a second iteration through all the alias references, and replace any inlinable
        // references.
        for alias_ref in alias_refs.iter() {
            let alias_ref_node = alias_ref.get_node();
            if alias_ref_node == alias_lhs_node {
                already_seen_initial_alias = true;
                continue;
            } else if alias_ref.is_declaration(compiler) {
                // Ignore any alias declarations, e.g. "var alias;", since there's nothing to
                // inline.
                continue;
            }

            let ref_block = alias_ref.get_basic_block();
            let same_block = match (ref_block, alias_block.as_ref()) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            if (!same_block
                && alias_block
                    .as_ref()
                    .expect("NullPointerException: aliasBlock")
                    .provably_executes_before(ref_block.expect("NullPointerException: refBlock")))
                || (same_block && already_seen_initial_alias)
            {
                // We replace the alias only if the alias and reference are in the same
                // BasicBlock, the aliasing assignment takes place before the reference, and the
                // alias is never reassigned.
                self.code_changed = true;
                new_nodes
                    .insert(self.replace_alias_reference(compiler, namespace, alias, alias_ref));
            } else {
                found_non_replaceable_alias = true;
            }
        }

        // We removed all references to the alias, so remove the original aliasing assignment.
        if !found_non_replaceable_alias {
            self.try_replacing_aliasing_assignment(
                compiler,
                namespace,
                alias,
                aliasing_name,
                alias_lhs_node,
            );
        }

        if self.code_changed {
            // Inlining the variable may have introduced new references
            // to descendants of {@code name}. So those need to be collected now.
            namespace.scan_new_nodes(compiler, &new_nodes);
        }
        !found_non_replaceable_alias
    }

    /// Replaces the rhs of an aliasing assignment with null, unless the assignment result is used
    /// in a complex expression.
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#tryReplacingAliasingAssignment
    fn try_replacing_aliasing_assignment(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        alias: Ref,
        alias_name: Name,
        alias_lhs_node: NodeId,
    ) -> bool {
        // either VAR/CONST/LET or ASSIGN.
        let assignment = alias_lhs_node.get_parent(compiler).unwrap();
        if !NodeUtil::is_name_declaration(compiler, Some(assignment))
            && NodeUtil::is_expression_result_used(compiler, assignment)
        {
            // e.g. don't change "if (alias = someVariable)" to "if (alias = null)"
            // TODO(lharker): instead replace the entire assignment with the RHS - "alias = x"
            // becomes "x"
            return false;
        }
        let alias_node = alias.get_node(namespace).unwrap();
        let alias_parent = alias_node.get_parent(compiler).unwrap();
        let null_node = IR::null_node(compiler);
        alias_node.replace_with(compiler, null_node);
        alias_name.remove_ref(namespace, compiler, alias);
        self.code_changed = true;
        compiler.report_change_to_enclosing_scope(alias_parent);
        true
    }

    /// Returns an AstChange representing the new node(s) added to the AST.
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#replaceAliasReference
    fn replace_alias_reference(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        alias: Ref,
        alias_ref: &crate::reference::Reference,
    ) -> AstChange {
        let original_ref_node = alias.get_node(namespace).unwrap();
        let node_to_replace = alias_ref.get_node();
        check_state!(
            node_to_replace.is_qualified_name(compiler),
            "%s",
            node_to_replace.to_string(compiler)
        );
        // If the reference node is a NAME it could be
        // const origName = value;
        // If we use cloneTree() for that we'll clone the value, which we don't want.
        // Otherwise, we do want to clone the tree of GETPROP nodes.
        let new_node = if original_ref_node.is_name(compiler) {
            original_ref_node.clone_node(compiler)
        } else {
            original_ref_node.clone_tree(compiler)
        };
        new_node.srcref_tree(compiler, node_to_replace);
        node_to_replace.replace_with(compiler, new_node);
        compiler.report_change_to_enclosing_scope(new_node);
        AstChange::SimpleAstChange(SimpleAstChange::new(
            new_node,
            alias.get_chunk(namespace),
            alias.scope(namespace),
        ))
    }

    /// Attempt to inline an global alias of a global name. This requires that the name is well
    /// defined: assigned unconditionally, assigned exactly once. It is assumed that, the name for
    /// which it is an alias must already meet these same requirements.
    ///
    /// If the alias is completely removed, also deletes the aliasing Ref.
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#inlineGlobalAliasIfPossible
    fn inline_global_alias_if_possible(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        name: Name,
        alias: Ref,
    ) {
        // Ensure that the alias is assigned to global name at that the
        // declaration.
        let alias_node = alias.get_node(namespace).unwrap();
        let alias_parent = alias_node.get_parent(compiler).unwrap();
        if ((alias_parent.is_assign(compiler) || alias_parent.is_name(compiler))
            && NodeUtil::is_executed_exactly_once(compiler, alias_parent))
            // We special-case for constructors here, to inline constructor aliases
            // more aggressively in global scope.
            // We do this because constructor properties are always collapsed,
            // so we want to inline the aliases also to avoid breakages.
            || (alias_parent.is_name(compiler) && name.is_constructor(namespace, compiler))
        {
            let lvalue = if alias_parent.is_name(compiler) {
                alias_parent
            } else {
                alias_parent.get_first_child(compiler).unwrap()
            };
            if !lvalue.is_qualified_name(compiler) {
                return;
            }
            if lvalue.is_name(compiler)
                && compiler
                    .get_coding_convention()
                    .is_exported(&lvalue.get_string(compiler), /* local= */ false)
            {
                return;
            }
            let lvalue_qname = lvalue.get_qualified_name(compiler).unwrap();
            let Some(aliasing_name) = namespace.get_slot(compiler, &lvalue_qname) else {
                // this is true for names in externs or properties on extern names
                return;
            };

            if name == aliasing_name && alias_parent.is_assign(compiler) {
                // Ignore `a.b.c = a.b.c;` with `a.b.c;`.
                return;
            }

            let alias_inlinability = aliasing_name.calculate_inlinability(namespace, compiler);
            if !alias_inlinability.should_inline_usages() {
                // nothing to do here
                return;
            }

            // Rewrite all references to the aliasing name, except for the initialization & any
            // cross-chunk references if they exist.
            let mut new_nodes = self.rewrite_alias_references(
                compiler,
                namespace,
                aliasing_name,
                alias,
                alias_inlinability,
            );
            self.rewrite_alias_props(
                compiler,
                namespace,
                aliasing_name,
                alias.get_node(namespace).unwrap(),
                0,
                &mut new_nodes,
            );

            if alias_inlinability.should_remove_declaration() {
                // Rewrite the initialization of the alias.
                let alias_initialization = aliasing_name.get_initialization(namespace).unwrap();
                if alias_initialization.is_twin(namespace) {
                    // This is in a nested assign.
                    // Replace
                    //   a.b = aliasing.name = aliased.name
                    // with
                    //   a.b = aliased.name
                    check_state!(
                        alias_parent.is_assign(compiler),
                        "%s",
                        alias_parent.to_string(compiler)
                    );
                    let alias_grandparent = alias_parent.get_parent(compiler).unwrap();
                    let detached = alias.get_node(namespace).unwrap().detach(compiler);
                    alias_parent.replace_with(compiler, detached);
                    // Remove the ref to 'aliasing.name' entirely
                    aliasing_name.remove_ref(namespace, compiler, alias_initialization);
                    // Force GlobalNamespace to revisit the new reference to 'aliased.name' and
                    // update its internal state.
                    new_nodes.insert(AstChange::RefBasedAstChange(RefBasedAstChange::new(
                        alias,
                        alias.get_node(namespace).unwrap(),
                    )));
                    compiler.report_change_to_enclosing_scope(alias_grandparent);
                } else {
                    // Replace
                    //  aliasing.name = aliased.name
                    // with
                    //  aliasing.name = null;
                    let null_node = IR::null_node(compiler);
                    alias
                        .get_node(namespace)
                        .unwrap()
                        .replace_with(compiler, null_node);
                    compiler.report_change_to_enclosing_scope(alias_parent);
                }
                self.code_changed = true;
                // Update the original aliased name to say that it has one less ALIASING_REF.
                name.remove_ref(namespace, compiler, alias);
            }

            // Inlining the variable may have introduced new references
            // to descendants of {@code name}. So those need to be collected now.
            namespace.scan_new_nodes(compiler, &new_nodes);
        }
    }

    /// Replaces reads of a name with the name it aliases
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#rewriteAliasReferences
    fn rewrite_alias_references(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        aliasing_name: Name,
        aliasing_ref: Ref,
        alias_inlinability: Inlinability,
    ) -> IndexSet<AstChange> {
        let mut new_nodes: IndexSet<AstChange> = IndexSet::new();
        let refs: Vec<Ref> = aliasing_name.get_refs(namespace);
        let initialization_chunk = match aliasing_name.get_initialization(namespace) {
            Some(initialization) => initialization.get_chunk(namespace),
            None => aliasing_name
                .get_declaration(namespace)
                .unwrap()
                .get_chunk(namespace),
        };
        for r#ref in refs {
            if r#ref.is_set_from_global(namespace) {
                // Handled elsewhere
                // This should either be the initialization or an irrelevant uninitialized
                // declaration.
                check_state!(
                    r#ref.get_chunk(namespace) == initialization_chunk
                        || r#ref.is_uninitialized_declaration(namespace, compiler),
                    "%s",
                    r#ref.to_string(namespace, compiler)
                );
                continue;
            }

            check_state!(
                r#ref.is_get(namespace),
                "%s",
                r#ref.to_string(namespace, compiler)
            ); // names with local sets should not be rewritten
            // Twin refs that are both gets and sets are handled later, with the other sets.
            check_state!(
                !r#ref.is_twin(namespace),
                "%s",
                r#ref.to_string(namespace, compiler)
            );

            if alias_inlinability == Inlinability::INLINE_UNLESS_INVALID_CROSS_CHUNK_DEPENDENCY
                && r#ref.get_chunk(namespace) != initialization_chunk
                && !compiler.get_chunk_graph().unwrap().depends_on(
                    r#ref.get_chunk(namespace).as_ref().unwrap(),
                    initialization_chunk.as_ref().unwrap(),
                )
            {
                continue;
            }

            let ref_node = r#ref.get_node(namespace).unwrap();
            if ref_node.is_string_key(compiler) {
                // e.g. `y` in `const {y} = x;`
                let new_name = aliasing_ref
                    .get_node(namespace)
                    .unwrap()
                    .clone_tree(compiler);
                DestructuringGlobalNameExtractor::reassign_destructring_lvalue(
                    ref_node,
                    new_name,
                    Some(&mut new_nodes),
                    r#ref,
                    compiler,
                );
            } else {
                // e.g. `x.y`
                check_state!(ref_node.is_get_prop(compiler) || ref_node.is_name(compiler));
                let new_node = aliasing_ref
                    .get_node(namespace)
                    .unwrap()
                    .clone_tree(compiler);
                let node = ref_node;
                new_node.srcref(compiler, node);
                node.replace_with(compiler, new_node);
                compiler.report_change_to_enclosing_scope(new_node);
                new_nodes.insert(AstChange::RefBasedAstChange(RefBasedAstChange::new(
                    r#ref, new_node,
                )));
            }
            aliasing_name.remove_ref(namespace, compiler, r#ref);
        }
        new_nodes
    }

    // port: InlineAndCollapseProperties.AggressiveInlineAliases#rewriteAliasProps
    fn rewrite_alias_props(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        name: Name,
        value: NodeId,
        depth: i32,
        new_nodes: &mut IndexSet<AstChange>,
    ) {
        let Some(props) = name.props(namespace) else {
            return;
        };
        let props: Vec<Name> = props.to_vec();
        check_state!(
            !value.matches_qualified_name(compiler, name.get_full_name(namespace)),
            "%s should not match name %s",
            value.to_string(compiler),
            name.get_full_name(namespace).to_string_lossy()
        );
        for prop in props {
            self.rewrite_nested_alias_reference(compiler, namespace, value, depth, new_nodes, prop);
        }
    }

    /// Replaces references to an alias that are nested inside a longer getprop chain or an object
    /// literal
    ///
    /// For example: if we have an inlined alias 'const A = B;', and reference a property 'A.x',
    /// then this method is responsible for replacing 'A.x' with 'B.x'.
    // port: InlineAndCollapseProperties.AggressiveInlineAliases#rewriteNestedAliasReference
    fn rewrite_nested_alias_reference(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        value: NodeId,
        depth: i32,
        new_nodes: &mut IndexSet<AstChange>,
        prop: Name,
    ) {
        self.rewrite_alias_props(compiler, namespace, prop, value, depth + 1, new_nodes);
        let refs: Vec<Ref> = prop.get_refs(namespace);
        for r#ref in refs {
            let mut target = r#ref.get_node(namespace).unwrap();
            if target.is_string_key(compiler)
                && target
                    .get_parent(compiler)
                    .unwrap()
                    .is_destructuring_pattern(compiler)
            {
                // Do nothing for alias properties accessed through object destructuring. This
                // would be redundant. See the Java comment for the full explanation.
                let grandparent = target.get_grandparent(compiler).unwrap();
                check_state!(
                    grandparent.is_assign(compiler) || grandparent.is_destructuring_lhs(compiler),
                    // Currently GlobalNamespace doesn't create Refs for 'b' in
                    // const {a: {b}} = obj; If it does start creating those Refs, we may have to
                    // update this method to handle them explicitly.
                    "Did not expect GlobalNamespace to create Ref for key in nested object pattern %s",
                    target.to_string(compiler)
                );
                continue;
            }

            if target.is_member_function_def(compiler) {
                // In the case of a method reference, the reference to the alias is not fully
                // qualified, so we should not touch it, see b/293184904.
                continue;
            }

            for _i in 0..=depth {
                if target.is_get_prop(compiler) {
                    target = target.get_first_child(compiler).unwrap();
                } else if NodeUtil::is_object_lit_key(compiler, target) {
                    // Object literal key definitions are a little trickier, as we
                    // need to find the assignment target
                    let gparent = target.get_grandparent(compiler).unwrap();
                    if gparent.is_assign(compiler) {
                        target = gparent.get_first_child(compiler).unwrap();
                    } else {
                        check_state!(NodeUtil::is_object_lit_key(compiler, gparent));
                        target = gparent;
                    }
                } else {
                    panic!("unexpected node: {}", target.to_string(compiler));
                }
            }
            check_state!(target.is_get_prop(compiler) || target.is_name(compiler));
            let new_value = value.clone_tree(compiler);
            target.replace_with(compiler, new_value);
            compiler.report_change_to_enclosing_scope(new_value);
            prop.remove_ref(namespace, compiler, r#ref);
            // Rescan the expression root.
            let ref_node = r#ref.get_node(namespace).unwrap();
            new_nodes.insert(AstChange::RefBasedAstChange(RefBasedAstChange::new(
                r#ref, ref_node,
            )));
            self.code_changed = true;
        }
    }
}

/// Rewrite "simple" destructuring aliases to a format that is more amenable to inlining.
///
/// To be specific, this rewrites aliases of the form: const {x} = qualified.name; to: const x =
/// qualified.name.x;
struct RewriteSimpleDestructuringAliases;

impl RewriteSimpleDestructuringAliases {
    /// Rust-only: the `AbstractPostOrderCallback` instance.
    fn callback() -> impl Callback {
        crate::node_traversal::AbstractPostOrderCallback::new(RewriteSimpleDestructuringAliases)
    }

    // port: InlineAndCollapseProperties.RewriteSimpleDestructuringAliases#isSimpleDestructuringAlias
    fn is_simple_destructuring_alias(ast: &closure_rhino::node::Ast, n: NodeId) -> bool {
        if !NodeUtil::is_statement(ast, n) || !n.is_const(ast) {
            return false;
        }
        check_state!(n.has_one_child(ast));
        let destructuring_lhs = n.get_first_child(ast).unwrap();
        if !destructuring_lhs.is_destructuring_lhs(ast) {
            return false;
        }
        let object_pattern = destructuring_lhs.get_first_child(ast).unwrap();
        if !object_pattern.is_object_pattern(ast) {
            return false;
        }
        let rhs = destructuring_lhs.get_last_child(ast).unwrap();
        if !rhs.is_qualified_name(ast) {
            return false;
        }
        Self::is_simple_destructuring_pattern(ast, object_pattern)
    }

    // port: InlineAndCollapseProperties.RewriteSimpleDestructuringAliases#isSimpleDestructuringPattern
    fn is_simple_destructuring_pattern(
        ast: &closure_rhino::node::Ast,
        object_pattern: NodeId,
    ) -> bool {
        check_argument!(object_pattern.is_object_pattern(ast));
        let mut key = object_pattern.get_first_child(ast);
        while let Some(k) = key {
            if !k.is_string_key(ast) || k.is_quoted_string_key(ast) {
                return false;
            }
            check_state!(k.has_one_child(ast));
            let rhs = k.get_first_child(ast).unwrap();
            if !rhs.is_object_pattern(ast) && !rhs.is_name(ast) {
                return false;
            }
            if rhs.is_object_pattern(ast) && !Self::is_simple_destructuring_pattern(ast, rhs) {
                return false;
            }
            key = k.get_next(ast);
        }
        true
    }

    // port: InlineAndCollapseProperties.RewriteSimpleDestructuringAliases#expandObjectPattern
    fn expand_object_pattern(
        t: &mut NodeTraversal<'_>,
        insertion_point: NodeId,
        object_pattern: NodeId,
        rhs: NodeId,
    ) -> NodeId {
        let mut insertion_point = insertion_point;
        let mut key = object_pattern.get_first_child(t);
        while let Some(k) = key {
            let key_child = k.get_first_child(t).unwrap();
            let name_node = if key_child.is_name(t) {
                key_child.detach(t)
            } else {
                check_state!(key_child.is_object_pattern(t));
                let input = t.get_input().cloned().unwrap();
                let unique_id = t
                    .get_compiler()
                    .get_unique_id_supplier()
                    .get_unique_id(&input);
                let name_node =
                    IR::name(t, format!("destructuring${unique_id}")).srcref(t, key_child);
                name_node.put_boolean_prop(t, Prop::IS_CONSTANT_NAME, true);
                name_node
            };
            let rhs_clone = rhs.clone_tree(t);
            let key_string = k.get_string(t);
            let new_rhs = IR::getprop(t, rhs_clone, key_string).srcref(t, key_child);
            let new_const_node = IR::const_node(t, name_node, new_rhs).srcref(t, object_pattern);
            new_const_node.insert_after(t, insertion_point);
            insertion_point = new_const_node;
            if key_child.is_object_pattern(t) {
                let name_clone = name_node.clone_node(t);
                insertion_point =
                    Self::expand_object_pattern(t, insertion_point, key_child, name_clone);
            }
            key = k.get_next(t);
        }
        insertion_point
    }
}

impl AbstractPostOrderCallbackInterface for RewriteSimpleDestructuringAliases {
    // port: InlineAndCollapseProperties.RewriteSimpleDestructuringAliases#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !Self::is_simple_destructuring_alias(t, n) {
            return;
        }

        let insertion_point = n;

        let destructuring_lhs = n.get_first_child(t).unwrap();
        let object_pattern = destructuring_lhs.get_first_child(t).unwrap();
        let rhs = destructuring_lhs.get_last_child(t).unwrap();
        let _unused_new_insertion_point =
            Self::expand_object_pattern(t, insertion_point, object_pattern, rhs);
        n.detach(t);
        t.report_code_change();
    }
}

// port: InlineAndCollapseProperties#maybeGetInnerNameNode
fn maybe_get_inner_name_node(
    ast: &closure_rhino::node::Ast,
    maybe_function_or_class_node: NodeId,
) -> Option<NodeId> {
    if NodeUtil::is_function_expression(ast, maybe_function_or_class_node) {
        let name_node = maybe_function_or_class_node.get_first_child(ast).unwrap();
        check_state!(name_node.is_name(ast), "%s", name_node.to_string(ast));
        // functions with no name have a NAME node with an empty string
        if name_node.get_string_ref(ast).is_empty() {
            None
        } else {
            Some(name_node)
        }
    } else if NodeUtil::is_class_expression(ast, maybe_function_or_class_node) {
        let name_node = maybe_function_or_class_node.get_first_child(ast).unwrap();
        // classes with no name have an EMPTY node first child
        if name_node.is_name(ast) {
            Some(name_node)
        } else {
            None
        }
    } else {
        None // not a function or class expression
    }
}

/// Adds properties of `name` to the worklist if the following conditions hold:
///
/// 1. The given property of `name` either meets condition (a) or is unsafely collapsible (as
///    defined by `Name#canCollapse()`
/// 2. `name` meets condition (b)
///
/// This only adds direct properties of a name, not all its descendants. For example, this adds
/// `a.b` given `a`, but not `a.b.c`.
// port: InlineAndCollapseProperties#maybeAddPropertiesToWorklist
fn maybe_add_properties_to_work_list(
    compiler: &AbstractCompiler,
    namespace: &GlobalNamespace,
    name: Name,
    work_list: &mut VecDeque<Name>,
) {
    if !(name.is_object_literal(namespace)
        || name.is_function(namespace)
        || name.is_class(namespace))
    {
        // Don't add properties for things like `Foo` in
        //   const Foo = someMysteriousFunctionCall();
        // Since `Foo` is not declared as an object, class, or function literal, assume its value
        // may be aliased somewhere and its properties do not meet condition (a).
        return;
    }
    if is_unsafely_reassigned(compiler, namespace, name) {
        // Don't add properties if this was assigned multiple times, except for 'safe'
        // reassignments:
        //    var ns = ns || {};
        // This is equivalent to condition (b)
        return;
    }
    let Some(props) = name.props(namespace) else {
        return;
    };

    if name.get_aliasing_gets(namespace) == 0 {
        // All of {@code name}'s children meet condition (a), so they can be
        // added to the worklist.
        work_list.extend(props.iter().copied());
    } else {
        // The children do NOT meet condition (a) but we may try to add them anyway.
        // This is because CollapseProperties will unsafely collapse properties on constructors
        // and enums, so we want to be more aggressive about inlining references to their
        // children.
        for &property in props {
            // Only add properties that would be unsafely collapsed by CollapseProperties
            if property.can_collapse(namespace, compiler) {
                work_list.push_back(property);
            }
        }
    }
}

/// Returns true if the alias is possibly defined in the global scope, which we handle with more
/// caution than with locally scoped variables. May return false positives.
// port: InlineAndCollapseProperties#mayBeGlobalAlias
fn may_be_global_alias(
    compiler: &mut AbstractCompiler,
    namespace: &GlobalNamespace,
    alias: Ref,
) -> bool {
    // Note: alias.scope is the closest scope in which the aliasing assignment occurred.
    // So for "if (true) { var alias = aliasedVar; }", the alias.scope would be the IF block
    // scope.
    let alias_scope = alias.scope(namespace).unwrap();
    if alias_scope.is_global(compiler) {
        return true;
    }
    // If the scope in which the alias is assigned is not global, look up the LHS of the
    // assignment.
    let alias_parent = alias
        .get_node(namespace)
        .unwrap()
        .get_parent(compiler)
        .unwrap();
    if !alias_parent.is_assign(compiler) && !alias_parent.is_name(compiler) {
        // Only handle variable assignments and initializing declarations.
        return true;
    }
    let alias_lhs_node = if alias_parent.is_name(compiler) {
        alias_parent
    } else {
        alias_parent.get_first_child(compiler).unwrap()
    };
    if !alias_lhs_node.is_name(compiler) {
        // Only handle assignments to simple names, not qualified names or GETPROPs.
        return true;
    }
    let alias_var_name = alias_lhs_node.get_string(compiler);
    let alias_var = alias_scope.get_var(compiler, &alias_var_name);
    if let Some(alias_var) = alias_var {
        return alias_var.is_global(compiler);
    }
    true
}

/// Returns whether a ReferenceCollection for some aliasing variable references a property on the
/// original aliased variable that may be collapsed in CollapseProperties.
// port: InlineAndCollapseProperties#referencesCollapsibleProperty
fn references_collapsible_property(
    compiler: &mut AbstractCompiler,
    namespace: &mut GlobalNamespace,
    alias_refs: &ReferenceCollection,
    aliased_name: Name,
) -> bool {
    for r#ref in alias_refs.references.iter() {
        let Some(ref_parent) = r#ref.get_node().get_parent(compiler) else {
            continue;
        };
        if NodeUtil::is_normal_or_opt_chain_get_prop(compiler, ref_parent) {
            // e.g. if the reference is "alias.b.someProp", this will be "b".
            let property_name = ref_parent.get_string(compiler);
            // e.g. if the aliased name is "originalName", this will be "originalName.b".
            let original_property_name = JsString::from(format!(
                "{}.{}",
                aliased_name.get_name(namespace).to_string_lossy(),
                property_name.to_string_lossy()
            ));
            let original_property = namespace.get_own_slot(compiler, &original_property_name);
            // If the original property isn't in the namespace or can't be collapsed, keep going.
            match original_property {
                Some(p) if p.can_collapse(namespace, compiler) => return true,
                _ => continue,
            }
        }
    }
    false
}

/// Check if the name has multiple sets that are not of the form "a = a || {}"
// port: InlineAndCollapseProperties#isUnsafelyReassigned
fn is_unsafely_reassigned(
    compiler: &AbstractCompiler,
    namespace: &GlobalNamespace,
    name: Name,
) -> bool {
    if name.get_total_sets(namespace) <= 1 {
        return false;
    }
    let mut found_original_definition = false;
    for r#ref in name.get_refs(namespace) {
        if !r#ref.is_set(namespace) {
            continue;
        }
        if InlineAndCollapseProperties::is_safe_namespace_reinit(compiler, namespace, r#ref) {
            continue;
        }
        if !found_original_definition {
            found_original_definition = true;
        } else {
            return true;
        }
    }
    false
}

/// Tries to find an lvalue for the subclass given the superclass node in an `class ... extends `
/// clause
///
/// Only handles cases where we have either a class declaration or a class expression in an
/// assignment or name declaration. Otherwise returns null.
// port: InlineAndCollapseProperties#getSubclassForEs6Superclass
fn get_subclass_for_es6_superclass(
    ast: &closure_rhino::node::Ast,
    superclass: NodeId,
) -> Option<NodeId> {
    let class_node = superclass.get_parent(ast).unwrap();
    check_argument!(class_node.is_class(ast), "%s", class_node.to_string(ast));
    if NodeUtil::is_name_declaration(ast, class_node.get_grandparent(ast)) {
        // const Clazz = class extends Super {
        class_node.get_parent(ast)
    } else if superclass
        .get_grandparent(ast)
        .is_some_and(|g| g.is_assign(ast))
    {
        // ns.foo.Clazz = class extends Super {
        class_node.get_previous(ast)
    } else if NodeUtil::is_class_declaration(ast, class_node) {
        // class Clazz extends Super {
        class_node.get_first_child(ast)
    } else {
        None
    }
}

impl InlineAndCollapseProperties {
    // port: InlineAndCollapseProperties#isSafeNamespaceReinit
    pub fn is_safe_namespace_reinit(
        compiler: &AbstractCompiler,
        namespace: &GlobalNamespace,
        r#ref: Ref,
    ) -> bool {
        // allow "a = a || {}" or "var a = a || {}" or "var a;"
        let val_parent = Self::get_value_parent(compiler, namespace, r#ref);
        let val = val_parent.get_last_child(compiler);
        if let Some(val) = val
            && val.is_or(compiler)
        {
            let maybe_name = val.get_first_child(compiler).unwrap();
            if r#ref
                .get_node(namespace)
                .unwrap()
                .matches_qualified_name_node(compiler, maybe_name)
            {
                return true;
            }
        }

        false
    }

    /// Gets the parent node of the value for any assignment to a Name. For example, in the
    /// assignment `var x = 3;` the parent would be the NAME node.
    // port: InlineAndCollapseProperties#getValueParent
    fn get_value_parent(
        compiler: &AbstractCompiler,
        namespace: &GlobalNamespace,
        r#ref: Ref,
    ) -> NodeId {
        // there are four types of declarations: VARs, LETs, CONSTs, and ASSIGNs
        let ref_node = r#ref.get_node(namespace).unwrap();
        let n = ref_node.get_parent(compiler);
        if n.is_some() && NodeUtil::is_name_declaration(compiler, n) {
            ref_node
        } else {
            ref_node.get_parent(compiler).unwrap()
        }
    }
}

/// Inline constant aliases
///
/// This pass was originally necessary because typechecking did not handle type aliases well.
/// Now typechecking understands type aliases. In theory, this pass can be deleted, but in
/// practice this pass affects some check passes that run post-typechecking.
///
/// This alias inliner is not very aggressive. It will only inline explicitly const aliases but
/// not effectively const ones (for example ones that are only ever assigned a value once).
///
/// TODO(b/124915436): Delete this pass.
struct InlineAliases {
    aliases: IndexMap<JsString, JsString>,
    namespace: Option<GlobalNamespace>,
    ast_factory: AstFactory,
}

impl InlineAliases {
    // port: InlineAndCollapseProperties.InlineAliases#InlineAliases
    fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            aliases: IndexMap::new(),
            namespace: None,
            ast_factory: compiler.create_ast_factory(),
        }
    }

    // port: InlineAndCollapseProperties.InlineAliases#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.namespace = Some(GlobalNamespace::new(compiler, externs, root));
        NodeTraversal::traverse_roots(
            compiler,
            &mut ExternsSkippingCallback::new(AliasesCollector { outer: self }),
            externs,
            root,
        );
        NodeTraversal::traverse_roots(
            compiler,
            &mut ExternsSkippingCallback::new(AliasesInliner { outer: self }),
            externs,
            root,
        );
    }
}

struct AliasesCollector<'a> {
    outer: &'a mut InlineAliases,
}

impl AbstractPostOrderCallbackInterface for AliasesCollector<'_> {
    // port: InlineAndCollapseProperties.InlineAliases.AliasesCollector#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow (switch case + if).
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::VAR | Token::CONST | Token::LET => {
                if n.has_one_child(t) && t.in_global_scope() {
                    let first = n.get_first_child(t).unwrap();
                    let info = NodeUtil::get_best_jsdoc_info(t, first);
                    self.visit_alias_definition(t, first, info.as_deref());
                }
            }
            Token::ASSIGN => {
                if parent.is_some_and(|p| p.is_expr_result(t)) && t.in_global_scope() {
                    let first = n.get_first_child(t).unwrap();
                    let info = n.get_jsdoc_info(t);
                    self.visit_alias_definition(t, first, info.as_deref());
                }
            }
            _ => {}
        }
    }
}

impl AliasesCollector<'_> {
    /// Maybe record that given lvalue is an alias of the qualified name on its rhs. Note that
    /// since we are doing a post-order traversal, any previous aliases contained in the rhs will
    /// have already been substituted by the time we record the new alias.
    // port: InlineAndCollapseProperties.InlineAliases.AliasesCollector#visitAliasDefinition
    fn visit_alias_definition(
        &mut self,
        t: &mut NodeTraversal<'_>,
        lhs: NodeId,
        info: Option<&JSDocInfo>,
    ) {
        if Self::is_declared_const(t, lhs, info)
            && info.is_none_or(|info| !info.has_type_information())
            && lhs.is_qualified_name(t)
        {
            let rhs = NodeUtil::get_r_value_of_l_value(t, lhs);
            if let Some(rhs) = rhs
                && rhs.is_qualified_name(t)
            {
                let lhs_qname = lhs.get_qualified_name(t).unwrap();
                let rhs_qname = rhs.get_qualified_name(t).unwrap();
                let namespace = self.outer.namespace.as_mut().unwrap();
                let compiler = t.get_compiler();
                let lhs_name = namespace.get_own_slot(compiler, &lhs_qname);
                let rhs_name = namespace.get_own_slot(compiler, &rhs_qname);
                if lhs_name.is_some_and(|l| {
                    l.calculate_inlinability(namespace, compiler)
                        .should_inline_usages()
                }) && rhs_name.is_some_and(|r| {
                    r.calculate_inlinability(namespace, compiler)
                        .should_inline_usages()
                }) {
                    self.outer.aliases.insert(lhs_qname, rhs_qname);
                }
            }
        }
    }

    // port: InlineAndCollapseProperties.InlineAliases.AliasesCollector#isDeclaredConst
    fn is_declared_const(t: &NodeTraversal<'_>, lhs: NodeId, info: Option<&JSDocInfo>) -> bool {
        if info.is_some_and(|info| info.has_const_annotation()) {
            return true;
        }
        lhs.get_parent(t).unwrap().is_const(t)
    }
}

struct AliasesInliner<'a> {
    outer: &'a mut InlineAliases,
}

/// Rust-only: `GlobalNamespace` as the `StaticScope` handed to `AstFactory#createQName`.
struct NamespaceStaticScope<'a>(RefCell<&'a mut GlobalNamespace>);

impl AstFactoryStaticScope<Compiler> for NamespaceStaticScope<'_> {
    fn get_slot_declaration_node(
        &self,
        cx: &mut Compiler,
        name: &JsString,
    ) -> Option<Option<NodeId>> {
        let mut namespace = self.0.borrow_mut();
        let slot = namespace.get_slot(cx, name)?;
        Some(
            slot.get_declaration(&namespace)
                .and_then(|declaration| declaration.get_node(&namespace)),
        )
    }
}

impl AbstractPostOrderCallbackInterface for AliasesInliner<'_> {
    // port: InlineAndCollapseProperties.InlineAliases.AliasesInliner#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow (switch case + if).
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::NAME | Token::GETPROP => {
                if n.is_qualified_name(t)
                    && self
                        .outer
                        .aliases
                        .contains_key(&n.get_qualified_name(t).unwrap())
                {
                    if Self::is_leftmost_name_local(t, n) {
                        // The alias is shadowed by a local variable. Don't rewrite.
                        return;
                    }
                    if NodeUtil::is_name_decl_or_simple_assign_lhs(t, n, parent.unwrap()) {
                        // The node defines an alias. Don't rewrite.
                        return;
                    }

                    let qname = n.get_qualified_name(t).unwrap();
                    let resolved = self.resolve_alias(t, qname, n);
                    let new_node = {
                        let InlineAliases {
                            namespace,
                            ast_factory,
                            ..
                        } = &mut *self.outer;
                        let scope = NamespaceStaticScope(RefCell::new(namespace.as_mut().unwrap()));
                        ast_factory.create_qname(
                            t.get_compiler(),
                            &scope,
                            &resolved.to_string_lossy(),
                        )
                    };
                    if Self::is_leftmost_name_local(t, new_node) {
                        // The aliased name is shadowed by a local variable. Don't rewrite.
                        return;
                    }

                    // If n is get_prop like "obj.foo" then newNode should use only location of
                    // foo, not obj.foo.
                    new_node.srcref_tree(t, n);
                    // Similarly if n is get_prop like "obj.foo" we should index only foo. obj
                    // should not be indexed as it's invisible to users.
                    if new_node.is_get_prop(t) {
                        new_node
                            .get_first_child(t)
                            .unwrap()
                            .make_non_indexable_recursive(t);
                    }
                    n.replace_with(t, new_node);
                    t.report_code_change();
                }
            }
            _ => {}
        }
    }
}

impl AliasesInliner<'_> {
    // port: InlineAndCollapseProperties.InlineAliases.AliasesInliner#isLeftmostNameLocal
    fn is_leftmost_name_local(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        check_state!(n.is_qualified_name(t));
        let leftmost_name = NodeUtil::get_root_of_qualified_name(t, n).get_string(t);
        let scope = t.get_scope();
        let compiler = t.get_compiler();
        let v = scope.get_var(compiler, &leftmost_name);
        v.is_some_and(|v| v.is_local(compiler))
    }

    /// Use the alias table to look up the resolved name of the given alias. If the result is also
    /// an alias repeat until the real name is resolved.
    // port: InlineAndCollapseProperties.InlineAliases.AliasesInliner#resolveAlias
    fn resolve_alias(&mut self, t: &mut NodeTraversal<'_>, name: JsString, n: NodeId) -> JsString {
        let mut name = name;
        let mut alias_path: IndexSet<JsString> = IndexSet::new();
        while self.outer.aliases.contains_key(&name) {
            if !alias_path.insert(name.clone()) {
                let path = format!(
                    "[{}]",
                    alias_path
                        .iter()
                        .map(JsString::to_string_lossy)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                let name_str = name.to_string_lossy();
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &ALIAS_CYCLE,
                    &[&path, &name_str],
                ));

                // Cut the cycle so that it doesn't get reported more than once.
                self.outer.aliases.shift_remove(&name);
                break;
            }

            name = self.outer.aliases.get(&name).unwrap().clone();
        }
        name
    }
}

/// Flattens global objects/namespaces by replacing each '.' with '$' in their names.
///
/// This reduces the number of property lookups the browser has to do and allows the
/// `RenameVars` pass to shorten namespaced names. For example, goog.events.handleEvent() ->
/// goog$events$handleEvent() -> Za().
///
/// If a global object's name is assigned to more than once, or if a property is added to the
/// global object in a complex expression, then none of its properties will be collapsed (for
/// safety/correctness).
///
/// If, after a global object is declared, it is never referenced except when its properties are
/// read or set, then the object will be removed after its properties have been collapsed.
///
/// Uninitialized variable stubs are created at a global object's declaration site for any of its
/// properties that are added late in a local scope.
///
/// Static properties of constructors are always collapsed, unsafely! For other objects: if,
/// after an object is declared, it is referenced directly in a way that might create an alias
/// for it, then none of its properties will be collapsed. This behavior is a safeguard to
/// prevent the values associated with the flattened names from getting out of sync with the
/// object's actual property values. For example, in the following case, an alias a$b, if
/// created, could easily keep the value 0 even after a.b became 5: ` a = {b: 0}; c = a; c.b =
/// 5; `.
///
/// This pass doesn't flatten property accesses of the form: a[b].
///
/// For lots of examples, see the unit test.
///
/// Rust: Java's inner class reads the outer instance's fields; `process` copies the ones it
/// reads into this struct and receives the shared `namespace` as `&mut GlobalNamespace`.
struct CollapseProperties {
    property_collapse_level: Option<PropertyCollapseLevel>,
    chunk_output_type: Option<ChunkOutputType>,
    have_modules_been_rewritten: bool,
    module_resolution_mode: Option<ResolutionMode>,
    decisions_log: Option<DecisionsLog>,

    /// Maps names (e.g. "a.b.c") to nodes in the global namespace tree
    ///
    /// Rust: a copy of the namespace's name index; collapsing adds no names to it.
    name_map: IndexMap<JsString, Name>,

    dynamically_imported_modules: IndexSet<JsString>,
}

impl CollapseProperties {
    fn new() -> Self {
        Self {
            property_collapse_level: None,
            chunk_output_type: None,
            have_modules_been_rewritten: false,
            module_resolution_mode: None,
            decisions_log: None,
            name_map: IndexMap::new(),
            dynamically_imported_modules: IndexSet::new(),
        }
    }

    // port: InlineAndCollapseProperties.CollapseProperties#process
    fn process(
        &mut self,
        outer: &mut InlineAndCollapseProperties,
        compiler: &mut AbstractCompiler,
        _externs: NodeId,
        root: NodeId,
    ) {
        self.property_collapse_level = outer.property_collapse_level;
        self.chunk_output_type = outer.chunk_output_type;
        self.have_modules_been_rewritten = outer.have_modules_been_rewritten;
        self.module_resolution_mode = outer.module_resolution_mode;
        self.decisions_log = outer.decisions_log.clone();

        if self.property_collapse_level == Some(PropertyCollapseLevel::MODULE_EXPORT)
            || self.chunk_output_type == Some(ChunkOutputType::ES_MODULES)
        {
            NodeTraversal::traverse(
                compiler,
                root,
                &mut crate::node_traversal::AbstractPostOrderCallback::new(
                    FindDynamicallyImportedModules::new(
                        self.have_modules_been_rewritten,
                        self.module_resolution_mode,
                        &mut self.dynamically_imported_modules,
                    ),
                ),
            );
        }

        let namespace = check_not_null!(outer.namespace.as_mut(), "namespace was not initialized");
        self.name_map = namespace.get_name_index(compiler).clone();
        let global_names: Vec<Name> = namespace.get_name_forest(compiler).to_vec();
        let escaped = self.check_namespaces(compiler, namespace);
        for name in global_names {
            let base_name = name.get_base_name(namespace);
            self.flatten_references_to_collapsible_descendant_names(
                compiler,
                namespace,
                name,
                base_name.clone(),
                &escaped,
            );
            // We collapse property definitions after collapsing property references
            // because this step can alter the parse tree above property references,
            // invalidating the node ancestry stored with each reference.
            self.collapse_declaration_of_name_and_descendants(
                compiler, namespace, name, base_name, &escaped,
            );
        }
    }

    // port: InlineAndCollapseProperties.CollapseProperties#canCollapse
    fn can_collapse(
        &self,
        compiler: &AbstractCompiler,
        namespace: &GlobalNamespace,
        name: Name,
    ) -> bool {
        let inlinability = name.can_collapse_or_inline(namespace, compiler);
        if !inlinability.can_collapse() {
            self.log_decision_for_name_inlinability(
                namespace,
                name,
                inlinability,
                "canCollapse() returns false",
            );
            return false;
        }

        if self.property_collapse_level == Some(PropertyCollapseLevel::MODULE_EXPORT) {
            if !name.is_module_export(namespace) {
                self.log_decision_for_name_inlinability(
                    namespace,
                    name,
                    inlinability,
                    "module export: canCollapse() returns false",
                );
                return false;
            } else if self
                .dynamically_imported_modules
                .contains(&name.get_base_name(namespace))
            {
                self.log_decision_for_name_inlinability(
                    namespace,
                    name,
                    inlinability,
                    "dynamic module export: canCollapse() returns false",
                );
                return false;
            }
        }

        self.log_decision_for_name_inlinability(
            namespace,
            name,
            inlinability,
            "canCollapse() returns true",
        );
        true
    }

    /// Returns true if it is safe to completely remove the rvalue assigned to the name from the
    /// AST.
    // port: InlineAndCollapseProperties.CollapseProperties#canEliminate
    fn can_eliminate(
        &self,
        compiler: &AbstractCompiler,
        namespace: &GlobalNamespace,
        name: Name,
        rvalue: NodeId,
    ) -> bool {
        if !name.can_eliminate(namespace, compiler) {
            return false;
        }

        if rvalue.is_object_lit(compiler) {
            let mut child = rvalue.get_first_child(compiler);
            while let Some(c) = child {
                match c.get_token(compiler) {
                    Token::GETTER_DEF
                    | Token::SETTER_DEF
                    | Token::COMPUTED_PROP
                    | Token::OBJECT_SPREAD => {
                        // We cannot completely eliminate the object literal, because it contains
                        // at least one feature that we cannot extract as a collapsed variable
                        // name.
                        return false;
                    }
                    Token::STRING_KEY | Token::MEMBER_FUNCTION_DEF => {}
                    _ => panic!(
                        "IllegalStateException: Unexpected child of OBJECTLIT: {}",
                        c.to_string_tree(compiler)
                    ),
                }
                child = c.get_next(compiler);
            }
        }
        if name.props(namespace).is_none_or(|props| props.is_empty())
            || self.property_collapse_level != Some(PropertyCollapseLevel::MODULE_EXPORT)
        {
            return true;
        }

        false
    }

    /// Runs through all namespaces (prefixes of classes and enums), and checks if any of them
    /// have been used in an unsafe way.
    // port: InlineAndCollapseProperties.CollapseProperties#checkNamespaces
    fn check_namespaces(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
    ) -> IndexSet<Name> {
        let mut escaped: IndexSet<Name> = IndexSet::new();
        let mut dynamically_imported_module_refs: IndexSet<JsString> =
            self.dynamically_imported_modules.clone();
        if !self.dynamically_imported_modules.is_empty() {
            // When the output chunk type is ES_MODULES, properties of the module namespace
            // must not be collapsed as they are referenced off the namespace object. The
            // namespace objects escape via the dynamic import expression.
            for &name in self.name_map.values() {
                // Test if the name is a rewritten module namespace variable and if so mark it as
                // escaped to prevent any property collapsing.
                //
                // example:
                // /** @const */ var module$foo = {};
                if self
                    .dynamically_imported_modules
                    .contains(&name.get_full_name(namespace))
                {
                    self.log_decision_for_name(
                        namespace,
                        name,
                        "escapes - dynamically imported module namespace",
                    );
                    escaped.insert(name);
                    let Some(props) = name.props(namespace) else {
                        continue;
                    };
                    for &prop in props {
                        let Some(prop_declaration) = prop.get_declaration(namespace) else {
                            continue;
                        };
                        if let Some(prop_declaration_node) = prop_declaration.get_node(namespace) {
                            // ES Module rewriting creates aliases on the module namespace object.
                            // These aliased names also escape and their properties may not be
                            // collapsed.
                            //
                            // example:
                            // class Foo$$module$foo {
                            //   static bar() { return 'bar'; }
                            // }
                            // /** @const */ var module$foo = {};
                            // /** @const */ module$foo.Foo = Foo$$module$foo;
                            //
                            // While module$foo.Foo cannot be collapsed because we marked the
                            // module namespace as escaped, we also need to prevent any property
                            // collapsing on the Foo$$module$foo class itself.
                            let r_value =
                                NodeUtil::get_r_value_of_l_value(compiler, prop_declaration_node)
                                    .unwrap();
                            if r_value.is_name(compiler) {
                                self.log_decision_for_name(
                                    namespace,
                                    name,
                                    "escapes - dynamically imported module namespace property alias",
                                );
                                dynamically_imported_module_refs
                                    .insert(r_value.get_qualified_name(compiler).unwrap());
                            }
                        }
                    }
                }
            }
        }

        for &name in self.name_map.values() {
            if !dynamically_imported_module_refs.is_empty()
                && dynamically_imported_module_refs.contains(&name.get_full_name(namespace))
            {
                escaped.insert(name);
            }
            if !name.is_namespace_object_lit(namespace) {
                continue;
            }
            if name.get_aliasing_gets(namespace) == 0
                && name.get_local_sets(namespace) + name.get_global_sets(namespace) <= 1
                && name.get_delete_props(namespace) == 0
            {
                continue;
            }
            let mut initialized = name.get_declaration(namespace).is_some();
            for r#ref in name.get_refs(namespace) {
                if r#ref.is_delete_prop(namespace) {
                    if initialized {
                        self.warn_about_namespace_redefinition(compiler, namespace, name, r#ref);
                    }
                } else if r#ref.is_set(namespace) && Some(r#ref) != name.get_declaration(namespace)
                {
                    if initialized
                        && !InlineAndCollapseProperties::is_safe_namespace_reinit(
                            compiler, namespace, r#ref,
                        )
                    {
                        self.warn_about_namespace_redefinition(compiler, namespace, name, r#ref);
                    }

                    initialized = true;
                } else if r#ref.is_aliasing_get(namespace) {
                    self.warn_about_namespace_aliasing(compiler, namespace, name, r#ref);
                    self.log_decision_for_name(namespace, name, "escapes");
                    escaped.insert(name);
                    break;
                }
            }
        }
        escaped
    }

    /// Reports a warning because a namespace was aliased.
    // port: InlineAndCollapseProperties.CollapseProperties#warnAboutNamespaceAliasing
    fn warn_about_namespace_aliasing(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        name_obj: Name,
        r#ref: Ref,
    ) {
        let full_name = name_obj.get_full_name(namespace).to_string_lossy();
        let error = JSError::make(
            compiler,
            r#ref.get_node(namespace).unwrap(),
            &PARTIAL_NAMESPACE_WARNING,
            &[&full_name],
        );
        compiler.report(error);
    }

    /// Reports a warning because a namespace was redefined.
    // port: InlineAndCollapseProperties.CollapseProperties#warnAboutNamespaceRedefinition
    fn warn_about_namespace_redefinition(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        name_obj: Name,
        r#ref: Ref,
    ) {
        let full_name = name_obj.get_full_name(namespace).to_string_lossy();
        let error = JSError::make(
            compiler,
            r#ref.get_node(namespace).unwrap(),
            &NAMESPACE_REDEFINED_WARNING,
            &[&full_name],
        );
        compiler.report(error);
    }

    /// Flattens all references to collapsible properties of a global name except their initial
    /// definitions. Recurs on subnames.
    ///
    /// `n`: An object representing a global name; `alias`: The flattened name for `n`
    // port: InlineAndCollapseProperties.CollapseProperties#flattenReferencesToCollapsibleDescendantNames
    fn flatten_references_to_collapsible_descendant_names(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        alias: JsString,
        escaped: &IndexSet<Name>,
    ) {
        let Some(props) = n.props(namespace).map(<[Name]>::to_vec) else {
            return;
        };
        if n.is_collapsing_explicitly_denied(namespace) {
            self.log_decision_for_name(
                namespace,
                n,
                "@nocollapse: will not flatten descendant name references",
            );
            return;
        }
        if escaped.contains(&n) {
            self.log_decision_for_name(
                namespace,
                n,
                "escapes: will not flatten descendant name references",
            );
            return;
        }

        for p in props {
            let prop_alias = self.append_prop_for_alias(&alias, p.get_base_name(namespace));
            let inlinability = p.can_collapse_or_inline(namespace, compiler);

            let is_allowed_to_collapse = self.property_collapse_level
                != Some(PropertyCollapseLevel::MODULE_EXPORT)
                || p.is_module_export(namespace);
            if is_allowed_to_collapse {
                if inlinability.can_collapse() {
                    self.log_decision_for_name_inlinability(
                        namespace,
                        p,
                        inlinability,
                        "will flatten references",
                    );
                    self.flatten_references_to(compiler, namespace, p, prop_alias.clone());
                } else if p.is_collapsing_explicitly_denied(namespace) {
                    self.log_decision_for_name(
                        namespace,
                        p,
                        "@nocollapse: will not flatten references",
                    );
                } else if p.is_simple_stub_declaration(namespace, compiler) {
                    self.log_decision_for_name(
                        namespace,
                        p,
                        "simple stub declaration: will flatten references",
                    );
                    self.flatten_simple_stub_declaration(
                        compiler,
                        namespace,
                        p,
                        prop_alias.clone(),
                    );
                } else {
                    self.log_decision_for_name_inlinability(
                        namespace,
                        p,
                        inlinability,
                        "will not flatten references",
                    );
                }
            }

            self.flatten_references_to_collapsible_descendant_names(
                compiler, namespace, p, prop_alias, escaped,
            );
        }
    }

    // port: InlineAndCollapseProperties.CollapseProperties#logDecisionForName(Name,Inlinability,String)
    fn log_decision_for_name_inlinability(
        &self,
        namespace: &GlobalNamespace,
        name: Name,
        inlinability: Inlinability,
        message: &str,
    ) {
        self.log_decision_for_name_supplier(namespace, name, || {
            format!("inlinability {inlinability}: {message}")
        });
    }

    // port: InlineAndCollapseProperties.CollapseProperties#logDecisionForName(Name,String)
    fn log_decision_for_name(&self, namespace: &GlobalNamespace, name: Name, message: &str) {
        self.log(&mut || format!("{}: {}", name.get_full_name(namespace), message));
    }

    // port: InlineAndCollapseProperties.CollapseProperties#logDecisionForName(Name,Supplier)
    fn log_decision_for_name_supplier(
        &self,
        namespace: &GlobalNamespace,
        name: Name,
        message_supplier: impl Fn() -> String,
    ) {
        self.log(&mut || format!("{}: {}", name.get_full_name(namespace), message_supplier()));
    }

    /// Rust-only: `decisionsLog.log(Supplier)`.
    fn log(&self, supplier: &mut dyn FnMut() -> String) {
        if let Some(decisions_log) = &self.decisions_log {
            decisions_log
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .log(supplier);
        }
    }

    /// Flattens a stub declaration. This is mostly a hack to support legacy users.
    // port: InlineAndCollapseProperties.CollapseProperties#flattenSimpleStubDeclaration
    fn flatten_simple_stub_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        name: Name,
        alias: JsString,
    ) {
        let refs = name.get_refs(namespace);
        check_argument!(
            refs.len() == 1,
            "expected one element but was: %s",
            refs.len()
        );
        let r#ref = refs[0];
        let ref_node = r#ref.get_node(namespace).unwrap();
        let name_node =
            NodeUtil::new_name_with_basis(compiler, alias, ref_node, name.get_full_name(namespace));
        let var_node = IR::var(compiler, name_node).srcref_if_missing(compiler, name_node);

        check_state!(
            ref_node
                .get_parent(compiler)
                .unwrap()
                .is_expr_result(compiler)
        );
        let parent = ref_node.get_parent(compiler).unwrap();
        parent.replace_with(compiler, var_node);
        compiler.report_change_to_enclosing_scope(var_node);
    }

    /// Returns whether a NAME node with the given `collapsed_name`, for a name originally
    /// declared at `declaration_node`, should be annotated with `Node#IS_CONSTANT_NAME`
    ///
    /// Validity checks in unit tests will otherwise throw an exception.
    // port: InlineAndCollapseProperties.CollapseProperties#shouldAddConstantName
    fn should_add_constant_name(
        &self,
        compiler: &AbstractCompiler,
        declaration_node: Option<NodeId>,
        collapsed_name: &JsString,
    ) -> bool {
        let Some(declaration_node) = declaration_node else {
            return false;
        };
        let info =
            NodeUtil::get_best_jsdoc_info(compiler, declaration_node.get_parent(compiler).unwrap());
        info.is_some_and(|info| info.is_constant())
            || declaration_node.get_boolean_prop(compiler, Prop::IS_CONSTANT_NAME)
            || compiler.get_coding_convention().is_constant(collapsed_name)
    }

    /// Flattens all references to a collapsible property of a global name except its initial
    /// definition.
    ///
    /// `n`: A global property name (e.g. "a.b" or "a.b.c.d"); `alias`: The flattened name (e.g.
    /// "a$b" or "a$b$c$d")
    // port: InlineAndCollapseProperties.CollapseProperties#flattenReferencesTo
    fn flatten_references_to(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        alias: JsString,
    ) {
        let original_name = n.get_full_name(namespace);
        let is_constant_name = self.should_add_constant_name(
            compiler,
            n.get_declaration(namespace)
                .and_then(|declaration| declaration.get_node(namespace)),
            &alias,
        );
        for r in n.get_refs(namespace) {
            if Some(r) == n.get_declaration(namespace) {
                // Declarations are handled separately.
                continue;
            }
            let r_node = r.get_node(namespace).unwrap();
            let r_parent = r_node.get_parent(compiler).unwrap();
            // We shouldn't flatten a reference that's an object literal key, because duplicate
            // keys show up as refs.
            if !NodeUtil::may_be_object_lit_key(compiler, r_node) {
                self.flatten_name_ref(
                    compiler,
                    alias.clone(),
                    r_node,
                    r_parent,
                    original_name.clone(),
                    is_constant_name,
                );
            } else if r_node.is_string_key(compiler)
                && r_node
                    .get_parent(compiler)
                    .unwrap()
                    .is_object_pattern(compiler)
            {
                let new_node = IR::name(compiler, alias.clone()).srcref(compiler, r_node);
                NodeUtil::copy_name_annotations(compiler, r_node, new_node);
                DestructuringGlobalNameExtractor::reassign_destructring_lvalue(
                    r_node, new_node, None, r, compiler,
                );
            }
        }

        // Flatten all occurrences of a name as a prefix of its subnames. For
        // example, if `n` corresponds to the name "a.b", then "a.b" will be
        // replaced with "a$b" in all occurrences of "a.b.c", "a.b.c.d", etc.
        if let Some(props) = n.props(namespace).map(<[Name]>::to_vec) {
            for p in props {
                self.flatten_prefixes(
                    compiler,
                    namespace,
                    alias.clone(),
                    original_name.concat(&p.get_base_name(namespace)),
                    p,
                    1,
                    is_constant_name,
                );
            }
        }
    }

    /// Flattens all occurrences of a name as a prefix of subnames beginning with a particular
    /// subname.
    ///
    /// `n`: A global property name (e.g. "a.b.c.d"); `alias`: A flattened prefix name (e.g.
    /// "a$b"); `original_name`: The full original name of the global property (e.g. "a.b.c.d")
    /// equivalent to n.getFullName(), but pre-computed to save on intermediate string allocation;
    /// `depth`: The difference in depth between the property name and the prefix name (e.g. 2)
    // port: InlineAndCollapseProperties.CollapseProperties#flattenPrefixes
    #[allow(clippy::too_many_arguments)]
    fn flatten_prefixes(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        alias: JsString,
        original_name: JsString,
        n: Name,
        depth: i32,
        is_constant_name: bool,
    ) {
        // Only flatten the prefix of a name declaration if the name being
        // initialized is fully qualified (i.e. not an object literal key).
        let decl = n.get_declaration(namespace);
        if let Some(decl) = decl
            && let Some(decl_node) = decl.get_node(namespace)
            && decl_node.is_get_prop(compiler)
        {
            self.flatten_name_ref_at_depth(
                compiler,
                alias.clone(),
                decl_node,
                depth,
                original_name.clone(),
                is_constant_name,
            );
        }

        for r in n.get_refs(namespace) {
            if Some(r) == decl {
                // Declarations are handled separately.
                continue;
            }

            self.flatten_name_ref_at_depth(
                compiler,
                alias.clone(),
                r.get_node(namespace).unwrap(),
                depth,
                original_name.clone(),
                is_constant_name,
            );
        }

        if let Some(props) = n.props(namespace) {
            for &p in props {
                self.flatten_prefixes(
                    compiler,
                    namespace,
                    alias.clone(),
                    original_name.concat(&p.get_base_name(namespace)),
                    p,
                    depth + 1,
                    is_constant_name,
                );
            }
        }
    }

    /// Flattens a particular prefix of a single name reference.
    ///
    /// `alias`: A flattened prefix name (e.g. "a$b"); `n`: The node corresponding to a
    /// subproperty name (e.g. "a.b.c.d"); `depth`: The difference in depth between the property
    /// name and the prefix name (e.g. 2); `original_name`: String version of the property name.
    // port: InlineAndCollapseProperties.CollapseProperties#flattenNameRefAtDepth
    fn flatten_name_ref_at_depth(
        &self,
        compiler: &mut AbstractCompiler,
        alias: JsString,
        n: NodeId,
        depth: i32,
        original_name: JsString,
        is_constant_name: bool,
    ) {
        let mut n = n;
        // This method has to work for both GETPROP chains and, in rare cases,
        // OBJLIT keys, possibly nested. That's why we check for children before
        // proceeding. In the OBJLIT case, we don't need to do anything.
        let n_type = n.get_token(compiler);
        let is_q_name = n_type == Token::NAME || n_type == Token::GETPROP;
        let is_obj_key = NodeUtil::may_be_object_lit_key(compiler, n);
        check_state!(is_obj_key || is_q_name);
        if is_q_name {
            let mut i = 1;
            while i < depth && n.has_children(compiler) {
                n = n.get_first_child(compiler).unwrap();
                i += 1;
            }
            if n.is_get_prop(compiler) && n.get_first_child(compiler).unwrap().is_get_prop(compiler)
            {
                let first_child = n.get_first_child(compiler).unwrap();
                self.flatten_name_ref(
                    compiler,
                    alias,
                    first_child,
                    n,
                    original_name,
                    is_constant_name,
                );
            }
        }
    }

    /// Replaces a GETPROP a.b.c with a NAME a$b$c.
    ///
    /// `alias`: A flattened prefix name (e.g. "a$b"); `n`: The GETPROP node corresponding to the
    /// original name (e.g. "a.b"); `parent`: `n`'s parent; `original_name`: String version of
    /// the property name; `is_constant_name`: whether to annotate with `Node#IS_CONSTANT_NAME`
    // port: InlineAndCollapseProperties.CollapseProperties#flattenNameRef
    fn flatten_name_ref(
        &self,
        compiler: &mut AbstractCompiler,
        alias: JsString,
        n: NodeId,
        parent: NodeId,
        original_name: JsString,
        is_constant_name: bool,
    ) {
        check_argument!(
            n.is_get_prop(compiler),
            "Expected GETPROP, found %s. Node: %s",
            n.get_token(compiler),
            n.to_string(compiler)
        );

        // BEFORE:
        //   getprop
        //     getprop
        //       name a
        //       string b
        //     string c
        // AFTER:
        //   name a$b$c
        let r#ref = NodeUtil::new_name_with_basis(compiler, alias, n, original_name)
            .copy_type_from(compiler, n);
        NodeUtil::copy_name_annotations(compiler, n, r#ref);
        if is_constant_name {
            r#ref.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, is_constant_name);
        }
        if NodeUtil::is_normal_or_opt_chain_call(compiler, parent)
            && n.is_first_child_of(compiler, Some(parent))
        {
            // The node was a call target. We are deliberately flattening these as
            // the "this" isn't provided by the namespace. Mark it as such:
            parent.put_boolean_prop(compiler, Prop::FREE_CALL, true);
        }

        n.replace_with(compiler, r#ref);
        compiler.report_change_to_enclosing_scope(r#ref);
    }

    /// Collapses definitions of the collapsible properties of a global name. Recurs on subnames
    /// that also represent JavaScript objects with collapsible properties.
    ///
    /// `n`: A node representing a global name; `alias`: The flattened name for `n`
    // port: InlineAndCollapseProperties.CollapseProperties#collapseDeclarationOfNameAndDescendants
    fn collapse_declaration_of_name_and_descendants(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        alias: JsString,
        escaped: &IndexSet<Name>,
    ) {
        let child_name_inlinability = n.can_collapse_or_inline_child_names(namespace, compiler);
        let can_collapse_child_names = if !child_name_inlinability.can_collapse() {
            self.log_decision_for_name_supplier(namespace, n, || {
                format!(
                    "child name inlinability: {child_name_inlinability}: will not collapse child names"
                )
            });
            false
        } else if escaped.contains(&n) {
            self.log_decision_for_name(namespace, n, "escapes: will not collapse child names");
            false
        } else {
            true
        };

        // Handle this name first so that nested object literals get unrolled.
        if self.can_collapse(compiler, namespace, n) {
            self.log_decision_for_name(namespace, n, "collapsing");
            self.update_global_name_declaration(
                compiler,
                namespace,
                n,
                alias.clone(),
                can_collapse_child_names,
            );
        }

        if n.props(namespace).is_none() || escaped.contains(&n) {
            return;
        }
        self.log_decision_for_name(namespace, n, "collapsing descendants");
        let props: Vec<Name> = n.props(namespace).unwrap().to_vec();
        for p in props {
            let prop_alias = self.append_prop_for_alias(&alias, p.get_base_name(namespace));
            self.collapse_declaration_of_name_and_descendants(
                compiler, namespace, p, prop_alias, escaped,
            );
        }
    }

    /// Updates the initial assignment to a collapsible property at global scope by adding a VAR
    /// stub and collapsing the property. e.g. c = a.b = 1; => var a$b; c = a$b = 1; This
    /// specifically handles "twinned" assignments, which are those where the assignment is also
    /// used as a reference and which need special handling.
    ///
    /// `alias`: The flattened property name (e.g. "a$b"); `ref_name`: The name for the reference
    /// being updated; `ref`: An object containing information about the assignment getting
    /// updated
    // port: InlineAndCollapseProperties.CollapseProperties#updateTwinnedDeclaration
    fn update_twinned_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        alias: JsString,
        ref_name: Name,
        r#ref: Ref,
    ) {
        check_state!(
            r#ref.is_twin(namespace),
            "%s",
            r#ref.to_string(namespace, compiler)
        );
        let ref_node = r#ref.get_node(namespace).unwrap();
        // Don't handle declarations of an already flat name, just qualified names.
        if !ref_node.is_get_prop(compiler) {
            return;
        }
        let rvalue = ref_node.get_next(compiler);
        let parent = ref_node.get_parent(compiler).unwrap();
        let grandparent = parent.get_parent(compiler).unwrap();

        if let Some(rvalue) = rvalue
            && rvalue.is_function(compiler)
        {
            self.check_for_receiver_affected_by_collapse(
                compiler,
                namespace,
                rvalue,
                ref_name.get_jsdoc_info(namespace),
                ref_name,
            );
        }

        // Create the new alias node.
        let grandparent_first_child = grandparent.get_first_child(compiler).unwrap();
        let name_node = NodeUtil::new_name_with_basis(
            compiler,
            alias,
            grandparent_first_child,
            ref_name.get_full_name(namespace),
        );
        NodeUtil::copy_name_annotations(compiler, ref_node, name_node);

        // BEFORE:
        // ... (x.y = 3);
        //
        // AFTER:
        // var x$y;
        // ... (x$y = 3);

        let mut current = grandparent;
        let mut current_parent = grandparent.get_parent(compiler).unwrap();
        while !current_parent.is_script(compiler) && !current_parent.is_block(compiler) {
            current = current_parent;
            current_parent = current_parent.get_parent(compiler).unwrap();
        }

        // Create a stub variable declaration right
        // before the current statement.
        let cloned_name = name_node.clone_tree(compiler);
        let stub_var = IR::var(compiler, cloned_name).srcref_if_missing(compiler, name_node);
        stub_var.insert_before(compiler, current);

        ref_node.replace_with(compiler, name_node);
        compiler.report_change_to_enclosing_scope(name_node);
    }

    /// Updates the first initialization (a.k.a "declaration") of a global name. This involves
    /// flattening the global name (if it's not just a global variable name already), collapsing
    /// object literal keys into global variables, declaring stub global variables for properties
    /// added later in a local scope.
    ///
    /// It may seem odd that this function also takes care of declaring stubs for direct
    /// children. The ultimate goal of this function is to eliminate the global name entirely
    /// (when possible), so that "middlemen" namespaces disappear, and to do that we need to make
    /// sure that all the direct children will be collapsed as well.
    ///
    /// `n`: An object representing a global name (e.g. "a", "a.b.c"); `alias`: The flattened name
    /// for `n` (e.g. "a", "a$b$c"); `can_collapse_child_names`: Whether it's possible to collapse
    /// children of this name. (This is mostly passed for convenience; it's equivalent to
    /// n.canCollapseChildNames()).
    // port: InlineAndCollapseProperties.CollapseProperties#updateGlobalNameDeclaration
    fn update_global_name_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        alias: JsString,
        can_collapse_child_names: bool,
    ) {
        let Some(decl) = n.get_declaration(namespace) else {
            // Some names do not have declarations, because they
            // are only defined in local scopes.
            self.log_decision_for_name(namespace, n, "no global declaration found");
            return;
        };

        let is_simple_name = n.is_simple_name(namespace);
        let decl_node = decl.get_node(namespace).unwrap();
        match decl_node.get_parent(compiler).unwrap().get_token(compiler) {
            Token::ASSIGN => {
                self.log_declaration_action(
                    compiler,
                    namespace,
                    n,
                    decl_node,
                    "updating assignment",
                );
                self.update_global_name_declaration_at_assign_node(
                    compiler,
                    namespace,
                    n,
                    alias,
                    can_collapse_child_names,
                );
            }
            Token::VAR | Token::LET | Token::CONST => {
                self.log_declaration_action(
                    compiler,
                    namespace,
                    n,
                    decl_node,
                    "updating variable declaration",
                );
                self.update_global_name_declaration_at_variable_node(
                    compiler,
                    namespace,
                    n,
                    can_collapse_child_names,
                );
            }
            Token::FUNCTION => {
                self.log_declaration_action(
                    compiler,
                    namespace,
                    n,
                    decl_node,
                    "updating function declaration",
                );
                self.update_global_name_declaration_at_function_node(
                    compiler,
                    namespace,
                    n,
                    can_collapse_child_names,
                );
            }
            Token::CLASS => {
                self.log_declaration_action(
                    compiler,
                    namespace,
                    n,
                    decl_node,
                    "updating class declaration",
                );
                self.update_global_name_declaration_at_class_node(
                    compiler,
                    namespace,
                    n,
                    can_collapse_child_names,
                );
            }
            Token::CLASS_MEMBERS => {
                self.log_declaration_action(
                    compiler,
                    namespace,
                    n,
                    decl_node,
                    "updating static member declaration",
                );
                self.update_global_name_declaration_at_static_member_node(
                    compiler,
                    namespace,
                    n,
                    alias,
                    can_collapse_child_names,
                );
            }
            _ => self.log_declaration_action(
                compiler,
                namespace,
                n,
                decl_node,
                "not updating an unsupported type of declaration node",
            ),
        }

        if !is_simple_name {
            let common_ancestor =
                n.get_deepest_common_ancestor_chunk(namespace, compiler.get_chunk_graph().unwrap());
            if common_ancestor != decl.get_chunk(namespace) {
                self.move_declaration_into_higher_chunk(
                    compiler,
                    namespace,
                    decl,
                    common_ancestor.unwrap(),
                );
            }
        }
    }

    // port: InlineAndCollapseProperties.CollapseProperties#logDeclarationAction
    fn log_declaration_action(
        &self,
        compiler: &AbstractCompiler,
        namespace: &GlobalNamespace,
        name: Name,
        declaration_node: NodeId,
        message: &str,
    ) {
        self.log_decision_for_name_supplier(namespace, name, || {
            format!("{}: {}", declaration_node.to_string(compiler), message)
        });
    }

    // port: InlineAndCollapseProperties.CollapseProperties#moveDeclarationIntoHigherChunk
    fn move_declaration_into_higher_chunk(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        decl: Ref,
        common_ancestor: JSChunk,
    ) {
        // Var hoisting means that early references are generally ok. E.g. `a$b; var a$b;` is
        // valid. However, if `var a$b;` were in a different, conditionally loaded chunk, then it
        // might not be parsed before `a$b;`. So we need to avoid a ReferenceError when
        // referencing `a$b`.
        let new_node = decl.get_node(namespace).unwrap();
        let var_node = new_node.get_parent(compiler).unwrap();
        let initializer = new_node.get_first_child(compiler);
        check_state!(
            var_node.is_var(compiler),
            "Unexpected declaration %s",
            var_node.to_string(compiler)
        );

        let new_root = common_ancestor.get_inputs()[0].get_ast_root(compiler);
        match initializer {
            None => {
                // Already a stub declaration - "var a$b;". Move it to the ancestor chunk.
                let detached = var_node.detach(compiler);
                new_root.add_child_to_front(compiler, detached);
            }
            Some(initializer) => {
                // Define `var a$b;` in the ancestor chunk.
                let cloned = new_node.clone_node(compiler);
                let new_var_node = IR::var(compiler, cloned).srcref(compiler, new_node);
                new_root.add_child_to_front(compiler, new_var_node);
                // Replace `var a$b = () => {};` with `a$b = () = {};`. Having multiple `var`
                // declarations of the same name is valid JavaScript but violates Normalize
                // constraints.
                let detached_name = new_node.detach(compiler);
                let detached_initializer = initializer.detach(compiler);
                let assign = IR::assign(compiler, detached_name, detached_initializer);
                let new_assign =
                    IR::expr_result(compiler, assign).srcref_tree_if_missing(compiler, new_node);
                var_node.replace_with(compiler, new_assign);
            }
        }
        compiler.report_change_to_change_scope(new_root);
    }

    /// Updates the first initialization (a.k.a "declaration") of a global name that occurs at an
    /// ASSIGN node. See comment for `update_global_name_declaration`.
    ///
    /// `n`: An object representing a global name (e.g. "a", "a.b.c"); `alias`: The flattened name
    /// for `n` (e.g. "a", "a$b$c")
    // port: InlineAndCollapseProperties.CollapseProperties#updateGlobalNameDeclarationAtAssignNode
    fn update_global_name_declaration_at_assign_node(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        alias: JsString,
        can_collapse_child_names: bool,
    ) {
        // NOTE: It's important that we don't add additional nodes
        // (e.g. a var node before the exprstmt) because the exprstmt might be
        // the child of an if statement that's not inside a block).

        // All qualified names - even for variables that are initially declared as LETS and
        // CONSTS - are being declared as VAR statements, but this is not incorrect because
        // we are only collapsing for global names.
        let r#ref = n.get_declaration(namespace).unwrap();
        let ref_node = r#ref.get_node(namespace).unwrap();
        let rvalue = ref_node.get_next(compiler);
        if r#ref.is_twin(namespace) {
            self.update_twinned_declaration(compiler, namespace, alias, n, r#ref);
            return;
        }
        let rvalue = rvalue.unwrap();
        let var_node = compiler.new_node(Token::VAR);
        let var_parent = ref_node.get_ancestor(compiler, 3).unwrap();
        let grandparent = ref_node.get_ancestor(compiler, 2).unwrap();
        let is_obj_lit = rvalue.is_object_lit(compiler);
        let mut inserted_var_node = false;

        if is_obj_lit && self.can_eliminate(compiler, namespace, n, rvalue) {
            // Eliminate the object literal altogether.
            grandparent.replace_with(compiler, var_node);
            n.update_ref_node(namespace, compiler, r#ref, None);
            inserted_var_node = true;
            compiler.report_change_to_enclosing_scope(var_node);
        } else if !n.is_simple_name(namespace) {
            // Create a VAR node to declare the name.
            if rvalue.is_function(compiler) {
                self.check_for_receiver_affected_by_collapse(
                    compiler,
                    namespace,
                    rvalue,
                    n.get_jsdoc_info(namespace),
                    n,
                );
            }

            compiler.report_change_to_enclosing_scope(rvalue);
            rvalue.detach(compiler);

            let basis_node = ref_node.get_ancestor(compiler, 2).unwrap();
            let name_node = NodeUtil::new_name_with_basis(
                compiler,
                alias.clone(),
                basis_node,
                n.get_full_name(namespace),
            );

            let is_constant_name = self.should_add_constant_name(compiler, Some(ref_node), &alias);
            name_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, is_constant_name);

            let info =
                NodeUtil::get_best_jsdoc_info(compiler, ref_node.get_parent(compiler).unwrap());
            if info.is_some() {
                var_node.set_jsdoc_info(compiler, info);
            }
            var_node.add_child_to_back(compiler, name_node);
            name_node.add_child_to_front(compiler, rvalue);
            grandparent.replace_with(compiler, var_node);

            // Update the node ancestry stored in the reference.
            n.update_ref_node(namespace, compiler, r#ref, Some(name_node));
            inserted_var_node = true;
            compiler.report_change_to_enclosing_scope(var_node);
        }

        if can_collapse_child_names {
            if is_obj_lit {
                let name_to_add_after = var_node.get_previous(compiler);
                self.declare_variables_for_obj_lit_values(
                    compiler,
                    namespace,
                    n,
                    alias.clone(),
                    rvalue,
                    var_node,
                    name_to_add_after,
                );
            }

            self.add_stubs_for_undeclared_properties(
                compiler, namespace, n, alias, var_parent, var_node,
            );
        }

        if inserted_var_node && !var_node.has_children(compiler) {
            var_node.detach(compiler);
        }
    }

    /// Warns about any references to "this" in the given FUNCTION. The function is getting
    /// collapsed, so the references will change.
    // port: InlineAndCollapseProperties.CollapseProperties#checkForReceiverAffectedByCollapse
    fn check_for_receiver_affected_by_collapse(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        function: NodeId,
        doc_info: Option<Arc<JSDocInfo>>,
        name: Name,
    ) {
        check_state!(function.is_function(compiler));

        if let Some(doc_info) = doc_info {
            // Don't rely on type inference for this check.

            if doc_info.is_constructor_or_interface() {
                return; // Ctors and interfaces need to be able to reference `this`
            }
            if doc_info.has_this_type() {
                // Use `@this` as a signal that the reference is intentional.
                //
                // TODO(b/156823102): This signal also silences the check on all transpiled
                // static methods.
                return;
            }
        }

        // Use the NodeUtil method so we don't forget to update this logic.
        if NodeUtil::references_own_receiver(compiler, function) {
            let full_name = name.get_full_name(namespace).to_string_lossy();
            let error = JSError::make(
                compiler,
                function,
                &RECEIVER_AFFECTED_BY_COLLAPSE,
                &[&full_name],
            );
            compiler.report(error);
        }
    }

    /// Updates the first initialization (a.k.a "declaration") of a global name that occurs at a
    /// VAR node. See comment for `update_global_name_declaration`.
    ///
    /// `n`: An object representing a global name (e.g. "a")
    // port: InlineAndCollapseProperties.CollapseProperties#updateGlobalNameDeclarationAtVariableNode
    fn update_global_name_declaration_at_variable_node(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        can_collapse_child_names: bool,
    ) {
        if !can_collapse_child_names {
            self.log_decision_for_name(namespace, n, "cannot collapse child names: skipping");
            return;
        }

        let r#ref = n.get_declaration(namespace).unwrap();
        let ref_node = r#ref.get_node(namespace).unwrap();
        let name = ref_node.get_string(compiler);
        let rvalue = ref_node.get_first_child(compiler).unwrap();
        let variable_node = ref_node.get_parent(compiler).unwrap();
        let grandparent = variable_node.get_parent(compiler).unwrap();

        let is_obj_lit = rvalue.is_object_lit(compiler);

        if is_obj_lit {
            let name_to_add_after = variable_node.get_previous(compiler);
            self.declare_variables_for_obj_lit_values(
                compiler,
                namespace,
                n,
                name.clone(),
                rvalue,
                variable_node,
                name_to_add_after,
            );
        }

        self.add_stubs_for_undeclared_properties(
            compiler,
            namespace,
            n,
            name,
            grandparent,
            variable_node,
        );

        if is_obj_lit && self.can_eliminate(compiler, namespace, n, rvalue) {
            ref_node.detach(compiler);
            compiler.report_change_to_enclosing_scope(variable_node);
            if !variable_node.has_children(compiler) {
                variable_node.detach(compiler);
            }

            // Clear out the object reference, since we've eliminated it from the
            // parse tree.
            n.update_ref_node(namespace, compiler, r#ref, None);
        }
    }

    /// Updates the first initialization (a.k.a "declaration") of a global name that occurs at a
    /// FUNCTION node. See comment for `update_global_name_declaration`.
    ///
    /// `n`: An object representing a global name (e.g. "a")
    // port: InlineAndCollapseProperties.CollapseProperties#updateGlobalNameDeclarationAtFunctionNode
    fn update_global_name_declaration_at_function_node(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        can_collapse_child_names: bool,
    ) {
        if !can_collapse_child_names || !self.can_collapse(compiler, namespace, n) {
            return;
        }

        let r#ref = n.get_declaration(namespace).unwrap();
        let ref_node = r#ref.get_node(namespace).unwrap();
        let fn_name = ref_node.get_string(compiler);
        let parent = ref_node.get_ancestor(compiler, 2).unwrap();
        let add_after = ref_node.get_parent(compiler).unwrap();
        self.add_stubs_for_undeclared_properties(
            compiler, namespace, n, fn_name, parent, add_after,
        );
    }

    /// Updates the first initialization (a.k.a "declaration") of a global name that occurs at a
    /// CLASS node. See comment for `update_global_name_declaration`.
    ///
    /// `n`: An object representing a global name (e.g. "a")
    // port: InlineAndCollapseProperties.CollapseProperties#updateGlobalNameDeclarationAtClassNode
    fn update_global_name_declaration_at_class_node(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        can_collapse_child_names: bool,
    ) {
        if !can_collapse_child_names || !self.can_collapse(compiler, namespace, n) {
            return;
        }

        let r#ref = n.get_declaration(namespace).unwrap();
        let ref_node = r#ref.get_node(namespace).unwrap();
        let class_name = ref_node.get_string(compiler);
        let parent = ref_node.get_ancestor(compiler, 2).unwrap();
        let add_after = ref_node.get_parent(compiler).unwrap();
        self.add_stubs_for_undeclared_properties(
            compiler, namespace, n, class_name, parent, add_after,
        );
    }

    /// Updates the first initialization (a.k.a "declaration") of a global name that occurs in a
    /// static MEMBER_FUNCTION_DEF or MEMBER_FIELD_DEF in a class. See comment for
    /// `update_global_name_declaration`.
    ///
    /// `n`: A static MEMBER_FUNCTION_DEF or MEMBER_FIELD_DEF in a class assigned to a global name
    /// (e.g. `a.b`); `alias`: The new flattened name for `n` (e.g. "a$b");
    /// `can_collapse_child_names`: whether properties of `n` are also collapsible, meaning that
    /// any properties only assigned locally need stub declarations
    // port: InlineAndCollapseProperties.CollapseProperties#updateGlobalNameDeclarationAtStaticMemberNode
    fn update_global_name_declaration_at_static_member_node(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        n: Name,
        alias: JsString,
        can_collapse_child_names: bool,
    ) {
        let declaration = n.get_declaration(namespace).unwrap();
        let declaration_node = declaration.get_node(namespace).unwrap();
        let class_node = declaration_node.get_grandparent(compiler).unwrap();
        check_state!(
            class_node.is_class(compiler),
            "%s",
            class_node.to_string(compiler)
        );
        let enclosing_statement = NodeUtil::get_enclosing_statement(compiler, class_node).unwrap();

        if can_collapse_child_names {
            let parent = enclosing_statement.get_parent(compiler).unwrap();
            self.add_stubs_for_undeclared_properties(
                compiler,
                namespace,
                n,
                alias.clone(),
                parent,
                class_node,
            );
        }

        // detach `static m() {}` from `class Foo { static m() {} }`
        let member = declaration_node.detach(compiler);
        let var_decl = if member.has_children(compiler) {
            let member_val = member.get_only_child(compiler);
            let member_jsdoc = member.get_jsdoc_info(compiler);
            self.check_for_receiver_affected_by_collapse(
                compiler,
                namespace,
                member_val,
                member_jsdoc,
                n,
            );
            let member_val = member_val.detach(compiler);

            // add a var declaration, creating `var Foo$m = function() {}; class Foo {}`
            let name_node = NodeUtil::new_name_with_srcref(compiler, alias, member);
            IR::var_with_value(compiler, name_node, member_val).srcref(compiler, member)
        } else {
            // Member field defs may not have initializers.
            let name_node = NodeUtil::new_name_with_srcref(compiler, alias, member);
            IR::var(compiler, name_node).srcref(compiler, member)
        };
        var_decl.insert_before(compiler, enclosing_statement);
        // We would lose optimization-relevant jsdoc tags here because they are stored on the
        // class member node, not the function node. Copy them over to the new declaration
        // statement so later passes can make use of them.
        let member_jsdoc = member.get_jsdoc_info(compiler);
        var_decl.set_jsdoc_info(compiler, member_jsdoc);
        compiler.report_change_to_enclosing_scope(var_decl);

        // collapsing this name's properties requires updating this Ref
        let first_child = var_decl.get_first_child(compiler);
        n.update_ref_node(namespace, compiler, declaration, first_child);
    }

    /// Declares global variables to serve as aliases for the values in an object literal,
    /// optionally removing all of the object literal's keys and values.
    ///
    /// `alias`: The object literal's flattened name (e.g. "a$b$c"); `objlit`: The OBJLIT node;
    /// `var_node`: The VAR node to which new global variables should be added as children;
    /// `name_to_add_after`: The child of `var_node` after which new variables should be added
    /// (may be null)
    // port: InlineAndCollapseProperties.CollapseProperties#declareVariablesForObjLitValues
    #[allow(clippy::too_many_arguments)]
    fn declare_variables_for_obj_lit_values(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &mut GlobalNamespace,
        objlit_name: Name,
        alias: JsString,
        objlit: NodeId,
        var_node: NodeId,
        name_to_add_after: Option<NodeId>,
    ) {
        let mut name_to_add_after = name_to_add_after;
        let mut arbitrary_name_counter = 0;
        let discard_keys = !objlit_name.should_keep_keys(namespace);

        let mut next_key = objlit.get_first_child(compiler);
        while let Some(key) = next_key {
            let value = key.get_first_child(compiler);
            next_key = key.get_next(compiler);

            // A computed property, or a get or a set can not be rewritten as a VAR. We don't know
            // what properties will be generated by a spread.
            match key.get_token(compiler) {
                Token::GETTER_DEF
                | Token::SETTER_DEF
                | Token::COMPUTED_PROP
                | Token::OBJECT_SPREAD => {
                    continue;
                }
                Token::STRING_KEY | Token::MEMBER_FUNCTION_DEF => {}
                _ => panic!(
                    "IllegalStateException: Unexpected child of OBJECTLIT: {}",
                    key.to_string_tree(compiler)
                ),
            }
            let value = value.unwrap();

            // We generate arbitrary names for keys that aren't valid JavaScript
            // identifiers, since those keys are never referenced. (If they were,
            // this object literal's child names wouldn't be collapsible.) The only
            // reason that we don't eliminate them entirely is the off chance that
            // their values are expressions that have side effects.
            let is_js_identifier = !key.is_number(compiler)
                && TokenStream::is_js_identifier(&key.get_string(compiler));
            let prop_name = if is_js_identifier {
                key.get_string(compiler)
            } else {
                arbitrary_name_counter += 1;
                JsString::from(arbitrary_name_counter.to_string())
            };

            // If the name cannot be collapsed, skip it.
            let q_name = objlit_name
                .get_full_name(namespace)
                .concat(&JsString::from("."))
                .concat(&prop_name);
            let p = self.name_map.get(&q_name).copied();
            if let Some(p) = p
                && !self.can_collapse(compiler, namespace, p)
            {
                continue;
            }

            let prop_alias = self.append_prop_for_alias(&alias, prop_name);
            let mut ref_node = None;
            let is_constant_name = self.should_add_constant_name(compiler, Some(key), &prop_alias);
            if discard_keys {
                key.detach(compiler);
                value.detach(compiler);
                // Don't report a change here because the objlit has already been removed from
                // the tree.
            } else {
                // Substitute a reference for the value.
                let new_ref_node = IR::name(compiler, prop_alias.clone());
                if is_constant_name {
                    new_ref_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
                }

                value.replace_with(compiler, new_ref_node);
                compiler.report_change_to_enclosing_scope(new_ref_node);
                ref_node = Some(new_ref_node);
            }

            // Declare the collapsed name as a variable with the original value.
            let name_node = IR::name(compiler, prop_alias);
            name_node.add_child_to_front(compiler, value);
            if is_constant_name {
                name_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
            }
            let new_var = IR::var(compiler, name_node).srcref_tree_if_missing(compiler, key);
            if let Some(name_to_add_after) = name_to_add_after {
                new_var.insert_after(compiler, name_to_add_after);
            } else {
                new_var.insert_before(compiler, var_node);
            }
            compiler.report_change_to_enclosing_scope(new_var);
            name_to_add_after = Some(new_var);

            // Update the global name's node ancestry if it hasn't already been
            // done. (Duplicate keys in an object literal can bring us here twice
            // for the same global name.)
            if is_js_identifier && let Some(p) = p {
                if !discard_keys {
                    p.add_aliasing_get_cloned_from_declaration(
                        namespace,
                        compiler,
                        ref_node.unwrap(),
                    );
                }

                let p_declaration = p.get_declaration(namespace).unwrap();
                p.update_ref_node(namespace, compiler, p_declaration, Some(name_node));

                if value.is_function(compiler) {
                    let key_jsdoc = key.get_jsdoc_info(compiler);
                    self.check_for_receiver_affected_by_collapse(
                        compiler, namespace, value, key_jsdoc, p,
                    );
                }
            }
        }
    }

    /// Adds global variable "stubs" for any properties of a global name that are only set in a
    /// local scope or read but never set.
    ///
    /// `n`: An object representing a global name (e.g. "a", "a.b.c"); `alias`: The flattened name
    /// of the object whose properties we are adding stubs for (e.g. "a$b$c"); `parent`: The node
    /// to which new global variables should be added as children; `add_after`: The child of
    /// after which new variables should be added
    // port: InlineAndCollapseProperties.CollapseProperties#addStubsForUndeclaredProperties
    fn add_stubs_for_undeclared_properties(
        &self,
        compiler: &mut AbstractCompiler,
        namespace: &GlobalNamespace,
        n: Name,
        alias: JsString,
        parent: NodeId,
        add_after: NodeId,
    ) {
        check_state!(
            n.can_collapse_unannotated_child_names(namespace, compiler),
            "%s",
            n.to_string(namespace)
        );
        check_argument!(
            NodeUtil::is_statement_block(compiler, parent),
            "%s",
            parent.to_string(compiler)
        );
        let mut add_after = add_after;
        let Some(props) = n.props(namespace) else {
            return;
        };
        for &p in props {
            if !p.needs_to_be_stubbed(namespace) {
                continue;
            }

            let prop_alias = self.append_prop_for_alias(&alias, p.get_base_name(namespace));
            let name_node = IR::name(compiler, prop_alias.clone());
            let new_var = IR::var(compiler, name_node).srcref_tree_if_missing(compiler, add_after);
            new_var.insert_after(compiler, add_after);

            // Determine if this is a constant var by checking the first
            // reference to it. Don't check the declaration, as it might be null.
            let const_prop_node = p.get_first_ref(namespace).get_node(namespace).unwrap();
            let is_constant_name =
                // Don't call shouldAddConstantName because it expects to see a declaration node
                // for a name, and this name is, by definition, undeclared.
                const_prop_node.get_boolean_prop(compiler, Prop::IS_CONSTANT_NAME)
                    || compiler.get_coding_convention().is_constant(&prop_alias);
            if is_constant_name {
                name_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
            }

            compiler.report_change_to_enclosing_scope(new_var);
            add_after = new_var;
        }
    }

    // port: InlineAndCollapseProperties.CollapseProperties#appendPropForAlias
    fn append_prop_for_alias(&self, root: &JsString, prop: JsString) -> JsString {
        let mut prop = prop;
        if prop.index_of_char(u16::from(b'$')) != -1 {
            // Encode '$' in a property as '$0'. Because '0' cannot be the
            // start of an identifier, this will never conflict with our
            // encoding from '.' -> '$'.
            prop = prop.replace(&JsString::from("$"), &JsString::from("$0"));
        }
        let dollar = JsString::from("$");
        let mut result = root.concat(&dollar).concat(&prop);
        let mut id = 1;
        while self.name_map.contains_key(&result) {
            result = root
                .concat(&dollar)
                .concat(&prop)
                .concat(&dollar)
                .concat(&JsString::from(id.to_string()));
            id += 1;
        }
        result
    }
}

/// Find all the module namespace objects which are referenced by a dynamic import
struct FindDynamicallyImportedModules<'a> {
    process_common_js_modules: bool,
    module_resolution_mode: Option<ResolutionMode>,
    /// Rust: the outer `CollapseProperties#dynamicallyImportedModules` this inner class adds to.
    dynamically_imported_modules: &'a mut IndexSet<JsString>,
}

impl<'a> FindDynamicallyImportedModules<'a> {
    // port: InlineAndCollapseProperties.CollapseProperties.FindDynamicallyImportedModules#FindDynamicallyImportedModules
    fn new(
        process_common_js_modules: bool,
        resolution_mode: Option<ResolutionMode>,
        dynamically_imported_modules: &'a mut IndexSet<JsString>,
    ) -> Self {
        Self {
            process_common_js_modules,
            module_resolution_mode: resolution_mode,
            dynamically_imported_modules,
        }
    }
}

impl AbstractPostOrderCallbackInterface for FindDynamicallyImportedModules<'_> {
    // port: InlineAndCollapseProperties.CollapseProperties.FindDynamicallyImportedModules#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        // After rewriting, CommonJS dynamic imports are of the form
        // __webpack_require__.e(2).then(function() { return module$mod1.default; })
        //
        // Mark the base module name as being dynamically imported.
        if self.process_common_js_modules
            && n.is_get_prop(t)
            && n.is_qualified_name(t)
            && n.get_parent(t).is_some()
            && n.get_parent(t).unwrap().is_return(t)
            && n.get_grandparent(t).unwrap().is_block(t)
            && n.get_grandparent(t).unwrap().has_one_child(t)
            && n.get_grandparent(t)
                .unwrap()
                .get_parent(t)
                .unwrap()
                .is_function(t)
        {
            let potential_callback = NodeUtil::get_enclosing_function(t, n);
            // Java passes the (nullable) resolution mode through; a null mode is not WEBPACK.
            if let Some(potential_callback) = potential_callback
                && self
                    .module_resolution_mode
                    .is_some_and(|module_resolution_mode| {
                        ProcessCommonJSModules::is_common_js_dynamic_import_callback(
                            t,
                            NodeUtil::get_enclosing_function(t, potential_callback),
                            module_resolution_mode,
                        )
                    })
            {
                let q_name = n.get_qualified_name(t).unwrap();
                self.dynamically_imported_modules
                    .insert(NodeUtil::get_root_of_qualified_name_string(&q_name));
            }
        } else if ConvertChunksToESModules::is_dynamic_import_callback(t, n) {
            let module_namespace =
                ConvertChunksToESModules::get_dynamic_import_callback_module_namespace(
                    t.get_compiler(),
                    n,
                );
            if let Some(module_namespace) = module_namespace {
                let q_name = module_namespace.get_qualified_name(t).unwrap();
                self.dynamically_imported_modules.insert(q_name);
            }
        }
    }
}
