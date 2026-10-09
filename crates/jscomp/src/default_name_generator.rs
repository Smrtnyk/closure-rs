/*
 * Copyright 2005 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DefaultNameGenerator.java.

use crate::name_generator::{NameGenerator, ReservedNames};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, token_stream::TokenStream};
use std::sync::{Arc, RwLock};

pub const FIRST_CHAR: &[u16] =
    &ascii_units(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ$");
pub const NONFIRST_CHAR: &[u16] =
    &ascii_units(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_0123456789$");
pub const BAD_NAMES: &[&str] = &["let", "yield", "await", "eval"];

// port: DefaultNameGenerator#FIRST_CHAR / NONFIRST_CHAR (String#toCharArray)
const fn ascii_units<const N: usize>(s: &[u8; N]) -> [u16; N] {
    let mut result = [0; N];
    let mut i = 0;
    while i < N {
        result[i] = s[i] as u16;
        i += 1;
    }
    result
}

#[derive(Debug)]
pub struct CharPriority {
    pub name: u16,
    pub occurrence: i32,
    pub order: i32,
}

impl CharPriority {
    // port: DefaultNameGenerator.CharPriority#CharPriority
    pub fn new(name: u16, order: i32) -> Self {
        Self {
            name,
            order,
            occurrence: 0,
        }
    }
    // port: DefaultNameGenerator.CharPriority#compareTo
    pub fn compare_to(&self, other: &Self) -> i32 {
        let result = other.occurrence.wrapping_sub(self.occurrence);
        if result != 0 {
            return result;
        }
        self.order.wrapping_sub(other.order)
    }
}
impl Clone for CharPriority {
    // port: DefaultNameGenerator.CharPriority#clone
    fn clone(&self) -> Self {
        let mut result = Self::new(self.name, self.order);
        result.occurrence = self.occurrence;
        result
    }
}

pub struct DefaultNameGenerator {
    priority_lookup_map: IndexMap<u16, CharPriority>,
    reserved_names: ReservedNames,
    prefix: JsString,
    name_count: i32,
    // Indices reference the objects owned by priority_lookup_map, including after favors().
    first_chars: Vec<usize>,
    non_first_chars: Vec<usize>,
}

impl DefaultNameGenerator {
    // port: DefaultNameGenerator#clone(Set,String,Set)
    pub fn clone_generator(
        &self,
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_characters: &IndexSet<u16>,
    ) -> Self {
        Self::with_priority_lookup_map(
            reserved_names,
            prefix,
            reserved_characters,
            &self.priority_lookup_map,
        )
    }
    // port: DefaultNameGenerator#DefaultNameGenerator()
    pub fn new() -> Self {
        let reserved_names = Arc::new(RwLock::new(IndexSet::<_>::default()));
        let mut result = Self {
            priority_lookup_map: IndexMap::<_, _>::default(),
            reserved_names: reserved_names.clone(),
            prefix: JsString::from(""),
            name_count: 0,
            first_chars: Vec::new(),
            non_first_chars: Vec::new(),
        };
        result.build_priority_lookup_map();
        result.reset(
            reserved_names,
            JsString::from(""),
            &IndexSet::<_>::default(),
        );
        result
    }
    // port: DefaultNameGenerator#DefaultNameGenerator(Set,String,Set)
    pub fn with_reserved_characters(
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_characters: &IndexSet<u16>,
    ) -> Self {
        Self::with_first_and_non_first_characters(
            reserved_names,
            prefix,
            reserved_characters,
            reserved_characters,
        )
    }
    // port: DefaultNameGenerator#DefaultNameGenerator(Set,String,Set,Set)
    pub fn with_first_and_non_first_characters(
        reserved_names: ReservedNames,
        prefix: JsString,
        first: &IndexSet<u16>,
        non_first: &IndexSet<u16>,
    ) -> Self {
        let mut result = Self {
            priority_lookup_map: IndexMap::<_, _>::default(),
            reserved_names: reserved_names.clone(),
            prefix: prefix.clone(),
            name_count: 0,
            first_chars: Vec::new(),
            non_first_chars: Vec::new(),
        };
        result.build_priority_lookup_map();
        result.reset_with_first_and_non_first_characters(reserved_names, prefix, first, non_first);
        result
    }
    // port: DefaultNameGenerator#DefaultNameGenerator(Set,String,Set,Map)
    fn with_priority_lookup_map(
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_characters: &IndexSet<u16>,
        priority_lookup_map: &IndexMap<u16, CharPriority>,
    ) -> Self {
        let mut result = Self {
            priority_lookup_map: IndexMap::with_capacity_and_hasher(
                NONFIRST_CHAR.len(),
                Default::default(),
            ),
            reserved_names: reserved_names.clone(),
            prefix: prefix.clone(),
            name_count: 0,
            first_chars: Vec::new(),
            non_first_chars: Vec::new(),
        };
        for (key, value) in priority_lookup_map {
            result.priority_lookup_map.insert(*key, value.clone());
        }
        result.reset_with_first_and_non_first_characters(
            reserved_names,
            prefix,
            reserved_characters,
            reserved_characters,
        );
        result
    }
    // port: DefaultNameGenerator#buildPriorityLookupMap
    fn build_priority_lookup_map(&mut self) {
        self.priority_lookup_map =
            IndexMap::with_capacity_and_hasher(NONFIRST_CHAR.len(), Default::default());
        let mut order: i32 = 0;
        for &c in NONFIRST_CHAR {
            self.priority_lookup_map
                .insert(c, CharPriority::new(c, order));
            order = order.wrapping_add(1);
        }
    }
    // port: DefaultNameGenerator#isBadName
    pub fn is_bad_name(name: &JsString) -> bool {
        BAD_NAMES.iter().any(|s| name == s)
    }
    // port: DefaultNameGenerator#favors
    pub fn favors(&mut self, sequence: &JsString) {
        let mut i: i32 = 0;
        while i < sequence.length() as i32 {
            if let Some(c) = self
                .priority_lookup_map
                .get_mut(&sequence.char_at(i as usize))
            {
                c.occurrence = c.occurrence.wrapping_add(1);
            }
            i = i.wrapping_add(1);
        }
    }
    // port: DefaultNameGenerator#reserveCharacters
    pub fn reserve_characters(
        &self,
        chars: &[u16],
        reserved_character_set: &IndexSet<u16>,
    ) -> Vec<usize> {
        if reserved_character_set.is_empty() {
            return chars
                .iter()
                .map(|c| self.priority_lookup_map.get_index_of(c).unwrap())
                .collect();
        }
        let mut char_set: IndexSet<u16> = chars.iter().copied().collect();
        char_set.retain(|c| !reserved_character_set.contains(c));
        char_set
            .iter()
            .map(|c| self.priority_lookup_map.get_index_of(c).unwrap())
            .collect()
    }
    // port: DefaultNameGenerator#checkPrefix
    fn check_prefix(&self, prefix: &JsString) {
        if !prefix.is_empty() {
            if !self.contains(&self.first_chars, prefix.char_at(0)) {
                panic!(
                    "prefix must start with one of: {}",
                    self.chars_to_string(&self.first_chars)
                );
            }
            let mut pos: i32 = 1;
            while pos < prefix.length() as i32 {
                let chars = self.chars_to_string(&self.non_first_chars);
                if !self.contains(&self.non_first_chars, prefix.char_at(pos as usize)) {
                    panic!("prefix has invalid characters, must be one of: {chars}");
                }
                pos = pos.wrapping_add(1);
            }
        }
    }
    // port: DefaultNameGenerator#checkPrefix (Arrays#toString(char[]))
    fn chars_to_string(&self, indices: &[usize]) -> String {
        format!(
            "[{}]",
            indices
                .iter()
                .map(|&i| char::from_u32(self.priority_lookup_map[i].name as u32)
                    .unwrap()
                    .to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
    // port: DefaultNameGenerator#contains
    fn contains(&self, arr: &[usize], c: u16) -> bool {
        arr.iter().any(|&i| self.priority_lookup_map[i].name == c)
    }
}

impl Default for DefaultNameGenerator {
    // port: DefaultNameGenerator#DefaultNameGenerator()
    fn default() -> Self {
        Self::new()
    }
}

impl NameGenerator for DefaultNameGenerator {
    // port: DefaultNameGenerator#reset(Set,String,Set)
    fn reset(
        &mut self,
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_characters: &IndexSet<u16>,
    ) {
        self.reset_with_first_and_non_first_characters(
            reserved_names,
            prefix,
            reserved_characters,
            reserved_characters,
        );
    }
    // port: DefaultNameGenerator#reset(Set,String,Set,Set)
    fn reset_with_first_and_non_first_characters(
        &mut self,
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_first_characters: &IndexSet<u16>,
        reserved_non_first_characters: &IndexSet<u16>,
    ) {
        self.reserved_names = reserved_names;
        self.prefix = prefix;
        self.name_count = 0;
        self.first_chars = self.reserve_characters(FIRST_CHAR, reserved_first_characters);
        self.non_first_chars =
            self.reserve_characters(NONFIRST_CHAR, reserved_non_first_characters);
        let priorities = &self.priority_lookup_map;
        self.first_chars
            .sort_by(|&a, &b| priorities[a].compare_to(&priorities[b]).cmp(&0));
        self.non_first_chars
            .sort_by(|&a, &b| priorities[a].compare_to(&priorities[b]).cmp(&0));
        self.check_prefix(&self.prefix);
    }
    // port: DefaultNameGenerator#clone(Set,String,Set)
    fn clone(
        &self,
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_characters: &IndexSet<u16>,
    ) -> Box<dyn NameGenerator> {
        Box::new(self.clone_generator(reserved_names, prefix, reserved_characters))
    }
    // port: DefaultNameGenerator#generateNextName
    fn generate_next_name(&mut self) -> JsString {
        loop {
            let mut sb = self.prefix.as_units().to_vec();
            let mut i: i32 = self.name_count;
            if sb.is_empty() {
                let pos: i32 = i % self.first_chars.len() as i32;
                sb.push(self.priority_lookup_map[self.first_chars[pos as usize]].name);
                i /= self.first_chars.len() as i32;
            }
            while i > 0 {
                i = i.wrapping_sub(1);
                let pos: i32 = i % self.non_first_chars.len() as i32;
                sb.push(self.priority_lookup_map[self.non_first_chars[pos as usize]].name);
                i /= self.non_first_chars.len() as i32;
            }
            self.name_count = self.name_count.wrapping_add(1);
            let name = JsString::from_units(sb);
            if !TokenStream::is_keyword(&name)
                && !self.reserved_names.read().unwrap().contains(&name)
                && !Self::is_bad_name(&name)
            {
                return name;
            }
        }
    }
}

// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
macro_rules! replay_default_name_generator_fields {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct DefaultNameGeneratorReplayFields<'a> { $(pub $name: &'a $ty,)* }
        pub struct DefaultNameGeneratorReplayFieldsMut<'a> { $(pub $name: &'a mut $ty,)* }
        impl DefaultNameGenerator {
            // port: java.lang.reflect.Field#get (native replay access)
            pub fn replay_fields(&self) -> DefaultNameGeneratorReplayFields<'_> {
                DefaultNameGeneratorReplayFields { $($name: &self.$name,)* }
            }
            // port: java.lang.reflect.Field#set (native replay access)
            pub fn replay_fields_mut(&mut self) -> DefaultNameGeneratorReplayFieldsMut<'_> {
                DefaultNameGeneratorReplayFieldsMut { $($name: &mut self.$name,)* }
            }
        }
    };
}
replay_default_name_generator_fields!(
    priority_lookup_map: IndexMap<u16, CharPriority>,
    reserved_names: ReservedNames,
    prefix: JsString,
    name_count: i32,
    // Indices reference the objects owned by priority_lookup_map, including after favors().
    first_chars: Vec<usize>,
    non_first_chars: Vec<usize>,
);
