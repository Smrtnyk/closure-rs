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
//   test/com/google/javascript/jscomp/ConvertChunksToESModulesTest.java.

#[path = "support/pass_test_case.rs"]
mod pass_test_case;

use closure_jscomp::{
    Compiler,
    convert_chunks_to_es_modules::{
        ASSIGNMENT_TO_IMPORT, ConvertChunksToESModules, DYNAMIC_IMPORT_CALLBACK_FN,
        UNABLE_TO_COMPUTE_RELATIVE_PATH, UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK,
    },
    deps::module_loader::LOAD_WARNING,
    source_file::SourceFile,
};
use closure_testing::testing::js_chunk_graph_builder::JSChunkGraphBuilder;
use pass_test_case::{PassTestCase, error, expected_chunks, srcs_chunks};

// port: ConvertChunksToESModulesTest#getProcessor
fn get_processor(_compiler: &mut Compiler) -> ConvertChunksToESModules {
    ConvertChunksToESModules::new()
}

fn t() -> PassTestCase {
    PassTestCase::new("")
}

// port: ConvertChunksToESModulesTest#testVarDeclarations_acrossChunks
#[test]
fn test_var_declarations_across_chunks() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1;")
                .add_chunk("a")
                .build(),
        ),
        expected_chunks(
            &JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1; export {a}")
                .add_chunk("import {a} from './m0.js'; a")
                .build(),
        ),
    );

    t.test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1, b = 2, c = 3;")
                .add_chunk("a;c;")
                .build(),
        ),
        expected_chunks(
            &JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1, b = 2, c = 3; export {a, c};")
                .add_chunk("import {a, c} from './m0.js'; a; c;")
                .build(),
        ),
    );
    t.test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1, b = 2, c = 3;")
                .add_chunk("b;c;")
                .build(),
        ),
        expected_chunks(
            &JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1, b = 2, c = 3; export {b,c};")
                .add_chunk("import {b, c} from './m0.js'; b;c;")
                .build(),
        ),
    );
}

// port: ConvertChunksToESModulesTest#testMultipleInputsPerChunk
#[test]
fn test_multiple_inputs_per_chunk() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk("var a = 1;")
        .add_chunk("a")
        .build();

    original[0].add_source_file(SourceFile::from_code("m0-1", "console.log(a)"));

    let expected = JSChunkGraphBuilder::for_star()
        .add_chunk("var a = 1; console.log(a); export {a}")
        .add_chunk("import {a} from './m0.js'; a")
        .build();

    expected[0].add_source_file(SourceFile::from_code("m0-1", ""));

    t.test(
        get_processor,
        srcs_chunks(original),
        expected_chunks(&expected),
    );
}

// port: ConvertChunksToESModulesTest#testImportPathReferenceAbsolute
#[test]
fn test_import_path_reference_absolute() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1;", "/js/m0")
        .add_chunk_with_name("a", "/js/m1")
        .build();

    let expected = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1; export {a}", "/js/m0")
        .add_chunk_with_name("import {a} from './m0.js'; a", "/js/m1")
        .build();

    t.test(
        get_processor,
        srcs_chunks(original),
        expected_chunks(&expected),
    );
}

// port: ConvertChunksToESModulesTest#testImportPathReferenceAbsoluteWithRelative1
#[test]
fn test_import_path_reference_absolute_with_relative1() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1;", "other/m0")
        .add_chunk_with_name("a", "/js/m1")
        .build();

    t.test_error(
        get_processor,
        srcs_chunks(original),
        error(&UNABLE_TO_COMPUTE_RELATIVE_PATH).with_message(
            "Unable to compute relative import path from \"/js/m1.js\" to \"other/m0.js\"",
        ),
    );
}

// port: ConvertChunksToESModulesTest#testImportPathReferenceAbsoluteWithRelative2
#[test]
fn test_import_path_reference_absolute_with_relative2() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1;", "/other/m0")
        .add_chunk_with_name("a", "js/m1")
        .build();

    t.test_error(
        get_processor,
        srcs_chunks(original),
        error(&UNABLE_TO_COMPUTE_RELATIVE_PATH).with_message(
            "Unable to compute relative import path from \"js/m1.js\" to \"/other/m0.js\"",
        ),
    );
}

// port: ConvertChunksToESModulesTest#testImportPathAmbiguous
#[test]
fn test_import_path_ambiguous() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1;", "js/m0")
        .add_chunk_with_name("a", "js/m1")
        .build();

    let expected = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1; export {a}", "js/m0")
        .add_chunk_with_name("import {a} from './m0.js'; a", "js/m1")
        .build();

    t.test(
        get_processor,
        srcs_chunks(original),
        expected_chunks(&expected),
    );
}

