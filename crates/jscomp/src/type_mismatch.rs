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
//   src/com/google/javascript/jscomp/TypeMismatch.java.

//! Port of TypeMismatch.java: a type mismatch found by TypeValidator, and the Accumulator that
//! TypeValidator registers them with.
use closure_jstype::prelude::*;
use closure_jstype::{JSTypeRegistry, TypeId};
use closure_rhino::ir::IR;
use closure_rhino::node::{Ast, NodeId};

/// Signals that the first type and the second type have been used interchangeably.
///
/// Type-based optimizations should take this into account so that they don't wreck code with
/// type warnings.
// port: TypeMismatch (record: found, required, location; requireNonNull holds by construction)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeMismatch {
    found: TypeId,
    required: TypeId,
    location: NodeId,
}

impl TypeMismatch {
    // port: TypeMismatch#TypeMismatch
    pub fn new(found: TypeId, required: TypeId, location: NodeId) -> Self {
        Self {
            found,
            required,
            location,
        }
    }
    // port: TypeMismatch#found
    pub fn found(&self) -> TypeId {
        self.found
    }
    // port: TypeMismatch#required
    pub fn required(&self) -> TypeId {
        self.required
    }
    // port: TypeMismatch#location
    pub fn location(&self) -> NodeId {
        self.location
    }
    // port: TypeMismatch#getFound
    pub fn get_found(&self) -> TypeId {
        self.found()
    }
    // port: TypeMismatch#getRequired
    pub fn get_required(&self) -> TypeId {
        self.required()
    }
    // port: TypeMismatch#getLocation
    pub fn get_location(&self) -> NodeId {
        self.location()
    }
    // port: TypeMismatch#create
    fn create(found: TypeId, required: TypeId, location: NodeId) -> Self {
        Self::new(found, required, location)
    }
    /// Java shares one static `IR.empty()` TEST_LOCATION node; nodes live in an arena here, so
    /// the EMPTY node is created in the caller's arena.
    // port: TypeMismatch#createForTesting
    pub fn create_for_testing(ast: &mut Ast, found: TypeId, required: TypeId) -> Self {
        let test_location = IR::empty(ast);
        Self::create(found, required, test_location)
    }
}

// port: TypeMismatch.Accumulator
#[derive(Clone, Debug, Default)]
pub struct Accumulator {
    mismatches: Vec<TypeMismatch>,
}

impl Accumulator {
    // port: TypeMismatch.Accumulator#Accumulator
    pub fn new() -> Self {
        Self::default()
    }
    // port: TypeMismatch.Accumulator#registerMismatch
    pub fn register_mismatch(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        location: NodeId,
        found: TypeId,
        required: TypeId,
    ) {
        // Don't register a mismatch for differences in null or undefined or if the
        // code didn't downcast.
        let found = Self::remove_null_undefined_and_templates(reg, ast, found);
        let required = Self::remove_null_undefined_and_templates(reg, ast, required);
        if found.is_subtype_of(reg, ast, required) || required.is_subtype_of(reg, ast, found) {
            return;
        }

        if required.is_union_type(reg) {
            let alternates = required
                .to_maybe_union_type(reg)
                .unwrap()
                .get_alternates(reg, ast);
            for required_alt_type in alternates.iter().copied() {
                if required_alt_type.is_subtype_of(reg, ast, found) {
                    return;
                }
            }
        }

        if Self::both_are_not_template_types(reg, found, required) {
            self.mismatches
                .push(TypeMismatch::create(found, required, location));
        }

        if found.is_function_type(reg) && required.is_function_type(reg) {
            let fn_type_a = found.to_maybe_function_type(reg).unwrap();
            let fn_type_b = required.to_maybe_function_type(reg).unwrap();
            let params_a = fn_type_a.get_parameters(reg);
            let params_b = fn_type_b.get_parameters(reg);
            let mut param_it_a = params_a.iter();
            let mut param_it_b = params_b.iter();
            while let (Some(a), Some(b)) = (param_it_a.next(), param_it_b.next()) {
                self.register_if_mismatch(
                    reg,
                    ast,
                    location,
                    Some(a.get_jstype()),
                    Some(b.get_jstype()),
                );
            }
            let return_a = fn_type_a.get_return_type(reg);
            let return_b = fn_type_b.get_return_type(reg);
            self.register_if_mismatch(reg, ast, location, Some(return_a), Some(return_b));
        }
    }
    // port: TypeMismatch.Accumulator#getMismatches
    pub fn get_mismatches(&self) -> Vec<TypeMismatch> {
        self.mismatches.clone()
    }
    // port: TypeMismatch.Accumulator#registerIfMismatch
    fn register_if_mismatch(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        location: NodeId,
        found: Option<TypeId>,
        required: Option<TypeId>,
    ) {
        if let (Some(found), Some(required)) = (found, required)
            && !found.is_subtype_of(reg, ast, required)
        {
            self.register_mismatch(reg, ast, location, found, required);
        }
    }
    // port: TypeMismatch.Accumulator#bothAreNotTemplateTypes
    fn both_are_not_template_types(reg: &JSTypeRegistry, found: TypeId, required: TypeId) -> bool {
        !found.is_template_type(reg) && !required.is_template_type(reg)
    }
    // port: TypeMismatch.Accumulator#removeNullUndefinedAndTemplates
    fn remove_null_undefined_and_templates(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        t: TypeId,
    ) -> TypeId {
        let result = t.restrict_by_not_null_or_undefined(reg, ast);
        let obj = result.to_maybe_object_type(reg);
        if let Some(obj) = obj
            && obj.is_templatized_type(reg)
        {
            // We don't care about the specific specalization involved in the mismatch because
            // all specializations share the same JS code.
            return obj
                .to_maybe_templatized_type(reg)
                .unwrap()
                .get_raw_type(reg);
        }
        result
    }
}
