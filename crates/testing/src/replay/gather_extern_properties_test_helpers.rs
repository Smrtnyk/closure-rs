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
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/GatherExternProperties.java,
//   test/com/google/javascript/jscomp/GatherExternPropertiesTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../GatherExternPropertiesTest_Helpers.java`
//! (DSL names `GatherExternPropertiesTest_Helpers` and
//! `GatherExternPropertiesTest_Helpers.ExpectExterns`), itself copied from
//! GatherExternPropertiesTest.java: the field `mode`, `getProcessor`, and the expectExterns
//! postcondition lambda.
use crate::{
    jscomp_api::Compiler,
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    gather_extern_properties::{GatherExternProperties, Mode},
};
use closure_rhino::node::NodeId;
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.GatherExternPropertiesTest_Helpers";
const EXPECT_EXTERNS: &str =
    "com.google.javascript.jscomp.GatherExternPropertiesTest_Helpers$ExpectExterns";
const MODE: &str = "com.google.javascript.jscomp.GatherExternProperties$Mode";

/// `final class GatherExternPropertiesTest_Helpers`.
pub struct GatherExternPropertiesTestHelpers {
    mode: Option<Mode>,
}

// port: GatherExternProperties.Mode#name
fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::CHECK => "CHECK",
        Mode::OPTIMIZE => "OPTIMIZE",
        Mode::CHECK_AND_OPTIMIZE => "CHECK_AND_OPTIMIZE",
    }
}

// port: GatherExternProperties.Mode#valueOf
fn mode_value_of(name: &str) -> Result<Mode, Throwable> {
    match name {
        "CHECK" => Ok(Mode::CHECK),
        "OPTIMIZE" => Ok(Mode::OPTIMIZE),
        "CHECK_AND_OPTIMIZE" => Ok(Mode::CHECK_AND_OPTIMIZE),
        _ => Err(bad()),
    }
}

impl NativeObject for GatherExternPropertiesTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert(
            "mode".into(),
            self.mode.map_or(DslValue::Null, |m| DslValue::Enum {
                class: MODE.into(),
                name: mode_name(m).into(),
            }),
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value.untyped()) {
            ("mode", DslValue::Null) => {
                self.mode = None;
                Ok(())
            }
            ("mode", DslValue::Enum { class, name }) if class == MODE => {
                self.mode = Some(mode_value_of(name)?);
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

// port: GatherExternPropertiesTest_Helpers#GatherExternPropertiesTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        GatherExternPropertiesTestHelpers { mode: None },
    ))))
}

// port: GatherExternPropertiesTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let mut this = this.borrow_mut();
    let this = this
        .as_any_mut()
        .downcast_mut::<GatherExternPropertiesTestHelpers>()
        .ok_or_else(bad)?;
    this.mode = Some(this.mode.unwrap_or(Mode::OPTIMIZE));
    let pass = GatherExternProperties::new(&compiler.borrow(), this.mode.unwrap());
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeGatherExternProperties(pass),
    ))))
}

/// The `GatherExternProperties` instance getProcessor returns.
struct NativeGatherExternProperties(GatherExternProperties);

impl NativeObject for NativeGatherExternProperties {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.GatherExternProperties"
    }
    // port: ReplayValues#findField (native object adapter)
    // UnitRecorder#collect walks the processor for result producers: GatherExternProperties
    // holds only its property set, its mode and the compiler, none of them a result producer.
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: GatherExternProperties#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.0.process(compiler, externs, root);
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The generated `static final class ExpectExterns implements Postcondition`.
struct ExpectExterns {
    properties: Vec<DslValue>,
}

impl NativeObject for ExpectExterns {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        EXPECT_EXTERNS
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert(
            "properties".into(),
            DslValue::Array {
                component: "java.lang.String".into(),
                items: self.properties.clone(),
            },
        );
        Ok(fields)
    }
    // port: GatherExternPropertiesTest_Helpers.ExpectExterns#verify
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method != "verify" {
            return Err(Throwable::Unported(format!("{EXPECT_EXTERNS}#{method}")));
        }
        let [DslValue::Compiler(compiler)] = args.as_slice() else {
            return Err(bad());
        };
        let compiler = compiler.borrow();
        // assertThat(compiler.getExternProperties()).containsExactlyElementsIn(properties)
        let Some(actual) = compiler.get_extern_properties() else {
            return Err(Throwable::Assertion {
                message: "expected an iterable, but was null".into(),
            });
        };
        let mut expected = Vec::new();
        for p in &self.properties {
            let DslValue::String(s) = p else {
                return Err(bad());
            };
            expected.push(s.to_string_lossy());
        }
        contains_exactly_elements_in(actual.iter().cloned().collect(), expected)?;
        Ok(DslValue::Null)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: IterableSubject#containsExactlyElementsIn (multiset equality, any order)
fn contains_exactly_elements_in(
    mut actual: Vec<String>,
    mut expected: Vec<String>,
) -> Result<(), Throwable> {
    let description = format!("{actual:?} contains exactly {expected:?}");
    actual.sort();
    expected.sort();
    crate::throwable::assert_that(actual == expected, description)
}

// port: GatherExternPropertiesTest_Helpers.ExpectExterns#ExpectExterns
pub fn expect_externs(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Array { items, .. }] = args.as_slice() else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(ExpectExterns {
        properties: items.clone(),
    }))))
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