// port: ConvertChunksToESModulesTest#testImportPathMixedDepth1
#[test]
fn test_import_path_mixed_depth1() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1;", "js/m0")
        .add_chunk_with_name("a", "m1")
        .build();

    t.test_error(
        get_processor,
        srcs_chunks(original),
        error(&UNABLE_TO_COMPUTE_RELATIVE_PATH)
            .with_message("Unable to compute relative import path from \"m1.js\" to \"js/m0.js\""),
    );
}

// port: ConvertChunksToESModulesTest#testImportPathMixedDepth2
#[test]
fn test_import_path_mixed_depth2() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1;", "m0")
        .add_chunk_with_name("a", "js/m1")
        .build();

    let expected = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1; export {a}", "m0")
        .add_chunk_with_name("import {a} from '../m0.js'; a", "js/m1")
        .build();

    t.test(
        get_processor,
        srcs_chunks(original),
        expected_chunks(&expected),
    );
}

// port: ConvertChunksToESModulesTest#testImportPathMixedDepth3
#[test]
fn test_import_path_mixed_depth3() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1;", "js/other/path/one/m0")
        .add_chunk_with_name("a", "external/path/m1")
        .build();

    let expected = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1; export {a}", "js/other/path/one/m0")
        .add_chunk_with_name(
            "import {a} from '../../js/other/path/one/m0.js'; a",
            "external/path/m1",
        )
        .build();

    t.test(
        get_processor,
        srcs_chunks(original),
        expected_chunks(&expected),
    );
}

// port: ConvertChunksToESModulesTest#testImportPathParentAboveRoot
#[test]
fn test_import_path_parent_above_root() {
    let t = t();
    let original = JSChunkGraphBuilder::for_star()
        .add_chunk_with_name("var a = 1;", "js/m0")
        .add_chunk_with_name("a", "../node_modules/m1")
        .build();

    t.test_error(
        get_processor,
        srcs_chunks(original),
        error(&UNABLE_TO_COMPUTE_RELATIVE_PATH).with_message(
            "Unable to compute relative import path from \"../node_modules/m1.js\" to \"js/m0.js\"",
        ),
    );
}

// port: ConvertChunksToESModulesTest#testForcedESModuleSemantics
#[test]
fn test_forced_es_module_semantics() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1;")
                .add_chunk("var b = 1;")
                .build(),
        ),
        expected_chunks(
            &JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1; export {};")
                .add_chunk("import './m0.js'; var b = 1;")
                .build(),
        ),
    );
}

// port: ConvertChunksToESModulesTest#testAssignToImport
#[test]
fn test_assign_to_import() {
    t().test_error(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk("var a = 1;")
                .add_chunk("a = 2;")
                .build(),
        ),
        error(&ASSIGNMENT_TO_IMPORT).with_message(
            "Imported symbol \"a\" in chunk \"m1.js\" cannot be assigned (defined in \"m0.js\")",
        ),
    );
}

// port: ConvertChunksToESModulesTest#testChunkDependenciesCreateImportStatementsForSideEffects
#[test]
fn test_chunk_dependencies_create_import_statements_for_side_effects() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk("window.a = true")
                .add_chunk("console.log(window.a) // should be true")
                .build(),
        ),
        expected_chunks(
            &JSChunkGraphBuilder::for_star()
                .add_chunk("window.a = true; export {};")
                .add_chunk("import './m0.js'; console.log(window.a);")
                .build(),
        ),
    );
}

// port: ConvertChunksToESModulesTest#testDynamicImportRewriting1
#[test]
fn test_dynamic_import_rewriting1() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "import('./m1.js')
    .then(DYNAMIC_IMPORT_CALLBACK_FN(function () { return module$i1; }))
    .then(function (ns) { console.log(ns.default); });
"
                    .replace("DYNAMIC_IMPORT_CALLBACK_FN", DYNAMIC_IMPORT_CALLBACK_FN),
                )
                .add_chunk(
                    "const a$$module$i1 = 1;
var $jscompDefaultExport$$module$i1 = a$$module$i1;
/** @const */ var module$i1 = {};
/** @const */ module$i1.default = $jscompDefaultExport$$module$i1;
",
                )
                .build(),
        ),
        expected_chunks(
            &JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "import('./m1.js')
    .then(function ($) { return $.module$i1; })
    .then(function (ns) { console.log(ns.default); });
export {};
",
                )
                .add_chunk(
                    "import './m0.js';
const a$$module$i1 = 1;
var $jscompDefaultExport$$module$i1 = a$$module$i1;
/** @const */ var module$i1 = {};
/** @const */ module$i1.default = $jscompDefaultExport$$module$i1;
export {module$i1};
",
                )
                .build(),
        ),
    );
}

