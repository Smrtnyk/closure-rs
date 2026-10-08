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

//! FORMAT.md "Post-call snapshot" and "Postcondition data": the record's `postCall` object.

use crate::json::{JsString, JsonValue};
use crate::reader::{
    ModelResult, Obj, ObjOut, arr, as_array, as_bool, as_i32, as_js_string, as_opt_js_string, err,
    index, int, js, list_of, map_of, opt_js,
};
use crate::value::Value;
use indexmap::IndexMap;

/// `postCall.compiler` keys (FORMAT.md "Post-call snapshot" table) whose values are tagged
/// [`Value`]s.
const COMPILER_TAGGED: &[&str] = &[
    "externProperties",
    "variableMap",
    "propertyMap",
    "stringMap",
    "accessorSummary",
    "packageJsonMainEntries",
    "typedPercent",
];

/// `postCall.compiler` keys whose values are untagged shapes (typed by `plain_from_json`).
const COMPILER_PLAIN: &[&str] = &[
    "moduleMetadataByPath",
    "sourceMap",
    "typeMismatches",
    "runJ2clPasses",
    "injectedLibraries",
    "allowableFeatures",
    "moduleMapBoundNames",
];

/// `postCall.pass` keys (result producers) whose values are tagged [`Value`]s.
const PASS_TAGGED: &[&str] = &[
    "pass.variableMap",
    "pass.propertyMap",
    "pass.stringMap",
    "pass.ambiguatedPropertyMap",
    "pass.exportedVariableNames",
];

/// `postCall.pass` keys whose values are untagged shapes (typed by `plain_from_json`).
const PASS_PLAIN: &[&str] = &[
    "pass.idGeneratorMappings",
    "pass.globalRegExpPropertiesUsed",
    "pass.referenceMap",
    "pass.crossChunkReferences",
];

/// `postCall.postcondition` keys (FORMAT.md "Postcondition data"); all untagged shapes.
const POSTCONDITION_PLAIN: &[&str] = &[
    "sideEffectFlags",
    "jsdocTypes",
    "externExport",
    "externProperties",
];

/// One snapshot value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostCallValue {
    /// A value in the referenceable encoding (FORMAT.md "Referenceable values").
    Tagged(Value),
    /// `moduleMetadataByPath`: path -> module metadata.
    ModuleMetadataByPath(IndexMap<String, ModuleMetadata>),
    /// `sourceMap`: source map V3 JSON text.
    SourceMap(JsString),
    /// `typeMismatches`, in recording order.
    TypeMismatches(Vec<TypeMismatch>),
    /// `runJ2clPasses` (present only when true) and `pass.globalRegExpPropertiesUsed`.
    Bool(bool),
    /// A list of strings: `injectedLibraries`, `allowableFeatures`, `postcondition.externProperties`.
    Strings(Vec<JsString>),
    /// `moduleMapBoundNames`: module path -> bound local names.
    ModuleMapBoundNames(IndexMap<String, Vec<JsString>>),
    /// `pass.idGeneratorMappings` (`getSerializedIdMappings()`) and `postcondition.externExport`
    /// (null when the result has none).
    Text(Option<JsString>),
    /// `pass.referenceMap`: `getAllSymbols()` in collection order.
    ReferenceMap(Vec<ReferenceMapSymbol>),
    /// `pass.crossChunkReferences`.
    CrossChunkReferences(CrossChunkReferences),
    /// `postcondition.sideEffectFlags`.
    SideEffectFlags(Vec<SideEffectFlag>),
    /// `postcondition.jsdocTypes`.
    JsdocTypes(Vec<JsdocTypes>),
    /// `<key>Error`: the exception class the accessor threw.
    Error(String),
}

/// A module metadata object of `moduleMetadataByPath` (multisets as tagged lists).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleMetadata {
    /// `{"enum": ModuleType, "name"}`
    pub module_type: Value,
    pub uses_closure: bool,
    pub is_test_only: bool,
    pub goog_namespaces: Value,
    pub strongly_required_goog_namespaces: Value,
    pub dynamically_required_goog_namespaces: Value,
    pub maybe_required_goog_namespaces: Value,
    pub weakly_required_goog_namespaces: Value,
    pub es6_import_specifiers: Value,
    pub read_toggles: Value,
    /// The nested `goog.loadModule` modules, same shape, in `nestedModules()` order.
    pub nested_modules: Vec<ModuleMetadata>,
}

