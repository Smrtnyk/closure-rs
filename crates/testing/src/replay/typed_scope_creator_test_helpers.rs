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
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   test/com/google/javascript/jscomp/TypedScopeCreatorTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../TypedScopeCreatorTest_Helpers.java`
//! (DSL name `TypedScopeCreatorTest_Helpers`), itself copied from TypedScopeCreatorTest.java: the
//! holder fields `registry` .. `labeledStatementMap`, `LabeledStatement`, `ScopeFinder` and
//! `getProcessor`. Also the recorder's `typedScope` dump of the scopes these fields hold
//! (oracle/patches/0002-recording-hooks.patch, UnitRecorder#typedScope).
use crate::{
    json::JsonValue,
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    gather_module_metadata::GatherModuleMetadata,
    infer_consts::InferConsts,
    modules::module_map_creator::ModuleMapCreator,
    node_traversal::{Callback, NodeTraversal},
    type_inference_pass::{SharedTypedScopeCreator, TypeInferencePass},
    typed_scope::TypedScope,
    typed_scope_creator::TypedScopeCreator,
};
use closure_jstype::js_type::JSType;
use closure_rhino::{js_string::JsString, node::NodeId};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.TypedScopeCreatorTest_Helpers";
const LABELED_STATEMENT: &str =
    "com.google.javascript.jscomp.TypedScopeCreatorTest_Helpers$LabeledStatement";

/// The state the test instance holds. `registry` is not modelled: the descriptor skips it
/// (testFieldsAfterSkip) and nothing reads it back.
#[derive(Default)]
struct State {
    compiler: Option<CompilerHandle>,
    global_scope: Option<TypedScope>,
    last_local_scope: Option<TypedScope>,
    last_function_scope: Option<TypedScope>,
    /// `Map<String, LabeledStatement>` (a HashMap), in insertion order; dumped as a HashMap.
    labeled_statement_map: Option<IndexMap<JsString, LabeledStatement>>,
}

/// Stores information about a labeled statement and allows making assertions on it.
// port: TypedScopeCreatorTest_Helpers.LabeledStatement
#[derive(Clone, Copy)]
struct LabeledStatement {
    statement_node: NodeId,
    enclosing_scope: TypedScope,
}

/// The helper holder (one instance plays the test instance).
pub struct TypedScopeCreatorTestHelpers {
    state: Rc<RefCell<State>>,
}

/// A `TypedScope` as a DSL value: the recorder dumps it under its own `typedScope` tag.
struct NativeTypedScope {
    scope: TypedScope,
    compiler: CompilerHandle,
}

struct NativeLabeledStatement {
    statement: LabeledStatement,
    compiler: CompilerHandle,
}

impl NativeObject for TypedScopeCreatorTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let state = self.state.borrow();
        let scope = |scope: Option<TypedScope>| -> Result<DslValue, Throwable> {
            Ok(match scope {
                None => DslValue::Null,
                Some(scope) => native_scope(scope, state.compiler.clone().ok_or_else(bad)?),
            })
        };
        let mut fields = IndexMap::new();
        fields.insert("globalScope".into(), scope(state.global_scope)?);
        fields.insert("lastLocalScope".into(), scope(state.last_local_scope)?);
        fields.insert(
            "lastFunctionScope".into(),
            scope(state.last_function_scope)?,
        );
        let map = match &state.labeled_statement_map {
            None => DslValue::Null,
            Some(map) => {
                let compiler = state.compiler.clone().ok_or_else(bad)?;
                DslValue::Typed {
                    class: "java.util.HashMap".into(),
                    value: Box::new(DslValue::Map(
                        map.iter()
                            .map(|(k, v)| {
                                (
                                    DslValue::String(k.clone()),
                                    DslValue::Native(Rc::new(RefCell::new(
                                        NativeLabeledStatement {
                                            statement: *v,
                                            compiler: compiler.clone(),
                                        },
                                    ))),
                                )
                            })
                            .collect(),
                    )),
                }
            }
        };
        fields.insert("labeledStatementMap".into(), map);
        Ok(fields)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl NativeObject for NativeLabeledStatement {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        LABELED_STATEMENT
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert(
            "statementNode".into(),
            DslValue::Node(self.statement.statement_node),
        );
        fields.insert(
            "enclosingScope".into(),
            native_scope(self.statement.enclosing_scope, self.compiler.clone()),
        );
        Ok(fields)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl NativeObject for NativeTypedScope {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.TypedScope"
    }
    // port: UnitRecorder#typedScope
    fn tagged_dump(&self) -> Result<Option<JsonValue>, Throwable> {
        let mut compiler = self.compiler.borrow_mut();
        Ok(Some(object([(
            "typedScope",
            typed_scope(&mut compiler, self.scope),
        )])))
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn native_scope(scope: TypedScope, compiler: CompilerHandle) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativeTypedScope { scope, compiler })))
}

/// A TypedScope (TypedScopeCreatorTest's globalScope/lastLocalScope/lastFunctionScope, which the
/// tests assert on after the hooked call): every own variable as [name, type string or null,
/// isTypeInferred], in the scope's iteration order, plus the root token and the type of this.
// port: UnitRecorder#typedScope
fn typed_scope(compiler: &mut crate::jscomp_api::Compiler, s: TypedScope) -> JsonValue {
    let root = s.get_root_node(compiler);
    let root_token = JsonValue::str(&root.get_token(compiler).to_string());
    let depth = JsonValue::int(i64::from(s.get_depth(compiler)));
    // Java catches a RuntimeException from getTypeOfThis ("no this type").
    let tt = s.get_type_of_this(compiler);
    let type_of_this = match tt {
        None => JsonValue::Null,
        Some(tt) => {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            truncate_type(&tt.to_string(reg, ast))
        }
    };
    let mut vars = vec![];
    for v in s.get_var_iterable(compiler) {
        let name = v.get_name(compiler);
        let t = v.get_type(compiler);
        let type_string = match t {
            None => JsonValue::Null,
            Some(t) => {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                truncate_type(&t.to_string(reg, ast))
            }
        };
        vars.push(JsonValue::Array(vec![
            JsonValue::String(crate::json::JsString(name.as_units().to_vec())),
            type_string,
            JsonValue::Bool(v.is_type_inferred(compiler)),
        ]));
    }
    object([
        ("rootToken", root_token),
        ("depth", depth),
        ("typeOfThis", type_of_this),
        ("vars", JsonValue::Array(vars)),
    ])
}

// port: UnitRecorder#truncateType
fn truncate_type(t: &str) -> JsonValue {
    let units = t.encode_utf16().collect::<Vec<_>>();
    if units.len() <= 2000 {
        JsonValue::String(crate::json::JsString(units))
    } else {
        let mut out = units[..2000].to_vec();
        out.extend(format!("...<truncated {} chars>", units.len() - 2000).encode_utf16());
        JsonValue::String(crate::json::JsString(out))
    }
}

fn object<const N: usize>(entries: [(&str, JsonValue); N]) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

// port: TypedScopeCreatorTest_Helpers.ScopeFinder
struct ScopeFinder {
    state: Rc<RefCell<State>>,
}

impl Callback for ScopeFinder {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: TypedScopeCreatorTest_Helpers.ScopeFinder#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let scope = t.get_typed_scope();
        let compiler = t.get_compiler();
        let mut state = self.state.borrow_mut();
        if scope.is_global(compiler) {
            state.global_scope = Some(scope);
        } else if scope.is_block_scope(compiler) {
            // TODO(bradfordcsmith): use labels to find scopes instead of lastLocalScope
            state.last_local_scope = Some(scope);
        } else if scope.is_function_scope(compiler) {
            state.last_function_scope = Some(scope);
        }
        if let Some(parent) = parent
            && parent.is_label(compiler)
            && !n.is_label_name(compiler)
        {
            // First child of a LABEL is a LABEL_NAME, n is the second child.
            let label_name_node = n.get_previous(compiler).expect("NullPointerException");
            assert!(
                label_name_node.is_label_name(compiler),
                "IllegalStateException"
            );
            let label_name = label_name_node.get_string(compiler).clone();
            let map = state
                .labeled_statement_map
                .as_mut()
                .expect("NullPointerException");
            assert!(
                !map.contains_key(&label_name),
                "Duplicate label name: {label_name}"
            );
            map.insert(
                label_name,
                LabeledStatement {
                    statement_node: n,
                    enclosing_scope: scope,
                },
            );
        }
    }
}

/// The CompilerPass lambda `getProcessor` returns.
struct Processor {
    state: Rc<RefCell<State>>,
}

impl NativeObject for Processor {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.TypedScopeCreatorTest_Helpers$$Lambda"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: UnitRecorder#collect (the processor's fields)
    // The lambda captures the holder and the compiler; neither is a recorded result producer.
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: TypedScopeCreatorTest_Helpers#getProcessor (the returned CompilerPass)
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        GatherModuleMetadata::new(false, ResolutionMode::BROWSER).process(compiler, externs, root);
        let module_metadata_map = compiler
            .get_module_metadata_map()
            .cloned()
            .expect("NullPointerException");
        ModuleMapCreator::new(module_metadata_map).process(compiler, externs, root);
        InferConsts::new(compiler).process(compiler, externs, root);

        let scope_creator = TypedScopeCreator::new(compiler);
        let reverse_interpreter = compiler.get_reverse_abstract_interpreter();
        let mut pass = TypeInferencePass::new(compiler, reverse_interpreter, scope_creator);
        let global_root = root.get_parent(compiler).expect("NullPointerException");
        pass.infer_all_scopes(compiler, global_root);
        let scope_creator = RefCell::new(pass.into_scope_creator());
        let mut finder = ScopeFinder {
            state: self.state.clone(),
        };
        let mut creator = SharedTypedScopeCreator(&scope_creator);
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(&mut finder)
            .set_scope_creator(&mut creator)
            .traverse_roots(externs, root);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: TypedScopeCreatorTest_Helpers#TypedScopeCreatorTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        TypedScopeCreatorTestHelpers {
            state: Rc::new(RefCell::new(State::default())),
        },
    ))))
}

// port: TypedScopeCreatorTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(receiver), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut receiver = receiver.borrow_mut();
    let receiver = receiver
        .as_any_mut()
        .downcast_mut::<TypedScopeCreatorTestHelpers>()
        .ok_or_else(bad)?;
    {
        // registry = compiler.getTypeRegistry();
        let mut state = receiver.state.borrow_mut();
        state.compiler = Some(compiler.clone());
        // Create a fresh statement map for each test case.
        state.labeled_statement_map = Some(IndexMap::new());
    }
    compiler.borrow_mut().get_type_registry();
    Ok(DslValue::Native(Rc::new(RefCell::new(Processor {
        state: receiver.state.clone(),
    }))))
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
