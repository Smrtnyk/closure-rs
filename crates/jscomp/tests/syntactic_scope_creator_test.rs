/*
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/testing/ScopeSubject.java,
//   test/com/google/javascript/jscomp/SyntacticScopeCreatorTest.java.

use closure_jscomp::{
    Compiler,
    compiler_input::CompilerInput,
    compiler_options::{CompilerOptions, LanguageMode},
    node_util::NodeUtil,
    scope::ScopeId,
    syntactic_scope_creator::{RedeclarationHandler, SyntacticScopeCreator},
    var::VarId,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{
    ir::IR, js_string::JsString, node::NodeId, static_source_file::SourceKind, token::Token,
};
use std::sync::{Arc, Mutex};

struct RecordingRedeclarationHandler(Arc<Mutex<IndexMap<JsString, usize>>>);
impl RedeclarationHandler for RecordingRedeclarationHandler {
    // port: SyntacticScopeCreatorTest.RecordingRedeclarationHandler#onRedeclaration
    fn on_redeclaration(
        &mut self,
        _compiler: &mut Compiler,
        _s: ScopeId,
        name: &JsString,
        _n: NodeId,
        _input: Option<CompilerInput>,
    ) {
        *self.0.lock().unwrap().entry(name.clone()).or_default() += 1;
    }
}
// port: SyntacticScopeCreatorTest#setUp
fn set_up() -> (
    Compiler,
    SyntacticScopeCreator<'static>,
    Arc<Mutex<IndexMap<JsString, usize>>>,
) {
    let mut compiler = Compiler::new();
    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::UNSUPPORTED);
    compiler.init_options(options);
    let redeclarations = Arc::new(Mutex::new(IndexMap::<_, _>::default()));
    let scope_creator = SyntacticScopeCreator::new_with_redeclaration_handler(Box::new(
        RecordingRedeclarationHandler(redeclarations.clone()),
    ));
    (compiler, scope_creator, redeclarations)
}
// port: SyntacticScopeCreatorTest#getRoot
fn get_root(compiler: &mut Compiler, js: &str) -> NodeId {
    let root = compiler.parse_test_code(js);
    assert!(
        compiler.get_errors().is_empty(),
        "{:?}",
        compiler.get_errors()
    );
    root
}
// port: SyntacticScopeCreatorTest#getScope
fn get_scope(
    compiler: &mut Compiler,
    scope_creator: &mut SyntacticScopeCreator<'_>,
    js: &str,
) -> ScopeId {
    let root = get_root(compiler, js);
    scope_creator.create_scope(compiler, root, None)
}
// port: ScopeSubject#declares
fn declares(compiler: &mut Compiler, scope: ScopeId, name: &str) -> ScopeId {
    let var = get_var(compiler, scope, name).expect("expected scope to declare name");
    let declaring_scope = var.get_scope(compiler);
    assert_eq!(
        get_topmost_scope_of_eventual_declaration(compiler, scope, name),
        Some(declaring_scope)
    );
    declaring_scope
}

// port: ScopeSubject#getVar
fn get_var(compiler: &mut Compiler, scope: ScopeId, name: &str) -> Option<VarId> {
    if scope.has_slot(compiler, name) {
        Some(scope.get_var(compiler, name).expect("checkNotNull"))
    } else {
        None
    }
}

// port: ScopeSubject#doesNotDeclare
fn does_not_declare(compiler: &mut Compiler, scope: ScopeId, name: &str) {
    assert!(get_var(compiler, scope, name).is_none());
}

// StaticScope#getTopmostScopeOfEventualDeclaration over ScopeIds, ported from Closure's
// Rhino-derived StaticScope (MPL-1.1 / GPL-2.0-or-later), is in its own file.
#[path = "rhino/syntactic_scope_creator_test.rs"]
mod rhino;
use rhino::get_topmost_scope_of_eventual_declaration;

// port: SyntacticScopeCreatorTest#testVarRedeclaration1
#[test]
fn test_var_redeclaration1() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let value_1 = get_scope(&mut compiler, &mut scope_creator, "var x; var x");
    let _ = value_1;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testVarRedeclaration2
#[test]
fn test_var_redeclaration2() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let value_1 = get_scope(&mut compiler, &mut scope_creator, "var x; var x; var x;");
    let _ = value_1;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        2
    );
}

// port: SyntacticScopeCreatorTest#testVarRedeclaration3
#[test]
fn test_var_redeclaration3() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "var x; if (true) { var x; } var x;";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_next(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(block.is_block(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, block, Some(global_scope));
    let _ = value_3;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        2
    );
}

// port: SyntacticScopeCreatorTest#testVarRedeclaration4
#[test]
fn test_var_redeclaration4() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "var x; if (true) { var x; var x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_next(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(block.is_block(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, block, Some(global_scope));
    let _ = value_3;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        2
    );
}

// port: SyntacticScopeCreatorTest#testVarRedeclaration_withDestructuring
#[test]
fn test_var_redeclaration_with_destructuring() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "function foo() { var x; var [x] = 1; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(block.is_block(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, block, Some(global_scope));
    let _ = value_3;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testVarRedeclaration5
#[test]
fn test_var_redeclaration5() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "if (true) { var x; var x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(block.is_block(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, block, Some(global_scope));
    let _ = value_3;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testVarShadowsParam
#[test]
fn test_var_shadows_param() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f(p) { var p; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global_scope));
    let function_scope = value_3;
    let body = function.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, body, Some(function_scope));
    let body_scope = value_4;
    assert_eq!(
        global_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("f")]
    );
    assert_eq!(
        function_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("p")]
    );
    assert!(body_scope.get_var_iterable(&compiler).is_empty());
}

// port: SyntacticScopeCreatorTest#testParamShadowsFunctionName
#[test]
fn test_param_shadows_function_name() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "var f = function g(g) { }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global_scope));
    let function_scope = value_3;
    let body = function.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, body, Some(function_scope));
    let body_scope = value_4;
    assert_eq!(
        global_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("f")]
    );
    assert_eq!(
        function_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("g")]
    );
    assert!(body_scope.get_var_iterable(&compiler).is_empty());
}

// port: SyntacticScopeCreatorTest#testVarShadowsFunctionName
#[test]
fn test_var_shadows_function_name() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "var f = function g() { var g; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global_scope));
    let function_scope = value_3;
    let body = function.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, body, Some(function_scope));
    let body_scope = value_4;
    assert_eq!(
        global_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("f")]
    );
    assert_eq!(
        function_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("g")]
    );
    assert_eq!(
        body_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("g")]
    );
}

// port: SyntacticScopeCreatorTest#testParamAndVarShadowFunctionName
#[test]
fn test_param_and_var_shadow_function_name() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "var f = function g(g) { var g; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global_scope));
    let function_scope = value_3;
    let body = function.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, body, Some(function_scope));
    let body_scope = value_4;
    assert_eq!(
        global_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("f")]
    );
    assert_eq!(
        function_scope
            .get_var_iterable(&compiler)
            .into_iter()
            .map(|var| var.get_name(&compiler))
            .collect::<Vec<_>>(),
        vec![JsString::from("g")]
    );
    assert!(body_scope.get_var_iterable(&compiler).is_empty());
}

// port: SyntacticScopeCreatorTest#testVarRedeclaration1_inES6Module
#[test]
fn test_var_redeclaration1_in_es6_module() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "export function f() { var x; var x; }";
    let value_1 = get_root(&mut compiler, js);
    let script = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, script, None);
    let global = value_2;
    let module_body = script.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global));
    let module_scope = value_3;
    let function = module_body.get_first_first_child(&compiler).unwrap();
    assert!(function.is_function(&compiler));
    let value_4 = scope_creator.create_scope(&mut compiler, function, Some(module_scope));
    let function_scope = value_4;
    let function_body = function.get_last_child(&compiler).unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, function_body, Some(function_scope));
    let _ = value_5;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testVarRedeclaration2_inES6Module
#[test]
fn test_var_redeclaration2_in_es6_module() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "export var x = 1; export var x = 2;";
    let value_1 = get_root(&mut compiler, js);
    let script = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, script, None);
    let global = value_2;
    let module_body = script.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global));
    let _ = value_3;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testRedeclaration3_inES6Module
#[test]
fn test_redeclaration3_in_es6_module() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "export function f() { var x; if (true) { var x; var x; } var x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let function = module_body.get_first_first_child(&compiler).unwrap();
    assert!(function.is_function(&compiler));
    let value_4 = scope_creator.create_scope(&mut compiler, function, Some(module_scope));
    let function_scope = value_4;
    let function_body = function.get_last_child(&compiler).unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, function_body, Some(function_scope));
    let function_block_scope = value_5;
    let inner_block = function_body
        .get_first_child(&compiler)
        .unwrap()
        .get_next(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(inner_block.is_block(&compiler));
    let value_6 =
        scope_creator.create_scope(&mut compiler, inner_block, Some(function_block_scope));
    let _ = value_6;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        3
    );
}

// port: SyntacticScopeCreatorTest#testLetRedeclaration1
#[test]
fn test_let_redeclaration1() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let value_1 = get_scope(&mut compiler, &mut scope_creator, "let x; let x");
    let _ = value_1;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testLetRedeclaration2
#[test]
fn test_let_redeclaration2() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let value_1 = get_scope(&mut compiler, &mut scope_creator, "let x; let x; let x;");
    let _ = value_1;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        2
    );
}

// port: SyntacticScopeCreatorTest#testLetRedeclaration3
#[test]
fn test_let_redeclaration3() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "let x; if (true) { let x; } let x;";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_next(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(block.is_block(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, block, Some(global_scope));
    let _ = value_3;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testLetRedeclaration3_withES6Module
#[test]
fn test_let_redeclaration3_with_es6_module() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "export function f() { let x; if (true) { let x; } let x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let function = module_body.get_first_first_child(&compiler).unwrap();
    assert!(function.is_function(&compiler));
    let value_4 = scope_creator.create_scope(&mut compiler, function, Some(module_scope));
    let function_scope = value_4;
    let function_body = function.get_last_child(&compiler).unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, function_body, Some(function_scope));
    let function_block_scope = value_5;
    let inner_block = function_body
        .get_first_child(&compiler)
        .unwrap()
        .get_next(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_6 =
        scope_creator.create_scope(&mut compiler, inner_block, Some(function_block_scope));
    let _ = value_6;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testLetRedeclaration4
#[test]
fn test_let_redeclaration4() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "let x; if (true) { let x; let x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_next(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(block.is_block(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, block, Some(global_scope));
    let _ = value_3;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testLetRedeclaration5
#[test]
fn test_let_redeclaration5() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let js = "if (true) { let x; let x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(block.is_block(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, block, Some(global_scope));
    let _ = value_3;
    assert_eq!(
        redeclarations
            .lock()
            .unwrap()
            .get(&JsString::from("x"))
            .copied()
            .unwrap_or(0),
        1
    );
}

// port: SyntacticScopeCreatorTest#testArrayDestructuring
#[test]
fn test_array_destructuring() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_scope(&mut compiler, &mut scope_creator, "var [x, y] = foo();");
    let scope = value_1;
    let value_2 = declares(&mut compiler, scope, "x");
    assert_eq!(value_2, scope);
    let value_3 = declares(&mut compiler, scope, "y");
    assert_eq!(value_3, scope);
}

// port: SyntacticScopeCreatorTest#testNestedArrayDestructuring
#[test]
fn test_nested_array_destructuring() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_scope(&mut compiler, &mut scope_creator, "var [x, [y,z]] = foo();");
    let scope = value_1;
    let value_2 = declares(&mut compiler, scope, "x");
    assert_eq!(value_2, scope);
    let value_3 = declares(&mut compiler, scope, "y");
    assert_eq!(value_3, scope);
    let value_4 = declares(&mut compiler, scope, "z");
    assert_eq!(value_4, scope);
}

// port: SyntacticScopeCreatorTest#testArrayDestructuringWithName
#[test]
fn test_array_destructuring_with_name() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_scope(
        &mut compiler,
        &mut scope_creator,
        "var a = 1, [x, y] = foo();",
    );
    let scope = value_1;
    let value_2 = declares(&mut compiler, scope, "a");
    assert_eq!(value_2, scope);
    let value_3 = declares(&mut compiler, scope, "x");
    assert_eq!(value_3, scope);
    let value_4 = declares(&mut compiler, scope, "y");
    assert_eq!(value_4, scope);
}

// port: SyntacticScopeCreatorTest#testArrayDestructuringLet
#[test]
fn test_array_destructuring_let() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo() {\n  var [a, b] = getVars();\n  if (true) {\n    let [x, y] = getLets();\n  }\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
    let function_scope = value_3;
    let function_block = function_node.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, function_block, Some(function_scope));
    let function_block_scope = value_4;
    let value_5 = declares(&mut compiler, function_block_scope, "a");
    assert_eq!(value_5, function_block_scope);
    let value_6 = declares(&mut compiler, function_block_scope, "b");
    assert_eq!(value_6, function_block_scope);
    does_not_declare(&mut compiler, function_block_scope, "x");
    does_not_declare(&mut compiler, function_block_scope, "y");
    let var = function_block.get_first_child(&compiler).unwrap();
    let if_stmt = var.get_next(&compiler).unwrap();
    let if_block = if_stmt.get_last_child(&compiler).unwrap();
    let value_7 = scope_creator.create_scope(&mut compiler, if_block, Some(function_block_scope));
    let block_scope = value_7;
    let value_8 = declares(&mut compiler, block_scope, "a");
    assert_ne!(value_8, block_scope);
    let value_9 = declares(&mut compiler, block_scope, "b");
    assert_ne!(value_9, block_scope);
    let value_10 = declares(&mut compiler, block_scope, "x");
    assert_eq!(value_10, block_scope);
    let value_11 = declares(&mut compiler, block_scope, "y");
    assert_eq!(value_11, block_scope);
}

// port: SyntacticScopeCreatorTest#testArrayDestructuringVarInBlock
#[test]
fn test_array_destructuring_var_in_block() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo() {\n  var [a, b] = getVars();\n  if (true) {\n    var [x, y] = getMoreVars();\n  }\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
    let function_scope = value_3;
    let function_block = function_node.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, function_block, Some(function_scope));
    let function_block_scope = value_4;
    let value_5 = declares(&mut compiler, function_block_scope, "a");
    assert_eq!(value_5, function_block_scope);
    let value_6 = declares(&mut compiler, function_block_scope, "b");
    assert_eq!(value_6, function_block_scope);
    let value_7 = declares(&mut compiler, function_block_scope, "x");
    assert_eq!(value_7, function_block_scope);
    let value_8 = declares(&mut compiler, function_block_scope, "y");
    assert_eq!(value_8, function_block_scope);
}

// port: SyntacticScopeCreatorTest#testObjectDestructuring
#[test]
fn test_object_destructuring() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo() {\n  var {a, b} = bar();\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
    let function_scope = value_3;
    let function_block = function_node.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, function_block, Some(function_scope));
    let function_block_scope = value_4;
    let value_5 = declares(&mut compiler, function_block_scope, "a");
    assert_eq!(value_5, function_block_scope);
    let value_6 = declares(&mut compiler, function_block_scope, "b");
    assert_eq!(value_6, function_block_scope);
}

// port: SyntacticScopeCreatorTest#testObjectDestructuring2
#[test]
fn test_object_destructuring2() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo() {\n  var {a: b = 1} = bar();\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
    let function_scope = value_3;
    let function_block = function_node.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, function_block, Some(function_scope));
    let function_block_scope = value_4;
    does_not_declare(&mut compiler, function_block_scope, "a");
    let value_5 = declares(&mut compiler, function_block_scope, "b");
    assert_eq!(value_5, function_block_scope);
}

// port: SyntacticScopeCreatorTest#testObjectDestructuringComputedProp
#[test]
fn test_object_destructuring_computed_prop() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo() {\n  var {['s']: a} = bar();\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
    let function_scope = value_3;
    let function_block = function_node.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, function_block, Some(function_scope));
    let function_block_scope = value_4;
    let value_5 = declares(&mut compiler, function_block_scope, "a");
    assert_eq!(value_5, function_block_scope);
}

// port: SyntacticScopeCreatorTest#testObjectDestructuringComputedPropParam
#[test]
fn test_object_destructuring_computed_prop_param() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo({['s']: a}) {}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
    let function_scope = value_3;
    let value_4 = declares(&mut compiler, function_scope, "a");
    assert_eq!(value_4, function_scope);
}

// port: SyntacticScopeCreatorTest#testObjectDestructuringNested
#[test]
fn test_object_destructuring_nested() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo() {\n  var {a:{b}} = bar();\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
    let function_scope = value_3;
    let function_block = function_node.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, function_block, Some(function_scope));
    let function_block_scope = value_4;
    does_not_declare(&mut compiler, function_block_scope, "a");
    let value_5 = declares(&mut compiler, function_block_scope, "b");
    assert_eq!(value_5, function_block_scope);
}

// port: SyntacticScopeCreatorTest#testObjectDestructuringWithInitializer
#[test]
fn test_object_destructuring_with_initializer() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo() {\n  var {a=1} = bar();\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let function_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function_node, Some(global_scope));
    let function_scope = value_3;
    let function_block = function_node.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, function_block, Some(function_scope));
    let function_block_scope = value_4;
    let value_5 = declares(&mut compiler, function_block_scope, "a");
    assert_eq!(value_5, function_block_scope);
}

// port: SyntacticScopeCreatorTest#testObjectDestructuringInForOfParam
#[test]
fn test_object_destructuring_in_for_of_param() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "{for (let {length: x} of gen()) {}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let block = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, block, Some(global_scope));
    let block_scope = value_3;
    let for_of = block.get_first_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, for_of, Some(block_scope));
    let for_of_scope = value_4;
    let value_5 = declares(&mut compiler, for_of_scope, "x");
    assert_eq!(value_5, for_of_scope);
}

// port: SyntacticScopeCreatorTest#testFunctionScope
#[test]
fn test_function_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_scope(
        &mut compiler,
        &mut scope_creator,
        "function foo() {}\nvar x = function bar(a1) {};\n[function bar2() { var y; }];\nif (true) { function z() {} }\n",
    );
    let scope = value_1;
    let value_2 = declares(&mut compiler, scope, "foo");
    assert_eq!(value_2, scope);
    let value_3 = declares(&mut compiler, scope, "x");
    assert_eq!(value_3, scope);
    does_not_declare(&mut compiler, scope, "z");
    does_not_declare(&mut compiler, scope, "a1");
    does_not_declare(&mut compiler, scope, "bar");
    does_not_declare(&mut compiler, scope, "bar2");
    does_not_declare(&mut compiler, scope, "y");
    does_not_declare(&mut compiler, scope, "");
}

// port: SyntacticScopeCreatorTest#testClassScope
#[test]
fn test_class_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_scope(
        &mut compiler,
        &mut scope_creator,
        "class Foo {}\nvar x = class Bar {};\n[class Bar2 { constructor(a1) {} static y() {} }];\nif (true) { class Z {} }\n",
    );
    let scope = value_1;
    let value_2 = declares(&mut compiler, scope, "Foo");
    assert_eq!(value_2, scope);
    let value_3 = declares(&mut compiler, scope, "x");
    assert_eq!(value_3, scope);
    does_not_declare(&mut compiler, scope, "Z");
    does_not_declare(&mut compiler, scope, "a1");
    does_not_declare(&mut compiler, scope, "Bar");
    does_not_declare(&mut compiler, scope, "Bar2");
    does_not_declare(&mut compiler, scope, "y");
    does_not_declare(&mut compiler, scope, "");
}

// port: SyntacticScopeCreatorTest#testScopeRootNode
#[test]
fn test_scope_root_node() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function foo() {\n var x = 10;\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    assert_eq!(global_scope.get_root_node(&compiler), root);
    assert!(!global_scope.is_block_scope(&compiler));
    assert_eq!(
        global_scope.get_closest_hoist_scope(&compiler).unwrap(),
        global_scope
    );
    assert!(global_scope.is_hoist_scope(&compiler));
    let function = root.get_first_child(&compiler).unwrap();
    assert!(function.is_function(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global_scope));
    let function_scope = value_3;
    let foo_block_node = NodeUtil::get_function_body(&compiler, function);
    let value_4 = scope_creator.create_scope(&mut compiler, foo_block_node, Some(function_scope));
    let foo_scope = value_4;
    assert_eq!(foo_scope.get_root_node(&compiler), foo_block_node);
    assert!(foo_scope.is_block_scope(&compiler));
    assert_eq!(
        foo_scope.get_closest_hoist_scope(&compiler).unwrap(),
        foo_scope
    );
    assert!(foo_scope.is_hoist_scope(&compiler));
    let value_5 = declares(&mut compiler, foo_scope, "x");
    assert_eq!(value_5, foo_scope);
}

// port: SyntacticScopeCreatorTest#testBlockScopeWithVar
#[test]
fn test_block_scope_with_var() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "if (true) { if (true) { var x; } }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "x");
    assert_eq!(value_3, global_scope);
    let first_level_block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, first_level_block, Some(global_scope));
    let first_level_block_scope = value_4;
    let value_5 = declares(&mut compiler, first_level_block_scope, "x");
    assert_ne!(value_5, first_level_block_scope);
    let second_level_block = first_level_block
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_6 = scope_creator.create_scope(
        &mut compiler,
        second_level_block,
        Some(first_level_block_scope),
    );
    let second_level_b_lock_scope = value_6;
    let value_7 = declares(&mut compiler, second_level_b_lock_scope, "x");
    assert_ne!(value_7, second_level_b_lock_scope);
}

// port: SyntacticScopeCreatorTest#testBlockScopeWithLet
#[test]
fn test_block_scope_with_let() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "if (true) { if (true) { let x; } }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "x");
    let first_level_block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, first_level_block, Some(global_scope));
    let first_level_block_scope = value_3;
    does_not_declare(&mut compiler, first_level_block_scope, "x");
    let second_level_block = first_level_block
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_4 = scope_creator.create_scope(
        &mut compiler,
        second_level_block,
        Some(first_level_block_scope),
    );
    let second_level_b_lock_scope = value_4;
    let value_5 = declares(&mut compiler, second_level_b_lock_scope, "x");
    assert_eq!(value_5, second_level_b_lock_scope);
}

// port: SyntacticScopeCreatorTest#testBlockScopeWithClass
#[test]
fn test_block_scope_with_class() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "if (true) { if (true) { class X {} } }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "X");
    let first_level_block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, first_level_block, Some(global_scope));
    let first_level_block_scope = value_3;
    does_not_declare(&mut compiler, first_level_block_scope, "X");
    let second_level_block = first_level_block
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_4 = scope_creator.create_scope(
        &mut compiler,
        second_level_block,
        Some(first_level_block_scope),
    );
    let second_level_b_lock_scope = value_4;
    let value_5 = declares(&mut compiler, second_level_b_lock_scope, "X");
    assert_eq!(value_5, second_level_b_lock_scope);
}

// port: SyntacticScopeCreatorTest#testClassFieldsThisAndSuper
#[test]
fn test_class_fields_this_and_super() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class Foo {\n  a = this.a;\n  [this.a] = this.a;\n}\nclass Bar extends Foo {\n  b = super.a;\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let class_foo = root.get_first_child(&compiler).unwrap();
    let class_bar = root.get_last_child(&compiler).unwrap();
    let member_field_def_a = class_foo
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let member_field_def_b = class_bar
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let computed_field_def = class_foo
        .get_last_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = scope_creator.create_scope(&mut compiler, class_foo, Some(global_scope));
    let foo_scope = value_3;
    let value_4 = scope_creator.create_scope(&mut compiler, class_bar, Some(global_scope));
    let bar_scope = value_4;
    let value_5 = scope_creator.create_scope(&mut compiler, member_field_def_a, Some(foo_scope));
    let member_field_def_a_scope = value_5;
    let value_6 = scope_creator.create_scope(&mut compiler, member_field_def_b, Some(bar_scope));
    let member_field_def_b_scope = value_6;
    let value_7 = scope_creator.create_scope(&mut compiler, computed_field_def, Some(foo_scope));
    let computed_field_def_rhs_scope = value_7;
    let value_8 = declares(&mut compiler, global_scope, "Foo");
    assert_eq!(value_8, global_scope);
    let value_9 = declares(&mut compiler, global_scope, "Bar");
    assert_eq!(value_9, global_scope);
    does_not_declare(&mut compiler, global_scope, "this");
    does_not_declare(&mut compiler, foo_scope, "this");
    does_not_declare(&mut compiler, bar_scope, "this");
    let value_10 = declares(&mut compiler, member_field_def_a_scope, "this");
    assert_eq!(value_10, member_field_def_a_scope);
    let value_11 = declares(&mut compiler, computed_field_def_rhs_scope, "this");
    assert_eq!(value_11, computed_field_def_rhs_scope);
    does_not_declare(&mut compiler, global_scope, "super");
    does_not_declare(&mut compiler, foo_scope, "super");
    does_not_declare(&mut compiler, bar_scope, "super");
    let value_12 = declares(&mut compiler, member_field_def_b_scope, "super");
    assert_eq!(value_12, member_field_def_b_scope);
}

// port: SyntacticScopeCreatorTest#testClassStaticFieldsThisAndSuper
#[test]
fn test_class_static_fields_this_and_super() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class Foo {\n  static a = 2;\n  static b = this.b;\n  static [this.a] = this.a;\n}\nclass Bar extends Foo {\n  static c = super.a + 1;\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let class_foo = root.get_first_child(&compiler).unwrap();
    let class_bar = root.get_last_child(&compiler).unwrap();
    let member_field_def_a = class_foo
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let member_field_def_b = class_bar
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let computed_field_def = class_foo
        .get_last_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = scope_creator.create_scope(&mut compiler, class_foo, Some(global_scope));
    let foo_scope = value_3;
    let value_4 = scope_creator.create_scope(&mut compiler, class_bar, Some(global_scope));
    let bar_scope = value_4;
    let value_5 = scope_creator.create_scope(&mut compiler, member_field_def_a, Some(foo_scope));
    let member_field_def_a_scope = value_5;
    let value_6 = scope_creator.create_scope(&mut compiler, member_field_def_b, Some(bar_scope));
    let member_field_def_b_scope = value_6;
    let value_7 = scope_creator.create_scope(&mut compiler, computed_field_def, Some(foo_scope));
    let computed_field_def_rhs_scope = value_7;
    let value_8 = declares(&mut compiler, global_scope, "Foo");
    assert_eq!(value_8, global_scope);
    let value_9 = declares(&mut compiler, global_scope, "Bar");
    assert_eq!(value_9, global_scope);
    does_not_declare(&mut compiler, global_scope, "this");
    does_not_declare(&mut compiler, foo_scope, "this");
    does_not_declare(&mut compiler, bar_scope, "this");
    let value_10 = declares(&mut compiler, member_field_def_a_scope, "this");
    assert_eq!(value_10, member_field_def_a_scope);
    let value_11 = declares(&mut compiler, computed_field_def_rhs_scope, "this");
    assert_eq!(value_11, computed_field_def_rhs_scope);
    does_not_declare(&mut compiler, global_scope, "super");
    does_not_declare(&mut compiler, foo_scope, "super");
    does_not_declare(&mut compiler, bar_scope, "super");
    let value_12 = declares(&mut compiler, member_field_def_b_scope, "super");
    assert_eq!(value_12, member_field_def_b_scope);
}

// port: SyntacticScopeCreatorTest#testStaticBlockScope
#[test]
fn test_static_block_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class C {static {}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    let value_4 = declares(&mut compiler, global_scope, "C");
    assert_eq!(value_4, global_scope);
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_5 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_5;
    let value_6 = declares(&mut compiler, static_block_scope, "C");
    assert_ne!(value_6, static_block_scope);
    assert!(static_block_scope.is_block_scope(&compiler));
    assert_eq!(
        static_block_scope
            .get_closest_hoist_scope(&compiler)
            .unwrap(),
        static_block_scope
    );
    assert!(static_block_scope.is_hoist_scope(&compiler));
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeClassExpr
#[test]
fn test_static_block_scope_class_expr() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "let c = class C {static {}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    let value_4 = declares(&mut compiler, global_scope, "c");
    assert_eq!(value_4, global_scope);
    does_not_declare(&mut compiler, global_scope, "C");
    let value_5 = declares(&mut compiler, class_scope, "c");
    assert_ne!(value_5, class_scope);
    let value_6 = declares(&mut compiler, class_scope, "C");
    assert_eq!(value_6, class_scope);
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_7 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_7;
    let value_8 = declares(&mut compiler, static_block_scope, "C");
    assert_ne!(value_8, static_block_scope);
    let value_9 = declares(&mut compiler, static_block_scope, "c");
    assert_ne!(value_9, static_block_scope);
    assert!(static_block_scope.is_block_scope(&compiler));
    assert_eq!(
        static_block_scope
            .get_closest_hoist_scope(&compiler)
            .unwrap(),
        static_block_scope
    );
    assert!(static_block_scope.is_hoist_scope(&compiler));
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeWithLet
#[test]
fn test_static_block_scope_with_let() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class C {static {let x;}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    let value_5 = declares(&mut compiler, static_block_scope, "x");
    assert_eq!(value_5, static_block_scope);
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeClassExprWithLet
#[test]
fn test_static_block_scope_class_expr_with_let() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "let _c = class C {static {let x;}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    let value_5 = declares(&mut compiler, static_block_scope, "x");
    assert_eq!(value_5, static_block_scope);
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeWithVar
#[test]
fn test_static_block_scope_with_var() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class C {static {var x;}} var y; ";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    let value_5 = declares(&mut compiler, static_block_scope, "x");
    assert_eq!(value_5, static_block_scope);
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeClassExprWithVar
#[test]
fn test_static_block_scope_class_expr_with_var() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "let _c = class C {static {var x;}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    let value_5 = declares(&mut compiler, static_block_scope, "x");
    assert_eq!(value_5, static_block_scope);
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeWithLetInnerBlock
#[test]
fn test_static_block_scope_with_let_inner_block() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class C {static {if (true) {let x;}}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    does_not_declare(&mut compiler, static_block_scope, "x");
    let if_body_node = class_static_block_node
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, if_body_node, Some(static_block_scope));
    let if_body_scope = value_5;
    let value_6 = declares(&mut compiler, if_body_scope, "x");
    assert_eq!(value_6, if_body_scope);
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeClassExprWithLetInnerBlock
#[test]
fn test_static_block_scope_class_expr_with_let_inner_block() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "let _c = class C {static {if (true) {let x;}}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    does_not_declare(&mut compiler, static_block_scope, "x");
    let if_body_node = class_static_block_node
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, if_body_node, Some(static_block_scope));
    let if_body_scope = value_5;
    let value_6 = declares(&mut compiler, if_body_scope, "x");
    assert_eq!(value_6, if_body_scope);
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeWithVarInnerBlock
#[test]
fn test_static_block_scope_with_var_inner_block() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class C {static {if (true) {var x;}}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    let value_5 = declares(&mut compiler, static_block_scope, "x");
    assert_eq!(value_5, static_block_scope);
    let if_body_node = class_static_block_node
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_6 = scope_creator.create_scope(&mut compiler, if_body_node, Some(static_block_scope));
    let if_body_scope = value_6;
    let value_7 = declares(&mut compiler, if_body_scope, "x");
    assert_eq!(
        value_7,
        if_body_scope.get_closest_container_scope(&compiler)
    );
}

// port: SyntacticScopeCreatorTest#testStaticBlockScopeClassExprWithVarInnerBlock
#[test]
fn test_static_block_scope_class_expr_with_var_inner_block() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "let _c = class C {static {if (true) {var x;}}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    let value_5 = declares(&mut compiler, static_block_scope, "x");
    assert_eq!(value_5, static_block_scope);
    let if_body_node = class_static_block_node
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_6 = scope_creator.create_scope(&mut compiler, if_body_node, Some(static_block_scope));
    let if_body_scope = value_6;
    let value_7 = declares(&mut compiler, if_body_scope, "x");
    assert_eq!(
        value_7,
        if_body_scope.get_closest_container_scope(&compiler)
    );
}

// port: SyntacticScopeCreatorTest#testClassStaticBlockWithLoop
#[test]
fn test_class_static_block_with_loop() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "let _f = class Foo { static { for (;;) { var x; }}}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_decl = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_decl, Some(global_scope));
    let class_scope = value_3;
    let class_static_block_node = class_decl
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    let for_body_node = class_static_block_node
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_5 =
        scope_creator.create_scope(&mut compiler, for_body_node, Some(static_block_scope));
    let for_body_scope = value_5;
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, class_scope, "x");
    let value_6 = declares(&mut compiler, for_body_scope, "x");
    assert_eq!(
        value_6,
        for_body_scope.get_closest_container_scope(&compiler)
    );
}

// port: SyntacticScopeCreatorTest#testStaticBlockThis
#[test]
fn test_static_block_this() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class Foo {\n  static {\n    this.x;\n  }\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_foo = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_foo, Some(global_scope));
    let class_scope = value_3;
    let class_static_block_node = class_foo
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_4 =
        scope_creator.create_scope(&mut compiler, class_static_block_node, Some(class_scope));
    let static_block_scope = value_4;
    assert!(static_block_scope.is_block_scope(&compiler));
    assert_eq!(
        static_block_scope
            .get_closest_hoist_scope(&compiler)
            .unwrap(),
        static_block_scope
    );
    assert!(static_block_scope.is_hoist_scope(&compiler));
    let value_5 = declares(&mut compiler, global_scope, "Foo");
    assert_eq!(value_5, global_scope);
    let value_6 = declares(&mut compiler, static_block_scope, "Foo");
    assert_ne!(value_6, static_block_scope);
    does_not_declare(&mut compiler, global_scope, "this");
    does_not_declare(&mut compiler, class_scope, "this");
    let value_7 = declares(&mut compiler, static_block_scope, "this");
    assert_eq!(value_7, static_block_scope);
}

// port: SyntacticScopeCreatorTest#testClassStaticBlockSuper
#[test]
fn test_class_static_block_super() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "class Foo {\n  static x;\n}\nclass Bar extends Foo {\n  static {\n    super.x = 'str';\n  }\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let class_foo = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, class_foo, Some(global_scope));
    let class_foo_scope = value_3;
    let class_bar = root.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, class_bar, Some(global_scope));
    let class_bar_scope = value_4;
    let class_static_block_node = class_bar
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    let value_5 = scope_creator.create_scope(
        &mut compiler,
        class_static_block_node,
        Some(class_bar_scope),
    );
    let static_block_scope = value_5;
    assert!(static_block_scope.is_block_scope(&compiler));
    assert_eq!(
        static_block_scope
            .get_closest_hoist_scope(&compiler)
            .unwrap(),
        static_block_scope
    );
    assert!(static_block_scope.is_hoist_scope(&compiler));
    let value_6 = declares(&mut compiler, global_scope, "Foo");
    assert_eq!(value_6, global_scope);
    let value_7 = declares(&mut compiler, static_block_scope, "Foo");
    assert_ne!(value_7, static_block_scope);
    let value_8 = declares(&mut compiler, global_scope, "Bar");
    assert_eq!(value_8, global_scope);
    let value_9 = declares(&mut compiler, static_block_scope, "Bar");
    assert_ne!(value_9, static_block_scope);
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, global_scope, "super");
    does_not_declare(&mut compiler, class_foo_scope, "x");
    does_not_declare(&mut compiler, class_foo_scope, "super");
    does_not_declare(&mut compiler, class_bar_scope, "x");
    does_not_declare(&mut compiler, class_bar_scope, "super");
    does_not_declare(&mut compiler, static_block_scope, "x");
    let value_10 = declares(&mut compiler, static_block_scope, "super");
    assert_eq!(value_10, static_block_scope);
}

// port: SyntacticScopeCreatorTest#testSwitchScope
#[test]
fn test_switch_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "switch (b) {\n  case 1:\n    b;\n  case 2:\n    let c = 4;\n    c;\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "c");
    let switch_node = root.get_first_child(&compiler).unwrap();
    let value_3 = switch_node.get_last_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, value_3, Some(global_scope));
    let switch_scope = value_4;
    let value_5 = declares(&mut compiler, switch_scope, "c");
    assert_eq!(value_5, switch_scope);
}

// port: SyntacticScopeCreatorTest#testForLoopScope
#[test]
fn test_for_loop_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "for (let i = 0;;) { let x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "i");
    does_not_declare(&mut compiler, global_scope, "x");
    let for_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, for_node, Some(global_scope));
    let for_scope = value_3;
    let value_4 = declares(&mut compiler, for_scope, "i");
    assert_eq!(value_4, for_scope);
    does_not_declare(&mut compiler, for_scope, "x");
    let for_block = for_node.get_last_child(&compiler).unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, for_block, Some(for_scope));
    let for_block_scope = value_5;
    let value_6 = declares(&mut compiler, for_block_scope, "i");
    assert_ne!(value_6, for_block_scope);
    let value_7 = declares(&mut compiler, for_block_scope, "x");
    assert_eq!(value_7, for_block_scope);
}

// port: SyntacticScopeCreatorTest#testForOfLoopScope
#[test]
fn test_for_of_loop_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "for (let i of arr) { let x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "i");
    does_not_declare(&mut compiler, global_scope, "x");
    let for_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, for_node, Some(global_scope));
    let for_scope = value_3;
    let value_4 = declares(&mut compiler, for_scope, "i");
    assert_eq!(value_4, for_scope);
    does_not_declare(&mut compiler, for_scope, "x");
    let for_block = for_node.get_last_child(&compiler).unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, for_block, Some(for_scope));
    let for_block_scope = value_5;
    let value_6 = declares(&mut compiler, for_block_scope, "i");
    assert_ne!(value_6, for_block_scope);
    let value_7 = declares(&mut compiler, for_block_scope, "x");
    assert_eq!(value_7, for_block_scope);
}

// port: SyntacticScopeCreatorTest#testFunctionArgument
#[test]
fn test_function_argument() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f(x) { if (true) { let y = 3; } }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global = value_2;
    let function = root.get_last_child(&compiler).unwrap();
    assert!(function.is_function(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global));
    let function_scope = value_3;
    let function_block = NodeUtil::get_function_body(&compiler, function);
    let value_4 = scope_creator.create_scope(&mut compiler, function_block, Some(function_scope));
    let f_block_scope = value_4;
    let value_5 = declares(&mut compiler, f_block_scope, "x");
    assert_ne!(function_scope, f_block_scope);
    assert_eq!(value_5, function_scope);
    does_not_declare(&mut compiler, f_block_scope, "y");
    let if_block = function_block
        .get_last_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    assert!(if_block.is_block(&compiler));
    let value_6 = scope_creator.create_scope(&mut compiler, if_block, Some(f_block_scope));
    let block_scope = value_6;
    let value_7 = declares(&mut compiler, block_scope, "x");
    assert_ne!(function_scope, block_scope);
    assert_eq!(value_7, function_scope);
    let value_8 = declares(&mut compiler, block_scope, "y");
    assert_eq!(value_8, block_scope);
}

// port: SyntacticScopeCreatorTest#testTheArgumentsVariable
#[test]
fn test_the_arguments_variable() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f() { if (true) { let arguments = 3; } }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global = value_2;
    let function = root.get_first_child(&compiler).unwrap();
    assert!(function.is_function(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global));
    let f_scope = value_3;
    assert!(f_scope.has_own_slot(&compiler, JsString::from("arguments")));
    assert!(f_scope.has_slot(&compiler, JsString::from("arguments")));
    let value_4 = f_scope.get_arguments_var(&mut compiler).unwrap();
    let arguments = value_4;
    let value_5 = f_scope
        .get_var(&mut compiler, JsString::from("arguments"))
        .unwrap();
    assert_eq!(value_5, arguments);
    let f_block = NodeUtil::get_function_body(&compiler, function);
    let value_6 = scope_creator.create_scope(&mut compiler, f_block, Some(f_scope));
    let f_block_scope = value_6;
    let value_7 = f_block_scope
        .get_var(&mut compiler, JsString::from("arguments"))
        .unwrap();
    assert_eq!(value_7, arguments);
    let value_8 = f_block_scope.get_arguments_var(&mut compiler).unwrap();
    assert_eq!(value_8, arguments);
    let if_block = f_block
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_9 = scope_creator.create_scope(&mut compiler, if_block, Some(f_block_scope));
    let block_scope = value_9;
    let value_10 = declares(&mut compiler, block_scope, "arguments");
    assert_eq!(value_10, block_scope);
    let value_11 = block_scope.get_arguments_var(&mut compiler).unwrap();
    assert_eq!(value_11, arguments);
    let value_12 = block_scope
        .get_var(&mut compiler, JsString::from("arguments"))
        .unwrap();
    assert_ne!(value_12, arguments);
}

// port: SyntacticScopeCreatorTest#testArgumentsVariableInArrowFunction
#[test]
fn test_arguments_variable_in_arrow_function() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function outer() { var inner = () => { alert(0); } }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global = value_2;
    let outer = root.get_first_child(&compiler).unwrap();
    assert!(outer.is_function(&compiler));
    assert!(!outer.is_arrow_function(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, outer, Some(global));
    let outer_function_scope = value_3;
    let value_4 = outer_function_scope
        .get_arguments_var(&mut compiler)
        .unwrap();
    let arguments = value_4;
    let outer_body = NodeUtil::get_function_body(&compiler, outer);
    let value_5 = scope_creator.create_scope(&mut compiler, outer_body, Some(outer_function_scope));
    let outer_body_scope = value_5;
    let inner = outer_body
        .get_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert!(inner.is_function(&compiler));
    assert!(inner.is_arrow_function(&compiler));
    let value_6 = scope_creator.create_scope(&mut compiler, inner, Some(outer_body_scope));
    let inner_function_scope = value_6;
    let value_7 = inner_function_scope
        .get_arguments_var(&mut compiler)
        .unwrap();
    assert_eq!(value_7, arguments);
}

// port: SyntacticScopeCreatorTest#testTheThisVariable
#[test]
fn test_the_this_variable() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f() { if (true) { function g() {} } }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global = value_2;
    let function = root.get_first_child(&compiler).unwrap();
    assert!(function.is_function(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global));
    let f_scope = value_3;
    assert!(f_scope.has_own_slot(&compiler, JsString::from("this")));
    assert!(f_scope.has_slot(&compiler, JsString::from("this")));
    let value_4 = f_scope
        .get_var(&mut compiler, JsString::from("this"))
        .unwrap();
    let this_var = value_4;
    assert!(this_var.is_this(&compiler));
    let f_block = NodeUtil::get_function_body(&compiler, function);
    let value_5 = scope_creator.create_scope(&mut compiler, f_block, Some(f_scope));
    let f_block_scope = value_5;
    let value_6 = declares(&mut compiler, f_block_scope, "this");
    let _ = value_6;
    let value_7 = f_block_scope
        .get_var(&mut compiler, JsString::from("this"))
        .unwrap();
    assert_eq!(value_7, this_var);
    let if_block = f_block
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_8 = scope_creator.create_scope(&mut compiler, if_block, Some(f_block_scope));
    let block_scope = value_8;
    let value_9 = declares(&mut compiler, block_scope, "this");
    let _ = value_9;
    let value_10 = block_scope
        .get_var(&mut compiler, JsString::from("this"))
        .unwrap();
    assert_eq!(value_10, this_var);
    let value_11 = block_scope
        .get_var(&mut compiler, JsString::from("this"))
        .unwrap();
    assert_eq!(value_11.get_scope(&compiler), f_scope);
    let g_function = if_block.get_first_child(&compiler).unwrap();
    let value_12 = scope_creator.create_scope(&mut compiler, g_function, Some(block_scope));
    let g_scope = value_12;
    let value_13 = declares(&mut compiler, g_scope, "this");
    assert_eq!(value_13, g_scope);
    let value_14 = g_scope
        .get_var(&mut compiler, JsString::from("this"))
        .unwrap();
    assert_eq!(value_14.get_scope(&compiler), g_scope);
}

// port: SyntacticScopeCreatorTest#testTheSuperVariable
#[test]
fn test_the_super_variable() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f() { if (true) { function g() {} } }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global = value_2;
    let function = root.get_first_child(&compiler).unwrap();
    assert!(function.is_function(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global));
    let f_scope = value_3;
    assert!(f_scope.has_own_slot(&compiler, JsString::from("super")));
    assert!(f_scope.has_slot(&compiler, JsString::from("super")));
    let value_4 = f_scope
        .get_var(&mut compiler, JsString::from("super"))
        .unwrap();
    let this_var = value_4;
    let f_block = NodeUtil::get_function_body(&compiler, function);
    let value_5 = scope_creator.create_scope(&mut compiler, f_block, Some(f_scope));
    let f_block_scope = value_5;
    let value_6 = declares(&mut compiler, f_block_scope, "super");
    let _ = value_6;
    let value_7 = f_block_scope
        .get_var(&mut compiler, JsString::from("super"))
        .unwrap();
    assert_eq!(value_7, this_var);
    let if_block = f_block
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_8 = scope_creator.create_scope(&mut compiler, if_block, Some(f_block_scope));
    let block_scope = value_8;
    let value_9 = declares(&mut compiler, block_scope, "super");
    let _ = value_9;
    let value_10 = block_scope
        .get_var(&mut compiler, JsString::from("super"))
        .unwrap();
    assert_eq!(value_10, this_var);
    let value_11 = block_scope
        .get_var(&mut compiler, JsString::from("super"))
        .unwrap();
    assert_eq!(value_11.get_scope(&compiler), f_scope);
    let g_function = if_block.get_first_child(&compiler).unwrap();
    let value_12 = scope_creator.create_scope(&mut compiler, g_function, Some(block_scope));
    let g_scope = value_12;
    let value_13 = declares(&mut compiler, g_scope, "super");
    assert_eq!(value_13, g_scope);
    let value_14 = g_scope
        .get_var(&mut compiler, JsString::from("super"))
        .unwrap();
    assert_eq!(value_14.get_scope(&compiler), g_scope);
    let value_15 = g_scope
        .get_var(&mut compiler, JsString::from("super"))
        .unwrap();
    let value_16 = g_scope
        .get_var(&mut compiler, JsString::from("this"))
        .unwrap();
    assert_ne!(value_15, value_16);
}

// port: SyntacticScopeCreatorTest#testTheThisVariableInArrowFunction
#[test]
fn test_the_this_variable_in_arrow_function() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function outer() { var inner = () => this.x; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global = value_2;
    let outer = root.get_first_child(&compiler).unwrap();
    assert!(outer.is_function(&compiler));
    assert!(!outer.is_arrow_function(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, outer, Some(global));
    let outer_function_scope = value_3;
    let value_4 = outer_function_scope
        .get_var(&mut compiler, JsString::from("this"))
        .unwrap();
    let this_var = value_4;
    let outer_body = NodeUtil::get_function_body(&compiler, outer);
    let value_5 = scope_creator.create_scope(&mut compiler, outer_body, Some(outer_function_scope));
    let outer_body_scope = value_5;
    let inner = outer_body
        .get_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert!(inner.is_function(&compiler));
    assert!(inner.is_arrow_function(&compiler));
    let value_6 = scope_creator.create_scope(&mut compiler, inner, Some(outer_body_scope));
    let inner_function_scope = value_6;
    let value_7 = inner_function_scope
        .get_var(&mut compiler, JsString::from("this"))
        .unwrap();
    assert_eq!(value_7, this_var);
}

// port: SyntacticScopeCreatorTest#testIsFunctionBlockScoped
#[test]
fn test_is_function_block_scoped() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "if (true) { function f() {}; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "f");
    let if_block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, if_block, Some(global_scope));
    let block_scope = value_3;
    let value_4 = declares(&mut compiler, block_scope, "f");
    assert_eq!(value_4, block_scope);
}

// port: SyntacticScopeCreatorTest#testIsClassBlockScoped
#[test]
fn test_is_class_block_scoped() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "if (true) { class X {}; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "X");
    let if_block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, if_block, Some(global_scope));
    let block_scope = value_3;
    let value_4 = declares(&mut compiler, block_scope, "X");
    assert_eq!(value_4, block_scope);
}

// port: SyntacticScopeCreatorTest#testIsCatchBlockScoped
#[test]
fn test_is_catch_block_scoped() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "try { var x = 2; } catch (e) { var y = 3; let z = 4; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "x");
    assert_eq!(value_3, global_scope);
    let value_4 = declares(&mut compiler, global_scope, "y");
    assert_eq!(value_4, global_scope);
    does_not_declare(&mut compiler, global_scope, "z");
    does_not_declare(&mut compiler, global_scope, "e");
    let try_block = root.get_first_first_child(&compiler).unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, try_block, Some(global_scope));
    let try_block_scope = value_5;
    let value_6 = declares(&mut compiler, try_block_scope, "x");
    assert_ne!(value_6, try_block_scope);
    let value_7 = declares(&mut compiler, try_block_scope, "y");
    assert_ne!(value_7, try_block_scope);
    does_not_declare(&mut compiler, try_block_scope, "z");
    does_not_declare(&mut compiler, try_block_scope, "e");
    let catch_block = try_block.get_next(&compiler).unwrap();
    let value_8 = scope_creator.create_scope(&mut compiler, catch_block, Some(try_block_scope));
    let catch_block_scope = value_8;
    let value_9 = declares(&mut compiler, catch_block_scope, "x");
    assert_ne!(value_9, catch_block_scope);
    let value_10 = declares(&mut compiler, catch_block_scope, "y");
    assert_ne!(value_10, catch_block_scope);
    let value_11 = declares(&mut compiler, catch_block_scope, "z");
    assert_eq!(value_11, catch_block_scope);
    let value_12 = declares(&mut compiler, catch_block_scope, "e");
    assert_eq!(value_12, catch_block_scope);
}

// port: SyntacticScopeCreatorTest#testImport
#[test]
fn test_import() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "import * as ns from 'm1';\nimport d from 'm2';\nimport {foo} from 'm3';\nimport {x as y} from 'm4';\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    assert!(global_scope.get_var_iterable(&compiler).is_empty());
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let value_4 = declares(&mut compiler, module_scope, "ns");
    assert_eq!(value_4, module_scope);
    let value_5 = declares(&mut compiler, module_scope, "d");
    assert_eq!(value_5, module_scope);
    let value_6 = declares(&mut compiler, module_scope, "foo");
    assert_eq!(value_6, module_scope);
    let value_7 = declares(&mut compiler, module_scope, "y");
    assert_eq!(value_7, module_scope);
    does_not_declare(&mut compiler, module_scope, "x");
}

// port: SyntacticScopeCreatorTest#testImportAsSelf
#[test]
fn test_import_as_self() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "import {x as x} from 'm';";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    assert!(global_scope.get_var_iterable(&compiler).is_empty());
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let value_4 = declares(&mut compiler, module_scope, "x");
    assert_eq!(value_4, module_scope);
}

// port: SyntacticScopeCreatorTest#testImportDefault
#[test]
fn test_import_default() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "import x from 'm';";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    assert!(global_scope.get_var_iterable(&compiler).is_empty());
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let value_4 = declares(&mut compiler, module_scope, "x");
    assert_eq!(value_4, module_scope);
}

// port: SyntacticScopeCreatorTest#testModuleScoped
#[test]
fn test_module_scoped() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "export function f() { var x; if (1) { let y; } }; var z;";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "f");
    does_not_declare(&mut compiler, global_scope, "x");
    does_not_declare(&mut compiler, global_scope, "y");
    does_not_declare(&mut compiler, global_scope, "z");
    let module_block = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, module_block, Some(global_scope));
    let module_block_scope = value_3;
    let value_4 = declares(&mut compiler, module_block_scope, "f");
    assert_eq!(value_4, module_block_scope);
    does_not_declare(&mut compiler, module_block_scope, "x");
    does_not_declare(&mut compiler, module_block_scope, "y");
    let value_5 = declares(&mut compiler, module_block_scope, "z");
    assert_eq!(value_5, module_block_scope);
}

// port: SyntacticScopeCreatorTest#testExportDefault
#[test]
fn test_export_default() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "export default function f() {};";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "f");
    let module_block = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, module_block, Some(global_scope));
    let module_block_scope = value_3;
    let value_4 = declares(&mut compiler, module_block_scope, "f");
    assert_eq!(value_4, module_block_scope);
}

// port: SyntacticScopeCreatorTest#testExportFrom
#[test]
fn test_export_from() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "export {PI} from './n.js';";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "PI");
    let module_block = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, module_block, Some(global_scope));
    let module_block_scope = value_3;
    does_not_declare(&mut compiler, module_block_scope, "PI");
}

// port: SyntacticScopeCreatorTest#testVarAfterLet
#[test]
fn test_var_after_let() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f() {\n  if (a) {\n    let x;\n  }\n  var y;\n}\n";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global = value_2;
    let function = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, function, Some(global));
    let f_scope = value_3;
    let f_block = root
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, f_block, Some(f_scope));
    let f_block_scope = value_4;
    let _ = f_block_scope;
    does_not_declare(&mut compiler, f_block_scope, "x");
    let value_5 = declares(&mut compiler, f_block_scope, "y");
    assert_eq!(value_5, f_block_scope);
    let if_block = f_block
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_6 = scope_creator.create_scope(&mut compiler, if_block, Some(f_block_scope));
    let if_block_scope = value_6;
    let value_7 = declares(&mut compiler, if_block_scope, "x");
    assert_eq!(value_7, if_block_scope);
    let value_8 = declares(&mut compiler, if_block_scope, "y");
    assert_ne!(value_8, if_block_scope);
}

// port: SyntacticScopeCreatorTest#testSimpleFunctionParam
#[test]
fn test_simple_function_param() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f(x) {}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let f_node = root.get_first_child(&compiler).unwrap();
    assert!(f_node.is_function(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, f_node, Some(global_scope));
    let f_scope = value_3;
    let value_4 = declares(&mut compiler, f_scope, "x");
    assert_eq!(value_4, f_scope);
    let f_block = NodeUtil::get_function_body(&compiler, f_node);
    let value_5 = scope_creator.create_scope(&mut compiler, f_block, Some(f_scope));
    let f_block_scope = value_5;
    let value_6 = declares(&mut compiler, f_block_scope, "x");
    assert_ne!(f_scope, f_block_scope);
    assert_eq!(value_6, f_scope);
}

// port: SyntacticScopeCreatorTest#testOnlyOneDeclaration
#[test]
fn test_only_one_declaration() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f(x) { if (!x) var x = 6; }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let f_node = root.get_first_child(&compiler).unwrap();
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = scope_creator.create_scope(&mut compiler, f_node, Some(global_scope));
    let f_scope = value_3;
    let value_4 = declares(&mut compiler, f_scope, "x");
    assert_eq!(value_4, f_scope);
    let f_block = f_node.get_last_child(&compiler).unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, f_block, Some(f_scope));
    let f_block_scope = value_5;
    let value_6 = declares(&mut compiler, f_block_scope, "x");
    assert_ne!(f_scope, f_block_scope);
    assert_eq!(value_6, f_scope);
    let if_block = f_block
        .get_first_child(&compiler)
        .unwrap()
        .get_last_child(&compiler)
        .unwrap();
    let value_7 = scope_creator.create_scope(&mut compiler, if_block, Some(f_block_scope));
    let if_block_scope = value_7;
    let value_8 = declares(&mut compiler, if_block_scope, "x");
    assert_ne!(f_scope, if_block_scope);
    assert_eq!(value_8, f_scope);
}

// port: SyntacticScopeCreatorTest#testCatchInFunction
#[test]
fn test_catch_in_function() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "function f(e) { try {} catch (e) {} }";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let f_node = root.get_first_child(&compiler).unwrap();
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = scope_creator.create_scope(&mut compiler, f_node, Some(global_scope));
    let f_scope = value_3;
    let value_4 = declares(&mut compiler, f_scope, "e");
    assert_eq!(value_4, f_scope);
    let f_block = f_node.get_last_child(&compiler).unwrap();
    let value_5 = scope_creator.create_scope(&mut compiler, f_block, Some(f_scope));
    let f_block_scope = value_5;
    let try_block = f_block.get_first_first_child(&compiler).unwrap();
    let value_6 = scope_creator.create_scope(&mut compiler, try_block, Some(f_block_scope));
    let try_scope = value_6;
    let catch_block = try_block.get_next(&compiler).unwrap();
    let value_7 = scope_creator.create_scope(&mut compiler, catch_block, Some(try_scope));
    let catch_scope = value_7;
    let value_8 = declares(&mut compiler, catch_scope, "e");
    assert_eq!(value_8, catch_scope);
}

// port: SyntacticScopeCreatorTest#testFunctionName
#[test]
fn test_function_name() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "var f = function foo() {}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "f");
    assert_eq!(value_3, global_scope);
    does_not_declare(&mut compiler, global_scope, "foo");
    let f_node = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, f_node, Some(global_scope));
    let f_scope = value_4;
    let value_5 = declares(&mut compiler, f_scope, "f");
    assert_ne!(value_5, f_scope);
    let value_6 = declares(&mut compiler, f_scope, "foo");
    assert_eq!(value_6, f_scope);
}

// port: SyntacticScopeCreatorTest#testFunctionNameMatchesParamName1
#[test]
fn test_function_name_matches_param_name1() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "var f = function foo(foo) {}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "f");
    assert_eq!(value_3, global_scope);
    does_not_declare(&mut compiler, global_scope, "foo");
    let f_node = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, f_node, Some(global_scope));
    let f_scope = value_4;
    let value_5 = declares(&mut compiler, f_scope, "f");
    assert_ne!(value_5, f_scope);
    let value_6 = declares(&mut compiler, f_scope, "foo");
    assert_eq!(value_6, f_scope);
    let value_7 = f_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert_eq!(
        value_7
            .get_node(&compiler)
            .unwrap()
            .get_parent(&compiler)
            .unwrap()
            .get_token(&compiler),
        Token::PARAM_LIST
    );
}

// port: SyntacticScopeCreatorTest#testFunctionNameMatchesParamName2
#[test]
fn test_function_name_matches_param_name2() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "var f = function foo(x = foo, foo) {}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "f");
    assert_eq!(value_3, global_scope);
    does_not_declare(&mut compiler, global_scope, "foo");
    let f_node = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, f_node, Some(global_scope));
    let f_scope = value_4;
    let value_5 = declares(&mut compiler, f_scope, "f");
    assert_ne!(value_5, f_scope);
    let value_6 = declares(&mut compiler, f_scope, "foo");
    assert_eq!(value_6, f_scope);
    let value_7 = f_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert_eq!(
        value_7
            .get_node(&compiler)
            .unwrap()
            .get_parent(&compiler)
            .unwrap()
            .get_token(&compiler),
        Token::PARAM_LIST
    );
}

// port: SyntacticScopeCreatorTest#testClassName
#[test]
fn test_class_name() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "var Clazz = class Foo {}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "Clazz");
    assert_eq!(value_3, global_scope);
    does_not_declare(&mut compiler, global_scope, "Foo");
    let class_node = root
        .get_first_child(&compiler)
        .unwrap()
        .get_first_first_child(&compiler)
        .unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, class_node, Some(global_scope));
    let class_scope = value_4;
    let value_5 = declares(&mut compiler, class_scope, "Clazz");
    assert_ne!(value_5, class_scope);
    let value_6 = declares(&mut compiler, class_scope, "Foo");
    assert_eq!(value_6, class_scope);
}

// port: SyntacticScopeCreatorTest#testFunctionExpressionInForLoopInitializer
#[test]
fn test_function_expression_in_for_loop_initializer() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "for (function foo() {};;) {}");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "foo");
    let for_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, for_node, Some(global_scope));
    let for_scope = value_3;
    does_not_declare(&mut compiler, for_scope, "foo");
    let f_node = for_node.get_first_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, f_node, Some(for_scope));
    let f_scope = value_4;
    let value_5 = declares(&mut compiler, f_scope, "foo");
    assert_eq!(value_5, f_scope);
}

// port: SyntacticScopeCreatorTest#testClassExpressionInForLoopInitializer
#[test]
fn test_class_expression_in_for_loop_initializer() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "for (class Clazz {};;) {}");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "Clazz");
    let for_node = root.get_first_child(&compiler).unwrap();
    let value_3 = scope_creator.create_scope(&mut compiler, for_node, Some(global_scope));
    let for_scope = value_3;
    does_not_declare(&mut compiler, for_scope, "Clazz");
    let class_node = for_node.get_first_child(&compiler).unwrap();
    let value_4 = scope_creator.create_scope(&mut compiler, class_node, Some(for_scope));
    let class_scope = value_4;
    let value_5 = declares(&mut compiler, class_scope, "Clazz");
    assert_eq!(value_5, class_scope);
}

// port: SyntacticScopeCreatorTest#testClassDeclarationInExportDefault
#[test]
fn test_class_declaration_in_export_default() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let js = "export default class Clazz {}";
    let value_1 = get_root(&mut compiler, js);
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "Clazz");
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let value_4 = declares(&mut compiler, module_scope, "Clazz");
    assert_eq!(value_4, module_scope);
}

// port: SyntacticScopeCreatorTest#testVarsInModulesNotGlobal
#[test]
fn test_vars_in_modules_not_global() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "goog.module('example'); var x;");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "x");
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let value_4 = declares(&mut compiler, module_scope, "x");
    assert_eq!(value_4, module_scope);
}

// port: SyntacticScopeCreatorTest#testGoogModuleDeclaresImplicitExports
#[test]
fn test_goog_module_declares_implicit_exports() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "goog.module('example');");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "exports");
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let value_4 = declares(&mut compiler, module_scope, "exports");
    assert_eq!(value_4, module_scope);
}

// port: SyntacticScopeCreatorTest#testGoogModuleCanOverrideImplicitExports
#[test]
fn test_goog_module_can_override_implicit_exports() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "goog.module('example'); var exports = {};");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "exports");
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_3 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_3;
    let value_4 = declares(&mut compiler, module_scope, "exports");
    assert_eq!(value_4, module_scope);
}

// port: SyntacticScopeCreatorTest#testGoogProvideOfNameInScope
#[test]
fn test_goog_provide_of_name_in_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "goog.provide('foo');");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_3, global_scope);
    let value_4 = global_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert!(value_4.is_implicit_goog_namespace(&compiler));
}

// port: SyntacticScopeCreatorTest#testGoogProvideOfNamespaceInScope
#[test]
fn test_goog_provide_of_namespace_in_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "goog.provide('foo.bar');");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_3, global_scope);
    does_not_declare(&mut compiler, global_scope, "foo.bar");
}

// port: SyntacticScopeCreatorTest#testTwoGoogProvidesOfNameInScope
#[test]
fn test_two_goog_provides_of_name_in_scope() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let value_1 = get_root(
        &mut compiler,
        "goog.provide('foo.bar'); goog.provide('foo.baz');",
    );
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_3, global_scope);
    let value_4 = global_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert!(value_4.is_implicit_goog_namespace(&compiler));
    assert!(redeclarations.lock().unwrap().is_empty());
}

// port: SyntacticScopeCreatorTest#testTwoGoogProvidesOfNameInScope_withTreatProvidesAsRedeclarations
#[test]
fn test_two_goog_provides_of_name_in_scope_with_treat_provides_as_redeclarations() {
    let (mut compiler, _scope_creator, redeclarations) = set_up();
    let mut scope_creator = SyntacticScopeCreator::new_with_options(
        Box::new(RecordingRedeclarationHandler(redeclarations.clone())),
        true,
    );
    let value_1 = get_root(
        &mut compiler,
        "goog.provide('foo.bar'); goog.provide('foo.baz');",
    );
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let _ = value_2;
    assert!(redeclarations.lock().unwrap().is_empty());
}

// port: SyntacticScopeCreatorTest#testVarFollowedByGoogProvidesOfNameInScope
#[test]
fn test_var_followed_by_goog_provides_of_name_in_scope() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "var foo; goog.provide('foo.bar');");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_3, global_scope);
    let value_4 = global_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert!(!value_4.is_implicit_goog_namespace(&compiler));
    assert!(redeclarations.lock().unwrap().is_empty());
}

// port: SyntacticScopeCreatorTest#testVarFollowedByGoogProvidesOfNameInScope_withTreatProvidesAsRedeclarations
#[test]
fn test_var_followed_by_goog_provides_of_name_in_scope_with_treat_provides_as_redeclarations() {
    let (mut compiler, _scope_creator, redeclarations) = set_up();
    let mut scope_creator = SyntacticScopeCreator::new_with_options(
        Box::new(RecordingRedeclarationHandler(redeclarations.clone())),
        true,
    );
    let value_1 = get_root(
        &mut compiler,
        "var foo; goog.provide('foo.bar'); goog.provide('foo');",
    );
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let _ = value_2;
    assert_eq!(redeclarations.lock().unwrap().values().sum::<usize>(), 2);
}

// port: SyntacticScopeCreatorTest#testLegacyGoogModuleNamespaceInScope
#[test]
fn test_legacy_goog_module_namespace_in_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(
        &mut compiler,
        "goog.module('foo.bar'); goog.module.declareLegacyNamespace();",
    );
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_3, global_scope);
    does_not_declare(&mut compiler, global_scope, "foo.bar");
    let module_body = root.get_first_child(&compiler).unwrap();
    assert!(module_body.is_module_body(&compiler));
    let value_4 = scope_creator.create_scope(&mut compiler, module_body, Some(global_scope));
    let module_scope = value_4;
    let value_5 = declares(&mut compiler, module_scope, "foo");
    assert_ne!(global_scope, module_scope);
    assert_eq!(value_5, global_scope);
}

// port: SyntacticScopeCreatorTest#testTwoLegacyNamespaceGoogModules
#[test]
fn test_two_legacy_namespace_goog_modules() {
    let (mut compiler, mut scope_creator, redeclarations) = set_up();
    let value_1 = get_root(
        &mut compiler,
        "goog.module('foo.bar'); goog.module.declareLegacyNamespace();",
    );
    let file1 = value_1;
    let value_2 = get_root(
        &mut compiler,
        "goog.module('foo.baz'); goog.module.declareLegacyNamespace();",
    );
    let file2 = value_2;
    let value_3 = IR::root(&mut compiler, &[file1, file2]);
    let value_4 = scope_creator.create_scope(&mut compiler, value_3, None);
    let global_scope = value_4;
    let value_5 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_5, global_scope);
    let value_6 = global_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert!(value_6.is_implicit_goog_namespace(&compiler));
    assert!(redeclarations.lock().unwrap().is_empty());
}

// port: SyntacticScopeCreatorTest#testTwoLegacyNamespaceGoogModules_withTreatProvidesAsRedeclarations
#[test]
fn test_two_legacy_namespace_goog_modules_with_treat_provides_as_redeclarations() {
    let (mut compiler, _scope_creator, redeclarations) = set_up();
    let mut scope_creator = SyntacticScopeCreator::new_with_options(
        Box::new(RecordingRedeclarationHandler(redeclarations.clone())),
        true,
    );
    let value_1 = get_root(
        &mut compiler,
        "goog.module('foo.bar'); goog.module.declareLegacyNamespace();",
    );
    let file1 = value_1;
    let value_2 = get_root(
        &mut compiler,
        "goog.module('foo.baz'); goog.module.declareLegacyNamespace();",
    );
    let file2 = value_2;
    let value_3 = IR::root(&mut compiler, &[file1, file2]);
    let value_4 = scope_creator.create_scope(&mut compiler, value_3, None);
    let global_scope = value_4;
    let value_5 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_5, global_scope);
    let value_6 = global_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert!(value_6.is_implicit_goog_namespace(&compiler));
    assert!(redeclarations.lock().unwrap().is_empty());
}

// port: SyntacticScopeCreatorTest#testVarFollowedByLegacyGoogModuleNamespace_withTreatProvidesAsRedeclarations
#[test]
fn test_var_followed_by_legacy_goog_module_namespace_with_treat_provides_as_redeclarations() {
    let (mut compiler, _scope_creator, redeclarations) = set_up();
    let mut scope_creator = SyntacticScopeCreator::new_with_options(
        Box::new(RecordingRedeclarationHandler(redeclarations.clone())),
        true,
    );
    let value_1 = get_root(&mut compiler, "var foo;");
    let file1 = value_1;
    let value_2 = get_root(
        &mut compiler,
        "goog.module('foo.bar'); goog.module.declareLegacyNamespace();",
    );
    let file2 = value_2;
    let value_3 = IR::root(&mut compiler, &[file1, file2]);
    let value_4 = scope_creator.create_scope(&mut compiler, value_3, None);
    let global_scope = value_4;
    let value_5 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_5, global_scope);
    let value_6 = global_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert!(!value_6.is_implicit_goog_namespace(&compiler));
    assert_eq!(redeclarations.lock().unwrap().values().sum::<usize>(), 1);
}

// port: SyntacticScopeCreatorTest#testBundledLegacyGoogModuleNamespaceInScope
#[test]
fn test_bundled_legacy_goog_module_namespace_in_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(
        &mut compiler,
        "goog.loadModule(function(exports) {\n  goog.module('foo.bar');\n  goog.module.declareLegacyNamespace();\n  return exports;\n});\n",
    );
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = declares(&mut compiler, global_scope, "foo");
    assert_eq!(value_3, global_scope);
    does_not_declare(&mut compiler, global_scope, "foo.bar");
}

// port: SyntacticScopeCreatorTest#testNonLegacyGoogModuleNamespace_notInScope
#[test]
fn test_non_legacy_goog_module_namespace_not_in_scope() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "goog.module('foo.bar');");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    does_not_declare(&mut compiler, global_scope, "foo");
}

// port: SyntacticScopeCreatorTest#testGoogProvideDeclaresStrength
#[test]
fn test_goog_provide_declares_strength() {
    let (mut compiler, mut scope_creator, _redeclarations) = set_up();
    let value_1 = get_root(&mut compiler, "goog.provide('foo');");
    let root = value_1;
    let value_2 = scope_creator.create_scope(&mut compiler, root, None);
    let global_scope = value_2;
    let value_3 = global_scope
        .get_var(&mut compiler, JsString::from("foo"))
        .unwrap();
    assert_eq!(
        value_3.get_implicit_goog_namespace_strength(&compiler),
        SourceKind::STRONG
    );
}
