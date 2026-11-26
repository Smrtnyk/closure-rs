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
#[allow(dead_code)] // The build script, which includes this file, counts lines only.
pub(super) fn is_line_terminator(unit: u16) -> bool {
    matches!(unit, 0x0A | 0x0D | 0x2028 | 0x2029)
}

/// Every file has at least one line, including an empty file.
///
/// Rust-only (performance divergence): Java walks the text and skips the LF of each CRLF pair.
/// This counts the same breaks as every line terminator less the CRLF pairs, in passes the
/// compiler can vectorize (the pairs are looked for only in texts with a CR).
pub(super) fn num_lines(units: &[u16]) -> i32 {
    let terminators = units
        .iter()
        .filter(|&&unit| unit == 0x0A || unit == 0x0D || (unit | 1) == 0x2029)
        .count();
    let crlf_pairs = if units.contains(&0x0D) {
        units
            .windows(2)
            .filter(|pair| pair[0] == 0x0D && pair[1] == 0x0A)
            .count()
    } else {
        0
    };
    // Java counts in an int.
    ((terminators - crlf_pairs) as i32).wrapping_add(1)
}

/// `SourceFile#findLineOffsets`: the offset of each line, the first at 0, the others after each
/// line terminator except a CR followed by LF (the line starts after the LF).
///
/// Rust-only (performance divergence): blocks of sixteen units without a line terminator, found
/// by a test the compiler can vectorize, are skipped (Java tests every unit).
#[allow(dead_code)] // The build script, which includes this file, counts lines only.
pub(super) fn line_offsets(units: &[u16]) -> Vec<i32> {
    let mut offsets = vec![0];
    let mut start = 0;
    for block in units.chunks(16) {
        if block
            .iter()
            .fold(false, |any, &unit| any | is_line_terminator(unit))
        {
            for (j, &unit) in block.iter().enumerate() {
                let i = start + j;
                if is_line_terminator(unit) && !(unit == 0x0D && units.get(i + 1) == Some(&0x0A)) {
                    offsets.push((i + 1) as i32);
                }
            }
        }
        start += block.len();
    }
    offsets
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Java's loop (`SourceFile#setCodeAndDoBookkeeping`).
    fn reference(units: &[u16]) -> i32 {
        let mut lines = 1i32;
        let mut i = 0;
        while i < units.len() {
            let ch = units[i];
            if is_line_terminator(ch) {
                if ch == 0x0D && units.get(i + 1) == Some(&0x0A) {
                    i += 1;
                }
                lines = lines.wrapping_add(1);
            }
            i += 1;
        }
        lines
    }

    /// Java's `SourceFile#findLineOffsets`.
    fn reference_offsets(units: &[u16]) -> Vec<i32> {
        let mut offsets = vec![0];
        let mut i = 0;
        while i < units.len() {
            let ch = units[i];
            if is_line_terminator(ch) {
                if ch == 0x0D && i + 1 < units.len() && units[i + 1] == 0x0A {
                    i += 1;
                }
                offsets.push((i + 1) as i32);
            }
            i += 1;
        }
        offsets
    }

    #[test]
    fn counts_like_the_java_loop() {
        let alphabet = [0x0A, 0x0D, 0x2028, 0x2029, 0x2027, 0x0B, 0x0C, 0x61, 0x0E];
        // Every text of up to five units over the alphabet.
        let mut level: Vec<Vec<u16>> = vec![Vec::new()];
        let mut texts = level.clone();
        for _ in 0..5 {
            level = level
                .iter()
                .flat_map(|t| {
                    alphabet.iter().map(move |&u| {
                        let mut t = t.clone();
                        t.push(u);
                        t
                    })
                })
                .collect();
            texts.extend(level.iter().cloned());
        }
        for text in &texts {
            assert_eq!(num_lines(text), reference(text), "{text:x?}");
            assert_eq!(line_offsets(text), reference_offsets(text), "{text:x?}");
        }
        // Long texts, across vector widths.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for len in [31, 64, 127, 1000, 4099] {
            let text: Vec<u16> = (0..len)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    alphabet[(seed % alphabet.len() as u64) as usize]
                })
                .collect();
            assert_eq!(num_lines(&text), reference(&text));
            assert_eq!(line_offsets(&text), reference_offsets(&text));
            assert_eq!(line_offsets(&text).len() as i32, num_lines(&text));
        }
    }
}
