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
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/Gson.java,
//   com/google/gson/TypeAdapter.java,
//   com/google/gson/internal/bind/CollectionTypeAdapterFactory.java,
//   com/google/gson/internal/bind/MapTypeAdapterFactory.java,
//   com/google/gson/internal/bind/TypeAdapters.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/diagnostic/LogsGson.java.

use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_sourcemap::gson::stream::json_writer::JsonWriter;

/// A logged Java Object. Gson serializes an Object with the TypeAdapter it selects for the
/// object's runtime type; each Rust type that is logged implements that adapter's `write`.
pub trait LogsGsonObject {
    // port: com.google.gson.TypeAdapter#write
    fn write(&self, out: &mut JsonWriter);
}

/// A conversion to a preferred logging representation.
///
/// When an implementor is logged, the result of this method will be JSON stringified in place
/// of the implementor.
///
/// Use this interface when the default Gson serialization for a class is bad and the class should
/// always be logged another way. Primitive wrappers and entity type are ideal candidates.
pub trait Able {
    // port: LogsGson.Able#toLogsGson
    fn to_logs_gson(&self) -> Box<dyn LogsGsonObject>;
}

/// A Gson instance tailored to generating logs output.
pub struct LogsGson;
impl LogsGson {
    // port: LogsGson#toJson
    pub fn to_json(o: Option<&dyn LogsGsonObject>) -> String {
        Self::gson_to_json(o).to_string_lossy()
    }

    // port: com.google.gson.Gson#toJson(Object)
    // GSON = new GsonBuilder()...create(): Gson's defaults (htmlSafe, no null serialization).
    fn gson_to_json(src: Option<&dyn LogsGsonObject>) -> JsString {
        let mut writer = JsonWriter::new(Vec::new());
        writer.set_html_safe(true);
        writer.set_serialize_nulls(false);
        // Gson#toJson(Object,Type,JsonWriter) writes leniently.
        writer.set_lenient(true);
        match src {
            None => {
                writer.null_value();
            }
            Some(src) => src.write(&mut writer),
        }
        writer.into_string()
    }
}

// registerTypeHierarchyAdapter(Able.class, ...)
impl<T: Able> LogsGsonObject for T {
    // port: LogsGson#GSON.Able adapter#write
    fn write(&self, out: &mut JsonWriter) {
        let value = self.to_logs_gson();
        out.json_value(Some(&LogsGson::gson_to_json(Some(&*value))));
    }
}

/// Guava's Multimap as LogsGson sees it: its `asMap()` view.
pub struct Multimap<K, V> {
    map: IndexMap<K, Vec<V>>,
}
impl<K: std::hash::Hash + Eq, V> Multimap<K, V> {
    pub fn new() -> Self {
        Self {
            map: IndexMap::<_, _>::default(),
        }
    }
    // port: com.google.common.collect.Multimap#put
    pub fn put(&mut self, key: K, value: V) -> bool {
        self.map.entry(key).or_default().push(value);
        true
    }
    // port: com.google.common.collect.Multimap#asMap
    pub fn as_map(&self) -> &IndexMap<K, Vec<V>> {
        &self.map
    }
}
impl<K: std::hash::Hash + Eq, V> Default for Multimap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
// registerTypeHierarchyAdapter(Multimap.class, ...)
impl<K: std::fmt::Display, V: LogsGsonObject> LogsGsonObject for Multimap<K, V> {
    // port: LogsGson#GSON.Multimap adapter#write
    fn write(&self, out: &mut JsonWriter) {
        out.json_value(Some(&LogsGson::gson_to_json(Some(&MapAdapter(&self.map)))));
    }
}

// Gson's built-in adapters for the value types logged in Rust.
impl LogsGsonObject for i32 {
    // port: com.google.gson.internal.bind.TypeAdapters.INTEGER#write
    fn write(&self, out: &mut JsonWriter) {
        out.value_long(i64::from(*self));
    }
}
impl LogsGsonObject for i64 {
    // port: com.google.gson.internal.bind.TypeAdapters.LONG#write
    fn write(&self, out: &mut JsonWriter) {
        out.value_long(*self);
    }
}
impl LogsGsonObject for bool {
    // port: com.google.gson.internal.bind.TypeAdapters.BOOLEAN#write
    fn write(&self, out: &mut JsonWriter) {
        out.value_boolean(*self);
    }
}
impl LogsGsonObject for String {
    // port: com.google.gson.internal.bind.TypeAdapters.STRING#write
    fn write(&self, out: &mut JsonWriter) {
        out.value(Some(JsString::from(self.as_str())));
    }
}
impl LogsGsonObject for &str {
    // port: com.google.gson.internal.bind.TypeAdapters.STRING#write
    fn write(&self, out: &mut JsonWriter) {
        out.value(Some(JsString::from(*self)));
    }
}
impl<T: LogsGsonObject> LogsGsonObject for Option<T> {
    // A null element: Gson's adapters write nullValue() for null.
    fn write(&self, out: &mut JsonWriter) {
        match self {
            None => {
                out.null_value();
            }
            Some(value) => value.write(out),
        }
    }
}
impl LogsGsonObject for Box<dyn LogsGsonObject> {
    fn write(&self, out: &mut JsonWriter) {
        (**self).write(out);
    }
}
impl<T: LogsGsonObject> LogsGsonObject for Vec<T> {
    // port: com.google.gson.internal.bind.CollectionTypeAdapterFactory.Adapter#write
    fn write(&self, out: &mut JsonWriter) {
        out.begin_array();
        for element in self {
            element.write(out);
        }
        out.end_array();
    }
}
impl<K: std::fmt::Display, V: LogsGsonObject> LogsGsonObject for IndexMap<K, V> {
    fn write(&self, out: &mut JsonWriter) {
        MapAdapter(self).write(out);
    }
}
struct MapAdapter<'a, K, V>(&'a IndexMap<K, V>);
impl<K: std::fmt::Display, V: LogsGsonObject> LogsGsonObject for MapAdapter<'_, K, V> {
    // port: com.google.gson.internal.bind.MapTypeAdapterFactory.Adapter#write
    fn write(&self, out: &mut JsonWriter) {
        // complexMapKeySerialization is off: keys are written with String.valueOf(key).
        out.begin_object();
        for (key, value) in self.0 {
            out.name(JsString::from(key.to_string()));
            value.write(out);
        }
        out.end_object();
    }
}

// ColorId implements LogsGson.Able (the class lives in closure-rhino, the interface here).
impl Able for closure_rhino::jscomp_colors::color_id::ColorId {
    // Delegates to the ported ColorId#toLogsGson.
    fn to_logs_gson(&self) -> Box<dyn LogsGsonObject> {
        Box::new(closure_rhino::jscomp_colors::color_id::ColorId::to_logs_gson(self))
    }
}
