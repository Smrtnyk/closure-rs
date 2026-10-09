/*
 * Copyright 2018 The Closure Compiler Authors.
 * Copyright 2022 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/testing/TestExternsBuilder.java,
//   test/com/google/javascript/jscomp/UnitTestUtils.java.

//! Test output generic-name mapping, retaining Java UTF-16 and 32-bit wrapping.
use crate::{
    compiler_test_case::{Expected, FlatSources},
    jscomp_api::{SourceFile, StaticSourceFile},
    replay::replay_dsl::DslValue,
    throwable::{Throwable, check_state},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use std::sync::Arc;
// port: TestExternsBuilder#addArray / addIterable / addObject / addUndefined / addFunction / addString / addExtra / build
pub fn externs_builder_call(
    builder: &DslValue,
    method: &str,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    match builder {
        DslValue::Native(object) => {
            let result = object.borrow_mut().call(method, args)?;
            Ok(if method.starts_with("add") {
                builder.clone()
            } else {
                result
            })
        }
        _ => Err(Throwable::HarnessError(
            "TestExternsBuilder adapter must be a native object".into(),
        )),
    }
}
// port: UnitTestUtils#updateGenericVarNamesInExpectedFiles(FlatSources,Expected,ImmutableMap,Map)
pub fn update_generic_var_names_in_expected_files(
    inputs: &FlatSources,
    outputs: &Expected,
    prefixes: &IndexMap<String, String>,
    replacements: &mut IndexMap<String, String>,
) -> Result<Vec<Arc<SourceFile>>, Throwable> {
    let outputs = outputs
        .expected
        .as_ref()
        .ok_or_else(|| Throwable::HarnessError("expected sources missing".into()))?;
    if inputs.sources.len() != outputs.len() {
        return Err(Throwable::Exception {
            class: "java.lang.IllegalArgumentException".into(),
            message: None,
        });
    }
    let mut updated = vec![];
    for (input, output) in inputs.sources.iter().zip(outputs) {
        let hash = JsString::from(input.get_name()).hash_code();
        let hs = if hash < 0 {
            format!("m{}", hash.wrapping_neg())
        } else {
            hash.to_string()
        };
        let name = output.get_name();
        let mut code = output.get_code().map_err(|_| Throwable::Exception {
            class: "java.lang.RuntimeException".into(),
            message: Some(format!("Read error: {name}")),
        })?;
        for (short, prefix) in prefixes {
            let long = format!("{prefix}{hs}");
            code = replace(
                &code,
                &JsString::from(short.as_str()),
                &JsString::from(long.as_str()),
            );
            replacements.insert(long, short.clone());
        }
        updated.push(Arc::new(SourceFile::from_code(name, code)));
    }
    Ok(updated)
}
// port: String#replace(CharSequence,CharSequence)
pub fn replace(source: &JsString, from: &JsString, to: &JsString) -> JsString {
    let mut out = vec![];
    let (source, from, to) = (source.as_units(), from.as_units(), to.as_units());
    if from.is_empty() {
        out.extend_from_slice(to);
        for c in source {
            out.push(*c);
            out.extend_from_slice(to);
        }
        return JsString::from_units(out);
    }
    let mut i = 0;
    while i < source.len() {
        if source[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(source[i]);
            i += 1;
        }
    }
    JsString::from_units(out)
}
// port: UnitTestUtils#updateGenericVarNamesInExpectedFiles(FlatSources,Expected,ImmutableMap)
pub fn update_generic_var_names(
    inputs: &FlatSources,
    outputs: &Expected,
    prefixes: &IndexMap<String, String>,
) -> Result<Vec<Arc<SourceFile>>, Throwable> {
    check_state(outputs.expected.is_some(), "")?;
    update_generic_var_names_in_expected_files(
        inputs,
        outputs,
        prefixes,
        &mut IndexMap::<_, _>::default(),
    )
}
