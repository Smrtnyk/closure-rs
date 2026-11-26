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

//! Rust-only (performance divergence): restores what the global scope builder computes for the
//! bundled externs from data prepared when closure-rs is built. Java types the default externs
//! from scratch in every compile.
//!
//! The "segment" is the global `NormalScopeBuilder`'s work on the bundled extern scripts: from the
//! entry of `should_traverse_script` for the first one to the end of `visit` of the last one. It
//! reads the scripts (unchanged parser snapshots, see `SnapshotUse`), the statements
//! `DeclaredGlobalExternsOnWindow` appended to them, their node properties (`InferConsts` marks
//! constants), the registry, the typed scope arena and the creator's state as they are when it
//! starts, and a few inputs of the compile through the registry (names marked non-nullable,
//! Closure namespaces, forward-declared types). `typed_externs.bin` holds, for the compile it was
//! made from (the test `typed_externs_data_is_current`), digests of the node state the segment
//! reads, the encoded state before it and the encoded state after it, and the names it looks up.
//! When every input matches (`Layout::compute`, `check`), the state after the segment is decoded
//! in place of running it (`restore`); nodes are relocated per script range. Otherwise the
//! ordinary path runs.
//!
//! What `[synthetic:externs]` declares (VarCheck's `var name;` for every name the program uses but
//! nothing declares, typed just before the segment) is left out of the encoded states, and the
//! properties and vars after it are numbered as if it were not there (`Synthetic`,
//! `SyntheticEntries`), so the data applies whatever names the program leaves undeclared.
//!
//! `CLOSURE_RS_TYPED_EXTERNS=off` always runs the ordinary path; `=verify` runs it and compares the
//! complete state after the segment with the data, failing on any difference.
//! `CLOSURE_RS_TYPED_EXTERNS_LOG=FILE` appends one line per compile saying whether the data applied.
//!
//! After a change to the type system, to typed scope creation or to the bundled externs, run
//! `CLOSURE_RS_REGENERATE_TYPED_EXTERNS=1 cargo test -p closure-jscomp
//! typed_externs_data_is_current`; that test, and `typed_externs_verify_on_bundled_externs`, fail
//! while the data is stale. Data made from other extern snapshots is never used (the build
//! script's fingerprint of the snapshots).

use super::{AbstractScopeBuilder, DeferredAction, ScopeBuilderKind, Stage, TypedScopeCreator};
use crate::abstract_compiler::AbstractCompiler;
use crate::abstract_scope::{AbstractScopeData, ImplicitVar};
use crate::abstract_var::AbstractVarData;
use crate::closure_coding_convention::ClosureCodingConvention;
use crate::coding_convention::CodingConvention;
use crate::coding_conventions::CodingConventions;
use crate::compiler_input::CompilerInput;
use crate::function_type_builder::ValidatorDescriptor;
use crate::node_traversal::NodeTraversal;
use crate::source_file::bundled_script::BundledScript;
use crate::typed_scope::{TypedScope, TypedScopeArena, TypedScopeData};
use crate::typed_var::{TypedVar, TypedVarData};
use closure_jstype::{
    JSTypeRegistry, TypeId,
    js_type::TypeValidator,
    js_type_registry::snapshot::{
        LookupLog, Reader, SnapshotEnv, SnapshotError, SnapshotResult, SyntheticEntries, Writer,
    },
    static_typed_scope::StaticTypedScope,
};
use closure_rhino::{
    fast_hash::{IndexMap, IndexSet},
    input_id::InputId,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId, Prop, SnapshotUse, typed_snapshot::Digest},
    static_source_file::SourceKind,
    token::Token,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    sync::{Arc, OnceLock},
};

/// The prepared data (`generate` writes it; a test checks that it is current).
static DATA: &[u8] = include_bytes!("typed_externs.bin");

const MAGIC: &[u8; 8] = b"CRSTXT01";

include!(concat!(env!("OUT_DIR"), "/bundled_externs_fingerprint.rs"));

/// How the segment is computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Always the ordinary path (`CLOSURE_RS_TYPED_EXTERNS=off`).
    Off,
    /// Restore from the data when it applies (the default).
    Restore,
    /// The ordinary path, compared with the data when it applies (`=verify`).
    Verify,
}

/// Rust-only: the compiler's control of the restore (tests and the generator set it before a
/// compile; the compile may run on another thread).
#[derive(Default, Debug)]
pub(crate) struct Control {
    /// Overrides `CLOSURE_RS_TYPED_EXTERNS`.
    pub(crate) mode: Option<Mode>,
    /// `generate`: Some(None) while a capture is requested, Some(Some(data)) once made.
    pub(crate) capture: Option<Option<Vec<u8>>>,
    /// The outcome of the last segment: "restored", "verified", "captured", or why the data did
    /// not apply.
    pub(crate) outcome: Option<String>,
    /// Tests: in verify mode, compare even where `check` refuses the data, and record a
    /// difference in `outcome` instead of failing.
    pub(crate) force_verify: bool,
    /// Tests: after a restore, encode the state again and compare it with the data.
    pub(crate) round_trip: bool,
}

fn mode(compiler: &AbstractCompiler) -> Mode {
    if let Some(mode) = compiler.typed_externs.mode {
        return mode;
    }
    static MODE: OnceLock<Mode> = OnceLock::new();
    *MODE.get_or_init(
        || match std::env::var("CLOSURE_RS_TYPED_EXTERNS").as_deref() {
            Ok("off") => Mode::Off,
            Ok("verify") => Mode::Verify,
            _ => Mode::Restore,
        },
    )
}

fn note_outcome(compiler: &mut AbstractCompiler, outcome: String) {
    if let Some(path) = std::env::var_os("CLOSURE_RS_TYPED_EXTERNS_LOG") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{outcome}");
        }
    }
    compiler.typed_externs.outcome = Some(outcome);
}

// ---------------------------------------------------------------------------------------------
// The data file.

/// The parsed data: the sections of `typed_externs.bin`.
struct Data {
    layout: &'static [u8],
    pre: &'static [u8],
    post: &'static [u8],
    non_nullable: &'static [u8],
    queried: &'static [u8],
    shared: &'static [u8],
}

/// What the layout section records.
struct RecordedLayout {
    names: Vec<Arc<str>>,
    range_lens: Vec<u32>,
    root_offsets: Vec<u32>,
    block_len: u32,
    prop_digest: u64,
    suffix_digest: u64,
}

impl Data {
    fn get() -> Option<&'static Data> {
        static PARSED: OnceLock<Option<Data>> = OnceLock::new();
        PARSED.get_or_init(|| Self::parse(DATA)).as_ref()
    }

    fn parse(bytes: &'static [u8]) -> Option<Data> {
        if bytes.len() < 16 || &bytes[..8] != MAGIC {
            return None;
        }
        let fingerprint = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        if fingerprint != BUNDLED_EXTERNS_FINGERPRINT {
            return None;
        }
        let mut rest = &bytes[16..];
        let mut sections = Vec::new();
        for _ in 0..6 {
            if rest.len() < 4 {
                return None;
            }
            let len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            if rest.len() < 4 + len {
                return None;
            }
            sections.push(&rest[4..4 + len]);
            rest = &rest[4 + len..];
        }
        rest.is_empty().then(|| Data {
            layout: sections[0],
            pre: sections[1],
            post: sections[2],
            non_nullable: sections[3],
            queried: sections[4],
            shared: sections[5],
        })
    }

    fn recorded_layout(&self) -> RecordedLayout {
        let mut r = Reader::new(self.layout);
        let count = r.u() as usize;
        let mut names = Vec::with_capacity(count);
        let mut range_lens = Vec::with_capacity(count);
        let mut root_offsets = Vec::with_capacity(count);
        for _ in 0..count {
            names.push(r.rs());
            range_lens.push(r.u() as u32);
            root_offsets.push(r.u() as u32);
        }
        let block_len = r.u() as u32;
        let prop_digest = r.u();
        let suffix_digest = r.u();
        RecordedLayout {
            names,
            range_lens,
            root_offsets,
            block_len,
            prop_digest,
            suffix_digest,
        }
    }

    fn assemble(
        layout: &[u8],
        pre: &[u8],
        post: &[u8],
        non_nullable: &[u8],
        queried: &[u8],
        shared: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&BUNDLED_EXTERNS_FINGERPRINT.to_le_bytes());
        for section in [layout, pre, post, non_nullable, queried, shared] {
            out.extend_from_slice(&(section.len() as u32).to_le_bytes());
            out.extend_from_slice(section);
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// The layout of the bundled externs in this compile's arena.

/// The fixed single nodes a snapshot may refer to, in this order (segments of length 0 or 1), then
/// the script ranges, then the block of appended statements, then the nodes the segment makes.
const ANCHORS: usize = 5;
const FIRST_RANGE: usize = ANCHORS;

/// Where the bundled externs are in this compile's arena: the segments node references are
/// relocated by, and the digests of the node state the segment reads.
struct Layout {
    scripts: Vec<NodeId>,
    names: Vec<String>,
    /// The start and length of every segment: the anchors (length 0 when absent), the script
    /// ranges, the block, the made nodes.
    segments: Vec<(u32, u32)>,
    /// Segment indexes sorted by start (non-empty segments only).
    by_start: Vec<usize>,
    /// `[synthetic:externs]` and the names and NAME nodes of its declarations.
    synthetic_script: NodeId,
    synthetic_names: Vec<JsString>,
    synthetic_nodes: Vec<NodeId>,
    root_offsets: Vec<u32>,
    /// The first appended statement of each script.
    suffixes: Vec<Option<NodeId>>,
    prop_digest: u64,
    suffix_digest: u64,
}

/// Feeds the shape of the tree at `root` (tokens, strings, numbers, positions, JSDoc) to `digest`.
fn shape_digest(
    ast: &Ast,
    root: NodeId,
    rel: &mut dyn FnMut(NodeId) -> u64,
    digest: &mut Digest,
    stack: &mut Vec<NodeId>,
) {
    stack.push(root);
    while let Some(n) = stack.pop() {
        digest.feed(rel(n));
        digest.feed(n.get_token(ast) as u64);
        digest.feed(n.get_child_count(ast) as u64);
        if let Some(string) = n.string_payload(ast) {
            digest.feed_units(string.as_units());
        }
        if n.is_number(ast) {
            digest.feed(n.get_double(ast).to_bits());
        }
        digest.feed(u64::from(n.get_lineno(ast) as u32));
        digest.feed(u64::from(n.get_charno(ast) as u32));
        match n.get_jsdoc_info_ref(ast) {
            None => digest.feed(0),
            Some(doc) => {
                digest.feed(1);
                doc.digest(rel, digest);
            }
        }
        let mut child = n.get_last_child(ast);
        while let Some(c) = child {
            stack.push(c);
            child = c.get_previous(ast);
        }
    }
}

/// The nodes of the tree at `root` in preorder.
fn preorder(ast: &Ast, root: NodeId, out: &mut Vec<NodeId>) {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        let mut child = n.get_last_child(ast);
        while let Some(c) = child {
            stack.push(c);
            child = c.get_previous(ast);
        }
    }
}

