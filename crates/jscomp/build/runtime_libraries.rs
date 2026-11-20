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

//! Index the single-shard runtime shipped by Closure. All offsets refer to the original
//! resource. Other package layouts use the compiler's ordinary TypedAST loader.
use crate::protobuf::{CodedInputStream, Message};
use crate::source_file_proto::{Loader, SourceFileProto, SourceKind};
use std::{collections::BTreeMap, fmt::Write, fs, ops::Range, path::Path};

fn message(input: &mut CodedInputStream<'_>, offset: usize) -> Option<Range<usize>> {
    let size = usize::try_from(input.read_raw_varint32().ok()?).ok()?;
    let start = offset + input.get_total_bytes_read();
    input.read_raw_bytes(size).ok()?;
    Some(start..start + size)
}

fn index(bytes: &[u8]) -> Option<String> {
    let mut list = CodedInputStream::new_instance(bytes);
    if list.read_tag().ok()? != 10 {
        return None;
    }
    let shard = message(&mut list, 0)?;
    if !list.is_at_end() {
        return None;
    }
    let mut input = CodedInputStream::new_instance(&bytes[shard.clone()]);
    let mut types = None;
    let mut strings = None;
    let mut summary = None;
    let mut sources = None;
    let mut scripts = BTreeMap::new();
    while !input.is_at_end() {
        let tag = input.read_tag().ok()?;
        let span = message(&mut input, shard.start)?;
        match tag {
            10 if types.is_none() => types = Some(span),
            18 if strings.is_none() => strings = Some(span),
            42 if sources.is_none() => sources = Some(span),
            50 if summary.is_none() => summary = Some(span),
            26 | 34 => {
                // LazyAst { script, source_file }. Other fields can carry source-map side effects.
                let mut ast = CodedInputStream::new_instance(&bytes[span.clone()]);
                if ast.read_tag().ok()? != 10 {
                    return None;
                }
                let script = message(&mut ast, span.start)?;
                if ast.read_tag().ok()? != 16 {
                    return None;
                }
                let file = usize::try_from(ast.read_int32().ok()?).ok()?;
                if !ast.is_at_end() || scripts.insert(file, script).is_some() {
                    return None;
                }
            }
            _ => return None,
        }
    }

    let sources = sources?;
    let mut pool = CodedInputStream::new_instance(&bytes[sources.clone()]);
    let mut records = Vec::new();
    let mut by_name = BTreeMap::new();
    while !pool.is_at_end() {
        if pool.read_tag().ok()? != 10 {
            return None;
        }
        let span = message(&mut pool, sources.start)?;
        let file = SourceFileProto::parse_from(&bytes[span.clone()]).ok()?;
        let name = file.get_filename();
        // Canonical path aliases need the general loader's matching/merging rules.
        if name.is_empty()
            || name.contains('\\')
            || name.contains("blaze-out/")
            || name.contains("bazel-out/")
            || ["bin/", "genfiles/", "testlogs/"]
                .iter()
                .any(|p| name.starts_with(p))
            || !matches!(file.source_kind, SourceKind::EXTERN | SourceKind::CODE)
            || matches!(file.loader, Loader::LOADER_NOT_SET)
            || file.num_lines_plus_one < 0
            || file.num_bytes_plus_one < 0
        {
            return None;
        }
        let position = records.len();
        let script = scripts.remove(&(position + 1))?;
        if by_name.insert(name.to_owned(), position).is_some() {
            return None;
        }
        records.push((span, script));
    }
    let synthetic = by_name.get(" [synthetic:externs] ")?;
    if !scripts.is_empty() {
        return None;
    }
    let mut out = format!(
        "Some(Index {{ types: {:?}, strings: {:?}, summary: {:?}, synthetic: {synthetic}, files: &[\n",
        types?, strings?, summary?,
    );
    for (source, script) in records {
        writeln!(out, "({source:?}, {script:?}),").unwrap();
    }
    out.push_str("], names: &[\n");
    for (name, index) in by_name {
        writeln!(out, "({name:?}, {index}),").unwrap();
    }
    out.push_str("] })\n");
    Some(out)
}

pub(super) fn generate(out: &Path) {
    println!("cargo:rerun-if-changed=build/runtime_libraries.rs");
    println!("cargo:rerun-if-changed=src/serialization/protobuf.rs");
    println!("cargo:rerun-if-changed=src/serialization/source_file_proto.rs");
    let resource = closure_resources::jar::get_resource_as_stream(
        "com.google.javascript.jscomp.Compiler",
        "/runtime_libs.typedast",
    )
    .expect("runtime_libs.typedast")
    .read_all_bytes();
    let index = index(resource).unwrap_or_else(|| "None\n".into());
    fs::write(out.join("runtime_library_index.rs"), index).unwrap();
}
