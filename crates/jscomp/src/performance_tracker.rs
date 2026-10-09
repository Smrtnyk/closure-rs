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
//   src/com/google/javascript/jscomp/PerformanceTracker.java.

use crate::recent_change::RecentChange;
use crate::{
    code_change_handler::CodeChangeHandler, compiler_input::CompilerInput,
    compiler_options::TracerMode, node_util::NodeUtil, pass_names, platform::Platform,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};
use std::{
    fmt::Write,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
pub struct PerformanceTracker {
    externs_root: NodeId,
    js_root: NodeId,
    mode: TracerMode,
    code_change: Arc<std::sync::Mutex<RecentChange>>,
    current_pass: Vec<Stats>,
    pass_summary: Option<IndexMap<String, Stats>>,
    ast_manifest: Option<IndexMap<Token, i32>>,
    log: Vec<Stats>,
    start_time: i64,
    init_ast_size: i32,
    init_code_size: i32,
    init_gz_code_size: i32,
    end_time: i64,
    passes_runtime: i32,
    max_mem: i32,
    runs: i32,
    changes: i32,
    loop_runs: i32,
    loop_changes: i32,
    js_lines: i32,
    js_sources: i32,
    extern_lines: i32,
    extern_sources: i32,
    transitive_summary_lines: i32,
    transitive_summary_sources: i32,
    transitive_summary_lines_parsed: i32,
    transitive_summary_sources_parsed: i32,
    pre_pruning_recorded: bool,
    ast_size: i32,
    code_size: i32,
    gz_code_size: i32,
    ast_diff: i32,
    diff: i32,
    gz_diff: i32,
    pruning_analysis_summary: String,
    disambiguate_properties_summary: String,
    ambiguate_properties_summary: String,
}
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub pass: String,
    pub is_one_time: bool,
    pub runtime: i64,
    pub alloc_mem: i32,
    pub runs: i32,
    pub changes: i32,
    pub diff: i32,
    pub gz_diff: i32,
    pub size: i32,
    pub gz_size: i32,
    pub ast_diff: i32,
    pub ast_size: i32,
}
impl Stats {
    // port: PerformanceTracker.Stats#Stats
    pub fn new(pass: impl Into<String>, iot: bool) -> Self {
        Self {
            pass: pass.into(),
            is_one_time: iot,
            ..Self::default()
        }
    }
}
impl PerformanceTracker {
    // port: PerformanceTracker#PerformanceTracker
    pub fn new(externs_root: NodeId, js_root: NodeId, mode: TracerMode) -> Self {
        assert!(
            mode != TracerMode::OFF,
            "PerformanceTracker can't work without tracer data."
        );
        Self {
            externs_root,
            js_root,
            mode,
            code_change: Arc::new(std::sync::Mutex::new(RecentChange::default())),
            current_pass: vec![],
            pass_summary: None,
            ast_manifest: None,
            log: vec![],
            start_time: current_time_millis(),
            init_ast_size: -1,
            init_code_size: -1,
            init_gz_code_size: -1,
            end_time: 0,
            passes_runtime: 0,
            max_mem: 0,
            runs: 0,
            changes: 0,
            loop_runs: 0,
            loop_changes: 0,
            js_lines: 0,
            js_sources: 0,
            extern_lines: 0,
            extern_sources: 0,
            transitive_summary_lines: 0,
            transitive_summary_sources: 0,
            transitive_summary_lines_parsed: 0,
            transitive_summary_sources_parsed: 0,
            pre_pruning_recorded: false,
            ast_size: -1,
            code_size: -1,
            gz_code_size: -1,
            ast_diff: 0,
            diff: 0,
            gz_diff: 0,
            pruning_analysis_summary: "not executed".into(),
            disambiguate_properties_summary: "not executed".into(),
            ambiguate_properties_summary: "not executed".into(),
        }
    }
    // port: PerformanceTracker#getCodeChangeHandler
    pub fn get_code_change_handler(&self) -> Arc<std::sync::Mutex<dyn CodeChangeHandler>> {
        self.code_change.clone()
    }
    // port: PerformanceTracker#recordPassStart
    pub fn record_pass_start(&mut self, pass_name: &str, is_one_time: bool) {
        self.current_pass.push(Stats::new(pass_name, is_one_time));
        if pass_name != "toSource" {
            self.code_change.lock().unwrap().reset();
        }
    }
    // port: PerformanceTracker#updateAfterDeserialize
    pub fn update_after_deserialize(&mut self, ast: &Ast, js_root: NodeId) {
        self.js_root = js_root;
        if !self.tracks_ast_size() {
            return;
        }
        self.ast_size = NodeUtil::count_ast_size(ast, self.js_root);
        self.init_ast_size = self.ast_size;
        if !self.tracks_size() {
            return;
        }
        let (size, gz) = estimate_code_size(ast, self.js_root, self.tracks_gz_size());
        self.code_size = size;
        self.init_code_size = size;
        if self.tracks_gz_size() {
            self.gz_code_size = gz;
            self.init_gz_code_size = gz;
        }
    }
    // port: PerformanceTracker#recordPassStop
    pub fn record_pass_stop(&mut self, ast: &Ast, pass_name: &str, runtime: i64) {
        let alloc_mem = self.get_allocated_megabytes();
        let mut stats = self.current_pass.pop().unwrap();
        assert_eq!(pass_name, stats.pass);
        stats.runtime = runtime;
        stats.alloc_mem = alloc_mem;
        stats.runs = 1;
        if self.code_change.lock().unwrap().has_code_changed() {
            stats.changes = 1;
        }
        if pass_name == pass_names::PARSE_INPUTS {
            self.record_parsing_stop(ast, &mut stats);
        } else if self.code_change.lock().unwrap().has_code_changed() && self.tracks_ast_size() {
            self.record_other_pass_stop(ast, &mut stats);
        }
        self.log.push(stats);
    }
    // port: PerformanceTracker#recordParsingStop
    fn record_parsing_stop(&mut self, ast: &Ast, stats: &mut Stats) {
        if self.tracks_ast_manifest() {
            self.populate_ast_manifest(ast);
        }
        self.record_input_count(ast);
        if !self.tracks_ast_size() {
            return;
        }
        self.ast_size = NodeUtil::count_ast_size(ast, self.js_root);
        self.init_ast_size = self.ast_size;
        stats.ast_size = self.ast_size;
        if !self.tracks_size() {
            return;
        }
        let (size, gz) = estimate_code_size(ast, self.js_root, self.tracks_gz_size());
        self.code_size = size;
        self.init_code_size = size;
        stats.size = size;
        if self.tracks_gz_size() {
            self.gz_code_size = gz;
            self.init_gz_code_size = gz;
            stats.gz_size = gz;
        }
    }
    // port: PerformanceTracker#recordOtherPassStop
    fn record_other_pass_stop(&mut self, ast: &Ast, stats: &mut Stats) {
        let mut new_size = NodeUtil::count_ast_size(ast, self.js_root);
        stats.ast_diff = self.ast_size - new_size;
        self.ast_size = new_size;
        stats.ast_size = new_size;
        if !self.tracks_size() {
            return;
        }
        let (size, gz) = estimate_code_size(ast, self.js_root, self.tracks_gz_size());
        new_size = size;
        stats.diff = self.code_size - new_size;
        self.code_size = new_size;
        stats.size = new_size;
        if self.tracks_gz_size() {
            new_size = gz;
            stats.gz_diff = self.gz_code_size - new_size;
            self.gz_code_size = new_size;
            stats.gz_size = new_size;
        }
    }
    // port: PerformanceTracker#recordInputCount
    fn record_input_count(&mut self, ast: &Ast) {
        for n in self.externs_root.children(ast) {
            let info = n.get_jsdoc_info(ast);
            if info.is_some_and(|i| i.is_type_summary()) {
                self.transitive_summary_sources += 1;
                self.transitive_summary_lines += estimate_num_lines(ast, n);
            } else {
                self.extern_sources += 1;
                self.extern_lines += estimate_num_lines(ast, n);
            }
        }
        for n in self.js_root.children(ast) {
            self.js_sources += 1;
            self.js_lines += estimate_num_lines(ast, n);
        }
    }
    // port: PerformanceTracker#bytesToMB
    fn bytes_to_mb(&self, bytes: i64) -> i32 {
        (bytes / (1024 * 1024)) as i32
    }
    // port: PerformanceTracker#getAllocatedMegabytes
    fn get_allocated_megabytes(&self) -> i32 {
        self.bytes_to_mb(Platform::total_memory() - Platform::free_memory())
    }
    // port: PerformanceTracker#tracksSize
    pub fn tracks_size(&self) -> bool {
        matches!(self.mode, TracerMode::RAW_SIZE | TracerMode::ALL)
    }
    // port: PerformanceTracker#tracksGzSize
    pub fn tracks_gz_size(&self) -> bool {
        self.mode == TracerMode::ALL
    }
    // port: PerformanceTracker#tracksAstSize
    pub fn tracks_ast_size(&self) -> bool {
        self.mode != TracerMode::TIMING_ONLY
    }
    // port: PerformanceTracker#tracksAstManifest
    pub fn tracks_ast_manifest(&self) -> bool {
        self.mode.is_on()
    }
    // port: PerformanceTracker#getRuntime
    pub fn get_runtime(&mut self) -> i32 {
        self.calc_total_stats();
        self.passes_runtime
    }
    // port: PerformanceTracker#getSize
    pub fn get_size(&mut self) -> i32 {
        self.calc_total_stats();
        self.code_size
    }
    // port: PerformanceTracker#getGzSize
    pub fn get_gz_size(&mut self) -> i32 {
        self.calc_total_stats();
        self.gz_code_size
    }
    // port: PerformanceTracker#getAstSize
    pub fn get_ast_size(&mut self) -> i32 {
        self.calc_total_stats();
        self.ast_size
    }
    // port: PerformanceTracker#getChanges
    pub fn get_changes(&mut self) -> i32 {
        self.calc_total_stats();
        self.changes
    }
    // port: PerformanceTracker#getLoopChanges
    pub fn get_loop_changes(&mut self) -> i32 {
        self.calc_total_stats();
        self.loop_changes
    }
    // port: PerformanceTracker#getRuns
    pub fn get_runs(&mut self) -> i32 {
        self.calc_total_stats();
        self.runs
    }
    // port: PerformanceTracker#getLoopRuns
    pub fn get_loop_runs(&mut self) -> i32 {
        self.calc_total_stats();
        self.loop_runs
    }
    // port: PerformanceTracker#getStats
    pub fn get_stats(&mut self) -> &IndexMap<String, Stats> {
        self.calc_total_stats();
        self.pass_summary.as_ref().unwrap()
    }
    // port: PerformanceTracker#calcTotalStats
    fn calc_total_stats(&mut self) {
        if self.pass_summary.is_some() {
            return;
        }
        self.end_time = current_time_millis();
        self.populate_pass_summary();
        for stats in self.pass_summary.as_ref().unwrap().values() {
            self.passes_runtime =
                (i64::from(self.passes_runtime).wrapping_add(stats.runtime)) as i32;
            self.max_mem = self.max_mem.max(stats.alloc_mem);
            self.runs += stats.runs;
            self.changes += stats.changes;
            if !stats.is_one_time {
                self.loop_runs += stats.runs;
                self.loop_changes += stats.changes;
            }
            self.ast_diff += stats.ast_diff;
            self.diff += stats.diff;
            self.gz_diff += stats.gz_diff;
        }
        assert!(!self.tracks_ast_size() || self.init_ast_size == self.ast_diff + self.ast_size);
        assert!(!self.tracks_size() || self.init_code_size == self.diff + self.code_size);
        assert!(
            !self.tracks_gz_size() || self.init_gz_code_size == self.gz_diff + self.gz_code_size
        );
    }
    // port: PerformanceTracker#populatePassSummary
    fn populate_pass_summary(&mut self) {
        let mut summary = IndexMap::<_, _>::default();
        for stat in &self.log {
            let entry = summary
                .entry(stat.pass.clone())
                .or_insert_with(|| Stats::new(&stat.pass, stat.is_one_time));
            entry.runtime += stat.runtime;
            entry.alloc_mem = entry.alloc_mem.max(stat.alloc_mem);
            entry.runs += 1;
            entry.changes += stat.changes;
            entry.ast_diff += stat.ast_diff;
            entry.diff += stat.diff;
            entry.gz_diff += stat.gz_diff;
        }
        self.pass_summary = Some(summary);
    }
    // port: PerformanceTracker#populateAstManifest
    fn populate_ast_manifest(&mut self, ast: &Ast) {
        if self.ast_manifest.is_some() {
            return;
        }
        let mut manifest = IndexMap::<_, _>::default();
        fn visit(ast: &Ast, n: NodeId, m: &mut IndexMap<Token, i32>) {
            *m.entry(n.get_token(ast)).or_default() += 1;
            for c in n.children(ast) {
                visit(ast, c, m);
            }
        }
        visit(ast, self.js_root, &mut manifest);
        self.ast_manifest = Some(manifest);
    }
    // port: PerformanceTracker#recordPrePruningInputCount
    pub fn record_pre_pruning_input_count(
        &mut self,
        original_inputs: &[CompilerInput],
        original_externs: &[CompilerInput],
    ) {
        for input in original_externs.iter().chain(original_inputs) {
            let source = input.get_source_file();
            use closure_rhino::static_source_file::StaticSourceFile;
            if source.get_name().ends_with(".i.js") {
                self.transitive_summary_sources_parsed += 1;
                self.transitive_summary_lines_parsed += source.get_num_lines();
            }
        }
        self.pre_pruning_recorded = true;
    }
    // port: PerformanceTracker#setPruningAnalysisSummary
    pub fn set_pruning_analysis_summary(&mut self, summary: impl Into<String>) {
        self.pruning_analysis_summary = summary.into();
    }
    // port: PerformanceTracker#setDisambiguatePropertiesSummary
    pub fn set_disambiguate_properties_summary(&mut self, summary: impl Into<String>) {
        self.disambiguate_properties_summary = summary.into();
    }
    // port: PerformanceTracker#setAmbiguatePropertiesSummary
    pub fn set_ambiguate_properties_summary(&mut self, summary: impl Into<String>) {
        self.ambiguate_properties_summary = summary.into();
    }
    // port: PerformanceTracker#outputTracerReport
    pub fn output_tracer_report(
        &mut self,
        output: &mut impl std::io::Write,
    ) -> std::io::Result<()> {
        crate::jvm_metrics::JvmMetrics::maybe_write_jvm_metrics(
            output,
            Some("verbose:pretty:all"),
        )?;
        self.calc_total_stats();
        let mut report = String::new();
        writeln!(report).unwrap();
        writeln!(report, "TOTAL:").unwrap();
        writeln!(report, "Start time(ms): {}", self.start_time).unwrap();
        writeln!(report, "End time(ms): {}", self.end_time).unwrap();
        writeln!(
            report,
            "Wall time(ms): {}",
            (self.end_time - self.start_time)
        )
        .unwrap();
        writeln!(report, "Passes runtime(ms): {}", self.passes_runtime).unwrap();
        writeln!(
            report,
            "Max mem usage (measured after each pass)(MB): {}",
            self.max_mem
        )
        .unwrap();
        writeln!(report, "#Runs: {}", self.runs).unwrap();
        writeln!(report, "#Changing runs: {}", self.changes).unwrap();
        writeln!(report, "#Loopable runs: {}", self.loop_runs).unwrap();
        writeln!(report, "#Changing loopable runs: {}", self.loop_changes).unwrap();
        writeln!(report, "Estimated AST reduction(#nodes): {}", self.ast_diff).unwrap();
        writeln!(report, "Estimated Reduction(bytes): {}", self.diff).unwrap();
        writeln!(report, "Estimated GzReduction(bytes): {}", self.gz_diff).unwrap();
        writeln!(report, "Estimated AST size(#nodes): {}", self.ast_size).unwrap();
        writeln!(report, "Estimated Size(bytes): {}", self.code_size).unwrap();
        writeln!(report, "Estimated GzSize(bytes): {}", self.gz_code_size).unwrap();
        writeln!(report).unwrap();
        writeln!(
            report,
            "DisambiguateProperties: {}",
            self.disambiguate_properties_summary
        )
        .unwrap();
        writeln!(
            report,
            "AmbiguateProperties: {}",
            self.ambiguate_properties_summary
        )
        .unwrap();
        writeln!(report).unwrap();
        writeln!(report, "Inputs:").unwrap();
        writeln!(report, "JS lines: {}", self.js_lines).unwrap();
        writeln!(report, "JS sources: {}", self.js_sources).unwrap();
        writeln!(report, "Extern lines: {}", self.extern_lines).unwrap();
        writeln!(report, "Extern sources: {}", self.extern_sources).unwrap();
        writeln!(
            report,
            "Type summary lines (raw input): {}",
            if self.pre_pruning_recorded {
                self.transitive_summary_lines_parsed
            } else {
                self.transitive_summary_lines
            }
        )
        .unwrap();
        writeln!(
            report,
            "Type summary lines (post-pruning): {}",
            self.transitive_summary_lines
        )
        .unwrap();
        writeln!(
            report,
            "Type summary sources (raw input): {}",
            if self.pre_pruning_recorded {
                self.transitive_summary_sources_parsed
            } else {
                self.transitive_summary_sources
            }
        )
        .unwrap();
        writeln!(
            report,
            "Type summary sources (post-pruning): {}",
            self.transitive_summary_sources
        )
        .unwrap();
        writeln!(report).unwrap();
        writeln!(
            report,
            "Dependency pruning analysis: {}",
            self.pruning_analysis_summary
        )
        .unwrap();
        writeln!(report).unwrap();
        writeln!(report, "Summary:").unwrap();
        writeln!(
            report,
            "pass,runtime,allocMem,runs,changingRuns,astReduction,reduction,gzReduction"
        )
        .unwrap();
        let mut entries: Vec<_> = self.pass_summary.as_ref().unwrap().iter().collect();
        entries.sort_by_key(|(_, stats)| stats.runtime);
        for (key, s) in entries {
            writeln!(
                report,
                "{},{},{},{},{},{},{},{}",
                key, s.runtime, s.alloc_mem, s.runs, s.changes, s.ast_diff, s.diff, s.gz_diff
            )
            .unwrap();
        }
        writeln!(report,"\nLog:\npass,runtime,allocMem,codeChanged,astReduction,reduction,gzReduction,astSize,size,gzSize").unwrap();
        for s in &self.log {
            writeln!(
                report,
                "{},{},{},{},{},{},{},{},{},{}",
                s.pass,
                s.runtime,
                s.alloc_mem,
                s.changes == 1,
                s.ast_diff,
                s.diff,
                s.gz_diff,
                s.ast_size,
                s.size,
                s.gz_size
            )
            .unwrap();
        }
        if let Some(manifest) = &self.ast_manifest {
            writeln!(report, "\nInput AST Manifest:\ntoken,count").unwrap();
            let mut rows: Vec<_> = manifest.iter().map(|(t, c)| format!("{t},{c}")).collect();
            rows.sort();
            for row in rows {
                writeln!(report, "{row}").unwrap();
            }
        }
        writeln!(report).unwrap();
        output.write_all(report.as_bytes())?;
        output.flush()
    }
}
fn current_time_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
fn estimate_num_lines(ast: &Ast, n: NodeId) -> i32 {
    NodeUtil::estimate_num_lines(ast, n)
}
fn estimate_code_size(ast: &Ast, root: NodeId, gzip: bool) -> (i32, i32) {
    let mut estimator=crate::performance_tracker_code_size_estimator::PerformanceTrackerCodeSizeEstimator::estimate(ast,root,gzip);
    (estimator.get_code_size(), estimator.get_zipped_code_size())
}
