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
 * Copyright 2009 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/AbstractCompiler.java,
//   test/com/google/javascript/jscomp/ProcessClosureProvidesAndRequiresTest.java.

//! Port of the replay helper
//! `oracle/replay/helpers/.../ProcessClosureProvidesAndRequiresTest_Helpers.java` (DSL name
//! `ProcessClosureProvidesAndRequiresTest_Helpers.GetProcessorLambda`), itself copied from
//! ProcessClosureProvidesAndRequiresTest.java: the holder fields `preserveGoogProvidesAndRequires`
//! and `lastProcessor`, `createClosureProcessor`, the getProcessor lambda and
//! `verifyCollectProvidedNamesDoesntChangeAst`. The post-call dump of `lastProcessor` reads the
//! private fields of ProcessClosureProvidesAndRequires, ProvidedName, JSChunkGraph and AstFactory
//! through their `replay_fields` views.
use crate::{
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject, Object},
    throwable::Throwable,
};
use closure_jscomp::{
    Compiler,
    abstract_compiler::LifeCycleStage,
    ast_factory::{AstFactory, TypeMode},
    js_chunk::JSChunk,
    process_closure_provides_and_requires::{ProcessClosureProvidesAndRequires, ProvidedName},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{node::NodeId, testing::node_subject::assert_node};
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.ProcessClosureProvidesAndRequiresTest_Helpers";
const GET_PROCESSOR_LAMBDA: &str =
    "com.google.javascript.jscomp.ProcessClosureProvidesAndRequiresTest_Helpers$GetProcessorLambda";
const PROCESSOR: &str = "com.google.javascript.jscomp.ProcessClosureProvidesAndRequires";
const PROVIDED_NAME: &str =
    "com.google.javascript.jscomp.ProcessClosureProvidesAndRequires$ProvidedName";
const JS_CHUNK: &str = "com.google.javascript.jscomp.JSChunk";

/// The helper holder (one test instance).
pub struct ProcessClosureProvidesAndRequiresTestHelpers {
    preserve_goog_provides_and_requires: bool,
    last_processor: Option<Rc<RefCell<LastProcessor>>>,
}

/// `ProcessClosureProvidesAndRequires lastProcessor`, with the compiler its `compiler` and
/// `chunkGraph` fields refer to.
struct LastProcessor {
    processor: ProcessClosureProvidesAndRequires,
    compiler: CompilerHandle,
}

impl NativeObject for ProcessClosureProvidesAndRequiresTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "preserveGoogProvidesAndRequires".into(),
            DslValue::Bool(self.preserve_goog_provides_and_requires),
        );
        fields.insert(
            "lastProcessor".into(),
            self.last_processor
                .as_ref()
                .map_or(DslValue::Null, |p| DslValue::Native(p.clone())),
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value) {
            ("preserveGoogProvidesAndRequires", DslValue::Bool(value)) => {
                self.preserve_goog_provides_and_requires = value;
                Ok(())
            }
            ("lastProcessor", DslValue::Null) => {
                self.last_processor = None;
                Ok(())
            }
            (name, _) => Err(Throwable::Unported(format!("{HOLDER}#{name}"))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ProcessClosureProvidesAndRequiresTestHelpers {
    // port: ProcessClosureProvidesAndRequiresTest_Helpers#createClosureProcessor
    fn create_closure_processor(
        &self,
        compiler: &mut Compiler,
    ) -> ProcessClosureProvidesAndRequires {
        ProcessClosureProvidesAndRequires::new(compiler, self.preserve_goog_provides_and_requires)
    }

    /// Validates that running {@link ProcessClosureProvidesAndRequires#collectProvidedNames(Node,
    /// Node)} does not modify the AST.
    ///
    /// <p>This is important because we want to call this method to gain information about
    /// goog.provides but preserve the original AST structure for future checks.
    // port: ProcessClosureProvidesAndRequiresTest_Helpers#verifyCollectProvidedNamesDoesntChangeAst
    fn verify_collect_provided_names_doesnt_change_ast(
        &self,
        externs: NodeId,
        root: NodeId,
        compiler: &mut Compiler,
    ) -> Result<(), Throwable> {
        // Validate that this does not modify the AST at all!
        let original_externs = externs.clone_tree(compiler);
        let original_root = root.clone_tree(compiler);
        let mut processor = self.create_closure_processor(compiler);
        processor.collect_provided_names(compiler, externs, root);

        assert_node(externs)
            .check_equal_to(compiler, original_externs, true)
            .map_err(|message| Throwable::Assertion { message })?;
        assert_node(root)
            .check_equal_to(compiler, original_root, true)
            .map_err(|message| Throwable::Assertion { message })?;
        Ok(())
    }
}

/// The lambda `(Node externs, Node root) -> {...}` returned by getProcessor.
struct GetProcessorLambda {
    outer: Rc<RefCell<dyn NativeObject>>,
    compiler: CompilerHandle,
}

impl NativeObject for GetProcessorLambda {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        GET_PROCESSOR_LAMBDA
    }
    // port: ReplayValues#findField (native object adapter: the inner class's fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert("compiler".into(), DslValue::Compiler(self.compiler.clone()));
        fields.insert("this$0".into(), DslValue::Native(self.outer.clone()));
        Ok(fields)
    }
    // port: ProcessClosureProvidesAndRequiresTest_Helpers.GetProcessorLambda#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let mut outer = self.outer.borrow_mut();
        let holder = outer
            .as_any_mut()
            .downcast_mut::<ProcessClosureProvidesAndRequiresTestHelpers>()
            .ok_or_else(bad)?;
        holder.verify_collect_provided_names_doesnt_change_ast(externs, root, compiler)?;
        let last_processor = Rc::new(RefCell::new(LastProcessor {
            processor: holder.create_closure_processor(compiler),
            compiler: self.compiler.clone(),
        }));
        holder.last_processor = Some(last_processor.clone());
        drop(outer);
        last_processor
            .borrow_mut()
            .processor
            .rewrite_provides_and_requires(compiler, externs, root);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// An object the dump only shows as a reference at the depth it is reached.
// port: UnitRecorder#dump (referenceable mode: a reference to the runtime class)
fn reference(class: &str) -> DslValue {
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: class.to_string(),
        fields: IndexMap::<_, _>::default(),
        field_types: IndexMap::<_, _>::default(),
    })))
}