/// `{"found": JSType.toString(), "required": JSType.toString()}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeMismatch {
    pub found: JsString,
    pub required: JsString,
}

/// One symbol of `pass.referenceMap`. Node positions are `"<Token> <file>:<line>:<column>"`,
/// null for a null node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceMapSymbol {
    pub name: JsString,
    pub scope_root: Option<JsString>,
    pub is_assigned_once_in_lifetime: bool,
    pub is_well_defined: bool,
    pub references: Vec<ReferenceMapReference>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceMapReference {
    pub node: Option<JsString>,
    pub is_declaration: bool,
    pub is_initializing_declaration: bool,
    pub is_lvalue: bool,
}

/// `pass.crossChunkReferences = {vars, topLevelStatements}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrossChunkReferences {
    pub vars: Vec<CrossChunkVar>,
    pub top_level_statements: Vec<TopLevelStatement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrossChunkVar {
    pub name: JsString,
    pub is_assigned_once_in_lifetime: bool,
    pub is_well_defined: bool,
    /// `None`: `references: null` (the var has no collection).
    pub references: Option<Vec<CrossChunkReference>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrossChunkReference {
    pub node: Option<JsString>,
    pub basic_block_root: Option<JsString>,
}

/// One `getTopLevelStatements()` entry; references are named `"<var>#<index>"` or
/// `"unlisted <node>"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopLevelStatement {
    pub original_order: i32,
    pub statement: Option<JsString>,
    pub is_declaration_statement: bool,
    pub is_movable_declaration: bool,
    pub declared_name_reference: Option<JsString>,
    pub non_declaration_references: Vec<JsString>,
    pub declared_value_node: Option<JsString>,
    /// `String.valueOf(Node.getDouble())`, present only for a number value.
    pub declared_value_number: Option<JsString>,
}

/// `[node, callee qualified name or null, Node.getSideEffectFlags()]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideEffectFlag {
    pub node: Option<JsString>,
    pub callee: Option<JsString>,
    pub flags: i32,
}

/// `[node, [toStringTree of each JSDoc type node]]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsdocTypes {
    pub node: Option<JsString>,
    pub types: Vec<JsString>,
}

fn js_list(v: &JsonValue, path: &str) -> ModelResult<Vec<JsString>> {
    list_of(v, path, as_js_string)
}

fn js_list_to_json(items: &[JsString]) -> JsonValue {
    arr(items, js)
}

fn b(v: bool) -> JsonValue {
    JsonValue::Bool(v)
}

