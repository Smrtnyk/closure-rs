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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/InvalidatingTypes.java.

//! Port of InvalidatingTypes.java: keeps track of "invalidating types", types that cannot be used
//! for property disambiguation or other type-based optimizations.
use crate::source_file::SourceFile;
use crate::type_mismatch::TypeMismatch;
use closure_jstype::prelude::*;
use closure_rhino::ir::IR;
use closure_rhino::node::{Ast, NodeId};
use std::sync::Arc;

/// Keeps track of "invalidating types" that force type-based optimizations to back off.
///
/// Specifically for disambiguation and ambiguation, it's possible that the same property is
/// used on two types that are not related; Java's `ImmutableSetMultimap<JSType, Node>` keys use
/// `JSType#equals`/`hashCode`, so lookups here compare keys with `JSType#equals`.
// port: InvalidatingTypes
pub struct InvalidatingTypes {
    type_to_location: Vec<(TypeId, Vec<NodeId>)>,
}

impl InvalidatingTypes {
    // port: InvalidatingTypes#InvalidatingTypes
    fn new(type_to_location: Vec<(TypeId, Vec<NodeId>)>) -> Self {
        Self { type_to_location }
    }

    // port: InvalidatingTypes#isInvalidating
    pub fn is_invalidating(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: Option<TypeId>,
    ) -> bool {
        let Some(mut type_) = type_ else {
            return true;
        };
        if type_.is_unknown_type(reg, ast) || type_.is_empty_type(reg) {
            return true;
        }

        // A union type is invalidating if any one of its members is invalidating
        if type_.is_union_type(reg) {
            type_ = type_.restrict_by_not_null_or_undefined(reg, ast);
            if type_.is_union_type(reg) {
                let members = type_.get_union_members(reg, ast).unwrap();
                for alt in members.iter().copied() {
                    if self.is_invalidating(reg, ast, Some(alt)) {
                        return true;
                    }
                }
                return false;
            }
        }

        let Some(mut obj_type) = type_.to_maybe_object_type(reg) else {
            // TODO(b/174534994): why can't scalars be invalidating?
            return false;
        };

        if obj_type.is_templatized_type(reg) {
            obj_type = TemplatizedType::get_referenced_type(
                obj_type.to_maybe_templatized_type(reg).unwrap(),
                reg,
            );
        }

        contains_key(&self.type_to_location, reg, ast, obj_type)
            // Don't disambiguate properties on object types that are structurally compared or that
            // don't come from a literal class or function definition
            || Self::is_ambiguous_or_structural_type(reg, ast, obj_type)
    }

    // port: InvalidatingTypes#getMismatchLocations
    pub fn get_mismatch_locations(&self) -> &[(TypeId, Vec<NodeId>)] {
        &self.type_to_location
    }

    // Returns true if any of the following hold:
    //  - this type obeys structural subtyping rules, as opposed to nominal subtyping
    //  - this type is some JSDoc-only or anonymous type like a mixin, as opposed to a class or
    //    function literal
    // port: InvalidatingTypes#isAmbiguousOrStructuralType
    fn is_ambiguous_or_structural_type(reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        if type_.is_enum_type(reg) {
            // enum types are created via object literals, which are normally structural, but
            // Closure special-cases them to behave as if nominal.
            false
        } else if type_.is_enum_element_type(reg) {
            let primitive = type_
                .to_maybe_enum_element_type(reg)
                .unwrap()
                .get_primitive_type(reg)
                .to_maybe_object_type(reg);
            // Treat an Enum<Foo> identically to a Foo
            match primitive {
                None => true,
                Some(primitive) => Self::is_ambiguous_or_structural_type(reg, ast, primitive),
            }
        } else if type_.is_function_type(reg) {
            !type_.is_nominal_constructor_or_interface(reg)
                || type_
                    .to_maybe_function_type(reg)
                    .unwrap()
                    .is_ambiguous_constructor(reg, ast)
        } else if type_.is_function_prototype_type(reg) {
            let owner_function = type_.get_owner_function(reg);
            match owner_function {
                None => true,
                Some(owner_function) => {
                    !owner_function.is_nominal_constructor_or_interface(reg)
                        || owner_function.is_ambiguous_constructor(reg, ast)
                        || owner_function.is_structural_interface(reg)
                }
            }
        } else if type_.is_instance_type(reg) {
            let ctor = type_.get_constructor(reg);
            match ctor {
                None => true,
                Some(ctor) => {
                    ctor.is_ambiguous_constructor(reg, ast) || ctor.is_structural_interface(reg)
                }
            }
        } else {
            true
        }
    }
}

/// Java's Builder keeps the JSTypeRegistry it was created with; registry and AST are passed to
/// the methods that need them here (closure-jstype's convention).
// port: InvalidatingTypes.Builder
#[derive(Default)]
pub struct Builder {
    type_to_location: Vec<(TypeId, Vec<NodeId>)>,
}

impl Builder {
    // port: InvalidatingTypes.Builder#Builder
    pub fn new(_registry: &JSTypeRegistry) -> Self {
        Self::default()
    }

