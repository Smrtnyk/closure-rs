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
 * Copyright 2007 The Closure Compiler Authors.
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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/ProcessDefinesTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../ProcessDefinesTest_Helpers.java` (DSL
//! name `ProcessDefinesTest_Helpers.GetProcessorHost`), itself copied from ProcessDefinesTest.java:
//! the test-instance fields, `getProcessor` and the inner class
//! `ProcessDefinesWithInjectedNamespace`.
use crate::{
    json::JsonValue,
    replay::{
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
        replay_values::object,
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    global_namespace::{GlobalNamespace, Name, Ref, RefsForNodeUnitDump},
    j2cl_source_file_checker::J2clSourceFileChecker,
    process_defines::{Builder, Mode},
};
use closure_rhino::node::NodeId;
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const HOST: &str = "com.google.javascript.jscomp.ProcessDefinesTest_Helpers$GetProcessorHost";
const PASS: &str = "com.google.javascript.jscomp.ProcessDefinesTest_Helpers$GetProcessorHost$ProcessDefinesWithInjectedNamespace";
const MODE: &str = "com.google.javascript.jscomp.ProcessDefines$Mode";
const GLOBAL_NAMESPACE: &str = "com.google.javascript.jscomp.GlobalNamespace";
const GLOBAL_NAMESPACE_LAMBDA: &str = "com.google.javascript.jscomp.GlobalNamespace$$Lambda";
const NAME: &str = "com.google.javascript.jscomp.GlobalNamespace$Name";
const REF: &str = "com.google.javascript.jscomp.GlobalNamespace$Ref";
const SOURCE_KIND: &str = "com.google.javascript.jscomp.GlobalNamespace$SourceKind";
const JSDOC_INFO: &str = "com.google.javascript.rhino.JSDocInfo";
const HASH_BASED_TABLE: &str = "com.google.common.collect.HashBasedTable";

fn bad() -> Throwable {
    Throwable::HarnessError("ProcessDefinesTest_Helpers: unexpected arguments".into())
}

type NamespaceCell = Rc<RefCell<Option<Rc<RefCell<GlobalNamespace>>>>>;

/// `private static final class GetProcessorHost extends CompilerTestCase`.
pub struct GetProcessorHost {
    overrides: DslValue,
    /// Written by `ProcessDefinesWithInjectedNamespace#process`.
    namespace: NamespaceCell,
    /// The compiler `getProcessor` was called with (the namespace's `compiler`).
    compiler: Option<CompilerHandle>,
    mode: DslValue,
    recognize_closure_defines: bool,
    enable_j2cl_passes: bool,
    enable_define_without_goog_define_check: bool,
}

/// The `GlobalNamespace` the inner pass stored in the host's `namespace` field.
struct NativeGlobalNamespace {
    /// The compiler the pass ran with (Java's `compiler` field; also the AST of the node fields).
    compiler: CompilerHandle,
    namespace: Rc<RefCell<GlobalNamespace>>,
}

/// A `GlobalNamespace.Name` reachable from the dumped namespace.
struct NativeName {
    compiler: CompilerHandle,
    namespace: Rc<RefCell<GlobalNamespace>>,
    name: Name,
}

/// An object the namespace dump reaches only below the recorded depth (`GlobalNamespace.Ref`,
/// `JSDocInfo`), so UnitRecorder writes it as a reference to its runtime class.
struct NativeReference(&'static str);

/// `shouldTraverseScript`: the field initializer `(n) -> true` (only Compiler's runtime-library
/// namespace replaces it, never the namespace this helper creates), a capture-free lambda.
struct NativeShouldTraverseScript;

/// `nameMapByModule`, a `HashBasedTable<ModuleMetadata, String, Name>`.
struct NativeNameMapByModule {
    is_empty: bool,
}

/// `node` as UnitRecorder writes it: a reference to its runtime class (`Node$StringNode`, ...).
fn node_value(compiler: &CompilerHandle, node: Option<NodeId>) -> Result<DslValue, Throwable> {
    let Some(node) = node else {
        return Ok(DslValue::Null);
    };
    let compiler = compiler
        .try_borrow()
        .map_err(|_| Throwable::HarnessError("compiler is borrowed".into()))?;
    Ok(DslValue::Typed {
        class: node.get_class(&compiler).into(),
        value: Box::new(DslValue::Node(node)),
    })
}

fn native(object: impl NativeObject) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(object)))
}

