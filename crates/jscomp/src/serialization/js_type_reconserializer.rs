/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/JSTypeReconserializer.java.

//! Port of serialization/JSTypeReconserializer.java.
//!
//! Java's reconserializer holds the JSTypeRegistry; the Rust registry lives in the compiler next to
//! the `Ast` its types refer to, so every method that reaches a type takes both.
use super::js_type_color_id_hasher::JSTypeColorIdHasher;
use super::serialization_options::SerializationOptions;
use super::string_pool::StringPoolBuilder;
use super::type_pointers::{OFFSET_TO_AXIOMATIC_COLOR, TypePointers};
use super::types_proto::{
    ObjectTypeProto, SubtypingEdge, TypePool, TypePoolDebugInfo, TypePoolDebugInfoMismatch,
    TypeProto, UnionTypeProto,
};
use crate::invalidating_types::InvalidatingTypes;
use closure_jstype::prelude::*;
use closure_rhino::closure_primitive::ClosurePrimitive;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_colors::color::Color;
use closure_rhino::jscomp_colors::color_id::ColorId;
use closure_rhino::jscomp_colors::standard_colors;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::{check_not_null, check_state};
use std::cell::RefCell;
use std::rc::Rc;

/// port: JSTypeReconserializer.State
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    COLLECTING_TYPES,
    GENERATING_POOL,
    FINISHED,
}

/// port: JSTypeReconserializer.SeenTypeRecord
///
/// Java shares the record objects between `seenTypeRecords`, `typeToRecordCache` and union member
/// sets; Rust keeps them in `seen_type_records` and refers to them by `pointer`, which is their
/// insertion index there.
#[derive(Debug)]
struct SeenTypeRecord {
    color_id: ColorId,
    pointer: i32,

    /// 2021-05-25: It's faster to build a list and reconcile duplicates than to deduplicate using a
    /// set. The likely cause is that the caches head of these lists have low hit-rates for unions,
    /// and that union reconciliation doesn't actually look at most entries in this list.
    jstypes: Vec<TypeId>,

    union_members: Option<IndexSet<i32>>,
}

impl SeenTypeRecord {
    // port: JSTypeReconserializer.SeenTypeRecord#<init>
    fn new(color_id: ColorId, pointer: i32) -> Self {
        Self {
            color_id,
            pointer,
            jstypes: Vec::new(),
            union_members: None,
        }
    }
}

/// port: JSTypeReconserializer
///
/// Takes `JSType`s produced by JSCompiler's typechecker and deduplicates and serializes them into
/// the TypedAST colors format.
///
/// The deduplication phase is called "reconciliation". It's necessary because the the TypedAST
/// colors format is simpler than the `JSType` format, and so there is a many-to-one relationship
/// between `JSType`s and TypedAST colors.
pub struct JSTypeReconserializer {
    serialization_mode: SerializationOptions,
    invalidating_types: InvalidatingTypes,
    string_pool_builder: Rc<RefCell<StringPoolBuilder>>,
    hasher: JSTypeColorIdHasher,
    should_propagate_property_name: Box<dyn Fn(&JsString) -> bool>,

    // Cache some commonly used types.
    unknown_record: i32,
    top_function_record: i32,

    type_to_record_cache: IndexMap<TypeId, i32>,
    seen_type_records: IndexMap<ColorId, SeenTypeRecord>,
    disambiguate_edges: IndexMap<i32, IndexSet<i32>>,

    state: State,
}

