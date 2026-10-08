/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/TypedAstDeserializer.java.

//! Port of serialization/TypedAstDeserializer.java: deserializes a list of TypedAst protos into
//! the JSCompiler AST structure.
#![allow(clippy::wrong_self_convention, clippy::unnecessary_unwrap)] // Java method names and control flow
use super::color_pool::{self, Builder as ColorPoolBuilder, ColorPool, ShardView};
use super::protobuf::{CodedInputStream, InvalidProtocolBufferException, WireFormat};
use super::script_node_deserializer::ScriptNodeDeserializer;
use super::string_pool::StringPool;
use super::typed_ast_proto::{LazyAst, TypedAst, TypedAstList};
use crate::compiler::{AstSupplier, Compiler};
use crate::source_file::SourceFile;
use crate::source_map_input::SourceMapInput;
use crate::source_map_resolver::SourceMapResolver;
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::check_argument;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_colors::color_registry::ColorRegistry;
use closure_rhino::node::{NodeId, ObjectProp};
use closure_rhino::static_source_file::StaticSourceFile;
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

/// Java's `ConcurrentMap<SourceFile, Supplier<Node>>`: keyed by the SourceFile's identity (its
/// `Arc` address), keeping the SourceFile for `getKey()`.
pub type TypedAstFilesystem = IndexMap<usize, (Arc<SourceFile>, AstSupplier)>;

/// Rust-only: the identity key of a SourceFile in a `TypedAstFilesystem` (and in the compiler's
/// `typedAstFilesystem`).
pub fn source_file_key(file: &SourceFile) -> usize {
    std::ptr::from_ref(file) as usize
}

/// port: TypedAstDeserializer.Mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    RUNTIME_LIBRARY_ONLY,
    FULL_AST,
}

/// port: TypedAstDeserializer
pub struct TypedAstDeserializer<'a> {
    mode: Mode,
    synthetic_externs: Arc<SourceFile>,
    color_pool_builder: Option<&'a mut ColorPoolBuilder>,

    /// Identity set of the required SourceFiles.
    required_input_files: Option<IndexSet<usize>>,
    file_pool_builder: IndexMap<String, Arc<SourceFile>>,
    canonical_file_pool: IndexMap<String, Arc<SourceFile>>,
    typed_ast_filesystem: TypedAstFilesystem,
    extern_properties: IndexSet<JsString>,
    runtime_libraries: IndexSet<String>,
    synthetic_externs_deserializers: Vec<Arc<ScriptNodeDeserializer>>,
}

impl<'a> TypedAstDeserializer<'a> {
    // port: TypedAstDeserializer#<init>
    fn new(
        synthetic_externs: Arc<SourceFile>,
        existing_color_pool: Option<&'a mut ColorPoolBuilder>,
        required_input_files: Option<IndexSet<usize>>,
        mode: Mode,
        include_type_information: bool,
        owned_color_pool: &'a mut Option<ColorPoolBuilder>,
    ) -> Self {
        let color_pool_builder = if include_type_information {
            Some(match existing_color_pool {
                Some(existing) => existing,
                None => owned_color_pool.insert(ColorPool::builder()),
            })
        } else {
            None
        };
        Self {
            synthetic_externs,
            mode,
            color_pool_builder,
            required_input_files,
            file_pool_builder: IndexMap::new(),
            canonical_file_pool: IndexMap::new(),
            typed_ast_filesystem: IndexMap::new(),
            extern_properties: IndexSet::new(),
            runtime_libraries: IndexSet::new(),
            synthetic_externs_deserializers: Vec::new(),
        }
    }
}

