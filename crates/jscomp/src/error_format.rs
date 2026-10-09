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
//   src/com/google/javascript/jscomp/ErrorFormat.java.

use crate::{
    lightweight_message_formatter::LightweightMessageFormatter,
    message_formatter::MessageFormatter,
    source_excerpt_provider::{SourceExcerpt, SourceExcerptProvider},
};
use std::sync::Arc;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorFormat {
    SINGLELINE,
    FULL,
    MULTILINE,
    SOURCELESS,
}
impl ErrorFormat {
    // port: ErrorFormat#toFormatter
    pub fn to_formatter(
        self,
        source: Option<Arc<dyn SourceExcerptProvider>>,
        colorize: bool,
    ) -> Box<dyn MessageFormatter> {
        let mut formatter = match self {
            Self::SINGLELINE => LightweightMessageFormatter::new(source.unwrap()),
            Self::FULL => {
                LightweightMessageFormatter::new_with_format(source.unwrap(), SourceExcerpt::FULL)
            }
            Self::MULTILINE => {
                LightweightMessageFormatter::new_with_format(source.unwrap(), SourceExcerpt::REGION)
            }
            Self::SOURCELESS => LightweightMessageFormatter::without_source(),
        };
        formatter.set_colorize(colorize);
        Box::new(formatter)
    }
}
