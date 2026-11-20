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

#[allow(dead_code)] // The target compiler consumes the fields of a verified declaration.
#[path = "src/source_file/declarations.rs"]
mod declarations;
#[path = "src/source_file/line_count.rs"]
mod line_count;
#[path = "src/deps/js_file_line_parser/lines.rs"]
mod lines;

#[allow(dead_code, clippy::upper_case_acronyms)] // Java protobuf enum names.
#[path = "src/serialization/protobuf.rs"]
mod protobuf;
#[path = "build/runtime_libraries.rs"]
mod runtime_libraries;
#[allow(non_camel_case_types)]
#[path = "src/serialization/source_file_proto.rs"]
mod source_file_proto;

/// One record per top-level statement, including unrecognized statements to preserve positions.
fn declaration_records(
    ast: &closure_rhino::node::Ast,
    script: closure_rhino::node::NodeId,
) -> Vec<u8> {
    use closure_rhino::token::Token;
    use declarations::*;
    script
        .children(ast)
        .map(|statement| {
            let record = match statement.get_token(ast) {
                Token::FUNCTION => FUNCTION,
                Token::VAR | Token::LET | Token::CONST => VARIABLE,
                Token::EXPR_RESULT
                    if statement
                        .get_first_child(ast)
                        .is_some_and(|n| n.is_assign(ast)) =>
                {
                    ASSIGNMENT
                }
                Token::EXPR_RESULT => PROPERTY,
                _ => OTHER,
            };
            if Declaration::read(ast, statement, record).is_some() {
                record
            } else {
                OTHER
            }
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
    println!("cargo:rerun-if-changed=src/source_file/line_count.rs");
    println!("cargo:rerun-if-changed=src/source_file/declarations.rs");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    runtime_libraries::generate(&out);
    let mut lookup = String::from("match name {\n");
    let mut dependencies = String::from("match name {\n");
    let mut declarations = String::from("match name {\n");
    let mut sizes = String::from("match name {\n");
    for (i, file) in AbstractCommandLineRunner::get_builtin_externs(Environment::BROWSER)
        .into_iter()
        .enumerate()
    {
        let source = Arc::new(SimpleSourceFile::new(&file.path, SourceKind::EXTERN));
        let code: JsString = file.get_code().into();
        // The ordinary SourceFile read path also handles BOM removal. Keep files needing
        // that normalization there, including its interaction with snapshot eligibility.
        if code.as_units().first() != Some(&0xfeff) {
            writeln!(
                sizes,
                "{:?} => Some(({}, {})),",
                file.path,
                code.length() as i32,
                line_count::num_lines(code.as_units())
            )
            .unwrap();
        }
        if let Some(flags) = plain_dependency_flags(code.clone()) {
            writeln!(dependencies, "{:?} => Some({flags:?}),", file.path).unwrap();
        }
        if let Some((bytes, records)) = closure_parsing::extern_snapshot::generate_bundled_with_ast(
            source,
            code,
            declaration_records,
        ) {
            let filename = format!("extern-{i}.bin");
            fs::write(out.join(&filename), bytes).unwrap();
            if records.iter().any(|&record| record != declarations::OTHER) {
                let properties = format!("extern-{i}-declarations.bin");
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
    sizes.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_extern_sizes.rs"), sizes).unwrap();
}
