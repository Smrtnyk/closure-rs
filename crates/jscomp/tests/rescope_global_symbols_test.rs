/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/RescopeGlobalSymbolsTest.java.

#[path = "support/pass_test_case.rs"]
mod pass_test_case;

use closure_jscomp::{
    Compiler,
    compiler_options::{LanguageMode, OptimizeLocalAccess},
    deps::module_loader::LOAD_WARNING,
    diagnostic_type::DiagnosticType,
    rescope_global_symbols::RescopeGlobalSymbols,
    rhino_error_reporter::PARSE_ERROR,
    source_file::SourceFile,
};
use closure_testing::testing::js_chunk_graph_builder::JSChunkGraphBuilder;
use pass_test_case::{
    PassTestCase, Sources, error, expected, expected_many, externs, srcs, srcs_chunks,
};
use std::sync::Arc;

const NAMESPACE: &str = "_";

/// Java's test(String, String) and test(Sources, Expected) overloads.
trait IntoSources {
    fn into_sources(self) -> Sources;
}
impl IntoSources for &str {
    fn into_sources(self) -> Sources {
        srcs(self)
    }
}
impl IntoSources for Sources {
    fn into_sources(self) -> Sources {
        self
    }
}
trait IntoExpected {
    fn into_expected(self) -> Vec<Arc<SourceFile>>;
}
impl IntoExpected for &str {
    fn into_expected(self) -> Vec<Arc<SourceFile>> {
        expected(self)
    }
}
impl IntoExpected for Vec<Arc<SourceFile>> {
    fn into_expected(self) -> Vec<Arc<SourceFile>> {
        self
    }
}

struct T {
    base: PassTestCase,
    assume_cross_chunk_names: bool,
    optimize_local_access: OptimizeLocalAccess,
}

