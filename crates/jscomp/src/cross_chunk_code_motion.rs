/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CrossChunkCodeMotion.java.

//! Port of CrossChunkCodeMotion.java.
//!
//! A compiler pass for moving global variable declarations and assignments to their properties to
//! a deeper chunk if possible.
//!
//! 1. Collect all global-level statements and the references they contain.
//! 2. Statements that do not declare global variables are assumed to exist for their side-effects
//!    and are considered immovable.
//! 3. Statements that declare global variables may be movable. See CrossChunkReferenceCollector.
//! 4. Within each chunk gather all declarations for a single global variable into a
//!    DeclarationStatementGroup (DSG). Keep track of the references to other globals that appear
//!    in the DSG. A DSG is movable if all of its statements are movable.
//! 5. Gather the DSGs for each global variable and note all of the chunks that contain immovable
//!    references to it.
//! 6. The global variables form a directed graph. Global A has an edge to B if A has a DSG with a
//!    reference to B.
//! 7. Convert this to a directed-acyclic graph by grouping together all of the strongly-connected
//!    global variables into GlobalSymbolCycles. There is an edge from GlobalSymbolCycle A to
//!    GlobalSymbolCycle B if A contains a DSG (in one of its GlobalSymbols) that contains a
//!    reference to a GlobalSymbol in B.
//! 8. Sort the GlobalSymbolCycles into a list c[1], c[2], c[3],..., c[n], such that there are no
//!    references to GlobalSymbols in c[i] from DSGs in GlobalSymbolCycles c[i+1]...c[n].
//! 9. Traverse the list in that order, so statements for GlobalSymbol X will only be moved after
//!    all statements that refer to X have already been moved.
//! 10. Within a GlobalSymbolCycle, combine statements from DSGs in the same chunk and keep them in
//!     the same order when moving them.
//!
//! Rust shape: Java's inner objects (GlobalSymbol, DeclarationStatementGroup) refer to each other
//! by identity; they live in the `GlobalSymbolArena` vectors and are named by index, so Java `==`
//! is index equality. TopLevelStatements are named by their index in the reference collector's
//! list. Java's `graph` field is `compiler.getChunkGraph()` at every construction site; the pass
//! reads it from the compiler (DESIGN section 6: a pass never stores the compiler).

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::cross_chunk_reference_collector::{CrossChunkReferenceCollector, TopLevelStatement};
use crate::diagnostic::log_file::LogFile;
use crate::js_chunk::JSChunk;
use crate::js_chunk_graph::{BitSet, JSChunkGraph};
use crate::node_util::NodeUtil;
use crate::reference::Reference;
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use crate::var::VarId;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::ir::IR;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use closure_rhino::{check_not_null, check_state};
use std::collections::VecDeque;

type GlobalSymbolId = usize;
type DsgId = usize;
type StatementId = usize;

pub struct CrossChunkCodeMotion {
    // Allocated & cleaned up by process()
    cccm_log: Option<Box<dyn LogFile>>,

    /// Map from chunk to the node in that chunk that should parent variable declarations that
    /// have to be moved into that chunk
    chunk_insertion_point_map: IndexMap<JSChunk, NodeId>,

    parent_chunk_can_see_symbols_declared_in_children: bool,
}

impl CrossChunkCodeMotion {
    /// Creates an instance.
    // port: CrossChunkCodeMotion#CrossChunkCodeMotion
    pub fn new(parent_chunk_can_see_symbols_declared_in_children: bool) -> Self {
        Self {
            cccm_log: None,
            chunk_insertion_point_map: IndexMap::<_, _>::default(),
            parent_chunk_can_see_symbols_declared_in_children,
        }
    }

    fn graph(compiler: &AbstractCompiler) -> &JSChunkGraph {
        compiler.get_chunk_graph().unwrap()
    }

    fn cccm_log(&mut self) -> &mut dyn LogFile {
        self.cccm_log.as_deref_mut().unwrap()
    }

    // port: CrossChunkCodeMotion#addInstanceofGuards
    fn add_instanceof_guards(
        &mut self,
        compiler: &mut AbstractCompiler,
        arena: &GlobalSymbolArena,
        global_symbols: &VecDeque<GlobalSymbolId>,
    ) {
        for &global_symbol in global_symbols {
            for instanceof_reference in &arena.symbols[global_symbol].instanceof_references_to_guard
            {
                let chunk = instanceof_reference.get_chunk(arena);
                if !arena.declarations_cover_chunk(Self::graph(compiler), global_symbol, &chunk) {
                    self.add_guard_to_instanceof_reference(
                        compiler,
                        instanceof_reference.get_reference().get_node(),
                    );
                }
            }
        }
    }

