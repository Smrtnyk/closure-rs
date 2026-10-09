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
//   src/com/google/javascript/jscomp/CompileMetricsRecorderInterface.java.

//! Port of `CompileMetricsRecorderInterface.java`.
use crate::{abstract_compiler::AbstractCompiler, result::Result};

/// Records metrics of a compile action.
pub trait CompileMetricsRecorderInterface {
    // port: CompileMetricsRecorderInterface#recordActionStart
    fn record_action_start(&mut self);
    // port: CompileMetricsRecorderInterface#recordActionName
    fn record_action_name(&mut self, action_name: &str);
    // port: CompileMetricsRecorderInterface#recordStartState
    fn record_start_state(&mut self, compiler: &AbstractCompiler);
    // port: CompileMetricsRecorderInterface#recordResultMetrics
    fn record_result_metrics(&mut self, compiler: &AbstractCompiler, result: Option<&Result>);
}
