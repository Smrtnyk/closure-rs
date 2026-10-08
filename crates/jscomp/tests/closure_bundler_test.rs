/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/deps/ClosureBundlerTest.java.

//! Port of deps/ClosureBundlerTest.java.
use closure_jscomp::{
    bundle::transpilation_exception::TranspilationException,
    deps::{
        closure_bundler::ClosureBundler, dependency_info::DependencyInfo,
        simple_dependency_info::SimpleDependencyInfo,
    },
    transpile::{
        base_transpiler::{ES5_TRANSPILER, LATEST_TRANSPILER},
        transpile_result::TranspileResult,
        transpiler::Transpiler,
    },
};
use closure_rhino::{java_lang::uri::URI, js_string::JsString};
use indexmap::IndexMap;
use std::sync::{Arc, LazyLock};

// port: ClosureBundlerTest#MODULE
static MODULE: LazyLock<SimpleDependencyInfo> = LazyLock::new(|| {
    SimpleDependencyInfo::builder("", "")
        .set_goog_module(true)
        .build()
});

// port: ClosureBundlerTest#TRADITIONAL
static TRADITIONAL: LazyLock<SimpleDependencyInfo> =
    LazyLock::new(|| SimpleDependencyInfo::builder("", "").build());

fn module() -> &'static dyn DependencyInfo {
    &*MODULE
}

fn traditional() -> &'static dyn DependencyInfo {
    &*TRADITIONAL
}

/// `Mockito.mock(Transpiler.class, RETURNS_SMART_NULLS)` with the stubbings of testTranspilation.
struct MockTranspiler {
    uri: URI,
    input: &'static str,
}

impl Transpiler for MockTranspiler {
    // port: ClosureBundlerTest#testTranspilation (when(transpiler.transpile(uri, input)))
    fn transpile(&self, path: URI, code: &JsString) -> TranspileResult {
        assert!(
            path == self.uri && *code == self.input,
            "SmartNullPointerException: unstubbed Transpiler#transpile"
        );
        TranspileResult::new(self.uri.clone(), self.input, "TRANSPILED;", "")
    }
    // port: ClosureBundlerTest#testTranspilation (when(transpiler.runtime()))
    fn runtime(&self) -> JsString {
        JsString::from("RUNTIME;")
    }
}

// port: ClosureBundlerTest#testGoogModule
#[test]
fn test_goog_module() {
    let mut sb = String::new();
    ClosureBundler::new_default()
        .append_to(&mut sb, module(), "\"a string\"")
        .unwrap();
    assert_eq!(
        sb,
        "goog.loadModule(function(exports) {'use strict';\"a string\"\n;return exports;});\n"
    );
}

// port: ClosureBundlerTest#testGoogModuleWithSourceURL
#[test]
fn test_goog_module_with_source_url() {
    let mut sb = String::new();
    ClosureBundler::new_default()
        .use_eval(true)
        .with_source_url(Some("URL"))
        .append_to(&mut sb, module(), "\"a string\"")
        .unwrap();
    assert_eq!(
        sb,
        "goog.loadModule(\"\\x22a string\\x22\\n//# sourceURL\\x3dURL\\n\");\n"
    );
}

// port: ClosureBundlerTest#testGoogModuleWithEval
#[test]
fn test_goog_module_with_eval() {
    let mut sb = String::new();
    ClosureBundler::new_default()
        .use_eval(true)
        .append_to(&mut sb, module(), "\"a string\"")
        .unwrap();
    assert_eq!(sb, "goog.loadModule(\"\\x22a string\\x22\");\n");
}

// port: ClosureBundlerTest#testGoogModuleWithEvalWithURL
#[test]
fn test_goog_module_with_eval_with_url() {
    let mut sb = String::new();
    ClosureBundler::new_default()
        .use_eval(true)
        .with_source_url(Some("URL"))
        .append_to(&mut sb, module(), "\"a string\"")
        .unwrap();
    assert_eq!(
        sb,
        "goog.loadModule(\"\\x22a string\\x22\\n//# sourceURL\\x3dURL\\n\");\n"
    );
}

// port: ClosureBundlerTest#testTraditional
#[test]
fn test_traditional() {
    let mut sb = String::new();
    ClosureBundler::new_default()
        .append_to(&mut sb, traditional(), "\"a string\"")
        .unwrap();
    assert_eq!(sb, "\"a string\"");
}

// port: ClosureBundlerTest#testTraditionalWithSourceURL
#[test]
fn test_traditional_with_source_url() {
    let mut sb = String::new();
    ClosureBundler::new_default()
        .with_source_url(Some("URL"))
        .append_to(&mut sb, traditional(), "\"a string\"")
        .unwrap();
    assert_eq!(sb, "\"a string\"\n//# sourceURL=URL\n");
}

// port: ClosureBundlerTest#testTraditionalWithEval
#[test]
fn test_traditional_with_eval() {
    let mut sb = String::new();
    ClosureBundler::new_default()
        .use_eval(true)
        .append_to(&mut sb, traditional(), "\"a string\"")
        .unwrap();
    assert_eq!(
        sb,
        "eval(this.CLOSURE_EVAL_PREFILTER(\"\\x22a string\\x22\"));\n"
    );
}

// port: ClosureBundlerTest#testTraditionalWithEvalWithSourceUrl
#[test]
fn test_traditional_with_eval_with_source_url() {
    let mut sb = String::new();
    ClosureBundler::new_default()
        .use_eval(true)
        .with_source_url(Some("URL"))
        .append_to(&mut sb, traditional(), "\"a string\"")
        .unwrap();
    assert_eq!(
        sb,
        "eval(this.CLOSURE_EVAL_PREFILTER(\"\\x22a string\\x22\\n//# sourceURL\\x3dURL\\n\"));\n"
    );
}