impl TypedAstDeserializer<'_> {
    /// Transforms a given TypedAst delimited stream into a compiler AST
    ///
    /// `required_input_files`: All SourceFiles that should go into the typedAstFilesystem. If a
    /// TypedAst contains a file not in this set, that file will not be added to the
    /// typedAstFilesystem to avoid wasting memory. `include_type_information`: Whether to
    /// deserialize the "Typed" half of a "TypedAst". If false ignores the TypePool, any
    /// TypePointers on AstNodes, and does not create a ColorRegistry.
    /// `resolve_source_map_annotations`: Whether to create and register a SourceMapInput for a
    /// given `//# sourceMappingURL` (stored in LazyAst) when deserializing.
    /// `parse_inline_source_maps`: Whether a given `//# sourceMappingURL` should be registered as
    /// a Base64 encoded source map.
    // port: TypedAstDeserializer#deserializeFullAst
    #[allow(clippy::too_many_arguments)]
    pub fn deserialize_full_ast(
        compiler: &mut Compiler,
        synthetic_externs: Arc<SourceFile>,
        required_input_files: &[Arc<SourceFile>],
        typed_asts_stream: &[u8],
        include_type_information: bool,
        resolve_source_map_annotations: bool,
        parse_inline_source_maps: bool,
    ) -> DeserializedAst {
        let mut source_files_by_name: IndexMap<String, Arc<SourceFile>> = IndexMap::new();
        for file in required_input_files {
            let previous = source_files_by_name.insert(file.get_name().to_string(), file.clone());
            check_argument!(
                previous.is_none(),
                "Multiple entries with same key: %s",
                file.get_name()
            );
        }
        Self::deserialize(
            compiler,
            synthetic_externs,
            Some(required_input_files),
            source_files_by_name,
            None,
            typed_asts_stream,
            Mode::FULL_AST,
            include_type_information,
            resolve_source_map_annotations,
            parse_inline_source_maps,
        )
    }

    /// Transforms the special runtime library TypedAst
    ///
    /// `color_pool`: a ColorPool.Builder holding the colors on the full AST. We want to merge
    /// these colors with the runtime library colors to allow injecting runtime libraries without
    /// re-typechecking them.
    // port: TypedAstDeserializer#deserializeRuntimeLibraries
    pub fn deserialize_runtime_libraries(
        compiler: &mut Compiler,
        synthetic_externs: Arc<SourceFile>,
        color_pool: Option<&mut ColorPoolBuilder>,
        typed_asts_stream: &[u8],
        resolve_source_map_annotations: bool,
        parse_inline_source_maps: bool,
    ) -> DeserializedAst {
        let include_type_information = color_pool.is_some();
        Self::deserialize(
            compiler,
            synthetic_externs,
            // we don't a priori know the SourceFiles corresponding to the runtime libraries like
            // we do for normal CompilerInputs
            None,
            IndexMap::new(),
            color_pool,
            typed_asts_stream,
            Mode::RUNTIME_LIBRARY_ONLY,
            include_type_information,
            resolve_source_map_annotations,
            parse_inline_source_maps,
        )
    }

    // port: TypedAstDeserializer#deserialize
    #[allow(clippy::too_many_arguments)]
    fn deserialize(
        compiler: &mut Compiler,
        synthetic_externs: Arc<SourceFile>,
        required_input_files: Option<&[Arc<SourceFile>]>,
        script_source_files: IndexMap<String, Arc<SourceFile>>,
        color_pool: Option<&mut ColorPoolBuilder>,
        typed_ast_stream: &[u8],
        mode: Mode,
        include_type_information: bool,
        resolve_source_map_annotations: bool,
        parse_inline_source_maps: bool,
    ) -> DeserializedAst {
        check_argument!(
            color_pool.is_some()
                == (mode == Mode::RUNTIME_LIBRARY_ONLY && include_type_information),
            "ColorPool.Builder required iff deserializing runtime libraries & including types"
        );

        let mut owned_color_pool: Option<ColorPoolBuilder> = None;
        let mut deserializer = TypedAstDeserializer::new(
            synthetic_externs.clone(),
            color_pool,
            required_input_files.map(|files| files.iter().map(|f| source_file_key(f)).collect()),
            mode,
            include_type_information,
            &mut owned_color_pool,
        );
        deserializer.add_to_file_pool(synthetic_externs.get_name(), synthetic_externs.clone());
        for (name, file) in script_source_files {
            deserializer.add_to_file_pool(&name, file);
        }

        if mode != Mode::RUNTIME_LIBRARY_ONLY {
            // skip this step if deserializing the runtime libraries to avoid an infinite loop
            // runtime library initialization needs an unbuilt ColorPool.Builder
            compiler
                .init_runtime_library_typed_asts(deserializer.color_pool_builder.as_deref_mut());
        }

        Self::deserialize_typed_asts(
            typed_ast_stream,
            &mut deserializer,
            compiler,
            resolve_source_map_annotations,
            parse_inline_source_maps,
        );

        let synthetic_externs_deserializer = SyntheticExternsDeserializer::new(
            synthetic_externs.clone(),
            Arc::new(std::mem::take(
                &mut deserializer.synthetic_externs_deserializers,
            )),
        );
        let supplier: AstSupplier = Arc::new(move |compiler: &mut Compiler| {
            synthetic_externs_deserializer.deserialize(compiler)
        });
        deserializer.typed_ast_filesystem.insert(
            source_file_key(&synthetic_externs),
            (synthetic_externs, supplier),
        );

        deserializer.to_deserialized_ast()
    }

    // port: TypedAstDeserializer#toDeserializedAst
    fn to_deserialized_ast(self) -> DeserializedAst {
        let registry =
            if self.mode == Mode::RUNTIME_LIBRARY_ONLY || self.color_pool_builder.is_none() {
                None
            } else {
                Some(
                    self.color_pool_builder
                        .unwrap()
                        .build()
                        .get_registry()
                        .clone(),
                )
            };
        DeserializedAst::create(
            self.typed_ast_filesystem,
            registry,
            self.extern_properties,
            self.runtime_libraries,
        )
    }

    // port: TypedAstDeserializer#deserializeTypedAst
    fn deserialize_typed_ast(
        &mut self,
        typed_ast_proto: &TypedAst,
        compiler: &mut Compiler,
        resolve_source_map_annotations: bool,
        parse_inline_source_maps: bool,
    ) {
        let file_shard = Arc::new(self.to_file_shard(typed_ast_proto));

        if self.mode == Mode::FULL_AST {
            // Exclude any TypedAST shards passed to the compiler that don't contain any actual
            // sources we care about. For example: if we're passed "--js=a.js --js=b.js", and see a
            // TypedAST only containing ["c.js", "synthetic:externs"], it's a drain on performance
            // to include it.
            let mut contains_required_input = false;
            for lazy_ast in typed_ast_proto
                .get_extern_ast_list()
                .iter()
                .chain(typed_ast_proto.get_code_ast_list())
            {
                let file = &file_shard[(lazy_ast.get_source_file() - 1) as usize];
                if self
                    .required_input_files
                    .as_ref()
                    .unwrap()
                    .contains(&source_file_key(file))
                {
                    contains_required_input = true;
                    break;
                }
            }
            if !contains_required_input {
                return;
            }
        }

        let runtime_library_list = typed_ast_proto.get_runtime_library_to_inject_list();
        for runtime_library in runtime_library_list {
            self.runtime_libraries.insert(runtime_library.clone());
        }

        // TODO(b/248351234): can we avoid some of this work if the shard only contains weak
        // srcs? one risk: could checks passes synthesize new externProperties even for weak srcs?
        let string_shard = Arc::new(StringPool::from_proto(typed_ast_proto.get_string_pool()));
        let color_shard: Option<Arc<ShardView>> = self.color_pool_builder.as_mut().map(|builder| {
            let type_pool = if typed_ast_proto.has_type_pool() {
                Arc::new(typed_ast_proto.get_type_pool().clone())
            } else {
                color_pool::default_type_pool()
            };
            builder.add_shard(type_pool, string_shard.clone())
        });
        for &x in typed_ast_proto
            .get_externs_summary()
            .get_prop_name_ptr_list()
        {
            self.extern_properties.insert(string_shard.get(x));
        }

        for lazy_ast in typed_ast_proto
            .get_extern_ast_list()
            .iter()
            .chain(typed_ast_proto.get_code_ast_list())
        {
            self.init_lazy_ast_deserializer(
                lazy_ast,
                &file_shard,
                &color_shard,
                &string_shard,
                compiler,
                resolve_source_map_annotations,
                parse_inline_source_maps,
            );
        }
    }

    // port: TypedAstDeserializer#addToFilePool
    fn add_to_file_pool(&mut self, filename: &str, file: Arc<SourceFile>) {
        self.file_pool_builder
            .insert(filename.to_string(), file.clone());
        let canonical_path = SourceFile::canonicalize_path_for_matching(filename);
        if canonical_path != filename {
            self.canonical_file_pool
                .entry(canonical_path)
                .or_insert(file);
        }
    }

    // port: TypedAstDeserializer#findExistingFile
    fn find_existing_file(&self, filename: &str) -> Option<Arc<SourceFile>> {
        if let Some(exact) = self.file_pool_builder.get(filename) {
            return Some(exact.clone());
        }
        let canonical_path = SourceFile::canonicalize_path_for_matching(filename);
        if let Some(canonical) = self.canonical_file_pool.get(&canonical_path) {
            return Some(canonical.clone());
        }
        self.file_pool_builder.get(&canonical_path).cloned()
    }

    // port: TypedAstDeserializer#toFileShard
    fn to_file_shard(&mut self, typed_ast_proto: &TypedAst) -> Vec<Arc<SourceFile>> {
        let mut file_shard_builder: Vec<Arc<SourceFile>> = Vec::new();
        let protos = typed_ast_proto
            .get_source_file_pool()
            .get_source_file_list();
        for p in protos {
            if let Some(existing_file) = self.find_existing_file(p.get_filename()) {
                // Merge the existing SourceFile with the information serialized in the TypedAST.
                existing_file.restore_cached_state_from(p);
                file_shard_builder.push(existing_file);
            } else {
                let proto_file = Arc::new(SourceFile::from_proto(p));
                self.add_to_file_pool(p.get_filename(), proto_file.clone());
                file_shard_builder.push(proto_file);
            }
        }
        file_shard_builder
    }

    // port: TypedAstDeserializer#initLazyAstDeserializer
    #[allow(clippy::too_many_arguments)]
    fn init_lazy_ast_deserializer(
        &mut self,
        lazy_ast: &LazyAst,
        file_shard: &Arc<Vec<Arc<SourceFile>>>,
        color_shard: &Option<Arc<ShardView>>,
        string_shard: &Arc<StringPool>,
        compiler: &mut Compiler,
        resolve_source_map_annotations: bool,
        parse_inline_source_maps: bool,
    ) {
        let file = file_shard[(lazy_ast.get_source_file() - 1) as usize].clone();

        if let Some(required_input_files) = &self.required_input_files
            && !required_input_files.contains(&source_file_key(&file))
            // Synthetic externs bypass the normal dependency system and are always included in a
            // compilation.
            && !Arc::ptr_eq(&self.synthetic_externs, &file)
        {
            return;
        }

        if file.is_weak() {
            let key = source_file_key(&file);
            self.typed_ast_filesystem.entry(key).or_insert_with(|| {
                (
                    file.clone(),
                    Self::create_stub_weak_script_node(file.clone()),
                )
            });
            return;
        }

        let deserializer = Arc::new(ScriptNodeDeserializer::new(
            lazy_ast,
            string_shard.clone(),
            color_shard.clone(),
            file_shard.clone(),
        ));

        if Arc::ptr_eq(&self.synthetic_externs, &file) {
            self.synthetic_externs_deserializers
                .push(deserializer.clone());
        } else {
            let key = source_file_key(&file);
            let supplier_deserializer = deserializer.clone();
            self.typed_ast_filesystem.entry(key).or_insert_with(|| {
                let supplier: AstSupplier = Arc::new(move |compiler: &mut Compiler| {
                    supplier_deserializer.deserialize_new(compiler)
                });
                (file.clone(), supplier)
            });
        }

        Self::add_input_source_map(
            &deserializer,
            compiler,
            resolve_source_map_annotations,
            parse_inline_source_maps,
        );
    }

    /// While deserializing a TypedAST script, if it has a corresponding sourceMappingURL, create
    /// a SourceMapInput and register it.
    // port: TypedAstDeserializer#addInputSourceMap
    fn add_input_source_map(
        deserializer: &ScriptNodeDeserializer,
        compiler: &mut Compiler,
        resolve_source_map_annotations: bool,
        parse_inline_source_maps: bool,
    ) {
        let source_file = deserializer.get_source_file();
        let source_mapping_url = deserializer.get_source_mapping_url(); // This is the encoded source map.

        if !source_mapping_url.is_empty() && resolve_source_map_annotations {
            // base64EncodedSourceMap adds "data:application/json;base64," prefix to the
            // sourceMappingURL
            let base64_encoded_source_map =
                SourceMapResolver::add_base64_prefix_to_encoded_source_map(source_mapping_url);
            let source_map_source_file = SourceMapResolver::extract_source_map(
                source_file,
                &base64_encoded_source_map,
                parse_inline_source_maps,
            );
            if let Some(source_map_source_file) = source_map_source_file {
                compiler.add_input_source_map(
                    source_file.get_name(),
                    SourceMapInput::new(Arc::new(source_map_source_file)),
                );
            }
        }
    }

    /// Adds a stub deserializer for weak files to the typed ast files system.
    ///
    /// Weak files don't actually need to be deserialized. They're only needed for typechecking
    /// and by definition a TypedAST has already been typechecked.
    // port: TypedAstDeserializer#createStubWeakScriptNode
    fn create_stub_weak_script_node(file: Arc<SourceFile>) -> AstSupplier {
        Arc::new(move |compiler: &mut Compiler| {
            let stub_script = IR::script(compiler);
            let file: Arc<dyn StaticSourceFile> = file.clone();
            stub_script.set_static_source_file(compiler, Some(file));
            stub_script.put_prop(
                compiler,
                NodeId::FEATURE_SET,
                Some(ObjectProp::Opaque(Arc::new(FeatureSet::BARE_MINIMUM))),
            );
            stub_script
        })
    }

    // port: TypedAstDeserializer#deserializeTypedAsts
    fn deserialize_typed_asts(
        typed_asts_stream: &[u8],
        deserializer: &mut TypedAstDeserializer<'_>,
        compiler: &mut Compiler,
        resolve_source_map_annotations: bool,
        parse_inline_source_maps: bool,
    ) {
        let result: Result<(), InvalidProtocolBufferException> = (|| {
            let mut coded_input = CodedInputStream::new_instance(typed_asts_stream);
            // The typedAstsStream is an encoded 'TypedAst.List' message:
            //  message TypedAst {
            //    // (other fields)
            //   message List {
            //     repeated TypedAst typed_asts = 1;
            //   }
            // }
            // We could use the Java proto API to create a TypedAst.List object from this stream.
            // However, in some compiler modes the TypedAst.List may contain thousands of TypedAst
            // objects, and pulling them all into memory at once is unnecessarily expensive.
            // Instead we read a single TypedAst object at a time from the stream.
            let mut typed_ast_builder = TypedAst::new_builder();
            while !coded_input.is_at_end() {
                let tag = coded_input.read_tag()?;
                if WireFormat::get_tag_field_number(tag)
                    != TypedAstList::TYPED_ASTS_FIELD_NUMBER as u32
                    || WireFormat::get_tag_wire_type(tag) != WireFormat::WIRETYPE_LENGTH_DELIMITED
                {
                    return Err(InvalidProtocolBufferException::new(format!(
                        "Unexpected field number {} or wire type {}",
                        WireFormat::get_tag_field_number(tag),
                        WireFormat::get_tag_wire_type(tag)
                    )));
                }
                coded_input.read_message(&mut typed_ast_builder)?;
                let typed_ast = std::mem::take(&mut typed_ast_builder).build();
                typed_ast_builder.clear();
                coded_input.reset_size_counter();
                deserializer.deserialize_typed_ast(
                    &typed_ast,
                    compiler,
                    resolve_source_map_annotations,
                    parse_inline_source_maps,
                );
            }
            Ok(())
        })();
        if let Err(ex) = result {
            panic!("Cannot read from TypedAST input stream: {ex}");
        }
    }
}

