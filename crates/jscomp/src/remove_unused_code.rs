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
/*
 * Copyright (C) 2007 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/RemoveUnusedCode.java.
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/collect/AbstractMapBasedMultimap.java,
//   com/google/common/collect/HashMultimap.java.

//! Port of `RemoveUnusedCode.java`.
//!
//! Garbage collection for variable and function definitions. Basically performs a mark-and-sweep
//! type algorithm over the JavaScript parse tree.
//!
//! For each scope: (1) Scan the variable/function declarations at that scope. (2) Traverse the
//! scope for references, marking all referenced variables. Unlike other compiler passes, this is a
//! pre-order traversal, not a post-order traversal. (3) If the traversal encounters an assign
//! without other side-effects, create a continuation. Continue the continuation iff the assigned
//! variable is referenced. (4) When the traversal completes, remove all unreferenced variables.
//!
//! Java's inner `Removable`, `VarInfo` and `PolyfillInfo` objects have identity and mutable state
//! shared between several collections, so they live in arenas owned by the pass and are referred
//! to by `RemovableId`, `VarInfoId` and `PolyfillInfoId` handles (handle equality is Java object
//! identity). Each Java subclass is a variant of the kind enum; each overridden method is a
//! function carrying the subclass's `// port:` marker, and the base-class method dispatches on the
//! kind.
//!
//! Java `HashMap` iteration order (docs/PORTING.md §8): `removablesForPropertyNames` (iterated by
//! `keySet()` in `removeIndependentlyRemovableProperties`) and `polyfills` (iterated by `values()`
//! in `removeUnreferencedVarsAndPolyfills`) are Guava `HashMultimap<String, ...>`s, whose key order
//! is the order of the backing `java.util.HashMap` (`HashMultimap.create()` backs it with a
//! `HashMap` of table length 16, checked against the reference jar). `JavaHashMultimap` below
//! simulates that table exactly (`putVal`, `removeNode`, `resize`, `treeifyBin` for tables under
//! 64 buckets). The values of one key are a Guava `HashSet` keyed by identity hash codes, whose
//! order Java does not fix; they are kept in insertion order.
use crate::{
    abstract_compiler::AbstractCompiler,
    accessor_summary::PropertyAccessKind,
    ast_analyzer::AstAnalyzer,
    compiler_pass::CompilerPass,
    diagnostic::log_file::LogFile,
    node_util::NodeUtil,
    polyfill_usage_finder::{PolyfillUsage, PolyfillUsageFinder, Polyfills},
    scope::ScopeId,
    syntactic_scope_creator::SyntacticScopeCreator,
    var::VarId,
};
use closure_resources::resources::resource_loader::ResourceLoader;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::{collections::VecDeque, sync::Arc};

/// Properties that are implicitly used as part of the JS language.
const IMPLICITLY_USED_PROPERTIES: [&str; 5] =
    ["length", "toString", "valueOf", "constructor", "prototype"];

const DOT_PROTOTYPE: &str = ".prototype";

/// Handle of a `Removable` in `RemoveUnusedCode::removables` (Java object identity).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct RemovableId(usize);

/// Handle of a `VarInfo` in `RemoveUnusedCode::var_infos` (Java object identity).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct VarInfoId(usize);

/// Handle of a `PolyfillInfo` in `RemoveUnusedCode::polyfill_infos` (Java object identity).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct PolyfillInfoId(usize);

// port: RemoveUnusedCode
pub struct RemoveUnusedCode {
    ast_analyzer: AstAnalyzer,

    remove_local_vars: bool,
    remove_globals: bool,

    preserve_function_expression_names: bool,

    /// Used to hold continuations that need to be invoked.
    ///
    /// When we find a subtree of the AST that may not need to be traversed, we create a
    /// Continuation for it. If we later discover that we do need to traverse it, we add it to this
    /// worklist rather than traversing it immediately. If we invoked the traversal immediately, we
    /// could end up modifying a data structure in the traversal as we're iterating over it.
    worklist: VecDeque<Continuation>,

    var_info_map: IndexMap<VarId, VarInfoId>,

    pinned_property_names: IndexSet<JsString>,

    /// Stores Removable objects for each property name that is currently considered removable.
    removables_for_property_names: JavaHashMultimap<RemovableId>,

    /// Single value to use for all vars for which we cannot remove anything at all.
    canonical_unremovable_var_info: VarInfoId,

    /// Keep track of scopes that we've traversed.
    all_function_param_scopes: Vec<ScopeId>,

    /// Stores the names of all "leaf" properties that are polyfilled, to avoid unnecessary
    /// qualified name matching and searches for all the other properties. This includes global
    /// names such as "Promise" and "Map", static methods on global names such as "Array.from" and
    /// "Math.fround", and instance properties such as "String.prototype.repeat" and
    /// "Promise.prototype.finally".
    polyfills: JavaHashMultimap<PolyfillInfoId>,

    guarded_usages: IndexSet<NodeId>,

    polyfills_from_table: Arc<Polyfills>,

    scope_creator: SyntacticScopeCreator<'static>,

    remove_unused_prototype_properties: bool,
    remove_unused_this_properties: bool,
    remove_unused_object_define_properties_definitions: bool,
    remove_unused_polyfills: bool,
    assume_getters_are_pure: bool,

    // Allocated & cleaned up by process()
    removal_log: Option<Box<dyn LogFile>>,
    unremovable_log: Option<Box<dyn LogFile>>,

    /// Arena of the Java `Removable` objects.
    removables: Vec<Removable>,
    /// Arena of the Java `VarInfo` objects.
    var_infos: Vec<VarInfo>,
    /// Arena of the Java `PolyfillInfo` objects.
    polyfill_infos: Vec<PolyfillInfo>,
}

impl RemoveUnusedCode {
    // port: RemoveUnusedCode#RemoveUnusedCode
    fn new(builder: Builder<'_>) -> Self {
        let compiler = builder.compiler;
        let ast_analyzer = compiler.get_ast_analyzer();
        let scope_creator = SyntacticScopeCreator::new();
        let polyfills_from_table =
            Arc::new(Polyfills::from_table(&ResourceLoader::load_text_resource(
                "com.google.javascript.jscomp.RemoveUnusedCode",
                "js/polyfills.txt",
            )));
        let mut this = Self {
            ast_analyzer,
            remove_local_vars: builder.remove_local_vars,
            remove_globals: builder.remove_globals,
            preserve_function_expression_names: builder.preserve_function_expression_names,
            worklist: VecDeque::new(),
            var_info_map: IndexMap::<_, _>::default(),
            pinned_property_names: IMPLICITLY_USED_PROPERTIES
                .iter()
                .map(|s| JsString::from(*s))
                .collect(),
            removables_for_property_names: JavaHashMultimap::create(),
            canonical_unremovable_var_info: VarInfoId(0),
            all_function_param_scopes: Vec::new(),
            polyfills: JavaHashMultimap::create(),
            guarded_usages: IndexSet::<_>::default(),
            polyfills_from_table,
            scope_creator,
            remove_unused_prototype_properties: builder.remove_unused_prototype_properties,
            remove_unused_this_properties: builder.remove_unused_this_properties,
            remove_unused_object_define_properties_definitions: builder
                .remove_unused_object_define_properties_definitions,
            remove_unused_polyfills: builder.remove_unused_polyfills,
            assume_getters_are_pure: builder.assume_getters_are_pure,
            removal_log: None,
            unremovable_log: None,
            removables: Vec::new(),
            var_infos: Vec::new(),
            polyfill_infos: Vec::new(),
        };
        // All Vars that are completely unremovable will share this VarInfo instance.
        this.canonical_unremovable_var_info = this.new_var_info(VarInfo::CanonicalUnremovable);
        this
    }

    // port: RemoveUnusedCode.Builder#Builder
    pub fn builder(compiler: &AbstractCompiler) -> Builder<'_> {
        Builder::new(compiler)
    }
}

// port: RemoveUnusedCode.Builder
pub struct Builder<'a> {
    compiler: &'a AbstractCompiler,

    remove_local_vars: bool,
    remove_globals: bool,
    preserve_function_expression_names: bool,
    remove_unused_prototype_properties: bool,
    remove_unused_this_properties: bool,
    remove_unused_object_define_properties_definitions: bool,
    remove_unused_polyfills: bool,
    assume_getters_are_pure: bool,
}

impl<'a> Builder<'a> {
    // port: RemoveUnusedCode.Builder#Builder
    pub fn new(compiler: &'a AbstractCompiler) -> Self {
        Self {
            compiler,
            remove_local_vars: false,
            remove_globals: false,
            preserve_function_expression_names: false,
            remove_unused_prototype_properties: false,
            remove_unused_this_properties: false,
            remove_unused_object_define_properties_definitions: false,
            remove_unused_polyfills: false,
            assume_getters_are_pure: false,
        }
    }

    // port: RemoveUnusedCode.Builder#removeLocalVars
    pub fn remove_local_vars(mut self, value: bool) -> Self {
        self.remove_local_vars = value;
        self
    }

    // port: RemoveUnusedCode.Builder#removeGlobals
    pub fn remove_globals(mut self, value: bool) -> Self {
        self.remove_globals = value;
        self
    }

    // port: RemoveUnusedCode.Builder#preserveFunctionExpressionNames
    pub fn preserve_function_expression_names(mut self, value: bool) -> Self {
        self.preserve_function_expression_names = value;
        self
    }

    // port: RemoveUnusedCode.Builder#removeUnusedPrototypeProperties
    pub fn remove_unused_prototype_properties(mut self, value: bool) -> Self {
        self.remove_unused_prototype_properties = value;
        self
    }

    // port: RemoveUnusedCode.Builder#removeUnusedThisProperties
    pub fn remove_unused_this_properties(mut self, value: bool) -> Self {
        self.remove_unused_this_properties = value;
        self
    }

    // port: RemoveUnusedCode.Builder#removeUnusedObjectDefinePropertiesDefinitions
    pub fn remove_unused_object_define_properties_definitions(mut self, value: bool) -> Self {
        self.remove_unused_object_define_properties_definitions = value;
        self
    }

    // port: RemoveUnusedCode.Builder#removeUnusedPolyfills
    pub fn remove_unused_polyfills(mut self, value: bool) -> Self {
        self.remove_unused_polyfills = value;
        self
    }

    // port: RemoveUnusedCode.Builder#assumeGettersArePure
    pub fn assume_getters_are_pure(mut self, value: bool) -> Self {
        self.assume_getters_are_pure = value;
        self
    }

    // port: RemoveUnusedCode.Builder#build
    pub fn build(self) -> RemoveUnusedCode {
        RemoveUnusedCode::new(self)
    }
}

/// A Java `Supplier<String>` of `RemovalLogRecord`; it reads the AST when the log evaluates it.
type NameSupplier = Box<dyn Fn(&Ast) -> String>;

/// Supplies the string needed for an entry in the removal log.
// port: RemoveUnusedCode.RemovalLogRecord
struct RemovalLogRecord {
    kind: &'static str,
    name_supplier: NameSupplier,
    function_name_supplier: NameSupplier,
}

impl RemovalLogRecord {
    /// Returns a log entry string.
    ///
    /// Each entry is one tab-separated line of the form:
    ///
    /// ```text
    ///   KIND NAME [FUNCTION_NAME]
    /// ```
    // port: RemoveUnusedCode.RemovalLogRecord#get
    fn get(&self, ast: &Ast) -> String {
        [
            self.kind.to_string(),
            (self.name_supplier)(ast),
            (self.function_name_supplier)(ast),
        ]
        .join("\t")
    }

    // port: RemoveUnusedCode.RemovalLogRecord#RemovalLogRecord(String,Supplier,Supplier)
    fn new_with_function_name(
        kind: &'static str,
        name_supplier: NameSupplier,
        function_name_supplier: NameSupplier,
    ) -> Self {
        Self {
            kind,
            name_supplier,
            function_name_supplier,
        }
    }

    // port: RemoveUnusedCode.RemovalLogRecord#RemovalLogRecord(String,Supplier)
    fn new(kind: &'static str, name_supplier: NameSupplier) -> Self {
        // No function name
        Self::new_with_function_name(kind, name_supplier, Box::new(|_| String::new()))
    }

    // port: RemoveUnusedCode.RemovalLogRecord#forProperty
    fn for_property(prop_name: JsString) -> Self {
        Self::new("prop", Box::new(move |_| prop_name.to_string_lossy()))
    }

    // port: RemoveUnusedCode.RemovalLogRecord#forVar
    fn for_var(var_name: JsString) -> Self {
        Self::new("var", Box::new(move |_| var_name.to_string_lossy()))
    }

    // port: RemoveUnusedCode.RemovalLogRecord#forPolyfill
    fn for_polyfill(polyfill_name: String) -> Self {
        Self::new("poly", Box::new(move |_| polyfill_name.clone()))
    }

    /// Records removal of a named function parameter.
    // port: RemoveUnusedCode.RemovalLogRecord#forNamedArg
    fn for_named_arg(name_node: NodeId, arg_list: NodeId) -> Self {
        Self::new_with_function_name(
            "arg",
            Box::new(move |ast| name_node.get_string(ast).to_string_lossy()),
            Self::get_loggable_function_name_supplier(arg_list),
        )
    }

    /// Records removal of a destructuring function parameter.
    // port: RemoveUnusedCode.RemovalLogRecord#forDestructuringArg
    fn for_destructuring_arg(arg_list: NodeId) -> Self {
        Self::new_with_function_name(
            "arg",
            Box::new(|_| "<pattern>".to_string()),
            Self::get_loggable_function_name_supplier(arg_list),
        )
    }

    /// Records that a named parameter is marked as unused for possible removal by
    /// `OptimizeParameters`.
    // port: RemoveUnusedCode.RemovalLogRecord#forMarkingNamedArg
    fn for_marking_named_arg(name_node: NodeId, arg_list: NodeId) -> Self {
        Self::new_with_function_name(
            "argmark",
            Box::new(move |ast| name_node.get_string(ast).to_string_lossy()),
            Self::get_loggable_function_name_supplier(arg_list),
        )
    }

    /// Returns a supplier for the FUNCTION_NAME field of an argument removal log entry.
    ///
    /// If no good name can be found, then `"<anonymous>"` will be supplied.
    // port: RemoveUnusedCode.RemovalLogRecord#getLoggableFunctionNameSupplier
    fn get_loggable_function_name_supplier(arg_list: NodeId) -> NameSupplier {
        Box::new(move |ast| {
            let function_name =
                NodeUtil::get_nearest_function_name(ast, arg_list.get_parent(ast).unwrap());
            match function_name {
                None => "<anonymous>".to_string(),
                Some(function_name) => function_name.to_string_lossy(),
            }
        })
    }
}

impl CompilerPass for RemoveUnusedCode {
    /// Traverses the root, removing all unused variables. Multiple traversals may occur to ensure
    /// all unused variables are removed.
    // port: RemoveUnusedCode#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        let extern_properties = compiler
            .get_extern_properties_js()
            .expect("compiler.getExternProperties() is null");
        self.pinned_property_names
            .extend(extern_properties.iter().cloned());

        let removal_log_file =
            compiler.create_or_reopen_indexed_log("RemoveUnusedCode", "removals.log", &[]);
        let keep_log_file =
            compiler.create_or_reopen_indexed_log("RemoveUnusedCode", "unremovable.log", &[]);
        self.removal_log = Some(removal_log_file); // avoid passing the log file through a bunch of methods
        self.unremovable_log = Some(keep_log_file);
        self.traverse_and_remove_unused_references(compiler, root);
        // try-with-resources closes the resources in reverse order, then `finally` clears both
        if let Some(mut keep_log_file) = self.unremovable_log.take() {
            keep_log_file.close();
        }
        if let Some(mut removal_log_file) = self.removal_log.take() {
            removal_log_file.close();
        }
    }
}

impl RemoveUnusedCode {
    /// Traverses a node recursively. Call this once per pass.
    // port: RemoveUnusedCode#traverseAndRemoveUnusedReferences
    fn traverse_and_remove_unused_references(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) {
        // Create scope from parent of root node, which also has externs as a child, so we'll
        // have extern definitions in scope.
        let root_parent = root.get_parent(compiler).unwrap();
        let scope = self.scope_creator.create_scope(compiler, root_parent, None);
        if !scope.has_slot(compiler, NodeUtil::JSC_PROPERTY_NAME_FN) {
            // TODO(b/70730762): Passes that add references to this should ensure it is declared.
            // NOTE: null input makes this an extern var.
            scope.declare(
                compiler,
                NodeUtil::JSC_PROPERTY_NAME_FN,
                /* no declaration node */ None,
                /* no input */ None,
            );
        }

        // Accumulate guarded usages of polyfills before removal starts.
        let mut guarded = Vec::new();
        PolyfillUsageFinder::new(Arc::clone(&self.polyfills_from_table)).traverse_only_guarded(
            compiler,
            root,
            &mut |_compiler: &mut AbstractCompiler, polyfill_usage: PolyfillUsage| {
                guarded.push(polyfill_usage)
            },
        );
        for polyfill_usage in guarded {
            self.store_polyfill(polyfill_usage);
        }

        self.worklist.push_back(Continuation::new(root, scope));
        while let Some(continuation) = self.worklist.pop_front() {
            self.continuation_apply(compiler, continuation);
        }

        self.remove_unreferenced_vars_and_polyfills(compiler);
        self.remove_independently_removable_properties(compiler);
        for fparam_scope in self.all_function_param_scopes.clone() {
            self.remove_unreferenced_function_args(compiler, fparam_scope);
        }
    }

    // port: RemoveUnusedCode#storePolyfill
    fn store_polyfill(&mut self, polyfill_usage: PolyfillUsage) {
        self.guarded_usages.insert(polyfill_usage.node());
    }

    // port: RemoveUnusedCode#removeIndependentlyRemovableProperties
    fn remove_independently_removable_properties(&mut self, compiler: &mut AbstractCompiler) {
        for prop_name in self.removables_for_property_names.key_set() {
            let record = RemovalLogRecord::for_property(prop_name.clone());
            self.removal_log
                .as_mut()
                .unwrap()
                .log(&mut || record.get(compiler));
            for removable in self.removables_for_property_names.get(&prop_name) {
                self.removable_remove(compiler, removable);
            }
        }
    }

    /// Traverses everything in the current scope and marks variables that are referenced.
    ///
    /// During traversal, we identify subtrees that will only be referenced if their enclosing
    /// variables are referenced. Instead of traversing those subtrees, we create a continuation
    /// for them, and traverse them lazily.
    // port: RemoveUnusedCode#traverseNode
    fn traverse_node(&mut self, compiler: &mut AbstractCompiler, n: NodeId, scope: ScopeId) {
        let parent = n.get_parent(compiler);
        let r#type = n.get_token(compiler);
        match r#type {
            Token::CATCH => self.traverse_catch(compiler, n, scope),
            Token::FUNCTION => {
                {
                    // If this function is a removable var, then create a continuation
                    // for it instead of traversing immediately.
                    if NodeUtil::is_function_declaration(compiler, n) {
                        let var_info = self.traverse_name_node(
                            compiler,
                            n.get_first_child(compiler).unwrap(),
                            scope,
                        );
                        let function_declaration = RemovableBuilder::new()
                            .add_continuation(Continuation::new(n, scope))
                            .build_function_declaration(self, compiler, n);
                        self.var_info_add_removable(compiler, var_info, function_declaration);
                        if parent.unwrap().is_export(compiler) {
                            self.var_info_set_is_explicitly_not_removable(
                                compiler,
                                var_info,
                                |_| "exported class".to_string(),
                            );
                        }
                    } else {
                        self.traverse_function(compiler, n, scope);
                    }
                }
            }
            Token::ASSIGN => self.traverse_assign(compiler, n, scope),
            Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_EXPONENT
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD => self.traverse_compound_assign(compiler, n, scope),
            Token::INC | Token::DEC => self.traverse_increment_or_decrement_op(compiler, n, scope),
            Token::CALL | Token::OPTCHAIN_CALL => self.traverse_call(compiler, n, scope),
            Token::SWITCH_BODY | Token::BLOCK => {
                // This case if for if there are let and const variables in block scopes.
                // Otherwise other variables will be hoisted up into the global scope and already
                // be handled.
                let child_scope = if NodeUtil::creates_block_scope(compiler, n) {
                    self.scope_creator.create_scope(compiler, n, Some(scope))
                } else {
                    scope
                };
                self.traverse_children(compiler, n, child_scope)
            }
            Token::MODULE_BODY => {
                let module_scope = self.scope_creator.create_scope(compiler, n, Some(scope));
                self.traverse_children(compiler, n, module_scope)
            }
            Token::CLASS => self.traverse_class(compiler, n, scope),
            Token::CLASS_MEMBERS => self.traverse_class_members(compiler, n, scope),
            Token::ARRAY_PATTERN | Token::PARAM_LIST => {
                self.traverse_indirect_assignment_list(compiler, n, scope)
            }
            Token::OBJECT_PATTERN => self.traverse_object_pattern(compiler, n, scope),
            Token::OBJECTLIT => self.traverse_object_literal(compiler, n, scope),
            Token::FOR => self.traverse_vanilla_for(compiler, n, scope),
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                self.traverse_enhanced_for(compiler, n, scope)
            }
            Token::LET | Token::CONST | Token::VAR => {
                // for-loop cases are handled by custom traversal methods.
                check_state!(NodeUtil::is_statement(compiler, n));
                self.traverse_declaration_statement(compiler, n, scope);
            }
            Token::INSTANCEOF => self.traverse_instanceof(compiler, n, scope),
            Token::NAME => {
                // The only cases that should reach this point are parameter declarations and
                // references to names. The name node does not have children in these cases.
                check_state!(!n.has_children(compiler));
                let parent = parent.unwrap();
                // the parameter declaration is not a read of the name
                if !parent.is_param_list(compiler) {
                    // var|let|const name;
                    // are handled at a higher level.
                    check_state!(!NodeUtil::is_name_declaration(compiler, Some(parent)));
                    // function name() {}
                    // class name() {}
                    // handled at a higher level
                    check_state!(
                        !((parent.is_function(compiler) || parent.is_class(compiler))
                            && parent.get_first_child(compiler) == Some(n))
                    );
                    let var_info = self.traverse_name_node(compiler, n, scope);
                    self.var_info_set_is_explicitly_not_removable(compiler, var_info, |ast| {
                        format!("reference found: {}", n.get_location(ast))
                    });
                }
            }
            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                self.traverse_normal_or_opt_chain_get_prop(compiler, n, scope)
            }
            _ => self.traverse_children(compiler, n, scope),
        }
    }

    // port: RemoveUnusedCode#traverseInstanceof
    fn traverse_instanceof(
        &mut self,
        compiler: &mut AbstractCompiler,
        instanceof_node: NodeId,
        scope: ScopeId,
    ) {
        check_argument!(
            instanceof_node.is_instance_of(compiler),
            "%s",
            instanceof_node.to_string(compiler)
        );
        let lhs = instanceof_node.get_first_child(compiler).unwrap();
        let rhs = lhs.get_next(compiler).unwrap();
        self.traverse_node(compiler, lhs, scope);
        if rhs.is_name(compiler) {
            let var_info = self.traverse_name_node(compiler, rhs, scope);
            let builder = RemovableBuilder::new();
            let removable = builder.build_instanceof_name(self, compiler, instanceof_node);
            self.var_info_add_removable(compiler, var_info, removable);
        } else {
            self.traverse_node(compiler, rhs, scope);
        }
    }

    /// Traverse `expr.prop` or `expr?.prop`.
    ///
    /// Note that this method is called only for RHS nodes. Property references that are being
    /// assigned to are handled by the logic traversing their parent (e.g. ASSIGN) node.
    ///
    /// The primary purpose of this method is to make sure the property reference is correctly
    /// recorded.
    // port: RemoveUnusedCode#traverseNormalOrOptChainGetProp
    fn traverse_normal_or_opt_chain_get_prop(
        &mut self,
        compiler: &mut AbstractCompiler,
        get_prop: NodeId,
        scope: ScopeId,
    ) {
        check_state!(
            NodeUtil::is_normal_or_opt_chain_get_prop(compiler, get_prop),
            "%s",
            get_prop.to_string(compiler)
        );
        let object_node = get_prop.get_first_child(compiler).unwrap();
        let property_name = get_prop.get_string(compiler);

        if self.polyfills.contains_key(&property_name) {
            for info in self.polyfills.get(&property_name) {
                if self.polyfill_infos[info.0].is_removable {
                    self.polyfill_info_consider_possible_reference(compiler, info, get_prop);
                }
            }
        }

        if NodeUtil::is_expression_result_used(compiler, get_prop)
            || self.consider_for_accessor_side_effects(
                compiler,
                get_prop,
                PropertyAccessKind::GETTER_ONLY,
            )
        {
            // must record as reference to the property and continue traversal.
            self.mark_property_name_as_pinned(compiler, property_name);
            self.traverse_node(compiler, object_node, scope);
        } else if object_node.is_this(compiler) {
            // This is probably the declaration of a class field in a constructor.
            // /** @private {number} */
            // this.propName;
            // We don't want to consider this a real usage that should prevent removal.
            let builder = RemovableBuilder::new().set_is_this_dot_property_reference(true);
            let removable = builder.build_unused_read_reference(self, compiler, get_prop, get_prop);
            self.consider_for_independent_removal(compiler, removable);
        } else if is_dot_prototype(compiler, object_node) {
            // (objExpression).prototype.propName;
            let mut builder = RemovableBuilder::new().set_is_prototype_dot_property_reference(true);
            let obj_expression = object_node.get_first_child(compiler).unwrap();
            if obj_expression.is_name(compiler) {
                // name.prototype.propName;
                let var_info = self.traverse_name_node(compiler, obj_expression, scope);
                let removable =
                    builder.build_unused_read_reference(self, compiler, get_prop, get_prop);
                self.var_info_add_removable(compiler, var_info, removable);
            } else {
                // (objExpression).prototype.propName;
                if self.may_have_side_effects(compiler, obj_expression) {
                    self.traverse_node(compiler, obj_expression, scope);
                } else {
                    builder = builder.add_continuation(Continuation::new(obj_expression, scope));
                }
                let removable =
                    builder.build_unused_read_reference(self, compiler, get_prop, get_prop);
                self.consider_for_independent_removal(compiler, removable);
            }
        } else {
            // TODO(bradfordcsmith): add removal of `varName.propName;`
            self.mark_property_name_as_pinned(compiler, property_name);
            self.traverse_node(compiler, object_node, scope);
        }
    }

    // TODO(b/137380742): Combine with `traverseCompoundAssign`.
    // port: RemoveUnusedCode#traverseIncrementOrDecrementOp
    fn traverse_increment_or_decrement_op(
        &mut self,
        compiler: &mut AbstractCompiler,
        inc_or_dec_op: NodeId,
        scope: ScopeId,
    ) {
        check_argument!(
            inc_or_dec_op.is_inc(compiler) || inc_or_dec_op.is_dec(compiler),
            "%s",
            inc_or_dec_op.to_string(compiler)
        );
        let arg = inc_or_dec_op.get_only_child(compiler);
        if NodeUtil::is_expression_result_used(compiler, inc_or_dec_op) {
            // If expression result is used, then this expression is definitely not removable.
            self.traverse_node(compiler, arg, scope);
        } else if arg.is_get_prop(compiler) {
            let get_prop_obj = arg.get_first_child(compiler).unwrap();

            if self.consider_for_accessor_side_effects(
                compiler,
                arg,
                PropertyAccessKind::GETTER_AND_SETTER,
            ) {
                self.traverse_node(compiler, get_prop_obj, scope); // Don't re-traverse the GETPROP as a read.
            } else if get_prop_obj.is_this(compiler) {
                // this.propName++
                let builder = RemovableBuilder::new().set_is_this_dot_property_reference(true);
                let removable =
                    builder.build_inc_or_dep_op(self, compiler, inc_or_dec_op, arg, None);
                self.consider_for_independent_removal(compiler, removable);
            } else if is_dot_prototype(compiler, get_prop_obj) {
                // someExpression.prototype.propName++
                let expr_obj = get_prop_obj.get_first_child(compiler).unwrap();
                let mut builder =
                    RemovableBuilder::new().set_is_prototype_dot_property_reference(true);
                if expr_obj.is_name(compiler) {
                    // varName.prototype.propName++
                    let var_info = self.traverse_name_node(compiler, expr_obj, scope);
                    let removable =
                        builder.build_inc_or_dep_op(self, compiler, inc_or_dec_op, arg, None);
                    self.var_info_add_removable(compiler, var_info, removable);
                } else {
                    // (someExpression).prototype.propName++
                    let mut to_preserve = None;
                    if self.may_have_side_effects(compiler, expr_obj) {
                        to_preserve = Some(expr_obj);
                        self.traverse_node(compiler, expr_obj, scope);
                    } else {
                        builder = builder.add_continuation(Continuation::new(expr_obj, scope));
                    }
                    let removable = builder.build_inc_or_dep_op(
                        self,
                        compiler,
                        inc_or_dec_op,
                        arg,
                        to_preserve,
                    );
                    self.consider_for_independent_removal(compiler, removable);
                }
            } else {
                // someExpression.propName++ is not removable except in the cases covered above
                self.traverse_node(compiler, arg, scope);
            }
        } else {
            // TODO(bradfordcsmith): varName++ should be removable if varName is otherwise unused
            self.traverse_node(compiler, arg, scope);
        }
    }

    // TODO(b/137380742): Combine with `traverseIncrementOrDecrement`.
    // port: RemoveUnusedCode#traverseCompoundAssign
    fn traverse_compound_assign(
        &mut self,
        compiler: &mut AbstractCompiler,
        compound_assign_node: NodeId,
        scope: ScopeId,
    ) {
        // We'll allow removal of compound assignment to a `this` property as long as the result
        // of the assignment is unused.
        // e.g. `this.prop += 3;`

        // NOTE: Some history here, as there were questions about "why is 'this' special".  The
        // "remove unused properties" is not a general property removal algorithm.  It only
        // removes unreferenced properties that are part of class definitions.  "SomeClass.prop +=
        // 3" and "SomeClass.prototype.prop += 3" could so be candidates but they aren't
        // considered here.

        let target_node = compound_assign_node.get_first_child(compiler).unwrap();
        let value_node = compound_assign_node.get_last_child(compiler).unwrap();
        if target_node.is_get_prop(compiler) {
            if self.consider_for_accessor_side_effects(
                compiler,
                target_node,
                PropertyAccessKind::GETTER_AND_SETTER,
            ) {
                let target_obj = target_node.get_first_child(compiler).unwrap();
                self.traverse_node(compiler, target_obj, scope); // Don't re-traverse the GETPROP as a read.
                self.traverse_node(compiler, value_node, scope);
            } else if target_node
                .get_first_child(compiler)
                .unwrap()
                .is_this(compiler)
                && !NodeUtil::is_expression_result_used(compiler, compound_assign_node)
            {
                let mut builder = RemovableBuilder::new().set_is_this_dot_property_reference(true);
                builder =
                    self.traverse_removable_assign_value(compiler, value_node, builder, scope);
                let removable = builder.build_named_property_assign(
                    self,
                    compiler,
                    compound_assign_node,
                    target_node,
                );
                self.consider_for_independent_removal(compiler, removable);
            } else {
                self.traverse_node(compiler, target_node, scope);
                self.traverse_node(compiler, value_node, scope);
            }
        } else {
            self.traverse_node(compiler, target_node, scope);
            self.traverse_node(compiler, value_node, scope);
        }
    }

    // port: RemoveUnusedCode#traverseNameNode
    fn traverse_name_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        scope: ScopeId,
    ) -> VarInfoId {
        let name = n.get_string(compiler);
        if self.polyfills.contains_key(&name) {
            for info in self.polyfills.get(&name) {
                if self.polyfill_infos[info.0].is_removable {
                    self.polyfill_info_consider_possible_reference(compiler, info, n);
                }
            }
        }

        let var = self.get_var_for_name_node(compiler, n, scope);
        self.traverse_var(compiler, var)
    }

    // port: RemoveUnusedCode#traverseCall
    fn traverse_call(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        scope: ScopeId,
    ) {
        let callee = call_node.get_first_child(compiler).unwrap();

        if compiler
            .get_coding_convention()
            .is_property_rename_function(compiler, callee)
        {
            let property_name_node = callee.get_next(compiler);
            if let Some(property_name_node) = property_name_node
                && property_name_node.is_string_lit(compiler)
            {
                let property_name = property_name_node.get_string(compiler);
                self.mark_property_name_as_pinned(compiler, property_name);
            }
            self.traverse_children(compiler, call_node, scope);
        } else if NodeUtil::is_object_define_properties_definition(compiler, call_node) {
            // TODO(bradfordcsmith): Should also handle Object.create() and
            // Object.defineProperty().
            self.traverse_object_define_properties_call(compiler, call_node, scope);
        } else if self.remove_unused_polyfills && is_jscomp_polyfill(compiler, callee) {
            let first_arg = callee.get_next(compiler).unwrap();
            let mut polyfill_name = first_arg.get_string(compiler).to_string_lossy();
            if callee
                .get_string(compiler)
                .ends_with("polyfillTypedArrayMethod")
            {
                polyfill_name = format!("TypedArray.prototype.{polyfill_name}");
            }
            let info = self.create_polyfill_info(compiler, call_node, scope, &polyfill_name);
            let key = self.polyfill_infos[info.0].key.clone();
            self.polyfills.put(key, info);
            // Only traverse the callee (to mark it as used).  The arguments may be traversed
            // later.
            self.traverse_node(
                compiler,
                call_node.get_first_child(compiler).unwrap(),
                scope,
            );
        } else if NodeUtil::is_goog_weak_usage_call(compiler, call_node)
            && call_node.has_two_children(compiler)
            && call_node
                .get_second_child(compiler)
                .unwrap()
                .is_name(compiler)
        {
            // goog.weakUsage() should have exactly one argument, and it should be either a name
            // or a qualified name (this is checked in ProcessClosurePrimitives.java).
            //
            // If it is a qualified name, then we do not attempt to remove it at this time (see
            // condition above). We rely on CollapseProperties to turn qualified names into simple
            // names where possible.

            // Mark this call as removable if the var is not otherwise referenced.
            let var_info = self.traverse_name_node(
                compiler,
                call_node.get_second_child(compiler).unwrap(),
                scope,
            );
            let builder = RemovableBuilder::new();
            let removable = builder.build_weak_usage_call(self, compiler, call_node);
            self.var_info_add_removable(compiler, var_info, removable);

            // We need to traverse the goog.weakUsage function itself (to mark it as used, in case
            // our usage of it is not removed).
            self.traverse_node(
                compiler,
                call_node.get_first_child(compiler).unwrap(),
                scope,
            );
        } else {
            let parent = call_node.get_parent(compiler).unwrap();
            let mut class_var_name: Option<JsString> = None;
            let mut class_defining_call = false;

            // A call that is a statement unto itself or the left side of a comma expression might
            // be a call to a known method for doing class setup
            // e.g. $jscomp.inherits(Class, BaseClass) or goog.addSingletonGetter(Class)
            // Such methods never have meaningful return values, so we won't look for them in
            // other contexts
            if parent.is_expr_result(compiler)
                || (parent.is_comma(compiler)
                    && parent.get_first_child(compiler) == Some(call_node))
            {
                let subclass_relationship = compiler
                    .get_coding_convention()
                    .get_classes_defined_by_call(compiler, call_node);
                if let Some(subclass_relationship) = subclass_relationship {
                    // e.g. goog.inherits(DerivedClass, BaseClass);
                    // NOTE: DerivedClass and BaseClass must be QNames. Otherwise
                    // getClassesDefinedByCall() will return null.
                    class_var_name = Some(subclass_relationship.subclass_name);
                    class_defining_call = true;
                } else {
                    // Look for calls to addSingletonGetter calls.
                    class_var_name = compiler
                        .get_coding_convention()
                        .get_singleton_getter_class_name(compiler, call_node);
                }
            }

            let mut class_var: Option<VarId> = None;
            if let Some(class_var_name) = &class_var_name
                && NodeUtil::is_valid_simple_name(class_var_name)
            {
                class_var = Some(check_not_null!(
                    scope.get_var(compiler, class_var_name.clone()),
                    "%s",
                    class_var_name
                ));
            }

            match class_var {
                Some(class_var) if class_var.is_global(compiler) => {
                    let mut builder = RemovableBuilder::new();
                    let mut child = call_node.get_first_child(compiler);
                    while let Some(c) = child {
                        builder = builder.add_continuation(Continuation::new(c, scope));
                        child = c.get_next(compiler);
                    }
                    let var_info = self.traverse_var(compiler, class_var);
                    let removable = builder.build_class_setup_call_with_class_defining_call(
                        self,
                        compiler,
                        call_node,
                        class_defining_call,
                    );
                    self.var_info_add_removable(compiler, var_info, removable);
                }
                _ => {
                    // The call we are traversing does not modify a class definition,
                    // or the class is not specified with a simple variable name,
                    // or the variable name is not global.
                    // TODO(bradfordcsmith): It would be more correct to check whether the class
                    // name references a known constructor and expand to allow QNames.
                    self.traverse_children(compiler, call_node, scope);
                }
            }
        }
    }

    /// Traverse `Object.defineProperties(someObject, propertyDefinitions);`.
    // port: RemoveUnusedCode#traverseObjectDefinePropertiesCall
    fn traverse_object_define_properties_call(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        scope: ScopeId,
    ) {
        // First child is Object.defineProperties or some equivalent of it.
        let callee = call_node.get_first_child(compiler).unwrap();
        let target_object = call_node.get_second_child(compiler).unwrap();
        let property_definitions = target_object.get_next(compiler).unwrap();

        if (target_object.is_name(compiler) || is_name_dot_prototype(compiler, target_object))
            && !NodeUtil::is_expression_result_used(compiler, call_node)
        {
            // NOTE: Object.defineProperties() returns its first argument, so if its return value
            // is used that counts as a use of the targetObject.
            let name_node = if target_object.is_name(compiler) {
                target_object
            } else {
                target_object.get_first_child(compiler).unwrap()
            };
            let var_info = self.traverse_name_node(compiler, name_node, scope);
            let mut builder = RemovableBuilder::new();
            // TODO(bradfordcsmith): Is it really necessary to traverse the callee
            // (aka. Object.defineProperties)?
            builder = builder.add_continuation(Continuation::new(callee, scope));
            if self.may_have_side_effects(compiler, property_definitions) {
                self.traverse_node(compiler, property_definitions, scope);
            } else {
                builder = builder.add_continuation(Continuation::new(property_definitions, scope));
            }
            let removable = builder.build_class_setup_call(self, compiler, call_node);
            self.var_info_add_removable(compiler, var_info, removable);
        } else {
            // TODO(bradfordcsmith): Is it really necessary to traverse the callee
            // (aka. Object.defineProperties)?
            self.traverse_node(compiler, callee, scope);
            self.traverse_node(compiler, target_object, scope);
            self.traverse_node(compiler, property_definitions, scope);
        }
    }

    /// Traverse the object literal passed as the second argument to
    /// `Object.defineProperties()`.
    // port: RemoveUnusedCode#traverseObjectDefinePropertiesLiteral
    fn traverse_object_define_properties_literal(
        &mut self,
        compiler: &mut AbstractCompiler,
        property_definitions: NodeId,
        scope: ScopeId,
    ) {
        let mut property = property_definitions.get_first_child(compiler);
        while let Some(p) = property {
            if p.is_quoted_string_key(compiler) {
                // Quoted property name counts as a reference to the property and protects it from
                // removal.
                let name = p.get_string(compiler);
                self.mark_property_name_as_pinned(compiler, name);
                self.traverse_node(compiler, p.get_only_child(compiler), scope);
            } else if p.is_string_key(compiler) {
                let definition = p.get_only_child(compiler);
                if self.may_have_side_effects(compiler, definition) {
                    self.traverse_node(compiler, definition, scope);
                } else {
                    let removable = RemovableBuilder::new()
                        .add_continuation(Continuation::new(definition, scope))
                        .build_object_define_properties_definition(self, compiler, p);
                    self.consider_for_independent_removal(compiler, removable);
                }
            } else {
                // TODO(bradfordcsmith): Maybe report error for anything other than a computed
                // property, since getters, setters, and methods don't make much sense in this
                // context.
                self.traverse_node(compiler, p, scope);
            }
            property = p.get_next(compiler);
        }
    }

    // port: RemoveUnusedCode#getVarForNameNode
    fn get_var_for_name_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        name_node: NodeId,
        scope: ScopeId,
    ) -> VarId {
        let name = name_node.get_string(compiler);
        check_not_null!(
            scope.get_var(compiler, name),
            "%s",
            name_node.to_string(compiler)
        )
    }
}