// port: ConvertChunksToESModulesTest#testDynamicImportRewriting2
#[test]
fn test_dynamic_import_rewriting2() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "const a$$module$i0 = 1;
var $jscompDefaultExport$$module$i0 = a$$module$i0;
/** @const */ var module$i0 = {};
/** @const */ module$i0.default = $jscompDefaultExport$$module$i0;
",
                )
                .add_chunk(
                    "import('./m0.js')
    .then(DYNAMIC_IMPORT_CALLBACK_FN(() => module$i0))
    .then((ns) => console.log(ns.default));
"
                    .replace("DYNAMIC_IMPORT_CALLBACK_FN", DYNAMIC_IMPORT_CALLBACK_FN),
                )
                .build(),
        ),
        expected_chunks(
            &JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "const a$$module$i0 = 1;
var $jscompDefaultExport$$module$i0 = a$$module$i0;
/** @const */ var module$i0 = {};
/** @const */ module$i0.default = $jscompDefaultExport$$module$i0;
export {module$i0};
",
                )
                .add_chunk(
                    "import './m0.js';
import('./m0.js')
    .then(($) => $.module$i0)
    .then((ns) => console.log(ns.default));
",
                )
                .build(),
        ),
    );
}

// port: ConvertChunksToESModulesTest#testDynamicImportWithoutCallback
#[test]
fn test_dynamic_import_without_callback() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.test(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "var a$$module$i0 = 1;
var $jscompDefaultExport$$module$i0 = a$$module$i0;
/** @const */ var module$i0 = {};
/** @const */ module$i0.default = $jscompDefaultExport$$module$i0;
",
                )
                .add_chunk(
                    "import('./m0.js')
    .then((ns) => console.log(ns.default));
",
                )
                .build(),
        ),
        expected_chunks(
            &JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "var a$$module$i0 = 1;
var $jscompDefaultExport$$module$i0 = a$$module$i0;
/** @const */ var module$i0 = {};
/** @const */ module$i0.default = $jscompDefaultExport$$module$i0;
export {};
",
                )
                .add_chunk(
                    "import './m0.js';
import('./m0.js')
    .then((ns) => console.log(ns.default));
",
                )
                .build(),
        ),
    );
}

// port: ConvertChunksToESModulesTest#testDynamicImportRewritingErrors1
#[test]
fn test_dynamic_import_rewriting_errors1() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.test_error(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "var a$$module$i0 = 1;
var $jscompDefaultExport$$module$i0 = a$$module$i0;
/** @const */ var module$i0 = {};
/** @const */ module$i0.default = $jscompDefaultExport$$module$i0;
",
                )
                .add_chunk(
                    "import('./m0.js')
    .then(DYNAMIC_IMPORT_CALLBACK_FN(() => {}))
    .then((ns) => console.log(ns.default));
"
                    .replace("DYNAMIC_IMPORT_CALLBACK_FN", DYNAMIC_IMPORT_CALLBACK_FN),
                )
                .build(),
        ),
        error(&UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK).with_message(
            "Dynamic import callback encountered wih an invalid format. Unable to find valid namespace reference.",
        ),
    );
}

// port: ConvertChunksToESModulesTest#testDynamicImportRewritingErrors2
#[test]
fn test_dynamic_import_rewriting_errors2() {
    let mut t = t();
    t.ignore_warnings(&[&LOAD_WARNING]);
    t.test_error(
        get_processor,
        srcs_chunks(
            JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "var a$$module$i0 = 1;
var $jscompDefaultExport$$module$i0 = a$$module$i0;
/** @const */ var module$i0 = {};
/** @const */ module$i0.default = $jscompDefaultExport$$module$i0;
",
                )
                .add_chunk(
                    "import('./m0.js')
    .then(DYNAMIC_IMPORT_CALLBACK_FN(true))
    .then((ns) => console.log(ns.default));
"
                    .replace("DYNAMIC_IMPORT_CALLBACK_FN", DYNAMIC_IMPORT_CALLBACK_FN),
                )
                .build(),
        ),
        error(&UNRECOGNIZED_DYNAMIC_IMPORT_CALLBACK).with_message(
            "Dynamic import callback encountered wih an invalid format. Unable to find valid callback function.",
        ),
    );
}
