/*
 * Copyright 2005 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/RenamePropertiesTest.java.

//! RenamePropertiesTest methods that drive the Compiler directly instead of the CompilerTestCase
//! harness. Every other RenamePropertiesTest method replays as a corpus record
//! (crates/testing, RenamePropertiesTest.jsonl.gz).
use closure_jscomp::{
    Compiler, compiler_options::CompilerOptions, js_chunk::JSChunk,
    property_renaming_policy::PropertyRenamingPolicy, source_file::SourceFile,
};
use std::sync::Arc;

// port: RenamePropertiesTest#compileChunks
fn compile_chunks(externs: &str, chunks: &[JSChunk]) -> Compiler {
    let externs_input = Arc::new(SourceFile::from_code("externs", externs));

    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_property_renaming(PropertyRenamingPolicy::ALL_UNQUOTED);

    let mut compiler = Compiler::new();
    let _unused = compiler.compile_chunks(&[externs_input], chunks.to_vec(), options);
    compiler
}

// port: RenamePropertiesTest#testChunks
#[test]
fn test_chunks() {
    let chunk1_js = "function Bar(){} Bar.prototype.getA=function(x){};\n\
                     var foo;foo.getA(foo);foo.doo=foo;foo.bloo=foo;\n";

    let chunk2_js = "function Far(){} Far.prototype.getB=function(y){};\n\
                     var too;too.getB(too);too.woo=too;too.bloo=too;\n";

    let chunk3_js = "function Car(){} Car.prototype.getC=function(z){};\n\
                     var noo;noo.getC(noo);noo.zoo=noo;noo.cloo=noo;\n";

    let chunk1 = JSChunk::new("m1");
    chunk1.add_source_file(SourceFile::from_code("input1", chunk1_js));

    let chunk2 = JSChunk::new("m2");
    chunk2.add_source_file(SourceFile::from_code("input2", chunk2_js));

    let chunk3 = JSChunk::new("m3");
    chunk3.add_source_file(SourceFile::from_code("input3", chunk3_js));

    let chunks = [chunk1.clone(), chunk2.clone(), chunk3.clone()];
    let mut compiler = compile_chunks("", &chunks);

    let result = compiler.get_result();
    assert!(result.success);

    assert_eq!(
        compiler.to_source_for_chunk(&chunk1).to_string(),
        "function Bar(){}Bar.prototype.b=function(x){};\
         var foo;foo.b(foo);foo.f=foo;foo.a=foo;"
    );

    assert_eq!(
        compiler.to_source_for_chunk(&chunk2).to_string(),
        "function Far(){}Far.prototype.c=function(y){};\
         var too;too.c(too);too.g=too;too.a=too;"
    );

    // Note that properties that occur most often globally get the earliest
    // names. The "getC" property, which doesn't occur until chunk 3, is
    // renamed to an earlier name in the alphabet than "woo", which appears
    // in chunk 2, because "getC" occurs more total times across all chunks.
    // Might be better to give early chunks the shortest names, but this is
    // how the pass currently works.
    assert_eq!(
        compiler.to_source_for_chunk(&chunk3).to_string(),
        "function Car(){}Car.prototype.d=function(z){};\
         var noo;noo.d(noo);noo.h=noo;noo.e=noo;"
    );
}
