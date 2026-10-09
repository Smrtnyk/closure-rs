/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/ReplaceStrings.java,
//   test/com/google/javascript/jscomp/ReplaceStringsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Port of the replay helper `oracle/replay/helpers/.../ReplaceStringsTest_Helpers.java`
//! (DSL name `ReplaceStringsTest_Helpers.GetProcessorPass`), itself copied from
//! ReplaceStringsTest.java: the nested `Renamer` callback and the anonymous `CompilerPass`
//! returned by `getProcessor`; plus the `ReplaceStrings` native object whose fields the record's
//! `testFieldsAfter.pass` and `postCall.pass.stringMap` compare.
use crate::{
    replay::{
        options_values::{OptionValue, encode_name_generator},
        replay_dsl::{Ctx, DslValue, NativeObject, Object},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    Compiler,
    compiler_options::{ChunkOutputType, PropertyCollapseLevel},
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    disambiguate::disambiguate_properties::DisambiguateProperties,
    es6_normalize_classes::Es6NormalizeClasses,
    inline_and_collapse_properties::InlineAndCollapseProperties,
    node_traversal::{Callback, NodeTraversal},
    replace_strings::ReplaceStrings,
    source_information_annotator::SourceInformationAnnotator,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc};

const REPLACE_STRINGS: &str = "com.google.javascript.jscomp.ReplaceStrings";
const GET_PROCESSOR_PASS: &str =
    "com.google.javascript.jscomp.ReplaceStringsTest_Helpers$GetProcessorPass";

// port: ReplayValues.Undecodable#Undecodable
fn bad(s: &str) -> Throwable {
    Throwable::HarnessError(format!("ReplaceStringsTest_Helpers replay adapter: {s}"))
}

/// The real `ReplaceStrings` pass inside its replay adapter; `compiler` is the Java field
/// `compiler` (the Rust pass takes the compiler per call).
pub struct NativeReplaceStrings {
    compiler: DslValue,
    pass: ReplaceStrings,
}

// port: ImmutableSet#copyOf (Guava runtime class by size)
fn immutable_set_class(len: usize) -> &'static str {
    if len == 1 {
        "com.google.common.collect.SingletonImmutableSet"
    } else {
        "com.google.common.collect.RegularImmutableSet"
    }
}

// port: UnitRecorder#fields (plain Java object)
fn java_object(class: &str, fields: IndexMap<String, DslValue>) -> DslValue {
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: class.into(),
        fields,
        field_types: IndexMap::<_, _>::default(),
    })))
}

