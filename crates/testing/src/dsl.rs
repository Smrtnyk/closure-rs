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

//! DSL.md "Expressions": the typed model of a descriptor expression.
//!
//! Each expression is a JSON object with exactly one tag key plus that tag's own keys. The tag is
//! the key `ReplayDsl#eval` dispatches on; an object with no tag, with two tags, or with a key that
//! does not belong to its tag is a load error. Evaluation is not part of this crate (no compiler
//! execution); the model only carries what the future harness evaluates.

use crate::json::{JsString, JsonValue};
use crate::reader::{
    ModelResult, Obj, ObjOut, arr, as_array, as_string, err, index, int, js, list_of, map_of, st,
    strs,
};
use closure_rhino::fx_hash::IndexMap;

/// One DSL expression (DSL.md "Expressions").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// A JSON `null` where an expression is expected: `ReplayDsl#eval` yields null for it.
    JsonNull,
    /// `{"int":1}`: 32-bit int.
    Int(i32),
    /// `{"long":"1"}`: 64-bit int, the decimal string as recorded (Java `Long.parseLong`).
    Long(String),
    /// `{"double":"1.5"}`: IEEE double, the string as recorded (Java `Double.parseDouble`).
    Double(String),
    /// `{"bool":true}`
    Bool(bool),
    /// `{"string":"s"}`
    String(JsString),
    /// `{"enum":"FQCN","name":"X"}`
    Enum { class: String, name: String },
    /// `{"null":true}`
    Null,
    /// `{"list":[E]}`
    List(Vec<Expr>),
    /// `{"set":[E]}`
    Set(Vec<Expr>),
    /// `{"map":[[E,E]]}`
    Map(Vec<(Expr, Expr)>),
    /// `{"array":"FQCN","items":[E]}`
    Array { component: String, items: Vec<Expr> },
    /// `{"compiler":true}`
    Compiler,
    /// `{"options":true}`
    Options,
    /// `{"class":"FQCN"}`
    Class(String),
    /// `{"var":"x"}`
    Var(String),
    /// `{"let":[["x",E],...],"in":E}`
    Let {
        bindings: Vec<(String, Expr)>,
        body: Box<Expr>,
    },
    /// `{"do":[E...],"value":E}`
    Do {
        effects: Vec<Expr>,
        value: Box<Expr>,
    },
    /// `{"lambda":["p",...],"iface":"FQCN","body":E}`
    Lambda {
        params: Vec<String>,
        iface: String,
        body: Box<Expr>,
    },
    /// `{"getField":E,"name":"f"}`
    GetField { target: Box<Expr>, name: String },
    /// `{"mutationPoint":"Name","value":E}`
    MutationPoint { name: String, value: Box<Expr> },
    /// `{"field":"name"}` or `{"field":"name","path":"a.b"}`
    Field { name: String, path: Option<String> },
    /// `{"record":"dotted.path"}`
    Record(String),
    /// `{"new":"FQCN","args":[E]}` (`args` absent means no arguments).
    New {
        class: String,
        args: Option<Vec<Expr>>,
    },
    /// `{"static":"FQCN","method":"m","args":[E]}`
    Static {
        class: String,
        method: String,
        args: Option<Vec<Expr>>,
    },
    /// `{"call":E,"method":"m","args":[E]}`
    Call {
        target: Box<Expr>,
        method: String,
        args: Option<Vec<Expr>>,
    },
    /// `{"once":"key","value":E}`
    Once { key: String, value: Box<Expr> },
    /// `{"withFields":E,"fields":{"f":E}}` (fields in JSON key order).
    WithFields {
        target: Box<Expr>,
        fields: IndexMap<String, Expr>,
    },
    /// `{"sequence":[E]}`
    Sequence(Vec<Expr>),
    /// `{"helper":"Holder.Inner","package":"p","outer":["f"],"outerInstance":E,"args":[E]}`
    Helper {
        name: String,
        package: Option<String>,
        outer: Option<Vec<String>>,
        outer_instance: Option<Box<Expr>>,
        args: Option<Vec<Expr>>,
    },
}

/// The tag keys, in the order `ReplayDsl#eval` tests them.
pub const TAGS: [&str; 29] = [
    "int",
    "long",
    "double",
    "bool",
    "string",
    "enum",
    "null",
    "list",
    "set",
    "map",
    "array",
    "compiler",
    "options",
    "class",
    "var",
    "let",
    "do",
    "lambda",
    "getField",
    "mutationPoint",
    "field",
    "record",
    "new",
    "static",
    "call",
    "once",
    "withFields",
    "sequence",
    "helper",
];

