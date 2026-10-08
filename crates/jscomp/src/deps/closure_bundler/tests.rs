/*
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/bundle/Transpiler.java,
//   src/com/google/javascript/jscomp/deps/ClosureBundler.java.

use super::*;
use crate::{
    deps::simple_dependency_info::SimpleDependencyInfo,
    transpile::transpile_result::TranspileResult,
};
struct TestTranspiler {
    map: &'static str,
    output: &'static str,
    runtime: &'static str,
}
impl Transpiler for TestTranspiler {
    // port: Transpiler#transpile (test double for supported constructors)
    fn transpile(&self, path: URI, code: &JsString) -> TranspileResult {
        TranspileResult::new(path, code.clone(), self.output, self.map)
    }
    // port: Transpiler#runtime (test double for supported constructors)
    fn runtime(&self) -> JsString {
        self.runtime.into()
    }
}
// port: ClosureBundler#appendTo (Rust verification of NULL reference identity and shared cache)
#[test]
fn null_identity_and_shared_source_maps() {
    let es6: Arc<dyn Transpiler> = Arc::new(TestTranspiler {
        map: "map",
        output: "ES6",
        runtime: "ES6_RUNTIME",
    });
    let info = SimpleDependencyInfo::builder("", "")
        .set_load_flags(IndexMap::from([("module".into(), "es6".into())]))
        .build();
    let original = ClosureBundler::new(NULL.clone(), es6.clone());
    let copy = original
        .with_path("foo.js")
        .embed_sourcemap()
        .disable_j2cl_minifier();
    let mut out = String::new();
    copy.append_to(&mut out, &info, "input").unwrap();
    assert_eq!(
        out,
        "ES6\n//# sourceMappingURL=data:application/json;base64,bWFw\n"
    );
    assert_eq!(original.get_source_map("foo.js"), "map");
    assert_eq!(original.get_source_map("absent"), "");
    let lookalike: Arc<dyn Transpiler> = Arc::new(TestTranspiler {
        map: "",
        output: "input",
        runtime: "",
    });
    out.clear();
    original
        .with_transpiler(lookalike)
        .append_to(&mut out, &info, "input")
        .unwrap();
    assert_eq!(out, "input");
    out.clear();
    original.append_runtime_to(&mut out).unwrap();
    assert!(out.starts_with("ES6_RUNTIMEthis.CLOSURE_EVAL_PREFILTER"));
}
// port: ClosureBundler.EvalMode#appendGoogModule (Rust verification of UTF-16 and sourceURL placement)
#[test]
fn utf16_output_and_source_url_placement() {
    let input = JsString::from_units(vec![b'"' as u16, 0xd800, b'"' as u16]);
    let info = SimpleDependencyInfo::builder("", "")
        .set_goog_module(true)
        .build();
    let bundler = ClosureBundler::new(NULL.clone(), NULL.clone()).with_source_url(Some("URL"));
    let mut normal = JsString::from("");
    bundler
        .append_to(&mut normal, &info, input.clone())
        .unwrap();
    assert_eq!(
        normal,
        JsString::from("goog.loadModule(function(exports) {'use strict';")
            .concat(&input)
            .concat(&JsString::from(
                "\n;return exports;});\n\n//# sourceURL=URL\n"
            ))
    );
    let mut eval = String::new();
    bundler
        .use_eval(true)
        .append_to(&mut eval, &info, input)
        .unwrap();
    assert_eq!(
        eval,
        "goog.loadModule(\"\\x22\\ud800\\x22\\n//# sourceURL\\x3dURL\\n\");\n"
    );
}
#[test]
#[should_panic(
    expected = "java.net.URISyntaxException: Illegal character in path at index 3: bad path"
)]
// port: ClosureBundler#transpile (Rust verification of Java exception wrapping)
fn invalid_uri_message() {
    ClosureBundler::new(NULL.clone(), NULL.clone())
        .with_path("bad path")
        .append_to(
            &mut String::new(),
            &SimpleDependencyInfo::builder("", "").build(),
            "",
        )
        .unwrap();
}
struct Es6FixtureTranspiler;
impl Transpiler for Es6FixtureTranspiler {
    // port: Transpiler#transpile (Java fixture test double)
    fn transpile(&self, path: URI, code: &JsString) -> TranspileResult {
        TranspileResult::new(
            path,
            code.clone(),
            JsString::from("ES6:").concat(code),
            "{\"version\":3}",
        )
    }
    // port: Transpiler#runtime (Java fixture test double)
    fn runtime(&self) -> JsString {
        "ES6_RUNTIME;".into()
    }
}
// port: ClosureBundler#appendTo, appendRuntimeTo (Java-generated fixture for the supported constructor)
#[test]
fn java_bundler_differential() {
    for line in include_str!("testdata/bundler.tsv").lines() {
        let f: Vec<_> = line.split('\t').collect();
        let module: u8 = f[1].parse().unwrap();
        let flags = if module == 0 {
            IndexMap::new()
        } else {
            IndexMap::from([(
                "module".into(),
                if module == 1 { "goog" } else { "es6" }.into(),
            )])
        };
        let info = SimpleDependencyInfo::builder("", "")
            .set_load_flags(flags)
            .build();
        let base = ClosureBundler::new(NULL.clone(), Arc::new(Es6FixtureTranspiler));
        let copy = base
            .with_path("foo.js")
            .use_eval(f[0] == "true")
            .with_source_url(Some("URL?x#y"))
            .embed_sourcemap();
        let mut output = JsString::from("");
        copy.append_to(&mut output, &info, decode(f[2])).unwrap();
        let mut runtime = JsString::from("");
        copy.append_runtime_to(&mut runtime).unwrap();
        assert_eq!(output, decode(f[3]));
        assert_eq!(runtime, decode(f[4]));
        assert_eq!(base.get_source_map("foo.js"), decode(f[5]));
    }
}
// port: String#charAt (fixture encoding)
fn decode(s: &str) -> JsString {
    JsString::from_units(
        (0..s.len())
            .step_by(4)
            .map(|i| u16::from_str_radix(&s[i..i + 4], 16).unwrap())
            .collect::<Vec<_>>(),
    )
}
