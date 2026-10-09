/*
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
//   src/com/google/javascript/jscomp/ReplaceIdGenerators.java,
//   test/com/google/javascript/jscomp/ReplaceIdGeneratorsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of the replay helper `oracle/replay/helpers/.../ReplaceIdGeneratorsTest_Helpers.java` (DSL
//! name `ReplaceIdGeneratorsTest_Helpers.IdTestMap`), itself copied from
//! ReplaceIdGeneratorsTest.java's anonymous `RenamingMap`, plus the native `ReplaceIdGenerators`
//! constructor and its recorded result accessor `getSerializedIdMappings`.
use crate::{
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass, renaming_map::RenamingMap, renaming_token::RenamingToken,
    replace_id_generators::ReplaceIdGenerators,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc, sync::Arc};

const ID_TEST_MAP: &str = "com.google.javascript.jscomp.ReplaceIdGeneratorsTest_Helpers$IdTestMap";
const PASS: &str = "com.google.javascript.jscomp.ReplaceIdGenerators";
const RENAMING_TOKEN: &str = "com.google.javascript.jscomp.RenamingToken";

/// The anonymous RenamingMap `idTestMap` built in getProcessor.
#[derive(Clone, Copy)]
pub struct IdTestMap;

impl IdTestMap {
    // port: ReplaceIdGeneratorsTest_Helpers.IdTestMap#map
    const MAP: [(&'static str, &'static str); 2] = [("foo", ":foo:"), ("bar", ":bar:")];
}

impl RenamingMap for IdTestMap {
    // port: ReplaceIdGeneratorsTest_Helpers.IdTestMap#get
    fn get(&self, value: &JsString) -> Option<JsString> {
        let replacement = Self::MAP
            .iter()
            .find(|(k, _)| *value == **k)
            .map(|(_, v)| JsString::from(*v));
        Some(replacement.unwrap_or_else(|| JsString::from("unknown:").concat(value)))
    }
}

impl NativeObject for IdTestMap {
    fn class_name(&self) -> &str {
        ID_TEST_MAP
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == ID_TEST_MAP || class == "com.google.javascript.jscomp.RenamingMap"
    }
    // port: UnitRecorder#collect (no recorded result producer is reachable from this map)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplaceIdGeneratorsTest_Helpers.IdTestMap#IdTestMap
pub fn id_test_map(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(IdTestMap))))
}

/// The pass behind the DSL value, with `getSerializedIdMappings` for the post-call snapshot.
struct NativeReplaceIdGenerators(ReplaceIdGenerators);

impl NativeObject for NativeReplaceIdGenerators {
    fn class_name(&self) -> &str {
        PASS
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == PASS || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: ReplaceIdGenerators#getSerializedIdMappings
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match method {
            "getSerializedIdMappings" => Ok(DslValue::String(JsString::from(
                self.0.get_serialized_id_mappings(),
            ))),
            _ => Err(Throwable::Unported(format!("{PASS}#{method}"))),
        }
    }
    // port: UnitRecorder#collect (the processor's fields hold no other result producer)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: ReplaceIdGenerators#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.0.process(compiler, externs, root);
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayDsl#invoke (RenamingMap argument cast)
fn renaming_map(value: &DslValue) -> Result<Arc<dyn RenamingMap + Send + Sync>, Throwable> {
    match value {
        DslValue::Enum { class, name } if class == RENAMING_TOKEN => {
            let token = RenamingToken::value_of(name).ok_or_else(bad)?;
            Ok(Arc::new(token))
        }
        DslValue::Native(o) => {
            let mut o = o.borrow_mut();
            let map = o.as_any_mut().downcast_mut::<IdTestMap>().ok_or_else(bad)?;
            Ok(Arc::new(*map))
        }
        _ => Err(bad()),
    }
}

// port: ReplaceIdGenerators#ReplaceIdGenerators
pub fn replace_id_generators(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(c),
        DslValue::Bool(template_literals_are_transpiled),
        id_gens,
        DslValue::Bool(generate_pseudo_names),
        previous_map_serialized,
        DslValue::Null,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let id_gens = match id_gens {
        DslValue::Typed { value, .. } => value.as_ref(),
        other => other,
    };
    let id_gens = match id_gens {
        DslValue::Null => None,
        DslValue::Map(entries) => {
            let mut map: IndexMap<String, Arc<dyn RenamingMap + Send + Sync>> =
                IndexMap::<_, _>::default();
            for (k, v) in entries {
                let DslValue::String(k) = k else {
                    return Err(bad());
                };
                map.insert(k.to_string(), renaming_map(v)?);
            }
            Some(map)
        }
        _ => return Err(bad()),
    };
    let previous_map_serialized = match previous_map_serialized {
        DslValue::Null => None,
        DslValue::String(s) => Some(s.to_string()),
        _ => return Err(bad()),
    };
    let pass = ReplaceIdGenerators::new(
        &c.borrow(),
        *template_literals_are_transpiled,
        id_gens.as_ref(),
        *generate_pseudo_names,
        previous_map_serialized.as_deref(),
        None,
    );
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeReplaceIdGenerators(pass),
    ))))
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