impl T {
    // port: RescopeGlobalSymbolsTest#setUp
    fn new() -> Self {
        Self {
            base: PassTestCase::new(""),
            assume_cross_chunk_names: true,
            optimize_local_access: OptimizeLocalAccess::DISABLED,
        }
    }
    // port: RescopeGlobalSymbolsTest#getProcessor
    fn get_processor(&self) -> impl FnMut(&mut Compiler) -> RescopeGlobalSymbols + use<> {
        let assume_cross_chunk_names = self.assume_cross_chunk_names;
        let optimize_local_access = self.optimize_local_access;
        move |_compiler| {
            RescopeGlobalSymbols::new_with_add_extern(
                NAMESPACE,
                false,
                assume_cross_chunk_names,
                optimize_local_access,
            )
        }
    }
    fn test(&self, srcs: impl IntoSources, expected: impl IntoExpected) {
        self.base.test(
            self.get_processor(),
            srcs.into_sources(),
            expected.into_expected(),
        );
    }
    fn test_externs(
        &self,
        externs: Vec<Arc<SourceFile>>,
        srcs: impl IntoSources,
        expected: impl IntoExpected,
    ) {
        self.base.test_with_externs(
            externs,
            self.get_processor(),
            srcs.into_sources(),
            expected.into_expected(),
        );
    }
    fn test_same(&self, srcs: impl IntoSources) {
        self.base
            .test_same(self.get_processor(), srcs.into_sources());
    }
    fn test_error(&self, srcs: impl IntoSources, diagnostic: &'static DiagnosticType) {
        self.base
            .test_error(self.get_processor(), srcs.into_sources(), error(diagnostic));
    }
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization
#[test]
fn test_local_access_optimization() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var a; a = _.a = 1; a + 1;", "_.a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_letNoInitializer
#[test]
fn test_local_access_optimization_let_no_initializer() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a; a = 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["let a; a = _.a = 1;", "_.a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_loopInitializerMixed
#[test]
fn test_local_access_optimization_loop_initializer_mixed() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("for (var a, b; ; ) { b = 1; }")
                .add_chunk("a = 2;")
                .build(),
        ),
        expected_many(&["var b; for (_.a, b; ; ) { b = 1; }", "_.a = 2;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_writeInDifferentChunk
#[test]
fn test_local_access_optimization_write_in_different_chunk() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1; function getA() { return a; }")
                .add_chunk("a = 2;")
                .build(),
        ),
        expected_many(&[
            "_.a = 1; var getA = function() { return _.a; };",
            "_.a = 2;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_writeInDifferentChunk_destructuring
#[test]
fn test_local_access_optimization_write_in_different_chunk_destructuring() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1; function getA() { return a; }")
                .add_chunk("var {a} = {a: 2};")
                .build(),
        ),
        expected_many(&[
            "_.a = 1; var getA = function() { return _.a; };",
            "({a: _.a} = {a: 2});",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_destructuring
#[test]
fn test_local_access_optimization_destructuring() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a} = {a: 1}; a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var a; ({a} = {a: 1}); _.a = a; a + 1;", "_.a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_notUsedInDefiningChunk
#[test]
fn test_local_access_optimization_not_used_in_defining_chunk() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["_.a = 1;", "_.a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testImplicitGlobal
#[test]
fn test_implicit_global() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = true;
    t.test_same("implicitGlobal = 1;");
}

// port: RescopeGlobalSymbolsTest#testVarDeclarations
#[test]
fn test_var_declarations() {
    let t = T::new();
    t.test("var a = 1;", "_.a = 1;");
    t.test("var a = 1, b = 2, c = 3;", "_.a = 1; _.b = 2; _.c = 3;");
    t.test(
        "var a = 'str', b = 1, c = { foo: 'bar' }, d = function() {};",
        "_.a = 'str'; _.b = 1; _.c = { foo: 'bar' }; _.d = function() {};",
    );
    t.test("if(1){var x = 1;}", "if(1){_.x = 1;}");
    t.test("var x;", "");
    t.test("var a, b = 1;", "_.b = 1");
}

// port: RescopeGlobalSymbolsTest#testVarDeclarations_allSameChunk
#[test]
fn test_var_declarations_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test_same("var a = 1;");
    t.test_same("var a = 1, b = 2, c = 3;");
    t.test_same("var a = 'str', b = 1, c = { foo: 'bar' }, d = function() {};");
    t.test_same("if(1){var x = 1;}");
    t.test_same("var x;");
    t.test_same("var a, b = 1;");
}

// port: RescopeGlobalSymbolsTest#testVarDeclarations_export
#[test]
fn test_var_declarations_export() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test("var _dumpException = 1;", "_._dumpException = 1");
}

// port: RescopeGlobalSymbolsTest#testVarDeclarations_acrossChunks
#[test]
fn test_var_declarations_across_chunks() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1;")
                .add_chunk("a")
                .build(),
        ),
        expected_many(&["_.a = 1", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1, b = 2, c = 3;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a = 1; b = 2; _.c = 3;", "_.a;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1, b = 2, c = 3;")
                .add_chunk("b;c;")
                .build(),
        ),
        expected_many(&["var a;a = 1; _.b = 2; _.c = 3;", "_.b;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1, b = 2, c = 3;b;c;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a = 1; b = 2; _.c = 3;b;_.c", "_.a;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a, b = 1;")
                .add_chunk("b")
                .build(),
        ),
        expected_many(&["var a;_.b = 1;", "_.b"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a, b = 1, c = 2;")
                .add_chunk("b")
                .build(),
        ),
        expected_many(&["var a, c;_.b = 1;c = 2", "_.b"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a, b = 1, c = 2;")
                .add_chunk("a")
                .build(),
        ),
        expected_many(&["var b, c;b = 1;c = 2", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a=1; var b=2,c=3;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a=1;b=2;_.c=3", "_.a;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("1;var a, b = 1, c = 2;")
                .add_chunk("b")
                .build(),
        ),
        expected_many(&["var a, c;1;_.b = 1;c = 2", "_.b"]),
    );
}

// port: RescopeGlobalSymbolsTest#testLetDeclarations
#[test]
fn test_let_declarations() {
    let t = T::new();
    t.test("let a = 1;", "_.a = 1;");
    t.test("let a = 1, b = 2, c = 3;", "_.a = 1; _.b = 2; _.c = 3;");
    t.test(
        "let a = 'str', b = 1, c = { foo: 'bar' }, d = function() {};",
        "_.a = 'str'; _.b = 1; _.c = { foo: 'bar' }; _.d = function() {};",
    );
    t.test_same("if(1){let x = 1;}");
    t.test("let x;", "");
    t.test("let a, b = 1;", "_.b = 1");
}

// port: RescopeGlobalSymbolsTest#testLetDeclarations_allSameChunk
#[test]
fn test_let_declarations_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test_same("let a = 1;");
    t.test_same("let a = 1, b = 2, c = 3;");
    t.test_same("let a = 'str', b = 1, c = { foo: 'bar' }, d = function() {};");
    t.test_same("if(1){let x = 1;}");
    t.test_same("let x;");
    t.test_same("let a, b = 1;");
}

// port: RescopeGlobalSymbolsTest#testLetDeclarations_export
#[test]
fn test_let_declarations_export() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test("let _dumpException = 1;", "_._dumpException = 1");
}

// port: RescopeGlobalSymbolsTest#testLetDeclarations_acrossChunks
#[test]
fn test_let_declarations_across_chunks() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    // test references across chunks.
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a = 1;")
                .add_chunk("a")
                .build(),
        ),
        expected_many(&["_.a = 1", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a = 1, b = 2, c = 3;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a = 1; b = 2; _.c = 3;", "_.a;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a = 1, b = 2, c = 3;")
                .add_chunk("b;c;")
                .build(),
        ),
        expected_many(&["var a;a = 1; _.b = 2; _.c = 3;", "_.b;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a = 1, b = 2, c = 3;b;c;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a = 1; b = 2; _.c = 3;b;_.c", "_.a;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a, b = 1;")
                .add_chunk("b")
                .build(),
        ),
        expected_many(&["var a;_.b = 1;", "_.b"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a, b = 1, c = 2;")
                .add_chunk("b")
                .build(),
        ),
        expected_many(&["var a, c;_.b = 1;c = 2", "_.b"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a, b = 1, c = 2;")
                .add_chunk("a")
                .build(),
        ),
        expected_many(&["var b, c;b = 1;c = 2", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a=1; let b=2,c=3;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a=1;b=2;_.c=3", "_.a;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("1;let a, b = 1, c = 2;")
                .add_chunk("b")
                .build(),
        ),
        expected_many(&["var a, c;1;_.b = 1;c = 2", "_.b"]),
    );
    // test non-globals with same name as cross-chunk globals.
    t.test_same(srcs_chunks(
        JSChunkGraphBuilder::for_unordered()
            .add_chunk("1;let a, b = 1, c = 2;")
            .add_chunk("if (true) { let b = 3; b; }")
            .build(),
    ));
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("1;let a, b = 1, c = 2;")
                .add_chunk("b; if (true) { let b = 3; b; }")
                .build(),
        ),
        expected_many(&[
            "var a, c; 1;_.b = 1;c = 2",
            "_.b; if (true) { let b = 3; b; }",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_let
#[test]
fn test_local_access_optimization_let() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("let a = 1; a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var a; a = _.a = 1; a + 1;", "_.a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_const
#[test]
fn test_local_access_optimization_const() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("const a = 1; a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var a; a = _.a = 1; a + 1;", "_.a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_updateAssignment
#[test]
fn test_local_access_optimization_update_assignment() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1; a++; a += 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var a; a = _.a = 1; a++; _.a = a; a += 1; _.a = a;", "_.a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testLocalAccessOptimization_multipleWrites
#[test]
fn test_local_access_optimization_multiple_writes() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::DEFINING_CHUNK_ONLY;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1; var b = 1; a = b = 2;")
                .add_chunk("a; b;")
                .build(),
        ),
        expected_many(&[
            "var a, b; a = _.a = 1; b = _.b = 1; a = _.a = b = _.b = 2;",
            "_.a; _.b;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testConstDeclarations
#[test]
fn test_const_declarations() {
    let t = T::new();
    t.test("const a = 1;", "_.a = 1;");
    t.test("const a = 1, b = 2, c = 3;", "_.a = 1; _.b = 2; _.c = 3;");
    t.test(
        "const a = 'str', b = 1, c = { foo: 'bar' }, d = function() {};",
        "_.a = 'str'; _.b = 1; _.c = { foo: 'bar' }; _.d = function() {};",
    );
    t.test_same("if(1){const x = 1;}");
}

// port: RescopeGlobalSymbolsTest#testConstDeclarations_allSameChunk
#[test]
fn test_const_declarations_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test_same("const a = 1;");
    t.test_same("const a = 1, b = 2, c = 3;");
    t.test_same("const a = 'str', b = 1, c = { foo: 'bar' }, d = function() {};");
    t.test_same("if(1){const x = 1;}");
}

// port: RescopeGlobalSymbolsTest#testConstDeclarations_export
#[test]
fn test_const_declarations_export() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test("const _dumpException = 1;", "_._dumpException = 1");
}

// port: RescopeGlobalSymbolsTest#testConstDeclarations_acrossChunks
#[test]
fn test_const_declarations_across_chunks() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    // test references across chunks.
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("const a = 1;")
                .add_chunk("a")
                .build(),
        ),
        expected_many(&["_.a = 1", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("const a = 1, b = 2, c = 3;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a = 1; b = 2; _.c = 3;", "_.a;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("const a = 1, b = 2, c = 3;")
                .add_chunk("b;c;")
                .build(),
        ),
        expected_many(&["var a;a = 1; _.b = 2; _.c = 3;", "_.b;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("const a = 1, b = 2, c = 3;b;c;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a = 1; b = 2; _.c = 3;b;_.c", "_.a;_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("const a=1; const b=2,c=3;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_many(&["var b;_.a=1;b=2;_.c=3", "_.a;_.c"]),
    );
    // test non-globals with same name as cross-chunk globals.
    t.test_same(srcs_chunks(
        JSChunkGraphBuilder::for_unordered()
            .add_chunk("1;const a = 1, b = 1, c = 2;")
            .add_chunk("if (true) { const b = 3; b; }")
            .build(),
    ));
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("1;const a = 1, b = 1, c = 2;")
                .add_chunk("b; if (true) { const b = 3; b; }")
                .build(),
        ),
        expected_many(&[
            "var a, c; 1;a = 1; _.b = 1;c = 2",
            "_.b; if (true) { const b = 3; b; }",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testObjectDestructuringDeclarations
#[test]
fn test_object_destructuring_declarations() {
    let t = T::new();
    t.test("var {a} = {}; a;", "({a: _.a} = {}); _.a;");
    t.test("var {a: a} = {}; a;", "({a: _.a} = {}); _.a;");
    t.test("var {key: a} = {}; a;", "({key: _.a} = {}); _.a;");
    t.test("var {a: {b}} = {}; b;", "({a: {b: _.b}} = {}); _.b;");
    t.test("var {a: {key: b}} = {}; b;", "({a: {key: _.b}} = {}); _.b;");
    t.test(
        "var {['computed']: a} = {}; a;",
        "({['computed']: _.a} = {}); _.a;",
    );
    //
    t.test(
        "var {...a} = {a: 1, b: 2, c: 3}; a;",
        "({..._.a} = {a: 1, b: 2, c: 3}); _.a;",
    );
    t.test(
        "var {a, ...b} = {x: 1, y: 2, z: 3}; a; b;",
        "({a: _.a, ..._.b} = {x: 1, y: 2, z: 3}); _.a; _.b;",
    );
    t.test("var {a = 3} = {}; a;", "({a: _.a = 3} = {}); _.a");
    t.test("var {key: a = 3} = {}; a;", "({key: _.a = 3} = {}); _.a");
    t.test(
        "var {a} = {}, [b] = [], c = 3",
        "({a: _.a} = {}); [_.b] = []; _.c = 3;",
    );
}

// port: RescopeGlobalSymbolsTest#testObjectDestructuringDeclarations_allSameChunk
#[test]
fn test_object_destructuring_declarations_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test_same("var {a} = {}; a;");
    t.test_same("var {a: a} = {}; a;");
    t.test_same("var {key: a} = {}; a;");
    t.test_same("var {a: {b: b}} = {}; b;");
    t.test_same("var {['computed']: a} = {}; a;");
    t.test_same("var {a: a = 3} = {}; a;");
    t.test_same("var {key: a = 3} = {}; a;");
    t.test_same("var {['computed']: a = 3} = {}; a;");
    t.test_same("var {a} = {}, b = 3;");
    t.test_same("var [a] = [], b = 3;");
    t.test_same("var a = 1, [b] = [], {c} = {};");
    t.test_same("var {...a} = {x: 1, y: 2, z: 3}; a;");
    t.test_same("var {a, ...b} = {x: 1, y: 2, z: 3}; a; b;");
}

// port: RescopeGlobalSymbolsTest#testObjectDestructuringDeclarations_acrossChunks
#[test]
fn test_object_destructuring_declarations_across_chunks() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a: a} = {};")
                .add_chunk("a")
                .build(),
        ),
        expected_many(&["({a: _.a} = {});", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a: a, b: b, c: c} = {};")
                .add_chunk("a; c;")
                .build(),
        ),
        expected_many(&["var b;({a: _.a, b: b, c: _.c} = {});", "_.a; _.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a: a, b: b, c: c} = {};")
                .add_chunk("b; c;")
                .build(),
        ),
        expected_many(&["var a;({a: a, b: _.b, c: _.c} = {});", "_.b; _.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a: a, b: b, c: c} = {}; b; c;")
                .add_chunk("a; c;")
                .build(),
        ),
        expected_many(&["var b;({a: _.a, b: b, c: _.c} = {});b;_.c;", "_.a; _.c"]),
    );
    // Test var declarations containing a mix of destructuring and regular names
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a} = {}, b;")
                .add_chunk("a")
                .build(),
        ),
        expected_many(&["var b; ({a: _.a} = {});", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a} = {}, b = 3;")
                .add_chunk("b")
                .build(),
        ),
        expected_many(&["var a; ({a} = {}); _.b = 3;", "_.b"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a: a, ...c} = {};")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var c; ({a: _.a, ...c} = {});", "_.a;"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var {a: a, ...c} = {};")
                .add_chunk("c;")
                .build(),
        ),
        expected_many(&["var a; ({a: a, ..._.c} = {});", "_.c"]),
    );
}

// port: RescopeGlobalSymbolsTest#testObjectDestructuringAssignments
#[test]
fn test_object_destructuring_assignments() {
    let t = T::new();
    t.test(
        "var a, b; ({key1: a, key2: b} = {}); a; b;",
        "({key1: _.a, key2: _.b} = {}); _.a; _.b;",
    );
    // Test a destructuring assignment with mixed global and local variables.
    t.test(
        "var a; if (true) { let b; ({key1: a, key2: b} = {}); b; } a;",
        "if (true) { let b; ({key1: _.a, key2: b} = {}); b; } _.a;",
    );
    t.test(
        "var obj = {}; ({a: obj.a} = {}); obj.a;",
        "_.obj = {}; ({a: _.obj.a} = {}); _.obj.a;",
    );
    t.test(
        "var obj = {}; ({a:   obj['foo bar']} = {});   obj['foo bar'];",
        "  _.obj = {}; ({a: _.obj['foo bar']} = {}); _.obj['foo bar'];",
    );
    //
    t.test(
        "var a = {}; ({...a.b} = {a: 1, b: 2, c: 3}); a.b;",
        "_.a = {}; ({..._.a.b} = {a: 1, b: 2, c: 3}); _.a.b;",
    );
    //
    t.test(
        "var a = {}; ({c, ...a.b} = {a: 1, b: 2, c: 3}); a.b;",
        "_.a = {}; ({c, ..._.a.b} = {a: 1, b: 2, c: 3}); _.a.b;",
    );
}

// port: RescopeGlobalSymbolsTest#testArrayDestructuringDeclarations
#[test]
fn test_array_destructuring_declarations() {
    let t = T::new();
    t.test("var [a] = [1]; a", "[_.a] = [1]; _.a;");
    t.test(
        "var [a, b, c] = [1, 2, 3]; a; b; c;",
        "[_.a, _.b, _.c] = [1, 2, 3]; _.a; _.b; _.c",
    );
    t.test(
        "var [[a, b], c] = []; a; b; c",
        "[[_.a, _.b], _.c] = []; _.a; _.b; _.c;",
    );
    t.test("var [a = 5] = [1]; a", "[_.a = 5] = [1]; _.a;");
    t.test(
        "var [a, b = 5] = [1]; a; b;",
        "[_.a, _.b = 5] = [1]; _.a; _.b;",
    );
    t.test("var [...a] = [1, 2, 3]; a;", "[..._.a] = [1, 2, 3]; _.a;");
    t.test(
        "var [a, ...b] = [1, 2, 3]; a; b;",
        "[_.a, ..._.b] = [1, 2, 3]; _.a; _.b;",
    );
    t.test("var [a] = 1, b = 2; a; b;", "[_.a] = 1; _.b = 2; _.a; _.b;");
}

// port: RescopeGlobalSymbolsTest#testArrayDestructuringDeclarations_sameChunk
#[test]
fn test_array_destructuring_declarations_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test_same("var [a] = [1]; a");
    t.test_same("var [a, b] = [1, 2]; a; b;");
    t.test_same("var [[a, b], c] = []; a; b; c");
    t.test_same("var [a = 5] = [1]; a");
    t.test_same("var [a, b = 5] = [1]; a; b;");
    t.test_same("var [...a] = [1, 2, 3]; a;");
    t.test_same("var [a, ...b] = [1, 2, 3]; a; b;");
}