// port: UnitRecorder#dump (referenceable mode: an object with its declared fields)
fn object(class: &str, fields: IndexMap<String, DslValue>) -> DslValue {
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: class.to_string(),
        fields,
        field_types: IndexMap::<_, _>::default(),
    })))
}

// port: UnitRecorder#dump (a nullable Node field)
fn node(n: Option<NodeId>) -> DslValue {
    n.map_or(DslValue::Null, DslValue::Node)
}

// port: UnitRecorder#dump (a nullable JSChunk field)
fn chunk(c: Option<&JSChunk>) -> DslValue {
    c.map_or(DslValue::Null, |_| reference(JS_CHUNK))
}

// port: UnitRecorder#dump (ProcessClosureProvidesAndRequires.ProvidedName fields)
fn provided_name(name: &ProvidedName, compiler: &CompilerHandle) -> DslValue {
    let f = name.replay_fields();
    let mut fields = IndexMap::<_, _>::default();
    fields.insert("namespace".into(), DslValue::String(f.namespace.clone()));
    fields.insert("firstNode".into(), node(*f.first_node));
    fields.insert("firstChunk".into(), chunk(f.first_chunk.as_ref()));
    fields.insert(
        "hasImplicitInitialization".into(),
        DslValue::Bool(*f.has_implicit_initialization),
    );
    fields.insert("explicitNode".into(), node(*f.explicit_node));
    fields.insert("candidateDefinition".into(), node(*f.candidate_definition));
    fields.insert("minimumChunk".into(), chunk(f.minimum_chunk.as_ref()));
    fields.insert("replacementNode".into(), node(*f.replacement_node));
    fields.insert(
        "fromLegacyModule".into(),
        DslValue::Bool(*f.from_legacy_module),
    );
    fields.insert("compiler".into(), DslValue::Compiler(compiler.clone()));
    fields.insert(
        "astFactory".into(),
        reference("com.google.javascript.jscomp.AstFactory"),
    );
    object(PROVIDED_NAME, fields)
}

// port: UnitRecorder#dump (AstFactory fields)
fn ast_factory(factory: &AstFactory) -> DslValue {
    let f = factory.replay_fields();
    let mut fields = IndexMap::<_, _>::default();
    fields.insert(
        "colorRegistry".into(),
        f.color_registry.as_ref().map_or(DslValue::Null, |_| {
            reference("com.google.javascript.jscomp.colors.ColorRegistry")
        }),
    );
    // Java keeps the JSTypeRegistry in a field only in JSTYPE mode.
    fields.insert(
        "registry".into(),
        if *f.type_mode == TypeMode::JSTYPE {
            reference("com.google.javascript.rhino.jstype.JSTypeRegistry")
        } else {
            DslValue::Null
        },
    );
    fields.insert(
        "unknownType".into(),
        f.unknown_type.as_ref().map_or(DslValue::Null, |_| {
            reference("com.google.javascript.rhino.jstype.UnknownType")
        }),
    );
    fields.insert(
        "typeMode".into(),
        DslValue::Enum {
            class: "com.google.javascript.jscomp.AstFactory$TypeMode".into(),
            name: format!("{:?}", f.type_mode),
        },
    );
    fields.insert(
        "lifeCycleStage".into(),
        DslValue::Enum {
            class: "com.google.javascript.jscomp.AbstractCompiler$LifeCycleStage".into(),
            name: life_cycle_stage_name(*f.life_cycle_stage).into(),
        },
    );
    fields.insert(
        "runtimeJsLibManager".into(),
        f.runtime_js_lib_manager
            .as_ref()
            .map_or(DslValue::Null, |_| {
                reference("com.google.javascript.jscomp.js.RuntimeJsLibManager")
            }),
    );
    object("com.google.javascript.jscomp.AstFactory", fields)
}

