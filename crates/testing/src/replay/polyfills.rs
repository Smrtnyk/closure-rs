/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/IsolatePolyfills.java,
//   src/com/google/javascript/jscomp/PolyfillUsageFinder.java,
//   src/com/google/javascript/jscomp/RewritePolyfills.java,
//   test/com/google/javascript/jscomp/GuardedCallbackTest.java,
//   test/com/google/javascript/jscomp/RewritePolyfillsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the polyfill passes: PolyfillUsageFinder.Polyfills#fromTable, the
//! RewritePolyfills and IsolatePolyfills constructors, and the GuardedCallbackTest and
//! RewritePolyfillsTest helpers (oracle/replay/helpers) as their descriptors call them.
use crate::{
    jscomp_api::Compiler,
    replay::{
        options_values::OptionValue,
        registry::{BorrowedEntry, Entry},
        replay_dsl::{Ctx, DslValue, NativeObject, Object},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_options::LanguageMode,
    compiler_pass::CompilerPass,
    guarded_callback::{GuardedCallback, GuardedCallbackSubclass},
    isolate_polyfills::IsolatePolyfills,
    js::runtime_js_lib_manager::{RuntimeJsLibManager, RuntimeLibraryMode},
    node_traversal::NodeTraversal,
    polyfill_usage_finder::Polyfills,
    rewrite_polyfills::RewritePolyfills,
};
use closure_rhino::{js_string::JsString, node::NodeId};
use indexmap::{IndexMap, IndexSet};
use std::{cell::RefCell, rc::Rc};

const POLYFILLS: &str = "com.google.javascript.jscomp.PolyfillUsageFinder$Polyfills";
const ISOLATE_POLYFILLS: &str = "com.google.javascript.jscomp.IsolatePolyfills";
const REWRITE_POLYFILLS_TEST_HELPERS: &str =
    "com.google.javascript.jscomp.RewritePolyfillsTest_Helpers";
const CREATE_RUNTIME_JS_LIB_MANAGER: &str =
    "com.google.javascript.jscomp.RewritePolyfillsTest_Helpers$CreateRuntimeJsLibManager";
const RUNTIME_JS_LIB_MANAGER: &str = "com.google.javascript.jscomp.js.RuntimeJsLibManager";

/// The helper holder's declared no-argument constructor, which `ReplayDsl#helper` instantiates
/// for a non-static inner helper (no recorded signature row: Java reaches it reflectively).
pub const REWRITE_POLYFILLS_TEST_HELPERS_INIT: &str =
    "com.google.javascript.jscomp.RewritePolyfillsTest_Helpers#<init>()";

// port: ReplayDsl#invoke (resolved signatures backed by the polyfill passes)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.PolyfillUsageFinder$Polyfills#fromTable(java.lang.String)" => {
            polyfills_from_table
        }
        "com.google.javascript.jscomp.RewritePolyfills#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.js.RuntimeJsLibManager,com.google.javascript.jscomp.PolyfillUsageFinder$Polyfills,boolean,boolean,com.google.javascript.jscomp.CompilerOptions$LanguageMode)" => {
            rewrite_polyfills
        }
        "com.google.javascript.jscomp.IsolatePolyfills#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.PolyfillUsageFinder$Polyfills)" => {
            isolate_polyfills
        }
        "com.google.javascript.jscomp.GuardedCallbackTest_Helpers$GetProcessorPass#<init>(com.google.javascript.jscomp.Compiler)" => {
            get_processor_pass
        }
        REWRITE_POLYFILLS_TEST_HELPERS_INIT => rewrite_polyfills_test_helpers,
        "com.google.javascript.jscomp.RewritePolyfillsTest_Helpers$CreateRuntimeJsLibManager#<init>(com.google.javascript.jscomp.RewritePolyfillsTest_Helpers)" => {
            create_runtime_js_lib_manager_helper
        }
        "com.google.javascript.jscomp.RewritePolyfillsTest_Helpers$CreateRuntimeJsLibManager#create(com.google.javascript.jscomp.Compiler)" => {
            create_runtime_js_lib_manager
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (constructors and methods while a factory or lambda borrows the compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        "com.google.javascript.jscomp.IsolatePolyfills#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.PolyfillUsageFinder$Polyfills)" => {
            isolate_polyfills_borrowed
        }
        "com.google.javascript.jscomp.IsolatePolyfills#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)" => {
            isolate_polyfills_process
        }
        "com.google.javascript.jscomp.RewritePolyfillsTest_Helpers$CreateRuntimeJsLibManager#create(com.google.javascript.jscomp.Compiler)" => {
            create_runtime_js_lib_manager_borrowed
        }
        _ => return None,
    })
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

fn pass(p: impl CompilerPass + 'static) -> DslValue {
    DslValue::Pass(Rc::new(RefCell::new(Box::new(p))))
}