// port: RescopeGlobalSymbolsTest#testArrayDestructuringDeclarations_acrossChunks
#[test]
fn test_array_destructuring_declarations_across_chunks() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var [a] = [];")
                .add_chunk("a")
                .build(),
        ),
        expected_many(&["[_.a] = [];", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var [a, b, c] = [];")
                .add_chunk("a; c;")
                .build(),
        ),
        expected_many(&["var b; [_.a, b, _.c] = [];", "_.a; _.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var [a, b, c] = [];")
                .add_chunk("b; c;")
                .build(),
        ),
        expected_many(&["var a; [a, _.b, _.c] = [];", "_.b; _.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var [a, b, c] = []; b; c;")
                .add_chunk("a; c;")
                .build(),
        ),
        expected_many(&["var b; [_.a, b, _.c] = []; b; _.c;", "_.a; _.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var [a, ...c] = [];")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var c; ([_.a, ...c] = []);", "_.a;"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var [a, ...c] = [];")
                .add_chunk("c;")
                .build(),
        ),
        expected_many(&["var a; ([a, ..._.c] = []);", "_.c"]),
    );
}

// port: RescopeGlobalSymbolsTest#testArrayDestructuringAssignments
#[test]
fn test_array_destructuring_assignments() {
    let t = T::new();
    t.test("var a, b; [a, b] = []; a; b;", "[_.a, _.b] = []; _.a; _.b;");
    // Test a destructuring assignment with mixed global and local variables.
    t.test(
        "var a; if (true) { let b; [a, b] = []; b; } a;",
        "if (true) { let b; [_.a, b] = []; b; } _.a;",
    );
    // Test assignments to qualified names and quoted properties.
    t.test(
        "var obj = {}; [obj.a] = []; obj.a;",
        "_.obj = {}; [_.obj.a] = []; _.obj.a;",
    );
    t.test(
        "var obj = {}; [  obj['foo bar']] = [];   obj['foo bar'];",
        "  _.obj = {}; [_.obj['foo bar']] = []; _.obj['foo bar'];",
    );
    //
    t.test(
        "var a = {}; ([...a.b] = [1, 2, 3]); a.b;",
        "_.a = {}; ([..._.a.b] = [1, 2, 3]); _.a.b;",
    );
    //
    t.test(
        "var a = {}; ([c, ...a.b] = [1, 2, 3]); a.b;",
        "_.a = {}; ([c, ..._.a.b] = [1, 2, 3]); _.a.b;",
    );
}