// port: ClosureBundlerTest#testTranspilation
#[test]
fn test_transpilation() {
    let input = "goog.module('Foo');\nclass Foo {}";
    let uri = URI::new("foo.js").unwrap();

    let transpiler: Arc<dyn Transpiler> = Arc::new(MockTranspiler { uri, input });

    let bundler = ClosureBundler::new_with_transpiler(transpiler).with_path("foo.js");
    let mut sb = String::new();
    bundler.append_runtime_to(&mut sb).unwrap();
    bundler.append_to(&mut sb, module(), input).unwrap();
    assert!(sb.starts_with("RUNTIME;"), "{sb}");
    // Call endsWith because the ES6 module runtime is also injected.
    assert!(
        sb.ends_with(
            "goog.loadModule(function(exports) {'use strict';TRANSPILED;\n;return exports;});\n"
        ),
        "{sb}"
    );

    // Without calling appendRuntimeTo(), the runtime is not included anymore.
    sb = String::new();
    bundler.append_to(&mut sb, module(), input).unwrap();
    assert_eq!(
        sb,
        "goog.loadModule(function(exports) {'use strict';TRANSPILED;\n;return exports;});\n"
    );
}

// port: ClosureBundlerTest#testEs6Module
#[test]
fn test_es6_module() {
    let input = "import {x} from './other.js';\n\
                 export {x as y};\n\
                 let local;\n\
                 export function foo() { return local; }\n";
    let bundler = ClosureBundler::new_with_transpiler(LATEST_TRANSPILER.clone())
        .with_path("nested/path/foo.js");
    let mut sb = String::new();
    bundler.append_runtime_to(&mut sb).unwrap();
    let mut load_flags = IndexMap::new();
    load_flags.insert("module".to_string(), "es6".to_string());
    bundler
        .append_to(
            &mut sb,
            &SimpleDependencyInfo::builder("", "")
                .set_load_flags(load_flags)
                .build(),
            input,
        )
        .unwrap();
    let result = sb;
    // ES6 module runtime should be injected.
    assert!(
        result.contains("$jscomp.require = createRequire();"),
        "{result}"
    );
    assert!(result.starts_with("var $jscomp"), "{result}");
    assert!(
        result.ends_with(concat!(
            "$jscomp.registerAndLoadModule(function($$require, $$exports, $$module) {\n",
            "  function foo() {\n",
            "    return local;\n",
            "  }\n",
            "  Object.defineProperties($$exports, {foo:{enumerable:true, get:function() {\n",
            "    return foo;\n",
            "  }}, y:{enumerable:true, get:function() {\n",
            "    return module$nested$path$other.x;\n",
            "  }}});\n",
            "  var module$nested$path$other = $$require(\"nested/path/other.js\");\n",
            "  let local;\n",
            "}, \"nested/path/foo.js\", [\"nested/path/other.js\"]);\n",
        )),
        "{result}"
    );
}

// port: ClosureBundlerTest#testPassThroughIfNoTranspilationNeeded
#[test]
fn test_pass_through_if_no_transpilation_needed() {
    let input = "/** Hello Comments! */ const s = 0;\n  let intended;";
    let bundler = ClosureBundler::new_with_transpiler(LATEST_TRANSPILER.clone());
    let mut sb = String::new();
    bundler
        .append_to(
            &mut sb,
            &SimpleDependencyInfo::builder("", "").build(),
            input,
        )
        .unwrap();
    assert_eq!(sb, input);
}

// port: ClosureBundlerTest#testCommentsAndFormattingRemovedWithTranspilation
#[test]
fn test_comments_and_formatting_removed_with_transpilation() {
    let input = "/** Hello Comments! */ const s = 0;\n  let intended;";
    let bundler = ClosureBundler::new_with_transpiler(ES5_TRANSPILER.clone());
    let mut sb = String::new();
    bundler
        .append_to(
            &mut sb,
            &SimpleDependencyInfo::builder("", "").build(),
            input,
        )
        .unwrap();
    assert_eq!(sb, "var s = 0;\nvar intended;\n");
}

// port: ClosureBundlerTest#testFullWidthLowLineWithDefaultTranspilerIsOkay
#[test]
fn test_full_width_low_line_with_default_transpiler_is_okay() {
    // The last character is something the compiler doesn't handle correctly
    let input = "var ａｅｓｔｈｅｔｉｃ＿";
    let bundler = ClosureBundler::new_default();
    let mut sb = String::new();
    bundler
        .append_to(
            &mut sb,
            &SimpleDependencyInfo::builder("", "").build(),
            input,
        )
        .unwrap();
    assert_eq!(sb, input);
}

// TODO(johnplaisted): If / when the compiler can parse full width low line in identifiers
// this should be okay to be transpiled.
// port: ClosureBundlerTest#testFullWidthLowLineInTranspiledCodeIsError
#[test]
fn test_full_width_low_line_in_transpiled_code_is_error() {
    // The last character is something the compiler doesn't handle correctly
    let input = "let ａｅｓｔｈｅｔｉｃ＿";
    let bundler = ClosureBundler::new_with_transpiler(ES5_TRANSPILER.clone());
    let mut sb = String::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bundler.append_to(
            &mut sb,
            &SimpleDependencyInfo::builder("", "").build(),
            input,
        )
    }));
    match result {
        Ok(_) => panic!("Expected an exception"),
        Err(payload) => {
            let e = payload
                .downcast::<TranspilationException>()
                .unwrap_or_else(|_| panic!("expected a TranspilationException"));
            assert!(
                e.get_message().contains(
                    "Parse error. Character '＿' (U+FF3F) is not a valid identifier start char"
                ),
                "{}",
                e.get_message()
            );
        }
    }
}
