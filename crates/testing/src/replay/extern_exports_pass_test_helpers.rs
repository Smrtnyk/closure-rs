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
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/ExternExportsPassTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../ExternExportsPassTest_Helpers.java` (DSL
//! name `ExternExportsPassTest_Helpers`), itself copied from ExternExportsPassTest.java: the
//! holder standing in for the test instance, the Postcondition lambda of compileAndExportExterns
//! and the Consumer lambda of compileAndCheck.
use crate::compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart};
use crate::replay::replay_dsl::CompilerHandle;
use crate::{
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::compiler_options::CompilerOptions;
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::js_string::JsString;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.ExternExportsPassTest_Helpers";
const POSTCONDITION: &str = "com.google.javascript.jscomp.ExternExportsPassTest_Helpers$$Lambda";
const CONSUMER: &str = "com.google.javascript.jscomp.ExternExportsPassTest_Helpers$$Lambda";

// port: ReplayDsl#invoke (resolved signatures backed by ExternExportsPassTest_Helpers)
pub fn entry(signature: &str) -> Option<crate::replay::registry::Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.ExternExportsPassTest_Helpers#<init>()" => holder,
        "com.google.javascript.jscomp.ExternExportsPassTest_Helpers#setUp()" => set_up,
        "com.google.javascript.jscomp.ExternExportsPassTest_Helpers#postcondition(java.util.function.Consumer)" => {
            postcondition
        }
        "com.google.javascript.jscomp.ExternExportsPassTest_Helpers#compileAndCheckConsumer(java.lang.String)" => {
            compile_and_check_consumer
        }
        "com.google.javascript.jscomp.ExternExportsPassTest_Helpers#useExportsAsExternsWithClassConsumer(java.lang.String)" => {
            use_exports_as_externs_with_class_consumer
        }
        _ => return None,
    })
}

/// `final class ExternExportsPassTest_Helpers extends CompilerTestCase`.
struct ExternExportsPassTestHelpers {
    /// The CompilerTestCase state of the holder, created by setUp (super.setUp(); enableTypeCheck())
    /// for a nested test(...) on the holder.
    harness: Option<CompilerTestCase>,
}

impl NativeObject for ExternExportsPassTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (the holder declares no fields of its own)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ExternExportsPassTest_Helpers#ExternExportsPassTest_Helpers
fn holder(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(
        ExternExportsPassTestHelpers { harness: None },
    ))))
}

// port: ExternExportsPassTest_Helpers#setUp
fn set_up(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(holder)] = args.as_slice() else {
        return Err(bad());
    };
    let mut holder = holder.borrow_mut();
    let Some(holder) = holder
        .as_any_mut()
        .downcast_mut::<ExternExportsPassTestHelpers>()
    else {
        return Err(bad());
    };
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness.enable_type_check()?;
    holder.harness = Some(harness);
    Ok(DslValue::Null)
}

/// The Postcondition lambda of compileAndExportExterns (capture `consumer`, possibly null).
struct Postcondition {
    consumer: DslValue,
}

impl NativeObject for Postcondition {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        POSTCONDITION
    }
    // port: ReplayValues#findField (lambda: its capture `consumer` as arg$1)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([(
            "arg$1".to_string(),
            self.consumer.clone(),
        )]))
    }
    // port: UnitRecorder#dump (lambda captures)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        Some(self.fields())
    }
    // port: ExternExportsPassTest_Helpers#postcondition (the returned lambda's verify)
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method != "verify" {
            return Err(Throwable::Unported(format!("{POSTCONDITION}#{method}")));
        }
        let [DslValue::Compiler(compiler)] = args.as_slice() else {
            return Err(bad());
        };
        match &self.consumer {
            DslValue::Null => {}
            DslValue::Native(consumer) => {
                let extern_export = compiler.borrow().get_result().extern_export;
                let value = extern_export.map_or(DslValue::Null, |s| {
                    DslValue::String(JsString::from(s.as_str()))
                });
                consumer.borrow_mut().call("accept", vec![value])?;
            }
            other => {
                return Err(Throwable::Unported(format!(
                    "{}#accept",
                    other.class_name()
                )));
            }
        }
        Ok(DslValue::Null)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ExternExportsPassTest_Helpers#postcondition
fn postcondition(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(_holder), consumer] = args.as_slice() else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(Postcondition {
        consumer: consumer.clone(),
    }))))
}

/// The Consumer lambda of compileAndCheck (capture `expected`).
struct CompileAndCheckConsumer {
    expected: JsString,
}

impl NativeObject for CompileAndCheckConsumer {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        CONSUMER
    }
    // port: ReplayValues#findField (lambda: its capture `expected` as arg$1)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([(
            "arg$1".to_string(),
            DslValue::String(self.expected.clone()),
        )]))
    }
    // port: UnitRecorder#dump (lambda captures)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        Some(self.fields())
    }
    // port: ExternExportsPassTest_Helpers#compileAndCheckConsumer (the returned lambda's accept)
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method != "accept" {
            return Err(Throwable::Unported(format!("{CONSUMER}#{method}")));
        }
        let [generated_externs] = args.as_slice() else {
            return Err(bad());
        };
        let generated_externs = match generated_externs {
            DslValue::String(s) => s.to_string_lossy(),
            // generatedExterns.replace(..) on null
            DslValue::Null => {
                return Err(Throwable::Exception {
                    class: "java.lang.NullPointerException".into(),
                    message: None,
                });
            }
            _ => return Err(bad()),
        };
        let fileoverview = concat!(
            "/**\n",
            " * @fileoverview Generated externs.\n",
            " * @externs\n",
            " */\n",
        );
        // NOTE(sdh): The type checker just produces {?}.
        // For now we will not worry about this distinction and just normalize it.
        let generated_externs = generated_externs.replace("?=", "?");

        let expected = format!("{fileoverview}{}", self.expected.to_string_lossy());
        crate::throwable::assert_that(
            generated_externs == expected,
            format!("expected: {expected}\nbut was : {generated_externs}"),
        )?;
        Ok(DslValue::Null)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ExternExportsPassTest_Helpers#compileAndCheckConsumer
fn compile_and_check_consumer(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(_holder), DslValue::String(expected)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(
        CompileAndCheckConsumer {
            expected: expected.clone(),
        },
    ))))
}

/// The CompilerTestCase overrides of the holder (verbatim from ExternExportsPassTest).
struct HolderHooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for HolderHooks {
    // port: ExternExportsPassTest_Helpers#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        options.set_extern_exports_path(Some("exports.js".into()));
        // Check types so we can make sure our exported externs have type information.
        options.set_check_symbols(true);
        Ok(options)
    }
    // port: ExternExportsPassTest_Helpers#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass =
            closure_jscomp::extern_exports_pass::ExternExportsPass::new(&mut compiler.borrow_mut());
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
    }
    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "ExternExportsPassTest_Helpers".into()
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// The Consumer lambda of the test method useExportsAsExternsWithClass (captures the test
/// instance `arg$1`, here the holder, and `clientSource` `arg$2`).
struct UseExportsAsExternsWithClassConsumer {
    holder: DslValue,
    client_source: JsString,
}

impl NativeObject for UseExportsAsExternsWithClassConsumer {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        CONSUMER
    }
    // port: ReplayValues#findField (lambda: its captures as arg$1, arg$2)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::from_iter([
            ("arg$1".to_string(), self.holder.clone()),
            (
                "arg$2".to_string(),
                DslValue::String(self.client_source.clone()),
            ),
        ]))
    }
    // port: UnitRecorder#dump (lambda captures)
    fn lambda_captures(&self) -> Option<Result<IndexMap<String, DslValue>, Throwable>> {
        Some(self.fields())
    }
    // port: ExternExportsPassTest_Helpers#useExportsAsExternsWithClassConsumer (the returned
    // lambda's accept: compileAndExportExterns(clientSource, MINIMAL_EXTERNS + generatedExterns))
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method != "accept" {
            return Err(Throwable::Unported(format!("{CONSUMER}#{method}")));
        }
        let [generated_externs] = args.as_slice() else {
            return Err(bad());
        };
        // MINIMAL_EXTERNS + generatedExterns ("null" when the pass exported nothing)
        let mut externs = crate::compiler_test_case::MINIMAL_EXTERNS
            .clone()?
            .as_units()
            .to_vec();
        match generated_externs {
            DslValue::String(s) => externs.extend_from_slice(s.as_units()),
            DslValue::Null => externs.extend("null".encode_utf16()),
            _ => return Err(bad()),
        }
        let DslValue::Native(holder) = &self.holder else {
            return Err(bad());
        };
        let mut holder = holder.borrow_mut();
        let holder = holder
            .as_any_mut()
            .downcast_mut::<ExternExportsPassTestHelpers>()
            .ok_or_else(bad)?;
        let harness = holder.harness.as_mut().ok_or_else(|| {
            Throwable::HarnessError("ExternExportsPassTest_Helpers#setUp did not run".into())
        })?;
        compile_and_export_externs(harness, &self.client_source, JsString::from_units(externs))?;
        Ok(DslValue::Null)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ExternExportsPassTest_Helpers#compileAndExportExterns(String,String)
fn compile_and_export_externs(
    harness: &mut CompilerTestCase,
    js: &JsString,
    externs: JsString,
) -> Result<(), Throwable> {
    // compileAndExportExterns(js, externs, null)
    let mut source = JsString::from(concat!(
        "/** @const */ var goog = {};\n",
        "goog.exportSymbol = function(a, b) {};\n",
        "goog.exportProperty = function(a, b, c) {};\n",
    ))
    .as_units()
    .to_vec();
    source.extend_from_slice(js.as_units());

    let mut hooks = HolderHooks {
        ctx: Ctx::new(
            "ExternExportsPassTest_Helpers".into(),
            crate::replay::replay_values::object([]),
            IndexMap::<_, _>::default(),
            crate::replay::registry::Registry::from_tsv(
                "descriptor\tlookup\tdeclaringClass\tsignature\twidened\n",
            )?,
        ),
    };
    harness.test(
        &mut hooks,
        vec![
            TestPart::Externs(CompilerTestCase::externs(externs)),
            TestPart::Sources(CompilerTestCase::srcs(JsString::from_units(source))),
            TestPart::Postcondition(DslValue::Native(Rc::new(RefCell::new(Postcondition {
                consumer: DslValue::Null,
            })))),
        ],
    )
}

// port: ExternExportsPassTest_Helpers#useExportsAsExternsWithClassConsumer
fn use_exports_as_externs_with_class_consumer(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [
        holder @ DslValue::Native(_),
        DslValue::String(client_source),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(
        UseExportsAsExternsWithClassConsumer {
            holder: holder.clone(),
            client_source: client_source.clone(),
        },
    ))))
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