/// Java `Long.parseLong` acceptance: optional sign, then decimal digits, within 64 bits.
fn is_java_long(s: &str) -> bool {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) && s.parse::<i64>().is_ok()
}

/// Java `Double.parseDouble` acceptance for the decimal forms (plus `NaN`/`Infinity` and a
/// trailing `d`/`f` type suffix); surrounding whitespace is trimmed as Java does.
fn is_java_double(s: &str) -> bool {
    let t = s.trim_matches(|c: char| c <= ' ');
    let unsigned = t.strip_prefix(['+', '-']).unwrap_or(t);
    if unsigned == "NaN" || unsigned == "Infinity" {
        return true;
    }
    let body = unsigned
        .strip_suffix(['d', 'D', 'f', 'F'])
        .unwrap_or(unsigned);
    !body.is_empty()
        && body
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
        && body.bytes().any(|b| b.is_ascii_digit())
        && body.parse::<f64>().is_ok()
}

fn exprs(v: &JsonValue, path: &str) -> ModelResult<Vec<Expr>> {
    list_of(v, path, Expr::from_json)
}

fn exprs_to_json(items: &[Expr]) -> JsonValue {
    arr(items, Expr::to_json)
}

fn boxed(o: &mut Obj<'_>, key: &str) -> ModelResult<Box<Expr>> {
    let p = o.sub(key);
    Ok(Box::new(Expr::from_json(o.req(key)?, &p)?))
}

fn opt_args(o: &mut Obj<'_>) -> ModelResult<Option<Vec<Expr>>> {
    let p = o.sub("args");
    o.opt("args").map(|v| exprs(v, &p)).transpose()
}

/// `{"compiler":true}` / `{"options":true}` / `{"null":true}`: the value must be `true`.
fn req_true(o: &mut Obj<'_>, key: &str) -> ModelResult<()> {
    if o.req_bool(key)? {
        Ok(())
    } else {
        err(&o.sub(key), "expected true")
    }
}

