/*
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/GatherGetterAndSetterPropertiesTest.java.

//! Port of the `GatherGetterAndSetterPropertiesTest.java` test that has no corpus record. The
//! other tests replay from corpus/unit/records (GatherGetterAndSetterPropertiesTest, 33 records).
use closure_jscomp::accessor_summary::PropertyAccessKind;
use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_options::CompilerOptions;
use closure_jscomp::compiler_pass::CompilerPass;
use closure_jscomp::gather_getter_and_setter_properties::GatherGetterAndSetterProperties;
use closure_rhino::js_string::JsString;

// port: GatherGetterAndSetterPropertiesTest#noAssumePropertiesAreStaticallyAnalyzable
#[test]
fn no_assume_properties_are_statically_analyzable() {
    let mut options = CompilerOptions::new();
    options.set_assume_properties_are_statically_analyzable(false);
    let mut compiler = Compiler::new();
    compiler.init(&[], &[], options);

    let externs = compiler.get_externs_root().unwrap();
    let root = compiler.get_js_root().unwrap();
    GatherGetterAndSetterProperties::new().process(&mut compiler, externs, root);

    assert_eq!(
        compiler
            .get_accessor_summary()
            .unwrap()
            .get_kind(&JsString::from("dne")),
        PropertyAccessKind::GETTER_AND_SETTER
    );
}
