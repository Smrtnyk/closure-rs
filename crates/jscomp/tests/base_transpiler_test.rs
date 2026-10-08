/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/transpile/BaseTranspiler.java,
//   test/com/google/javascript/jscomp/transpile/BaseTranspilerTest.java.

//! Port of transpile/BaseTranspilerTest.java.
use closure_jscomp::{
    bundle::transpilation_exception::TranspilationException,
    transpile::{
        base_transpiler::{BaseTranspiler, CompileResult, CompilerSupplier, CompilerSupplierApi},
        transpile_result::TranspileResult,
        transpiler::Transpiler,
    },
};
use closure_rhino::{java_lang::uri::URI, js_string::JsString};
use std::sync::Arc;

/// `@Mock(answer = RETURNS_SMART_NULLS) BaseTranspiler.CompilerSupplier mockCompiler` with the
/// stubbings of one test: an unstubbed `runtime` returns the smart null `""`.
#[derive(Default)]
struct MockCompilerSupplier {
    compile: Option<(URI, &'static str, CompileResult)>,
    runtime: Option<(&'static str, &'static str)>,
}

impl CompilerSupplierApi for MockCompilerSupplier {
    // port: BaseTranspilerTest#mockCompiler (Mockito stub of CompilerSupplier#compile)
    fn compile(&self, path: &URI, code: &JsString) -> CompileResult {
        match &self.compile {
            Some((p, c, result)) if p == path && *code == *c => result.clone(),
            _ => panic!("SmartNullPointerException: unstubbed CompilerSupplier#compile"),
        }
    }
    // port: BaseTranspilerTest#mockCompiler (Mockito stub of CompilerSupplier#runtime)
    fn runtime(&self, library: &str) -> JsString {
        match self.runtime {
            Some((l, result)) if l == library => JsString::from(result),
            _ => JsString::from(""),
        }
    }
}

struct Fixture {
    transpiler: BaseTranspiler,
    compiler: CompilerSupplier,
}

fn foo_js() -> URI {
    URI::new("foo.js").unwrap()
}

fn source_js() -> URI {
    URI::new("source.js").unwrap()
}

// port: BaseTranspilerTest#setUp
fn set_up(mock_compiler: MockCompilerSupplier) -> Fixture {
    Fixture {
        transpiler: BaseTranspiler::new(Arc::new(mock_compiler), "es6_runtime"),
        compiler: CompilerSupplier::new(),
    }
}

// Tests for BaseTranspiler

// port: BaseTranspilerTest#testTranspiler_transpile
#[test]
fn test_transpiler_transpile() {
    let t = set_up(MockCompilerSupplier {
        compile: Some((
            foo_js(),
            "bar",
            CompileResult::new("result", true, "srcmap"),
        )),
        ..Default::default()
    });
    assert_eq!(
        t.transpiler.transpile(foo_js(), &JsString::from("bar")),
        TranspileResult::new(foo_js(), "bar", "result", "srcmap")
    );
}

// port: BaseTranspilerTest#testTranspiler_noTranspilation
#[test]
fn test_transpiler_no_transpilation() {
    let t = set_up(MockCompilerSupplier {
        compile: Some((
            foo_js(),
            "bar",
            CompileResult::new("result", false, "srcmap"),
        )),
        ..Default::default()
    });
    assert_eq!(
        t.transpiler.transpile(foo_js(), &JsString::from("bar")),
        TranspileResult::new(foo_js(), "bar", "bar", "")
    );
}

// port: BaseTranspilerTest#testTranspiler_runtime
#[test]
fn test_transpiler_runtime() {
    let t = set_up(MockCompilerSupplier {
        runtime: Some(("es6_runtime", "$jscomp.es6();")),
        ..Default::default()
    });
    assert_eq!(t.transpiler.runtime(), JsString::from("$jscomp.es6();"));
}

// Tests for CompilerSupplier

// port: BaseTranspilerTest#testCompilerSupplier_compileChanged
#[test]
fn test_compiler_supplier_compile_changed() {
    let t = set_up(MockCompilerSupplier::default());
    let result = t
        .compiler
        .compile(&source_js(), &JsString::from("const x = () => 42;"));
    assert_eq!(
        result.source,
        JsString::from("var x = function() {\n  return 42;\n};\n")
    );
    assert!(result.transpiled);
    assert!(
        result
            .source_map
            .to_string_lossy()
            .contains("\"mappings\":\"AAAA,IAAMA,IAAIA,QAAA,EAAM;AAAA,SAAA,EAAA;AAAA,CAAhB;;\""),
        "{}",
        result.source_map
    );
}

// port: BaseTranspilerTest#testCompilerSupplier_compileNoChange
#[test]
fn test_compiler_supplier_compile_no_change() {
    let t = set_up(MockCompilerSupplier::default());
    let result = t
        .compiler
        .compile(&source_js(), &JsString::from("var x = 42;"));
    assert_eq!(result.source, JsString::from("var x = 42;\n"));
    assert!(!result.transpiled);
    assert!(result.source_map.is_empty());
}

// port: BaseTranspilerTest#testCompilerSupplier_error
#[test]
fn test_compiler_supplier_error() {
    let t = set_up(MockCompilerSupplier::default());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.compiler
            .compile(&source_js(), &JsString::from("cons x = () => 42;"))
    }));
    match outcome {
        Ok(_) => panic!("Expected an exception."),
        Err(payload) => assert!(
            payload.downcast_ref::<TranspilationException>().is_some(),
            "expected TranspilationException"
        ),
    }
}