/// Checks whether this is a recognizable call to $jscomp.polyfill.
// port: RemoveUnusedCode#isJscompPolyfill
fn is_jscomp_polyfill(ast: &Ast, n: NodeId) -> bool {
    match n.get_token(ast) {
        Token::NAME => {
            // Need to work correctly after CollapseProperties.
            let name = n.get_string(ast);
            (name == "$jscomp$polyfill"
                || name == "$jscomp$patch"
                || name == "$jscomp$polyfillTypedArrayMethod")
                && n.get_next(ast).unwrap().is_string_lit(ast)
        }
        Token::GETPROP => {
            // Need to work correctly without CollapseProperties.
            let property_name = n.get_string(ast);
            (property_name == "polyfill"
                || property_name == "patch"
                || property_name == "polyfillTypedArrayMethod")
                && n.get_first_child(ast).unwrap().is_name(ast)
                && n.get_first_child(ast).unwrap().get_string_ref(ast) == "$jscomp"
                && n.get_next(ast).unwrap().is_string_lit(ast)
        }
        _ => false,
    }
}

/// True for `someExpression.prototype`.
// port: RemoveUnusedCode#isDotPrototype
fn is_dot_prototype(ast: &Ast, n: NodeId) -> bool {
    NodeUtil::is_normal_or_opt_chain_get_prop(ast, n) && n.get_string_ref(ast) == "prototype"
}