    /// Java's ALWAYS_INVALIDATING_LOCATION is one static node; nodes live in an arena here, so
    /// build creates it in the caller's arena.
    // port: InvalidatingTypes.Builder#build
    pub fn build(mut self, reg: &mut JSTypeRegistry, ast: &mut Ast) -> InvalidatingTypes {
        let always_invalidating_location = always_invalidating_location(ast);
        for t in ALWAYS_INVALIDATING_TYPES {
            let native = reg.get_native_type(t);
            put(
                &mut self.type_to_location,
                reg,
                ast,
                native,
                always_invalidating_location,
            );
        }
        InvalidatingTypes::new(self.type_to_location)
    }

    // port: InvalidatingTypes.Builder#addAllTypeMismatches
    pub fn add_all_type_mismatches<'a>(
        mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        mismatches: impl IntoIterator<Item = &'a TypeMismatch>,
    ) -> Self {
        for mismatch in mismatches {
            self.add_type_with_reason(reg, ast, mismatch.found(), mismatch.location());
            self.add_type_with_reason(reg, ast, mismatch.required(), mismatch.location());
        }
        self
    }

    // port: InvalidatingTypes.Builder#addTypeWithReason
    fn add_type_with_reason(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
        location: NodeId,
    ) {
        let type_ = type_.restrict_by_not_null_or_undefined(reg, ast);

        if type_.is_union_type(reg) {
            let members = type_.get_union_members(reg, ast).unwrap();
            for alt in members.iter().copied() {
                self.add_type_with_reason(reg, ast, alt, location);
            }
            return;
        }

        if type_.is_enum_element_type(reg) {
            let enumerated = type_.get_enumerated_type_of_enum_element(reg).unwrap();
            self.add_type_with_reason(reg, ast, enumerated, location);
            return;
        }

        let Some(obj_type) = type_.to_maybe_object_type(reg) else {
            return;
        };

        self.record_type_with_reason(reg, ast, Some(obj_type), location);
        let implicit_prototype = obj_type.get_implicit_prototype(reg, ast);
        self.record_type_with_reason(reg, ast, implicit_prototype, location);

        if obj_type.is_constructor(reg) {
            // TODO(b/142431852): This should never be null but it is possible.
            // Case: `function(new:T)`, `T = number`.
            let instance_type = obj_type
                .to_maybe_function_type(reg)
                .unwrap()
                .get_instance_type(reg);
            self.record_type_with_reason(reg, ast, instance_type, location);
        } else if obj_type.is_instance_type(reg) {
            let constructor = obj_type.get_constructor(reg);
            self.record_type_with_reason(reg, ast, constructor, location);
        }
    }

    // port: InvalidatingTypes.Builder#recordTypeWithReason
    fn record_type_with_reason(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: Option<TypeId>,
        location: NodeId,
    ) {
        let Some(mut type_) = type_ else {
            return;
        };

        if type_.is_templatized_type(reg) {
            type_ = TemplatizedType::get_referenced_type(
                type_.to_maybe_templatized_type(reg).unwrap(),
                reg,
            );
        }

        if InvalidatingTypes::is_ambiguous_or_structural_type(reg, ast, type_) {
            // This type is inherently invalidating. Putting it in the map wastes memory.
            // This also fixes a performance regression: previously we saw ~4k structural types
            // hash to the same bucket in the "typeToLocation" map, causing >100 seconds spent in
            // hash map lookups for some builds.
            return;
        }

        put(&mut self.type_to_location, reg, ast, type_, location);
    }
}

// port: InvalidatingTypes#ALWAYS_INVALIDATING_TYPES
const ALWAYS_INVALIDATING_TYPES: [JSTypeNative; 7] = [
    JSTypeNative::FUNCTION_FUNCTION_TYPE,
    JSTypeNative::FUNCTION_TYPE,
    JSTypeNative::FUNCTION_PROTOTYPE,
    JSTypeNative::FUNCTION_INSTANCE_PROTOTYPE,
    JSTypeNative::OBJECT_TYPE,
    JSTypeNative::OBJECT_PROTOTYPE,
    JSTypeNative::OBJECT_FUNCTION_TYPE,
];

// port: InvalidatingTypes#ALWAYS_INVALIDATING_LOCATION
fn always_invalidating_location(ast: &mut Ast) -> NodeId {
    IR::name(ast, "alwaysInvalidatingLocation").set_static_source_file(
        ast,
        Some(Arc::new(SourceFile::from_code(
            "InvalidatingTypes_alwaysInvalidatingLocation",
            "",
        ))),
    )
}

/// ImmutableSetMultimap.Builder#put: keys compare with JSType#equals (insertion order kept),
/// values with Node identity.
fn put(
    map: &mut Vec<(TypeId, Vec<NodeId>)>,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    key: TypeId,
    value: NodeId,
) {
    for (k, values) in map.iter_mut() {
        if key.equals(reg, ast, *k) {
            if !values.contains(&value) {
                values.push(value);
            }
            return;
        }
    }
    map.push((key, vec![value]));
}

/// ImmutableSetMultimap#containsKey with JSType#equals keys.
fn contains_key(
    map: &[(TypeId, Vec<NodeId>)],
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    key: TypeId,
) -> bool {
    map.iter().any(|(k, _)| key.equals(reg, ast, *k))
}
