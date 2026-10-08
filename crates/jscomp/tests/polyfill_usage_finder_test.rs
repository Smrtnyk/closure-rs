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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/PolyfillUsageFinderTest.java.

//! Port of PolyfillUsageFinderTest.java: unit tests for PolyfillUsageFinder.
use closure_jscomp::{
    Compiler,
    compiler_options::CompilerOptions,
    polyfill_usage_finder::{Kind, Polyfill, PolyfillUsage, PolyfillUsageFinder, Polyfills},
    source_file::SourceFile,
};
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};
use closure_testing::testing::test_externs_builder::TestExternsBuilder;
use std::sync::Arc;

/// Covers testing of most common cases.
///
/// <ul>
///   <li>guarded & unguarded references
///   <li>global symbols & prototype methods
///   <li>non-polyfill references
///   <li>references via a named global object
/// </ul>
// port: PolyfillUsageFinderTest#basicCoverageTest
#[test]
fn basic_coverage_test() {
    let mut test_polyfill_usage_finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines(
            "Array.prototype.fill es6 es3 es6/array/fill\n\
             Array.from es6 es3 es6/array/from\n\
             Map es6 es3 es6/map\n",
        )
        .for_source_lines(
            "use(new Map()); // 1\n\
             globalThis.Array.from // 2\n\
             \x20 ? globalThis.Array.from([]) // 3\n\
             \x20 : []; // 4\n\
             [1, 2, 3].fill(0); // 5\n\
             notAPolyfill(); // 6\n",
        )
        .build();

    let unguarded_polyfill_usages = test_polyfill_usage_finder.get_unguarded_usages();

    // Map() and [].fill() references
    assert_eq!(unguarded_polyfill_usages.len(), 2);
    let ast: &Ast = &test_polyfill_usage_finder.compiler;

    // Source line 1: `use(new Map())`
    let map_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &unguarded_polyfill_usages[0]);
    map_usage_subject
        .has_node_that()
        .is_name("Map")
        .has_lineno(1);
    map_usage_subject.has_name("Map").is_not_explicit_global();
    map_usage_subject
        .has_polyfill_that()
        .has_native_symbol("Map")
        .has_native_version_str("es6")
        .has_polyfill_version_str("es3")
        .has_library("es6/map")
        .has_kind(Kind::STATIC);

    // Source line 5: `[1, 2, 3].fill(0);`
    let array_fill_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &unguarded_polyfill_usages[1]);
    array_fill_usage_subject
        .has_node_that()
        .is_get_prop()
        .has_lineno(5);
    array_fill_usage_subject
        .has_name("fill")
        .is_not_explicit_global();
    array_fill_usage_subject
        .has_polyfill_that()
        .has_native_symbol("Array.prototype.fill")
        .has_native_version_str("es6")
        .has_polyfill_version_str("es3")
        .has_library("es6/array/fill")
        .has_kind(Kind::METHOD);

    // The 2 references to `Array.from` on line 2 and 3
    // Note that the guard expression itself counts as a guarded use.
    // Source line 2: `globalThis.Array.from ? globalThis.Array.from([]) : [];`
    let guarded_polyfill_usages = test_polyfill_usage_finder.get_guarded_usages();
    assert_eq!(guarded_polyfill_usages.len(), 2);
    let ast: &Ast = &test_polyfill_usage_finder.compiler;

    let hook_condition_use_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &guarded_polyfill_usages[0]);
    hook_condition_use_subject
        .has_node_that()
        .is_get_prop()
        .has_lineno(2);

    let array_dot_from_use_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &guarded_polyfill_usages[1]);
    array_dot_from_use_subject
        .has_node_that()
        .is_get_prop()
        .has_lineno(3);

    // Other than the node they reference, both usage objects are the same
    for array_dot_from_usage_subject in [hook_condition_use_subject, array_dot_from_use_subject] {
        array_dot_from_usage_subject.has_name("Array.from");
        // reference via `globalThis` makes these explicit global references
        array_dot_from_usage_subject.is_explicit_global();
        array_dot_from_usage_subject
            .has_polyfill_that()
            .has_native_symbol("Array.from")
            .has_native_version_str("es6")
            .has_polyfill_version_str("es3")
            .has_library("es6/array/from")
            .has_kind(Kind::STATIC);
    }
}

