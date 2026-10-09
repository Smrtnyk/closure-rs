/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/resources/ResourceLoader.java.

//! `ResourceLoader` (and the JDK class-path lookup under it) against Java's results.

mod common;

use closure_resources::jar;
use closure_resources::resources::resource_loader::ResourceLoader;
use common::{fixture_rows, panic_message, sha256_hex, unescape};

// port: ResourceLoader#loadTextResource (fixture load_text_resource.tsv)
#[test]
fn load_text_resource_matches_java() {
    let rows = fixture_rows("load_text_resource.tsv");
    assert_eq!(rows.len(), 8);
    for row in rows {
        let text = ResourceLoader::load_text_resource(&row[0], &row[1]);
        assert_eq!(text.len().to_string(), row[2], "{} {}", row[0], row[1]);
        assert_eq!(sha256_hex(text.as_bytes()), row[3], "{} {}", row[0], row[1]);
    }
}

// port: ResourceLoader#resourceExists (fixture resource_exists.tsv)
#[test]
fn resource_exists_matches_java() {
    let rows = fixture_rows("resource_exists.tsv");
    assert_eq!(rows.len(), 12);
    for row in rows {
        assert_eq!(
            ResourceLoader::resource_exists(&row[0], &row[1]).to_string(),
            row[2],
            "{} {}",
            row[0],
            row[1]
        );
    }
}

// port: ResourceLoader#loadTextResource (missing resource, fixture load_missing.tsv)
#[test]
fn load_text_resource_missing() {
    let row = &fixture_rows("load_missing.tsv")[0];
    assert_eq!(row[0], "java.lang.RuntimeException");
    let message = panic_message(|| {
        ResourceLoader::load_text_resource(
            "com.google.javascript.jscomp.Compiler",
            "js/nonexistent.js",
        )
    });
    assert_eq!(message, row[1]);
}

// port: ResourceLoader#loadPropertiesMap (fixture load_properties_map.tsv)
#[test]
fn load_properties_map_matches_java() {
    let rows = fixture_rows("load_properties_map.tsv");
    assert_eq!(rows.len(), 3);
    for row in rows {
        if let Some(exception) = row[2].strip_prefix("EXC:") {
            let message = exception
                .strip_prefix("java.lang.IllegalArgumentException:")
                .unwrap();
            let (clazz, path) = (row[0].clone(), row[1].clone());
            let actual = panic_message(move || ResourceLoader::load_properties_map(&clazz, &path));
            assert_eq!(actual, unescape(message), "{} {}", row[0], row[1]);
        } else {
            let map = ResourceLoader::load_properties_map(&row[0], &row[1]);
            let mut joined = String::new();
            for (k, v) in &map {
                joined.push_str(&format!("{k}=>{v}\n"));
            }
            assert_eq!(map.len().to_string(), row[2], "{} {}", row[0], row[1]);
            assert_eq!(
                sha256_hex(joined.as_bytes()),
                row[3],
                "{} {}",
                row[0],
                row[1]
            );
        }
    }
}

// port: Compiler#loadResourceContents (ResourceLoader.loadTextResource(Compiler.class, "js/" + resourceName + ".js"))
#[test]
fn runtime_library_text_through_compiler_class() {
    let text = ResourceLoader::load_text_resource(
        "com.google.javascript.jscomp.Compiler",
        "js/es6/set.js",
    );
    assert!(text.contains("$jscomp.polyfill('Set'"), "{}", &text[..200]);
}

// port: Compiler#initRuntimeLibraryTypedAsts (Compiler.class.getResourceAsStream("/runtime_libs.typedast"))
#[test]
fn runtime_libs_typedast_through_compiler_class() {
    let stream = jar::get_resource_as_stream(
        "com.google.javascript.jscomp.Compiler",
        "/runtime_libs.typedast",
    )
    .unwrap();
    let bytes = stream.read_all_bytes();
    assert_eq!(bytes.len(), 3_516_009);
    assert_eq!(
        sha256_hex(bytes),
        "5b49763aae9aa8851782ddc3520493c2a33ef2d40a26cc6227c8787b25103bd9"
    );
}
