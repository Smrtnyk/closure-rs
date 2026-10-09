/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/MakeDeclaredNamesUnique.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `MakeDeclaredNamesUnique.java`: finds all functions, vars and exception names and
//! makes them unique, plus the renamers (`ContextualRenamer`, `InlineRenamer`,
//! `BoilerplateRenamer`, `TargettedRenamer`) and `ContextualRenameInverter`.
use crate::{
    abstract_compiler::AbstractCompiler,
    coding_convention::CodingConvention,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    scope::ScopeId,
    var::VarId,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    jsdoc_info::Builder as JSDocInfoBuilder,
    node::{Ast, NodeId, Prop},
    token::Token,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::Arc,
};

/// Java's `Supplier<String>` of unique ids (`Compiler#getUniqueNameIdSupplier`).
pub type UniqueIdSupplierFn = Arc<dyn Fn() -> String + Send + Sync>;

/// A shared, mutable `Renamer` object. Java renamers are referenced from the renamer stack and
/// from their child renamers (`hoistRenamer`); Java identity is `Rc::ptr_eq`.
pub type RenamerRef = Rc<RefCell<dyn Renamer>>;

/// Find all Functions, VARs, and Exception names and make them unique. Specifically, it will not
/// modify object properties.
pub struct MakeDeclaredNamesUnique {
    // There is one renamer on the stack for each scope. This was added before any support for ES6
    // was in place, so it was necessary to maintain this separate stack, rather than just using
    // the NodeTraversal's stack of scopes, in order to handle catch blocks and function names
    // correctly. (Java's ArrayDeque pushes at its head; the head is the last element here.)
    renamer_stack: Vec<RenamerRef>,
    root_renamer: RenamerRef,
    mark_changes: bool,
    assert_on_change: bool,
}

impl MakeDeclaredNamesUnique {
    // Arguments is special cased to handle cases where a local name shadows
    // the arguments declaration.
    pub const ARGUMENTS: &'static str = "arguments";

    // port: MakeDeclaredNamesUnique#MakeDeclaredNamesUnique
    fn new(renamer: RenamerRef, mark_changes: bool, assert_on_change: bool) -> Self {
        Self {
            renamer_stack: Vec::new(),
            root_renamer: renamer,
            mark_changes,
            assert_on_change,
        }
    }

    // port: MakeDeclaredNamesUnique#builder
    pub fn builder() -> Builder {
        Builder::new()
    }

    // port: MakeDeclaredNamesUnique#getContextualRenameInverter
    pub fn get_contextual_rename_inverter(
        _compiler: &mut AbstractCompiler,
    ) -> Box<dyn CompilerPass> {
        Box::new(ContextualRenameInverter::new())
    }

    // port: MakeDeclaredNamesUnique#visitNameOrImportStar
    fn visit_name_or_import_star(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        // Don't rename the exported name foo in export {a as foo}; or import {foo as b};
        if n.is_name(t) && NodeUtil::is_nonlocal_module_export_name(t, n) {
            return;
        }
        let Some(new_name) = self.get_replacement_name(&n.get_string(t)) else {
            return;
        };
        if self.assert_on_change {
            panic!(
                "Unexpected renaming in MakeDeclaredNamesUnique. new name: {} for {}",
                new_name,
                n.to_string(t)
            );
        }

        let renamer = self.renamer_stack.last().unwrap().clone();
        if renamer.borrow().strip_const_if_replaced() {
            n.put_boolean_prop(t, Prop::IS_CONSTANT_NAME, false);
            let js_doc_info_node = NodeUtil::get_best_jsdoc_info_node(t, n);
            if let Some(js_doc_info_node) = js_doc_info_node
                && let Some(info) = js_doc_info_node.get_jsdoc_info(t)
            {
                let mut builder = JSDocInfoBuilder::copy_from(&info);
                builder.record_mutable();
                let info = builder.build();
                js_doc_info_node.set_jsdoc_info(t, info);
            }
        }
        n.set_string(t, new_name);
        if self.mark_changes {
            t.report_code_change();
            // If we are renaming a function declaration, make sure the containing scope
            // has the opportunity to act on the change.
            let parent = check_not_null!(parent);
            if parent.is_function(t) && NodeUtil::is_function_declaration(t, parent) {
                t.get_compiler().report_change_to_enclosing_scope(parent);
            }
        }
    }

    /// Walks the stack of name maps and finds the replacement name for the current scope.
    // port: MakeDeclaredNamesUnique#getReplacementName
    fn get_replacement_name(&self, old_name: &JsString) -> Option<JsString> {
        for renamer in self.renamer_stack.iter().rev() {
            let new_name = renamer.borrow().get_replacement_name(old_name);
            if new_name.is_some() {
                return new_name;
            }
        }
        None
    }

    /// Traverses the current scope and collects declared names by calling `addDeclaredName` on
    /// the `Renamer` that is at the top of the `renamerStack`.
    // port: MakeDeclaredNamesUnique#findDeclaredNames
    fn find_declared_names(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        check_state!(
            NodeUtil::creates_scope(t, n) || n.is_script(t),
            "%s",
            n.to_string(t)
        );

        let scope = t.get_scope();
        for v in scope.get_var_iterable(t.get_compiler()) {
            let name = v.get_name(t.get_compiler());
            self.renamer_stack
                .last()
                .unwrap()
                .borrow_mut()
                .add_declared_name(&name, false);
        }
    }
}

impl Callback for MakeDeclaredNamesUnique {
    // port: NodeTraversal.AbstractScopedCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: MakeDeclaredNamesUnique#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::NAME | Token::IMPORT_STAR => self.visit_name_or_import_star(t, n, parent),
            _ => {}
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for MakeDeclaredNamesUnique {
    // port: MakeDeclaredNamesUnique#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let declaration_root = check_not_null!(t.get_scope_root());

        let renamer: RenamerRef = if self.renamer_stack.is_empty() {
            // If the contextual renamer is being used, the starting context can not
            // be a function.
            check_state!(
                !declaration_root.is_function(t)
                    || !self.root_renamer.borrow().is_contextual_renamer()
            );
            self.root_renamer.clone()
        } else {
            let hoist = !declaration_root.is_function(t)
                && !NodeUtil::creates_block_scope(t, declaration_root);
            let scope_root = check_not_null!(t.get_scope_root());
            let top = self.renamer_stack.last().unwrap().clone();
            top.borrow().create_for_child_scope(t, scope_root, hoist)
        };

        self.renamer_stack.push(renamer);

        self.find_declared_names(t, declaration_root);
    }

