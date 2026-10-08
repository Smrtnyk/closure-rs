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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AstValidator.java,
//   test/com/google/javascript/jscomp/AstValidatorTest.java.

//! Port of the replay helper `oracle/replay/helpers/.../AstValidatorTest_Helpers.java` (DSL names
//! `AstValidatorTest_Helpers` and `AstValidatorTest_Helpers.CreateValidatorViolationHandler`),
//! itself copied from AstValidatorTest.java: the field `lastCheckViolationMessages` and the
//! anonymous ViolationHandler of `createValidator`; plus the `AstValidator` constructor and
//! `setTypeValidationMode` the descriptor calls.
use crate::{
    jscomp_api::Compiler,
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    ast_validator::{AstValidator, TypeInfoValidation, ViolationHandler},
    compiler_pass::CompilerPass,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const HOLDER: &str = "com.google.javascript.jscomp.AstValidatorTest_Helpers";
const HANDLER: &str =
    "com.google.javascript.jscomp.AstValidatorTest_Helpers$CreateValidatorViolationHandler";
const VALIDATOR: &str = "com.google.javascript.jscomp.AstValidator";
const TYPE_INFO_VALIDATION: &str = "com.google.javascript.jscomp.AstValidator$TypeInfoValidation";

/// The holder's `List<String> lastCheckViolationMessages`, shared with the inner handler (Java's
/// inner class reads the field of its outer instance at every call).
type Messages = Rc<RefCell<Option<Vec<String>>>>;

/// `final class AstValidatorTest_Helpers`.
pub struct AstValidatorTestHelpers {
    last_check_violation_messages: Messages,
}

impl NativeObject for AstValidatorTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert(
            "lastCheckViolationMessages".into(),
            self.last_check_violation_messages.borrow().as_ref().map_or(
                DslValue::Null,
                |messages| {
                    DslValue::List(
                        messages
                            .iter()
                            .map(|m| DslValue::String(JsString::from(m.as_str())))
                            .collect(),
                    )
                },
            ),
        );
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match (name, value.untyped()) {
            ("lastCheckViolationMessages", DslValue::Null) => {
                *self.last_check_violation_messages.borrow_mut() = None;
                Ok(())
            }
            ("lastCheckViolationMessages", DslValue::List(items)) => {
                let messages = items
                    .iter()
                    .map(|item| match item.untyped() {
                        DslValue::String(s) => Ok(s.to_string()),
                        _ => Err(bad()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                *self.last_check_violation_messages.borrow_mut() = Some(messages);
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

// port: AstValidatorTest_Helpers#AstValidatorTest_Helpers
pub fn holder(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(
        AstValidatorTestHelpers {
            last_check_violation_messages: Rc::new(RefCell::new(Some(Vec::new()))),
        },
    ))))
}

/// `private class CreateValidatorViolationHandler implements ViolationHandler`.
struct CreateValidatorViolationHandler {
    last_check_violation_messages: Messages,
}

impl ViolationHandler for CreateValidatorViolationHandler {
    // port: AstValidatorTest_Helpers.CreateValidatorViolationHandler#handleViolation
    fn handle_violation(&mut self, _ast: &Ast, message: &str, _n: Option<NodeId>) {
        self.last_check_violation_messages
            .borrow_mut()
            .as_mut()
            .expect("NullPointerException")
            .push(message.to_string());
    }
}

impl NativeObject for CreateValidatorViolationHandler {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HANDLER
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: AstValidatorTest_Helpers.CreateValidatorViolationHandler#CreateValidatorViolationHandler
pub fn create_validator_violation_handler(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(outer)] = args.as_slice() else {
        return Err(bad());
    };
    let mut outer = outer.borrow_mut();
    let outer = outer
        .as_any_mut()
        .downcast_mut::<AstValidatorTestHelpers>()
        .ok_or_else(bad)?;
    Ok(DslValue::Native(Rc::new(RefCell::new(
        CreateValidatorViolationHandler {
            last_check_violation_messages: outer.last_check_violation_messages.clone(),
        },
    ))))
}

/// The `AstValidator` the descriptor's getProcessor builds.
struct NativeAstValidator(AstValidator<'static>);

impl NativeObject for NativeAstValidator {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        VALIDATOR
    }
    // port: ReplayValues#findField (native object adapter)
    // UnitRecorder#collect walks the processor for result producers: AstValidator holds its
    // compiler, violation handler, current script, validation mode and two flags, none of them a
    // result producer.
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: AstValidator#process
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

// port: AstValidator#AstValidator(AbstractCompiler,ViolationHandler,boolean,boolean)
pub fn ast_validator(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(compiler),
        DslValue::Native(handler),
        validate_script_features,
        should_validate_required_inlinings,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let (
        DslValue::Bool(validate_script_features),
        DslValue::Bool(should_validate_required_inlinings),
    ) = (
        validate_script_features.untyped(),
        should_validate_required_inlinings.untyped(),
    )
    else {
        return Err(bad());
    };
    let mut handler = handler.borrow_mut();
    let handler = handler
        .as_any_mut()
        .downcast_mut::<CreateValidatorViolationHandler>()
        .ok_or_else(|| {
            Throwable::Unported(format!("{VALIDATOR}#<init> with a native ViolationHandler"))
        })?;
    let validator = AstValidator::new_with_handler(
        &compiler.borrow(),
        CreateValidatorViolationHandler {
            last_check_violation_messages: handler.last_check_violation_messages.clone(),
        },
        *validate_script_features,
        *should_validate_required_inlinings,
    );
    Ok(DslValue::Native(Rc::new(RefCell::new(NativeAstValidator(
        validator,
    )))))
}

// port: AstValidator.TypeInfoValidation#valueOf
fn type_info_validation_value_of(name: &str) -> Result<TypeInfoValidation, Throwable> {
    match name {
        "JSTYPE" => Ok(TypeInfoValidation::JSTYPE),
        "COLOR" => Ok(TypeInfoValidation::COLOR),
        "NONE" => Ok(TypeInfoValidation::NONE),
        _ => Err(bad()),
    }
}

// port: AstValidator#setTypeValidationMode
pub fn set_type_validation_mode(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [this @ DslValue::Native(validator), mode] = args.as_slice() else {
        return Err(bad());
    };
    let DslValue::Enum { class, name } = mode.untyped() else {
        return Err(bad());
    };
    if class != TYPE_INFO_VALIDATION {
        return Err(bad());
    }
    let mode = type_info_validation_value_of(name)?;
    {
        let mut validator = validator.borrow_mut();
        let validator = validator
            .as_any_mut()
            .downcast_mut::<NativeAstValidator>()
            .ok_or_else(bad)?;
        validator.0.set_type_validation_mode(mode);
    }
    // Java returns `this`.
    Ok(this.clone())
}

// port: ReplayDsl#invoke (argument conversion failure)
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
