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

#![forbid(unsafe_code)]
#[path = "../tests/diff_support/mod.rs"]
mod diff_support;
fn main() {
    if std::env::var_os("SOURCEMAP_DIFF_PANICS").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }
    let mut failed = false;
    for file in std::env::args().skip(1) {
        let (matched, total) = diff_support::replay_file(std::path::Path::new(&file), true);
        failed |= matched != total;
    }
    if failed {
        std::process::exit(1);
    }
}