// ---------------------------------------------------------------------------------------------
// Continuation
// ---------------------------------------------------------------------------------------------

/// Our progress in a traversal can be expressed completely as the current node and scope. The
/// continuation lets us save that information so that we can continue the traversal later.
// port: RemoveUnusedCode.Continuation
#[derive(Copy, Clone, Debug)]
struct Continuation {
    node: NodeId,
    scope: ScopeId,
}

impl Continuation {
    // port: RemoveUnusedCode.Continuation#Continuation
    fn new(node: NodeId, scope: ScopeId) -> Self {
        Self { node, scope }
    }
}

impl RemoveUnusedCode {
    // port: RemoveUnusedCode.Continuation#apply
    fn continuation_apply(&mut self, compiler: &mut AbstractCompiler, continuation: Continuation) {
        let Continuation { node, scope } = continuation;
        if node.is_function(compiler) {
            // Calling traverseNode here would create infinite recursion for a function
            // declaration
            self.traverse_function(compiler, node, scope);
        } else {
            self.traverse_node(compiler, node, scope);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Guava HashMultimap<String, V> over java.util.HashMap (Java iteration order)
// ---------------------------------------------------------------------------------------------

// The java.util.HashMap table behind JavaHashMultimap (hashing, resizing, bins), ported from
// OpenJDK (GPL-2.0 with the Classpath exception), is in its own file.
#[path = "remove_unused_code_jdk.rs"]
mod jdk;

/// Guava's `HashMultimap<String, V>`: keys iterate in the order of the backing
/// `java.util.HashMap` table (simulated bucket by bucket), values of a key in insertion order
/// (Java's per-key `HashSet` of identity-hashed objects has no fixed order).
struct JavaHashMultimap<V: Copy + Eq> {
    /// The `HashMap` table: each bin lists its keys in `next` order.
    table: Vec<Vec<JsString>>,
    /// `HashMap.threshold`.
    threshold: usize,
    /// The value collections, looked up by key (lookup only; order comes from `table`).
    values: IndexMap<JsString, Vec<V>>,
}

impl<V: Copy + Eq> JavaHashMultimap<V> {
    // port: HashMultimap#create
    fn create() -> Self {
        Self {
            table: Vec::new(),
            threshold: 0,
            values: IndexMap::<_, _>::default(),
        }
    }

    // port: HashMultimap#put
    fn put(&mut self, key: JsString, value: V) {
        if let Some(values) = self.values.get_mut(&key) {
            if !values.contains(&value) {
                values.push(value);
            }
            return;
        }
        self.values.insert(key.clone(), vec![value]);
        self.put_key(key);
    }

    // port: HashMultimap#containsKey
    fn contains_key(&self, key: &JsString) -> bool {
        self.values.contains_key(key)
    }

    // port: HashMultimap#get
    fn get(&self, key: &JsString) -> Vec<V> {
        self.values.get(key).cloned().unwrap_or_default()
    }

    // port: HashMultimap#removeAll
    fn remove_all(&mut self, key: &JsString) -> Vec<V> {
        match self.values.shift_remove(key) {
            None => Vec::new(),
            Some(values) => {
                self.remove_node(key);
                values
            }
        }
    }

    // port: HashMultimap#keySet
    fn key_set(&self) -> Vec<JsString> {
        self.table.iter().flatten().cloned().collect()
    }

    // port: HashMultimap#values
    fn values(&self) -> Vec<(JsString, V)> {
        let mut out = Vec::new();
        for key in self.table.iter().flatten() {
            for value in &self.values[key] {
                out.push((key.clone(), *value));
            }
        }
        out
    }

    /// `Iterator#remove` on `values()`: removes the value, and the key once it has none left.
    // port: AbstractMapBasedMultimap.Itr#remove
    fn remove_value(&mut self, key: &JsString, value: V) {
        let values = self.values.get_mut(key).unwrap();
        values.retain(|v| *v != value);
        if values.is_empty() {
            self.remove_all(key);
        }
    }
}

impl RemoveUnusedCode {
    // port: RemoveUnusedCode#traverseObjectLiteral
    fn traverse_object_literal(
        &mut self,
        compiler: &mut AbstractCompiler,
        object_literal: NodeId,
        scope: ScopeId,
    ) {
        check_argument!(
            object_literal.is_object_lit(compiler),
            "%s",
            object_literal.to_string(compiler)
        );
        // Is this an object literal that is assigned directly to a 'prototype' property?
        if is_assignment_to_prototype(compiler, object_literal.get_parent(compiler).unwrap()) {
            self.traverse_prototype_literal(compiler, object_literal, scope);
        } else if is_object_define_properties_second_argument(compiler, object_literal) {
            // TODO(bradfordcsmith): Consider restricting special handling of the properties
            // literal to cases where the target object is a known class, prototype, or this.
            self.traverse_object_define_properties_literal(compiler, object_literal, scope);
        } else {
            self.traverse_non_prototype_object_literal(compiler, object_literal, scope);
        }
    }

    // port: RemoveUnusedCode#traverseNonPrototypeObjectLiteral
    fn traverse_non_prototype_object_literal(
        &mut self,
        compiler: &mut AbstractCompiler,
        object_literal: NodeId,
        scope: ScopeId,
    ) {
        let mut property_node = object_literal.get_first_child(compiler);
        while let Some(p) = property_node {
            if p.is_string_key(compiler) {
                // A property name in an object literal counts as a reference,
                // because of some reflection patterns.
                // Note that we are intentionally treating both quoted and unquoted keys as
                // references.
                let name = p.get_string(compiler);
                self.mark_property_name_as_pinned(compiler, name);
                self.traverse_node(compiler, p.get_first_child(compiler).unwrap(), scope);
            } else {
                self.traverse_node(compiler, p, scope);
            }
            property_node = p.get_next(compiler);
        }
    }

    // port: RemoveUnusedCode#traversePrototypeLiteral
    fn traverse_prototype_literal(
        &mut self,
        compiler: &mut AbstractCompiler,
        object_literal: NodeId,
        scope: ScopeId,
    ) {
        let mut property_node = object_literal.get_first_child(compiler);
        while let Some(p) = property_node {
            if p.is_computed_prop(compiler) || p.is_quoted_string_key(compiler) {
                self.traverse_children(compiler, p, scope);
            } else {
                let value_node = p.get_only_child(compiler);
                if self.may_have_side_effects(compiler, value_node) {
                    // TODO(bradfordcsmith): Ideally we should preserve the side-effect without
                    // keeping the property itself alive.
                    self.traverse_node(compiler, value_node, scope);
                } else {
                    // If we've come this far, we already know we're keeping the prototype literal
                    // itself, but we may be able to remove unreferenced properties in it.
                    let removable = RemovableBuilder::new()
                        .add_continuation(Continuation::new(value_node, scope))
                        .build_class_or_prototype_named_property(self, compiler, p);
                    self.consider_for_independent_removal(compiler, removable);
                }
            }
            property_node = p.get_next(compiler);
        }
    }

    // port: RemoveUnusedCode#traverseCatch
    fn traverse_catch(
        &mut self,
        compiler: &mut AbstractCompiler,
        catch_node: NodeId,
        scope: ScopeId,
    ) {
        let exception_name_node = catch_node.get_first_child(compiler).unwrap();
        let block = exception_name_node.get_next(compiler).unwrap();
        if exception_name_node.is_name(compiler) {
            // exceptionNameNode can be an empty node if not using a binding in 2019.
            let exception_var_info = self.traverse_name_node(compiler, exception_name_node, scope);
            self.var_info_set_is_explicitly_not_removable(compiler, exception_var_info, |_| {
                "catch variable".to_string()
            });
        } else {
            self.traverse_node(compiler, exception_name_node, scope);
        }
        self.traverse_node(compiler, block, scope);
    }

    // port: RemoveUnusedCode#traverseEnhancedFor
    fn traverse_enhanced_for(
        &mut self,
        compiler: &mut AbstractCompiler,
        enhanced_for: NodeId,
        scope: ScopeId,
    ) {
        let for_scope = self
            .scope_creator
            .create_scope(compiler, enhanced_for, Some(scope));
        // for (iterationTarget in|of collection) body;
        let iteration_target = enhanced_for.get_first_child(compiler).unwrap();
        let collection = iteration_target.get_next(compiler).unwrap();
        let body = collection.get_next(compiler).unwrap();
        if iteration_target.is_name(compiler) {
            // using previously-declared loop variable. e.g.
            // `for (varName of collection) {}`
            let var_info = self.traverse_name_node(compiler, iteration_target, for_scope);
            self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                "for-of or for-in loop variable".to_string()
            });
        } else if NodeUtil::is_name_declaration(compiler, Some(iteration_target)) {
            // loop has const/var/let declaration
            let decl_node = iteration_target.get_only_child(compiler);
            if decl_node.is_destructuring_lhs(compiler) {
                // e.g.
                // `for (const [a, b] of pairList) {}`
                // destructuring is handled at a lower level
                // Note that destructuring assignments are always considered to set an unknown
                // value equivalent to what we set for the var name case above and below.
                // It isn't necessary to set the variable names as not removable, though, because
                // the thing that isn't removable is the destructuring pattern itself, which we
                // never remove.
                // TODO(bradfordcsmith): The need to explain all the above shows this should be
                // reworked.
                self.traverse_node(compiler, decl_node, for_scope);
            } else {
                // e.g.
                // `for (const varName of collection) {}`
                check_state!(decl_node.is_name(compiler));
                check_state!(!decl_node.has_children(compiler));
                // We can never remove the loop variable of a for-in or for-of loop, because it's
                // essential to loop syntax.
                let var_info = self.traverse_name_node(compiler, decl_node, for_scope);
                self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                    "for-of or for-in loop variable".to_string()
                });
            }
        } else {
            // using some general LHS value e.g.
            // `for ([a, b] of collection) {}` destructuring with existing vars
            // `for (a.x of collection) {}` using a property as the loop var
            // TODO(bradfordcsmith): This should be considered a write if it's a property
            // reference.
            self.traverse_node(compiler, iteration_target, for_scope);
        }
        self.traverse_node(compiler, collection, for_scope);
        self.traverse_node(compiler, body, for_scope);
    }

    // port: RemoveUnusedCode#traverseVanillaFor
    fn traverse_vanilla_for(
        &mut self,
        compiler: &mut AbstractCompiler,
        for_node: NodeId,
        scope: ScopeId,
    ) {
        let for_scope = self
            .scope_creator
            .create_scope(compiler, for_node, Some(scope));
        let initialization = for_node.get_first_child(compiler).unwrap();
        let condition = initialization.get_next(compiler).unwrap();
        let update = condition.get_next(compiler).unwrap();
        let block = update.get_next(compiler).unwrap();
        if NodeUtil::is_name_declaration(compiler, Some(initialization)) {
            self.traverse_vanilla_for_name_declarations(compiler, initialization, for_scope);
        } else {
            self.traverse_node(compiler, initialization, for_scope);
        }
        self.traverse_node(compiler, condition, for_scope);
        self.traverse_node(compiler, update, for_scope);
        self.traverse_node(compiler, block, for_scope);
    }

    // port: RemoveUnusedCode#traverseVanillaForNameDeclarations
    fn traverse_vanilla_for_name_declarations(
        &mut self,
        compiler: &mut AbstractCompiler,
        name_declaration: NodeId,
        scope: ScopeId,
    ) {
        let mut child = name_declaration.get_first_child(compiler);
        while let Some(c) = child {
            if !c.is_name(compiler) {
                // TODO(bradfordcsmith): Customize handling of destructuring
                self.traverse_node(compiler, c, scope);
            } else {
                let name_node = c;
                let value_node: Option<NodeId> = c.get_first_child(compiler);
                let var_info = self.traverse_name_node(compiler, name_node, scope);
                match value_node {
                    None => {
                        let removable = RemovableBuilder::new()
                            .build_vanilla_for_name_declaration(self, compiler, name_node);
                        self.var_info_add_removable(compiler, var_info, removable);
                    }
                    Some(value_node) if self.may_have_side_effects(compiler, value_node) => {
                        // TODO(bradfordcsmith): Actually allow for removing the variable while
                        // keeping the valueNode for its side-effects.
                        self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                            "for-loop variable initialization has side-effects".to_string()
                        });
                        self.traverse_node(compiler, value_node, scope);
                    }
                    Some(value_node) => {
                        let vanilla_for_name_declaration = RemovableBuilder::new()
                            .add_continuation(Continuation::new(value_node, scope))
                            .build_vanilla_for_name_declaration(self, compiler, name_node);
                        self.var_info_add_removable(
                            compiler,
                            var_info,
                            vanilla_for_name_declaration,
                        );
                    }
                }
            }
            child = c.get_next(compiler);
        }
    }

    // port: RemoveUnusedCode#traverseDeclarationStatement
    fn traverse_declaration_statement(
        &mut self,
        compiler: &mut AbstractCompiler,
        declaration_statement: NodeId,
        scope: ScopeId,
    ) {
        // Normalization should ensure that declaration statements always have just one child.
        let name_node = declaration_statement.get_only_child(compiler);
        if !name_node.is_name(compiler) {
            // Destructuring declarations are handled elsewhere.
            self.traverse_node(compiler, name_node, scope);
        } else {
            let value_node = name_node.get_first_child(compiler);
            let var_info = self.traverse_name_node(compiler, name_node, scope);
            let mut builder = RemovableBuilder::new();
            match value_node {
                None => {
                    let removable = builder.build_name_declaration_statement(
                        self,
                        compiler,
                        declaration_statement,
                    );
                    self.var_info_add_removable(compiler, var_info, removable);
                }
                Some(value_node) => {
                    if self.may_have_side_effects(compiler, value_node) {
                        self.traverse_node(compiler, value_node, scope);
                    } else {
                        builder = builder.add_continuation(Continuation::new(value_node, scope));
                    }
                    let removable = builder.build_name_declaration_statement(
                        self,
                        compiler,
                        declaration_statement,
                    );
                    self.var_info_add_removable(compiler, var_info, removable);
                }
            }
        }
    }

    // port: RemoveUnusedCode#traverseAssign
    fn traverse_assign(
        &mut self,
        compiler: &mut AbstractCompiler,
        assign_node: NodeId,
        scope: ScopeId,
    ) {
        check_state!(NodeUtil::is_assignment_op(compiler, assign_node));

        let lhs = assign_node.get_first_child(compiler).unwrap();
        let value_node = assign_node.get_last_child(compiler).unwrap();
        if lhs.is_name(compiler) {
            // varName = something
            let var_info = self.traverse_name_node(compiler, lhs, scope);
            let mut builder = RemovableBuilder::new();
            builder = self.traverse_removable_assign_value(compiler, value_node, builder, scope);
            let removable = builder.build_variable_assign(self, compiler, assign_node, var_info);
            self.var_info_add_removable(compiler, var_info, removable);
        } else if lhs.is_get_elem(compiler) {
            let get_elem_obj = lhs.get_first_child(compiler).unwrap();
            let get_elem_key = lhs.get_last_child(compiler).unwrap();
            let var_name_node = if get_elem_obj.is_name(compiler) {
                Some(get_elem_obj)
            } else if is_name_dot_prototype(compiler, get_elem_obj) {
                get_elem_obj.get_first_child(compiler)
            } else {
                None
            };

            if let Some(var_name_node) = var_name_node {
                // varName[someExpression] = someValue
                // OR
                // varName.prototype[someExpression] = someValue
                let var_info = self.traverse_name_node(compiler, var_name_node, scope);
                let mut builder = RemovableBuilder::new();
                if self.may_have_side_effects(compiler, get_elem_key) {
                    self.traverse_node(compiler, get_elem_key, scope);
                } else {
                    builder = builder.add_continuation(Continuation::new(get_elem_key, scope));
                }
                builder =
                    self.traverse_removable_assign_value(compiler, value_node, builder, scope);
                let removable = builder.build_computed_property_assign(
                    self,
                    compiler,
                    assign_node,
                    get_elem_key,
                    var_info,
                );
                self.var_info_add_removable(compiler, var_info, removable);
            } else {
                self.traverse_node(compiler, get_elem_obj, scope);
                self.traverse_node(compiler, get_elem_key, scope);
                self.traverse_node(compiler, value_node, scope);
            }
        } else if lhs.is_get_prop(compiler) {
            let get_prop_lhs = lhs.get_first_child(compiler).unwrap();
            // Assignments `Foo.prototype.bar = function() {`
            let is_dot_prototype_lhs = is_dot_prototype(compiler, get_prop_lhs);
            let is_prototype_method_def = is_dot_prototype_lhs && value_node.is_function(compiler);

            if !is_prototype_method_def
                && self.consider_for_accessor_side_effects(
                    compiler,
                    lhs,
                    PropertyAccessKind::SETTER_ONLY,
                )
            {
                // And the possible side-effects mean we can't do any removal. We don't use the
                // `AstAnalyzer` because we only want to consider side-effect from the assignment,
                // not the entire l-value subtree.
                // Assume prototype method assignments never trigger setters, matching ES class
                // semantics
                self.traverse_node(compiler, get_prop_lhs, scope); // Don't re-traverse the GETPROP as a read.
                self.traverse_node(compiler, value_node, scope);
            } else if get_prop_lhs.is_name(compiler) {
                // varName.propertyName = someValue
                let var_info = self.traverse_name_node(compiler, get_prop_lhs, scope);
                let mut builder = RemovableBuilder::new();
                builder =
                    self.traverse_removable_assign_value(compiler, value_node, builder, scope);
                let removable = builder.build_named_property_assign_with_var_info(
                    self,
                    compiler,
                    assign_node,
                    lhs,
                    Some(var_info),
                );
                self.var_info_add_removable(compiler, var_info, removable);
            } else if is_dot_prototype_lhs {
                // objExpression.prototype.propertyName = someValue
                let obj_expression = get_prop_lhs.get_first_child(compiler).unwrap();
                let mut builder =
                    RemovableBuilder::new().set_is_prototype_dot_property_reference(true);
                builder =
                    self.traverse_removable_assign_value(compiler, value_node, builder, scope);
                if obj_expression.is_name(compiler) {
                    // varName.prototype.propertyName = someValue
                    let var_info = self.traverse_name_node(
                        compiler,
                        get_prop_lhs.get_first_child(compiler).unwrap(),
                        scope,
                    );
                    let removable = builder.build_named_property_assign_with_var_info(
                        self,
                        compiler,
                        assign_node,
                        lhs,
                        Some(var_info),
                    );
                    self.var_info_add_removable(compiler, var_info, removable);
                } else {
                    // (someExpression).prototype.propertyName = someValue
                    if self.may_have_side_effects(compiler, obj_expression) {
                        self.traverse_node(compiler, obj_expression, scope);
                    } else {
                        builder =
                            builder.add_continuation(Continuation::new(obj_expression, scope));
                    }
                    let property_name = lhs.get_string(compiler);
                    let removable = builder.build_anonymous_prototype_named_property_assign(
                        self,
                        compiler,
                        assign_node,
                        property_name,
                    );
                    self.consider_for_independent_removal(compiler, removable);
                }
            } else if get_prop_lhs.is_this(compiler) {
                // this.propertyName = someValue
                let mut builder = RemovableBuilder::new().set_is_this_dot_property_reference(true);
                builder =
                    self.traverse_removable_assign_value(compiler, value_node, builder, scope);
                let removable =
                    builder.build_named_property_assign(self, compiler, assign_node, lhs);
                self.consider_for_independent_removal(compiler, removable);
            } else {
                self.traverse_node(compiler, lhs, scope);
                self.traverse_node(compiler, value_node, scope);
            }
        } else {
            // no other cases are removable
            self.traverse_node(compiler, lhs, scope);
            self.traverse_node(compiler, value_node, scope);
        }
    }

    /// Takes the builder and returns it (Java mutates the builder it is passed).
    // port: RemoveUnusedCode#traverseRemovableAssignValue
    fn traverse_removable_assign_value(
        &mut self,
        compiler: &mut AbstractCompiler,
        value_node: NodeId,
        builder: RemovableBuilder,
        scope: ScopeId,
    ) -> RemovableBuilder {
        if self.may_have_side_effects(compiler, value_node)
            || NodeUtil::is_expression_result_used(
                compiler,
                value_node.get_parent(compiler).unwrap(),
            )
        {
            self.traverse_node(compiler, value_node, scope);
            builder
        } else {
            builder.add_continuation(Continuation::new(value_node, scope))
        }
    }

    // port: RemoveUnusedCode#traverseObjectPattern
    fn traverse_object_pattern(
        &mut self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
        scope: ScopeId,
    ) {
        check_state!(
            pattern.is_object_pattern(compiler),
            "%s",
            pattern.to_string(compiler)
        );

        let mut elem = pattern.get_first_child(compiler);
        while let Some(e) = elem {
            match e.get_token(compiler) {
                Token::COMPUTED_PROP => {
                    let target = e.get_second_child(compiler).unwrap();
                    self.traverse_indirect_assignment(compiler, e, target, scope)
                }
                Token::STRING_KEY => {
                    if !e.is_quoted_string_key(compiler) {
                        let name = e.get_string(compiler);
                        self.mark_property_name_as_pinned(compiler, name);
                    }
                    let target = e.get_only_child(compiler);
                    self.traverse_indirect_assignment(compiler, e, target, scope);
                }
                Token::ITER_REST | Token::OBJECT_REST => {
                    // Recall that the rest target can be any l-value expression
                    let target = e.get_only_child(compiler);
                    self.traverse_indirect_assignment(compiler, e, target, scope)
                }
                _ => panic!(
                    "Unexpected child of {}: {}",
                    pattern.get_token(compiler),
                    e.to_string_tree(compiler)
                ),
            }
            elem = e.get_next(compiler);
        }
    }

    // port: RemoveUnusedCode#traverseIndirectAssignmentList
    fn traverse_indirect_assignment_list(
        &mut self,
        compiler: &mut AbstractCompiler,
        list: NodeId,
        scope: ScopeId,
    ) {
        check_state!(
            list.is_array_pattern(compiler) || list.is_param_list(compiler),
            "%s",
            list.to_string(compiler)
        );

        let mut elem = list.get_first_child(compiler);
        while let Some(e) = elem {
            match e.get_token(compiler) {
                Token::EMPTY => {}
                Token::ARRAY_PATTERN
                | Token::DEFAULT_VALUE
                | Token::GETELEM
                | Token::GETPROP
                | Token::NAME
                | Token::OBJECT_PATTERN => self.traverse_indirect_assignment(compiler, e, e, scope),
                Token::ITER_REST | Token::OBJECT_REST => {
                    let target = e.get_only_child(compiler);
                    self.traverse_indirect_assignment(compiler, e, target, scope)
                }
                _ => panic!(
                    "Unexpected child of {}: {}",
                    list.get_token(compiler),
                    e.to_string_tree(compiler)
                ),
            }
            elem = e.get_next(compiler);
        }
    }

    /// Traverse an AST structure representing an assignment operation for which the target and
    /// value are far apart.
    ///
    /// Examples include destructurings and function parameters.
    ///
    /// `root`: The root of the assignment subtree. `target`: The l-value expression being
    /// assigned to.
    // port: RemoveUnusedCode#traverseIndirectAssignment
    fn traverse_indirect_assignment(
        &mut self,
        compiler: &mut AbstractCompiler,
        root: NodeId,
        target: NodeId,
        scope: ScopeId,
    ) {
        let mut target = target;
        let root_parent = root.get_parent(compiler).unwrap();
        check_argument!(
            root_parent.is_destructuring_pattern(compiler) || root_parent.is_param_list(compiler),
            "%s",
            root_parent.to_string(compiler)
        );

        // Flatten out the case where the target is a default value. We always have to consider
        // it.
        if target.is_default_value(compiler) {
            target = target.get_first_child(compiler).unwrap();
        }

        if target.is_get_prop(compiler) {
            self.consider_for_accessor_side_effects(
                compiler,
                target,
                PropertyAccessKind::SETTER_ONLY,
            );
        }

        let builder = RemovableBuilder::new().add_continuation(Continuation::new(root, scope));

        if self.may_have_side_effects(compiler, root) {
            // If anywhere in the assignment subtree has side-effects, it means that even if the
            // target is removable the subtree is not.
            self.traverse_node(compiler, root, scope);
            // TODO(bradfordcsmith): Preserve side effects without preventing removal of variables
            // and properties. We could probably do this by subbing in an empty object pattern.
        } else if target.is_name(compiler) {
            let var_info = self.traverse_name_node(compiler, target, scope);
            let removable = builder.build_indirect_assign(self, compiler, root, target);
            self.var_info_add_removable(compiler, var_info, removable);
        } else if is_name_dot_prototype(compiler, target) || is_this_dot_property(compiler, target)
        {
            let removable = builder.build_indirect_assign(self, compiler, root, target);
            self.consider_for_independent_removal(compiler, removable);
        } else {
            // TODO(bradfordcsmith): Handle property assignments also
            // e.g. `({a: foo.bar, b: foo.baz}) = {a: 1, b: 2}`
            self.traverse_node(compiler, root, scope);
        }
    }

    // port: RemoveUnusedCode#traverseChildren
    fn traverse_children(&mut self, compiler: &mut AbstractCompiler, n: NodeId, scope: ScopeId) {
        let mut c = n.get_first_child(compiler);
        while let Some(child) = c {
            self.traverse_node(compiler, child, scope);
            c = child.get_next(compiler);
        }
    }

    /// Handle a class that is not the RHS child of an assignment or a variable declaration
    /// initializer.
    // port: RemoveUnusedCode#traverseClass
    fn traverse_class(
        &mut self,
        compiler: &mut AbstractCompiler,
        class_node: NodeId,
        scope: ScopeId,
    ) {
        check_argument!(class_node.is_class(compiler));
        if NodeUtil::is_class_declaration(compiler, class_node) {
            self.traverse_class_declaration(compiler, class_node, scope);
        } else {
            self.traverse_class_expression(compiler, class_node, scope);
        }
    }

    // port: RemoveUnusedCode#traverseClassDeclaration
    fn traverse_class_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        class_node: NodeId,
        scope: ScopeId,
    ) {
        check_argument!(class_node.is_class(compiler));
        let class_name_node = class_node.get_first_child(compiler).unwrap();
        let base_class_expression = class_name_node.get_next(compiler).unwrap();
        let class_body_node = base_class_expression.get_next(compiler).unwrap();
        let class_scope = self
            .scope_creator
            .create_scope(compiler, class_node, Some(scope));

        let var_info = self.traverse_name_node(compiler, class_name_node, scope);
        if class_node.get_parent(compiler).unwrap().is_export(compiler) {
            // Cannot remove an exported class.
            self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                "exported class".to_string()
            });
            self.traverse_node(compiler, base_class_expression, scope);
            // Use traverseChildren() here, because we should not consider any properties on the
            // exported class to be removable.
            self.traverse_children(compiler, class_body_node, class_scope);
        } else if self.may_have_side_effects(compiler, base_class_expression) {
            // TODO(bradfordcsmith): implement removal without losing side-effects for this case
            self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                "base class expression has side-effects".to_string()
            });
            self.traverse_node(compiler, base_class_expression, scope);
            self.traverse_class_members(compiler, class_body_node, class_scope);
        } else if self.may_have_side_effects(compiler, class_body_node) {
            self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                "class body has side-effects".to_string()
            });
            self.traverse_node(compiler, base_class_expression, scope);
            self.traverse_class_members(compiler, class_body_node, class_scope);
        } else {
            let builder = RemovableBuilder::new()
                .add_continuation(Continuation::new(base_class_expression, class_scope))
                .add_continuation(Continuation::new(class_body_node, class_scope));
            let removable = builder.build_class_declaration(self, compiler, class_node);
            self.var_info_add_removable(compiler, var_info, removable);
        }
    }

    // port: RemoveUnusedCode#traverseClassExpression
    fn traverse_class_expression(
        &mut self,
        compiler: &mut AbstractCompiler,
        class_node: NodeId,
        scope: ScopeId,
    ) {
        check_argument!(class_node.is_class(compiler));
        let class_name_node = class_node.get_first_child(compiler).unwrap();
        let base_class_expression = class_name_node.get_next(compiler).unwrap();
        let class_body_node = base_class_expression.get_next(compiler).unwrap();
        let class_scope = self
            .scope_creator
            .create_scope(compiler, class_node, Some(scope));

        if class_name_node.is_name(compiler) {
            // We may be able to remove the name node if nothing ends up referring to it.
            let var_info = self.traverse_name_node(compiler, class_name_node, class_scope);
            // The class is non-local, because it is accessible by unknown code outside
            // of the scope where InnerName is defined.
            // e.g. `use(class InnerName {})`
            self.var_info_set_has_non_local_or_non_literal_value(var_info);
            let removable =
                RemovableBuilder::new().build_named_class_expression(self, compiler, class_node);
            self.var_info_add_removable(compiler, var_info, removable);
        }
        // If we're traversing the class expression, we've already decided we cannot remove it.
        self.traverse_node(compiler, base_class_expression, scope);
        self.traverse_class_members(compiler, class_body_node, class_scope);
    }

    // port: RemoveUnusedCode#traverseClassMembers
    fn traverse_class_members(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        scope: ScopeId,
    ) {
        check_argument!(
            node.is_class_members(compiler),
            "%s",
            node.to_string(compiler)
        );
        if !self.remove_unused_prototype_properties {
            self.traverse_children(compiler, node, scope);
            return;
        }

        let mut member = node.get_first_child(compiler);
        while let Some(m) = member {
            match m.get_token(compiler) {
                Token::GETTER_DEF | Token::SETTER_DEF | Token::MEMBER_FUNCTION_DEF => {
                    // If we get as far as traversing the members of a class, we've already decided
                    // that we cannot remove the class itself, so just consider individual members
                    // for removal.
                    let removable = RemovableBuilder::new()
                        .add_continuation(Continuation::new(m, scope))
                        .build_class_or_prototype_named_property(self, compiler, m);
                    self.consider_for_independent_removal(compiler, removable)
                }
                Token::MEMBER_FIELD_DEF => {
                    // TODO(bradfordcsmith): currently if the RHS of a field has side effects, we
                    // do not remove any part of the field. The proper behavior of class C { x =
                    // alert(); } would be to remove x, leaving class C { constructor() { alert();
                    // } } but currently we aren't removing anything.
                    if !m.has_children(compiler)
                        || !self
                            .may_have_side_effects(compiler, m.get_first_child(compiler).unwrap())
                    {
                        let removable = RemovableBuilder::new()
                            .add_continuation(Continuation::new(m, scope))
                            .build_class_or_prototype_named_property(self, compiler, m);
                        self.consider_for_independent_removal(compiler, removable);
                    }
                    // TODO: b/354704593 - remove the entire class when it is not referenced
                    if m.has_children(compiler) {
                        self.traverse_children(compiler, m, scope);
                    }
                }
                Token::COMPUTED_PROP | Token::COMPUTED_FIELD_DEF => {
                    self.traverse_children(compiler, m, scope)
                }
                Token::BLOCK => {
                    let block_scope = self.scope_creator.create_scope(compiler, m, Some(scope));
                    self.traverse_children(compiler, m, block_scope)
                }
                _ => panic!(
                    "Unexpected child of CLASS_MEMBERS: {}",
                    m.to_string_tree(compiler)
                ),
            }
            member = m.get_next(compiler);
        }
    }
}

