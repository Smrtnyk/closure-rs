/*
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/OptimizeCalls.java,
//   src/com/google/javascript/jscomp/PureFunctionIdentifier.java,
//   test/com/google/javascript/jscomp/OptimizeCallsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for OptimizeCalls (its Builder chain) and PureFunctionIdentifier.Driver, and
//! the port of the unit-corpus helper `OptimizeCallsTest_Helpers.java` (oracle/replay/helpers):
//! the CallGraphCompilerPass lambda of OptimizeCallsTest#getProcessor that stores the collected
//! ReferenceMap into the test field `references`.
use crate::{
    replay::replay_dsl::{Ctx, DslValue, NativeObject, Object},
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_scope::ImplicitVar,
    compiler_pass::CompilerPass,
    optimize_calls::{Builder, CallGraphCompilerPass, OptimizeCalls, ReferenceMap},
    pure_function_identifier::Driver,
    scope::ScopeId,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc};

const BUILDER: &str = "com.google.javascript.jscomp.OptimizeCalls$Builder";
const REFERENCES_CAPTURING_PASS: &str =
    "com.google.javascript.jscomp.OptimizeCallsTest_Helpers$ReferencesCapturingPass";
const CALL_GRAPH_COMPILER_PASS: &str =
    "com.google.javascript.jscomp.OptimizeCalls$CallGraphCompilerPass";

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!("{what} arguments"))
}

/// The DSL receiver for `OptimizeCalls.Builder`. Java's builder methods return `this`; the Rust
/// builder is consumed by value, so the adapter keeps it in an `Option` and returns itself.
struct BuilderObject {
    builder: Option<Builder>,
}

impl NativeObject for BuilderObject {
    fn class_name(&self) -> &str {
        BUILDER
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayDsl#invoke (instance receiver of OptimizeCalls.Builder)
fn with_builder(
    receiver: &DslValue,
    f: impl FnOnce(Builder) -> Result<Builder, Throwable>,
) -> Result<DslValue, Throwable> {
    let DslValue::Native(object) = receiver else {
        return Err(bad("OptimizeCalls$Builder receiver"));
    };
    {
        let mut borrowed = object.borrow_mut();
        let Some(this) = borrowed.as_any_mut().downcast_mut::<BuilderObject>() else {
            return Err(bad("OptimizeCalls$Builder receiver"));
        };
        let builder = this
            .builder
            .take()
            .ok_or_else(|| bad("OptimizeCalls$Builder (already built)"))?;
        this.builder = Some(f(builder)?);
    }
    Ok(receiver.clone())
}

// port: OptimizeCalls#builder
pub fn builder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(BuilderObject {
        builder: Some(OptimizeCalls::builder()),
    }))))
}

// port: OptimizeCalls.Builder#setCompiler
pub fn set_compiler(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [receiver, DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad("OptimizeCalls$Builder.setCompiler"));
    };
    with_builder(receiver, |b| Ok(b.set_compiler(&compiler.borrow())))
}

// port: OptimizeCalls.Builder#setConsiderExterns
pub fn set_consider_externs(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [receiver, value] = args.as_slice() else {
        return Err(bad("OptimizeCalls$Builder.setConsiderExterns"));
    };
    // Unboxing a null Boolean throws NullPointerException.
    let value = match value.untyped() {
        DslValue::Bool(b) => *b,
        DslValue::Null => {
            return Err(Throwable::Exception {
                class: "java.lang.NullPointerException".into(),
                message: None,
            });
        }
        _ => return Err(bad("OptimizeCalls$Builder.setConsiderExterns")),
    };
    with_builder(receiver, |b| Ok(b.set_consider_externs(value)))
}

/// Forwards `CallGraphCompilerPass#process` to a shared native helper, so the helper keeps its
/// identity for descriptor `once` and `testFieldsAfter`.
struct SharedCallGraphPass {
    pass: Rc<RefCell<dyn NativeObject>>,
}

impl CallGraphCompilerPass for SharedCallGraphPass {
    // port: OptimizeCalls.CallGraphCompilerPass#process (shared DSL helper receiver)
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
        references: &mut ReferenceMap,
    ) {
        let mut pass = self.pass.borrow_mut();
        let pass = pass.as_any_mut();
        if let Some(pass) =
            pass.downcast_mut::<crate::replay::optimize_calls_task_helpers::CallGraphPassObject>()
        {
            // A ported CallGraphCompilerPass, e.g. DevirtualizeMethods.
            CallGraphCompilerPass::process(pass, compiler, externs, root, references);
            return;
        }
        let Some(pass) = pass.downcast_mut::<ReferencesCapturingPass>() else {
            panic!("not a CallGraphCompilerPass helper");
        };
        CallGraphCompilerPass::process(pass, compiler, externs, root, references);
    }
}

// port: OptimizeCalls.Builder#addPass
pub fn add_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [receiver, DslValue::Native(pass)] = args.as_slice() else {
        return Err(bad("OptimizeCalls$Builder.addPass"));
    };
    if !pass.borrow().is_instance_of(CALL_GRAPH_COMPILER_PASS) {
        return Err(Throwable::Unported(format!(
            "{}#process",
            pass.borrow().class_name()
        )));
    }
    let pass = SharedCallGraphPass { pass: pass.clone() };
    with_builder(receiver, |b| Ok(b.add_pass(Box::new(pass))))
}

// port: OptimizeCalls.Builder#build
pub fn build(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(object)] = args.as_slice() else {
        return Err(bad("OptimizeCalls$Builder.build"));
    };
    let mut borrowed = object.borrow_mut();
    let Some(this) = borrowed.as_any_mut().downcast_mut::<BuilderObject>() else {
        return Err(bad("OptimizeCalls$Builder.build"));
    };
    let builder = this
        .builder
        .take()
        .ok_or_else(|| bad("OptimizeCalls$Builder (already built)"))?;
    let pass: Box<dyn CompilerPass> = Box::new(builder.build());
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: PureFunctionIdentifier.Driver#Driver
pub fn pure_function_identifier_driver(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad("PureFunctionIdentifier$Driver"));
    };
    let pass: Box<dyn CompilerPass> = Box::new(Driver::new());
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

/// OptimizeCallsTest_Helpers.ReferencesCapturingPass. Java stores the live ReferenceMap; nothing
/// changes the map or its global scope after the call-graph passes ran, so the adapter stores the
/// dump-ready view of it, taken while the compiler is at hand.
struct ReferencesCapturingPass {
    // Will be assigned with the references collected from the most recent pass.
    references: DslValue,
}

// port: OptimizeCallsTest_Helpers.ReferencesCapturingPass#ReferencesCapturingPass
pub fn references_capturing_pass(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        ReferencesCapturingPass {
            references: DslValue::Null,
        },
    ))))
}

impl CallGraphCompilerPass for ReferencesCapturingPass {
    // port: OptimizeCallsTest_Helpers.ReferencesCapturingPass#process
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        _externs: NodeId,
        _root: NodeId,
        references: &mut ReferenceMap,
    ) {
        self.references = reference_map_value(compiler, references);
    }
}

impl NativeObject for ReferencesCapturingPass {
    fn class_name(&self) -> &str {
        REFERENCES_CAPTURING_PASS
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == REFERENCES_CAPTURING_PASS || class == CALL_GRAPH_COMPILER_PASS
    }
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([(
            "references".to_string(),
            self.references.clone(),
        )]))
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: UnitRecorder#dump (object with its declared fields)
fn object(class: &str, fields: Vec<(&str, DslValue)>) -> DslValue {
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: class.into(),
        fields: fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        field_types: IndexMap::<_, _>::default(),
    })))
}

// port: UnitRecorder#dump (a Node by its runtime class)
fn node_value(compiler: &AbstractCompiler, n: NodeId) -> DslValue {
    DslValue::Typed {
        class: n.get_class(compiler).into(),
        value: Box::new(DslValue::Node(n)),
    }
}

// port: UnitRecorder#dump (OptimizeCalls.ReferenceMap: globalScope, names, props)
fn reference_map_value(compiler: &AbstractCompiler, references: &ReferenceMap) -> DslValue {
    let references_value = |map: &IndexMap<JsString, Vec<NodeId>>| {
        DslValue::Map(
            map.iter()
                .map(|(name, nodes)| {
                    (
                        DslValue::String(name.clone()),
                        DslValue::List(nodes.iter().map(|&n| node_value(compiler, n)).collect()),
                    )
                })
                .collect(),
        )
    };
    object(
        "com.google.javascript.jscomp.OptimizeCalls$ReferenceMap",
        vec![
            (
                "globalScope",
                references
                    .get_global_scope()
                    .map_or(DslValue::Null, |s| scope_value(compiler, s)),
            ),
            ("names", references_value(references.get_name_references())),
            ("props", references_value(references.get_prop_references())),
        ],
    )
}

// port: UnitRecorder#dump (Scope: parent, depth, AbstractScope.vars/implicitVars/rootNode)
fn scope_value(compiler: &AbstractCompiler, scope: ScopeId) -> DslValue {
    let var_ref = || object("com.google.javascript.jscomp.Var", vec![]);
    let vars = DslValue::Map(
        scope
            .get_var_iterable(compiler)
            .into_iter()
            .map(|v| (DslValue::String(v.get_name(compiler)), var_ref()))
            .collect(),
    );
    // AbstractScope#implicitVars starts as ImmutableMap.of() and becomes an EnumMap (ordinal
    // order) when the first implicit variable is made.
    let implicit: Vec<_> = [
        ImplicitVar::ARGUMENTS,
        ImplicitVar::EXPORTS,
        ImplicitVar::SUPER,
        ImplicitVar::THIS,
    ]
    .into_iter()
    .filter(|&v| scope.has_own_implicit_slot(compiler, Some(v)))
    .map(|v| {
        (
            DslValue::Enum {
                class: "com.google.javascript.jscomp.AbstractScope$ImplicitVar".into(),
                name: format!("{v:?}"),
            },
            var_ref(),
        )
    })
    .collect();
    let implicit_vars = DslValue::Typed {
        class: if implicit.is_empty() {
            "com.google.common.collect.RegularImmutableMap".into()
        } else {
            "java.util.EnumMap".into()
        },
        value: Box::new(DslValue::Map(implicit)),
    };
    let parent = scope
        .get_parent(compiler)
        .map_or(DslValue::Null, |p| scope_value(compiler, p));
    let root = scope.get_root_node(compiler);
    object(
        "com.google.javascript.jscomp.Scope",
        vec![
            ("parent", parent),
            ("depth", DslValue::Int(scope.get_depth(compiler))),
            ("AbstractScope.vars", vars),
            ("AbstractScope.implicitVars", implicit_vars),
            ("AbstractScope.rootNode", node_value(compiler, root)),
        ],
    )
}
