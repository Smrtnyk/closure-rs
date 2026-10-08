/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/GlobalNamespaceTest.java.

//! Port of GlobalNamespaceTest (JUnit). Every helper compiles through `Compiler#compile` with
//! `setSkipNonTranspilationPasses(true)` (DefaultPassConfig#getTranspileOnlyPasses).
//!
//! NodeSubject assertions the Rust NodeSubject does not have (matchesQualifiedName, isAssign,
//! isNumber, ...) are written as the Node predicates they call.
use closure_jscomp::{
    Compiler,
    abstract_compiler::AbstractCompiler,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    gather_module_metadata::GatherModuleMetadata,
    global_namespace::{
        AstChange, GlobalNamespace, Inlinability, Name, Ref, RefType, SimpleAstChange,
    },
    js_chunk::JSChunk,
    modules::{module_map_creator::ModuleMapCreator, module_metadata_map::ModuleMetadata},
    source_file::SourceFile,
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_jstype::{
    rhino::js_type_expression::JSTypeExpressionExt, testing::type_subject::TypeSubject,
};
use closure_rhino::{ir::IR, js_string::JsString, node::NodeId, token::Token};
use closure_testing::testing::js_chunk_graph_builder::JSChunkGraphBuilder;
use indexmap::IndexSet;
use std::sync::Arc;

/// `GlobalNamespaceTest`'s instance fields.
struct GlobalNamespaceTest {
    // port: GlobalNamespaceTest#lastCompiler
    last_compiler: Option<Compiler>,
    // port: GlobalNamespaceTest#assumeStaticInheritanceIsNotUsed
    assume_static_inheritance_is_not_used: bool,
}

impl GlobalNamespaceTest {
    fn new() -> Self {
        Self {
            last_compiler: None,
            assume_static_inheritance_is_not_used: true,
        }
    }

    /// `lastCompiler`, non-null after a parse.
    fn c(&mut self) -> &mut AbstractCompiler {
        self.last_compiler.as_mut().expect("lastCompiler")
    }

    // port: GlobalNamespaceTest#parse(String)
    fn parse(&mut self, js: &str) -> GlobalNamespace {
        let options = self.get_default_options();
        self.compile(js, options);
        let compiler = self.c();
        let root = compiler.get_root().expect("root");
        GlobalNamespace::new_without_externs(compiler, root)
    }

    /// This method exists for testing module metadata lookups.
    // port: GlobalNamespaceTest#parseAndGatherModuleData
    fn parse_and_gather_module_data(&mut self, js: &str) -> GlobalNamespace {
        let options = self.get_default_options();
        let process_common_js_modules = options.get_process_common_js_modules();
        let module_resolution_mode = options.get_module_resolution_mode();
        let compiler = self.compile(js, options);

        // Disabling transpilation also disables these passes that we need to have run when
        // testing behavior related to module metadata.
        let externs = compiler.get_externs_root().expect("externsRoot");
        let js_root = compiler.get_js_root().expect("jsRoot");
        GatherModuleMetadata::new(process_common_js_modules, module_resolution_mode)
            .process(compiler, externs, js_root);
        let module_metadata_map = compiler
            .get_module_metadata_map()
            .expect("moduleMetadataMap")
            .clone();
        ModuleMapCreator::new(module_metadata_map).process(compiler, externs, js_root);
        assert_eq!(compiler.get_errors(), vec![]);
        let root = compiler.get_root().expect("root");
        GlobalNamespace::new_without_externs(compiler, root)
    }

    // port: GlobalNamespaceTest#parse(JSChunk[])
    fn parse_chunks(&mut self, chunks: &[JSChunk]) -> GlobalNamespace {
        let options = self.get_default_options();
        let mut compiler = Compiler::new();
        let result = compiler.compile_chunks(&[], chunks.to_vec(), options);
        assert_eq!(compiler.get_errors(), vec![]);
        assert!(result.success);
        self.last_compiler = Some(compiler);
        let compiler = self.c();
        let root = compiler.get_root().expect("root");
        GlobalNamespace::new_without_externs(compiler, root)
    }

    /// `lastCompiler.getModuleMetadataMap().getModulesByGoogNamespace().get(namespace)`
    fn module_by_goog_namespace(&mut self, namespace: &str) -> Arc<ModuleMetadata> {
        self.c()
            .get_module_metadata_map()
            .expect("moduleMetadataMap")
            .get_modules_by_goog_namespace()
            .get(&JsString::from(namespace))
            .expect("module")
            .clone()
    }

    /// `lastCompiler.getModuleMetadataMap().getModulesByPath().get(path)`
    fn module_by_path(&mut self, path: &str) -> Arc<ModuleMetadata> {
        self.c()
            .get_module_metadata_map()
            .expect("moduleMetadataMap")
            .get_modules_by_path()
            .get(path)
            .expect("module")
            .clone()
    }

    /// `namespace.getNameFromModule(metadata, name)`
    fn name_from_module(
        &mut self,
        namespace: &mut GlobalNamespace,
        metadata: &Arc<ModuleMetadata>,
        name: &str,
    ) -> Option<Name> {
        namespace.get_name_from_module(self.c(), metadata, &JsString::from(name))
    }

    // port: GlobalNamespaceTest#getDefaultOptions
    fn get_default_options(&self) -> CompilerOptions {
        let mut options = CompilerOptions::new();
        options.set_language(LanguageMode::UNSUPPORTED);
        // Don't optimize, because we want to know how GlobalNamespace responds to the original
        // code in `js`.
        options.set_skip_non_transpilation_passes(true);
        options.set_wrap_goog_modules_for_whitespace_only(false);
        // Test the latest features supported for input and don't transpile, because we want to
        // test how GlobalNamespace deals with the language features actually present in `js`.
        options
            .set_assume_static_inheritance_is_not_used(self.assume_static_inheritance_is_not_used);
        options
    }

    // port: GlobalNamespaceTest#compile
    fn compile(&mut self, js: &str, options: CompilerOptions) -> &mut Compiler {
        let mut compiler = Compiler::new();
        compiler.compile_single(
            Arc::new(SourceFile::from_code("ex.js", "")),
            Arc::new(SourceFile::from_code("test.js", js)),
            options,
        );
        assert_eq!(compiler.get_errors(), vec![]);
        self.last_compiler = Some(compiler);
        self.c()
    }

    fn slot(&mut self, namespace: &mut GlobalNamespace, name: &str) -> Option<Name> {
        let compiler = self.last_compiler.as_mut().expect("lastCompiler");
        namespace.get_slot(compiler, &JsString::from(name))
    }

    fn own_slot(&mut self, namespace: &mut GlobalNamespace, name: &str) -> Option<Name> {
        let compiler = self.last_compiler.as_mut().expect("lastCompiler");
        namespace.get_own_slot(compiler, &JsString::from(name))
    }
}

impl GlobalNamespaceTest {
    // port: GlobalNamespaceTest#createGlobalAstChangeForNode
    fn create_global_ast_change_for_node(&mut self, js_root: NodeId, n: NodeId) -> AstChange {
        // This only creates a global scope, so don't use this with local nodes
        let c = self.c();
        let global_scope = SyntacticScopeCreator::new().create_scope(c, js_root, None);
        let first_chunk = c.get_chunks().and_then(|chunks| chunks.first().cloned());
        AstChange::SimpleAstChange(SimpleAstChange::new(n, first_chunk, Some(global_scope)))
    }

    fn js_root(&mut self) -> NodeId {
        self.c().get_js_root().expect("jsRoot")
    }
}

/// `assertThat(name.getRefs()).hasSize(n)`
fn assert_ref_count(gn: &GlobalNamespace, name: Name, n: usize) {
    assert_eq!(name.get_refs(gn).len(), n);
}

/// `assertThrows(type, runnable)`. A failed Preconditions check is a panic in Rust, whatever the
/// Java exception class; the assertion checks the panic carries the message of the Java check
/// that throws it (`message`).
fn assert_throws(message: &str, f: impl FnOnce()) {
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .expect_err("expected an exception");
    let actual = error
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        actual.contains(message),
        "expected a panic with {message:?}, got: {actual}"
    );
}

// port: GlobalNamespaceTest#detectsPropertySetsInAssignmentOperators
#[test]
fn detects_property_sets_in_assignment_operators() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const a = {b: 0}; a.b += 1; a.b = 2;");

    let ab = t.slot(&mut namespace, "a.b").unwrap();
    assert_eq!(ab.get_global_sets(&namespace), 3);
}

// port: GlobalNamespaceTest#detectsPropertySetsInDestructuring
#[test]
fn detects_property_sets_in_destructuring() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const a = {b: 0}; [a.b] = [1]; ({b: a.b} = {b: 2});");

    // TODO(b/120303257): this should be 3
    let ab = t.slot(&mut namespace, "a.b").unwrap();
    assert_eq!(ab.get_global_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#detectsPropertySetsInIncDecOperators
#[test]
fn detects_property_sets_in_inc_dec_operators() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const a = {b: 0}; a.b++; a.b--;");

    let ab = t.slot(&mut namespace, "a.b").unwrap();
    assert_eq!(ab.get_global_sets(&namespace), 3);
}