// port: RescopeGlobalSymbolsTest#testClasses
#[test]
fn test_classes() {
    let t = T::new();
    t.test("class A {}", "_.A = class {};");
    t.test(
        "class A {} class B extends A {}",
        "_.A = class {}; _.B = class extends _.A {}",
    );
    t.test(
        "class A {} let a = new A;",
        "_.A = class {}; _.a = new _.A;",
    );
    t.test("const PI = 3.14;\nclass A {\n  static printPi() {\n    console.log(PI);\n  }\n}\nA.printPi();\n", "_.PI = 3.14;\n_.A = class {\n  static printPi() {\n    console.log(_.PI);\n  }\n}\n_.A.printPi();\n");
    // Test that class expression names are not rewritten.
    t.test("var A = class Name {};", "_.A = class Name {};");
    t.test("var A = class A {};", "_.A = class A {};");
}

// port: RescopeGlobalSymbolsTest#testClasses_nonGlobal
#[test]
fn test_classes_non_global() {
    let t = T::new();
    t.test_same("if (true) { class A {} }");
    t.test(
        "function foo() { class A {} }",
        "_.foo = function() { class A {} };",
    );
    t.test("const A = 5; { class A {} }", "_.A = 5; { class A {} }");
}

// port: RescopeGlobalSymbolsTest#testClasses_allSameChunk
#[test]
fn test_classes_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test("class A {}", "var A = class {};");
    t.test(
        "class A {} class B extends A {}",
        "var A = class {}; var B = class extends A {}",
    );
    t.test_same("if (true) { class A {} }");
}