impl Layout {
    /// The layout when the externs root holds `[synthetic:externs]` followed by the bundled
    /// scripts from `first`, each the unchanged parser snapshot of its resource, and their
    /// appended statements are one block of nodes.
    fn compute(
        compiler: &AbstractCompiler,
        externs_root: NodeId,
        first: NodeId,
    ) -> Result<Layout, String> {
        let ast: &Ast = compiler;
        let synthetic_script = externs_root.get_first_child(ast).unwrap();
        if synthetic_script.get_next(ast) != Some(first) {
            return Err("the bundled externs do not follow the synthetic externs".into());
        }
        // VarCheck's declarations: `var name;`, one name each.
        let mut synthetic_names = Vec::new();
        let mut synthetic_nodes = Vec::new();
        for statement in synthetic_script.children(ast) {
            let name = statement.get_first_child(ast);
            match name {
                Some(name)
                    if statement.is_var(ast)
                        && statement.has_one_child(ast)
                        && name.is_name(ast)
                        && !name.has_children(ast)
                        && name.get_jsdoc_info_ref(ast).is_none()
                        && statement.get_jsdoc_info_ref(ast).is_none() =>
                {
                    synthetic_names.push(name.get_string(ast));
                    synthetic_nodes.push(name);
                }
                _ => return Err("an unexpected synthetic extern".into()),
            }
        }
        let mut scripts = Vec::new();
        let mut names = Vec::new();
        let mut ranges = Vec::new();
        let mut root_offsets = Vec::new();
        let mut suffixes = Vec::new();
        let mut script = Some(first);
        while let Some(s) = script {
            let Some((_, suffix)) =
                ast.parsed_script_record::<BundledScript>(s, SnapshotUse::SyntaxNamesAndJsDoc)
            else {
                break;
            };
            let (start, end) = ast.parsed_script_range(s).unwrap();
            scripts.push(s);
            names.push(s.get_source_file_name(ast).unwrap_or_default());
            ranges.push((start, end - start));
            root_offsets.push(s.arena_index() as u32 - start);
            suffixes.push(suffix);
            script = s.get_next(ast);
        }
        if scripts.is_empty() {
            return Err("no unchanged bundled script".into());
        }
        // The appended statements: one block of nodes, each reached exactly once.
        let mut appended = Vec::new();
        for &suffix in &suffixes {
            let mut statement = suffix;
            while let Some(st) = statement {
                preorder(ast, st, &mut appended);
                statement = st.get_next(ast);
            }
        }
        let block = match (
            appended.iter().map(|n| n.arena_index() as u32).min(),
            appended.iter().map(|n| n.arena_index() as u32).max(),
        ) {
            (Some(min), Some(max)) => {
                if (max - min + 1) as usize != appended.len() {
                    return Err("the appended statements are not one block".into());
                }
                (min, appended.len() as u32)
            }
            _ => (0, 0),
        };
        let root = externs_root.get_parent(ast).unwrap();
        let js_root = root.get_second_child(ast).unwrap();
        let name_table_root = compiler
            .type_registry_if_present()
            .map(JSTypeRegistry::snapshot_name_table_global_root);
        let bound = ast.implicit_template_bound_root();
        // An anchor inside a script range or the block is found there.
        let inside = |i: u32| {
            ranges
                .iter()
                .chain(std::iter::once(&block))
                .any(|&(start, len)| (start..start + len).contains(&i))
        };
        let anchor = |n: Option<NodeId>| match n {
            Some(n) if !inside(n.arena_index() as u32) => (n.arena_index() as u32, 1),
            _ => (0, 0),
        };
        let mut segments = vec![
            anchor(Some(root)),
            anchor(Some(externs_root)),
            anchor(Some(js_root)),
            anchor(name_table_root),
            anchor(bound),
        ];
        segments.extend(ranges.iter().copied());
        segments.push(block);
        // The nodes the segment makes (see `NewNodes`), after every node made before it.
        segments.push((ast.node_count(), 0));
        let mut by_start: Vec<usize> = (0..segments.len()).filter(|&i| segments[i].1 > 0).collect();
        by_start.sort_by_key(|&i| segments[i].0);
        for pair in by_start.windows(2) {
            let (a, b) = (segments[pair[0]], segments[pair[1]]);
            if a.0 + a.1 > b.0 {
                return Err("overlapping segments".into());
            }
        }
        let mut layout = Layout {
            scripts,
            names,
            segments,
            by_start,
            synthetic_script,
            synthetic_names,
            synthetic_nodes,
            root_offsets,
            suffixes: Vec::new(),
            prop_digest: 0,
            suffix_digest: 0,
        };
        layout.suffixes = suffixes;
        if layout
            .ranges()
            .iter()
            .chain(std::iter::once(&layout.block()))
            .any(|&(start, len)| ast.has_types_in(start, start + len))
        {
            return Err("a bundled extern node already has a type".into());
        }
        Ok(layout)
    }

    /// Computes the digests of the node state the segment reads: the properties of every node of
    /// the segments, then the shape and JSDoc of the block.
    fn compute_digests(&mut self, ast: &Ast) {
        let mut digest = Digest::default();
        {
            let mut rel = |n: NodeId| self.rel(n);
            for &(start, len) in self.ranges().iter().chain(std::iter::once(&self.block())) {
                ast.prop_digest_range(start, start + len, &mut rel, &mut digest);
            }
        }
        let prop_digest = digest.0;
        let mut digest = Digest::default();
        let mut stack = Vec::new();
        {
            let mut rel = |n: NodeId| self.rel(n);
            for &suffix in &self.suffixes {
                let mut statement = suffix;
                while let Some(st) = statement {
                    shape_digest(ast, st, &mut rel, &mut digest, &mut stack);
                    statement = st.get_next(ast);
                }
                digest.feed(u64::MAX);
            }
        }
        self.prop_digest = prop_digest;
        self.suffix_digest = digest.0;
    }

    fn ranges(&self) -> &[(u32, u32)] {
        &self.segments[FIRST_RANGE..FIRST_RANGE + self.scripts.len()]
    }

    fn block(&self) -> (u32, u32) {
        self.segments[FIRST_RANGE + self.scripts.len()]
    }

    /// Records that the segment made `count` nodes after the ones that existed when it started.
    fn set_new_nodes(&mut self, count: u32) {
        let index = FIRST_RANGE + self.scripts.len() + 1;
        self.segments[index].1 = count;
        self.by_start.retain(|&i| i != index);
        if count > 0 {
            self.by_start.push(index);
            self.by_start.sort_by_key(|&i| self.segments[i].0);
        }
    }

    fn new_nodes_start(&self) -> u32 {
        self.segments[FIRST_RANGE + self.scripts.len() + 1].0
    }

    fn last_script(&self) -> NodeId {
        *self.scripts.last().unwrap()
    }

    /// The segment and offset of `n`, if it is in one.
    fn node_ref(&self, n: NodeId) -> Option<(usize, u32)> {
        find_segment(&self.segments, &self.by_start, n)
    }

    fn node_at(&self, segment: usize, offset: u32) -> NodeId {
        NodeId::from_arena_index((self.segments[segment].0 + offset) as usize)
    }

    /// A node's segment and offset as one number (for digests), or u64::MAX outside the segments.
    fn rel(&self, n: NodeId) -> u64 {
        match self.node_ref(n) {
            Some((segment, offset)) => ((segment as u64) << 32) | u64::from(offset),
            None => u64::MAX,
        }
    }

    /// Every node of the script ranges and the block, in slot order.
    fn segment_nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.ranges()
            .iter()
            .copied()
            .chain(std::iter::once(self.block()))
            .flat_map(|(start, len)| {
                (start..start + len).map(|i| NodeId::from_arena_index(i as usize))
            })
    }

    fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u(self.scripts.len() as u64);
        for (i, name) in self.names.iter().enumerate() {
            w.rs(name);
            w.u(u64::from(self.ranges()[i].1));
            w.u(u64::from(self.root_offsets[i]));
        }
        w.u(u64::from(self.block().1));
        w.u(self.prop_digest);
        w.u(self.suffix_digest);
        w.finish()
    }

    /// Whether this compile's layout is the recorded one.
    fn matches(&self, recorded: &RecordedLayout) -> Result<(), String> {
        if self.scripts.len() != recorded.names.len() {
            return Err(format!(
                "{} unchanged bundled scripts, the data has {}",
                self.scripts.len(),
                recorded.names.len()
            ));
        }
        for i in 0..self.scripts.len() {
            if *self.names[i] != *recorded.names[i]
                || self.ranges()[i].1 != recorded.range_lens[i]
                || self.root_offsets[i] != recorded.root_offsets[i]
            {
                return Err(format!("bundled script {i} ({}) differs", self.names[i]));
            }
        }
        if self.block().1 != recorded.block_len {
            return Err("the appended statements differ in size".into());
        }
        Ok(())
    }

    /// Whether the digests (`compute_digests`) are the recorded ones.
    fn digests_match(&self, recorded: &RecordedLayout) -> Result<(), String> {
        if self.suffix_digest != recorded.suffix_digest {
            return Err("the appended statements differ".into());
        }
        if self.prop_digest != recorded.prop_digest {
            return Err("the properties of the bundled extern nodes differ".into());
        }
        Ok(())
    }
}

fn find_segment(segments: &[(u32, u32)], by_start: &[usize], n: NodeId) -> Option<(usize, u32)> {
    let i = n.arena_index() as u32;
    let k = by_start.partition_point(|&s| segments[s].0 <= i);
    if k == 0 {
        return None;
    }
    let s = by_start[k - 1];
    let (start, len) = segments[s];
    (i < start + len).then_some((s, i - start))
}

// ---------------------------------------------------------------------------------------------
// The synthetic externs.

/// What the synthetic externs (`var name;` for each name the program uses but nothing declares,
/// typed just before the segment) added: in the registry (`SyntheticEntries`), and a var each at
/// the end of the typed scope arena and of the global scope's declarations. The snapshot leaves
/// them out and numbers what comes after them as if they were not there, so that the data applies
/// whatever names the program leaves undeclared; a restore puts them back where they were.
struct Synthetic {
    entries: SyntheticEntries,
    names: Vec<JsString>,
    /// The arena slot of the first synthetic var, and the global scope's position of its
    /// declaration.
    var_start: u32,
    declaration_position: usize,
    vars: Vec<TypedVar>,
    global: TypedScope,
}

