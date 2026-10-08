/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/SourceMapResolverTest.java.

//! Port of SourceMapResolverTest.java.
use base64::{Engine, engine::general_purpose::STANDARD};
use closure_jscomp::{source_file::SourceFile, source_map_resolver::SourceMapResolver};
use closure_rhino::static_source_file::StaticSourceFile;

#[test]
// port: SourceMapResolverTest#testResolveBase64Inline
fn test_resolve_base64_inline() {
    let source_map = "{map: 'asdfasdf'}";
    let encoded = STANDARD.encode(source_map.as_bytes());
    let url = format!("data:application/json;base64,{encoded}");
    let code = format!("console.log('asdf')\n//# sourceMappingURL={url}");
    let s = SourceMapResolver::extract_source_map(
        &SourceFile::from_code("somePath/hello.js", code.as_str()),
        &url,
        true,
    )
    .unwrap();
    assert_eq!(s.get_code().unwrap().to_string_lossy(), source_map);
    assert_eq!(s.get_name(), "somePath/hello.js.inline.map");

    // --parse_inline_source_maps=false
    let no_inline = SourceMapResolver::extract_source_map(
        &SourceFile::from_code("somePath/hello.js", code.as_str()),
        &url,
        false,
    );
    assert!(no_inline.is_none());
}

#[test]
// port: SourceMapResolverTest#testResolveBase64WithCharsetInline
fn test_resolve_base64_with_charset_inline() {
    let source_map = "{map: 'asdfasdf'}";
    let encoded = STANDARD.encode(source_map.as_bytes());
    let url = format!("data:application/json;charset=utf-8;base64,{encoded}");
    let code = format!("console.log('asdf')\n//# sourceMappingURL={url}");
    let s = SourceMapResolver::extract_source_map(
        &SourceFile::from_code("somePath/hello.js", code.as_str()),
        &url,
        true,
    )
    .unwrap();
    assert_eq!(s.get_code().unwrap().to_string_lossy(), source_map);
    assert_eq!(s.get_name(), "somePath/hello.js.inline.map");

    // Try non supported charset.
    let data_url_with_bad_charset = format!("data:application/json;charset=asdf;base64,{encoded}");
    let charset_code =
        format!("console.log('asdf')\n//# sourceMappingURL={data_url_with_bad_charset}");
    let result = SourceMapResolver::extract_source_map(
        &SourceFile::from_code("somePath/hello.js", charset_code.as_str()),
        &data_url_with_bad_charset,
        true,
    );
    assert!(result.is_none());
}

#[test]
// port: SourceMapResolverTest#testAbsolute
fn test_absolute() {
    let js_file = SourceFile::from_code("somePath/hello.js", "console.log(1)");
    // We cannot reslove absolute urls.
    assert!(SourceMapResolver::extract_source_map(&js_file, "/asdf/asdf.js", true).is_none());
    assert!(SourceMapResolver::extract_source_map(&js_file, "/asdf/.././asdf.js", true).is_none());
    assert!(
        SourceMapResolver::extract_source_map(&js_file, "http://google.com/asdf/asdf.js", true)
            .is_none()
    );
    assert!(
        SourceMapResolver::extract_source_map(&js_file, "https://google.com/asdf/asdf.js", true)
            .is_none()
    );

    // We can resolve relative urls
    assert!(SourceMapResolver::extract_source_map(&js_file, "asdf.js", true).is_some());
    assert!(SourceMapResolver::extract_source_map(&js_file, "asdf/asdf.js", true).is_some());
    assert!(SourceMapResolver::extract_source_map(&js_file, "asdf/.././asdf.js", true).is_some());
    assert!(
        SourceMapResolver::extract_source_map(&js_file, "not/.././a/js/file.txt", true).is_some()
    );
}

#[test]
// port: SourceMapResolverTest#testRelativePaths
fn test_relative_paths() {
    assert_eq!(
        SourceMapResolver::get_relative_path("basefile.js", "basefile.js.map").get_name(),
        "basefile.js.map"
    );
    assert_eq!(
        SourceMapResolver::get_relative_path("path/basefile.js", "relative/path/basefile.js.map")
            .get_name(),
        "path/relative/path/basefile.js.map"
    );
    assert_eq!(
        SourceMapResolver::get_relative_path("some/longer/path/basefile.js", "../sourcemap.js.map")
            .to_string(),
        "some/longer/sourcemap.js.map"
    );
    assert_eq!(
        SourceMapResolver::get_relative_path(
            "some/longer/path/basefile.js",
            ".././../sourcemap.js.map"
        )
        .get_name(),
        "some/sourcemap.js.map"
    );
    assert_eq!(
        SourceMapResolver::get_relative_path("basefile.js", "../basefile.js.map").get_name(),
        "../basefile.js.map"
    );
    assert_eq!(
        SourceMapResolver::get_relative_path("baz/bam/qux.js", "../foo/bar.js").get_name(),
        "baz/foo/bar.js"
    );
}

#[test]
// port: SourceMapResolverTest#testIntegration
fn test_integration() {
    let url = "relative/path/to/sourcemap/hello.js.map";
    let s = SourceMapResolver::extract_source_map(
        &SourceFile::from_code("somePath/hello.js", ""),
        url,
        false,
    )
    .unwrap();
    assert_eq!(
        s.get_name(),
        "somePath/relative/path/to/sourcemap/hello.js.map"
    );
}

#[test]
// port: SourceMapResolverTest#testAddBase64PrefixToEncodedSourceMap
fn test_add_base64_prefix_to_encoded_source_map() {
    let base64_prefix = "data:application/json;base64,";
    let encoded_source_map = "eyJ2ZXJzaW9uIjozLCJmaWxlIjoiZm9vLmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsiZm9vLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBO0lBR0UsV0FBWSxLQUFhO1FBQ3ZCLElBQUksQ0FBQyxDQUFDLEdBQUcsS0FBSyxDQUFDO0lBQ2pCLENBQUM7SUFDSCxRQUFDO0FBQUQsQ0FBQyxBQU5ELElBTUM7QUFFRCxPQUFPLENBQUMsR0FBRyxDQUFDLElBQUksQ0FBQyxDQUFDLENBQUMsQ0FBQyxDQUFDLENBQUMifQ==";
    let base64_encoded_source_map =
        SourceMapResolver::add_base64_prefix_to_encoded_source_map(encoded_source_map);
    assert_eq!(
        base64_encoded_source_map,
        format!("{base64_prefix}{encoded_source_map}")
    );
}