// This is a one-way mapping because some JSTypes go to the same Color.
// port: JSTypeReconserializer#JSTYPE_NATIVE_TO_AXIOMATIC_COLOR_MAP
fn jstype_native_to_axiomatic_color_map() -> [(JSTypeNative, &'static Color); 20] {
    [
        // Merge all the various top/bottom-like/unknown types into a single unknown type.
        (JSTypeNative::ALL_TYPE, &standard_colors::UNKNOWN),
        (
            JSTypeNative::CHECKED_UNKNOWN_TYPE,
            &standard_colors::UNKNOWN,
        ),
        (JSTypeNative::NO_OBJECT_TYPE, &standard_colors::UNKNOWN),
        (JSTypeNative::NO_TYPE, &standard_colors::UNKNOWN),
        (JSTypeNative::UNKNOWN_TYPE, &standard_colors::UNKNOWN),
        // Map all the primitives in the obvious way.
        (JSTypeNative::BIGINT_TYPE, &standard_colors::BIGINT),
        (JSTypeNative::BOOLEAN_TYPE, &standard_colors::BOOLEAN),
        (JSTypeNative::GBIGINT_TYPE, &standard_colors::GBIGINT),
        (JSTypeNative::NULL_TYPE, &standard_colors::NULL_OR_VOID),
        (JSTypeNative::NUMBER_TYPE, &standard_colors::NUMBER),
        (JSTypeNative::STRING_TYPE, &standard_colors::STRING),
        (JSTypeNative::SYMBOL_TYPE, &standard_colors::SYMBOL),
        (JSTypeNative::VOID_TYPE, &standard_colors::NULL_OR_VOID),
        // Smoosh top-like function types into a single type.
        (
            JSTypeNative::FUNCTION_FUNCTION_TYPE,
            &standard_colors::TOP_FUNCTION,
        ),
        (JSTypeNative::FUNCTION_TYPE, &standard_colors::TOP_FUNCTION),
        // Smoosh top-like objects into a single type.
        (
            JSTypeNative::FUNCTION_PROTOTYPE,
            &standard_colors::TOP_OBJECT,
        ),
        (
            JSTypeNative::FUNCTION_INSTANCE_PROTOTYPE,
            &standard_colors::TOP_OBJECT,
        ),
        (
            JSTypeNative::OBJECT_FUNCTION_TYPE,
            &standard_colors::TOP_OBJECT,
        ),
        (JSTypeNative::OBJECT_PROTOTYPE, &standard_colors::TOP_OBJECT),
        (JSTypeNative::OBJECT_TYPE, &standard_colors::TOP_OBJECT),
    ]
}

impl JSTypeReconserializer {
    // port: JSTypeReconserializer#<init>
    fn new(
        registry: &JSTypeRegistry,
        invalidating_types: InvalidatingTypes,
        string_pool_builder: Rc<RefCell<StringPoolBuilder>>,
        should_propagate_property_name: Box<dyn Fn(&JsString) -> bool>,
        serialization_mode: SerializationOptions,
    ) -> Self {
        let mut this = Self {
            hasher: JSTypeColorIdHasher::new(registry),
            invalidating_types,
            string_pool_builder,
            should_propagate_property_name,
            serialization_mode,
            unknown_record: -1,
            top_function_record: -1,
            type_to_record_cache: IndexMap::<_, _>::default(),
            seen_type_records: IndexMap::<_, _>::default(),
            disambiguate_edges: IndexMap::<_, _>::default(),
            state: State::COLLECTING_TYPES,
        };

        this.seed_caches_with_axiomatic_types(registry);

        this.unknown_record = this.seen_type_records[&standard_colors::UNKNOWN.get_id()].pointer;
        this.top_function_record =
            this.seen_type_records[&standard_colors::TOP_FUNCTION.get_id()].pointer;
        this
    }

    /// Initializes a JSTypeReconserializer
    ///
    /// `should_propagate_property_name` decides whether a property present on some ObjectType
    /// should actually be serialized. Used to avoid serializing properties that won't impact
    /// optimizations (because they aren't present in the AST)
    // port: JSTypeReconserializer#create
    pub fn create(
        registry: &JSTypeRegistry,
        invalidating_types: InvalidatingTypes,
        string_pool_builder: Rc<RefCell<StringPoolBuilder>>,
        should_propagate_property_name: Box<dyn Fn(&JsString) -> bool>,
        serialization_mode: SerializationOptions,
    ) -> Self {
        let serializer = Self::new(
            registry,
            invalidating_types,
            string_pool_builder,
            should_propagate_property_name,
            serialization_mode,
        );
        serializer.check_valid_linear_time();
        serializer
    }

    /// Returns a pointer to the given type. If it is not already serialized, serializes it too
    // port: JSTypeReconserializer#serializeType
    pub fn serialize_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, r#type: TypeId) -> i32 {
        self.record_type(reg, ast, r#type)
    }

    /// Returns the pointer of the record of `type` (Java returns the record itself).
    // port: JSTypeReconserializer#recordType
    fn record_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, r#type: TypeId) -> i32 {
        let forwarded_type: Option<TypeId> = if r#type.is_named_type(reg) {
            Some(closure_jstype::named_type::NamedType::get_referenced_type(
                r#type.to_maybe_named_type(reg).unwrap(),
                reg,
            ))
        } else if r#type.is_enum_element_type(reg) {
            Some(
                r#type
                    .to_maybe_enum_element_type(reg)
                    .unwrap()
                    .get_primitive_type(reg),
            )
        } else if r#type.is_known_symbol_value_type(reg) {
            Some(reg.get_native_type(JSTypeNative::SYMBOL_TYPE))
        } else if r#type.is_templatized_type(reg) {
            Some(
                closure_jstype::templatized_type::TemplatizedType::get_referenced_type(
                    r#type.to_maybe_templatized_type(reg).unwrap(),
                    reg,
                ),
            )
        } else if r#type.is_function_type(reg)
            && r#type
                .to_maybe_function_type(reg)
                .unwrap()
                .get_canonical_representation(reg)
                .is_some()
        {
            r#type
                .to_maybe_function_type(reg)
                .unwrap()
                .get_canonical_representation(reg)
        } else {
            None
        };
        if let Some(forwarded_type) = forwarded_type {
            return self.record_type(reg, ast, forwarded_type);
        }

        if r#type.is_unknown_type(reg, ast)
            || r#type.is_no_resolved_type(reg)
            || r#type.is_template_type(reg)
        {
            // template types are not serialized because optimizations don't seem to care about them.
            // serialize as the UNKNOWN_TYPE because bounded generics are unsupported
            return self.unknown_record;
        }
        if r#type.is_function_type(reg) {
            let fn_type = r#type.to_maybe_function_type(reg).unwrap();
            if !fn_type.has_instance_type(reg) && fn_type.get_closure_primitive(reg).is_none() {
                // Distinguishing different function types does not matter for optimizations unless
                // they are a constructor/interface or have a @closurePrimitive tag associated.
                // Optimization colors don't track function parameter/return/template types, and
                // function literals are all invalidating types during property disambiguation.
                return self.top_function_record;
            }
        }

        if let Some(&jstype_record) = self.type_to_record_cache.get(&r#type) {
            return jstype_record;
        }

        if r#type.is_union_type(reg) {
            let union = r#type.to_maybe_union_type(reg).unwrap();
            return self.record_union_type(reg, ast, union);
        } else if r#type.is_object_type(reg, ast) {
            let object = r#type.to_maybe_object_type(reg).unwrap();
            return self.record_object_type(reg, ast, object);
        }

        panic!("AssertionError: {}", r#type.to_string(reg, ast));
    }

    // port: JSTypeReconserializer#recordUnionType
    fn record_union_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, r#type: TypeId) -> i32 {
        let mut alt_records: IndexSet<i32> = IndexSet::<_>::default();
        let alternates = r#type.get_alternates(reg, ast);
        for &alt_type in alternates.iter() {
            let alt = self.record_type(reg, ast, alt_type);
            match &self.record(alt).union_members {
                None => {
                    alt_records.insert(alt);
                }
                Some(members) => {
                    // Flatten out any nested unions. They are possible due to proxy-like types.
                    alt_records.extend(members.iter().copied());
                }
            }
        }

        // Some elements of the union may be equal as Colors
        if alt_records.len() == 1 {
            return *alt_records.first().unwrap();
        }

        let mut alternate_ids: IndexSet<ColorId> = IndexSet::<_>::default();
        for &alt_record in &alt_records {
            alternate_ids.insert(self.record(alt_record).color_id);
        }
        let union_id = ColorId::union(&alternate_ids);
        let record = self.get_or_create_record(union_id, r#type);

        let run_validation = self.serialization_mode.run_validation();
        let existing = self.record(record).union_members.clone();
        match existing {
            None => {
                self.record_mut(record).union_members = Some(alt_records);
            }
            Some(union_members) if run_validation => {
                if alt_records != union_members {
                    let ids = |records: &IndexSet<i32>| -> String {
                        let ids: Vec<String> = records
                            .iter()
                            .map(|&r| self.record(r).color_id.to_string())
                            .collect();
                        ["[", &ids.join(", "), "]"].join("")
                    };
                    check_state!(
                        false,
                        "Unions with same ID must have same members: %s => %s == %s",
                        union_id,
                        ids(&alt_records),
                        ids(&union_members)
                    );
                }
            }
            Some(_) => {}
        }

        record
    }

    // port: JSTypeReconserializer#recordObjectType
    fn record_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, r#type: TypeId) -> i32 {
        let id = self.hasher.hash_object_type(reg, ast, r#type);
        let record = self.get_or_create_record(id, r#type);
        self.add_supertype_edges(reg, ast, r#type, record);

        if r#type.is_function_type(reg) {
            let fn_type = r#type.to_maybe_function_type(reg).unwrap();
            if fn_type.has_instance_type(reg)
                && let Some(instance_type) = fn_type.get_instance_type(reg)
            {
                // We have to serialize these here ahead of time to avoid a
                // ConcurrentModificationException during reconciliation.
                self.serialize_type(reg, ast, instance_type);
                let prototype = FunctionType::get_prototype(fn_type, reg, ast);
                self.serialize_type(reg, ast, prototype);
            }
        }

        record
    }

    // port: JSTypeReconserializer#addSupertypeEdges
    fn add_supertype_edges(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        serialized_subtype: i32,
    ) {
        let ancestors = self.own_ancestor_interfaces_of(reg, ast, subtype);
        // SetMultimap#putAll with no values adds no key (the key order is the multimap's
        // key iteration order in Java).
        if !ancestors.is_empty() {
            self.disambiguate_edges
                .entry(serialized_subtype)
                .or_default()
                .extend(ancestors);
        }
        if let Some(implicit_prototype) = subtype.get_implicit_prototype(reg, ast) {
            let supertype = self.serialize_type(reg, ast, implicit_prototype);
            self.disambiguate_edges
                .entry(serialized_subtype)
                .or_default()
                .insert(supertype);
        }
    }

    // port: JSTypeReconserializer#getOrCreateRecord
    fn get_or_create_record(&mut self, id: ColorId, jstype: TypeId) -> i32 {
        check_state!(State::COLLECTING_TYPES == self.state || State::GENERATING_POOL == self.state);

        let pointer = self.seen_type_records.len() as i32;
        let record = self
            .seen_type_records
            .entry(id)
            .or_insert_with(|| SeenTypeRecord::new(id, pointer));
        record.jstypes.push(jstype);
        let record = record.pointer;
        self.type_to_record_cache.insert(jstype, record);

        record
    }

    // port: JSTypeReconserializer#reconcileUnionTypes
    fn reconcile_union_types(&self, seen: &SeenTypeRecord) -> TypeProto {
        let mut members: Vec<i32> = seen
            .union_members
            .as_ref()
            .unwrap()
            .iter()
            .copied()
            .collect();
        members.sort();
        TypeProto::new_builder()
            .set_union(
                UnionTypeProto::new_builder()
                    .add_all_union_member(members)
                    .build(),
            )
            .build()
    }

    // port: JSTypeReconserializer#reconcileObjectTypes
    fn reconcile_object_types(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        seen: i32,
    ) -> TypeProto {
        let mut instance_pointers: IndexSet<i32> = IndexSet::<_>::default();
        let mut prototype_pointers: IndexSet<i32> = IndexSet::<_>::default();
        let mut own_properties: IndexSet<i32> = IndexSet::<_>::default();
        let mut is_closure_assert = false;
        let mut is_constructor = false;
        let mut is_invalidating = false;
        let mut properties_keep_original_name = false;

        let jstypes = self.record(seen).jstypes.clone();
        for r#type in jstypes {
            let obj_type = r#type.to_maybe_object_type(reg);
            if obj_type.is_none() {
                let message = r#type.to_string(reg, ast);
                let _ = check_not_null!(obj_type, "%s", message);
            }
            let obj_type = obj_type.unwrap();

            if obj_type.is_function_type(reg) {
                let fn_type = obj_type.to_maybe_function_type(reg).unwrap();

                // Serialize prototypes and instance types for instantiable types. Even if these
                // types never appear on the AST, optimizations need to know that at runtime these
                // types may be present.
                if fn_type.has_instance_type(reg)
                    && let Some(instance_type) = fn_type.get_instance_type(reg)
                {
                    instance_pointers.insert(self.serialize_type(reg, ast, instance_type));
                    let prototype = FunctionType::get_prototype(fn_type, reg, ast);
                    prototype_pointers.insert(self.serialize_type(reg, ast, prototype));
                    is_constructor |= fn_type.is_constructor(reg);
                }

                is_closure_assert |= Self::is_closure_assert(fn_type.get_closure_primitive(reg));
            }

            for own_property in obj_type.get_own_property_names(reg) {
                // TODO(b/169899789): consider omitting common, well-known properties like
                // "prototype" to save space.
                if (self.should_propagate_property_name)(&own_property) {
                    own_properties.insert(self.string_pool_builder.borrow_mut().put(own_property));
                }
            }

            is_invalidating |= self
                .invalidating_types
                .is_invalidating(reg, ast, Some(obj_type));

            // To support legacy code, property disambiguation never renames properties of enums
            // (e.g. 'A' in '/** @enum */ const E = {A: 0}`). In theory this would be safe to
            // remove if we clean up code depending on the lack of renaming
            properties_keep_original_name |= obj_type.is_enum_type(reg);
        }

        let object_proto = ObjectTypeProto::new_builder()
            .add_all_instance_type(instance_pointers)
            .add_all_own_property(own_properties)
            .add_all_prototype(prototype_pointers)
            .set_closure_assert(is_closure_assert)
            .set_is_invalidating(is_invalidating)
            .set_marked_constructor(is_constructor)
            .set_properties_keep_original_name(properties_keep_original_name)
            .set_uuid(self.record(seen).color_id.as_byte_string())
            .build();
        TypeProto::new_builder().set_object(object_proto).build()
    }

    /// Returns the interfaces directly implemented and extended by `type`.
    ///
    /// Some of these relationships represent type errors; however, the graph needs to contain
    /// those edges for safe disambiguation. In particular, code generated from other languages
    /// (e.g TS) might have more flexible subtyping rules.
    // port: JSTypeReconserializer#ownAncestorInterfacesOf
    fn own_ancestor_interfaces_of(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        r#type: TypeId,
    ) -> Vec<i32> {
        let Some(ctor_type) = ObjectType::get_constructor(r#type, reg) else {
            return Vec::new();
        };

        let mut ancestors = Vec::new();
        let interfaces: Vec<TypeId> = ctor_type
            .get_extended_interfaces(reg)
            .into_iter()
            .chain(ctor_type.get_own_implemented_interfaces(reg))
            .collect();
        for ancestor in interfaces {
            ancestors.push(self.serialize_type(reg, ast, ancestor));
        }
        ancestors
    }

    /// Inserts dummy pointers corresponding to all `PrimitiveType`s in the type pool.
    ///
    /// These types will never correspond to an actual `TypeProto`. Instead, all normal `Integer`
    /// offsets into the pool are offset by a number equivalent to the number of `PrimitiveType`
    /// enum elements.
    // port: JSTypeReconserializer#seedCachesWithAxiomaticTypes
    fn seed_caches_with_axiomatic_types(&mut self, registry: &JSTypeRegistry) {
        check_state!(self.seen_type_records.is_empty());

        // Load all the axiomatic records in the right order without any types.
        for axiomatic in OFFSET_TO_AXIOMATIC_COLOR.iter() {
            let index = self.seen_type_records.len() as i32;
            let record = SeenTypeRecord::new(axiomatic.get_id(), index);
            self.seen_type_records.insert(axiomatic.get_id(), record);
        }

        // Add JSTypes corresponding to axiomatic IDs.
        for (jstype_native, axiomatic) in jstype_native_to_axiomatic_color_map() {
            self.get_or_create_record(axiomatic.get_id(), registry.get_native_type(jstype_native));
        }

        check_state!(self.seen_type_records.len() == OFFSET_TO_AXIOMATIC_COLOR.len());
    }

    /// Checks that this instance is in a valid state.
    // port: JSTypeReconserializer#checkValidLinearTime
    fn check_valid_linear_time(&self) {
        if !self.serialization_mode.run_validation() {
            return;
        }

        let total_type_count = self.seen_type_records.len() as i32;
        for seen in self.seen_type_records.values() {
            let offset = seen.pointer;
            check_state!(offset >= 0);
            check_state!(
                offset <= total_type_count,
                "Found invalid pointer %s, out of a total of %s user-defined types",
                offset,
                total_type_count
            );
        }
    }

    /// Generates a "type-pool" representing all the types that this class has encountered through
    /// calls to `serialize_type`.
    ///
    /// After generation, no new types can be added, so subsequent calls to `serialize_type` can
    /// only be used to retrieve pointers to existing types in the type pool.
    // port: JSTypeReconserializer#generateTypePool
    pub fn generate_type_pool(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypePool {
        check_state!(self.state == State::COLLECTING_TYPES);
        self.check_valid_linear_time();

        let mut builder = TypePool::new_builder();

        if self.serialization_mode.include_debug_info() {
            let mut debug_info = TypePoolDebugInfo::new_builder();
            // Key by source ref to deduplicate the strings, which are pretty long.
            // (ImmutableSetMultimap#inverse().asMap())
            let mut inverse: IndexMap<NodeId, IndexSet<TypeId>> = IndexMap::<_, _>::default();
            for (r#type, locations) in self.invalidating_types.get_mismatch_locations() {
                for &location in locations {
                    inverse.entry(location).or_default().insert(*r#type);
                }
            }
            for (location, types) in inverse {
                let mut involved_colors: Vec<i32> = Vec::new();
                for t in types {
                    if t.is_union_type(reg) {
                        let message = t.to_string(reg, ast);
                        check_state!(!t.is_union_type(reg), "%s", message);
                    }
                    // Ensure all types are recorded before reconciliation.
                    let pointer = self.serialize_type(reg, ast, t);
                    if !involved_colors.contains(&pointer) {
                        involved_colors.push(pointer);
                    }
                }
                involved_colors.sort();
                debug_info = debug_info.add_mismatch(
                    TypePoolDebugInfoMismatch::new_builder()
                        .set_source_ref(location.get_location(ast))
                        .add_all_involved_color(involved_colors)
                        .build(),
                );
            }
            builder = builder.set_debug_info(debug_info.build());
        }

        self.state = State::GENERATING_POOL;

        let mut i = 0;
        while i < self.seen_type_records.len() {
            let (color_id, pointer, is_union) = {
                let seen = &self.seen_type_records[i];
                (seen.color_id, seen.pointer, seen.union_members.is_some())
            };
            i += 1;
            if standard_colors::AXIOMATIC_COLORS.contains_key(&color_id) {
                check_state!(
                    TypePointers::is_axiomatic(pointer),
                    "Missing .type for SeenTypeRecord %s",
                    format!("{:?}", self.record(pointer))
                );
                continue;
            }
            let type_proto = if is_union {
                self.reconcile_union_types(self.record(pointer))
            } else {
                self.reconcile_object_types(reg, ast, pointer)
            };
            builder = builder.add_type(type_proto);
        }

        for (&subtype, supertypes) in &self.disambiguate_edges {
            for &supertype in supertypes {
                builder = builder.add_disambiguation_edges(
                    SubtypingEdge::new_builder()
                        .set_subtype(subtype)
                        .set_supertype(supertype)
                        .build(),
                );
            }
        }

        self.state = State::FINISHED;
        self.check_valid_linear_time();
        builder.build()
    }

    /// Returns a map from `ObjectTypeProto#getUuid()` to the originating `JSType`s.
    ///
    /// Only intended to be used for debug logging.
    // port: JSTypeReconserializer#getColorIdToJSTypeMapForDebugging
    pub fn get_color_id_to_jstype_map_for_debugging(&self) -> IndexMap<String, Vec<TypeId>> {
        // note: returns JSType values instead of String values, even though all that's needed for
        // debugging are the strings, to avoid the memory overhead of calculating all string
        // representations at once
        let mut color_id_to_types: IndexMap<String, Vec<TypeId>> = IndexMap::<_, _>::default();
        for (&jstype, &record) in &self.type_to_record_cache {
            color_id_to_types
                .entry(self.record(record).color_id.to_string())
                .or_default()
                .push(jstype);
        }
        // ImmutableMultimap.Builder#orderKeysBy(naturalOrder())
        color_id_to_types.sort_keys();
        color_id_to_types
    }

    /// Rust-only: the record with the given pointer (its index in `seen_type_records`).
    fn record(&self, pointer: i32) -> &SeenTypeRecord {
        &self.seen_type_records[pointer as usize]
    }

    /// Rust-only: the record with the given pointer (its index in `seen_type_records`).
    fn record_mut(&mut self, pointer: i32) -> &mut SeenTypeRecord {
        &mut self.seen_type_records[pointer as usize]
    }

    /// Returns whether this is some assertion call that should be removed by optimizations when
    /// --remove_closure_asserts is enabled.
    // port: JSTypeReconserializer#isClosureAssert
    fn is_closure_assert(primitive: Option<ClosurePrimitive>) -> bool {
        let Some(primitive) = primitive else {
            return false;
        };

        match primitive {
            ClosurePrimitive::ASSERTS_TRUTHY | ClosurePrimitive::ASSERTS_MATCHES_RETURN => true,
            ClosurePrimitive::ASSERTS_FAIL =>
            // technically an assertion function, but not removed by ClosureCodeRemoval
            {
                false
            }
        }
    }
}