impl Synthetic {
    fn find(
        compiler: &AbstractCompiler,
        layout: &Layout,
        global: TypedScope,
    ) -> Result<Synthetic, String> {
        let registry = compiler
            .type_registry_if_present()
            .ok_or_else(|| "no type registry".to_string())?;
        let entries = registry
            .snapshot_synthetic_entries(&layout.synthetic_names, &layout.synthetic_nodes)?;
        let k = layout.synthetic_names.len();
        let arena = compiler.typed_scope_arena();
        if arena.vars.len() < k {
            return Err("fewer vars than synthetic externs".into());
        }
        let var_start = arena.vars.len() - k;
        let declarations = &arena.scopes[global.index()].abstract_scope.vars;
        let declaration_position = declarations
            .len()
            .checked_sub(k)
            .ok_or("the global scope")?;
        let tail: Vec<(&JsString, &TypedVar)> =
            declarations.iter().skip(declaration_position).collect();
        let mut vars = Vec::with_capacity(k);
        for (j, (name, &declared)) in tail.into_iter().enumerate() {
            let var = &arena.vars[var_start + j];
            let handle = var_handle((var_start + j + 1) as u64);
            if var.abstract_var.name_node != Some(layout.synthetic_nodes[j])
                || var.abstract_var.scope != Some(global)
                || name != &layout.synthetic_names[j]
                || declared != handle
            {
                return Err(format!(
                    "the var of synthetic extern {}",
                    layout.synthetic_names[j]
                ));
            }
            vars.push(handle);
        }
        Ok(Synthetic {
            entries,
            names: layout.synthetic_names.clone(),
            var_start: var_start as u32,
            declaration_position,
            vars,
            global,
        })
    }

    fn count(&self) -> u32 {
        self.vars.len() as u32
    }

    /// The var's number in the snapshot (1-based, the synthetic vars left out), or None for a
    /// synthetic var.
    fn norm_var(&self, var: TypedVar) -> Option<u64> {
        let index = var.index() as u32;
        if index < self.var_start {
            Some(u64::from(index) + 1)
        } else if index < self.var_start + self.count() {
            None
        } else {
            Some(u64::from(index - self.count()) + 1)
        }
    }

    /// The var a snapshot number stands for.
    fn var(&self, number: u64) -> TypedVar {
        let index = number as u32 - 1;
        let index = if index < self.var_start {
            index
        } else {
            index + self.count()
        };
        var_handle(u64::from(index) + 1)
    }
}

// ---------------------------------------------------------------------------------------------
// The environment of the registry snapshot: relocation, JSDoc, scopes, validators, inputs.

struct Env<'l> {
    layout: &'l Layout,
    /// Encoding: the arena, and the node holding each JSDoc (by address), made on first use.
    ast: Option<&'l Ast>,
    jsdoc_nodes: Option<IndexMap<usize, NodeId>>,
    /// Encoding and decoding: the canonical view of the global scope.
    global_view: Option<Arc<dyn StaticTypedScope>>,
    /// Decoding: the report function of the validators.
    report: Option<crate::function_type_builder::CompilerReportFn>,
    /// The inputs of the bundled scripts and of the synthetic externs, by script.
    inputs: Vec<Option<CompilerInput>>,
    input_ids: Vec<Option<Arc<InputId>>>,
    input_index: IndexMap<String, usize>,
    /// Node offsets are written as the difference to the last one in the same segment.
    last_offsets: Vec<u32>,
    synthetic: &'l Synthetic,
}

impl<'l> Env<'l> {
    fn new(
        compiler: &AbstractCompiler,
        layout: &'l Layout,
        synthetic: &'l Synthetic,
        global: TypedScope,
    ) -> Self {
        let mut inputs = Vec::with_capacity(layout.scripts.len() + 1);
        let mut input_ids = Vec::with_capacity(layout.scripts.len() + 1);
        let mut input_index = IndexMap::default();
        let synthetic_script = layout.synthetic_script;
        for (i, &script) in layout
            .scripts
            .iter()
            .chain(std::iter::once(&synthetic_script))
            .enumerate()
        {
            let id = script.get_input_id(compiler);
            if let Some(id) = &id {
                input_index.insert(id.get_id_name().to_owned(), i);
            }
            inputs.push(id.as_ref().and_then(|id| compiler.get_input(id).cloned()));
            input_ids.push(id);
        }
        Self {
            layout,
            ast: None,
            jsdoc_nodes: None,
            global_view: Some(global.as_static_typed_scope_arc(compiler)),
            report: None,
            inputs,
            input_ids,
            input_index,
            last_offsets: vec![0; layout.segments.len()],
            synthetic,
        }
    }

    /// An environment for encoding the state of `compiler`.
    fn encoding(
        compiler: &'l AbstractCompiler,
        layout: &'l Layout,
        synthetic: &'l Synthetic,
        global: TypedScope,
    ) -> Self {
        let mut env = Self::new(compiler, layout, synthetic, global);
        env.ast = Some(compiler);
        env
    }

    fn jsdoc_node(&mut self, doc: &Arc<JSDocInfo>) -> Option<NodeId> {
        if self.jsdoc_nodes.is_none() {
            let ast = self.ast.expect("an encoding environment");
            let mut map = IndexMap::default();
            for n in self.layout.segment_nodes() {
                if let Some(doc) = n.get_jsdoc_info_ref(ast) {
                    map.entry(Arc::as_ptr(doc) as usize).or_insert(n);
                }
            }
            self.jsdoc_nodes = Some(map);
        }
        self.jsdoc_nodes
            .as_ref()
            .unwrap()
            .get(&(Arc::as_ptr(doc) as usize))
            .copied()
    }

    /// The strings the nodes of the script ranges and the block hold, each
    /// by its first node: a restore takes them from the nodes (`Reader::new_with`).
    fn node_strings(&self) -> IndexMap<JsString, (u64, u64)> {
        let ast = self.ast.expect("an encoding environment");
        let mut map = IndexMap::default();
        for n in self.layout.segment_nodes() {
            if let Some(string) = n.string_payload(ast)
                && !map.contains_key(string)
            {
                let (segment, offset) = self.layout.node_ref(n).unwrap();
                map.insert(string.clone(), (segment as u64, u64::from(offset)));
            }
        }
        map
    }

    fn write_input_id(&self, w: &mut Writer, id: Option<&Arc<InputId>>) {
        match id {
            None => w.u(0),
            Some(id) => match self.input_index.get(id.get_id_name()) {
                Some(&k) => w.u(k as u64 + 2),
                None => {
                    w.u(1);
                    w.rs(id.get_id_name());
                }
            },
        }
    }

    fn read_input_id(&self, r: &mut Reader<'_>) -> Option<Arc<InputId>> {
        match r.u() {
            0 => None,
            1 => Some(Arc::new(InputId::new(r.rs().to_string()))),
            k => self.input_ids[k as usize - 2].clone(),
        }
    }

    fn write_input(&self, w: &mut Writer, input: Option<&CompilerInput>) -> SnapshotResult<()> {
        match input {
            None => w.u(0),
            Some(input) => match self.input_index.get(input.get_input_id().get_id_name()) {
                Some(&k) => w.u(k as u64 + 1),
                None => {
                    return Err(SnapshotError(format!(
                        "a var of input {} outside the bundled externs",
                        input.get_name()
                    )));
                }
            },
        }
        Ok(())
    }

    fn read_input(&self, r: &mut Reader<'_>) -> Option<CompilerInput> {
        match r.u() {
            0 => None,
            k => self.inputs[k as usize - 1].clone(),
        }
    }
}

impl SnapshotEnv for Env<'_> {
    fn write_node(&mut self, w: &mut Writer, n: NodeId) -> SnapshotResult<()> {
        match self.layout.node_ref(n) {
            Some((segment, offset)) => {
                w.u(segment as u64);
                let last = &mut self.last_offsets[segment];
                w.i(i64::from(offset) - i64::from(*last));
                *last = offset;
                Ok(())
            }
            None => Err(SnapshotError(format!(
                "node {} outside the bundled externs",
                n.arena_index()
            ))),
        }
    }

    fn read_node(&mut self, r: &mut Reader<'_>) -> NodeId {
        let segment = r.u() as usize;
        let last = &mut self.last_offsets[segment];
        let offset = (i64::from(*last) + r.i()) as u32;
        *last = offset;
        self.layout.node_at(segment, offset)
    }

    fn write_jsdoc(&mut self, w: &mut Writer, doc: &Arc<JSDocInfo>) -> SnapshotResult<()> {
        match self.jsdoc_node(doc) {
            Some(n) => self.write_node(w, n),
            None => Err(SnapshotError("a JSDoc of no bundled extern node".into())),
        }
    }

    fn read_jsdoc(&mut self, r: &mut Reader<'_>, ast: &Ast) -> Arc<JSDocInfo> {
        let n = self.read_node(r);
        n.get_jsdoc_info_ref(ast).expect("restored JSDoc").clone()
    }

    fn write_scope(
        &mut self,
        w: &mut Writer,
        scope: &Arc<dyn StaticTypedScope>,
    ) -> SnapshotResult<()> {
        match &self.global_view {
            Some(global) if Arc::ptr_eq(global, scope) => {
                w.u(0);
                Ok(())
            }
            _ => Err(SnapshotError("a scope other than the global scope".into())),
        }
    }

    fn read_scope(&mut self, r: &mut Reader<'_>) -> Arc<dyn StaticTypedScope> {
        let _ = r.u();
        self.global_view.clone().expect("global scope view")
    }

    fn write_validator(&mut self, w: &mut Writer, validator: &TypeValidator) -> SnapshotResult<()> {
        let (kind, error_root, name) = match ValidatorDescriptor::of(validator) {
            Some(ValidatorDescriptor::Extended {
                error_root,
                formatted_fn_name,
            }) => (0, error_root, formatted_fn_name),
            Some(ValidatorDescriptor::Implemented {
                error_root,
                formatted_fn_name,
            }) => (1, error_root, formatted_fn_name),
            None => return Err(SnapshotError("an unknown validator".into())),
        };
        w.u(kind);
        self.write_node(w, error_root)?;
        w.rs(&name);
        Ok(())
    }

    fn read_validator(&mut self, r: &mut Reader<'_>) -> TypeValidator {
        let kind = r.u();
        let error_root = self.read_node(r);
        let formatted_fn_name = r.rs().to_string();
        let descriptor = if kind == 0 {
            ValidatorDescriptor::Extended {
                error_root,
                formatted_fn_name,
            }
        } else {
            ValidatorDescriptor::Implemented {
                error_root,
                formatted_fn_name,
            }
        };
        descriptor.make_with(self.report.clone().expect("validator report function"))
    }
}

