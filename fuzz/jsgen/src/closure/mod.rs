//! Closure Compiler idioms and JSDoc types (builder B owns this directory).
//!
//! Enabled by `Config::closure`. Same table convention as `lang/`.
//!
//! - `jsdoc.rs`: random JSDoc type expressions.
//! - `typed.rs`: a closed set of types with well-typed value generators, so annotated code
//!   type-checks and ADVANCED output stays non-trivial.
//! - `items.rs`: typed top-level items (functions, prototype and ES classes, interfaces,
//!   enums, constants, `@template` functions) in script, goog.module, goog.provide, ES module
//!   and CommonJS styles, plus typed uses of them.
//! - `idioms.rs`, `script.rs`: single-file statement and expression productions.
//! - `passes.rs`: inputs for passes the fuzzer never reached before (D-016 item 5):
//!   extractPrototypeMemberDeclarations, crossChunkMethodMotion, deadPropertyAssignmentElimination,
//!   optimizeConstructors, `@nosideeffects`, `@dict`.
//! - `program.rs`: multi-file programs whose imports are legal under one output file,
//!   `chunks2` and `chunks3` at once ([`program::generate_closure_nth`]).

use crate::engine::Production;

pub mod idioms;
pub mod items;
pub mod jsdoc;
pub mod passes;
pub mod program;
pub mod script;
pub mod typed;

#[cfg(test)]
mod tests;

pub static STATEMENTS: &[&[Production]] =
    &[idioms::STATEMENTS, script::STATEMENTS, passes::STATEMENTS];
pub static EXPRESSIONS: &[&[Production]] = &[idioms::EXPRESSIONS];