impl Expr {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<Expr> {
        if v.is_null() {
            return Ok(Expr::JsonNull);
        }
        let mut o = Obj::new(v, path)?;
        let tags: Vec<&str> = TAGS.iter().copied().filter(|t| o.has(t)).collect();
        let tag = match tags.as_slice() {
            [t] => *t,
            [] => return err(path, "expression has no tag key"),
            many => return err(path, format!("expression has several tag keys {many:?}")),
        };
        let e = match tag {
            "int" => {
                let p = o.sub("int");
                let n = o.req("int")?;
                match n.as_number() {
                    Some(num) if num.is_integral_literal() => match num.as_i32() {
                        Some(i) => Expr::Int(i),
                        None => return err(&p, format!("not a 32-bit int: {}", num.0)),
                    },
                    _ => return err(&p, "expected an integer literal"),
                }
            }
            "long" => {
                let s = o.req_string("long")?;
                if !is_java_long(&s) {
                    return err(&o.sub("long"), format!("not a Java long: {s:?}"));
                }
                Expr::Long(s)
            }
            "double" => {
                let s = o.req_string("double")?;
                if !is_java_double(&s) {
                    return err(&o.sub("double"), format!("not a Java double: {s:?}"));
                }
                Expr::Double(s)
            }
            "bool" => Expr::Bool(o.req_bool("bool")?),
            "string" => Expr::String(o.req_js_string("string")?),
            "enum" => Expr::Enum {
                class: o.req_string("enum")?,
                name: o.req_string("name")?,
            },
            "null" => {
                req_true(&mut o, "null")?;
                Expr::Null
            }
            "list" => {
                let p = o.sub("list");
                Expr::List(exprs(o.req("list")?, &p)?)
            }
            "set" => {
                let p = o.sub("set");
                Expr::Set(exprs(o.req("set")?, &p)?)
            }
            "map" => {
                let p = o.sub("map");
                Expr::Map(list_of(o.req("map")?, &p, |kv, kp| {
                    match as_array(kv, kp)?.as_slice() {
                        [k, v] => Ok((
                            Expr::from_json(k, &index(kp, 0))?,
                            Expr::from_json(v, &index(kp, 1))?,
                        )),
                        _ => err(kp, "a map entry must be [key, value]"),
                    }
                })?)
            }
            "array" => {
                let component = o.req_string("array")?;
                let p = o.sub("items");
                Expr::Array {
                    component,
                    items: exprs(o.req("items")?, &p)?,
                }
            }
            "compiler" => {
                req_true(&mut o, "compiler")?;
                Expr::Compiler
            }
            "options" => {
                req_true(&mut o, "options")?;
                Expr::Options
            }
            "class" => Expr::Class(o.req_string("class")?),
            "var" => Expr::Var(o.req_string("var")?),
            "let" => {
                let p = o.sub("let");
                let bindings = list_of(o.req("let")?, &p, |b, bp| {
                    match as_array(b, bp)?.as_slice() {
                        [n, e] => Ok((
                            as_string(n, &index(bp, 0))?,
                            Expr::from_json(e, &index(bp, 1))?,
                        )),
                        _ => err(bp, "a let binding must be [name, expr]"),
                    }
                })?;
                Expr::Let {
                    bindings,
                    body: boxed(&mut o, "in")?,
                }
            }
            "do" => {
                let p = o.sub("do");
                Expr::Do {
                    effects: exprs(o.req("do")?, &p)?,
                    value: boxed(&mut o, "value")?,
                }
            }
            "lambda" => {
                let p = o.sub("lambda");
                Expr::Lambda {
                    params: list_of(o.req("lambda")?, &p, as_string)?,
                    iface: o.req_string("iface")?,
                    body: boxed(&mut o, "body")?,
                }
            }
            "getField" => Expr::GetField {
                target: boxed(&mut o, "getField")?,
                name: o.req_string("name")?,
            },
            "mutationPoint" => Expr::MutationPoint {
                name: o.req_string("mutationPoint")?,
                value: boxed(&mut o, "value")?,
            },
            "field" => Expr::Field {
                name: o.req_string("field")?,
                path: o.opt_string("path")?,
            },
            "record" => Expr::Record(o.req_string("record")?),
            "new" => Expr::New {
                class: o.req_string("new")?,
                args: opt_args(&mut o)?,
            },
            "static" => Expr::Static {
                class: o.req_string("static")?,
                method: o.req_string("method")?,
                args: opt_args(&mut o)?,
            },
            "call" => Expr::Call {
                target: boxed(&mut o, "call")?,
                method: o.req_string("method")?,
                args: opt_args(&mut o)?,
            },
            "once" => Expr::Once {
                key: o.req_string("once")?,
                value: boxed(&mut o, "value")?,
            },
            "withFields" => {
                let target = boxed(&mut o, "withFields")?;
                let p = o.sub("fields");
                Expr::WithFields {
                    target,
                    fields: map_of(o.req("fields")?, &p, Expr::from_json)?,
                }
            }
            "sequence" => {
                let p = o.sub("sequence");
                Expr::Sequence(exprs(o.req("sequence")?, &p)?)
            }
            "helper" => {
                let name = o.req_string("helper")?;
                let package = o.opt_string("package")?;
                let op = o.sub("outer");
                let outer = o
                    .opt("outer")
                    .map(|v| list_of(v, &op, as_string))
                    .transpose()?;
                let ip = o.sub("outerInstance");
                let outer_instance = o
                    .opt("outerInstance")
                    .map(|v| Expr::from_json(v, &ip).map(Box::new))
                    .transpose()?;
                Expr::Helper {
                    name,
                    package,
                    outer,
                    outer_instance,
                    args: opt_args(&mut o)?,
                }
            }
            _ => unreachable!("every TAGS entry is handled"),
        };
        o.finish()?;
        Ok(e)
    }