    /// Moves all of the declaration statements that can move to their best possible chunk
    /// location.
    // port: CrossChunkCodeMotion#moveGlobalSymbols
    fn move_global_symbols(
        &mut self,
        compiler: &mut AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        collector: &CrossChunkReferenceCollector<'_>,
        global_symbols: &VecDeque<GlobalSymbolId>,
    ) {
        for global_symbol_cycle in
            OrderAndCombineGlobalSymbols::new(global_symbols).order_and_combine(compiler, arena)
        {
            // Symbols whose declarations refer to each other must be grouped together and their
            // declaration statements moved together.
            self.move_declaration_statements(compiler, arena, collector, &global_symbol_cycle);
        }
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCycle#moveDeclarationStatements
    fn move_declaration_statements(
        &mut self,
        compiler: &mut AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        collector: &CrossChunkReferenceCollector<'_>,
        cycle: &GlobalSymbolCycle,
    ) {
        for &symbol in &cycle.symbols {
            check_state!(
                !arena.symbols[symbol].is_move_declaration_statements_done,
                "duplicate attempt to move %s",
                arena.symbols[symbol].to_string(compiler)
            );
        }
        let mut chunks_with_immovable_references = BitSet::new();
        let cycles_latest_first = cycle.get_dsg_cycles_latest_first(arena);
        for dsg_cycle in &cycles_latest_first {
            // Each move may change chunksWithImmovableReferences
            for &symbol in &cycle.symbols {
                chunks_with_immovable_references
                    .or(&arena.symbols[symbol].chunks_with_immovable_references);
            }
            self.move_to_preferred_chunk(
                compiler,
                arena,
                collector,
                dsg_cycle,
                &chunks_with_immovable_references,
            );
        }
        for &symbol in &cycle.symbols {
            arena.symbols[symbol].is_move_declaration_statements_done = true;
        }
    }

    // port: CrossChunkCodeMotion#addGuardToInstanceofReference
    fn add_guard_to_instanceof_reference(
        &mut self,
        compiler: &mut AbstractCompiler,
        reference_node: NodeId,
    ) {
        check_state!(
            self.is_unguarded_instanceof_reference(compiler, reference_node),
            "instanceof Reference is already guarded: %s",
            reference_node.to_string(compiler)
        );
        let instanceof_node = check_not_null!(reference_node.get_parent(compiler));
        let reference_for_type_of = reference_node.clone_node(compiler);
        let tmp = IR::block(compiler);
        // Wrap "foo instanceof Bar" in
        // "('function' == typeof Bar && foo instanceof Bar)"
        instanceof_node.replace_with(compiler, tmp);
        let function_string = IR::string(compiler, "function");
        let type_of = compiler.new_node_with_child(Token::TYPEOF, reference_for_type_of);
        let eq = compiler.new_node_with_children2(Token::EQ, function_string, type_of);
        let and = IR::and(compiler, eq, instanceof_node);
        and.srcref_tree_if_missing(compiler, instanceof_node);
        tmp.replace_with(compiler, and);
        compiler.report_change_to_enclosing_scope(and);
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroupCycle#moveToPreferredChunk
    fn move_to_preferred_chunk(
        &mut self,
        compiler: &mut AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        collector: &CrossChunkReferenceCollector<'_>,
        dsg_cycle: &DeclarationStatementGroupCycle,
        chunks_with_immovable_references: &BitSet,
    ) {
        let preferred_chunk = dsg_cycle.get_preferred_chunk(
            compiler,
            arena,
            collector,
            chunks_with_immovable_references,
        );
        if preferred_chunk != dsg_cycle.current_chunk {
            if self.cccm_log().is_logging() {
                // Write a separate log for each global symbol, because this is easier to work
                // with when analyzing the log.
                // Include a header, so we can tell which ones were treated as a cycle
                self.cccm_log()
                    .log(&mut || "start DSG Cycle move".to_string());
                for global_symbol_name in dsg_cycle.get_global_symbol_names(compiler, arena) {
                    let current_chunk = &dsg_cycle.current_chunk;
                    let preferred = &preferred_chunk;
                    self.cccm_log().log(&mut || {
                        format!(
                            "Moving DSG for {global_symbol_name} from chunk {current_chunk} to \
                             chunk {preferred}"
                        )
                    });
                }
            }
            self.move_statements_to_chunk(compiler, arena, collector, dsg_cycle, &preferred_chunk);
        }
        // Now that all the statements have been moved, update the current chunk for all the DSGs
        // and treat all the references they contain as now immovable.
        for &dsg in &dsg_cycle.dsgs {
            arena.dsgs[dsg].current_chunk = preferred_chunk.clone();
            arena.make_references_immovable(compiler, dsg);
        }
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroupCycle#moveStatementsToChunk
    fn move_statements_to_chunk(
        &mut self,
        compiler: &mut AbstractCompiler,
        arena: &GlobalSymbolArena,
        collector: &CrossChunkReferenceCollector<'_>,
        dsg_cycle: &DeclarationStatementGroupCycle,
        preferred_chunk: &JSChunk,
    ) {
        let dest_parent = match self.chunk_insertion_point_map.get(preferred_chunk) {
            Some(&dest_parent) => dest_parent,
            None => {
                let dest_parent = compiler.get_node_for_code_insertion(Some(preferred_chunk));
                self.chunk_insertion_point_map
                    .insert(preferred_chunk.clone(), dest_parent);
                dest_parent
            }
        };
        let statements_last_first = dsg_cycle.get_statements_last_first(arena, collector);
        for statement in statements_last_first {
            let statement_node =
                collector.get_top_level_statements()[statement].get_statement_node();
            // Remove it
            compiler.report_change_to_enclosing_scope(statement_node);
            let original_script = NodeUtil::get_enclosing_script(compiler, statement_node).unwrap();

            statement_node.detach(compiler);

            // Add it to the new spot
            dest_parent.add_child_to_front(compiler, statement_node);
            let features = NodeUtil::get_feature_set_of_script(compiler, original_script).unwrap();
            NodeUtil::add_features_to_script(compiler, dest_parent, features);
            compiler.report_change_to_enclosing_scope(statement_node);
        }
    }

    /// Is the reference node the first `Ref` in an expression like `'undefined' != typeof Ref &&
    /// x instanceof Ref`?
    ///
    /// It's safe to ignore this kind of reference when moving the definition of `Ref`.
    // port: CrossChunkCodeMotion#isUndefinedTypeofGuardReference
    fn is_undefined_typeof_guard_reference(
        &self,
        compiler: &AbstractCompiler,
        reference: NodeId,
    ) -> bool {
        // reference => typeof => `!=`
        let maybe_typeof_guard = reference.get_grandparent(compiler);
        if let Some(maybe_typeof_guard) = maybe_typeof_guard
            .filter(|&guard| self.is_existence_typeof_guard_for(compiler, guard, reference))
        {
            let and_node = maybe_typeof_guard.get_parent(compiler);
            and_node.is_some_and(|and_node| {
                and_node.is_and(compiler)
                    && self.is_instanceof_for(
                        compiler,
                        and_node.get_last_child(compiler).unwrap(),
                        reference,
                    )
            })
        } else {
            false
        }
    }

    /// Is the expression of the form `'undefined' != typeof Ref` or `'function' == typeof Ref`?
    ///
    /// @param expression The expression being checked.
    /// @param reference Ref node must be equivalent to this node
    // port: CrossChunkCodeMotion#isExistenceTypeofGuardFor
    fn is_existence_typeof_guard_for(
        &self,
        compiler: &AbstractCompiler,
        expression: NodeId,
        reference: NodeId,
    ) -> bool {
        if expression.is_ne(compiler) || expression.is_shne(compiler) {
            let undefined_string = expression.get_first_child(compiler).unwrap();
            let typeof_node = expression.get_last_child(compiler).unwrap();
            undefined_string.is_string_lit(compiler)
                && undefined_string.get_string_ref(compiler) == "undefined"
                && typeof_node.is_type_of(compiler)
                && typeof_node
                    .get_first_child(compiler)
                    .unwrap()
                    .is_equivalent_to(compiler, reference)
        } else if expression.is_eq(compiler) || expression.is_sheq(compiler) {
            let function_string = expression.get_first_child(compiler).unwrap();
            let typeof_node = expression.get_last_child(compiler).unwrap();
            function_string.is_string_lit(compiler)
                && function_string.get_string_ref(compiler) == "function"
                && typeof_node.is_type_of(compiler)
                && typeof_node
                    .get_first_child(compiler)
                    .unwrap()
                    .is_equivalent_to(compiler, reference)
        } else {
            false
        }
    }

    /// Is the reference node the second `Ref` in an expression like `'undefined' != typeof Ref &&
    /// x instanceof Ref`?
    ///
    /// It's safe to ignore this kind of reference when moving the definition of `Ref`.
    // port: CrossChunkCodeMotion#isGuardedInstanceofReference
    fn is_guarded_instanceof_reference(
        &self,
        compiler: &AbstractCompiler,
        reference: NodeId,
    ) -> bool {
        let instanceof_node = reference.get_parent(compiler).unwrap();
        if self.is_instanceof_for(compiler, instanceof_node, reference) {
            let and_node = instanceof_node.get_parent(compiler);
            and_node.is_some_and(|and_node| {
                and_node.is_and(compiler)
                    && self.is_existence_typeof_guard_for(
                        compiler,
                        and_node.get_first_child(compiler).unwrap(),
                        reference,
                    )
            })
        } else {
            false
        }
    }

    /// Is the reference the right hand side of an `instanceof` and not guarded?
    // port: CrossChunkCodeMotion#isUnguardedInstanceofReference
    fn is_unguarded_instanceof_reference(
        &self,
        compiler: &mut AbstractCompiler,
        reference: NodeId,
    ) -> bool {
        let instanceof_node = reference.get_parent(compiler).unwrap();
        if self.is_instanceof_for(compiler, instanceof_node, reference) {
            let first_child = instanceof_node.get_first_child(compiler).unwrap();
            if compiler
                .get_ast_analyzer()
                .may_have_side_effects(compiler, first_child)
            {
                return false;
            }
            !self.is_guarded_instanceof_reference(compiler, reference)
        } else {
            false
        }
    }

    /// Is the expression of the form `x instanceof Ref`?
    ///
    /// @param reference Ref node must be equivalent to this node
    // port: CrossChunkCodeMotion#isInstanceofFor
    fn is_instanceof_for(
        &self,
        compiler: &AbstractCompiler,
        expression: NodeId,
        reference: NodeId,
    ) -> bool {
        expression.is_instance_of(compiler)
            && expression
                .get_last_child(compiler)
                .unwrap()
                .is_equivalent_to(compiler, reference)
    }
}

impl CompilerPass for CrossChunkCodeMotion {
    // port: CrossChunkCodeMotion#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let log_file =
            compiler.create_or_reopen_indexed_log("CrossChunkCodeMotion", "cccm.log", &[]);
        self.cccm_log = Some(log_file);
        self.cccm_log().log(&mut || "doing cccm".to_string());
        // If there are <2 chunks, then we will never move anything, so we're done
        if Self::graph(compiler).get_chunk_count() > 1 {
            let mut reference_collector =
                CrossChunkReferenceCollector::new(Box::new(SyntacticScopeCreator::new()));
            reference_collector.process_root(compiler, root);
            let mut arena = GlobalSymbolArena::default();
            let global_symbols = GlobalSymbolCollector::new().collect_global_symbols(
                self,
                compiler,
                &mut arena,
                &reference_collector,
            );
            self.move_global_symbols(compiler, &mut arena, &reference_collector, &global_symbols);
            self.add_instanceof_guards(compiler, &arena, &global_symbols);
        } else {
            self.cccm_log()
                .log(&mut || "only one chunk exists".to_string());
        }
        // try-with-resources closes the log; the finally block clears the field.
        if let Some(mut log_file) = self.cccm_log.take() {
            log_file.close();
        }
    }
}

/// Owns Java's GlobalSymbol and DeclarationStatementGroup objects (Rust-only).
#[derive(Default)]
struct GlobalSymbolArena {
    symbols: Vec<GlobalSymbol>,
    dsgs: Vec<DeclarationStatementGroup>,
}

/// Collects all global symbols, their declaration statements and references.
struct GlobalSymbolCollector {
    global_symbolfor_var: IndexMap<VarId, GlobalSymbolId>,

    /// Returning the symbols in the reverse order in which they are defined helps to minimize
    /// unnecessary reordering of declaration statements.
    symbol_stack: VecDeque<GlobalSymbolId>,
}

impl GlobalSymbolCollector {
    fn new() -> Self {
        Self {
            global_symbolfor_var: IndexMap::<_, _>::default(),
            symbol_stack: VecDeque::new(),
        }
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCollector#collectGlobalSymbols
    fn collect_global_symbols(
        mut self,
        outer: &CrossChunkCodeMotion,
        compiler: &mut AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        reference_collector: &CrossChunkReferenceCollector<'_>,
    ) -> VecDeque<GlobalSymbolId> {
        for (statement_id, statement) in reference_collector
            .get_top_level_statements()
            .iter()
            .enumerate()
        {
            if statement.is_declaration_statement() {
                self.process_declaration_statement(outer, compiler, arena, statement_id, statement);
            } else {
                self.process_immovable_statement(outer, compiler, arena, statement);
            }
        }
        self.symbol_stack
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCollector#processImmovableStatement
    fn process_immovable_statement(
        &mut self,
        outer: &CrossChunkCodeMotion,
        compiler: &mut AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        statement: &TopLevelStatement,
    ) {
        for r#ref in statement.get_non_declaration_references() {
            self.process_immovable_reference(
                outer,
                compiler,
                arena,
                r#ref,
                statement.get_chunk().unwrap(),
            );
        }
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCollector#processImmovableReference
    fn process_immovable_reference(
        &mut self,
        outer: &CrossChunkCodeMotion,
        compiler: &mut AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        r#ref: &Reference,
        chunk: &JSChunk,
    ) {
        let symbol = r#ref.get_symbol(compiler).unwrap();
        let global_symbol = self.get_global_symbol(arena, symbol);
        if outer.parent_chunk_can_see_symbols_declared_in_children {
            // It is possible to move the declaration of `Foo` after
            // `'undefined' != typeof Foo && x instanceof Foo`.
            // We'll add the undefined check, if necessary.
            let n = r#ref.get_node();
            if outer.is_guarded_instanceof_reference(compiler, n)
                || outer.is_undefined_typeof_guard_reference(compiler, n)
            {
                return;
            } else if outer.is_unguarded_instanceof_reference(compiler, n) {
                let instanceof_reference = InstanceofReference::Immovable {
                    chunk: chunk.clone(),
                    reference: r#ref.clone(),
                };

                arena.symbols[global_symbol]
                    .instanceof_references_to_guard
                    .push_front(instanceof_reference);
                return;
            }
        }
        arena.symbols[global_symbol].add_immovable_reference(chunk);
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCollector#processDeclarationStatement
    fn process_declaration_statement(
        &mut self,
        outer: &CrossChunkCodeMotion,
        compiler: &mut AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        statement_id: StatementId,
        statement: &TopLevelStatement,
    ) {
        let symbol = statement
            .get_declared_name_reference()
            .get_symbol(compiler)
            .unwrap();
        let declared_symbol = self.get_global_symbol(arena, symbol);
        let dsg = arena.add_declaration_statement(declared_symbol, statement_id, statement);
        self.process_declaration_statement_contained_references(
            outer,
            compiler,
            arena,
            statement,
            declared_symbol,
            dsg,
        );
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCollector#processDeclarationStatementContainedReferences
    fn process_declaration_statement_contained_references(
        &mut self,
        outer: &CrossChunkCodeMotion,
        compiler: &mut AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        statement: &TopLevelStatement,
        declared_symbol: GlobalSymbolId,
        dsg: DsgId,
    ) {
        for r#ref in statement.get_non_declaration_references() {
            let symbol = r#ref.get_symbol(compiler).unwrap();
            let ref_symbol = self.get_global_symbol(arena, symbol);
            if ref_symbol == declared_symbol {
                continue; // ignore circular reference
            }
            if outer.parent_chunk_can_see_symbols_declared_in_children {
                // It is possible to move the declaration of `Foo` after
                // `'undefined' != typeof Foo && x instanceof Foo`.
                // We'll add the undefined check, if necessary.
                let n = r#ref.get_node();
                if outer.is_guarded_instanceof_reference(compiler, n)
                    || outer.is_undefined_typeof_guard_reference(compiler, n)
                {
                    continue;
                } else if outer.is_unguarded_instanceof_reference(compiler, n) {
                    let instanceof_reference = InstanceofReference::Movable {
                        containing_dsg: dsg,
                        reference: r#ref.clone(),
                    };

                    arena.symbols[ref_symbol]
                        .instanceof_references_to_guard
                        .push_front(instanceof_reference);
                    continue;
                }
            }
            arena.dsgs[dsg].add_reference_to_global_symbol(ref_symbol);
            arena.symbols[ref_symbol].add_referring_global_symbol(declared_symbol);
        }
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCollector#getGlobalSymbol
    fn get_global_symbol(&mut self, arena: &mut GlobalSymbolArena, var: VarId) -> GlobalSymbolId {
        if let Some(&global_symbol) = self.global_symbolfor_var.get(&var) {
            return global_symbol;
        }
        let global_symbol = arena.symbols.len();
        arena.symbols.push(GlobalSymbol::new(var));
        self.global_symbolfor_var.insert(var, global_symbol);
        self.symbol_stack.push_front(global_symbol);
        global_symbol
    }
}

/// Represents a global symbol whose declaration statements may be moved.
struct GlobalSymbol {
    var: VarId,
    /// As we traverse the statements in execution order the top of the stack represents the most
    /// recently seen DSG for the variable.
    dsg_stack: VecDeque<DsgId>,

    chunks_with_immovable_references: BitSet,

    /// Symbols whose declaration statements refer to this symbol.
    ///
    /// This is a LinkedHashSet in order to enforce a consistent ordering when we iterate over
    /// these to identify cycles. This guarantees that the order in which statements are moved
    /// won't depend on the arbitrary ordering of LinkedHashSet.
    referencing_global_symbols: IndexSet<GlobalSymbolId>,

    /// Instanceof references we may need to update with a guard after moving declarations.
    instanceof_references_to_guard: VecDeque<InstanceofReference>,
    /// Used by OrderAndCombineGlobalSymbols to find reference cycles.
    preorder_number: i32,
    /// Used by OrderAndCombineGlobalSymbols to find reference cycles.
    has_been_assigned_to_a_strongly_connected_component: bool,
    /// Used to confirm all symbols get moved in the correct order.
    is_move_declaration_statements_done: bool,
}

impl GlobalSymbol {
    // port: CrossChunkCodeMotion.GlobalSymbol#GlobalSymbol
    fn new(var: VarId) -> Self {
        Self {
            var,
            dsg_stack: VecDeque::new(),
            chunks_with_immovable_references: BitSet::new(),
            referencing_global_symbols: IndexSet::<_>::default(),
            instanceof_references_to_guard: VecDeque::new(),
            preorder_number: -1,
            has_been_assigned_to_a_strongly_connected_component: false,
            is_move_declaration_statements_done: false,
        }
    }

    // port: CrossChunkCodeMotion.GlobalSymbol#toString
    fn to_string(&self, compiler: &AbstractCompiler) -> String {
        self.var.get_name(compiler).to_string_lossy()
    }

    // port: CrossChunkCodeMotion.GlobalSymbol#addImmovableReference
    fn add_immovable_reference(&mut self, chunk: &JSChunk) {
        self.chunks_with_immovable_references
            .set(chunk.get_index() as usize);
    }

    // port: CrossChunkCodeMotion.GlobalSymbol#addReferringGlobalSymbol
    fn add_referring_global_symbol(&mut self, declared_symbol: GlobalSymbolId) {
        self.referencing_global_symbols.insert(declared_symbol);
    }
}

impl GlobalSymbolArena {
    /// Adds the statement to the appropriate DeclarationStatementGroup and returns it.
    // port: CrossChunkCodeMotion.GlobalSymbol#addDeclarationStatement
    fn add_declaration_statement(
        &mut self,
        symbol: GlobalSymbolId,
        statement_id: StatementId,
        statement: &TopLevelStatement,
    ) -> DsgId {
        let chunk = statement.get_chunk().unwrap();
        let last_dsg = match self.symbols[symbol].dsg_stack.front() {
            Some(&last_dsg) => last_dsg,
            None => {
                let last_dsg = self.new_dsg(symbol, chunk.clone());
                self.symbols[symbol].dsg_stack.push_front(last_dsg);
                last_dsg
            }
        };
        let statement_dsg = if *chunk == self.dsgs[last_dsg].current_chunk {
            last_dsg
        } else {
            // new chunk requires a new DSG
            let statement_dsg = self.new_dsg(symbol, chunk.clone());
            self.symbols[symbol].dsg_stack.push_front(statement_dsg);
            statement_dsg
        };
        self.dsgs[statement_dsg]
            .statement_stack
            .push_front(statement_id);
        statement_dsg
    }

    fn new_dsg(&mut self, declared_global_symbol: GlobalSymbolId, current_chunk: JSChunk) -> DsgId {
        self.dsgs.push(DeclarationStatementGroup::new(
            declared_global_symbol,
            current_chunk,
        ));
        self.dsgs.len() - 1
    }

    /// Does the chunk depend on at least one of the chunks containing declaration statements for
    /// this symbol?
    // port: CrossChunkCodeMotion.GlobalSymbol#declarationsCoverChunk
    fn declarations_cover_chunk(
        &self,
        graph: &JSChunkGraph,
        symbol: GlobalSymbolId,
        chunk: &JSChunk,
    ) -> bool {
        for &dsg in &self.symbols[symbol].dsg_stack {
            let current_chunk = &self.dsgs[dsg].current_chunk;
            if chunk == current_chunk || graph.depends_on(chunk, current_chunk) {
                return true;
            }
        }
        false
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroup#makeReferencesImmovable
    fn make_references_immovable(&mut self, compiler: &AbstractCompiler, dsg: DsgId) {
        let current_chunk = self.dsgs[dsg].current_chunk.clone();
        let declared_global_symbol = self.dsgs[dsg].declared_global_symbol;
        self.symbols[declared_global_symbol].add_immovable_reference(&current_chunk);
        for i in 0..self.dsgs[dsg].referenced_global_symbols.len() {
            let symbol = self.dsgs[dsg].referenced_global_symbols[i];
            check_state!(
                !self.symbols[symbol].is_move_declaration_statements_done,
                "symbol %s moved before referring symbol %s",
                self.symbols[symbol].to_string(compiler),
                self.symbols[declared_global_symbol].to_string(compiler)
            );
            self.symbols[symbol].add_immovable_reference(&current_chunk);
        }
    }
}

/// Represents a set of global symbols that all refer to each other, and so must be considered for
/// movement as a group.
struct GlobalSymbolCycle {
    symbols: VecDeque<GlobalSymbolId>,
}

impl GlobalSymbolCycle {
    fn new() -> Self {
        Self {
            symbols: VecDeque::new(),
        }
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCycle#addSymbol
    fn add_symbol(&mut self, symbol: GlobalSymbolId) {
        self.symbols.push_back(symbol);
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCycle#getDsgCyclesLatestFirst
    fn get_dsg_cycles_latest_first(
        &self,
        arena: &GlobalSymbolArena,
    ) -> Vec<DeclarationStatementGroupCycle> {
        let mut cycles_latest_first: Vec<DeclarationStatementGroupCycle> = Vec::new();
        let dsgs_latest_first = self.get_dsgs_latest_first(arena);
        for dsg in dsgs_latest_first {
            let dsg_chunk = &arena.dsgs[dsg].current_chunk;
            let start_new = match cycles_latest_first.last() {
                None => true,
                Some(cycle) => cycle.current_chunk != *dsg_chunk,
            };
            if start_new {
                cycles_latest_first.push(DeclarationStatementGroupCycle::new(dsg_chunk.clone()));
            }
            cycles_latest_first.last_mut().unwrap().dsgs.push_back(dsg);
        }
        cycles_latest_first
    }

    // port: CrossChunkCodeMotion.GlobalSymbolCycle#getDsgsLatestFirst
    fn get_dsgs_latest_first(&self, arena: &GlobalSymbolArena) -> VecDeque<DsgId> {
        let mut result_stack: VecDeque<DsgId> = VecDeque::new();
        for &symbol in &self.symbols {
            let mut stack1 = result_stack;
            let mut stack2: VecDeque<DsgId> = arena.symbols[symbol].dsg_stack.clone();
            result_stack = VecDeque::with_capacity(stack1.len() + stack2.len());
            loop {
                if stack1.is_empty() {
                    result_stack.extend(stack2);
                    break;
                } else if stack2.is_empty() {
                    result_stack.extend(stack1);
                    break;
                } else {
                    let dsg1 = *stack1.front().unwrap();
                    let dsg2 = *stack2.front().unwrap();
                    let dsg1_index = arena.dsgs[dsg1].current_chunk.get_index();
                    let dsg2_index = arena.dsgs[dsg2].current_chunk.get_index();
                    if dsg1_index > dsg2_index {
                        result_stack.push_back(stack1.pop_front().unwrap());
                        check_state!(
                            stack1.is_empty()
                                || arena.dsgs[*stack1.front().unwrap()]
                                    .current_chunk
                                    .get_index()
                                    <= dsg1_index,
                            "DSG stacks are out of order."
                        );
                    } else {
                        result_stack.push_back(stack2.pop_front().unwrap());
                        check_state!(
                            stack2.is_empty()
                                || arena.dsgs[*stack2.front().unwrap()]
                                    .current_chunk
                                    .get_index()
                                    <= dsg2_index,
                            "DSG stacks are out of order."
                        );
                    }
                }
            }
        }
        result_stack
    }
}

/// A group of declaration statements that must be moved (or not) as a group.
///
/// All of the statements must be in the same chunk initially. If there are declarations for the
/// same variable in different chunks, they will be grouped separately.
struct DeclarationStatementGroup {
    declared_global_symbol: GlobalSymbolId,
    referenced_global_symbols: IndexSet<GlobalSymbolId>,

    /// chunk containing the statements
    current_chunk: JSChunk,

    /// statements in the group, latest first
    statement_stack: VecDeque<StatementId>,
}

impl DeclarationStatementGroup {
    // port: CrossChunkCodeMotion.DeclarationStatementGroup#DeclarationStatementGroup
    fn new(declared_global_symbol: GlobalSymbolId, current_chunk: JSChunk) -> Self {
        Self {
            declared_global_symbol,
            referenced_global_symbols: IndexSet::<_>::default(),
            current_chunk,
            statement_stack: VecDeque::new(),
        }
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroup#allStatementsCanMove
    fn all_statements_can_move(
        &self,
        compiler: &mut AbstractCompiler,
        collector: &CrossChunkReferenceCollector<'_>,
    ) -> bool {
        for &s in &self.statement_stack {
            if !collector.get_top_level_statements()[s].is_movable_declaration(compiler, collector)
            {
                return false;
            }
        }
        true
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroup#addReferenceToGlobalSymbol
    fn add_reference_to_global_symbol(&mut self, ref_symbol: GlobalSymbolId) {
        self.referenced_global_symbols.insert(ref_symbol);
    }
}

/// Orders DeclarationStatementGroups so that each DSG appears in the list only after all of the
/// DSGs that contain references to it.
///
/// DSGs that form cycles are combined into a single DSG. This happens when declarations within a
/// chunk form cycles by referring to each other.
///
/// This is an implementation of the path-based strong component algorithm as it is described in
/// the Wikipedia article https://en.wikipedia.org/wiki/Path-based_strong_component_algorithm.
struct OrderAndCombineGlobalSymbols<'s> {
    input_symbols: &'s VecDeque<GlobalSymbolId>,
    /// Tracks DSGs that may be part of a strongly connected component (reference cycle).
    component_contents: VecDeque<GlobalSymbolId>,
    /// Tracks DSGs that may be the root of a strongly connected component (reference cycle).
    component_roots: VecDeque<GlobalSymbolId>,
    /// Filled with lists of strongly connected DSGs as they are discovered.
    strongly_connected_symbols: VecDeque<GlobalSymbolCycle>,

    preorder_counter: i32,
}

impl<'s> OrderAndCombineGlobalSymbols<'s> {
    // port: CrossChunkCodeMotion.OrderAndCombineGlobalSymbols#OrderAndCombineGlobalSymbols
    fn new(dsgs: &'s VecDeque<GlobalSymbolId>) -> Self {
        Self {
            input_symbols: dsgs,
            component_contents: VecDeque::new(),
            component_roots: VecDeque::new(),
            strongly_connected_symbols: VecDeque::with_capacity(dsgs.len()),
            preorder_counter: 0,
        }
    }

    // port: CrossChunkCodeMotion.OrderAndCombineGlobalSymbols#orderAndCombine
    fn order_and_combine(
        mut self,
        compiler: &AbstractCompiler,
        arena: &mut GlobalSymbolArena,
    ) -> VecDeque<GlobalSymbolCycle> {
        for &global_symbol in self.input_symbols {
            if arena.symbols[global_symbol].preorder_number < 0 {
                self.process_global_symbol(compiler, arena, global_symbol);
            } // else already processed
        }
        // At this point stronglyConnectedSymbols has been filled.
        self.strongly_connected_symbols
    }

    /// Determines to which strongly connected component this GlobalSymbol belongs.
    ///
    /// Called exactly once for each GlobalSymbol. When this method returns we will have determined
    /// which strongly connected component contains globalSymbol. Either:
    ///
    /// - globalSymbol was the first member of the strongly connected component we encountered,
    ///   and the entire component has now been added to stronglyConnectedSymbols.
    /// - OR, we encountered the first member of the strongly connected component in an earlier
    ///   call to this method on the call stack. In this case the top of componentRoots will be
    ///   the first member, and we'll add the component to stronglyConnectedSymbols once execution
    ///   has fallen back to the call made with that GlobalSymbol.
    // port: CrossChunkCodeMotion.OrderAndCombineGlobalSymbols#processGlobalSymbol
    fn process_global_symbol(
        &mut self,
        compiler: &AbstractCompiler,
        arena: &mut GlobalSymbolArena,
        symbol: GlobalSymbolId,
    ) {
        // preorderNumber is used to track whether we've already processed a DSG and the order in
        // which they have been processed. It is initially -1.
        check_state!(
            arena.symbols[symbol].preorder_number < 0,
            "already processed: %s",
            arena.symbols[symbol].to_string(compiler)
        );
        arena.symbols[symbol].preorder_number = self.preorder_counter;
        self.preorder_counter += 1;
        self.component_roots.push_front(symbol); // could be the start of a new strongly connected component
        self.component_contents.push_front(symbol); // could be part of an existing strongly connected component
        for i in 0..arena.symbols[symbol].referencing_global_symbols.len() {
            let referring_symbol = arena.symbols[symbol].referencing_global_symbols[i];
            if arena.symbols[referring_symbol].preorder_number < 0 {
                self.process_global_symbol(compiler, arena, referring_symbol);
            } else if !arena.symbols[referring_symbol]
                .has_been_assigned_to_a_strongly_connected_component
            {
                // This GlobalSymbol is part of a not-yet-completed strongly connected component.
                // Back off the potential roots stack to the earliest symbol that is part of the
                // component.
                while arena.symbols[*self.component_roots.front().unwrap()].preorder_number
                    > arena.symbols[referring_symbol].preorder_number
                {
                    self.component_roots.pop_front();
                }
            }
        }
        if *self.component_roots.front().unwrap() == symbol {
            // After exploring all paths from here, this symbol is still at the top of the
            // potential component roots stack, so this symbol is the root of a strongly connected
            // component.
            self.component_roots.pop_front();
            let mut cycle = GlobalSymbolCycle::new();
            // this symbol and all those after it on the componentContents stack are part of a
            // single cycle.
            loop {
                let connected_symbol = self.component_contents.pop_front().unwrap();
                cycle.add_symbol(connected_symbol);
                arena.symbols[connected_symbol]
                    .has_been_assigned_to_a_strongly_connected_component = true;
                if connected_symbol == symbol {
                    break;
                }
            }
            self.strongly_connected_symbols.push_back(cycle);
        }
    }
}

/// One or more DeclarationStatementGroups that that share the same chunk and whose declared
/// global symbols refer to each other.
struct DeclarationStatementGroupCycle {
    current_chunk: JSChunk,
    dsgs: VecDeque<DsgId>,
}

impl DeclarationStatementGroupCycle {
    // port: CrossChunkCodeMotion.DeclarationStatementGroupCycle#DeclarationStatementGroupCycle
    fn new(current_chunk: JSChunk) -> Self {
        Self {
            current_chunk,
            dsgs: VecDeque::new(),
        }
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroupCycle#getGlobalSymbolNames
    fn get_global_symbol_names(
        &self,
        compiler: &AbstractCompiler,
        arena: &GlobalSymbolArena,
    ) -> Vec<String> {
        let mut builder = Vec::new();
        for &dsg in &self.dsgs {
            let symbol = arena.dsgs[dsg].declared_global_symbol;
            builder.push(
                arena.symbols[symbol]
                    .var
                    .get_name(compiler)
                    .to_string_lossy(),
            );
        }
        builder
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroupCycle#getStatementsLastFirst
    fn get_statements_last_first(
        &self,
        arena: &GlobalSymbolArena,
        collector: &CrossChunkReferenceCollector<'_>,
    ) -> VecDeque<StatementId> {
        let statements = collector.get_top_level_statements();
        let order = |s: StatementId| statements[s].get_original_order();
        let mut result: VecDeque<StatementId> = VecDeque::new();

        for &dsg in &self.dsgs {
            // combine previous result with statements for current DSG
            let mut stack1: VecDeque<StatementId> = arena.dsgs[dsg].statement_stack.clone();
            let mut stack2 = result;
            result = VecDeque::with_capacity(stack1.len() + stack2.len());
            loop {
                if stack1.is_empty() {
                    result.extend(stack2);
                    break;
                } else if stack2.is_empty() {
                    result.extend(stack1);
                    break;
                } else {
                    let s1 = *stack1.front().unwrap();
                    let s2 = *stack2.front().unwrap();
                    if order(s1) > order(s2) {
                        result.push_back(stack1.pop_front().unwrap());
                        check_state!(
                            stack1.is_empty() || order(*stack1.front().unwrap()) < order(s1),
                            "Statements are recorded in the wrong order."
                        );
                    } else {
                        result.push_back(stack2.pop_front().unwrap());
                        check_state!(
                            stack2.is_empty() || order(*stack2.front().unwrap()) < order(s2),
                            "Statements are recorded in the wrong order."
                        );
                    }
                }
            }
        }
        result
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroupCycle#getPreferredChunk
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    fn get_preferred_chunk(
        &self,
        compiler: &mut AbstractCompiler,
        arena: &GlobalSymbolArena,
        collector: &CrossChunkReferenceCollector<'_>,
        chunks_with_immovable_references: &BitSet,
    ) -> JSChunk {
        if chunks_with_immovable_references.is_empty() {
            self.current_chunk.clone()
        } else if !self.all_statements_can_move(compiler, arena, collector) {
            self.current_chunk.clone()
        } else {
            CrossChunkCodeMotion::graph(compiler).get_smallest_covering_subtree(
                &self.current_chunk,
                chunks_with_immovable_references,
            )
        }
    }

    // port: CrossChunkCodeMotion.DeclarationStatementGroupCycle#allStatementsCanMove
    fn all_statements_can_move(
        &self,
        compiler: &mut AbstractCompiler,
        arena: &GlobalSymbolArena,
        collector: &CrossChunkReferenceCollector<'_>,
    ) -> bool {
        for &dsg in &self.dsgs {
            if !arena.dsgs[dsg].all_statements_can_move(compiler, collector) {
                return false;
            }
        }
        true
    }
}

/// Java interface CrossChunkCodeMotion.InstanceofReference with its two implementations.
enum InstanceofReference {
    // port: CrossChunkCodeMotion.ImmovableInstanceofReference#ImmovableInstanceofReference
    Immovable {
        chunk: JSChunk,
        reference: Reference,
    },
    // port: CrossChunkCodeMotion.MovableInstanceofReference#MovableInstanceofReference
    Movable {
        containing_dsg: DsgId,
        reference: Reference,
    },
}

impl InstanceofReference {
    // port: CrossChunkCodeMotion.InstanceofReference#getChunk
    fn get_chunk(&self, arena: &GlobalSymbolArena) -> JSChunk {
        match self {
            // port: CrossChunkCodeMotion.ImmovableInstanceofReference#getChunk
            Self::Immovable { chunk, .. } => chunk.clone(),
            // port: CrossChunkCodeMotion.MovableInstanceofReference#getChunk
            Self::Movable { containing_dsg, .. } => {
                arena.dsgs[*containing_dsg].current_chunk.clone()
            }
        }
    }

    // port: CrossChunkCodeMotion.InstanceofReference#getReference
    fn get_reference(&self) -> &Reference {
        match self {
            Self::Immovable { reference, .. } | Self::Movable { reference, .. } => reference,
        }
    }
}