// port: RescopeGlobalSymbolsTest#testForLoops
#[test]
fn test_for_loops() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("for (var i = 0, c = 2; i < 1000; i++);")
                .add_chunk("c")
                .build(),
        ),
        expected_many(&["var i;for (i = 0, _.c = 2; i < 1000; i++);", "_.c"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("      for (var i = 0, c = 2;   i < 1000;   i++);")
                .add_chunk("i")
                .build(),
        ),
        expected_many(&["var c;for (  _.i = 0, c = 2; _.i < 1000; _.i++);", "_.i"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("       for (var {i: i, c:   c} = {};  i < 1000; i++);")
                .add_chunk("c")
                .build(),
        ),
        expected_many(&[
            "var i; for (   ({i: i, c: _.c} = {}); i < 1000; i++);",
            "_.c",
        ]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("       for (var [i,   c] = [0, 2]; i < 1000; i++);")
                .add_chunk("c;")
                .build(),
        ),
        expected_many(&["var i; for (    [i, _.c] = [0, 2]; i < 1000; i++);", "_.c;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testForLoops_acrossChunks
#[test]
fn test_for_loops_across_chunks() {
    let t = T::new();
    t.test(
        "for (var i = 0; i < 1000; i++);",
        "for (_.i = 0; _.i < 1000; _.i++);",
    );
    t.test(
        "for (var i = 0, c = 2; i < 1000; i++);",
        "for (_.i = 0, _.c = 2; _.i < 1000; _.i++);",
    );
    t.test(
        "for (var i = 0, c = 2, d = 3; i < 1000; i++);",
        "for (_.i = 0, _.c = 2, _.d = 3; _.i < 1000; _.i++);",
    );
    t.test(
        "for (var i = 0, c = 2, d = 3, e = 4; i < 1000; i++);",
        "for (_.i = 0, _.c = 2, _.d = 3, _.e = 4; _.i < 1000; _.i++);",
    );
    t.test(
        "for (var i = 0; i < 1000;)i++;",
        "for (_.i = 0; _.i < 1000;)_.i++;",
    );
    t.test(
        "for (var i = 0,b; i < 1000;)i++;b++",
        "for (_.i = 0,_.b; _.i < 1000;)_.i++;_.b++",
    );
    t.test(
        "var o={};for (var i in o)i++;",
        "_.o={};for (_.i in _.o)_.i++;",
    );
    // Test destructuring.
    t.test(
        "for (var [i] = [0]; i < 1000; i++);",
        "for ([_.i] = [0]; _.i < 1000; _.i++);",
    );
    t.test(
        "for (var {i: i} = {}; i < 1000; i++);",
        " for (({i: _.i} = {}); _.i < 1000; _.i++);",
    );
    t.test_same("for (let [i] = [0]; i  < 1000; i++);");
}

// port: RescopeGlobalSymbolsTest#testForInLoops_allSameChunk
#[test]
fn test_for_in_loops_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test_same("for (var i in {});");
    t.test_same("for (var [a] in {});");
    t.test_same("for (var {a: a} in {});");
}

// port: RescopeGlobalSymbolsTest#testForInLoops_acrossChunks
#[test]
fn test_for_in_loops_across_chunks() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("for (var i in {});")
                .add_chunk("i")
                .build(),
        ),
        expected_many(&["for (_.i in {});", "_.i"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("       for (var [  a, b] in {});")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var b; for (    [_.a, b] in {});", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("       for (var {i: i, c:   c} in {});")
                .add_chunk("c")
                .build(),
        ),
        expected_many(&["var i; for (    {i: i, c: _.c} in {});", "_.c"]),
    );
}

// port: RescopeGlobalSymbolsTest#testForOfLoops_allSameChunk
#[test]
fn test_for_of_loops_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test_same("for (var i of [1, 2, 3]);");
    t.test_same("for (var [a] of []);");
    t.test_same("for (var {a: a} of {});");
}

// port: RescopeGlobalSymbolsTest#testForOfLoops_acrossChunks
#[test]
fn test_for_of_loops_across_chunks() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("for (var i of []);")
                .add_chunk("i")
                .build(),
        ),
        expected_many(&["for (_.i of []);", "_.i"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("       for (var [  a, b] of []);")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var b; for (    [_.a, b] of []);", "_.a"]),
    );
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("       for (var {i: i, c:   c} of []);")
                .add_chunk("c")
                .build(),
        ),
        expected_many(&["var i; for (    {i: i, c: _.c} of []);", "_.c"]),
    );
}

// port: RescopeGlobalSymbolsTest#testForAwaitOfLoops_allSameChunk
#[test]
fn test_for_await_of_loops_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test_same("async () => { for await (var i of [1, 2, 3]);}");
    t.test_same("async () => { for await (var [a] of []);}");
    t.test_same("async () => { for await (var {a: a} of {});}");
}

// port: RescopeGlobalSymbolsTest#testForAwaitOfLoops_acrossChunks
#[test]
fn test_for_await_of_loops_across_chunks() {
    let mut t = T::new();
    // TODO(b/128938049): re-enable it once we support top-level await.
    t.assume_cross_chunk_names = false;
    t.test_error(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("for await (var i of []);")
                .add_chunk("i")
                .build(),
        ),
        &PARSE_ERROR,
    );
    t.test_error(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("       for await (var [  a, b] of []);")
                .add_chunk("a;")
                .build(),
        ),
        &PARSE_ERROR,
    );
    t.test_error(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("       for await (var {i: i, c:   c} of []);")
                .add_chunk("c")
                .build(),
        ),
        &PARSE_ERROR,
    );
}

// port: RescopeGlobalSymbolsTest#testFunctionStatements
#[test]
fn test_function_statements() {
    let t = T::new();
    t.test("function test(){}", "_.test=function (){}");
    // Ignore block-scoped function declarations.
    t.test_same("if(1) function test(){}");
    t.test("async function test() {}", "_.test = async function() {}");
    t.test("function *test() {}", "_.test = function *() {}");
}

// port: RescopeGlobalSymbolsTest#testFunctionStatements_allSameChunk
#[test]
fn test_function_statements_all_same_chunk() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test("function f() {}", "var f = function() {}");
    t.test_same("if (true) { function f() {} }");
}

