/*
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CrossChunkReferenceCollector.java,
//   src/com/google/javascript/jscomp/ReferenceCollection.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/CrossChunkReferenceCollectorTest.java.

//! Port of CrossChunkReferenceCollectorTest (CompilerTestCase over the crates/testing port).
mod support;

use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    cross_chunk_reference_collector::{CrossChunkReferenceCollector, TopLevelStatement},
    reference::Reference,
    reference_collection::ReferenceCollection,
    syntactic_scope_creator::SyntacticScopeCreator,
    var::VarId,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue},
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};
use support::cross_chunk_fixture::{check, native_ctx, pass, srcs_strings};

type Collector = Rc<RefCell<CrossChunkReferenceCollector<'static>>>;

/// The processor getProcessor returns: Java returns `testedCollector` itself, so the test keeps
/// a second handle on the same collector.
struct TestedCollectorPass(Collector);
impl CompilerPass for TestedCollectorPass {
    // port: CrossChunkReferenceCollector#process(Node,Node)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.0.borrow_mut().process(compiler, externs, root);
    }
}

struct Hooks {
    ctx: Ctx,
    tested_collector: Option<Collector>,
    /// The compiler getProcessor received; Var, Reference and Node reads go through it.
    compiler: Option<CompilerHandle>,
}

impl CompilerTestCaseHooks for Hooks {
    // port: CrossChunkReferenceCollectorTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let scope_creator = SyntacticScopeCreator::new();
        let collector: Collector = Rc::new(RefCell::new(CrossChunkReferenceCollector::new(
            Box::new(scope_creator),
        )));
        self.tested_collector = Some(collector.clone());
        self.compiler = Some(compiler);
        Ok(pass(TestedCollectorPass(collector)))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

struct Test {
    harness: CompilerTestCase,
    hooks: Hooks,
}

impl Test {
    // port: CompilerTestCase#setUp + CrossChunkReferenceCollectorTest#customSetUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        let mut t = Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("CrossChunkReferenceCollectorTest"),
                tested_collector: None,
                compiler: None,
            },
        };
        t.harness.enable_normalize().unwrap();
        t
    }
    // port: CompilerTestCase#testSame(String)
    fn test_same(&mut self, js: &str) {
        check(
            self.harness
                .test_same(&mut self.hooks, vec![srcs_strings(&[&js])]),
        );
    }
    fn collector(&self) -> std::cell::Ref<'_, CrossChunkReferenceCollector<'static>> {
        self.hooks.tested_collector.as_ref().unwrap().borrow()
    }
    fn compiler(&self) -> std::cell::RefMut<'_, AbstractCompiler> {
        self.hooks.compiler.as_ref().unwrap().borrow_mut()
    }

    // port: CrossChunkReferenceCollector#getGlobalVariableNamesMap
    fn global_variable_names_map(&self) -> IndexMap<JsString, VarId> {
        self.collector().get_global_variable_names_map()
    }
    // port: CrossChunkReferenceCollector#getReferences
    fn references(&self, var: VarId) -> ReferenceCollection {
        self.collector().get_references(var).unwrap().clone()
    }
    // port: CrossChunkReferenceCollector#getTopLevelStatements
    fn top_level_statements(&self) -> std::cell::Ref<'_, [TopLevelStatement]> {
        std::cell::Ref::map(self.collector(), |c| c.get_top_level_statements())
    }
    // port: CrossChunkReferenceCollector.TopLevelStatement#isMovableDeclaration
    fn is_movable_declaration(&self, index: usize) -> bool {
        let collector = self.collector();
        collector.get_top_level_statements()[index]
            .is_movable_declaration(&mut self.compiler(), &collector)
    }
    // port: ReferenceCollection#isAssignedOnceInLifetime
    fn is_assigned_once_in_lifetime(&self, refs: &ReferenceCollection) -> bool {
        refs.is_assigned_once_in_lifetime(&mut self.compiler())
    }
    // port: ReferenceCollection#isWellDefined
    fn is_well_defined(&self, refs: &ReferenceCollection) -> bool {
        refs.is_well_defined(&self.compiler())
    }
    // assertNode(reference.getBasicBlock().getRoot()).hasType(token)
    fn assert_basic_block_root_type(&self, reference: &Reference, token: Token) {
        let root = reference.get_basic_block().unwrap().get_root();
        assert_eq!(root.get_token(&self.compiler()), token);
    }
    // valueNode.getDouble()
    fn get_double(&self, n: NodeId) -> f64 {
        n.get_double(&self.compiler())
    }

    // port: CrossChunkReferenceCollectorTest#assertStatementIsImmovable
    fn assert_statement_is_immovable(&mut self, statement: &str) {
        self.test_same(statement);
        assert!(!self.is_movable_declaration(0));
    }

    // port: CrossChunkReferenceCollectorTest#getReferencesForName
    fn get_references_for_name(&self, name: &str) -> ReferenceCollection {
        let v = self
            .global_variable_names_map()
            .get(&JsString::from(name))
            .copied();
        assert!(v.is_some());
        self.references(v.unwrap())
    }
}

/// Java compares References by identity: each collected Reference is its own name node's,
/// sharing the BasicBlock object it was created in.
fn same_reference(a: &Reference, b: &Reference) -> bool {
    a.get_node() == b.get_node()
        && match (a.get_basic_block(), b.get_basic_block()) {
            (Some(x), Some(y)) => Rc::ptr_eq(x, y),
            (None, None) => true,
            _ => false,
        }
}

// assertThat(actual).isEqualTo(expected) for References
fn assert_same_reference(actual: &Reference, expected: &Reference) {
    assert!(same_reference(actual, expected));
}

// assertThat(actual).containsExactly(expected...) for References
fn assert_contains_exactly(actual: &[Reference], expected: &[&Reference]) {
    assert_eq!(actual.len(), expected.len());
    let mut unmatched: Vec<&Reference> = expected.to_vec();
    for a in actual {
        let i = unmatched.iter().position(|e| same_reference(a, e));
        assert!(i.is_some(), "unexpected reference {a:?}");
        unmatched.remove(i.unwrap());
    }
}

// port: CrossChunkReferenceCollectorTest#testVarInBlock
#[test]
fn test_var_in_block() {
    let mut t = Test::new();
    t.test_same(
        "  if (true) {
    var y = x;
    y;
    y;
  }
",
    );
    let global_variable_names_map = t.global_variable_names_map();
    assert!(global_variable_names_map.contains_key(&JsString::from("y")));
    let y_var = global_variable_names_map[&JsString::from("y")];
    let y_refs = t.references(y_var);
    assert!(t.is_assigned_once_in_lifetime(&y_refs));
    assert!(t.is_well_defined(&y_refs));
}

/// The two halves of the tests below that run once with `var` and once with `let`.
fn assert_assigned_once_in_lifetime(t: &mut Test, js: &str, expected: bool) {
    t.test_same(js);
    let global_variable_names_map = t.global_variable_names_map();
    let x_var = global_variable_names_map.get(&JsString::from("x")).copied();
    assert!(global_variable_names_map.contains_key(&JsString::from("x")));
    let x_refs = t.references(x_var.unwrap());
    assert_eq!(t.is_assigned_once_in_lifetime(&x_refs), expected);
}

// port: CrossChunkReferenceCollectorTest#testVarInLoopNotAssignedOnlyOnceInLifetime
#[test]
fn test_var_in_loop_not_assigned_only_once_in_lifetime() {
    let mut t = Test::new();
    assert_assigned_once_in_lifetime(&mut t, "var x; for (; true;) { x = 0; }", false);
    assert_assigned_once_in_lifetime(&mut t, "let x; for (; true;) { x = 0; }", false);
}

/// Although there is only one assignment to x in the code, it's in a function which could be
/// called multiple times, so `isAssignedOnceInLifetime()` returns false.
// port: CrossChunkReferenceCollectorTest#testVarInFunctionNotAssignedOnlyOnceInLifetime
#[test]
fn test_var_in_function_not_assigned_only_once_in_lifetime() {
    let mut t = Test::new();
    assert_assigned_once_in_lifetime(&mut t, "var x; function f() { x = 0; }", false);
    assert_assigned_once_in_lifetime(&mut t, "let x; function f() { x = 0; }", false);
}

// port: CrossChunkReferenceCollectorTest#testVarAssignedOnceInLifetime1
#[test]
fn test_var_assigned_once_in_lifetime1() {
    let mut t = Test::new();
    assert_assigned_once_in_lifetime(&mut t, "var x = 0;", true);
    assert_assigned_once_in_lifetime(&mut t, "let x = 0;", true);
}

// port: CrossChunkReferenceCollectorTest#testVarAssignedOnceInLifetime2
#[test]
fn test_var_assigned_once_in_lifetime2() {
    let mut t = Test::new();
    assert_assigned_once_in_lifetime(&mut t, "{ var x = 0; }", true);
}

/// testBasicBlocks / testClassStaticBlock / nullishCoalesce: the roots of x's references'
/// basic blocks.
fn assert_x_basic_block_roots(t: &Test, roots: &[Token]) -> ReferenceCollection {
    let global_variable_names_map = t.global_variable_names_map();
    assert!(global_variable_names_map.contains_key(&JsString::from("x")));
    let x_var = global_variable_names_map[&JsString::from("x")];
    let x_refs = t.references(x_var);
    assert_eq!(x_refs.references.len(), roots.len());
    for (reference, &token) in x_refs.references.iter().zip(roots) {
        t.assert_basic_block_root_type(reference, token);
    }
    x_refs
}

// port: CrossChunkReferenceCollectorTest#testBasicBlocks
#[test]
fn test_basic_blocks() {
    let mut t = Test::new();
    t.test_same(
        "var x = 0;
switch (x) {
  case 0:
    x;
}
",
    );
    assert_x_basic_block_roots(&t, &[Token::ROOT, Token::ROOT, Token::CASE]);
}

// port: CrossChunkReferenceCollectorTest#testClassStaticBlock
#[test]
fn test_class_static_block() {
    let mut t = Test::new();
    t.test_same(
        "var x=1;
class C {
  static {
    x;
    x=2;
  }
}
",
    );
    let global_variable_names_map = t.global_variable_names_map();
    assert!(global_variable_names_map.contains_key(&JsString::from("x")));
    let x_refs = t.references(global_variable_names_map[&JsString::from("x")]);
    assert!(!t.is_assigned_once_in_lifetime(&x_refs));
    assert_x_basic_block_roots(
        &t,
        &[Token::ROOT, Token::CLASS_MEMBERS, Token::CLASS_MEMBERS],
    );
}

// port: CrossChunkReferenceCollectorTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    let mut t = Test::new();
    t.test_same("var x = 0; var y = x ?? (x = 1)");
    assert_x_basic_block_roots(&t, &[Token::ROOT, Token::ROOT, Token::ASSIGN]);
}

// port: CrossChunkReferenceCollectorTest#testTopLevelStatements
#[test]
fn test_top_level_statements() {
    let mut t = Test::new();
    t.test_same(
        "var x = 1;
const y = x;
let z = x - y;
function f(x1, y1) { // only f and z globals referenced
// NOTE: If we try to name the parameters the same as the global variables, then
// Normalization will rename them. Normalization always runs before
// CrossChunkReferenceCollector does.
  return x1 + y1 + z;
}
",
    );

    // Pull out all the references for comparison.
    let global_variable_names_map = t.global_variable_names_map();
    let refs = |name: &str| -> Vec<Reference> {
        t.references(global_variable_names_map[&JsString::from(name)])
            .iter()
            .cloned()
            .collect()
    };
    let x_references = refs("x");
    let y_references = refs("y");
    let z_references = refs("z");
    let f_references = refs("f");

    // Make sure the statements have the references we expect.
    let top_level_statements = t.top_level_statements();
    assert_eq!(top_level_statements.len(), 4);
    // var x = 1;
    let x_equals1 = &top_level_statements[0];
    assert_eq!(x_equals1.get_original_order(), 0);
    assert_same_reference(x_equals1.get_declared_name_reference(), &x_references[0]);
    assert!(x_equals1.get_non_declaration_references().is_empty());
    // const y = x;
    let y_equals_x = &top_level_statements[1];
    assert_eq!(y_equals_x.get_original_order(), 1);
    assert_contains_exactly(
        y_equals_x.get_non_declaration_references(),
        &[&x_references[1]],
    );
    // let z = x - y;
    let z_equals_x_minus_y = &top_level_statements[2];
    assert_eq!(z_equals_x_minus_y.get_original_order(), 2);
    assert_contains_exactly(
        z_equals_x_minus_y.get_non_declaration_references(),
        &[&x_references[2], &y_references[1]],
    );
    // function f(x, y) { return x + y + z; }
    let function_declaration = &top_level_statements[3];
    assert_eq!(function_declaration.get_original_order(), 3);
    assert_same_reference(
        function_declaration.get_declared_name_reference(),
        &f_references[0],
    );
    assert_contains_exactly(
        function_declaration.get_non_declaration_references(),
        &[&z_references[1]],
    );
}

// port: CrossChunkReferenceCollectorTest#testVarDeclarationStatement
#[test]
fn test_var_declaration_statement() {
    let mut t = Test::new();
    t.test_same("var x = 1;");

    let statements = t.top_level_statements();
    let x_refs = t.get_references_for_name("x");
    assert_eq!(statements.len(), 1);
    let var_statement = &statements[0];
    let declared_name_reference = var_statement.get_declared_name_reference();
    assert_same_reference(declared_name_reference, &x_refs.references[0]);
    assert!(var_statement.get_non_declaration_references().is_empty());
    let value_node = var_statement.get_declared_value_node();
    assert!(value_node.is_some());
    assert_eq!(t.get_double(value_node.unwrap()), 1.0);
}

// port: CrossChunkReferenceCollectorTest#testFunctionDeclarationStatement
#[test]
fn test_function_declaration_statement() {
    let mut t = Test::new();
    t.test_same("function x() {}");

    let statements = t.top_level_statements();
    assert_eq!(statements.len(), 1);
    let x_refs = t.get_references_for_name("x");

    let function_declaration = &statements[0];
    let declared_name_reference = function_declaration.get_declared_name_reference();
    assert_same_reference(declared_name_reference, &x_refs.references[0]);
    assert!(
        function_declaration
            .get_non_declaration_references()
            .is_empty()
    );
}

/// testVariableAssignmentStatement / testPropertyAssignmentStatement
fn assert_second_statement_assigns_x_one(t: &mut Test, js: &str) {
    t.test_same(js);

    let statements = t.top_level_statements();
    assert_eq!(statements.len(), 2);
    let x_refs = t.get_references_for_name("x");

    let assignment_statement = &statements[1];
    let declared_name_reference = assignment_statement.get_declared_name_reference();
    assert_same_reference(declared_name_reference, &x_refs.references[1]);
    assert!(
        assignment_statement
            .get_non_declaration_references()
            .is_empty()
    );
    let value_node = assignment_statement.get_declared_value_node();
    assert!(value_node.is_some());
    assert_eq!(t.get_double(value_node.unwrap()), 1.0);
}

// port: CrossChunkReferenceCollectorTest#testVariableAssignmentStatement
#[test]
fn test_variable_assignment_statement() {
    let mut t = Test::new();
    assert_second_statement_assigns_x_one(&mut t, "var x; x = 1;");
}

// port: CrossChunkReferenceCollectorTest#testPropertyAssignmentStatement
#[test]
fn test_property_assignment_statement() {
    let mut t = Test::new();
    assert_second_statement_assigns_x_one(&mut t, "var x = {}; x.prop = 1;");
}

// port: CrossChunkReferenceCollectorTest#testGoogInheritsIsMovableDeclaration
#[test]
fn test_goog_inherits_is_movable_declaration() {
    let mut t = Test::new();
    t.test_same("function A() {} function B() {} goog.inherits(B, A);");

    let statements = t.top_level_statements();
    assert_eq!(statements.len(), 3);

    let refs_to_a = t.get_references_for_name("A");
    let refs_to_b = t.get_references_for_name("B");

    let inherits_statement = &statements[2];
    let declared_name_reference = inherits_statement.get_declared_name_reference();
    assert_same_reference(declared_name_reference, &refs_to_b.references[1]);
    assert_contains_exactly(
        inherits_statement.get_non_declaration_references(),
        &[&refs_to_a.references[1]],
    );
    // inherits statements are always movable
    assert!(t.is_movable_declaration(2));
}

/// testDefinePropertiesIsMovableDeclaration / testDefinePropertiesWithPrototypeIsMovableDeclaration
fn assert_define_properties_is_movable_declaration(t: &mut Test, js: &str) {
    t.test_same(js);

    let statements = t.top_level_statements();
    assert_eq!(statements.len(), 2);

    let define_properties_statement = &statements[1];
    // assertThat(declaredNameReference).isNotNull()
    assert!(define_properties_statement.is_declaration_statement());
    let _declared_name_reference = define_properties_statement.get_declared_name_reference();
    assert!(
        define_properties_statement
            .get_non_declaration_references()
            .is_empty()
    );
    // defineProperties statements are always movable
    assert!(t.is_movable_declaration(1));
}

// port: CrossChunkReferenceCollectorTest#testDefinePropertiesIsMovableDeclaration
#[test]
fn test_define_properties_is_movable_declaration() {
    let mut t = Test::new();
    assert_define_properties_is_movable_declaration(
        &mut t,
        "function A() {} Object.defineProperties(A, {});",
    );
}

// port: CrossChunkReferenceCollectorTest#testDefinePropertiesWithPrototypeIsMovableDeclaration
#[test]
fn test_define_properties_with_prototype_is_movable_declaration() {
    let mut t = Test::new();
    assert_define_properties_is_movable_declaration(
        &mut t,
        "function A() {} Object.defineProperties(A.prototype, {});",
    );
}

// port: CrossChunkReferenceCollectorTest#testFunctionDeclarationOrAssignmentIsMovable
#[test]
fn test_function_declaration_or_assignment_is_movable() {
    let mut t = Test::new();
    t.test_same("function f() {}");
    assert!(t.is_movable_declaration(0));
    t.test_same("var f = function() {};");
    assert!(t.is_movable_declaration(0));
}

// port: CrossChunkReferenceCollectorTest#testLiteralValueIsMovable
#[test]
fn test_literal_value_is_movable() {
    let mut t = Test::new();
    t.test_same("var f = 1;");
    assert!(t.is_movable_declaration(0));
}

// port: CrossChunkReferenceCollectorTest#testFunctionCallsAreNotMovableExceptForMethodStubs
#[test]
fn test_function_calls_are_not_movable_except_for_method_stubs() {
    let mut t = Test::new();
    t.test_same(
        "function Foo() {}
Foo.prototype.stub = JSCompiler_stubMethod(x);
Foo.prototype.unstub = JSCompiler_unstubMethod(x);
Foo.prototype.other = other();
",
    );
    assert!(t.is_movable_declaration(1));
    assert!(!t.is_movable_declaration(2));
    assert!(!t.is_movable_declaration(3));
}

// port: CrossChunkReferenceCollectorTest#testUnknownNameValueIsImmovable
#[test]
fn test_unknown_name_value_is_immovable() {
    let mut t = Test::new();
    t.assert_statement_is_immovable("var a = unknownName;");
}

// port: CrossChunkReferenceCollectorTest#testWellDefinedNameValueIsMovable
#[test]
fn test_well_defined_name_value_is_movable() {
    let mut t = Test::new();
    t.test_same("var wellDefined = 1; var other = wellDefined;");
    assert!(t.is_movable_declaration(1));
}

// port: CrossChunkReferenceCollectorTest#testUninitializedNameValueIsNotMovable
#[test]
fn test_uninitialized_name_value_is_not_movable() {
    let mut t = Test::new();
    t.test_same("var value; var other = value;");
    assert!(!t.is_movable_declaration(1));
}

// port: CrossChunkReferenceCollectorTest#testReDefinedNameValueIsNotMovable
#[test]
fn test_re_defined_name_value_is_not_movable() {
    let mut t = Test::new();
    t.test_same("var redefined = 1; redefined = 2; var other = redefined;");
    assert!(!t.is_movable_declaration(2));
}

// port: CrossChunkReferenceCollectorTest#testEmptyArrayLiteralIsMovable
#[test]
fn test_empty_array_literal_is_movable() {
    let mut t = Test::new();
    t.test_same("var a = [];");
    assert!(t.is_movable_declaration(0));
}

// port: CrossChunkReferenceCollectorTest#testArrayLiteralOfMovablesIsMovable
#[test]
fn test_array_literal_of_movables_is_movable() {
    let mut t = Test::new();
    t.test_same("var wellDefinedName = 1; var a = [function(){}, 1, wellDefinedName, []];");
    assert!(t.is_movable_declaration(1));
}

// port: CrossChunkReferenceCollectorTest#testArrayLiteralWithImmovableIsImmovable
#[test]
fn test_array_literal_with_immovable_is_immovable() {
    let mut t = Test::new();
    t.assert_statement_is_immovable("var a = [unknownValue];");
}

// port: CrossChunkReferenceCollectorTest#testEmptyObjectLiteralIsMovable
#[test]
fn test_empty_object_literal_is_movable() {
    let mut t = Test::new();
    t.test_same("var o = {};");
    assert!(t.is_movable_declaration(0));
}

// port: CrossChunkReferenceCollectorTest#testObjectLiteralOfMovablesIsMovable
#[test]
fn test_object_literal_of_movables_is_movable() {
    let mut t = Test::new();
    t.test_same(
        "var wellDefinedName = 1;
var o = {
  f: function(){},
  one: 1,
  n: wellDefinedName,
  o: {},
  'quoted': 1,
  123: 2,
// computed
  ['computed string']: 1,
  [234]: 1,
// method shorthand
  method() {},
  'quoted method'() {},
  ['computed method']() {},
  [345]() {},
// variable shorthand
  wellDefinedName,
// getters
  get x() {},
  get 'a'() {},
  get ['a']() {},
  get 456() {},
  get [567]() {},
// setters
  set x(x) {},
  set 'a'(v) {},
  set ['a'](v1) {},
  set 678(v2) {},
  set [678](v3) {},
// spread
  ...wellDefinedName,
};
",
    );
    assert!(t.is_movable_declaration(1));
}

// port: CrossChunkReferenceCollectorTest#testObjectLiteralWithImmovableIsImmovable
#[test]
fn test_object_literal_with_immovable_is_immovable() {
    let mut t = Test::new();
    t.assert_statement_is_immovable("var o = { v: unknownValue };");
    t.assert_statement_is_immovable("var o = { ...unknownValue };");
    t.assert_statement_is_immovable("var o = { [unknownValue]: 1 };");
    t.assert_statement_is_immovable("var o = { [unknownValue]() {} };");
    t.assert_statement_is_immovable("var o = { get [unknownValue]() {} };");
    t.assert_statement_is_immovable("var o = { set [unknownValue](x) {} };");
}

// port: CrossChunkReferenceCollectorTest#testTemplateLiteralIsMovableIfSubstitutionsAreMovable
#[test]
fn test_template_literal_is_movable_if_substitutions_are_movable() {
    let mut t = Test::new();
    t.test_same(
        "var wellDefinedName = 1;
var t = `${wellDefinedName}`;
",
    );
    assert!(t.is_movable_declaration(1));
}

// port: CrossChunkReferenceCollectorTest#testTemplateLiteralIsImmovableIfSubstitutionsAreImmovable
#[test]
fn test_template_literal_is_immovable_if_substitutions_are_immovable() {
    let mut t = Test::new();
    t.assert_statement_is_immovable("var t = `${unknownValue}`");
}

// port: CrossChunkReferenceCollectorTest#testPureOrBreakMyCodeAnnotatedStringConcatIsMovable
#[test]
fn test_pure_or_break_my_code_annotated_string_concat_is_movable() {
    let mut t = Test::new();
    t.test_same(
        "/** @pureOrBreakMyCode */
