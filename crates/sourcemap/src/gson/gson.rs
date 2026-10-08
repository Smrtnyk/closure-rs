/*
 * Copyright (C) 2008 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * Copyright (C) 2011 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/Gson.java,
//   com/google/gson/internal/bind/TypeAdapters.java.

use super::{
    JsonElement, JsonParseException,
    json_io_exception::JsonIOException,
    json_syntax_exception::JsonSyntaxException,
    stream::json_reader::{JsonReader, JsonToken},
    stream::json_writer::JsonWriter,
};
use closure_rhino::js_string::JsString;

#[derive(Clone, Copy)]
pub enum Target {
    JsonObject,
    JsonArray,
    JsonElement,
}

// The default Gson configuration used by SourceMapObjectParser. Reflection and
// the adapters for other Java types are outside this Gson subset.
pub struct Gson {
    serialize_nulls: bool,
    generate_non_executable_json: bool,
    html_safe: bool,
    pretty_printing: bool,
    lenient: bool,
}
impl Default for Gson {
    fn default() -> Self {
        Self::new()
    }
}
impl Gson {
    const DEFAULT_JSON_NON_EXECUTABLE: bool = false;
    const DEFAULT_LENIENT: bool = false;
    const DEFAULT_PRETTY_PRINT: bool = false;
    const DEFAULT_ESCAPE_HTML: bool = true;
    const DEFAULT_SERIALIZE_NULLS: bool = false;
    const JSON_NON_EXECUTABLE_PREFIX: &'static str = ")]}'\n";

    // port: com.google.gson.Gson#Gson
    pub const fn new() -> Self {
        // The supported tree adapter types require only these default config fields.
        Self {
            serialize_nulls: Self::DEFAULT_SERIALIZE_NULLS,
            generate_non_executable_json: Self::DEFAULT_JSON_NON_EXECUTABLE,
            html_safe: Self::DEFAULT_ESCAPE_HTML,
            pretty_printing: Self::DEFAULT_PRETTY_PRINT,
            lenient: Self::DEFAULT_LENIENT,
        }
    }

    // Preserve Gson's instance method name.
    #[allow(clippy::wrong_self_convention)]
    // port: com.google.gson.Gson#fromJson(String,Class)
    pub fn from_json(
        &self,
        json: impl Into<JsString>,
        class_of_t: Target,
    ) -> Result<Option<JsonElement>, JsonParseException> {
        let object = self.from_json_with_type(json, class_of_t)?;
        // Primitives.wrap(classOfT).cast(object): all three supported targets are
        // reference types; the type hierarchy adapter performs the cast check.
        Ok(object)
    }

    // Preserve Gson's instance method name.
    #[allow(clippy::wrong_self_convention)]
    // port: com.google.gson.Gson#fromJson(String,Type)
    fn from_json_with_type(
        &self,
        json: impl Into<JsString>,
        type_of_t: Target,
    ) -> Result<Option<JsonElement>, JsonParseException> {
        let reader = json.into(); // StringReader, whose storage is a Java String.
        self.from_json_with_reader(reader, type_of_t)
    }

    // Preserve Gson's instance method name.
    #[allow(clippy::wrong_self_convention)]
    // port: com.google.gson.Gson#fromJson(Reader,Type)
    fn from_json_with_reader(
        &self,
        json: JsString,
        type_of_t: Target,
    ) -> Result<Option<JsonElement>, JsonParseException> {
        let mut json_reader = self.new_json_reader(json);
        let object = self.from_json_with_json_reader(&mut json_reader, type_of_t)?;
        Self::assert_full_consumption(object.as_ref(), &mut json_reader)?;
        Ok(object)
    }

    // port: com.google.gson.Gson#assertFullConsumption
    fn assert_full_consumption(
        obj: Option<&JsonElement>,
        reader: &mut JsonReader,
    ) -> Result<(), JsonParseException> {
        let result = (|| {
            if obj.is_some() && reader.peek()? != JsonToken::END_DOCUMENT {
                return Err(JsonSyntaxException::from_message(
                    "JSON document was not fully consumed.",
                ));
            }
            Ok(())
        })();
        result.map_err(|e| {
            if e.class == "com.google.gson.stream.MalformedJsonException" {
                JsonSyntaxException::from_cause(e.java_to_string())
            } else if e.class.starts_with("java.io.") {
                JsonIOException::from_cause(e.java_to_string())
            } else {
                e
            }
        })
    }

    // Preserve Gson's instance method name.
    #[allow(clippy::wrong_self_convention)]
    // port: com.google.gson.Gson#fromJson(JsonReader,Type)
    fn from_json_with_json_reader(
        &self,
        reader: &mut JsonReader,
        type_of_t: Target,
    ) -> Result<Option<JsonElement>, JsonParseException> {
        let mut is_empty = true;
        let old_lenient = reader.is_lenient();
        reader.set_lenient(true);
        let result: Result<Option<JsonElement>, JsonParseException> = (|| {
            reader.peek()?;
            is_empty = false;
            // getAdapter(TypeToken.get(typeOfT)) selects the JsonElement adapter
            // and, for Object/Array targets, its type hierarchy adapter below.
            let object = JsonElementTypeHierarchyAdapter::read(reader, type_of_t)?;
            Ok(Some(object))
        })();
        let result = match result {
            Err(e) if e.class == "java.io.EOFException" => {
                if is_empty {
                    Ok(None)
                } else {
                    Err(JsonSyntaxException::from_cause(e.java_to_string()))
                }
            }
            Err(e)
                if e.class == "java.lang.IllegalStateException"
                    || e.class.starts_with("java.io.")
                    || e.class == "com.google.gson.stream.MalformedJsonException" =>
            {
                Err(JsonSyntaxException::from_cause(e.java_to_string()))
            }
            result => result,
        };
        reader.set_lenient(old_lenient);
        result
    }

    // port: com.google.gson.Gson#toJson(JsonElement)
    pub fn to_json(&self, json_element: &JsonElement) -> JsString {
        let writer = Vec::new(); // StringWriter's WTF-16 storage.
        self.to_json_with_appendable(json_element, writer)
    }

    // port: com.google.gson.Gson#toJson(JsonElement,Appendable)
    fn to_json_with_appendable(&self, json_element: &JsonElement, writer: Vec<u16>) -> JsString {
        let mut json_writer = self.new_json_writer(writer);
        self.to_json_with_json_writer(json_element, &mut json_writer);
        json_writer.into_string()
    }

    // port: com.google.gson.Gson#newJsonWriter
    fn new_json_writer(&self, mut writer: Vec<u16>) -> JsonWriter {
        if self.generate_non_executable_json {
            writer.extend_from_slice(JsString::from(Self::JSON_NON_EXECUTABLE_PREFIX).as_units());
        }
        let mut json_writer = JsonWriter::new(writer);
        if self.pretty_printing {
            json_writer.set_indent("  ");
        }
        json_writer.set_html_safe(self.html_safe);
        json_writer.set_lenient(self.lenient);
        json_writer.set_serialize_nulls(self.serialize_nulls);
        json_writer
    }

    // port: com.google.gson.Gson#newJsonReader
    fn new_json_reader(&self, reader: JsString) -> JsonReader {
        let mut json_reader = JsonReader::new(reader);
        json_reader.set_lenient(self.lenient);
        json_reader
    }

    // port: com.google.gson.Gson#toJson(JsonElement,JsonWriter)
    fn to_json_with_json_writer(&self, json_element: &JsonElement, writer: &mut JsonWriter) {
        let old_lenient = writer.is_lenient();
        writer.set_lenient(true);
        let old_html_safe = writer.is_html_safe();
        writer.set_html_safe(self.html_safe);
        let old_serialize_nulls = writer.get_serialize_nulls();
        writer.set_serialize_nulls(self.serialize_nulls);
        writer.write(json_element);
        writer.set_lenient(old_lenient);
        writer.set_html_safe(old_html_safe);
        writer.set_serialize_nulls(old_serialize_nulls);
        // StringWriter cannot throw IOException, and the tree adapter has no
        // Java assert statements; those catch branches cannot be reached here.
    }
}

struct JsonElementTypeHierarchyAdapter;
impl JsonElementTypeHierarchyAdapter {
    // port: com.google.gson.internal.bind.TypeAdapters#newTypeHierarchyFactory.read
    fn read(
        reader: &mut JsonReader,
        requested_type: Target,
    ) -> Result<JsonElement, JsonParseException> {
        let result = reader.read_element()?;
        let requested_type = match requested_type {
            Target::JsonObject => Some("com.google.gson.JsonObject"),
            Target::JsonArray => Some("com.google.gson.JsonArray"),
            Target::JsonElement => None,
        };
        if let Some(requested_type) = requested_type
            && result.class_name() != requested_type
        {
            let message = JsString::from(format!(
                "Expected a {requested_type} but was {}; at path ",
                result.class_name()
            ))
            .concat(&reader.get_previous_path());
            return Err(JsonSyntaxException::from_message(message));
        }
        Ok(result)
    }
}