// port: PolyfillUsageFinderTest#shadowedPolyfillTest
#[test]
fn shadowed_polyfill_test() {
    let mut test_polyfill_usage_finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines(
            "Promise es6 es3 es6/promise/promise\n\
             Promise.allSettled es_2020 es3 es6/promise/allsettled\n",
        )
        .for_source_lines(
            "// usage of Promise and of Promise.allSettled\n\
             Promise.allSettled([])(result => console.log(x));\n\
             function foo() {\n\
             \x20 // These aren't references to Promise or Promise.allSettled because of the\n\
             \x20 // shadowing variable\n\
             \x20 let Promise = { allSettled: 'I solemnly swear that I am up to no good.' };\n\
             \x20 console.log(Promise.allSettled);\n\
             }\n",
        )
        .build();

    let all_usages = test_polyfill_usage_finder.get_all_usages();
    assert_eq!(all_usages.len(), 2);
    let ast: &Ast = &test_polyfill_usage_finder.compiler;

    let promise_usage_subject = PolyfillUsageSubject::assert_polyfill_usage(ast, &all_usages[0]);
    promise_usage_subject //
        .has_node_that()
        .is_name("Promise")
        .has_lineno(2);
    promise_usage_subject
        .has_polyfill_that()
        .has_native_symbol("Promise")
        .has_native_version_str("es6")
        .has_polyfill_version_str("es3")
        .has_library("es6/promise/promise")
        .has_kind(Kind::STATIC);

    let all_settled_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &all_usages[1]);

    all_settled_usage_subject //
        .has_node_that()
        .matches_qualified_name("Promise.allSettled")
        .has_lineno(2);
    all_settled_usage_subject
        .has_polyfill_that()
        .has_native_symbol("Promise.allSettled")
        .has_native_version_str("es_2020")
        .has_polyfill_version_str("es3")
        .has_library("es6/promise/allsettled")
        .has_kind(Kind::STATIC);
}

// port: PolyfillUsageFinderTest#optChainCoverageTest
#[test]
fn opt_chain_coverage_test() {
    let mut test_polyfill_usage_finder = TestPolyfillUsageFinder::builder()
        .with_polyfill_table_lines("Array.from es6 es3 es6/array/from")
        .for_source_lines(
            "// Checks for the existence of `Array`, but not `.from`, so not guarded\n\
             Array?.from([]) ?? [];\n\
             // Correctly guarded on existence of `Array.from`\n\
             Array.from?.([]) ?? [];\n",
        )
        .build();

    // First usage is unguarded
    let unguarded_polyfill_usages = test_polyfill_usage_finder.get_unguarded_usages();
    assert_eq!(unguarded_polyfill_usages.len(), 1);
    // Second usage is guarded
    let guarded_polyfill_usages = test_polyfill_usage_finder.get_guarded_usages();
    let ast: &Ast = &test_polyfill_usage_finder.compiler;
    let unguarded_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &unguarded_polyfill_usages[0]);
    unguarded_usage_subject
        .has_node_that()
        .has_token(Token::OPTCHAIN_GETPROP)
        .has_lineno(2);

    assert_eq!(guarded_polyfill_usages.len(), 1);
    let guarded_usage_subject =
        PolyfillUsageSubject::assert_polyfill_usage(ast, &guarded_polyfill_usages[0]);
    guarded_usage_subject
        .has_node_that()
        .has_token(Token::GETPROP)
        .has_lineno(4);

    // Other than the node they reference, both usage objects are the same
    for array_dot_from_usage_subject in [unguarded_usage_subject, guarded_usage_subject] {
        array_dot_from_usage_subject.has_name("Array.from");
        // reference via `globalThis` makes these explicit global references
        array_dot_from_usage_subject.is_not_explicit_global();
        array_dot_from_usage_subject
            .has_polyfill_that()
            .has_native_symbol("Array.from")
            .has_native_version_str("es6")
            .has_polyfill_version_str("es3")
            .has_library("es6/array/from")
            .has_kind(Kind::STATIC);
    }
}

