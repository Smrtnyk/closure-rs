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
 * Copyright 2017 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AbstractVar.java,
//   src/com/google/javascript/jscomp/BasicBlock.java,
//   src/com/google/javascript/jscomp/CrossChunkReferenceCollector.java,
//   src/com/google/javascript/jscomp/Reference.java,
//   src/com/google/javascript/jscomp/ReferenceCollection.java,
//   test/com/google/javascript/jscomp/CrossChunkReferenceCollectorTest.java.

//! Port of the unit-corpus helper `CrossChunkReferenceCollectorTest_Helpers.java`
//! (oracle/replay/helpers): the holder of CrossChunkReferenceCollectorTest's `testedCollector`
//! field and its `getProcessor`, plus the native views of the collector that the recorder reads
//! (`postCall.pass.crossChunkReferences` accessors and the `testFieldsAfter` field dump).
//!
//! The collector's Java objects (Var, ReferenceCollection, Reference, BasicBlock,
//! TopLevelStatement) are read through compiler-backed APIs in Rust, so the views are taken once
//! when the pass has run; the collector is not changed afterwards (Java reads the same state
//! lazily after `testSame` returns).
use crate::{
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass,
    cross_chunk_reference_collector::CrossChunkReferenceCollector, reference::Reference,
    syntactic_scope_creator::SyntacticScopeCreator, var::VarId,
};
use closure_rhino::node::NodeId;
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.CrossChunkReferenceCollectorTest_Helpers";
const COLLECTOR: &str = "com.google.javascript.jscomp.CrossChunkReferenceCollector";
const TOP_LEVEL_STATEMENT: &str =
    "com.google.javascript.jscomp.CrossChunkReferenceCollector$TopLevelStatement";

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!("CrossChunkReferenceCollectorTest_Helpers: {what}"))
}

fn native(o: impl NativeObject) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(o)))
}

/// A Java object the referenceable dump only names (`{"ref": class}`): every such value of the
/// collector sits at object depth 0, where UnitRecorder#dump writes a reference.
struct JavaRef {
    class: String,
}
impl NativeObject for JavaRef {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        &self.class
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
fn java_ref(class: &str) -> DslValue {
    native(JavaRef {
        class: class.into(),
    })
}
fn node_ref(compiler: &AbstractCompiler, n: Option<NodeId>) -> DslValue {
    n.map_or(DslValue::Null, |n| java_ref(n.get_class(compiler)))
}

/// `com.google.javascript.jscomp.Var` (fields of AbstractVar in declaration order).
struct VarView {
    fields: IndexMap<String, DslValue>,
}
impl NativeObject for VarView {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.Var"
    }
    // port: ReplayValues#findField (AbstractVar fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(self.fields.clone())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `com.google.javascript.jscomp.BasicBlock`, read through `getRoot`.
struct BasicBlockView {
    root: NodeId,
}
impl NativeObject for BasicBlockView {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.BasicBlock"
    }
    // port: BasicBlock#getRoot
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.len()) {
            ("getRoot", 0) => Ok(DslValue::Node(self.root)),
            _ => Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            ))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `com.google.javascript.jscomp.Reference`, read through `getNode` and `getBasicBlock`.
struct ReferenceView {
    node: NodeId,
    basic_block: DslValue,
}
impl NativeObject for ReferenceView {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.Reference"
    }
    // port: Reference#getNode / Reference#getBasicBlock
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.len()) {
            ("getNode", 0) => Ok(DslValue::Node(self.node)),
            ("getBasicBlock", 0) => Ok(self.basic_block.clone()),
            _ => Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            ))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `com.google.javascript.jscomp.ReferenceCollection`.