impl ModuleMetadata {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<ModuleMetadata> {
        let mut o = Obj::new(v, path)?;
        let val = |o: &mut Obj<'_>, k: &str| -> ModelResult<Value> {
            let p = o.sub(k);
            Value::from_json(o.req(k)?, &p)
        };
        let module_type = val(&mut o, "moduleType")?;
        let uses_closure = o.req_bool("usesClosure")?;
        let is_test_only = o.req_bool("isTestOnly")?;
        let goog_namespaces = val(&mut o, "googNamespaces")?;
        let strongly_required_goog_namespaces = val(&mut o, "stronglyRequiredGoogNamespaces")?;
        let dynamically_required_goog_namespaces =
            val(&mut o, "dynamicallyRequiredGoogNamespaces")?;
        let maybe_required_goog_namespaces = val(&mut o, "maybeRequiredGoogNamespaces")?;
        let weakly_required_goog_namespaces = val(&mut o, "weaklyRequiredGoogNamespaces")?;
        let es6_import_specifiers = val(&mut o, "es6ImportSpecifiers")?;
        let read_toggles = val(&mut o, "readToggles")?;
        let np = o.sub("nestedModules");
        let nested_modules = list_of(o.req("nestedModules")?, &np, ModuleMetadata::from_json)?;
        o.finish()?;
        Ok(ModuleMetadata {
            module_type,
            uses_closure,
            is_test_only,
            goog_namespaces,
            strongly_required_goog_namespaces,
            dynamically_required_goog_namespaces,
            maybe_required_goog_namespaces,
            weakly_required_goog_namespaces,
            es6_import_specifiers,
            read_toggles,
            nested_modules,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("moduleType", self.module_type.to_json())
            .put("usesClosure", b(self.uses_closure))
            .put("isTestOnly", b(self.is_test_only))
            .put("googNamespaces", self.goog_namespaces.to_json())
            .put(
                "stronglyRequiredGoogNamespaces",
                self.strongly_required_goog_namespaces.to_json(),
            )
            .put(
                "dynamicallyRequiredGoogNamespaces",
                self.dynamically_required_goog_namespaces.to_json(),
            )
            .put(
                "maybeRequiredGoogNamespaces",
                self.maybe_required_goog_namespaces.to_json(),
            )
            .put(
                "weaklyRequiredGoogNamespaces",
                self.weakly_required_goog_namespaces.to_json(),
            )
            .put("es6ImportSpecifiers", self.es6_import_specifiers.to_json())
            .put("readToggles", self.read_toggles.to_json())
            .put(
                "nestedModules",
                arr(&self.nested_modules, ModuleMetadata::to_json),
            )
            .build()
    }
}

impl TypeMismatch {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<TypeMismatch> {
        let mut o = Obj::new(v, path)?;
        let found = o.req_js_string("found")?;
        let required = o.req_js_string("required")?;
        o.finish()?;
        Ok(TypeMismatch { found, required })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("found", js(&self.found))
            .put("required", js(&self.required))
            .build()
    }
}

fn opt_node(o: &mut Obj<'_>, key: &str) -> ModelResult<Option<JsString>> {
    let p = o.sub(key);
    as_opt_js_string(o.req(key)?, &p)
}

impl ReferenceMapSymbol {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<ReferenceMapSymbol> {
        let mut o = Obj::new(v, path)?;
        let name = o.req_js_string("name")?;
        let scope_root = opt_node(&mut o, "scopeRoot")?;
        let is_assigned_once_in_lifetime = o.req_bool("isAssignedOnceInLifetime")?;
        let is_well_defined = o.req_bool("isWellDefined")?;
        let rp = o.sub("references");
        let references = list_of(o.req("references")?, &rp, |x, p| {
            let mut r = Obj::new(x, p)?;
            let node = opt_node(&mut r, "node")?;
            let is_declaration = r.req_bool("isDeclaration")?;
            let is_initializing_declaration = r.req_bool("isInitializingDeclaration")?;
            let is_lvalue = r.req_bool("isLvalue")?;
            r.finish()?;
            Ok(ReferenceMapReference {
                node,
                is_declaration,
                is_initializing_declaration,
                is_lvalue,
            })
        })?;
        o.finish()?;
        Ok(ReferenceMapSymbol {
            name,
            scope_root,
            is_assigned_once_in_lifetime,
            is_well_defined,
            references,
        })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("name", js(&self.name))
            .put("scopeRoot", opt_js(&self.scope_root))
            .put(
                "isAssignedOnceInLifetime",
                b(self.is_assigned_once_in_lifetime),
            )
            .put("isWellDefined", b(self.is_well_defined))
            .put(
                "references",
                arr(&self.references, |r| {
                    ObjOut::new()
                        .put("node", opt_js(&r.node))
                        .put("isDeclaration", b(r.is_declaration))
                        .put(
                            "isInitializingDeclaration",
                            b(r.is_initializing_declaration),
                        )
                        .put("isLvalue", b(r.is_lvalue))
                        .build()
                }),
            )
            .build()
    }
}

impl CrossChunkReferences {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<CrossChunkReferences> {
        let mut o = Obj::new(v, path)?;
        let vp = o.sub("vars");
        let vars = list_of(o.req("vars")?, &vp, |x, p| {
            let mut r = Obj::new(x, p)?;
            let name = r.req_js_string("name")?;
            let is_assigned_once_in_lifetime = r.req_bool("isAssignedOnceInLifetime")?;
            let is_well_defined = r.req_bool("isWellDefined")?;
            let rp = r.sub("references");
            let references = match r.req("references")? {
                JsonValue::Null => None,
                refs => Some(list_of(refs, &rp, |y, q| {
                    let mut c = Obj::new(y, q)?;
                    let node = opt_node(&mut c, "node")?;
                    let basic_block_root = opt_node(&mut c, "basicBlockRoot")?;
                    c.finish()?;
                    Ok(CrossChunkReference {
                        node,
                        basic_block_root,
                    })
                })?),
            };
            r.finish()?;
            Ok(CrossChunkVar {
                name,
                is_assigned_once_in_lifetime,
                is_well_defined,
                references,
            })
        })?;
        let tp = o.sub("topLevelStatements");
        let top_level_statements = list_of(o.req("topLevelStatements")?, &tp, |x, p| {
            let mut t = Obj::new(x, p)?;
            let original_order = t.req_i32("originalOrder")?;
            let statement = opt_node(&mut t, "statement")?;
            let is_declaration_statement = t.req_bool("isDeclarationStatement")?;
            let is_movable_declaration = t.req_bool("isMovableDeclaration")?;
            let declared_name_reference = opt_node(&mut t, "declaredNameReference")?;
            let np = t.sub("nonDeclarationReferences");
            let non_declaration_references = js_list(t.req("nonDeclarationReferences")?, &np)?;
            let declared_value_node = opt_node(&mut t, "declaredValueNode")?;
            let dp = t.sub("declaredValueNumber");
            let declared_value_number = t
                .opt("declaredValueNumber")
                .map(|x| as_js_string(x, &dp))
                .transpose()?;
            t.finish()?;
            Ok(TopLevelStatement {
                original_order,
                statement,
                is_declaration_statement,
                is_movable_declaration,
                declared_name_reference,
                non_declaration_references,
                declared_value_node,
                declared_value_number,
            })
        })?;
        o.finish()?;
        Ok(CrossChunkReferences {
            vars,
            top_level_statements,
        })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put(
                "vars",
                arr(&self.vars, |v| {
                    ObjOut::new()
                        .put("name", js(&v.name))
                        .put(
                            "isAssignedOnceInLifetime",
                            b(v.is_assigned_once_in_lifetime),
                        )
                        .put("isWellDefined", b(v.is_well_defined))
                        .put(
                            "references",
                            v.references.as_ref().map_or(JsonValue::Null, |refs| {
                                arr(refs, |r| {
                                    ObjOut::new()
                                        .put("node", opt_js(&r.node))
                                        .put("basicBlockRoot", opt_js(&r.basic_block_root))
                                        .build()
                                })
                            }),
                        )
                        .build()
                }),
            )
            .put(
                "topLevelStatements",
                arr(&self.top_level_statements, |t| {
                    ObjOut::new()
                        .put("originalOrder", int(t.original_order))
                        .put("statement", opt_js(&t.statement))
                        .put("isDeclarationStatement", b(t.is_declaration_statement))
                        .put("isMovableDeclaration", b(t.is_movable_declaration))
                        .put("declaredNameReference", opt_js(&t.declared_name_reference))
                        .put(
                            "nonDeclarationReferences",
                            js_list_to_json(&t.non_declaration_references),
                        )
                        .put("declaredValueNode", opt_js(&t.declared_value_node))
                        .put_opt(
                            "declaredValueNumber",
                            t.declared_value_number.as_ref().map(js),
                        )
                        .build()
                }),
            )
            .build()
    }
}

fn tuple<'a>(v: &'a JsonValue, path: &str, n: usize) -> ModelResult<&'a [JsonValue]> {
    let a = as_array(v, path)?;
    if a.len() == n {
        Ok(a)
    } else {
        err(
            path,
            format!("expected a {n}-element array, found {}", a.len()),
        )
    }
}

