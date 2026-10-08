/*
 * Copyright 2026 The closure-rs Authors.
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

//! Java harness exceptions used as control flow.
use std::fmt;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Throwable {
    Assertion {
        message: String,
    },
    Exception {
        class: String,
        message: Option<String>,
    },
    Unported(String),
    HarnessError(String),
}
impl fmt::Display for Throwable {
    // port: Throwable#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assertion { message } => write!(f, "AssertionError: {message}"),
            Self::Exception { class, message } => {
                write!(f, "{class}: {}", message.as_deref().unwrap_or("null"))
            }
            Self::Unported(item) => write!(f, "unported: {item}"),
            Self::HarnessError(message) => write!(f, "harness error: {message}"),
        }
    }
}
impl std::error::Error for Throwable {}
// port: Preconditions#checkState
pub fn check_state(ok: bool, message: &str) -> Result<(), Throwable> {
    if ok {
        Ok(())
    } else {
        Err(Throwable::Exception {
            class: "java.lang.IllegalStateException".into(),
            message: Some(message.into()),
        })
    }
}
// port: Truth#assertWithMessage
pub fn assert_that(ok: bool, message: impl Into<String>) -> Result<(), Throwable> {
    if ok {
        Ok(())
    } else {
        Err(Throwable::Assertion {
            message: message.into(),
        })
    }
}
