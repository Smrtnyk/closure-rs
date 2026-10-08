/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/testing/JSCompCorrespondences.java.

//! Port of testing/JSCompCorrespondences.java: well known Correspondence instances for use in
//! tests.
//!
//! Truth's Correspondence is a predicate plus a description; it is modelled as that pair.
use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_input::CompilerInput;
use closure_jscomp::diagnostic_group::DiagnosticGroup;
use closure_jscomp::diagnostic_type::DiagnosticType;
use closure_jscomp::js_error::JSError;
use closure_jscomp::source_file::SourceFile;
use closure_rhino::check_not_null;
use closure_rhino::node::{Ast, NodeId};
use std::cell::RefCell;
use std::sync::Arc;

/// Truth's `Correspondence<A, E>`: whether an actual element corresponds to an expected one.
// port: Correspondence (com.google.common.truth)
pub struct Correspondence<A: ?Sized, E: ?Sized> {
    compare: fn(&A, &E) -> bool,
    description: &'static str,
}

impl<A: ?Sized, E: ?Sized> Correspondence<A, E> {
    // port: Correspondence#from
    pub const fn from(compare: fn(&A, &E) -> bool, description: &'static str) -> Self {
        Self {
            compare,
            description,
        }
    }
    // port: Correspondence#compare
    pub fn compare(&self, actual: &A, expected: &E) -> bool {
        (self.compare)(actual, expected)
    }
    // port: Correspondence#toString
    pub fn description(&self) -> &'static str {
        self.description
    }
}

// port: JSCompCorrespondences#DIAGNOSTIC_EQUALITY
pub static DIAGNOSTIC_EQUALITY: Correspondence<JSError, DiagnosticType> = Correspondence::from(
    |actual, expected| actual.get_type() == expected,
    "has diagnostic type equal to",
);

// port: JSCompCorrespondences#OWNING_DIAGNOSTIC_GROUP
pub static OWNING_DIAGNOSTIC_GROUP: Correspondence<JSError, DiagnosticGroup> = Correspondence::from(
    |actual, expected| expected.matches(actual),
    "is part of diagnostic group",
);

// port: JSCompCorrespondences#DESCRIPTION_EQUALITY
pub static DESCRIPTION_EQUALITY: Correspondence<JSError, str> = Correspondence::from(
    |actual, expected| actual.description() == expected,
    "has description equal to",
);

// port: JSCompCorrespondences#INPUT_NAME_EQUALITY
pub static INPUT_NAME_EQUALITY: Correspondence<CompilerInput, str> = Correspondence::from(
    |actual, expected| actual.get_name() == expected,
    "has name equal to",
);

/// The actual side of EQUALITY_WHEN_PARSED_AS_EXPRESSION: a node and the arena that owns it.
pub struct NodeInAst<'a> {
    pub ast: &'a Ast,
    pub node: NodeId,
}

// port: JSCompCorrespondences#EQUALITY_WHEN_PARSED_AS_EXPRESSION
pub static EQUALITY_WHEN_PARSED_AS_EXPRESSION: Correspondence<NodeInAst<'static>, str> =
    Correspondence::from(
        |actual, expected| equality_when_parsed_as_expression(actual.ast, actual.node, expected),
        "matches nodes parsed from",
    );

/// EQUALITY_WHEN_PARSED_AS_EXPRESSION's predicate for a node in any arena.
// port: JSCompCorrespondences#EQUALITY_WHEN_PARSED_AS_EXPRESSION (predicate)
pub fn equality_when_parsed_as_expression(
    actual_ast: &Ast,
    actual: NodeId,
    expected: &str,
) -> bool {
    COMPILER_FOR_PARSING.with(|compiler| {
        let mut compiler = compiler.borrow_mut();
        let expr = parse_expr(&mut compiler, expected);
        expr.is_equivalent_to_across(&compiler, actual_ast, actual)
    })
}

// port: JSCompCorrespondences#referenceEquality
pub fn reference_equality<A: ?Sized, B: ?Sized>() -> Correspondence<Arc<A>, Arc<B>> {
    Correspondence::from(
        |actual, expected| std::ptr::addr_eq(Arc::as_ptr(actual), Arc::as_ptr(expected)),
        "is same instance as",
    )
}

thread_local! {
    // port: JSCompCorrespondences#COMPILER_FOR_PARSING (one per thread: Compiler is not shared
    // across threads)
    static COMPILER_FOR_PARSING: RefCell<Compiler> = RefCell::new(Compiler::new());
}

// port: JSCompCorrespondences#parseExpr
fn parse_expr(compiler: &mut Compiler, expr: &str) -> NodeId {
    let expr_root = compiler
        .parse_file(Arc::new(SourceFile::from_code(
            "expr",
            format!("({expr})").as_str(),
        ))) // SCRIPT
        .get_first_first_child(compiler); // EXPR_RESULT > expr
    check_not_null!(expr_root, "Failed to parse expression")
}
