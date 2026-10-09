/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CheckClosureImports.java,
//   src/com/google/javascript/jscomp/ClosureCheckModule.java,
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/ProcessClosurePrimitives.java,
//   src/com/google/javascript/jscomp/ScopedAliases.java,
//   src/com/google/javascript/jscomp/modules/ModuleMetadataMap.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the closure-primitives passes (ProcessClosurePrimitives, ClosureCheckModule,
//! CheckClosureImports, ScopedAliases) and the module metadata values their tests build.
use crate::{
    replay::{
        registry::{BorrowedEntry, Entry},
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    check_closure_imports::CheckClosureImports,
    closure_check_module::ClosureCheckModule,
    compiler_pass::CompilerPass,
    modules::module_metadata_map::{self, ModuleMetadata, ModuleMetadataMap, ModuleType},
    process_closure_primitives::ProcessClosurePrimitives,
    scoped_aliases::{InvalidModuleGetHandling, ScopedAliases},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc, sync::Arc};

const CLOSURE_CHECK_MODULE_INIT: &str = "com.google.javascript.jscomp.ClosureCheckModule#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.modules.ModuleMetadataMap)";
const CHECK_CLOSURE_IMPORTS_INIT: &str = "com.google.javascript.jscomp.CheckClosureImports#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.modules.ModuleMetadataMap)";
const SCOPED_ALIASES_BUILDER: &str = "com.google.javascript.jscomp.ScopedAliases#builder(com.google.javascript.jscomp.AbstractCompiler)";
const PASS_PROCESS: &str =
    "#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)";

const MODULE_METADATA_MAP_CLASS: &str = "com.google.javascript.jscomp.modules.ModuleMetadataMap";
const MODULE_METADATA_CLASS: &str =
    "com.google.javascript.jscomp.modules.AutoValue_ModuleMetadataMap_ModuleMetadata";
const MODULE_METADATA_BUILDER_CLASS: &str =
    "com.google.javascript.jscomp.modules.AutoValue_ModuleMetadataMap_ModuleMetadata$Builder";
const SCOPED_ALIASES_BUILDER_CLASS: &str = "com.google.javascript.jscomp.ScopedAliases$Builder";

// port: ReplayDsl#invoke (resolved closure-primitives signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.ProcessClosurePrimitives#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            process_closure_primitives
        }
        CLOSURE_CHECK_MODULE_INIT => closure_check_module,
        CHECK_CLOSURE_IMPORTS_INIT => check_closure_imports,
        SCOPED_ALIASES_BUILDER => scoped_aliases_builder,
        "com.google.javascript.jscomp.ScopedAliases$Builder#setModuleMetadataMap(com.google.javascript.jscomp.modules.ModuleMetadataMap)" => {
            scoped_aliases_set_module_metadata_map
        }
        "com.google.javascript.jscomp.ScopedAliases$Builder#setInvalidModuleGetHandling(com.google.javascript.jscomp.ScopedAliases$InvalidModuleGetHandling)" => {
            scoped_aliases_set_invalid_module_get_handling
        }
        "com.google.javascript.jscomp.ScopedAliases$Builder#build()" => scoped_aliases_build,
        "com.google.javascript.jscomp.modules.ModuleMetadataMap#<init>(java.util.Map,java.util.Map)" => {
            new_module_metadata_map
        }
        "com.google.javascript.jscomp.modules.ModuleMetadataMap$ModuleMetadata#builder()" => {
            module_metadata_builder
        }
        "com.google.javascript.jscomp.modules.ModuleMetadataMap$ModuleMetadata$Builder#addGoogNamespace(java.lang.String)" => {
            module_metadata_add_goog_namespace
        }
        "com.google.javascript.jscomp.modules.AutoValue_ModuleMetadataMap_ModuleMetadata$Builder#moduleType(com.google.javascript.jscomp.modules.ModuleMetadataMap$ModuleType)" => {
            module_metadata_module_type
        }
        "com.google.javascript.jscomp.modules.AutoValue_ModuleMetadataMap_ModuleMetadata$Builder#usesClosure(boolean)" => {
            module_metadata_uses_closure
        }
        "com.google.javascript.jscomp.modules.AutoValue_ModuleMetadataMap_ModuleMetadata$Builder#isTestOnly(boolean)" => {
            module_metadata_is_test_only
        }
        "com.google.javascript.jscomp.modules.AutoValue_ModuleMetadataMap_ModuleMetadata$Builder#build()" => {
            module_metadata_build
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (the same signatures while a factory has loaned out the compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        CLOSURE_CHECK_MODULE_INIT => closure_check_module_borrowed,
        CHECK_CLOSURE_IMPORTS_INIT => check_closure_imports_borrowed,
        _ if signature.ends_with(PASS_PROCESS)
            && (signature.starts_with("com.google.javascript.jscomp.ClosureCheckModule#")
                || signature.starts_with("com.google.javascript.jscomp.GatherModuleMetadata#")) =>
        {
            process_borrowed
        }
        _ => return None,
    })
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

