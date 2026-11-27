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

//! Rust-only (performance divergence): clean function subtrees, which a behavior that opts in
//! (`Behavior::clean_subtree_cache`: InlineVariables, CrossChunkReferenceCollector) does not
//! traverse again while they are unchanged.
//!
//! While a function is traversed, the collector keeps a frame for it from the entry of its function
//! scope to that scope's exit. Every reference to a variable declared outside the function (a free
//! reference) is noted in the frame with its basic block, in the order the traversal makes them,
//! also when the filter rejects it; so are the names that resolve to no variable and the change
//! stamps of the nested functions (taken when each is left or skipped). The behavior reports after
//! each scope exit whether it changed the tree and the depth of the outermost scope declaring a
//! variable it looked up by name (`Behavior::take_scope_exit_report`); the frames of the functions
//! around that scope exit are dirty when it changed the tree, and those of the functions not
//! containing the variable looked up. A function that leaves its frame clean is kept with its own
//! and nested stamps, whether it is a declaration, an expression and an arrow function, whether a
//! jump to a hoisted function declared in it happened, and its free references ([`FreeSites`]).
//!
//! A kept function whose stamps and kind are unchanged is then skipped, as long as each spelling of
//! its free references resolves from the scope around it (no scope in the function declares one)
//! and each absent name still resolves to nothing. The behavior's scope exits inside it would
//! change nothing and read only what lies in the subtree, so they are left out; what the function
//! contributes to the scopes around it are its free references, which are added again in the same
//! order: each gets a basic block made from the kept block structure below the block the traversal
//! is in, the filter is applied, and a reference to a hoisted function the traversal has not
//! collected yet jumps to it, as the traversal would. The references get one scope, an empty child
//! of the scope around the function rooted at the function, instead of the scope of each
//! reference inside the function: the behavior must read the scope of a free reference only
//! through what this scope answers as the real one does (its variables come from the scopes
//! around the function, it is a hoist scope inside the function, and its parents are those of
//! the function's scope).
//!
//! A jump to a hoisted function from inside a frame hides the frames of the functions that do not
//! contain the jumped-to function while it is traversed: its references belong to the functions
//! around its declaration.

use crate::abstract_compiler::AbstractCompiler;
use crate::basic_block::BasicBlock;
use crate::node_util::NodeUtil;
use crate::syntactic_scope_cache::SyntacticScopeCache;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

/// `CLOSURE_RS_CLEAN_SUBTREES`: `off` neither keeps nor skips clean function subtrees; `verify`
/// keeps them and traverses a kept function anyway, checking that it is clean again with the same
/// free references.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Off,
    On,
    Verify,
}

pub(crate) static MODE: LazyLock<Mode> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_CLEAN_SUBTREES").as_deref() {
            Ok("off") => Mode::Off,
            Ok("verify") => Mode::Verify,
            _ => Mode::On,
        },
    );

/// No parent in the kept block structure: the block the traversal is in at the function.
const BASE: u32 = u32::MAX;

/// The free references of a kept function, in the order they were added.
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct FreeSites {
    /// The first NAME of each spelling, in the order of the first references.
    pub(super) first_names: Box<[NodeId]>,
    /// The basic blocks inside the function that hold a free reference or such a block, each with
    /// the root node and the index of its parent (`BASE`: the block the traversal is in).
    pub(super) blocks: Box<[(NodeId, u32)]>,
    /// Each free reference: its NAME, the index of its spelling and of its block.
    pub(super) sites: Box<[(NodeId, u32, u32)]>,
    /// The first NAME of each spelling in the function that resolved to no variable: a scope
    /// around the function would make it resolve if it declared it.
    pub(super) absent_names: Box<[NodeId]>,
}

/// The change stamps (`SyntacticScopeCache::subtree_stamp_with_flags`) of the functions nested in
/// a function, at any depth.
pub(super) type NestedStamps = Arc<[(NodeId, (u32, u32))]>;

/// A clean function, kept by its node.
struct CleanSubtree {
    /// The function's own change stamp.
    own: (u32, u32),
    nested: NestedStamps,
    is_declaration: bool,
    is_expression: bool,
    is_arrow: bool,
    /// Whether no jump to a hoisted function declared in the function happened while it was
    /// traversed: its free references are then in the order of a traversal without jumps too.
    jump_free: bool,
    free: Arc<FreeSites>,
}

/// How a behavior's kept clean functions are found in the compiler.
pub type CacheOf = fn(&mut AbstractCompiler) -> &mut CleanSubtreeCache;

/// The kept clean functions of one behavior.
#[derive(Default)]
pub struct CleanSubtreeCache {
    entries: IndexMap<NodeId, CleanSubtree>,
}