impl NativeObject for NativeReplaceStrings {
    fn class_name(&self) -> &str {
        REPLACE_STRINGS
    }
    fn is_instance_of(&self, class: &str) -> bool {
        matches!(
            class,
            REPLACE_STRINGS
                | "com.google.javascript.jscomp.CompilerPass"
                | "com.google.javascript.jscomp.NodeTraversal$Callback"
                | "java.lang.Object"
        )
    }
    // port: ReplaceStrings#getStringMap (UnitRecorder result producer)
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match method {
            "getStringMap" => Ok(crate::replay::native_finalize_rename::variable_map_value(
                self.pass.get_string_map(),
            )),
            _ => Err(Throwable::Unported(format!("{REPLACE_STRINGS}#{method}"))),
        }
    }
    // port: UnitRecorder#fields (ReplaceStrings: placeholderToken, compiler, functions,
    // nameGenerator, results in declaration order)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let f = self.pass.replay_fields();
        let mut functions = Vec::new();
        for (name, config) in f.functions {
            let c = config.replay_fields();
            functions.push((
                DslValue::String(name.clone()),
                java_object(
                    "com.google.javascript.jscomp.ReplaceStrings$Config",
                    IndexMap::<_, _>::from_iter([
                        ("name".into(), DslValue::String(c.name.clone())),
                        (
                            "parameters".into(),
                            DslValue::List(
                                c.parameters.iter().map(|p| DslValue::Int(*p)).collect(),
                            ),
                        ),
                        (
                            "excludedFilenameSuffixes".into(),
                            DslValue::Typed {
                                class: immutable_set_class(c.excluded_filename_suffixes.len())
                                    .into(),
                                value: Box::new(DslValue::Set(
                                    c.excluded_filename_suffixes
                                        .iter()
                                        .map(|s| DslValue::String(JsString::from(s.as_str())))
                                        .collect(),
                                )),
                            },
                        ),
                    ]),
                ),
            ));
        }
        // ReplaceStrings#createNameGenerator passes ImmutableSet.of() as the reserved names, which
        // DefaultNameGenerator#reset stores as is.
        let mut name_generator = encode_name_generator(f.name_generator)?;
        if let DslValue::Object(o) = &mut name_generator {
            let mut o = o.borrow_mut();
            if let Some(reserved) = o.fields.get_mut("reservedNames") {
                let len = f
                    .name_generator
                    .replay_fields()
                    .reserved_names
                    .read()
                    .unwrap()
                    .len();
                *reserved = DslValue::Typed {
                    class: immutable_set_class(len).into(),
                    value: Box::new(reserved.clone()),
                };
            }
        }
        let results = f
            .results
            .iter()
            .map(|(original, result)| {
                (
                    DslValue::String(original.clone()),
                    java_object(
                        "com.google.javascript.jscomp.ReplaceStrings$Result",
                        IndexMap::<_, _>::from_iter([
                            ("original".into(), DslValue::String(result.original.clone())),
                            (
                                "replacement".into(),
                                DslValue::String(result.replacement.clone()),
                            ),
                            (
                                "didReplacement".into(),
                                DslValue::Bool(result.did_replacement),
                            ),
                        ]),
                    ),
                )
            })
            .collect();
        Ok(IndexMap::<_, _>::from_iter([
            (
                "placeholderToken".into(),
                DslValue::String(f.placeholder_token.clone()),
            ),
            ("compiler".into(), self.compiler.clone()),
            ("functions".into(), DslValue::Map(functions)),
            ("nameGenerator".into(), name_generator),
            ("results".into(), DslValue::Map(results)),
        ]))
    }
    // port: ReplaceStrings#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.pass.process(compiler, externs, root);
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayDsl#invoke (List<String> argument cast)
fn arg_string_list(args: &[DslValue], i: usize) -> Result<Vec<String>, Throwable> {
    let Some(v) = args.get(i) else {
        return Err(bad(&format!("missing argument {i}")));
    };
    let (DslValue::List(items) | DslValue::Set(items)) = v.untyped() else {
        return Err(bad(&format!("argument {i} is not a list")));
    };
    items
        .iter()
        .map(|item| match item.untyped() {
            DslValue::String(s) => Ok(s.to_string()),
            _ => Err(bad(&format!("argument {i} has a non-string element"))),
        })
        .collect()
}

// port: ReplayDsl#invoke (String argument cast)
fn arg_string(args: &[DslValue], i: usize) -> Result<String, Throwable> {
    match args.get(i).map(DslValue::untyped) {
        Some(DslValue::String(s)) => Ok(s.to_string()),
        _ => Err(bad(&format!("argument {i} is not a string"))),
    }
}

// port: ReplaceStrings#ReplaceStrings
fn native_replace_strings(
    args: &[DslValue],
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let placeholder_token = arg_string(args, 1)?;
    let functions_to_inspect = arg_string_list(args, 2)?;
    let pass = ReplaceStrings::new(compiler, &placeholder_token, &functions_to_inspect);
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeReplaceStrings {
            compiler: args[0].clone(),
            pass,
        },
    ))))
}

// port: ReplaceStrings#ReplaceStrings
pub fn new_replace_strings(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let Some(DslValue::Compiler(c)) = args.first().map(DslValue::untyped) else {
        return Err(bad("ReplaceStrings needs the compiler"));
    };
    let c = c.clone();
    native_replace_strings(&args, &mut c.borrow_mut())
}