// port: GlobalNamespaceTest#firstGlobalAssignmentIsConsideredDeclaration
#[test]
fn first_global_assignment_is_considered_declaration() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("");
    let n = namespace.create_name_for_testing("a");
    let c = t.c();
    let set1_node = IR::name(c, "set1");
    let set1 = n.add_single_ref_for_testing(&mut namespace, c, set1_node, RefType::SET_FROM_GLOBAL);
    let set2_node = IR::name(c, "set2");
    let set2 = n.add_single_ref_for_testing(&mut namespace, c, set2_node, RefType::SET_FROM_GLOBAL);

    assert_eq!(n.get_refs(&namespace), vec![set1, set2]);

    assert_eq!(n.get_declaration(&namespace), Some(set1));
    assert_eq!(n.get_global_sets(&namespace), 2);

    n.remove_ref(&mut namespace, c, set1);

    // declaration moves to next global assignment when first is removed
    assert_eq!(n.get_declaration(&namespace), Some(set2));
    assert_eq!(n.get_global_sets(&namespace), 1);
    assert_eq!(n.get_refs(&namespace), vec![set2]);
}

// port: GlobalNamespaceTest#testReferencesToUndefinedRootName
#[test]
fn test_references_to_undefined_root_name() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("a; a.b = 0; a.b; a?.b");
    assert_eq!(t.slot(&mut namespace, "a"), None);
    assert_eq!(t.slot(&mut namespace, "a.b"), None);
}

// port: GlobalNamespaceTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("var a = a ?? {};");
    let a = t.slot(&mut namespace, "a");

    let a = a.expect("a");
    assert_ref_count(&namespace, a, 2);
    assert_eq!(a.get_local_sets(&namespace), 0);
    assert_eq!(a.get_global_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#logicalAssignment1
#[test]
fn logical_assignment1() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("var a = a ||= {};");
    let a = t.slot(&mut namespace, "a");

    let a = a.expect("a");
    assert_ref_count(&namespace, a, 2);
    assert_eq!(a.get_local_sets(&namespace), 0);
    assert_eq!(a.get_global_sets(&namespace), 2);
}

// port: GlobalNamespaceTest#logicalAssignment2
#[test]
fn logical_assignment2() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("var a = a || (a = {});");
    let a = t.slot(&mut namespace, "a");

    let a = a.expect("a");
    assert_ref_count(&namespace, a, 3);
    assert_eq!(a.get_local_sets(&namespace), 0);
    assert_eq!(a.get_global_sets(&namespace), 2);
}

/// The three `getGlobalSets`/`getAliasingGets`/`getTotalGets` checks the logical-assignment
/// gets tests make for one name.
fn assert_sets_and_gets(
    gn: &GlobalNamespace,
    name: Name,
    global_sets: i32,
    aliasing_gets: i32,
    total_gets: i32,
) {
    assert_eq!(name.get_global_sets(gn), global_sets);
    assert_eq!(name.get_aliasing_gets(gn), aliasing_gets);
    assert_eq!(name.get_total_gets(gn), total_gets);
}

// port: GlobalNamespaceTest#testlogicalAssignmentGets1
#[test]
fn testlogical_assignment_gets1() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "const ns = {};
ns.n1 = 1;
ns.n2 = 2;
ns.n1 ??= ns.n2;
ns.n1 &&= ns.n2;
",
    );

    let bar = t.slot(&mut namespace, "ns").unwrap();
    assert_sets_and_gets(&namespace, bar, 1, 0, 0);

    let n1 = t.slot(&mut namespace, "ns.n1").unwrap();
    assert_sets_and_gets(&namespace, n1, 3, 0, 0);

    let n2 = t.slot(&mut namespace, "ns.n2").unwrap();
    assert_sets_and_gets(&namespace, n2, 1, 2, 2);
}

// port: GlobalNamespaceTest#testlogicalAssignmentGets2
#[test]
fn testlogical_assignment_gets2() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "const ns = {};
ns.n1 = 1;
ns.n2 = 2;
ns.n1 ?? (ns.n1 = ns.n2);
ns.n1 && (ns.n1 = ns.n2);
",
    );

    let bar = t.slot(&mut namespace, "ns").unwrap();
    assert_sets_and_gets(&namespace, bar, 1, 0, 0);

    let n1 = t.slot(&mut namespace, "ns.n1").unwrap();
    assert_sets_and_gets(&namespace, n1, 3, 2, 4);

    let n2 = t.slot(&mut namespace, "ns.n2").unwrap();
    assert_sets_and_gets(&namespace, n2, 1, 2, 2);
}

// port: GlobalNamespaceTest#detectsPropertySetsInLogicalAssignmentOperators1
#[test]
fn detects_property_sets_in_logical_assignment_operators1() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const a = {b: 0}; a.b ||= 1; a.b = 2;");

    let ab = t.slot(&mut namespace, "a.b").unwrap();
    assert_eq!(ab.get_global_sets(&namespace), 3);
}

// port: GlobalNamespaceTest#detectsPropertySetsInLogicalAssignmentOperators2
#[test]
fn detects_property_sets_in_logical_assignment_operators2() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const a = {b: 0}; a.b || (a.b = 1); a.b = 2;");

    let ab = t.slot(&mut namespace, "a.b").unwrap();
    assert_eq!(ab.get_global_sets(&namespace), 3);
}

// port: GlobalNamespaceTest#hook
#[test]
fn hook() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("var a = a ? a : {}");
    let a = t.slot(&mut namespace, "a");

    let a = a.expect("a");
    assert_ref_count(&namespace, a, 3);
    assert_eq!(a.get_local_sets(&namespace), 0);
    assert_eq!(a.get_global_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#localAssignmentWillNotBeConsideredADeclaration
#[test]
fn local_assignment_will_not_be_considered_a_declaration() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("");
    let n = namespace.create_name_for_testing("a");
    let c = t.c();
    let set1_node = IR::name(c, "set1");
    let set1 = n.add_single_ref_for_testing(&mut namespace, c, set1_node, RefType::SET_FROM_GLOBAL);
    let local_set_node = IR::name(c, "localSet");
    let local_set =
        n.add_single_ref_for_testing(&mut namespace, c, local_set_node, RefType::SET_FROM_LOCAL);

    assert_eq!(n.get_refs(&namespace), vec![set1, local_set]);

    assert_eq!(n.get_declaration(&namespace), Some(set1));
    assert_eq!(n.get_global_sets(&namespace), 1);
    assert_eq!(n.get_local_sets(&namespace), 1);

    n.remove_ref(&mut namespace, c, set1);

    // local set will not be used as the declaration
    assert_eq!(n.get_declaration(&namespace), None);
    assert_eq!(n.get_global_sets(&namespace), 0);
}

// port: GlobalNamespaceTest#firstDeclarationJSDocAlwaysWins
#[test]
fn first_declaration_js_doc_always_wins() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "const X = {};
/** @type {symbol} */ // later assignment should win
X.number;
/** @type {number} */ // this is the JSDoc we should use
X.number = 3;
/** @type {string} */
X.number = 'hi';
/** @type {Object} */
X.number;
",
    );
    let name_x = t.own_slot(&mut namespace, "X.number").unwrap();
    let declaration_ref = name_x.get_declaration(&namespace);
    let declaration_ref = declaration_ref.expect("declarationRef");

    // make sure first assignment is considered to be the declaration
    let c = t.c();
    let declaration_node = declaration_ref.get_node(&namespace).unwrap();
    assert!(declaration_node.matches_qualified_name(c, "X.number"));
    let assign_node = declaration_node.get_parent(c).unwrap();
    assert!(assign_node.is_assign(c));
    let value_node = declaration_node.get_next(c).unwrap();
    assert!(value_node.is_number(c));
    assert_eq!(value_node.get_double(c), 3.0);

    // Make sure JSDoc on the first assignment is the JSDoc for the name
    let js_doc_info = name_x.get_jsdoc_info(&namespace).expect("jsDocInfo");
    let js_type_expression = js_doc_info.get_type().expect("jsTypeExpression");
    let (registry, ast) = c.get_type_registry_and_ast();
    let js_type = js_type_expression.evaluate(registry, ast, /* scope= */ None);
    TypeSubject::assert_type(js_type).is_number(registry);
}

// port: GlobalNamespaceTest#withoutAssignmentFirstQnameDeclarationStatementJSDocWins
#[test]
fn without_assignment_first_qname_declaration_statement_js_doc_wins() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "const X = {};
/** @type {string} */
X.number;
/** @type {Object} */
X.number;
",
    );
    let name_x = t.own_slot(&mut namespace, "X.number").unwrap();
    let declaration_ref = name_x.get_declaration(&namespace);
    assert_eq!(declaration_ref, None);

    // Make sure JSDoc on the first assignment is the JSDoc for the name
    let js_doc_info = name_x.get_jsdoc_info(&namespace).expect("jsDocInfo");
    let js_type_expression = js_doc_info.get_type().expect("jsTypeExpression");
    let (registry, ast) = t.c().get_type_registry_and_ast();
    let js_type = js_type_expression.evaluate(registry, ast, /* scope= */ None);
    TypeSubject::assert_type(js_type).is_string(registry);
}

// port: GlobalNamespaceTest#testSimpleSubclassingRefCollection
#[test]
fn test_simple_subclassing_ref_collection() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "class Superclass {}
class Subclass extends Superclass {}
",
    );

    let superclass = t.own_slot(&mut namespace, "Superclass").unwrap();
    assert_ref_count(&namespace, superclass, 2);
    assert_eq!(superclass.get_subclassing_gets(&namespace), 1);
}

