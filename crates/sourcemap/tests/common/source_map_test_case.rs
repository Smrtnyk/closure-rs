/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   test/com/google/debugging/sourcemap/SourceMapTestCase.java.

use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_sourcemap::{
    file_position::FilePosition, source_map_consumer_factory::SourceMapConsumerFactory,
    source_map_supplier::SourceMapSupplier,
};

pub struct SourceMapTestCase {
    validate_columns: bool,
}
pub struct Token {
    token_name: JsString,
    input_name: JsString,
    position: FilePosition,
}
impl Token {
    // port: SourceMapTestCase.Token#Token
    fn new(token_name: JsString, input_name: JsString, position: FilePosition) -> Self {
        Self {
            token_name,
            input_name,
            position,
        }
    }
}
impl Default for SourceMapTestCase {
    // port: SourceMapTestCase#SourceMapTestCase
    fn default() -> Self {
        Self {
            validate_columns: true,
        }
    }
}
impl SourceMapTestCase {
    // port: SourceMapTestCase#disableColumnValidation
    pub fn disable_column_validation(&mut self) {
        self.validate_columns = false;
    }
    // port: SourceMapTestCase#findTokens(Map)
    pub fn find_tokens(inputs: &IndexMap<JsString, JsString>) -> IndexMap<JsString, Token> {
        let mut tokens = IndexMap::<_, _>::default();
        for entry in inputs {
            Self::find_tokens_into(&mut tokens, entry.0, entry.1);
        }
        tokens
    }
    // port: SourceMapTestCase#findTokens(String)
    pub fn find_tokens_in_source(src: &JsString) -> IndexMap<JsString, Token> {
        let mut tokens = IndexMap::<_, _>::default();
        Self::find_tokens_into(&mut tokens, &"".into(), src);
        tokens
    }
    // port: SourceMapTestCase#findTokens(Map,String,String)
    fn find_tokens_into<'a>(
        tokens: &'a mut IndexMap<JsString, Token>,
        input_name: &JsString,
        js: &JsString,
    ) -> &'a mut IndexMap<JsString, Token> {
        let mut current_line = 0;
        let mut position_offset = 0;
        let mut i = 0;
        while i < js.length() {
            let current = js.char_at(i);
            if current == b'\n' as u16 {
                position_offset = i + 1;
                current_line += 1;
                i += 1; // Java for-loop update before continue.
                continue;
            }
            if current == b'_' as u16 && (i as i32) < js.length() as i32 - 5 {
                if js.char_at(i + 1) != b'_' as u16 {
                    i += 1; // Java for-loop update before continue.
                    continue;
                }
                let mut token_name = JsString::from("");
                let mut j = i + 2;
                while j < js.length() {
                    if js.char_at(j) == b'_' as u16 {
                        break;
                    }
                    token_name = token_name.concat(&JsString::from_units(vec![js.char_at(j)]));
                    j += 1;
                }
                if !token_name.is_empty() {
                    let current_position = i - position_offset;
                    let token = Token::new(
                        token_name.clone(),
                        input_name.clone(),
                        FilePosition::new(current_line, current_position as i32),
                    );
                    tokens.insert(token_name, token);
                }
                i = j;
            }
            i += 1;
        }
        tokens
    }
    // port: SourceMapTestCase#check(String,String,String,String)
    pub fn check(
        &self,
        input_name: &JsString,
        input: &JsString,
        output: &JsString,
        source_map_file_content: &JsString,
    ) {
        let mut input_map = IndexMap::<_, _>::default();
        input_map.insert(input_name.clone(), input.clone());
        self.check_inputs(&input_map, output, source_map_file_content);
    }
    // port: SourceMapTestCase#check(Map,String,String)
    pub fn check_inputs(
        &self,
        original_inputs: &IndexMap<JsString, JsString>,
        generated_source: &JsString,
        source_map_file_content: &JsString,
    ) {
        self.check_with_supplier(
            original_inputs,
            generated_source,
            source_map_file_content,
            None,
        );
    }
    // port: SourceMapTestCase#check(Map,String,String,SourceMapSupplier)
    pub fn check_with_supplier(
        &self,
        original_inputs: &IndexMap<JsString, JsString>,
        generated_source: &JsString,
        source_map_file_content: &JsString,
        supplier: Option<&dyn SourceMapSupplier>,
    ) {
        let original_tokens = Self::find_tokens(original_inputs);
        let result_tokens = Self::find_tokens_in_source(generated_source);
        assert_eq!(result_tokens.len(), original_tokens.len());
        let reader =
            SourceMapConsumerFactory::parse_with_supplier(source_map_file_content, supplier)
                .unwrap_or_else(|e| panic!("unexpected exception: {e}"));
        for token in result_tokens.values() {
            let mapping = reader.get_mapping_for_line(
                token.position.get_line() + 1,
                token.position.get_column() + 1,
            );
            assert!(mapping.is_some());
            let mapping = mapping.unwrap();
            let input_token = original_tokens.get(&token.token_name);
            assert!(input_token.is_some());
            let input_token = input_token.unwrap();
            assert_eq!(input_token.input_name, mapping.get_original_file());
            assert_eq!(
                input_token.position.get_line() + 1,
                mapping.get_line_number()
            );
            let mut start = input_token.position.get_column() + 1;
            if input_token.token_name.starts_with("STR") {
                start -= 1;
            }
            if self.validate_columns {
                assert_eq!(mapping.get_column_position(), start);
            }
            if !input_token.token_name.starts_with("STR") {
                assert!(!mapping.get_identifier().is_empty());
            }
            if !mapping.get_identifier().is_empty() {
                assert_eq!(
                    JsString::from("__")
                        .concat(&input_token.token_name)
                        .concat(&"__".into()),
                    mapping.get_identifier()
                );
            }
        }
    }
}
