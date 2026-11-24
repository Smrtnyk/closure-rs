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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/serialization/ColorPool.java.

//! Port of serialization/ColorPool.java.
use super::malformed_typed_ast_exception::{DebugParam, MalformedTypedAstException};
use super::string_pool::StringPool;
use super::type_pointers::{OFFSET_TO_AXIOMATIC_COLOR, TypePointers};
use super::types_proto::{TypePool, TypeProto, TypeProtoKindCase};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_base::tri::Tri;
use closure_rhino::jscomp_colors::color::Color;
use closure_rhino::jscomp_colors::color_id::ColorId;
use closure_rhino::jscomp_colors::color_registry::{self, ColorRegistry};
use closure_rhino::jscomp_colors::standard_colors;
use closure_rhino::{check_not_null, check_state};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

/// port: ColorPool
///
/// A set of `Color`s reconstructed from possibly many `TypePool` protos.
///
/// Protos representing the same Color are reconciled while the pool is built. Reconciliation
/// ensures that, within a pool, each ColorId corresponds to at most one Color.
///
/// It's possible for two Color objects with the same ID to have different definitions between
/// different ColorPools, but mixing multiple ColorPools is likely a design error.
pub struct ColorPool {
    id_to_color: IndexMap<ColorId, Color>,
    color_registry: Arc<ColorRegistry>,
    /// Non-empty for testing only. Normally empty to save on memory.
    shard_views: Vec<Arc<ShardView>>,
}

impl ColorPool {
    // port: ColorPool#<init>
    fn new(
        id_to_color: IndexMap<ColorId, Color>,
        color_registry: Arc<ColorRegistry>,
        shard_views: Vec<Arc<ShardView>>,
    ) -> Self {
        Self {
            id_to_color,
            color_registry,
            shard_views,
        }
    }

    // port: ColorPool#getColor
    pub fn get_color(&self, id: ColorId) -> Option<Color> {
        self.id_to_color.get(&id).cloned()
    }

    // port: ColorPool#getRegistry
    pub fn get_registry(&self) -> &Arc<ColorRegistry> {
        &self.color_registry
    }

    // port: ColorPool#getOnlyShardForTesting
    pub fn get_only_shard_for_testing(&self) -> &Arc<ShardView> {
        assert_eq!(self.shard_views.len(), 1);
        &self.shard_views[0]
    }

    // port: ColorPool#builder
    pub fn builder() -> Builder {
        Builder::new()
    }

    // port: ColorPool#fromOnlyShardForTesting
    pub fn from_only_shard_for_testing(
        type_pool: Arc<TypePool>,
        string_pool: Arc<StringPool>,
    ) -> Arc<ColorPool> {
        let mut builder = ColorPool::builder();
        builder.add_shard_and(type_pool, string_pool).for_testing();
        builder.build()
    }
}

/// The `TypePool.getDefaultInstance()` object: Java adds it as a shard by identity, so every
/// TypedAst without a type pool shares this one `Arc`.
static DEFAULT_TYPE_POOL: LazyLock<Arc<TypePool>> =
    LazyLock::new(|| Arc::new(TypePool::get_default_instance()));

/// Rust-only: Java's `TypePool.getDefaultInstance()` object identity.
pub fn default_type_pool() -> Arc<TypePool> {
    DEFAULT_TYPE_POOL.clone()
}

/// port: ColorPool.ShardView
///
/// A view of the pool based on one of the input shards.
pub struct ShardView {
    trimmed_offset_to_id: Vec<ColorId>,

    // Fields only present before/while the ColorPool is being built. Null afterwards.
    pools: Mutex<Option<(Arc<TypePool>, ShardStrings)>>,

    // Set once the complete pool is built.
    color_pool: OnceLock<Arc<ColorPool>>,
}

impl ShardView {
    // port: ColorPool.ShardView#<init>
    fn new(
        type_pool: Arc<TypePool>,
        string_pool: ShardStrings,
        trimmed_offset_to_id: Vec<ColorId>,
    ) -> Self {
        Self {
            trimmed_offset_to_id,
            pools: Mutex::new(Some((type_pool, string_pool))),
            color_pool: OnceLock::new(),
        }
    }

    // port: ColorPool.ShardView#getColor
    pub fn get_color(&self, pointer: i32) -> Color {
        let color_pool = self.color_pool.get();
        check_state!(color_pool.is_some(), "%s", "ShardView");
        color_pool.unwrap().get_color(self.get_id(pointer)).unwrap()
    }