impl CleanSubtreeCache {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// The free references and nested stamps of `fn_node` when it is kept and neither it nor any
    /// function nested in it has changed, nor its kind; a stale entry is dropped.
    pub(super) fn current(
        compiler: &mut AbstractCompiler,
        cache: CacheOf,
        fn_node: NodeId,
    ) -> Option<(Arc<FreeSites>, NestedStamps)> {
        Self::current_if(compiler, cache, fn_node, false)
    }

    /// `current`, for a traversal without jumps to hoisted functions, from the clean functions of
    /// a behavior whose traversal makes those jumps: of those recorded without a jump inside.
    pub(super) fn current_jump_free(
        compiler: &mut AbstractCompiler,
        cache: CacheOf,
        fn_node: NodeId,
    ) -> Option<(Arc<FreeSites>, NestedStamps)> {
        Self::current_if(compiler, cache, fn_node, true)
    }

    fn current_if(
        compiler: &mut AbstractCompiler,
        cache: CacheOf,
        fn_node: NodeId,
        jump_free: bool,
    ) -> Option<(Arc<FreeSites>, NestedStamps)> {
        let entry = cache(compiler).entries.get(&fn_node)?;
        if jump_free && !entry.jump_free {
            return None;
        }
        let (own, nested, free) = (entry.own, entry.nested.clone(), entry.free.clone());
        let kind = (entry.is_declaration, entry.is_expression, entry.is_arrow);
        // A function added to the subtree since changes the change scope it was added to.
        let current = SyntacticScopeCache::subtree_stamp_with_flags(compiler, fn_node) == own
            && kind
                == (
                    NodeUtil::is_function_declaration(compiler, fn_node),
                    NodeUtil::is_function_expression(compiler, fn_node),
                    fn_node.is_arrow_function(compiler),
                )
            && nested.iter().all(|&(function, stamp)| {
                SyntacticScopeCache::cached_subtree_stamp_with_flags(compiler, function) == stamp
            });
        if current {
            Some((free, nested))
        } else {
            cache(compiler).entries.swap_remove(&fn_node);
            None
        }
    }
}

/// A frame of the collector's frame stack.
pub(super) enum Frame {
    /// A function being traversed, from the entry of its function scope to its exit.
    Function(FunctionFrame),
    /// A jump to a hoisted function declared in a scope at this depth: below it, only the frames
    /// of functions around that scope see references and scope exits.
    Jump(i32),
}

pub(super) struct FunctionFrame {
    pub(super) fn_node: NodeId,
    /// The depth of the function's scope: a variable is declared outside the function exactly
    /// when its scope is less deep (for the variables names in the function resolve to).
    pub(super) depth: i32,
    /// The block the traversal was in when the function scope was entered.
    pub(super) base: Rc<BasicBlock>,
    /// The free references so far.
    pub(super) refs: Vec<(NodeId, Rc<BasicBlock>)>,
    /// The names so far that resolved to no variable.
    pub(super) absent: Vec<NodeId>,
    /// The change stamps of the functions nested in the function seen so far.
    pub(super) nested: Vec<(NodeId, (u32, u32))>,
    /// Whether the traversal jumped to a hoisted function declared in the function.
    pub(super) jumped: bool,
    /// Why the frame is dirty, if it is (for `verify`).
    pub(super) dirty: Option<&'static str>,
    /// With `CLOSURE_RS_CLEAN_SUBTREES=verify`: the free references kept for this function, which
    /// it must have again.
    pub(super) verify: Option<Arc<FreeSites>>,
}

/// The frames that see what happens at the top of `frames`, innermost first.
pub(super) fn visible_frames(frames: &mut [Frame]) -> impl Iterator<Item = &mut FunctionFrame> {
    let mut limit = i32::MAX;
    frames
        .iter_mut()
        .rev()
        .filter_map(move |frame| match frame {
            Frame::Jump(depth) => {
                limit = limit.min(*depth);
                None
            }
            Frame::Function(f) if f.depth <= limit => Some(f),
            Frame::Function(_) => None,
        })
}

/// Notes a reference to a variable declared at `var_depth` in every visible frame of a function
/// that does not declare it.
pub(super) fn note_reference(
    frames: &mut [Frame],
    var_depth: i32,
    name: NodeId,
    block: &Rc<BasicBlock>,
) {
    for frame in visible_frames(frames) {
        // The visible frames are those of nested functions, innermost first.
        if var_depth >= frame.depth {
            break;
        }
        frame.refs.push((name, block.clone()));
    }
}

/// Notes a name that resolved to no variable in every visible frame.
pub(super) fn note_absent(frames: &mut [Frame], name: NodeId) {
    for frame in visible_frames(frames) {
        frame.absent.push(name);
    }
}

