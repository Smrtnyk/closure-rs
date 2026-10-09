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
/*
 * Copyright (C) 2008 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/serialization/SerializeTypesToPointers.java.
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/Gson.java.

//! Port of serialization/SerializeTypesToPointers.java.
use super::js_type_reconserializer::JSTypeReconserializer;
use super::serialization_options::SerializationOptions;
use super::string_pool::StringPoolBuilder;
use super::type_pointers::{OFFSET_TO_AXIOMATIC_COLOR, TypePointers};
use super::types_proto::{TypePool, TypeProtoKindCase};
use crate::abstract_compiler::AbstractCompiler;
use crate::diagnostic::log_file::StreamedJsonProducer;
use crate::invalidating_types;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::type_mismatch::TypeMismatch;
use closure_jstype::prelude::*;
use closure_rhino::check_state;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_colors::color_id::ColorId;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_sourcemap::gson::stream::json_writer::JsonWriter;
use std::cell::RefCell;
use std::rc::Rc;

/// port: SerializeTypesToPointers
///
/// Grab an integer TypePool pointer for each JSType on the AST and log information about the
/// pointers.
pub struct SerializeTypesToPointers {
    jstype_reconserializer: JSTypeReconserializer,
    properties_referenced_in_ast: Rc<RefCell<IndexSet<JsString>>>,
    type_pointers_by_jstype: IndexMap<TypeId, i32>,
    type_pool: Option<TypePool>,
}

impl SerializeTypesToPointers {
    // port: SerializeTypesToPointers#<init>
    fn new(
        jstype_reconserializer: JSTypeReconserializer,
        properties_referenced_in_ast: Rc<RefCell<IndexSet<JsString>>>,
    ) -> Self {
        Self {
            jstype_reconserializer,
            properties_referenced_in_ast,
            type_pointers_by_jstype: IndexMap::<_, _>::default(),
            type_pool: None,
        }
    }

    // port: SerializeTypesToPointers#create
    pub fn create(
        compiler: &mut AbstractCompiler,
        string_pool_builder: &Rc<RefCell<StringPoolBuilder>>,
        serialization_options: SerializationOptions,
    ) -> Self {
        let mismatches: Vec<TypeMismatch> = compiler.get_type_mismatches().to_vec();
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let invalidating_types = invalidating_types::Builder::new(registry)
            .add_all_type_mismatches(registry, ast, &mismatches)
            .build(registry, ast);

        // this set requires access to the externs and src ASTs, so can't be populated yet.
        let properties_referenced_in_ast: Rc<RefCell<IndexSet<JsString>>> =
            Rc::new(RefCell::new(IndexSet::<_>::default()));
        let contains = Rc::clone(&properties_referenced_in_ast);
        let js_type_reconserializer = JSTypeReconserializer::create(
            registry,
            invalidating_types,
            Rc::clone(string_pool_builder),
            Box::new(move |s: &JsString| contains.borrow().contains(s)),
            serialization_options,
        );
        Self::new(js_type_reconserializer, properties_referenced_in_ast)
    }

    // port: SerializeTypesToPointers#gatherTypesOnAst
    pub fn gather_types_on_ast(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        check_state!(self.type_pool.is_none(), "Cannot call process() twice");
        NodeTraversal::traverse(
            compiler,
            root,
            &mut PropertySearchCallback {
                properties_referenced_in_ast: &self.properties_referenced_in_ast,
            },
        );
        NodeTraversal::traverse(
            compiler,
            root,
            &mut TypeSearchCallback {
                jstype_reconserializer: &mut self.jstype_reconserializer,
                type_pointers_by_jstype: &mut self.type_pointers_by_jstype,
            },
        );

        // these types are only used when debug logging is enabled, but we always serialize them as
        // not to have a different TypePool with and without debug logging.
        let mismatches: Vec<TypeMismatch> = compiler.get_type_mismatches().to_vec();
        let (registry, ast) = compiler.get_type_registry_and_ast();
        for mismatch in &mismatches {
            self.jstype_reconserializer
                .serialize_type(registry, ast, mismatch.found());
            self.jstype_reconserializer
                .serialize_type(registry, ast, mismatch.required());
        }

        let type_pool = self
            .jstype_reconserializer
            .generate_type_pool(registry, ast);
        self.type_pool = Some(type_pool);

        self.log_serialization_debug_info(compiler);
    }

    // port: SerializeTypesToPointers#getTypePointersByJstype
    pub fn get_type_pointers_by_jstype(&self) -> &IndexMap<TypeId, i32> {
        &self.type_pointers_by_jstype
    }

    // port: SerializeTypesToPointers#getTypePool
    pub fn get_type_pool(&self) -> Option<&TypePool> {
        self.type_pool.as_ref()
    }

    // port: SerializeTypesToPointers#logSerializationDebugInfo
    fn log_serialization_debug_info(&mut self, compiler: &mut AbstractCompiler) {
        // Log information about how the JSTypes correspond to the colors. This may be useful later
        // on in optimizations.
        {
            let mut log = compiler.create_or_reopen_log(
                "com.google.javascript.jscomp.serialization.SerializeTypesToPointers",
                "object_uuids.log",
                &[],
            );
            if log.is_logging() {
                let all_serialized_types = self
                    .jstype_reconserializer
                    .get_color_id_to_jstype_map_for_debugging();
                // Stream json writing here rather than building up the entire json representation
                // at once because the latter used to cause OOMs.
                let (registry, ast) = compiler.get_type_registry_and_ast();
                let mut type_strings: IndexMap<String, Vec<String>> = IndexMap::<_, _>::default();
                for (color_id, jstypes) in &all_serialized_types {
                    type_strings.insert(
                        color_id.clone(),
                        jstypes.iter().map(|t| t.to_string(registry, ast)).collect(),
                    );
                }
                log.log_json(&StreamObjectUuidsJson::new(type_strings));
            }
            log.close();
        }

        // Log type mismatches, which contribute to the definition of an "invalidating" type
        {
            let mut log = compiler.create_or_reopen_log(
                "com.google.javascript.jscomp.serialization.SerializeTypesToPointers",
                "mismatches.log",
                &[],
            );
            if log.is_logging() {
                let mismatches: Vec<TypeMismatch> = compiler.get_type_mismatches().to_vec();
                let (registry, ast) = compiler.get_type_registry_and_ast();
                let json = gson_to_json(&Self::log_type_mismatches(
                    &mismatches,
                    &mut self.jstype_reconserializer,
                    registry,
                    ast,
                    self.type_pool.as_ref().unwrap(),
                ));
                log.log(&mut || json.clone());
            }
            log.close();
        }
    }

    /// Serializes a type not necessarily attached to an AST node.
    ///
    /// Not part of the main API for this callback. For use when serializing additional types for
    /// debug logging.
    // port: SerializeTypesToPointers#logTypeMismatches
    fn log_type_mismatches(
        type_mismatches: &[TypeMismatch],
        serializer: &mut JSTypeReconserializer,
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        type_pool: &TypePool,
    ) -> Vec<TypeMismatchJson> {
        let mut sorted: Vec<TypeMismatchJson> = Vec::new();
        for mismatch in type_mismatches {
            let json = TypeMismatchJson::create(mismatch, serializer, registry, ast, type_pool);
            // ImmutableSortedSet drops elements that compare equal.
            if let Err(index) = sorted.binary_search_by(|e| e.compare_to(&json)) {
                sorted.insert(index, json);
            }
        }
        sorted
    }
}

/// `new Gson().toJson(ImmutableSortedSet<TypeMismatchJson>)`: an array of objects with the
/// declared fields in order.
// port: com.google.gson.Gson#toJson(Object)
fn gson_to_json(mismatches: &[TypeMismatchJson]) -> String {
    let mut writer = JsonWriter::new(Vec::new());
    writer.set_html_safe(true);
    writer.set_serialize_nulls(false);
    // Gson#toJson(Object,Type,JsonWriter) writes leniently.
    writer.set_lenient(true);
    writer.begin_array();
    for mismatch in mismatches {
        writer.begin_object();
        writer.name("location");
        writer.value(Some(JsString::from(mismatch.location.as_str())));
        writer.name("foundColorId");
        writer.value(Some(JsString::from(mismatch.found_color_id.as_str())));
        writer.name("requiredColorId");
        writer.value(Some(JsString::from(mismatch.required_color_id.as_str())));
        writer.end_object();
    }
    writer.end_array();
    writer.into_string().to_string_lossy()
}

/// port: SerializeTypesToPointers.PropertySearchCallback
///
/// Finds all unquoted property names referenced outside @typeSummary files
///
/// Only property names found in this traversal will be serialized onto a color's list of "own
/// properties". Properties referenced only inside `@typeSummary` files may be excluded because
/// they don't matter for optimization of this library's srcs.
struct PropertySearchCallback<'a> {
    properties_referenced_in_ast: &'a Rc<RefCell<IndexSet<JsString>>>,
}

impl Callback for PropertySearchCallback<'_> {
    // port: SerializeTypesToPointers.PropertySearchCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let ast: &Ast = t.get_compiler();
        !n.is_script(ast) || !NodeUtil::is_from_type_summary(ast, n)
    }

    // port: SerializeTypesToPointers.PropertySearchCallback#visit
    #[allow(clippy::collapsible_match)] // Java's switch case with its nested if
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let ast: &Ast = t.get_compiler();
        match n.get_token(ast) {
            // "name" from (someObject.name)
            // "name" from (someObject?.name)
            Token::GETPROP | Token::OPTCHAIN_GETPROP => {
                self.properties_referenced_in_ast
                    .borrow_mut()
                    .insert(n.get_string(ast).clone());
            }
            // "name" from obj = {name: 0}
            // "name" from class C { name() {} }
            // "name" from class C { name = 0; }
            // "name" from class C { get name() {} }
            // "name" from class C { set name(n) {} }
            Token::STRING_KEY
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::GETTER_DEF
            | Token::SETTER_DEF => {
                if !n.is_quoted_string_key(ast) {
                    self.properties_referenced_in_ast
                        .borrow_mut()
                        .insert(n.get_string(ast).clone());
                }
            }
            _ => {}
        }
    }
}

/// port: SerializeTypesToPointers.TypeSearchCallback
struct TypeSearchCallback<'a> {
    jstype_reconserializer: &'a mut JSTypeReconserializer,
    type_pointers_by_jstype: &'a mut IndexMap<TypeId, i32>,
}

impl Callback for TypeSearchCallback<'_> {
    // port: SerializeTypesToPointers.TypeSearchCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let ast: &Ast = t.get_compiler();
        !n.is_script(ast) || !NodeUtil::is_from_type_summary(ast, n)
    }

    // port: SerializeTypesToPointers.TypeSearchCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        if n.is_root(compiler) {
            // the ROOT node is given the 'global this' type for use during typechecking, but this
            // is not needed for optimizations. (The 'global this' type will still be serialized if
            // referenced in actual code though.)
            return;
        }
        let r#type = n.get_jstype(compiler);
        if let Some(r#type) = r#type
            && !self.type_pointers_by_jstype.contains_key(&r#type)
        {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let pointer = self
                .jstype_reconserializer
                .serialize_type(registry, ast, r#type);
            self.type_pointers_by_jstype.insert(r#type, pointer);
        }
        let shadow = n.get_closure_unaware_shadow(compiler);
        if let Some(shadow) = shadow {
            // Shadow roots are structured as
            // ROOT -> SCRIPT -> EXPR_RESULT -> FUNCTION
            let function = shadow
                .get_first_first_child(compiler)
                .unwrap()
                .get_first_child(compiler)
                .unwrap();
            NodeTraversal::traverse(
                compiler,
                function,
                &mut TypeSearchCallback {
                    jstype_reconserializer: &mut *self.jstype_reconserializer,
                    type_pointers_by_jstype: &mut *self.type_pointers_by_jstype,
                },
            );
        }
    }
}

/// port: SerializeTypesToPointers.StreamObjectUuidsJson
///
/// Writes a JSON object whose keys are color ids and values are arrays of JSType strings.
///
/// Example: `{0: ['*', '?', 'None'], 10f34lksdf: ['SomeProtoCtor']}`
///
/// Java holds the JSTypes and renders them while writing; Rust renders them first, because
/// `JSType#toString` needs the registry.
struct StreamObjectUuidsJson {
    all_serialized_types: IndexMap<String, Vec<String>>,
}

impl StreamObjectUuidsJson {
    // port: SerializeTypesToPointers.StreamObjectUuidsJson#<init>
    fn new(all_serialized_types: IndexMap<String, Vec<String>>) -> Self {
        Self {
            all_serialized_types,
        }
    }
}

impl StreamedJsonProducer for StreamObjectUuidsJson {
    // port: SerializeTypesToPointers.StreamObjectUuidsJson#writeJson
    fn write_json(&self, json_writer: &mut JsonWriter) -> std::io::Result<()> {
        json_writer.begin_object();
        for (key, value) in &self.all_serialized_types {
            json_writer.name(key.as_str()); // color id
            json_writer.begin_array();
            // sort JSTypes by string representation
            let mut jstypes: Vec<JsString> = Vec::new();
            for jstype in value {
                jstypes.push(JsString::from(jstype.as_str()));
            }
            jstypes.sort();
            for type_name in jstypes {
                // all corresponding JSTypes
                json_writer.value(Some(type_name));
            }
            json_writer.end_array();
        }
        json_writer.end_object();
        Ok(())
    }
}

/// port: SerializeTypesToPointers.TypeMismatchJson
struct TypeMismatchJson {
    location: String,
    found_color_id: String,
    required_color_id: String,
}

impl TypeMismatchJson {
    // port: SerializeTypesToPointers.TypeMismatchJson#<init>
    fn new(x: &TypeMismatch, ast: &Ast, found: ColorId, required: ColorId) -> Self {
        Self {
            location: x.location().get_location(ast),
            found_color_id: found.to_string(),
            required_color_id: required.to_string(),
        }
    }

    // port: SerializeTypesToPointers.TypeMismatchJson#create
    fn create(
        x: &TypeMismatch,
        serializer: &mut JSTypeReconserializer,
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        type_pool: &TypePool,
    ) -> Self {
        let found_pointer = serializer.serialize_type(registry, ast, x.found());
        let required_pointer = serializer.serialize_type(registry, ast, x.required());

        Self::new(
            x,
            ast,
            Self::type_pointer_to_id(found_pointer, type_pool),
            Self::type_pointer_to_id(required_pointer, type_pool),
        )
    }

    /// Returns the unique ID of this pointer if in the type pool.
    ///
    /// The given type may not be in the type pool because the type pool was generated based on
    /// all types reachable from the AST, while a TypeMismatch may contain a type in dead code no
    /// longer reachable from the AST.
    // port: SerializeTypesToPointers.TypeMismatchJson#typePointerToId
    fn type_pointer_to_id(pool_offset: i32, type_pool: &TypePool) -> ColorId {
        if TypePointers::is_axiomatic(pool_offset) {
            return OFFSET_TO_AXIOMATIC_COLOR[pool_offset as usize].get_id();
        }

        let type_proto =
            &type_pool.get_type_list()[TypePointers::trim_offset(pool_offset) as usize];
        match type_proto.get_kind_case() {
            TypeProtoKindCase::UNION => {
                let mut ids: IndexSet<ColorId> = IndexSet::<_>::default();
                for &pointer in type_proto.get_union().get_union_member_list() {
                    ids.insert(Self::type_pointer_to_id(pointer, type_pool));
                }
                return ColorId::union(&ids);
            }
            TypeProtoKindCase::OBJECT => {
                return ColorId::from_byte_string(type_proto.get_object().get_uuid());
            }
            TypeProtoKindCase::KIND_NOT_SET => {}
        }
        panic!("AssertionError: Unrecognized TypeProto {type_proto:?}");
    }

    // port: SerializeTypesToPointers.TypeMismatchJson#compareTo
    fn compare_to(&self, x: &TypeMismatchJson) -> std::cmp::Ordering {
        // ComparisonChain over Strings: String#compareTo compares UTF-16 code units.
        let utf16 = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        utf16(&self.found_color_id)
            .cmp(&utf16(&x.found_color_id))
            .then_with(|| utf16(&self.required_color_id).cmp(&utf16(&x.required_color_id)))
            .then_with(|| utf16(&self.location).cmp(&utf16(&x.location)))
    }
}