/// Uses {@link PolyfillUsageFinder} to find polyfill usages in source code for testing.
struct TestPolyfillUsageFinder {
    polyfill_usage_finder: PolyfillUsageFinder,
    root_node: NodeId,
    compiler: Compiler,
}

#[derive(Default)]
struct Builder {
    polyfills_table: Option<Polyfills>,
    compiler: Option<Compiler>,
    polyfill_usage_finder: Option<PolyfillUsageFinder>,
    root_node: Option<NodeId>,
}

impl TestPolyfillUsageFinder {
    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#builder
    fn builder() -> Builder {
        Builder::default()
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#TestPolyfillUsageFinder
    fn new(builder: Builder) -> Self {
        Self {
            polyfill_usage_finder: builder.polyfill_usage_finder.unwrap(),
            root_node: builder.root_node.unwrap(),
            compiler: builder.compiler.unwrap(),
        }
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#getUnguardedUsages
    fn get_unguarded_usages(&mut self) -> Vec<PolyfillUsage> {
        let mut consumer = PolyfillUsageCollectingConsumer::default();
        self.polyfill_usage_finder.traverse_excluding_guarded(
            &mut self.compiler,
            self.root_node,
            &mut |_compiler: &mut Compiler, usage| consumer.accept(usage),
        );
        consumer.get_usages()
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#getGuardedUsages
    fn get_guarded_usages(&mut self) -> Vec<PolyfillUsage> {
        let mut consumer = PolyfillUsageCollectingConsumer::default();
        self.polyfill_usage_finder.traverse_only_guarded(
            &mut self.compiler,
            self.root_node,
            &mut |_compiler: &mut Compiler, usage| consumer.accept(usage),
        );
        consumer.get_usages()
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder#getAllUsages
    fn get_all_usages(&mut self) -> Vec<PolyfillUsage> {
        let mut consumer = PolyfillUsageCollectingConsumer::default();
        self.polyfill_usage_finder.traverse_including_guarded(
            &mut self.compiler,
            self.root_node,
            &mut |_compiler: &mut Compiler, usage| consumer.accept(usage),
        );
        consumer.get_usages()
    }
}

impl Builder {
    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder.Builder#withPolyfillTableLines
    fn with_polyfill_table_lines(mut self, polyfill_table: &str) -> Self {
        self.polyfills_table = Some(Polyfills::from_table(polyfill_table));
        self
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder.Builder#forSourceLines
    fn for_source_lines(mut self, src: &str) -> Self {
        let src_file = SourceFile::builder()
            .with_path("src.js")
            .with_content(src)
            .build();
        let mut options = CompilerOptions::new();
        // Don't include `"use strict";` when printing the AST as source text
        options.set_emit_use_strict(false);
        let mut compiler = Compiler::new();
        compiler.init(
            &[Arc::new(
                TestExternsBuilder::new().build_externs_file("externs.js"),
            )],
            &[Arc::new(src_file)],
            options,
        );
        compiler.parse();
        self.root_node = compiler.get_js_root();
        self.compiler = Some(compiler);
        self
    }

    // port: PolyfillUsageFinderTest.TestPolyfillUsageFinder.Builder#build
    fn build(mut self) -> TestPolyfillUsageFinder {
        assert!(self.compiler.is_some());
        self.polyfill_usage_finder = Some(PolyfillUsageFinder::new(Arc::new(
            self.polyfills_table.take().unwrap(),
        )));
        TestPolyfillUsageFinder::new(self)
    }
}

/// Consumes {@link PolyfillUsage} objects by storing them into a retrievable list.
#[derive(Default)]
struct PolyfillUsageCollectingConsumer {
    polyfill_usage_list_builder: Vec<PolyfillUsage>,
}

impl PolyfillUsageCollectingConsumer {
    // port: PolyfillUsageFinderTest.PolyfillUsageCollectingConsumer#accept
    fn accept(&mut self, polyfill_usage: PolyfillUsage) {
        self.polyfill_usage_list_builder.push(polyfill_usage);
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageCollectingConsumer#getUsages
    fn get_usages(self) -> Vec<PolyfillUsage> {
        self.polyfill_usage_list_builder
    }
}

struct PolyfillSubject<'a> {
    actual: &'a Polyfill,
}

impl<'a> PolyfillSubject<'a> {
    // port: PolyfillUsageFinderTest.PolyfillSubject#assertPolyfill
    fn assert_polyfill(polyfill: &'a Polyfill) -> Self {
        Self { actual: polyfill }
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasNativeSymbol
    fn has_native_symbol(&self, expected: &str) -> &Self {
        assert_eq!(self.actual.native_symbol, expected, "nativeSymbol");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasNativeVersionStr
    fn has_native_version_str(&self, expected: &str) -> &Self {
        assert_eq!(self.actual.native_version, expected, "nativeVersion");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasPolyfillVersionStr
    fn has_polyfill_version_str(&self, expected: &str) -> &Self {
        assert_eq!(self.actual.polyfill_version, expected, "polyfillVersion");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasLibrary
    fn has_library(&self, expected_library_path: &str) -> &Self {
        assert_eq!(self.actual.library, expected_library_path, "library");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillSubject#hasKind
    fn has_kind(&self, expected_kind: Kind) -> &Self {
        assert_eq!(self.actual.kind, expected_kind, "kind");
        self
    }
}

#[derive(Clone, Copy)]
struct PolyfillUsageSubject<'a> {
    ast: &'a Ast,
    actual: &'a PolyfillUsage,
}

impl<'a> PolyfillUsageSubject<'a> {
    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#assertPolyfillUsage
    fn assert_polyfill_usage(ast: &'a Ast, polyfill_usage: &'a PolyfillUsage) -> Self {
        Self {
            ast,
            actual: polyfill_usage,
        }
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#hasName
    fn has_name(&self, expected_name: &str) -> &Self {
        assert_eq!(*self.actual.name(), expected_name, "name()");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#isExplicitGlobal
    fn is_explicit_global(&self) -> &Self {
        assert!(self.actual.is_explicit_global(), "isExplicitGlobal()");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#isNotExplicitGlobal
    fn is_not_explicit_global(&self) -> &Self {
        assert!(!self.actual.is_explicit_global(), "isExplicitGlobal()");
        self
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#hasNodeThat
    fn has_node_that(&self) -> NodeAssertions<'a> {
        NodeAssertions {
            ast: self.ast,
            actual: self.actual.node(),
        }
    }

    // port: PolyfillUsageFinderTest.PolyfillUsageSubject#hasPolyfillThat
    fn has_polyfill_that(&self) -> PolyfillSubject<'a> {
        PolyfillSubject::assert_polyfill(self.actual.polyfill())
    }
}

/// The NodeSubject assertions these tests use (rhino's Rust NodeSubject has no isName, isGetProp,
/// hasToken or matchesQualifiedName yet); each checks what Java's NodeSubject method checks.
struct NodeAssertions<'a> {
    ast: &'a Ast,
    actual: NodeId,
}

// The NodeSubject assertions of NodeAssertions, which name Closure's Rhino-derived NodeSubject
// (MPL-1.1 / GPL-2.0-or-later), are in their own file.
#[path = "rhino/polyfill_usage_finder_test.rs"]
mod rhino;
