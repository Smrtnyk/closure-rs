/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ReferenceCollection.java.

//! Port of `ReferenceCollection.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::abstract_scope::ImplicitVar;
use crate::node_util::NodeUtil;
use crate::reference::Reference;
use crate::scope::ScopeId;
use closure_rhino::check_state;
use closure_rhino::node::Ast;
use std::sync::Arc;

/// A collection of references. Can be subclassed to apply checks or store additional state when
/// adding.
#[derive(Debug, Clone, Default)]
pub struct ReferenceCollection {
    pub references: Vec<Reference>,
}

impl ReferenceCollection {
    pub fn new() -> Self {
        Self::default()
    }

    // port: ReferenceCollection#iterator
    pub fn iter(&self) -> std::slice::Iter<'_, Reference> {
        self.references.iter()
    }

    // port: ReferenceCollection#add
    pub fn add(&mut self, reference: Reference) {
        self.references.push(reference);
    }

    /// Determines if the variable for this reference collection is "well-defined." A variable is
    /// well-defined if we can prove at compile-time that it's assigned a value before it's used.
    ///
    /// Notice that if this function returns false, this doesn't imply that the variable is used
    /// before it's assigned. It just means that we don't have enough information to make a
    /// definitive judgment.
    // port: ReferenceCollection#isWellDefined
    pub fn is_well_defined(&self, ast: &Ast) -> bool {
        let size = self.references.len();
        if size == 0 {
            return false;
        }

        // If this is a declaration that does not instantiate the variable,
        // it's not well-defined.
        let Some(init) = self.get_initializing_reference(ast) else {
            return false;
        };

        check_state!(self.references[0].is_declaration(ast));
        let init_block = init.get_basic_block().unwrap();
        for i in 1..size {
            if !init_block.provably_executes_before(self.references[i].get_basic_block().unwrap()) {
                return false;
            }
        }

        true
    }

    /// Whether the variable is escaped into an inner function.
    // port: ReferenceCollection#isEscaped
    pub fn is_escaped(&self, compiler: &AbstractCompiler) -> bool {
        let mut hoist_scope: Option<ScopeId> = None;
        for r in &self.references {
            if hoist_scope.is_none() {
                hoist_scope = r.get_scope().unwrap().get_closest_hoist_scope(compiler);
            } else if hoist_scope != r.get_scope().unwrap().get_closest_hoist_scope(compiler) {
                return true;
            }
        }
        false
    }

    /// Whether `this` is rebound in any reference's scope before reaching the declaration's hoist
    /// scope.
    // port: ReferenceCollection#isThisRebound
    pub fn is_this_rebound(&self, compiler: &AbstractCompiler) -> bool {
        let mut hoist_scope: Option<ScopeId> = None;
        for r in &self.references {
            let ref_hoist_scope = r.get_scope().unwrap().get_closest_hoist_scope(compiler);
            if hoist_scope.is_none() {
                hoist_scope = ref_hoist_scope;
            } else if hoist_scope != ref_hoist_scope {
                return true;
            }
            let mut s = r.get_scope();
            while s != hoist_scope && s.is_some() {
                let scope = s.unwrap();
                if ImplicitVar::THIS.is_made_by_scope(compiler, scope) {
                    return true;
                }
                s = scope.get_parent(compiler);
            }
        }
        false
    }

    /// `index`: The index into the references array to look for an assigning declaration.
    ///
    /// This is either the declaration if a value is assigned (such as "var a = 2", "function
    /// a()...", "... catch (a)...").
    // port: ReferenceCollection#isInitializingDeclarationAt
    fn is_initializing_declaration_at(&self, ast: &Ast, index: usize) -> bool {
        let maybe_init = &self.references[index];
        if maybe_init.is_initializing_declaration(ast) {
            // This is a declaration that represents the initial value.
            // Specifically, var declarations without assignments such as "var a;"
            // are not.
            return true;
        }
        false
    }

    /// `index`: The index into the references array to look for an initialized assignment
    /// reference. That is, an assignment immediately follow a variable declaration that itself
    /// does not initialize the variable.
    // port: ReferenceCollection#isInitializingAssignmentAt
    fn is_initializing_assignment_at(&self, ast: &Ast, index: usize) -> bool {
        if index < self.references.len() && index > 0 {
            let maybe_decl = &self.references[index - 1];
            if maybe_decl.is_var_declaration(ast) || maybe_decl.is_let_declaration(ast) {
                check_state!(!maybe_decl.is_initializing_declaration(ast));
                let maybe_init = &self.references[index];
                if maybe_init.is_simple_assignment_to_name(ast) {
                    return true;
                }
            }
        }
        false
    }

    /// Returns the reference that provides the value for the variable at the time of the first
    /// read, if known, otherwise null.
    ///
    /// This is either the variable declaration ("var a = ...") or first reference following the
    /// declaration if it is an assignment in the same `JSChunk`.
    // port: ReferenceCollection#getInitializingReference
    pub fn get_initializing_reference(&self, ast: &Ast) -> Option<&Reference> {
        if self.is_initializing_declaration_at(ast, 0) {
            return Some(&self.references[0]);
        } else if self.is_initializing_assignment_at(ast, 1)
            // Check that these references are in the same file as a way of ensuring they are in
            // the same chunk: checking for the same chunk would also be reasonable, but is
            // slightly more complicated and as of time of writing, didn't seem to help code size
            // in practice.
            && self.are_in_same_file(0, 1)
        {
            return Some(&self.references[1]);
        }
        None
    }

    // port: ReferenceCollection#areInSameFile
    fn are_in_same_file(&self, ref0: usize, ref1: usize) -> bool {
        self.references[ref0].get_input_id() == self.references[ref1].get_input_id()
    }

    /// Constants are allowed to be defined after their first use.
    // port: ReferenceCollection#getInitializingReferenceForConstants
    pub fn get_initializing_reference_for_constants(&self, ast: &Ast) -> Option<&Reference> {
        let size = self.references.len();
        for i in 0..size {
            if self.is_initializing_declaration_at(ast, i)
                || self.is_initializing_assignment_at(ast, i)
            {
                return Some(&self.references[i]);
            }
        }
        None
    }

    /// Returns whether the variable is only assigned a value once for its lifetime.
    // port: ReferenceCollection#isAssignedOnceInLifetime
    pub fn is_assigned_once_in_lifetime(&self, compiler: &mut AbstractCompiler) -> bool {
        let Some(r) = self.get_one_and_only_assignment(compiler) else {
            return false;
        };

        // Make sure this assignment is not in a loop or an enclosing function.
        let mut block = r.get_basic_block().cloned();
        while let Some(b) = block {
            if b.is_function() {
                let symbol_scope = r.get_symbol(compiler).unwrap().get_scope(compiler);
                if symbol_scope.get_closest_hoist_scope(compiler)
                    != r.get_scope().unwrap().get_closest_hoist_scope(compiler)
                {
                    return false;
                }
                break;
            }
            let symbol_scope = r.get_symbol(compiler).unwrap().get_scope(compiler);
            if b.get_root() == symbol_scope.get_root_node(compiler) {
                break;
            }
            if b.is_loop() {
                return false;
            }
            block = b.get_parent().map(Arc::clone);
        }

        true
    }

    /// Returns whether the variable is ever referenced weakly.
    // port: ReferenceCollection#isReferencedWeakly
    pub fn is_referenced_weakly(&self, ast: &Ast) -> bool {
        for r in &self.references {
            if NodeUtil::is_goog_weak_usage_call(ast, r.get_parent(ast)) {
                return true;
            }
        }
        false
    }

    /// Returns the one and only assignment. Returns null if the number of assignments is not
    /// exactly one.
    // port: ReferenceCollection#getOneAndOnlyAssignment
    pub fn get_one_and_only_assignment(&self, ast: &Ast) -> Option<&Reference> {
        let mut assignment: Option<&Reference> = None;
        let size = self.references.len();
        for i in 0..size {
            let r = &self.references[i];
            if r.is_lvalue(ast) || r.is_initializing_declaration(ast) {
                if assignment.is_none() {
                    assignment = Some(r);
                } else {
                    return None;
                }
            }
        }
        assignment
    }

    /// Returns whether the variable is never assigned a value.
    // port: ReferenceCollection#isNeverAssigned
    pub fn is_never_assigned(&self, ast: &Ast) -> bool {
        let size = self.references.len();
        for i in 0..size {
            let r = &self.references[i];
            if r.is_lvalue(ast) || r.is_initializing_declaration(ast) {
                return false;
            }
        }
        true
    }

    // port: ReferenceCollection#firstReferenceIsAssigningDeclaration
    pub fn first_reference_is_assigning_declaration(&self, ast: &Ast) -> bool {
        let size = self.references.len();
        size > 0 && self.references[0].is_initializing_declaration(ast)
    }

    // port: ReferenceCollection#toString
    pub fn to_string(&self, compiler: &mut AbstractCompiler) -> String {
        let init_ref = match self.get_initializing_reference(compiler) {
            Some(r) => r.to_string(compiler),
            None => "null".to_string(),
        };
        let references = self
            .references
            .iter()
            .map(|r| r.to_string(compiler))
            .collect::<Vec<_>>()
            .join(", ");
        let well_defined = self.is_well_defined(compiler);
        let assigned_once = self.is_assigned_once_in_lifetime(compiler);
        format!(
            "ReferenceCollection{{initRef={init_ref}, references=[{references}], \
             wellDefined={well_defined}, assignedOnce={assigned_once}}}"
        )
    }
}

impl<'a> IntoIterator for &'a ReferenceCollection {
    type Item = &'a Reference;
    type IntoIter = std::slice::Iter<'a, Reference>;

    fn into_iter(self) -> Self::IntoIter {
        self.references.iter()
    }
}
