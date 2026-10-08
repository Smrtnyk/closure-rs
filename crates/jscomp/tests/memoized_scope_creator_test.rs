/*
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
//   test/com/google/javascript/jscomp/MemoizedScopeCreatorTest.java.

use closure_jscomp::{
    Compiler, compiler_options::CompilerOptions, memoized_scope_creator::MemoizedScopeCreator,
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_rhino::token::Token;
use std::panic::{AssertUnwindSafe, catch_unwind};

// port: MemoizedScopeCreatorTest#testMemoization
#[test]
fn test_memoization() {
    let mut compiler = Compiler::new();
    let root1 = compiler.new_node(Token::ROOT);
    let root2 = compiler.new_node(Token::ROOT);
    compiler.init_options(CompilerOptions::new());
    let mut creator = MemoizedScopeCreator::new(Box::new(SyntacticScopeCreator::new()));
    let scope_a = creator.create_scope(&mut compiler, root1, None);
    assert_eq!(creator.create_scope(&mut compiler, root1, None), scope_a);
    assert_ne!(creator.create_scope(&mut compiler, root2, None), scope_a);
}

// port: MemoizedScopeCreatorTest#testPreconditionCheck
#[test]
fn test_precondition_check() {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    let root = compiler.new_node(Token::ROOT);
    let mut creator = MemoizedScopeCreator::new(Box::new(SyntacticScopeCreator::new()));
    let scope_a = creator.create_scope(&mut compiler, root, None);
    let handled = catch_unwind(AssertUnwindSafe(|| {
        creator.create_scope(&mut compiler, root, Some(scope_a));
    }))
    .is_err();
    assert!(handled);
}
