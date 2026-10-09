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
/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CompilerOptions.java.

//! Port of ReplayDsl: left-to-right evaluation with per-record identity and lexical scope.
use crate::{
    dsl::Expr,
    jscomp_api::{
        Compiler, CompilerOptions, CompilerPass, DiagnosticGroup, DiagnosticType, SourceFile,
        WarningsGuard,
    },
    json::JsonValue,
    replay::{
        registry::Registry,
        replay_values::{adapt, decode_json},
    },
    throwable::Throwable,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc, sync::Arc};
pub type CompilerHandle = Rc<RefCell<Compiler>>;
pub type OptionsHandle = Rc<RefCell<CompilerOptions>>;
pub trait NativeObject: std::any::Any {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str;
    // port: UnitRecorder#isInstance
    fn is_instance_of(&self, class: &str) -> bool {
        self.class_name() == class
    }
    // port: ReplayDsl#invoke (native method adapter, also used by recorder accessors)
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        Err(Throwable::Unported(format!(
            "{}#{method}",
            self.class_name()
        )))
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Err(Throwable::Unported(format!("{}#fields", self.class_name())))
    }
    // port: UnitRecorder#dump (values the recorder writes under a dedicated tag, e.g. typedScope)
    fn tagged_dump(&self) -> Result<Option<JsonValue>, Throwable> {
        Ok(None)
    }
    // port: UnitRecorder#dump (a Java lambda: its captured values, `arg$1`.. in capture order;
    // `None` for an object that is not a lambda)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        None
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, _value: DslValue) -> Result<(), Throwable> {
        Err(Throwable::Unported(format!("{}#{name}", self.class_name())))
    }
    // port: ReplayDsl.SequencePass#process (native CompilerPass adapter)
    fn process(
        &mut self,
        _compiler: &mut Compiler,
        _externs: NodeId,
        _root: NodeId,
    ) -> Result<(), Throwable> {
        Err(Throwable::Unported(format!(
            "{}#process",
            self.class_name()
        )))
    }
    // port: ReplayDsl#invoke (native method receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    // port: ReplayDsl.SequencePass#process (native context for factory and DSL callbacks)
    fn process_with_ctx(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
        _ctx: &mut Ctx,
    ) -> Result<(), Throwable> {
        self.process(compiler, externs, root)
    }
    // port: ReplayDsl#invoke (native traversal callback receiver)
    fn as_traversal_callback(
        &mut self,
    ) -> Option<&mut dyn closure_jscomp::node_traversal::Callback> {
        None
    }
    // port: ReplayDsl#invoke (native node visitor receiver)
    fn as_node_visitor(&mut self) -> Option<&mut dyn closure_jscomp::node_util::Visitor> {
        None
    }
    // port: ReplayDsl#invoke (native Send custom-pass receiver)
    fn as_custom_pass(
        &mut self,
    ) -> Option<std::sync::Arc<std::sync::Mutex<dyn CompilerPass + Send>>> {
        None
    }
    // port: ReplayDsl#invoke (native RenamingMap option value, e.g. a CompilerOptions#setIdGenerators
    // map value)
    fn as_renaming_map(
        &mut self,
    ) -> Option<std::sync::Arc<dyn closure_jscomp::renaming_map::RenamingMap + Send + Sync>> {
        None
    }
}
#[derive(Clone)]
pub enum DslValue {
    Typed {
        class: String,
        value: Box<DslValue>,
    },
    Null,
    Int(i32),
    Long(i64),
    Double(f64),
    Bool(bool),
    Char(u16),
    String(JsString),
    Enum {
        class: String,
        name: String,
    },
    Class(String),
    List(Vec<DslValue>),
    Set(Vec<DslValue>),
    Map(Vec<(DslValue, DslValue)>),
    Array {
        component: String,
        items: Vec<DslValue>,
    },
    Optional(Option<Box<DslValue>>),
    CodingConvention(Arc<dyn closure_jscomp::coding_convention::CodingConvention + Send + Sync>),
    NameGenerator(Arc<dyn closure_jscomp::name_generator::NameGenerator + Send + Sync>),
    Compiler(CompilerHandle),
    Options(OptionsHandle),
    OptionsView(CompilerHandle),
    Native(Rc<RefCell<dyn NativeObject>>),
    Object(Rc<RefCell<Object>>),
    Lambda(Rc<Lambda>),
    Pass(Rc<RefCell<Box<dyn CompilerPass>>>),
    Sequence(Vec<DslValue>),
    Node(NodeId),
    PassFactory {
        name: String,
        factory: Rc<Lambda>,
        native: crate::jscomp_api::PassFactory,
        token: std::sync::Arc<crate::replay::native_factories::FactoryToken>,
    },
    DiagnosticType(&'static DiagnosticType),
    DiagnosticGroup(Arc<DiagnosticGroup>),
    WarningsGuard {
        guard: Arc<dyn WarningsGuard>,
        encoding: JsonValue,
    },
    SourceFile(Arc<SourceFile>),
    Regex {
        pattern: JsString,
        flags: i32,
    },
    Proto(crate::value::ProtoMessage),
}
impl std::fmt::Debug for DslValue {
    // port: ReplayDsl#lambda (diagnostic representation)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.class_name())
    }
}
#[derive(Clone, Debug)]
pub struct Object {
    pub class: String,
    pub fields: IndexMap<String, DslValue>,
    pub field_types: IndexMap<String, String>,
}
#[derive(Clone, Debug)]
pub struct Lambda {
    pub params: Vec<String>,
    pub iface: String,
    pub body: Expr,
    pub captured: IndexMap<String, DslValue>,
}
impl DslValue {
    // port: ReplayValues#adapt (boxed collection)
    pub fn untyped(&self) -> &Self {
        if let Self::Typed { value, .. } = self {
            value.untyped()
        } else {
            self
        }
    }
    // port: Object#getClass
    pub fn class_name(&self) -> String {
        match self {
            Self::Typed { class, .. } => return class.clone(),
            Self::Null => "null",
            Self::Int(_) => "java.lang.Integer",
            Self::Long(_) => "java.lang.Long",
            Self::Double(_) => "java.lang.Double",
            Self::Bool(_) => "java.lang.Boolean",
            Self::Char(_) => "java.lang.Character",
            Self::String(_) => "java.lang.String",
            Self::Enum { class, .. } => return class.clone(),
            Self::Class(_) => "java.lang.Class",
            Self::List(_) => "java.util.ArrayList",
            Self::Set(_) => "java.util.LinkedHashSet",
            Self::Map(_) => "java.util.LinkedHashMap",
            Self::Array { component, .. } => return format!("{component}[]"),
            Self::Optional(_) => "java.util.Optional",
            Self::CodingConvention(c) => {
                crate::replay::options_values::coding_convention_class(c.as_ref())
            }
            Self::NameGenerator(_) => "com.google.javascript.jscomp.DefaultNameGenerator",
            Self::Compiler(_) => "com.google.javascript.jscomp.Compiler",
            Self::Options(_) | Self::OptionsView(_) => {
                "com.google.javascript.jscomp.CompilerOptions"
            }
            Self::Native(o) => return o.borrow().class_name().into(),
            Self::Object(o) => return o.borrow().class.clone(),
            Self::Lambda(l) => return l.iface.clone(),
            Self::Pass(_) => "com.google.javascript.jscomp.CompilerPass",
            Self::Sequence(_) => "com.google.javascript.jscomp.ReplayDsl$SequencePass",
            Self::Node(_) => "com.google.javascript.rhino.Node",
            Self::PassFactory { .. } => "com.google.javascript.jscomp.PassFactory",
            Self::DiagnosticType(_) => "com.google.javascript.jscomp.DiagnosticType",
            Self::DiagnosticGroup(_) => "com.google.javascript.jscomp.DiagnosticGroup",
            Self::WarningsGuard { .. } => "com.google.javascript.jscomp.ComposeWarningsGuard",
            Self::SourceFile(_) => "com.google.javascript.jscomp.SourceFile",
            Self::Regex { .. } => "java.util.regex.Pattern",
            Self::Proto(p) => return p.name.clone(),
        }
        .into()
    }
    // port: Object#equals (DSL collection membership)
    pub fn equals(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Typed { value: a, .. }, b) => a.equals(b.untyped()),
            (a, Self::Typed { value: b, .. }) => a.untyped().equals(b),
            (Self::Null, Self::Null) => true,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Long(a), Self::Long(b)) => a == b,
            (Self::Double(a), Self::Double(b)) => {
                a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan()
            }
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Char(a), Self::Char(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Class(a), Self::Class(b)) => a == b,
            (Self::Enum { class: a, name: x }, Self::Enum { class: b, name: y }) => {
                a == b && x == y
            }
            (Self::List(a), Self::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.equals(y))
            }
            (Self::Set(a), Self::Set(b)) => {
                a.len() == b.len() && a.iter().all(|x| b.iter().any(|y| x.equals(y)))
            }
            (Self::Map(a), Self::Map(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.iter().any(|(l, w)| k.equals(l) && v.equals(w)))
            }
            (Self::DiagnosticType(a), Self::DiagnosticType(b)) => a == b,
            (Self::SourceFile(a), Self::SourceFile(b)) => Arc::ptr_eq(a, b),
            (Self::Object(a), Self::Object(b)) => Rc::ptr_eq(a, b),
            (Self::Lambda(a), Self::Lambda(b)) => Rc::ptr_eq(a, b),
            (Self::Node(a), Self::Node(b)) => a == b,
            (Self::Compiler(a), Self::Compiler(b)) => Rc::ptr_eq(a, b),
            (Self::Options(a), Self::Options(b)) => Rc::ptr_eq(a, b),
            (Self::OptionsView(a), Self::OptionsView(b)) => Rc::ptr_eq(a, b),
            (Self::Native(a), Self::Native(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}
pub struct Ctx {
    pub postcondition_compiler: Option<CompilerHandle>,
    pub compiler: Option<CompilerHandle>,
    pub record: JsonValue,
    pub field_overrides: IndexMap<String, DslValue>,
    pub once: IndexMap<String, DslValue>,
    pub vars: IndexMap<String, DslValue>,
    pub options: Option<OptionsHandle>,
    pub descriptor: String,
    pub class_map: IndexMap<String, String>,
    pub registry: Registry,
}
impl Ctx {
    // port: ReplayDsl.Ctx#Ctx
    pub fn new(
        descriptor: String,
        record: JsonValue,
        class_map: IndexMap<String, String>,
        registry: Registry,
    ) -> Self {
        Self {
            postcondition_compiler: None,
            compiler: None,
            record,
            field_overrides: IndexMap::<_, _>::default(),
            once: IndexMap::<_, _>::default(),
            vars: IndexMap::<_, _>::default(),
            options: None,
            descriptor,
            class_map,
            registry,
        }
    }
    // port: ReplayValues#classForName
    pub fn map_class(&self, name: &str) -> String {
        self.class_map
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.into())
    }
}
// port: ReplayDsl#evalAll
pub fn eval_all(list: &[Expr], ctx: &mut Ctx) -> Result<(), Throwable> {
    eval_all_with_compiler(list, ctx, None)
}
// port: ReplayDsl#eval
pub fn eval(expr: &Expr, ctx: &mut Ctx) -> Result<DslValue, Throwable> {
    eval_with_compiler(expr, ctx, None)
}
// port: ReplayDsl#lambda
pub fn invoke_lambda(
    lambda: &Lambda,
    values: Vec<DslValue>,
    ctx: &mut Ctx,
) -> Result<DslValue, Throwable> {
    invoke_lambda_with_compiler(lambda, values, ctx, None)
}
// port: ReplayDsl.SequencePass#process
pub fn process(
    value: &DslValue,
    compiler: CompilerHandle,
    externs: NodeId,
    root: NodeId,
    ctx: &mut Ctx,
) -> Result<(), Throwable> {
    {
        ctx.compiler = Some(compiler.clone());
        process_in_compiler(value, &mut compiler.borrow_mut(), externs, root, ctx)
    }
}
// port: ReplayValues#findField
pub fn get_field(target: &DslValue, name: &str) -> Result<DslValue, Throwable> {
    get_field_with_compiler(target, name, None)
}
// port: ReplayValues#setField
pub fn set_field(
    target: &DslValue,
    name: &str,
    value: DslValue,
    ctx: &Ctx,
) -> Result<(), Throwable> {
    set_field_with_compiler(target, name, value, ctx, None)
}
// port: ReplayDsl#evalAll
pub fn eval_all_with_compiler(
    list: &[Expr],
    ctx: &mut Ctx,
    mut native_compiler: Option<&mut Compiler>,
) -> Result<(), Throwable> {
    for e in list {
        eval_with_compiler(e, ctx, native_compiler.as_deref_mut())?;
    }
    Ok(())
}
// port: ReplayDsl#args
fn args(
    list: Option<&Vec<Expr>>,
    ctx: &mut Ctx,
    mut native_compiler: Option<&mut Compiler>,
) -> Result<Vec<DslValue>, Throwable> {
    list.into_iter()
        .flatten()
        .map(|e| eval_with_compiler(e, ctx, native_compiler.as_deref_mut()))
        .collect()
}
// port: ReplayDsl#eval
pub fn eval_with_compiler(
    expr: &Expr,
    ctx: &mut Ctx,
    mut native_compiler: Option<&mut Compiler>,
) -> Result<DslValue, Throwable> {
    Ok(match expr {
        Expr::JsonNull | Expr::Null => DslValue::Null,
        Expr::Int(i) => DslValue::Int(*i),
        Expr::Long(s) => DslValue::Long(s.parse().map_err(|_| error("invalid long"))?),
        Expr::Double(s) => DslValue::Double(parse_double(s)?),
        Expr::Bool(b) => DslValue::Bool(*b),
        Expr::String(s) => DslValue::String(JsString::from_units(s.0.clone())),
        Expr::Enum { class, name } => DslValue::Enum {
            class: ctx.map_class(class),
            name: name.clone(),
        },
        Expr::Class(s) => DslValue::Class(ctx.map_class(s)),
        Expr::List(es) => DslValue::List(
            es.iter()
                .map(|e| eval_with_compiler(e, ctx, native_compiler.as_deref_mut()))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Set(es) => {
            let mut out: Vec<DslValue> = Vec::new();
            for e in es {
                let v = eval_with_compiler(e, ctx, native_compiler.as_deref_mut())?;
                if !out.iter().any(|x| x.equals(&v)) {
                    out.push(v);
                }
            }
            DslValue::Set(out)
        }
        Expr::Map(es) => {
            let mut out: Vec<(DslValue, DslValue)> = Vec::new();
            for (k, v) in es {
                let k = eval_with_compiler(k, ctx, native_compiler.as_deref_mut())?;
                let v = eval_with_compiler(v, ctx, native_compiler.as_deref_mut())?;
                if let Some((_, old)) = out.iter_mut().find(|(x, _)| x.equals(&k)) {
                    *old = v;
                } else {
                    out.push((k, v));
                }
            }
            DslValue::Map(out)
        }
        Expr::Array { component, items } => {
            let component = ctx.map_class(component);
            let items = items
                .iter()
                .map(|e| {
                    eval_with_compiler(e, ctx, native_compiler.as_deref_mut())
                        .and_then(|v| adapt(v, &component))
                })
                .collect::<Result<_, _>>()?;
            DslValue::Array { component, items }
        }
        Expr::Compiler => ctx
            .compiler
            .as_ref()
            .map_or(DslValue::Null, |c| DslValue::Compiler(c.clone())),
        Expr::Options => {
            if let Some(o) = &ctx.options {
                DslValue::Options(o.clone())
            } else {
                let c = ctx
                    .compiler
                    .as_ref()
                    .ok_or_else(|| error("options outside compiler/options scope"))?;
                DslValue::OptionsView(c.clone())
            }
        }
        Expr::Var(n) => ctx
            .vars
            .get(n)
            .cloned()
            .ok_or_else(|| error(&format!("unbound DSL variable {n}")))?,
        Expr::Let { bindings, body } => {
            let saved = ctx.vars.clone();
            let result = (|| {
                for (name, e) in bindings {
                    let v = eval_with_compiler(e, ctx, native_compiler.as_deref_mut())?;
                    ctx.vars.insert(name.clone(), v);
                }
                eval_with_compiler(body, ctx, native_compiler.as_deref_mut())
            })();
            ctx.vars = saved;
            result?
        }
        Expr::Do { effects, value } => {
            eval_all_with_compiler(effects, ctx, native_compiler.as_deref_mut())?;
            eval_with_compiler(value, ctx, native_compiler.as_deref_mut())?
        }
        Expr::Lambda {
            params,
            iface,
            body,
        } => DslValue::Lambda(Rc::new(Lambda {
            params: params.clone(),
            iface: ctx.map_class(iface),
            body: *body.clone(),
            captured: ctx.vars.clone(),
        })),
        Expr::GetField { target, name } => get_field_with_compiler(
            &eval_with_compiler(target, ctx, native_compiler.as_deref_mut())?,
            name,
            native_compiler.as_deref_mut(),
        )?,
        Expr::MutationPoint { value, .. } => {
            eval_with_compiler(value, ctx, native_compiler.as_deref_mut())?
        }
        Expr::Field { name, path } => {
            if let Some(v) = ctx.field_overrides.get(name) {
                return Ok(v.clone());
            }
            let tf = ctx
                .record
                .get("testFields")
                .and_then(JsonValue::as_object)
                .ok_or_else(|| error("record has no testFields"))?;
            let raw = tf
                .get(name)
                .or_else(|| {
                    tf.iter()
                        .find(|(k, _)| k.ends_with(&format!(".{name}")))
                        .map(|(_, v)| v)
                })
                .ok_or_else(|| error(&format!("record has no testFields.{name}")))?;
            let raw = if let Some(p) = path {
                crate::descriptor::path(raw, p)
                    .map_err(|e| error(&e.to_string()))?
                    .unwrap_or(&JsonValue::Null)
            } else {
                raw
            };
            decode_json(raw, "java.lang.Object", &ctx.class_map)?
        }
        Expr::Record(p) => {
            let raw = crate::descriptor::path(&ctx.record, p)
                .map_err(|e| error(&e.to_string()))?
                .unwrap_or(&JsonValue::Null);
            decode_json(raw, "java.lang.Object", &ctx.class_map)?
        }
        Expr::New { class, args: es } => {
            let class = ctx.map_class(class);
            let values = args(es.as_ref(), ctx, native_compiler.as_deref_mut())?;
            invoke(ctx, &class, values, None, native_compiler)?
        }
        Expr::Static {
            class,
            method,
            args: es,
        } => {
            let class = ctx.map_class(class);
            let values = args(es.as_ref(), ctx, native_compiler.as_deref_mut())?;
            invoke(
                ctx,
                &format!("{class}.{method}"),
                values,
                None,
                native_compiler.as_deref_mut(),
            )?
        }
        Expr::Call {
            target,
            method,
            args: es,
        } => {
            let target = eval_with_compiler(target, ctx, native_compiler.as_deref_mut())?;
            let values = args(es.as_ref(), ctx, native_compiler.as_deref_mut())?;
            if matches!(target, DslValue::Null) {
                return Err(Throwable::Exception {
                    class: "java.lang.NullPointerException".into(),
                    message: Some(format!("DSL call of {method} on null")),
                });
            }
            if let (DslValue::Pass(_), "process", [DslValue::Node(externs), DslValue::Node(root)]) =
                (&target, method.as_str(), values.as_slice())
            {
                // port: ReplayDsl#eval ("call": invoke(target.getClass(), ..) resolves
                // <PassClass>#process(Node,Node)). A native pass value does not keep its runtime
                // class, and CompilerPass#process(Node,Node) is its only method, so it runs here.
                let compiler = native_compiler
                    .as_deref_mut()
                    .ok_or_else(|| error("CompilerPass#process outside the compiler"))?;
                process_in_compiler(&target, compiler, *externs, *root, ctx)?;
                return Ok(DslValue::Null);
            }
            invoke(
                ctx,
                &format!("{}.{method}", target.class_name()),
                values,
                Some(target),
                native_compiler.as_deref_mut(),
            )?
        }
        Expr::Once { key, value } => {
            if !ctx.once.contains_key(key) {
                let v = eval_with_compiler(value, ctx, native_compiler.as_deref_mut())?;
                ctx.once.insert(key.clone(), v);
            }
            ctx.once[key].clone()
        }
        Expr::WithFields { target, fields } => {
            let value = eval_with_compiler(target, ctx, native_compiler.as_deref_mut())?;
            for (name, e) in fields {
                let v = eval_with_compiler(e, ctx, native_compiler.as_deref_mut())?;
                set_field_with_compiler(&value, name, v, ctx, native_compiler.as_deref_mut())?;
            }
            value
        }
        Expr::Sequence(es) => DslValue::Sequence(
            es.iter()
                .map(|e| eval_with_compiler(e, ctx, native_compiler.as_deref_mut()))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Helper {
            name,
            package,
            outer,
            outer_instance,
            args: es,
        } => helper(
            name,
            package.as_deref(),
            outer.as_deref(),
            outer_instance.as_deref(),
            es.as_ref(),
            ctx,
            native_compiler,
        )?,
    })
}
// port: ReplayDsl#helper
fn helper(
    name: &str,
    package: Option<&str>,
    outer: Option<&[String]>,
    outer_instance: Option<&Expr>,
    es: Option<&Vec<Expr>>,
    ctx: &mut Ctx,
    mut native_compiler: Option<&mut Compiler>,
) -> Result<DslValue, Throwable> {
    let pkg = package.unwrap_or("com.google.javascript.jscomp");
    let (holder, inner) = name
        .split_once('.')
        .map_or((name, None), |(h, i)| (h, Some(i)));
    let holder = ctx.map_class(&format!("{pkg}.{holder}"));
    let class = inner.map_or_else(
        || holder.clone(),
        |i| ctx.map_class(&format!("{holder}${i}")),
    );
    let mut values = args(es, ctx, native_compiler.as_deref_mut())?;
    // Recorded constructor arity says whether this is a non-static inner helper.
    let is_inner = inner.is_some()
        && ctx
            .registry
            .has_resolution(&ctx.descriptor, &class, values.len() + 1);
    if is_inner {
        let obj = if let Some(e) = outer_instance {
            eval_with_compiler(e, ctx, native_compiler.as_deref_mut())?
        } else {
            let h = instantiate(&holder, ctx)?;
            for name in outer.unwrap_or(&[]) {
                let value = eval_with_compiler(
                    &Expr::Field {
                        name: name.clone(),
                        path: None,
                    },
                    ctx,
                    native_compiler.as_deref_mut(),
                )?;
                set_field_with_compiler(&h, name, value, ctx, native_compiler.as_deref_mut())?;
            }
            h
        };
        if obj.class_name() != holder {
            return Err(error("outerInstance has wrong holder class"));
        }
        values.insert(0, obj);
    }
    invoke(ctx, &class, values, None, native_compiler)
}
// port: ReplayValues#instantiate (helper outer holder's declared no-argument constructor)
fn instantiate(class: &str, ctx: &mut Ctx) -> Result<DslValue, Throwable> {
    // The holder's no-argument constructor has no TSV row (Java instantiates it reflectively), so
    // it is looked up among the native registrations directly.
    let signature = format!("{class}#<init>()");
    let constructor = ctx
        .registry
        .entry(&signature)
        .or_else(|| crate::replay::native_registry::entry(&signature))
        .ok_or_else(|| Throwable::Unported(class.into()))?;
    constructor(ctx, vec![])
}
// port: ReplayDsl#invoke (recorded overload resolution)
fn invoke(
    ctx: &mut Ctx,
    lookup: &str,
    mut values: Vec<DslValue>,
    target: Option<DslValue>,
    native_compiler: Option<&mut Compiler>,
) -> Result<DslValue, Throwable> {
    let resolved = ctx
        .registry
        .resolve(&ctx.descriptor, lookup, values.len())?
        .clone();
    for (v, t) in values.iter_mut().zip(&resolved.parameters) {
        *v = adapt(v.clone(), t)?;
    }
    if let Some(t) = target {
        values.insert(0, t);
    }
    if let Some(compiler) = native_compiler {
        if let Some(entry) = ctx.registry.borrowed_entry(&resolved.signature) {
            return entry(ctx, values, compiler);
        }
        if let Some(result) = crate::replay::native_registry::invoke_borrowed(
            &resolved.signature,
            ctx,
            &values,
            compiler,
        ) {
            return result;
        }
    }
    let f = ctx
        .registry
        .entry(&resolved.signature)
        .ok_or_else(|| Throwable::Unported(resolved.signature.clone()))?;
    f(ctx, values)
}
// port: ReplayDsl#lambda (call-time body evaluation)
pub fn invoke_lambda_with_compiler(
    lambda: &Lambda,
    values: Vec<DslValue>,
    ctx: &mut Ctx,
    native_compiler: Option<&mut Compiler>,
) -> Result<DslValue, Throwable> {
    if values.len() != lambda.params.len() {
        return Err(error("lambda argument count differs"));
    }
    let saved = std::mem::replace(&mut ctx.vars, lambda.captured.clone());
    for (name, value) in lambda.params.iter().zip(values) {
        ctx.vars.insert(name.clone(), value);
    }
    let result = eval_with_compiler(&lambda.body, ctx, native_compiler);
    ctx.vars = saved;
    result
}
// port: ReplayDsl.SequencePass#process
pub fn process_in_compiler(
    value: &DslValue,
    compiler: &mut Compiler,
    externs: NodeId,
    root: NodeId,
    ctx: &mut Ctx,
) -> Result<(), Throwable> {
    match value {
        // a pass carrying its runtime class (e.g. RemoveUnusedCode from Builder#build)
        DslValue::Typed { value, .. } => process_in_compiler(value, compiler, externs, root, ctx),
        DslValue::Native(p) => p
            .borrow_mut()
            .process_with_ctx(compiler, externs, root, ctx),
        // A PassFactory placeholder for a class not ported yet panics with "unported: <class>";
        // the record is then Unported by that class, as for DefaultPassConfig compiles.
        DslValue::Pass(p) => crate::replay::native_unported::capture(|| {
            p.borrow_mut().process(compiler, externs, root);
        }),
        DslValue::Sequence(passes) => {
            for pass in passes {
                process_in_compiler(pass, compiler, externs, root, ctx)?;
            }
            Ok(())
        }
        DslValue::Lambda(lambda) if lambda.iface == "com.google.javascript.jscomp.CompilerPass" => {
            invoke_lambda_with_compiler(
                lambda,
                vec![DslValue::Node(externs), DslValue::Node(root)],
                ctx,
                Some(compiler),
            )?;
            Ok(())
        }
        _ => Err(error("processor is not a CompilerPass")),
    }
}
// port: ReplayValues#findField
pub fn get_field_with_compiler(
    target: &DslValue,
    name: &str,
    native_compiler: Option<&mut Compiler>,
) -> Result<DslValue, Throwable> {
    match target {
        DslValue::Sequence(items) if name == "passes" => Ok(DslValue::List(items.clone())),
        DslValue::Native(o) => o
            .borrow()
            .fields()?
            .get(name)
            .cloned()
            .ok_or_else(|| error(&format!("no field {name} in {}", o.borrow().class_name()))),
        DslValue::OptionsView(c) => decode_json(
            &match native_compiler {
                Some(compiler) => {
                    crate::replay::options_fields::get_field(compiler.get_options(), name)?
                }
                None => crate::replay::options_fields::get_field(c.borrow().get_options(), name)?,
            },
            crate::replay::options_fields::defaults()
                .field_types
                .get(name)
                .ok_or_else(|| error("no option field"))?,
            &IndexMap::<_, _>::default(),
        ),
        DslValue::Object(o) => o
            .borrow()
            .fields
            .get(name)
            .cloned()
            .ok_or_else(|| error(&format!("no field {name} in {}", o.borrow().class))),
        DslValue::Options(o) => decode_json(
            &crate::replay::options_fields::get_field(&o.borrow(), name)?,
            crate::replay::options_fields::defaults()
                .field_types
                .get(name)
                .ok_or_else(|| error("no option field"))?,
            &IndexMap::<_, _>::default(),
        ),
        _ => Err(error("getField requires an object")),
    }
}
// port: ReplayValues#setField
pub fn set_field_with_compiler(
    target: &DslValue,
    name: &str,
    value: DslValue,
    ctx: &Ctx,
    native_compiler: Option<&mut Compiler>,
) -> Result<(), Throwable> {
    match target {
        DslValue::Native(o) => o.borrow_mut().set_field(name, value),
        DslValue::OptionsView(c) => {
            let encoded = crate::replay::replay_values::encode(&value)?;
            let v =
                crate::value::Value::from_json(&encoded, "$").map_err(|e| error(&e.to_string()))?;
            match native_compiler {
                Some(compiler) => crate::replay::options_fields::set_field(
                    compiler.get_options_mut(),
                    name,
                    &v,
                    &ctx.class_map,
                ),
                None => crate::replay::options_fields::set_field(
                    c.borrow_mut().get_options_mut(),
                    name,
                    &v,
                    &ctx.class_map,
                ),
            }
        }
        DslValue::Object(o) => {
            let ty = o
                .borrow()
                .field_types
                .get(name)
                .cloned()
                .ok_or_else(|| Throwable::Unported(format!("{}#{name}", o.borrow().class)))?;
            o.borrow_mut()
                .fields
                .insert(name.into(), adapt(value, &ty)?);
            Ok(())
        }
        DslValue::Options(o) => {
            let encoded = crate::replay::replay_values::encode(&value)?;
            let v =
                crate::value::Value::from_json(&encoded, "$").map_err(|e| error(&e.to_string()))?;
            crate::replay::options_fields::set_field(&mut o.borrow_mut(), name, &v, &ctx.class_map)
        }
        DslValue::Pass(p) => {
            crate::replay::native_peephole::set_pass_field(&mut p.borrow_mut(), name, &value)
                .unwrap_or_else(|| Err(error("withFields requires an object")))
        }
        _ => Err(error("withFields requires an object")),
    }
}
// port: Double#parseDouble
pub fn parse_double(s: &str) -> Result<f64, Throwable> {
    let s = s.trim_matches(|c: char| c <= ' ');
    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
    if unsigned == "NaN" {
        return Ok(f64::NAN);
    }
    if unsigned == "Infinity" {
        return Ok(if s.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    s.strip_suffix(['d', 'D', 'f', 'F'])
        .unwrap_or(s)
        .parse()
        .map_err(|_| error("invalid double"))
}
// port: ReplayValues.Undecodable#Undecodable
fn error(message: &str) -> Throwable {
    Throwable::HarnessError(message.into())
}
