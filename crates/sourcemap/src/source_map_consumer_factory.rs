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
//   src/com/google/debugging/sourcemap/SourceMapConsumerFactory.java.

use crate::{
    source_map_consumer_v3::SourceMapConsumerV3, source_map_object_parser::SourceMapObjectParser,
    source_map_parse_exception::SourceMapParseException as Error,
    source_map_supplier::SourceMapSupplier, source_mapping::SourceMapping,
};
use closure_rhino::js_string::JsString;
pub struct SourceMapConsumerFactory;
impl SourceMapConsumerFactory {
    // port: SourceMapConsumerFactory#SourceMapConsumerFactory
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }

    // port: SourceMapConsumerFactory#parse(String)
    pub fn parse(contents: impl Into<JsString>) -> Result<Box<dyn SourceMapping>, Error> {
        Self::parse_with_supplier(contents, None)
    }
    // port: SourceMapConsumerFactory#parse(String,SourceMapSupplier)
    pub fn parse_with_supplier(
        contents: impl Into<JsString>,
        supplier: Option<&dyn SourceMapSupplier>,
    ) -> Result<Box<dyn SourceMapping>, Error> {
        let contents = contents.into();
        if contents.starts_with(&"/** Begin line maps. **/".into()) {
            return Err(Error::new(
                "This appears to be a V1 SourceMap, which is not supported.",
            ));
        } else if contents.starts_with(&"{".into()) {
            let source_map_object = SourceMapObjectParser::parse(contents)?;
            return match source_map_object.get_version() {
                3 => {
                    let mut consumer = SourceMapConsumerV3::new();
                    consumer.parse_object(source_map_object, supplier)?;
                    Ok(Box::new(consumer))
                }
                _ => Err(Error::new(format!(
                    "Unknown source map version:{}",
                    source_map_object.get_version()
                ))),
            };
        }
        Err(Error::new("unable to detect source map format"))
    }
    // port: SourceMapConsumerFactory#parseFast
    pub fn parse_fast(contents: impl Into<JsString>) -> Result<SourceMapConsumerV3, Error> {
        let contents = contents.into();
        if contents.starts_with(&"{".into()) {
            let source_map_object = SourceMapObjectParser::parse_fast(&contents)?;
            assert!(
                source_map_object.get_version() == 3,
                "java.lang.IllegalArgumentException: Unknown source map version:{}",
                source_map_object.get_version()
            );
            let mut consumer = SourceMapConsumerV3::new();
            consumer.parse_object(source_map_object, None)?;
            return Ok(consumer);
        }
        Err(Error::new("unable to detect source map format"))
    }
}
