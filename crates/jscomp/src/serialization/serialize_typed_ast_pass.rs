/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/SerializeTypedAstPass.java.

//! Port of serialization/SerializeTypedAstPass.java.
use super::fast_gzip_output_stream::FastGzipOutputStream;
use super::protobuf::Message;
use super::serialization_options::SerializationOptions;
use super::typed_ast_proto::{TypedAst, TypedAstList};
use super::typed_ast_serializer::TypedAstSerializer;
use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::remove_cast_nodes::RemoveCastNodes;
use closure_rhino::node::NodeId;
use std::io::Write;
use std::path::PathBuf;

/// port: SerializeTypedAstPass
///
/// A compiler pass intended to serialize the types in the AST.
///
/// Serialization means that the AST is converted to a proto representation and output to a file
/// or stream. Deserialization means the reverse process.
pub struct SerializeTypedAstPass<'a> {
    consumer: Box<dyn FnMut(TypedAst) + 'a>,
    serialization_options: SerializationOptions,
}

impl<'a> SerializeTypedAstPass<'a> {
    // port: SerializeTypedAstPass#<init>
    pub fn new(
        ast_consumer: impl FnMut(TypedAst) + 'a,
        serialization_options: SerializationOptions,
    ) -> Self {
        Self {
            consumer: Box::new(ast_consumer),
            serialization_options,
        }
    }

    /// Serializes a TypedAst to the given output stream.
    ///
    /// Unlike `createFromPath(AbstractCompiler, Path)`, this method does not automatically gzip the
    /// TypedAST. The "out" parameter may or may not already be a GZIPOutputStream.
    // port: SerializeTypedAstPass#createFromOutputStream
    pub fn create_from_output_stream(
        out: &'a mut dyn Write,
        serialization_options: SerializationOptions,
    ) -> Self {
        let to_output_stream = move |ast: TypedAst| {
            let bytes = TypedAstList::new_builder()
                .add_typed_asts(ast)
                .build()
                .to_byte_array();
            if let Err(e) = out.write_all(&bytes) {
                panic!("Cannot write to stream: {e}");
            }
        };
        Self::new(to_output_stream, serialization_options)
    }

    /// Serializes a gzipped TypedAst to the specified outputPath
    // port: SerializeTypedAstPass#createFromPath
    pub fn create_from_path(
        output_path: PathBuf,
        serialization_options: SerializationOptions,
    ) -> Self {
        let to_path = move |ast: TypedAst| {
            let result = (|| -> std::io::Result<()> {
                let file = std::fs::File::create(&output_path)?;
                let mut out = FastGzipOutputStream::new(file)?;
                let bytes = TypedAstList::new_builder()
                    .add_typed_asts(ast)
                    .build()
                    .to_byte_array();
                out.write_all(&bytes)?;
                out.finish()?;
                Ok(())
            })();
            if let Err(e) = result {
                panic!("Cannot create TypedAst output file: {e}");
            }
        };
        Self::new(to_path, serialization_options)
    }
}

impl CompilerPass for SerializeTypedAstPass<'_> {
    // port: SerializeTypedAstPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        RemoveCastNodes::new().process(compiler, externs, root);
        let mut serializer = TypedAstSerializer::new(compiler, self.serialization_options.clone());
        let ast = serializer.serialize_roots(externs, root);
        (self.consumer)(ast);
    }
}