// ---------------------------------------------------------------------------------------------
// The compiler's part of the state: the typed scope arena, the creator, the builder, node types and
// node properties.

fn write_source_kind(w: &mut Writer, kind: Option<SourceKind>) {
    w.u(match kind {
        None => 0,
        Some(SourceKind::STRONG) => 1,
        Some(SourceKind::WEAK) => 2,
        Some(SourceKind::EXTERN) => 3,
        Some(SourceKind::NON_CODE) => 4,
    });
}

fn read_source_kind(r: &mut Reader<'_>) -> Option<SourceKind> {
    match r.u() {
        0 => None,
        1 => Some(SourceKind::STRONG),
        2 => Some(SourceKind::WEAK),
        3 => Some(SourceKind::EXTERN),
        _ => Some(SourceKind::NON_CODE),
    }
}

fn implicit_var_code(v: ImplicitVar) -> u64 {
    match v {
        ImplicitVar::ARGUMENTS => 0,
        ImplicitVar::EXPORTS => 1,
        ImplicitVar::SUPER => 2,
        ImplicitVar::THIS => 3,
    }
}

fn implicit_var_of(code: u64) -> ImplicitVar {
    match code {
        0 => ImplicitVar::ARGUMENTS,
        1 => ImplicitVar::EXPORTS,
        2 => ImplicitVar::SUPER,
        _ => ImplicitVar::THIS,
    }
}

fn scope_handle(index: u64) -> TypedScope {
    TypedScope(NonZeroU32::new(index as u32).unwrap())
}

fn var_handle(index: u64) -> TypedVar {
    TypedVar(NonZeroU32::new(index as u32).unwrap())
}

fn write_string_set(w: &mut Writer, set: &IndexSet<JsString>) {
    w.u(set.len() as u64);
    for s in set {
        w.js(s);
    }
}

fn read_string_set(r: &mut Reader<'_>) -> IndexSet<JsString> {
    let len = r.u() as usize;
    let mut set = IndexSet::with_capacity_and_hasher(len, Default::default());
    for _ in 0..len {
        set.insert(r.js());
    }
    set
}

fn write_arena(w: &mut Writer, env: &mut Env<'_>, arena: &TypedScopeArena) -> SnapshotResult<()> {
    let synthetic = env.synthetic;
    // `watch` is the change counter of a Rust-only cache (`VarWatch`), not typed state: it is not
    // encoded, and a restore reports a change to it (`read_arena`).
    let TypedScopeArena {
        scopes,
        vars,
        watch: _,
    } = arena;
    w.label(|| "arena: scopes".into());
    w.u(scopes.len() as u64);
    for scope in scopes {
        // The canonical view is made on first use and is not part of the state (it is the same
        // object whenever it is made).
        let TypedScopeData {
            abstract_scope:
                AbstractScopeData {
                    vars,
                    implicit_vars,
                    root_node,
                },
            parent,
            module,
            is_bottom,
            reserved_names,
            qualified_roots,
            has_this_qualified,
            view: _,
        } = scope;
        // The synthetic externs' declarations are left out.
        let declared: Vec<(&JsString, u64)> = vars
            .iter()
            .filter_map(|(name, &var)| synthetic.norm_var(var).map(|n| (name, n)))
            .collect();
        w.u(declared.len() as u64);
        for (name, number) in declared {
            w.js(name);
            w.u(number);
        }
        w.u(implicit_vars.len() as u64);
        for (&k, &var) in implicit_vars {
            w.u(implicit_var_code(k));
            w.u(synthetic
                .norm_var(var)
                .ok_or_else(|| SnapshotError("an implicit synthetic var".into()))?);
        }
        env.write_node(w, *root_node)?;
        w.u(parent.map_or(0, |p| p.0.get() as u64));
        if module.is_some() {
            return Err(SnapshotError("a module scope".into()));
        }
        w.b(*is_bottom);
        write_string_set(w, reserved_names);
        write_string_set(w, qualified_roots);
        w.b(*has_this_qualified);
    }
    w.label(|| "arena: vars".into());
    let start = synthetic.var_start as usize;
    let k = synthetic.count() as usize;
    w.u((vars.len() - k) as u64);
    for (i, var) in vars.iter().enumerate() {
        if (start..start + k).contains(&i) {
            continue;
        }
        w.label(|| format!("var {i}"));
        let TypedVarData {
            abstract_var:
                AbstractVarData {
                    name,
                    name_node,
                    implicit_goog_namespace_strength,
                    input,
                    index,
                    scope,
                },
            type_,
            marked_escaped,
            marked_assigned_exactly_once,
            type_inferred,
            view: _,
        } = var;
        // A global declared after the synthetic externs counts them in its index.
        let after = i >= start + k && *scope == Some(synthetic.global);
        let index = if after { *index - k as i32 } else { *index };
        let number = if i < start { i } else { i - k };
        w.flags(&[
            name_node.is_some(),
            *marked_escaped,
            *marked_assigned_exactly_once,
            *type_inferred,
        ]);
        w.js(name);
        if let Some(n) = name_node {
            env.write_node(w, *n)?;
        }
        write_source_kind(w, *implicit_goog_namespace_strength);
        env.write_input(w, input.as_ref())?;
        w.i(i64::from(index) - number as i64);
        w.u(scope.map_or(0, |s| s.0.get() as u64));
        w.oty(*type_);
    }
    Ok(())
}

/// Replaces the arena's state with the state in `r`. The scopes are the same ones (the pre-state
/// check compared them); the vars before the synthetic externs' are the same ones, the synthetic
/// externs' stay, and the vars after them are new.
fn read_arena(r: &mut Reader<'_>, env: &mut Env<'_>, arena: &mut TypedScopeArena) {
    // The restore rewrites vars wholesale: whatever watches them must look again.
    arena.watch.bump();
    let synthetic = env.synthetic;
    let scope_count = r.u() as usize;
    assert_eq!(
        scope_count,
        arena.scopes.len(),
        "typed externs: scope count"
    );
    for (s, scope) in arena.scopes.iter_mut().enumerate() {
        let len = r.u() as usize;
        let mut declared = Vec::with_capacity(len + synthetic.vars.len());
        for _ in 0..len {
            let name = r.js();
            declared.push((name, synthetic.var(r.u())));
        }
        if s == synthetic.global.index() {
            let position = synthetic.declaration_position;
            declared.splice(
                position..position,
                synthetic
                    .names
                    .iter()
                    .cloned()
                    .zip(synthetic.vars.iter().copied()),
            );
        }
        let mut vars = crate::var_map::VarMap::default();
        for (name, var) in declared {
            vars.insert(name, var);
        }
        let len = r.u() as usize;
        let mut implicit_vars = BTreeMap::new();
        for _ in 0..len {
            let k = implicit_var_of(r.u());
            implicit_vars.insert(k, synthetic.var(r.u()));
        }
        let root_node = env.read_node(r);
        let parent = match r.u() {
            0 => None,
            p => Some(scope_handle(p)),
        };
        scope.abstract_scope = AbstractScopeData {
            vars,
            implicit_vars,
            root_node,
        };
        scope.parent = parent;
        scope.module = None;
        scope.is_bottom = r.b();
        scope.reserved_names = read_string_set(r);
        scope.qualified_roots = read_string_set(r);
        scope.has_this_qualified = r.b();
    }
    let start = synthetic.var_start as usize;
    let k = synthetic.count() as usize;
    let var_count = r.u() as usize;
    let existing = arena.vars.len();
    assert_eq!(existing, start + k, "typed externs: var count");
    assert!(var_count >= start, "typed externs: var count");
    arena.vars.reserve(var_count - start);
    for number in 0..var_count {
        let flags = r.u();
        let name = r.js();
        let name_node = (flags & 1 != 0).then(|| env.read_node(r));
        let implicit_goog_namespace_strength = read_source_kind(r);
        let input = env.read_input(r);
        let index = (r.i() + number as i64) as i32;
        let scope = match r.u() {
            0 => None,
            s => Some(scope_handle(s)),
        };
        let type_ = r.oty();
        let marked_escaped = flags & 2 != 0;
        let marked_assigned_exactly_once = flags & 4 != 0;
        let type_inferred = flags & 8 != 0;
        let after = number >= start;
        let index = if after && scope == Some(synthetic.global) {
            index + k as i32
        } else {
            index
        };
        let abstract_var = AbstractVarData {
            name,
            name_node,
            implicit_goog_namespace_strength,
            input,
            index,
            scope,
        };
        if !after {
            let var = &mut arena.vars[number];
            var.abstract_var = abstract_var;
            var.type_ = type_;
            var.marked_escaped = marked_escaped;
            var.marked_assigned_exactly_once = marked_assigned_exactly_once;
            var.type_inferred = type_inferred;
        } else {
            arena.vars.push(TypedVarData {
                abstract_var,
                type_,
                marked_escaped,
                marked_assigned_exactly_once,
                type_inferred,
                view: OnceLock::new(),
            });
        }
    }
}

/// The creator's state the segment changes or depends on. Of its other fields, the validator and
/// the coding convention are fixed for the compile (`check` compares the convention), and the rest
/// are what the passes before the segment found in the whole program, which the segment reads
/// only with keys of bundled extern nodes or names that program code cannot enter (globals are
/// never escaped or assigned there, see `FirstOrderFunctionAnalyzer#visit`), or not at all:
/// `module_map`, `module_import_resolver` and `process_closure_primitives` serve module scopes,
/// imports and `goog.provide` calls, of which the bundled externs have none, and
/// `omit_empty_extern_stub_scopes` serves the passes before and after the segment.
/// `input_digest` checks that the segment changes none of them.
fn write_creator(
    w: &mut Writer,
    env: &mut Env<'_>,
    creator: &TypedScopeCreator,
) -> SnapshotResult<()> {
    let TypedScopeCreator {
        validator: _,
        coding_convention: _,
        module_map: _,
        metadata_map: _,
        module_import_resolver: _,
        process_closure_primitives: _,
        memoized,
        reserved_names_for_scope: _,
        functions_with_non_empty_returns: _,
        escaped_var_names: _,
        assigned_var_names: _,
        unknown_type,
        weak_imports,
        unresolved_nodes,
        undeclared_names_for_closure: _,
        provided_names_from_call: _,
        stage,
        omit_empty_extern_stub_scopes: _,
    } = creator;
    w.label(|| "creator".into());
    w.u(memoized.len() as u64);
    for (&root, scope) in memoized {
        env.write_node(w, root)?;
        w.u(scope.0.get() as u64);
    }
    w.ty(*unknown_type);
    if !weak_imports.is_empty() {
        return Err(SnapshotError("weak module imports".into()));
    }
    w.label(|| "creator: unresolved nodes".into());
    w.u(unresolved_nodes.len() as u64);
    for &n in unresolved_nodes {
        env.write_node(w, n)?;
    }
    w.u(match stage {
        Stage::BUILDING => 0,
        Stage::FROZEN => 1,
    });
    Ok(())
}