fn null_pointer() -> Throwable {
    Throwable::Exception {
        class: "java.lang.NullPointerException".into(),
        message: None,
    }
}

/// The post-state only walks fields to find recorded result producers (UnitRecorder
/// RESULT_PRODUCERS: RenameVars, ReplaceStrings, ...). No field of the passes and module metadata
/// values here reaches one, so, as for a plain `DslValue::Pass`, they contribute nothing.
fn no_result_producer_fields() -> Result<IndexMap<String, DslValue>, Throwable> {
    Ok(IndexMap::<_, _>::default())
}

/// A real Rust pass behind the Java class the descriptor constructed, so the DSL can call its own
/// `process(Node, Node)` (resolved against that class) and run it as the test's processor.
pub struct NamedPass {
    class: &'static str,
    pass: Box<dyn CompilerPass>,
}

impl NamedPass {
    pub fn value(class: &'static str, pass: Box<dyn CompilerPass>) -> DslValue {
        DslValue::Native(Rc::new(RefCell::new(Self { class, pass })))
    }
}

impl NativeObject for NamedPass {
    fn class_name(&self) -> &str {
        self.class
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: UnitRecorder#collect (the processor's fields)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        no_result_producer_fields()
    }
    // port: CompilerPass#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.pass.process(compiler, externs, root);
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: CompilerPass#process (called on the pass object while the compiler is loaned out)
fn process_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [
        DslValue::Native(pass),
        DslValue::Node(externs),
        DslValue::Node(root),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    pass.borrow_mut().process(compiler, *externs, *root)?;
    Ok(DslValue::Null)
}

/// `ModuleMetadataMap` as a DSL value. Java hands the compiler's map around by reference.
pub struct NativeModuleMetadataMap(pub Arc<ModuleMetadataMap>);

impl NativeObject for NativeModuleMetadataMap {
    fn class_name(&self) -> &str {
        MODULE_METADATA_MAP_CLASS
    }
    // port: UnitRecorder#collect (fields of a value reachable from the processor)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        no_result_producer_fields()
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: Compiler#getModuleMetadataMap (return value conversion)
pub fn module_metadata_map_value(map: Option<&Arc<ModuleMetadataMap>>) -> DslValue {
    map.map_or(DslValue::Null, |map| {
        DslValue::Native(Rc::new(RefCell::new(NativeModuleMetadataMap(map.clone()))))
    })
}

// port: ReplayDsl#invoke (ModuleMetadataMap argument cast; None for null)
pub(crate) fn module_metadata_map_of(
    value: &DslValue,
) -> Result<Option<Arc<ModuleMetadataMap>>, Throwable> {
    match value {
        DslValue::Null => Ok(None),
        DslValue::Native(object) => object
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<NativeModuleMetadataMap>()
            .map(|m| Some(m.0.clone()))
            .ok_or_else(bad),
        _ => Err(bad()),
    }
}

/// `ModuleMetadata` (an AutoValue with reference equality) as a DSL value.
pub struct NativeModuleMetadata(pub Arc<ModuleMetadata>);

impl NativeObject for NativeModuleMetadata {
    fn class_name(&self) -> &str {
        MODULE_METADATA_CLASS
    }
    // port: UnitRecorder#collect (fields of a value reachable from the processor)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        no_result_producer_fields()
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn module_metadata_of(value: &DslValue) -> Result<Arc<ModuleMetadata>, Throwable> {
    let DslValue::Native(object) = value else {
        return Err(bad());
    };
    object
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<NativeModuleMetadata>()
        .map(|m| m.0.clone())
        .ok_or_else(bad)
}

/// `ModuleMetadata.Builder` as a DSL value; its setters return the same builder.
pub struct NativeModuleMetadataBuilder(module_metadata_map::Builder);

impl NativeObject for NativeModuleMetadataBuilder {
    fn class_name(&self) -> &str {
        MODULE_METADATA_BUILDER_CLASS
    }
    // port: UnitRecorder#collect (fields of a value reachable from the processor)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        no_result_producer_fields()
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Runs `action` on the receiver builder (`args[0]`) and returns the receiver, as the Java
/// builder's setters return `this`.
fn with_module_metadata_builder(
    args: &[DslValue],
    action: impl FnOnce(&mut module_metadata_map::Builder),
) -> Result<DslValue, Throwable> {
    let Some(DslValue::Native(object)) = args.first() else {
        return Err(bad());
    };
    {
        let mut object = object.borrow_mut();
        let builder = object
            .as_any_mut()
            .downcast_mut::<NativeModuleMetadataBuilder>()
            .ok_or_else(bad)?;
        action(&mut builder.0);
    }
    Ok(args[0].clone())
}

// port: ProcessClosurePrimitives#ProcessClosurePrimitives
fn process_closure_primitives(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    let pass: Box<dyn CompilerPass> = Box::new(ProcessClosurePrimitives::new(&c.borrow()));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

fn is_current(ctx: &Ctx, c: &CompilerHandle) -> bool {
    ctx.compiler
        .as_ref()
        .is_some_and(|current| Rc::ptr_eq(c, current))
}

// port: ClosureCheckModule#ClosureCheckModule
fn new_closure_check_module(
    compiler: &AbstractCompiler,
    module_metadata_map: &DslValue,
) -> Result<DslValue, Throwable> {
    // Java keeps the nullable map and dereferences it when the pass runs; the Rust pass takes
    // the map itself.
    let module_metadata_map =
        module_metadata_map_of(module_metadata_map)?.ok_or_else(null_pointer)?;
    Ok(NamedPass::value(
        "com.google.javascript.jscomp.ClosureCheckModule",
        Box::new(ClosureCheckModule::new(compiler, module_metadata_map)),
    ))
}

// port: ClosureCheckModule#ClosureCheckModule
fn closure_check_module(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), map] = args.as_slice() else {
        return Err(bad());
    };
    new_closure_check_module(&c.borrow(), map)
}

// port: ClosureCheckModule#ClosureCheckModule (compiler loaned out)
fn closure_check_module_borrowed(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), map] = args.as_slice() else {
        return Err(bad());
    };
    if !is_current(ctx, c) {
        return closure_check_module(ctx, args);
    }
    new_closure_check_module(compiler, map)
}

// port: CheckClosureImports#CheckClosureImports
fn new_check_closure_imports(
    compiler: &AbstractCompiler,
    module_metadata_map: &DslValue,
) -> Result<DslValue, Throwable> {
    // Java keeps the nullable map and dereferences it when the pass runs; the Rust pass takes
    // the map itself.
    let module_metadata_map =
        module_metadata_map_of(module_metadata_map)?.ok_or_else(null_pointer)?;
    Ok(NamedPass::value(
        "com.google.javascript.jscomp.CheckClosureImports",
        Box::new(CheckClosureImports::new(compiler, module_metadata_map)),
    ))
}

// port: CheckClosureImports#CheckClosureImports
fn check_closure_imports(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), map] = args.as_slice() else {
        return Err(bad());
    };
    new_check_closure_imports(&c.borrow(), map)
}

