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
//   src/com/google/javascript/jscomp/AbstractCompiler.java.

use crate::{compiler::Compiler, diagnostic_type::DiagnosticType};

pub type AbstractCompiler = Compiler;

pub const READ_ERROR: DiagnosticType =
    DiagnosticType::error("JSC_READ_ERROR", "Cannot read file {0}: {1}");

impl Compiler {
    pub const RUNTIME_LIB_DIR: &'static str = "src/com/google/javascript/jscomp/js/";
}

const FILL_FILE_SUFFIX: &str = "$fillFile";
impl AbstractCompiler {
    // port: AbstractCompiler#createFillFileName
    pub fn create_fill_file_name(chunk_name: &str) -> String {
        format!("{chunk_name}{FILL_FILE_SUFFIX}")
    }
    // port: AbstractCompiler#isFillFileName
    pub fn is_fill_file_name(file_name: &str) -> bool {
        file_name.ends_with(FILL_FILE_SUFFIX)
    }
}

// port: AbstractCompiler.ConfigContext
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigContext {
    DEFAULT,
    EXTERNS,
}
// port: AbstractCompiler.LifeCycleStage
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LifeCycleStage {
    #[default]
    RAW,
    COLORS_AND_SIMPLIFIED_JSDOC,
    NORMALIZED,
    NORMALIZED_OBFUSCATED,
}
impl LifeCycleStage {
    // port: AbstractCompiler.LifeCycleStage#isNormalized
    pub fn is_normalized(self) -> bool {
        matches!(self, Self::NORMALIZED | Self::NORMALIZED_OBFUSCATED)
    }
    // port: AbstractCompiler.LifeCycleStage#isNormalizedUnobfuscated
    pub fn is_normalized_unobfuscated(self) -> bool {
        self == Self::NORMALIZED
    }
    // port: AbstractCompiler.LifeCycleStage#isNormalizedObfuscated
    pub fn is_normalized_obfuscated(self) -> bool {
        self == Self::NORMALIZED_OBFUSCATED
    }
    // port: AbstractCompiler.LifeCycleStage#hasColorAndSimplifiedJSDoc
    pub fn has_color_and_simplified_jsdoc(self) -> bool {
        self != Self::RAW
    }
}
