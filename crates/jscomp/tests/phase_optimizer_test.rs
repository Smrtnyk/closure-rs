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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/PhaseOptimizerTest.java.

use closure_jscomp::{
    Compiler,
    compiler_options::TracerMode,
    compiler_pass::CompilerPass,
    pass_factory::{PassFactory, PreconditionResult},
    pass_names,
    performance_tracker::PerformanceTracker,
    phase_optimizer::{MAX_LOOPS, OPTIMIZE_LOOP_ERROR, PhaseOptimizer},
};
use closure_rhino::{ir::IR, node::NodeId, token::Token};
use std::sync::{Arc, Mutex};
// port: PhaseOptimizerTest#setUp
fn setup() -> (
    Compiler,
    PhaseOptimizer,
    NodeId,
    NodeId,
    NodeId,
    Arc<Mutex<Vec<String>>>,
) {
    let mut compiler = Compiler::new();
    compiler.init_compiler_options_if_testing();
    let externs = compiler.new_node(Token::ROOT);
    let script = IR::script(&mut compiler);
    let root = IR::root(&mut compiler, &[script]);
    let tracker = Arc::new(Mutex::new(PerformanceTracker::new(
        externs,
        root,
        TracerMode::TIMING_ONLY,
    )));
    let optimizer = PhaseOptimizer::new(&compiler, Some(tracker));
    (
        compiler,
        optimizer,
        externs,
        root,
        script,
        Arc::new(Mutex::new(vec![])),
    )
}
// port: PhaseOptimizerTest#createPassFactory
fn factory(
    name: &str,
    num_changes: i32,
    is_one_time: bool,
    script: NodeId,
    runs: &Arc<Mutex<Vec<String>>>,
) -> PassFactory {
    let changes = Arc::new(Mutex::new(num_changes));
    let name = name.to_owned();
    let runs = runs.clone();
    PassFactory::builder()
        .set_name(name.clone())
        .set_run_in_fixed_point_loop(!is_one_time)
        .set_internal_factory(Arc::new(move |_| {
            let changes = changes.clone();
            let runs = runs.clone();
            let name = name.clone();
            Box::new(move |compiler: &mut Compiler, _, _| {
                runs.lock().unwrap().push(name.clone());
                let mut count = changes.lock().unwrap();
                if *count > 0 {
                    *count -= 1;
                    compiler.report_change_to_enclosing_scope(script);
                }
            })
        }))
        .build()
}
// port: PhaseOptimizerTest#assertPasses
fn assert_passes(
    compiler: &mut Compiler,
    optimizer: &mut PhaseOptimizer,
    externs: NodeId,
    root: NodeId,
    runs: &Arc<Mutex<Vec<String>>>,
    expected: &[&str],
) {
    optimizer.process(compiler, externs, root);
    assert_eq!(*runs.lock().unwrap(), expected);
}
fn panic_message(f: impl FnOnce()) -> String {
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_err();
    if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        err.downcast_ref::<&str>().unwrap().to_string()
    }
}
// port: PhaseOptimizerTest#testOneRun
#[test]
fn test_one_run() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    optimizer.add_one_time_pass(factory("x", 0, true, script, &runs));
    assert_passes(&mut compiler, &mut optimizer, externs, root, &runs, &["x"]);
}
// port: PhaseOptimizerTest#testLoop1
#[test]
fn test_loop1() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    {
        let lp = optimizer.add_fixed_point_loop();
        lp.add_looped_pass(factory("x", 0, false, script, &runs));
    }
    assert_passes(&mut compiler, &mut optimizer, externs, root, &runs, &["x"]);
}
// port: PhaseOptimizerTest#testLoop2
#[test]
fn test_loop2() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    {
        let lp = optimizer.add_fixed_point_loop();
        lp.add_looped_pass(factory("x", 3, false, script, &runs));
    }
    assert_passes(
        &mut compiler,
        &mut optimizer,
        externs,
        root,
        &runs,
        &["x", "x", "x", "x"],
    );
}
// port: PhaseOptimizerTest#testSchedulingOfLoopablePasses
#[test]
fn test_scheduling_of_loopable_passes() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    {
        let lp = optimizer.add_fixed_point_loop();
        lp.add_looped_pass(factory("x", 3, false, script, &runs));
        lp.add_looped_pass(factory("y", 1, false, script, &runs));
    }
    assert_passes(
        &mut compiler,
        &mut optimizer,
        externs,
        root,
        &runs,
        &["x", "y", "x", "y", "x", "x", "y"],
    );
}
// port: PhaseOptimizerTest#testCapLoopIterations
#[test]
fn test_cap_loop_iterations() {
    let (mut compiler, _optimizer, externs, root, script, runs) = setup();
    compiler
        .get_options_mut()
        .set_max_optimization_loop_iterations(1);
    let mut optimizer = PhaseOptimizer::new(
        &compiler,
        Some(Arc::new(Mutex::new(PerformanceTracker::new(
            externs,
            root,
            TracerMode::TIMING_ONLY,
        )))),
    );
    {
        let lp = optimizer.add_fixed_point_loop();
        lp.add_looped_pass(factory(
            pass_names::PEEPHOLE_OPTIMIZATIONS,
            2,
            false,
            script,
            &runs,
        ));
    }
    assert_passes(
        &mut compiler,
        &mut optimizer,
        externs,
        root,
        &runs,
        &[pass_names::PEEPHOLE_OPTIMIZATIONS],
    );
}
// port: PhaseOptimizerTest#testNotInfiniteLoop
#[test]
fn test_not_infinite_loop() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    {
        let lp = optimizer.add_fixed_point_loop();
        lp.add_looped_pass(factory("x", MAX_LOOPS - 2, false, script, &runs));
    }
    optimizer.process(&mut compiler, externs, root);
    assert_eq!(compiler.get_error_count(), 0);
}
// port: PhaseOptimizerTest#testInfiniteLoop
#[test]
fn test_infinite_loop() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    {
        let lp = optimizer.add_fixed_point_loop();
        lp.add_looped_pass(factory("x", MAX_LOOPS + 1, false, script, &runs));
    }
    assert!(
        panic_message(|| optimizer.process(&mut compiler, externs, root))
            .contains(OPTIMIZE_LOOP_ERROR)
    );
}
// port: PhaseOptimizerTest#testSchedulingOfAnyKindOfPasses1
#[test]
fn test_scheduling_of_any_kind_of_passes1() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    optimizer.add_one_time_pass(factory("a", 0, true, script, &runs));
    {
        let lp = optimizer.add_fixed_point_loop();
        lp.add_looped_pass(factory("x", 3, false, script, &runs));
        lp.add_looped_pass(factory("y", 1, false, script, &runs));
    }
    optimizer.add_one_time_pass(factory("z", 0, true, script, &runs));
    assert_passes(
        &mut compiler,
        &mut optimizer,
        externs,
        root,
        &runs,
        &["a", "x", "y", "x", "y", "x", "x", "y", "z"],
    );
}
// port: PhaseOptimizerTest#testSchedulingOfAnyKindOfPasses2
#[test]
fn test_scheduling_of_any_kind_of_passes2() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    optimizer.consume(vec![
        factory("a", 0, true, script, &runs),
        factory("b", 1, false, script, &runs),
        factory("c", 2, false, script, &runs),
        factory("d", 1, false, script, &runs),
        factory("e", 1, true, script, &runs),
        factory("f", 0, true, script, &runs),
    ]);
    assert_passes(
        &mut compiler,
        &mut optimizer,
        externs,
        root,
        &runs,
        &["a", "b", "c", "d", "b", "c", "d", "c", "b", "e", "f"],
    );
}
// port: PhaseOptimizerTest#testSchedulingOfAnyKindOfPasses3
#[test]
fn test_scheduling_of_any_kind_of_passes3() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    optimizer.consume(vec![
        factory("a", 2, false, script, &runs),
        factory("b", 1, true, script, &runs),
        factory("c", 1, false, script, &runs),
    ]);
    assert_passes(
        &mut compiler,
        &mut optimizer,
        externs,
        root,
        &runs,
        &["a", "a", "a", "b", "c", "c"],
    );
}
// port: PhaseOptimizerTest#testSchedulingOfAnyKindOfPasses4
#[test]
fn test_scheduling_of_any_kind_of_passes4() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    optimizer.consume(vec![
        factory("a", 2, true, script, &runs),
        factory("b", 0, false, script, &runs),
        factory("c", 0, false, script, &runs),
    ]);
    assert_passes(
        &mut compiler,
        &mut optimizer,
        externs,
        root,
        &runs,
        &["a", "b", "c"],
    );
}
// port: PhaseOptimizerTest#testDuplicateLoop
#[test]
fn test_duplicate_loop() {
    let (_compiler, mut optimizer, _externs, _root, script, runs) = setup();
    let lp = optimizer.add_fixed_point_loop();
    lp.add_looped_pass(factory("x", 1, false, script, &runs));
    let message = panic_message(|| lp.add_looped_pass(factory("x", 1, false, script, &runs)));
    assert_eq!(message, "Already a pass with name 'x' in this loop");
}
// port: PhaseOptimizerTest#preconditionCheck_success
#[test]
fn precondition_check_success() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    let pass_factory = factory("myPass", 0, true, script, &runs)
        .to_builder()
        .set_precondition_check(Arc::new(|_| PreconditionResult {
            success: true,
            message: None,
        }))
        .build();
    optimizer.add_one_time_pass(pass_factory);
    assert_passes(
        &mut compiler,
        &mut optimizer,
        externs,
        root,
        &runs,
        &["myPass"],
    );
}
// port: PhaseOptimizerTest#preconditionCheck_failure
#[test]
fn precondition_check_failure() {
    let (mut compiler, mut optimizer, externs, root, script, runs) = setup();
    let pass_factory = factory("myPass", 0, true, script, &runs)
        .to_builder()
        .set_precondition_check(Arc::new(|_| PreconditionResult {
            success: false,
            message: Some("message".into()),
        }))
        .build();
    optimizer.add_one_time_pass(pass_factory);
    assert_eq!(
        panic_message(|| assert_passes(
            &mut compiler,
            &mut optimizer,
            externs,
            root,
            &runs,
            &["myPass"]
        )),
        "Precondition for pass myPass failed: message"
    );
}
