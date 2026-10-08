/*
 * Copyright (C) 2008 The Guava Authors
 * Copyright (C) 2018 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * Copyright (C) 2009 The Guava Authors
 * Copyright (C) 2011 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
 * in compliance with the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License
 * is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
 * or implied. See the License for the specific language governing permissions and limitations under
 * the License.
 */
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/base/Splitter.java, com/google/common/collect/ImmutableMap.java,
//   com/google/common/collect/JdkBackedImmutableMap.java,
//   com/google/common/collect/RegularImmutableMap.java, com/google/common/hash/Hashing.java.

//! The Guava behaviour the ported code relies on (`ImmutableMap.Builder#buildOrThrow`,
//! `Splitter`), with Guava's exact results and failure messages.

use indexmap::IndexMap;

/// `ImmutableMap.Builder<String, String>`: entries in insertion order; `buildOrThrow` fails on a
/// duplicate key with the message Guava 33.4.6 (the version in the reference jar) produces.
#[derive(Debug, Default)]
pub struct ImmutableMapBuilder {
    entries: Vec<(String, String)>,
}

// port: RegularImmutableMap#MAX_LOAD_FACTOR
const MAX_LOAD_FACTOR: f64 = 1.2;
// port: RegularImmutableMap#MAX_HASH_BUCKET_LENGTH
const MAX_HASH_BUCKET_LENGTH: i32 = 8;
// port: Hashing#MAX_TABLE_SIZE
const MAX_TABLE_SIZE: i32 = 1 << 30;
// port: Hashing#C1
const C1: i32 = 0xcc9e2d51_u32 as i32;
// port: Hashing#C2
const C2: i32 = 0x1b873593;

impl ImmutableMapBuilder {
    // port: ImmutableMap#builder
    pub fn new() -> Self {
        Self::default()
    }

    // port: ImmutableMap.Builder#put
    pub fn put(&mut self, key: String, value: String) -> &mut Self {
        self.entries.push((key, value));
        self
    }

    /// Panics like Guava's `IllegalArgumentException` on a duplicate key. Guava finds the
    /// duplicate while building the hash table from the LAST entry backwards, so the message
    /// names the later entry first: `Multiple entries with same key: k=later and k=earlier`; if a
    /// hash bucket overflows first, it rebuilds forwards (`JdkBackedImmutableMap`).
    // port: ImmutableMap.Builder#buildOrThrow
    pub fn build_or_throw(self) -> IndexMap<String, String> {
        // port: ImmutableMap.Builder#build(boolean)
        match self.entries.len() {
            0 | 1 => self.entries.into_iter().collect(),
            n => match Self::from_entry_array_checking_bucket_overflow(n, &self.entries) {
                Ok(()) => self.entries.into_iter().collect(),
                Err(BucketOverflowException) => Self::jdk_backed_create(self.entries),
            },
        }
    }

    /// The duplicate check of the hash table Guava builds (the table itself is not needed: the
    /// map iterates in insertion order either way).
    // port: RegularImmutableMap#fromEntryArrayCheckingBucketOverflow
    fn from_entry_array_checking_bucket_overflow(
        n: usize,
        entries: &[(String, String)],
    ) -> Result<(), BucketOverflowException> {
        let table_size = closed_table_size(n as i32, MAX_LOAD_FACTOR);
        let mut table: Vec<Option<usize>> = vec![None; table_size as usize];
        let mut next_in_key_bucket: Vec<Option<usize>> = vec![None; n];
        let mask = table_size - 1;
        for entry_index in (0..n).rev() {
            let (key, value) = &entries[entry_index];
            let table_index = (smear(java_string_hash_code(key)) & mask) as usize;
            let key_bucket_head = table[table_index];
            Self::check_no_conflict_in_key_bucket(
                key,
                value,
                key_bucket_head,
                entries,
                &next_in_key_bucket,
            )?;
            next_in_key_bucket[entry_index] = key_bucket_head;
            table[table_index] = Some(entry_index);
        }
        Ok(())
    }

    // port: RegularImmutableMap#checkNoConflictInKeyBucket (throwIfDuplicateKeys = true)
    fn check_no_conflict_in_key_bucket(
        key: &str,
        new_value: &str,
        mut key_bucket_head: Option<usize>,
        entries: &[(String, String)],
        next_in_key_bucket: &[Option<usize>],
    ) -> Result<(), BucketOverflowException> {
        let mut bucket_size = 0;
        while let Some(head) = key_bucket_head {
            let (head_key, head_value) = &entries[head];
            if head_key == key {
                // port: ImmutableMap#checkNoConflict / ImmutableMap#conflictException
                panic!(
                    "Multiple entries with same key: {head_key}={head_value} and {key}={new_value}"
                );
            }
            bucket_size += 1;
            if bucket_size > MAX_HASH_BUCKET_LENGTH {
                return Err(BucketOverflowException);
            }
            key_bucket_head = next_in_key_bucket[head];
        }
        Ok(())
    }

    // port: JdkBackedImmutableMap#create (throwIfDuplicateKeys = true)
    fn jdk_backed_create(entries: Vec<(String, String)>) -> IndexMap<String, String> {
        let mut delegate_map: IndexMap<String, String> = IndexMap::with_capacity(entries.len());
        for (key, value) in entries {
            if let Some(old_value) = delegate_map.get(&key) {
                // port: ImmutableMap#conflictException
                panic!("Multiple entries with same key: {key}={value} and {key}={old_value}");
            }
            delegate_map.insert(key, value);
        }
        delegate_map
    }
}

// port: RegularImmutableMap.BucketOverflowException
#[derive(Debug)]
struct BucketOverflowException;

// port: Hashing#smear
fn smear(hash_code: i32) -> i32 {
    C2.wrapping_mul(hash_code.wrapping_mul(C1).rotate_left(15))
}

// port: Hashing#closedTableSize
fn closed_table_size(expected_entries: i32, load_factor: f64) -> i32 {
    // Get the recommended table size.
    // Round down to the nearest power of 2.
    let expected_entries = expected_entries.max(2);
    let mut table_size = 1_i32 << (31 - expected_entries.leading_zeros()); // Integer#highestOneBit
    // Check to make sure that we will not exceed the maximum load factor.
    if expected_entries > (load_factor * table_size as f64) as i32 {
        table_size <<= 1;
        return if table_size > 0 {
            table_size
        } else {
            MAX_TABLE_SIZE
        };
    }
    table_size
}

/// Java's `String#hashCode` over UTF-16 code units.
// port: String#hashCode
fn java_string_hash_code(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0_i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

/// `Splitter.on(separator).omitEmptyStrings().split(sequence)`.
// port: Splitter#on(char).omitEmptyStrings().split(CharSequence)
pub fn split_omit_empty_strings(sequence: &str, separator: char) -> Vec<&str> {
    sequence
        .split(separator)
        .filter(|s| !s.is_empty())
        .collect()
}

/// `Splitter.on(separator).limit(limit).splitToList(sequence)`: at most `limit` parts, the last
/// part holds the rest of the input including further separators.
// port: Splitter#on(char).limit(int).splitToList(CharSequence)
pub fn split_limit_to_list(sequence: &str, separator: char, limit: usize) -> Vec<&str> {
    sequence.splitn(limit, separator).collect()
}