fn read_creator(r: &mut Reader<'_>, env: &mut Env<'_>, creator: &mut TypedScopeCreator) {
    let len = r.u() as usize;
    let mut memoized = IndexMap::with_capacity_and_hasher(len, Default::default());
    for _ in 0..len {
        let root = env.read_node(r);
        memoized.insert(root, scope_handle(r.u()));
    }
    creator.memoized = memoized;
    creator.unknown_type = r.ty();
    let len = r.u() as usize;
    let mut unresolved = Vec::with_capacity(len);
    for _ in 0..len {
        unresolved.push(env.read_node(r));
    }
    creator.unresolved_nodes = unresolved;
    creator.stage = if r.u() == 0 {
        Stage::BUILDING
    } else {
        Stage::FROZEN
    };
}

fn write_builder(
    w: &mut Writer,
    env: &mut Env<'_>,
    builder: &AbstractScopeBuilder<'_>,
) -> SnapshotResult<()> {
    let AbstractScopeBuilder {
        creator: _,
        kind,
        current_scope,
        current_hoist_scope,
        has_source_name,
        input_id,
        deferred_actions,
        deferred_statements,
        bundled_declarations: _,
        typed_externs: _,
    } = builder;
    w.label(|| "builder".into());
    w.u(match kind {
        ScopeBuilderKind::NormalScopeBuilder => 0,
        ScopeBuilderKind::FunctionScopeBuilder => 1,
        ScopeBuilderKind::ClassScopeBuilder => 2,
    });
    w.u(current_scope.0.get() as u64);
    w.u(current_hoist_scope.0.get() as u64);
    w.b(*has_source_name);
    env.write_input_id(w, input_id.as_ref());
    w.u(deferred_actions.len() as u64);
    for (key, action) in deferred_actions {
        env.write_node(w, *key)?;
        match action {
            DeferredAction::DefineObjectLiteral(n) => {
                w.u(0);
                env.write_node(w, *n)?;
            }
            DeferredAction::ResolveStubDeclaration {
                n,
                is_extern,
                owner_name,
            } => {
                w.u(1);
                env.write_node(w, *n)?;
                w.b(*is_extern);
                w.js(owner_name);
            }
        }
    }
    w.u(deferred_statements.len() as u64);
    for &n in deferred_statements {
        env.write_node(w, n)?;
    }
    Ok(())
}

fn read_builder(r: &mut Reader<'_>, env: &mut Env<'_>, builder: &mut AbstractScopeBuilder<'_>) {
    let _kind = r.u();
    builder.current_scope = scope_handle(r.u());
    builder.current_hoist_scope = scope_handle(r.u());
    builder.has_source_name = r.b();
    builder.input_id = env.read_input_id(r);
    let len = r.u() as usize;
    let mut actions = Vec::with_capacity(len);
    for _ in 0..len {
        let key = env.read_node(r);
        let action = if r.u() == 0 {
            DeferredAction::DefineObjectLiteral(env.read_node(r))
        } else {
            let n = env.read_node(r);
            let is_extern = r.b();
            DeferredAction::ResolveStubDeclaration {
                n,
                is_extern,
                owner_name: r.js(),
            }
        };
        actions.push((key, action));
    }
    builder.deferred_actions = actions;
    let len = r.u() as usize;
    let mut statements = IndexSet::with_capacity_and_hasher(len, Default::default());
    for _ in 0..len {
        statements.insert(env.read_node(r));
    }
    builder.deferred_statements = statements;
    builder.bundled_declarations = None;
}

/// The types of the segment's nodes, per segment in slot order.
fn write_node_types(w: &mut Writer, layout: &Layout, ast: &Ast) {
    w.label(|| "node types".into());
    let segments = layout
        .ranges()
        .iter()
        .copied()
        .chain(std::iter::once(layout.block()));
    for (start, len) in segments {
        let typed: Vec<(u32, TypeId)> = (0..len)
            .filter_map(|offset| {
                NodeId::from_arena_index((start + offset) as usize)
                    .get_jstype(ast)
                    .map(|t| (offset, t))
            })
            .collect();
        w.u(typed.len() as u64);
        let mut previous = 0;
        for (offset, t) in typed {
            w.u(u64::from(offset - previous));
            previous = offset;
            w.ty(t);
        }
    }
}

fn read_node_types(r: &mut Reader<'_>, layout: &Layout, ast: &mut Ast) {
    let mut types = Vec::new();
    for &(start, _) in layout
        .ranges()
        .iter()
        .chain(std::iter::once(&layout.block()))
    {
        let count = r.u() as usize;
        let mut offset = 0u32;
        for _ in 0..count {
            offset += r.u() as u32;
            types.push((NodeId::from_arena_index((start + offset) as usize), r.ty()));
        }
    }
    ast.set_node_jstypes(types.into_iter());
}

const TYPEDEF_TYPE_BIT: u64 = 1 << (Prop::TYPEDEF_TYPE as u8);

/// The `TYPEDEF_TYPE` properties the segment sets (the only node properties it changes: a
/// capture fails on any other change, see `PropWatch`).
fn write_node_props(
    w: &mut Writer,
    env: &mut Env<'_>,
    set: &[(NodeId, TypeId)],
) -> SnapshotResult<()> {
    w.label(|| "node properties".into());
    w.u(set.len() as u64);
    for &(n, t) in set {
        env.write_node(w, n)?;
        w.ty(t);
    }
    Ok(())
}

fn read_node_props(r: &mut Reader<'_>, env: &mut Env<'_>, ast: &mut Ast) -> Vec<(NodeId, TypeId)> {
    let len = r.u() as usize;
    let mut set = Vec::with_capacity(len);
    for _ in 0..len {
        let n = env.read_node(r);
        let t = r.ty();
        n.set_typedef_type_prop(ast, Some(t));
        set.push((n, t));
    }
    set
}

/// The node state of the whole arena before the segment, to find what it changed.
struct PropWatch {
    states: Vec<(u64, usize)>,
    typed: Vec<bool>,
}

impl PropWatch {
    fn new(ast: &Ast) -> Self {
        let node_count = ast.node_count();
        let states = (0..node_count as usize)
            .map(|i| ast.prop_state(NodeId::from_arena_index(i)))
            .collect();
        let typed = (0..node_count as usize)
            .map(|i| NodeId::from_arena_index(i).get_jstype(ast).is_some())
            .collect();
        Self { states, typed }
    }

    /// The `TYPEDEF_TYPE` properties set since, or what else changed.
    fn changes(&self, ast: &Ast, layout: &Layout) -> Result<Vec<(NodeId, TypeId)>, String> {
        let mut set = Vec::new();
        for (i, &(mask, list)) in self.states.iter().enumerate() {
            let n = NodeId::from_arena_index(i);
            let (new_mask, new_list) = ast.prop_state(n);
            if (new_mask, new_list) != (mask, list) {
                let head = ast.prop_list_head_item(n);
                match (n.get_typedef_type_prop(ast), head) {
                    (Some(t), Some((prop, next)))
                        if new_mask == mask | TYPEDEF_TYPE_BIT
                            && mask & TYPEDEF_TYPE_BIT == 0
                            && prop == Prop::TYPEDEF_TYPE as u8
                            && next == list =>
                    {
                        set.push((n, t));
                    }
                    _ => return Err(format!("node {i}: a property change other than a typedef")),
                }
            }
            let typed = n.get_jstype(ast).is_some();
            if typed != self.typed[i] && layout.node_ref(n).is_none_or(|(s, _)| s < FIRST_RANGE) {
                return Err(format!("node {i} outside the bundled externs got a type"));
            }
        }
        Ok(set)
    }
}

/// The detached nodes the segment makes: `FunctionTypeBuilder#inferParameterTypes(JSDocInfo)`
/// builds a fake parameter list (`IR.paramList()` with an `IR.name` per documented parameter) for
/// a function declared without a literal. Java leaves them to the collector; here they stay in the
/// arena, so a restore makes the same nodes in the same order.
fn write_new_nodes(w: &mut Writer, ast: &Ast, start: u32) -> Result<(), String> {
    w.label(|| "new nodes".into());
    let count = ast.node_count() - start;
    w.u(u64::from(count));
    for i in start..start + count {
        let n = NodeId::from_arena_index(i as usize);
        let fresh = || format!("node {i} the segment made ({:?})", n.get_token(ast));
        if n.get_jsdoc_info_ref(ast).is_some()
            || ast.prop_state(n) != (0, 0)
            || n.get_jstype(ast).is_some()
        {
            return Err(format!("{} has JSDoc, properties or a type", fresh()));
        }
        // Each node is appended to its parent right after it is made.
        let parent = n.get_parent(ast);
        if let Some(parent) = parent {
            let p = parent.arena_index() as u32;
            if p < start || p >= i {
                return Err(format!(
                    "{} is not appended to a node made before it",
                    fresh()
                ));
            }
        }
        let mut previous = None;
        for child in n.children(ast) {
            let c = child.arena_index() as u32;
            if c <= i || previous.is_some_and(|p| c <= p) {
                return Err(format!("{} has children it was not made before", fresh()));
            }
            previous = Some(c);
        }
        match n.get_token(ast) {
            Token::PARAM_LIST => w.u(0),
            Token::NAME => {
                w.u(1);
                w.js(n.get_string_ref(ast));
            }
            _ => return Err(format!("{} is of an unexpected kind", fresh())),
        }
        w.u(parent.map_or(0, |p| u64::from(p.arena_index() as u32 - start) + 1));
        w.i(i64::from(n.get_lineno(ast)));
        w.i(i64::from(n.get_charno(ast)));
    }
    Ok(())
}

fn read_new_nodes(r: &mut Reader<'_>, ast: &mut Ast) -> u32 {
    let start = ast.node_count();
    let count = r.u() as u32;
    for _ in 0..count {
        let n = if r.u() == 0 {
            closure_rhino::ir::IR::param_list(ast, &[])
        } else {
            let name = r.js();
            closure_rhino::ir::IR::name(ast, name)
        };
        if let Some(parent) = r.u().checked_sub(1) {
            NodeId::from_arena_index((start as u64 + parent) as usize).add_child_to_back(ast, n);
        }
        let lineno = r.i() as i32;
        let charno = r.i() as i32;
        assert!(
            n.get_lineno(ast) == lineno && n.get_charno(ast) == charno,
            "typed externs: a made node's position"
        );
    }
    count
}

