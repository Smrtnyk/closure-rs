/*
 * Copyright 2013 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/PerformanceTrackerTest.java.

use closure_jscomp::{
    compiler_input::CompilerInput, compiler_options::TracerMode, pass_names,
    performance_tracker::PerformanceTracker, source_file::SourceFile,
};
use closure_rhino::{
    input_id::InputId,
    ir::IR,
    java_lang::pattern::Pattern,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
};
use std::sync::Arc;
fn roots() -> (Ast, NodeId, NodeId) {
    let mut ast = Ast::new();
    let externs = IR::block(&mut ast);
    let js = IR::block(&mut ast);
    (ast, externs, js)
}
// port: PerformanceTrackerTest#extractReport
fn report(tracker: &mut PerformanceTracker) -> String {
    let mut output = Vec::new();
    tracker.output_tracer_report(&mut output).unwrap();
    String::from_utf8(output).unwrap()
}
// port: PerformanceTrackerTest#testStatsCalculation
#[test]
fn test_stats_calculation() {
    let (ast, externs, js) = roots();
    let mut tracker = PerformanceTracker::new(externs, js, TracerMode::ALL);
    let handler = tracker.get_code_change_handler();
    let pass_runtime = 5;
    for (name, one_time, changed) in [
        ("noloopA", true, true),
        ("noloopB", true, true),
        ("loopA", false, true),
        ("loopA", false, false),
        ("noloopB", true, true),
        ("loopB", false, false),
        ("noloopB", true, false),
    ] {
        tracker.record_pass_start(name, one_time);
        if changed {
            handler.lock().unwrap().report_change();
        }
        tracker.record_pass_stop(&ast, name, pass_runtime);
    }
    let num_runs = tracker.get_runs();
    assert_eq!(num_runs, 7);
    assert_eq!(num_runs * pass_runtime as i32, tracker.get_runtime());
    assert_eq!(tracker.get_loop_runs(), 3);
    assert_eq!(tracker.get_changes(), 4);
    assert_eq!(tracker.get_loop_changes(), 1);
    let stats = tracker.get_stats();
    for (name, runs, changes) in [
        ("noloopA", 1, 1),
        ("noloopB", 3, 2),
        ("loopA", 2, 1),
        ("loopB", 1, 0),
    ] {
        let st = stats.get(name).unwrap();
        assert_eq!(st.runs, runs);
        assert_eq!(st.runtime, i64::from(runs) * pass_runtime);
        assert_eq!(st.changes, changes);
    }
}
// port: PerformanceTrackerTest#testAstSummaryAndFormat
#[test]
fn test_ast_summary_and_format() {
    let (mut ast, externs, _) = roots();
    let a = IR::string(&mut ast, "a");
    let b = IR::string(&mut ast, "b");
    let c = IR::string(&mut ast, "c");
    let hook = IR::hook(&mut ast, a, b, c);
    let expr = IR::expr_result(&mut ast, hook);
    let inner = IR::block_with_child(&mut ast, expr);
    let outer = IR::block_with_child(&mut ast, inner);
    let name = IR::name(&mut ast, "name");
    let params = IR::param_list(&mut ast, &[]);
    let body = IR::block(&mut ast);
    let function = IR::function(&mut ast, name, params, body);
    let script = IR::script(&mut ast);
    script.add_child_to_back(&mut ast, outer);
    script.add_child_to_back(&mut ast, function);
    let main = IR::root(&mut ast, &[script]);
    let mut tracker = PerformanceTracker::new(externs, main, TracerMode::TIMING_ONLY);
    tracker.record_pass_start(pass_names::PARSE_INPUTS, true);
    tracker.record_pass_stop(&ast, pass_names::PARSE_INPUTS, 0);
    let report = report(&mut tracker);
    assert!(Pattern::compile("(?s)Input AST Manifest:\ntoken,count\nBLOCK,3\nEXPR_RESULT,1\nFUNCTION,1\nHOOK,1\nNAME,1\nPARAM_LIST,1\nROOT,1\nSCRIPT,1\nSTRINGLIT,3\n").matcher(&report).find(),"{report}");
}
// port: PerformanceTrackerTest#createScript
fn create_script(ast: &mut Ast, name: &str) -> NodeId {
    let script = IR::script(ast);
    script.set_static_source_file(ast, Some(Arc::new(SourceFile::from_code(name, ""))));
    script
}
// port: PerformanceTrackerTest#createTypeSummary
fn create_type_summary(ast: &mut Ast, name: &str) -> NodeId {
    let script = create_script(ast, name);
    let mut info = JSDocInfo::builder();
    info.record_type_summary();
    script.set_jsdoc_info(ast, info.build());
    script
}
// port: PerformanceTrackerTest#createExtern
fn create_extern(ast: &mut Ast, name: &str) -> NodeId {
    let script = create_script(ast, name);
    let mut info = JSDocInfo::builder();
    info.record_externs();
    script.set_jsdoc_info(ast, info.build());
    script
}
// port: PerformanceTrackerTest#testDistinguishBetweenSrcExternAndIjs
#[test]
fn test_distinguish_between_src_extern_and_ijs() {
    let (mut ast, externs, js) = roots();
    for name in ["externs1.js", "externs2.js", "externs3.js"] {
        let script = create_extern(&mut ast, name);
        externs.add_child_to_front(&mut ast, script);
    }
    let summary = create_type_summary(&mut ast, "type_dep.js.i.js");
    externs.add_child_to_front(&mut ast, summary);
    for name in ["input1.js", "input2.js"] {
        let script = create_script(&mut ast, name);
        js.add_child_to_front(&mut ast, script);
    }
    let mut tracker = PerformanceTracker::new(externs, js, TracerMode::ALL);
    tracker.record_pass_start(pass_names::PARSE_INPUTS, true);
    tracker.record_pass_stop(&ast, pass_names::PARSE_INPUTS, 0);
    let report = report(&mut tracker);
    assert!(Pattern::compile("(?s)JS lines:\\s* 0\nJS sources:\\s* 2\nExtern lines:\\s* 0\nExtern sources:\\s* 3\nType summary lines \\(raw input\\):\\s* 0\nType summary lines \\(post-pruning\\):\\s* 0\nType summary sources \\(raw input\\):\\s* 1\nType summary sources \\(post-pruning\\):\\s* 1\n").matcher(&report).find(),"{report}");
}
// port: PerformanceTrackerTest#testRecordPrePruningInputCount_distinguishesPrunedAndNonPrunedIjs
#[test]
fn test_record_pre_pruning_input_count_distinguishes_pruned_and_non_pruned_ijs() {
    let (mut ast, externs, js) = roots();
    let first = CompilerInput::new_with_input_id(
        SourceFile::from_code("type1.js.i.js", ""),
        InputId::new("type1.js.i.js"),
    );
    let second = CompilerInput::new_with_input_id(
        SourceFile::from_code("type2.js.i.js", ""),
        InputId::new("type2.js.i.js"),
    );
    let pruned = CompilerInput::new_with_input_id(
        SourceFile::from_code("type_depPruned.js.i.js", "a \n b \n c"),
        InputId::new("pruned.js.i.js"),
    );
    for name in ["type1.js.i.js", "type2.js.i.js"] {
        let script = create_type_summary(&mut ast, name);
        externs.add_child_to_front(&mut ast, script);
    }
    let mut tracker = PerformanceTracker::new(externs, js, TracerMode::ALL);
    tracker.record_pass_start(pass_names::PARSE_INPUTS, true);
    tracker.record_pre_pruning_input_count(&[first, pruned], &[second]);
    tracker.record_pass_stop(&ast, pass_names::PARSE_INPUTS, 0);
    let report = report(&mut tracker);
    assert!(Pattern::compile("(?s)Type summary lines \\(raw input\\):\\s* 5\nType summary lines \\(post-pruning\\):\\s* 0\nType summary sources \\(raw input\\):\\s* 3\nType summary sources \\(post-pruning\\):\\s* 2\n").matcher(&report).find(),"{report}");
}

// port: PerformanceTrackerTest#testOutputFormat
#[test]
fn test_output_format() {
    let (_ast, externs, js) = roots();
    let mut tracker = PerformanceTracker::new(externs, js, TracerMode::ALL);
    let report = report(&mut tracker);
    let pattern = Pattern::compile(
        &concat!(
            "(?s)",
            r#".*TOTAL:
Start time\(ms\): [0-9]+
End time\(ms\): [0-9]+
Wall time\(ms\): [0-9]+
Passes runtime\(ms\): [0-9]+
Max mem usage \(measured after each pass\)\(MB\): -?[0-9]+
#Runs: [0-9]+
#Changing runs: [0-9]+
#Loopable runs: [0-9]+
#Changing loopable runs: [0-9]+
Estimated AST reduction\(#nodes\): [0-9]+
Estimated Reduction\(bytes\): [0-9]+
Estimated GzReduction\(bytes\): [0-9]+
Estimated AST size\(#nodes\): -?[0-9]+
Estimated Size\(bytes\): -?[0-9]+
Estimated GzSize\(bytes\): -?[0-9]+

DisambiguateProperties: not executed
AmbiguateProperties: not executed

Inputs:
JS lines:\s*[0-9]+
JS sources:\s*[0-9]+
Extern lines:\s*[0-9]+
Extern sources:\s*[0-9]+
Type summary lines \(raw input\):\s*[0-9]+
Type summary lines \(post-pruning\):\s*[0-9]+
Type summary sources \(raw input\):\s*[0-9]+
Type summary sources \(post-pruning\):\s*[0-9]+

Dependency pruning analysis: not executed

Summary:
pass,runtime,allocMem,runs,changingRuns,astReduction,reduction,gzReduction

Log:
pass,runtime,allocMem,codeChanged,astReduction,reduction,gzReduction,astSize,size,gzSize
.*
"#
        )
        .replace(".*", r"[\s\S]*"),
    );
    assert!(pattern.matcher(&report).matches(), "{report}");
}
#[test]
fn code_size_estimator_matches_java() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("data/compiler_estimator.json")).unwrap();
    for row in rows {
        let mut compiler = closure_jscomp::Compiler::new();
        let script = compiler.parse_test_code(row["source"].as_str().unwrap());
        let root = IR::root(&mut compiler, &[script]);
        let mut estimator=closure_jscomp::performance_tracker_code_size_estimator::PerformanceTrackerCodeSizeEstimator::estimate(&compiler,root,true);
        assert_eq!(
            i64::from(estimator.get_code_size()),
            row["size"].as_i64().unwrap(),
            "{}",
            row["source"]
        );
        assert_eq!(
            i64::from(estimator.get_zipped_code_size()),
            row["gzip"].as_i64().unwrap(),
            "{}",
            row["source"]
        );
    }
}