// port: RescopeGlobalSymbolsTest#testFunctionStatements_freeCallSemantics1
#[test]
fn test_function_statements_free_call_semantics1() {
    let mut t = T::new();
    t.base.disable_compare_as_tree();
    // This triggers free call.
    t.test(
        "function x(){this};var y=function(){var val=x()||{}}",
        "_.x=function(){this};_.y=function(){var val=(0,_.x)()||{}}",
    );
    t.test("function x(){this;x()}", "_.x=function(){this;(0,_.x)()}");
    t.test(
        "var a=function(){this};a()",
        "_.a=function(){this};(0,_.a)()",
    );
    // Always trigger free calls for variables assigned through destructuring.
    t.test(
        "var {a: a} = {a: function() {}}; a();",
        "({a:_.a}={a:function(){}});(0,_.a)()",
    );
    t.test(
        "var ns = {}; ns.a = function() {}; ns.a()",
        "_.ns={};_.ns.a=function(){};_.ns.a()",
    );
}

// port: RescopeGlobalSymbolsTest#testFunctionStatements_freeCallSemantics2
#[test]
fn test_function_statements_free_call_semantics2() {
    let t = T::new();
    // Cases where free call forcing through (0, foo)() is not necessary.
    t.test("var a=function(){};a()", "_.a=function(){};_.a()");
    t.test("function a(){};a()", "_.a=function(){};;_.a()");
    t.test("var a;a=function(){};a()", "_.a=function(){};_.a()");
    // Test that calls to arrow functions are not forced to be free calls
    t.test("var a = () => {}; a();", "_.a = () => {}; _.a();");
    t.test("var a = () => this; a();", "_.a = () => this; _.a();");
}

// port: RescopeGlobalSymbolsTest#testFunctionStatements_freeCallSemantics3
#[test]
fn test_function_statements_free_call_semantics3() {
    let mut t = T::new();
    t.base.disable_compare_as_tree();
    // Ambiguous cases.
    t.test(
        "var a=1;a=function(){};a()",
        "_.a=1;_.a=function(){};(0,_.a)()",
    );
    t.test("var b;var a=b;a()", "_.a=_.b;(0,_.a)()");
}

// port: RescopeGlobalSymbolsTest#testFunctionStatements_defaultParameters
#[test]
fn test_function_statements_default_parameters() {
    let t = T::new();
    t.test(
        "var a = 1; function f(param = a) {}",
        "_.a = 1; _.f = function(param = _.a) {}",
    );
}

// port: RescopeGlobalSymbolsTest#testDeeperScopes
#[test]
fn test_deeper_scopes() {
    let t = T::new();
    t.test(
        "var a = function(b){return b}",
        "_.a = function(b){return b}",
    );
    t.test(
        "var a = function(b){var a; return a+b}",
        "_.a = function(b){var a; return a+b}",
    );
    t.test(
        "var a = function(a,b){return a+b}",
        "_.a = function(a,b){return a+b}",
    );
    t.test(
        "var x=1,a = function(b){var a; return a+b+x}",
        "_.x=1;_.a = function(b){var a; return a+b+_.x}",
    );
    t.test(
        "var x=1,a = function(b){return function(){var a;return a+b+x}}",
        "_.x=1;_.a = function(b){return function(){var a; return a+b+_.x}}",
    );
}

// port: RescopeGlobalSymbolsTest#testTryCatch
#[test]
fn test_try_catch() {
    let t = T::new();
    t.test(
        "try{var a = 1}catch(e){throw e}",
        "try{_.a = 1}catch(e){throw e}",
    );
}

// port: RescopeGlobalSymbolsTest#testShadowInFunctionScope
#[test]
fn test_shadow_in_function_scope() {
    let t = T::new();
    t.test(
        "var _ = 1; (function () { _ = 2 })()",
        "_._ = 1; (function () { _._ = 2 })()",
    );
    t.test(
        "function foo() { var _ = {}; _.foo = foo; _.bar = 1; }",
        "_.foo = function () { var _$ = {}; _$.foo = _.foo; _$.bar = 1}",
    );
    t.test(
        "function foo() { var {key: _} = {}; _.foo = foo; _.bar = 1; }",
        "_.foo = function () { var {key: _$} = {}; _$.foo = _.foo; _$.bar = 1}",
    );
    t.test("function foo() { var _ = {}; _.foo = foo; _.bar = 1;\n(function() { var _ = 0;})() }\n", "_.foo = function () { var _$ = {}; _$.foo = _.foo; _$.bar = 1;\n(function() { var _$ = 0;})() }\n");
    t.test(
        "function foo() { var _ = {}; _.foo = foo; _.bar = 1;\nvar _$ = 1; }\n",
        "_.foo = function () { var _$ = {}; _$.foo = _.foo; _$.bar = 1;\nvar _$$ = 1; }\n",
    );
    t.test("function foo() { var _ = {}; _.foo = foo; _.bar = 1;\nvar _$ = 1; (function() { _ = _$ })() }\n", "_.foo = function () { var _$ = {}; _$.foo = _.foo; _$.bar = 1;\nvar _$$ = 1; (function() { _$ = _$$ })() }\n");
    t.test("function foo() { var _ = {}; _.foo = foo; _.bar = 1;\nvar _$ = 1, _$$ = 2 (function() { _ = _$ = _$$;\nvar _$, _$$$ })() }\n", "_.foo = function () { var _$ = {}; _$.foo = _.foo; _$.bar = 1;\nvar _$$ = 1, _$$$ = 2 (function() { _$ = _$$ = _$$$;\nvar _$$, _$$$$ })() }\n");
    t.test(
        "var a = 5; function foo(_) { return _; }",
        "_.a = 5; _.foo = function(_$) { return _$; }",
    );
    t.test(
        "var a = 5; function foo({key: _}) { return _; }",
        "_.a = 5; _.foo = function({key: _$}) { return _$; }",
    );
    t.test(
        "var a = 5; function foo([_]) { return _; }",
        "_.a = 5; _.foo = function([_$]) { return _$; }",
    );
    // We accept this unnecessary renaming as acceptable to simplify pattern
    // matching in the traversal.
    t.test(
        "function foo() { var _$a = 1;}",
        "_.foo = function () { var _$a$ = 1;}",
    );
}