struct ReferenceCollectionView {
    references: Vec<DslValue>,
    is_assigned_once_in_lifetime: bool,
    is_well_defined: bool,
}
impl NativeObject for ReferenceCollectionView {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.ReferenceCollection"
    }
    // port: ReferenceCollection#isAssignedOnceInLifetime / ReferenceCollection#isWellDefined
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.len()) {
            ("isAssignedOnceInLifetime", 0) => {
                Ok(DslValue::Bool(self.is_assigned_once_in_lifetime))
            }
            ("isWellDefined", 0) => Ok(DslValue::Bool(self.is_well_defined)),
            _ => Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            ))),
        }
    }
    // port: ReplayValues#findField (ReferenceCollection#references)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "references".to_string(),
            DslValue::List(self.references.clone()),
        )]))
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `CrossChunkReferenceCollector.TopLevelStatement`.
struct TopLevelStatementView {
    original_order: i32,
    chunk: DslValue,
    statement_node: NodeId,
    statement_node_ref: DslValue,
    non_declaration_references: Vec<DslValue>,
    declared_name_reference: DslValue,
    declared_value_node: Option<NodeId>,
    declared_value_node_ref: DslValue,
    is_movable_declaration: bool,
}
impl NativeObject for TopLevelStatementView {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        TOP_LEVEL_STATEMENT
    }
    // port: CrossChunkReferenceCollector.TopLevelStatement (getters)
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.len()) {
            ("getOriginalOrder", 0) => Ok(DslValue::Int(self.original_order)),
            ("getStatementNode", 0) => Ok(DslValue::Node(self.statement_node)),
            ("isDeclarationStatement", 0) => Ok(DslValue::Bool(!matches!(
                self.declared_name_reference,
                DslValue::Null
            ))),
            ("isMovableDeclaration", 0) => Ok(DslValue::Bool(self.is_movable_declaration)),
            ("getDeclaredNameReference", 0) => match &self.declared_name_reference {
                DslValue::Null => Err(Throwable::Exception {
                    class: "java.lang.NullPointerException".into(),
                    message: None,
                }),
                r => Ok(r.clone()),
            },
            ("getNonDeclarationReferences", 0) => {
                Ok(DslValue::List(self.non_declaration_references.clone()))
            }
            ("getDeclaredValueNode", 0) => Ok(self
                .declared_value_node
                .map_or(DslValue::Null, DslValue::Node)),
            _ => Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            ))),
        }
    }
    // port: ReplayValues#findField (TopLevelStatement fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([
            (
                "originalOrder".to_string(),
                DslValue::Int(self.original_order),
            ),
            ("chunk".to_string(), self.chunk.clone()),
            ("statementNode".to_string(), self.statement_node_ref.clone()),
            (
                "nonDeclarationReferences".to_string(),
                // ImmutableList-free Java: Collections.unmodifiableList(ArrayList).
                DslValue::Typed {
                    class: "java.util.Collections$UnmodifiableRandomAccessList".into(),
                    value: Box::new(DslValue::List(self.non_declaration_references.clone())),
                },
            ),
            (
                "declaredNameReference".to_string(),
                self.declared_name_reference.clone(),
            ),
            (
                "declaredValueNode".to_string(),
                self.declared_value_node_ref.clone(),
            ),
        ]))
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// What the collector holds once it has run, as replay values.
#[derive(Default)]
struct CollectorState {
    vars: Vec<(VarId, String, DslValue)>,
    references: Vec<(VarId, DslValue)>,
    top_level_statements: Vec<DslValue>,
    statement_counter: i32,
}

/// The `CrossChunkReferenceCollector` that getProcessor returns (and stores in testedCollector).
struct CollectorView {
    compiler: CompilerHandle,
    collector: CrossChunkReferenceCollector<'static>,
    state: CollectorState,
}