var t = 'a' + 'b';
",
    );
    assert!(t.is_movable_declaration(0));
}

// port: CrossChunkReferenceCollectorTest#testPureOrBreakMyCodeAnnotatedFunctionCallIsMovable
#[test]
fn test_pure_or_break_my_code_annotated_function_call_is_movable() {
    let mut t = Test::new();
    t.test_same(
        "/** @pureOrBreakMyCode */
var t = someFn();
",
    );
    assert!(t.is_movable_declaration(0));
}

// port: CrossChunkReferenceCollectorTest#testPureOrBreakMyCodeAnnotatedStaticClassPropertyInitializerIsMovable
#[test]
fn test_pure_or_break_my_code_annotated_static_class_property_initializer_is_movable() {
    let mut t = Test::new();
    t.test_same(
        "class SomeClass {}
SomeClass.staticProp = /** @pureOrBreakMyCode */ someFn();
",
    );
    assert!(t.is_movable_declaration(0));
    assert!(t.is_movable_declaration(1));
}

// port: CrossChunkReferenceCollectorTest#testPureOrBreakMyCodeAnnotatedStaticClassPropertyInitializerIsMovable2
#[test]
fn test_pure_or_break_my_code_annotated_static_class_property_initializer_is_movable2() {
    let mut t = Test::new();
    t.test_same(
        "class SomeClass {}
SomeClass.staticProp = /** @pureOrBreakMyCode */ { prop: someFn() ? 'a' : 'b' };
",
    );
    assert!(t.is_movable_declaration(0));
    assert!(t.is_movable_declaration(1));
}

// port: CrossChunkReferenceCollectorTest#testPureOrBreakMyCodeAnnotatedNonstaticClassFieldIsMovable
#[test]
fn test_pure_or_break_my_code_annotated_nonstatic_class_field_is_movable() {
    let mut t = Test::new();
    t.test_same(
        "class SomeClass {
  a;
  static b = 2;
  ['c'] = 3;
  static 'd' = 'hi';
  1 = 2;
}
",
    );
    assert!(t.is_movable_declaration(0));
}

// port: CrossChunkReferenceCollectorTest#testPureOrBreakMyCodeAnnotatedStaticClassFieldIsMovable
#[test]
fn test_pure_or_break_my_code_annotated_static_class_field_is_movable() {
    let mut t = Test::new();
    t.test_same(
        "class SomeClass {
  static staticProp = /** @pureOrBreakMyCode */ someFn();
}
",
    );
    assert!(t.is_movable_declaration(0));
}
