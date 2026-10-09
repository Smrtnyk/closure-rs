/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AliasStrings.java,
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/ExportTestFunctions.java,
//   src/com/google/javascript/jscomp/ExternExportsPass.java,
//   src/com/google/javascript/jscomp/FunctionRewriter.java,
//   src/com/google/javascript/jscomp/ProcessTweaks.java,
//   src/com/google/javascript/jscomp/StripCode.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for the API passes: AliasStrings, FunctionRewriter,
//! CreateSyntheticBlocks, ExportTestFunctions, ProcessTweaks, StripCode, ExternExportsPass and
//! ReplaceStrings, plus the `*Test_Helpers` classes their descriptors name.
use crate::{
    replay::{
        registry::{BorrowedEntry, Entry},
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    Compiler, alias_strings::AliasStrings, compiler_options::AliasStringsMode,
    compiler_pass::CompilerPass,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc};

fn bad(s: &str) -> Throwable {
    Throwable::HarnessError(format!("api-passes replay adapter: {s}"))
}

// port: ReplayDsl#invoke (resolved signatures backed by the api-passes ports)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.AliasStrings#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.JSChunkGraph,boolean,com.google.javascript.jscomp.CompilerOptions$AliasStringsMode)" => {
            new_alias_strings
        }
        "com.google.javascript.jscomp.FunctionRewriter#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            new_function_rewriter
        }
        "com.google.javascript.jscomp.ExportTestFunctions#<init>(com.google.javascript.jscomp.AbstractCompiler,java.lang.String,java.lang.String)" => {
            new_export_test_functions
        }
        "com.google.javascript.jscomp.ProcessTweaks#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean)" => {
            new_process_tweaks
        }
        "com.google.javascript.jscomp.StripCode#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.common.collect.ImmutableSet,com.google.common.collect.ImmutableSet,com.google.common.collect.ImmutableSet,boolean)" => {
            new_strip_code
        }
        EXTERN_EXPORTS_PASS_INIT => new_extern_exports_pass,
        "com.google.javascript.jscomp.CreateSyntheticBlocksTest_Helpers$GetProcessor#<init>(java.lang.String)" => {
            crate::replay::create_synthetic_blocks_test_helpers::get_processor_new
        }
        "com.google.javascript.jscomp.CreateSyntheticBlocksTest_Helpers$GetProcessor#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            crate::replay::create_synthetic_blocks_test_helpers::get_processor
        }
        REPLACE_STRINGS_INIT => crate::replay::replace_strings_test_helpers::new_replace_strings,
        "com.google.javascript.jscomp.ReplaceStringsTest_Helpers$GetProcessorPass#<init>(com.google.javascript.jscomp.Compiler,com.google.javascript.jscomp.ReplaceStrings,boolean,boolean)" => {
            crate::replay::replace_strings_test_helpers::new_get_processor_pass
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (String argument cast; null stays null)
fn arg_opt_string(args: &[DslValue], i: usize) -> Result<Option<JsString>, Throwable> {
    match args.get(i) {
        Some(DslValue::String(s)) => Ok(Some(s.clone())),
        Some(DslValue::Null) => Ok(None),
        Some(DslValue::Typed { value, .. }) => arg_opt_string(std::slice::from_ref(value), 0),
        _ => Err(bad(&format!("argument {i} is not a string"))),
    }
}

// port: ReplayDsl#invoke (argument casts)
fn arg_bool(args: &[DslValue], i: usize) -> Result<bool, Throwable> {
    match args.get(i) {
        Some(DslValue::Bool(b)) => Ok(*b),
        Some(DslValue::Typed { value, .. }) => arg_bool(std::slice::from_ref(value), 0),
        _ => Err(bad(&format!("argument {i} is not a boolean"))),
    }
}

// port: ReplayDsl#invoke (ImmutableSet<String> argument cast)
fn arg_string_set(args: &[DslValue], i: usize) -> Result<IndexSet<String>, Throwable> {
    let Some(v) = args.get(i) else {
        return Err(bad(&format!("missing argument {i}")));
    };
    let (DslValue::Set(items) | DslValue::List(items)) = v.untyped() else {
        return Err(bad(&format!("argument {i} is not a set")));
    };
    items
        .iter()
        .map(|item| match item.untyped() {
            DslValue::String(s) => Ok(s.to_string()),
            _ => Err(bad(&format!("argument {i} has a non-string element"))),
        })
        .collect()
}

// port: ReplayDsl#invoke (enum argument cast)
fn arg_enum_name(args: &[DslValue], i: usize) -> Result<String, Throwable> {
    match args.get(i) {
        Some(DslValue::Enum { name, .. }) => Ok(name.clone()),
        Some(DslValue::Typed { value, .. }) => arg_enum_name(std::slice::from_ref(value), 0),
        _ => Err(bad(&format!("argument {i} is not an enum constant"))),
    }
}

/// The real `AliasStrings` pass inside its replay adapter.
struct NativeAliasStrings(AliasStrings);

impl NativeObject for NativeAliasStrings {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.AliasStrings"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        matches!(
            class,
            "com.google.javascript.jscomp.AliasStrings"
                | "com.google.javascript.jscomp.CompilerPass"
                | "com.google.javascript.jscomp.NodeTraversal$Callback"
                | "java.lang.Object"
        )
    }
    // port: ReplayValues#findField (native object adapter; no producer roots)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value.untyped()) {
            ("unitTestHashReductionMask", DslValue::Int(v)) => {
                self.0.unit_test_hash_reduction_mask = *v;
                Ok(())
            }
            (name, _) => Err(Throwable::Unported(format!(
                "com.google.javascript.jscomp.AliasStrings#{name}"
            ))),
        }
    }
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.0.process(compiler, externs, root);
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: AliasStrings#AliasStrings
fn new_alias_strings(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let output_string_usage = arg_bool(&args, 2)?;
    let mode = arg_enum_name(&args, 3)?;
    let mode = AliasStringsMode::value_of(&mode).ok_or_else(|| bad("AliasStringsMode"))?;
    Ok(DslValue::Native(Rc::new(RefCell::new(NativeAliasStrings(
        AliasStrings::new(output_string_usage, mode),
    )))))
}

