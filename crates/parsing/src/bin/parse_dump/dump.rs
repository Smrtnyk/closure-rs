/*
 * Copyright 2026 The closure-rs Authors.
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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/src/com/google/javascript/jscomp/ParseDump.java.

use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::{
        JSDocInfo, Marker, NamePosition, PerFileClosureUnawareMode, StringPosition, TypePosition,
        Visibility,
    },
    node::{Ast, NodeId, ObjectProp, Prop, PropValue},
    non_jsdoc_comment::NonJSDocComment,
};
use serde_json::{Map, Value, json};
use std::sync::Arc;

use super::source_file::SourceFile;

pub struct Dump<'a> {
    pub ast: &'a Ast,
    pub source: &'a SourceFile,
}

// port: ParseDump#jsString
pub fn js_string(s: &JsString) -> Value {
    match String::from_utf16(s.as_units()) {
        Ok(text) => Value::String(text),
        Err(_) => json!({"utf16": s.as_units()}),
    }
}

// port: ParseDump#featureSet
pub fn feature_set(fs: FeatureSet) -> Value {
    let mut names: Vec<_> = fs
        .get_features()
        .into_iter()
        .map(|f| format!("{f:?}"))
        .collect();
    names.sort();
    json!(names)
}

impl Dump<'_> {
    // port: ParseDump#node
    pub fn node(&self, n: NodeId) -> Value {
        let mut o = Map::new();
        let class = n.get_class(self.ast).rsplit(['.', '$']).next().unwrap();
        o.insert(
            "token".into(),
            json!(format!("{:?}", n.get_token(self.ast))),
        );
        o.insert("node_class".into(), json!(class));
        o.insert("lineno".into(), json!(n.get_lineno(self.ast)));
        o.insert("charno".into(), json!(n.get_charno(self.ast)));
        let source_offset = match n.get_static_source_file(self.ast) {
            None => json!(-1),
            Some(_) if n.get_lineno(self.ast) == -1 => json!(-1),
            Some(sf) if sf.get_name() == self.source.name => {
                match self.source.try_get_line_offset(n.get_lineno(self.ast)) {
                    Ok(offset) => json!(offset.wrapping_add(n.get_charno(self.ast))),
                    Err(_) => json!({"$threw": "java.lang.IllegalArgumentException"}),
                }
            }
            Some(sf) => json!(
                sf.get_line_offset(n.get_lineno(self.ast))
                    .wrapping_add(n.get_charno(self.ast))
            ),
        };
        o.insert("source_offset".into(), source_offset);
        o.insert(
            "source_position".into(),
            json!(n.get_source_position(self.ast)),
        );
        o.insert("length".into(), json!(n.get_length(self.ast)));
        o.insert(
            "source_name".into(),
            json!(n.get_source_file_name(self.ast)),
        );
        match class {
            "NumberNode" => {
                let d = n.get_double(self.ast);
                o.insert(
                    "double_bits".into(),
                    json!(format!("0x{:016x}", d.to_bits())),
                );
                o.insert(
                    "double_java".into(),
                    json!(closure_rhino::java_lang::double_to_string(d)),
                );
                // CodePrinter is not linked into this tool; the comparator ignores this field.
                o.insert("double_closure".into(), Value::Null);
            }
            "BigIntNode" => {
                o.insert("bigint".into(), json!(n.get_big_int(self.ast).to_string()));
            }
            "StringNode" => {
                o.insert("string".into(), js_string(&n.get_string(self.ast)));
            }
            "TemplateLiteralSubstringNode" => {
                o.insert("cooked".into(), n.get_cooked_string(self.ast).dump(self));
                o.insert("raw".into(), js_string(&n.get_raw_string(self.ast)));
            }
            _ => {}
        }
        let mut props = Map::new();
        for prop in Prop::VALUES {
            if let Some(p) = n.lookup_property(self.ast, prop) {
                let value = match &p.value {
                    PropValue::Int(value) => json!(value),
                    PropValue::Object(value) => self.object(value),
                };
                props.insert(format!("{prop:?}"), value);
            }
        }
        o.insert("props".into(), Value::Object(props));
        o.insert(
            "children".into(),
            Value::Array(n.children(self.ast).map(|c| self.node(c)).collect()),
        );
        Value::Object(o)
    }

    // port: ParseDump#value
    fn object(&self, v: &ObjectProp) -> Value {
        match v {
            ObjectProp::NonJSDocComment(c) => c.dump(self),
            ObjectProp::JSDocInfo(info) => info.dump(self),
            ObjectProp::StaticSourceFile(sf) => {
                json!({"$class":"StaticSourceFile","name":sf.get_name(),"kind":format!("{:?}",sf.get_kind())})
            }
            ObjectProp::InputId(id) => json!({"$class":"InputId","name":id.get_id_name()}),
            ObjectProp::Node(n) => self.node(*n),
            ObjectProp::Opaque(o) => {
                if let Some(fs) = o.as_ref().as_any().downcast_ref::<FeatureSet>() {
                    feature_set(*fs)
                } else {
                    panic!("unported Node property value: {o}");
                }
            }
            ObjectProp::JSType(_) => panic!("parse_dump does not create JSType properties"),
        }
    }
}

// port: ParseDump#value
pub trait DumpValue {
    fn dump(&self, d: &Dump<'_>) -> Value;
}
impl DumpValue for bool {
    fn dump(&self, _: &Dump<'_>) -> Value {
        json!(self)
    }
}
impl DumpValue for i32 {
    fn dump(&self, _: &Dump<'_>) -> Value {
        json!(self)
    }
}
impl DumpValue for JsString {
    fn dump(&self, _: &Dump<'_>) -> Value {
        js_string(self)
    }
}
impl DumpValue for NodeId {
    fn dump(&self, d: &Dump<'_>) -> Value {
        d.node(*self)
    }
}
impl<T: DumpValue + ?Sized> DumpValue for &T {
    fn dump(&self, d: &Dump<'_>) -> Value {
        (*self).dump(d)
    }
}
impl<T: DumpValue> DumpValue for Arc<T> {
    fn dump(&self, d: &Dump<'_>) -> Value {
        self.as_ref().dump(d)
    }
}
impl<T: DumpValue> DumpValue for Option<T> {
    fn dump(&self, d: &Dump<'_>) -> Value {
        self.as_ref().map_or(Value::Null, |x| x.dump(d))
    }
}
impl<T: DumpValue> DumpValue for Vec<T> {
    fn dump(&self, d: &Dump<'_>) -> Value {
        Value::Array(self.iter().map(|x| x.dump(d)).collect())
    }
}
impl<T: DumpValue> DumpValue for IndexSet<T> {
    fn dump(&self, d: &Dump<'_>) -> Value {
        Value::Array(self.iter().map(|x| x.dump(d)).collect())
    }
}
impl<K: DumpValue, V: DumpValue> DumpValue for IndexMap<K, V> {
    fn dump(&self, d: &Dump<'_>) -> Value {
        json!({"$map": self.iter().map(|(k,v)| vec![k.dump(d),v.dump(d)]).collect::<Vec<_>>()})
    }
}
impl DumpValue for Visibility {
    fn dump(&self, _: &Dump<'_>) -> Value {
        json!(format!("{self:?}"))
    }
}
impl DumpValue for PerFileClosureUnawareMode {
    fn dump(&self, _: &Dump<'_>) -> Value {
        json!(format!("{self:?}"))
    }
}
impl DumpValue for JSTypeExpression {
    // port: ParseDump#value
    fn dump(&self, d: &Dump<'_>) -> Value {
        json!({"$class":"JSTypeExpression","source_name":self.get_source_name(),"root":d.node(self.get_root())})
    }
}
impl DumpValue for NonJSDocComment {
    // port: ParseDump#beanJson
    fn dump(&self, d: &Dump<'_>) -> Value {
        json!({"$class":"com.google.javascript.rhino.NonJSDocComment",
            "getCommentString":self.get_comment_string().dump(d),
            "getEndPosition":self.get_end_position().dump(d),
            "getStartPosition":self.get_start_position().dump(d),
            "isEndingAsLineComment":self.is_ending_as_line_comment(),
            "isInline":self.is_inline()})
    }
}
impl DumpValue for closure_rhino::jscomp_parsing_parser::util::source_position::SourcePosition {
    // port: ParseDump#value
    fn dump(&self, _: &Dump<'_>) -> Value {
        json!({"$class":"com.google.javascript.jscomp.parsing.parser.util.SourcePosition","$toString":self.to_string()})
    }
}

// port: ParseDump#beanJson
fn position<T: DumpValue>(
    p: &closure_rhino::source_position::SourcePosition<T>,
    class: &str,
    d: &Dump<'_>,
) -> Value {
    json!({"$class":class,"getItem":p.get_item().dump(d),"getEndLine":p.get_end_line(),
        "getStartLine":p.get_start_line(),"getPositionOnEndLine":p.get_position_on_end_line(),
        "getPositionOnStartLine":p.get_position_on_start_line()})
}
impl DumpValue for StringPosition {
    fn dump(&self, d: &Dump<'_>) -> Value {
        position(
            self,
            "com.google.javascript.rhino.JSDocInfo$StringPosition",
            d,
        )
    }
}
impl DumpValue for NamePosition {
    fn dump(&self, d: &Dump<'_>) -> Value {
        position(
            self,
            "com.google.javascript.rhino.JSDocInfo$NamePosition",
            d,
        )
    }
}
impl DumpValue for TypePosition {
    fn dump(&self, d: &Dump<'_>) -> Value {
        let mut o = position(
            self,
            "com.google.javascript.rhino.JSDocInfo$TypePosition",
            d,
        );
        o["hasBrackets"] = json!(self.has_brackets());
        o
    }
}
impl DumpValue for Marker {
    // port: ParseDump#beanJson
    fn dump(&self, d: &Dump<'_>) -> Value {
        let annotation = self.get_annotation().map_or(Value::Null, |p| {
            position(
                p,
                "com.google.javascript.rhino.JSDocInfo$TrimmedStringPosition",
                d,
            )
        });
        json!({"$class":"com.google.javascript.rhino.JSDocInfo$Marker","getAnnotation":annotation,
            "getDescription":self.get_description().dump(d),"getNameNode":self.get_name_node().dump(d),"getType":self.get_type().dump(d)})
    }
}

include!("jsdoc_getters.rs");
