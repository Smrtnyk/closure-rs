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

use closure_jscomp::{
    default_name_generator::{CharPriority, DefaultNameGenerator, FIRST_CHAR, NONFIRST_CHAR},
    name_generator::{NameGenerator, ReservedNames},
    variable_map::{FromStreamError, VariableMap},
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{java_lang::parse_exception::ParseException, js_string::JsString};
use std::{
    io::{self, Read},
    sync::{Arc, RwLock},
};

#[test]
fn clone_trait_preserves_priorities_and_live_reservations() {
    let reserved: ReservedNames = Arc::new(RwLock::new(IndexSet::<_>::default()));
    let mut original = DefaultNameGenerator::new();
    original.favors(&"zzz".into());
    let mut cloned: Box<dyn NameGenerator> = NameGenerator::clone(
        &original,
        reserved.clone(),
        "".into(),
        &IndexSet::<_>::default(),
    );
    original.favors(&"aaaa".into());
    original.reset(reserved.clone(), "".into(), &IndexSet::<_>::default());
    assert_eq!(original.generate_next_name(), "a");
    assert_eq!(cloned.generate_next_name(), "z");
    reserved.write().unwrap().insert("a".into());
    assert_eq!(cloned.generate_next_name(), "b");
    reserved.write().unwrap().insert("c".into());
    assert_eq!(original.generate_next_name(), "z");
    assert_eq!(original.generate_next_name(), "b");
    assert_eq!(cloned.generate_next_name(), "d");
}

#[test]
fn priorities_wrap_like_java_and_reservations_retain_order() {
    let mut a = CharPriority::new(b'a' as u16, 0);
    let mut b = CharPriority::new(b'b' as u16, 1);
    a.occurrence = i32::MAX;
    b.occurrence = -1;
    assert_eq!(a.compare_to(&b), i32::MIN);
    b.occurrence = i32::MAX;
    assert_eq!(a.compare_to(&b), -1);
    let ng = DefaultNameGenerator::new();
    let reserved = IndexSet::<_>::from_iter([b'c' as u16, b'a' as u16, 0xffff]);
    let indices = ng.reserve_characters(NONFIRST_CHAR, &reserved);
    assert_eq!(
        indices
            .iter()
            .map(|&i| NONFIRST_CHAR[i])
            .collect::<Vec<_>>(),
        NONFIRST_CHAR
            .iter()
            .copied()
            .filter(|c| !reserved.contains(c))
            .collect::<Vec<_>>()
    );
    assert_eq!(FIRST_CHAR.len(), 53);
}

struct BrokenStream;
impl Read for BrokenStream {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("read failed"))
    }
}
#[test]
fn stream_io_and_parse_errors_remain_distinct() {
    let err = VariableMap::from_stream(BrokenStream).unwrap_err();
    assert!(matches!(err,FromStreamError::Io(ref e) if e.to_string() == "read failed"));
    let err = VariableMap::from_stream(&b"bad"[..]).unwrap_err();
    assert!(
        matches!(err,FromStreamError::Parse(ref e) if e.get_message() == "Bad line: bad" && e.get_error_offset() == 0)
    );
    assert_eq!(err.to_string(), "java.text.ParseException: Bad line: bad");
}

#[test]
fn file_round_trip_and_checked_load_error() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/namegen-file-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{}.map", std::process::id()));
    let filename = file.to_str().unwrap();
    let mut map = IndexMap::<_, _>::from_iter([
        (JsString::from("z"), JsString::from("last")),
        (JsString::from("a"), JsString::from("first")),
    ]);
    let vm = VariableMap::from_map(&map);
    map.insert("a".into(), "changed".into());
    assert_eq!(vm.lookup_new_name(&"a".into()), Some("first".into()));
    assert_eq!(vm.lookup_new_name(&"missing".into()), None);
    assert_eq!(vm.lookup_source_name(&"missing".into()), None);
    vm.save(filename).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"a:first\nz:last\n");
    let loaded = VariableMap::load(filename).unwrap();
    assert_eq!(
        loaded.get_original_name_to_new_name_map(),
        vm.get_original_name_to_new_name_map()
    );
    std::fs::write(&file, b"bad").unwrap();
    let err = VariableMap::load(filename).unwrap_err();
    assert_eq!(err.to_string(), "java.text.ParseException: Bad line: bad");
    assert!(
        err.get_ref()
            .unwrap()
            .downcast_ref::<ParseException>()
            .is_some()
    );
    std::fs::remove_file(&file).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    assert!(VariableMap::load(filename).is_err());
}