// port: FunctionRewriter#FunctionRewriter
fn new_function_rewriter(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
        closure_jscomp::function_rewriter::FunctionRewriter::new(),
    )))))
}

// port: ExportTestFunctions#ExportTestFunctions
fn new_export_test_functions(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let export_symbol_function =
        arg_opt_string(&args, 1)?.ok_or_else(|| bad("exportSymbolFunction is null"))?;
    let export_property_function = arg_opt_string(&args, 2)?;
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
        closure_jscomp::export_test_functions::ExportTestFunctions::new(
            export_symbol_function,
            export_property_function,
        ),
    )))))
}

/// A ported pass whose Java class the DSL names when it calls a method on it (descriptors that
/// call `new X(..).process(externs, root)` resolve `X.process` by the runtime class).
struct NativePass {
    class: &'static str,
    pass: Box<dyn CompilerPass>,
}

impl NativeObject for NativePass {
    fn class_name(&self) -> &str {
        self.class
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class
            || matches!(
                class,
                "com.google.javascript.jscomp.CompilerPass" | "java.lang.Object"
            )
    }
    // port: ReplayValues#findField (native object adapter; no producer roots)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    fn process(
        &mut self,
        compiler: &mut Compiler,
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

pub(crate) fn native_pass(class: &'static str, pass: Box<dyn CompilerPass>) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativePass { class, pass })))
}

/// `X#process(Node,Node)` called by a descriptor on a pass it built, while the replayed
/// compiler is lent to the processor.
pub fn register(registry: &mut crate::replay::registry::Registry) {
    for signature in [
        "com.google.javascript.jscomp.ProcessTweaks#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)",
        "com.google.javascript.jscomp.StripCode#process(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)",
    ] {
        let borrowed: BorrowedEntry = process_borrowed;
        registry.register_with_compiler(signature, borrowed);
    }
    let borrowed: BorrowedEntry = new_extern_exports_pass_borrowed;
    registry.register_with_compiler(EXTERN_EXPORTS_PASS_INIT, borrowed);
    let borrowed: BorrowedEntry =
        crate::replay::replace_strings_test_helpers::new_replace_strings_borrowed;
    registry.register_with_compiler(REPLACE_STRINGS_INIT, borrowed);
}

// port: CompilerPass#process (DSL method call on a native pass)
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
        return Err(bad("process needs a native pass and two nodes"));
    };
    pass.borrow_mut().process(compiler, *externs, *root)?;
    Ok(DslValue::Null)
}

// port: ProcessTweaks#ProcessTweaks
fn new_process_tweaks(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let strip_tweaks = arg_bool(&args, 1)?;
    Ok(native_pass(
        "com.google.javascript.jscomp.ProcessTweaks",
        Box::new(closure_jscomp::process_tweaks::ProcessTweaks::new(
            strip_tweaks,
        )),
    ))
}

// port: StripCode#StripCode
fn new_strip_code(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let strip_types = arg_string_set(&args, 1)?;
    let strip_name_suffixes = arg_string_set(&args, 2)?;
    let strip_name_prefixes = arg_string_set(&args, 3)?;
    let enable_tweak_stripping = arg_bool(&args, 4)?;
    Ok(native_pass(
        "com.google.javascript.jscomp.StripCode",
        Box::new(closure_jscomp::strip_code::StripCode::new(
            &strip_types,
            &strip_name_suffixes,
            &strip_name_prefixes,
            enable_tweak_stripping,
        )),
    ))
}

const REPLACE_STRINGS_INIT: &str = "com.google.javascript.jscomp.ReplaceStrings#<init>(com.google.javascript.jscomp.AbstractCompiler,java.lang.String,java.util.List)";

const EXTERN_EXPORTS_PASS_INIT: &str = "com.google.javascript.jscomp.ExternExportsPass#<init>(com.google.javascript.jscomp.AbstractCompiler)";

// port: ExternExportsPass#ExternExportsPass
fn new_extern_exports_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c)] = args.as_slice() else {
        return Err(bad("ExternExportsPass needs the compiler"));
    };
    let pass = closure_jscomp::extern_exports_pass::ExternExportsPass::new(&mut c.borrow_mut());
    Ok(native_pass(
        "com.google.javascript.jscomp.ExternExportsPass",
        Box::new(pass),
    ))
}

// port: ExternExportsPass#ExternExportsPass (compiler loaned to a running factory)
fn new_extern_exports_pass_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad("ExternExportsPass needs the compiler"));
    };
    Ok(native_pass(
        "com.google.javascript.jscomp.ExternExportsPass",
        Box::new(closure_jscomp::extern_exports_pass::ExternExportsPass::new(
            compiler,
        )),
    ))
}