// port: RescopeGlobalSymbolsTest#testShadowInBlockScope
#[test]
fn test_shadow_in_block_scope() {
    let t = T::new();
    t.test(
        "var foo = 1; if (true) { const _ = {}; _.foo = foo; _.bar = 1; }",
        "_.foo = 1; if (true) { const _$ = {}; _$.foo = _.foo; _$.bar = 1}",
    );
    t.test(
        "var foo = 1; if (true) { const {key: _} = {}; _.foo = foo; _.bar = 1; }",
        "_.foo = 1; if (true) { const {key: _$} = {}; _$.foo = _.foo; _$.bar = 1}",
    );
    t.test(
        "var foo = 1; if (true) { const _ = {}; _.foo = foo; _.bar = 1; const _$ = 1; }",
        "_.foo = 1; if (true) { const _$ = {}; _$.foo = _.foo; _$.bar = 1;  const _$$ = 1; }",
    );
    t.test(
        "var foo = 1; if (true) { const [_] = [{}]; _.foo = foo; }",
        "  _.foo = 1; if (true) { const [_$] = [{}]; _$.foo = _.foo; }",
    );
}

// port: RescopeGlobalSymbolsTest#testExterns
#[test]
fn test_externs() {
    let t = T::new();
    t.test_externs(
        externs("var document;"),
        srcs("document"),
        expected("document"),
    );
    t.test_externs(
        externs("var document;"),
        srcs("document.getElementsByTagName('test')"),
        expected("document.getElementsByTagName('test')"),
    );
    t.test_externs(
        externs("var document;"),
        srcs("document.getElementsByTagName('test')"),
        expected("document.getElementsByTagName('test')"),
    );
    t.test_externs(
        externs("var document;document.getElementsByTagName"),
        srcs("document.getElementsByTagName('test')"),
        expected("document.getElementsByTagName('test')"),
    );
    t.test_externs(
        externs("var document,navigator"),
        srcs("document.navigator;navigator"),
        expected("document.navigator;navigator"),
    );
    t.test_externs(
        externs("var iframes"),
        srcs("function test() { iframes.resize(); }"),
        expected("_.test = function() { iframes.resize(); }"),
    );
    t.test_externs(
        externs("var iframes"),
        srcs("var foo = iframes;"),
        expected("_.foo = iframes;"),
    );
    t.test_externs(externs("class A {}"), srcs("A"), expected("A"));
    // Special names.
    t.test_externs(
        externs("var arguments, window, eval;"),
        srcs("arguments;window;eval;"),
        expected("arguments;window;eval;"),
    );
    // Actually not an extern.
    t.test_externs(externs(""), srcs("document"), expected("document"));
    // Javascript builtin objects
    t.test_same("Object;Function;Array;String;Boolean;Number;Math;\nDate;RegExp;JSON;Error;EvalError;ReferenceError;\nSyntaxError;TypeError;URIError;\n");
}

// port: RescopeGlobalSymbolsTest#testSameVarDeclaredInExternsAndSource
#[test]
fn test_same_var_declared_in_externs_and_source() {
    let t = T::new();
    t.test_externs(
        externs("/** @const */ var ns = {}; function f() {}"),
        srcs("/** @const */ var ns = ns || {};"),
        expected("/** @const */ ns = ns || {};"),
    );
    t.test_externs(
        externs("var x;"),
        srcs("var x = 1; x = 2;"),
        expected("x = 1; x = 2;"),
    );
    t.test_externs(
        externs("var x;"),
        srcs("function f() { var x; x = 1; }"),
        expected("_.f = function() { var x; x = 1; }"),
    );
    t.test_externs(
        externs("var x;"),
        srcs("function f() { x = 1; }"),
        expected("_.f = function() { x = 1; };"),
    );
    t.test_externs(
        externs("var x, y;"),
        srcs("var x = 1, y = 2;"),
        expected("x = 1; y = 2;"),
    );
    t.test_externs(
        externs("var x, y;"),
        srcs("var x, y = 2;"),
        expected("y = 2;"),
    );
    t.test_externs(
        externs("var x;"),
        srcs("var x = 1, y = 2;"),
        expected("x = 1; _.y = 2;"),
    );
    t.test_externs(
        externs("var x;"),
        srcs("var y = 2, x = 1;"),
        expected("_.y = 2; x = 1;"),
    );
    t.test_externs(externs("var x;"), srcs("var y, x = 1;"), expected("x = 1;"));
    t.test_externs(
        externs("var foo;"),
        srcs("var foo = function(x) { if (x > 0) { var y = foo; } };"),
        expected("foo = function(x) { if (x > 0) { var y = foo; } };"),
    );
    // The parameter x doesn't conflict with the x in the source
    t.test_externs(
        externs("function f(x) {}"),
        srcs("var f = 1; var x = 2;"),
        expected("f = 1; _.x = 2;"),
    );
}

// port: RescopeGlobalSymbolsTest#testSameVarDeclaredInExternsAndSource2
#[test]
fn test_same_var_declared_in_externs_and_source2() {
    let mut t = T::new();
    t.assume_cross_chunk_names = false;
    t.test(srcs_chunks(JSChunkGraphBuilder::for_unordered().add_chunk("Foo = function() { this.b = ns; };\nvar f = function(a) {\n  if (a instanceof Foo && a.b === ns) {}\n},\nns = {},\ng = function(a) { var b = new Foo; };\n").add_chunk("f; g;").build()), expected_many(&["var ns;\nFoo = function() { this.b = ns; };\n_.f = function(a) {\n  if (a instanceof Foo && a.b === ns) {}\n};\nns = {};\n_.g = function(a) { var b = new Foo; };\n", "_.f; _.g;"]));
    t.test_externs(
        externs("var y;"),
        srcs("var x = 1, y = 2; function f() { return x + y; }"),
        expected("var x; x = 1; y = 2; var f = function() { return x + y; }"),
    );
}

// port: RescopeGlobalSymbolsTest#testArrowFunctions
#[test]
fn test_arrow_functions() {
    let t = T::new();
    t.test("const fn = () => 3;", "_.fn = () => 3;");
    t.test(
        "const PI = 3.14; const fn = () => PI;",
        "_.PI = 3.14; _.fn = () => _.PI",
    );
    t.test(
        "let a = 3; const fn = () => a = 4;",
        "_.a = 3; _.fn = () => _.a = 4;",
    );
    t.test(
        "const PI = 3.14; (() => PI)()",
        "_.PI = 3.14; (() => _.PI)();",
    );
    t.test(
        "const PI = 3.14; (() => { return PI; })()",
        "_.PI = 3.14; (() => { return _.PI; })()",
    );
}

// port: RescopeGlobalSymbolsTest#testEnhancedObjectLiterals
#[test]
fn test_enhanced_object_literals() {
    let t = T::new();
    t.test(
        "var a = 3; var obj = {[a]: a};",
        "_.a = 3; _.obj = {[_.a]: _.a};",
    );
    t.test(
        "var g = 3; var obj = {a() {}, b() { return g; }};",
        "_.g = 3; _.obj = {a() {}, b() { return _.g; }};",
    );
    t.test("var a = 1; var obj = {a}", "_.a = 1; _.obj = {a: _.a};");
}