// ---------------------------------------------------------------------------------------------
// The segment.

/// The encoded registry, arena, creator and builder (the pre-state; with node types and properties
/// the post-state).
fn encode_state(
    builder: &AbstractScopeBuilder<'_>,
    compiler: &AbstractCompiler,
    env: &mut Env<'_>,
    w: &mut Writer,
) -> SnapshotResult<()> {
    let registry = compiler
        .type_registry_if_present()
        .ok_or_else(|| SnapshotError("no type registry".into()))?;
    let synthetic = env.synthetic;
    registry.snapshot_encode(w, env, Some(&synthetic.entries))?;
    write_arena(w, env, compiler.typed_scope_arena())?;
    write_creator(w, env, builder.creator)?;
    write_builder(w, env, builder)
}

/// The creator fields `write_creator` leaves out, which the segment must not change.
fn input_digest(creator: &TypedScopeCreator, compiler: &AbstractCompiler) -> Vec<String> {
    let mut out = compiler
        .type_registry_if_present()
        .map(JSTypeRegistry::snapshot_input_digest)
        .unwrap_or_default();
    out.push(format!(
        "{} {} {} {} {} {} {} {} {}",
        creator.module_map.is_some(),
        creator.process_closure_primitives,
        creator.omit_empty_extern_stub_scopes,
        creator.reserved_names_for_scope.len(),
        creator.functions_with_non_empty_returns.len(),
        creator.escaped_var_names.len(),
        creator.assigned_var_names.len(),
        creator.undeclared_names_for_closure.len(),
        creator.provided_names_from_call.len()
    ));
    for (k, v) in &creator.assigned_var_names {
        out.push(format!("assigned {} {v}", k.get_name(compiler)));
    }
    out.push(format!(
        "reports {} pending {} mismatches {}",
        compiler.reports_made(),
        compiler.type_registry_error_queue().has_pending(),
        creator.validator.get_mismatches().len()
    ));
    out
}

/// Whether `convention` is the CLI's default `ClosureCodingConvention`.
fn is_default_convention(convention: &dyn CodingConvention) -> bool {
    let any: &dyn std::any::Any = convention;
    any.downcast_ref::<ClosureCodingConvention>()
        .is_some_and(|c| Arc::ptr_eq(c.next_convention(), &CodingConventions::get_default()))
}

/// The segment's progress in one global scope builder.
#[derive(Default)]
pub(super) struct SegmentState {
    stage: SegmentStage,
}

#[derive(Default)]
enum SegmentStage {
    #[default]
    NotStarted,
    Restored {
        scripts: Vec<NodeId>,
        next: usize,
    },
    Watching(Box<Watch>),
    Done,
}

struct Watch {
    capture: bool,
    layout: Layout,
    synthetic: Synthetic,
    pre: Vec<u8>,
    props: PropWatch,
    inputs: Vec<String>,
    non_nullable_before: Vec<JsString>,
    /// Capture: the global this property names and the global declarations before the segment.
    names_before: IndexSet<JsString>,
    vars_before: IndexSet<JsString>,
}

/// The names the segment adds as index keys, global vars or global this properties, which a
/// synthetic extern must not share unless the data's synthetic externs share it too (see
/// `Data::shared`): their place depends on which came first.
fn name_hash(name: &JsString) -> u32 {
    let mut digest = Digest::default();
    digest.feed_units(name.as_units());
    digest.0 as u32
}

fn global_names(compiler: &AbstractCompiler, global: TypedScope) -> IndexSet<JsString> {
    compiler.typed_scope_arena().scopes[global.index()]
        .abstract_scope
        .vars
        .iter()
        .map(|(name, _)| name.clone())
        .collect()
}

/// Called at the entry of `should_traverse_script` of a global `NormalScopeBuilder`: true when the
/// script is a bundled one whose typing was restored (the builder skips it).
pub(super) fn before_script(
    builder: &mut AbstractScopeBuilder<'_>,
    t: &mut NodeTraversal<'_>,
    script: NodeId,
    parent: Option<NodeId>,
) -> bool {
    match &mut builder.typed_externs.stage {
        SegmentStage::Done | SegmentStage::Watching(_) => false,
        SegmentStage::Restored { scripts, next } => {
            if *next < scripts.len() && scripts[*next] == script {
                *next += 1;
                if *next == scripts.len() {
                    builder.typed_externs.stage = SegmentStage::Done;
                }
                true
            } else {
                // The traversal reached a script the restore did not expect: the data cannot hold
                // its typing, and the scripts before it were skipped.
                panic!("typed externs: the restored scripts were not traversed in order");
            }
        }
        SegmentStage::NotStarted => {
            let compiler = t.get_compiler();
            let Some(parent) = parent else {
                return false;
            };
            let is_externs_root = parent.is_root(compiler)
                && parent
                    .get_parent(compiler)
                    .is_some_and(|root| root.get_first_child(compiler) == Some(parent));
            if !is_externs_root {
                builder.typed_externs.stage = SegmentStage::Done;
                return false;
            }
            if parent.get_first_child(compiler) == Some(script) {
                // [synthetic:externs], before the bundled scripts.
                return false;
            }
            start(builder, t, parent, script)
        }
    }
}

/// Decides how the segment starting at `first` runs.
fn start(
    builder: &mut AbstractScopeBuilder<'_>,
    t: &mut NodeTraversal<'_>,
    externs_root: NodeId,
    first: NodeId,
) -> bool {
    builder.typed_externs.stage = SegmentStage::Done;
    let capture = matches!(t.get_compiler().typed_externs.capture, Some(None));
    let mode = mode(t.get_compiler());
    if mode == Mode::Off && !capture {
        return false;
    }
    if t.may_contain_synthetic_blocks() {
        note_outcome(t.get_compiler(), "not applicable: synthetic blocks".into());
        return false;
    }
    let compiler = t.get_compiler();
    let global = builder.current_scope;
    let prepared = Layout::compute(compiler, externs_root, first).and_then(|layout| {
        Synthetic::find(compiler, &layout, global).map(|synthetic| (layout, synthetic))
    });
    let (mut layout, synthetic) = match prepared {
        Ok(prepared) => prepared,
        Err(reason) => {
            note_outcome(compiler, format!("not applicable: {reason}"));
            return false;
        }
    };
    if capture {
        layout.compute_digests(compiler);
        let mut env = Env::encoding(compiler, &layout, &synthetic, global);
        let mut w = Writer::new();
        if let Err(e) = encode_state(builder, compiler, &mut env, &mut w) {
            panic!(
                "typed externs capture: the state before the bundled externs: {}",
                e.0
            );
        }
        let pre = w.finish();
        let props = PropWatch::new(compiler);
        let inputs = input_digest(builder.creator, compiler);
        let vars_before = global_names(compiler, global);
        let registry = compiler.get_type_registry();
        let names_before = registry.snapshot_global_this_names();
        let non_nullable_before = registry
            .snapshot_non_nullable_names(registry.snapshot_name_table_global_root())
            .map(|names| names.iter().cloned().collect())
            .unwrap_or_default();
        registry.snapshot_start_lookup_log();
        builder.typed_externs.stage = SegmentStage::Watching(Box::new(Watch {
            capture: true,
            layout,
            synthetic,
            pre,
            props,
            inputs,
            non_nullable_before,
            names_before,
            vars_before,
        }));
        return false;
    }
    let Some(data) = Data::get() else {
        note_outcome(compiler, "not applicable: no data".into());
        return false;
    };
    let forced = mode == Mode::Verify && compiler.typed_externs.force_verify;
    if let Err(reason) = check(builder, compiler, data, &mut layout, &synthetic) {
        note_outcome(compiler, format!("not applicable: {reason}"));
        if !forced {
            return false;
        }
        layout.compute_digests(compiler);
    }
    match mode {
        Mode::Off => false,
        Mode::Restore => {
            let props = restore(builder, compiler, data, &mut layout, &synthetic);
            if compiler.typed_externs.round_trip {
                let mut w = Writer::new();
                let node_strings =
                    encode_post(builder, compiler, &layout, &synthetic, &props, &mut w)
                        .unwrap_or_else(|e| panic!("typed externs round trip: {e}"));
                let post = w.bytes_with(&mut |s| node_strings.get(s).copied());
                assert!(
                    post == data.post,
                    "typed externs round trip: the restored state differs"
                );
            }
            note_outcome(compiler, "restored".into());
            let scripts = layout.scripts;
            builder.typed_externs.stage = if scripts.len() == 1 {
                SegmentStage::Done
            } else {
                SegmentStage::Restored { scripts, next: 1 }
            };
            true
        }
        Mode::Verify => {
            let props = PropWatch::new(compiler);
            let inputs = input_digest(builder.creator, compiler);
            builder.typed_externs.stage = SegmentStage::Watching(Box::new(Watch {
                capture: false,
                layout,
                synthetic,
                pre: data.pre.to_vec(),
                props,
                inputs,
                non_nullable_before: Vec::new(),
                names_before: IndexSet::default(),
                vars_before: IndexSet::default(),
            }));
            false
        }
    }
}