// port: GlobalNamespaceTest#testStaticInheritedReferencesDontReferToSuperclass
#[test]
fn test_static_inherited_references_dont_refer_to_superclass() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "class Superclass {
  static staticMethod() {}
}
class Subclass extends Superclass {}
Subclass.staticMethod();
Subclass.staticMethod?.();
Subclass?.staticMethod();
",
    );

    let superclass = t.own_slot(&mut namespace, "Superclass").unwrap();
    assert_eq!(superclass.get_subclassing_gets(&namespace), 1);

    let superclass_static_method = t
        .own_slot(&mut namespace, "Superclass.staticMethod")
        .unwrap();
    assert_ref_count(&namespace, superclass_static_method, 1);
    assert!(
        superclass_static_method
            .get_declaration(&namespace)
            .is_some()
    );

    let subclass_static_method = t.own_slot(&mut namespace, "Subclass.staticMethod").unwrap();
    // 2 references:
    // `Subclass.staticMethod()`
    // `Subclass.staticMethod?.()`
    // `SubClass?.staticMethod()` is a reference to `SubClass`, but not
    // to `SubClass.staticmethod`.
    assert_ref_count(&namespace, subclass_static_method, 2);
    assert_eq!(subclass_static_method.get_declaration(&namespace), None);
    assert_eq!(subclass_static_method.get_call_gets(&namespace), 2);

    let subclass = t.own_slot(&mut namespace, "Subclass").unwrap();
    assert_ref_count(&namespace, subclass, 2);
    // `class Subclass` is the declaration reference
    assert!(subclass.get_declaration(&namespace).is_some());
    // `SubClass?.staticMethod` is an aliasing get on `SubClass`
    assert_eq!(subclass.get_aliasing_gets(&namespace), 1);
}

// port: GlobalNamespaceTest#updateRefNodeRejectsRedundantUpdate
#[test]
fn update_ref_node_rejects_redundant_update() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const A = 3;");

    let name_a = t.own_slot(&mut namespace, "A").unwrap();
    let ref_a = name_a.get_first_ref(&namespace);

    let c = t.c();
    let node = ref_a.get_node(&namespace);
    // IllegalArgumentException
    assert_throws("redundant update to Ref node", || {
        name_a.update_ref_node(&mut namespace, c, ref_a, node)
    });
}

// port: GlobalNamespaceTest#updateRefNodeMovesRefFromOldNodeToNewNode
#[test]
fn update_ref_node_moves_ref_from_old_node_to_new_node() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const A = 3;");

    let name_a = t.own_slot(&mut namespace, "A").unwrap();
    let ref_a = name_a.get_first_ref(&namespace);

    let c = t.c();
    let old_node = ref_a.get_node(&namespace).unwrap();
    let new_node = IR::name(c, "A");

    assert_eq!(name_a.get_ref_for_node(&namespace, old_node), Some(ref_a));

    name_a.update_ref_node(&mut namespace, c, ref_a, Some(new_node));

    assert_eq!(ref_a.get_node(&namespace), Some(new_node));
    assert_eq!(name_a.get_ref_for_node(&namespace, old_node), None);
    assert_eq!(name_a.get_ref_for_node(&namespace, new_node), Some(ref_a));
}

// port: GlobalNamespaceTest#updateRefNodeCanSetNodeToNullButPreventsFurtherUpdates
#[test]
fn update_ref_node_can_set_node_to_null_but_prevents_further_updates() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const A = 3;");

    let name_a = t.own_slot(&mut namespace, "A").unwrap();
    let ref_a = name_a.get_first_ref(&namespace);

    let c = t.c();
    let old_node = ref_a.get_node(&namespace).unwrap();

    assert_eq!(name_a.get_ref_for_node(&namespace, old_node), Some(ref_a));

    name_a.update_ref_node(&mut namespace, c, ref_a, None);

    assert_eq!(ref_a.get_node(&namespace), None);
    assert_eq!(name_a.get_ref_for_node(&namespace, old_node), None);
    // cannot get refs for null: `getRefForNode(null)` takes a NodeId in Rust, so the null
    // argument is unrepresentable (the Java NullPointerException check has no Rust caller).
    // cannot update the node again once it's been set to null
    // IllegalArgumentException
    assert_throws("redundant update to Ref node", || {
        name_a.update_ref_node(&mut namespace, c, ref_a, None)
    });
    // IllegalStateException
    assert_throws("Ref's node is already null", || {
        name_a.update_ref_node(&mut namespace, c, ref_a, Some(old_node))
    });
}

// port: GlobalNamespaceTest#updateRefNodeRejectsNodeWithExistingRefs
#[test]
fn update_ref_node_rejects_node_with_existing_refs() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "const A = 3; // declaration ref
A;
",
    ); // use ref

    let name_a = t.own_slot(&mut namespace, "A").unwrap();
    let declaration_ref = name_a.get_declaration(&namespace).unwrap();
    let use_ref = name_a.get_refs(&namespace)[1]; // use ref is 2nd

    let use_node = use_ref.get_node(&namespace);

    let c = t.c();
    // IllegalArgumentException
    assert_throws("refs already exist", || {
        name_a.update_ref_node(&mut namespace, c, declaration_ref, use_node)
    });
}

/// `let A; const B = A = 3;`: A will have twin refs here.
const TWIN_SOURCE: &str = "let A;
const B = A = 3;
";

// port: GlobalNamespaceTest#confirmTwinsAreCreated
#[test]
fn confirm_twins_are_created() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(TWIN_SOURCE); // A will have twin refs here

    let name_a = t.own_slot(&mut namespace, "A").unwrap();
    // first ref is declaration of A
    let twin_ref = name_a.get_refs(&namespace)[1]; // second is the GET_AND_SET twin

    // confirm that they start as twins
    assert!(twin_ref.is_twin(&namespace));
    assert!(twin_ref.is_set_from_global(&namespace));
    assert!(twin_ref.is_aliasing_get(&namespace));

    let old_node = twin_ref.get_node(&namespace).unwrap();

    // confirm that it is associated with oldNode
    assert_eq!(
        name_a.get_ref_for_node(&namespace, old_node),
        Some(twin_ref)
    );

    // confirm that nameA correctly tracks its aliasingGets and globalSets
    assert_eq!(name_a.get_global_sets(&namespace), 2);
    assert_eq!(name_a.get_aliasing_gets(&namespace), 1);
}

// port: GlobalNamespaceTest#updateRefNodeCanRemoveTwinRefs
#[test]
fn update_ref_node_can_remove_twin_refs() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(TWIN_SOURCE); // A will have twin refs here

    let name_a = t.own_slot(&mut namespace, "A").unwrap();
    // first ref is declaration of A
    let twin_ref = name_a.get_refs(&namespace)[1]; // second is the GET_AND_SET twin

    let old_node = twin_ref.get_node(&namespace).unwrap();

    // move the getTwinRef
    let c = t.c();
    let new_node = IR::name(c, "A");
    name_a.update_ref_node(&mut namespace, c, twin_ref, Some(new_node));

    // see confirmTwinsAreCreated() for verification of the original twin relationship

    // confirm that getTwinRef has been updated
    assert_eq!(twin_ref.get_node(&namespace), Some(new_node));
    assert_eq!(
        name_a.get_ref_for_node(&namespace, new_node),
        Some(twin_ref)
    );
    assert!(twin_ref.is_twin(&namespace));

    // confirm that all references to oldNode are removed.
    assert_eq!(name_a.get_ref_for_node(&namespace, old_node), None);
}

// port: GlobalNamespaceTest#removeTwinRef
#[test]
fn remove_twin_ref() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(TWIN_SOURCE); // A will have twin refs here

    let name_a = t.own_slot(&mut namespace, "A").unwrap();
    // first ref is declaration of A
    let twin_ref = name_a.get_refs(&namespace)[1]; // second is the GET_AND_SET twin

    // see confirmTwinsAreCreated() for verification of the original twin relationship

    let old_node = twin_ref.get_node(&namespace).unwrap();

    // confirm that they are both associated with oldNode
    assert_eq!(
        name_a.get_ref_for_node(&namespace, old_node),
        Some(twin_ref)
    );

    let c = t.c();
    name_a.remove_ref(&mut namespace, c, twin_ref);

    assert!(!name_a.get_refs(&namespace).contains(&twin_ref));
    assert_eq!(name_a.get_ref_for_node(&namespace, old_node), None);
    assert_eq!(name_a.get_aliasing_gets(&namespace), 0);
    assert_eq!(name_a.get_global_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#rescanningExistingNodesDoesNotCreateDuplicateRefs
#[test]
fn rescanning_existing_nodes_does_not_create_duplicate_refs() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("class Foo {} const Bar = Foo; const Baz = Bar;");

    let foo = t.own_slot(&mut namespace, "Foo").unwrap();
    let bar = t.own_slot(&mut namespace, "Bar").unwrap();
    let baz = t.own_slot(&mut namespace, "Baz").unwrap();
    let original_foo_refs: Vec<Ref> = foo.get_refs(&namespace);
    let original_bar_refs: Vec<Ref> = bar.get_refs(&namespace);
    let original_baz_refs: Vec<Ref> = baz.get_refs(&namespace);

    // Rescan all of the nodes for which we got refs as if they were newly added
    let root = t.js_root();
    let mut ast_change_set_builder: IndexSet<AstChange> = IndexSet::new();
    for name in [foo, bar, baz] {
        for r in name.get_refs(&namespace) {
            let node = r.get_node(&namespace).unwrap();
            ast_change_set_builder.insert(t.create_global_ast_change_for_node(root, node));
        }
    }
    namespace.scan_new_nodes(t.c(), &ast_change_set_builder);

    // We should get the same Name objects
    assert_eq!(t.own_slot(&mut namespace, "Foo"), Some(foo));
    assert_eq!(t.own_slot(&mut namespace, "Bar"), Some(bar));
    assert_eq!(t.own_slot(&mut namespace, "Baz"), Some(baz));

    // ...and they should contain the same refs with no duplicates added
    assert_eq!(foo.get_refs(&namespace), original_foo_refs);
    assert_eq!(bar.get_refs(&namespace), original_bar_refs);
    assert_eq!(baz.get_refs(&namespace), original_baz_refs);
}