/// Reads the documented plain shape of `key` in a section.
fn plain_from_json(key: &str, v: &JsonValue, path: &str) -> ModelResult<PostCallValue> {
    Ok(match key {
        "moduleMetadataByPath" => {
            PostCallValue::ModuleMetadataByPath(map_of(v, path, ModuleMetadata::from_json)?)
        }
        "sourceMap" => PostCallValue::SourceMap(as_js_string(v, path)?),
        "typeMismatches" => {
            PostCallValue::TypeMismatches(list_of(v, path, TypeMismatch::from_json)?)
        }
        "runJ2clPasses" | "pass.globalRegExpPropertiesUsed" => {
            PostCallValue::Bool(as_bool(v, path)?)
        }
        "injectedLibraries" | "allowableFeatures" | "externProperties" => {
            PostCallValue::Strings(js_list(v, path)?)
        }
        "moduleMapBoundNames" => PostCallValue::ModuleMapBoundNames(map_of(v, path, js_list)?),
        "pass.idGeneratorMappings" | "externExport" => {
            PostCallValue::Text(as_opt_js_string(v, path)?)
        }
        "pass.referenceMap" => {
            PostCallValue::ReferenceMap(list_of(v, path, ReferenceMapSymbol::from_json)?)
        }
        "pass.crossChunkReferences" => {
            PostCallValue::CrossChunkReferences(CrossChunkReferences::from_json(v, path)?)
        }
        "sideEffectFlags" => PostCallValue::SideEffectFlags(list_of(v, path, |x, p| {
            let t = tuple(x, p, 3)?;
            Ok(SideEffectFlag {
                node: as_opt_js_string(&t[0], &index(p, 0))?,
                callee: as_opt_js_string(&t[1], &index(p, 1))?,
                flags: as_i32(&t[2], &index(p, 2))?,
            })
        })?),
        "jsdocTypes" => PostCallValue::JsdocTypes(list_of(v, path, |x, p| {
            let t = tuple(x, p, 2)?;
            Ok(JsdocTypes {
                node: as_opt_js_string(&t[0], &index(p, 0))?,
                types: js_list(&t[1], &index(p, 1))?,
            })
        })?),
        _ => return err(path, "undocumented postCall key"),
    })
}