// port: RemoveUnusedCode#isObjectDefinePropertiesSecondArgument
fn is_object_define_properties_second_argument(ast: &Ast, n: NodeId) -> bool {
    let parent = n.get_parent(ast).unwrap();
    NodeUtil::is_object_define_properties_definition(ast, parent)
        && parent.get_last_child(ast) == Some(n)
}

// port: RemoveUnusedCode#isAssignmentToPrototype
fn is_assignment_to_prototype(ast: &Ast, n: NodeId) -> bool {
    n.is_assign(ast) && is_dot_prototype(ast, n.get_first_child(ast).unwrap())
}

// port: RemoveUnusedCode#isNameDotPrototype
fn is_name_dot_prototype(ast: &Ast, n: NodeId) -> bool {
    n.is_get_prop(ast)
        && n.get_first_child(ast).unwrap().is_name(ast)
        && n.get_string_ref(ast) == "prototype"
}

impl RemoveUnusedCode {
    /// Traverses a function
    ///
    /// ES6 scopes of a function include the parameter scope and the body scope of the function.
    ///
    /// Note that CATCH blocks also create a new scope, but only for the catch variable.
    /// Declarations within the block actually belong to the enclosing scope. Because we don't
    /// remove catch variables, there's no need to treat CATCH blocks differently like we do
    /// functions.
    // port: RemoveUnusedCode#traverseFunction
    fn traverse_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        function: NodeId,
        parent_scope: ScopeId,
    ) {
        check_state!(
            function.has_x_children(compiler, 3),
            "%s",
            function.to_string(compiler)
        );
        check_state!(
            function.is_function(compiler),
            "%s",
            function.to_string(compiler)
        );

        let paramlist = NodeUtil::get_function_parameters(compiler, function);
        let body = function.get_last_child(compiler).unwrap();
        check_state!(
            body.get_next(compiler).is_none() && body.is_block(compiler),
            "%s",
            body.to_string(compiler)
        );

        // Checking the parameters
        let fparam_scope = self
            .scope_creator
            .create_scope(compiler, function, Some(parent_scope));

        // Checking the function body
        let fbody_scope = self
            .scope_creator
            .create_scope(compiler, body, Some(fparam_scope));

        let name_node = function.get_first_child(compiler).unwrap();
        if !name_node.get_string_ref(compiler).is_empty() {
            // var x = function funcName() {};
            // make sure funcName gets into the varInfoMap so it will be considered for removal.
            let var_info = self.traverse_name_node(compiler, name_node, fparam_scope);
            if NodeUtil::is_expression_result_used(compiler, function) {
                // var f = function g() {};
                // The f is an alias for g, so g escapes from the scope where it is defined.
                self.var_info_set_has_non_local_or_non_literal_value(var_info);
            }
        }

        self.traverse_node(compiler, paramlist, fparam_scope);
        self.traverse_children(compiler, body, fbody_scope);

        self.all_function_param_scopes.push(fparam_scope);
    }

    // port: RemoveUnusedCode#canRemoveParameters
    fn can_remove_parameters(&self, ast: &Ast, parameter_list: NodeId) -> bool {
        check_state!(parameter_list.is_param_list(ast));
        let function = parameter_list.get_parent(ast).unwrap();
        self.remove_globals && !NodeUtil::is_get_or_set_key(ast, function.get_parent(ast).unwrap())
    }

    /// Removes unreferenced arguments from a function declaration and when possible the
    /// function's callSites.
    // port: RemoveUnusedCode#removeUnreferencedFunctionArgs
    fn remove_unreferenced_function_args(
        &mut self,
        compiler: &mut AbstractCompiler,
        fparam_scope: ScopeId,
    ) {
        // Notice that removing unreferenced function args breaks
        // Function.prototype.length. In advanced mode, we don't really care
        // about this: we consider "length" the equivalent of reflecting on
        // the function's lexical source.
        //
        // Rather than create a new option for this, we assume that if the user
        // is removing globals, then it's OK to remove unused function args.
        //
        // See http://blickly.github.io/closure-compiler-issues/#253
        if !self.remove_globals {
            return;
        }

        let function = fparam_scope.get_root_node(compiler);
        check_state!(function.is_function(compiler));
        if NodeUtil::is_get_or_set_key(compiler, function.get_parent(compiler).unwrap()) {
            // The parameters object literal setters can not be removed.
            return;
        }

        let arg_list = NodeUtil::get_function_parameters(compiler, function);
        // Strip as many unreferenced args off the end of the function declaration as possible.
        self.maybe_remove_unused_trailing_parameters(compiler, arg_list, fparam_scope);

        // Mark any remaining unused parameters are unused to OptimizeParameters can try to remove
        // them.
        self.mark_unused_parameters(compiler, arg_list, fparam_scope);
    }

    // port: RemoveUnusedCode#markPropertyNameAsPinned
    fn mark_property_name_as_pinned(
        &mut self,
        compiler: &mut AbstractCompiler,
        property_name: JsString,
    ) {
        if self.pinned_property_names.insert(property_name.clone()) {
            // Continue traversal of all of the property name's values and no longer consider them
            // for removal.
            for removable in self
                .removables_for_property_names
                .remove_all(&property_name)
            {
                self.removable_apply_continuations(removable);
            }
        }
        let _ = compiler;
    }

    // port: RemoveUnusedCode#considerForIndependentRemoval
    fn consider_for_independent_removal(
        &mut self,
        compiler: &mut AbstractCompiler,
        removable: RemovableId,
    ) {
        if self.removable_is_named_property(compiler, removable) {
            let property_name = self.removable_get_property_name(compiler, removable);

            if self.pinned_property_names.contains(&property_name)
                || compiler
                    .get_coding_convention()
                    .is_exported(&property_name, /* local= */ false)
            {
                // Referenced or exported, so not removable.
                self.removable_apply_continuations(removable);
            } else if self.is_independently_removable(compiler, removable) {
                // Store for possible removal later.
                self.removables_for_property_names
                    .put(property_name, removable);
            } else {
                self.removable_apply_continuations(removable);
                // This assignment counts as a reference, since we won't be removing it.
                // This is necessary in order to preserve getters and setters for the property.
                self.mark_property_name_as_pinned(compiler, property_name);
            }
        } else {
            self.removable_apply_continuations(removable);
        }
    }

    /// Returns whether or not accessor side-effect are a possibility.
    // port: RemoveUnusedCode#considerForAccessorSideEffects
    fn consider_for_accessor_side_effects(
        &mut self,
        compiler: &mut AbstractCompiler,
        getprop: NodeId,
        usage: PropertyAccessKind,
    ) -> bool {
        // Other node types may make sense in the future.
        check_state!(
            NodeUtil::is_normal_or_opt_chain_get_prop(compiler, getprop),
            "%s",
            getprop.to_string(compiler)
        );
        let prop_name = getprop.get_string(compiler);
        let recorded = compiler
            .get_accessor_summary()
            .expect("compiler.getAccessorSummary() is null")
            .get_kind(&prop_name);
        if (recorded.has_getter() && usage.has_getter() && !self.assume_getters_are_pure)
            || (recorded.has_setter() && usage.has_setter())
        {
            self.mark_property_name_as_pinned(compiler, prop_name);
            return true;
        }

        false
    }

    // port: RemoveUnusedCode#isIndependentlyRemovable
    fn is_independently_removable(
        &self,
        compiler: &AbstractCompiler,
        removable: RemovableId,
    ) -> bool {
        if self.removable_is_prototype_property(compiler, removable) {
            // `foo.prototype.prop = something;`
            // `class C { prop() {} }`
            self.remove_unused_prototype_properties
        } else if self.removable_is_object_define_properties_definition(removable) {
            // `Object.defineProperties({ prop: {...}});`
            self.remove_unused_object_define_properties_definitions
        } else if self.removable_is_this_dot_property_reference(compiler, removable) {
            // `this.prop = something;`
            self.remove_unused_this_properties
        } else if self.removable_is_static_property(compiler, removable) {
            // `class Foo { static prop() {} }`
            // `Foo.otherStaticProp = value;`
            // TODO(b/139319709): removeUnusedThisProperties has ended up covering more than it
            // was originally intended to cover for arbitrary reasons.
            self.remove_unused_this_properties
        } else {
            false
        }
    }

    /// Mark any remaining unused parameters as being unused so it can be used elsewhere.
    // port: RemoveUnusedCode#markUnusedParameters
    fn mark_unused_parameters(
        &mut self,
        compiler: &mut AbstractCompiler,
        param_list: NodeId,
        fparam_scope: ScopeId,
    ) {
        check_argument!(
            param_list.is_param_list(compiler),
            "%s",
            param_list.to_string(compiler)
        );

        let mut param = param_list.get_first_child(compiler);
        while let Some(p) = param {
            param = p.get_next(compiler);
            if p.is_unused_parameter(compiler) {
                // already marked
                continue;
            }

            let Some(param_name_node) = name_of_param(compiler, p) else {
                // destructuring pattern parameters don't have a name that applies to the whole
                // parameter
                // TODO(bradfordcsmith): We could mark this if we determined that all vars created
                // by the pattern are unused.
                continue;
            };

            let var_info = self.traverse_name_node(compiler, param_name_node, fparam_scope);
            if self.var_info_is_removable(var_info) {
                p.set_unused_parameter(compiler, true);
                compiler.report_change_to_enclosing_scope(param_list);
                let record = RemovalLogRecord::for_marking_named_arg(param_name_node, param_list);
                self.removal_log
                    .as_mut()
                    .unwrap()
                    .log(&mut || record.get(compiler));
            }
        }
    }

    /// Strip as many unreferenced args off the end of the function declaration as possible. We
    /// start from the end of the function declaration because removing parameters from the middle
    /// of the param list could mess up the interpretation of parameters being sent over by any
    /// function calls.
    // port: RemoveUnusedCode#maybeRemoveUnusedTrailingParameters
    fn maybe_remove_unused_trailing_parameters(
        &mut self,
        compiler: &mut AbstractCompiler,
        arg_list: NodeId,
        fparam_scope: ScopeId,
    ) {
        check_argument!(
            arg_list.is_param_list(compiler),
            "%s",
            arg_list.to_string(compiler)
        );
        while let Some(last_arg) = arg_list.get_last_child(compiler) {
            let mut arg_node = last_arg;
            if last_arg.is_default_value(compiler) {
                arg_node = last_arg.get_first_child(compiler).unwrap();
                if self.may_have_side_effects(compiler, last_arg.get_last_child(compiler).unwrap())
                {
                    break;
                }
            }

            if arg_node.is_rest(compiler) {
                arg_node = arg_node.get_first_child(compiler).unwrap();
            }

            if arg_node.is_destructuring_pattern(compiler) {
                if arg_node.has_children(compiler) {
                    // TODO(johnlenz): handle the case where there are no assignments.
                    break;
                } else {
                    // Remove empty destructuring patterns and their associated object literal
                    // assignment if it exists and if the right hand side does not have side
                    // effects. Note, a destructuring pattern with a "leftover" property key as in
                    // {a:{}} is not considered empty in this case!
                    NodeUtil::delete_node(compiler, last_arg);
                    let record = RemovalLogRecord::for_destructuring_arg(arg_list);
                    self.removal_log
                        .as_mut()
                        .unwrap()
                        .log(&mut || record.get(compiler));
                    continue;
                }
            }

            let var = self.get_var_for_name_node(compiler, arg_node, fparam_scope);
            let var_info = self.get_var_info(compiler, var);
            if self.var_info_is_removable(var_info) {
                NodeUtil::delete_node(compiler, last_arg);
                let record = RemovalLogRecord::for_named_arg(arg_node, arg_list);
                self.removal_log
                    .as_mut()
                    .unwrap()
                    .log(&mut || record.get(compiler));
            } else {
                break;
            }
        }
    }

    /// Handles a variable reference seen during traversal and returns a `VarInfo` object
    /// appropriate for the given `Var`.
    ///
    /// This is a wrapper for `getVarInfo` that handles additional logic needed when we're getting
    /// the `VarInfo` during traversal.
    // port: RemoveUnusedCode#traverseVar
    fn traverse_var(&mut self, compiler: &mut AbstractCompiler, var: VarId) -> VarInfoId {
        if self.remove_local_vars && var.is_arguments(compiler) {
            // If we are considering removing local variables, that includes parameters.
            // If `arguments` is used in a function we must consider all parameters to be
            // referenced.
            let function_scope = var
                .get_scope(compiler)
                .get_closest_hoist_scope(compiler)
                .unwrap();
            let param_list =
                NodeUtil::get_function_parameters(compiler, function_scope.get_root_node(compiler));
            let mut param = param_list.get_first_child(compiler);
            while let Some(p) = param {
                param = p.get_next(compiler);
                let Some(l_value) = name_of_param(compiler, p) else {
                    continue;
                };

                let param_var = self.get_var_for_name_node(compiler, l_value, function_scope);
                let var_info = self.get_var_info(compiler, param_var);
                self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                    "parameter in function using arguments".to_string()
                });
            }
            // `arguments` is never removable.
            self.canonical_unremovable_var_info
        } else {
            self.get_var_info(compiler, var)
        }
    }

    /// Get the right `VarInfo` object to use for the given `Var`.
    ///
    /// This method is responsible for managing the entries in `varInfoMap`.
    ///
    /// Note: Several `Var`s may share the same `VarInfo` when they should be treated the same
    /// way.
    // port: RemoveUnusedCode#getVarInfo
    fn get_var_info(&mut self, compiler: &mut AbstractCompiler, var: VarId) -> VarInfoId {
        let is_global = var.is_global(compiler);
        if var.is_extern(compiler) {
            let name = var.get_name(compiler);
            self.unremovable_log
                .as_mut()
                .unwrap()
                .log(&mut || format!("{name}: extern"));
            self.canonical_unremovable_var_info
        } else if compiler
            .get_coding_convention()
            .is_exported(&var.get_name(compiler), /* local= */ !is_global)
        {
            let name = var.get_name(compiler);
            self.unremovable_log
                .as_mut()
                .unwrap()
                .log(&mut || format!("{name}: exported by convention"));
            self.canonical_unremovable_var_info
        } else if var.is_arguments(compiler) {
            // No point in logging that we cannot remove "arguments"
            self.canonical_unremovable_var_info
        } else {
            if let Some(var_info) = self.var_info_map.get(&var) {
                return *var_info;
            }
            let var_info =
                self.new_var_info(VarInfo::Real(RealVarInfo::new(var.get_name(compiler))));
            if var
                .get_parent_node(compiler)
                .unwrap()
                .is_param_list(compiler)
            {
                self.var_info_set_has_non_local_or_non_literal_value(var_info);
            }
            // Cannot use canonicalUnremovableVarInfo for the 2 non-removable cases below, because
            // each varInfo needs to track what value is assigned to it for the purpose of
            // correctly allowing or preventing removal of properties set on it.
            if !self.remove_globals && is_global {
                self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                    "not removing globals".to_string()
                });
            } else if !self.remove_local_vars && !is_global {
                self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                    "not removing locals".to_string()
                });
            }
            self.var_info_map.insert(var, var_info);
            var_info
        }
    }
}

