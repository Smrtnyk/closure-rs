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
 * Copyright 2021 The Closure Compiler Authors.
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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/serialization/JSTypeColorIdHasherTest.java,
//   test/com/google/javascript/jscomp/serialization/JSTypeReconserializerTest.java.

//! Ports of the replay helpers
//! `oracle/replay/helpers/.../serialization/JSTypeColorIdHasherTest_Helpers.java` and
//! `oracle/replay/helpers/.../serialization/JSTypeReconserializerTest_Helpers.java`, themselves
//! copied from JSTypeColorIdHasherTest.java (fields hasher, labelToColorId, colorIdToJSTypes;
//! LabeledTypeFinder; getProcessor) and JSTypeReconserializerTest.java (fields
//! shouldSerializeProperty, typePool, stringPoolBuilder, labelToPointer; getProcessor).
use crate::{
    jscomp_api::Compiler,
    proto_neutral,
    replay::replay_dsl::{self, Ctx, DslValue, NativeObject, Object},
    throwable::Throwable,
};
use closure_jscomp::{
    invalidating_types,
    node_traversal::{
        AbstractPostOrderCallback, AbstractPostOrderCallbackInterface, NodeTraversal,
    },
    serialization::{
        js_type_color_id_hasher::JSTypeColorIdHasher,
        js_type_reconserializer::JSTypeReconserializer,
        serialization_options::SerializationOptions,
        string_pool::{StringPool, StringPoolBuilder},
        types_proto::TypePool,
    },
    type_mismatch::TypeMismatch,
};
use closure_jstype::prelude::*;
use closure_rhino::{js_string::JsString, jscomp_colors::color_id::ColorId, node::NodeId};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const HASHER_HOLDER: &str =
    "com.google.javascript.jscomp.serialization.JSTypeColorIdHasherTest_Helpers";
const RECONSERIALIZER_HOLDER: &str =
    "com.google.javascript.jscomp.serialization.JSTypeReconserializerTest_Helpers";

/// `ColorId` as the recorder dumps it (`{"object": ColorId, "fields": {"rightAligned": long}}`).
fn color_id_value(id: ColorId) -> DslValue {
    let mut fields = IndexMap::new();
    fields.insert("rightAligned".into(), DslValue::Long(id.right_aligned()));
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: "com.google.javascript.jscomp.colors.ColorId".into(),
        fields,
        field_types: IndexMap::new(),
    })))
}

/// `LinkedHashMultimap<ColorId, JSType> colorIdToJSTypes` (shared with the traversal callback).
type ColorIdToJSTypes = Rc<RefCell<IndexMap<ColorId, Vec<TypeId>>>>;

/// `final class JSTypeColorIdHasherTest_Helpers`.
pub struct JSTypeColorIdHasherTestHelpers {
    hasher: Option<Rc<JSTypeColorIdHasher>>,
    label_to_color_id: Option<Rc<RefCell<IndexMap<JsString, ColorId>>>>,
    // Useful for debugging.
    color_id_to_js_types: Option<ColorIdToJSTypes>,
}

impl NativeObject for JSTypeColorIdHasherTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HASHER_HOLDER
    }
    // port: ReplayValues#findField (native object adapter). `hasher` and `colorIdToJSTypes` are
    // skipped by the descriptor (testFieldsAfterSkip): they hold Java-internal JSType graphs.
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert(
            "labelToColorId".into(),
            match &self.label_to_color_id {
                None => DslValue::Null,
                Some(map) => DslValue::Map(
                    map.borrow()
                        .iter()
                        .map(|(label, id)| (DslValue::String(label.clone()), color_id_value(*id)))
                        .collect(),
                ),
            },
        );
        Ok(fields)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: JSTypeColorIdHasherTest_Helpers#JSTypeColorIdHasherTest_Helpers
pub fn hasher_holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        JSTypeColorIdHasherTestHelpers {
            hasher: None,
            label_to_color_id: None,
            color_id_to_js_types: None,
        },
    ))))
}

// port: JSTypeColorIdHasherTest_Helpers#getProcessor
pub fn hasher_get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut this = this.borrow_mut();
    let this = this
        .as_any_mut()
        .downcast_mut::<JSTypeColorIdHasherTestHelpers>()
        .ok_or_else(bad)?;
    let hasher = {
        let mut compiler = compiler.borrow_mut();
        let (registry, _ast) = compiler.get_type_registry_and_ast();
        Rc::new(JSTypeColorIdHasher::new(registry))
    };
    let label_to_color_id = Rc::new(RefCell::new(IndexMap::new()));
    let color_id_to_js_types = Rc::new(RefCell::new(IndexMap::new()));
    this.hasher = Some(hasher.clone());
    this.label_to_color_id = Some(label_to_color_id.clone());
    this.color_id_to_js_types = Some(color_id_to_js_types.clone());
    Ok(DslValue::Native(Rc::new(RefCell::new(HasherProcessor {
        hasher,
        label_to_color_id,
        color_id_to_js_types,
    }))))
}