/// A native value the replay hands from one call to the next (a constructor result passed as
/// an argument or used as a receiver).
struct NativeValue<T: 'static> {
    class: &'static str,
    value: Option<T>,
}
impl<T: 'static> NativeObject for NativeValue<T> {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        self.class
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn native<T: 'static>(class: &'static str, value: T) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativeValue {
        class,
        value: Some(value),
    })))
}

/// Takes the value out of a native argument: the Rust pass owns what the Java pass references.
fn take_native<T: 'static>(value: &DslValue) -> Result<T, Throwable> {
    let DslValue::Native(object) = value else {
        return Err(bad());
    };
    let mut object = object.borrow_mut();
    object
        .as_any_mut()
        .downcast_mut::<NativeValue<T>>()
        .and_then(|v| v.value.take())
        .ok_or_else(bad)
}

fn string(value: &DslValue) -> Result<String, Throwable> {
    match value {
        DslValue::String(s) => Ok(s.to_string()),
        _ => Err(bad()),
    }
}

// port: PolyfillUsageFinder.Polyfills#fromTable
fn polyfills_from_table(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [table] = args.as_slice() else {
        return Err(bad());
    };
    Ok(native(POLYFILLS, Polyfills::from_table(&string(table)?)))
}

// port: RewritePolyfills#RewritePolyfills(AbstractCompiler,RuntimeJsLibManager,Polyfills,boolean,boolean,LanguageMode)
fn rewrite_polyfills(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(_),
        manager,
        polyfills,
        DslValue::Bool(inject_polyfills),
        DslValue::Bool(isolate_polyfills),
        inject_polyfills_newer_than,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let inject_polyfills_newer_than = match inject_polyfills_newer_than {
        DslValue::Null => None,
        mode => Some(LanguageMode::decode_value(mode)?),
    };
    Ok(pass(RewritePolyfills::new_for_testing(
        take_native::<RuntimeJsLibManager>(manager)?,
        take_native::<Polyfills>(polyfills)?,
        *inject_polyfills,
        *isolate_polyfills,
        inject_polyfills_newer_than,
    )))
}

/// `new IsolatePolyfills(compiler, polyfills)`; its `process` is called on the result.
fn isolate_polyfills_value(
    compiler: &mut Compiler,
    args: &[DslValue],
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_), polyfills] = args else {
        return Err(bad());
    };
    let polyfills = take_native::<Polyfills>(polyfills)?;
    Ok(native(
        ISOLATE_POLYFILLS,
        IsolatePolyfills::new_with_polyfills(compiler, polyfills),
    ))
}

// port: IsolatePolyfills#IsolatePolyfills(AbstractCompiler,Polyfills)
fn isolate_polyfills(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let Some(DslValue::Compiler(c)) = args.first() else {
        return Err(bad());
    };
    let c = c.clone();
    isolate_polyfills_value(&mut c.borrow_mut(), &args)
}

// port: IsolatePolyfills#IsolatePolyfills(AbstractCompiler,Polyfills) (compiler borrowed by a lambda)
fn isolate_polyfills_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    isolate_polyfills_value(compiler, &args)
}

// port: IsolatePolyfills#process
fn isolate_polyfills_process(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [
        DslValue::Native(object),
        DslValue::Node(externs),
        DslValue::Node(root),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let mut object = object.borrow_mut();
    let pass = object
        .as_any_mut()
        .downcast_mut::<NativeValue<IsolatePolyfills>>()
        .and_then(|v| v.value.as_mut())
        .ok_or_else(bad)?;
    pass.process(compiler, *externs, *root);
    Ok(DslValue::Null)
}

/// GuardedCallbackTest_Helpers.GetProcessorPass: the anonymous `new CompilerPass() {...}` of
/// GuardedCallbackTest#getProcessor, named.
struct GetProcessorPass;
impl CompilerPass for GetProcessorPass {
    // port: GuardedCallbackTest_Helpers.GetProcessorPass#process
    fn process(&mut self, compiler: &mut Compiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, &mut GuardSwitchingCallback::new());
    }
}

// port: GuardedCallbackTest_Helpers.GetProcessorPass#GetProcessorPass
fn get_processor_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(pass(GetProcessorPass))
}

/// Replaces all guarded name and property references with "GUARDED_NAME" and "GUARDED_PROP",
/// respectively.
struct GuardSwitchingCallback {
    base: GuardedCallback<JsString>,
}
impl GuardSwitchingCallback {
    // port: GuardedCallbackTest_Helpers.GuardSwitchingCallback#GuardSwitchingCallback
    fn new() -> Self {
        Self {
            base: GuardedCallback::new(),
        }
    }
}
impl GuardedCallbackSubclass for GuardSwitchingCallback {
    type Resource = JsString;

    fn guarded_callback(&mut self) -> &mut GuardedCallback<JsString> {
        &mut self.base
    }

    // port: GuardedCallbackTest_Helpers.GuardSwitchingCallback#visitGuarded
    fn visit_guarded(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) {
        if n.is_name(traversal) && self.is_guarded(n.get_string(traversal)) {
            n.set_string(traversal, "GUARDED_NAME");
            traversal.report_code_change();
        } else if n.is_get_prop(traversal) || n.is_opt_chain_get_prop(traversal) {
            // prefix guarded resource name with "." to keep properties distinct from names
            if self.is_guarded(JsString::from(".").concat(&n.get_string(traversal))) {
                n.set_string(traversal, "GUARDED_PROP");
                traversal.report_code_change();
            }
        }
    }
}