// port: GlobalNamespaceTest#testScanFromNodeDoesntDuplicateVarDeclarationSets
#[test]
fn test_scan_from_node_doesnt_duplicate_var_declaration_sets() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("class Foo {} const Bar = Foo; const Baz = Bar;");

    let foo = t.own_slot(&mut namespace, "Foo").unwrap();
    assert_eq!(foo.get_aliasing_gets(&namespace), 1);
    let baz = t.own_slot(&mut namespace, "Baz").unwrap();
    assert_eq!(baz.get_global_sets(&namespace), 1);

    // Replace "const Baz = Bar" with "const Baz = Foo"
    let root = t.js_root();
    let c = t.c();
    let bar_ref = root
        .get_first_child(c)
        .unwrap()
        .get_last_child(c)
        .unwrap()
        .get_first_first_child(c)
        .unwrap();
    assert!(bar_ref.get_string(c) == "Bar", "{}", bar_ref.to_string(c));
    let foo_name = IR::name(c, "Foo");
    bar_ref.replace_with(c, foo_name);

    // Rescan the new nodes
    let change = t.create_global_ast_change_for_node(root, foo_name);
    namespace.scan_new_nodes(t.c(), &IndexSet::from([change]));

    assert_eq!(foo.get_aliasing_gets(&namespace), 2);
    // A bug in scanFromNode used to make this `2`
    assert_eq!(baz.get_global_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#testScanFromNodeAddsReferenceToParentGetprop
#[test]
fn test_scan_from_node_adds_reference_to_parent_getprop() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const x = {bar: 0}; const y = x; const baz = y.bar;");

    let xbar = t.own_slot(&mut namespace, "x.bar").unwrap();
    assert_eq!(xbar.get_aliasing_gets(&namespace), 0);
    let baz = t.own_slot(&mut namespace, "baz").unwrap();
    assert_eq!(baz.get_global_sets(&namespace), 1);

    // Replace "const baz = y.bar" with "const baz = x.bar"
    let root = t.js_root();
    let c = t.c();
    let y_ref = root
        .get_first_child(c)
        .unwrap()
        .get_last_child(c)
        .unwrap()
        .get_first_first_child(c)
        .unwrap()
        .get_first_child(c)
        .unwrap();
    assert!(y_ref.get_string(c) == "y", "{}", y_ref.to_string(c));
    let x_name = IR::name(c, "x");
    y_ref.replace_with(c, x_name);

    // Rescan the new nodes
    let change = t.create_global_ast_change_for_node(root, x_name);
    namespace.scan_new_nodes(t.c(), &IndexSet::from([change]));

    assert_eq!(xbar.get_aliasing_gets(&namespace), 1);
    assert_eq!(baz.get_global_sets(&namespace), 1);
    let x_bar_get = xbar
        .get_refs(&namespace)
        .into_iter()
        .find(|r| r.is_aliasing_get(&namespace))
        .unwrap();
    let c = t.c();
    assert_eq!(x_bar_get.get_node(&namespace), x_name.get_parent(c));
    assert!(x_bar_get.is_aliasing_get(&namespace));
    assert_eq!(
        x_bar_get.get_chunk(&namespace),
        xbar.get_declaration(&namespace)
            .unwrap()
            .get_chunk(&namespace)
    );
}

// port: GlobalNamespaceTest#testScanFromNodeNoticesHasOwnProperty
#[test]
fn test_scan_from_node_notices_has_own_property() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const x = {bar: 0}; const y = x; y.hasOwnProperty('bar');");

    let x_name = t.own_slot(&mut namespace, "x").unwrap();
    let y_name = t.own_slot(&mut namespace, "y").unwrap();
    assert!(!x_name.uses_has_own_property(&namespace));
    assert!(y_name.uses_has_own_property(&namespace));

    // Replace "const baz = y.bar" with "const baz = x.bar"
    let root = t.js_root();
    let c = t.c();
    let y_dot_has_own_property = root
        .get_first_child(c) // SCRIPT
        .unwrap()
        .get_last_child(c) // EXPR_RESULT `y.hasOwnProperty('bar');`
        .unwrap()
        .get_first_first_child(c) // `y.hasOwnProperty`
        .unwrap();
    let y_node = y_dot_has_own_property.get_first_child(c).unwrap(); // `y`
    assert!(y_dot_has_own_property.matches_qualified_name(c, "y.hasOwnProperty"));
    let x_node = IR::name(c, "x");
    y_node.replace_with(c, x_node);

    // Rescan the new nodes
    // In this case the new node is `x.hasOwnProperty`, since that's the full, new qualified name.
    let change = t.create_global_ast_change_for_node(root, y_dot_has_own_property);
    namespace.scan_new_nodes(t.c(), &IndexSet::from([change]));

    assert!(x_name.uses_has_own_property(&namespace));
}

/// `/** @constructor */ function Bar() {} use(Bar);`
const ESCAPED_CONSTRUCTOR: &str = "/** @constructor */
function Bar() {}
use(Bar);
";

// port: GlobalNamespaceTest#testCollapsing_forEscapedConstructor_ignoringStaticInheritance
#[test]
fn test_collapsing_for_escaped_constructor_ignoring_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(ESCAPED_CONSTRUCTOR);

    let bar = t.slot(&mut namespace, "Bar").unwrap();
    let c = t.c();
    assert!(bar.can_collapse(&namespace, c)); // trivially true, already collapsed
    // we collapse properties of Bar even though it's escaped, intentionally unsafe.
    // this is mostly to support minification for goog.provide namespaces containing @constructors
    assert!(bar.can_collapse_unannotated_child_names(&namespace, c));
}

// port: GlobalNamespaceTest#testCollapsing_forEscapedConstructor_consideringStaticInheritance
#[test]
fn test_collapsing_for_escaped_constructor_considering_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    t.assume_static_inheritance_is_not_used = false;
    let mut namespace = t.parse(ESCAPED_CONSTRUCTOR);

    let bar = t.slot(&mut namespace, "Bar").unwrap();
    let c = t.c();
    assert!(bar.can_collapse(&namespace, c)); // trivially true, already collapsed
    assert!(!bar.can_collapse_unannotated_child_names(&namespace, c));
}

/// The source of the two testInlinability_forAliasingPropertyOnEscapedConstructor tests.
const ALIASING_PROPERTY_ON_ESCAPED_CONSTRUCTOR: &str = "var prop = 1;
/** @constructor */
var Foo = function() {}

Foo.prop = prop;

/** @constructor */
function Bar() {}
Bar.aliasOfFoo = Foo; // alias Foo
use(Bar); // uninlinable alias of Bar
const BarAlias = Bar; // inlinable alias of Bar
alert(Bar.aliasOfFoo.prop);
alert(BarAlias.aliasOfFoo.prop);
";

// port: GlobalNamespaceTest#testInlinability_forAliasingPropertyOnEscapedConstructor_ignoringStaticInheritance
#[test]
fn test_inlinability_for_aliasing_property_on_escaped_constructor_ignoring_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(ALIASING_PROPERTY_ON_ESCAPED_CONSTRUCTOR);

    let bar_alias_of_foo = t.slot(&mut namespace, "Bar.aliasOfFoo").unwrap();
    let bar_alias_inlinability = bar_alias_of_foo.calculate_inlinability(&namespace, t.c());

    // We should convert references to `Bar.aliasOfFoo.prop` to become `Foo.prop`
    // because...
    assert!(bar_alias_inlinability.should_inline_usages());
    // However, we should not remove the assignment (`Bar.aliasOfFoo = Foo`) that creates the
    // alias, because "BarAlias" still needs to be inlined to "Bar", which will create another
    // usage of "Bar.aliasOfFoo" in the last line, We will locate the value to inline
    // Bar.aliasOfFoo again from `Bar.aliasOfFoo = Foo`.
    assert!(!bar_alias_inlinability.should_remove_declaration());
}

// port: GlobalNamespaceTest#testInlinability_forAliasingPropertyOnEscapedConstructor_consideringStaticInheritance
#[test]
fn test_inlinability_for_aliasing_property_on_escaped_constructor_considering_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    t.assume_static_inheritance_is_not_used = false;
    let mut namespace = t.parse(ALIASING_PROPERTY_ON_ESCAPED_CONSTRUCTOR);

    let bar_alias_of_foo = t.slot(&mut namespace, "Bar.aliasOfFoo").unwrap();
    let bar_alias_inlinability = bar_alias_of_foo.calculate_inlinability(&namespace, t.c());

    assert!(!bar_alias_inlinability.should_inline_usages());
    assert!(!bar_alias_inlinability.should_remove_declaration());
}

// port: GlobalNamespaceTest#testClassPrototypeProp
#[test]
fn test_class_prototype_prop() {
    let mut t = GlobalNamespaceTest::new();
    let mut ns = t.parse("class C { x() {} }");

    assert_eq!(t.slot(&mut ns, "C.x"), None);
}

