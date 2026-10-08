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

//! Rust-only: Compiler#initRuntimeLibraryTypedAsts deserializes runtime_libs.typedast and the
//! colors branch of Compiler#loadResourceContents returns a deserialized SCRIPT.
use closure_jscomp::abstract_compiler::LifeCycleStage;
use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_options::CompilerOptions;
use closure_rhino::token::Token;

#[test]
fn loads_runtime_library_from_typed_ast() {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    compiler.init_runtime_library_typed_asts(None);
    compiler.set_life_cycle_stage(LifeCycleStage::COLORS_AND_SIMPLIFIED_JSDOC);
    let script = compiler.load_resource_contents(
        "util/global",
        "src/com/google/javascript/jscomp/js/util/global.js",
    );
    assert_eq!(script.get_token(&compiler), Token::SCRIPT);
    let source = compiler.to_source_for_node(script);
    assert!(source.contains("$jscomp.getGlobal"), "{source}");
}
