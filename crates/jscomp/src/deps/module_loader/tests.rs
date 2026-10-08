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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/deps/ModuleLoader.java,
//   test/com/google/javascript/jscomp/deps/ModuleLoaderTest.java.

use super::*;
use crate::deps::{
    browser_with_transformed_prefixes_module_resolver as prefixes,
    module_resolver::ModuleResolverBase, node_module_resolver as node,
    simple_dependency_info::SimpleDependencyInfo, webpack_module_resolver as webpack,
};
use crate::source_file::SourceFile;
use closure_rhino::static_source_file::StaticSourceFile;
// port: ModuleLoaderTest#PACKAGE_JSON_MAIN_ENTRIES
fn package_json_main_entries() -> IndexMap<String, String> {
    IndexMap::from([
        ("/B/package.json".into(), "/B/lib/b".into()),
        (
            "/node_modules/B/package.json".into(),
            "/node_modules/B/lib/b.js".into(),
        ),
    ])
}
// port: ModuleLoaderTest#testWindowsAddresses
#[test]
fn test_windows_addresses() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["js\\a.js", "js\\b.js"]))
        .set_factory(Arc::new(node::Factory::new()))
        .build();
    assert_uri("js/a.js", loader.resolve("js\\a.js"));
    assert_uri(
        "js/b.js",
        resolve_js_module(loader.resolve("js\\a.js"), "./b"),
    );
}
// port: ModuleLoaderTest#testJsExtensionNode
#[test]
fn test_js_extension_node() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["js/a.js", "js/b.js"]))
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries()),
        )))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();
    assert_uri("js/a.js", loader.resolve("js/a.js"));
    assert_uri(
        "js/b.js",
        resolve_js_module(loader.resolve("js/a.js"), "./b"),
    );
    assert_uri(
        "js/b.js",
        resolve_js_module(loader.resolve("js/a.js"), "./b.js"),
    );
}
// port: ModuleLoaderTest#testLocateJsNode
#[test]
fn test_locate_js_node() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["A/index.js", "B/index.js", "app.js"]))
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries()),
        )))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();

    input("A/index.js");
    input("B/index.js");
    input("app.js");
    assert_uri("A/index.js", loader.resolve("A/index.js"));
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "../A"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "../A/"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "../A/index"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("app.js"), "./A/index"),
    );
    assert!(resolve_js_module(loader.resolve("app.js"), "A/index").is_none());
    assert!(resolve_js_module(loader.resolve("folder/app.js"), "A/index").is_none());
    assert!(resolve_js_module(loader.resolve("folder/app.js"), "index").is_none());
}
// port: ModuleLoaderTest#testLocateNodeModuleNode
#[test]
fn test_locate_node_module_node() {
    let compiler_inputs = inputs(&[
        "/A/index.js",
        "/A/index.json",
        "/node_modules/A/index.js",
        "/node_modules/A/foo.js",
        "/node_modules/A/node_modules/A/index.json",
        "/B/package.json",
        "/B/lib/b.js",
        "/node_modules/B/package.json",
        "/node_modules/B/lib/b.js",
    ]);

    let loader = ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(compiler_inputs)
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries()),
        )))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();

    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve(" /foo.js"), "/A"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "/A/index.js"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "./A"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "./A/index.js"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "/A"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "/A/index"),
    );
    assert_uri(
        "/A/index.json",
        resolve_js_module(loader.resolve("/foo.js"), "/A/index.json"),
    );

    assert_uri(
        "/node_modules/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "A"),
    );
    assert_uri(
        "/node_modules/A/node_modules/A/index.json",
        resolve_js_module(loader.resolve("/node_modules/A/foo.js"), "A"),
    );
    assert_uri(
        "/node_modules/A/foo.js",
        resolve_js_module(loader.resolve("/node_modules/A/index.js"), "./foo"),
    );

    assert_uri(
        "/B/lib/b.js",
        resolve_js_module(loader.resolve("/app.js"), "/B"),
    );
    assert_uri(
        "/B/lib/b.js",
        resolve_js_module(loader.resolve("/app.js"), "/B/"),
    );

    assert_uri(
        "/node_modules/B/lib/b.js",
        resolve_js_module(loader.resolve("/app.js"), "B"),
    );
}
// port: ModuleLoaderTest#testJsExtensionBrowser
#[test]
fn test_js_extension_browser() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["js/a.js", "js/b.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();
    assert_uri("js/a.js", loader.resolve("js/a.js"));
    assert!(resolve_js_module(loader.resolve("js/a.js"), "./b").is_none());
    assert_uri(
        "js/b.js",
        resolve_js_module(loader.resolve("js/a.js"), "./b.js"),
    );
}
// port: ModuleLoaderTest#testLocateJsBrowser
#[test]
fn test_locate_js_browser() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["A/index.js", "B/index.js", "app.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    input("A/index.js");
    input("B/index.js");
    input("app.js");
    assert_uri("A/index.js", loader.resolve("A/index.js"));
    assert!(resolve_js_module(loader.resolve("B/index.js"), "../A").is_none());
    assert!(resolve_js_module(loader.resolve("B/index.js"), "../A/index").is_none());
    assert!(resolve_js_module(loader.resolve("app.js"), "./A/index").is_none());
    assert!(resolve_js_module(loader.resolve("app.js"), "A/index").is_none());
    assert!(resolve_js_module(loader.resolve("folder/app.js"), "A/index").is_none());
    assert!(resolve_js_module(loader.resolve("folder/app.js"), "index").is_none());

    assert!(resolve_js_module(loader.resolve("B/index.js"), "../A").is_none());
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "../A/index.js"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("app.js"), "./A/index.js"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("folder/app.js"), "../A/index.js"),
    );
    assert!(resolve_js_module(loader.resolve("folder/app.js"), "index").is_none());
}
// port: ModuleLoaderTest#testLocateNodeModuleBrowser
#[test]
fn test_locate_node_module_browser() {
    let compiler_inputs = inputs(&[
        "/A/index.js",
        "/A/index.json",
        "/node_modules/A/index.js",
        "/node_modules/A/foo.js",
        "/node_modules/A/node_modules/A/index.json",
        "/B/package.json",
        "/B/lib/b.js",
        "/node_modules/B/package.json",
        "/node_modules/B/lib/b.js",
    ]);

    let loader = ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(compiler_inputs)
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    assert!(resolve_js_module(loader.resolve("/foo.js"), "/A").is_none());
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "/A/index.js"),
    );
    assert!(resolve_js_module(loader.resolve("/foo.js"), "./A").is_none());
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "./A/index.js"),
    );
    assert!(resolve_js_module(loader.resolve("/foo.js"), "/A").is_none());
    assert!(resolve_js_module(loader.resolve("/foo.js"), "/A/index").is_none());
    assert_uri(
        "/A/index.json",
        resolve_js_module(loader.resolve("/foo.js"), "/A/index.json"),
    );

    assert!(resolve_js_module(loader.resolve("/foo.js"), "A").is_none());
    assert!(resolve_js_module(loader.resolve("/node_modules/A/foo.js"), "A").is_none());
    assert!(resolve_js_module(loader.resolve("/node_modules/A/index.js"), "./foo").is_none());
    assert_uri(
        "/node_modules/A/foo.js",
        resolve_js_module(loader.resolve("/node_modules/A/index.js"), "./foo.js"),
    );

    assert!(resolve_js_module(loader.resolve("/app.js"), "/B").is_none());

    assert!(resolve_js_module(loader.resolve("/app.js"), "B").is_none());
}
// port: ModuleLoaderTest#testNormalizeUris
#[test]
fn test_normalize_uris() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["a", "b", "/c"])
        .set_inputs(inputs(&[]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();
    assert_uri("a.js", loader.resolve("a/a.js"));
    assert_uri("a.js", loader.resolve("a.js"));
    assert_uri("some.js", loader.resolve("some.js"));
    assert_uri("/x.js", loader.resolve("/x.js"));
    assert_uri("x-y.js", loader.resolve("x:y.js"));
    assert_uri("foo%20bar.js", loader.resolve("foo bar.js"));
}
// port: ModuleLoaderTest#testDuplicateUris
#[test]
fn test_duplicate_uris() {
    let e = std::panic::catch_unwind(|| {
        ModuleLoader::builder()
            .set_module_roots(["a", "b"])
            .set_inputs(inputs(&["a/f.js", "b/f.js"]))
            .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
            .build()
    });
    let e = e.err().unwrap();
    let msg = e
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| e.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(msg.contains("Duplicate module path"));
}
// port: ModuleLoaderTest#testCanonicalizePath
#[test]
fn test_canonicalize_path() {
    assert_eq!(ModuleNames::canonicalize_path("a/b/c"), "a/b/c");
    assert_eq!(ModuleNames::canonicalize_path("a/b/../c"), "a/c");
    assert_eq!(ModuleNames::canonicalize_path("a/b/../../b/c"), "b/c");
    assert_eq!(ModuleNames::canonicalize_path("a/b/../../c"), "c");
    assert_eq!(ModuleNames::canonicalize_path("../a/b/.."), "../a");
    assert_eq!(ModuleNames::canonicalize_path("/a/b/../../.."), "/");
    assert_eq!(ModuleNames::canonicalize_path("/a/../../../b"), "/b");
    assert_eq!(ModuleNames::canonicalize_path("/a/.."), "/");
}
// port: ModuleLoaderTest#testToJSIdentifier
#[test]
fn test_to_js_identifier() {
    assert_eq!(
        ModuleNames::to_js_identifier("com/example/test"),
        "com$example$test"
    );
    assert_eq!(
        ModuleNames::to_js_identifier("file://a/b.jar!com/example/test"),
        "file_$$a$b_jar$com$example$test"
    );
}
// port: ModuleLoaderTest#testEscapePath
#[test]
fn test_escape_path() {
    let loader = ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(inputs(&["/has:special:chars.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .set_path_resolver(PathResolver::RELATIVE)
        .set_path_escaper(PathEscaper::ESCAPE)
        .build();

    assert_eq!(
        loader.resolve("file://my/file.js").to_string(),
        "file-//my/file.js"
    );
    assert_eq!(
        resolve_js_module(loader.resolve("c"), "/has:special:chars.js")
            .unwrap()
            .to_string(),
        "/has-special-chars.js"
    );
}
// port: ModuleLoaderTest#testDoNoEscapePath
#[test]
fn test_do_no_escape_path() {
    let loader = ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(inputs(&["/has:special:chars.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .set_path_resolver(PathResolver::RELATIVE)
        .set_path_escaper(PathEscaper::CANONICALIZE_ONLY)
        .build();

    assert_eq!(
        loader.resolve("file://my/file.js").to_string(),
        "file://my/file.js"
    );
    assert_eq!(
        resolve_js_module(loader.resolve("c"), "/has:special:chars.js")
            .unwrap()
            .to_string(),
        "/has:special:chars.js"
    );
}
// port: ModuleLoaderTest#testLocateNodeModulesNoLeadingSlash
#[test]
fn test_locate_node_modules_no_leading_slash() {
    let compiler_inputs = inputs(&[
        "/A/index.js",
        "/A/index.json",
        "node_modules/A/index.js",
        "node_modules/A/foo.js",
        "node_modules/A/node_modules/A/index.json",
        "/B/package.json",
        "/B/lib/b.js",
        "node_modules/B/package.json",
        "node_modules/B/lib/b.js",
    ]);

    let loader = ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(compiler_inputs)
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries()),
        )))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();

    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve(" /foo.js"), "/A"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "/A/index.js"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "./A"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "./A/index.js"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "/A"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "/A/index"),
    );
    assert_uri(
        "/A/index.json",
        resolve_js_module(loader.resolve("/foo.js"), "/A/index.json"),
    );

    assert_uri(
        "/node_modules/A/index.js",
        resolve_js_module(loader.resolve("/foo.js"), "A"),
    );
    assert_uri(
        "/node_modules/A/node_modules/A/index.json",
        resolve_js_module(loader.resolve("node_modules/A/foo.js"), "A"),
    );
    assert_uri(
        "node_modules/A/foo.js",
        resolve_js_module(loader.resolve("node_modules/A/index.js"), "./foo"),
    );

    assert_uri(
        "/B/lib/b.js",
        resolve_js_module(loader.resolve("/app.js"), "/B"),
    );

    assert_uri(
        "/node_modules/B/lib/b.js",
        resolve_js_module(loader.resolve("/app.js"), "B"),
    );
}
// port: ModuleLoaderTest#testLocateNodeModulesBrowserFieldAdvancedUsage
#[test]
fn test_locate_node_modules_browser_field_advanced_usage() {
    let package_json_main_entries = IndexMap::from([
        (
            "/node_modules/mymodule/package.json".into(),
            "/node_modules/mymodule/server.js".into(),
        ),
        (
            "/node_modules/mymodule/server.js".into(),
            "/node_modules/mymodule/client.js".into(),
        ),
        (
            "/node_modules/mymodule/override/relative.js".into(),
            "/node_modules/mymodule/./with/this.js".into(),
        ),
        (
            "/node_modules/mymodule/exclude/this.js".into(),
            ModuleLoader::JSC_BROWSER_SKIPLISTED_MARKER.into(),
        ),
        (
            "/node_modules/mymodule/replace/other.js".into(),
            "/node_modules/mymodule/with/alternative.js".into(),
        ),
    ]);

    let compiler_inputs = inputs(&[
        "/node_modules/mymodule/package.json",
        "/node_modules/mymodule/client.js",
        "/node_modules/mymodule/with/alternative.js",
        "/node_modules/mymodule/with/this.js",
        "/foo.js",
    ]);

    let loader = ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(compiler_inputs)
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries),
        )))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();

    assert_uri(
        "/node_modules/mymodule/client.js",
        resolve_js_module(loader.resolve("/foo.js"), "mymodule"),
    );
    assert_uri(
        "/node_modules/mymodule/with/alternative.js",
        resolve_js_module(loader.resolve("/foo.js"), "mymodule/replace/other.js"),
    );
    assert_uri(
        "/node_modules/mymodule/with/alternative.js",
        resolve_js_module(
            loader.resolve("/node_modules/mymodule/client.js"),
            "./replace/other.js",
        ),
    );
    assert_uri(
        "/node_modules/mymodule/with/this.js",
        resolve_js_module(loader.resolve("/foo.js"), "mymodule/override/relative.js"),
    );
    assert_uri(
        "/node_modules/mymodule/with/this.js",
        resolve_js_module(
            loader.resolve("/node_modules/mymodule/client.js"),
            "./override/relative.js",
        ),
    );
    assert!(
        resolve_js_module(
            loader.resolve("/node_modules/mymodule/client.js"),
            "./exclude/this.js"
        )
        .is_none()
    );
}
// port: ModuleLoaderTest#testLocateNodeModuleWithMultipleRootsSimple
#[test]
fn test_locate_node_module_with_multiple_roots_simple() {
    let compiler_inputs = inputs(&["/generated_files/node_modules/second.js", "/foo.js"]);

    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["generated_files/"])
        .set_inputs(compiler_inputs)
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(IndexMap::from([])),
        )))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();

    assert_uri(
        "/node_modules/second.js",
        resolve_js_module(loader.resolve("/foo.js"), "second"),
    );
}
// port: ModuleLoaderTest#testLocateNodeModuleWithMultipleRoots
#[test]
fn test_locate_node_module_with_multiple_roots() {
    let compiler_inputs = inputs(&[
        "/node_modules/first.js",
        "/generated_files/node_modules/second.js",
        "/some_other/node_modules/third.js",
        "/foo.js",
    ]);

    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["generated_files/"])
        .set_inputs(compiler_inputs)
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(IndexMap::from([])),
        )))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();

    assert_uri(
        "/node_modules/first.js",
        resolve_js_module(loader.resolve("/foo.js"), "first"),
    );
    assert_uri(
        "/node_modules/second.js",
        resolve_js_module(loader.resolve("/foo.js"), "second"),
    );

    assert!(resolve_js_module(loader.resolve("/foo.js"), "third").is_none());
}
// port: ModuleLoaderTest#testWebpack
#[test]
fn test_webpack() {
    let webpack_modules_by_id = IndexMap::from([
        ("1".into(), "A/index.js".into()),
        ("B/index.js".into(), "B/index.js".into()),
        ("3".into(), "app.js".into()),
    ]);

    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["A/index.js", "B/index.js", "app.js"]))
        .set_factory(Arc::new(webpack::Factory::new(webpack_modules_by_id)))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();

    input("A/index.js");
    input("B/index.js");
    input("app.js");
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("A/index.js"), "1"),
    );
    assert_uri(
        "/B/index.js",
        resolve_js_module(loader.resolve("A/index.js"), "B/index.js"),
    );
    assert_uri(
        "/app.js",
        resolve_js_module(loader.resolve("A/index.js"), "3"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "1"),
    );
    assert_uri(
        "/B/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "B/index.js"),
    );
    assert_uri(
        "/app.js",
        resolve_js_module(loader.resolve("B/index.js"), "3"),
    );
    assert_uri(
        "/A/index.js",
        resolve_js_module(loader.resolve("app.js"), "1"),
    );
    assert_uri(
        "/B/index.js",
        resolve_js_module(loader.resolve("app.js"), "B/index.js"),
    );
    assert_uri("/app.js", resolve_js_module(loader.resolve("app.js"), "3"));

    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "../A"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "../A/"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "../A/index"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("app.js"), "./A/index"),
    );
}
// port: ModuleLoaderTest#testBrowserWithPrefixReplacement
#[test]
fn test_browser_with_prefix_replacement() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&[
            "/path/to/project0/index.js",
            "/path/to/project1/index.js",
            "app.js",
        ]))
        .set_factory(Arc::new(prefixes::Factory::new(IndexMap::from([
            ("@project0/".into(), "/path/to/project0/".into()),
            ("+project1/".into(), "/path/to/project1/".into()),
            ("@root/".into(), "/".into()),
        ]))))
        .build();

    assert_uri(
        "/path/to/project0/index.js",
        resolve_js_module(loader.resolve("fake.js"), "@project0/index.js"),
    );
    assert_uri(
        "/path/to/project1/index.js",
        resolve_js_module(loader.resolve("fake.js"), "+project1/index.js"),
    );
    assert_uri(
        "/app.js",
        resolve_js_module(loader.resolve("fake.js"), "@root/app.js"),
    );
}
// port: ModuleLoaderTest#testBrowserWithPrefixReplacementResolveModuleAsPath
#[test]
fn test_browser_with_prefix_replacement_resolve_module_as_path() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec![".", "/path/to/project0/", "/path/to/project1/"])
        .set_inputs(inputs(&[]))
        .set_factory(Arc::new(prefixes::Factory::new(IndexMap::from([
            ("@project0/".into(), "/path/to/project0/".into()),
            ("+project1/".into(), "/path/to/project1/".into()),
            ("@root/".into(), "/".into()),
        ]))))
        .build();

    assert_uri(
        "index.js",
        loader
            .resolve("fake.js")
            .resolve_module_as_path("@project0/index.js"),
    );
    assert_uri(
        "foo/bar/index.js",
        loader
            .resolve("fake.js")
            .resolve_module_as_path("+project1/foo/bar/index.js"),
    );
    assert_uri(
        "@not/a/root/index.js",
        loader
            .resolve("fake.js")
            .resolve_module_as_path("@not/a/root/index.js"),
    );
}
// port: ModuleLoaderTest#testBrowserWithPrefixReplacementAppliedMostSpecificToLeast
#[test]
fn test_browser_with_prefix_replacement_applied_most_specific_to_least() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["/p0/p1/p2/file.js"]))
        .set_factory(Arc::new(prefixes::Factory::new(IndexMap::from([
            ("0/1/2/".into(), "/p0/p1/p2/".into()),
            ("0/".into(), "/p0/".into()),
            ("0/1/".into(), "/p0/p1/".into()),
        ]))))
        .build();

    assert_uri(
        "/p0/p1/p2/file.js",
        resolve_js_module(loader.resolve("fake.js"), "0/p1/p2/file.js"),
    );
    assert_uri(
        "/p0/p1/p2/file.js",
        resolve_js_module(loader.resolve("fake.js"), "0/1/p2/file.js"),
    );
    assert_uri(
        "/p0/p1/p2/file.js",
        resolve_js_module(loader.resolve("fake.js"), "0/1/2/file.js"),
    );
}
// port: ModuleLoaderTest#testBrowserWithPrefixReplacementInvalidPrefix
#[test]
fn test_browser_with_prefix_replacement_invalid_prefix() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["/path/to/file.js"]))
        .set_factory(Arc::new(prefixes::Factory::new(IndexMap::from([(
            "prefix/".into(),
            "/path/to/".into(),
        )]))))
        .build();

    assert_uri(
        "/path/to/file.js",
        resolve_js_module(loader.resolve("fake.js"), "prefix/file.js"),
    );
    assert!(errors.lock().unwrap().is_empty());

    resolve_js_module(loader.resolve("fake.js"), "invalid/file.js");
    assert_eq!(errors.lock().unwrap().len(), 1);
    assert!(std::ptr::eq(
        errors.lock().unwrap()[0].get_type(),
        &prefixes::INVALID_AMBIGUOUS_PATH
    ));
}
// port: ModuleLoaderTest#testCustomResolution
#[test]
fn test_custom_resolution() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["A/index.js", "B/index.js", "app.js"]))
        .set_factory(Arc::new(CustomFactory))
        .build();

    assert_uri("A/index.js", loader.resolve("A/index.js"));
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "../A/index.js"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("app.js"), "./A/index.js"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("folder/app.js"), "../A/index.js"),
    );

    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("B/index.js"), "@custom/A/index.js"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("app.js"), "@custom/A/index.js"),
    );
    assert_uri(
        "A/index.js",
        resolve_js_module(loader.resolve("folder/app.js"), "@custom/A/index.js"),
    );
}
// port: ModuleLoaderTest#testRootsAppliedMostSpecificFirst
#[test]
fn test_roots_applied_most_specific_first() {
    let loader = ModuleLoader::builder()
        .set_module_roots(vec!["/path/", "/path/to/project/", "/path/to/"])
        .set_inputs(inputs(&[]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    assert_uri("file.js", loader.resolve("/path/to/project/file.js"));
}
// port: ModuleLoaderTest#testBrowserResolverSilentHandlesExternalUrl
#[test]
fn test_browser_resolver_silent_handles_external_url() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["app.js", "local/module.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    let app_path = loader.resolve("app.js");
    let result = app_path.resolve_js_module_silently("https://cdn.example.com/library.js");

    assert!(result.is_none());
    assert!(errors.lock().unwrap().is_empty());
}
// port: ModuleLoaderTest#testBrowserResolverSilentHandlesBareModuleName
#[test]
fn test_browser_resolver_silent_handles_bare_module_name() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["app.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    let app_path = loader.resolve("app.js");
    assert!(app_path.resolve_js_module_silently("react").is_none());
    assert!(app_path.resolve_js_module_silently("lodash").is_none());
    assert!(app_path.resolve_js_module_silently("@babel/core").is_none());
    assert!(errors.lock().unwrap().is_empty());
}
// port: ModuleLoaderTest#testBrowserResolverSilentResolvesRelativePath
#[test]
fn test_browser_resolver_silent_resolves_relative_path() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["app.js", "lib/utils.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    let app_path = loader.resolve("app.js");
    let result = app_path.resolve_js_module_silently("./lib/utils.js");

    assert!(result.is_some());
    assert_eq!(result.unwrap().to_string(), "lib/utils.js");
    assert!(errors.lock().unwrap().is_empty());
}
// port: ModuleLoaderTest#testBrowserResolverSilentHandlesUnresolvableRelativePath
#[test]
fn test_browser_resolver_silent_handles_unresolvable_relative_path() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["app.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    let app_path = loader.resolve("app.js");
    let result = app_path.resolve_js_module_silently("./missing/module.js");

    assert!(result.is_none());
    assert!(errors.lock().unwrap().is_empty());
}
// port: ModuleLoaderTest#testBrowserResolverDoesNotHandleUnresolvableRelativePath
#[test]
fn test_browser_resolver_does_not_handle_unresolvable_relative_path() {
    let errors_from_regular = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let regular_loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(
            errors_from_regular.clone(),
        )))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["app.js"]))
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build();

    let app_path1 = regular_loader.resolve("app.js");
    app_path1.resolve_js_module("https://cdn.example.com/lib.js", Some("test.js"), 1, 1);
    assert_eq!(errors_from_regular.lock().unwrap().len(), 1);
}
// port: ModuleLoaderTest#testNodeResolverDoesNotHandleUnresolvableRelativePath
#[test]
fn test_node_resolver_does_not_handle_unresolvable_relative_path() {
    let errors_from_regular = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let regular_loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(
            errors_from_regular.clone(),
        )))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["app.js"]))
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries()),
        )))
        .build();

    let app_path = regular_loader.resolve("app.js");
    app_path.resolve_js_module("https://cdn.example.com/lib.js", Some("test.js"), 1, 1);
    assert_eq!(errors_from_regular.lock().unwrap().len(), 1);
}
// port: ModuleLoaderTest#testNodeResolverSilentHandlesAmbiguousIdentifier
#[test]
fn test_node_resolver_silent_handles_ambiguous_identifier() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["app.js"]))
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries()),
        )))
        .build();

    let app_path = loader.resolve("app.js");
    let result = app_path.resolve_js_module_silently("https://cdn.example.com/lib.js");

    assert!(result.is_none());
    assert!(errors.lock().unwrap().is_empty());
}
// port: ModuleLoaderTest#testNodeResolverSilentResolvesRelativePath
#[test]
fn test_node_resolver_silent_resolves_relative_path() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["app.js", "lib/utils.js"]))
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries()),
        )))
        .build();

    let app_path = loader.resolve("app.js");
    let result = app_path.resolve_js_module_silently("./lib/utils.js");

    assert!(result.is_some());
    assert_eq!(result.unwrap().to_string(), "lib/utils.js");
    assert!(errors.lock().unwrap().is_empty());
}
// port: ModuleLoaderTest#testBrowserWithPrefixesSilentHandlesAmbiguousIdentifier
#[test]
fn test_browser_with_prefixes_silent_handles_ambiguous_identifier() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["/p0/p1/p2/file.js"]))
        .set_factory(Arc::new(prefixes::Factory::new(IndexMap::from([
            ("0/1/2/".into(), "/p0/p1/p2/".into()),
            ("0/".into(), "/p0/".into()),
            ("0/1/".into(), "/p0/p1/".into()),
        ]))))
        .build();

    let app_path = loader.resolve("/p0/file.js");
    let result = app_path.resolve_js_module_silently("lodash");

    assert!(result.is_none());
    assert!(errors.lock().unwrap().is_empty());
}
// port: ModuleLoaderTest#testBrowserWithPrefixesSilentResolvesTransformedPrefixes
#[test]
fn test_browser_with_prefixes_silent_resolves_transformed_prefixes() {
    let errors = Arc::new(Mutex::new(Vec::<JSError>::new()));

    let loader = ModuleLoader::builder()
        .set_error_handler(Some(Arc::new(Mutex::new(RecordingHandler(errors.clone())))))
        .set_module_roots(vec!["."])
        .set_inputs(inputs(&["/p0/p1/p2/file.js"]))
        .set_factory(Arc::new(prefixes::Factory::new(IndexMap::from([
            ("0/1/2/".into(), "/p0/p1/p2/".into()),
            ("0/".into(), "/p0/".into()),
            ("0/1/".into(), "/p0/p1/".into()),
        ]))))
        .build();

    let app_path = loader.resolve("fake.js");
    let result = app_path.resolve_js_module_silently("0/1/2/file.js");

    assert!(result.is_some());
    assert_eq!(result.unwrap().to_string(), "/p0/p1/p2/file.js");
    assert!(errors.lock().unwrap().is_empty());
}
// port: ModuleLoaderTest#testRelativeIdentifierDotAndDotDot
#[test]
fn test_relative_identifier_dot_and_dot_dot() {
    assert!(ModuleLoader::is_relative_identifier("."));
    assert!(ModuleLoader::is_relative_identifier(".."));
    assert!(!ModuleLoader::is_ambiguous_identifier("."));
    assert!(!ModuleLoader::is_ambiguous_identifier(".."));
}
// port: ModuleLoaderTest#testLocateDotAndDotDotNode
#[test]
fn test_locate_dot_and_dot_dot_node() {
    let compiler_inputs = inputs(&[
        "/app/index.js",
        "/app/sub/sub.js",
        "/app/sub/index.js",
        "/app/sub/sub2/sub2.js",
        "/B/package.json",
        "/B/lib/b.js",
        "/B/lib/index.js",
    ]);

    let loader = ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(compiler_inputs)
        .set_factory(Arc::new(node::Factory::with_package_json_main_entries(
            Some(package_json_main_entries()),
        )))
        .set_path_resolver(PathResolver::RELATIVE)
        .build();

    assert_uri(
        "/app/sub/index.js",
        resolve_js_module(loader.resolve("/app/sub/sub.js"), "."),
    );
    assert_uri(
        "/app/sub/index.js",
        resolve_js_module(loader.resolve("/app/sub/sub2/sub2.js"), ".."),
    );
    assert_uri(
        "/app/index.js",
        resolve_js_module(loader.resolve("/app/sub/sub.js"), ".."),
    );
    assert_uri(
        "/B/lib/b.js",
        resolve_js_module(loader.resolve("/B/lib/sub.js"), ".."),
    );
    assert_uri(
        "/B/lib/index.js",
        resolve_js_module(loader.resolve("/B/lib/sub.js"), "."),
    );
}

