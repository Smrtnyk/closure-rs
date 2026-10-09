/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PhaseOptimizer.java.

use crate::{
    abstract_compiler::AbstractCompiler, change_tracker::ChangeTracker,
    change_verifier::ChangeVerifier, code_change_handler::CodeChangeHandler,
    compiler_pass::CompilerPass, node_util::NodeUtil, pass_factory::PassFactory, pass_names,
    performance_tracker::PerformanceTracker, platform::Platform,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::node::NodeId;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, Ordering},
};
pub const MAX_LOOPS: i32 = 100;
pub const CODE_REMOVING_PASSES: &[&str] = &[pass_names::PEEPHOLE_OPTIMIZATIONS];
pub const OPTIMIZE_LOOP_ERROR: &str = "Fixed point loop exceeded the maximum number of iterations.";
const START_TIME: i32 = 0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    RUN_PASSES_NOT_RUN_IN_PREV_ITER,
    RUN_PASSES_THAT_CHANGED_STH_IN_PREV_ITER,
}
#[derive(Default)]
pub struct ScopeChangeState {
    pub in_loop: bool,
    pub time_of_last_run: i32,
}
pub struct PhaseOptimizer {
    passes: Vec<ScheduledPass>,
    in_loop: bool,
    tracker: Option<Arc<Mutex<PerformanceTracker>>>,
    validity_check: Option<PassFactory>,
    print_ast_hashcodes: bool,
    current_pass: Option<usize>,
    last_runs: IndexMap<usize, i32>,
    last_change: Arc<AtomicI32>,
    js_root: Option<NodeId>,
    use_size_heuristic_to_stop_optimization_loop: bool,
    optimization_loop_max_iterations: i32,
    change_verifier: Option<ChangeVerifier>,
    scope_change_state: Arc<Mutex<ScopeChangeState>>,
}
enum ScheduledPass {
    Named(NamedPass),
    Loop(Loop),
}
pub struct NamedPass {
    pub name: String,
    factory: PassFactory,
}
pub struct Loop {
    my_passes: Vec<NamedPass>,
    my_names: IndexSet<String>,
    is_code_removal_loop: bool,
    howmany_iterations_under_threshold: i32,
}
impl PhaseOptimizer {
    // port: PhaseOptimizer#PhaseOptimizer
    pub fn new(comp: &AbstractCompiler, tracker: Option<Arc<Mutex<PerformanceTracker>>>) -> Self {
        let max_iterations = comp.get_options().get_max_optimization_loop_iterations();
        Self {
            js_root: comp.get_js_root(),
            tracker,
            passes: vec![],
            in_loop: false,
            last_change: Arc::new(AtomicI32::new(START_TIME)),
            use_size_heuristic_to_stop_optimization_loop: comp
                .get_options()
                .should_use_size_heuristic_to_stop_optimization_loop(),
            optimization_loop_max_iterations: if max_iterations > 0 && max_iterations <= MAX_LOOPS {
                max_iterations
            } else {
                MAX_LOOPS
            },
            validity_check: None,
            print_ast_hashcodes: false,
            current_pass: None,
            last_runs: IndexMap::<_, _>::default(),
            change_verifier: None,
            scope_change_state: Arc::new(Mutex::new(ScopeChangeState::default())),
        }
    }
    // port: PhaseOptimizer#consume
    pub fn consume(&mut self, factories: Vec<PassFactory>) {
        let mut current_loop = Loop::new();
        for factory in factories {
            if factory.is_run_in_fixed_point_loop() {
                current_loop.add_looped_pass(factory);
            } else {
                if current_loop.is_populated() {
                    self.passes.push(ScheduledPass::Loop(current_loop));
                    current_loop = Loop::new();
                }
                self.add_one_time_pass(factory);
            }
        }
        if current_loop.is_populated() {
            self.passes.push(ScheduledPass::Loop(current_loop));
        }
    }
    // port: PhaseOptimizer#addOneTimePass
    pub fn add_one_time_pass(&mut self, factory: PassFactory) {
        self.passes
            .push(ScheduledPass::Named(NamedPass::new(factory)));
    }
    // port: PhaseOptimizer#addFixedPointLoop
    pub fn add_fixed_point_loop(&mut self) -> &mut Loop {
        self.passes.push(ScheduledPass::Loop(Loop::new()));
        match self.passes.last_mut().unwrap() {
            ScheduledPass::Loop(l) => l,
            _ => unreachable!(),
        }
    }
    // port: PhaseOptimizer#setValidityCheck
    pub fn set_validity_check(&mut self, compiler: &mut AbstractCompiler, check: PassFactory) {
        self.validity_check = Some(check);
        self.change_verifier =
            Some(ChangeVerifier::new(compiler).snapshot(compiler, self.js_root.unwrap()));
    }
    // port: PhaseOptimizer#setPrintAstHashcodes
    pub fn set_print_ast_hashcodes(&mut self, print: bool) {
        self.print_ast_hashcodes = print;
    }
    // port: PhaseOptimizer#maybePrintAstHashcodes
    fn maybe_print_ast_hashcodes(
        &self,
        compiler: &mut AbstractCompiler,
        pass_name: &str,
        root: NodeId,
    ) {
        if self.print_ast_hashcodes {
            let source = compiler.to_source_for_node(root);
            use closure_rhino::java_lang::JavaHashCode;
            eprintln!("AST hashCode after {pass_name}: {}", source.hash_code());
        }
    }
    // port: PhaseOptimizer#maybeRunValidityCheck
    fn maybe_run_validity_check(
        &self,
        compiler: &mut AbstractCompiler,
        pass_name: &str,
        externs: NodeId,
        root: NodeId,
    ) {
        if let Some(check) = &self.validity_check {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                check.create(compiler).process(compiler, externs, root);
                self.change_verifier
                    .as_ref()
                    .unwrap()
                    .check_recorded_changes_named(compiler, pass_name, self.js_root.unwrap());
            }));
            if result.is_err() {
                panic!("Validity checks failed for pass: {pass_name}");
            }
        }
    }
    // port: PhaseOptimizer#hasHaltingErrors
    fn has_halting_errors(&self, compiler: &AbstractCompiler) -> bool {
        compiler.has_halting_errors()
    }
    // port: PhaseOptimizer#hasScopeChanged
    pub fn has_scope_changed(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        if !self.in_loop {
            return true;
        }
        let time_of_last_run = self.last_runs[&self.current_pass.unwrap()];
        time_of_last_run == START_TIME || n.get_change_time(compiler) > time_of_last_run
    }
}
impl CompilerPass for PhaseOptimizer {
    // port: PhaseOptimizer#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // The list is temporarily moved so nested passes can borrow optimizer bookkeeping.
        let mut passes = std::mem::take(&mut self.passes);
        let previous_state = compiler
            .phase_optimizer
            .replace(self.scope_change_state.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for pass in &mut passes {
                if Platform::is_thread_interrupted() {
                    panic!("java.lang.InterruptedException");
                }
                match pass {
                    ScheduledPass::Named(p) => p.process(self, compiler, externs, root),
                    ScheduledPass::Loop(l) => l.process(self, compiler, externs, root),
                }
                if self.has_halting_errors(compiler) {
                    return;
                }
            }
        }));
        compiler.phase_optimizer = previous_state;
        self.passes = passes;
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
}
impl NamedPass {
    // port: PhaseOptimizer.NamedPass#NamedPass
    pub fn new(factory: PassFactory) -> Self {
        Self {
            name: factory.get_name().into(),
            factory,
        }
    }
    // port: PhaseOptimizer.NamedPass#process
    fn process(
        &mut self,
        optimizer: &mut PhaseOptimizer,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        self.factory.validate_preconditions(compiler.get_options());
        if optimizer.validity_check.is_some() {
            optimizer.change_verifier =
                Some(ChangeVerifier::new(compiler).snapshot(compiler, optimizer.js_root.unwrap()));
        }
        if let Some(tracker) = &optimizer.tracker {
            tracker
                .lock()
                .unwrap()
                .record_pass_start(&self.name, !self.factory.is_run_in_fixed_point_loop());
        }
        let mut tracer = crate::tracer::Tracer::new(Some("Compiler"), Some(&self.name));
        compiler.before_pass(&self.name);
        self.factory
            .create(compiler)
            .process(compiler, externs, root);
        compiler.after_pass(&self.name);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let trace_runtime = tracer.stop();
            if let Some(tracker) = &optimizer.tracker {
                tracker
                    .lock()
                    .unwrap()
                    .record_pass_stop(compiler, &self.name, trace_runtime);
            }
            optimizer.maybe_print_ast_hashcodes(compiler, &self.name, root);
            optimizer.maybe_run_validity_check(compiler, &self.name, externs, root);
        }));
        if result.is_err() {
            panic!("Validity check failed for {}", self.name);
        }
    }
}
impl std::fmt::Display for NamedPass {
    // port: PhaseOptimizer.NamedPass#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pass: {}", self.name)
    }
}
struct ScopedChangeHandler {
    last_code_change_query: i32,
    last_change: Arc<AtomicI32>,
    change_stamp: Arc<AtomicI32>,
}
impl ScopedChangeHandler {
    // port: PhaseOptimizer.ScopedChangeHandler#ScopedChangeHandler
    fn new(change_tracker: &ChangeTracker, last_change: Arc<AtomicI32>) -> Self {
        Self {
            last_code_change_query: change_tracker.get_change_stamp(),
            last_change,
            change_stamp: change_tracker.get_change_stamp_source(),
        }
    }
    // port: PhaseOptimizer.ScopedChangeHandler#hasCodeChangedSinceLastCall
    fn has_code_changed_since_last_call(&mut self, change_tracker: &mut ChangeTracker) -> bool {
        let result = self.last_change.load(Ordering::Relaxed) > self.last_code_change_query;
        self.last_code_change_query = change_tracker.get_change_stamp();
        change_tracker.increment_change_stamp();
        result
    }
}
impl CodeChangeHandler for ScopedChangeHandler {
    // port: PhaseOptimizer.ScopedChangeHandler#reportChange
    fn report_change(&mut self) {
        self.last_change
            .store(self.change_stamp.load(Ordering::Relaxed), Ordering::Relaxed);
    }
}
impl Default for Loop {
    fn default() -> Self {
        Self::new()
    }
}
impl Loop {
    fn new() -> Self {
        Self {
            my_passes: vec![],
            my_names: IndexSet::<_>::default(),
            is_code_removal_loop: false,
            howmany_iterations_under_threshold: 0,
        }
    }
    // port: PhaseOptimizer.Loop#addLoopedPass
    pub fn add_looped_pass(&mut self, factory: PassFactory) {
        let name = factory.get_name();
        assert!(
            !self.my_names.contains(name),
            "Already a pass with name '{name}' in this loop"
        );
        self.my_names.insert(name.into());
        self.my_passes.push(NamedPass::new(factory));
    }
    // port: PhaseOptimizer.Loop#process
    fn process(
        &mut self,
        optimizer: &mut PhaseOptimizer,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
    ) {
        assert!(!optimizer.in_loop, "Nested loops are forbidden");
        optimizer.in_loop = true;
        self.is_code_removal_loop = self.is_code_removal_loop();
        optimizer.scope_change_state.lock().unwrap().in_loop = true;

        let scope_handler = Arc::new(Mutex::new(ScopedChangeHandler::new(
            compiler.get_change_tracker_ref(),
            optimizer.last_change.clone(),
        )));
        let handler: Arc<Mutex<dyn CodeChangeHandler>> = scope_handler.clone();
        compiler
            .get_change_tracker()
            .add_change_handler(handler.clone());
        optimizer.last_runs.clear();
        for i in 0..self.my_passes.len() {
            optimizer.last_runs.insert(i, START_TIME);
        }
        let mut made_changes = IndexSet::<_>::default();
        let mut run_in_prev_iter = IndexSet::<_>::default();
        let mut did_not_make_changes = IndexSet::<_>::default();
        let mut state = State::RUN_PASSES_NOT_RUN_IN_PREV_ITER;
        let mut count = 1;
        let mut ast_size = NodeUtil::count_ast_size(compiler, root);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            loop {
                if count > optimizer.optimization_loop_max_iterations && self.is_code_removal_loop {
                    return;
                }
                if count > MAX_LOOPS {
                    compiler.throw_internal_error(OPTIMIZE_LOOP_ERROR, "");
                }
                count += 1;
                let mut last_iter_made_changes = false;
                for (i, pass) in self.my_passes.iter_mut().enumerate() {
                    if (state == State::RUN_PASSES_NOT_RUN_IN_PREV_ITER
                        && !run_in_prev_iter.contains(&i)
                        && !did_not_make_changes.contains(&i))
                        || (state == State::RUN_PASSES_THAT_CHANGED_STH_IN_PREV_ITER
                            && made_changes.contains(&i))
                    {
                        compiler.get_change_tracker().increment_change_stamp();
                        optimizer.current_pass = Some(i);
                        optimizer
                            .scope_change_state
                            .lock()
                            .unwrap()
                            .time_of_last_run = optimizer.last_runs[&i];
                        pass.process(optimizer, compiler, externs, root);
                        run_in_prev_iter.insert(i);
                        optimizer
                            .last_runs
                            .insert(i, compiler.get_change_tracker_ref().get_change_stamp());
                        if optimizer.has_halting_errors(compiler) {
                            return;
                        } else if scope_handler
                            .lock()
                            .unwrap()
                            .has_code_changed_since_last_call(compiler.get_change_tracker())
                        {
                            made_changes.insert(i);
                            did_not_make_changes.clear();
                            last_iter_made_changes = true;
                        } else {
                            made_changes.shift_remove(&i);
                            did_not_make_changes.insert(i);
                        }
                    } else {
                        run_in_prev_iter.shift_remove(&i);
                    }
                }
                let previous_ast_size = ast_size;
                ast_size = NodeUtil::count_ast_size(compiler, root);
                if state == State::RUN_PASSES_NOT_RUN_IN_PREV_ITER {
                    if last_iter_made_changes
                        && self.is_ast_sufficiently_changing(optimizer, previous_ast_size, ast_size)
                    {
                        state = State::RUN_PASSES_THAT_CHANGED_STH_IN_PREV_ITER;
                    } else {
                        return;
                    }
                } else {
                    assert_eq!(state, State::RUN_PASSES_THAT_CHANGED_STH_IN_PREV_ITER);
                    if !last_iter_made_changes
                        || !self.is_ast_sufficiently_changing(
                            optimizer,
                            previous_ast_size,
                            ast_size,
                        )
                    {
                        state = State::RUN_PASSES_NOT_RUN_IN_PREV_ITER;
                    }
                }
            }
        }));
        optimizer.in_loop = false;
        optimizer.scope_change_state.lock().unwrap().in_loop = false;
        compiler
            .get_change_tracker()
            .remove_change_handler(&handler);
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
    // port: PhaseOptimizer.Loop#isAstSufficientlyChanging
    fn is_ast_sufficiently_changing(
        &mut self,
        optimizer: &PhaseOptimizer,
        old_ast_size: i32,
        new_ast_size: i32,
    ) -> bool {
        if optimizer.use_size_heuristic_to_stop_optimization_loop && self.is_code_removal_loop {
            let percent_change = 100.0f32
                * (new_ast_size.wrapping_sub(old_ast_size).wrapping_abs() as f32
                    / old_ast_size as f32);
            if f64::from(percent_change) < 0.05 {
                self.howmany_iterations_under_threshold += 1;
            } else {
                self.howmany_iterations_under_threshold = 0;
            }
            return self.howmany_iterations_under_threshold < 2;
        }
        true
    }
    // port: PhaseOptimizer.Loop#isPopulated
    pub fn is_populated(&self) -> bool {
        !self.my_passes.is_empty()
    }
    // port: PhaseOptimizer.Loop#isCodeRemovalLoop
    fn is_code_removal_loop(&self) -> bool {
        self.my_passes
            .iter()
            .any(|p| CODE_REMOVING_PASSES.contains(&p.name.as_str()))
    }
}
pub struct ProgressRange {
    pub initial_value: f64,
    pub max_value: f64,
}
impl ProgressRange {
    // port: PhaseOptimizer.ProgressRange#ProgressRange
    pub fn new(initial_value: f64, max_value: f64) -> Self {
        Self {
            initial_value,
            max_value,
        }
    }
}
