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

//! Parse bundled externs with the host parser. Cargo rebuilds the script when its parser or
//! resource dependencies change, including for cross-compiled release binaries.
use closure_resources::{
    abstract_command_line_runner::AbstractCommandLineRunner, compiler_options::Environment,
};
use closure_rhino::{
    js_string::JsString, simple_source_file::SimpleSourceFile, static_source_file::SourceKind,
};
use std::{fmt::Write, fs, path::PathBuf, sync::Arc};

#[path = "src/deps/js_file_line_parser/lines.rs"]
mod lines;

/// One byte per top-level statement, in source order. Mark bare qualified property
/// declarations. Their names and JSDoc will be read from the live tree.
fn property_declarations(
    ast: &closure_rhino::node::Ast,
    script: closure_rhino::node::NodeId,
) -> Vec<u8> {
    script
        .children(ast)
        .map(|statement| {
            u8::from(
                statement.is_expr_result(ast)
                    && statement.get_first_child(ast).is_some_and(|property| {
                        property.is_get_prop(ast) && property.is_qualified_name(ast)
                    }),
            )
        })
        .collect()
}

/// JsFileRegexParser can produce dependencies or diagnostics only for Closure primitives,
/// module syntax or @provideGoog. Leave any such file to the configured module loader.
fn plain_dependency_flags(code: JsString) -> Option<(bool, bool)> {
    let mut comments = lines::Comments::default();
    let mut externs = false;
    let mut no_compile = false;
    let mut provides_goog = false;
    let mut uncommented = String::new();
    for line in lines::StringLines::new(code) {
        let line = comments.strip(line.unwrap(), |doc| {
            if doc.index_of("@provideGoog") >= 0 {
                provides_goog = true;
            } else if doc.index_of("@externs") >= 0 {
                externs = true;
            } else if doc.index_of("@nocompile") >= 0 {
                no_compile = true;
            }
            true
        })?;
        if provides_goog || line.starts_with("import") || line.starts_with("export") {
            return None;
        }
        // The dependency parser can join consecutive lines before matching goog.*.
        uncommented.push_str(&line.to_string_lossy());
    }
    (!uncommented.contains("goog.")).then_some((externs, no_compile))
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/deps/js_file_line_parser/lines.rs");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let mut lookup = String::from("match name {\n");
    let mut dependencies = String::from("match name {\n");
    let mut declarations = String::from("match name {\n");
    for (i, file) in AbstractCommandLineRunner::get_builtin_externs(Environment::BROWSER)
        .into_iter()
        .enumerate()
    {
        let source = Arc::new(SimpleSourceFile::new(&file.path, SourceKind::EXTERN));
        let code: JsString = file.get_code().into();
        if let Some(flags) = plain_dependency_flags(code.clone()) {
            writeln!(dependencies, "{:?} => Some({flags:?}),", file.path).unwrap();
        }
        if let Some((bytes, records)) =
            closure_parsing::extern_snapshot::generate_with_ast(source, code, property_declarations)
        {
            let filename = format!("extern-{i}.bin");
            fs::write(out.join(&filename), bytes).unwrap();
            if records.contains(&1) {
                let properties = format!("extern-{i}-properties.bin");
                fs::write(out.join(&properties), records).unwrap();
                writeln!(
                    declarations,
                    "{:?} => Some(include_bytes!({properties:?})),",
                    file.path
                )
                .unwrap();
            }
            writeln!(
                lookup,
                "{:?} => Some(include_bytes!({filename:?})),",
                file.path
            )
            .unwrap();
        }
    }
    lookup.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_externs.rs"), lookup).unwrap();
    dependencies.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_extern_dependencies.rs"), dependencies).unwrap();
    declarations.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_extern_declarations.rs"), declarations).unwrap();
}