fn ref_value(r: Option<Ref>) -> DslValue {
    r.map_or(DslValue::Null, |_| native(NativeReference(REF)))
}

fn jsdoc_value<T>(info: Option<T>) -> DslValue {
    info.map_or(DslValue::Null, |_| native(NativeReference(JSDOC_INFO)))
}

impl NativeGlobalNamespace {
    fn name_value(&self, name: Name) -> DslValue {
        native(NativeName {
            compiler: Rc::clone(&self.compiler),
            namespace: Rc::clone(&self.namespace),
            name,
        })
    }
}

impl NativeObject for NativeGlobalNamespace {
    fn class_name(&self) -> &str {
        GLOBAL_NAMESPACE
    }
    // port: UnitRecorder#collect (GlobalNamespace fields, Java declaration order)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let dump = self.namespace.borrow().unit_dump_fields();
        let mut fields = IndexMap::new();
        fields.insert(
            "compiler".into(),
            DslValue::Compiler(Rc::clone(&self.compiler)),
        );
        fields.insert(
            "enableImplicitlyAliasedValues".into(),
            DslValue::Bool(dump.enable_implicitly_aliased_values),
        );
        fields.insert("root".into(), node_value(&self.compiler, Some(dump.root))?);
        fields.insert(
            "externsRoot".into(),
            node_value(&self.compiler, dump.externs_root)?,
        );
        fields.insert(
            "globalRoot".into(),
            node_value(&self.compiler, Some(dump.global_root))?,
        );
        let spread_sibling_cache = dump
            .spread_sibling_cache
            .iter()
            .map(|&(node, cached)| {
                Ok((
                    node_value(&self.compiler, Some(node))?,
                    DslValue::Bool(cached),
                ))
            })
            .collect::<Result<_, Throwable>>()?;
        fields.insert(
            "spreadSiblingCache".into(),
            DslValue::Map(spread_sibling_cache),
        );
        fields.insert(
            "sourceKind".into(),
            dump.source_kind
                .map_or(DslValue::Null, |kind| DslValue::Enum {
                    class: SOURCE_KIND.into(),
                    name: format!("{kind:?}"),
                }),
        );
        fields.insert("generated".into(), DslValue::Bool(dump.generated));
        if dump.decisions_log.is_some() {
            return Err(Throwable::Unported(format!(
                "{GLOBAL_NAMESPACE} field dump of a non-null decisionsLog (LogFile)"
            )));
        }
        fields.insert("decisionsLog".into(), DslValue::Null);
        fields.insert(
            "globalNames".into(),
            DslValue::List(
                dump.global_names
                    .iter()
                    .map(|&name| self.name_value(name))
                    .collect(),
            ),
        );
        fields.insert(
            "nameMap".into(),
            DslValue::Map(
                dump.name_map
                    .iter()
                    .map(|(key, name)| (DslValue::String(key.clone()), self.name_value(*name)))
                    .collect(),
            ),
        );
        fields.insert(
            "nameMapByModule".into(),
            native(NativeNameMapByModule {
                is_empty: dump.name_map_by_module.is_empty(),
            }),
        );
        fields.insert(
            "shouldTraverseScript".into(),
            native(NativeShouldTraverseScript),
        );
        Ok(fields)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl NativeObject for NativeName {
    fn class_name(&self) -> &str {
        NAME
    }
    // port: UnitRecorder#collect (GlobalNamespace.Name fields, Java declaration order)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let dump = self.name.unit_dump_fields(&self.namespace.borrow());
        let name_value = |name: Name| {
            native(NativeName {
                compiler: Rc::clone(&self.compiler),
                namespace: Rc::clone(&self.namespace),
                name,
            })
        };
        let mut fields = IndexMap::new();
        fields.insert("baseName".into(), DslValue::String(dump.base_name));
        fields.insert(
            "parent".into(),
            dump.parent.map_or(DslValue::Null, name_value),
        );
        fields.insert(
            "props".into(),
            dump.props.map_or(DslValue::Null, |props| {
                DslValue::List(props.into_iter().map(name_value).collect())
            }),
        );
        fields.insert("declaration".into(), ref_value(dump.declaration));
        fields.insert("initialization".into(), ref_value(dump.initialization));
        fields.insert(
            "refsForNode".into(),
            match dump.refs_for_node {
                RefsForNodeUnitDump::Null => DslValue::Null,
                RefsForNodeUnitDump::Single(r) => ref_value(Some(r)),
                RefsForNodeUnitDump::Map(entries) => DslValue::Map(
                    entries
                        .into_iter()
                        .map(|(node, r)| {
                            Ok((node_value(&self.compiler, node)?, ref_value(Some(r))))
                        })
                        .collect::<Result<_, Throwable>>()?,
                ),
            },
        );
        for (key, value) in [
            ("globalSets", dump.global_sets),
            ("localSets", dump.local_sets),
            ("localSetsWithNoCollapse", dump.local_sets_with_no_collapse),
            ("aliasingGets", dump.aliasing_gets),
            ("totalGets", dump.total_gets),
            ("callGets", dump.call_gets),
            ("deleteProps", dump.delete_props),
            ("subclassingGets", dump.subclassing_gets),
            ("propertyBitSet", dump.property_bit_set),
        ] {
            fields.insert(key.into(), DslValue::Int(value));
        }
        fields.insert(
            "firstDeclarationJSDocInfo".into(),
            jsdoc_value(dump.first_declaration_jsdoc_info),
        );
        fields.insert(
            "firstQnameDeclarationWithoutAssignmentJsDocInfo".into(),
            jsdoc_value(dump.first_qname_declaration_without_assignment_jsdoc_info),
        );
        Ok(fields)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl NativeObject for NativeReference {
    fn class_name(&self) -> &str {
        self.0
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl NativeObject for NativeShouldTraverseScript {
    fn class_name(&self) -> &str {
        GLOBAL_NAMESPACE_LAMBDA
    }
    // port: UnitRecorder#dump (a lambda: a reference to its class with its captured arguments)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        Some(Ok(IndexMap::new()))
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl NativeObject for NativeNameMapByModule {
    fn class_name(&self) -> &str {
        HASH_BASED_TABLE
    }
    // port: UnitRecorder#dump (Guava Table: {"table": [[row, column, value], ...], "impl": FQCN})
    fn tagged_dump(&self) -> Result<Option<JsonValue>, Throwable> {
        if !self.is_empty {
            // The cells' row keys are ModuleMetadata objects; no record reaches a non-empty table.
            return Err(Throwable::Unported(format!(
                "{GLOBAL_NAMESPACE} field dump of a non-empty nameMapByModule"
            )));
        }
        Ok(Some(object([
            ("table", JsonValue::Array(Vec::new())),
            ("impl", JsonValue::str(HASH_BASED_TABLE)),
        ])))
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl NativeObject for GetProcessorHost {
    fn class_name(&self) -> &str {
        HOST
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert("overrides".into(), self.overrides.clone());
        fields.insert(
            "namespace".into(),
            match (&*self.namespace.borrow(), &self.compiler) {
                (None, _) => DslValue::Null,
                (Some(namespace), Some(compiler)) => native(NativeGlobalNamespace {
                    compiler: Rc::clone(compiler),
                    namespace: Rc::clone(namespace),
                }),
                (Some(_), None) => return Err(bad()),
            },
        );
        fields.insert("mode".into(), self.mode.clone());
        fields.insert(
            "recognizeClosureDefines".into(),
            DslValue::Bool(self.recognize_closure_defines),
        );
        fields.insert(
            "enableJ2clPasses".into(),
            DslValue::Bool(self.enable_j2cl_passes),
        );
        fields.insert(
            "enableDefineWithoutGoogDefineCheck".into(),
            DslValue::Bool(self.enable_define_without_goog_define_check),
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value) {
            ("overrides", value) => self.overrides = value,
            ("mode", value) => self.mode = value,
            ("recognizeClosureDefines", DslValue::Bool(b)) => self.recognize_closure_defines = b,
            ("enableJ2clPasses", DslValue::Bool(b)) => self.enable_j2cl_passes = b,
            ("enableDefineWithoutGoogDefineCheck", DslValue::Bool(b)) => {
                self.enable_define_without_goog_define_check = b;
            }
            _ => return Err(Throwable::Unported(format!("{HOST}#{name}"))),
        }
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ProcessDefinesTest_Helpers.GetProcessorHost#GetProcessorHost
pub fn host(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessorHost {
        // private final Map<String, Node> overrides = new HashMap<>();
        overrides: DslValue::Typed {
            class: "java.util.HashMap".into(),
            value: Box::new(DslValue::Map(Vec::new())),
        },
        namespace: Rc::new(RefCell::new(None)),
        compiler: None,
        mode: DslValue::Null,
        recognize_closure_defines: true,
        enable_j2cl_passes: false,
        enable_define_without_goog_define_check: false,
    }))))
}

/// `ProcessDefinesWithInjectedNamespace`, with the host fields its `process` reads.
struct ProcessDefinesWithInjectedNamespace {
    namespace: NamespaceCell,
    overrides: IndexMap<String, NodeId>,
    mode: Mode,
    recognize_closure_defines: bool,
}

impl NativeObject for ProcessDefinesWithInjectedNamespace {
    fn class_name(&self) -> &str {
        PASS
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == PASS || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: UnitRecorder#collect (no recorded result producer is reachable from this pass)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: ProcessDefinesTest_Helpers.GetProcessorHost.ProcessDefinesWithInjectedNamespace#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        js: NodeId,
    ) -> Result<(), Throwable> {
        let namespace = Rc::new(RefCell::new(GlobalNamespace::new(compiler, externs, js)));
        *self.namespace.borrow_mut() = Some(Rc::clone(&namespace));
        let cell = Rc::clone(&self.namespace);
        let enable_zones_define_name = compiler
            .get_options()
            .get_enable_zones_define_name()
            .map(str::to_string);
        let zone_input_pattern = compiler.get_options().get_zone_input_pattern().clone();
        Builder::new(compiler)
            .put_replacements(self.overrides.clone())
            .set_mode(self.mode)
            .inject_namespace(Box::new(move || cell.borrow().clone()))
            .set_recognize_closure_defines(self.recognize_closure_defines)
            .set_enable_zones_define_name(enable_zones_define_name)
            .set_zone_input_pattern(zone_input_pattern)
            .build(compiler)
            .process(compiler, externs, js);
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn decode_mode(mode: &DslValue) -> Result<Mode, Throwable> {
    let DslValue::Enum { class, name } = mode.untyped() else {
        return Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        });
    };
    if class != MODE {
        return Err(bad());
    }
    Ok(match name.as_str() {
        "CHECK" => Mode::CHECK,
        "OPTIMIZE" => Mode::OPTIMIZE,
        "CHECK_AND_OPTIMIZE" => Mode::CHECK_AND_OPTIMIZE,
        _ => return Err(bad()),
    })
}

fn decode_overrides(overrides: &DslValue) -> Result<IndexMap<String, NodeId>, Throwable> {
    let DslValue::Map(entries) = overrides.untyped() else {
        return Err(bad());
    };
    let mut map = IndexMap::new();
    for (k, v) in entries {
        let DslValue::String(k) = k.untyped() else {
            return Err(bad());
        };
        let DslValue::Node(v) = v.untyped() else {
            return Err(bad());
        };
        map.insert(k.to_string(), *v);
    }
    Ok(map)
}

// port: ProcessDefinesTest_Helpers.GetProcessorHost#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut this = this.borrow_mut();
    let this = this
        .as_any_mut()
        .downcast_mut::<GetProcessorHost>()
        .ok_or_else(bad)?;
    this.compiler = Some(Rc::clone(compiler));
    if this.enable_j2cl_passes {
        J2clSourceFileChecker::mark_to_run_j2cl_passes(&mut compiler.borrow_mut());
    }
    // The inner class reads the host's fields when process runs; none is written in between.
    let pass = ProcessDefinesWithInjectedNamespace {
        namespace: Rc::clone(&this.namespace),
        overrides: decode_overrides(&this.overrides)?,
        mode: decode_mode(&this.mode)?,
        recognize_closure_defines: this.recognize_closure_defines,
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(pass))))
}