    // port: MakeDeclaredNamesUnique#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if !t.in_global_scope() {
            self.renamer_stack.pop();
        }
    }
}

pub struct Builder {
    renamer: Option<RenamerRef>,
    mark_changes: bool,
    assert_on_change: bool,
}

impl Builder {
    // port: MakeDeclaredNamesUnique.Builder#Builder
    fn new() -> Self {
        Self {
            renamer: None,
            mark_changes: true,
            assert_on_change: false,
        }
    }

    // port: MakeDeclaredNamesUnique.Builder#withRenamer
    pub fn with_renamer(mut self, renamer: RenamerRef) -> Self {
        self.renamer = Some(renamer);
        self
    }

    /// Enables the option to report any code changes to the compiler object.
    ///
    /// This option is `true` by default.
    // port: MakeDeclaredNamesUnique.Builder#withMarkChanges
    pub fn with_mark_changes(mut self, mark_changes: bool) -> Self {
        self.mark_changes = mark_changes;
        self
    }

    /// If this class finds any names that are not unique, throws an exception
    ///
    /// This is intended for use in validating that an AST is already normalized.
    ///
    /// This option is `false` by default.
    // port: MakeDeclaredNamesUnique.Builder#withAssertOnChange
    pub fn with_assert_on_change(mut self, assert_on_change: bool) -> Self {
        self.assert_on_change = assert_on_change;
        self
    }

    // port: MakeDeclaredNamesUnique.Builder#build
    pub fn build(mut self) -> MakeDeclaredNamesUnique {
        if self.renamer.is_none() {
            self.renamer = Some(ContextualRenamer::new());
        }
        MakeDeclaredNamesUnique::new(
            self.renamer.unwrap(),
            self.mark_changes,
            self.assert_on_change,
        )
    }
}

/// Declared names renaming policy interface.
pub trait Renamer {
    /// Called when a declared name is found in the local current scope.
    ///
    /// `hoisted`: Whether this name should be declared in the nearest enclosing "hoist scope"
    /// instead of the scope represented by this Renamer.
    // port: MakeDeclaredNamesUnique.Renamer#addDeclaredName
    fn add_declared_name(&mut self, name: &JsString, hoisted: bool);

    /// Returns a replacement name, null if oldName is unknown or should not be replaced.
    // port: MakeDeclaredNamesUnique.Renamer#getReplacementName
    fn get_replacement_name(&self, old_name: &JsString) -> Option<JsString>;

    /// Returns whether the constant-ness of a name should be removed.
    // port: MakeDeclaredNamesUnique.Renamer#stripConstIfReplaced
    fn strip_const_if_replaced(&self) -> bool;

    /// `hoisted`: True if this is a "hoist" scope: A function, module, or global scope.
    /// Returns a Renamer for a scope within the scope of the current Renamer.
    // port: MakeDeclaredNamesUnique.Renamer#createForChildScope
    fn create_for_child_scope(&self, ast: &Ast, scope_root: NodeId, hoisted: bool) -> RenamerRef;

    /// Returns the closest hoisting target for var and function declarations.
    // port: MakeDeclaredNamesUnique.Renamer#getHoistRenamer
    fn get_hoist_renamer(&self) -> RenamerRef;

    // Rust-only counterpart of Java's `rootRenamer instanceof ContextualRenamer`
    // (BoilerplateRenamer extends ContextualRenamer).
    fn is_contextual_renamer(&self) -> bool {
        false
    }
}

/// Inverts the transformation by `ContextualRenamer`, when possible.
pub struct ContextualRenameInverter {
    /// The top of this stack is always an object storing information about the current variable
    /// scope or `null` for the global scope. (The top is the last element; the `parent` of each
    /// `ScopeContext` is the element below it.)
    scope_context_stack: Vec<ScopeContext>,

    /// Keeps track of all `VariableInfo` objects for all variables visible in the current
    /// variable scope.
    ///
    /// Basically this is a map whose keys are names and whose values are stacks of
    /// `VariableInfo` objects. The top of each stack represents the variable with that name that
    /// is visible in the scope we are currently traversing.
    variables_stacked_by_name: VariableInfoStackMap,

    // Rust-only arena of the Java VariableInfo objects; a VariableInfoId is Java object identity.
    // It is emptied together with variablesStackedByName, when no ScopeContext refers to it.
    variable_infos: Vec<VariableInfo>,
}

impl CompilerPass for ContextualRenameInverter {
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, js: NodeId) {
        NodeTraversal::traverse(compiler, js, self);
    }
}