// port: ModuleLoaderTest#inputs
fn inputs(names: &[&str]) -> Vec<SimpleDependencyInfo> {
    names.iter().map(|name| input(name)).collect()
}
// port: ModuleLoaderTest#input
fn input(name: &str) -> SimpleDependencyInfo {
    // CompilerInput#getName returns SourceFile#getName; this stand-in is sufficient until CompilerInput is ported.
    let source = SourceFile::from_code(name, "");
    SimpleDependencyInfo::builder("", source.get_name()).build()
}
// port: ModuleLoaderTest#assertUri
fn assert_uri(expected: &str, actual: impl Into<Option<ModulePath>>) {
    assert_eq!(actual.into().unwrap().to_string(), expected);
}
// port: ModuleLoaderTest#resolveJsModule
fn resolve_js_module(context: ModulePath, module_address: &str) -> Option<ModulePath> {
    context.resolve_js_module(module_address, None, -1, -1)
}
struct RecordingHandler(Arc<Mutex<Vec<JSError>>>);
impl ErrorHandler for RecordingHandler {
    // port: ModuleLoaderTest#testBrowserWithPrefixReplacementInvalidPrefix (error callback)
    fn report(&mut self, _level: CheckLevel, error: JSError) {
        self.0.lock().unwrap().push(error);
    }
}
struct Custom {
    base: ModuleResolverBase,
}
impl ModuleResolver for Custom {
    // port: ModuleLoaderTest#testCustomResolution (anonymous resolver base)
    fn base(&self) -> &ModuleResolverBase {
        &self.base
    }
    // port: ModuleLoaderTest#testCustomResolution (anonymous resolveJsModule)
    fn resolve_js_module(
        &self,
        script_address: &str,
        module_address: &str,
        _source: Option<&str>,
        _line: i32,
        _col: i32,
    ) -> Option<String> {
        self.base.locate(
            script_address,
            module_address
                .strip_prefix("@custom/")
                .unwrap_or(module_address),
        )
    }
    // port: ModuleLoaderTest#testCustomResolution (anonymous resolveJsModuleSilently)
    fn resolve_js_module_silently(
        &self,
        script_address: &str,
        module_address: &str,
    ) -> Option<String> {
        self.resolve_js_module(script_address, module_address, Some(""), 0, 0)
    }
}
struct CustomFactory;
impl ModuleResolverFactory for CustomFactory {
    // port: ModuleLoaderTest#testCustomResolution (factory callback)
    fn create(
        &self,
        paths: IndexSet<String>,
        roots: Vec<String>,
        handler: SharedErrorHandler,
        escaper: PathEscaper,
    ) -> Arc<dyn ModuleResolver> {
        Arc::new(Custom {
            base: ModuleResolverBase::new(paths, roots, handler, escaper),
        })
    }
}

// port: ModuleLoader#relativePathFrom (IllegalArgumentException cases through try_relative_path_from)
#[test]
fn test_try_relative_path_from_reports_illegal_argument() {
    assert_eq!(
        ModuleLoader::try_relative_path_from("m1.js", "js/m0.js").unwrap_err(),
        "Relative path between URIs cannot be calculated"
    );
    assert_eq!(
        ModuleLoader::try_relative_path_from("/js/m1.js", "other/m0.js").unwrap_err(),
        "'other' is different type of Path"
    );
    assert_eq!(
        ModuleLoader::try_relative_path_from("../node_modules/m1.js", "js/m0.js").unwrap_err(),
        "Unable to compute relative  path from ../node_modules to js/m0.js"
    );
    assert_eq!(
        ModuleLoader::try_relative_path_from("js/m1.js", "m0.js").unwrap(),
        "../m0.js"
    );
    assert_eq!(
        ModuleLoader::try_relative_path_from("m1.js", "m0.js").unwrap(),
        "./m0.js"
    );
}
