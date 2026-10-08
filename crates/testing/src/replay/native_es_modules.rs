/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/Es6RelativizeImportPaths.java,
//   src/com/google/javascript/jscomp/Es6RewriteModules.java,
//   src/com/google/javascript/jscomp/Es6RewriteModulesToCommonJsModules.java,
//   src/com/google/javascript/jscomp/ForbidDynamicImportUsage.java,
//   src/com/google/javascript/jscomp/modules/ModuleMapCreator.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the es-modules passes (Es6RewriteModules, ForbidDynamicImportUsage, ...)
//! and the ModuleMap value their descriptors pass around.
use crate::{
    jscomp_api::Compiler,
    replay::{
        native_closure_primitives::{NamedPass, module_metadata_map_of},
        options_values::OptionValue,
        registry::{BorrowedEntry, Entry},
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_options::ChunkOutputType, compiler_pass::CompilerPass,
    es6_relativize_import_paths::Es6RelativizeImportPaths, es6_rewrite_modules::Es6RewriteModules,
    es6_rewrite_modules_to_common_js_modules::Es6RewriteModulesToCommonJsModules,
    forbid_dynamic_import_usage::ForbidDynamicImportUsage, modules::module_map::ModuleMap,
    modules::module_map_creator::ModuleMapCreator,
};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc, sync::Arc};

const PASS_PROCESS: &str =
    "#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)";
const MODULE_MAP_CLASS: &str = "com.google.javascript.jscomp.modules.ModuleMap";
const ES6_REWRITE_MODULES_CLASS: &str = "com.google.javascript.jscomp.Es6RewriteModules";
const MODULE_MAP_CREATOR_CLASS: &str = "com.google.javascript.jscomp.modules.ModuleMapCreator";
const ES6_REWRITE_MODULES_INIT5: &str = "com.google.javascript.jscomp.Es6RewriteModules#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.modules.ModuleMetadataMap,com.google.javascript.jscomp.modules.ModuleMap,com.google.javascript.jscomp.PreprocessorSymbolTable,com.google.javascript.jscomp.TypedScope)";
const ES6_REWRITE_MODULES_INIT6: &str = "com.google.javascript.jscomp.Es6RewriteModules#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.modules.ModuleMetadataMap,com.google.javascript.jscomp.modules.ModuleMap,com.google.javascript.jscomp.PreprocessorSymbolTable,com.google.javascript.jscomp.TypedScope,com.google.javascript.jscomp.CompilerOptions$ChunkOutputType)";
const MODULE_MAP_CREATOR_INIT: &str = "com.google.javascript.jscomp.modules.ModuleMapCreator#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.modules.ModuleMetadataMap)";

// port: ReplayDsl#invoke (resolved es-modules signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.ForbidDynamicImportUsage#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            forbid_dynamic_import_usage
        }
        "com.google.javascript.jscomp.Es6RelativizeImportPaths#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            es6_relativize_import_paths
        }
        "com.google.javascript.jscomp.Es6RewriteModulesToCommonJsModules#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            es6_rewrite_modules_to_common_js_modules
        }
        ES6_REWRITE_MODULES_INIT5 | ES6_REWRITE_MODULES_INIT6 => es6_rewrite_modules,
        MODULE_MAP_CREATOR_INIT => module_map_creator,
        _ => return None,
    })
}

// port: ReplayDsl#invoke (the same signatures while a factory has loaned out the compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        ES6_REWRITE_MODULES_INIT5 | ES6_REWRITE_MODULES_INIT6 => es6_rewrite_modules_borrowed,
        MODULE_MAP_CREATOR_INIT => module_map_creator_borrowed,
        _ if signature.ends_with(PASS_PROCESS)
            && (signature.starts_with("com.google.javascript.jscomp.Es6RewriteModules#")
                || signature
                    .starts_with("com.google.javascript.jscomp.modules.ModuleMapCreator#")) =>
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

/// `ModuleMap` as a DSL value. Java hands the compiler's map around by reference.
pub struct NativeModuleMap(pub Arc<ModuleMap>);