impl CollectorView {
    // port: UnitRecorder#postCallSnapshot (CrossChunkReferenceCollector state after the pass)
    fn read_state(&mut self, compiler: &mut AbstractCompiler) -> Result<CollectorState, Throwable> {
        if !self.collector.get_block_stack().is_empty()
            || self.collector.has_top_level_statement_draft()
        {
            return Err(bad("collector left a block or draft statement behind"));
        }
        let mut references_by_node: IndexMap<NodeId, DslValue> = IndexMap::new();
        let mut reference = |r: &Reference| -> DslValue {
            references_by_node
                .entry(r.get_node())
                .or_insert_with(|| {
                    native(ReferenceView {
                        node: r.get_node(),
                        basic_block: r.get_basic_block().map_or(DslValue::Null, |b| {
                            native(BasicBlockView { root: b.get_root() })
                        }),
                    })
                })
                .clone()
        };
        let mut vars = vec![];
        for (name, var) in self.collector.get_global_variable_names_map() {
            vars.push((var, name.to_string_lossy(), var_view(compiler, var)));
        }
        let mut references = vec![];
        let symbols: Vec<VarId> = self.collector.get_all_symbols().collect();
        for var in symbols {
            let collection = self.collector.get_references(var).unwrap().clone();
            let refs = collection.iter().map(&mut reference).collect::<Vec<_>>();
            references.push((
                var,
                native(ReferenceCollectionView {
                    references: refs,
                    is_assigned_once_in_lifetime: collection.is_assigned_once_in_lifetime(compiler),
                    is_well_defined: collection.is_well_defined(compiler),
                }),
            ));
        }
        let mut top_level_statements = vec![];
        for i in 0..self.collector.get_top_level_statements().len() {
            let is_movable_declaration = self.collector.get_top_level_statements()[i]
                .is_movable_declaration(compiler, &self.collector);
            let t = &self.collector.get_top_level_statements()[i];
            top_level_statements.push(native(TopLevelStatementView {
                original_order: t.get_original_order(),
                chunk: t.get_chunk().map_or(DslValue::Null, |_| {
                    java_ref("com.google.javascript.jscomp.JSChunk")
                }),
                statement_node: t.get_statement_node(),
                statement_node_ref: node_ref(compiler, Some(t.get_statement_node())),
                non_declaration_references: t
                    .get_non_declaration_references()
                    .iter()
                    .map(&mut reference)
                    .collect(),
                declared_name_reference: if t.is_declaration_statement() {
                    reference(t.get_declared_name_reference())
                } else {
                    DslValue::Null
                },
                declared_value_node: t.get_declared_value_node(),
                declared_value_node_ref: node_ref(compiler, t.get_declared_value_node()),
                is_movable_declaration,
            }));
        }
        Ok(CollectorState {
            vars,
            references,
            top_level_statements,
            statement_counter: self.collector.get_statement_counter(),
        })
    }

    fn var_for(&self, value: &DslValue) -> Option<VarId> {
        let DslValue::Native(v) = value else {
            return None;
        };
        self.state
            .vars
            .iter()
            .find(|(_, _, view)| matches!(view, DslValue::Native(o) if Rc::ptr_eq(o, v)))
            .map(|(id, ..)| *id)
    }
}

// port: AbstractVar fields (name, nameNode, implicitGoogNamespaceStrength, input, index, scope)
fn var_view(compiler: &AbstractCompiler, var: VarId) -> DslValue {
    let strength = if var.is_implicit_goog_namespace(compiler) {
        DslValue::Enum {
            class: "com.google.javascript.rhino.StaticSourceFile$SourceKind".into(),
            name: format!("{:?}", var.get_implicit_goog_namespace_strength(compiler)),
        }
    } else {
        DslValue::Null
    };
    native(VarView {
        fields: IndexMap::from([
            (
                "AbstractVar.name".to_string(),
                DslValue::String(var.get_name(compiler)),
            ),
            (
                "AbstractVar.nameNode".to_string(),
                node_ref(compiler, var.get_name_node(compiler)),
            ),
            (
                "AbstractVar.implicitGoogNamespaceStrength".to_string(),
                strength,
            ),
            (
                "AbstractVar.input".to_string(),
                var.get_input(compiler).map_or(DslValue::Null, |_| {
                    java_ref("com.google.javascript.jscomp.CompilerInput")
                }),
            ),
            (
                "AbstractVar.index".to_string(),
                DslValue::Int(var.get_index(compiler)),
            ),
            (
                "AbstractVar.scope".to_string(),
                java_ref("com.google.javascript.jscomp.Scope"),
            ),
        ]),
    })
}