// port: BaseTranspilerTest#testCompilerSupplier_runtime
#[test]
fn test_compiler_supplier_runtime() {
    let t = set_up(MockCompilerSupplier::default());
    let runtime = t.compiler.runtime("es6_runtime").to_string_lossy();
    assert!(runtime.contains("$jscomp.polyfill(\"Map\""), "{runtime}");
    assert!(runtime.contains("$jscomp.makeIterator"), "{runtime}");
    assert!(runtime.contains("$jscomp.inherits"), "{runtime}");
}

// port: BaseTranspilerTest#testCompilerSupplier_inlineSourceMap
#[test]
fn test_compiler_supplier_inline_source_map() {
    // This is a source map generated by
    // tsc --inlineSourceMap --inlineSources -t ES2017 source.ts
    // and the content of source.ts is "const x = () => 42".
    let t = set_up(MockCompilerSupplier::default());
    let result = t.compiler.compile(
        &source_js(),
        &JsString::from(concat!(
            "const x = () => 42;\n",
            "//# sourceMappingURL=data:application/json;base64,",
            "eyJ2ZXJzaW9uIjozLCJmaWxlIjoic291cmNlLmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsi",
            "c291cmNlLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBLE1BQU0sQ0FBQyxHQUFHLEdBQUcs",
            "RUFBRSxDQUFDLEVBQUUsQ0FBQyIsInNvdXJjZXNDb250ZW50IjpbImNvbnN0IHggPSAoKSA9PiA0Mjtc",
            "biJdfQ==",
        )),
    );
    assert_eq!(
        result.source,
        JsString::from("var x = function() {\n  return 42;\n};\n")
    );
    assert!(result.transpiled);
    // This should map back to the typescript source file.
    assert_eq!(
        result.source_map,
        JsString::from(concat!(
            "{\n",
            "\"version\":3,\n",
            "\"file\":\"/source.js\",\n",
            "\"lineCount\":4,\n",
            "\"mappings\":\"AAAA,IAAMA,IAAIA,QAAA,EAAM;AAAA,SAAA,EAAA;AAAA,CAAhB;;\",\n",
            "\"sources\":[\"/source.ts\"],\n",
            "\"sourcesContent\":[\"const x = () => 42;\\n\"],\n",
            "\"names\":[\"x\"]\n",
            "}\n",
        ))
    );
}
