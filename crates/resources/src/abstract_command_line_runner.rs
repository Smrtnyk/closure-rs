/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AbstractCommandLineRunner.java,
//   src/com/google/javascript/jscomp/SourceFile.java.

//! Port of the builtin-externs loading of `com.google.javascript.jscomp.AbstractCommandLineRunner`
//! (`getBuiltinExterns`, `getExternsInput`). The rest of the class lives in closure-jscomp.

use indexmap::IndexMap;

use crate::compiler_options::Environment;
use crate::default_externs::DefaultExterns;
use crate::jar;
use crate::jar_contents::{ArchiveEntry, EntryContent};

/// The class whose resources `getExternsInput` looks up.
const ABSTRACT_COMMAND_LINE_RUNNER: &str = "com.google.javascript.jscomp.AbstractCommandLineRunner";

/// What Java builds with `SourceFile.builder().withPath(path).withContent(entryStream).build()`;
/// closure-jscomp converts it into its `SourceFile`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternSourceFile {
    pub path: String,
    pub content: &'static [u8],
}

impl ExternSourceFile {
    // port: SourceFile#getName
    pub fn get_name(&self) -> &str {
        &self.path
    }

    /// The code, decoded like `CharStreams.toString(new InputStreamReader(entryStream, UTF_8))`
    /// in `SourceFile.Builder#withContent(InputStream)`.
    // port: SourceFile#getCode
    pub fn get_code(&self) -> String {
        jar::decode_utf8(self.content)
    }
}

pub struct AbstractCommandLineRunner;

impl AbstractCommandLineRunner {
    /// Returns a mutable list.
    // port: AbstractCommandLineRunner#getBuiltinExterns
    pub fn get_builtin_externs(env: Environment) -> Vec<ExternSourceFile> {
        let zip = Self::get_externs_input();
        let env_prefix = format!("{}/", env.to_string().to_ascii_lowercase());
        let mut map_from_externs_zip: IndexMap<String, ExternSourceFile> = IndexMap::new();
        for entry in zip {
            let mut filename: &str = entry.name;

            // Always load externs in the root folder.
            // If the non-core-JS externs are organized in subfolders, only load
            // the ones in a subfolder matching the specified environment. Strip the subfolder.
            if filename.contains('/') {
                if !filename.starts_with(&env_prefix) {
                    continue;
                }
                filename = &filename[env_prefix.len()..]; // remove envPrefix, including '/'
            }

            let entry_stream = entry.content.read_all_bytes();
            map_from_externs_zip.insert(
                filename.to_string(),
                // Give the files an odd prefix, so that they do not conflict
                // with the user's files.
                ExternSourceFile {
                    path: format!("externs.zip//{filename}"),
                    content: entry_stream,
                },
            );
        }
        DefaultExterns::prepare_externs(env, &mut map_from_externs_zip)
    }

    /// The entries of the jar's `externs.zip`, in zip order (what Java reads through a
    /// `ZipInputStream` over this resource).
    // port: AbstractCommandLineRunner#getExternsInput
    fn get_externs_input() -> &'static [ArchiveEntry] {
        let mut input = jar::get_resource_as_stream(ABSTRACT_COMMAND_LINE_RUNNER, "/externs.zip");
        if input.is_none() {
            // In some environments, the externs.zip is relative to this class.
            input = jar::get_resource_as_stream(ABSTRACT_COMMAND_LINE_RUNNER, "externs.zip");
        }
        match input.expect("") {
            EntryContent::Zip(entries) => entries,
            EntryContent::Directory | EntryContent::File(_) => {
                panic!("externs.zip is not a zip archive")
            }
        }
    }
}
