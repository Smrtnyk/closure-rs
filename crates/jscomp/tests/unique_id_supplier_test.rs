/*
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 9954c6d:
//   test/com/google/javascript/jscomp/UniqueIdSupplierTest.java.

//! Port of `UniqueIdSupplierTest.java`.

use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_input::CompilerInput;
use closure_jscomp::source_file::SourceFile;
use closure_jscomp::unique_id_supplier::UniqueIdSupplier;
use closure_rhino::js_string::JsString;
use closure_rhino::static_source_file::StaticSourceFile;

fn hash_string(name: &str) -> String {
    let file_hash_code = JsString::from(name).hash_code();
    if file_hash_code < 0 {
        format!("m{}", file_hash_code.wrapping_neg())
    } else {
        format!("{file_hash_code}")
    }
}

// port: UniqueIdSupplierTest#testSingleCompilerInputGeneratesUniqueIds
#[test]
fn test_single_compiler_input_generates_unique_ids() {
    let mut unique_id_supplier = UniqueIdSupplier::new();
    let input = CompilerInput::new(SourceFile::from_code("tmp", "function foo() {}"));
    let input_hash_string = hash_string(input.get_source_file().get_name());

    let unique_id1 = unique_id_supplier.get_unique_id(&input);
    let unique_id2 = unique_id_supplier.get_unique_id(&input);
    assert!(!unique_id1.is_empty());
    assert!(!unique_id2.is_empty());
    assert_ne!(unique_id1, unique_id2);
    assert!(unique_id1.contains(&format!("{input_hash_string}$0")));
    assert!(unique_id2.contains(&format!("{input_hash_string}$1")));
}

// port: UniqueIdSupplierTest#testFilePathsHavingSameHashCode_generatesUniqueIds
#[test]
fn test_file_paths_having_same_hash_code_generates_unique_ids() {
    let mut unique_id_supplier = UniqueIdSupplier::new();
    // Strings "FB" and "Ea" generate the same hashcode.
    let input1 = CompilerInput::new(SourceFile::from_code("FB", "function foo() {}"));
    let input2 = CompilerInput::new(SourceFile::from_code("Ea", "function foo() {}"));

    let unique_id1 = unique_id_supplier.get_unique_id(&input1);
    let unique_id2 = unique_id_supplier.get_unique_id(&input2);
    assert!(!unique_id1.is_empty());
    assert!(!unique_id2.is_empty());
    assert_ne!(unique_id1, unique_id2);

    let input_hash_string1 = hash_string(input1.get_source_file().get_name());
    let input_hash_string2 = hash_string(input2.get_source_file().get_name());
    // The hash strings for the two files are the same, still the supplier generates unique IDs
    // for them
    assert_eq!(input_hash_string1, input_hash_string2);
    assert!(unique_id1.contains(&format!("{input_hash_string1}$0")));
    assert!(unique_id2.contains(&format!("{input_hash_string2}$1")));
}

// port: UniqueIdSupplierTest#testMultipleCompilerInputsGenerateUniqueIds
#[test]
fn test_multiple_compiler_inputs_generate_unique_ids() {
    let mut unique_id_supplier = UniqueIdSupplier::new();
    let input1 = CompilerInput::new(SourceFile::from_code("tmp1", "function foo() {}"));
    let input_hash_string1 = hash_string(input1.get_source_file().get_name());

    let input2 = CompilerInput::new(SourceFile::from_code("tmp2", "function foo() {}"));
    let input_hash_string2 = hash_string(input2.get_source_file().get_name());

    let unique_id1 = unique_id_supplier.get_unique_id(&input1);
    let unique_id2 = unique_id_supplier.get_unique_id(&input2);
    assert!(!unique_id1.is_empty());
    assert!(!unique_id2.is_empty());
    assert_ne!(unique_id1, unique_id2);
    assert!(unique_id1.contains(&format!("{input_hash_string1}$0")));
    assert!(unique_id2.contains(&format!("{input_hash_string2}$0")));
}

/// Ensures that unique Ids generated are the same across compiler runs
// port: UniqueIdSupplierTest#testGeneratedIdsAreDeterministicAcrossRuns
#[test]
fn test_generated_ids_are_deterministic_across_runs() {
    let mut compiler1 = Compiler::new();
    let mut compiler2 = Compiler::new();

    let input1 = CompilerInput::new(SourceFile::from_code("tmp", "function foo() {}"));
    let input2 = CompilerInput::new(SourceFile::from_code("tmp", "function foo() {}"));

    let unique_id1 = compiler1.get_unique_id_supplier().get_unique_id(&input1);
    let different_unique_id1 = compiler1.get_unique_id_supplier().get_unique_id(&input1);

    let unique_id2 = compiler2.get_unique_id_supplier().get_unique_id(&input2);

    assert!(!unique_id1.is_empty());
    assert!(!unique_id2.is_empty());
    // different IDs for same input with different compilers
    assert_eq!(unique_id1, unique_id2);
    // different IDs for same input with same compiler
    assert_ne!(unique_id1, different_unique_id1);
}

// port: UniqueIdSupplierTest#testProtoRoundTripPreservesInsertionOrder
#[test]
fn test_proto_round_trip_preserves_insertion_order() {
    let mut unique_id_supplier = UniqueIdSupplier::new();
    let input1 = CompilerInput::new(SourceFile::from_code("fileA.js", ""));
    let input2 = CompilerInput::new(SourceFile::from_code("fileB.js", ""));
    let input3 = CompilerInput::new(SourceFile::from_code("fileC.js", ""));

    let _unused1 = unique_id_supplier.get_unique_id(&input1);
    let _unused2 = unique_id_supplier.get_unique_id(&input2);
    let _unused3 = unique_id_supplier.get_unique_id(&input3);
    let _unused4 = unique_id_supplier.get_unique_id(&input1); // increment counter for fileA

    let protos = unique_id_supplier.to_proto();

    assert_eq!(protos.len(), 3);
    assert_eq!(protos[0].get_hash(), JsString::from("fileA.js").hash_code());
    assert_eq!(protos[0].get_counter(), 2);
    assert_eq!(protos[1].get_hash(), JsString::from("fileB.js").hash_code());
    assert_eq!(protos[1].get_counter(), 1);
    assert_eq!(protos[2].get_hash(), JsString::from("fileC.js").hash_code());
    assert_eq!(protos[2].get_counter(), 1);

    let restored = UniqueIdSupplier::from_proto(&protos);
    assert_eq!(restored.to_proto(), protos);
}