impl NativeObject for CollectorView {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        COLLECTOR
    }
    // port: CrossChunkReferenceCollector#getGlobalVariableNamesMap / #getReferences /
    // #getTopLevelStatements
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.as_slice()) {
            ("getGlobalVariableNamesMap", []) => Ok(DslValue::Map(
                self.state
                    .vars
                    .iter()
                    .map(|(_, name, view)| (DslValue::String(name.as_str().into()), view.clone()))
                    .collect(),
            )),
            ("getReferences", [var]) => Ok(self
                .var_for(var)
                .and_then(|id| {
                    self.state
                        .references
                        .iter()
                        .find(|(v, _)| *v == id)
                        .map(|(_, r)| r.clone())
                })
                .unwrap_or(DslValue::Null)),
            ("getTopLevelStatements", []) => {
                Ok(DslValue::List(self.state.top_level_statements.clone()))
            }
            _ => Err(Throwable::Unported(format!("{COLLECTOR}#{method}"))),
        }
    }
    // port: ReplayValues#findField (CrossChunkReferenceCollector fields in declaration order)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let var_view = |id: VarId| {
            self.state
                .vars
                .iter()
                .find(|(v, ..)| *v == id)
                .map(|(.., view)| view.clone())
                .ok_or_else(|| bad("referenced variable is not a global name"))
        };
        Ok(IndexMap::from([
            (
                "varsByName".to_string(),
                DslValue::Map(
                    self.state
                        .vars
                        .iter()
                        .map(|(_, name, view)| {
                            (DslValue::String(name.as_str().into()), view.clone())
                        })
                        .collect(),
                ),
            ),
            (
                "referenceMap".to_string(),
                DslValue::Map(
                    self.state
                        .references
                        .iter()
                        .map(|(var, refs)| Ok((var_view(*var)?, refs.clone())))
                        .collect::<Result<_, Throwable>>()?,
                ),
            ),
            ("blockStack".to_string(), DslValue::List(vec![])),
            (
                "topLevelStatements".to_string(),
                DslValue::List(self.state.top_level_statements.clone()),
            ),
            (
                "scopeCreator".to_string(),
                native(ScopeCreatorView {
                    compiler: self.compiler.clone(),
                }),
            ),
            (
                "compiler".to_string(),
                DslValue::Compiler(self.compiler.clone()),
            ),
            (
                "statementCounter".to_string(),
                DslValue::Int(self.state.statement_counter),
            ),
            ("topLevelStatementDraft".to_string(), DslValue::Null),
        ]))
    }
    // port: CrossChunkReferenceCollector#process(Node,Node)
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.collector.process(compiler, externs, root);
        self.state = self.read_state(compiler)?;
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The `SyntacticScopeCreator` getProcessor builds (`new SyntacticScopeCreator(compiler)`).
struct ScopeCreatorView {
    compiler: CompilerHandle,
}
impl NativeObject for ScopeCreatorView {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.SyntacticScopeCreator"
    }
    // port: ReplayValues#findField (SyntacticScopeCreator fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([
            (
                "compiler".to_string(),
                DslValue::Compiler(self.compiler.clone()),
            ),
            (
                "redeclarationHandler".to_string(),
                java_ref(
                    "com.google.javascript.jscomp.SyntacticScopeCreator$DefaultRedeclarationHandler",
                ),
            ),
            (
                "treatProvidesAsRedeclarations".to_string(),
                DslValue::Bool(false),
            ),
        ]))
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// CrossChunkReferenceCollectorTest_Helpers: the holder of the test's `testedCollector` field.
struct Holder {
    tested_collector: DslValue,
}
impl NativeObject for Holder {
    // port: Object#getClass
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (CrossChunkReferenceCollectorTest_Helpers#testedCollector)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "testedCollector".to_string(),
            self.tested_collector.clone(),
        )]))
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: CrossChunkReferenceCollectorTest_Helpers#CrossChunkReferenceCollectorTest_Helpers
pub fn holder(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad("constructor arguments"));
    }
    Ok(native(Holder {
        tested_collector: DslValue::Null,
    }))
}

// port: CrossChunkReferenceCollectorTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(holder), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad("getProcessor arguments"));
    };
    // ScopeCreator scopeCreator = new SyntacticScopeCreator(compiler);
    // testedCollector = new CrossChunkReferenceCollector(compiler, scopeCreator);
    let collector = native(CollectorView {
        compiler: compiler.clone(),
        collector: CrossChunkReferenceCollector::new(Box::new(SyntacticScopeCreator::new())),
        state: CollectorState::default(),
    });
    let mut holder = holder.borrow_mut();
    let holder = holder
        .as_any_mut()
        .downcast_mut::<Holder>()
        .ok_or_else(|| bad("getProcessor receiver"))?;
    holder.tested_collector = collector.clone();
    // return testedCollector;
    Ok(collector)
}

// port: CrossChunkReferenceCollectorTest_Helpers#getProcessor (compiler borrowed by the harness)
pub fn get_processor_borrowed(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
    _compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    get_processor(ctx, args)
}
