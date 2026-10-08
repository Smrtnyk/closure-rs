//! Generator core: output buffer, depth cap, scopes, syntactic context and the production
//! registry. Feature code lives in `lang/` (core ECMAScript) and `closure/` (Closure idioms,
//! JSDoc); it adds productions to its own tables and never edits this file's dispatch.

use crate::Config;
use crate::rng::Rng;

/// What kind of binding a name is; decides whether it may be assigned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Var,
    Let,
    Const,
    Param,
    Function,
    Class,
}

impl Binding {
    pub fn mutable(self) -> bool {
        matches!(self, Binding::Var | Binding::Let | Binding::Param)
    }
}

/// Syntactic context: which statements/expressions are legal at the current point.
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    pub in_function: bool,
    pub in_generator: bool,
    pub in_async: bool,
    pub in_loop: bool,
    pub in_switch: bool,
    pub in_class_method: bool,
    pub labels: Vec<String>,
    /// Set by jump statements (`break`/`continue`/`return`/`throw`); `block` emits nothing
    /// after a jump. Reason: dead block-scoped declarations after a jump crash the Java
    /// reference (IllegalStateException in NodeUtil.getInsertionPointAfterAllInnerFunctionDeclarations
    /// via PeepholeRemoveDeadCode.redeclareIfBlockScopedVar), and D-009 drops Java crashes.
    pub terminated: bool,
}

/// One grammar production. `leaf` productions never recurse (used at the depth cap).
pub struct Production {
    pub name: &'static str,
    pub weight: u32,
    pub leaf: bool,
    pub when: fn(&Ctx) -> bool,
    pub emit: fn(&mut Gen),
}

pub fn always(_: &Ctx) -> bool {
    true
}

pub struct Gen<'c> {
    pub rng: Rng,
    pub cfg: &'c Config,
    pub out: String,
    pub ctx: Ctx,
    depth: u32,
    indent: usize,
    scopes: Vec<Vec<(String, Binding)>>,
    next_id: u32,
}

impl<'c> Gen<'c> {
    pub fn new(rng: Rng, cfg: &'c Config) -> Self {
        Gen {
            rng,
            cfg,
            out: String::new(),
            ctx: Ctx::default(),
            depth: 0,
            indent: 0,
            scopes: vec![Vec::new()],
            next_id: 0,
        }
    }

    // ---- output -----------------------------------------------------------------------

    pub fn w(&mut self, s: &str) {
        self.out.push_str(s);
    }

    pub fn nl(&mut self) {
        self.out.push('\n');
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
    }

    // ---- depth ------------------------------------------------------------------------

    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// True when productions must not recurse any further.
    pub fn at_cap(&self) -> bool {
        self.depth + 1 >= self.cfg.max_depth()
    }

    // ---- names and scopes -------------------------------------------------------------

    pub fn fresh(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{}", self.next_id)
    }

    pub fn declare(&mut self, name: &str, b: Binding) {
        self.scopes
            .last_mut()
            .expect("scope")
            .push((name.to_string(), b));
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// A random visible name matching `f`, innermost scopes preferred equally.
    pub fn visible(&mut self, f: impl Fn(Binding) -> bool) -> Option<String> {
        let all: Vec<&String> = self
            .scopes
            .iter()
            .flatten()
            .filter(|(_, b)| f(*b))
            .map(|(n, _)| n)
            .collect();
        if all.is_empty() {
            return None;
        }
        let i = self.rng.below(all.len() as u64) as usize;
        Some(all[i].clone())
    }

    /// Every name of the outermost (program) scope matching `f`, in declaration order.
    pub fn top_level(&self, f: impl Fn(Binding) -> bool) -> Vec<String> {
        self.scopes[0]
            .iter()
            .filter(|(_, b)| f(*b))
            .map(|(n, _)| n.clone())
            .collect()
    }

    // ---- dispatch ---------------------------------------------------------------------

    fn choose(&mut self, tables: &[&'static [Production]]) -> Option<fn(&mut Gen)> {
        let leaf_only = self.at_cap();
        let mut total = 0u64;
        for t in tables {
            for p in t.iter() {
                if (!leaf_only || p.leaf) && (p.when)(&self.ctx) {
                    total += u64::from(p.weight);
                }
            }
        }
        if total == 0 {
            return None;
        }
        let mut k = self.rng.below(total);
        for t in tables {
            for p in t.iter() {
                if (!leaf_only || p.leaf) && (p.when)(&self.ctx) {
                    let w = u64::from(p.weight);
                    if k < w {
                        return Some(p.emit);
                    }
                    k -= w;
                }
            }
        }
        None
    }

    fn statement_tables(&self) -> Vec<&'static [Production]> {
        let mut t: Vec<&'static [Production]> = crate::lang::STATEMENTS.to_vec();
        if self.cfg.closure {
            t.extend_from_slice(crate::closure::STATEMENTS);
        }
        t
    }

    fn expression_tables(&self) -> Vec<&'static [Production]> {
        let mut t: Vec<&'static [Production]> = crate::lang::EXPRESSIONS.to_vec();
        if self.cfg.closure {
            t.extend_from_slice(crate::closure::EXPRESSIONS);
        }
        t
    }

    /// Emit one statement (on the current line; caller handles newlines).
    pub fn stmt(&mut self) {
        let tables = self.statement_tables();
        self.depth += 1;
        match self.choose(&tables) {
            Some(f) => f(self),
            None => self.w(";"),
        }
        self.depth -= 1;
    }

    /// Emit one expression, always parenthesisable as a primary by the caller.
    pub fn expr(&mut self) {
        let tables = self.expression_tables();
        self.depth += 1;
        match self.choose(&tables) {
            Some(f) => f(self),
            None => self.w("0"),
        }
        self.depth -= 1;
    }

    /// `(expr)`: safe in every operand position.
    pub fn pexpr(&mut self) {
        self.w("(");
        self.expr();
        self.w(")");
    }

    /// `{ stmt* }` in a fresh block scope.
    pub fn block(&mut self, max_stmts: u32) {
        self.w("{");
        self.indent += 1;
        self.push_scope();
        self.depth += 1;
        let n = if self.at_cap() {
            0
        } else {
            self.rng.range(0, max_stmts)
        };
        let outer_terminated = std::mem::take(&mut self.ctx.terminated);
        for _ in 0..n {
            self.nl();
            self.stmt();
            if self.ctx.terminated {
                break;
            }
        }
        self.ctx.terminated = outer_terminated;
        self.depth -= 1;
        self.pop_scope();
        self.indent -= 1;
        self.nl();
        self.w("}");
    }

    /// Run `f` with a modified context, restoring it afterwards.
    pub fn with_ctx(&mut self, m: impl FnOnce(&mut Ctx), f: impl FnOnce(&mut Gen)) {
        let saved = self.ctx.clone();
        m(&mut self.ctx);
        f(self);
        self.ctx = saved;
    }
}