// port: RewritePolyfillsTest_Helpers#RewritePolyfillsTest_Helpers
fn rewrite_polyfills_test_helpers(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: REWRITE_POLYFILLS_TEST_HELPERS.into(),
        fields: IndexMap::from([
            ("injectableLibraries".into(), DslValue::Map(vec![])),
            ("injectBeforePass".into(), DslValue::Set(vec![])),
        ]),
        field_types: IndexMap::from([
            ("injectableLibraries".into(), "java.util.Map".into()),
            ("injectBeforePass".into(), "java.util.Set".into()),
        ]),
    }))))
}

/// The holder fields `createRuntimeJsLibManager` reads, copied out of the outer instance.
struct CreateRuntimeJsLibManagerHelper {
    injectable_libraries: IndexMap<String, String>,
    inject_before_pass: IndexSet<String>,
}

// port: RewritePolyfillsTest_Helpers.CreateRuntimeJsLibManager#CreateRuntimeJsLibManager
fn create_runtime_js_lib_manager_helper(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer)] = args.as_slice() else {
        return Err(bad());
    };
    let outer = outer.borrow();
    let mut injectable_libraries = IndexMap::new();
    match outer.fields.get("injectableLibraries") {
        Some(DslValue::Map(entries)) => {
            for (k, v) in entries {
                injectable_libraries.insert(string(k)?, string(v)?);
            }
        }
        Some(DslValue::Typed { value, .. }) => match value.as_ref() {
            DslValue::Map(entries) => {
                for (k, v) in entries {
                    injectable_libraries.insert(string(k)?, string(v)?);
                }
            }
            _ => return Err(bad()),
        },
        _ => return Err(bad()),
    }
    let mut inject_before_pass = IndexSet::new();
    let items = match outer.fields.get("injectBeforePass") {
        Some(DslValue::Set(items) | DslValue::List(items)) => items,
        Some(DslValue::Typed { value, .. }) => match value.as_ref() {
            DslValue::Set(items) | DslValue::List(items) => items,
            _ => return Err(bad()),
        },
        _ => return Err(bad()),
    };
    for item in items {
        inject_before_pass.insert(string(item)?);
    }
    Ok(native(
        CREATE_RUNTIME_JS_LIB_MANAGER,
        Rc::new(CreateRuntimeJsLibManagerHelper {
            injectable_libraries,
            inject_before_pass,
        }),
    ))
}

// port: RewritePolyfillsTest_Helpers.CreateRuntimeJsLibManager#create
fn create_runtime_js_lib_manager(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let Some(DslValue::Compiler(c)) = args.get(1) else {
        return Err(bad());
    };
    let c = c.clone();
    create_runtime_js_lib_manager_value(&args, &mut c.borrow_mut())
}

// port: RewritePolyfillsTest_Helpers.CreateRuntimeJsLibManager#create (compiler borrowed)
fn create_runtime_js_lib_manager_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    create_runtime_js_lib_manager_value(&args, compiler)
}

fn create_runtime_js_lib_manager_value(
    args: &[DslValue],
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(helper), DslValue::Compiler(_)] = args else {
        return Err(bad());
    };
    let helper = {
        let mut helper = helper.borrow_mut();
        helper
            .as_any_mut()
            .downcast_mut::<NativeValue<Rc<CreateRuntimeJsLibManagerHelper>>>()
            .and_then(|v| v.value.clone())
            .ok_or_else(bad)?
    };
    Ok(native(
        RUNTIME_JS_LIB_MANAGER,
        create_runtime_js_lib_manager_impl(&helper, compiler),
    ))
}

// port: RewritePolyfillsTest_Helpers#createRuntimeJsLibManager
fn create_runtime_js_lib_manager_impl(
    helper: &CreateRuntimeJsLibManagerHelper,
    compiler: &mut Compiler,
) -> RuntimeJsLibManager {
    let injectable_libraries = helper.injectable_libraries.clone();
    let mut runtime_libs = RuntimeJsLibManager::create(
        RuntimeLibraryMode::INJECT,
        // stub out the resource parsing
        Box::new(
            move |compiler: &mut Compiler, resource: &str, _path: &str| {
                let code = injectable_libraries.get(resource).unwrap_or_else(|| {
                    panic!("NullPointerException: injectableLibraries.get({resource})")
                });
                Some(compiler.parse_test_code(code.as_str()))
            },
        ),
        Compiler::get_change_tracker_and_ast,
        Box::new(|compiler: &mut Compiler| compiler.get_node_for_code_insertion(None)),
    );
    for to_inject in &helper.inject_before_pass {
        runtime_libs.ensure_library_injected(compiler, to_inject, /* force= */ false);
    }
    runtime_libs
}
