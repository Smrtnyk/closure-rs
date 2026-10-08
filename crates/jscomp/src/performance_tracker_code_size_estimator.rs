/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PerformanceTrackerCodeSizeEstimator.java.

use crate::{
    code_consumer::{CodeConsumer, CodeConsumerState},
    code_generator::CodeGenerator,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::io::Write;
pub struct PerformanceTrackerCodeSizeEstimator {
    size: i32,
    last_char: u16,
    stream: Option<flate2::write::GzEncoder<Vec<u8>>>,
    output: Option<Vec<u8>>,
    track_gz_size: bool,
    state: CodeConsumerState,
}
impl PerformanceTrackerCodeSizeEstimator {
    // port: PerformanceTrackerCodeSizeEstimator#estimate
    pub fn estimate(ast: &Ast, js_root: NodeId, track_gz_size: bool) -> Self {
        let mut estimator = Self::new(track_gz_size);
        CodeGenerator::for_cost_estimation(&mut estimator).add_node(ast, js_root);
        estimator
    }
    // port: PerformanceTrackerCodeSizeEstimator#PerformanceTrackerCodeSizeEstimator
    fn new(track_gz_size: bool) -> Self {
        Self {
            size: 0,
            last_char: 0,
            stream: track_gz_size
                .then(|| flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default())),
            output: None,
            track_gz_size,
            state: CodeConsumerState::default(),
        }
    }
    // port: PerformanceTrackerCodeSizeEstimator#getCodeSize
    pub fn get_code_size(&self) -> i32 {
        self.size
    }
    // port: PerformanceTrackerCodeSizeEstimator#getZippedCodeSize
    pub fn get_zipped_code_size(&mut self) -> i32 {
        if self.track_gz_size {
            if let Some(stream) = self.stream.take() {
                self.output = Some(stream.finish().unwrap());
            }
            self.output.as_ref().unwrap().len() as i32
        } else {
            0
        }
    }
}
impl CodeConsumer for PerformanceTrackerCodeSizeEstimator {
    fn state(&self) -> &CodeConsumerState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut CodeConsumerState {
        &mut self.state
    }
    // port: PerformanceTrackerCodeSizeEstimator#append
    fn append(&mut self, str: &JsString) {
        let len = str.length();
        if len > 0 {
            self.size = self.size.wrapping_add(len as i32);
            self.last_char = str.char_at(len - 1);
            if self.track_gz_size {
                let mut bytes = Vec::new();
                for c in char::decode_utf16(str.as_units().iter().copied()) {
                    match c {
                        Ok(c) => {
                            let mut buffer = [0; 4];
                            bytes.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                        }
                        Err(_) => bytes.push(b'?'),
                    }
                }
                self.stream.as_mut().unwrap().write_all(&bytes).unwrap();
            }
        }
    }
    // port: PerformanceTrackerCodeSizeEstimator#getLastChar
    fn get_last_char(&self) -> u16 {
        self.last_char
    }
}
