/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/debugging/sourcemap/SourceMapObjectParser.java.

use crate::{
    gson::{Gson, JsonElement, JsonObject, Target},
    source_map_generator_v3::ExtensionValue,
    source_map_json_lexer::SourceMapJsonLexer,
    source_map_object::SourceMapObject,
    source_map_parse_exception::SourceMapParseException as Error,
    source_map_section::SourceMapSection,
};
use closure_rhino::js_string::JsString;
use indexmap::IndexMap;
pub struct SourceMapObjectParser;
// Java's shared, immutable Gson instance.
#[allow(non_upper_case_globals)]
static gson: Gson = SourceMapObjectParser::init_gson();
impl SourceMapObjectParser {
    // port: SourceMapObjectParser#<clinit>
    const fn init_gson() -> Gson {
        Gson::new()
    }

    // port: SourceMapObjectParser#parse
    pub fn parse(contents: impl Into<JsString>) -> Result<SourceMapObject, Error> {
        let contents = contents.into();
        let mut builder = SourceMapObject::builder();
        let source_map_root = gson.from_json(contents, Target::JsonObject)
            .map_err(|ex| Error::new(JsString::from("JSON parse exception: ").concat(&ex.java_to_string())))?
            .unwrap_or_else(|| panic!("java.lang.NullPointerException: Cannot invoke \"com.google.gson.JsonObject.get(String)\" because \"sourceMapRoot\" is null"));
        let source_map_root = source_map_root.get_as_json_object();
        builder.set_version(Self::required(source_map_root, "version", "getAsInt").get_as_int());
        builder.set_file(Self::get_string_or_null(source_map_root, "file"));
        builder.set_line_count(if source_map_root.has("lineCount") {
            Self::required(source_map_root, "lineCount", "getAsInt").get_as_int()
        } else {
            -1
        });
        builder.set_mappings(Self::get_string_or_null(source_map_root, "mappings"));
        builder.set_source_root(Self::get_string_or_null(source_map_root, "sourceRoot"));
        if source_map_root.has("sections") {
            let mut list_builder = Vec::new();
            for each in &Self::required(source_map_root, "sections", "getAsJsonArray")
                .get_as_json_array()
                .elements
            {
                list_builder.push(Self::build_section(each.get_as_json_object())?);
            }
            builder.set_sections(Some(list_builder));
        }
        builder.set_sources(Self::get_java_string_array(source_map_root.get("sources")));
        builder.set_sources_content(Self::get_java_string_array(
            source_map_root.get("sourcesContent"),
        ));
        builder.set_names(Self::get_java_string_array(source_map_root.get("names")));
        let mut extensions = IndexMap::new();
        for (key, value) in source_map_root.entry_set() {
            if key.starts_with(&"x_".into()) {
                extensions.insert(key.clone(), ExtensionValue::JsonElement(value.clone()));
            }
        }
        builder.set_extensions(extensions);
        Ok(builder.build())
    }

