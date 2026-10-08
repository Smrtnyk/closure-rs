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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/testing/TestExternsBuilder.java.

use closure_testing::{
    json::parse_json,
    replay::replay_dsl::{DslValue, NativeObject},
    testing::test_externs_builder::TestExternsBuilder,
};
// port: TestExternsBuilder#build / add* (pinned Java text and dependency ordering)
#[test]
fn all_extern_sections_match_java() {
    let golden = parse_json(include_str!("data/test_externs_builder.json")).unwrap();
    for (name, expected) in golden.as_object().unwrap() {
        let mut builder = TestExternsBuilder::new();
        match name.as_str() {
            "empty" => {}
            "extra" => {
                builder.add_extra(&["var a;".into(), "var b;".into()]);
            }
            "minimal" => {
                builder
                    .add_array()
                    .add_iterable()
                    .add_object()
                    .add_undefined()
                    .add_function()
                    .add_string();
            }
            "default" => {
                builder
                    .add_array()
                    .add_iterable()
                    .add_object()
                    .add_undefined()
                    .add_function()
                    .add_string()
                    .add_promise()
                    .add_closure_externs()
                    .add_i_template_array();
            }
            name => {
                assert!(matches!(
                    builder.call(name, vec![]).unwrap(),
                    DslValue::Null
                ));
            }
        }
        assert_eq!(
            builder.build().as_units(),
            expected.as_js_string().unwrap().0.as_slice(),
            "{name}"
        );
    }
}
