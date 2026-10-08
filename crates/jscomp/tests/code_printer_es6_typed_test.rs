/*
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
//   test/com/google/javascript/jscomp/CodePrinterEs6TypedTest.java.

use closure_jscomp::{code_printer::Builder, compiler_options::CompilerOptions};
use closure_rhino::{
    ir::IR,
    node::{Ast, NodeId},
    token::Token,
    type_declarations_ir::TypeDeclarationsIR,
};

// port: CodePrinterEs6TypedTest#assertPrettyPrint
fn assert_pretty_print(ast: &Ast, root: NodeId, js: &str) {
    let mut options = CompilerOptions::default();
    options.set_pretty_print(true);
    options.set_prefer_single_quotes(true);
    options.set_gents_mode(true);
    let parsed = Builder::new(root).set_compiler_options(&options).build(ast);
    assert_eq!(parsed.to_string_lossy().trim(), js); // strip trailing line break.
}

// Constructs `var foo: <typeName> = "hello"`;
// port: CodePrinterEs6TypedTest#constructVarDeclarationWithType
fn construct_var_declaration_with_type(ast: &mut Ast, type_name: &str) -> NodeId {
    let lhs = IR::name(ast, "foo");
    let type_ = TypeDeclarationsIR::named_type(ast, type_name);
    lhs.set_declared_type_expression(ast, Some(type_));
    let value = IR::string(ast, "hello");
    IR::var_with_value(ast, lhs, value)
}
// port: CodePrinterEs6TypedTest#testVariableDeclaration
#[test]
fn test_variable_declaration() {
    let mut ast = Ast::default();
    for (type_name, expected) in [
        ("any", "var foo: any = 'hello';"),
        ("number", "var foo: number = 'hello';"),
        ("boolean", "var foo: boolean = 'hello';"),
        ("string", "var foo: string = 'hello';"),
        ("void", "var foo: void = 'hello';"),
        ("hello", "var foo: hello = 'hello';"),
    ] {
        let root = construct_var_declaration_with_type(&mut ast, type_name);
        assert_pretty_print(&ast, root, expected);
    }
}
// port: CodePrinterEs6TypedTest#testFunctionParamDeclaration
#[test]
fn test_function_param_declaration() {
    let mut ast = Ast::default();
    let string_type = TypeDeclarationsIR::named_type(&mut ast, "string");
    let x_param = IR::name(&mut ast, "x");
    x_param.set_declared_type_expression(&mut ast, Some(string_type));
    let name = IR::name(&mut ast, "foo");
    let params = IR::param_list(&mut ast, &[x_param]);
    let body = IR::block(&mut ast);
    let fn_node = IR::function(&mut ast, name, params, body);
    assert_pretty_print(&ast, fn_node, "function foo(x: string) {\n}");
}
// port: CodePrinterEs6TypedTest#testFunctionParamDeclaration_defaultValue
#[test]
fn test_function_param_declaration_default_value() {
    let mut ast = Ast::default();
    let string_type = TypeDeclarationsIR::named_type(&mut ast, "string");
    let x_param = IR::name(&mut ast, "x");
    x_param.set_declared_type_expression(&mut ast, Some(string_type));
    let name = IR::name(&mut ast, "foo");
    let value = IR::string(&mut ast, "hello");
    let default = ast.new_node_with_children2(Token::DEFAULT_VALUE, x_param, value);
    let params = IR::param_list(&mut ast, &[default]);
    let body = IR::block(&mut ast);
    let fn_node = IR::function(&mut ast, name, params, body);
    assert_pretty_print(&ast, fn_node, "function foo(x: string = 'hello') {\n}");
}
// port: CodePrinterEs6TypedTest#testFunctionParamDeclaration_arrow
#[test]
fn test_function_param_declaration_arrow() {
    let mut ast = Ast::default();
    let string_type = TypeDeclarationsIR::named_type(&mut ast, "string");
    let param = IR::name(&mut ast, "x");
    param.set_declared_type_expression(&mut ast, Some(string_type));
    let name = IR::name(&mut ast, "");
    let params = IR::param_list(&mut ast, &[param]);
    let hello = IR::string(&mut ast, "hello");
    let x = IR::name(&mut ast, "x");
    let body = IR::add(&mut ast, hello, x);
    let fn_node = ast.new_node_with_children3(Token::FUNCTION, name, params, body);
    fn_node.set_is_arrow_function(&mut ast, true);
    let fn_statement = IR::expr_result(&mut ast, fn_node);
    assert_pretty_print(&ast, fn_statement, "(x: string) => 'hello' + x;");
}
// port: CodePrinterEs6TypedTest#testFunctionReturn
#[test]
fn test_function_return() {
    let mut ast = Ast::default();
    let return_type = TypeDeclarationsIR::named_type(&mut ast, "string");
    let name = IR::name(&mut ast, "foo");
    let params = IR::param_list(&mut ast, &[]);
    let hello = IR::string(&mut ast, "hello");
    let return_node = IR::return_node_with_expression(&mut ast, hello);
    let body = IR::block_with_children(&mut ast, &[return_node]);
    let fn_node = IR::function(&mut ast, name, params, body);
    fn_node.set_declared_type_expression(&mut ast, Some(return_type));
    assert_pretty_print(
        &ast,
        fn_node,
        "function foo(): string {\n  return 'hello';\n}",
    );
}
// port: CodePrinterEs6TypedTest#testFunctionReturn_arrow
#[test]
fn test_function_return_arrow() {
    let mut ast = Ast::default();
    let return_type = TypeDeclarationsIR::named_type(&mut ast, "string");
    let name = IR::name(&mut ast, "");
    let params = IR::param_list(&mut ast, &[]);
    let body = IR::string(&mut ast, "hello");
    let fn_node = ast.new_node_with_children3(Token::FUNCTION, name, params, body);
    fn_node.set_declared_type_expression(&mut ast, Some(return_type));
    fn_node.set_is_arrow_function(&mut ast, true);
    let fn_statement = IR::expr_result(&mut ast, fn_node);
    assert_pretty_print(&ast, fn_statement, "(): string => 'hello';");
}
// port: CodePrinterEs6TypedTest#testCompositeType
#[test]
fn test_composite_type() {
    let mut ast = Ast::default();
    let type_ = TypeDeclarationsIR::named_type(&mut ast, "mymod.ns.Type");
    let lhs = IR::name(&mut ast, "foo");
    lhs.set_declared_type_expression(&mut ast, Some(type_));
    let var = IR::var(&mut ast, lhs);
    assert_pretty_print(&ast, var, "var foo: mymod.ns.Type;");
}
// port: CodePrinterEs6TypedTest#testArrayType
#[test]
fn test_array_type() {
    let mut ast = Ast::default();
    let string_type = TypeDeclarationsIR::named_type(&mut ast, "string");
    let type_ = ast.new_node_with_child(Token::ARRAY_TYPE, string_type);
    let lhs = IR::name(&mut ast, "foo");
    lhs.set_declared_type_expression(&mut ast, Some(type_));
    let var = IR::var(&mut ast, lhs);
    assert_pretty_print(&ast, var, "var foo: string[];");
}
// port: CodePrinterEs6TypedTest#testArrayType_qualifiedType
#[test]
fn test_array_type_qualified_type() {
    let mut ast = Ast::default();
    let named_type = TypeDeclarationsIR::named_type(&mut ast, "mymod.ns.Type");
    let array_type = TypeDeclarationsIR::array_type(&mut ast, named_type);
    let lhs = IR::name(&mut ast, "foo");
    lhs.set_declared_type_expression(&mut ast, Some(array_type));
    let var = IR::var(&mut ast, lhs);
    assert_pretty_print(&ast, var, "var foo: mymod.ns.Type[];");
}
// port: CodePrinterEs6TypedTest#testParameterizedType
#[test]
fn test_parameterized_type() {
    let mut ast = Ast::default();
    let named_type = TypeDeclarationsIR::named_type(&mut ast, "my.parameterized.Type");
    let a = TypeDeclarationsIR::named_type(&mut ast, "ns.A");
    let b = TypeDeclarationsIR::named_type(&mut ast, "ns.B");
    let type_ = TypeDeclarationsIR::parameterized_type(&mut ast, named_type, &[a, b]);
    let lhs = IR::name(&mut ast, "x");
    lhs.set_declared_type_expression(&mut ast, Some(type_));
    let var = IR::var(&mut ast, lhs);
    assert_pretty_print(&ast, var, "var x: my.parameterized.Type<ns.A, ns.B>;");
}