// port: CheckClosureImports#CheckClosureImports (compiler loaned out)
fn check_closure_imports_borrowed(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), map] = args.as_slice() else {
        return Err(bad());
    };
    if !is_current(ctx, c) {
        return check_closure_imports(ctx, args);
    }
    new_check_closure_imports(compiler, map)
}

/// `ScopedAliases.Builder` as a DSL value. Java's builder holds the compiler and its settings,
/// and `build()` leaves it usable; the Rust builder is consumed by `build(compiler)`, so the
/// adapter keeps the compiler and the settings and builds a Rust builder per `build()`.
pub struct NativeScopedAliasesBuilder {
    compiler: CompilerHandle,
    module_metadata_map: Option<Arc<ModuleMetadataMap>>,
    invalid_module_get_handling: InvalidModuleGetHandling,
}

impl NativeObject for NativeScopedAliasesBuilder {
    fn class_name(&self) -> &str {
        SCOPED_ALIASES_BUILDER_CLASS
    }
    // port: UnitRecorder#collect (fields of a value reachable from the processor)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        no_result_producer_fields()
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Runs `action` on the receiver (`args[0]`) and returns the receiver, as the Java builder's
/// setters return `this`.
fn with_scoped_aliases_builder(
    args: &[DslValue],
    action: impl FnOnce(&mut NativeScopedAliasesBuilder),
) -> Result<DslValue, Throwable> {
    let Some(DslValue::Native(object)) = args.first() else {
        return Err(bad());
    };
    {
        let mut object = object.borrow_mut();
        let receiver = object
            .as_any_mut()
            .downcast_mut::<NativeScopedAliasesBuilder>()
            .ok_or_else(bad)?;
        action(receiver);
    }
    Ok(args[0].clone())
}

// port: ScopedAliases#builder
fn scoped_aliases_builder(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    // ScopedAliases.Builder's field defaults (ScopedAliases.Builder#Builder).
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeScopedAliasesBuilder {
            compiler: c.clone(),
            module_metadata_map: None,
            invalid_module_get_handling: InvalidModuleGetHandling::PRESERVE,
        },
    ))))
}