impl NativeObject for NativeModuleMap {
    fn class_name(&self) -> &str {
        MODULE_MAP_CLASS
    }
    // port: UnitRecorder#collect (fields of a value reachable from the processor)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        // No field of a ModuleMap reaches a recorded result producer.
        Ok(IndexMap::new())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: Compiler#getModuleMap (return value conversion)
pub fn module_map_value(map: Option<&Arc<ModuleMap>>) -> DslValue {
    map.map_or(DslValue::Null, |map| {
        DslValue::Native(Rc::new(RefCell::new(NativeModuleMap(map.clone()))))
    })
}

// port: ReplayDsl#invoke (ModuleMap argument cast; None for null)
fn module_map_of(value: &DslValue) -> Result<Option<Arc<ModuleMap>>, Throwable> {
    match value {
        DslValue::Null => Ok(None),
        DslValue::Native(object) => object
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<NativeModuleMap>()
            .map(|m| Some(m.0.clone()))
            .ok_or_else(bad),
        _ => Err(bad()),
    }
}

// port: CompilerPass#process (called on the pass object while the compiler is loaned out)
fn process_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
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

// port: ForbidDynamicImportUsage#ForbidDynamicImportUsage
fn forbid_dynamic_import_usage(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    let pass: Box<dyn CompilerPass> = Box::new(ForbidDynamicImportUsage::new(&c.borrow()));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: Es6RelativizeImportPaths#Es6RelativizeImportPaths
fn es6_relativize_import_paths(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    let pass: Box<dyn CompilerPass> = Box::new(Es6RelativizeImportPaths::new(&c.borrow()));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: Es6RewriteModulesToCommonJsModules#Es6RewriteModulesToCommonJsModules
fn es6_rewrite_modules_to_common_js_modules(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad());
    };
    let pass: Box<dyn CompilerPass> =
        Box::new(Es6RewriteModulesToCommonJsModules::new(&c.borrow()));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: Es6RewriteModules#Es6RewriteModules
fn es6_rewrite_modules(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), ..] = args.as_slice() else {
        return Err(bad());
    };
    let c = c.clone();
    new_es6_rewrite_modules(&args, &mut c.borrow_mut())
}

// port: Es6RewriteModules#Es6RewriteModules
fn es6_rewrite_modules_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    new_es6_rewrite_modules(&args, compiler)
}

// port: Es6RewriteModules#Es6RewriteModules
fn new_es6_rewrite_modules(
    args: &[DslValue],
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let (module_metadata_map, module_map, preprocessor_symbol_table, global_typed_scope, chunk) =
        match args {
            [DslValue::Compiler(_), a, b, c, d] => (a, b, c, d, None),
            [DslValue::Compiler(_), a, b, c, d, e] => (a, b, c, d, Some(e)),
            _ => return Err(bad()),
        };
    let module_metadata_map = module_metadata_map_of(module_metadata_map)?;
    let module_map = module_map_of(module_map)?;
    // The recorded tests pass a null PreprocessorSymbolTable.
    if !matches!(preprocessor_symbol_table, DslValue::Null) {
        return Err(Throwable::Unported(
            "com.google.javascript.jscomp.PreprocessorSymbolTable (replay value)".into(),
        ));
    }
    // A non-null TypedScope comes from TypeCheck#processForTesting.
    let global_typed_scope = crate::replay::native_type_check::typed_scope_of(global_typed_scope)?;
    if module_metadata_map.is_none() {
        // checkNotNull(moduleMetadataMap)
        return Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        });
    }
    let pass = match chunk {
        None => Es6RewriteModules::new(
            compiler,
            module_metadata_map,
            module_map,
            None,
            global_typed_scope,
        ),
        Some(chunk) => Es6RewriteModules::new_with_chunk_output_type(
            compiler,
            module_metadata_map,
            module_map,
            None,
            global_typed_scope,
            ChunkOutputType::decode_value(chunk)?,
        ),
    };
    Ok(NamedPass::value(ES6_REWRITE_MODULES_CLASS, Box::new(pass)))
}

// port: ModuleMapCreator#ModuleMapCreator
fn module_map_creator(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), ..] = args.as_slice() else {
        return Err(bad());
    };
    let c = c.clone();
    new_module_map_creator(&args, &mut c.borrow_mut())
}

// port: ModuleMapCreator#ModuleMapCreator
fn module_map_creator_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    new_module_map_creator(&args, compiler)
}

// port: ModuleMapCreator#ModuleMapCreator
fn new_module_map_creator(
    args: &[DslValue],
    _compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_), module_metadata_map] = args else {
        return Err(bad());
    };
    let Some(module_metadata_map) = module_metadata_map_of(module_metadata_map)? else {
        return Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        });
    };
    Ok(NamedPass::value(
        MODULE_MAP_CREATOR_CLASS,
        Box::new(ModuleMapCreator::new(module_metadata_map)),
    ))
}