// port: ReplaceStrings#ReplaceStrings (compiler loaned to a running factory)
pub fn new_replace_strings_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let Some(DslValue::Compiler(_)) = args.first().map(DslValue::untyped) else {
        return Err(bad("ReplaceStrings needs the compiler"));
    };
    native_replace_strings(&args, compiler)
}

/// `private static class Renamer extends AbstractPostOrderCallback`.
struct Renamer;

impl Callback for Renamer {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ReplaceStringsTest_Helpers.Renamer#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_name(t) || n.is_get_prop(t) {
            let original_name = n.get_string(t);
            n.set_original_name(t, Some(original_name.clone()));
            n.set_string(t, JsString::from("renamed_").concat(&original_name));
            t.report_code_change();
        }
    }
}

/// The anonymous `CompilerPass` returned by `getProcessor` (`ReplaceStringsTest$1`).
struct GetProcessorPass {
    compiler: DslValue,
    pass: DslValue,
    rename: bool,
    run_disambiguate_properties: bool,
}

impl NativeObject for GetProcessorPass {
    fn class_name(&self) -> &str {
        GET_PROCESSOR_PASS
    }
    fn is_instance_of(&self, class: &str) -> bool {
        matches!(
            class,
            GET_PROCESSOR_PASS | "com.google.javascript.jscomp.CompilerPass" | "java.lang.Object"
        )
    }
    // port: UnitRecorder#fields (GetProcessorPass: compiler, pass, rename,
    // runDisambiguateProperties in declaration order)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([
            ("compiler".into(), self.compiler.clone()),
            ("pass".into(), self.pass.clone()),
            ("rename".into(), DslValue::Bool(self.rename)),
            (
                "runDisambiguateProperties".into(),
                DslValue::Bool(self.run_disambiguate_properties),
            ),
        ]))
    }
    // port: ReplaceStringsTest_Helpers.GetProcessorPass#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        js: NodeId,
    ) -> Result<(), Throwable> {
        if self.rename {
            NodeTraversal::traverse(compiler, js, &mut Renamer);
        }
        Es6NormalizeClasses::new(compiler).process(compiler, externs, js);
        InlineAndCollapseProperties::builder(compiler)
            .set_property_collapse_level(PropertyCollapseLevel::ALL)
            .set_chunk_output_type(ChunkOutputType::GLOBAL_NAMESPACE)
            .set_have_modules_been_rewritten(false)
            .set_module_resolution_mode(ResolutionMode::BROWSER)
            .build()
            .process(compiler, externs, js);
        if self.run_disambiguate_properties {
            let mut sia = SourceInformationAnnotator::create();
            NodeTraversal::traverse(compiler, js, &mut sia);

            DisambiguateProperties::new(
                compiler,
                IndexSet::<_>::from_iter([JsString::from("foobar")]),
            )
            .process(compiler, externs, js);
        }
        let DslValue::Native(pass) = &self.pass else {
            return Err(bad("pass is not a native ReplaceStrings"));
        };
        pass.borrow_mut().process(compiler, externs, js)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplaceStringsTest_Helpers.GetProcessorPass#GetProcessorPass
pub fn new_get_processor_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [compiler, pass, rename, run_disambiguate_properties] = args.as_slice() else {
        return Err(bad("GetProcessorPass takes four arguments"));
    };
    let DslValue::Compiler(_) = compiler.untyped() else {
        return Err(bad("argument 0 is not the compiler"));
    };
    let DslValue::Native(_) = pass.untyped() else {
        return Err(bad("argument 1 is not a ReplaceStrings"));
    };
    let rename = bool::decode_value(rename)?;
    let run_disambiguate_properties = bool::decode_value(run_disambiguate_properties)?;
    Ok(DslValue::Native(Rc::new(RefCell::new(GetProcessorPass {
        compiler: compiler.untyped().clone(),
        pass: pass.untyped().clone(),
        rename,
        run_disambiguate_properties,
    }))))
}
