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

//! Rust-only check of registry-aware node printing (crates/testing/src/node_printing.rs over
//! rhino's `JSTypePrinter` variants of `Node#toString`/`Node#toStringTree`): JSTypes and colors
//! print as Java's `Node#toStringTree` prints them, including the JSDoc type expressions inside
//! `[jsdoc_info: ..]`. Expected texts come from Java (closure-compiler.jar, program and output
//! cached under corpus-cache/var-checks/node_printing/).

use closure_jscomp::{
    compiler::Compiler, compiler_options::CompilerOptions, source_file::SourceFile,
    type_check::TypeCheck,
};
use closure_testing::node_printing;
use std::sync::Arc;

const SOURCE: &str = "/** @param {?string} s */ function f(s) { var y = s; return y; }";

// Java: compiler.getJsRoot().toStringTree() after TypeCheck#processForTesting.
const TYPED_TREE: &str = r#"ROOT : global this
    SCRIPT 1:0  [length: 64] [source_file: source] [input_id: InputId: source] [feature_set: []]
        FUNCTION f 1:26  [length: 38] [jsdoc_info: JSDocInfo{parameters={s=type: QMARK 1:12  [source_file: source] : (null|string)
    STRINGLIT string 1:13  [length: 6] [source_file: source]
}}] [source_file: source] : function((null|string)): ?
            NAME f 1:35  [length: 1] [source_file: source] : function((null|string)): ?
            PARAM_LIST 1:36  [length: 3] [source_file: source]
                NAME s 1:37  [length: 1] [source_file: source] : (null|string)
            BLOCK 1:40  [length: 24] [source_file: source]
                VAR 1:42  [length: 10] [source_file: source]
                    NAME y 1:46  [length: 5] [source_file: source] : (null|string)
                        NAME s 1:50  [length: 1] [source_file: source] : (null|string)
                RETURN 1:53  [length: 9] [source_file: source]
                    NAME y 1:60  [length: 1] [source_file: source] : (null|string)
"#;

// Java: functionNode.toString(false, false, true).
const TYPED_FUNCTION: &str = r#"FUNCTION f : function((null|string)): ?"#;

// Java: compiler.getJsRoot().toStringTree() after ConvertTypesToColors.
const COLORED_TREE: &str = r#"ROOT
    SCRIPT 1:0  [length: 64] [source_file: source] [input_id: InputId: source] [feature_set: []]
        FUNCTION f 1:26  [length: 38] [source_file: source] : Color{id=283b5838, prototypes=[], instanceColors=[], invalidating=true, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}
            NAME f 1:35  [length: 1] [source_file: source] : Color{id=283b5838, prototypes=[], instanceColors=[], invalidating=true, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}
            PARAM_LIST 1:36  [length: 3] [source_file: source]
                NAME s 1:37  [length: 1] [source_file: source] : Color{id=3ca41e9947796d8, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=8c4d8f65, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=186008a9, closureAssert=false, unionElements=[]}, Color{id=22b49f69, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}]}
            BLOCK 1:40  [length: 24] [source_file: source]
                VAR 1:42  [length: 10] [source_file: source]
                    NAME y 1:46  [length: 5] [source_file: source] : Color{id=3ca41e9947796d8, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=8c4d8f65, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=186008a9, closureAssert=false, unionElements=[]}, Color{id=22b49f69, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}]}
                        NAME s 1:50  [length: 1] [source_file: source] : Color{id=3ca41e9947796d8, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=8c4d8f65, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=186008a9, closureAssert=false, unionElements=[]}, Color{id=22b49f69, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}]}
                RETURN 1:53  [length: 9] [source_file: source]
                    NAME y 1:60  [length: 1] [source_file: source] : Color{id=3ca41e9947796d8, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[Color{id=8c4d8f65, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=186008a9, closureAssert=false, unionElements=[]}, Color{id=22b49f69, prototypes=[], instanceColors=[], invalidating=false, propertiesKeepOriginalName=false, constructor=false, ownProperties=[], boxId=null, closureAssert=false, unionElements=[]}]}
"#;

fn parse_and_add_types(compiler: &mut Compiler) {
    compiler.init(
        &[Arc::new(SourceFile::from_code("externs", ""))],
        &[Arc::new(SourceFile::from_code("source", SOURCE))],
        CompilerOptions::new(),
    );
    compiler.parse_inputs();
    assert!(compiler.get_errors().is_empty(), "parse error");
    let interpreter = compiler.get_reverse_abstract_interpreter();
    let mut type_check = TypeCheck::new(compiler, interpreter);
    let externs_root = compiler.get_externs_root();
    let js_root = compiler.get_js_root().unwrap();
    type_check.process_for_testing(compiler, externs_root, js_root);
    compiler.set_type_checking_has_run(true);
}

#[test]
fn typed_tree_prints_jstypes_and_typed_jsdoc_type_expressions() {
    let mut compiler = Compiler::new();
    parse_and_add_types(&mut compiler);
    let js_root = compiler.get_js_root().unwrap();
    assert_eq!(
        node_printing::to_string_tree(&mut compiler, None, js_root),
        TYPED_TREE
    );
    let function = js_root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert_eq!(
        node_printing::to_string_with_options(&mut compiler, None, function, false, false, true),
        TYPED_FUNCTION
    );
}

#[test]
fn colored_tree_after_convert_types_to_colors_prints_colors() {
    let mut compiler = Compiler::new();
    parse_and_add_types(&mut compiler);
    let externs_root = compiler.get_externs_root().unwrap();
    let js_root = compiler.get_js_root().unwrap();
    closure_testing::harness_passes::convert_types_to_colors(
        &mut compiler,
        externs_root,
        js_root,
        false,
    )
    .unwrap();
    assert_eq!(
        node_printing::to_string_tree(&mut compiler, None, js_root),
        COLORED_TREE
    );
    // Without JSTypes the registry-free rhino printer prints the same text.
    assert_eq!(js_root.to_string_tree(&compiler), COLORED_TREE);
}
