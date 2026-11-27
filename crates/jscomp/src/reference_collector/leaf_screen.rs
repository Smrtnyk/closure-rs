/*
 * Copyright 2026 The closure-rs Authors.
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

//! Rust-only (performance divergence): clean leaf functions found without a traversal, from the
//! `Ast` alone, for InlineVariables in `Mode::ALL`.
//!
//! A function whose subtree is unchanged since a traversal found it clean is skipped
//! (`clean_subtree`), but the first run of the pass, and every run for the functions changed since
//! the last one, traverse each function on the compiler thread. Most of those traversals find the
//! function clean: none of its variables is inlined and no initial value names a variable. For a
//! leaf function (no function below it) with plain parameters and a body whose references have
//! sites (`FunctionBodySites`, so no class, catch clause, destructuring, default value or nested
//! `let`/`const` in it), `screen` decides that from the references of each variable alone, in the
//! order of the sites, and makes the free references the traversal's frame would note. Such a
//! function is then kept as if a traversal had found it clean, where the traversal reaches it
//! (`try_skip_clean_subtree`), and skipped.
//!
//! What a scope exit of InlineVariables decides about a variable of the function reads only the
//! variable's references and the subtree around them, apart from the checks that can only refuse
//! (an exported or extern variable, `@noinline`, a weak reference, a parameter changed through
//! `arguments`), from the alias lookup of an initial value that is a name, and from the side
//! effects that `canMoveModerately` asks for. The screen refuses a function in which a variable
//! may get past the cheap refusals of `StandardVarExpert#analyze` and `#analyzeWithInitialValue`
//! towards an inlining (an initial value that is a name, an immutable value of a constant, a
//! variable read once or never, never assigned, or initialized with an immutable value or `this`)
//! and keeps the others: those refuse every variable whatever the refusals above, so the scope
//! exits change nothing and look up nothing. A parameter is refused at once (its first reference
//! is in the parameter list), and nothing aliases one inside a function the screen keeps.
//!
//! The free references are those of the spellings that no scope of the function declares (a
//! parameter, or a `var`, `let` or `const` of the body or of a block in it, the only declarations
//! such a function holds), in the order of the sites, each with its basic block below the
//! function's block and the body's block. Whether a variable outside the function declares each
//! of them is known where the function is reached, from the scope around it: those that resolve
//! are the free references the traversal would note, the others its absent names (as
//! `try_skip_clean_subtree` checks again before it skips). Names that may be implicit variables
//! (`exports`, `super`, `this`, and `arguments` in an arrow function) are left to the traversal.

use super::clean_subtree::{CacheOf, CleanSubtreeCache, FreeSites};
use super::function_body::{FunctionBodySites, FunctionReferenceCache, SiteKind};
use crate::abstract_compiler::AbstractCompiler;
use crate::abstract_scope::ImplicitVar;
use crate::node_util::NodeUtil;
use crate::reference::Reference;
use crate::scope::ScopeId;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;

/// Which scope exits a leaf screen answers for (`Behavior::leaf_screen`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeafScreen {
    /// No leaf function is screened.
    None,
    /// Those of InlineVariables in `Mode::ALL`: every variable of the function must be refused.
    InlineVariables,
    /// Scope exits that do nothing (CrossChunkReferenceCollector): every leaf with sites is clean.
    FreeReferencesOnly,
}

/// No parent in the kept block structure: the block the traversal is in at the function.
const BASE: u32 = u32::MAX;

/// Whether `fn_node` may be screened: a function with plain parameters, no name of its own scope
/// (a function expression's name) and a body that is a block. The caller checks that no function
/// lies below it.
pub(super) fn may_screen(ast: &Ast, fn_node: NodeId) -> bool {
    let Some(name) = fn_node.get_first_child(ast) else {
        return false;
    };
    let Some(params) = name.get_next(ast) else {
        return false;
    };
    let Some(body) = params.get_next(ast) else {
        return false;
    };
    if !body.is_block(ast) || !params.is_param_list(ast) {
        return false;
    }
    if !name.get_string_ref(ast).is_empty() && !NodeUtil::is_function_declaration(ast, fn_node) {
        return false;
    }
    let mut param = params.get_first_child(ast);
    while let Some(p) = param {
        if !p.is_name(ast)
            || p.has_children(ast)
            || p.string_payload(ast).is_none_or(|s| s.is_empty())
        {
            return false;
        }
        param = p.get_next(ast);
    }
    true
}

/// What a spelling of a screened body is bound to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    /// A parameter, or the `arguments` of a function that is no arrow function: a variable of
    /// the function's scope that no scope exit inlines.
    Param,
    /// A variable of the body's scope or of a scope in it, with its declaring NAME (the first
    /// declaration).
    Declared(NodeId),
    /// No scope of the function declares it.
    Free,
}

/// The binding of each spelling of the body of the leaf function `fn_node` (see `may_screen`)
/// whose body has the sites `sites`, when the scope exits of InlineVariables in it change nothing
/// and look up no variable (see the module comment); `None` when that is not shown. `normalized`:
/// whether every declared name is unique, so that a spelling declared in a nested scope names one
/// variable.
fn screen(
    ast: &Ast,
    fn_node: NodeId,
    sites: &FunctionBodySites,
    normalized: bool,
    kind: LeafScreen,
) -> Option<Vec<Binding>> {
    if sites.has_delegates
        || (sites.declares_in_nested_scopes && (sites.nested_names_escape || !normalized))
    {
        return None;
    }
    let is_arrow = fn_node.is_arrow_function(ast);
    let params = fn_node.get_second_child(ast)?;
    let mut param_names: Vec<&JsString> = Vec::new();
    let mut param = params.get_first_child(ast);
    while let Some(p) = param {
        let spelling = p.get_string_ref(ast);
        if ImplicitVar::of(spelling).is_some() || param_names.contains(&spelling) {
            return None;
        }
        param_names.push(spelling);
        param = p.get_next(ast);
    }

    let mut bindings = Vec::with_capacity(sites.first_names.len());
    for &first in &sites.first_names {
        let spelling = first.get_string_ref(ast);
        bindings.push(if param_names.contains(&spelling) {
            Binding::Param
        } else if let Some(implicit) = ImplicitVar::of(spelling) {
            // A plain function makes its own `arguments`, which a reference names without
            // looking outside; others may name a variable made on demand elsewhere.
            if implicit != ImplicitVar::ARGUMENTS || is_arrow {
                return None;
            }
            Binding::Param
        } else {
            Binding::Free
        });
    }
    // The declarations, in the order of the sites (the order `ScopeScanner` declares them in, as
    // no statement of such a body declares inside another). A `let` or `const` of a nested scope
    // is the only variable of its spelling in a normalized tree.
    for site in &sites.sites {
        if let SiteKind::Reference(name, slot) = site.decode()
            && bindings[slot as usize] == Binding::Free
            && matches!(
                name.get_parent(ast)?.get_token(ast),
                Token::VAR | Token::LET | Token::CONST
            )
        {
            bindings[slot as usize] = Binding::Declared(name);
        }
    }

    // The references of each declared variable with their basic blocks (by entry into a block,
    // the body's being 0).
    let mut refs: Vec<Vec<(NodeId, u32)>> = vec![Vec::new(); bindings.len()];
    let mut blocks: Vec<u32> = vec![0];
    let mut entered_blocks = 0;
    for site in &sites.sites {
        match site.decode() {
            SiteKind::Reference(name, slot) => {
                if matches!(bindings[slot as usize], Binding::Declared(_)) {
                    refs[slot as usize].push((name, *blocks.last().expect("the body's block")));
                }
            }
            SiteKind::EnterBlock(_) => {
                entered_blocks += 1;
                blocks.push(entered_blocks);
            }
            SiteKind::ExitBlock(_) => {
                blocks.pop();
            }
            SiteKind::EnterEmptyScope(_)
            | SiteKind::ExitEmptyScope
            | SiteKind::EnterScope(_)
            | SiteKind::ExitScope => {}
            SiteKind::Delegate(_) => return None,
        }
    }
    // Every declared variable must be refused.
    if kind == LeafScreen::InlineVariables {
        for (binding, refs) in bindings.iter().zip(&refs) {
            if let Binding::Declared(declaration) = *binding
                && !is_refused(ast, declaration, refs)?
            {
                return None;
            }
        }
    }
    Some(bindings)
}

/// The free references the frame of the screened leaf function `fn_node` notes (`clean_subtree`):
/// the references of the free spellings that resolve around the function (`resolves`, by
/// spelling), in the order of the sites, each with its block in the kept block structure (the
/// function's, the body's, then the blocks the sites enter), and the first NAME of each free
/// spelling that resolves to nothing.
fn free_sites(
    ast: &Ast,
    fn_node: NodeId,
    sites: &FunctionBodySites,
    bindings: &[Binding],
    resolves: &[bool],
) -> Option<FreeSites> {
    let body = fn_node.get_last_child(ast)?;
    let mut stack: Vec<(NodeId, Option<u32>)> = vec![(fn_node, None), (body, None)];
    let mut blocks: Vec<(NodeId, u32)> = Vec::new();
    let mut names: IndexMap<u32, NodeId> = IndexMap::default();
    let mut free = Vec::new();
    for site in &sites.sites {
        match site.decode() {
            SiteKind::Reference(name, slot) => {
                if bindings[slot as usize] != Binding::Free || !resolves[slot as usize] {
                    continue;
                }
                let entry = names.entry(slot);
                let index = u32::try_from(entry.index()).ok()?;
                entry.or_insert(name);
                let mut parent = BASE;
                for (root, made) in stack.iter_mut() {
                    parent = match *made {
                        Some(i) => i,
                        None => {
                            let i = u32::try_from(blocks.len()).ok()?;
                            blocks.push((*root, parent));
                            *made = Some(i);
                            i
                        }
                    };
                }
                free.push((name, index, parent));
            }
            SiteKind::EnterBlock(n) => stack.push((n, None)),
            SiteKind::ExitBlock(_) => {
                stack.pop();
            }
            SiteKind::EnterEmptyScope(_)
            | SiteKind::ExitEmptyScope
            | SiteKind::EnterScope(_)
            | SiteKind::ExitScope => {}
            SiteKind::Delegate(_) => return None,
        }
    }
    let absent_names = sites
        .first_names
        .iter()
        .zip(bindings.iter().zip(resolves))
        .filter(|(_, (binding, resolves))| **binding == Binding::Free && !**resolves)
        .map(|(&name, _)| name)
        .collect();
    Some(FreeSites {
        first_names: names.into_values().collect(),
        blocks: blocks.into_boxed_slice(),
        sites: free.into_boxed_slice(),
        absent_names,
    })
}

/// Whether `StandardVarExpert#analyze` refuses the variable declared at `declaration` whose
/// references, in the order the collector adds them and each with its basic block, are `refs`,
/// without an alias lookup and before any check that may inline it (`None` for a shape the
/// analysis would reject with an exception, left to the traversal).
fn is_refused(ast: &Ast, declaration: NodeId, refs: &[(NodeId, u32)]) -> Option<bool> {
    let node = |i: usize| refs[i].0;
    // `Var#isDeclaredOrInferredConst`.
    let is_constant = declaration.is_declared_constant_var(ast)
        || declaration.is_inferred_constant_var(ast)
        || declaration.get_boolean_prop(ast, NodeId::IS_CONSTANT_NAME);
    let initialization = if is_constant {
        (0..refs.len()).find(|&i| {
            Reference::is_initializing_declaration_node(ast, node(i))
                || is_initializing_assignment_at(ast, refs, i)
        })
    } else if Reference::is_initializing_declaration_node(ast, node(0)) {
        Some(0)
    } else if is_initializing_assignment_at(ast, refs, 1) {
        // The references of a function body share its input.
        Some(1)
    } else {
        None
    };
    let init_value = initialization.and_then(|i| NodeUtil::get_r_value_of_l_value(ast, node(i)));
    if let Some(value) = init_value
        && (value.is_name(ast) || (is_constant && NodeUtil::is_immutable_value(ast, value)))
    {
        return Some(false);
    }
    // `analyzeWithInitialValue`.
    if refs.len() > 1 && all_references_are_valid(ast, refs, initialization)? {
        let never_assigned = refs.iter().all(|&(r, _)| {
            !Reference::is_lvalue_node(ast, r)
                && !Reference::is_initializing_declaration_node(ast, r)
        });
        let (Some(initialization), Some(value)) = (initialization, init_value) else {
            return Some(false);
        };
        if never_assigned || NodeUtil::is_immutable_value(ast, value) || value.is_this(ast) {
            return Some(false);
        }
        let first_read = if initialization == 0 { 1 } else { 2 };
        match refs.len() - first_read {
            0 => return Some(false),
            // `canInline` refuses before it asks for scopes or side effects: the value is read
            // more than once, the references are in other basic blocks, or a property read
            // would become the target of a call.
            1 => {
                let (init, init_block) = refs[initialization];
                let (read, read_block) = refs[first_read];
                let refused = (initialization != 0
                    && !init
                        .get_grandparent(ast)
                        .is_some_and(|g| g.is_expr_result(ast)))
                    || refs[0].1 != init_block
                    || refs[0].1 != read_block
                    || (value.is_get_prop(ast)
                        && read.get_parent(ast).is_some_and(|p| {
                            p.is_call(ast) && p.get_first_child(ast) == Some(read)
                        }));
                if !refused {
                    return Some(false);
                }
            }
            _ => {}
        }
    }
    Some(true)
}

/// `ReferenceCollection#isInitializingAssignmentAt` over the NAMEs of the references.
fn is_initializing_assignment_at(ast: &Ast, refs: &[(NodeId, u32)], index: usize) -> bool {
    if index < refs.len() && index > 0 {
        let maybe_decl = refs[index - 1].0.get_parent(ast);
        if maybe_decl.is_some_and(|p| p.is_var(ast) || p.is_let(ast)) {
            let maybe_init = refs[index].0;
            return maybe_init
                .get_parent(ast)
                .is_some_and(|p| p.is_assign(ast) && p.get_first_child(ast) == Some(maybe_init));
        }
    }
    false
}

/// `StandardVarExpert#allReferencesAreValid` over the NAMEs of the references.
fn all_references_are_valid(
    ast: &Ast,
    refs: &[(NodeId, u32)],
    initialization: Option<usize>,
) -> Option<bool> {
    let node = |i: usize| refs[i].0;
    let declaration = node(0);
    let parent = declaration.get_parent(ast)?;
    let has_valid_declaration = (NodeUtil::is_name_declaration(ast, Some(parent))
        && !NodeUtil::is_loop_structure(ast, parent.get_parent(ast)?))
        || NodeUtil::is_function_declaration(ast, parent);
    let has_valid_initialization = match initialization {
        None => false,
        Some(i) if Reference::is_declaration_node(ast, node(i)) => {
            NodeUtil::is_function_declaration(ast, node(i).get_parent(ast)?)
                || node(i).has_children(ast)
        }
        Some(i) => {
            let parent = node(i).get_parent(ast)?;
            // Java checks this state.
            if !parent.is_assign(ast) || parent.get_first_child(ast) != Some(node(i)) {
                return None;
            }
            true
        }
    };
    let never_assigned = refs.iter().all(|&(r, _)| {
        !Reference::is_lvalue_node(ast, r) && !Reference::is_initializing_declaration_node(ast, r)
    });
    if has_valid_declaration && (has_valid_initialization || never_assigned) {
        return Some(refs.iter().enumerate().all(|(i, &(r, _))| {
            i == 0
                || Some(i) == initialization
                || (!Reference::is_declaration_node(ast, r) && !Reference::is_lvalue_node(ast, r))
        }));
    }
    Some(false)
}

/// `CLOSURE_RS_CLEAN_SEEDS=off`: no leaf function is screened (`screen_and_keep`); the kept clean
/// functions then come from traversals alone.
static SEEDS: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("CLOSURE_RS_CLEAN_SEEDS").as_deref() != Ok("off"));

/// Screens `fn_node`, which the traversal is about to enter with `enclosing` the scope around it
/// and does not keep as clean and unchanged in `cache`, and keeps it as clean when the screen finds
/// it so (see the module comment). Returns whether it did. The screen reads the function's subtree
/// as it is now, and which of its free spellings resolve in `enclosing`, as its traversal would.
pub(super) fn screen_and_keep(
    compiler: &mut AbstractCompiler,
    cache: CacheOf,
    fn_node: NodeId,
    enclosing: ScopeId,
    kind: LeafScreen,
) -> bool {
    if !*SEEDS || !may_screen(compiler, fn_node) {
        return false;
    }
    let body = fn_node.get_last_child(compiler).expect("function body");
    let free = if body.has_children(compiler) {
        // The traversal of the body asks for the same sites.
        let Some(sites) = FunctionReferenceCache::sites(compiler, body) else {
            return false;
        };
        let normalized = compiler.get_life_cycle_stage().is_normalized();
        let Some(bindings) = screen(compiler, fn_node, &sites, normalized, kind) else {
            return false;
        };
        let resolves: Vec<bool> = bindings
            .iter()
            .zip(&sites.first_names)
            .map(|(binding, &name)| {
                *binding == Binding::Free && enclosing.get_var_of_node(compiler, name).is_some()
            })
            .collect();
        match free_sites(compiler, fn_node, &sites, &bindings, &resolves) {
            Some(free) => free,
            None => return false,
        }
    } else {
        FreeSites::default()
    };
    CleanSubtreeCache::keep_screened(compiler, cache, fn_node, free);
    true
}
