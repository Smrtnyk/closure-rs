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
//   src/com/google/javascript/jscomp/testing/GeneratorSubject.java.

//! Port of testing/GeneratorSubject.java: a Truth Subject for a lazy sequence of values.
use closure_rhino::fast_hash::IndexSet;
use std::{fmt::Display, hash::Hash};

/// Create some result, optionally based on an index.
// port: GeneratorSubject.Generator
pub trait Generator<U> {
    // port: GeneratorSubject.Generator#generate
    fn generate(&self, index: i32) -> U;
}

impl<U, F: Fn(i32) -> U> Generator<U> for F {
    fn generate(&self, index: i32) -> U {
        self(index)
    }
}

// port: GeneratorSubject#MAX_GENERATION_COUNT
const MAX_GENERATION_COUNT: i32 = 1000;

/// Subject for a lazy sequence of values.
// port: GeneratorSubject
pub struct GeneratorSubject<'a, U> {
    source: &'a dyn Generator<U>,
}

// port: GeneratorSubject#assertGenerator
pub fn assert_generator<U: Eq + Hash + Display>(
    actual: &dyn Generator<U>,
) -> GeneratorSubject<'_, U> {
    GeneratorSubject::assert_generator(actual)
}

impl<'a, U: Eq + Hash + Display> GeneratorSubject<'a, U> {
    // port: GeneratorSubject#GeneratorSubject
    fn new(source: &'a dyn Generator<U>) -> Self {
        Self { source }
    }

    // port: GeneratorSubject#generators
    // port: GeneratorSubject#assertGenerator
    pub fn assert_generator(actual: &'a dyn Generator<U>) -> Self {
        Self::new(actual)
    }

    // port: GeneratorSubject#generatesAtLeast
    pub fn generates_at_least(&self, expected: Vec<U>) {
        let mut expected_set: IndexSet<U> = IndexSet::<_>::default();
        expected_set.extend(expected);

        let mut missing_set: IndexSet<&U> = expected_set.iter().collect();
        let mut found_set: IndexSet<&U> = IndexSet::<_>::default();
        let mut extra_set: IndexSet<U> = IndexSet::<_>::default();

        let mut i = 0;
        while i < MAX_GENERATION_COUNT {
            if missing_set.is_empty() {
                return;
            }

            let v = self.source.generate(i);
            if let Some(expected) = expected_set.get(&v) {
                found_set.insert(expected);
                missing_set.shift_remove(&v);
            } else {
                extra_set.insert(v);
            }
            i += 1;
        }

        fail_without_actual(&[
            ("total generation count", (i + 1).to_string()),
            ("found expected", render_set(found_set.iter().copied())),
            ("still missing", render_set(missing_set.iter().copied())),
            ("found extras", render_set(extra_set.iter())),
        ]);
    }
}

// port: GeneratorSubject#renderSet
fn render_set<'v, U: Display + 'v>(set: impl Iterator<Item = &'v U>) -> String {
    let rendered: Vec<String> = set.map(ToString::to_string).collect();
    format!("[{}]", rendered.join(", "))
}

/// Truth's `Subject#failWithoutActual`: the facts, keys padded to the longest key.
fn fail_without_actual(facts: &[(&str, String)]) -> ! {
    let longest_key_length = facts.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
    let message: Vec<String> = facts
        .iter()
        .map(|(key, value)| format!("{key:<longest_key_length$}: {value}"))
        .collect();
    panic!("{}", message.join("\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_at_least_finds_values() {
        assert_generator(&|i: i32| i % 7).generates_at_least(vec![3, 6, 0]);
    }

    #[test]
    #[should_panic(
        expected = "total generation count: 1001\nfound expected        : [2]\nstill missing         : [9]\nfound extras          : [0, 1]"
    )]
    fn generates_at_least_reports_missing() {
        assert_generator(&|i: i32| i % 3).generates_at_least(vec![2, 9]);
    }
}