/// Whether the data applies to this compile (see the module comment), cheapest checks first.
fn check(
    builder: &AbstractScopeBuilder<'_>,
    compiler: &AbstractCompiler,
    data: &Data,
    layout: &mut Layout,
    synthetic: &Synthetic,
) -> Result<(), String> {
    let recorded = data.recorded_layout();
    layout.matches(&recorded)?;
    if !is_default_convention(compiler.get_coding_convention())
        || !is_default_convention(&*builder.creator.coding_convention)
    {
        return Err("another coding convention".into());
    }
    let registry = compiler
        .type_registry_if_present()
        .ok_or_else(|| "no type registry".to_string())?;
    if !registry.snapshot_closure_namespaces_are_legacy() {
        return Err("a non-legacy Closure namespace".into());
    }
    // The synthetic externs declare names the bundled externs do not (VarCheck declares only
    // names nothing declares): their places among the global declarations and global this
    // properties do not depend on which came first.
    {
        let mut r = Reader::new(data.shared);
        let len = r.u() as usize;
        let mut hash = 0u32;
        let hashes: Vec<u32> = (0..len)
            .map(|_| {
                hash += r.u() as u32;
                hash
            })
            .collect();
        if let Some(name) = synthetic
            .names
            .iter()
            .find(|name| hashes.binary_search(&name_hash(name)).is_ok())
        {
            return Err(format!(
                "a synthetic extern shares a name with the bundled externs ({name})"
            ));
        }
    }
    // The state before the segment.
    {
        let mut env = Env::encoding(compiler, layout, synthetic, builder.current_scope);
        let mut w = Writer::with_labels();
        encode_state(builder, compiler, &mut env, &mut w)
            .map_err(|e| format!("the state before the bundled externs: {}", e.0))?;
        let pre = w.bytes();
        if pre != data.pre {
            let header = pre.len() - w.body_len();
            let at = pre
                .iter()
                .zip(data.pre.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(pre.len().min(data.pre.len()));
            let label = at
                .checked_sub(header)
                .and_then(|offset| w.label_at(offset))
                .unwrap_or("the string tables");
            return Err(format!(
                "the state before the bundled externs differs ({label})"
            ));
        }
    }
    // The names marked non-nullable that the segment asks about.
    let mut r = Reader::new(data.non_nullable);
    let recorded_names = read_string_set(&mut r);
    let current = registry.snapshot_non_nullable_names(registry.snapshot_name_table_global_root());
    if recorded_names
        .iter()
        .any(|name| !current.is_some_and(|c| c.contains(name)))
    {
        return Err("a bundled non-nullable name is missing".into());
    }
    let mut r = Reader::new(data.queried);
    let queried = read_string_set(&mut r);
    let forward_declared = read_string_set(&mut r);
    if let Some(current) = current
        && current.len() > recorded_names.len()
        && current
            .iter()
            .any(|name| !recorded_names.contains(name) && queried.contains(name))
    {
        return Err("a name the bundled externs use is marked non-nullable".into());
    }
    if forward_declared
        .iter()
        .any(|name| registry.is_forward_declared_type(name.clone()))
    {
        return Err("a name the bundled externs use is forward declared".into());
    }
    // Last, the digests of the node state the segment reads.
    layout.compute_digests(compiler);
    layout.digests_match(&recorded)
}

/// Replaces the state before the segment with the state after it.
fn restore(
    builder: &mut AbstractScopeBuilder<'_>,
    compiler: &mut AbstractCompiler,
    data: &Data,
    layout: &mut Layout,
    synthetic: &Synthetic,
) -> Vec<(NodeId, TypeId)> {
    let mut r = {
        let layout = &*layout;
        let ast: &Ast = compiler;
        Reader::new_with(data.post, &mut |segment, offset| {
            layout
                .node_at(segment as usize, offset as u32)
                .string_payload(ast)
                .expect("a restored string")
                .clone()
        })
    };
    let count = read_new_nodes(&mut r, compiler);
    layout.set_new_nodes(count);
    let layout = &*layout;
    let mut env = Env::new(compiler, layout, synthetic, builder.current_scope);
    env.report = Some(crate::function_type_builder::get_compiler_report_fn(
        compiler,
    ));
    {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        registry.snapshot_restore(&mut r, &mut env, ast, Some(&synthetic.entries));
    }
    read_arena(&mut r, &mut env, compiler.typed_scope_arena_mut());
    read_creator(&mut r, &mut env, builder.creator);
    read_builder(&mut r, &mut env, builder);
    read_node_types(&mut r, layout, compiler);
    let props = read_node_props(&mut r, &mut env, compiler);
    assert!(r.at_end(), "typed externs: trailing data");
    props
}

/// The state after the segment (see `restore` for the order).
fn encode_post(
    builder: &AbstractScopeBuilder<'_>,
    compiler: &AbstractCompiler,
    layout: &Layout,
    synthetic: &Synthetic,
    props: &[(NodeId, TypeId)],
    w: &mut Writer,
) -> Result<IndexMap<JsString, (u64, u64)>, String> {
    write_new_nodes(w, compiler, layout.new_nodes_start())?;
    let mut env = Env::encoding(compiler, layout, synthetic, builder.current_scope);
    encode_state(builder, compiler, &mut env, w).map_err(|e| e.0)?;
    write_node_types(w, layout, compiler);
    write_node_props(w, &mut env, props).map_err(|e| e.0)?;
    Ok(env.node_strings())
}

/// Called at the end of `visit`: finishes a watched segment after its last script.
pub(super) fn after_visit(
    builder: &mut AbstractScopeBuilder<'_>,
    t: &mut NodeTraversal<'_>,
    n: NodeId,
) {
    let SegmentStage::Watching(watch) = &builder.typed_externs.stage else {
        return;
    };
    if watch.layout.last_script() != n {
        return;
    }
    let SegmentStage::Watching(watch) = std::mem::take(&mut builder.typed_externs.stage) else {
        unreachable!()
    };
    builder.typed_externs.stage = SegmentStage::Done;
    let Watch {
        capture,
        mut layout,
        synthetic,
        pre,
        props,
        inputs,
        non_nullable_before,
        names_before,
        vars_before,
    } = *watch;
    let compiler = t.get_compiler();
    let failure = |what: String| -> ! {
        panic!(
            "typed externs {}: {what}",
            if capture { "capture" } else { "verify" }
        )
    };
    if input_digest(builder.creator, compiler) != inputs {
        failure("the segment changed an input of the compile".into());
    }
    let node_props = match props.changes(compiler, &layout) {
        Ok(node_props) => node_props,
        Err(e) => failure(e),
    };
    for &script in &layout.scripts {
        if compiler
            .parsed_script_record::<BundledScript>(script, SnapshotUse::SyntaxNamesAndJsDoc)
            .is_none()
        {
            failure("the segment changed a bundled script".into());
        }
    }
    let new_nodes = compiler.node_count() - layout.new_nodes_start();
    layout.set_new_nodes(new_nodes);
    let mut w = Writer::with_labels();
    let node_strings =
        match encode_post(builder, compiler, &layout, &synthetic, &node_props, &mut w) {
            Ok(node_strings) => node_strings,
            Err(e) => failure(format!("the state after the bundled externs: {e}")),
        };
    let post = w.bytes_with(&mut |s| node_strings.get(s).copied());
    if capture {
        let global = builder.current_scope;
        let vars_after = global_names(compiler, global);
        let registry = compiler.get_type_registry();
        let log = registry.snapshot_take_lookup_log().unwrap_or_default();
        let global_root = registry.snapshot_name_table_global_root();
        let names_after = registry.snapshot_global_this_names();
        let LookupLog {
            non_nullable,
            forward_declared,
            closure_namespace,
        } = log;
        if !closure_namespace.is_empty() {
            failure("the bundled externs looked up a Closure namespace".into());
        }
        let mut queried = IndexSet::<JsString>::default();
        for (root, name) in non_nullable {
            if root != global_root {
                failure(format!(
                    "a non-nullable lookup of {name} outside the global scope"
                ));
            }
            queried.insert(name);
        }
        // The global declarations and global this properties the segment adds.
        let added: IndexSet<JsString> = names_after
            .difference(&names_before)
            .chain(vars_after.difference(&vars_before))
            .cloned()
            .collect();
        if let Some(name) = synthetic.names.iter().find(|name| added.contains(*name)) {
            failure(format!(
                "the bundled externs declare the synthetic extern {name}"
            ));
        }
        let mut hashes: Vec<u32> = added.iter().map(name_hash).collect();
        hashes.sort_unstable();
        hashes.dedup();
        let mut sh = Writer::new();
        sh.u(hashes.len() as u64);
        let mut previous = 0u32;
        for hash in hashes {
            sh.u(u64::from(hash - previous));
            previous = hash;
        }
        let mut nn = Writer::new();
        nn.u(non_nullable_before.len() as u64);
        for name in &non_nullable_before {
            nn.js(name);
        }
        let mut q = Writer::new();
        write_string_set(&mut q, &queried);
        write_string_set(&mut q, &forward_declared);
        let data = Data::assemble(
            &layout.encode(),
            &pre,
            &post,
            &nn.finish(),
            &q.finish(),
            &sh.finish(),
        );
        compiler.typed_externs.capture = Some(Some(data));
        note_outcome(compiler, "captured".into());
    } else {
        let data = Data::get().expect("verified data");
        if post != data.post {
            let header = post.len() - w.body_len();
            let at = post
                .iter()
                .zip(data.post.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(post.len().min(data.post.len()));
            let label = at
                .checked_sub(header)
                .and_then(|offset| w.label_at(offset))
                .unwrap_or("the string tables");
            let what = format!(
                "the computed state differs from the data at byte {at} ({label}); regenerate \
                 typed_externs.bin"
            );
            if compiler.typed_externs.force_verify {
                note_outcome(compiler, format!("differs: {label}"));
                return;
            }
            failure(what);
        }
        if !compiler.typed_externs.force_verify {
            note_outcome(compiler, "verified".into());
        } else if compiler
            .typed_externs
            .outcome
            .as_ref()
            .is_none_or(|o| !o.starts_with("not applicable"))
        {
            note_outcome(compiler, "verified".into());
        } else {
            let refused = compiler.typed_externs.outcome.clone().unwrap();
            note_outcome(compiler, format!("same despite: {refused}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_level::CompilationLevel;
    use crate::compiler::Compiler;
    use crate::compiler_options::{CompilerOptions, LanguageMode};
    use crate::custom_pass_execution_time::CustomPassExecutionTime;
    use crate::source_file::SourceFile;
    use closure_jstype::{JSTypeNative, prelude::*};
    use closure_resources::compiler_options::Environment;

    /// The data file in the source tree.
    fn data_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/typed_scope_creator/typed_externs.bin")
    }

    /// The options of an ADVANCED compile as the CLI makes it.
    fn options() -> CompilerOptions {
        let mut options = CompilerOptions::new();
        CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
        CompilationLevel::ADVANCED_OPTIMIZATIONS.set_type_based_optimization_options(&mut options);
        options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
        options.set_coding_convention(Arc::new(ClosureCodingConvention::new()));
        options.set_environment(crate::compiler_options::Environment::BROWSER);
        options
    }

    fn externs() -> Vec<Arc<SourceFile>> {
        SourceFile::bundled_externs(Environment::BROWSER)
            .into_iter()
            .map(Arc::new)
            .collect()
    }

    /// What a compile wrote: the code, the diagnostics, the restore's outcome.
    #[derive(Debug, PartialEq)]
    struct Output {
        code: String,
        diagnostics: Vec<String>,
    }

    fn run(
        externs: Vec<Arc<SourceFile>>,
        source: &str,
        options: CompilerOptions,
        setup: impl FnOnce(&mut Control),
    ) -> (Output, Option<String>, Compiler) {
        let mut compiler = Compiler::new();
        setup(&mut compiler.typed_externs);
        let input = Arc::new(SourceFile::from_code("app.js", source));
        compiler.compile(&externs, &[input], options);
        let mut diagnostics: Vec<String> = compiler
            .get_errors()
            .iter()
            .chain(compiler.get_warnings().iter())
            .map(ToString::to_string)
            .collect();
        diagnostics.sort();
        let code = compiler.to_source().to_string();
        let outcome = compiler.typed_externs.outcome.clone();
        (Output { code, diagnostics }, outcome, compiler)
    }

    fn with_mode(mode: Mode) -> impl FnOnce(&mut Control) {
        move |control| control.mode = Some(mode)
    }

    /// A program that uses extern types, with a warning naming one.
    const PROGRAM: &str = "
        /** @type {Element} */ var e = 5;
        var p = Promise.resolve(document.title);
        p.then(function(t) { window.alert(t.length); });
        Math.max(1, 2);
    ";

    /// The data the canonical compile makes.
    fn generated() -> Vec<u8> {
        let (_, outcome, compiler) = run(externs(), "var lane = 1;", options(), |control| {
            control.capture = Some(None);
        });
        match compiler.typed_externs.capture {
            Some(Some(data)) => data,
            _ => panic!("the canonical compile did not capture: {outcome:?}"),
        }
    }

    // Regenerate with `CLOSURE_RS_REGENERATE_TYPED_EXTERNS=1 cargo test -p closure-jscomp
    // typed_externs_data_is_current` after a change to the type system, to the scope creator or to
    // the bundled externs.
    #[test]
    fn typed_externs_data_is_current() {
        let data = generated();
        if std::env::var_os("CLOSURE_RS_REGENERATE_TYPED_EXTERNS").is_some() {
            std::fs::write(data_path(), &data).unwrap();
            return;
        }
        assert!(
            data == DATA,
            "typed_externs.bin is stale: regenerate it with CLOSURE_RS_REGENERATE_TYPED_EXTERNS=1 \
             cargo test -p closure-jscomp typed_externs_data_is_current"
        );
    }

    /// The comparison of `CLOSURE_RS_TYPED_EXTERNS=verify` on the bundled externs: the state the
    /// ordinary path computes is the one the data holds (it fails when the data is stale or
    /// leaves something out).
    #[test]
    fn typed_externs_verify_on_bundled_externs() {
        let (_, outcome, _) = run(externs(), PROGRAM, options(), with_mode(Mode::Verify));
        assert_eq!(outcome.as_deref(), Some("verified"));
    }

    /// A restore makes the state the data holds (decoding and encoding again gives the data), and
    /// the compile writes what the ordinary path writes.
    #[test]
    fn typed_externs_restore_round_trips_and_writes_the_same() {
        let (restored, outcome, _) = run(externs(), PROGRAM, options(), |control| {
            control.mode = Some(Mode::Restore);
            control.round_trip = true;
        });
        assert_eq!(outcome.as_deref(), Some("restored"));
        let (ordinary, outcome, _) = run(externs(), PROGRAM, options(), with_mode(Mode::Off));
        assert_eq!(outcome, None);
        assert_eq!(restored, ordinary);
        assert!(
            restored.diagnostics.iter().any(|d| d.contains("Element")),
            "{:?}",
            restored.diagnostics
        );
    }

    /// The guard refuses the data, and the state the ordinary path computes there is not the
    /// data's (a restore would be wrong): `differs` names the first part that differs.
    fn assert_refused(
        externs: Vec<Arc<SourceFile>>,
        source: &str,
        options: impl Fn() -> CompilerOptions,
        reason: &str,
        differs: bool,
    ) {
        let (restored, outcome, _) =
            run(externs.clone(), source, options(), with_mode(Mode::Restore));
        let outcome = outcome.unwrap_or_default();
        assert!(
            outcome.starts_with(&format!("not applicable: {reason}")),
            "{outcome}"
        );
        let (ordinary, _, _) = run(externs.clone(), source, options(), with_mode(Mode::Off));
        assert_eq!(restored, ordinary);
        if differs {
            let (_, outcome, _) = run(externs, source, options(), |control| {
                control.mode = Some(Mode::Verify);
                control.force_verify = true;
            });
            let outcome = outcome.unwrap_or_default();
            assert!(outcome.starts_with("differs: "), "{outcome}");
        }
    }

    /// The program leaves other names undeclared than the data's compile: VarCheck declares them
    /// in [synthetic:externs], typed before the bundled externs; the data leaves those out.
    #[test]
    fn typed_externs_restore_with_other_synthetic_externs() {
        let source = "if (typeof laneUndeclared != 'undefined') window.alert(laneUndeclared);";
        let (restored, outcome, _) = run(externs(), source, options(), |control| {
            control.mode = Some(Mode::Restore);
            control.round_trip = true;
        });
        assert_eq!(outcome.as_deref(), Some("restored"));
        let (ordinary, _, _) = run(externs(), source, options(), with_mode(Mode::Off));
        assert_eq!(restored, ordinary);
        let (_, outcome, _) = run(externs(), source, options(), with_mode(Mode::Verify));
        assert_eq!(outcome.as_deref(), Some("verified"));
    }

    /// An undeclared name that the bundled externs register as a property name: the index by
    /// property name lists the synthetic extern's global this first under that key.
    #[test]
    fn typed_externs_restore_with_a_synthetic_extern_named_like_a_property() {
        let source = "if (typeof byteLength != 'undefined') window.alert(byteLength);";
        let (restored, outcome, _) = run(externs(), source, options(), |control| {
            control.mode = Some(Mode::Restore);
            control.round_trip = true;
        });
        assert_eq!(outcome.as_deref(), Some("restored"));
        let (ordinary, _, _) = run(externs(), source, options(), with_mode(Mode::Off));
        assert_eq!(restored, ordinary);
        let (_, outcome, _) = run(externs(), source, options(), with_mode(Mode::Verify));
        assert_eq!(outcome.as_deref(), Some("verified"));
    }

    #[test]
    fn typed_externs_guard_non_nullable_names() {
        // A typedef named like a type the bundled externs use makes their `{Element}` non-null.
        assert_refused(
            externs(),
            "/** @typedef {string} @suppress {duplicate} */ var Element;",
            options,
            "a name the bundled externs use is marked non-nullable",
            true,
        );
    }

    #[test]
    fn typed_externs_guard_coding_convention() {
        assert_refused(
            externs(),
            PROGRAM,
            || {
                let mut options = options();
                options.set_coding_convention(Arc::new(
                    crate::google_coding_convention::GoogleCodingConvention::new(),
                ));
                options
            },
            "another coding convention",
            false,
        );
    }

    #[test]
    fn typed_externs_guard_synthetic_blocks() {
        assert_refused(
            externs(),
            PROGRAM,
            || {
                let mut options = options();
                options.set_synthetic_block_start_marker("start".into());
                options.set_synthetic_block_end_marker("end".into());
                options
            },
            "synthetic blocks",
            false,
        );
    }

    #[test]
    fn typed_externs_guard_changed_bundled_script() {
        // A bundled file read from text: its nodes are not the build-time parse.
        let mut externs = externs();
        let i = externs.len() / 2;
        let name =
            closure_rhino::static_source_file::StaticSourceFile::get_name(&*externs[i]).to_owned();
        let code = externs[i].get_code().unwrap();
        externs[i] = Arc::new(SourceFile::from_code(&name, code));
        assert_refused(
            externs,
            PROGRAM,
            options,
            &format!("{i} unchanged bundled scripts"),
            false,
        );
    }

    /// Options with a pass that runs before the checks.
    fn options_with_pass(
        pass: impl FnMut(&mut AbstractCompiler, NodeId, NodeId) + Send + 'static,
    ) -> CompilerOptions {
        let mut options = options();
        options.add_custom_pass(
            CustomPassExecutionTime::BEFORE_CHECKS,
            Arc::new(std::sync::Mutex::new(pass)),
        );
        options
    }

    #[test]
    fn typed_externs_guard_registry_state() {
        // The registry is not in its initial state: a native type has a property more.
        assert_refused(
            externs(),
            PROGRAM,
            || {
                options_with_pass(|compiler, _, _| {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let global_this = reg.get_native_object_type(JSTypeNative::GLOBAL_THIS);
                    let number = reg.get_native_type(JSTypeNative::NUMBER_TYPE);
                    global_this.define_declared_property(reg, ast, "laneProbe", number, None);
                })
            },
            "the state before the bundled externs differs",
            true,
        );
    }

    /// The first statement of the bundled script `name`.
    fn bundled_statement(compiler: &AbstractCompiler, externs: NodeId, name: &str) -> NodeId {
        externs
            .children(compiler)
            .find(|s| {
                s.get_source_file_name(compiler)
                    .is_some_and(|n| n.ends_with(name))
            })
            .and_then(|s| s.get_first_child(compiler))
            .unwrap()
    }

    #[test]
    fn typed_externs_guard_appended_statements() {
        // A statement appended to a bundled script (the parser snapshot watch allows appends).
        assert_refused(
            externs(),
            PROGRAM,
            || {
                options_with_pass(|compiler, externs, _| {
                    let script = bundled_statement(compiler, externs, "/es3.js")
                        .get_parent(compiler)
                        .unwrap();
                    let extra = compiler
                        .parse_synthetic_code("extra", "/** @type {number} */ var laneExtra;");
                    let statement = extra.get_first_child(compiler).unwrap();
                    statement.detach(compiler);
                    script.add_child_to_back(compiler, statement);
                })
            },
            "the appended statements are not one block",
            false,
        );
    }

    #[test]
    fn typed_externs_guard_node_properties() {
        // InferConsts marks extern names constant unless the program assigns them; here a pass
        // marks one more.
        assert_refused(
            externs(),
            PROGRAM,
            || {
                options_with_pass(|compiler, externs, _| {
                    let script = bundled_statement(compiler, externs, "/es3.js")
                        .get_parent(compiler)
                        .unwrap();
                    let name = script
                        .children(compiler)
                        .filter(|s| s.is_var(compiler))
                        .map(|s| s.get_first_child(compiler).unwrap())
                        .next()
                        .unwrap();
                    name.set_declared_constant_var(compiler, true);
                })
            },
            "the properties of the bundled extern nodes differ",
            false,
        );
    }

    #[test]
    fn typed_externs_data_needs_the_bundled_snapshots_it_was_made_from() {
        let mut data = DATA.to_vec();
        assert!(Data::parse(DATA).is_some());
        data[8] ^= 1;
        let data: &'static [u8] = Box::leak(data.into_boxed_slice());
        assert!(Data::parse(data).is_none());
    }

    #[test]
    fn typed_externs_off_runs_the_ordinary_path() {
        let (_, outcome, _) = run(externs(), PROGRAM, options(), with_mode(Mode::Off));
        assert_eq!(outcome, None);
    }
}
