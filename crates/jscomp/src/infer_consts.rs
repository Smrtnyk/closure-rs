/*
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/InferConsts.java.

//! Port of `InferConsts.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::NodeTraversal;
use crate::reference_collection::ReferenceCollection;
use crate::reference_collector::{Behavior, ReferenceCollector};
use crate::reference_map::ReferenceMap;
use crate::syntactic_scope_cache::{ChangeMark, SyntacticScopeCache};
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use crate::var::VarId;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HandoffMode {
    Off,
    On,
    Verify,
}

/// `CLOSURE_RS_INFER_CONSTS_HANDOFF=off` collects the references again in the InferConsts run
/// after VariableReferenceCheck; `=verify` collects them again and compares (see
/// `InferConstsHandoff`).
pub(crate) static HANDOFF_MODE: LazyLock<HandoffMode> =
    LazyLock::new(
        || match std::env::var("CLOSURE_RS_INFER_CONSTS_HANDOFF").as_deref() {
            Ok("off") => HandoffMode::Off,
            Ok("verify") => HandoffMode::Verify,
            _ => HandoffMode::On,
        },
    );

/// Rust-only (performance divergence, crates/DESIGN.md): what VariableReferenceCheck, which
/// collects the references of every variable, found out for the InferConsts run that follows it
/// in the checks. Java's InferConsts collects the references of the code (not the externs) again
/// and calls `considerVar` for every variable that its filter passes; the only thing it reads from
/// the references is `isInferredConst`. VariableReferenceCheck learns all references of a variable
/// at the exit of its scope, and there evaluates the filter and `isInferredConst` for every
/// variable declared in the code; InferConsts then calls `considerVar` with that answer instead of
/// collecting.
///
/// Why the answers are the same: both collectors traverse the code in the same order with the same
/// hoisted-function jumps, so a variable declared in a function, block or module of the code has
/// the same references with blocks and scopes of the same shape (VariableReferenceCheck's global
/// block is rooted at the ROOT, InferConsts' at the root of the code; both are global blocks).
/// InferConsts' filter only leaves out references, never jumps. A global variable of the code has
/// in VariableReferenceCheck also the references from the externs: one with any such reference
/// makes the handoff unusable (a jump from the externs into the code would also reorder
/// references); a name the externs declare as well is a different variable in the two global
/// scopes, so InferConsts uses the handoff only when every global variable of its own scope that
/// its filter passes was answered. The answers hold while the code does not change: InferConsts
/// uses them only when no node below the root of the code was stamped by the scope cache for a
/// structural or an in-place JSDoc or constant flag change since the end of VariableReferenceCheck
/// (changes to the externs, such as VarCheck's synthetic declarations, do not count), so the
/// filter, the JSDoc and the flags `considerVar` reads are also the same. An answer that
/// `isInferredConst` would
/// have to give by throwing makes the handoff unusable, and so does a variable without a name
/// node that the filter passes. The externs are considered as before.
pub(crate) struct InferConstsHandoff {
    js_root: NodeId,
    mark: ChangeMark,
    /// Each variable the filter passed, with `isInferredConst` of its references when its name
    /// node was not yet inferred constant.
    vars: Vec<(VarId, Option<bool>)>,
    /// The name nodes of the global variables among `vars`.
    globals: IndexSet<NodeId>,
}

/// Rust-only: makes an `InferConstsHandoff` during VariableReferenceCheck.
pub(crate) struct HandoffBuilder {
    vars: Vec<(VarId, Option<bool>)>,
    globals: IndexSet<NodeId>,
    usable: bool,
}

impl HandoffBuilder {
    /// A builder for a VariableReferenceCheck run that InferConsts follows, or None.
    pub(crate) fn for_run(compiler: &AbstractCompiler) -> Option<Self> {
        (*HANDOFF_MODE != HandoffMode::Off && compiler.get_options().should_infer_consts()).then(
            || Self {
                vars: Vec::new(),
                globals: IndexSet::default(),
                usable: true,
            },
        )
    }

    /// At the exit of the scope declaring `v`, with all of its references.
    pub(crate) fn note(
        &mut self,
        compiler: &mut AbstractCompiler,
        global: bool,
        v: VarId,
        references: &ReferenceCollection,
    ) {
        if !self.usable {
            return;
        }
        // A variable declared in the externs (read from the scope arena before the name node).
        if v.input_ref(compiler).is_some_and(|input| input.is_extern()) {
            return;
        }
        let Some(name_node) = v.get_name_node(compiler) else {
            if var_needs_const_analysis(compiler, v) {
                self.usable = false;
            }
            return;
        };
        if name_node.is_from_externs(compiler) {
            return;
        }
        if global
            && references
                .references
                .iter()
                .any(|r| r.get_node().is_from_externs(compiler))
        {
            self.usable = false;
            return;
        }
        if !var_needs_const_analysis(compiler, v) {
            return;
        }
        let answer = if name_node.is_inferred_constant_var(compiler) {
            None
        } else {
            match InferConsts::inferred_const_from_references(compiler, v, references) {
                Some(answer) => Some(answer),
                None => {
                    self.usable = false;
                    return;
                }
            }
        };
        if global {
            self.globals.insert(name_node);
        }
        self.vars.push((v, answer));
    }

    /// The handoff, at the end of VariableReferenceCheck on `js_root`.
    pub(crate) fn finish(
        self,
        compiler: &mut AbstractCompiler,
        js_root: NodeId,
    ) -> Option<InferConstsHandoff> {
        self.usable.then(|| InferConstsHandoff {
            js_root,
            mark: SyntacticScopeCache::change_mark(compiler),
            vars: self.vars,
            globals: self.globals,
        })
    }
}

impl InferConstsHandoff {
    /// Whether the InferConsts run on `js` may use this handoff (see the type).
    fn usable(&self, compiler: &mut AbstractCompiler, js: NodeId) -> bool {
        if self.js_root != js {
            return false;
        }
        // Changes elsewhere (VarCheck and CheckSideEffects add to the synthetic externs) leave
        // the code as it was.
        for flags in [false, true] {
            match SyntacticScopeCache::nodes_changed_since(compiler, &self.mark, flags) {
                Some(changed) if !crate::strict_mode_check::any_below(compiler, &changed, js) => {}
                _ => return false,
            }
        }
        let global_scope = SyntacticScopeCreator::new().create_scope(compiler, js, None);
        global_scope
            .get_var_iterable(compiler)
            .into_iter()
            .all(|v| {
                v.is_implicit_goog_namespace(compiler)
                    || !var_needs_const_analysis(compiler, v)
                    || v.get_name_node(compiler)
                        .is_some_and(|n| self.globals.contains(&n))
            })
    }

    /// What `considerVar` sets for each variable: `(name node, declared, inferred)`.
    fn decisions(&self, compiler: &AbstractCompiler) -> Vec<(NodeId, bool, bool)> {
        self.vars
            .iter()
            .filter_map(|&(v, answer)| InferConsts::decide(compiler, v, answer))
            .collect()
    }
}

/// Rust-only (performance divergence): enables `ReferenceCollector`'s zero-allocation leaf
/// function fast path for `InferConsts`.
struct InferConstsBehavior;

impl Behavior for InferConstsBehavior {
    fn infer_consts_leaf_fast_path(&self) -> bool {
        true
    }

    // The scope exits do nothing.
    fn scope_exits_change_only_their_scopes(&self) -> bool {
        true
    }

    fn after_exit_scope(&mut self, _t: &mut NodeTraversal<'_>, _reference_map: &dyn ReferenceMap) {}
}

/// Rust-only (performance divergence): `InferConsts` only sets monotone `declared_constant_var`
/// and `inferred_constant_var` flags, so variables already marked inferred-const whose declared
/// status cannot change need no reference collection on subsequent rounds.
pub(crate) fn var_needs_const_analysis(compiler: &AbstractCompiler, v: VarId) -> bool {
    if v.is_implicit_goog_namespace(compiler) {
        return false;
    }
    let Some(name_node) = v.get_name_node(compiler) else {
        return v.get_jsdoc_info(compiler).is_some_and(|d| d.is_constant());
    };
    if !name_node.is_inferred_constant_var(compiler) {
        return true;
    }
    !name_node.is_declared_constant_var(compiler)
        && (name_node
            .get_parent(compiler)
            .is_some_and(|p| p.is_const(compiler))
            || v.get_jsdoc_info(compiler).is_some_and(|d| d.is_constant()))
}

/// Attaches the CONST_VAR annotation to any variable that's
/// 1) Provably well-defined and assigned once in its lifetime.
/// 2) Annotated 'const'
/// 3) Declared with the 'const' keyword.
///
/// These 3 are considered semantically equivalent. Notice that a variable in a loop is never
/// considered const.
///
/// Note that criteria (1) is only used for normal code, not externs.
#[derive(Debug, Default)]
pub struct InferConsts;

impl InferConsts {
    // port: InferConsts#InferConsts
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: InferConsts#considerVar
    fn consider_var(
        &mut self,
        compiler: &mut AbstractCompiler,
        v: VarId,
        ref_collection: Option<&ReferenceCollection>,
    ) {
        if v.is_implicit_goog_namespace(compiler) {
            return; // no name node for provided variables.
        }

        let name_node = v.get_name_node(compiler);
        if !name_node.is_some_and(|n| n.is_declared_constant_var(compiler)) {
            let doc_info = v.get_jsdoc_info(compiler);
            if doc_info.is_some_and(|doc_info| doc_info.is_constant()) {
                name_node
                    .expect("InferConsts#considerVar nameNode")
                    .set_declared_constant_var(compiler, true);
            } else if name_node
                .is_some_and(|name_node| name_node.get_parent(compiler).unwrap().is_const(compiler))
            {
                name_node.unwrap().set_declared_constant_var(compiler, true);
            }
        }

        if !name_node.is_some_and(|n| n.is_inferred_constant_var(compiler))
            && Self::is_inferred_const(compiler, v, ref_collection)
        {
            name_node.unwrap().set_inferred_constant_var(compiler, true);
        }
    }

    // port: InferConsts#isInferredConst
    fn is_inferred_const(
        compiler: &mut AbstractCompiler,
        v: VarId,
        ref_collection: Option<&ReferenceCollection>,
    ) -> bool {
        let name_node = v.get_name_node(compiler);
        let (Some(name_node), Some(ref_collection)) = (name_node, ref_collection) else {
            return false;
        };
        if !ref_collection.is_assigned_once_in_lifetime(compiler) {
            return false;
        }

        if v.is_implicit_goog_namespace(compiler) {
            return false;
        }

        let declaration_type = v.declaration_type(compiler);
        match declaration_type {
            Some(Token::LET) => {
                // Check that non-destructuring let names are actually assigned at declaration.
                !name_node.get_parent(compiler).unwrap().is_let(compiler)
                    || name_node.has_children(compiler)
            }
            Some(
                Token::CONST
                | Token::CATCH
                | Token::CLASS
                | Token::PARAM_LIST // Parameters cannot be referenced before declaration.
                | Token::FUNCTION, // Function hoisting means no references before declaration.
            ) => true,
            Some(Token::IMPORT) => {
                // ES module exports are mutable.
                // TODO(lharker): make this check smarter if we start optimizing unrewritten
                // modules.
                false
            }
            Some(Token::VAR) => {
                // var hoisting requires this extra work to make sure the 'declaration' is also the
                // first reference.
                ref_collection.first_reference_is_assigning_declaration(compiler)
                    && ref_collection.is_well_defined(compiler)
            }
            // Java's switch on a null enum throws NullPointerException before any case.
            None => panic!("NullPointerException: InferConsts#isInferredConst declarationType"),
            Some(declaration_type) => {
                panic!("Unrecognized declaration type {declaration_type}")
            }
        }
    }

    /// Rust-only (performance divergence): `isInferredConst` of a variable with a name node and
    /// all of its `references`, or None where Java would throw (`InferConstsHandoff`).
    pub(crate) fn inferred_const_from_references(
        compiler: &mut AbstractCompiler,
        v: VarId,
        references: &ReferenceCollection,
    ) -> Option<bool> {
        let name_node = v.get_name_node(compiler)?;
        if !references.is_assigned_once_in_lifetime(compiler) {
            return Some(false);
        }
        if v.is_implicit_goog_namespace(compiler) {
            return Some(false);
        }
        // AbstractVar#declarationType without its checkState.
        let mut current = v.get_node(compiler);
        let declaration_type = loop {
            match current {
                Some(n)
                    if matches!(
                        n.get_token(compiler),
                        Token::VAR
                            | Token::LET
                            | Token::CONST
                            | Token::FUNCTION
                            | Token::CLASS
                            | Token::CATCH
                            | Token::IMPORT
                            | Token::PARAM_LIST
                    ) =>
                {
                    break n.get_token(compiler);
                }
                Some(n) => current = n.get_parent(compiler),
                None => return None,
            }
        };
        Some(match declaration_type {
            Token::LET => {
                !name_node.get_parent(compiler).unwrap().is_let(compiler)
                    || name_node.has_children(compiler)
            }
            Token::CONST | Token::CATCH | Token::CLASS | Token::PARAM_LIST | Token::FUNCTION => {
                true
            }
            Token::IMPORT => false,
            Token::VAR => {
                references.first_reference_is_assigning_declaration(compiler)
                    && references.is_well_defined(compiler)
            }
            _ => return None,
        })
    }

    /// Rust-only (performance divergence): what `considerVar` sets for a variable with a name
    /// node given `isInferredConst` (`answer`, None when its name node is already inferred
    /// constant): `(name node, declared, inferred)`, or None when it sets nothing.
    fn decide(
        compiler: &AbstractCompiler,
        v: VarId,
        answer: Option<bool>,
    ) -> Option<(NodeId, bool, bool)> {
        if v.is_implicit_goog_namespace(compiler) {
            return None;
        }
        let name_node = v.get_name_node(compiler)?;
        let declared = !name_node.is_declared_constant_var(compiler)
            && (v.get_jsdoc_info(compiler).is_some_and(|d| d.is_constant())
                || name_node.get_parent(compiler).unwrap().is_const(compiler));
        let inferred = !name_node.is_inferred_constant_var(compiler)
            && answer.expect("an answer for a name node not inferred constant");
        (declared || inferred).then_some((name_node, declared, inferred))
    }

    /// `considerVar` for every variable of the code from `handoff`.
    fn consider_handed_off(compiler: &mut AbstractCompiler, handoff: &InferConstsHandoff) {
        for (name_node, declared, inferred) in handoff.decisions(compiler) {
            if declared {
                name_node.set_declared_constant_var(compiler, true);
            }
            if inferred {
                name_node.set_inferred_constant_var(compiler, true);
            }
        }
    }

    /// The constant flags of every NAME or IMPORT_STAR node below `root` that has any.
    fn constant_flags(compiler: &AbstractCompiler, root: NodeId) -> IndexMap<NodeId, (bool, bool)> {
        let mut flags = IndexMap::default();
        let mut pending = vec![root];
        while let Some(n) = pending.pop() {
            if n.is_name(compiler) || n.is_import_star(compiler) {
                let declared = n.is_declared_constant_var(compiler);
                let inferred = n.is_inferred_constant_var(compiler);
                if declared || inferred {
                    flags.insert(n, (declared, inferred));
                }
            }
            pending.extend(n.children(compiler));
        }
        flags
    }

    // port: InferConsts#process (the reference collection and the considerVar calls for the code)
    fn collect_and_consider(&mut self, compiler: &mut AbstractCompiler, js: NodeId) {
        let mut scope_creator = SyntacticScopeCreator::new();
        let mut collector = ReferenceCollector::new_with_filter(
            compiler,
            InferConstsBehavior,
            &mut scope_creator,
            Box::new(var_needs_const_analysis),
        );
        collector.reuse_reference_sites();
        collector.process(compiler, js);

        for v in collector.get_all_symbols() {
            self.consider_var(compiler, v, collector.get_references(v));
        }
    }
}

impl CompilerPass for InferConsts {
    // port: InferConsts#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, js: NodeId) {
        let handoff = compiler
            .infer_consts_handoff
            .take()
            .filter(|handoff| handoff.usable(compiler, js));
        match (handoff, *HANDOFF_MODE) {
            (Some(handoff), HandoffMode::On) => Self::consider_handed_off(compiler, &handoff),
            (Some(handoff), HandoffMode::Verify) => {
                let before = Self::constant_flags(compiler, js);
                let mut expected = before.clone();
                for (name_node, declared, inferred) in handoff.decisions(compiler) {
                    let entry = expected.entry(name_node).or_insert((false, false));
                    entry.0 |= declared;
                    entry.1 |= inferred;
                }
                self.collect_and_consider(compiler, js);
                let after = Self::constant_flags(compiler, js);
                if after != expected {
                    let nodes: IndexSet<NodeId> =
                        after.keys().chain(expected.keys()).copied().collect();
                    let mut differences = Vec::new();
                    for n in nodes {
                        let collected = after.get(&n).copied().unwrap_or_default();
                        let handed_off = expected.get(&n).copied().unwrap_or_default();
                        if collected != handed_off {
                            differences.push(format!(
                                "{} (declared, inferred): before {:?}, collected {:?}, handed off {:?}",
                                n.to_string(compiler),
                                before.get(&n).copied().unwrap_or_default(),
                                collected,
                                handed_off
                            ));
                        }
                    }
                    panic!(
                        "CLOSURE_RS_INFER_CONSTS_HANDOFF=verify: constant flags differ:\n{}",
                        differences.join("\n")
                    );
                }
            }
            _ => self.collect_and_consider(compiler, js),
        }

        let global_externs_scope =
            SyntacticScopeCreator::new().create_scope(compiler, externs, None);
        for v in global_externs_scope.get_all_symbols(compiler) {
            self.consider_var(compiler, v, None);
        }
    }
}
