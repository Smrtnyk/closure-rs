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
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckRegExp.java,
//   test/com/google/javascript/jscomp/CheckRegExpTest.java.

//! Port of the unit-corpus helper `CheckRegExpTest_Helpers.java` (oracle/replay/helpers): the
//! holder of CheckRegExpTest's fields `last`, `reportErrors` and
//! `assumeAllGlobalRegexpUsagesVisible`, and its copy of CheckRegExpTest#getProcessor.
use crate::{
    jscomp_api::Compiler,
    replay::{
        registry::Entry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject, Object},
    },
    throwable::Throwable,
};
use closure_jscomp::{check_reg_exp::CheckRegExp, compiler_pass::CompilerPass};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.CheckRegExpTest_Helpers";
const CHECK_REG_EXP: &str = "com.google.javascript.jscomp.CheckRegExp";

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.CheckRegExpTest_Helpers#<init>()" => holder,
        "com.google.javascript.jscomp.CheckRegExpTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)" => {
            get_processor
        }
        _ => return None,
    })
}

// port: CheckRegExpTest_Helpers#CheckRegExpTest_Helpers
fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    // private @Nullable CheckRegExp last = null;
    fields.insert("last".to_string(), DslValue::Null);
    field_types.insert("last".to_string(), CHECK_REG_EXP.to_string());
    // private boolean reportErrors;
    // private boolean assumeAllGlobalRegexpUsagesVisible;
    for name in ["reportErrors", "assumeAllGlobalRegexpUsagesVisible"] {
        fields.insert(name.to_string(), DslValue::Bool(false));
        field_types.insert(name.to_string(), "boolean".to_string());
    }
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: HOLDER.into(),
        fields,
        field_types,
    }))))
}

// port: CheckRegExpTest_Helpers#getProcessor
fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "CheckRegExpTest_Helpers#getProcessor arguments".into(),
        ));
    };
    let field = |name: &str| matches!(outer.borrow().fields.get(name), Some(DslValue::Bool(true)));
    // last = new CheckRegExp(compiler, assumeAllGlobalRegexpUsagesVisible, reportErrors);
    let last = DslValue::Native(Rc::new(RefCell::new(NativeCheckRegExp {
        compiler: compiler.clone(),
        pass: CheckRegExp::new(
            field("assumeAllGlobalRegexpUsagesVisible"),
            field("reportErrors"),
        ),
    })));
    outer
        .borrow_mut()
        .fields
        .insert("last".to_string(), last.clone());
    // return last;
    Ok(last)
}

/// A CheckRegExp the helper keeps in `last` and returns as its CompilerPass (one object).
struct NativeCheckRegExp {
    compiler: CompilerHandle,
    pass: CheckRegExp,
}

impl NativeObject for NativeCheckRegExp {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        CHECK_REG_EXP
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    // port: ReplayDsl#invoke (CheckRegExp's public getter, read by the recorder)
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method == "isGlobalRegExpPropertiesUsed" {
            Ok(DslValue::Bool(
                self.pass.is_global_reg_exp_properties_used(),
            ))
        } else {
            Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )))
        }
    }
    // port: ReplayValues#findField (CheckRegExp's instance fields, in declaration order)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let fields = self.pass.replay_fields();
        Ok(IndexMap::<_, _>::from_iter([
            (
                "compiler".to_string(),
                DslValue::Compiler(self.compiler.clone()),
            ),
            (
                "reportErrors".to_string(),
                DslValue::Bool(*fields.report_errors),
            ),
            (
                "globalRegExpPropertiesUsed".to_string(),
                DslValue::Bool(*fields.global_reg_exp_properties_used),
            ),
            (
                "assumeAllGlobalRegexpUsagesVisible".to_string(),
                DslValue::Bool(*fields.assume_all_global_regexp_usages_visible),
            ),
        ]))
    }
    // port: CheckRegExp#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.pass.process(compiler, externs, root);
        Ok(())
    }
}
