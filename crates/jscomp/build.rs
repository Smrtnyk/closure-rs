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

#[allow(dead_code)] // The target reads these fields from the generated data.
#[path = "src/source_file/bundled_script.rs"]
mod bundled_script;
#[path = "build/bundled_script_summary.rs"]
mod bundled_script_summary;
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
/// Also returns the parameter lists of candidate `PLAIN_STUB_NO_SCOPE` statements (by statement
/// index) and all `@template` / `@typeTransformation` names declared anywhere in the script, so
/// `main` can clear `PLAIN_STUB_NO_SCOPE` if any parameter collides with any template name across
/// the browser externs.
fn declaration_records(
    ast: &closure_rhino::node::Ast,
    script: closure_rhino::node::NodeId,
    all_template_names: &mut closure_rhino::fast_hash::IndexSet<JsString>,
) -> (Vec<u8>, Vec<(usize, Vec<JsString>)>) {
    use closure_rhino::{fast_hash::IndexSet, token::Token};
    use declarations::*;
    fn collect_templates(
        ast: &closure_rhino::node::Ast,
        n: closure_rhino::node::NodeId,
        out: &mut IndexSet<JsString>,
    ) {
        if let Some(doc) = n.get_jsdoc_info_ref(ast) {
            out.extend(doc.get_template_type_names());
            out.extend(doc.get_type_transformations().into_keys());
        }
        for c in n.children(ast) {
            collect_templates(ast, c, out);
        }
    }
    collect_templates(ast, script, all_template_names);

    let mut candidate_stub_params: Vec<(usize, Vec<JsString>)> = Vec::new();
    let records = script
        .children(ast)
        .enumerate()
        .map(|(stmt_idx, statement)| {
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
            let Some(declaration) = Declaration::read(ast, statement, record) else {
                return OTHER;
            };
            let info = match declaration.doc {
                DocSite::None => None,
                DocSite::On(node) => node.get_jsdoc_info_ref(ast),
                // The NAME in this shape has a single VAR, LET or CONST parent at script level.
                DocSite::BestForName(name) => name
                    .get_jsdoc_info_ref(ast)
                    .or_else(|| statement.get_jsdoc_info_ref(ast)),
            };
            let check_non_nullable = declaration.possible_alias
                || info.is_some_and(|doc| doc.has_enum_parameter_type() || doc.has_typedef_type());
            let primitive = if record == PROPERTY {
                info.and_then(|doc| doc.get_type())
                    .map_or(0, |expr| closed_annotation(ast, expr.get_root()))
            } else {
                0
            };
            let mut plain_stub_no_scope = 0u8;
            if let Some(fn_node) = declaration.function {
                let fn_name = fn_node.get_first_child(ast).unwrap();
                let fn_name_str = fn_name.get_string_ref(ast);
                let params = fn_name.get_next(ast).unwrap();
                let is_decl = record == FUNCTION;
                let mut ok = !is_decl || fn_name_str != "arguments";
                let mut seen: IndexSet<JsString> = IndexSet::default();
                let mut names: Vec<JsString> = Vec::new();
                for param in params.children(ast) {
                    let p_str = param.get_string_ref(ast);
                    if !param.is_name(ast)
                        || param.has_children(ast)
                        || param.get_jsdoc_info_ref(ast).is_some()
                        || p_str.is_empty()
                        || p_str == "arguments"
                        || p_str == "this"
                        || p_str == "super"
                        || p_str == "new.target"
                        || p_str == "exports"
                        || (is_decl && p_str == fn_name_str)
                    {
                        ok = false;
                        break;
                    }
                    let owned = p_str.clone();
                    if !seen.insert(owned.clone()) {
                        ok = false;
                        break;
                    }
                    names.push(owned);
                }
                if ok {
                    plain_stub_no_scope = PLAIN_STUB_NO_SCOPE;
                    if !names.is_empty() {
                        candidate_stub_params.push((stmt_idx, names));
                    }
                }
            }
            // A sequence of ordinary typed fields can bind one live prototype.
            let prototype_field = record == PROPERTY
                && info.is_some_and(|info| info.has_type())
                && statement.get_first_child(ast).is_some_and(|property| {
                    let name = property.get_string_ref(ast);
                    let owner = property.get_first_child(ast).unwrap();
                    name != "prototype"
                        && !name.ends_with(".prototype")
                        && owner.is_get_prop(ast)
                        && owner.get_string_ref(ast) == "prototype"
                        && owner
                            .get_first_child(ast)
                            .is_some_and(|root| root.is_name(ast))
                });
            record
                | if prototype_field { PROTOTYPE_FIELD } else { 0 }
                | primitive
                | plain_stub_no_scope
                | if declaration.function.is_some() {
                    HAS_FUNCTION
                } else {
                    0
                }
                | if check_non_nullable {
                    CHECK_NON_NULLABLE
                } else {
                    0
                }
        })
        .collect();
    (records, candidate_stub_params)
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
    println!("cargo:rerun-if-changed=src/source_file/bundled_script.rs");
    println!("cargo:rerun-if-changed=build/bundled_script_summary.rs");
    let mut summaries = String::from("match name {\n");
    let mut sizes = String::from("match name {\n");
    let mut all_template_names: closure_rhino::fast_hash::IndexSet<JsString> =
        closure_rhino::fast_hash::IndexSet::default();
    type PendingDeclarationRecord = (String, Vec<u8>, Vec<(usize, Vec<JsString>)>);
    let mut pending_records: Vec<PendingDeclarationRecord> = Vec::new();
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
        let declaration_file = format!("extern-{i}-declarations.bin");
        if let Some((bytes, (records, candidate_stub_params, summary))) =
            closure_parsing::extern_snapshot::generate_bundled_with_ast(
                source,
                code,
                |ast, root| {
                    let (records, candidate_stub_params) =
                        declaration_records(ast, root, &mut all_template_names);
                    let summary =
                        bundled_script_summary::generate(ast, root, &declaration_file, &records);
                    (records, candidate_stub_params, summary)
                },
            )
        {
            if let Some(summary) = summary {
                writeln!(summaries, "{:?} => Some(&{summary}),", file.path).unwrap();
            }
            let filename = format!("extern-{i}.bin");
            fs::write(out.join(&filename), bytes).unwrap();
            if records.iter().any(|&record| record != declarations::OTHER) {
                writeln!(
                    declarations,
                    "{:?} => Some(include_bytes!({declaration_file:?})),",
                    file.path
                )
                .unwrap();
            }
            pending_records.push((declaration_file, records, candidate_stub_params));
            writeln!(
                lookup,
                "{:?} => Some(include_bytes!({filename:?})),",
                file.path
            )
            .unwrap();
        }
    }
    for (declaration_file, mut records, candidate_stub_params) in pending_records {
        for (stmt_idx, params) in candidate_stub_params {
            if params.iter().any(|p| all_template_names.contains(p)) {
                records[stmt_idx] &= !declarations::PLAIN_STUB_NO_SCOPE;
            }
        }
        fs::write(out.join(&declaration_file), &records).unwrap();
    }
    summaries.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_script_summaries.rs"), summaries).unwrap();
    lookup.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_externs.rs"), lookup).unwrap();
    dependencies.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_extern_dependencies.rs"), dependencies).unwrap();
    declarations.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_extern_declarations.rs"), declarations).unwrap();
    sizes.push_str("_ => None,\n}\n");
    fs::write(out.join("bundled_extern_sizes.rs"), sizes).unwrap();
}
