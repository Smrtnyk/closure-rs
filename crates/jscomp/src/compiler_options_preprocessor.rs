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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CompilerOptionsPreprocessor.java.

use crate::compiler_options::{CompilerOptions, Reach};
pub struct CompilerOptionsPreprocessor;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidOptionsException {
    message: &'static str,
}
impl std::fmt::Display for InvalidOptionsException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for InvalidOptionsException {}
impl CompilerOptionsPreprocessor {
    // port: CompilerOptionsPreprocessor#preprocess
    pub fn preprocess(options: &CompilerOptions) -> Result<(), InvalidOptionsException> {
        if options.get_inline_functions_level() == Reach::NONE
            && options.get_max_function_size_after_inlining()
                != CompilerOptions::UNLIMITED_FUN_SIZE_AFTER_INLINING
        {
            return Err(InvalidOptionsException::new(
                "max_function_size_after_inlining has no effect if inlining is disabled.",
            ));
        }
        Ok(())
    }
    // port: CompilerOptionsPreprocessor#CompilerOptionsPreprocessor
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }
}
impl InvalidOptionsException {
    // port: CompilerOptionsPreprocessor.InvalidOptionsException#InvalidOptionsException
    fn new(message: &'static str) -> Self {
        Self { message }
    }
    // port: Throwable#getMessage (InvalidOptionsException)
    pub fn get_message(&self) -> &'static str {
        self.message
    }
}