/// The checks the class-static-field tests make on `C` and `C.x`.
fn assert_c_and_c_dot_x(ns: &GlobalNamespace, c: Name, c_dot_x: Name) {
    assert_eq!(c.get_global_sets(ns), 1);
    assert_eq!(c.props(ns).unwrap_or_default(), &[c_dot_x]);

    assert_eq!(c_dot_x.get_global_sets(ns), 1);
    assert_eq!(c_dot_x.get_parent(ns), Some(c));
}

// port: GlobalNamespaceTest#testClassStaticField_withInitializer
#[test]
fn test_class_static_field_with_initializer() {
    let mut t = GlobalNamespaceTest::new();
    let mut ns = t.parse(
        "class C {
  static x = 1;
}
",
    );

    let c = t.slot(&mut ns, "C").unwrap();
    let c_dot_x = t.slot(&mut ns, "C.x").unwrap();

    assert_c_and_c_dot_x(&ns, c, c_dot_x);
    assert!(c_dot_x.can_collapse(&ns, t.c()));
}

// port: GlobalNamespaceTest#testClassStaticField_withoutInitializer
#[test]
fn test_class_static_field_without_initializer() {
    let mut t = GlobalNamespaceTest::new();
    let mut ns = t.parse(
        "class C {
  /** @type {number} */
  static x;
}
",
    );

    let c = t.slot(&mut ns, "C").unwrap();
    let c_dot_x = t.slot(&mut ns, "C.x").unwrap();

    assert_c_and_c_dot_x(&ns, c, c_dot_x);
    assert!(c_dot_x.can_collapse(&ns, t.c()));

    let js_doc_info = c_dot_x.get_jsdoc_info(&ns).expect("jsDocInfo");
    let js_type_expression = js_doc_info.get_type().expect("jsTypeExpression");
    let (registry, ast) = t.c().get_type_registry_and_ast();
    let js_type = js_type_expression.evaluate(registry, ast, /* scope= */ None);
    TypeSubject::assert_type(js_type).is_number(registry);
}

// port: GlobalNamespaceTest#testClassStaticField_withSuper
#[test]
fn test_class_static_field_with_super() {
    let mut t = GlobalNamespaceTest::new();
    t.assume_static_inheritance_is_not_used = false;
    let mut ns = t.parse(
        "class A {
  static y = 1;
}
class C extends A {
  static x = super.y;
}
",
    );

    let c_dot_x = t.slot(&mut ns, "C.x").unwrap();
    assert!(!c_dot_x.can_collapse(&ns, t.c()));
}

// port: GlobalNamespaceTest#testClassStaticAndPrototypePropWithSameName
#[test]
fn test_class_static_and_prototype_prop_with_same_name() {
    let mut t = GlobalNamespaceTest::new();
    let mut ns = t.parse("class C { x() {} static x() {} }");

    let c = t.slot(&mut ns, "C").unwrap();
    let c_dot_x = t.slot(&mut ns, "C.x").unwrap();

    assert_c_and_c_dot_x(&ns, c, c_dot_x);
}

// port: GlobalNamespaceTest#testLocalVarsDefinedinStaticBlocks
#[test]
fn test_local_vars_definedin_static_blocks() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("class C{ static{ var x; }}");
    assert_eq!(t.slot(&mut namespace, "x"), None);
}

// port: GlobalNamespaceTest#testAddPropertytoGlobalObjectinClassStaticBlock
#[test]
fn test_add_propertyto_global_objectin_class_static_block() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const a = {}; class C{ static { a.b = 1;}}");

    let a = t.slot(&mut namespace, "a").unwrap();

    assert_eq!(a.get_global_sets(&namespace), 1);
    assert_eq!(a.get_total_sets(&namespace), 1);

    let ab = t.slot(&mut namespace, "a.b").unwrap();

    assert_eq!(ab.get_parent(&namespace), Some(a));
    assert_eq!(ab.get_global_sets(&namespace), 0);
    assert_eq!(ab.get_local_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#testDirectGets
#[test]
fn test_direct_gets() {
    let mut t = GlobalNamespaceTest::new();
    // None of the symbol uses here should be considered aliasing gets.
    let mut namespace = t.parse(
        "const ns = {};
ns.n1 = 1;
ns.n2 = 2;
ns.n1 === ns.n2;
ns.n1 == ns.n2;
ns.n1 !== ns.n2;
ns.n1 != ns.n2;
ns.n1 <  ns.n2;
ns.n1 <= ns.n2;
ns.n1 >  ns.n2;
ns.n1 >= ns.n2;
ns.n1 + ns.n2;
ns.n1 - ns.n2;
ns.n1 * ns.n2;
ns.n1 / ns.n2;
ns.n1 % ns.n2;
ns.n1 ** ns.n2;
ns.n1 & ns.n2;
ns.n1 | ns.n2;
ns.n1 ^ ns.n2;
ns.n1 << ns.n2;
ns.n1 >> ns.n2;
ns.n1 >>> ns.n2;
ns.n1 && ns.n2;
ns.n1 || ns.n2;
",
    );

    let bar = t.slot(&mut namespace, "ns").unwrap();
    // getting `ns.n1` doesn't count as a get on `ns`
    assert_sets_and_gets(&namespace, bar, 1, 0, 0);

    let n1 = t.slot(&mut namespace, "ns.n1").unwrap();
    assert_sets_and_gets(&namespace, n1, 1, 0, 22);

    let n2 = t.slot(&mut namespace, "ns.n2").unwrap();
    assert_sets_and_gets(&namespace, n2, 1, 0, 22);
}

// port: GlobalNamespaceTest#testObjectPatternAliasInDeclaration
#[test]
fn test_object_pattern_alias_in_declaration() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: 3}; const {a: b} = ns;");

    let bar = t.slot(&mut namespace, "ns").unwrap();
    assert_sets_and_gets(&namespace, bar, 1, 1, 1);

    let ns_a = t.slot(&mut namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(&namespace), 1);
    assert_eq!(ns_a.get_aliasing_gets(&namespace), 1);

    let b = t.slot(&mut namespace, "b").unwrap();
    assert_eq!(b.get_global_sets(&namespace), 1);
    assert_eq!(b.get_total_gets(&namespace), 0);
}

// port: GlobalNamespaceTest#testConditionalDestructuringDoesNotHideAliasingGet
#[test]
fn test_conditional_destructuring_does_not_hide_aliasing_get() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "const ns1 = {a: 3};
const ns2 = {b: 3};
// Creates an aliasing get for both ns1 and ns2
const {a, b} = Math.random() ? ns1 : ns2;
",
    );

    let ns1 = t.slot(&mut namespace, "ns1").unwrap();
    assert_eq!(ns1.get_aliasing_gets(&namespace), 1);
    assert_eq!(ns1.get_total_gets(&namespace), 1);

    let ns2 = t.slot(&mut namespace, "ns2").unwrap();
    assert_eq!(ns2.get_aliasing_gets(&namespace), 1);
    assert_eq!(ns2.get_total_gets(&namespace), 1);
}

// port: GlobalNamespaceTest#testNestedObjectPatternAliasInDeclaration
#[test]
fn test_nested_object_pattern_alias_in_declaration() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: {b: 3}}; const {a: {b}} = ns;");

    let bar = t.slot(&mut namespace, "ns").unwrap();
    assert_eq!(bar.get_global_sets(&namespace), 1);
    assert_eq!(bar.get_aliasing_gets(&namespace), 1);

    // we treat ns.a as having an 'aliasing' get since we don't traverse into the nested pattern
    let ns_a = t.slot(&mut namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(&namespace), 1);
    assert_eq!(ns_a.get_aliasing_gets(&namespace), 1);

    let ns_ab = t.slot(&mut namespace, "ns.a.b").unwrap();
    assert_eq!(ns_ab.get_global_sets(&namespace), 1);
    // we don't consider this an 'aliasing get' because it's in a nested pattern
    assert_eq!(ns_ab.get_aliasing_gets(&namespace), 0);

    let b = t.slot(&mut namespace, "b").unwrap();
    assert_eq!(b.get_global_sets(&namespace), 1);
    assert_eq!(b.get_total_gets(&namespace), 0);
}

// port: GlobalNamespaceTest#testObjectPatternAliasInAssign
#[test]
fn test_object_pattern_alias_in_assign() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: 3}; const x = {}; ({a: x.y} = ns);");

    let bar = t.slot(&mut namespace, "ns").unwrap();
    assert_eq!(bar.get_global_sets(&namespace), 1);
    assert_eq!(bar.get_aliasing_gets(&namespace), 1);

    let ns_a = t.slot(&mut namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(&namespace), 1);
    assert_eq!(ns_a.get_aliasing_gets(&namespace), 1);

    let x_y = t.slot(&mut namespace, "x.y").unwrap();
    // TODO(b/117673791): this should be 1
    assert_eq!(x_y.get_global_sets(&namespace), 0);
}

