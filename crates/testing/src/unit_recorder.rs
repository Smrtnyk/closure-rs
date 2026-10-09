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
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch).
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Compiler.java.

//! UnitRecorder's error and post-call projections, recomputed from live state.
#![allow(clippy::collapsible_if)] // Retain the Java branches.
use crate::jscomp_api::ErrorManager;
pub use crate::replay::replay_values::errors;
use crate::{
    jscomp_api,
    json::JsonValue,
    replay::{
        replay_dsl::{CompilerHandle, DslValue},
        replay_values::{encode, object},
    },
    throwable::Throwable,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use std::rc::Rc;
use std::sync::Arc;
// port: UnitRecorder#refFields
pub fn ref_fields(
    holder: &DslValue,
    keys: &[String],
    depth_left: i32,
) -> Result<JsonValue, Throwable> {
    let mut fields = IndexMap::<_, _>::default();
    for key in keys {
        let value = crate::replay::replay_dsl::get_field(holder, key)?;
        fields.insert(key.clone(), ref_value(&value, depth_left)?);
    }
    Ok(JsonValue::Object(fields))
}
// port: UnitRecorder#refValue
pub fn ref_value(value: &DslValue, depth_left: i32) -> Result<JsonValue, Throwable> {
    dump(value, depth_left, &mut IndexSet::<_>::default())
}
// port: UnitRecorder#dump (referenceable mode)
fn dump(v: &DslValue, depth: i32, seen: &mut IndexSet<usize>) -> Result<JsonValue, Throwable> {
    Ok(match v {
        DslValue::Typed { class, value } => {
            let mut encoded = dump(value, depth, seen)?;
            if let JsonValue::Object(o) = &mut encoded {
                if o.contains_key("impl") {
                    o.insert("impl".into(), JsonValue::str(class));
                }
                // A Node is written as a reference to its runtime class (Node$StringNode, ...).
                if matches!(value.untyped(), DslValue::Node(_)) && o.contains_key("ref") {
                    o.insert("ref".into(), JsonValue::str(class));
                }
            }
            encoded
        }
        DslValue::Native(o) => {
            let id = Rc::as_ptr(o).cast::<()>() as usize;
            let o = o.borrow();
            if let Some(tagged) = o.tagged_dump()? {
                return Ok(tagged);
            }
            if let Some(captures) = o.lambda_captures() {
                return Ok(object([
                    ("ref", JsonValue::str(o.class_name())),
                    (
                        "captures",
                        JsonValue::Object(
                            captures?
                                .iter()
                                .map(|(k, v)| Ok((k.clone(), dump(v, depth, seen)?)))
                                .collect::<Result<_, Throwable>>()?,
                        ),
                    ),
                ]));
            }
            if depth <= 0 || seen.contains(&id) {
                object([("ref", JsonValue::str(o.class_name()))])
            } else {
                seen.insert(id);
                let fields = o
                    .fields()?
                    .into_iter()
                    .map(|(k, v)| Ok((k, dump(&v, depth - 1, seen)?)))
                    .collect::<Result<_, Throwable>>()?;
                seen.shift_remove(&id);
                object([
                    ("object", JsonValue::str(o.class_name())),
                    ("fields", JsonValue::Object(fields)),
                ])
            }
        }
        DslValue::Object(o) => {
            let id = Rc::as_ptr(o) as usize;
            let o = o.borrow();
            if depth <= 0 || seen.contains(&id) {
                object([("ref", JsonValue::str(&o.class))])
            } else {
                seen.insert(id);
                let fields = o
                    .fields
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), dump(v, depth - 1, seen)?)))
                    .collect::<Result<_, Throwable>>()?;
                seen.shift_remove(&id);
                object([
                    ("object", JsonValue::str(&o.class)),
                    ("fields", JsonValue::Object(fields)),
                ])
            }
        }
        DslValue::Sequence(items) => {
            if depth <= 0 {
                object([("ref", JsonValue::str(&v.class_name()))])
            } else {
                object([
                    ("object", JsonValue::str(&v.class_name())),
                    (
                        "fields",
                        object([(
                            "passes",
                            dump(&DslValue::List(items.clone()), depth - 1, seen)?,
                        )]),
                    ),
                ])
            }
        }
        DslValue::List(items) | DslValue::Set(items) => {
            let tag = if matches!(v, DslValue::List(_)) {
                "list"
            } else {
                "set"
            };
            object([
                (
                    tag,
                    JsonValue::Array(
                        items
                            .iter()
                            .map(|v| dump(v, depth, seen))
                            .collect::<Result<_, _>>()?,
                    ),
                ),
                (
                    "impl",
                    JsonValue::str(if tag == "list" {
                        "java.util.ArrayList"
                    } else {
                        "java.util.LinkedHashSet"
                    }),
                ),
            ])
        }
        DslValue::Map(items) => object([
            (
                "map",
                JsonValue::Array(
                    items
                        .iter()
                        .map(|(k, v)| {
                            Ok(JsonValue::Array(vec![
                                dump(k, depth, seen)?,
                                dump(v, depth, seen)?,
                            ]))
                        })
                        .collect::<Result<_, Throwable>>()?,
                ),
            ),
            ("impl", JsonValue::str("java.util.LinkedHashMap")),
        ]),
        DslValue::Array { component, items } => object([
            ("arrayOf", JsonValue::str(component)),
            (
                "items",
                JsonValue::Array(
                    items
                        .iter()
                        .map(|v| dump(v, depth, seen))
                        .collect::<Result<_, _>>()?,
                ),
            ),
        ]),
        DslValue::Lambda(l) => object([
            ("ref", JsonValue::str(&format!("{}$$Lambda", l.iface))),
            (
                "captures",
                JsonValue::Object(
                    l.captured
                        .iter()
                        .enumerate()
                        .map(|(i, (_, v))| Ok((format!("arg${}", i + 1), dump(v, depth, seen)?)))
                        .collect::<Result<_, Throwable>>()?,
                ),
            ),
        ]),
        _ => encode(v)?,
    })
}
const RESULT_PRODUCERS: &[(&str, &str, &str)] = &[
    (
        "com.google.javascript.jscomp.RenameVars",
        "pass.variableMap",
        "getVariableMap",
    ),
    (
        "com.google.javascript.jscomp.RenameProperties",
        "pass.propertyMap",
        "getPropertyMap",
    ),
    (
        "com.google.javascript.jscomp.ReplaceStrings",
        "pass.stringMap",
        "getStringMap",
    ),
    (
        "com.google.javascript.jscomp.disambiguate.AmbiguateProperties",
        "pass.ambiguatedPropertyMap",
        "field:renamingMap",
    ),
    (
        "com.google.javascript.jscomp.ReplaceIdGenerators",
        "pass.idGeneratorMappings",
        "getSerializedIdMappings",
    ),
    (
        "com.google.javascript.jscomp.GatherRawExports",
        "pass.exportedVariableNames",
        "getExportedVariableNames",
    ),
    (
        "com.google.javascript.jscomp.CheckRegExp",
        "pass.globalRegExpPropertiesUsed",
        "isGlobalRegExpPropertiesUsed",
    ),
];
// port: UnitRecorder#moduleMetadata (native ModuleMetadata)
fn native_module_metadata(
    m: &closure_jscomp::modules::module_metadata_map::ModuleMetadata,
) -> JsonValue {
    // refValue of an ImmutableMultiset<String>: its iteration order, as a tagged list.
    let multiset = |set: &closure_jscomp::modules::module_metadata_map::Multiset<
        closure_rhino::js_string::JsString,
    >| {
        object([
            (
                "list",
                JsonValue::Array(
                    set.iter()
                        .map(|s| JsonValue::String(crate::json::JsString(s.as_units().to_vec())))
                        .collect(),
                ),
            ),
            (
                "impl",
                JsonValue::str("com.google.common.collect.RegularImmutableMultiset"),
            ),
        ])
    };
    object([
        (
            "moduleType",
            object([
                (
                    "enum",
                    JsonValue::str(
                        "com.google.javascript.jscomp.modules.ModuleMetadataMap$ModuleType",
                    ),
                ),
                ("name", JsonValue::str(m.module_type().name())),
            ]),
        ),
        ("usesClosure", JsonValue::Bool(m.uses_closure())),
        ("isTestOnly", JsonValue::Bool(m.is_test_only())),
        ("googNamespaces", multiset(m.goog_namespaces())),
        (
            "stronglyRequiredGoogNamespaces",
            multiset(m.strongly_required_goog_namespaces()),
        ),
        (
            "dynamicallyRequiredGoogNamespaces",
            multiset(m.dynamically_required_goog_namespaces()),
        ),
        (
            "maybeRequiredGoogNamespaces",
            multiset(m.maybe_required_goog_namespaces()),
        ),
        (
            "weaklyRequiredGoogNamespaces",
            multiset(m.weakly_required_goog_namespaces()),
        ),
        ("es6ImportSpecifiers", multiset(m.es6_import_specifiers())),
        ("readToggles", multiset(m.read_toggles())),
        // D-017 item 2: the nested goog.loadModule modules' metadata in full (recursively), in
        // nestedModules() order.
        (
            "nestedModules",
            JsonValue::Array(
                m.nested_modules()
                    .iter()
                    .map(|n| native_module_metadata(n))
                    .collect(),
            ),
        ),
    ])
}
// port: UnitRecorder#postCallSnapshot
pub fn post_call_snapshot(
    compiler: Option<&CompilerHandle>,
    processor: Option<&DslValue>,
    extra_roots: &[DslValue],
    postcondition_compiler: Option<&CompilerHandle>,
) -> Result<JsonValue, Throwable> {
    let mut comp = IndexMap::<_, _>::default();
    if let Some(c) = compiler {
        for key in [
            "externProperties",
            "variableMap",
            "propertyMap",
            "stringMap",
            "accessorSummary",
            "moduleMetadataByPath",
            "sourceMap",
            "typeMismatches",
            "runJ2clPasses",
            "injectedLibraries",
            "allowableFeatures",
            "packageJsonMainEntries",
            "typedPercent",
            "moduleMapBoundNames",
        ] {
            match compiler_snapshot_value(&mut c.borrow_mut(), key) {
                Ok(Some(value)) => {
                    let nonempty = match key {
                        "typeMismatches" | "injectedLibraries" => {
                            value.as_array().is_some_and(|a| !a.is_empty())
                        }
                        "moduleMapBoundNames" => value.as_object().is_some_and(|o| !o.is_empty()),
                        "packageJsonMainEntries" => value
                            .get("map")
                            .and_then(JsonValue::as_array)
                            .is_some_and(|a| !a.is_empty()),
                        "runJ2clPasses" => value.as_bool() == Some(true),
                        "typedPercent" => !value
                            .get("double")
                            .and_then(JsonValue::as_js_string)
                            .is_some_and(|s| s.eq_str("0.0")),
                        _ => true,
                    };
                    if nonempty {
                        comp.insert(key.into(), value);
                    }
                }
                Ok(None) => {}
                Err(e @ Throwable::Unported(_)) => return Err(e),
                Err(Throwable::Exception { class, message }) => {
                    if key == "typeMismatches"
                        && message.as_deref().is_some_and(|m| {
                            m.starts_with("Can't ask for type mismatches before type checking")
                        })
                    {
                        continue;
                    }
                    let error_key = match key {
                        "moduleMetadataByPath" => "moduleMetadataError".into(),
                        _ => format!("{key}Error"),
                    };
                    comp.insert(error_key, JsonValue::str(&class));
                }
                Err(e) => return Err(e),
            }
        }
    }
    let mut objs = vec![];
    let mut seen = IndexSet::<_>::default();
    if let Some(p) = processor {
        collect(p, 2, &mut objs, &mut seen)?;
    }
    for r in extra_roots {
        collect(r, 1, &mut objs, &mut seen)?;
    }
    let mut pass = IndexMap::<_, _>::default();
    for value in &objs {
        for (class, key, accessor) in RESULT_PRODUCERS {
            if pass.contains_key(*key) || !is_instance(value, class) {
                continue;
            }
            let value = if let Some(field) = accessor.strip_prefix("field:") {
                crate::replay::replay_dsl::get_field(value, field)
            } else {
                call(value, accessor, vec![])
            };
            let value = value.and_then(|v| {
                if is_instance(&v, "com.google.javascript.jscomp.VariableMap") {
                    call(&v, "getOriginalNameToNewNameMap", vec![])
                } else {
                    Ok(v)
                }
            });
            insert_result(
                &mut pass,
                key,
                value.and_then(|v| {
                    if matches!(v, DslValue::Null) {
                        Ok(None)
                    } else {
                        ref_value(&v, 2).map(Some)
                    }
                }),
            )?;
        }
    }
    if let Some(c) = compiler {
        for value in &objs {
            for (class, key) in [
                (
                    "com.google.javascript.jscomp.ReferenceCollector",
                    "pass.referenceMap",
                ),
                (
                    "com.google.javascript.jscomp.CrossChunkReferenceCollector",
                    "pass.crossChunkReferences",
                ),
            ] {
                if pass.contains_key(key)
                    || pass.contains_key(&format!("{key}Error"))
                    || !is_instance(value, class)
                {
                    continue;
                }
                let result = if key == "pass.referenceMap" {
                    reference_map(value, &c.borrow())
                } else {
                    cross_chunk_references(value, &c.borrow())
                };
                insert_result(&mut pass, key, result.map(Some))?;
            }
        }
    }
    let mut snap = IndexMap::<_, _>::default();
    snap.insert("compiler".into(), JsonValue::Object(comp));
    snap.insert("pass".into(), JsonValue::Object(pass));
    if let Some(c) = postcondition_compiler {
        snap.insert("postcondition".into(), postcondition_data(c)?);
    }
    Ok(JsonValue::Object(snap))
}
// port: UnitRecorder#collect
#[expect(clippy::collapsible_match, reason = "Java collection depth check")]
fn collect(
    value: &DslValue,
    depth: i32,
    out: &mut Vec<DslValue>,
    seen: &mut IndexSet<usize>,
) -> Result<(), Throwable> {
    match value {
        DslValue::Native(o) => {
            let id = Rc::as_ptr(o).cast::<()>() as usize;
            if !seen.insert(id) {
                return Ok(());
            }
            out.push(value.clone());
            let o = o.borrow();
            if depth > 0
                && !o.class_name().starts_with("java.")
                && !o.class_name().starts_with("com.google.common.")
            {
                for v in o.fields()?.values() {
                    collect(v, depth - 1, out, seen)?;
                }
            }
        }
        DslValue::Object(o) => {
            let id = Rc::as_ptr(o) as usize;
            if !seen.insert(id) {
                return Ok(());
            }
            out.push(value.clone());
            let o = o.borrow();
            if depth > 0
                && !o.class.starts_with("java.")
                && !o.class.starts_with("com.google.common.")
            {
                for v in o.fields.values() {
                    collect(v, depth - 1, out, seen)?;
                }
            }
        }
        DslValue::Sequence(items) => {
            out.push(value.clone());
            if depth > 0 {
                collect(&DslValue::List(items.clone()), depth - 1, out, seen)?;
            }
        }
        DslValue::List(items) | DslValue::Set(items) | DslValue::Array { items, .. } => {
            if depth > 0 {
                for v in items {
                    collect(v, depth - 1, out, seen)?;
                }
            }
        }
        DslValue::Lambda(l) => {
            let id = Rc::as_ptr(l) as usize;
            if seen.insert(id) {
                out.push(value.clone());
                if depth > 0 {
                    for v in l.captured.values() {
                        collect(v, depth - 1, out, seen)?;
                    }
                }
            }
        }
        DslValue::Typed { value, .. } => collect(value, depth, out, seen)?,
        _ => {}
    }
    Ok(())
}
// port: UnitRecorder#nodePos
pub fn node_pos(
    ast: &closure_rhino::node::Ast,
    node: Option<closure_rhino::node::NodeId>,
) -> JsonValue {
    node.map_or(JsonValue::Null, |n| {
        JsonValue::str(&format!(
            "{:?} {}:{}:{}",
            n.get_token(ast),
            n.get_source_file_name(ast).as_deref().unwrap_or("null"),
            n.get_lineno(ast),
            n.get_charno(ast)
        ))
    })
}
// port: UnitRecorder#postconditionData
fn postcondition_data(c: &CompilerHandle) -> Result<JsonValue, Throwable> {
    let mut c = c.borrow_mut();
    let mut flags = vec![];
    let mut types = vec![];
    if let Some(root) = c.get_root() {
        let js = c.get_js_root().unwrap();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if n.is_call(&c)
                || n.is_new(&c)
                || n.is_tagged_template_lit(&c)
                || n.is_opt_chain_call(&c)
            {
                let callee = n.get_first_child(&c).and_then(|n| n.get_qualified_name(&c));
                flags.push(JsonValue::Array(vec![
                    node_pos(&c, Some(n)),
                    callee.map_or(JsonValue::Null, |s| {
                        JsonValue::String(crate::json::JsString(s.as_units().to_vec()))
                    }),
                    JsonValue::int(i64::from(n.get_side_effect_flags(&c))),
                ]));
            }
            if let Some(info) = n.get_jsdoc_info(&c) {
                let tn = info.get_type_nodes();
                if !tn.is_empty() && (n == js || n.ancestors(&c).any(|a| a == js)) {
                    types.push(JsonValue::Array(vec![
                        node_pos(&c, Some(n)),
                        JsonValue::Array(
                            tn.iter()
                                .map(|n| {
                                    JsonValue::str(&crate::node_printing::to_string_tree(
                                        &mut c, None, *n,
                                    ))
                                })
                                .collect(),
                        ),
                    ]));
                }
            }
            let children = n.children(&c).collect::<Vec<_>>();
            stack.extend(children.into_iter().rev());
        }
    }
    let mut out = IndexMap::<_, _>::default();
    out.insert("sideEffectFlags".into(), JsonValue::Array(flags));
    out.insert("jsdocTypes".into(), JsonValue::Array(types));
    let extern_export = compiler_snapshot_value(&mut c, "externExport")?.unwrap_or(JsonValue::Null);
    out.insert("externExport".into(), extern_export);
    if let Some(ep) = c.get_extern_properties() {
        let mut ep = ep.iter().cloned().collect::<Vec<_>>();
        ep.sort();
        out.insert(
            "externProperties".into(),
            JsonValue::Array(ep.iter().map(|s| JsonValue::str(s)).collect()),
        );
    }
    Ok(JsonValue::Object(out))
}
// port: UnitRecorder#variableMap
fn variable_map(
    value: Option<&Arc<closure_jscomp::variable_map::VariableMap>>,
) -> Result<Option<JsonValue>, Throwable> {
    Ok(value.map(|v| {
        let entries = v
            .get_original_name_to_new_name_map()
            .iter()
            .map(|(k, v)| {
                JsonValue::Array(vec![
                    JsonValue::String(crate::json::JsString(k.as_units().to_vec())),
                    JsonValue::String(crate::json::JsString(v.as_units().to_vec())),
                ])
            })
            .collect();
        object([
            ("map", JsonValue::Array(entries)),
            (
                "impl",
                JsonValue::str("com.google.common.collect.RegularImmutableBiMap"),
            ),
        ])
    }))
}
// port: UnitRecorder#findMethod (native accessor invocation)
pub fn call(value: &DslValue, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if let DslValue::Native(o) = value {
        o.borrow_mut().call(method, args)
    } else {
        Err(Throwable::Unported(format!(
            "{}#{method}",
            value.class_name()
        )))
    }
}
// port: UnitRecorder#moduleMetadata
pub fn module_metadata(value: &DslValue) -> Result<JsonValue, Throwable> {
    let mut out = IndexMap::<_, _>::default();
    for key in [
        "moduleType",
        "usesClosure",
        "isTestOnly",
        "googNamespaces",
        "stronglyRequiredGoogNamespaces",
        "dynamicallyRequiredGoogNamespaces",
        "maybeRequiredGoogNamespaces",
        "weaklyRequiredGoogNamespaces",
        "es6ImportSpecifiers",
        "readToggles",
    ] {
        out.insert(key.into(), ref_value(&call(value, key, vec![])?, 1)?);
    }
    let nested = call(value, "nestedModules", vec![])?;
    let nested = list_items(&nested)?
        .iter()
        .map(module_metadata)
        .collect::<Result<_, _>>()?;
    out.insert("nestedModules".into(), JsonValue::Array(nested));
    Ok(JsonValue::Object(out))
}
// port: UnitRecorder#postCallSnapshot (compiler getters and neutral projections)
fn compiler_snapshot_value(
    c: &mut jscomp_api::Compiler,
    key: &str,
) -> Result<Option<JsonValue>, Throwable> {
    use closure_rhino::js_string::JsString;
    Ok(match key {
        "externProperties" => c
            .get_extern_properties()
            .map(|p| {
                ref_value(
                    &DslValue::Set(
                        p.iter()
                            .map(|s| DslValue::String(JsString::from(s.as_str())))
                            .collect(),
                    ),
                    2,
                )
            })
            .transpose()?,
        "variableMap" => variable_map(c.get_variable_map())?,
        "propertyMap" => variable_map(c.get_property_map())?,
        "stringMap" => variable_map(c.get_string_map())?,
        "accessorSummary" => crate::harness_passes::accessor_summary(c)?
            .map(|s| {
                ref_value(
                    &DslValue::Map(
                        s.into_iter()
                            .map(|(k, v)| (DslValue::String(JsString::from(k)), v))
                            .collect(),
                    ),
                    2,
                )
            })
            .transpose()?,
        "moduleMetadataByPath" => c.get_module_metadata_map().map(|mm| {
            JsonValue::Object(
                mm.get_modules_by_path()
                    .iter()
                    .map(|(path, m)| (path.clone(), native_module_metadata(m)))
                    .collect(),
            )
        }),
        "sourceMap" => c
            .get_source_map()
            .map(|sm| {
                crate::harness_passes::source_map_append_to(&mut sm.lock().unwrap(), "out.js").map(
                    |value| JsonValue::String(crate::json::JsString(value.as_units().to_vec())),
                )
            })
            .transpose()?,
        // port: Compiler#getTypeMismatches ({"found": JSType.toString(), "required": ...} pairs)
        "typeMismatches" => {
            if !c.has_type_checking_run() {
                return Err(Throwable::Exception {
                    class: "java.lang.RuntimeException".into(),
                    message: Some("Can't ask for type mismatches before type checking.".into()),
                });
            }
            if c.is_type_registry_cleared() {
                // Compiler#getTypeValidator's checkState: the validator was cleared with the
                // registry (Compiler#clearJSTypeRegistry).
                return Err(Throwable::Exception {
                    class: "java.lang.IllegalStateException".into(),
                    message: Some(
                        "Attempted to re-initialize TypeValidator after it had been cleared".into(),
                    ),
                });
            }
            let mismatches = c.get_type_mismatches();
            if mismatches.is_empty() {
                None
            } else {
                use closure_jstype::js_type::JSType;
                let (reg, ast) = c.get_type_registry_and_ast();
                Some(JsonValue::Array(
                    mismatches
                        .iter()
                        .map(|m| {
                            let found = m.get_found().to_string(reg, ast);
                            let required = m.get_required().to_string(reg, ast);
                            JsonValue::Object(IndexMap::<_, _>::from_iter([
                                ("found".to_owned(), JsonValue::str(&found)),
                                ("required".to_owned(), JsonValue::str(&required)),
                            ]))
                        })
                        .collect(),
                ))
            }
        }
        "runJ2clPasses" => Some(JsonValue::Bool(c.run_j2cl_passes())),
        "injectedLibraries" => Some(JsonValue::Array(
            crate::harness_passes::injected_libraries(c)?
                .iter()
                .map(|name| JsonValue::str(name))
                .collect(),
        )),
        "allowableFeatures" => Some(JsonValue::Array(
            c.get_allowable_features()
                .get_features()
                .iter()
                .map(|f| JsonValue::str(&format!("{f:?}")))
                .collect(),
        )),
        "packageJsonMainEntries" => Some({
            let entries = c.get_module_loader().get_package_json_main_entries();
            object([
                (
                    "map",
                    JsonValue::Array(
                        entries
                            .iter()
                            .map(|(k, v)| {
                                JsonValue::Array(vec![JsonValue::str(k), JsonValue::str(v)])
                            })
                            .collect(),
                    ),
                ),
                (
                    "impl",
                    JsonValue::str("com.google.common.collect.RegularImmutableMap"),
                ),
            ])
        }),
        "typedPercent" => Some(encode(&DslValue::Double(
            c.get_error_manager().get_typed_percent(),
        ))?),
        // Compiler.getModuleMap().getModulesByPath() -> Module.boundNames().keySet()
        "moduleMapBoundNames" => c.get_module_map().map(|mm| {
            JsonValue::Object(
                mm.get_modules_by_path()
                    .iter()
                    .map(|(path, m)| {
                        (
                            path.clone(),
                            JsonValue::Array(
                                m.bound_names()
                                    .keys()
                                    .map(|k| {
                                        JsonValue::String(crate::json::JsString(
                                            k.as_units().to_vec(),
                                        ))
                                    })
                                    .collect(),
                            ),
                        )
                    })
                    .collect(),
            )
        }),
        "externExport" => c.get_result().extern_export.map(|s| JsonValue::str(&s)),
        _ => {
            return Err(Throwable::HarnessError(format!(
                "unknown compiler snapshot key {key}"
            )));
        }
    })
}
// port: UnitRecorder#postCallSnapshot (Map iteration)
fn map_items(value: &DslValue) -> Result<&[(DslValue, DslValue)], Throwable> {
    if let DslValue::Map(items) = value.untyped() {
        Ok(items)
    } else {
        Err(Throwable::HarnessError(
            "snapshot accessor did not return a Map".into(),
        ))
    }
}
// port: UnitRecorder#moduleMetadata (Collection iteration)
fn list_items(value: &DslValue) -> Result<&[DslValue], Throwable> {
    match value.untyped() {
        DslValue::List(v) | DslValue::Set(v) => Ok(v),
        _ => Err(Throwable::HarnessError(
            "snapshot accessor did not return a Collection".into(),
        )),
    }
}
// port: UnitRecorder#postCallSnapshot (String map keys)
fn string(value: &DslValue) -> Result<String, Throwable> {
    if let DslValue::String(s) = value {
        Ok(s.to_string_lossy())
    } else {
        Err(Throwable::HarnessError(
            "snapshot map key not a String".into(),
        ))
    }
}
// port: UnitRecorder#isInstance
fn is_instance(value: &DslValue, class: &str) -> bool {
    if let DslValue::Native(o) = value {
        o.borrow().is_instance_of(class)
    } else {
        value.class_name() == class
    }
}
// port: UnitRecorder#postCallSnapshot (runtime-exception result key)
fn insert_result(
    out: &mut IndexMap<String, JsonValue>,
    key: &str,
    value: Result<Option<JsonValue>, Throwable>,
) -> Result<(), Throwable> {
    match value {
        Ok(Some(v)) => {
            out.insert(key.into(), v);
        }
        Ok(None) => {}
        Err(Throwable::Exception { class, .. }) => {
            out.insert(format!("{key}Error"), JsonValue::str(&class));
        }
        Err(e) => return Err(e),
    }
    Ok(())
}
// port: UnitRecorder#postCallSnapshot (ReferenceCollector)
fn reference_map(collector: &DslValue, c: &jscomp_api::Compiler) -> Result<JsonValue, Throwable> {
    let symbols = call(collector, "getAllSymbols", vec![])?;
    let mut vars = vec![];
    for var in list_items(&symbols)? {
        let refs = call(collector, "getReferences", vec![var.clone()])?;
        let references = crate::replay::replay_dsl::get_field(&refs, "references")?;
        let mut array = vec![];
        for r in list_items(&references)? {
            array.push(object([
                ("node", position(c, &call(r, "getNode", vec![])?)?),
                ("isDeclaration", encode(&call(r, "isDeclaration", vec![])?)?),
                (
                    "isInitializingDeclaration",
                    encode(&call(r, "isInitializingDeclaration", vec![])?)?,
                ),
                ("isLvalue", encode(&call(r, "isLvalue", vec![])?)?),
            ]));
        }
        vars.push(object([
            ("name", encode(&call(var, "getName", vec![])?)?),
            (
                "scopeRoot",
                position(c, &call(var, "getScopeRoot", vec![])?)?,
            ),
            (
                "isAssignedOnceInLifetime",
                encode(&call(&refs, "isAssignedOnceInLifetime", vec![])?)?,
            ),
            (
                "isWellDefined",
                encode(&call(&refs, "isWellDefined", vec![])?)?,
            ),
            ("references", JsonValue::Array(array)),
        ]));
    }
    Ok(JsonValue::Array(vars))
}
// port: UnitRecorder#postCallSnapshot (CrossChunkReferenceCollector)
fn cross_chunk_references(
    collector: &DslValue,
    c: &jscomp_api::Compiler,
) -> Result<JsonValue, Throwable> {
    let global_names = call(collector, "getGlobalVariableNamesMap", vec![])?;
    let mut vars = vec![];
    let mut ids = IndexMap::<_, _>::default();
    for (name, var) in map_items(&global_names)? {
        let refs = call(collector, "getReferences", vec![var.clone()])?;
        let mut vo = IndexMap::<_, _>::default();
        vo.insert("name".into(), encode(name)?);
        if matches!(refs, DslValue::Null) {
            vo.insert("references".into(), JsonValue::Null);
        } else {
            vo.insert(
                "isAssignedOnceInLifetime".into(),
                encode(&call(&refs, "isAssignedOnceInLifetime", vec![])?)?,
            );
            vo.insert(
                "isWellDefined".into(),
                encode(&call(&refs, "isWellDefined", vec![])?)?,
            );
            let references = crate::replay::replay_dsl::get_field(&refs, "references")?;
            let mut array = vec![];
            for (i, r) in list_items(&references)?.iter().enumerate() {
                ids.insert(identity(r)?, format!("{}#{i}", string(name)?));
                let block = call(r, "getBasicBlock", vec![])?;
                let block_root = if matches!(block, DslValue::Null) {
                    JsonValue::Null
                } else {
                    position(c, &call(&block, "getRoot", vec![])?)?
                };
                array.push(object([
                    ("node", position(c, &call(r, "getNode", vec![])?)?),
                    ("basicBlockRoot", block_root),
                ]));
            }
            vo.insert("references".into(), JsonValue::Array(array));
        }
        vars.push(JsonValue::Object(vo));
    }
    let statements = call(collector, "getTopLevelStatements", vec![])?;
    let mut stmts = vec![];
    for t in list_items(&statements)? {
        let declaration = call(t, "isDeclarationStatement", vec![])?;
        let declared_ref = if declaration.equals(&DslValue::Bool(true)) {
            reference_id(&ids, &call(t, "getDeclaredNameReference", vec![])?, c)?
        } else {
            JsonValue::Null
        };
        let references = call(t, "getNonDeclarationReferences", vec![])?;
        let declared_value = call(t, "getDeclaredValueNode", vec![])?;
        let mut out = object([
            (
                "originalOrder",
                encode(&call(t, "getOriginalOrder", vec![])?)?,
            ),
            (
                "statement",
                position(c, &call(t, "getStatementNode", vec![])?)?,
            ),
            ("isDeclarationStatement", encode(&declaration)?),
            (
                "isMovableDeclaration",
                encode(&call(t, "isMovableDeclaration", vec![])?)?,
            ),
            ("declaredNameReference", declared_ref),
            (
                "nonDeclarationReferences",
                JsonValue::Array(
                    list_items(&references)?
                        .iter()
                        .map(|r| reference_id(&ids, r, c))
                        .collect::<Result<_, _>>()?,
                ),
            ),
            ("declaredValueNode", position(c, &declared_value)?),
        ]);
        if let DslValue::Node(node) = declared_value {
            if node.is_number(c) {
                if let JsonValue::Object(o) = &mut out {
                    o.insert(
                        "declaredValueNumber".into(),
                        JsonValue::str(&closure_rhino::java_lang::double_to_string(
                            node.get_double(c),
                        )),
                    );
                }
            }
        }
        stmts.push(out);
    }
    Ok(object([
        ("vars", JsonValue::Array(vars)),
        ("topLevelStatements", JsonValue::Array(stmts)),
    ]))
}
// port: UnitRecorder#nodePos (decoded live Node)
fn position(c: &jscomp_api::Compiler, node: &DslValue) -> Result<JsonValue, Throwable> {
    match node {
        DslValue::Node(n) => Ok(node_pos(c, Some(*n))),
        DslValue::Null => Ok(JsonValue::Null),
        _ => Err(Throwable::HarnessError("nodePos requires a Node".into())),
    }
}
// port: UnitRecorder#refId (IdentityHashMap key)
fn identity(value: &DslValue) -> Result<usize, Throwable> {
    match value {
        DslValue::Native(o) => Ok(Rc::as_ptr(o).cast::<()>() as usize),
        DslValue::Object(o) => Ok(Rc::as_ptr(o) as usize),
        _ => Err(Throwable::HarnessError(
            "reference has no object identity".into(),
        )),
    }
}
// port: UnitRecorder#refId
fn reference_id(
    ids: &IndexMap<usize, String>,
    value: &DslValue,
    c: &jscomp_api::Compiler,
) -> Result<JsonValue, Throwable> {
    if matches!(value, DslValue::Null) {
        return Ok(JsonValue::Null);
    }
    if let Some(id) = ids.get(&identity(value)?) {
        Ok(JsonValue::str(id))
    } else {
        let node = position(c, &call(value, "getNode", vec![])?)?;
        Ok(JsonValue::str(&format!(
            "unlisted {}",
            node.as_js_string()
                .map_or_else(|| "null".into(), |s| s.to_string_lossy())
        )))
    }
}