impl PostCallValue {
    pub fn to_json(&self) -> JsonValue {
        match self {
            PostCallValue::Tagged(t) => t.to_json(),
            PostCallValue::ModuleMetadataByPath(m) => {
                JsonValue::Object(m.iter().map(|(k, x)| (k.clone(), x.to_json())).collect())
            }
            PostCallValue::SourceMap(s) => js(s),
            PostCallValue::TypeMismatches(l) => arr(l, TypeMismatch::to_json),
            PostCallValue::Bool(v) => b(*v),
            PostCallValue::Strings(l) => js_list_to_json(l),
            PostCallValue::ModuleMapBoundNames(m) => JsonValue::Object(
                m.iter()
                    .map(|(k, x)| (k.clone(), js_list_to_json(x)))
                    .collect(),
            ),
            PostCallValue::Text(t) => opt_js(t),
            PostCallValue::ReferenceMap(l) => arr(l, ReferenceMapSymbol::to_json),
            PostCallValue::CrossChunkReferences(c) => c.to_json(),
            PostCallValue::SideEffectFlags(l) => arr(l, |f| {
                JsonValue::Array(vec![opt_js(&f.node), opt_js(&f.callee), int(f.flags)])
            }),
            PostCallValue::JsdocTypes(l) => arr(l, |j| {
                JsonValue::Array(vec![opt_js(&j.node), js_list_to_json(&j.types)])
            }),
            PostCallValue::Error(e) => JsonValue::str(e),
        }
    }
}

/// One `postCall` section: key -> value in recorded order.
pub type PostCallSection = IndexMap<String, PostCallValue>;

/// `postCall = {"compiler": {...}, "pass": {...}, "postcondition"?: {...}}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostCall {
    pub compiler: PostCallSection,
    pub pass: PostCallSection,
    pub postcondition: Option<PostCallSection>,
}

fn section_from_json(
    v: &JsonValue,
    path: &str,
    tagged: &[&str],
    plain: &[&str],
) -> ModelResult<PostCallSection> {
    let mut o = Obj::new(v, path)?;
    let mut out = IndexMap::new();
    for (k, x) in o.rest() {
        let p = format!("{path}.{k}");
        let val = if tagged.contains(&k.as_str()) {
            PostCallValue::Tagged(Value::from_json(x, &p)?)
        } else if plain.contains(&k.as_str()) {
            plain_from_json(&k, x, &p)?
        } else if let Some(base) = k.strip_suffix("Error")
            && (tagged.contains(&base) || plain.contains(&base))
        {
            PostCallValue::Error(crate::reader::as_string(x, &p)?)
        } else {
            return err(&p, "undocumented postCall key");
        };
        out.insert(k, val);
    }
    o.finish()?;
    Ok(out)
}

fn section_to_json(s: &PostCallSection) -> JsonValue {
    let mut o = ObjOut::new();
    for (k, v) in s {
        o.put(k, v.to_json());
    }
    o.build()
}

impl PostCall {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<PostCall> {
        let mut o = Obj::new(v, path)?;
        let p = o.sub("compiler");
        let compiler = section_from_json(o.req("compiler")?, &p, COMPILER_TAGGED, COMPILER_PLAIN)?;
        let p = o.sub("pass");
        let pass = section_from_json(o.req("pass")?, &p, PASS_TAGGED, PASS_PLAIN)?;
        let p = o.sub("postcondition");
        let postcondition = o
            .opt("postcondition")
            .map(|x| section_from_json(x, &p, &[], POSTCONDITION_PLAIN))
            .transpose()?;
        o.finish()?;
        Ok(PostCall {
            compiler,
            pass,
            postcondition,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("compiler", section_to_json(&self.compiler))
            .put("pass", section_to_json(&self.pass))
            .put_opt(
                "postcondition",
                self.postcondition.as_ref().map(section_to_json),
            )
            .build()
    }
}
