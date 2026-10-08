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

//! Every descriptor of corpus/unit/descriptors loads with zero errors and round-trips; the
//! descriptor stems are exactly the non-empty record files; options_defaults.json and
//! derived/expected_pipeline.jsonl.gz load and round-trip.

use closure_testing::corpus::{
    descriptor_files, file_stem, load_descriptor, load_expected_pipeline, load_options_defaults,
    read_gz, record_files,
};
use closure_testing::dsl::Expr;
use indexmap::{IndexMap, IndexSet};

#[test]
fn all_descriptors_load_and_round_trip() {
    let files = descriptor_files().unwrap();
    let mut errors = Vec::new();
    let mut cases = 0usize;
    let mut tags: IndexMap<&'static str, usize> = IndexMap::new();
    let mut keys: IndexMap<&'static str, usize> = IndexMap::new();
    let mut stems = IndexSet::new();
    for f in &files {
        stems.insert(file_stem(f));
        match load_descriptor(f) {
            Err(e) => errors.push(e.to_string()),
            Ok(ld) => {
                if ld.descriptor.to_json() != ld.raw {
                    errors.push(format!("{}: round trip differs", f.display()));
                }
                let d = &ld.descriptor;
                assert!(!d.source.is_empty(), "{}: source", f.display());
                cases += d.cases.len();
                let mut count = |e: &Expr| {
                    if let Some(t) = e.tag() {
                        *tags.entry(t).or_default() += 1;
                    }
                };
                for c in &d.cases {
                    let mut key = |k: &'static str, present: bool| {
                        if present {
                            *keys.entry(k).or_default() += 1;
                        }
                    };
                    key("when", c.when.is_some());
                    key("processor", c.processor.is_some());
                    key("compilerSetup", c.compiler_setup.is_some());
                    key("options", c.options.is_some());
                    key("postconditions", c.postconditions.is_some());
                    key("testFieldsAfter", c.test_fields_after.is_some());
                    key("testFieldsAfterAlso", c.test_fields_after_also.is_some());
                    key("testFieldsAfterSkip", c.test_fields_after_skip.is_some());
                    key(
                        "testFieldsAfterUnchecked",
                        c.test_fields_after_unchecked.is_some(),
                    );
                    if let Some(skip) = &c.test_fields_after_skip {
                        for (name, reason) in skip {
                            assert!(!reason.is_empty(), "{}: skip reason of {name}", f.display());
                        }
                    }
                    let exprs = c
                        .processor
                        .iter()
                        .chain(c.test_fields_after.iter())
                        .chain(c.compiler_setup.iter().flatten())
                        .chain(c.postconditions.iter().flatten())
                        .chain(c.options.iter().flat_map(|o| o.then.iter().flatten()));
                    for e in exprs {
                        e.visit(&mut count);
                    }
                }
            }
        }
    }
    for e in errors.iter().take(20) {
        eprintln!("{e}");
    }
    assert!(errors.is_empty(), "{} descriptors failed", errors.len());
    println!("descriptors: {}, cases: {cases}", files.len());
    println!("case keys: {keys:?}");
    println!("expression tags: {tags:?}");
    assert_eq!(files.len(), 255);
    assert_eq!(cases, 374);
    assert_eq!(keys["processor"], 370);
    assert_eq!(keys["testFieldsAfter"], 64);
    assert_eq!(keys["postconditions"], 17);

    // One descriptor per non-empty record file, and no other.
    let nonempty: IndexSet<String> = record_files()
        .unwrap()
        .iter()
        .filter(|f| !read_gz(f).unwrap().trim().is_empty())
        .map(|f| file_stem(f))
        .collect();
    assert_eq!(
        stems, nonempty,
        "descriptor stems vs non-empty record files"
    );
}

#[test]
fn options_defaults_load_and_round_trip() {
    let (d, raw) = load_options_defaults().unwrap();
    assert_eq!(d.to_json(), raw);
    assert_eq!(d.v, 2);
    assert_eq!(d.class, "com.google.javascript.jscomp.CompilerOptions");
    assert_eq!(d.depth, 4);
    assert_eq!(d.fields.len(), 205);
    assert_eq!(d.field_types.len(), 205);
    assert!(
        d.fields.keys().eq(d.field_types.keys()),
        "fields vs fieldTypes"
    );
    assert!(d.unrepresentable.is_empty());
}

#[test]
fn expected_pipeline_loads_and_round_trips() {
    let entries = load_expected_pipeline().unwrap();
    assert_eq!(entries.len(), 21_297);
    for (e, raw) in &entries {
        assert_eq!(&e.to_json(), raw);
    }
    let normalize = entries.iter().filter(|(e, _)| e.normalize_expected).count();
    println!(
        "expected_pipeline: {} entries, normalizeExpected: {normalize}",
        entries.len()
    );
}
