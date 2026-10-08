//! Core ECMAScript productions (builder A owns this directory).
//!
//! Each file exports `STATEMENTS` and/or `EXPRESSIONS` tables of [`Production`]s. To add a
//! feature, add productions to the file that fits (or a new file plus one line below).
//! `dialect` holds the opt-in sloppy / unsupported dialects; `util` the shared helpers.

use crate::engine::Production;

pub mod class;
pub mod dialect;
pub mod expr;
pub mod flow;
pub mod func;
pub mod literal;
pub mod pattern;
pub mod stmt;
pub mod util;

#[cfg(test)]
mod tests;

pub static STATEMENTS: &[&[Production]] = &[
    stmt::STATEMENTS,
    flow::STATEMENTS,
    pattern::STATEMENTS,
    func::STATEMENTS,
    class::STATEMENTS,
];
pub static EXPRESSIONS: &[&[Production]] = &[
    literal::EXPRESSIONS,
    expr::EXPRESSIONS,
    func::EXPRESSIONS,
    class::EXPRESSIONS,
];
