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

//! The built-in runtime carries one shared type/string shard and many independent scripts.
//! Install the shard at the usual time, then materialize only files that a script references.
use super::color_pool::{Builder, ShardView};
use super::protobuf::Message;
use super::script_node_deserializer::ScriptNodeDeserializer;
use super::source_file_proto::SourceFileProto;
use super::string_pool::StringPool;
use super::typed_ast_deserializer::TypedAstDeserializer;
use super::typed_ast_proto::{ExternsSummary, StringPoolProto};
use super::types_proto::TypePool;
use crate::compiler::{AstSupplier, Compiler};
use crate::source_file::SourceFile;
use closure_rhino::ir::IR;
use closure_rhino::static_source_file::StaticSourceFile;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

struct Index {
    types: Range<usize>,
    strings: Range<usize>,
    summary: Range<usize>,
    synthetic: usize,
    files: &'static [(Range<usize>, Range<usize>)],
    names: &'static [(&'static str, usize)],
}

static INDEX: Option<Index> = include!(concat!(env!("OUT_DIR"), "/runtime_library_index.rs"));

pub(super) struct IndexedFiles {
    index: &'static Index,
    bytes: &'static [u8],
    files: Box<[OnceLock<Arc<SourceFile>>]>,
}

impl IndexedFiles {
    fn proto(&self, position: usize) -> SourceFileProto {
        SourceFileProto::parse_from(&self.bytes[self.index.files[position].0.clone()]).unwrap()
    }

    pub(super) fn get(&self, position: usize) -> &Arc<SourceFile> {
        self.files[position].get_or_init(|| Arc::new(SourceFile::from_proto(&self.proto(position))))
    }
}

pub(crate) struct RuntimeLibraries(Libraries);

enum Libraries {
    Indexed {
        files: Arc<IndexedFiles>,
        strings: Arc<StringPool>,
        colors: Option<Arc<ShardView>>,
    },
    General(closure_rhino::fast_hash::IndexMap<String, AstSupplier>),
}

impl RuntimeLibraries {
    pub(crate) fn new(
        compiler: &mut Compiler,
        synthetic: Arc<SourceFile>,
        colors: Option<&mut Builder>,
    ) -> Self {
        Self::load(compiler, synthetic, colors, INDEX.as_ref())
    }

    fn load(
        compiler: &mut Compiler,
        synthetic: Arc<SourceFile>,
        colors: Option<&mut Builder>,
        index: Option<&'static Index>,
    ) -> Self {
        let resolve_maps = compiler.get_options().get_resolve_source_map_annotations();
        let parse_inline_maps = compiler.get_options().get_parse_inline_source_maps();
        let bytes = closure_resources::jar::get_resource_as_stream(
            "com.google.javascript.jscomp.Compiler",
            "/runtime_libs.typedast",
        )
        .expect("runtime_libs.typedast")
        .read_all_bytes();
        if let Some(index) = index {
            let files = Arc::new(IndexedFiles {
                index,
                bytes,
                files: (0..index.files.len()).map(|_| OnceLock::new()).collect(),
            });
            synthetic.restore_cached_state_from(&files.proto(index.synthetic));
            assert!(files.files[index.synthetic].set(synthetic).is_ok());
            let strings = Arc::new(StringPool::from_proto(
                &StringPoolProto::parse_from(&bytes[index.strings.clone()]).unwrap(),
            ));
            let colors = colors.map(|builder| {
                let pool = TypePool::parse_from(&bytes[index.types.clone()]).unwrap();
                builder.add_shard(Arc::new(pool), strings.clone())
            });
            // The original loader interns the extern properties as it registers the shard.
            for &name in ExternsSummary::parse_from(&bytes[index.summary.clone()])
                .unwrap()
                .get_prop_name_ptr_list()
            {
                strings.get(name);
            }
            Self(Libraries::Indexed {
                files,
                strings,
                colors,
            })
        } else {
            let ast = TypedAstDeserializer::deserialize_runtime_libraries(
                compiler,
                synthetic,
                colors,
                bytes,
                resolve_maps,
                parse_inline_maps,
            );
            let mut scripts = closure_rhino::fast_hash::IndexMap::default();
            for (file, supplier) in ast.get_filesystem().values() {
                scripts
                    .entry(file.get_name().to_owned())
                    .or_insert_with(|| supplier.clone());
            }
            Self(Libraries::General(scripts))
        }
    }

    pub(crate) fn supplier(&self, name: &str) -> Option<AstSupplier> {
        match &self.0 {
            Libraries::General(scripts) => scripts.get(name).cloned(),
            Libraries::Indexed {
                files,
                strings,
                colors,
            } => {
                let index = files.index;
                let position = index
                    .names
                    .binary_search_by_key(&name, |&(name, _)| name)
                    .ok()?;
                let file = index.names[position].1;
                let script = ScriptNodeDeserializer::from_runtime(
                    &files.bytes[index.files[file].1.clone()],
                    file,
                    strings.clone(),
                    colors.clone(),
                    files.clone(),
                );
                if file == index.synthetic {
                    let source = files.get(file).clone();
                    Some(Arc::new(move |compiler| {
                        // The general filesystem presents synthetic externs as a concatenated SCRIPT.
                        let root = IR::script(compiler);
                        root.set_static_source_file(compiler, Some(source.clone()));
                        let children = script.deserialize_new(compiler).remove_children(compiler);
                        root.add_children_to_back(compiler, children);
                        root
                    }))
                } else {
                    Some(Arc::new(move |compiler| script.deserialize_new(compiler)))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_options::CompilerOptions;
    use crate::serialization::color_pool::ColorPool;

    #[test]
    fn indexed_and_general_loaders_materialize_the_same_runtime() {
        let Some(index) = &INDEX else { return };
        for typed in [false, true] {
            let load = |index| {
                let mut compiler = Compiler::new();
                compiler.init_options(CompilerOptions::new());
                let synthetic = compiler.synthetic_externs_file();
                let mut colors = ColorPool::builder();
                let libs = RuntimeLibraries::load(
                    &mut compiler,
                    synthetic,
                    typed.then_some(&mut colors),
                    index,
                );
                colors.build();
                (compiler, libs)
            };
            let (mut eager, general) = load(None);
            let (mut lazy, indexed) = load(Some(index));
            for &(name, _) in index.names {
                let original = general.supplier(name).unwrap()(&mut eager);
                let script = indexed.supplier(name).unwrap()(&mut lazy);
                assert_eq!(
                    original.to_string_tree_utf16(&eager),
                    script.to_string_tree_utf16(&lazy),
                    "{name}"
                );
                let again = indexed.supplier(name).unwrap()(&mut lazy);
                assert_ne!(script, again);
                let file = script.get_static_source_file(&lazy).unwrap();
                assert!(Arc::ptr_eq(
                    &file,
                    &again.get_static_source_file(&lazy).unwrap()
                ));
                assert!(!Arc::ptr_eq(
                    &file,
                    &original.get_static_source_file(&eager).unwrap()
                ));
            }
        }
    }
}