// port: ScopedAliases.Builder#setModuleMetadataMap
fn scoped_aliases_set_module_metadata_map(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [_, map] = args.as_slice() else {
        return Err(bad());
    };
    let map = module_metadata_map_of(map)?;
    with_scoped_aliases_builder(&args, |b| b.module_metadata_map = map)
}

// port: ScopedAliases.Builder#setInvalidModuleGetHandling
fn scoped_aliases_set_invalid_module_get_handling(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [_, DslValue::Enum { class, name }] = args.as_slice() else {
        return Err(bad());
    };
    if !class.ends_with("ScopedAliases$InvalidModuleGetHandling") {
        return Err(bad());
    }
    let handling = match name.as_str() {
        "PRESERVE" => InvalidModuleGetHandling::PRESERVE,
        "GIVE_UNIQUE_NAME" => InvalidModuleGetHandling::GIVE_UNIQUE_NAME,
        _ => return Err(bad()),
    };
    with_scoped_aliases_builder(&args, |b| b.invalid_module_get_handling = handling)
}

// port: ScopedAliases.Builder#build
fn scoped_aliases_build(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(object)] = args.as_slice() else {
        return Err(bad());
    };
    let mut object = object.borrow_mut();
    let receiver = object
        .as_any_mut()
        .downcast_mut::<NativeScopedAliasesBuilder>()
        .ok_or_else(bad)?;
    let pass = ScopedAliases::builder()
        .set_module_metadata_map(receiver.module_metadata_map.clone())
        .set_invalid_module_get_handling(receiver.invalid_module_get_handling)
        .build(&receiver.compiler.borrow());
    Ok(NamedPass::value(
        "com.google.javascript.jscomp.ScopedAliases",
        Box::new(pass),
    ))
}

