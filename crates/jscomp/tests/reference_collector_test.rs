/*
 * Copyright 2008 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/ReferenceCollector.java,
//   test/com/google/javascript/jscomp/ReferenceCollectorTest.java.

//! Port of the `ReferenceCollectorTest.java` test that has no corpus record. The
//! `testBehavior` tests replay from corpus/unit/records (ReferenceCollectorTest, 30 records).
use closure_jscomp::node_traversal::NodeTraversal;
use closure_jscomp::reference_collector::{Behavior, ReferenceCollector};
use closure_jscomp::reference_map::ReferenceMap;
use closure_jscomp::syntactic_scope_creator::SyntacticScopeCreator;
use closure_rhino::js_string::JsString;
use closure_rhino::token::Token;
use closure_testing::compiler_test_case::CompilerTestCase;

/// The anonymous `Behavior` of `testProcessScopeThatsNotABasicBlock`.
struct NotABasicBlockBehavior;
impl Behavior for NotABasicBlockBehavior {
    // port: ReferenceCollectorTest#testProcessScopeThatsNotABasicBlock (Behavior#afterExitScope)
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) {
        let scope = t.get_scope();
        let var = scope
            .get_var(t.get_compiler(), JsString::from("y"))
            .unwrap();
        let y = rm.get_references(var).unwrap();
        let ast = t.get_compiler();
        assert!(y.is_well_defined(ast));
        let first_basic_block = y.references[0].get_basic_block().unwrap();
        assert_eq!(first_basic_block.get_root().get_token(ast), Token::BLOCK);
        assert_eq!(
            first_basic_block
                .get_root()
                .get_parent(ast)
                .unwrap()
                .get_token(ast),
            Token::SCRIPT
        );

        // We do not create a new BasicBlock for the second { use(y); }
        let second_basic_block = y.references[1].get_basic_block().unwrap();
        assert_eq!(second_basic_block.get_root().get_token(ast), Token::BLOCK);
        assert_eq!(
            second_basic_block
                .get_root()
                .get_parent(ast)
                .unwrap()
                .get_token(ast),
            Token::IF
        );
    }
}

// port: ReferenceCollectorTest#testProcessScopeThatsNotABasicBlock
#[test]
fn test_process_scope_thats_not_a_basic_block() {
    // Tests the case where the scope we pass in is not really a basic block, but we create a new
    // basic block anyway because ReferenceCollector expects all nodes to be in a block.
    let mut case = CompilerTestCase::new("");
    case.set_up();
    let handle = case.create_compiler().unwrap();
    let mut compiler = handle.borrow_mut();
    let compiler = &mut *compiler;
    let mut syntactic_scope_creator = SyntacticScopeCreator::new();

    let js = "let x = 5; { let y = x + 1; if (true) { { use(y); } } }";
    let root = compiler.parse_test_code(js);
    let block = root.get_second_child(compiler).unwrap();

    // Java creates the scopes after constructing the collector with the same creator; the Rust
    // collector borrows the creator mutably, so the scopes are created first.
    let global_scope = syntactic_scope_creator.create_scope(compiler, root, None);
    let block_scope = syntactic_scope_creator.create_scope(compiler, block, Some(global_scope));
    let mut reference_collecting_callback = ReferenceCollector::new(
        compiler,
        NotABasicBlockBehavior,
        &mut syntactic_scope_creator,
    );
    reference_collecting_callback.process_scope(compiler, block_scope);
}
