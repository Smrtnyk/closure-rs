/*
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CheckMissingRequires.java,
//   src/com/google/javascript/jscomp/ClosureRewriteModule.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the closure-rewrite-module passes (ClosureRewriteModule, ...).
use crate::replay::native_closure_primitives::NativeModuleMetadataMap;
use crate::{
    jscomp_api::Compiler,
    replay::{
        registry::{BorrowedEntry, Entry},
        replay_dsl::{Ctx, DslValue},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    check_missing_requires::CheckMissingRequires, closure_rewrite_module::ClosureRewriteModule,
    compiler_pass::CompilerPass,
};
use std::{cell::RefCell, rc::Rc};

// port: ReplayDsl#invoke (resolved closure-rewrite-module signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.ClosureRewriteModule#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.PreprocessorSymbolTable,com.google.javascript.jscomp.TypedScope)" => {
            closure_rewrite_module
        }
        "com.google.javascript.jscomp.CheckMissingRequires#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.modules.ModuleMetadataMap)" => {
            check_missing_requires
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (the same constructors called inside a running processor, which holds
// the compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        "com.google.javascript.jscomp.ClosureRewriteModule#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.PreprocessorSymbolTable,com.google.javascript.jscomp.TypedScope)" => {
            closure_rewrite_module_borrowed
        }
        "com.google.javascript.jscomp.CheckMissingRequires#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.modules.ModuleMetadataMap)" => {
            check_missing_requires_borrowed
        }
        _ => return None,
    })
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

// port: ClosureRewriteModule#ClosureRewriteModule
fn closure_rewrite_module(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), ..] = args.as_slice() else {
        return Err(bad());
    };
    let c = c.clone();
    new_closure_rewrite_module(&args, &mut c.borrow_mut())
}

// port: ClosureRewriteModule#ClosureRewriteModule
fn closure_rewrite_module_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    new_closure_rewrite_module(&args, compiler)
}

// port: ClosureRewriteModule#ClosureRewriteModule
fn new_closure_rewrite_module(
    args: &[DslValue],
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(_),
        preprocessor_symbol_table,
        global_typed_scope,
    ] = args
    else {
        return Err(bad());
    };
    // The harness and the recorded tests pass a null PreprocessorSymbolTable.
    if !matches!(preprocessor_symbol_table, DslValue::Null) {
        return Err(Throwable::Unported(
            "com.google.javascript.jscomp.PreprocessorSymbolTable (replay value)".into(),
        ));
    }
    // A non-null TypedScope comes from TypeCheck#processForTesting.
    let global_typed_scope = crate::replay::native_type_check::typed_scope_of(global_typed_scope)?;
    let pass: Box<dyn CompilerPass> = Box::new(ClosureRewriteModule::new(
        compiler,
        None,
        global_typed_scope,
    ));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: CheckMissingRequires#CheckMissingRequires
fn check_missing_requires(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), ..] = args.as_slice() else {
        return Err(bad());
    };
    let c = c.clone();
    new_check_missing_requires(&args, &c.borrow())
}

// port: CheckMissingRequires#CheckMissingRequires
fn check_missing_requires_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    new_check_missing_requires(&args, compiler)
}

// port: CheckMissingRequires#CheckMissingRequires
fn new_check_missing_requires(
    args: &[DslValue],
    compiler: &Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_), module_metadata_map] = args else {
        return Err(bad());
    };
    let module_metadata_map = match module_metadata_map {
        // Java dereferences the map in the constructor (moduleMetadataMap.getModulesByGoogNamespace()).
        DslValue::Null => {
            return Err(Throwable::Exception {
                class: "java.lang.NullPointerException".into(),
                message: None,
            });
        }
        DslValue::Native(native) => {
            let mut native = native.borrow_mut();
            let Some(map) = native
                .as_any_mut()
                .downcast_mut::<NativeModuleMetadataMap>()
            else {
                return Err(bad());
            };
            map.0.clone()
        }
        _ => return Err(bad()),
    };
    let pass: Box<dyn CompilerPass> =
        Box::new(CheckMissingRequires::new(compiler, module_metadata_map));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
