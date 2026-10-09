/*
 * Copyright 2026 The closure-rs Authors.
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
 * Copyright 2008 The Closure Compiler Authors.
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
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/disambiguate/AmbiguateProperties.java,
//   src/com/google/javascript/jscomp/disambiguate/DisambiguateProperties.java,
//   test/com/google/javascript/jscomp/disambiguate/ColorFindPropertyReferencesTest.java,
//   test/com/google/javascript/jscomp/disambiguate/ColorGraphBuilderTest.java,
//   test/com/google/javascript/jscomp/disambiguate/DisambiguatePropertiesTest.java.

//! Replay adapters for the property (dis)ambiguation passes: the
//! constructors the AmbiguatePropertiesTest / DisambiguatePropertiesTest descriptors call, the
//! `renamingMap` field UnitRecorder reads (`pass.ambiguatedPropertyMap`), and the warnings guards
//! copied into the replay helpers (`DisambiguatePropertiesTest_Helpers$SilenceNoiseGuard`,
//! `ColorFindPropertyReferencesTest_Helpers$SilenceChecksWarningsGuard`), the holders
//! `oracle/replay/helpers/.../disambiguate/ColorFindPropertyReferencesTest_Helpers.java` and
//! `ColorGraphBuilderTest_Helpers.java`, and the UnitRecorder field dumps of the colors and
//! disambiguation objects their `testFieldsAfter` fields reach.
use crate::{
    replay::{
        options_values::OptionValue,
        registry::Entry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    check_level::CheckLevel,
    colors::{Color, color_id::ColorId, color_registry::ColorRegistry},
    compiler::Compiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    disambiguate::{
        ambiguate_properties::AmbiguateProperties,
        color_find_property_references::ColorFindPropertyReferences,
        color_graph_node::{
            ColorGraphNode, ColorGraphNodeId, DisambiguateArena, PropertyClusteringId,
        },
        color_graph_node_factory::{ColorGraphNodeFactory, ColorGraphNodeFactoryMethods},
        disambiguate_properties::{DisambiguateProperties, PROPERTY_INVALIDATION},
        invalidation::Invalidation,
    },
    js_error::JSError,
    node_traversal::{AbstractPostOrderCallback, NodeTraversal},
    warnings_guard::{Priority, WarningsGuard},
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};
use std::{any::Any, cell::RefCell, fmt, rc::Rc, sync::Arc};

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.disambiguate.AmbiguateProperties#makePassForTesting(com.google.javascript.jscomp.AbstractCompiler,java.util.Set,java.util.Set,java.util.Set)" => {
            make_pass_for_testing
        }
        "com.google.javascript.jscomp.disambiguate.DisambiguateProperties#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.common.collect.ImmutableSet)" => {
            disambiguate_properties
        }
        "com.google.javascript.jscomp.disambiguate.ColorFindPropertyReferencesTest_Helpers#<init>(com.google.javascript.jscomp.Compiler,boolean,com.google.common.collect.ImmutableSet)" => {
            color_find_property_references_test_helpers
        }
        "com.google.javascript.jscomp.disambiguate.ColorFindPropertyReferencesTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            color_find_property_references_test_get_processor
        }
        "com.google.javascript.jscomp.disambiguate.ColorGraphBuilderTest_Helpers#<init>(com.google.javascript.jscomp.Compiler)" => {
            color_graph_builder_test_helpers
        }
        "com.google.javascript.jscomp.disambiguate.ColorGraphBuilderTest_Helpers#prepareProcessor()" => {
            color_graph_builder_test_prepare_processor
        }
        "com.google.javascript.jscomp.disambiguate.ColorGraphBuilderTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            color_graph_builder_test_get_processor
        }
        _ => return None,
    })
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
// port: ReplayDsl#invoke (receiver cast)
fn compiler(args: &[DslValue]) -> Result<CompilerHandle, Throwable> {
    if let Some(DslValue::Compiler(c)) = args.first() {
        Ok(c.clone())
    } else {
        Err(bad())
    }
}

// port: AmbiguateProperties#makePassForTesting (replay entry)
fn make_pass_for_testing(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [
        _,
        reserved_first_characters,
        reserved_non_first_characters,
        extern_properties,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    // ImmutableSet.Builder#addAll(null) throws NullPointerException.
    let Some(extern_properties) = Option::<IndexSet<JsString>>::decode_value(extern_properties)?
    else {
        return Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        });
    };
    let pass = AmbiguateProperties::make_pass_for_testing(
        &c.borrow(),
        IndexSet::<u16>::decode_value(reserved_first_characters)?,
        IndexSet::<u16>::decode_value(reserved_non_first_characters)?,
        &extern_properties,
    );
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeAmbiguateProperties(pass),
    ))))
}

// port: DisambiguateProperties#DisambiguateProperties (replay entry)
fn disambiguate_properties(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [_, properties_that_must_disambiguate] = args.as_slice() else {
        return Err(bad());
    };
    let pass = DisambiguateProperties::new(
        &c.borrow(),
        IndexSet::<JsString>::decode_value(properties_that_must_disambiguate)?,
    );
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

struct NativeAmbiguateProperties(AmbiguateProperties);
impl NativeObject for NativeAmbiguateProperties {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.disambiguate.AmbiguateProperties"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: AmbiguateProperties#getRenamingMap
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method == "getRenamingMap" {
            Ok(renaming_map_value(Some(self.0.get_renaming_map())))
        } else {
            Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )))
        }
    }
    // port: UnitRecorder#fields (AmbiguateProperties#renamingMap, the field RESULT_PRODUCERS reads)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([(
            "renamingMap".into(),
            renaming_map_value(self.0.replay_renaming_map()),
        )]))
    }
    // port: AmbiguateProperties#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.0.process(compiler, externs, root);
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
// port: UnitRecorder#value (LinkedHashMap<String, String>)
fn renaming_map_value(map: Option<&IndexMap<JsString, JsString>>) -> DslValue {
    match map {
        None => DslValue::Null,
        Some(map) => DslValue::Map(
            map.iter()
                .map(|(k, v)| (DslValue::String(k.clone()), DslValue::String(v.clone())))
                .collect(),
        ),
    }
}

/// DisambiguatePropertiesTest$SilenceNoiseGuard (DisambiguatePropertiesTest.java lines
/// 1302-1321), copied verbatim into `DisambiguatePropertiesTest_Helpers`.
#[derive(Debug, Default)]
pub struct SilenceNoiseGuard;
impl SilenceNoiseGuard {
    pub const CLASS: &'static str =
        "com.google.javascript.jscomp.DisambiguatePropertiesTest_Helpers$SilenceNoiseGuard";
}
// port: DisambiguatePropertiesTest$SilenceNoiseGuard#RELEVANT_DIAGNOSTICS
fn relevant_diagnostics() -> [&'static DiagnosticType; 1] {
    [&PROPERTY_INVALIDATION]
}
impl WarningsGuard for SilenceNoiseGuard {
    // port: DisambiguatePropertiesTest$SilenceNoiseGuard#getPriority
    fn get_priority(&self) -> i32 {
        Priority::MAX.get_value()
    }
    // port: DisambiguatePropertiesTest$SilenceNoiseGuard#level
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        if error.description().contains("Parse") {
            None
        } else if relevant_diagnostics().contains(&error.get_type()) {
            None
        } else {
            Some(CheckLevel::OFF)
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for SilenceNoiseGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{:x}", Self::CLASS, self as *const Self as usize)
    }
}

/// The anonymous `new WarningsGuard() {...}` of ColorFindPropertyReferencesTest's
/// SILENCE_CHECKS_WARNINGS_GUARD, named `SilenceChecksWarningsGuard` in the replay helper.
#[derive(Debug, Default)]
pub struct SilenceChecksWarningsGuard;
impl SilenceChecksWarningsGuard {
    pub const CLASS: &'static str = "com.google.javascript.jscomp.disambiguate.ColorFindPropertyReferencesTest_Helpers$SilenceChecksWarningsGuard";
}
impl WarningsGuard for SilenceChecksWarningsGuard {
    // port: ColorFindPropertyReferencesTest_Helpers$SilenceChecksWarningsGuard#getPriority
    fn get_priority(&self) -> i32 {
        Priority::MAX.get_value()
    }
    // port: ColorFindPropertyReferencesTest_Helpers$SilenceChecksWarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        if error.description().contains("Parse") {
            None
        } else {
            Some(CheckLevel::OFF)
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for SilenceChecksWarningsGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{:x}", Self::CLASS, self as *const Self as usize)
    }
}

// ---------------------------------------------------------------------------------------------
// UnitRecorder field dumps (referenceable mode) of the colors and disambiguation objects.
// ---------------------------------------------------------------------------------------------

const AUTO_VALUE_COLOR: &str = "com.google.javascript.jscomp.colors.AutoValue_Color";
const COLOR_ID: &str = "com.google.javascript.jscomp.colors.ColorId";
const COLOR_REGISTRY: &str = "com.google.javascript.jscomp.colors.ColorRegistry";
const COLOR_GRAPH_NODE: &str = "com.google.javascript.jscomp.disambiguate.ColorGraphNode";
const PROP_ASSOCIATION: &str =
    "com.google.javascript.jscomp.disambiguate.ColorGraphNode$PropAssociation";
const COLOR_GRAPH_NODE_FACTORY: &str =
    "com.google.javascript.jscomp.disambiguate.ColorGraphNodeFactory";
const PROPERTY_CLUSTERING: &str = "com.google.javascript.jscomp.disambiguate.PropertyClustering";
const INVALIDATION: &str = "com.google.javascript.jscomp.disambiguate.Invalidation";
const COLOR_FIND_PROPERTY_REFERENCES: &str =
    "com.google.javascript.jscomp.disambiguate.ColorFindPropertyReferences";
const HASH_BI_MAP: &str = "com.google.common.collect.HashBiMap";
const CFPR_HOLDER: &str =
    "com.google.javascript.jscomp.disambiguate.ColorFindPropertyReferencesTest_Helpers";
const CFPR_STUB: &str = "com.google.javascript.jscomp.disambiguate.ColorFindPropertyReferencesTest_Helpers$StubColorGraphNodeFactory";
const CFPR_LAMBDA: &str =
    "com.google.javascript.jscomp.disambiguate.ColorFindPropertyReferencesTest_Helpers$$Lambda";
const CGBT_HOLDER: &str = "com.google.javascript.jscomp.disambiguate.ColorGraphBuilderTest_Helpers";
const CGBT_LAMBDA: &str =
    "com.google.javascript.jscomp.disambiguate.ColorGraphBuilderTest_Helpers$$Lambda";

/// The object web a holder's disambiguation objects live in: the arena of its ColorGraphNodes
/// and PropertyClusterings, and the runtime class (`Node$StringNode`, ...) of every use-site
/// Node, taken when the processor finished (the dumps run without the compiler's AST).
#[derive(Default)]
struct Web {
    arena: RefCell<DisambiguateArena>,
    node_classes: RefCell<IndexMap<NodeId, &'static str>>,
}

// port: UnitRecorder#value (a reachable object)
fn native(o: impl NativeObject + 'static) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(o)))
}
// port: UnitRecorder#value (a guava HashBiMap, compared unordered)
fn hash_bi_map(entries: Vec<(DslValue, DslValue)>) -> DslValue {
    DslValue::Typed {
        class: HASH_BI_MAP.into(),
        value: Box::new(DslValue::Map(entries)),
    }
}
// port: UnitRecorder#value (@Nullable Color)
fn color_value(color: Option<&Color>) -> DslValue {
    match color {
        None => DslValue::Null,
        Some(color) => native(ColorDump(color.clone())),
    }
}
// port: UnitRecorder#value (ImmutableSet<Color>)
fn color_set(colors: &IndexSet<Color>) -> DslValue {
    DslValue::Set(colors.iter().map(|c| color_value(Some(c))).collect())
}
// port: UnitRecorder#value (ColorId)
fn color_id_value(id: ColorId) -> DslValue {
    native(ColorIdDump(id))
}
// port: UnitRecorder#value (ColorGraphNode)
fn graph_node_value(web: &Rc<Web>, id: ColorGraphNodeId) -> DslValue {
    native(ColorGraphNodeDump {
        web: web.clone(),
        id,
    })
}
// port: UnitRecorder#value (@Nullable ColorGraphNode)
fn optional_graph_node_value(web: &Rc<Web>, id: Option<ColorGraphNodeId>) -> DslValue {
    id.map_or(DslValue::Null, |id| graph_node_value(web, id))
}
// port: UnitRecorder#value (PropertyClustering)
fn property_clustering_value(web: &Rc<Web>, id: PropertyClusteringId) -> DslValue {
    native(PropertyClusteringDump {
        web: web.clone(),
        id,
    })
}
// port: UnitRecorder#value (LinkedHashMap<Color, ColorGraphNode> typeIndex)
fn type_index_value(web: &Rc<Web>, type_index: &IndexMap<Color, ColorGraphNodeId>) -> DslValue {
    DslValue::Map(
        type_index
            .iter()
            .map(|(color, node)| (color_value(Some(color)), graph_node_value(web, *node)))
            .collect(),
    )
}
// port: UnitRecorder#value (ColorRegistry)
fn color_registry_value(registry: &Arc<ColorRegistry>) -> DslValue {
    native(ColorRegistryDump(registry.clone()))
}

/// `AutoValue_Color` (fields of `$AutoValue_Color` plus the `@Memoized` `subtractNullOrVoid`).
struct ColorDump(Color);
impl NativeObject for ColorDump {
    fn class_name(&self) -> &str {
        AUTO_VALUE_COLOR
    }
    // port: UnitRecorder#fields (AutoValue_Color)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let c = &self.0;
        Ok(IndexMap::<_, _>::from_iter([
            (
                "subtractNullOrVoid".to_string(),
                color_value(c.replay_memoized_subtract_null_or_void()),
            ),
            (
                "$AutoValue_Color.id".to_string(),
                color_id_value(c.get_id()),
            ),
            (
                "$AutoValue_Color.prototypes".to_string(),
                color_set(c.get_prototypes()),
            ),
            (
                "$AutoValue_Color.instanceColors".to_string(),
                color_set(c.get_instance_colors()),
            ),
            (
                "$AutoValue_Color.invalidating".to_string(),
                DslValue::Bool(c.is_invalidating()),
            ),
            (
                "$AutoValue_Color.propertiesKeepOriginalName".to_string(),
                DslValue::Bool(c.get_properties_keep_original_name()),
            ),
            (
                "$AutoValue_Color.constructor".to_string(),
                DslValue::Bool(c.is_constructor()),
            ),
            (
                "$AutoValue_Color.ownProperties".to_string(),
                DslValue::Set(
                    c.get_own_properties()
                        .iter()
                        .map(|p| DslValue::String(p.clone()))
                        .collect(),
                ),
            ),
            (
                "$AutoValue_Color.boxId".to_string(),
                c.get_box_id().map_or(DslValue::Null, color_id_value),
            ),
            (
                "$AutoValue_Color.closureAssert".to_string(),
                DslValue::Bool(c.is_closure_assert()),
            ),
            (
                "$AutoValue_Color.unionElements".to_string(),
                color_set(c.get_union_elements()),
            ),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `ColorId` (its one field `rightAligned`).
struct ColorIdDump(ColorId);
impl NativeObject for ColorIdDump {
    fn class_name(&self) -> &str {
        COLOR_ID
    }
    // port: UnitRecorder#fields (ColorId)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([(
            "rightAligned".to_string(),
            DslValue::Long(self.0.right_aligned()),
        )]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `ColorRegistry` (`nativeColors`, `colorToDisambiguationSupertypeGraph`, `mismatchLocations`).
struct ColorRegistryDump(Arc<ColorRegistry>);
impl NativeObject for ColorRegistryDump {
    fn class_name(&self) -> &str {
        COLOR_REGISTRY
    }
    // port: UnitRecorder#fields (ColorRegistry)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let (native_colors, supertype_graph) = self.0.replay_fields();
        let mismatch_locations = self.0.get_mismatch_locations_for_debugging();
        Ok(IndexMap::<_, _>::from_iter([
            (
                "nativeColors".to_string(),
                DslValue::Map(
                    native_colors
                        .iter()
                        .map(|(id, color)| (color_id_value(*id), color_value(Some(color))))
                        .collect(),
                ),
            ),
            (
                "colorToDisambiguationSupertypeGraph".to_string(),
                native(ImmutableSetMultimapDump {
                    empty: supertype_graph.values().all(IndexSet::is_empty),
                }),
            ),
            (
                "mismatchLocations".to_string(),
                native(ImmutableSetMultimapDump {
                    empty: mismatch_locations.values().all(IndexSet::is_empty),
                }),
            ),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `ImmutableSetMultimap.copyOf(SetMultimap)`: the shared `EmptyImmutableSetMultimap.INSTANCE`
/// when empty, else an `ImmutableSetMultimap`. Its private Guava fields are not modelled.
struct ImmutableSetMultimapDump {
    empty: bool,
}
impl NativeObject for ImmutableSetMultimapDump {
    fn class_name(&self) -> &str {
        if self.empty {
            "com.google.common.collect.EmptyImmutableSetMultimap"
        } else {
            "com.google.common.collect.ImmutableSetMultimap"
        }
    }
    // port: UnitRecorder#fields (ImmutableSetMultimap)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Err(Throwable::Unported(format!(
            "{} field dump",
            self.class_name()
        )))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A Java library object whose private fields no recorded dump reaches (`java.util.BitSet`,
/// `StandardUnionFind`): it is always written as `{"ref": class}` at these depths.
struct OpaqueDump(&'static str);
impl NativeObject for OpaqueDump {
    fn class_name(&self) -> &str {
        self.0
    }
    // port: UnitRecorder#fields (not modelled)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Err(Throwable::Unported(format!("{} field dump", self.0)))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `ColorGraphNode` (`color`, `associatedProps`, `index`, `subtypeIndices`).
struct ColorGraphNodeDump {
    web: Rc<Web>,
    id: ColorGraphNodeId,
}
impl NativeObject for ColorGraphNodeDump {
    fn class_name(&self) -> &str {
        COLOR_GRAPH_NODE
    }
    // port: UnitRecorder#fields (ColorGraphNode)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let arena = self.web.arena.borrow();
        Ok(IndexMap::<_, _>::from_iter([
            (
                "color".to_string(),
                color_value(Some(self.id.get_color(&arena))),
            ),
            (
                "associatedProps".to_string(),
                DslValue::Map(
                    self.id
                        .get_associated_props(&arena)
                        .iter()
                        .map(|(prop, association)| {
                            (
                                property_clustering_value(&self.web, *prop),
                                DslValue::Enum {
                                    class: PROP_ASSOCIATION.into(),
                                    name: association.name().into(),
                                },
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "index".to_string(),
                DslValue::Int(self.id.get_index(&arena)),
            ),
            (
                "subtypeIndices".to_string(),
                native(OpaqueDump("java.util.BitSet")),
            ),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `PropertyClustering` (`name`, `useSites`, `clusters`, `originalNameClusterRep`,
/// `lastInvalidation`).
struct PropertyClusteringDump {
    web: Rc<Web>,
    id: PropertyClusteringId,
}
impl NativeObject for PropertyClusteringDump {
    fn class_name(&self) -> &str {
        PROPERTY_CLUSTERING
    }
    // port: UnitRecorder#fields (PropertyClustering)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let arena = self.web.arena.borrow();
        let node_classes = self.web.node_classes.borrow();
        let mut use_sites = vec![];
        for (node, flat) in self.id.get_use_sites(&arena) {
            let class = node_classes.get(node).ok_or_else(|| {
                Throwable::HarnessError("use-site Node without a recorded runtime class".into())
            })?;
            use_sites.push((
                DslValue::Typed {
                    class: (*class).into(),
                    value: Box::new(DslValue::Node(*node)),
                },
                graph_node_value(&self.web, *flat),
            ));
        }
        let (original_name_cluster_rep, last_invalidation) = self.id.replay_fields(&arena);
        Ok(IndexMap::<_, _>::from_iter([
            (
                "name".to_string(),
                DslValue::String(self.id.get_name(&arena).clone()),
            ),
            ("useSites".to_string(), DslValue::Map(use_sites)),
            (
                "clusters".to_string(),
                native(OpaqueDump(
                    "com.google.javascript.jscomp.graph.StandardUnionFind",
                )),
            ),
            (
                "originalNameClusterRep".to_string(),
                optional_graph_node_value(&self.web, original_name_cluster_rep),
            ),
            (
                "lastInvalidation".to_string(),
                last_invalidation.map_or(DslValue::Null, |i| native(InvalidationDump(i))),
            ),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `Invalidation` or its subclass `Invalidation$WithReceiverType`.
struct InvalidationDump(Invalidation);
impl NativeObject for InvalidationDump {
    fn class_name(&self) -> &str {
        match self.0.replay_fields().1 {
            None => INVALIDATION,
            Some(_) => "com.google.javascript.jscomp.disambiguate.Invalidation$WithReceiverType",
        }
    }
    // port: UnitRecorder#fields (Invalidation)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let (reason, receiver_type) = self.0.replay_fields();
        let reason = DslValue::Enum {
            class: format!("{INVALIDATION}$Reason"),
            name: reason.into(),
        };
        Ok(match receiver_type {
            None => IndexMap::<_, _>::from_iter([("reason".to_string(), reason)]),
            Some(receiver_type) => IndexMap::<_, _>::from_iter([
                ("receiverType".to_string(), DslValue::Int(receiver_type)),
                ("Invalidation.reason".to_string(), reason),
            ]),
        })
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `ColorGraphNodeFactory` (`typeIndex`, `registry`).
struct ColorGraphNodeFactoryDump {
    web: Rc<Web>,
    factory: Rc<RefCell<ColorGraphNodeFactory>>,
}
impl NativeObject for ColorGraphNodeFactoryDump {
    fn class_name(&self) -> &str {
        COLOR_GRAPH_NODE_FACTORY
    }
    // port: UnitRecorder#fields (ColorGraphNodeFactory)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let factory = self.factory.borrow();
        let (type_index, registry) = factory.replay_fields();
        Ok(IndexMap::<_, _>::from_iter([
            (
                "typeIndex".to_string(),
                type_index_value(&self.web, type_index),
            ),
            ("registry".to_string(), color_registry_value(registry)),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------
// ColorFindPropertyReferencesTest_Helpers
// ---------------------------------------------------------------------------------------------

/// `ColorFindPropertyReferencesTest_Helpers.StubColorGraphNodeFactory` (helper lines copied from
/// ColorFindPropertyReferencesTest.java 632-653).
struct StubColorGraphNodeFactory {
    created: IndexSet<Option<Color>>,
    created_nodes: IndexMap<Option<Color>, ColorGraphNodeId>,
    /// The `ColorGraphNodeFactory` superclass part.
    base: ColorGraphNodeFactory,
}
impl StubColorGraphNodeFactory {
    // port: ColorFindPropertyReferencesTest_Helpers.StubColorGraphNodeFactory#StubColorGraphNodeFactory
    fn new(registry: Arc<ColorRegistry>) -> Self {
        Self {
            created: IndexSet::<_>::default(),
            created_nodes: IndexMap::<_, _>::default(),
            base: ColorGraphNodeFactory::new(IndexMap::<_, _>::default(), registry),
        }
    }
}
// port: ColorFindPropertyReferencesTest_Helpers#createColorGraphNode
fn create_color_graph_node(arena: &mut DisambiguateArena) -> ColorGraphNodeId {
    ColorGraphNode::create_for_testing(arena, -1)
}
impl ColorGraphNodeFactoryMethods for StubColorGraphNodeFactory {
    // port: ColorFindPropertyReferencesTest_Helpers.StubColorGraphNodeFactory#createNode
    fn create_node(
        &mut self,
        arena: &mut DisambiguateArena,
        color: Option<&Color>,
    ) -> ColorGraphNodeId {
        self.created.insert(color.cloned());
        *self
            .created_nodes
            .entry(color.cloned())
            .or_insert_with(|| create_color_graph_node(arena))
    }
    // port: ColorFindPropertyReferencesTest_Helpers.StubColorGraphNodeFactory#getAllKnownTypes
    fn get_all_known_types(&self) -> IndexSet<ColorGraphNodeId> {
        panic!("java.lang.UnsupportedOperationException")
    }
}

/// `StubColorGraphNodeFactory`'s dump (own fields, then the inherited ones prefixed with
/// `ColorGraphNodeFactory.`).
struct StubColorGraphNodeFactoryDump {
    web: Rc<Web>,
    stub: Rc<RefCell<StubColorGraphNodeFactory>>,
}
impl NativeObject for StubColorGraphNodeFactoryDump {
    fn class_name(&self) -> &str {
        CFPR_STUB
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == CFPR_STUB || class == COLOR_GRAPH_NODE_FACTORY
    }
    // port: UnitRecorder#fields (ColorFindPropertyReferencesTest_Helpers.StubColorGraphNodeFactory)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let stub = self.stub.borrow();
        let (type_index, registry) = stub.base.replay_fields();
        Ok(IndexMap::<_, _>::from_iter([
            (
                "created".to_string(),
                DslValue::Set(
                    stub.created
                        .iter()
                        .map(|c| color_value(c.as_ref()))
                        .collect(),
                ),
            ),
            (
                "createdNodes".to_string(),
                hash_bi_map(
                    stub.created_nodes
                        .iter()
                        .map(|(c, n)| (color_value(c.as_ref()), graph_node_value(&self.web, *n)))
                        .collect(),
                ),
            ),
            (
                "ColorGraphNodeFactory.typeIndex".to_string(),
                type_index_value(&self.web, type_index),
            ),
            (
                "ColorGraphNodeFactory.registry".to_string(),
                color_registry_value(registry),
            ),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The state of `ColorFindPropertyReferencesTest_Helpers` the processor lambda and the field
/// dumps share.
struct ColorFindPropertyReferencesTestState {
    compiler: CompilerHandle,
    /// Which `externsCallback` the constructor installed: setUp's no-op, or (when
    /// `stripExternsSourceInfo`) the `clearSourceFileRecursive` lambda.
    strip_externs_source_info: bool,
    property_reflector_names: IndexSet<JsString>,
    labeled_statement_map: IndexMap<JsString, Option<Color>>,
    web: Rc<Web>,
    /// `finder`: null until the processor ran, else its `propIndex`.
    finder: Option<Option<IndexMap<JsString, PropertyClusteringId>>>,
    flattener: Option<Rc<RefCell<StubColorGraphNodeFactory>>>,
}

struct ColorFindPropertyReferencesTestHelpers {
    state: Rc<RefCell<ColorFindPropertyReferencesTestState>>,
    processor: Rc<RefCell<ColorFindPropertyReferencesTestProcessor>>,
}

// port: ColorFindPropertyReferencesTest_Helpers#ColorFindPropertyReferencesTest_Helpers
fn color_find_property_references_test_helpers(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [_, strip_externs_source_info, property_reflector_names] = args.as_slice() else {
        return Err(bad());
    };
    let strip_externs_source_info = bool::decode_value(strip_externs_source_info)?;
    let property_reflector_names = IndexSet::<JsString>::decode_value(property_reflector_names)?;
    // this.compiler = compiler; this.externsCallback = ...; labeledStatementMap = HashBiMap.create();
    let state = Rc::new(RefCell::new(ColorFindPropertyReferencesTestState {
        compiler: c,
        strip_externs_source_info,
        property_reflector_names,
        labeled_statement_map: IndexMap::<_, _>::default(),
        web: Rc::new(Web::default()),
        finder: None,
        flattener: None,
    }));
    // this.processor = (e, s) -> {...};
    let processor = Rc::new(RefCell::new(ColorFindPropertyReferencesTestProcessor {
        state: state.clone(),
    }));
    Ok(native(ColorFindPropertyReferencesTestHelpers {
        state,
        processor,
    }))
}

// port: ColorFindPropertyReferencesTest_Helpers#getProcessor
fn color_find_property_references_test_get_processor(
    _ctx: &mut Ctx,
    mut args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(holder), DslValue::Compiler(c)] = args.as_mut_slice() else {
        return Err(bad());
    };
    let mut holder = holder.borrow_mut();
    let Some(holder) = holder
        .as_any_mut()
        .downcast_mut::<ColorFindPropertyReferencesTestHelpers>()
    else {
        return Err(bad());
    };
    // checkState(compiler == this.compiler);
    crate::throwable::check_state(Rc::ptr_eq(c, &holder.state.borrow().compiler), "")?;
    // return checkNotNull(this.processor);
    let processor: Rc<RefCell<dyn NativeObject>> = holder.processor.clone();
    Ok(DslValue::Native(processor))
}

impl NativeObject for ColorFindPropertyReferencesTestHelpers {
    fn class_name(&self) -> &str {
        CFPR_HOLDER
    }
    // port: ReplayValues#findField (the holder's instance fields, in declaration order)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let state = self.state.borrow();
        let web = &state.web;
        let processor: Rc<RefCell<dyn NativeObject>> = self.processor.clone();
        let flattener = |stub: &Rc<RefCell<StubColorGraphNodeFactory>>| {
            native(StubColorGraphNodeFactoryDump {
                web: web.clone(),
                stub: stub.clone(),
            })
        };
        Ok(IndexMap::<_, _>::from_iter([
            (
                "compiler".to_string(),
                DslValue::Compiler(state.compiler.clone()),
            ),
            (
                "externsCallback".to_string(),
                native(LambdaDump {
                    class: CFPR_LAMBDA,
                    captures: IndexMap::<_, _>::default(),
                }),
            ),
            ("processor".to_string(), DslValue::Native(processor)),
            (
                "labeledStatementMap".to_string(),
                hash_bi_map(
                    state
                        .labeled_statement_map
                        .iter()
                        .map(|(k, v)| (DslValue::String(k.clone()), color_value(v.as_ref())))
                        .collect(),
                ),
            ),
            (
                "finder".to_string(),
                match (&state.finder, &state.flattener) {
                    (Some(prop_index), Some(stub)) => native(ColorFindPropertyReferencesDump {
                        prop_index: prop_index.as_ref().map(|prop_index| {
                            DslValue::Map(
                                prop_index
                                    .iter()
                                    .map(|(name, prop)| {
                                        (
                                            DslValue::String(name.clone()),
                                            property_clustering_value(web, *prop),
                                        )
                                    })
                                    .collect(),
                            )
                        }),
                        factory: flattener(stub),
                        property_reflector_names: state.property_reflector_names.clone(),
                    }),
                    _ => DslValue::Null,
                },
            ),
            (
                "flattener".to_string(),
                state.flattener.as_ref().map_or(DslValue::Null, flattener),
            ),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `ColorFindPropertyReferences`'s dump (`propIndex`, `colorGraphNodeFactory`,
/// `isPropertyReflector`), taken from the finder the processor built.
struct ColorFindPropertyReferencesDump {
    prop_index: Option<DslValue>,
    factory: DslValue,
    property_reflector_names: IndexSet<JsString>,
}
impl NativeObject for ColorFindPropertyReferencesDump {
    fn class_name(&self) -> &str {
        COLOR_FIND_PROPERTY_REFERENCES
    }
    // port: UnitRecorder#fields (ColorFindPropertyReferences)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([
            (
                "propIndex".to_string(),
                self.prop_index.clone().unwrap_or(DslValue::Null),
            ),
            ("colorGraphNodeFactory".to_string(), self.factory.clone()),
            (
                "isPropertyReflector".to_string(),
                // (node) -> propertyReflectorNames.contains(node.getQualifiedName())
                native(LambdaDump {
                    class: CFPR_LAMBDA,
                    captures: IndexMap::<_, _>::from_iter([(
                        "arg$1".to_string(),
                        DslValue::Set(
                            self.property_reflector_names
                                .iter()
                                .map(|n| DslValue::String(n.clone()))
                                .collect(),
                        ),
                    )]),
                }),
            ),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A helper lambda written as `{"ref": class, "captures": {...}}`.
struct LambdaDump {
    class: &'static str,
    captures: IndexMap<String, DslValue>,
}
impl NativeObject for LambdaDump {
    fn class_name(&self) -> &str {
        self.class
    }
    // port: ReplayValues#findField (a lambda has no named fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: UnitRecorder#dump (lambda captures)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        Some(Ok(self.captures.clone()))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The helper's `processor` lambda `(e, s) -> {...}`.
struct ColorFindPropertyReferencesTestProcessor {
    state: Rc<RefCell<ColorFindPropertyReferencesTestState>>,
}

// port: ColorFindPropertyReferencesTest_Helpers#clearSourceFileRecursive
fn clear_source_file_recursive(compiler: &mut Compiler, root: NodeId) {
    root.set_static_source_file(compiler, None);
    let children = root.children(compiler).collect::<Vec<_>>();
    for child in children {
        clear_source_file_recursive(compiler, child);
    }
}

impl NativeObject for ColorFindPropertyReferencesTestProcessor {
    fn class_name(&self) -> &str {
        CFPR_LAMBDA
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == CFPR_LAMBDA || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: ReplayValues#findField (a lambda has no named fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: UnitRecorder#dump (lambda captures: arg$1 is the holder, arg$2 the reflector names)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        let state = self.state.borrow();
        Some(Ok(IndexMap::<_, _>::from_iter([(
            "arg$2".to_string(),
            DslValue::Set(
                state
                    .property_reflector_names
                    .iter()
                    .map(|n| DslValue::String(n.clone()))
                    .collect(),
            ),
        )])))
    }
    // port: ColorFindPropertyReferencesTest_Helpers#ColorFindPropertyReferencesTest_Helpers (the processor lambda)
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let mut state = self.state.borrow_mut();
        // this.externsCallback.accept(e);
        if state.strip_externs_source_info {
            let externs_script = compiler
                .get_root()
                .unwrap()
                .get_first_child(compiler)
                .unwrap()
                .get_last_child(compiler)
                .unwrap();
            let children = externs_script.children(compiler).collect::<Vec<_>>();
            for externs_node in children {
                clear_source_file_recursive(compiler, externs_node);
            }
        }
        // NodeTraversal.traverse(compiler, s, new LabelledStatementCollector());
        let mut failure = None;
        {
            let labeled_statement_map = &mut state.labeled_statement_map;
            NodeTraversal::traverse(
                compiler,
                root,
                &mut AbstractPostOrderCallback::new(
                    |t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>| {
                        if failure.is_none() {
                            failure = labelled_statement_collector_visit(
                                t,
                                n,
                                parent,
                                labeled_statement_map,
                            )
                            .err();
                        }
                    },
                ),
            );
        }
        if let Some(failure) = failure {
            return Err(failure);
        }
        // this.flattener = new StubColorGraphNodeFactory();
        let flattener = Rc::new(RefCell::new(StubColorGraphNodeFactory::new(
            compiler.get_color_registry().clone(),
        )));
        state.flattener = Some(flattener.clone());
        // this.finder = new ColorFindPropertyReferences(this.flattener,
        //     (node) -> propertyReflectorNames.contains(node.getQualifiedName()));
        let web = state.web.clone();
        let property_reflector_names = state.property_reflector_names.clone();
        let prop_index = {
            let mut flattener = flattener.borrow_mut();
            let mut arena = web.arena.borrow_mut();
            let mut finder = ColorFindPropertyReferences::new(
                &mut *flattener,
                &mut arena,
                Box::new(move |ast, node| {
                    node.get_qualified_name(ast)
                        .is_some_and(|name| property_reflector_names.contains(&name))
                }),
            );
            // NodeTraversal.traverse(this.compiler, e.getParent(), checkNotNull(this.finder));
            let parent = externs.get_parent(compiler).unwrap();
            NodeTraversal::traverse(compiler, parent, &mut finder);
            // The finder object stays alive in the helper's `finder` field; its `propIndex` is
            // what the dump reads.
            finder.get_property_index()
        };
        // The runtime classes of the use-site Nodes, for the dumps.
        {
            let arena = web.arena.borrow();
            let mut node_classes = web.node_classes.borrow_mut();
            for prop in prop_index.values() {
                for node in prop.get_use_sites(&arena).keys() {
                    node_classes.insert(*node, node.get_class(compiler));
                }
            }
        }
        state.finder = Some(Some(prop_index));
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// port: ColorFindPropertyReferencesTest_Helpers.LabelledStatementCollector#visit
fn labelled_statement_collector_visit(
    t: &mut NodeTraversal<'_>,
    n: NodeId,
    parent: Option<NodeId>,
    labeled_statement_map: &mut IndexMap<JsString, Option<Color>>,
) -> Result<(), Throwable> {
    if let Some(parent) = parent
        && parent.is_label(t)
        && !n.is_label_name(t)
    {
        // First child of a LABEL is a LABEL_NAME, n is the second child.
        let Some(label_name_node) = n.get_previous(t) else {
            return Err(Throwable::Exception {
                class: "java.lang.NullPointerException".into(),
                message: Some(n.to_string(t)),
            });
        };
        crate::throwable::check_state(
            label_name_node.is_label_name(t),
            &label_name_node.to_string(t),
        )?;
        let label_name = label_name_node.get_string(t);
        crate::throwable::assert_that(
            !labeled_statement_map.contains_key(&label_name),
            format!("Duplicate label name: {label_name}"),
        )?;
        crate::throwable::assert_that(
            n.get_token(t) == Token::EXPR_RESULT,
            format!("expected token EXPR_RESULT but was {}", n.get_token(t)),
        )?;
        let color = n.get_only_child(t).get_color(t);
        labeled_statement_map.insert(label_name, color);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// ColorGraphBuilderTest_Helpers
// ---------------------------------------------------------------------------------------------

struct ColorGraphBuilderTestHelpers {
    compiler: CompilerHandle,
    web: Rc<Web>,
    graph_node_factory: Rc<RefCell<ColorGraphNodeFactory>>,
    processor: Option<Rc<RefCell<ColorGraphBuilderTestProcessor>>>,
    label_to_id: Option<Rc<RefCell<IndexMap<JsString, ColorId>>>>,
}

// port: ColorGraphBuilderTest_Helpers#ColorGraphBuilderTest_Helpers
fn color_graph_builder_test_helpers(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let web = Rc::new(Web::default());
    // private final ColorGraphNodeFactory graphNodeFactory = ColorGraphNodeFactory.createFactory(
    //     ColorRegistry.builder().setDefaultNativeColorsForTesting().build());
    let graph_node_factory = ColorGraphNodeFactory::create_factory(
        &mut web.arena.borrow_mut(),
        Arc::new(
            ColorRegistry::builder()
                .set_default_native_colors_for_testing()
                .build(),
        ),
    );
    Ok(native(ColorGraphBuilderTestHelpers {
        compiler: c,
        web,
        graph_node_factory: Rc::new(RefCell::new(graph_node_factory)),
        processor: None,
        label_to_id: None,
    }))
}

// port: ColorGraphBuilderTest_Helpers#prepareProcessor
fn color_graph_builder_test_prepare_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [this @ DslValue::Native(holder)] = args.as_slice() else {
        return Err(bad());
    };
    {
        let mut holder = holder.borrow_mut();
        let Some(holder) = holder
            .as_any_mut()
            .downcast_mut::<ColorGraphBuilderTestHelpers>()
        else {
            return Err(bad());
        };
        // ColorGraphNodeFactory graphNodeFactory = this.graphNodeFactory;
        // LinkedHashMap<String, ColorGraphNode> testTypes = new LinkedHashMap<>();
        // this.labelToId = new LinkedHashMap<>();
        let label_to_id = Rc::new(RefCell::new(IndexMap::<_, _>::default()));
        holder.label_to_id = Some(label_to_id.clone());
        // this.processor = (externs, main) -> NodeTraversal.traverse(...);
        holder.processor = Some(Rc::new(RefCell::new(ColorGraphBuilderTestProcessor {
            web: holder.web.clone(),
            test_types: Rc::new(RefCell::new(IndexMap::<_, _>::default())),
            graph_node_factory: holder.graph_node_factory.clone(),
            label_to_id,
        })));
    }
    // return this;
    Ok(this.clone())
}

// port: ColorGraphBuilderTest_Helpers#getProcessor
fn color_graph_builder_test_get_processor(
    _ctx: &mut Ctx,
    mut args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(holder), DslValue::Compiler(c)] = args.as_mut_slice() else {
        return Err(bad());
    };
    let mut holder = holder.borrow_mut();
    let Some(holder) = holder
        .as_any_mut()
        .downcast_mut::<ColorGraphBuilderTestHelpers>()
    else {
        return Err(bad());
    };
    // assertThat(compiler).isSameInstanceAs(this.compiler);
    crate::throwable::assert_that(
        Rc::ptr_eq(c, &holder.compiler),
        "expected specific instance",
    )?;
    // return this.processor;
    Ok(match &holder.processor {
        None => DslValue::Null,
        Some(processor) => {
            let processor: Rc<RefCell<dyn NativeObject>> = processor.clone();
            DslValue::Native(processor)
        }
    })
}

impl NativeObject for ColorGraphBuilderTestHelpers {
    fn class_name(&self) -> &str {
        CGBT_HOLDER
    }
    // port: ReplayValues#findField (the holder's instance fields, in declaration order)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([
            (
                "compiler".to_string(),
                DslValue::Compiler(self.compiler.clone()),
            ),
            (
                "graphNodeFactory".to_string(),
                native(ColorGraphNodeFactoryDump {
                    web: self.web.clone(),
                    factory: self.graph_node_factory.clone(),
                }),
            ),
            (
                "processor".to_string(),
                match &self.processor {
                    None => DslValue::Null,
                    Some(processor) => {
                        let processor: Rc<RefCell<dyn NativeObject>> = processor.clone();
                        DslValue::Native(processor)
                    }
                },
            ),
            (
                "labelToId".to_string(),
                match &self.label_to_id {
                    None => DslValue::Null,
                    Some(label_to_id) => DslValue::Map(
                        label_to_id
                            .borrow()
                            .iter()
                            .map(|(k, v)| (DslValue::String(k.clone()), color_id_value(*v)))
                            .collect(),
                    ),
                },
            ),
        ]))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The `processor` lambda `prepareProcessor` assigns (captures `this`, `testTypes`,
/// `graphNodeFactory`).
struct ColorGraphBuilderTestProcessor {
    web: Rc<Web>,
    test_types: Rc<RefCell<IndexMap<JsString, ColorGraphNodeId>>>,
    graph_node_factory: Rc<RefCell<ColorGraphNodeFactory>>,
    /// `this.labelToId`, the field the lambda writes through `this`.
    label_to_id: Rc<RefCell<IndexMap<JsString, ColorId>>>,
}

impl NativeObject for ColorGraphBuilderTestProcessor {
    fn class_name(&self) -> &str {
        CGBT_LAMBDA
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == CGBT_LAMBDA || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: ReplayValues#findField (a lambda has no named fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: UnitRecorder#dump (lambda captures; arg$1, the holder, is not modelled)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        Some(Ok(IndexMap::<_, _>::from_iter([
            (
                "arg$2".to_string(),
                DslValue::Map(
                    self.test_types
                        .borrow()
                        .iter()
                        .map(|(k, v)| {
                            (DslValue::String(k.clone()), graph_node_value(&self.web, *v))
                        })
                        .collect(),
                ),
            ),
            (
                "arg$3".to_string(),
                native(ColorGraphNodeFactoryDump {
                    web: self.web.clone(),
                    factory: self.graph_node_factory.clone(),
                }),
            ),
        ])))
    }
    // port: ColorGraphBuilderTest_Helpers#prepareProcessor (the processor lambda)
    fn process(
        &mut self,
        compiler: &mut Compiler,
        _externs: NodeId,
        main: NodeId,
    ) -> Result<(), Throwable> {
        let mut test_types = self.test_types.borrow_mut();
        let mut graph_node_factory = self.graph_node_factory.borrow_mut();
        let mut label_to_id = self.label_to_id.borrow_mut();
        let mut arena = self.web.arena.borrow_mut();
        let mut failure = None;
        NodeTraversal::traverse(
            compiler,
            main,
            &mut AbstractPostOrderCallback::new(
                |t: &mut NodeTraversal<'_>, n: NodeId, _unused: Option<NodeId>| {
                    if failure.is_some() {
                        return;
                    }
                    if n.is_name(t) && n.get_string_ref(t).starts_with("test") {
                        let color = n.get_color(t);
                        test_types.insert(
                            n.get_string(t),
                            graph_node_factory.create_node(&mut arena, color.as_ref()),
                        );
                    }

                    if n.is_label(t) {
                        let label_name = n.get_first_child(t).unwrap().get_string(t);
                        let second = n.get_second_child(t).unwrap();
                        let labeled_expr = if second.is_expr_result(t) {
                            second.get_only_child(t)
                        } else {
                            second
                        };
                        match labeled_expr.get_color(t) {
                            Some(color) => {
                                label_to_id.insert(label_name, color.get_id());
                            }
                            None => {
                                failure = Some(Throwable::Exception {
                                    class: "java.lang.NullPointerException".into(),
                                    message: None,
                                });
                            }
                        }
                    }
                },
            ),
        );
        failure.map_or(Ok(()), Err)
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
