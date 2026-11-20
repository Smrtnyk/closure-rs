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

/// ECMAScript line terminators. SourceFile counts a CRLF pair as one break.
pub(super) fn is_line_terminator(unit: u16) -> bool {
    matches!(unit, 0x0A | 0x0D | 0x2028 | 0x2029)
}

/// Every file has at least one line, including an empty file.
pub(super) fn num_lines(units: &[u16]) -> i32 {
    let mut lines = 1;
    let mut i = 0;
    while i < units.len() {
        let ch = units[i];
        if is_line_terminator(ch) {
            if ch == 0x0D && units.get(i + 1) == Some(&0x0A) {
                i += 1;
            }
            lines += 1;
        }
        i += 1;
    }
    lines
}