/// `private class LabeledTypeFinder extends AbstractPostOrderCallback`.
struct LabeledTypeFinder {
    hasher: Rc<JSTypeColorIdHasher>,
    label_to_color_id: Rc<RefCell<IndexMap<JsString, ColorId>>>,
    color_id_to_js_types: ColorIdToJSTypes,
    /// Rust-only: the exception thrown inside the traversal (reported after it).
    failure: Rc<RefCell<Option<Throwable>>>,
}

impl AbstractPostOrderCallbackInterface for LabeledTypeFinder {
    // port: JSTypeColorIdHasherTest_Helpers.LabeledTypeFinder#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        if !n.is_label(compiler) || self.failure.borrow().is_some() {
            return;
        }

        let label = n.get_first_child(compiler).unwrap().get_string(compiler);
        let r#type = n
            .get_second_child(compiler)
            .unwrap()
            .get_first_child(compiler)
            .unwrap()
            .get_jstype(compiler);
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let r#type = r#type.and_then(|r#type| r#type.to_maybe_object_type(registry));
        let Some(r#type) = r#type else {
            *self.failure.borrow_mut() = Some(Throwable::Exception {
                class: "java.lang.NullPointerException".into(),
                message: None,
            });
            return;
        };
        let id = self.hasher.hash_object_type(registry, ast, r#type);

        // assertThat(labelToColorId).doesNotContainKey(label);
        if self.label_to_color_id.borrow().contains_key(&label) {
            *self.failure.borrow_mut() = Some(Throwable::Assertion {
                message: format!("expected not to contain key: {label}"),
            });
            return;
        }
        self.label_to_color_id.borrow_mut().insert(label, id);
        // LinkedHashMultimap#put
        let mut multimap = self.color_id_to_js_types.borrow_mut();
        let types = multimap.entry(id).or_default();
        if !types.contains(&r#type) {
            types.push(r#type);
        }
    }
}

/// The lambda `(Node externs, Node root) -> NodeTraversal.traverseRoots(compiler, new
/// LabeledTypeFinder(), externs, root)` returned by getProcessor.
struct HasherProcessor {
    hasher: Rc<JSTypeColorIdHasher>,
    label_to_color_id: Rc<RefCell<IndexMap<JsString, ColorId>>>,
    color_id_to_js_types: ColorIdToJSTypes,
}

impl NativeObject for HasherProcessor {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.serialization.JSTypeColorIdHasherTest_Helpers$$Lambda"
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: JSTypeColorIdHasherTest_Helpers#getProcessor (the CompilerPass lambda)
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let failure = Rc::new(RefCell::new(None));
        let mut callback = AbstractPostOrderCallback::new(LabeledTypeFinder {
            hasher: self.hasher.clone(),
            label_to_color_id: self.label_to_color_id.clone(),
            color_id_to_js_types: self.color_id_to_js_types.clone(),
            failure: failure.clone(),
        });
        NodeTraversal::traverse_roots(compiler, &mut callback, externs, root);
        match failure.borrow_mut().take() {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `final class JSTypeReconserializerTest_Helpers`.
pub struct JSTypeReconserializerTestHelpers {
    should_serialize_property: DslValue,
    type_pool: Rc<RefCell<Option<TypePool>>>,
    string_pool_builder: Option<Rc<RefCell<StringPoolBuilder>>>,
    label_to_pointer: Option<Rc<RefCell<IndexMap<JsString, i32>>>>,
}

impl NativeObject for JSTypeReconserializerTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        RECONSERIALIZER_HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert(
            "shouldSerializeProperty".into(),
            self.should_serialize_property.clone(),
        );
        fields.insert(
            "typePool".into(),
            match &*self.type_pool.borrow() {
                None => DslValue::Null,
                Some(pool) => DslValue::Proto(
                    proto_neutral::to_proto_message("jscomp.TypePool", pool)
                        .map_err(Throwable::HarnessError)?,
                ),
            },
        );
        fields.insert(
            "stringPoolBuilder".into(),
            match &self.string_pool_builder {
                None => DslValue::Null,
                Some(builder) => {
                    let builder = builder.borrow();
                    let mut fields = IndexMap::new();
                    fields.insert("maxLength".into(), DslValue::Int(builder.max_length()));
                    fields.insert(
                        "pool".into(),
                        DslValue::Map(
                            builder
                                .pool()
                                .iter()
                                .map(|(s, i)| (DslValue::String(s.clone()), DslValue::Int(*i)))
                                .collect(),
                        ),
                    );
                    DslValue::Object(Rc::new(RefCell::new(Object {
                        class: "com.google.javascript.jscomp.serialization.StringPool$Builder"
                            .into(),
                        fields,
                        field_types: IndexMap::new(),
                    })))
                }
            },
        );
        fields.insert(
            "labelToPointer".into(),
            match &self.label_to_pointer {
                None => DslValue::Null,
                Some(map) => DslValue::Map(
                    map.borrow()
                        .iter()
                        .map(|(label, pointer)| {
                            (DslValue::String(label.clone()), DslValue::Int(*pointer))
                        })
                        .collect(),
                ),
            },
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match name {
            "shouldSerializeProperty" => {
                self.should_serialize_property = value;
                Ok(())
            }
            _ => Err(Throwable::Unported(format!(
                "{RECONSERIALIZER_HOLDER}#{name}"
            ))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: JSTypeReconserializerTest_Helpers#JSTypeReconserializerTest_Helpers
pub fn reconserializer_holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        JSTypeReconserializerTestHelpers {
            should_serialize_property: DslValue::Null,
            type_pool: Rc::new(RefCell::new(None)),
            string_pool_builder: None,
            label_to_pointer: None,
        },
    ))))
}