// port: RescopeGlobalSymbolsTest#testEs6Modules
#[test]
fn test_es6_modules() {
    let mut t = T::new();
    t.base.ignore_warnings(&[&LOAD_WARNING]);
    // Test that this pass does nothing to ES6 modules.
    t.test_same("var a = 3; a; export default a;");
    t.assume_cross_chunk_names = false;
    t.test_same(srcs_chunks(
        JSChunkGraphBuilder::for_unordered()
            .add_chunk("var a = 3; export {a};")
            .add_chunk("import {a} from './input0';")
            .build(),
    ));
}

// port: RescopeGlobalSymbolsTest#testEmptyDestructuring
#[test]
fn test_empty_destructuring() {
    let t = T::new();
    t.test_same("var {} = {};");
}

// port: RescopeGlobalSymbolsTest#testAllChunksLocalAccessOptimization
#[test]
fn test_all_chunks_local_access_optimization() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var a; a = _.a = 1; a + 1;", "var {a} = _; a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksLocalAccessOptimization_reassignedInDefiningChunk
#[test]
fn test_all_chunks_local_access_optimization_reassigned_in_defining_chunk() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; a = 2; a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var a; a = _.a = 1; a = _.a = 2; a + 1;", "var {a} = _; a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksLocalAccessOptimization_reassignedOutsideStaticExecution
#[test]
fn test_all_chunks_local_access_optimization_reassigned_outside_static_execution() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; function f() { a = 2; } a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&[
            "var a; a = _.a = 1; var f = function() { a = _.a = 2; }; a + 1;",
            "_.a;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksLocalAccessOptimization_reassignedInOtherChunk
#[test]
fn test_all_chunks_local_access_optimization_reassigned_in_other_chunk() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("a = 2; a;")
                .build(),
        ),
        expected_many(&["_.a = 1; _.a + 1;", "_.a = 2; _.a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksLocalAccessOptimization_multipleChunks
#[test]
fn test_all_chunks_local_access_optimization_multiple_chunks() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("a; a + 2;")
                .add_chunk("a; a + 3;")
                .build(),
        ),
        expected_many(&[
            "var a; a = _.a = 1; a + 1;",
            "var {a} = _; a; a + 2;",
            "var {a} = _; a; a + 3;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksLocalAccessOptimization_independentChunks
#[test]
fn test_all_chunks_local_access_optimization_independent_chunks() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_unordered()
                .add_chunk("var a = 1;")
                .add_chunk("function f() { return a; }")
                .build(),
        ),
        expected_many(&["_.a = 1;", "var f = function() { return _.a; };"]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksLocalAccessOptimization_definingChunkDependsOnUsingChunk
#[test]
fn test_all_chunks_local_access_optimization_defining_chunk_depends_on_using_chunk() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("function f() { return a; }")
                .add_chunk("var a = 1;")
                .build(),
        ),
        expected_many(&["var f = function() { return _.a; };", "_.a = 1;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksWithWrappedMutables_reassignedOutsideStaticExecution
#[test]
fn test_all_chunks_with_wrapped_mutables_reassigned_outside_static_execution() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; function f() { a = 2; } a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&[
            "var a = _.a = {}; a._ = 1; var f = function() { a._ = 2; }; a._ + 1;",
            "var {a} = _; a._;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksWithWrappedMutables_reassignedInOtherChunk
#[test]
fn test_all_chunks_with_wrapped_mutables_reassigned_in_other_chunk() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("a = 2; a;")
                .build(),
        ),
        expected_many(&[
            "var a = _.a = {}; a._ = 1; a._ + 1;",
            "var {a} = _; a._ = 2; a._;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksWithWrappedMutables_multipleHolderSymbols
#[test]
fn test_all_chunks_with_wrapped_mutables_multiple_holder_symbols() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("var b = 1; b + 1;")
                .add_chunk("a = 2; b = 2;")
                .build(),
        ),
        expected_many(&[
            "var a = _.a = {}; a._ = 1; a._ + 1;",
            "var b = _.b = {}; b._ = 1; b._ + 1;",
            "var {a, b} = _; a._ = 2; b._ = 2;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksWithWrappedMutables_parentAccessesChildHolder
#[test]
fn test_all_chunks_with_wrapped_mutables_parent_accesses_child_holder() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("function f() { return a; }")
                .add_chunk("var a = 1; function g() { a = 2; } a + 1;")
                .build(),
        ),
        expected_many(&[
            "var a = _.a = {}; var f = function() { return a._; };",
            "var {a} = _; a._ = 1; var g = function() { a._ = 2; }; a._ + 1;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksWithWrappedMutables_branchingCommonParent
#[test]
fn test_all_chunks_with_wrapped_mutables_branching_common_parent() {
    let mut t = T::new();
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_tree()
                .add_chunk("var x = 1;")
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("a = 2; a + 2;")
                .build(),
        ),
        expected_many(&[
            "_.a = {}; var x = 1;",
            "var {a} = _; a._ = 1; a._ + 1;",
            "var {a} = _; a._ = 2; a._ + 2;",
        ]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksLocalAccessOptimization_es5
#[test]
fn test_all_chunks_local_access_optimization_es5() {
    let mut t = T::new();
    t.base.language_out = LanguageMode::ECMASCRIPT5;
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("a;")
                .build(),
        ),
        expected_many(&["var a; a = _.a = 1; a + 1;", "var a = _.a; a;"]),
    );
}

// port: RescopeGlobalSymbolsTest#testAllChunksWithWrappedMutables_multipleHolderSymbols_es5
#[test]
fn test_all_chunks_with_wrapped_mutables_multiple_holder_symbols_es5() {
    let mut t = T::new();
    t.base.language_out = LanguageMode::ECMASCRIPT5;
    t.optimize_local_access = OptimizeLocalAccess::ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS;
    t.assume_cross_chunk_names = false;
    t.test(
        srcs_chunks(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var a = 1; a + 1;")
                .add_chunk("var b = 1; b + 1;")
                .add_chunk("a = 2; b = 2;")
                .build(),
        ),
        expected_many(&[
            "var a = _.a = {}; a._ = 1; a._ + 1;",
            "var b = _.b = {}; b._ = 1; b._ + 1;",
            "var a = _.a; var b = _.b; a._ = 2; b._ = 2;",
        ]),
    );
}