// port: GlobalNamespaceTest#testObjectPatternRestInDeclaration
#[test]
fn test_object_pattern_rest_in_declaration() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: 3}; const {a, ...b} = ns;");

    let ns = t.slot(&mut namespace, "ns").unwrap();
    assert_eq!(ns.get_global_sets(&namespace), 1);
    assert_eq!(ns.get_total_gets(&namespace), 1);
    assert_eq!(ns.get_aliasing_gets(&namespace), 1);

    let ns_a = t.slot(&mut namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(&namespace), 1);
    assert_eq!(ns_a.get_total_gets(&namespace), 1);
    assert_eq!(ns_a.get_aliasing_gets(&namespace), 1);

    let b = t.slot(&mut namespace, "b").unwrap();
    assert_eq!(b.get_global_sets(&namespace), 1);
    assert_eq!(b.get_total_gets(&namespace), 0);
}

// port: GlobalNamespaceTest#testObjectPatternRestNestedInDeclaration
#[test]
fn test_object_pattern_rest_nested_in_declaration() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: 3, b: {}}; const {a, b: {...c}} = ns;");

    let ns = t.slot(&mut namespace, "ns").unwrap();
    assert_eq!(ns.get_global_sets(&namespace), 1);
    assert_eq!(ns.get_total_gets(&namespace), 1);
    assert_eq!(ns.get_aliasing_gets(&namespace), 1);

    let ns_b = t.slot(&mut namespace, "ns.b").unwrap();
    assert_eq!(ns_b.get_global_sets(&namespace), 1);
    assert_eq!(ns_b.get_total_gets(&namespace), 1);
    assert_eq!(ns_b.get_aliasing_gets(&namespace), 1);
}

/// The `ns`, `ns.a` and `x.y` checks of the tests that assign a pattern onto `x.y`.
fn assert_ns_alias_and_x_y(
    t: &mut GlobalNamespaceTest,
    namespace: &mut GlobalNamespace,
    ns_a_aliasing_gets: i32,
) {
    let ns = t.slot(namespace, "ns").unwrap();
    assert_eq!(ns.get_global_sets(namespace), 1);
    assert_eq!(ns.get_aliasing_gets(namespace), 1);

    let ns_a = t.slot(namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(namespace), 1);
    assert_eq!(ns_a.get_aliasing_gets(namespace), ns_a_aliasing_gets);

    let x_y = t.slot(namespace, "x.y").unwrap();
    // TODO(b/117673791): this should be 1
    assert_eq!(x_y.get_global_sets(namespace), 0);
}

// port: GlobalNamespaceTest#testObjectPatternRestAliasInAssign
#[test]
fn test_object_pattern_rest_alias_in_assign() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: 3}; const x = {}; ({a, ...x.y} = ns);");

    assert_ns_alias_and_x_y(&mut t, &mut namespace, 1);
}

// port: GlobalNamespaceTest#testObjectPatternAliasInForOf
#[test]
fn test_object_pattern_alias_in_for_of() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: 3}; for (const {a: b} of [ns]) {}");

    let bar = t.slot(&mut namespace, "ns").unwrap();
    assert_eq!(bar.get_global_sets(&namespace), 1);
    assert_eq!(bar.get_aliasing_gets(&namespace), 1);

    // GlobalNamespace ignores for-of and array literals, not realizing that `b` reads `ns.a`
    let ns_a = t.slot(&mut namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(&namespace), 1);
    assert_eq!(ns_a.get_aliasing_gets(&namespace), 0);
}

// port: GlobalNamespaceTest#testObjectLitSpreadAliasInDeclaration
#[test]
fn test_object_lit_spread_alias_in_declaration() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: 3}; const {a} = {...ns};");

    let ns = t.slot(&mut namespace, "ns").unwrap();
    assert_eq!(ns.get_global_sets(&namespace), 1);
    assert_eq!(ns.get_aliasing_gets(&namespace), 1);

    let ns_a = t.slot(&mut namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(&namespace), 1);
    assert_eq!(ns_a.get_aliasing_gets(&namespace), 0);

    let a = t.slot(&mut namespace, "a").unwrap();
    assert_eq!(a.get_global_sets(&namespace), 1);
}

/// The `obj`, `obj.a`, `ns` and `ns.a` checks of the tests that assign a copy of `obj` to `ns.a`.
fn assert_obj_copied_onto_ns_a(
    t: &mut GlobalNamespaceTest,
    namespace: &mut GlobalNamespace,
) -> Name {
    let obj = t.slot(namespace, "obj").unwrap();
    assert_eq!(obj.get_global_sets(namespace), 1);
    assert_eq!(obj.get_aliasing_gets(namespace), 1);

    let obj_a = t.slot(namespace, "obj.a").unwrap();
    assert_eq!(obj_a.get_global_sets(namespace), 1);
    assert_eq!(obj_a.get_aliasing_gets(namespace), 0);

    let ns = t.slot(namespace, "ns").unwrap();
    assert_eq!(ns.get_global_sets(namespace), 1);
    assert_eq!(ns.get_aliasing_gets(namespace), 0);

    let ns_a = t.slot(namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(namespace), 1);
    assert_eq!(ns_a.get_aliasing_gets(namespace), 0);
    ns_a
}

// port: GlobalNamespaceTest#testObjectAssignOntoAGetProp
#[test]
fn test_object_assign_onto_a_get_prop() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace =
        t.parse("const obj = {a:3}; const ns = {}; ns.a = Object.assign({}, obj); ");

    let ns_a = assert_obj_copied_onto_ns_a(&mut t, &mut namespace);
    assert!(!ns_a.is_object_literal(&namespace)); // `ns.a` is considered an "OTHER" type
}

// port: GlobalNamespaceTest#testObjectLitSpreadOntoAGetProp
#[test]
fn test_object_lit_spread_onto_a_get_prop() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const obj = {a:3}; const ns = {}; ns.a = {...obj}");

    let ns_a = assert_obj_copied_onto_ns_a(&mut t, &mut namespace);
    assert!(ns_a.is_object_literal(&namespace)); // `ns.a` is an "OBJECTLIT" type despite containing spread.
}

// port: GlobalNamespaceTest#testObjectLitSpreadAliasInAssign
#[test]
fn test_object_lit_spread_alias_in_assign() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("const ns = {a: 3}; const x = {}; ({a: x.y} = {...ns});");

    assert_ns_alias_and_x_y(&mut t, &mut namespace, 0);
}

/// The checks of the two LHS-cast tests.
fn assert_lhs_cast(t: &mut GlobalNamespaceTest, namespace: &mut GlobalNamespace) {
    let ns = t.slot(namespace, "ns").unwrap();
    assert_eq!(ns.get_global_sets(namespace), 1);
    assert_eq!(ns.get_total_gets(namespace), 0);

    let ns_a = t.slot(namespace, "ns.a").unwrap();
    assert_eq!(ns_a.get_global_sets(namespace), 0); // TODO(b/127505242): Should be 1.
    assert_eq!(ns_a.get_total_gets(namespace), 1); // TODO(b/127505242): Should be 0.

    let b = t.slot(namespace, "b").unwrap();
    assert_eq!(b.get_global_sets(namespace), 1);
    assert_eq!(b.get_aliasing_gets(namespace), 1);
}

// port: GlobalNamespaceTest#testLhsCastInAssignment
#[test]
fn test_lhs_cast_in_assignment() {
    let mut t = GlobalNamespaceTest::new();
    // The type of the cast doesn't matter.
    // Casting is only legal JS syntax in simple assignments, not with destructuring or declaration.
    let mut namespace = t.parse("const ns = {}; const b = 5; /** @type {*} */ (ns.a) = b;");

    assert_lhs_cast(&mut t, &mut namespace);
}

// port: GlobalNamespaceTest#testDoubleLhsCastInAssignment_doesNotCrash
#[test]
fn test_double_lhs_cast_in_assignment_does_not_crash() {
    let mut t = GlobalNamespaceTest::new();
    // The type of the cast doesn't matter.
    // Casting is only legal JS syntax in simple assignments, not with destructuring or declaration.
    let mut namespace = t.parse(
        "const ns = {};
 const b = 5;
 /** @type {*} */ (/** @type {*} */ (ns.a)) = b;
",
    );

    assert_lhs_cast(&mut t, &mut namespace);
}

/// `namespace.getSlot(name).canCollapse()` for a fresh `parse(js)`.
fn can_collapse(t: &mut GlobalNamespaceTest, js: &str, name: &str) -> bool {
    let mut namespace = t.parse(js);
    let n = t.slot(&mut namespace, name).unwrap();
    n.can_collapse(&namespace, t.c())
}

// port: GlobalNamespaceTest#testCannotCollapseAliasedObjectLitProperty
#[test]
fn test_cannot_collapse_aliased_object_lit_property() {
    let mut t = GlobalNamespaceTest::new();
    // We should not convert foo.prop -> foo$prop because use(foo) might read foo.prop
    assert!(!can_collapse(
        &mut t,
        "var foo = {prop: 0}; use(foo);",
        "foo.prop"
    ));
}

// port: GlobalNamespaceTest#testCannotCollapseConditionalObjectLitProperty
#[test]
fn test_cannot_collapse_conditional_object_lit_property() {
    let mut t = GlobalNamespaceTest::new();
    // We should not convert foo.prop -> foo$prop because use(foo) might read foo.prop
    assert!(!can_collapse(
        &mut t,
        "var foo = x || {prop: 0}; use(foo.prop);",
        "foo.prop"
    ));
}

// port: GlobalNamespaceTest#testCannotCollapseConditionalObjectLitNestedProperty
#[test]
fn test_cannot_collapse_conditional_object_lit_nested_property() {
    let mut t = GlobalNamespaceTest::new();
    // We should not convert foo.prop -> foo$prop because use(foo) might read foo.prop
    assert!(!can_collapse(
        &mut t,
        "var foo = x || {prop: {nested: 0}}; use(foo.prop.nested);",
        "foo.prop.nested"
    ));
}

