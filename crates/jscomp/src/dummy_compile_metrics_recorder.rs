/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DummyCompileMetricsRecorder.java.

//! Port of `DummyCompileMetricsRecorder.java`.
use crate::{
    abstract_compiler::AbstractCompiler,
    compile_metrics_recorder_interface::CompileMetricsRecorderInterface, result::Result,
};

/// A do-nothing metrics recorder that `AbstractCompilerRunner` uses by default.
#[derive(Debug, Default)]
pub struct DummyCompileMetricsRecorder;

impl CompileMetricsRecorderInterface for DummyCompileMetricsRecorder {
    // port: DummyCompileMetricsRecorder#recordActionStart
    fn record_action_start(&mut self) {}
    // port: DummyCompileMetricsRecorder#recordActionName
    fn record_action_name(&mut self, _action_name: &str) {}
    // port: DummyCompileMetricsRecorder#recordStartState
    fn record_start_state(&mut self, _compiler: &AbstractCompiler) {}
    // port: DummyCompileMetricsRecorder#recordResultMetrics
    fn record_result_metrics(&mut self, _compiler: &AbstractCompiler, _result: Option<&Result>) {}
}
