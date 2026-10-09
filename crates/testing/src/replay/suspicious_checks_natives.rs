/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
 * Copyright 2024 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckDebuggerStatement.java,
//   src/com/google/javascript/jscomp/CheckGlobalThis.java,
//   src/com/google/javascript/jscomp/CheckJSDoc.java,
//   src/com/google/javascript/jscomp/CheckMissingReturn.java,
//   src/com/google/javascript/jscomp/CheckSideEffects.java,
//   src/com/google/javascript/jscomp/CheckSuper.java,
//   src/com/google/javascript/jscomp/CheckSuspiciousCode.java,
//   src/com/google/javascript/jscomp/CheckTypeImportCodeReferences.java,
//   src/com/google/javascript/jscomp/CheckUnreachableCode.java,
//   src/com/google/javascript/jscomp/Es6CheckModule.java,
//   src/com/google/javascript/jscomp/RewriteCallerCodeLocation.java,
//   src/com/google/javascript/jscomp/lint/CheckDuplicateCase.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for the suspicious-checks passes (CheckSideEffects, CheckSuspiciousCode,
//! CheckJSDoc, CheckSuper, the CFG checks and the other suspicious-code checks).
use crate::{
    replay::{
        registry::Entry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    check_debugger_statement::CheckDebuggerStatement,
    check_global_this::CheckGlobalThis,
    check_jsdoc::CheckJSDoc,
    check_missing_return::CheckMissingReturn,
    check_side_effects::{CheckSideEffects, StripProtection},
    check_super::CheckSuper,
    check_suspicious_code::CheckSuspiciousCode,
    check_type_import_code_references::CheckTypeImportCodeReferences,
    check_unreachable_code::CheckUnreachableCode,
    compiler_pass::CompilerPass,
    es6_check_module::Es6CheckModule,
    lint::check_duplicate_case::CheckDuplicateCase,
    node_traversal::Callback,
    rewrite_caller_code_location::RewriteCallerCodeLocation,
};
use std::{cell::RefCell, rc::Rc};

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.CheckDebuggerStatement#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_debugger_statement
        }
        "com.google.javascript.jscomp.lint.CheckDuplicateCase#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_duplicate_case
        }
        "com.google.javascript.jscomp.CheckGlobalThis#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_global_this
        }
        "com.google.javascript.jscomp.CheckJSDoc#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_jsdoc
        }
        "com.google.javascript.jscomp.CheckMissingReturn#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_missing_return
        }
        "com.google.javascript.jscomp.CheckSideEffects#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean,boolean)" => {
            check_side_effects
        }
        "com.google.javascript.jscomp.CheckSideEffects$StripProtection#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            strip_protection
        }
        "com.google.javascript.jscomp.CheckSuper#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_super
        }
        "com.google.javascript.jscomp.CheckSuspiciousCode#<init>()" => check_suspicious_code,
        "com.google.javascript.jscomp.CheckTypeImportCodeReferences#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_type_import_code_references
        }
        "com.google.javascript.jscomp.CheckUnreachableCode#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            check_unreachable_code
        }
        "com.google.javascript.jscomp.Es6CheckModule#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            es6_check_module
        }
        "com.google.javascript.jscomp.RewriteCallerCodeLocation#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            rewrite_caller_code_location
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (receiver cast)
fn compiler(args: &[DslValue]) -> Result<CompilerHandle, Throwable> {
    if let Some(DslValue::Compiler(c)) = args.first() {
        Ok(c.clone())
    } else {
        Err(bad())
    }
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

// port: ReplayDsl#invoke (CompilerPass return value)
fn pass(pass: impl CompilerPass + 'static) -> DslValue {
    DslValue::Pass(Rc::new(RefCell::new(Box::new(pass))))
}

/// A NodeTraversal.Callback that is not a CompilerPass (it is passed to CombinedCompilerPass).
struct NativeCallback<C: Callback + 'static> {
    class: &'static str,
    callback: C,
}
impl<C: Callback + 'static> NativeObject for NativeCallback<C> {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        self.class
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    // port: ReplayDsl#invoke (native traversal callback receiver)
    fn as_traversal_callback(&mut self) -> Option<&mut dyn Callback> {
        Some(&mut self.callback)
    }
}
// port: ReplayDsl#invoke (NodeTraversal.Callback return value)
fn callback(class: &'static str, callback: impl Callback + 'static) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativeCallback { class, callback })))
}

// port: CheckDebuggerStatement#CheckDebuggerStatement
fn check_debugger_statement(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(CheckDebuggerStatement::new()))
}
// port: CheckDuplicateCase#CheckDuplicateCase
fn check_duplicate_case(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(CheckDuplicateCase::new()))
}
// port: CheckGlobalThis#CheckGlobalThis
fn check_global_this(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(callback(
        "com.google.javascript.jscomp.CheckGlobalThis",
        CheckGlobalThis::new(),
    ))
}
// port: CheckJSDoc#CheckJSDoc
fn check_jsdoc(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(CheckJSDoc::new()))
}
// port: CheckMissingReturn#CheckMissingReturn
fn check_missing_return(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(callback(
        "com.google.javascript.jscomp.CheckMissingReturn",
        CheckMissingReturn::new(),
    ))
}
// port: CheckSideEffects#CheckSideEffects
fn check_side_effects(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(_),
        DslValue::Bool(report),
        DslValue::Bool(protect_side_effect_free_code),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    Ok(pass(CheckSideEffects::new(
        *report,
        *protect_side_effect_free_code,
    )))
}
// port: CheckSideEffects.StripProtection#StripProtection
fn strip_protection(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(StripProtection::new()))
}
// port: CheckSuper#CheckSuper
fn check_super(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(CheckSuper::new()))
}
// port: CheckSuspiciousCode#CheckSuspiciousCode
fn check_suspicious_code(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(callback(
        "com.google.javascript.jscomp.CheckSuspiciousCode",
        CheckSuspiciousCode::new(),
    ))
}
// port: CheckTypeImportCodeReferences#CheckTypeImportCodeReferences
fn check_type_import_code_references(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(CheckTypeImportCodeReferences::new()))
}
// port: CheckUnreachableCode#CheckUnreachableCode
fn check_unreachable_code(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(callback(
        "com.google.javascript.jscomp.CheckUnreachableCode",
        CheckUnreachableCode::new(),
    ))
}
// port: Es6CheckModule#Es6CheckModule
fn es6_check_module(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(Es6CheckModule::new()))
}
// port: RewriteCallerCodeLocation#RewriteCallerCodeLocation
fn rewrite_caller_code_location(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass_value = RewriteCallerCodeLocation::new(&mut c.borrow_mut());
    Ok(pass(pass_value))
}
