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
 * Copyright 2021 The Closure Compiler Authors.
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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/LocaleDataPassesTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../LocaleDataPassesTest_Helpers.java` (DSL
//! name `LocaleDataPassesTest_Helpers`), itself copied from LocaleDataPassesTest.java: the
//! `TestMode` enum, the holder field `testMode` and `getProcessor` with its two anonymous
//! CompilerPasses.
use crate::{
    jscomp_api::Compiler,
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    locale_data_passes::{LocaleSubstitutions, ProtectGoogLocale},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.LocaleDataPassesTest_Helpers";
const TEST_MODE: &str = "com.google.javascript.jscomp.LocaleDataPassesTest_Helpers$TestMode";

/// port: LocaleDataPassesTest_Helpers.TestMode
/// Indicates which part of the replacement we're currently testing
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
enum TestMode {
    PROTECT_DATA,
    // Test replacement of the protected function call form with the final message values.
    REPLACE_PROTECTED_DATA,
}

impl TestMode {
    // port: LocaleDataPassesTest_Helpers.TestMode#name
    fn name(self) -> &'static str {
        match self {
            TestMode::PROTECT_DATA => "PROTECT_DATA",
            TestMode::REPLACE_PROTECTED_DATA => "REPLACE_PROTECTED_DATA",
        }
    }
    // port: LocaleDataPassesTest_Helpers.TestMode#valueOf
    fn value_of(name: &str) -> Option<TestMode> {
        match name {
            "PROTECT_DATA" => Some(TestMode::PROTECT_DATA),
            "REPLACE_PROTECTED_DATA" => Some(TestMode::REPLACE_PROTECTED_DATA),
            _ => None,
        }
    }
}

/// `final class LocaleDataPassesTest_Helpers`.
pub struct LocaleDataPassesTestHelpers {
    // Messages returned from fake bundle, keyed by `JsMessage.id`.
    test_mode: TestMode,
}

impl NativeObject for LocaleDataPassesTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "testMode".into(),
            DslValue::Enum {
                class: TEST_MODE.into(),
                name: self.test_mode.name().into(),
            },
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value) {
            ("testMode", DslValue::Enum { class, name }) if class == TEST_MODE => {
                self.test_mode = TestMode::value_of(&name).ok_or_else(bad)?;
                Ok(())
            }
            (name, _) => Err(Throwable::Unported(format!("{HOLDER}#{name}"))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: LocaleDataPassesTest_Helpers#LocaleDataPassesTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        LocaleDataPassesTestHelpers {
            test_mode: TestMode::PROTECT_DATA,
        },
    ))))
}

// port: LocaleDataPassesTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let test_mode = {
        let mut this = this.borrow_mut();
        let this = this
            .as_any_mut()
            .downcast_mut::<LocaleDataPassesTestHelpers>()
            .ok_or_else(bad)?;
        this.test_mode
    };
    Ok(match test_mode {
        TestMode::PROTECT_DATA => DslValue::Native(Rc::new(RefCell::new(ProtectDataPass))),
        TestMode::REPLACE_PROTECTED_DATA => {
            DslValue::Native(Rc::new(RefCell::new(ReplaceProtectedDataPass)))
        }
    })
}

/// The anonymous `new CompilerPass() { ... }` of `case PROTECT_DATA`
/// (`LocaleDataPassesTest_Helpers$1`). It captures `compiler`; the Rust pass receives the
/// compiler at process time.
struct ProtectDataPass;

impl NativeObject for ProtectDataPass {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.LocaleDataPassesTest_Helpers$1"
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: LocaleDataPassesTest_Helpers$1#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let mut extract = ProtectGoogLocale::new(compiler);
        extract.process(compiler, externs, root);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The anonymous `new CompilerPass() { ... }` of `case REPLACE_PROTECTED_DATA`
/// (`LocaleDataPassesTest_Helpers$2`).
struct ReplaceProtectedDataPass;

impl NativeObject for ReplaceProtectedDataPass {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.LocaleDataPassesTest_Helpers$2"
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: LocaleDataPassesTest_Helpers$2#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let mut extract = ProtectGoogLocale::new(compiler);
        extract.process(compiler, externs, root);
        let locale = compiler.get_options().get_locale().map(str::to_string);
        let mut subs = LocaleSubstitutions::new(compiler, locale.as_deref());
        subs.process(compiler, externs, root);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