// port: GlobalNamespaceTest#testCannotCollapseConditionalAndObjectLitProperty
#[test]
fn test_cannot_collapse_conditional_and_object_lit_property() {
    let mut t = GlobalNamespaceTest::new();
    // We should not convert foo.prop -> foo$prop because use(foo) might read foo.prop
    assert!(!can_collapse(
        &mut t,
        "var foo = x && {prop: 0}; use(foo.prop);",
        "foo.prop"
    ));
}

// port: GlobalNamespaceTest#testCannotCollapseConditionalAndObjectLitNestedProperty
#[test]
fn test_cannot_collapse_conditional_and_object_lit_nested_property() {
    let mut t = GlobalNamespaceTest::new();
    // We should not convert foo.prop -> foo$prop because use(foo) might read foo.prop
    assert!(!can_collapse(
        &mut t,
        "var foo = x && {prop: {nested: 0}}; use(foo.prop.nested);",
        "foo.prop.nested"
    ));
}

/// `X` and `X.Y` can collapse neither (testGitHubIssue3733, testThrowPreventsCollapsingChildNames).
fn assert_x_and_x_y_cannot_collapse(t: &mut GlobalNamespaceTest, js: &str) {
    let mut namespace = t.parse(js);

    let name_x = t.slot(&mut namespace, "X").unwrap();
    assert!(!name_x.can_collapse_unannotated_child_names(&namespace, t.c()));

    let prop_y = t.slot(&mut namespace, "X.Y").unwrap();
    assert!(!prop_y.can_collapse(&namespace, t.c()));
}

// port: GlobalNamespaceTest#testGitHubIssue3733
#[test]
fn test_git_hub_issue3733() {
    let mut t = GlobalNamespaceTest::new();
    assert_x_and_x_y_cannot_collapse(
        &mut t,
        "const X = {Y: 1};

function fn(a) {
  if (a) {
// Before issue #3733 was fixed GlobalNamespace failed to see this reference
// as creating an alias for X due to a switch statement that failed to check
// for the RETURN node type, so X.Y was incorrectly collapsed.
    return a ? X : {};
  }
}

console.log(fn(true).Y);
",
    );
}

// port: GlobalNamespaceTest#testThrowPreventsCollapsingChildNames
#[test]
fn test_throw_prevents_collapsing_child_names() {
    let mut t = GlobalNamespaceTest::new();
    assert_x_and_x_y_cannot_collapse(
        &mut t,
        "const X = {Y: 1};

function fn(a) {
// This is specifically testing a bugfix closely related to GitHub issue
// #3733. A quirk of the implementation hides the bug when the throw isn't
// inside an if statement or the thrown value isn't a conditional expression.
  if (a) {
    throw a ? X : {};
  }
}

console.log(fn(true).Y);
",
    );
}

// port: GlobalNamespaceTest#testCannotCollapseObjectLitPropertyEscapedWithOptChainCall
#[test]
fn test_cannot_collapse_object_lit_property_escaped_with_opt_chain_call() {
    let mut t = GlobalNamespaceTest::new();
    // We should not convert foo.prop -> foo$prop because use(foo) might read foo.prop
    assert!(!can_collapse(
        &mut t,
        "var foo = {prop: 0}; use?.(foo);",
        "foo.prop"
    ));
}

/// `/** @constructor */ var Foo = function() {} Foo.prop = prop; use(Foo);`
const ALIASED_CONSTRUCTOR: &str = "/** @constructor */
var Foo = function() {}

Foo.prop = prop;
use(Foo);
";

/// `/** @interface */ var Foo = function() {} Foo.prop = prop; use(Foo);`
const ALIASED_INTERFACE: &str = "/** @interface */
var Foo = function() {}

Foo.prop = prop;
use(Foo);
";

// port: GlobalNamespaceTest#testCanCollapseAliasedConstructorProperty_ignoringStaticInheritance
#[test]
fn test_can_collapse_aliased_constructor_property_ignoring_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    // We should still convert Foo.prop -> Foo$prop, even though use(Foo) might read Foo.prop,
    // because Foo is a constructor
    assert!(can_collapse(&mut t, ALIASED_CONSTRUCTOR, "Foo.prop"));
}

// port: GlobalNamespaceTest#testCannotCollapseAliasedConstructorProperty_consideringStaticInheritance
#[test]
fn test_cannot_collapse_aliased_constructor_property_considering_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    t.assume_static_inheritance_is_not_used = false;
    assert!(!can_collapse(&mut t, ALIASED_CONSTRUCTOR, "Foo.prop"));
}

// port: GlobalNamespaceTest#testCanCollapseAliasedInterfaceProperty_ignoringStaticInheritance
#[test]
fn test_can_collapse_aliased_interface_property_ignoring_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    // We should still convert Foo.prop -> Foo$prop, even though use(Foo) might read Foo.prop,
    // because Foo is a constructor
    assert!(can_collapse(&mut t, ALIASED_INTERFACE, "Foo.prop"));
}

// port: GlobalNamespaceTest#testCannotCollapseAliasedInterfaceProperty_consideringStaticInheritance
#[test]
fn test_cannot_collapse_aliased_interface_property_considering_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    t.assume_static_inheritance_is_not_used = false;
    assert!(!can_collapse(&mut t, ALIASED_INTERFACE, "Foo.prop"));
}

// port: GlobalNamespaceTest#testCanCollapseAliasedClassProperty_ignoringStaticInheritance
#[test]
fn test_can_collapse_aliased_class_property_ignoring_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    // We should still convert Foo.prop -> Foo$prop, even though use(Foo) might read Foo.prop,
    // because Foo is a constructor
    assert!(can_collapse(
        &mut t,
        "class Foo {} Foo.prop = prop; use(Foo);",
        "Foo.prop"
    ));
}

// port: GlobalNamespaceTest#testCanCollapseAliasedClassProperty_consideringStaticInheritance
#[test]
fn test_can_collapse_aliased_class_property_considering_static_inheritance() {
    let mut t = GlobalNamespaceTest::new();
    t.assume_static_inheritance_is_not_used = false;
    assert!(!can_collapse(
        &mut t,
        "class Foo {} Foo.prop = prop; use(Foo);",
        "Foo.prop"
    ));
}

// port: GlobalNamespaceTest#testCannotCollapseOrInlineDeletedProperty
#[test]
fn test_cannot_collapse_or_inline_deleted_property() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse(
        "const global = window;
delete global.HTMLElement;
global.HTMLElement = (class {});
",
    );

    let deleted_prop = t.slot(&mut namespace, "global.HTMLElement").unwrap();
    assert_eq!(
        deleted_prop.can_collapse_or_inline(&namespace, t.c()),
        Inlinability::DO_NOT_INLINE
    );
}

// port: GlobalNamespaceTest#testCanCollapse_objectLitProperty_declaredBeforeASpread
#[test]
fn test_can_collapse_object_lit_property_declared_before_a_spread() {
    let mut t = GlobalNamespaceTest::new();
    assert!(!can_collapse(
        &mut t,
        "var foo = {prop: 0, ...bar}; use(foo.prop);",
        "foo.prop"
    ));
}

/// `assertNode(name.getDeclaration().getNode().getParent()).hasToken(token)`
fn assert_declaration_parent_token(
    t: &mut GlobalNamespaceTest,
    namespace: &GlobalNamespace,
    name: Name,
    token: Token,
) {
    let c = t.c();
    let node = name
        .get_declaration(namespace)
        .unwrap()
        .get_node(namespace)
        .unwrap();
    assert_eq!(node.get_parent(c).unwrap().get_token(c), token);
}

// port: GlobalNamespaceTest#testGoogProvideName
#[test]
fn test_goog_provide_name() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("goog.provide('a'); var a = {};");

    let a = t.slot(&mut namespace, "a");
    let a = a.expect("a");
    assert_eq!(a.get_global_sets(&namespace), 1);
    // The VAR, not the goog.provide, is considered the 'declaration' of `a`.
    assert_declaration_parent_token(&mut t, &namespace, a, Token::VAR);
}

// port: GlobalNamespaceTest#testGoogProvideNamespace_noExplicitAssignment
#[test]
fn test_goog_provide_namespace_no_explicit_assignment() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("goog.provide('a.b');");

    let a = t.slot(&mut namespace, "a");
    let a = a.expect("a");
    assert_eq!(a.get_global_sets(&namespace), 0);
    let ab = t.slot(&mut namespace, "a.b");
    let ab = ab.expect("a.b");
    assert_eq!(ab.get_global_sets(&namespace), 0);
    assert_eq!(a.get_declaration(&namespace), None);
    assert_eq!(ab.get_declaration(&namespace), None);
    assert_eq!(ab.get_parent(&namespace), Some(a));
}

// port: GlobalNamespaceTest#testGoogProvideLongNamespace
#[test]
fn test_goog_provide_long_namespace() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("goog.provide('a.b.c.d');");

    assert!(t.slot(&mut namespace, "a.b.c.d").is_some());
}

