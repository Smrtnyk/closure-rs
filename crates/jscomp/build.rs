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
use closure_rhino::{simple_source_file::SimpleSourceFile, static_source_file::SourceKind};
use std::{fmt::Write, fs, path::PathBuf, sync::Arc};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let mut lookup = String::from("match name {\n");
    for (i, file) in AbstractCommandLineRunner::get_builtin_externs(Environment::BROWSER)
        .into_iter()
        .enumerate()
    {
        let source = Arc::new(SimpleSourceFile::new(&file.path, SourceKind::EXTERN));
        if let Some(bytes) =
            closure_parsing::extern_snapshot::generate(source, file.get_code().into())
        {
            let filename = format!("extern-{i}.bin");
            fs::write(out.join(&filename), bytes).unwrap();
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
}