/// Return the NAME node associated with a function parameter (the child of a PARAM_LIST), or
/// null if there is no single name.
// port: RemoveUnusedCode#nameOfParam
fn name_of_param(ast: &Ast, param: NodeId) -> Option<NodeId> {
    match param.get_token(ast) {
        Token::NAME => Some(param),
        Token::DEFAULT_VALUE => name_of_param(ast, param.get_first_child(ast).unwrap()),
        Token::ITER_REST => name_of_param(ast, param.get_only_child(ast)),
        Token::ARRAY_PATTERN | Token::OBJECT_PATTERN => None,
        _ => panic!(
            "Unexpected child of PARAM_LIST: {}",
            param.to_string_tree(ast)
        ),
    }
}

impl RemoveUnusedCode {
    /// Removes any vars in the scope that were not referenced. Removes any assignments to those
    /// variables as well.
    // port: RemoveUnusedCode#removeUnreferencedVarsAndPolyfills
    fn remove_unreferenced_vars_and_polyfills(&mut self, compiler: &mut AbstractCompiler) {
        let entries: Vec<(VarId, VarInfoId)> =
            self.var_info_map.iter().map(|(k, v)| (*k, *v)).collect();
        for (var, var_info) in entries {
            if !self.var_info_is_removable(var_info) {
                continue;
            }

            let record = RemovalLogRecord::for_var(var.get_name(compiler));
            self.removal_log
                .as_mut()
                .unwrap()
                .log(&mut || record.get(compiler));
            // Regardless of what happens to the original declaration,
            // we need to remove all assigns, because they may contain references
            // to other unreferenced variables.
            self.var_info_remove_all_removables(compiler, var_info);

            let name_node = var.get_name_node(compiler).unwrap();
            let to_remove = name_node.get_parent(compiler);
            match to_remove {
                Some(to_remove) if !already_removed(compiler, to_remove) => {
                    if NodeUtil::is_function_expression(compiler, to_remove) {
                        // TODO(bradfordcsmith): Add a Removable for this case.
                        if !self.preserve_function_expression_names {
                            let fn_name_node = to_remove.get_first_child(compiler).unwrap();
                            compiler.report_change_to_enclosing_scope(fn_name_node);
                            fn_name_node.set_string(compiler, "");
                        }
                    } else {
                        // Removables are not created for theses cases.
                        // function foo(unused1 = someSideEffectingValue, ...unused2) {}
                        // removeUnreferencedFunctionArgs() is responsible for removing these.
                        // TODO(bradfordcsmith): handle parameter declarations with removables
                        check_state!(
                            to_remove.is_param_list(compiler)
                                || (to_remove
                                    .get_parent(compiler)
                                    .unwrap()
                                    .is_param_list(compiler)
                                    && (to_remove.is_default_value(compiler)
                                        || to_remove.is_rest(compiler))),
                            "unremoved code: %s",
                            to_remove.to_string(compiler)
                        );
                    }
                }
                _ => {
                    // assignedVarInfo.removeAllRemovables () already removed it
                }
            }
        }

        for (key, polyfill) in self.polyfills.values() {
            if self.polyfill_infos[polyfill.0].is_removable {
                let record = RemovalLogRecord::for_polyfill(self.polyfill_info_get_name(polyfill));
                self.removal_log
                    .as_mut()
                    .unwrap()
                    .log(&mut || record.get(compiler));
                let removable = self.polyfill_infos[polyfill.0].removable;
                self.removable_remove(compiler, removable);
                self.polyfills.remove_value(&key, polyfill);
            }
        }
    }

    // port: RemoveUnusedCode#mayHaveSideEffects
    fn may_have_side_effects(&self, compiler: &mut AbstractCompiler, node: NodeId) -> bool {
        // check for @pureOrBreakMyCode on call expressions.
        let js_doc_info = if node.is_call(compiler) {
            node.get_jsdoc_info(compiler)
        } else {
            None
        };
        if let Some(js_doc_info) = js_doc_info
            && js_doc_info.is_pure_or_break_my_code()
        {
            return false;
        }
        self.ast_analyzer.may_have_side_effects(compiler, node)
    }

    /// Makes a new PolyfillInfo, including the correct Removable. Parses the name to determine
    /// whether this is a global, static, or prototype polyfill.
    // port: RemoveUnusedCode#createPolyfillInfo
    fn create_polyfill_info(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        scope: ScopeId,
        name: &str,
    ) -> PolyfillInfoId {
        check_state!(call.get_parent(compiler).unwrap().is_expr_result(compiler));
        // Make the removable and polyfill info.  Add continuations for all arguments.
        let mut builder = RemovableBuilder::new();
        let mut n = call.get_first_child(compiler).unwrap().get_next(compiler);
        while let Some(arg) = n {
            builder = builder.add_continuation(Continuation::new(arg, scope));
            n = arg.get_next(compiler);
        }
        let call_parent = call.get_parent(compiler).unwrap();
        let removable = builder.build_polyfill(self, compiler, call_parent);
        let Some(last_dot) = name.rfind('.') else {
            return self.new_polyfill_info(removable, name, PolyfillInfoKind::Global);
        };
        let mut owner = &name[..last_dot];
        let prop = &name[last_dot + 1..];
        if owner.ends_with(DOT_PROTOTYPE) {
            owner = &owner[..owner.len() - DOT_PROTOTYPE.len()];
            return self.new_polyfill_info(
                removable,
                prop,
                PolyfillInfoKind::PrototypeProperty {
                    polyfill_owner_name: owner.to_string(),
                },
            );
        }
        self.new_polyfill_info(
            removable,
            prop,
            PolyfillInfoKind::StaticProperty {
                polyfill_owner_name: owner.to_string(),
            },
        )
    }

    // port: RemoveUnusedCode#removeExpressionCompletely
    fn remove_expression_completely(
        &mut self,
        compiler: &mut AbstractCompiler,
        expression: NodeId,
    ) {
        check_state!(
            !NodeUtil::is_expression_result_used(compiler, expression),
            "%s",
            expression.to_string(compiler)
        );
        let parent = expression.get_parent(compiler).unwrap();
        if parent.is_expr_result(compiler) {
            NodeUtil::delete_node(compiler, parent);
        } else if parent.is_comma(compiler) {
            // Expression is probably the first child of the comma,
            // but it could be the second if the entire comma expression value is unused.
            let other_child = match expression.get_next(compiler) {
                Some(next) => next,
                None => expression.get_previous(compiler).unwrap(),
            };
            let other_child = other_child.detach(compiler);
            self.replace_node_with(compiler, parent, other_child);
        } else {
            // value isn't needed, but we need to keep the AST valid.
            let zero = IR::number(compiler, 0.0).srcref(compiler, expression);
            self.replace_node_with(compiler, expression, zero);
        }
    }

    // port: RemoveUnusedCode#replaceNodeWith
    fn replace_node_with(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        replacement: NodeId,
    ) {
        compiler.report_change_to_enclosing_scope(n);
        n.replace_with(compiler, replacement);
        NodeUtil::mark_functions_deleted(compiler, n);
    }
}

// port: RemoveUnusedCode#alreadyRemoved
fn already_removed(ast: &Ast, n: NodeId) -> bool {
    let Some(parent) = n.get_parent(ast) else {
        return true;
    };
    if parent.is_root(ast) {
        return false;
    }
    already_removed(ast, parent)
}

/// True for `this.propertyName`
// port: RemoveUnusedCode#isThisDotProperty
fn is_this_dot_property(ast: &Ast, n: NodeId) -> bool {
    NodeUtil::is_normal_or_opt_chain_get_prop(ast, n)
        && n.get_first_child(ast).unwrap().is_this(ast)
}

/// True for `(something).prototype.propertyName`
// port: RemoveUnusedCode#isDotPrototypeDotProperty
fn is_dot_prototype_dot_property(ast: &Ast, n: NodeId) -> bool {
    NodeUtil::is_normal_or_opt_chain_get_prop(ast, n)
        && is_dot_prototype(ast, n.get_first_child(ast).unwrap())
}

/// If `value_node` has the form `qualifiedName || defaultValue` and `qualifiedName` matches
/// `target_node`, return `defaultValue`. Otherwise return `value_node`.
// port: RemoveUnusedCode#maybeUnwrapQnameOrDefaultValueNode
fn maybe_unwrap_qname_or_default_value_node(
    ast: &Ast,
    target_node: NodeId,
    value_node: NodeId,
) -> NodeId {
    if value_node.is_or(ast) && target_node.is_qualified_name(ast) {
        let lhs_of_or = check_not_null!(value_node.get_first_child(ast));
        if lhs_of_or.matches_qualified_name_node(ast, target_node) {
            // We use `matchesQualifiedName` rather than `isEquivalentTo` to properly handle the
            // variable declaration case where the assigned value is a child of the name we want
            // to match.
            return value_node.get_last_child(ast).unwrap();
        }
    }
    value_node
}

// ---------------------------------------------------------------------------------------------
// Removable
// ---------------------------------------------------------------------------------------------

// port: RemoveUnusedCode.Kind
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Kind {
    // X = something;
    VARIABLE,
    // X.propertyName = something;
    // X.prototype.propertyName = something;
    NAMED_PROPERTY,
    // X[expression] = something;
    // X.prototype[expression] = something;
    COMPUTED_PROPERTY,
}

/// The Java subclass of a `Removable`, with the subclass's own fields.
#[derive(Clone, Debug)]
enum RemovableKind {
    UnusedReadReference {
        reference_node: NodeId,
    },
    InstanceofName {
        instanceof_node: NodeId,
    },
    WeakUsageCall {
        weak_call_node: NodeId,
    },
    IncOrDecOp {
        inc_or_dec_node: NodeId,
        to_preserve: Option<NodeId>,
    },
    IndirectAssign {
        root: NodeId,
    },
    Polyfill {
        polyfill_node: NodeId,
        is_patch: bool,
    },
    ClassDeclaration {
        class_declaration_node: NodeId,
    },
    NamedClassExpression {
        class_node: NodeId,
    },
    ClassOrPrototypeNamedProperty {
        property_node: NodeId,
    },
    ObjectDefinePropertiesDefinition {
        property_node: NodeId,
    },
    FunctionDeclaration {
        function_declaration_node: NodeId,
    },
    NameDeclarationStatement {
        declaration_statement: NodeId,
    },
    Assign {
        assign_node: NodeId,
        kind: Kind,
        var_info: Option<VarInfoId>,
    },
    AnonymousPrototypeNamedPropertyAssign {
        assign_node: NodeId,
    },
    ClassSetupCall {
        call_node: NodeId,
        class_defining_call: bool,
    },
    VanillaForNameDeclaration {
        name_node: NodeId,
    },
}

/// Represents a portion of the AST that can be removed.
// port: RemoveUnusedCode.Removable
struct Removable {
    continuations: Vec<Continuation>,

    /// If this object represents an assignment of a value to a property. This is the name of
    /// the property.
    property_name: Option<JsString>,

    /// If this object represents a variable declaration or assignment of a value, this is the
    /// node representing where the value is being stored. e.g. the LHS of an assignment.
    target_node: Option<NodeId>,

    is_prototype_dot_property_reference: bool,
    is_this_dot_property_reference: bool,

    continuations_are_applied: bool,
    is_removed: bool,

    kind: RemovableKind,
}

impl Removable {
    // port: RemoveUnusedCode.Removable#Removable
    fn new(target_node: Option<NodeId>, builder: RemovableBuilder, kind: RemovableKind) -> Self {
        Self {
            continuations: builder.continuations,
            property_name: builder.property_name,
            is_prototype_dot_property_reference: builder.is_prototype_dot_property_reference,
            is_this_dot_property_reference: builder.is_this_dot_property_reference,
            target_node,
            continuations_are_applied: false,
            is_removed: false,
            kind,
        }
    }
}

impl RemoveUnusedCode {
    fn new_removable(&mut self, removable: Removable) -> RemovableId {
        self.removables.push(removable);
        RemovableId(self.removables.len() - 1)
    }

    fn target_node(&self, removable: RemovableId) -> NodeId {
        self.removables[removable.0].target_node.unwrap()
    }

    // port: RemoveUnusedCode.Removable#getPropertyName
    fn removable_get_property_name(&self, ast: &Ast, removable: RemovableId) -> JsString {
        let r = &self.removables[removable.0];
        match r.kind {
            RemovableKind::IndirectAssign { .. } => {
                self.indirect_assign_get_property_name(ast, removable)
            }
            _ => check_not_null!(r.property_name.clone()),
        }
    }

    // port: RemoveUnusedCode.IndirectAssign#getPropertyName
    fn indirect_assign_get_property_name(&self, ast: &Ast, removable: RemovableId) -> JsString {
        let target_node = self.target_node(removable);
        check_state!(
            target_node.is_get_prop(ast),
            "%s",
            target_node.to_string(ast)
        );
        target_node.get_string(ast)
    }

    /// Remove the associated nodes from the AST, unless they've already been removed.
    // port: RemoveUnusedCode.Removable#remove
    fn removable_remove(&mut self, compiler: &mut AbstractCompiler, removable: RemovableId) {
        if !self.removables[removable.0].is_removed {
            self.removables[removable.0].is_removed = true;
            self.removable_remove_internal(compiler, removable);
        }
    }