/// port: TypedAstDeserializer.SyntheticExternsDeserializer
///
/// Helper class that is a lazy deserializer for the synthetic externs script
///
/// The "checks" phase of JSCompiler may synthesize new extern nodes. This means that if we are
/// merging multiple TypedAST shards from different independent "checks" phases, each shard may
/// have its own synthetic externs script. This class concatenates each synthetic externs script
/// into one.
///
/// Note: while it's possible that other files appear in multiple TypedAST shards, we assume that
/// those files are identical, and arbitrarily choose one copy of each file. The synthetic externs
/// are the only case where we concatenate multiple versions of the same file rather than just
/// choosing one.
struct SyntheticExternsDeserializer {
    synthetic_externs: Arc<SourceFile>,
    synthetic_externs_deserializers: Arc<Vec<Arc<ScriptNodeDeserializer>>>,
}

impl SyntheticExternsDeserializer {
    // port: TypedAstDeserializer.SyntheticExternsDeserializer#<init>
    fn new(
        synthetic_externs: Arc<SourceFile>,
        synthetic_externs_deserializers: Arc<Vec<Arc<ScriptNodeDeserializer>>>,
    ) -> Self {
        Self {
            synthetic_externs,
            synthetic_externs_deserializers,
        }
    }

    // port: TypedAstDeserializer.SyntheticExternsDeserializer#deserialize
    fn deserialize(&self, compiler: &mut Compiler) -> NodeId {
        let script = IR::script(compiler);
        let file: Arc<dyn StaticSourceFile> = self.synthetic_externs.clone();
        script.set_static_source_file(compiler, Some(file));
        for d in self.synthetic_externs_deserializers.iter() {
            let deserialized = d.deserialize_new(compiler);
            let children = deserialized.remove_children(compiler);
            script.add_children_to_back(compiler, children);
        }
        script
    }
}