/// Notes a function nested in the innermost visible frame's function, with its change stamp and
/// the stamps of the functions nested in it.
pub(super) fn note_nested(
    frames: &mut [Frame],
    function: NodeId,
    stamp: (u32, u32),
    nested: &[(NodeId, (u32, u32))],
) {
    if let Some(frame) = visible_frames(frames).next() {
        frame.nested.push((function, stamp));
        frame.nested.extend_from_slice(nested);
    }
}

/// Notes a jump to a hoisted function declared in a scope at `depth` in the visible frames of the
/// functions around that scope.
pub(super) fn note_jump(frames: &mut [Frame], depth: i32) {
    for frame in visible_frames(frames) {
        if frame.depth <= depth {
            frame.jumped = true;
        }
    }
}

/// Marks dirty the visible frame of every function whose scope is deeper than `depth`.
pub(super) fn mark_dirty_deeper_than(frames: &mut [Frame], depth: i32, why: &'static str) {
    for frame in visible_frames(frames) {
        if frame.depth <= depth {
            break;
        }
        frame.dirty.get_or_insert(why);
    }
}

/// Builds the free references of a clean frame, or `None` when a block of one is not below the
/// frame's base block.
pub(super) fn free_sites(ast: &Ast, frame: &FunctionFrame) -> Option<FreeSites> {
    let mut names: IndexMap<&JsString, NodeId> = IndexMap::default();
    let mut blocks: IndexMap<*const BasicBlock, (NodeId, u32)> = IndexMap::default();
    let mut sites = Vec::with_capacity(frame.refs.len());
    let mut path = Vec::new();
    let mut absent: IndexMap<&JsString, NodeId> = IndexMap::default();
    for &name in &frame.absent {
        let spelling = name.get_string_ref(ast);
        // Resolving an implicit name may make its variable.
        if crate::abstract_scope::ImplicitVar::of(spelling).is_some() {
            return None;
        }
        absent.entry(spelling).or_insert(name);
    }
    for (name, block) in &frame.refs {
        let entry = names.entry(name.get_string_ref(ast));
        let slot = u32::try_from(entry.index()).ok()?;
        entry.or_insert(*name);
        // The blocks from this one up to the base, which are indexed parents first.
        path.clear();
        let mut current = block;
        let index = loop {
            if Rc::ptr_eq(current, &frame.base) {
                break BASE;
            }
            if let Some(&index) = blocks.get_index_of(&Rc::as_ptr(current)).as_ref() {
                break u32::try_from(index).ok()?;
            }
            path.push(current);
            current = current.get_parent()?;
        };
        let mut parent = index;
        while let Some(b) = path.pop() {
            let (i, _) = blocks.insert_full(Rc::as_ptr(b), (b.get_root(), parent));
            parent = u32::try_from(i).ok()?;
        }
        sites.push((*name, slot, parent));
    }
    Some(FreeSites {
        first_names: names.into_values().collect(),
        blocks: blocks.into_values().collect(),
        sites: sites.into_boxed_slice(),
        absent_names: absent.into_values().collect(),
    })
}

/// Keeps a finished frame's function when it is clean (else forgets it); with `verify`, panics
/// unless the frame is clean with the kept free references. Returns the function's change stamp
/// and those of the functions nested in it.
pub(super) fn finish_frame(
    compiler: &mut AbstractCompiler,
    cache: CacheOf,
    frame: FunctionFrame,
) -> ((u32, u32), NestedStamps) {
    let free = if frame.dirty.is_some() {
        None
    } else {
        free_sites(compiler, &frame)
    };
    if let Some(expected) = &frame.verify {
        assert!(
            free.as_ref() == Some(&**expected),
            "CLOSURE_RS_CLEAN_SUBTREES=verify: the kept clean function at {} is now {}; kept free \
             references: {:?}; now: {:?}",
            frame.fn_node.get_location(compiler),
            frame.dirty.unwrap_or("clean"),
            expected,
            free
        );
    }
    let own = SyntacticScopeCache::subtree_stamp_with_flags(compiler, frame.fn_node);
    let nested: NestedStamps = frame.nested.into();
    match free {
        Some(free) => {
            let entry = CleanSubtree {
                own,
                nested: nested.clone(),
                is_declaration: NodeUtil::is_function_declaration(compiler, frame.fn_node),
                is_expression: NodeUtil::is_function_expression(compiler, frame.fn_node),
                is_arrow: frame.fn_node.is_arrow_function(compiler),
                jump_free: !frame.jumped,
                free: Arc::new(free),
            };
            cache(compiler).entries.insert(frame.fn_node, entry);
        }
        None => {
            cache(compiler).entries.swap_remove(&frame.fn_node);
        }
    }
    (own, nested)
}