// port: JSTypeReconserializerTest_Helpers#getProcessor
pub fn reconserializer_get_processor(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut this = this.borrow_mut();
    let this = this
        .as_any_mut()
        .downcast_mut::<JSTypeReconserializerTestHelpers>()
        .ok_or_else(bad)?;
    let string_pool_builder = Rc::new(RefCell::new(StringPool::builder()));
    let label_to_pointer = Rc::new(RefCell::new(IndexMap::new()));
    this.string_pool_builder = Some(string_pool_builder.clone());
    this.label_to_pointer = Some(label_to_pointer.clone());
    let DslValue::Lambda(predicate) = &this.should_serialize_property else {
        return Err(Throwable::Unported(format!(
            "{RECONSERIALIZER_HOLDER}#shouldSerializeProperty of class {}",
            this.should_serialize_property.class_name()
        )));
    };
    // The Predicate<String> is a DSL lambda: it runs in its own evaluation context.
    let predicate_ctx = Ctx::new(
        ctx.descriptor.clone(),
        ctx.record.clone(),
        ctx.class_map.clone(),
        ctx.registry.clone(),
    );
    Ok(DslValue::Native(Rc::new(RefCell::new(
        ReconserializerProcessor {
            should_serialize_property: predicate.clone(),
            predicate_ctx: Rc::new(RefCell::new(predicate_ctx)),
            type_pool: this.type_pool.clone(),
            string_pool_builder,
            label_to_pointer,
        },
    ))))
}

/// The lambda `(externs, root) -> { ... }` returned by getProcessor.
struct ReconserializerProcessor {
    should_serialize_property: Rc<replay_dsl::Lambda>,
    predicate_ctx: Rc<RefCell<Ctx>>,
    type_pool: Rc<RefCell<Option<TypePool>>>,
    string_pool_builder: Rc<RefCell<StringPoolBuilder>>,
    label_to_pointer: Rc<RefCell<IndexMap<JsString, i32>>>,
}

impl NativeObject for ReconserializerProcessor {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.serialization.JSTypeReconserializerTest_Helpers$$Lambda"
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: JSTypeReconserializerTest_Helpers#getProcessor (the CompilerPass lambda)
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let predicate_failure: Rc<RefCell<Option<Throwable>>> = Rc::new(RefCell::new(None));
        let mismatches: Vec<TypeMismatch> = compiler.get_type_mismatches().to_vec();
        let mut serializer = {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let invalidating_types = invalidating_types::Builder::new(registry)
                .add_all_type_mismatches(registry, ast, &mismatches)
                .build(registry, ast);
            let lambda = self.should_serialize_property.clone();
            let predicate_ctx = self.predicate_ctx.clone();
            let failure = predicate_failure.clone();
            JSTypeReconserializer::create(
                registry,
                invalidating_types,
                self.string_pool_builder.clone(),
                Box::new(move |property_name: &JsString| {
                    let result = replay_dsl::invoke_lambda(
                        &lambda,
                        vec![DslValue::String(property_name.clone())],
                        &mut predicate_ctx.borrow_mut(),
                    );
                    match result {
                        Ok(DslValue::Bool(b)) => b,
                        Ok(_) => {
                            failure.borrow_mut().get_or_insert(bad());
                            false
                        }
                        Err(e) => {
                            failure.borrow_mut().get_or_insert(e);
                            false
                        }
                    }
                }),
                SerializationOptions::builder()
                    .set_include_debug_info(true)
                    .set_run_validation(true)
                    .build(),
            )
        };

        let label_to_pointer = self.label_to_pointer.clone();
        let mut visit = |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| {
            let compiler = t.get_compiler();
            if let Some(r#type) = n.get_jstype(compiler) {
                let (registry, ast) = compiler.get_type_registry_and_ast();
                let pointer = serializer.serialize_type(registry, ast, r#type);

                let parent = n.get_parent(compiler).unwrap();
                if parent.is_expr_result(compiler)
                    && n.get_grandparent(compiler).unwrap().is_label(compiler)
                {
                    let label_name = parent.get_previous(compiler).unwrap().get_string(compiler);
                    label_to_pointer.borrow_mut().insert(label_name, pointer);
                }
            }
        };
        let mut callback = AbstractPostOrderCallback::new(&mut visit);
        NodeTraversal::traverse_roots(compiler, &mut callback, externs, root);

        let (registry, ast) = compiler.get_type_registry_and_ast();
        *self.type_pool.borrow_mut() = Some(serializer.generate_type_pool(registry, ast));
        match predicate_failure.borrow_mut().take() {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