// port: AbstractCompiler.LifeCycleStage#name
fn life_cycle_stage_name(stage: LifeCycleStage) -> &'static str {
    match stage {
        LifeCycleStage::RAW => "RAW",
        LifeCycleStage::COLORS_AND_SIMPLIFIED_JSDOC => "COLORS_AND_SIMPLIFIED_JSDOC",
        LifeCycleStage::NORMALIZED => "NORMALIZED",
        LifeCycleStage::NORMALIZED_OBFUSCATED => "NORMALIZED_OBFUSCATED",
    }
}

// port: UnitRecorder#dump (JSChunkGraph fields)
fn chunk_graph(compiler: &Compiler) -> DslValue {
    let Some(graph) = compiler.get_chunk_graph() else {
        return DslValue::Null;
    };
    let f = graph.replay_fields();
    let mut fields = IndexMap::<_, _>::default();
    fields.insert(
        "chunks".into(),
        DslValue::Array {
            component: JS_CHUNK.into(),
            items: f.chunks.iter().map(|_| reference(JS_CHUNK)).collect(),
        },
    );
    fields.insert(
        "selfPlusTransitiveDeps".into(),
        DslValue::Array {
            component: "java.util.BitSet".into(),
            items: f
                .self_plus_transitive_deps
                .iter()
                .map(|_| reference("java.util.BitSet"))
                .collect(),
        },
    );
    fields.insert(
        "subtreeSize".into(),
        DslValue::Array {
            component: "int".into(),
            items: f.subtree_size.iter().map(|s| DslValue::Int(*s)).collect(),
        },
    );
    fields.insert(
        "chunksByDepth".into(),
        DslValue::List(
            f.chunks_by_depth
                .iter()
                .map(|chunks| DslValue::List(chunks.iter().map(|_| reference(JS_CHUNK)).collect()))
                .collect(),
        ),
    );
    fields.insert(
        "dependencyMap".into(),
        reference("com.google.javascript.jscomp.base.LinkedIdentityHashMap"),
    );
    object("com.google.javascript.jscomp.JSChunkGraph", fields)
}

impl NativeObject for LastProcessor {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        PROCESSOR
    }
    // port: ReplayValues#findField (native object adapter: ProcessClosureProvidesAndRequires)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let compiler = self
            .compiler
            .try_borrow()
            .map_err(|_| Throwable::HarnessError("compiler is borrowed".into()))?;
        let f = self.processor.replay_fields();
        let mut fields = IndexMap::<_, _>::default();
        fields.insert("compiler".into(), DslValue::Compiler(self.compiler.clone()));
        fields.insert("chunkGraph".into(), chunk_graph(&compiler));
        fields.insert(
            "providedNames".into(),
            DslValue::Map(
                f.provided_names
                    .iter()
                    .map(|(k, v)| {
                        (
                            DslValue::String(k.clone()),
                            provided_name(v, &self.compiler),
                        )
                    })
                    .collect(),
            ),
        );
        fields.insert(
            "exportedVariables".into(),
            DslValue::Set(
                f.exported_variables
                    .iter()
                    .map(|v| DslValue::String(v.clone()))
                    .collect(),
            ),
        );
        fields.insert(
            "preserveGoogProvidesAndRequires".into(),
            DslValue::Bool(*f.preserve_goog_provides_and_requires),
        );
        fields.insert(
            "requiresToBeRemoved".into(),
            DslValue::List(
                f.requires_to_be_removed
                    .iter()
                    .map(|n| DslValue::Node(*n))
                    .collect(),
            ),
        );
        fields.insert(
            "hasRewritingOccurred".into(),
            DslValue::Bool(*f.has_rewriting_occurred),
        );
        fields.insert(
            "forwardDeclaresToRemove".into(),
            DslValue::Set(
                f.forward_declares_to_remove
                    .iter()
                    .map(|n| DslValue::Node(*n))
                    .collect(),
            ),
        );
        fields.insert("astFactory".into(), ast_factory(f.ast_factory));
        Ok(fields)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ProcessClosureProvidesAndRequiresTest_Helpers#ProcessClosureProvidesAndRequiresTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        ProcessClosureProvidesAndRequiresTestHelpers {
            preserve_goog_provides_and_requires: false,
            last_processor: None,
        },
    ))))
}

// port: ProcessClosureProvidesAndRequiresTest_Helpers.GetProcessorLambda#GetProcessorLambda
pub fn get_processor_lambda_new(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(outer), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    if outer.borrow().class_name() != HOLDER {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(
        GetProcessorLambda {
            outer: outer.clone(),
            compiler: compiler.clone(),
        },
    ))))
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