    /// The tag key of this expression (`None` for [`Expr::JsonNull`]).
    pub fn tag(&self) -> Option<&'static str> {
        Some(match self {
            Expr::JsonNull => return None,
            Expr::Int(_) => "int",
            Expr::Long(_) => "long",
            Expr::Double(_) => "double",
            Expr::Bool(_) => "bool",
            Expr::String(_) => "string",
            Expr::Enum { .. } => "enum",
            Expr::Null => "null",
            Expr::List(_) => "list",
            Expr::Set(_) => "set",
            Expr::Map(_) => "map",
            Expr::Array { .. } => "array",
            Expr::Compiler => "compiler",
            Expr::Options => "options",
            Expr::Class(_) => "class",
            Expr::Var(_) => "var",
            Expr::Let { .. } => "let",
            Expr::Do { .. } => "do",
            Expr::Lambda { .. } => "lambda",
            Expr::GetField { .. } => "getField",
            Expr::MutationPoint { .. } => "mutationPoint",
            Expr::Field { .. } => "field",
            Expr::Record(_) => "record",
            Expr::New { .. } => "new",
            Expr::Static { .. } => "static",
            Expr::Call { .. } => "call",
            Expr::Once { .. } => "once",
            Expr::WithFields { .. } => "withFields",
            Expr::Sequence(_) => "sequence",
            Expr::Helper { .. } => "helper",
        })
    }

    /// Calls `f` on this expression and every expression nested in it, parents first.
    pub fn visit(&self, f: &mut dyn FnMut(&Expr)) {
        f(self);
        let each = |items: &[Expr], f: &mut dyn FnMut(&Expr)| {
            for x in items {
                x.visit(f);
            }
        };
        match self {
            Expr::List(items) | Expr::Set(items) | Expr::Sequence(items) => each(items, f),
            Expr::Array { items, .. } => each(items, f),
            Expr::Map(entries) => {
                for (k, v) in entries {
                    k.visit(f);
                    v.visit(f);
                }
            }
            Expr::Let { bindings, body } => {
                for (_, e) in bindings {
                    e.visit(f);
                }
                body.visit(f);
            }
            Expr::Do { effects, value } => {
                each(effects, f);
                value.visit(f);
            }
            Expr::Lambda { body, .. } => body.visit(f),
            Expr::GetField { target, .. } => target.visit(f),
            Expr::MutationPoint { value, .. } | Expr::Once { value, .. } => value.visit(f),
            Expr::New { args, .. } | Expr::Static { args, .. } => {
                each(args.as_deref().unwrap_or(&[]), f)
            }
            Expr::Call { target, args, .. } => {
                target.visit(f);
                each(args.as_deref().unwrap_or(&[]), f);
            }
            Expr::WithFields { target, fields } => {
                target.visit(f);
                for e in fields.values() {
                    e.visit(f);
                }
            }
            Expr::Helper {
                outer_instance,
                args,
                ..
            } => {
                each(args.as_deref().unwrap_or(&[]), f);
                if let Some(e) = outer_instance {
                    e.visit(f);
                }
            }
            Expr::JsonNull
            | Expr::Int(_)
            | Expr::Long(_)
            | Expr::Double(_)
            | Expr::Bool(_)
            | Expr::String(_)
            | Expr::Enum { .. }
            | Expr::Null
            | Expr::Compiler
            | Expr::Options
            | Expr::Class(_)
            | Expr::Var(_)
            | Expr::Field { .. }
            | Expr::Record(_) => {}
        }
    }

    pub fn to_json(&self) -> JsonValue {
        let opt_args = |o: &mut ObjOut, args: &Option<Vec<Expr>>| {
            o.put_opt("args", args.as_deref().map(exprs_to_json));
        };
        let mut o = ObjOut::new();
        match self {
            Expr::JsonNull => return JsonValue::Null,
            Expr::Int(i) => {
                o.put("int", int(*i));
            }
            Expr::Long(s) => {
                o.put("long", st(s));
            }
            Expr::Double(s) => {
                o.put("double", st(s));
            }
            Expr::Bool(b) => {
                o.put("bool", JsonValue::Bool(*b));
            }
            Expr::String(s) => {
                o.put("string", js(s));
            }
            Expr::Enum { class, name } => {
                o.put("enum", st(class)).put("name", st(name));
            }
            Expr::Null => {
                o.put("null", JsonValue::Bool(true));
            }
            Expr::List(items) => {
                o.put("list", exprs_to_json(items));
            }
            Expr::Set(items) => {
                o.put("set", exprs_to_json(items));
            }
            Expr::Map(entries) => {
                o.put(
                    "map",
                    arr(entries, |(k, v)| {
                        JsonValue::Array(vec![k.to_json(), v.to_json()])
                    }),
                );
            }
            Expr::Array { component, items } => {
                o.put("array", st(component))
                    .put("items", exprs_to_json(items));
            }
            Expr::Compiler => {
                o.put("compiler", JsonValue::Bool(true));
            }
            Expr::Options => {
                o.put("options", JsonValue::Bool(true));
            }
            Expr::Class(c) => {
                o.put("class", st(c));
            }
            Expr::Var(v) => {
                o.put("var", st(v));
            }
            Expr::Let { bindings, body } => {
                o.put(
                    "let",
                    arr(bindings, |(n, e)| {
                        JsonValue::Array(vec![st(n), e.to_json()])
                    }),
                )
                .put("in", body.to_json());
            }
            Expr::Do { effects, value } => {
                o.put("do", exprs_to_json(effects))
                    .put("value", value.to_json());
            }
            Expr::Lambda {
                params,
                iface,
                body,
            } => {
                o.put("lambda", strs(params))
                    .put("iface", st(iface))
                    .put("body", body.to_json());
            }
            Expr::GetField { target, name } => {
                o.put("getField", target.to_json()).put("name", st(name));
            }
            Expr::MutationPoint { name, value } => {
                o.put("mutationPoint", st(name))
                    .put("value", value.to_json());
            }
            Expr::Field { name, path } => {
                o.put("field", st(name))
                    .put_opt("path", path.as_deref().map(st));
            }
            Expr::Record(p) => {
                o.put("record", st(p));
            }
            Expr::New { class, args } => {
                o.put("new", st(class));
                opt_args(&mut o, args);
            }
            Expr::Static {
                class,
                method,
                args,
            } => {
                o.put("static", st(class)).put("method", st(method));
                opt_args(&mut o, args);
            }
            Expr::Call {
                target,
                method,
                args,
            } => {
                o.put("call", target.to_json()).put("method", st(method));
                opt_args(&mut o, args);
            }
            Expr::Once { key, value } => {
                o.put("once", st(key)).put("value", value.to_json());
            }
            Expr::WithFields { target, fields } => {
                let fields: IndexMap<String, JsonValue> = fields
                    .iter()
                    .map(|(k, e)| (k.clone(), e.to_json()))
                    .collect();
                o.put("withFields", target.to_json())
                    .put("fields", JsonValue::Object(fields));
            }
            Expr::Sequence(items) => {
                o.put("sequence", exprs_to_json(items));
            }
            Expr::Helper {
                name,
                package,
                outer,
                outer_instance,
                args,
            } => {
                o.put("helper", st(name))
                    .put_opt("package", package.as_deref().map(st))
                    .put_opt("outer", outer.as_deref().map(strs))
                    .put_opt(
                        "outerInstance",
                        outer_instance.as_deref().map(Expr::to_json),
                    );
                opt_args(&mut o, args);
            }
        }
        o.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse_json;

    fn rt(text: &str) -> Expr {
        let v = parse_json(text).unwrap();
        let e = Expr::from_json(&v, "$").unwrap();
        assert_eq!(e.to_json(), v, "{text}");
        e
    }

    #[test]
    fn tags_and_round_trip() {
        assert_eq!(rt(r#"{"int":-3}"#), Expr::Int(-3));
        assert_eq!(rt(r#"{"long":"-12"}"#), Expr::Long("-12".into()));
        rt(r#"{"double":"1.5"}"#);
        rt(r#"{"call":{"compiler":true},"method":"getOptions"}"#);
        rt(r#"{"let":[["x",{"null":true}]],"in":{"var":"x"}}"#);
        rt(
            r#"{"helper":"H.I","outer":["f"],"outerInstance":{"once":"h","value":{"helper":"H"}},"args":[]}"#,
        );
        rt(r#"{"map":[[{"string":"k"},{"bool":false}]]}"#);
        rt(r#"{"withFields":{"new":"A"},"fields":{"b":{"int":1},"a":null}}"#);
    }

    #[test]
    fn rejects_bad_shapes() {
        for bad in [
            r#"{}"#,
            r#"{"int":1,"long":"1"}"#,
            r#"{"int":1.5}"#,
            r#"{"int":2147483648}"#,
            r#"{"long":"1x"}"#,
            r#"{"double":"x"}"#,
            r#"{"compiler":false}"#,
            r#"{"new":"A","extra":1}"#,
            r#"{"map":[[{"int":1}]]}"#,
        ] {
            let v = parse_json(bad).unwrap();
            assert!(Expr::from_json(&v, "$").is_err(), "{bad}");
        }
    }
}
