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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 8ac03b6:
//   src/com/google/javascript/jscomp/UniqueIdSupplier.java.

//! Port of `UniqueIdSupplier.java`.

use crate::compiler_input::CompilerInput;
use closure_rhino::js_string::JsString;
use closure_rhino::static_source_file::StaticSourceFile;

pub use crate::compiler_state_proto::UniqueIdProto;

/// Generates unique String Ids when requested via a compiler instance.
///
/// This supplier provides Ids that are deterministic and unique across all input files given to
/// the compiler. The generated ID format is: uniqueId = "fileHashCode$counterForThisFile"
#[derive(Debug, Clone, Default)]
pub struct UniqueIdSupplier {
    /// Guava `LinkedHashMultiset<Integer>`: element -> count, in insertion order.
    counter: closure_rhino::fast_hash::IndexMap<i32, i32>,
}

impl UniqueIdSupplier {
    // port: UniqueIdSupplier#UniqueIdSupplier
    pub fn new() -> Self {
        Self {
            counter: closure_rhino::fast_hash::IndexMap::<_, _>::default(),
        }
    }

    /// Guava `Multiset#add(E, int)`: returns the count before the operation.
    fn counter_add(&mut self, element: i32, occurrences: i32) -> i32 {
        if occurrences == 0 {
            return self.counter.get(&element).copied().unwrap_or(0);
        }
        let count = self.counter.entry(element).or_insert(0);
        let old = *count;
        *count = old.wrapping_add(occurrences);
        old
    }

    /// Creates and returns a unique Id across all compiler input source files.
    // port: UniqueIdSupplier#getUniqueId
    pub fn get_unique_id(&mut self, input: &CompilerInput) -> String {
        let file_path = JsString::from(input.get_source_file().get_name());
        let file_hash_code = file_path.hash_code();
        let id = self.counter_add(file_hash_code, 1);
        let file_hash_string = if file_hash_code < 0 {
            format!("m{}", file_hash_code.wrapping_neg())
        } else {
            format!("{file_hash_code}")
        };
        format!("{file_hash_string}${id}")
    }

    // port: UniqueIdSupplier#toProto
    pub fn to_proto(&self) -> Vec<UniqueIdProto> {
        let mut result = Vec::new();
        for (&element, &count) in &self.counter {
            result.push(UniqueIdProto {
                hash: element,
                counter: count,
            });
        }
        result
    }

    // port: UniqueIdSupplier#fromProto
    pub fn from_proto(protos: &[UniqueIdProto]) -> UniqueIdSupplier {
        let mut supplier = UniqueIdSupplier::new();
        for p in protos {
            supplier.counter_add(p.get_hash(), p.get_counter());
        }
        supplier
    }
}