// port: GlobalNamespaceTest#testGoogProvideNamespace_explicitAssignment
#[test]
fn test_goog_provide_namespace_explicit_assignment() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("goog.provide('a.b'); /** @const */ a.b = {};");

    let a = t.slot(&mut namespace, "a");
    let a = a.expect("a");
    assert_eq!(a.get_global_sets(&namespace), 0);
    let ab = t.slot(&mut namespace, "a.b");
    let ab = ab.expect("a.b");
    assert_eq!(ab.get_global_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#testGoogProvideNamespace_assignmentToProperty
#[test]
fn test_goog_provide_namespace_assignment_to_property() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("goog.provide('a.b'); a.b.Class = class {};");

    let ab_class = t.slot(&mut namespace, "a.b.Class");
    let ab_class = ab_class.expect("a.b.Class");
    assert_eq!(ab_class.get_global_sets(&namespace), 1);
    let ab = t.slot(&mut namespace, "a.b");
    assert_eq!(ab_class.get_parent(&namespace), ab);
}

// port: GlobalNamespaceTest#testGoogProvideName_multipleProvidesForName
#[test]
fn test_goog_provide_name_multiple_provides_for_name() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse("goog.provide('a.b'); goog.provide('a.c');");

    let a = t.slot(&mut namespace, "a");
    let a = a.expect("a");
    assert_eq!(a.get_global_sets(&namespace), 0);
}

// port: GlobalNamespaceTest#googModuleLevelNamesAreCaptured
#[test]
fn goog_module_level_names_are_captured() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_and_gather_module_data("goog.module('m'); const x = 0;");
    let metadata = t.module_by_goog_namespace("m");
    let x = t.name_from_module(&mut namespace, &metadata, "x");

    let x = x.expect("x");
    assert!(x.get_declaration(&namespace).is_some());
}

// port: GlobalNamespaceTest#googModuleLevelQualifiedNamesAreCaptured
#[test]
fn goog_module_level_qualified_names_are_captured() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace =
        t.parse_and_gather_module_data("goog.module('m'); class Foo {} Foo.Bar = 0;");
    let metadata = t.module_by_goog_namespace("m");
    let x = t.name_from_module(&mut namespace, &metadata, "Foo.Bar");

    let x = x.expect("Foo.Bar");
    assert!(x.get_declaration(&namespace).is_some());
}

// port: GlobalNamespaceTest#googModule_containsExports
#[test]
fn goog_module_contains_exports() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_and_gather_module_data("goog.module('m'); const x = 0;");
    let metadata = t.module_by_goog_namespace("m");
    let exports = t
        .name_from_module(&mut namespace, &metadata, "exports")
        .unwrap();
    assert_eq!(exports.get_global_sets(&namespace), 0);
}

// port: GlobalNamespaceTest#googLoadModule_containsExports
#[test]
fn goog_load_module_contains_exports() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_and_gather_module_data(
        "goog.loadModule(function(exports) {
  goog.module('m');
  const x = 0;
  return exports;
});
",
    );
    let metadata = t.module_by_goog_namespace("m");
    let exports = t
        .name_from_module(&mut namespace, &metadata, "exports")
        .unwrap();
    assert_eq!(exports.get_global_sets(&namespace), 0);
}

// port: GlobalNamespaceTest#googLoadModule_capturesQualifiedNames
#[test]
fn goog_load_module_captures_qualified_names() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_and_gather_module_data(
        "goog.loadModule(function(exports) {
  goog.module('m');
  class Foo {}
  Foo.Bar = class {};
  return exports;
});
",
    );
    let metadata = t.module_by_goog_namespace("m");
    let foo = t.name_from_module(&mut namespace, &metadata, "Foo");
    let foo_bar = t
        .name_from_module(&mut namespace, &metadata, "Foo.Bar")
        .unwrap();
    assert_eq!(foo_bar.get_parent(&namespace), foo);
}

// port: GlobalNamespaceTest#googLoadModule_containsExportsPropertyAssignments
#[test]
fn goog_load_module_contains_exports_property_assignments() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_and_gather_module_data(
        "goog.loadModule(function(exports) {
  goog.module('m');
  exports.Foo = class {};
  return exports;
});
",
    );
    let metadata = t.module_by_goog_namespace("m");
    let exports_foo = t
        .name_from_module(&mut namespace, &metadata, "exports.Foo")
        .unwrap();
    assert_eq!(exports_foo.get_global_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#googModule_containsExports_explicitAssign
#[test]
fn goog_module_contains_exports_explicit_assign() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace =
        t.parse_and_gather_module_data("goog.module('m'); const x = 0; exports = {x};");
    let metadata = t.module_by_goog_namespace("m");
    let exports = t
        .name_from_module(&mut namespace, &metadata, "exports")
        .unwrap();
    assert_eq!(exports.get_global_sets(&namespace), 1);
    let x = t.name_from_module(&mut namespace, &metadata, "x").unwrap();
    assert_eq!(x.get_global_sets(&namespace), 1);
}

// port: GlobalNamespaceTest#assignToGlobalNameInLoadModule_doesNotCreateModuleName
#[test]
fn assign_to_global_name_in_load_module_does_not_create_module_name() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_and_gather_module_data(
        "class Foo {}
goog.loadModule(function(exports) {
  goog.module('m');
  Foo.Bar = 0
  return exports;
});
",
    );

    let metadata = t.module_by_goog_namespace("m");
    assert_eq!(
        t.name_from_module(&mut namespace, &metadata, "Foo.Bar"),
        None
    );
    assert!(t.slot(&mut namespace, "Foo.Bar").is_some());
}

/// `parseAndGatherModuleData(js)`, then the Name `name` of the module at path test.js, whose
/// declaration's parent must have `token`.
fn assert_es_module_name_declared_by(js: &str, name: &str, token: Token) {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_and_gather_module_data(js);
    let metadata = t.module_by_path("test.js");
    let x = t.name_from_module(&mut namespace, &metadata, name);

    let x = x.expect("x");
    assert_declaration_parent_token(&mut t, &namespace, x, token);
}

// port: GlobalNamespaceTest#moduleLevelNamesAreCaptured_esExportDecl
#[test]
fn module_level_names_are_captured_es_export_decl() {
    assert_es_module_name_declared_by("export const x = 0;", "x", Token::CONST);
}

// port: GlobalNamespaceTest#moduleLevelNamesAreCaptured_esExportClassDecl
#[test]
fn module_level_names_are_captured_es_export_class_decl() {
    assert_es_module_name_declared_by("export class Foo {}", "Foo", Token::CLASS);
}

// port: GlobalNamespaceTest#moduleLevelNamesAreCaptured_esExportFunctionDecl
#[test]
fn module_level_names_are_captured_es_export_function_decl() {
    assert_es_module_name_declared_by("export function fn() {}", "fn", Token::FUNCTION);
}

// port: GlobalNamespaceTest#moduleLevelNamesAreCaptured_esExportDefaultFunctionDecl
#[test]
fn module_level_names_are_captured_es_export_default_function_decl() {
    assert_es_module_name_declared_by("export default function fn() {}", "fn", Token::FUNCTION);
}

// port: GlobalNamespaceTest#esModuleLevelNamesAreCaptured
#[test]
fn es_module_level_names_are_captured() {
    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_and_gather_module_data("class Foo {} Foo.Bar = 0; export {Foo};");
    let metadata = t.module_by_path("test.js");
    let x = t.name_from_module(&mut namespace, &metadata, "Foo.Bar");

    let x = x.expect("Foo.Bar");
    assert!(x.get_declaration(&namespace).is_some());
}

// port: GlobalNamespaceTest#getCommonAncestorChunk_returnsDeclarationChunk_whenDeclarationLoadedFirst
#[test]
fn get_common_ancestor_chunk_returns_declaration_chunk_when_declaration_loaded_first() {
    let chunks = JSChunkGraphBuilder::for_chain()
        .add_chunk("console.log('base');")
        .add_chunk("const parent = {};")
        .add_chunk("parent.child = {};")
        .build();

    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_chunks(&chunks);
    let parent_name = t.slot(&mut namespace, "parent").unwrap();
    let child_name = t.slot(&mut namespace, "parent.child").unwrap();

    let chunk_graph = t.c().get_chunk_graph().expect("chunkGraph");
    assert_eq!(
        parent_name.get_deepest_common_ancestor_chunk(&namespace, chunk_graph),
        Some(chunks[1].clone())
    );
    assert_eq!(
        child_name.get_deepest_common_ancestor_chunk(&namespace, chunk_graph),
        Some(chunks[2].clone())
    );
}

// port: GlobalNamespaceTest#getCommonAncestorChunk_findsCommonAncestorOfSiblingChunks
#[test]
fn get_common_ancestor_chunk_finds_common_ancestor_of_sibling_chunks() {
    let chunks = JSChunkGraphBuilder::for_bush()
        .add_chunk("console.log('base');")
        .add_chunk("const parent = {};") // parent depends on base
        .add_chunk("parent.crossChunk = {};") // depends on parent
        .add_chunk("if (parent.crossChunk) { console.log(parent.crossChunk ); }") // depends on parent
        .build();

    let mut t = GlobalNamespaceTest::new();
    let mut namespace = t.parse_chunks(&chunks);
    let cross_chunk_name = t.slot(&mut namespace, "parent.crossChunk").unwrap();

    let chunk_graph = t.c().get_chunk_graph().expect("chunkGraph");
    assert_eq!(
        cross_chunk_name.get_deepest_common_ancestor_chunk(&namespace, chunk_graph),
        Some(chunks[1].clone())
    );
    assert_eq!(
        cross_chunk_name
            .get_declaration(&namespace)
            .unwrap()
            .get_chunk(&namespace),
        Some(chunks[2].clone())
    );
}