    // port: SourceMapObjectParser#parseFast
    pub fn parse_fast(contents: impl Into<JsString>) -> Result<SourceMapObject, Error> {
        let contents = contents.into();
        let mut builder = SourceMapObject::builder();
        builder.set_line_count(-1);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), Error> {
                let mut lexer = SourceMapJsonLexer::new(contents);
                lexer.begin_object()?;
                let mut extensions = IndexMap::new();
                while lexer.has_next() {
                    let name = lexer.next_name()?;
                    // Java switches on the complete String. Non-ASCII units cannot match any arm.
                    match name.as_units() {
                        n if n == JsString::from("version").as_units() => {
                            builder.set_version(lexer.next_int()?);
                        }
                        n if n == JsString::from("file").as_units() => {
                            builder.set_file(lexer.next_string_or_null()?);
                        }
                        n if n == JsString::from("lineCount").as_units() => {
                            builder.set_line_count(lexer.next_int()?);
                        }
                        n if n == JsString::from("mappings").as_units() => {
                            builder.set_mappings(lexer.next_string_or_null()?);
                        }
                        n if n == JsString::from("sourceRoot").as_units() => {
                            builder.set_source_root(lexer.next_string_or_null()?);
                        }
                        n if n == JsString::from("sections").as_units() => {
                            let raw_sections = lexer.next_raw_value()?;
                            let sections_array = gson
                                .from_json(raw_sections, Target::JsonArray)
                                .unwrap_or_else(|ex| {
                                    crate::JavaException::throw("", &ex.java_to_string(), "")
                                })
                                .unwrap();
                            let mut list_builder = Vec::new();
                            for each in &sections_array.get_as_json_array().elements {
                                list_builder.push(Self::build_section(each.get_as_json_object())?);
                            }
                            builder.set_sections(Some(list_builder));
                        }
                        n if n == JsString::from("sources").as_units() => {
                            builder.set_sources(Self::read_string_array(&mut lexer)?);
                        }
                        n if n == JsString::from("sourcesContent").as_units() => {
                            builder.set_sources_content(Self::read_string_array(&mut lexer)?);
                        }
                        n if n == JsString::from("names").as_units() => {
                            builder.set_names(Self::read_string_array(&mut lexer)?);
                        }
                        _ => {
                            if name.starts_with(&"x_".into()) {
                                let value = gson
                                    .from_json(lexer.next_raw_value()?, Target::JsonElement)
                                    .unwrap_or_else(|ex| {
                                        crate::JavaException::throw("", &ex.java_to_string(), "")
                                    });
                                extensions.insert(
                                    name,
                                    ExtensionValue::JsonElement(value.unwrap_or(JsonElement::Null)),
                                );
                            } else {
                                lexer.skip_value()?;
                            }
                        }
                    }
                    lexer.check_comma()?;
                }
                lexer.end_object()?;
                lexer.skip_whitespace();
                if lexer.pos < lexer.length {
                    return Err(Error::new("Unexpected trailing characters"));
                }
                builder.set_extensions(extensions);
                Ok(())
            }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(ex)) => {
                return Err(Error::new(
                    JsString::from("Parse exception: ").concat(&ex.java_to_string()),
                ));
            }
            Err(ex) => {
                return Err(Error::new(
                    JsString::from("Parse exception: ").concat(&crate::panic_message(ex)),
                ));
            }
        }
        Ok(builder.build())
    }

    // port: SourceMapObjectParser#buildSection
    fn build_section(section: &JsonObject) -> Result<SourceMapSection, Error> {
        let offset = Self::required(section, "offset", "getAsJsonObject").get_as_json_object();
        let line = Self::required(offset, "line", "getAsInt").get_as_int();
        let column = Self::required(offset, "column", "getAsInt").get_as_int();
        if section.has("map") && section.has("url") {
            return Err(Error::new(
                "Invalid map format: section may not have both 'map' and 'url'",
            ));
        } else if section.has("url") {
            return Ok(SourceMapSection::for_url(
                Self::required(section, "url", "getAsString").get_as_string(),
                line,
                column,
            ));
        } else if section.has("map") {
            let map = section.get("map").unwrap();
            let map_str = if map.is_json_primitive() && map.get_as_json_primitive().is_string() {
                map.get_as_string()
            } else {
                map.to_js_string()
            };
            return Ok(SourceMapSection::for_map(map_str, line, column));
        }
        Err(Error::new(
            "Invalid map format: section must have either 'map' or 'url'",
        ))
    }

    // port: SourceMapObjectParser#getStringOrNull
    fn get_string_or_null(object: &JsonObject, key: impl Into<JsString>) -> Option<JsString> {
        let key = key.into();
        if object.has(&key) && !object.get(&key).unwrap().is_json_null() {
            Some(object.get(&key).unwrap().get_as_string())
        } else {
            None
        }
    }

    // Preserve the Java indexed result assignment.
    #[allow(clippy::needless_range_loop)]
    // port: SourceMapObjectParser#getJavaStringArray
    fn get_java_string_array(element: Option<&JsonElement>) -> Option<Vec<Option<JsString>>> {
        if element.is_none() || element.unwrap().is_json_null() {
            return None;
        }
        let array = element.unwrap().get_as_json_array();
        let len = array.size();
        let mut result = vec![None; len];
        for i in 0..len {
            let item = array.get(i);
            result[i] = if item.is_json_null() {
                None
            } else {
                Some(item.get_as_string())
            };
        }
        Some(result)
    }

    // port: SourceMapObjectParser#readStringArray
    fn read_string_array(
        lexer: &mut SourceMapJsonLexer,
    ) -> Result<Option<Vec<Option<JsString>>>, Error> {
        lexer.skip_whitespace();
        if lexer.pos < lexer.length
            && crate::java_string::starts_with(&lexer.json, &"null".into(), lexer.pos as i32)
        {
            lexer.pos += 4;
            return Ok(None);
        }
        let mut list = Vec::new();
        lexer.begin_array()?;
        while lexer.has_next() {
            list.push(lexer.next_string_or_null()?);
            lexer.check_comma()?;
        }
        lexer.end_array()?;
        Ok(Some(list))
    }

    // Trivial nullable-reference dereference preserving the JVM's exception text.
    fn required<'a>(obj: &'a JsonObject, key: &str, method: &str) -> &'a JsonElement {
        obj.get(key).unwrap_or_else(|| panic!("java.lang.NullPointerException: Cannot invoke \"com.google.gson.JsonElement.{method}()\" because the return value of \"com.google.gson.JsonObject.get(String)\" is null"))
    }

    // port: SourceMapObjectParser#SourceMapObjectParser
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }
}