/// port: TypedAstDeserializer.DeserializedAst
///
/// The result of deserializing a TypedAst.List
pub struct DeserializedAst {
    filesystem: TypedAstFilesystem,
    color_registry: Option<Arc<ColorRegistry>>,
    extern_properties: IndexSet<JsString>,
    runtime_libraries: IndexSet<String>,
}

impl DeserializedAst {
    /// Maps from SourceFile to a lazy deserializer of the SCRIPT node for that file
    ///
    /// The supplier creates a new Node whenever called (but the results should be .equals)
    // port: TypedAstDeserializer.DeserializedAst#getFilesystem
    pub fn get_filesystem(&self) -> &TypedAstFilesystem {
        &self.filesystem
    }

    /// Rust-only: moves the filesystem out (Java hands the same map object to the compiler).
    pub fn take_filesystem(&mut self) -> TypedAstFilesystem {
        std::mem::take(&mut self.filesystem)
    }

    /// The built ColorRegistry.
    ///
    /// Note that this is absent if either a) deserializeRuntimeLibraries was called, as this sort
    /// of deserialization does not build a complete AST + colors, just a shard of it, or b) type
    /// information was not requested.
    // port: TypedAstDeserializer.DeserializedAst#getColorRegistry
    pub fn get_color_registry(&self) -> Option<&Arc<ColorRegistry>> {
        self.color_registry.as_ref()
    }

    /// Returns a list of all known extern properties, including properties that were present in
    /// type annotations in source code but not serialized on the AST
    // port: TypedAstDeserializer.DeserializedAst#getExternProperties
    pub fn get_extern_properties(&self) -> &IndexSet<JsString> {
        &self.extern_properties
    }

    // port: TypedAstDeserializer.DeserializedAst#getRuntimeLibraries
    pub fn get_runtime_libraries(&self) -> &IndexSet<String> {
        &self.runtime_libraries
    }

    // port: TypedAstDeserializer.DeserializedAst#create
    fn create(
        filesystem: TypedAstFilesystem,
        color_registry: Option<Arc<ColorRegistry>>,
        extern_properties: IndexSet<JsString>,
        runtime_libraries: IndexSet<String>,
    ) -> Self {
        Self {
            filesystem,
            color_registry,
            extern_properties,
            runtime_libraries,
        }
    }
}