    // port: ColorPool.ShardView#getId
    fn get_id(&self, untrimmed_offset: i32) -> ColorId {
        if TypePointers::is_axiomatic(untrimmed_offset) {
            OFFSET_TO_AXIOMATIC_COLOR[untrimmed_offset as usize].get_id()
        } else {
            self.trimmed_offset_to_id[TypePointers::trim_offset(untrimmed_offset) as usize]
        }
    }

    // port: ColorPool.ShardView#updateStateAfterColorPoolIsBuilt
    fn update_state_after_color_pool_is_built(&self, color_pool: Arc<ColorPool>) {
        let _ = self.color_pool.set(color_pool);
        *self.pools.lock().unwrap() = None;
    }

    fn pools(&self) -> (Arc<TypePool>, ShardStrings) {
        self.pools.lock().unwrap().as_ref().unwrap().clone()
    }
}

#[derive(Clone)]
enum ShardStrings {
    Serialized(Arc<StringPool>),
    Interned(Arc<[JsString]>),
}

impl ShardStrings {
    fn get(&self, offset: i32) -> JsString {
        match self {
            Self::Serialized(pool) => pool.get(offset),
            Self::Interned(names) => names[offset as usize].clone(),
        }
    }
}

struct ShardInputs {
    index: usize,
    view: Arc<ShardView>,
    types: Arc<TypePool>,
    strings: ShardStrings,
}

/// Borrow the original definitions for the duration of reconciliation. Both row order and
/// the order of contributing shards are significant for colors and diagnostics.
#[derive(Clone, Copy)]
struct Definition<'a> {
    shard: &'a ShardInputs,
    proto: &'a TypeProto,
}