    /// Remove the associated nodes from the AST.
    // port: RemoveUnusedCode.Removable#removeInternal
    fn removable_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        removable: RemovableId,
    ) {
        match self.removables[removable.0].kind.clone() {
            RemovableKind::UnusedReadReference { reference_node } => {
                self.unused_read_reference_remove_internal(compiler, reference_node)
            }
            RemovableKind::InstanceofName { instanceof_node } => {
                self.instanceof_name_remove_internal(compiler, instanceof_node)
            }
            RemovableKind::WeakUsageCall { weak_call_node } => {
                self.weak_usage_call_remove_internal(compiler, weak_call_node)
            }
            RemovableKind::IncOrDecOp {
                inc_or_dec_node,
                to_preserve,
            } => self.inc_or_dec_op_remove_internal(compiler, inc_or_dec_node, to_preserve),
            RemovableKind::IndirectAssign { root } => {
                self.indirect_assign_remove_internal(compiler, removable, root)
            }
            RemovableKind::Polyfill { polyfill_node, .. } => {
                // port: RemoveUnusedCode.Polyfill#removeInternal
                NodeUtil::delete_node(compiler, polyfill_node);
            }
            RemovableKind::ClassDeclaration {
                class_declaration_node,
            } => {
                // port: RemoveUnusedCode.ClassDeclaration#removeInternal
                NodeUtil::delete_node(compiler, class_declaration_node);
            }
            RemovableKind::NamedClassExpression { class_node } => {
                self.named_class_expression_remove_internal(compiler, class_node)
            }
            RemovableKind::ClassOrPrototypeNamedProperty { property_node } => {
                // port: RemoveUnusedCode.ClassOrPrototypeNamedProperty#removeInternal
                NodeUtil::delete_node(compiler, property_node);
            }
            RemovableKind::ObjectDefinePropertiesDefinition { property_node } => {
                // port: RemoveUnusedCode.ObjectDefinePropertiesDefinition#removeInternal
                NodeUtil::delete_node(compiler, property_node);
            }
            RemovableKind::FunctionDeclaration {
                function_declaration_node,
            } => {
                // port: RemoveUnusedCode.FunctionDeclaration#removeInternal
                NodeUtil::delete_node(compiler, function_declaration_node);
            }
            RemovableKind::NameDeclarationStatement {
                declaration_statement,
            } => self.name_declaration_statement_remove_internal(compiler, declaration_statement),
            RemovableKind::Assign { assign_node, .. } => {
                self.assign_remove_internal(compiler, assign_node)
            }
            RemovableKind::AnonymousPrototypeNamedPropertyAssign { assign_node } => self
                .anonymous_prototype_named_property_assign_remove_internal(compiler, assign_node),
            RemovableKind::ClassSetupCall {
                call_node,
                class_defining_call,
            } => self.class_setup_call_remove_internal(compiler, call_node, class_defining_call),
            RemovableKind::VanillaForNameDeclaration { name_node } => {
                self.vanilla_for_name_declaration_remove_internal(compiler, name_node)
            }
        }
    }

    // port: RemoveUnusedCode.Removable#applyContinuations
    fn removable_apply_continuations(&mut self, removable: RemovableId) {
        let r = &mut self.removables[removable.0];
        if !r.continuations_are_applied {
            r.continuations_are_applied = true;
            for c in r.continuations.drain(..) {
                // Enqueue the continuation for processing.
                // Don't invoke the continuation immediately, because that can lead to concurrent
                // modification of data structures.
                self.worklist.push_back(c);
            }
        }
    }

    /// True if this object represents assignment to a variable.
    // port: RemoveUnusedCode.Removable#isVariableAssignment
    fn removable_is_variable_assignment(&self, ast: &Ast, removable: RemovableId) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.IndirectAssign#isVariableAssignment
            RemovableKind::IndirectAssign { .. } => self.target_node(removable).is_name(ast),
            // port: RemoveUnusedCode.ClassDeclaration#isVariableAssignment
            RemovableKind::ClassDeclaration { .. } => true,
            // port: RemoveUnusedCode.FunctionDeclaration#isVariableAssignment
            RemovableKind::FunctionDeclaration { .. } => true,
            // port: RemoveUnusedCode.NameDeclarationStatement#isVariableAssignment
            RemovableKind::NameDeclarationStatement { .. } => true,
            // port: RemoveUnusedCode.Assign#isVariableAssignment
            RemovableKind::Assign { kind, .. } => kind == Kind::VARIABLE,
            // port: RemoveUnusedCode.VanillaForNameDeclaration#isVariableAssignment
            RemovableKind::VanillaForNameDeclaration { .. } => true,
            _ => false,
        }
    }

    /// True if this object represents a named property, either assignment or declaration.
    // port: RemoveUnusedCode.Removable#isNamedProperty
    fn removable_is_named_property(&self, ast: &Ast, removable: RemovableId) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.IndirectAssign#isNamedProperty
            RemovableKind::IndirectAssign { .. } => self.target_node(removable).is_get_prop(ast),
            _ => self.removables[removable.0].property_name.is_some(),
        }
    }

    /// True if this object represents assignment to a named property.
    ///
    /// This does not include class or object literal member declarations.
    // port: RemoveUnusedCode.Removable#isNamedPropertyAssignment
    fn removable_is_named_property_assignment(&self, ast: &Ast, removable: RemovableId) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.IndirectAssign#isNamedPropertyAssignment
            RemovableKind::IndirectAssign { .. } => self.target_node(removable).is_get_prop(ast),
            // port: RemoveUnusedCode.Assign#isNamedPropertyAssignment
            RemovableKind::Assign { kind, .. } => kind == Kind::NAMED_PROPERTY,
            _ => false,
        }
    }

    // port: RemoveUnusedCode.Removable#isAssignedValueLocal
    fn removable_is_assigned_value_local(&self, ast: &Ast, removable: RemovableId) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.ClassDeclaration#isAssignedValueLocal
            RemovableKind::ClassDeclaration { .. } => true,
            // port: RemoveUnusedCode.FunctionDeclaration#isAssignedValueLocal
            // The declared function is always created locally.
            RemovableKind::FunctionDeclaration { .. } => true,
            RemovableKind::NameDeclarationStatement {
                declaration_statement,
            } => name_declaration_statement_is_assigned_value_local(ast, declaration_statement),
            // port: RemoveUnusedCode.Assign#isAssignedValueLocal
            RemovableKind::Assign { assign_node, .. } => {
                assign_get_local_assigned_value(ast, assign_node).is_some()
            }
            RemovableKind::VanillaForNameDeclaration { name_node } => {
                vanilla_for_name_declaration_is_assigned_value_local(ast, name_node)
            }
            _ => false, // assume non-local by default
        }
    }

    /// Returns the Node representing the local value that is being assigned or `None` if the
    /// value is non-local or cannot be determined.
    // port: RemoveUnusedCode.Removable#getLocalAssignedValue
    fn removable_get_local_assigned_value(
        &self,
        ast: &Ast,
        removable: RemovableId,
    ) -> Option<NodeId> {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.ClassDeclaration#getLocalAssignedValue
            RemovableKind::ClassDeclaration {
                class_declaration_node,
            } => Some(class_declaration_node),
            // port: RemoveUnusedCode.FunctionDeclaration#getLocalAssignedValue
            RemovableKind::FunctionDeclaration {
                function_declaration_node,
            } => Some(function_declaration_node),
            RemovableKind::NameDeclarationStatement {
                declaration_statement,
            } => name_declaration_statement_get_local_assigned_value(ast, declaration_statement),
            RemovableKind::Assign { assign_node, .. } => {
                assign_get_local_assigned_value(ast, assign_node)
            }
            RemovableKind::VanillaForNameDeclaration { name_node } => {
                vanilla_for_name_declaration_get_local_assigned_value(ast, name_node)
            }
            _ => None,
        }
    }

    /// Is this a direct assignment to `varName.prototype`?
    // port: RemoveUnusedCode.Removable#isPrototypeAssignment
    fn removable_is_prototype_assignment(&self, ast: &Ast, removable: RemovableId) -> bool {
        self.removable_is_named_property_assignment(ast, removable)
            && self.removables[removable.0].property_name.as_ref().unwrap() == "prototype"
    }

    /// Is this an assignment to a property on a prototype object?
    // port: RemoveUnusedCode.Removable#isPrototypeDotPropertyReference
    fn removable_is_prototype_dot_property_reference(&self, removable: RemovableId) -> bool {
        self.removables[removable.0].is_prototype_dot_property_reference
    }

    // port: RemoveUnusedCode.Removable#isClassOrPrototypeNamedProperty
    fn removable_is_class_or_prototype_named_property(
        &self,
        ast: &Ast,
        removable: RemovableId,
    ) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.ClassOrPrototypeNamedProperty#isClassOrPrototypeNamedProperty
            RemovableKind::ClassOrPrototypeNamedProperty { .. } => {
                !self.removable_is_static_property(ast, removable)
            }
            _ => false,
        }
    }

    // port: RemoveUnusedCode.Removable#isPrototypeProperty
    fn removable_is_prototype_property(&self, ast: &Ast, removable: RemovableId) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.AnonymousPrototypeNamedPropertyAssign#isPrototypeProperty
            RemovableKind::AnonymousPrototypeNamedPropertyAssign { .. } => true,
            _ => {
                self.removable_is_prototype_dot_property_reference(removable)
                    || self.removable_is_class_or_prototype_named_property(ast, removable)
            }
        }
    }

    // port: RemoveUnusedCode.Removable#isThisDotPropertyReference
    fn removable_is_this_dot_property_reference(&self, ast: &Ast, removable: RemovableId) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.IndirectAssign#isThisDotPropertyReference
            RemovableKind::IndirectAssign { .. } => {
                is_this_dot_property(ast, self.target_node(removable))
            }
            _ => self.removables[removable.0].is_this_dot_property_reference,
        }
    }

    // port: RemoveUnusedCode.Removable#isObjectDefinePropertiesDefinition
    fn removable_is_object_define_properties_definition(&self, removable: RemovableId) -> bool {
        // port: RemoveUnusedCode.ObjectDefinePropertiesDefinition#isObjectDefinePropertiesDefinition
        matches!(
            self.removables[removable.0].kind,
            RemovableKind::ObjectDefinePropertiesDefinition { .. }
        )
    }

    // TODO(b/134610338): Combine this method with `isPrototypeProperty`.
    // port: RemoveUnusedCode.Removable#isStaticProperty
    fn removable_is_static_property(&self, ast: &Ast, removable: RemovableId) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.ClassOrPrototypeNamedProperty#isStaticProperty
            RemovableKind::ClassOrPrototypeNamedProperty { property_node } => {
                property_node.is_static_member(ast)
            }
            // port: RemoveUnusedCode.Assign#isStaticProperty
            RemovableKind::Assign { kind, var_info, .. } => {
                if kind == Kind::NAMED_PROPERTY
                    && let Some(var_info) = var_info
                    && self.var_info_has_function_or_class_literal_value(var_info)
                {
                    // We have either
                    // `classOrFunctionVar.prop = something;` which is static
                    // or
                    // `classOrFunctionVar.prototype.prop = something;` which is not.
                    self.target_node(removable)
                        .get_first_child(ast)
                        .unwrap()
                        .is_name(ast)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Would a nonlocal or nonliteral value prevent removal of a variable associated with this
    /// `Removable`?
    ///
    /// True if the nature of this removable is such that a variable associated with it must not
    /// be removed if its value or its prototype is not a local, literal value.
    // port: RemoveUnusedCode.Removable#preventsRemovalOfVariableWithNonLocalValueOrPrototype
    fn removable_prevents_removal_of_variable_with_non_local_value_or_prototype(
        &self,
        ast: &Ast,
        removable: RemovableId,
    ) -> bool {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.InstanceofName#preventsRemovalOfVariableWithNonLocalValueOrPrototype
            // If we aren't sure where X comes from and what aliases it might have, we cannot be
            // sure there are no instances of it.
            RemovableKind::InstanceofName { .. } => true,
            // port: RemoveUnusedCode.IndirectAssign#preventsRemovalOfVariableWithNonLocalValueOrPrototype
            RemovableKind::IndirectAssign { .. } => {
                let target_node = self.target_node(removable);
                if target_node.is_get_prop(ast) {
                    let get_prop_lhs = target_node.get_first_child(ast).unwrap();
                    // assignment to varName.property or varName.prototype.property
                    // cannot be removed unless varName and varName.prototype have literal, local
                    // values.
                    get_prop_lhs.is_name(ast) || is_name_dot_prototype(ast, get_prop_lhs)
                } else {
                    false
                }
            }
            // port: RemoveUnusedCode.Assign#preventsRemovalOfVariableWithNonLocalValueOrPrototype
            // If we don't know where the variable comes from or where it may go, then we don't
            // know whether it is safe to remove assignments to properties on it.
            // port: RemoveUnusedCode.Assign#isComputedPropertyAssignment (`kind == COMPUTED_PROPERTY`)
            RemovableKind::Assign { kind, .. } => {
                kind == Kind::NAMED_PROPERTY || kind == Kind::COMPUTED_PROPERTY
            }
            // port: RemoveUnusedCode.ClassSetupCall#preventsRemovalOfVariableWithNonLocalValueOrPrototype
            // If we aren't sure where X comes from and what aliases it might have, we cannot be
            // sure it's safe to remove the class setup for it.
            RemovableKind::ClassSetupCall { .. } => true,
            _ => false,
        }
    }

    // port: RemoveUnusedCode.Removable#toString
    #[allow(dead_code)] // Java's toString, for debugging
    fn removable_to_string(&self, ast: &Ast, removable: RemovableId) -> String {
        match self.removables[removable.0].kind {
            // port: RemoveUnusedCode.UnusedReadReference#toString
            RemovableKind::UnusedReadReference { reference_node } => {
                format!("UnusedReadReference:{}", reference_node.to_string(ast))
            }
            // port: RemoveUnusedCode.InstanceofName#toString
            RemovableKind::InstanceofName { instanceof_node } => {
                format!("InstanceofName:{}", instanceof_node.to_string(ast))
            }
            // port: RemoveUnusedCode.WeakUsageCall#toString
            RemovableKind::WeakUsageCall { weak_call_node } => {
                format!("weakUsageCall:{}", weak_call_node.to_string(ast))
            }
            // port: RemoveUnusedCode.IncOrDecOp#toString
            RemovableKind::IncOrDecOp {
                inc_or_dec_node, ..
            } => format!("IncOrDecOp:{}", inc_or_dec_node.to_string(ast)),
            // port: RemoveUnusedCode.Polyfill#toString
            RemovableKind::Polyfill {
                polyfill_node,
                is_patch,
            } => format!(
                "{}{}",
                if is_patch { "Patch:" } else { "Polyfill:" },
                polyfill_node.to_string(ast)
            ),
            // port: RemoveUnusedCode.ClassDeclaration#toString
            RemovableKind::ClassDeclaration {
                class_declaration_node,
            } => format!("ClassDeclaration:{}", class_declaration_node.to_string(ast)),
            // port: RemoveUnusedCode.NamedClassExpression#toString
            RemovableKind::NamedClassExpression { class_node } => {
                format!("NamedClassExpression:{}", class_node.to_string(ast))
            }
            // port: RemoveUnusedCode.ClassOrPrototypeNamedProperty#toString
            RemovableKind::ClassOrPrototypeNamedProperty { property_node } => {
                format!(
                    "ClassOrPrototypeNamedProperty:{}",
                    property_node.to_string(ast)
                )
            }
            // port: RemoveUnusedCode.FunctionDeclaration#toString
            RemovableKind::FunctionDeclaration {
                function_declaration_node,
            } => format!(
                "FunctionDeclaration:{}",
                function_declaration_node.to_string(ast)
            ),
            // port: RemoveUnusedCode.NameDeclarationStatement#toString
            RemovableKind::NameDeclarationStatement {
                declaration_statement,
            } => format!("NameDeclStmt:{}", declaration_statement.to_string(ast)),
            // port: RemoveUnusedCode.Assign#toString
            RemovableKind::Assign { assign_node, .. } => {
                format!("Assign:{}", assign_node.to_string(ast))
            }
            // port: RemoveUnusedCode.AnonymousPrototypeNamedPropertyAssign#toString
            RemovableKind::AnonymousPrototypeNamedPropertyAssign { assign_node } => format!(
                "AnonymousPrototypeNamedPropertyAssign:{}",
                assign_node.to_string(ast)
            ),
            // port: RemoveUnusedCode.ClassSetupCall#toString
            RemovableKind::ClassSetupCall { call_node, .. } => {
                format!("ClassSetupCall:{}", call_node.to_string(ast))
            }
            // Object#toString (no override): class name and identity hash
            RemovableKind::IndirectAssign { .. }
            | RemovableKind::ObjectDefinePropertiesDefinition { .. }
            | RemovableKind::VanillaForNameDeclaration { .. } => {
                format!("Removable@{}", removable.0)
            }
        }
    }
}

// port: RemoveUnusedCode.RemovableBuilder
#[derive(Default)]
struct RemovableBuilder {
    continuations: Vec<Continuation>,

    property_name: Option<JsString>,
    is_prototype_dot_property_reference: bool,
    is_this_dot_property_reference: bool,
}

impl RemovableBuilder {
    fn new() -> Self {
        Self::default()
    }

    // port: RemoveUnusedCode.RemovableBuilder#addContinuation
    fn add_continuation(mut self, continuation: Continuation) -> Self {
        self.continuations.push(continuation);
        self
    }

    // port: RemoveUnusedCode.RemovableBuilder#setIsPrototypeDotPropertyReference
    fn set_is_prototype_dot_property_reference(mut self, value: bool) -> Self {
        self.is_prototype_dot_property_reference = value;
        self
    }