// port: ModuleMetadataMap#ModuleMetadataMap
fn new_module_metadata_map(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [by_path, by_namespace] = args.as_slice() else {
        return Err(bad());
    };
    fn entries(value: &DslValue) -> Result<Vec<(JsString, Arc<ModuleMetadata>)>, Throwable> {
        let value = match value {
            DslValue::Typed { value, .. } => value.as_ref(),
            v => v,
        };
        let DslValue::Map(items) = value else {
            return Err(bad());
        };
        items
            .iter()
            .map(|(k, v)| {
                let DslValue::String(k) = k else {
                    return Err(bad());
                };
                Ok((k.clone(), module_metadata_of(v)?))
            })
            .collect()
    }
    let modules_by_path: IndexMap<String, Arc<ModuleMetadata>> = entries(by_path)?
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let modules_by_goog_namespace: IndexMap<JsString, Arc<ModuleMetadata>> =
        entries(by_namespace)?.into_iter().collect();
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeModuleMetadataMap(Arc::new(ModuleMetadataMap::new(
            modules_by_path,
            modules_by_goog_namespace,
        ))),
    ))))
}

// port: ModuleMetadataMap.ModuleMetadata#builder
fn module_metadata_builder(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeModuleMetadataBuilder(ModuleMetadata::builder()),
    ))))
}

// port: ModuleMetadataMap.ModuleMetadata.Builder#addGoogNamespace
fn module_metadata_add_goog_namespace(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [_, DslValue::String(namespace)] = args.as_slice() else {
        return Err(bad());
    };
    let namespace = namespace.clone();
    with_module_metadata_builder(&args, |b| {
        b.add_goog_namespace(namespace);
    })
}

// port: ModuleMetadataMap.ModuleMetadata.Builder#moduleType
fn module_metadata_module_type(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [_, DslValue::Enum { class, name }] = args.as_slice() else {
        return Err(bad());
    };
    if !class.ends_with("ModuleMetadataMap$ModuleType") {
        return Err(bad());
    }
    // ModuleType#valueOf
    let module_type = [
        ModuleType::ES6_MODULE,
        ModuleType::GOOG_MODULE,
        ModuleType::LEGACY_GOOG_MODULE,
        ModuleType::COMMON_JS,
        ModuleType::GOOG_PROVIDE,
        ModuleType::SCRIPT,
    ]
    .into_iter()
    .find(|t| t.name() == name)
    .ok_or_else(bad)?;
    with_module_metadata_builder(&args, |b| {
        b.module_type(module_type);
    })
}

// port: ModuleMetadataMap.ModuleMetadata.Builder#usesClosure
fn module_metadata_uses_closure(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [_, DslValue::Bool(value)] = args.as_slice() else {
        return Err(bad());
    };
    let value = *value;
    with_module_metadata_builder(&args, |b| {
        b.uses_closure(value);
    })
}

// port: ModuleMetadataMap.ModuleMetadata.Builder#isTestOnly
fn module_metadata_is_test_only(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [_, DslValue::Bool(value)] = args.as_slice() else {
        return Err(bad());
    };
    let value = *value;
    with_module_metadata_builder(&args, |b| {
        b.is_test_only(value);
    })
}

// port: ModuleMetadataMap.ModuleMetadata.Builder#build
fn module_metadata_build(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(object)] = args.as_slice() else {
        return Err(bad());
    };
    let mut object = object.borrow_mut();
    let builder = object
        .as_any_mut()
        .downcast_mut::<NativeModuleMetadataBuilder>()
        .ok_or_else(bad)?;
    let metadata = builder.0.build();
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeModuleMetadata(metadata),
    ))))
}