impl ContextualRenameInverter {
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#ContextualRenameInverter
    fn new() -> Self {
        Self {
            scope_context_stack: Vec::new(),
            variables_stacked_by_name: VariableInfoStackMap::new(),
            variable_infos: Vec::new(),
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#getOriginalName
    pub fn get_original_name(name: &JsString) -> JsString {
        let index = Self::index_of_separator(name);
        if index <= 0 {
            name.clone()
        } else {
            name.substring(0, index as usize)
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#indexOfSeparator
    fn index_of_separator(name: &JsString) -> i32 {
        name.last_index_of(ContextualRenamer::UNIQUE_ID_SEPARATOR)
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#createDeclaredVariableInfo
    fn create_declared_variable_info(
        &mut self,
        compiler: &AbstractCompiler,
        var: VarId,
    ) -> VariableInfoId {
        let current_name = var.get_name(compiler);
        let index_of_separator = Self::index_of_separator(&current_name);
        let renaming_info = if index_of_separator > 0 {
            // We check for an index > 0, because a variable created directly by the compiler
            // generally begin with `$jscomp$` (the separator). We can't and don't want to try to
            // rename any variable to an empty string.
            let inverted_name = current_name.substring(0, index_of_separator as usize);
            Some(RenamingInfo::new(current_name.clone(), inverted_name))
        } else {
            None
        };
        self.new_variable_info(VariableInfo::Declared(DeclaredVariableInfo::new(
            compiler,
            var,
            renaming_info,
        )))
    }

    // Rust-only allocation of a Java VariableInfo object in the arena.
    fn new_variable_info(&mut self, info: VariableInfo) -> VariableInfoId {
        self.variable_infos.push(info);
        VariableInfoId(self.variable_infos.len() - 1)
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.DeclaredVariableInfo#tryToInvertName
    fn try_to_invert_name(&mut self, compiler: &mut AbstractCompiler, id: VariableInfoId) {
        let VariableInfo::Declared(declared) = &self.variable_infos[id.0] else {
            unreachable!("only DeclaredVariableInfo objects are inverted")
        };
        if let Some(renaming_info) = &declared.renaming_info {
            // update the name for the sake of other variables that may check this one for
            // conflicts.
            let name = renaming_info.attempt_rename(compiler, &self.variable_infos);
            let VariableInfo::Declared(declared) = &mut self.variable_infos[id.0] else {
                unreachable!()
            };
            declared.name = name;
            declared.renaming_info = None; // allow garbage collection
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#createUndeclaredVariableInfo
    fn create_undeclared_variable_info(&mut self, name: &JsString) -> VariableInfoId {
        self.new_variable_info(VariableInfo::Undeclared(UndeclaredVariableInfo::new(
            name.clone(),
        )))
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfoStackMap#getOrCreateCurrentVariableInfo
    fn get_or_create_current_variable_info(&mut self, name: &JsString) -> VariableInfoId {
        let variable_info_stack_is_empty = self
            .variables_stacked_by_name
            .get_variable_info_stack(name)
            .is_empty();
        if variable_info_stack_is_empty {
            let variable_info = self.create_undeclared_variable_info(name);
            self.variables_stacked_by_name
                .get_variable_info_stack(name)
                .push(variable_info);
            variable_info
        } else {
            *self
                .variables_stacked_by_name
                .get_variable_info_stack(name)
                .last()
                .unwrap()
        }
    }
}

impl Callback for ContextualRenameInverter {
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, node: NodeId, _parent: Option<NodeId>) {
        if !self.scope_context_stack.is_empty() {
            // We're not in the global scope
            if NodeUtil::is_reference_name(t, node) || node.is_import_star(t) {
                let name = node.get_string(t);
                // Get the corresponding VariableInfo, creating one if we haven't found a
                // declaration for it.
                let variable_info = self.get_or_create_current_variable_info(&name);
                // add this reference to the variable
                self.variable_infos[variable_info.0].add_reference_node(node);
                // tell the current scope that it references the variable
                self.scope_context_stack
                    .last_mut()
                    .unwrap()
                    .add_referenced_variable_info(variable_info);
            }
        } // else nothing to do for the global scope
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for ContextualRenameInverter {
    /// Prepare a set for the new scope.
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_global_scope() {
            // Don't track any information for the global scope, because it is likely to be
            // very large, and we will never rename any variables it contains.
            // MakeDeclaredNamesUnique favors keeping global variables unchanged, so they won't
            // need to be restored.
            return;
        }

        let scope = t.get_scope();

        // Create DeclaredVariableInfo objects for all variables declared in this scope.
        // Push them onto the variablesByName data structure to make them visible,
        // and store them into a ScopeContext object for the scope we're entering.
        let mut declared_variables_builder = Vec::new();
        for var in scope.get_var_iterable(t.get_compiler()) {
            let variable_info = self.create_declared_variable_info(t.get_compiler(), var);
            declared_variables_builder.push(variable_info);
            self.variables_stacked_by_name
                .push_variable_info(&self.variable_infos, variable_info);
        }
        self.scope_context_stack
            .push(ScopeContext::new(scope, declared_variables_builder));
    }

    /// Rename vars for the current scope, and merge any referenced names into the parent scope
    /// reference set.
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_global_scope() {
            return;
        }

        let scope_context = check_not_null!(self.scope_context_stack.pop());

        // Tell all the variables we referenced and the variables that might conflict with them
        // about each other.
        // This needs to happen before we start trying to rename the variables in this scope
        // next.
        for referenced_variable in scope_context.referenced_variables.iter().copied() {
            scope_context.record_potential_conflicts(
                t.get_compiler(),
                &self.scope_context_stack,
                &mut self.variable_infos,
                referenced_variable,
            );
        }

        // Pop each variable declared in the scope we're exiting off of the variable stack
        // data structure.
        // Try to rename each one.
        for declared_variable_info in scope_context.declared_variable_infos.iter().copied() {
            let variable_name = self.variable_infos[declared_variable_info.0]
                .get_name()
                .clone();
            // Out of an abundance of caution:
            // 1. Pop the variable from its name stack and make sure it matches
            // 2. Do this before renaming causes the name to be different.
            let old_stack_top = self
                .variables_stacked_by_name
                .pop_variable_info(&variable_name);
            check_state!(
                old_stack_top == declared_variable_info,
                "Declared variable \"%s\" was not the top of the name stack",
                variable_name
            );
            self.try_to_invert_name(t.get_compiler(), declared_variable_info);
        }

        if self.scope_context_stack.is_empty() {
            // clear the records for any global or undeclared variables before we start
            // traversing a new local scope.
            self.variables_stacked_by_name.clear();
            // Rust-only: no ScopeContext or name stack refers to a VariableInfo any more.
            self.variable_infos.clear();
        }
    }
}

/// Java object identity of a `VariableInfo` (its index in the inverter's arena).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct VariableInfoId(usize);

/// Keep track of information needed to rename a `DeclaredVariableInfo`.
struct RenamingInfo {
    /// Will be filled with all nodes referring to the variable.
    reference_nodes: Vec<NodeId>,

    /// Will be filled with objects representing variables whose names must not conflict with
    /// this one.
    potential_shadow_variables: IndexSet<VariableInfoId>,

    /// The name the variable has currently
    current_name: JsString,

    /// The name we want to use when renaming the variable.
    preferred_name: JsString,
}

impl RenamingInfo {
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.RenamingInfo#RenamingInfo
    fn new(current_name: JsString, preferred_name: JsString) -> Self {
        Self {
            reference_nodes: Vec::new(),
            potential_shadow_variables: IndexSet::<_>::default(),
            current_name,
            preferred_name,
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.RenamingInfo#addPotentialShadowVariable
    fn add_potential_shadow_variable(&mut self, variable_info: VariableInfoId) {
        self.potential_shadow_variables.insert(variable_info);
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.RenamingInfo#attemptRename
    fn attempt_rename(
        &self,
        compiler: &mut AbstractCompiler,
        variable_infos: &[VariableInfo],
    ) -> JsString {
        let mut disallowed_names: IndexSet<JsString> = IndexSet::<_>::default();
        // If we somehow ended up with "arguments$jscomp$..." it's not safe to rename
        // that to "arguments", because that name is special, but it would still be good
        // to simplify its name, if possible. See the note below regarding why we
        // rename variables to something other than their original name.
        disallowed_names.insert(MakeDeclaredNamesUnique::ARGUMENTS.into());
        for potential_shadow_variable in self.potential_shadow_variables.iter().copied() {
            // NOTE: We need to look up these names immediately before making our rename
            // decision, because some of these variables could themselves have been renamed.
            let potential_shadow_name = variable_infos[potential_shadow_variable.0]
                .get_name()
                .clone();
            disallowed_names.insert(potential_shadow_name);
        }
        let mut new_name = self.preferred_name.clone();
        if disallowed_names.contains(&self.preferred_name) {
            // Why are we bothering to find a name other than the original one?
            // The reason is that we would like the final output name to depend more on the
            // shape of the output code than on the process the compiler followed to reach that
            // shape. This keeps our unit tests more stable. Without it, a tweak to the details of
            // some optimization pass would be more likely to make a trivial change to the output
            // that breaks unit tests.
            let base_name = self
                .preferred_name
                .concat(&ContextualRenamer::UNIQUE_ID_SEPARATOR.into());
            let mut i: i32 = 0;
            loop {
                new_name = base_name.concat(&JsString::from(i.to_string()));
                i += 1;
                if !disallowed_names.contains(&new_name) {
                    break;
                }
            }
        }

        // It's possible we ended up generating the same name we already had.
        if new_name == self.current_name {
            return self.current_name.clone();
        }

        for reference_node in self.reference_nodes.iter().copied() {
            reference_node.set_string(compiler, new_name.clone());
            // NOTE: This could probably be made more efficient by only reporting after doing all
            // the nodes that occur in the same scope.
            compiler.report_change_to_enclosing_scope(reference_node);
            let parent = reference_node.get_parent(compiler).unwrap();
            // If we are renaming a function declaration, make sure the containing scope
            // has the opportunity to act on the change.
            if parent.is_function(compiler) && NodeUtil::is_function_declaration(compiler, parent) {
                compiler.report_change_to_enclosing_scope(parent);
            }
        }

        new_name
    }
}

/// One of these will be created for every variable we encounter.
enum VariableInfo {
    Declared(DeclaredVariableInfo),
    Undeclared(UndeclaredVariableInfo),
}

impl VariableInfo {
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfo#addReferenceNode
    fn add_reference_node(&mut self, name_node: NodeId) {
        match self {
            // port: MakeDeclaredNamesUnique.ContextualRenameInverter.DeclaredVariableInfo#addReferenceNode
            Self::Declared(declared) => {
                if let Some(renaming_info) = &mut declared.renaming_info {
                    renaming_info.reference_nodes.push(name_node);
                } // else we won't rename, so we don't care
            }
            // port: MakeDeclaredNamesUnique.ContextualRenameInverter.UndeclaredVariableInfo#addReferenceNode
            Self::Undeclared(_) => {
                // no need to track references
            }
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfo#getName
    fn get_name(&self) -> &JsString {
        match self {
            // port: MakeDeclaredNamesUnique.ContextualRenameInverter.DeclaredVariableInfo#getName
            Self::Declared(declared) => &declared.name,
            // port: MakeDeclaredNamesUnique.ContextualRenameInverter.UndeclaredVariableInfo#getName
            Self::Undeclared(undeclared) => &undeclared.name,
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfo#getScopeDepth
    fn get_scope_depth(&self, compiler: &AbstractCompiler) -> i32 {
        match self {
            // port: MakeDeclaredNamesUnique.ContextualRenameInverter.DeclaredVariableInfo#getScopeDepth
            Self::Declared(declared) => declared.var.get_scope(compiler).get_depth(compiler),
            // port: MakeDeclaredNamesUnique.ContextualRenameInverter.UndeclaredVariableInfo#getScopeDepth
            Self::Undeclared(_) => {
                // NOTE: Pretend that any variable for which we didn't see a declaration was
                // declared in the global scope, which is depth 0. This will most often be
                // actually true, since we skip creating variables for the global scope to avoid
                // wasting time and space on variables that will never be renamed.
                0
            }
        }
    }
}

// port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfo#addPotentialConflict
// (`this` is the receiver object's identity in the arena)
fn add_potential_conflict(
    variable_infos: &mut [VariableInfo],
    this: VariableInfoId,
    variable_info: VariableInfoId,
) {
    match &mut variable_infos[this.0] {
        // port: MakeDeclaredNamesUnique.ContextualRenameInverter.DeclaredVariableInfo#addPotentialConflict
        VariableInfo::Declared(declared) => {
            if let Some(renaming_info) = &mut declared.renaming_info
                && variable_info != this
            {
                renaming_info.add_potential_shadow_variable(variable_info);
            } // else we won't rename, so we don't care.
        }
        // port: MakeDeclaredNamesUnique.ContextualRenameInverter.UndeclaredVariableInfo#addPotentialConflict
        VariableInfo::Undeclared(_) => {
            // No need to record the potential conflict, because this variable will never be
            // renamed.
        }
    }
}

/// Records information about a variable for which we have seen a non-global declaration.
struct DeclaredVariableInfo {
    var: VarId,
    name: JsString,

    /// Keep track of information needed to rename this variable, if that is possible.
    ///
    /// This will be `null` if renaming has already been done or cannot be done.
    renaming_info: Option<RenamingInfo>,
}

impl DeclaredVariableInfo {
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.DeclaredVariableInfo#DeclaredVariableInfo
    fn new(compiler: &AbstractCompiler, var: VarId, renaming_info: Option<RenamingInfo>) -> Self {
        Self {
            var,
            name: var.get_name(compiler),
            renaming_info,
        }
    }
}

/// Keep track of information about variables declared and referenced in a local scope.
///
/// We do not create one of these objects for the global scope, because we know those won't be
/// renamed. It isn't worthwhile to track all of them.
struct ScopeContext {
    scope: ScopeId,
    declared_variable_infos: Vec<VariableInfoId>,

    /// Will be filled with objects for all variables we reference in this scope.
    referenced_variables: IndexSet<VariableInfoId>,
}

impl ScopeContext {
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.ScopeContext#ScopeContext
    fn new(scope: ScopeId, declared_variable_infos: Vec<VariableInfoId>) -> Self {
        Self {
            scope,
            declared_variable_infos,
            referenced_variables: IndexSet::<_>::default(),
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.ScopeContext#addReferencedVariableInfo
    fn add_referenced_variable_info(&mut self, variable_info: VariableInfoId) {
        self.referenced_variables.insert(variable_info);
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.ScopeContext#recordPotentialConflicts
    // (`parents` is the scope context stack below this context: `parent` is its last element)
    fn record_potential_conflicts(
        &self,
        compiler: &AbstractCompiler,
        parents: &[ScopeContext],
        variable_infos: &mut [VariableInfo],
        referenced_variable: VariableInfoId,
    ) {
        let this_scope_depth = self.scope.get_depth(compiler);
        let referenced_variable_declaration_depth =
            variable_infos[referenced_variable.0].get_scope_depth(compiler);
        if this_scope_depth >= referenced_variable_declaration_depth {
            for declared_variable_info in self.declared_variable_infos.iter().copied() {
                // NOTE: No need to check whether the 2 variables are actually the same here.
                // The logic in addPotentialConflict() can do that more efficiently.

                // The referenced variable must be sure not to "hide behind" another variable
                // declared between its declaration and the location where it is referenced.
                add_potential_conflict(variable_infos, referenced_variable, declared_variable_info);

                // The declared variable must be sure not to "shadow" a variable declared in its
                // own or a higher scope, thus preventing the reference from seeing it.
                add_potential_conflict(variable_infos, declared_variable_info, referenced_variable);
            }
            if let Some((parent, grandparents)) = parents.split_last() {
                parent.record_potential_conflicts(
                    compiler,
                    grandparents,
                    variable_infos,
                    referenced_variable,
                );
            }
        }
    }
}

/// Handle storing and looking up `VariableInfo` objects for the variables that are visible in
/// the current `ScopeContext`.
struct VariableInfoStackMap {
    // Java's Deque stacks push at their head; the head is the last element here.
    map_of_stacks: IndexMap<JsString, Vec<VariableInfoId>>,
}

impl VariableInfoStackMap {
    // Rust-only constructor (Java field initializer).
    fn new() -> Self {
        Self {
            map_of_stacks: IndexMap::<_, _>::default(),
        }
    }

    /// Push a `VariableInfo` onto the stack corresponding to its name.
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfoStackMap#pushVariableInfo
    fn push_variable_info(&mut self, variable_infos: &[VariableInfo], info: VariableInfoId) {
        let name = variable_infos[info.0].get_name().clone();
        let variable_info_stack = self.get_variable_info_stack(&name);
        variable_info_stack.push(info);
    }

    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfoStackMap#getVariableInfoStack
    fn get_variable_info_stack(&mut self, name: &JsString) -> &mut Vec<VariableInfoId> {
        self.map_of_stacks.entry(name.clone()).or_default()
    }

    /// Pop the topmost `VariableInfo` off of the stack for the given variable name.
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfoStackMap#popVariableInfo
    fn pop_variable_info(&mut self, name: &JsString) -> VariableInfoId {
        let variable_infos = check_not_null!(
            self.map_of_stacks.get_mut(name),
            "Nonexistent variable info requested: '%s'",
            name
        );
        let info = variable_infos.pop().expect("NoSuchElementException");
        if variable_infos.is_empty() {
            // Remove empty stacks, so they can be garbage collected.
            self.map_of_stacks.shift_remove(name);
        }
        info
    }

    /// Empty all the variable name stacks.
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.VariableInfoStackMap#clear
    fn clear(&mut self) {
        self.map_of_stacks.clear();
    }
}

/// Represent a variable we've seen referenced but not declared.
///
/// One of these will be created when a global variable or simply undeclared variable is
/// referenced. It doesn't do much more than keep track of the name and the fact that we don't
/// have a declaration for it (as a 0 scope depth).
struct UndeclaredVariableInfo {
    name: JsString,
}

impl UndeclaredVariableInfo {
    // port: MakeDeclaredNamesUnique.ContextualRenameInverter.UndeclaredVariableInfo#UndeclaredVariableInfo
    fn new(name: JsString) -> Self {
        Self { name }
    }
}

/// Renames every local name to be unique. The first encountered declaration of a given name
/// (specifically a global declaration) is left in its original form. Those that are renamed are
/// made unique by giving them a unique suffix based on the number of declarations of the name.
///
/// The root ContextualRenamer is assumed to be in GlobalScope.
///
/// Used by the Normalize pass.
pub struct ContextualRenamer {
    scope_root: Option<NodeId>,

    // This multiset is shared between this ContextualRenamer and its parent (and its parent's
    // parent, etc.) because it tracks counts of variables across the entire JS program.
    // (Guava HashMultiset as element -> count; no code depends on its iteration order.)
    name_usage: Rc<RefCell<IndexMap<JsString, i32>>>,

    // By contrast, this is a different map for each ContextualRenamer because it's just keeping
    // track of the names used by this renamer. (A null value: the name keeps its name.)
    declarations: IndexMap<JsString, Option<JsString>>,
    global: bool,

    // None is Java's `hoistRenamer == this`.
    hoist_renamer: Option<RenamerRef>,

    // Rust-only: Java's `this` as a shared renamer (for a BoilerplateRenamer, the
    // BoilerplateRenamer object this ContextualRenamer is part of).
    this: Weak<RefCell<dyn Renamer>>,
}

impl ContextualRenamer {
    pub const UNIQUE_ID_SEPARATOR: &'static str = "$jscomp$";

    // port: MakeDeclaredNamesUnique.ContextualRenamer#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        let name_usage = self
            .name_usage
            .borrow()
            .iter()
            .map(|(name, count)| {
                if *count == 1 {
                    name.to_string()
                } else {
                    format!("{name} x {count}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let declarations = self
            .declarations
            .iter()
            .map(|(name, new_name)| match new_name {
                Some(new_name) => format!("{name}={new_name}"),
                None => format!("{name}=null"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "ContextualRenamer{{scopeRoot={}, nameUsage=[{}], declarations={{{}}}, global={}}}",
            self.scope_root
                .map_or_else(|| "null".to_string(), |n| n.to_string(ast)),
            name_usage,
            declarations,
            self.global
        )
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#ContextualRenamer()
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> RenamerRef {
        let renamer: Rc<RefCell<ContextualRenamer>> =
            Rc::new_cyclic(|this: &Weak<RefCell<ContextualRenamer>>| {
                let this: Weak<RefCell<dyn Renamer>> = this.clone();
                RefCell::new(Self::new_root(this))
            });
        renamer
    }

    // Rust-only body of the no-argument constructor, shared with BoilerplateRenamer's implicit
    // super() call.
    fn new_root(this: Weak<RefCell<dyn Renamer>>) -> Self {
        Self {
            scope_root: None,
            global: true,
            name_usage: Rc::new(RefCell::new(IndexMap::<_, _>::default())),
            declarations: IndexMap::<_, _>::default(),
            hoist_renamer: None,
            this,
        }
    }

    /// Constructor for child scopes.
    // port: MakeDeclaredNamesUnique.ContextualRenamer#ContextualRenamer(Node, Multiset, boolean, Renamer)
    fn new_child(
        ast: &Ast,
        scope_root: NodeId,
        name_usage: Rc<RefCell<IndexMap<JsString, i32>>>,
        hoisting_target_scope: bool,
        parent: &dyn Renamer,
    ) -> RenamerRef {
        check_state!(
            NodeUtil::creates_scope(ast, scope_root),
            "%s",
            scope_root.to_string(ast)
        );

        if scope_root.is_function(ast) {
            check_state!(!hoisting_target_scope, "%s", scope_root.to_string(ast));
        }

        let hoist_renamer = if hoisting_target_scope {
            check_state!(
                !NodeUtil::creates_block_scope(ast, scope_root),
                "%s",
                scope_root.to_string(ast)
            );
            None
        } else {
            check_state!(
                NodeUtil::creates_block_scope(ast, scope_root) || scope_root.is_function(ast),
                "%s",
                scope_root.to_string(ast)
            );
            Some(parent.get_hoist_renamer())
        };

        let renamer: Rc<RefCell<ContextualRenamer>> =
            Rc::new_cyclic(|this: &Weak<RefCell<ContextualRenamer>>| {
                let this: Weak<RefCell<dyn Renamer>> = this.clone();
                RefCell::new(Self {
                    scope_root: Some(scope_root),
                    global: false,
                    name_usage,
                    declarations: IndexMap::<_, _>::default(),
                    hoist_renamer,
                    this,
                })
            });
        renamer
    }

    /// Given a name and the associated id, create a new unique name.
    // port: MakeDeclaredNamesUnique.ContextualRenamer#getUniqueName
    fn get_unique_name(name: &JsString, id: i32) -> JsString {
        name.concat(&Self::UNIQUE_ID_SEPARATOR.into())
            .concat(&JsString::from(id.to_string()))
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#reserveName
    fn reserve_name(&self, name: &JsString) {
        // Multiset#setCount(name, 0, 1)
        let mut name_usage = self.name_usage.borrow_mut();
        if name_usage.get(name).copied().unwrap_or(0) == 0 {
            name_usage.insert(name.clone(), 1);
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#incrementNameCount
    fn increment_name_count(&self, name: &JsString) -> i32 {
        // Multiset#add(name, 1) returns the count before the addition.
        let mut name_usage = self.name_usage.borrow_mut();
        let count = name_usage.entry(name.clone()).or_insert(0);
        let old_count = *count;
        *count += 1;
        old_count
    }

    // Rust-only: Java's `this` as a shared renamer.
    fn this(&self) -> RenamerRef {
        self.this
            .upgrade()
            .expect("a renamer is alive while it is used")
    }
}

impl Renamer for ContextualRenamer {
    /// Adds a name to the map of names declared in this scope.
    // port: MakeDeclaredNamesUnique.ContextualRenamer#addDeclaredName
    fn add_declared_name(&mut self, name: &JsString, hoisted: bool) {
        if hoisted && let Some(hoist_renamer) = &self.hoist_renamer {
            hoist_renamer.borrow_mut().add_declared_name(name, true);
        } else if *name != MakeDeclaredNamesUnique::ARGUMENTS {
            if self.global {
                self.reserve_name(name);
            } else {
                // It hasn't been declared locally yet, so increment the count.
                if !self.declarations.contains_key(name) {
                    let mut id = self.increment_name_count(name);
                    let mut new_name = None;
                    if id != 0 {
                        // this name is exists at another source location, create another
                        // The stack of renamers traverse all scopes and track all declared
                        // names and their counts. This means, they could generate conflicting
                        // names if it does not see the entire program (e.g. when used via
                        // library level checks). Currently, the renamer stack is only used by
                        // Normalize and ScopedAliases pass.
                        // 1. When used in normalize, renamer stack see the whole program and
                        // tracks the name counts. Hence, when generating a unique name, it's
                        // guaranteed to generate names unique across the entire program.
                        // 2. When used in ScopedAliases, the renamer stack sees only the current
                        // library sources. But, in this use case, is only renames locals within
                        // a function body. Hence it does not generate conflicting declarations.
                        // We could stop tracking the counts and switch the renamer to use the
                        // file hashcode based UniqueIdSupplier, but that requires updating
                        // several unit tests that run normalize.
                        let mut unique_name = Self::get_unique_name(name, id);
                        while self
                            .name_usage
                            .borrow()
                            .get(&unique_name)
                            .is_some_and(|count| *count > 0)
                        {
                            // if the newName also exists at another location, create another
                            id = self.increment_name_count(&unique_name);
                            unique_name = Self::get_unique_name(name, id);
                        }
                        self.reserve_name(&unique_name); // reserve this new name so that it's never reused
                        new_name = Some(unique_name);
                    }
                    self.declarations.insert(name.clone(), new_name);
                }
            }
        }
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#getReplacementName
    fn get_replacement_name(&self, old_name: &JsString) -> Option<JsString> {
        self.declarations.get(old_name).cloned().flatten()
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#stripConstIfReplaced
    fn strip_const_if_replaced(&self) -> bool {
        false
    }

    /// Create a ContextualRenamer
    // port: MakeDeclaredNamesUnique.ContextualRenamer#createForChildScope
    fn create_for_child_scope(
        &self,
        ast: &Ast,
        scope_root: NodeId,
        hoisting_target_scope: bool,
    ) -> RenamerRef {
        ContextualRenamer::new_child(
            ast,
            scope_root,
            self.name_usage.clone(),
            hoisting_target_scope,
            self,
        )
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#getHoistRenamer
    fn get_hoist_renamer(&self) -> RenamerRef {
        match &self.hoist_renamer {
            Some(hoist_renamer) => hoist_renamer.clone(),
            None => self.this(),
        }
    }

    fn is_contextual_renamer(&self) -> bool {
        true
    }
}

/// Rename every declared name to be unique. Typically, this would be used when injecting code to
/// ensure that names do not conflict with existing names.
///
/// Used by the FunctionInjector
pub struct InlineRenamer {
    declarations: IndexMap<JsString, JsString>,
    unique_id_supplier: UniqueIdSupplierFn,
    id_prefix: String,
    remove_constness: bool,
    convention: Arc<dyn CodingConvention>,

    // None is Java's `hoistRenamer == this`.
    hoist_renamer: Option<RenamerRef>,

    // Rust-only: Java's `this` as a shared renamer.
    this: Weak<RefCell<dyn Renamer>>,
}

impl InlineRenamer {
    // port: MakeDeclaredNamesUnique.InlineRenamer#InlineRenamer
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        convention: Arc<dyn CodingConvention>,
        unique_id_supplier: UniqueIdSupplierFn,
        id_prefix: impl Into<String>,
        remove_constness: bool,
        hoisting_target_scope: bool,
        parent: Option<&dyn Renamer>,
    ) -> RenamerRef {
        let id_prefix = id_prefix.into();
        // To ensure that the id does not conflict with the id from the
        // ContextualRenamer some prefix is needed.
        check_argument!(!id_prefix.is_empty());

        let hoist_renamer = if hoisting_target_scope {
            None
        } else {
            Some(check_not_null!(parent).get_hoist_renamer())
        };

        let renamer: Rc<RefCell<InlineRenamer>> =
            Rc::new_cyclic(|this: &Weak<RefCell<InlineRenamer>>| {
                let this: Weak<RefCell<dyn Renamer>> = this.clone();
                RefCell::new(Self {
                    declarations: IndexMap::<_, _>::default(),
                    unique_id_supplier,
                    id_prefix,
                    remove_constness,
                    convention,
                    hoist_renamer,
                    this,
                })
            });
        renamer
    }

    // port: MakeDeclaredNamesUnique.InlineRenamer#getUniqueName
    fn get_unique_name(&self, name: &JsString) -> JsString {
        if name.is_empty() {
            return name.clone();
        }

        let separator: JsString = ContextualRenamer::UNIQUE_ID_SEPARATOR.into();
        let mut name = name.clone();
        if name.index_of(&separator) >= 0 {
            name = name.substring(0, name.last_index_of(&separator) as usize);
        }

        if self.convention.is_exported_name(&name) {
            // The google internal coding convention includes a naming convention
            // to export names starting with "_".  Simply strip "_" those to avoid
            // exporting names.
            name = JsString::from("JSCompiler_").concat(&name);
        }

        // By using the same separator the id will be stripped if it isn't
        // needed when variable renaming is turned off.
        name.concat(&separator)
            .concat(&JsString::from(self.id_prefix.as_str()))
            .concat(&JsString::from((self.unique_id_supplier)()))
    }

    // Rust-only: Java's `this` as a shared renamer.
    fn this(&self) -> RenamerRef {
        self.this
            .upgrade()
            .expect("a renamer is alive while it is used")
    }
}

impl Renamer for InlineRenamer {
    // port: MakeDeclaredNamesUnique.InlineRenamer#addDeclaredName
    fn add_declared_name(&mut self, name: &JsString, hoisted: bool) {
        check_state!(*name != MakeDeclaredNamesUnique::ARGUMENTS);
        if hoisted && let Some(hoist_renamer) = &self.hoist_renamer {
            hoist_renamer.borrow_mut().add_declared_name(name, hoisted);
        } else if !self.declarations.contains_key(name) {
            let unique_name = self.get_unique_name(name);
            self.declarations.insert(name.clone(), unique_name);
        }
    }

    // port: MakeDeclaredNamesUnique.InlineRenamer#getReplacementName
    fn get_replacement_name(&self, old_name: &JsString) -> Option<JsString> {
        self.declarations.get(old_name).cloned()
    }

    // port: MakeDeclaredNamesUnique.InlineRenamer#createForChildScope
    fn create_for_child_scope(
        &self,
        _ast: &Ast,
        _scope_root: NodeId,
        hoisting_target_scope: bool,
    ) -> RenamerRef {
        InlineRenamer::new(
            self.convention.clone(),
            self.unique_id_supplier.clone(),
            self.id_prefix.clone(),
            self.remove_constness,
            hoisting_target_scope,
            Some(self),
        )
    }

    // port: MakeDeclaredNamesUnique.InlineRenamer#stripConstIfReplaced
    fn strip_const_if_replaced(&self) -> bool {
        self.remove_constness
    }

    // port: MakeDeclaredNamesUnique.InlineRenamer#getHoistRenamer
    fn get_hoist_renamer(&self) -> RenamerRef {
        match &self.hoist_renamer {
            Some(hoist_renamer) => hoist_renamer.clone(),
            None => self.this(),
        }
    }
}

/// For injecting boilerplate libraries. Leaves global names alone and renames local names like
/// InlineRenamer.
pub struct BoilerplateRenamer {
    // The ContextualRenamer this class extends (its `this` is this BoilerplateRenamer).
    base: ContextualRenamer,
    unique_id_supplier: UniqueIdSupplierFn,
    id_prefix: String,
    convention: Arc<dyn CodingConvention>,
}

impl BoilerplateRenamer {
    // port: MakeDeclaredNamesUnique.BoilerplateRenamer#BoilerplateRenamer
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        convention: Arc<dyn CodingConvention>,
        unique_id_supplier: UniqueIdSupplierFn,
        id_prefix: impl Into<String>,
    ) -> RenamerRef {
        let id_prefix = id_prefix.into();
        let renamer: Rc<RefCell<BoilerplateRenamer>> =
            Rc::new_cyclic(|this: &Weak<RefCell<BoilerplateRenamer>>| {
                let this: Weak<RefCell<dyn Renamer>> = this.clone();
                RefCell::new(Self {
                    base: ContextualRenamer::new_root(this),
                    unique_id_supplier,
                    id_prefix,
                    convention,
                })
            });
        renamer
    }
}

impl Renamer for BoilerplateRenamer {
    // port: MakeDeclaredNamesUnique.ContextualRenamer#addDeclaredName (inherited)
    fn add_declared_name(&mut self, name: &JsString, hoisted: bool) {
        self.base.add_declared_name(name, hoisted);
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#getReplacementName (inherited)
    fn get_replacement_name(&self, old_name: &JsString) -> Option<JsString> {
        self.base.get_replacement_name(old_name)
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#stripConstIfReplaced (inherited)
    fn strip_const_if_replaced(&self) -> bool {
        self.base.strip_const_if_replaced()
    }

    // port: MakeDeclaredNamesUnique.BoilerplateRenamer#createForChildScope
    fn create_for_child_scope(&self, _ast: &Ast, _scope_root: NodeId, hoisted: bool) -> RenamerRef {
        InlineRenamer::new(
            self.convention.clone(),
            self.unique_id_supplier.clone(),
            self.id_prefix.clone(),
            false,
            hoisted,
            Some(self),
        )
    }

    // port: MakeDeclaredNamesUnique.ContextualRenamer#getHoistRenamer (inherited)
    fn get_hoist_renamer(&self) -> RenamerRef {
        self.base.get_hoist_renamer()
    }

    fn is_contextual_renamer(&self) -> bool {
        true
    }
}

/// Only rename things that match specific names. Wraps another renamer.
pub struct TargettedRenamer {
    delegate: RenamerRef,
    targets: Rc<IndexSet<JsString>>,
}

impl TargettedRenamer {
    // port: MakeDeclaredNamesUnique.TargettedRenamer#TargettedRenamer
    #[allow(clippy::new_ret_no_self)]
    pub fn new(delegate: RenamerRef, targets: Rc<IndexSet<JsString>>) -> RenamerRef {
        Rc::new(RefCell::new(Self { delegate, targets }))
    }
}

impl Renamer for TargettedRenamer {
    // port: MakeDeclaredNamesUnique.TargettedRenamer#addDeclaredName
    fn add_declared_name(&mut self, name: &JsString, hoisted: bool) {
        if self.targets.contains(name) {
            self.delegate.borrow_mut().add_declared_name(name, hoisted);
        }
    }

    // port: MakeDeclaredNamesUnique.TargettedRenamer#getReplacementName
    fn get_replacement_name(&self, old_name: &JsString) -> Option<JsString> {
        if self.targets.contains(old_name) {
            self.delegate.borrow().get_replacement_name(old_name)
        } else {
            None
        }
    }

    // port: MakeDeclaredNamesUnique.TargettedRenamer#stripConstIfReplaced
    fn strip_const_if_replaced(&self) -> bool {
        self.delegate.borrow().strip_const_if_replaced()
    }

    // port: MakeDeclaredNamesUnique.TargettedRenamer#createForChildScope
    fn create_for_child_scope(
        &self,
        ast: &Ast,
        scope_root: NodeId,
        hoisting_target_scope: bool,
    ) -> RenamerRef {
        let delegate =
            self.delegate
                .borrow()
                .create_for_child_scope(ast, scope_root, hoisting_target_scope);
        TargettedRenamer::new(delegate, self.targets.clone())
    }

    // port: MakeDeclaredNamesUnique.TargettedRenamer#getHoistRenamer
    fn get_hoist_renamer(&self) -> RenamerRef {
        self.delegate.borrow().get_hoist_renamer()
    }
}