    // port: RemoveUnusedCode.RemovableBuilder#setIsThisDotPropertyReference
    fn set_is_this_dot_property_reference(mut self, value: bool) -> Self {
        self.is_this_dot_property_reference = value;
        self
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildIndirectAssign
    fn build_indirect_assign(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        root: NodeId,
        target_node: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.IndirectAssign#IndirectAssign
        let removable = Removable::new(
            Some(target_node),
            self,
            RemovableKind::IndirectAssign { root },
        );
        let root_parent = root.get_parent(ast).unwrap();
        check_state!(
            root_parent.is_destructuring_pattern(ast) || root_parent.is_param_list(ast),
            "%s",
            root_parent.to_string(ast)
        );
        check_state!(
            target_node.is_name(ast) || target_node.is_get_prop(ast),
            "%s",
            target_node.to_string(ast)
        );
        rc.new_removable(removable)
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildPolyfill
    fn build_polyfill(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        polyfill_node: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.Polyfill#Polyfill
        let is_patch = polyfill_node
            .get_first_first_child(ast)
            .unwrap()
            .get_string(ast)
            .to_string_lossy()
            .contains("patch");
        rc.new_removable(Removable::new(
            /* targetNode= */ None,
            self,
            RemovableKind::Polyfill {
                polyfill_node,
                is_patch,
            },
        ))
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildClassDeclaration
    fn build_class_declaration(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        class_node: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.ClassDeclaration#ClassDeclaration
        // First child of the CLASS is the NAME node for the name to which the class is being
        // assigned.
        rc.new_removable(Removable::new(
            class_node.get_first_child(ast),
            self,
            RemovableKind::ClassDeclaration {
                class_declaration_node: class_node,
            },
        ))
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildNamedClassExpression
    fn build_named_class_expression(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        class_node: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.NamedClassExpression#NamedClassExpression
        rc.new_removable(Removable::new(
            class_node.get_first_child(ast),
            self,
            RemovableKind::NamedClassExpression { class_node },
        ))
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildClassOrPrototypeNamedProperty
    fn build_class_or_prototype_named_property(
        mut self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        property_node: NodeId,
    ) -> RemovableId {
        check_argument!(
            property_node.is_member_function_def(ast)
                || property_node.is_member_field_def(ast)
                || NodeUtil::is_get_or_set_key(ast, property_node)
                || (property_node.is_string_key(ast) && !property_node.is_quoted_string_key(ast)),
            "%s",
            property_node.to_string(ast)
        );
        self.property_name = Some(property_node.get_string(ast));
        // port: RemoveUnusedCode.ClassOrPrototypeNamedProperty#ClassOrPrototypeNamedProperty
        rc.new_removable(Removable::new(
            /* targetNode= */ None,
            self,
            RemovableKind::ClassOrPrototypeNamedProperty { property_node },
        ))
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildObjectDefinePropertiesDefinition
    fn build_object_define_properties_definition(
        mut self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        property_node: NodeId,
    ) -> RemovableId {
        self.property_name = Some(property_node.get_string(ast));
        // port: RemoveUnusedCode.ObjectDefinePropertiesDefinition#ObjectDefinePropertiesDefinition
        rc.new_removable(Removable::new(
            /* targetNode= */ None,
            self,
            RemovableKind::ObjectDefinePropertiesDefinition { property_node },
        ))
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildFunctionDeclaration
    fn build_function_declaration(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        function_node: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.FunctionDeclaration#FunctionDeclaration
        rc.new_removable(Removable::new(
            function_node.get_first_child(ast),
            self,
            RemovableKind::FunctionDeclaration {
                function_declaration_node: function_node,
            },
        ))
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildNameDeclarationStatement
    fn build_name_declaration_statement(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        declaration_statement: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.NameDeclarationStatement#NameDeclarationStatement
        let removable = Removable::new(
            Some(declaration_statement.get_only_child(ast)),
            self,
            RemovableKind::NameDeclarationStatement {
                declaration_statement,
            },
        );
        check_argument!(
            NodeUtil::is_name_declaration(ast, Some(declaration_statement)),
            "%s",
            declaration_statement.to_string(ast)
        );
        rc.new_removable(removable)
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildNamedPropertyAssign(Node,Node)
    fn build_named_property_assign(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        assign_node: NodeId,
        property_node: NodeId,
    ) -> RemovableId {
        self.build_named_property_assign_with_var_info(rc, ast, assign_node, property_node, None)
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildNamedPropertyAssign(Node,Node,VarInfo)
    fn build_named_property_assign_with_var_info(
        mut self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        assign_node: NodeId,
        property_node: NodeId,
        var_info: Option<VarInfoId>,
    ) -> RemovableId {
        self.property_name = Some(property_node.get_string(ast));
        Self::new_assign(
            self,
            rc,
            ast,
            assign_node,
            Kind::NAMED_PROPERTY,
            Some(property_node),
            var_info,
        )
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildComputedPropertyAssign
    fn build_computed_property_assign(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        assign_node: NodeId,
        property_node: NodeId,
        var_info: VarInfoId,
    ) -> RemovableId {
        Self::new_assign(
            self,
            rc,
            ast,
            assign_node,
            Kind::COMPUTED_PROPERTY,
            Some(property_node),
            Some(var_info),
        )
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildVariableAssign
    fn build_variable_assign(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        assign_node: NodeId,
        var_info: VarInfoId,
    ) -> RemovableId {
        Self::new_assign(
            self,
            rc,
            ast,
            assign_node,
            Kind::VARIABLE,
            /* propertyNode= */ None,
            Some(var_info),
        )
    }

    // port: RemoveUnusedCode.Assign#Assign
    fn new_assign(
        builder: RemovableBuilder,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        assign_node: NodeId,
        kind: Kind,
        property_node: Option<NodeId>,
        var_info: Option<VarInfoId>,
    ) -> RemovableId {
        let removable = Removable::new(
            assign_node.get_first_child(ast),
            builder,
            RemovableKind::Assign {
                assign_node,
                kind,
                var_info,
            },
        );
        check_argument!(
            NodeUtil::is_assignment_op(ast, assign_node),
            "%s",
            assign_node.to_string(ast)
        );
        let property_node_string = || match property_node {
            None => "null".to_string(),
            Some(n) => n.to_string(ast),
        };
        if kind == Kind::VARIABLE {
            check_argument!(
                property_node.is_none(),
                "got property node for simple variable assignment: %s",
                property_node_string()
            );
            check_argument!(
                var_info.is_some(),
                "missing VarInfo for variable assignment: %s",
                property_node_string()
            );
        } else {
            check_argument!(property_node.is_some(), "missing property node");
            if kind == Kind::NAMED_PROPERTY {
                check_argument!(
                    property_node.unwrap().is_get_prop(ast),
                    "property name is not a GETPROP: %s",
                    property_node_string()
                );
            }
        }
        rc.new_removable(removable)
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildClassSetupCall(Node)
    fn build_class_setup_call(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        call_node: NodeId,
    ) -> RemovableId {
        self.build_class_setup_call_with_class_defining_call(
            rc, ast, call_node, /* classDefiningCall= */ false,
        )
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildClassSetupCall(Node,boolean)
    fn build_class_setup_call_with_class_defining_call(
        self,
        rc: &mut RemoveUnusedCode,
        _ast: &Ast,
        call_node: NodeId,
        class_defining_call: bool,
    ) -> RemovableId {
        // port: RemoveUnusedCode.ClassSetupCall#ClassSetupCall
        rc.new_removable(Removable::new(
            /* targetNode= */ None,
            self,
            RemovableKind::ClassSetupCall {
                call_node,
                class_defining_call,
            },
        ))
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildVanillaForNameDeclaration
    fn build_vanilla_for_name_declaration(
        self,
        rc: &mut RemoveUnusedCode,
        _ast: &Ast,
        name_node: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.VanillaForNameDeclaration#VanillaForNameDeclaration
        rc.new_removable(Removable::new(
            Some(name_node),
            self,
            RemovableKind::VanillaForNameDeclaration { name_node },
        ))
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildAnonymousPrototypeNamedPropertyAssign
    fn build_anonymous_prototype_named_property_assign(
        mut self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        assign_node: NodeId,
        property_name: JsString,
    ) -> RemovableId {
        self.property_name = Some(property_name);
        // port: RemoveUnusedCode.AnonymousPrototypeNamedPropertyAssign#AnonymousPrototypeNamedPropertyAssign
        check_not_null!(self.property_name.as_ref());
        let removable = Removable::new(
            assign_node.get_first_child(ast),
            self,
            RemovableKind::AnonymousPrototypeNamedPropertyAssign { assign_node },
        );
        check_argument!(assign_node.is_assign(ast), "%s", assign_node.to_string(ast));
        rc.new_removable(removable)
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildIncOrDepOp
    fn build_inc_or_dep_op(
        mut self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        inc_or_dec_op: NodeId,
        property_node: NodeId,
        to_preseve: Option<NodeId>,
    ) -> RemovableId {
        self.property_name = Some(property_node.get_string(ast));
        // port: RemoveUnusedCode.IncOrDecOp#IncOrDecOp
        let removable = Removable::new(
            Some(inc_or_dec_op.get_only_child(ast)),
            self,
            RemovableKind::IncOrDecOp {
                inc_or_dec_node: inc_or_dec_op,
                to_preserve: to_preseve,
            },
        );
        check_argument!(
            inc_or_dec_op.is_inc(ast) || inc_or_dec_op.is_dec(ast),
            "%s",
            inc_or_dec_op.to_string(ast)
        );

        let arg = inc_or_dec_op.get_only_child(ast);
        // TODO(bradfordcsmith): handle `name;` and `name.property;` references
        check_state!(
            is_this_dot_property(ast, arg) || is_dot_prototype_dot_property(ast, arg),
            "%s",
            arg.to_string(ast)
        );
        rc.new_removable(removable)
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildUnusedReadReference
    fn build_unused_read_reference(
        mut self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        reference_node: NodeId,
        property_node: NodeId,
    ) -> RemovableId {
        self.property_name = Some(property_node.get_string(ast));
        // port: RemoveUnusedCode.UnusedReadReference#UnusedReadReference
        let removable = Removable::new(
            /* targetNode= */ None,
            self,
            RemovableKind::UnusedReadReference { reference_node },
        );
        // TODO(bradfordcsmith): handle `name;` and `name.property;` references
        check_state!(
            is_this_dot_property(ast, reference_node)
                || is_dot_prototype_dot_property(ast, reference_node),
            "%s",
            reference_node.to_string(ast)
        );
        rc.new_removable(removable)
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildInstanceofName
    fn build_instanceof_name(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        instanceof_node: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.InstanceofName#InstanceofName
        let removable = Removable::new(
            /* targetNode= */ None,
            self,
            RemovableKind::InstanceofName { instanceof_node },
        );
        check_argument!(
            instanceof_node.is_instance_of(ast),
            "%s",
            instanceof_node.to_string(ast)
        );
        rc.new_removable(removable)
    }

    // port: RemoveUnusedCode.RemovableBuilder#buildWeakUsageCall
    fn build_weak_usage_call(
        self,
        rc: &mut RemoveUnusedCode,
        ast: &Ast,
        weak_usage_call: NodeId,
    ) -> RemovableId {
        // port: RemoveUnusedCode.WeakUsageCall#WeakUsageCall
        let removable = Removable::new(
            /* targetNode= */ None,
            self,
            RemovableKind::WeakUsageCall {
                weak_call_node: weak_usage_call,
            },
        );
        check_argument!(
            weak_usage_call.is_call(ast),
            "%s",
            weak_usage_call.to_string(ast)
        );
        rc.new_removable(removable)
    }
}

impl RemoveUnusedCode {
    // port: RemoveUnusedCode.UnusedReadReference#removeInternal
    fn unused_read_reference_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        reference_node: NodeId,
    ) {
        if !already_removed(compiler, reference_node) {
            if is_this_dot_property(compiler, reference_node) {
                self.remove_expression_completely(compiler, reference_node);
            } else {
                check_state!(
                    is_dot_prototype_dot_property(compiler, reference_node),
                    "%s",
                    reference_node.to_string(compiler)
                );
                // objExpression.prototype.propertyName
                let obj_expression = reference_node.get_first_first_child(compiler).unwrap();
                if self.may_have_side_effects(compiler, obj_expression) {
                    let detached = obj_expression.detach(compiler);
                    self.replace_node_with(compiler, reference_node, detached);
                } else {
                    self.remove_expression_completely(compiler, reference_node);
                }
            }
        }
    }

    // port: RemoveUnusedCode.InstanceofName#removeInternal
    fn instanceof_name_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        instanceof_node: NodeId,
    ) {
        if !already_removed(compiler, instanceof_node) {
            let lhs = instanceof_node.get_first_child(compiler).unwrap();
            let false_node = IR::false_node(compiler).srcref(compiler, instanceof_node);
            if self.may_have_side_effects(compiler, lhs) {
                let lhs = lhs.detach(compiler);
                let comma = IR::comma(compiler, lhs, false_node).srcref(compiler, instanceof_node);
                self.replace_node_with(compiler, instanceof_node, comma);
            } else {
                self.replace_node_with(compiler, instanceof_node, false_node);
            }
        }
    }

    // port: RemoveUnusedCode.WeakUsageCall#removeInternal
    fn weak_usage_call_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        weak_call_node: NodeId,
    ) {
        if !already_removed(compiler, weak_call_node) {
            let undefined_node = NodeUtil::new_undefined_node(compiler, Some(weak_call_node));
            self.replace_node_with(compiler, weak_call_node, undefined_node);
        }
    }

    // port: RemoveUnusedCode.IncOrDecOp#removeInternal
    fn inc_or_dec_op_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        inc_or_dec_node: NodeId,
        to_preserve: Option<NodeId>,
    ) {
        if already_removed(compiler, inc_or_dec_node) {
            return;
        }

        let arg = inc_or_dec_node.get_only_child(compiler);
        check_state!(arg.is_get_prop(compiler), "%s", arg.to_string(compiler));

        match to_preserve {
            None => self.remove_expression_completely(compiler, inc_or_dec_node),
            Some(to_preserve) => {
                let detached = to_preserve.detach(compiler);
                self.replace_node_with(compiler, inc_or_dec_node, detached);
            }
        }
    }

    // port: RemoveUnusedCode.IndirectAssign#removeInternal
    fn indirect_assign_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        removable: RemovableId,
        root: NodeId,
    ) {
        if !already_removed(compiler, self.target_node(removable)) {
            self.indirect_assign_remove_root(compiler, root);
        }
    }

    // port: RemoveUnusedCode.IndirectAssign#removeRoot
    fn indirect_assign_remove_root(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        let root_parent = root.get_parent(compiler).unwrap();

        match root_parent.get_token(compiler) {
            Token::ARRAY_PATTERN => {
                // [a, root, b] = something;
                // [a, root] = something;
                // Replace root with an empty node to avoid messing up the order of patterns,
                // then clean up trailing empties.
                let empty = IR::empty(compiler).srcref(compiler, root);
                self.replace_node_with(compiler, root, empty);
                // We prefer `[a, b]` to `[a, b, , , , ]`
                // So remove any trailing empty nodes.
                let mut maybe_empty = root_parent.get_last_child(compiler);
                while let Some(e) = maybe_empty
                    && e.is_empty(compiler)
                {
                    e.detach(compiler);
                    maybe_empty = root_parent.get_last_child(compiler);
                }
                compiler.report_change_to_enclosing_scope(root_parent);
                // TODO(bradfordcsmith): If the array pattern is now empty, try to remove it
                // entirely.
            }
            Token::PARAM_LIST => {
                if !root.is_default_value(compiler) {
                    // removeUnreferencedFunctionArgs() is responsible for removal of function
                    // parameter positions, so all we can do here is remove the default value.
                    // NOTE: traverseRest() avoids creating a removable for a rest parameter.
                    // TODO(bradfordcsmith): Handle parameter removal consistently with other
                    // removals.
                    return;
                }

                // function(removableName = removableValue)
                compiler.report_change_to_enclosing_scope(root_parent);
                // preserve the slot in the parameter list
                let name = root.get_first_child(compiler).unwrap();
                check_state!(name.is_name(compiler));
                if root_parent.get_last_child(compiler) == Some(root)
                    && self.remove_globals
                    && self.can_remove_parameters(compiler, root_parent)
                {
                    // function(p1, removableName = removableDefault)
                    // and we're allowed to remove the parameter entirely
                    root.detach(compiler);
                } else {
                    // function(removableName = removableDefault, otherParam)
                    // or removableName is at the end, but cannot be completely removed.
                    let name = name.detach(compiler);
                    root.replace_with(compiler, name);
                }
                NodeUtil::mark_functions_deleted(compiler, root);
            }
            Token::OBJECT_PATTERN => {
                // ({ [propExpression]: root } = something)
                // becomes
                // ({} = something)
                NodeUtil::delete_node(compiler, root)
            }
            _ => panic!(
                "Unexpected parent of indirect assignment: {}",
                root_parent.to_string_tree(compiler)
            ),
        }
    }

    // port: RemoveUnusedCode.NamedClassExpression#removeInternal
    fn named_class_expression_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        class_node: NodeId,
    ) {
        if !already_removed(compiler, class_node) {
            let name_node = class_node.get_first_child(compiler).unwrap();
            if !name_node.is_empty(compiler) {
                // Just empty the class's name. If the expression is assigned to an unused
                // variable, then the whole class might still be removed as part of that
                // assignment.
                let empty = IR::empty(compiler).srcref(compiler, name_node);
                name_node.replace_with(compiler, empty);
                compiler.report_change_to_enclosing_scope(class_node);
            }
        }
    }

    // port: RemoveUnusedCode.NameDeclarationStatement#removeInternal
    fn name_declaration_statement_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        declaration_statement: NodeId,
    ) {
        let name_node = declaration_statement.get_only_child(compiler);
        let value_node = name_node.get_first_child(compiler);
        if let Some(value_node) = value_node
            && self.may_have_side_effects(compiler, value_node)
        {
            compiler.report_change_to_enclosing_scope(declaration_statement);
            value_node.detach(compiler);
            let expr_result = IR::expr_result(compiler, value_node).srcref(compiler, value_node);
            declaration_statement.replace_with(compiler, expr_result);
        } else {
            NodeUtil::delete_node(compiler, declaration_statement);
        }
    }

    /// Replace the current assign with its right hand side.
    // port: RemoveUnusedCode.Assign#removeInternal
    fn assign_remove_internal(&mut self, compiler: &mut AbstractCompiler, assign_node: NodeId) {
        if already_removed(compiler, assign_node) {
            return;
        }
        let parent = assign_node.get_parent(compiler).unwrap();
        compiler.report_change_to_enclosing_scope(parent);
        let lhs = assign_node.get_first_child(compiler).unwrap();
        let rhs = assign_node.get_second_child(compiler).unwrap();
        let must_preserve_rhs = self.may_have_side_effects(compiler, rhs)
            || NodeUtil::is_expression_result_used(compiler, assign_node);
        let must_preserve_get_elm_expr = lhs.is_get_elem(compiler)
            && self.may_have_side_effects(compiler, lhs.get_last_child(compiler).unwrap());

        if must_preserve_rhs && must_preserve_get_elm_expr {
            let key = lhs.get_last_child(compiler).unwrap().detach(compiler);
            let rhs = rhs.detach(compiler);
            let replacement = IR::comma(compiler, key, rhs).srcref(compiler, assign_node);
            self.replace_node_with(compiler, assign_node, replacement);
        } else if must_preserve_get_elm_expr {
            let key = lhs.get_last_child(compiler).unwrap().detach(compiler);
            self.replace_node_with(compiler, assign_node, key);
        } else if must_preserve_rhs {
            let rhs = rhs.detach(compiler);
            self.replace_node_with(compiler, assign_node, rhs);
        } else {
            self.remove_expression_completely(compiler, assign_node);
        }
    }

    // port: RemoveUnusedCode.AnonymousPrototypeNamedPropertyAssign#removeInternal
    fn anonymous_prototype_named_property_assign_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        assign_node: NodeId,
    ) {
        if already_removed(compiler, assign_node) {
            return;
        }
        let parent = assign_node.get_parent(compiler).unwrap();
        compiler.report_change_to_enclosing_scope(parent);
        let lhs = assign_node.get_first_child(compiler).unwrap();
        let rhs = assign_node.get_last_child(compiler).unwrap();

        check_state!(lhs.is_get_prop(compiler), "%s", lhs.to_string(compiler));
        let obj_dot_prototype = lhs.get_first_child(compiler).unwrap();
        check_state!(
            obj_dot_prototype.is_get_prop(compiler),
            "%s",
            obj_dot_prototype.to_string(compiler)
        );
        let obj_expression = obj_dot_prototype.get_first_child(compiler).unwrap();
        check_state!(
            obj_dot_prototype.get_string_ref(compiler) == "prototype",
            "%s",
            obj_dot_prototype.to_string(compiler)
        );

        let must_preserve_rhs = self.may_have_side_effects(compiler, rhs)
            || NodeUtil::is_expression_result_used(compiler, assign_node);
        let must_preserve_obj_expression = self.may_have_side_effects(compiler, obj_expression);

        if must_preserve_rhs && must_preserve_obj_expression {
            let obj_expression = obj_expression.detach(compiler);
            let rhs = rhs.detach(compiler);
            let replacement =
                IR::comma(compiler, obj_expression, rhs).srcref(compiler, assign_node);
            self.replace_node_with(compiler, assign_node, replacement);
        } else if must_preserve_obj_expression {
            let obj_expression = obj_expression.detach(compiler);
            self.replace_node_with(compiler, assign_node, obj_expression);
        } else if must_preserve_rhs {
            let rhs = rhs.detach(compiler);
            self.replace_node_with(compiler, assign_node, rhs);
        } else {
            self.remove_expression_completely(compiler, assign_node);
        }
    }

    // port: RemoveUnusedCode.ClassSetupCall#removeInternal
    fn class_setup_call_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        class_defining_call: bool,
    ) {
        let parent = call_node.get_parent(compiler).unwrap();

        let mut replacement: Option<NodeId> = None;
        // Need to keep call args that have side effects.
        // Easiest thing to do is break apart the call node as we go.
        // First child is the callee (aka. Object.defineProperties or equivalent)
        call_node.remove_first_child(compiler);
        while let Some(arg) = call_node.get_last_child(compiler) {
            arg.detach(compiler);
            // If this is a class defining call, the arguments are well defined and verified so
            // they are always safe to drop.
            if !class_defining_call && self.may_have_side_effects(compiler, arg) {
                replacement = Some(match replacement {
                    None => arg,
                    Some(replacement) => {
                        IR::comma(compiler, arg, replacement).srcref(compiler, call_node)
                    }
                });
            } else {
                NodeUtil::mark_functions_deleted(compiler, arg);
            }
        }

        // This can be part of an arbitrary expression but the results must be unused.
        if let Some(replacement) = replacement {
            self.replace_node_with(compiler, call_node, replacement);
        } else if parent.is_expr_result(compiler) {
            NodeUtil::delete_node(compiler, parent);
        } else {
            // We have been asked to remove the value inside an expression. This will only happen
            // if we know the result of this sub-expression is otherwise unused (doesn't change
            // the result of the expression when removed).
            if parent.is_comma(compiler) || parent.is_and(compiler) || parent.is_or(compiler) {
                if parent.get_first_child(compiler) == Some(call_node) {
                    // `(goog.inherits(A, B), something)` -> `something`
                    let rhs = check_not_null!(call_node.get_next(compiler));
                    compiler.report_change_to_enclosing_scope(parent);
                    let rhs = rhs.detach(compiler);
                    parent.replace_with(compiler, rhs);
                } else {
                    // `(something, Object.defineProperties(A, B))` -> `something`
                    let lhs = parent.get_first_child(compiler).unwrap();
                    compiler.report_change_to_enclosing_scope(parent);
                    let lhs = lhs.detach(compiler);
                    parent.replace_with(compiler, lhs);
                }
            } else {
                // `x ? Object.defineProperties(A, B) : something` -> `x ? 0 : something`
                // Leave simplifying arbitrary expressions to the peephole passes.
                compiler.report_change_to_enclosing_scope(parent);
                let zero = IR::number(compiler, 0.0);
                call_node.replace_with(compiler, zero);
            }
        }
    }

    // port: RemoveUnusedCode.VanillaForNameDeclaration#removeInternal
    fn vanilla_for_name_declaration_remove_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        name_node: NodeId,
    ) {
        let declaration = check_not_null!(name_node.get_parent(compiler));
        compiler.report_change_to_enclosing_scope(declaration);
        // NOTE: We don't need to preserve the initializer value, because we currently do not
        //     remove for-loop vars whose initializing values have side effects.
        if name_node.get_previous(compiler).is_none() && name_node.get_next(compiler).is_none() {
            // only child, so we can remove the whole declaration
            let empty = IR::empty(compiler).srcref(compiler, declaration);
            declaration.replace_with(compiler, empty);
        } else {
            name_node.detach(compiler);
        }
        NodeUtil::mark_functions_deleted(compiler, name_node);
    }
}

// port: RemoveUnusedCode.NameDeclarationStatement#isAssignedValueLocal
fn name_declaration_statement_is_assigned_value_local(
    ast: &Ast,
    declaration_statement: NodeId,
) -> bool {
    let name_node = declaration_statement.get_only_child(ast);
    let Some(initial_value_node) = name_node.get_first_child(ast) else {
        // `var foo;`
        // the "assigned" value is undefined, which should be considered a "local" value,
        // since it is a constant.
        return true;
    };
    // Handle `var name = name || defaultValue;`
    let value_node = maybe_unwrap_qname_or_default_value_node(ast, name_node, initial_value_node);
    NodeUtil::evaluates_to_local_value(ast, value_node)
}

// port: RemoveUnusedCode.NameDeclarationStatement#getLocalAssignedValue
fn name_declaration_statement_get_local_assigned_value(
    ast: &Ast,
    declaration_statement: NodeId,
) -> Option<NodeId> {
    let name_node = declaration_statement.get_only_child(ast);
    // `var foo;` has no node to represent the `undefined` value that is assigned.
    let initial_value_node = name_node.get_first_child(ast)?;
    // Handle `var name = name || defaultValue;`
    let value_node = maybe_unwrap_qname_or_default_value_node(ast, name_node, initial_value_node);
    if NodeUtil::evaluates_to_local_value(ast, value_node) {
        Some(value_node)
    } else {
        None
    }
}

// port: RemoveUnusedCode.Assign#getLocalAssignedValue
fn assign_get_local_assigned_value(ast: &Ast, assign_node: NodeId) -> Option<NodeId> {
    if NodeUtil::is_expression_result_used(ast, assign_node) {
        // assigned value may escape or be aliased
        None
    } else {
        // Handle `qname = qname || defaultValue;`
        let value_node = maybe_unwrap_qname_or_default_value_node(
            ast,
            assign_node.get_first_child(ast).unwrap(),
            assign_node.get_last_child(ast).unwrap(),
        );
        if NodeUtil::evaluates_to_local_value(ast, value_node) {
            Some(value_node)
        } else {
            None
        }
    }
}

// port: RemoveUnusedCode.VanillaForNameDeclaration#isAssignedValueLocal
fn vanilla_for_name_declaration_is_assigned_value_local(ast: &Ast, name_node: NodeId) -> bool {
    let Some(initial_value_node) = name_node.get_first_child(ast) else {
        // `var foo;`
        // the "assigned" value is undefined, which should be considered a "local" value,
        // since it is a constant.
        return true;
    };
    // Handle `var name = name || defaultValue;`
    let value_node = maybe_unwrap_qname_or_default_value_node(ast, name_node, initial_value_node);
    NodeUtil::evaluates_to_local_value(ast, value_node)
}

// port: RemoveUnusedCode.VanillaForNameDeclaration#getLocalAssignedValue
fn vanilla_for_name_declaration_get_local_assigned_value(
    ast: &Ast,
    name_node: NodeId,
) -> Option<NodeId> {
    // `var foo;` has no node to represent the `undefined` value that is assigned.
    let initial_value_node = name_node.get_first_child(ast)?;
    // Handle `var name = name || defaultValue;`
    let value_node = maybe_unwrap_qname_or_default_value_node(ast, name_node, initial_value_node);
    if NodeUtil::evaluates_to_local_value(ast, value_node) {
        Some(value_node)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// VarInfo
// ---------------------------------------------------------------------------------------------

/// Tracks whether a variable is removable or not, including tracking the Removable objects
/// associated with it.
// port: RemoveUnusedCode.VarInfo
enum VarInfo {
    /// Represents a variable that we know can never be removed regardless of how it is used.
    ///
    /// We create just one instance of this class and use it for many variables in order to save
    /// memory.
    // port: RemoveUnusedCode.CanonicalUnremovableVarInfo
    CanonicalUnremovable,
    Real(RealVarInfo),
}

/// Tracks the removable code and other state related to variables we may be able to remove.
// port: RemoveUnusedCode.RealVarInfo
struct RealVarInfo {
    var_name: JsString,

    /// Objects that represent variable declarations, assignments, or class setup calls that can
    /// be removed.
    ///
    /// NOTE: Once we realize that we cannot remove the variable, this list will be cleared and no
    /// more will be added.
    removables: Vec<RemovableId>,

    is_entirely_removable: bool,

    // At least one assignment to the variable is a non-local and/or non-literal value.
    has_non_local_or_non_literal_value: bool,

    // NOTE: We are assuming that if one value assigned to a variable is a class or function
    //     literal, than it is very likely that all other values, if any, assigned to the variable
    //     are functions or classes. At present this information is used only to decide whether
    //     `varName.propName = something` should be considered to be an ES5-style static property.
    //     If this assumption is wrong we may end up removing `propName` even though it's not
    //     actually a static class property. This seems a reasonable risk, because that removal
    //     would only occur if there were no references to `propName` anywhere in the sources or
    //     externs.
    //
    // At least one assignment to the variable is a function or class literal.
    has_function_or_class_literal_value: bool,
    requires_local_literal_value_for_removal: bool,
}

impl RealVarInfo {
    // port: RemoveUnusedCode.RealVarInfo#RealVarInfo
    fn new(var_name: JsString) -> Self {
        Self {
            var_name,
            removables: Vec::new(),
            is_entirely_removable: true,
            has_non_local_or_non_literal_value: false,
            has_function_or_class_literal_value: false,
            requires_local_literal_value_for_removal: false,
        }
    }
}

impl RemoveUnusedCode {
    fn new_var_info(&mut self, var_info: VarInfo) -> VarInfoId {
        self.var_infos.push(var_info);
        VarInfoId(self.var_infos.len() - 1)
    }

    fn real_var_info(&mut self, var_info: VarInfoId) -> &mut RealVarInfo {
        match &mut self.var_infos[var_info.0] {
            VarInfo::Real(real) => real,
            VarInfo::CanonicalUnremovable => unreachable!(),
        }
    }

    /// Add a Removable representing code that must be removed if this variable is removed.
    ///
    /// - The contents of the Removable could cause the variable to be no longer safe to remove.
    /// - If the variable is not safe to remove, this method will either apply the continuations
    ///   within the `removable` or allow it to be considered for independent removal.
    // port: RemoveUnusedCode.VarInfo#addRemovable
    fn var_info_add_removable(
        &mut self,
        compiler: &mut AbstractCompiler,
        var_info: VarInfoId,
        removable: RemovableId,
    ) {
        match self.var_infos[var_info.0] {
            // port: RemoveUnusedCode.CanonicalUnremovableVarInfo#addRemovable
            VarInfo::CanonicalUnremovable => {
                // Immediately pass the argument off for potential independent removal.
                self.consider_for_independent_removal(compiler, removable);
            }
            VarInfo::Real(_) => self.real_var_info_add_removable(compiler, var_info, removable),
        }
    }

    // port: RemoveUnusedCode.RealVarInfo#addRemovable
    fn real_var_info_add_removable(
        &mut self,
        compiler: &mut AbstractCompiler,
        var_info: VarInfoId,
        removable: RemovableId,
    ) {
        if self.removable_is_variable_assignment(compiler, removable) {
            // class name {}
            // function name {}
            // let name = something;
            // name = something;
            // let {a} = something;
            if self.removable_is_assigned_value_local(compiler, removable) {
                let local_value = self.removable_get_local_assigned_value(compiler, removable);
                // Still have to check for null local value because of variable declarations
                // without initial values.
                // `var a;` isAssignedValueLocal() == true but getLocalAssignedValue() == null
                if let Some(local_value) = local_value
                    && (local_value.is_function(compiler) || local_value.is_class(compiler))
                {
                    self.real_var_info(var_info)
                        .has_function_or_class_literal_value = true;
                }
            } else {
                self.real_var_info(var_info)
                    .has_non_local_or_non_literal_value = true;
            }
        } else if self.removable_is_prototype_assignment(compiler, removable)
            && !self.removable_is_assigned_value_local(compiler, removable)
        {
            // `name.prototype = someNonLocalValue;`
            self.real_var_info(var_info)
                .has_non_local_or_non_literal_value = true;
        }
        if self.removable_prevents_removal_of_variable_with_non_local_value_or_prototype(
            compiler, removable,
        ) {
            self.real_var_info(var_info)
                .requires_local_literal_value_for_removal = true;
        }
        let real = self.real_var_info(var_info);
        if real.has_non_local_or_non_literal_value && real.requires_local_literal_value_for_removal
        {
            self.var_info_set_is_explicitly_not_removable(compiler, var_info, |_| {
                "hasNonLocalOrNonLiteralValue && requiresLocalLiteralValueForRemoval".to_string()
            });
        }

        if self.real_var_info(var_info).is_entirely_removable {
            // Store for possible removal later.
            self.real_var_info(var_info).removables.push(removable);
        } else {
            self.consider_for_independent_removal(compiler, removable);
        }
    }

    /// At the current point of execution, does the variable appear safe to remove?
    // port: RemoveUnusedCode.VarInfo#isRemovable
    fn var_info_is_removable(&self, var_info: VarInfoId) -> bool {
        match &self.var_infos[var_info.0] {
            // port: RemoveUnusedCode.CanonicalUnremovableVarInfo#isRemovable
            VarInfo::CanonicalUnremovable => false,
            // port: RemoveUnusedCode.RealVarInfo#isRemovable
            VarInfo::Real(real) => real.is_entirely_removable,
        }
    }

    /// Record that the variable cannot be removed.
    ///
    /// If the variable was previously considered safe to remove, then this method will examine
    /// all of the `Removable` objects associated with this variable and either apply their
    /// continuations or consider them for independent removal.
    // port: RemoveUnusedCode.VarInfo#setIsExplicitlyNotRemovable
    fn var_info_set_is_explicitly_not_removable(
        &mut self,
        compiler: &mut AbstractCompiler,
        var_info: VarInfoId,
        reason_supplier: impl FnOnce(&Ast) -> String,
    ) {
        match &mut self.var_infos[var_info.0] {
            // port: RemoveUnusedCode.CanonicalUnremovableVarInfo#setIsExplicitlyNotRemovable
            VarInfo::CanonicalUnremovable => {
                // nothing to do
            }
            // port: RemoveUnusedCode.RealVarInfo#setIsExplicitlyNotRemovable
            VarInfo::Real(real) => {
                if real.is_entirely_removable {
                    real.is_entirely_removable = false;
                    let var_name = real.var_name.clone();
                    let removables = std::mem::take(&mut real.removables);
                    let ast: &Ast = compiler;
                    // Java formats the message eagerly; the reason is a pure function of the
                    // AST, so it is formatted only when the log file writes it (the default
                    // NoOpLogFile never does).
                    let mut reason_supplier = Some(reason_supplier);
                    self.unremovable_log.as_mut().unwrap().log(&mut || {
                        let reason = reason_supplier.take().map_or_else(String::new, |f| f(ast));
                        format!("{var_name}: {reason}")
                    });
                    for r in removables {
                        self.consider_for_independent_removal(compiler, r);
                    }
                }
            }
        }
    }

    /// Record that at least one value assigned to the variable is non-local (comes from or
    /// escapes to another scope) and / or non-literal.
    // port: RemoveUnusedCode.VarInfo#setHasNonLocalOrNonLiteralValue
    fn var_info_set_has_non_local_or_non_literal_value(&mut self, var_info: VarInfoId) {
        match &mut self.var_infos[var_info.0] {
            // port: RemoveUnusedCode.CanonicalUnremovableVarInfo#setHasNonLocalOrNonLiteralValue
            VarInfo::CanonicalUnremovable => {
                // nothing to do
            }
            // port: RemoveUnusedCode.RealVarInfo#setHasNonLocalOrNonLiteralValue
            VarInfo::Real(real) => real.has_non_local_or_non_literal_value = true,
        }
    }

    /// Is at least one value assigned to the variable a function or class literal?
    // port: RemoveUnusedCode.VarInfo#hasFunctionOrClassLiteralValue
    fn var_info_has_function_or_class_literal_value(&self, var_info: VarInfoId) -> bool {
        match &self.var_infos[var_info.0] {
            // port: RemoveUnusedCode.CanonicalUnremovableVarInfo#hasFunctionOrClassLiteralValue
            // Returning false here will prevent some properties assigned on unremovable variables
            // from being independently removed, but returning `true` would cause incorrect
            // removal of properties.
            VarInfo::CanonicalUnremovable => false,
            // port: RemoveUnusedCode.RealVarInfo#hasFunctionOrClassLiteralValue
            VarInfo::Real(real) => real.has_function_or_class_literal_value,
        }
    }

    /// Invokes the `remove()` method on all removables associated with this variable.
    ///
    /// Does nothing if the variable has been found to be unsafe to remove.
    // port: RemoveUnusedCode.VarInfo#removeAllRemovables
    fn var_info_remove_all_removables(
        &mut self,
        compiler: &mut AbstractCompiler,
        var_info: VarInfoId,
    ) {
        match &mut self.var_infos[var_info.0] {
            // port: RemoveUnusedCode.CanonicalUnremovableVarInfo#removeAllRemovables
            VarInfo::CanonicalUnremovable => {
                // nothing to do
            }
            // port: RemoveUnusedCode.RealVarInfo#removeAllRemovables
            VarInfo::Real(real) => {
                check_state!(real.is_entirely_removable);
                let removables = real.removables.clone();
                for removable in removables {
                    self.removable_remove(compiler, removable);
                }
                self.real_var_info(var_info).removables.clear();
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// PolyfillInfo
// ---------------------------------------------------------------------------------------------

/// The Java subclass of a `PolyfillInfo`.
enum PolyfillInfoKind {
    // port: RemoveUnusedCode.GlobalPolyfillInfo
    Global,
    // port: RemoveUnusedCode.StaticPropertyPolyfillInfo
    StaticProperty {
        // Name of the owning type, used only for debugging.
        polyfill_owner_name: String,
    },
    // port: RemoveUnusedCode.PrototypePropertyPolyfillInfo
    PrototypeProperty {
        // Name of the owning type, used only for debugging.
        polyfill_owner_name: String,
    },
}

/// Stores information about definitions and usages of polyfills.
///
/// The polyfill removal strategy is as follows. First, look for all the polyfill definitions,
/// whose names are stores as strings passed as the first argument to `$jscomp.polyfill`. Each
/// definition falls into one of three categories: (1) global names, such as `Map` or `Promise`;
/// (2) static properties, such as `Array.from` or `Reflect.get`, which must always have exactly
/// two name components; or (3) prototype properties, such as `String.prototype.repeat` or
/// `Promise.prototype.finally`, which must always have exactly three name components. The
/// definition can be removed once it is found that there are no references to it.
///
/// References are ignored if they are "guarded". This allows removing, e.g, the Promise polyfill
/// if it is only referenced in `if (typeof Promise === 'function') { use(Promise); }`. Note that
/// a guarded reference to a polyfill does not guarantee its removal, either. Polyfills may have
/// nonguarded references as well.
// port: RemoveUnusedCode.PolyfillInfo
struct PolyfillInfo {
    /// The `Polyfill` instance corresponding to the polyfill's definition.
    removable: RemovableId,

    /// The rightmost component of the polyfill's qualified name (does not contain a dot).
    key: JsString,

    /// Whether the polyfill is unreferenced and this can be removed safely.
    is_removable: bool,

    kind: PolyfillInfoKind,
}

impl RemoveUnusedCode {
    // port: RemoveUnusedCode.PolyfillInfo#PolyfillInfo
    // port: RemoveUnusedCode.GlobalPolyfillInfo#GlobalPolyfillInfo
    // port: RemoveUnusedCode.StaticPropertyPolyfillInfo#StaticPropertyPolyfillInfo
    // port: RemoveUnusedCode.PrototypePropertyPolyfillInfo#PrototypePropertyPolyfillInfo
    fn new_polyfill_info(
        &mut self,
        removable: RemovableId,
        key: &str,
        kind: PolyfillInfoKind,
    ) -> PolyfillInfoId {
        self.polyfill_infos.push(PolyfillInfo {
            removable,
            key: JsString::from(key),
            is_removable: true,
            kind,
        });
        PolyfillInfoId(self.polyfill_infos.len() - 1)
    }

    /// Accepts a NAME or GETPROP node whose (property) string matches `key` and checks whether
    /// the node should be considered as a possible reference to this polyfill. If so, mark the
    /// polyfill as referenced and therefore not removable.
    // port: RemoveUnusedCode.PolyfillInfo#considerPossibleReference
    fn polyfill_info_consider_possible_reference(
        &mut self,
        compiler: &mut AbstractCompiler,
        polyfill_info: PolyfillInfoId,
        n: NodeId,
    ) {
        let removable = self.polyfill_infos[polyfill_info.0].removable;
        let is_patch = match self.removables[removable.0].kind {
            RemovableKind::Polyfill { is_patch, .. } => is_patch,
            _ => unreachable!(),
        };
        if self.polyfill_infos[polyfill_info.0].is_removable
            && (is_patch || !self.guarded_usages.contains(&n))
        {
            self.polyfill_info_consider_possible_reference_internal(compiler, polyfill_info, n);
            if !self.polyfill_infos[polyfill_info.0].is_removable {
                self.removable_apply_continuations(removable);
            }
        }
    }

    // port: RemoveUnusedCode.PolyfillInfo#getName
    fn polyfill_info_get_name(&self, polyfill_info: PolyfillInfoId) -> String {
        let info = &self.polyfill_infos[polyfill_info.0];
        match &info.kind {
            PolyfillInfoKind::Global => info.key.to_string_lossy(),
            // port: RemoveUnusedCode.StaticPropertyPolyfillInfo#getName
            PolyfillInfoKind::StaticProperty {
                polyfill_owner_name,
            } => format!("{}.{}", polyfill_owner_name, info.key),
            // port: RemoveUnusedCode.PrototypePropertyPolyfillInfo#getName
            PolyfillInfoKind::PrototypeProperty {
                polyfill_owner_name,
            } => format!("{}.prototype.{}", polyfill_owner_name, info.key),
        }
    }

    /// Template method to check the node.
    // port: RemoveUnusedCode.PolyfillInfo#considerPossibleReferenceInternal
    fn polyfill_info_consider_possible_reference_internal(
        &mut self,
        compiler: &mut AbstractCompiler,
        polyfill_info: PolyfillInfoId,
        possibly_referencing_node: NodeId,
    ) {
        let info = &mut self.polyfill_infos[polyfill_info.0];
        match info.kind {
            // port: RemoveUnusedCode.GlobalPolyfillInfo#considerPossibleReferenceInternal
            PolyfillInfoKind::Global => {
                if possibly_referencing_node.is_name(compiler) {
                    // A matching NAME node must be a reference (there's no need to check that the
                    // referenced Var is global, since local variables have all been renamed by
                    // normalization).
                    info.is_removable = false;
                } else if NodeUtil::is_normal_or_opt_chain_get_prop(
                    compiler,
                    possibly_referencing_node,
                ) {
                    // Assume that the owner is possibly the global `this` and skip removal.
                    info.is_removable = false;
                }
            }
            // port: RemoveUnusedCode.StaticPropertyPolyfillInfo#considerPossibleReferenceInternal
            PolyfillInfoKind::StaticProperty { .. } => {
                if NodeUtil::is_normal_or_opt_chain_get_prop(compiler, possibly_referencing_node) {
                    info.is_removable = false;
                }
            }
            // port: RemoveUnusedCode.PrototypePropertyPolyfillInfo#considerPossibleReferenceInternal
            PolyfillInfoKind::PrototypeProperty { .. } => {
                if NodeUtil::is_normal_or_opt_chain_get_prop(compiler, possibly_referencing_node) {
                    // Prototype properties are simply not removable.
                    info.is_removable = false;
                }
            }
        }
    }
}
