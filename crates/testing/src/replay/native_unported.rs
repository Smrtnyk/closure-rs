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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! Preserve a missing PassFactory cause when compiler exception wrappers replace its message.
use crate::throwable::Throwable;
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Once,
};

thread_local! {
    static WITNESSES: RefCell<Vec<Option<Throwable>>> = const { RefCell::new(Vec::new()) };
}
static HOOK: Once = Once::new();

// port: ReplayMain#main (native dependency outcome through wrapped compiler exceptions)
pub fn capture<T>(action: impl FnOnce() -> T) -> Result<T, Throwable> {
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info
                .payload()
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| info.payload().downcast_ref::<&str>().copied());
            let missing = info
                .location()
                .filter(|location| location.file().ends_with("/pass_factory.rs"))
                .and_then(|_| message.and_then(|s| s.strip_prefix("unported: ")));
            let observing = WITNESSES.with(|stack| {
                let Ok(mut stack) = stack.try_borrow_mut() else {
                    return false;
                };
                let Some(witness) = stack.last_mut() else {
                    return false;
                };
                if let Some(class) = missing {
                    witness.get_or_insert_with(|| {
                        Throwable::Unported({
                            if class.starts_with("com.") {
                                class.into()
                            } else {
                                format!("com.google.javascript.jscomp.{}", class.replace('.', "$"))
                            }
                        })
                    });
                } else if let Some(error) = info.payload().downcast_ref::<Throwable>() {
                    witness.get_or_insert_with(|| error.clone());
                }
                missing.is_some() || witness.is_some()
            });
            if !observing {
                previous(info);
            }
        }));
    });
    WITNESSES.with(|stack| stack.borrow_mut().push(None));
    let outcome = catch_unwind(AssertUnwindSafe(action));
    let missing =
        WITNESSES.with(|stack| stack.borrow_mut().pop().expect("native dependency witness"));
    if let Some(error) = missing {
        return Err(error);
    }
    match outcome {
        Ok(value) => Ok(value),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