/// Most colors are defined in one shard. Keep the first definition in the table itself.
enum DefinitionRow<'a> {
    One(Definition<'a>),
    Several(Vec<Definition<'a>>),
}

impl<'a> DefinitionRow<'a> {
    fn push(&mut self, definition: Definition<'a>) {
        match self {
            Self::One(first) => *self = Self::Several(vec![*first, definition]),
            Self::Several(definitions) => definitions.push(definition),
        }
    }

    fn as_slice(&self) -> &[Definition<'a>] {
        match self {
            Self::One(first) => std::slice::from_ref(first),
            Self::Several(definitions) => definitions,
        }
    }
}

type Definitions<'a> = IndexMap<ColorId, DefinitionRow<'a>>;

/// port: ColorPool.Builder
///
/// Collects `TypePool`s and other data into `ShardView`s and then reconciles them into a single
/// `ColorPool`.
pub struct Builder {
    /// LinkedIdentityHashMap<TypePool, ShardView>: keyed by the TypePool's `Arc` address.
    proto_to_shard: IndexMap<usize, (Arc<TypePool>, Arc<ShardView>)>,
    id_to_color: IndexMap<ColorId, Color>,
    registry: color_registry::Builder,
    // Java uses a nonempty definition table as the guard against building twice.
    has_proto_rows: bool,
    for_testing: bool,

    reconcilation_debug_stack: Vec<ColorId>,
}

static PENDING_COLOR: LazyLock<Color> = LazyLock::new(|| {
    Color::single_builder()
        .set_id(ColorId::from_unsigned(0xDEADBEEF))
        .build()
});

impl Builder {
    // port: ColorPool.Builder#<init>
    fn new() -> Self {
        let mut id_to_color = IndexMap::<_, _>::default();
        for (id, color) in standard_colors::AXIOMATIC_COLORS.iter() {
            id_to_color.insert(*id, color.clone());
        }
        Self {
            proto_to_shard: IndexMap::<_, _>::default(),
            id_to_color,
            registry: ColorRegistry::builder(),
            has_proto_rows: false,
            for_testing: false,
            reconcilation_debug_stack: Vec::new(),
        }
    }

    // port: ColorPool.Builder#addShardAnd
    pub fn add_shard_and(
        &mut self,
        type_pool: Arc<TypePool>,
        string_pool: Arc<StringPool>,
    ) -> &mut Self {
        self.add_shard(type_pool, string_pool);
        self
    }

    // port: ColorPool.Builder#forTesting
    fn for_testing(&mut self) -> &mut Self {
        self.for_testing = true;
        self
    }

    // port: ColorPool.Builder#addShard
    pub fn add_shard(
        &mut self,
        type_pool: Arc<TypePool>,
        string_pool: Arc<StringPool>,
    ) -> Arc<ShardView> {
        self.register_shard(
            type_pool,
            ShardStrings::Serialized(string_pool),
            create_trimmed_offset_to_id,
        )
    }

    /// An in-memory compilation has already assigned these color IDs and string pointers.
    /// Deserialized shards still derive and validate their IDs from the serialized definitions.
    pub(super) fn add_optimization_shard(
        &mut self,
        type_pool: TypePool,
        ids: Vec<ColorId>,
        strings: Vec<JsString>,
    ) -> Arc<ShardView> {
        debug_assert_eq!(ids, create_trimmed_offset_to_id(&type_pool));
        self.register_shard(
            Arc::new(type_pool),
            ShardStrings::Interned(strings.into()),
            |_| ids,
        )
    }

    fn register_shard(
        &mut self,
        type_pool: Arc<TypePool>,
        string_pool: ShardStrings,
        color_ids: impl FnOnce(&TypePool) -> Vec<ColorId>,
    ) -> Arc<ShardView> {
        check_state!(!self.has_proto_rows, "build has already been called");

        let key = Arc::as_ptr(&type_pool) as usize;
        if let Some((_, existing)) = self.proto_to_shard.get(&key) {
            check_state!(
                Arc::ptr_eq(&type_pool, &DEFAULT_TYPE_POOL),
                "%s",
                format!("{type_pool:?}")
            );
            return existing.clone();
        }

        let trimmed_offset_to_id = color_ids(&type_pool);
        let shard = Arc::new(ShardView::new(
            type_pool.clone(),
            string_pool,
            trimmed_offset_to_id,
        ));
        self.proto_to_shard
            .insert(key, (type_pool.clone(), shard.clone()));

        if type_pool.has_debug_info() {
            for m in type_pool.get_debug_info().get_mismatch_list() {
                for &pointer in m.get_involved_color_list() {
                    self.registry.add_mismatch_location(
                        shard.get_id(pointer),
                        m.get_source_ref().to_string(),
                    );
                }
            }
        }

        shard
    }

    // port: ColorPool.Builder#build
    pub fn build(&mut self) -> Arc<ColorPool> {
        check_state!(!self.has_proto_rows, "build has already been called");

        let shards: Vec<_> = self
            .proto_to_shard
            .values()
            .enumerate()
            .map(|(index, (_, view))| {
                let (types, strings) = view.pools();
                ShardInputs {
                    index,
                    view: view.clone(),
                    types,
                    strings,
                }
            })
            .collect();
        let total_protos: usize = shards
            .iter()
            .map(|s| s.types.get_type_count() as usize)
            .sum();
        let mut rows = Definitions::with_capacity_and_hasher(total_protos, Default::default());
        for shard in &shards {
            for (i, proto) in shard.types.get_type_list().iter().enumerate() {
                let id = shard.view.trimmed_offset_to_id[i];
                let definition = Definition { shard, proto };
                rows.entry(id)
                    .and_modify(|row| row.push(definition))
                    .or_insert(DefinitionRow::One(definition));
                self.has_proto_rows = true;
            }
        }
        self.id_to_color.reserve(rows.len());

        for id in standard_colors::AXIOMATIC_COLORS.keys() {
            MalformedTypedAstException::check_well_formed_with_param(
                !rows.contains_key(id),
                "Found serialized definiton for axiomatic color",
                id,
            );
        }

        for &id in rows.keys() {
            if !self.id_to_color.contains_key(&id) {
                self.lookup_or_reconcile_color(id, &rows);
            }
        }

        for &color_id in color_registry::REQUIRED_IDS.iter() {
            let color = self
                .id_to_color
                .entry(color_id)
                .or_insert_with(|| Color::single_builder().set_id(color_id).build())
                .clone();
            self.registry.set_native_color(color);
        }

        for inputs in &shards {
            let shard = &inputs.view;
            for edge in inputs.types.get_disambiguation_edges_list() {
                let subtype = self.id_to_color
                    [&shard.get_id(validate_pointer(edge.get_subtype(), shard))]
                    .clone();
                let supertype = self.id_to_color
                    [&shard.get_id(validate_pointer(edge.get_supertype(), shard))]
                    .clone();
                self.registry.add_disambiguation_edge(subtype, supertype);
            }
        }

        let mut shard_views_for_testing = Vec::new();
        if self.for_testing {
            for (_, shard) in self.proto_to_shard.values() {
                shard_views_for_testing.push(shard.clone());
            }
        }

        let color_pool = Arc::new(ColorPool::new(
            std::mem::take(&mut self.id_to_color),
            Arc::new(self.registry.build()),
            shard_views_for_testing,
        ));

        for (_, shard) in self.proto_to_shard.values() {
            shard.update_state_after_color_pool_is_built(color_pool.clone());
        }
        color_pool
    }

    // port: ColorPool.Builder#lookupOrReconcileColor
    fn lookup_or_reconcile_color(&mut self, id: ColorId, rows: &Definitions) -> Color {
        self.reconcilation_debug_stack.push(id);
        if let Some(existing) = self.id_to_color.get(&id) {
            if existing.ptr_eq(&PENDING_COLOR) {
                let rows: Vec<String> = self
                    .reconcilation_debug_stack
                    .iter()
                    .map(|id| {
                        let row = rows.get(id).map(DefinitionRow::as_slice);
                        let entries: Vec<String> = row
                            .into_iter()
                            .flatten()
                            .map(|entry| {
                                format!("ShardView#{}={:?}", entry.shard.index, entry.proto)
                            })
                            .collect();
                        format!("{{{}}}", entries.join(", "))
                    })
                    .collect();
                MalformedTypedAstException::new(format!(
                    "Cyclic Color structure detected: [{}]",
                    rows.join(", ")
                ))
                .throw();
            }
            let existing = existing.clone();
            self.reconcilation_debug_stack.pop();
            return existing;
        }
        self.id_to_color.insert(id, PENDING_COLOR.clone());

        let row = check_not_null!(rows.get(&id), "%s", id).as_slice();
        let sample = row[0].proto;
        let result = match sample.get_kind_case() {
            TypeProtoKindCase::OBJECT => self.reconcile_object_protos(id, row, rows),
            TypeProtoKindCase::UNION => self.reconcile_union_protos(id, row, rows),
            _ => panic!("{sample:?}"),
        };

        self.id_to_color.insert(id, result.clone());
        self.reconcilation_debug_stack.pop();
        result
    }

    // port: ColorPool.Builder#reconcileObjectProtos
    fn reconcile_object_protos(
        &mut self,
        id: ColorId,
        row: &[Definition<'_>],
        rows: &Definitions,
    ) -> Color {
        let (inst_cap, proto_cap, prop_cap) = if row.len() == 1 && row[0].proto.has_object() {
            let obj = row[0].proto.get_object();
            (
                obj.get_instance_type_list().len(),
                obj.get_prototype_list().len(),
                obj.get_own_property_count() as usize,
            )
        } else {
            (0, 0, 0)
        };
        let mut instance_colors: IndexSet<Color> = if inst_cap > 0 {
            IndexSet::with_capacity_and_hasher(inst_cap, Default::default())
        } else {
            IndexSet::default()
        };
        let mut prototypes: IndexSet<Color> = if proto_cap > 0 {
            IndexSet::with_capacity_and_hasher(proto_cap, Default::default())
        } else {
            IndexSet::default()
        };
        let mut own_properties: IndexSet<JsString> = if prop_cap > 0 {
            IndexSet::with_capacity_and_hasher(prop_cap, Default::default())
        } else {
            IndexSet::default()
        };
        let mut is_closure_assert = Tri::UNKNOWN;
        let mut is_constructor = false;
        let mut is_invalidating = false;
        let mut properties_keep_original_name = false;

        for Definition { shard, proto } in row {
            check_state!(proto.has_object());
            let obj_proto = proto.get_object();

            let instance_type_ids = obj_proto.get_instance_type_list();
            for &instance_type_pointer in instance_type_ids {
                let color =
                    self.lookup_or_reconcile_color(shard.view.get_id(instance_type_pointer), rows);
                instance_colors.insert(color);
            }

            let is_closure_assert_bool = obj_proto.get_closure_assert();
            MalformedTypedAstException::check_well_formed_with_param(
                is_closure_assert.to_boolean(is_closure_assert_bool) == is_closure_assert_bool,
                "Inconsistent values for closure_assert",
                &DebugParam(&obj_proto),
            );
            is_closure_assert = Tri::for_boolean(is_closure_assert_bool);

            is_constructor |= obj_proto.get_marked_constructor();
            is_invalidating |= obj_proto.get_is_invalidating();
            properties_keep_original_name |= obj_proto.get_properties_keep_original_name();

            let prototype_ids = obj_proto.get_prototype_list();
            for &prototype_pointer in prototype_ids {
                let color =
                    self.lookup_or_reconcile_color(shard.view.get_id(prototype_pointer), rows);
                prototypes.insert(color);
            }
            for i in 0..obj_proto.get_own_property_count() {
                own_properties.insert(shard.strings.get(obj_proto.get_own_property(i)));
            }
        }

        Color::single_builder()
            .set_id(id)
            .set_instance_colors(instance_colors)
            .set_prototypes(prototypes)
            .set_own_properties(own_properties)
            .set_closure_assert(is_closure_assert.to_boolean(false))
            .set_constructor(is_constructor)
            .set_invalidating(is_invalidating)
            .set_properties_keep_original_name(properties_keep_original_name)
            .build()
    }

    // port: ColorPool.Builder#reconcileUnionProtos
    fn reconcile_union_protos(
        &mut self,
        id: ColorId,
        row: &[Definition<'_>],
        rows: &Definitions,
    ) -> Color {
        let union_cap = if row.len() == 1 && row[0].proto.has_union() {
            row[0].proto.get_union().get_union_member_list().len()
        } else {
            0
        };
        let mut union: IndexSet<Color> = if union_cap > 0 {
            IndexSet::with_capacity_and_hasher(union_cap, Default::default())
        } else {
            IndexSet::default()
        };
        for Definition { shard, proto } in row {
            check_state!(proto.has_union(), "%s", format!("{proto:?}"));
            let union_member_list = proto.get_union().get_union_member_list();
            for &member_pointer in union_member_list {
                let member_id = shard.view.get_id(member_pointer);
                let member = self.lookup_or_reconcile_color(member_id, rows);
                MalformedTypedAstException::check_well_formed_with_param(
                    !member.is_union(),
                    "Reconciling union with non-union",
                    &DebugParam(&proto),
                );
                union.insert(member);
            }
        }

        let result = Color::create_union_owned_flat(union);
        check_state!(id == result.get_id(), "%s == %s", id, result);
        result
    }
}

// port: ColorPool#createTrimmedOffsetToId
#[allow(clippy::needless_range_loop)] // Java index loops over ids
fn create_trimmed_offset_to_id(type_pool: &TypePool) -> Vec<ColorId> {
    let mut ids: Vec<Option<ColorId>> = vec![None; type_pool.get_type_count() as usize];

    for i in 0..ids.len() {
        let proto = type_pool.get_type(i as i32);
        match proto.get_kind_case() {
            TypeProtoKindCase::OBJECT => {
                ids[i] = Some(ColorId::from_byte_string(proto.get_object().get_uuid()));
            }
            TypeProtoKindCase::UNION => {
                // Defer generating union IDs until we have the IDs of all the element types.
            }
            _ => MalformedTypedAstException::new(format!("{proto:?}")).throw(),
        }
    }

    for i in 0..ids.len() {
        let proto = type_pool.get_type(i as i32);
        match proto.get_kind_case() {
            TypeProtoKindCase::OBJECT => {}
            TypeProtoKindCase::UNION => ids[i] = Some(create_union_color_id(proto, &ids)),
            _ => panic!("{proto:?}"),
        }
    }

    let mut seen_ids: IndexSet<ColorId> = IndexSet::<_>::default();
    for (i, id) in ids.iter().enumerate() {
        let proto = type_pool.get_type(i as i32);
        MalformedTypedAstException::check_well_formed_with_param(
            seen_ids.insert(id.unwrap()),
            "Duplicate ID in single shard",
            &DebugParam(&proto),
        );
    }

    ids.into_iter().map(Option::unwrap).collect()
}

// port: ColorPool#createUnionColorId
fn create_union_color_id(proto: &TypeProto, all_object_ids: &[Option<ColorId>]) -> ColorId {
    MalformedTypedAstException::check_well_formed_with_param(
        proto.get_union().get_union_member_count() > 1,
        "Union has too few members",
        &DebugParam(&proto),
    );
    let union_member_list = proto.get_union().get_union_member_list();
    ColorId::union_from_iter(union_member_list.iter().map(|&member_pointer| {
        let member_id = if TypePointers::is_axiomatic(member_pointer) {
            Some(OFFSET_TO_AXIOMATIC_COLOR[member_pointer as usize].get_id())
        } else {
            all_object_ids[TypePointers::trim_offset(member_pointer) as usize]
        };
        MalformedTypedAstException::check_well_formed_with_param(
            member_id.is_some(),
            "Union member not found",
            &DebugParam(&proto),
        );
        member_id.unwrap()
    }))
}

// port: ColorPool#validatePointer
fn validate_pointer(offset: i32, shard: &ShardView) -> i32 {
    MalformedTypedAstException::check_well_formed_with_param(
        0 <= offset
            && offset < TypePointers::untrim_offset(shard.trimmed_offset_to_id.len() as i32),
        "Pointer offset outside of shard",
        &offset,
    );
    offset
}
