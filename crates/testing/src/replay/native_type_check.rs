/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/TypeCheck.java,
//   src/com/google/javascript/jscomp/type/SemanticReverseAbstractInterpreter.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for the typed setup descriptors build in their processors
//! (Es6RewriteModulesTest, Es6RewriteModulesWithGoogInteropTest, ClosureRewriteModuleTest):
//! `new SemanticReverseAbstractInterpreter(compiler.getTypeRegistry())` and
//! `new TypeCheck(compiler, rai, compiler.getTypeRegistry()).processForTesting(externs, root)`,
//! whose TypedScope those processors hand to the pass under test.
use crate::{
    harness_passes::NativeTypedScope,
    jscomp_api::Compiler,
    replay::{
        native_type_inference::NativeReverseAbstractInterpreter,
        registry::{BorrowedEntry, Entry},
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter,
    type_check::TypeCheck, typed_scope::TypedScope,
};
use std::{cell::RefCell, rc::Rc};

const SEMANTIC_REVERSE_ABSTRACT_INTERPRETER_INIT: &str = "com.google.javascript.jscomp.type.SemanticReverseAbstractInterpreter#<init>(com.google.javascript.rhino.jstype.JSTypeRegistry)";
const TYPE_CHECK_INIT: &str = "com.google.javascript.jscomp.TypeCheck#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.type.ReverseAbstractInterpreter,com.google.javascript.rhino.jstype.JSTypeRegistry)";
const TYPE_CHECK_PROCESS_FOR_TESTING: &str = "com.google.javascript.jscomp.TypeCheck#processForTesting(com.google.javascript.rhino.Node,com.google.javascript.rhino.Node)";
const JS_TYPE_REGISTRY: &str = "com.google.javascript.rhino.jstype.JSTypeRegistry";

// port: ReplayDsl#invoke (type-check signatures)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        SEMANTIC_REVERSE_ABSTRACT_INTERPRETER_INIT => semantic_reverse_abstract_interpreter,
        TYPE_CHECK_INIT => type_check,
        TYPE_CHECK_PROCESS_FOR_TESTING => process_for_testing,
        _ => return None,
    })
}

// port: ReplayDsl#invoke (type-check signatures while the processor has the compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        SEMANTIC_REVERSE_ABSTRACT_INTERPRETER_INIT => {
            semantic_reverse_abstract_interpreter_borrowed
        }
        TYPE_CHECK_INIT => type_check_borrowed,
        TYPE_CHECK_PROCESS_FOR_TESTING => process_for_testing_borrowed,
        _ => return None,
    })
}

/// The TypedScope argument of a pass constructor: null, or the value
/// `TypeCheck#processForTesting` returned.
pub fn typed_scope_of(value: &DslValue) -> Result<Option<TypedScope>, Throwable> {
    match value {
        DslValue::Null => Ok(None),
        DslValue::Native(native) => native
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<NativeTypedScope>()
            .map(|s| Some(s.scope))
            .ok_or_else(bad),
        _ => Err(bad()),
    }
}

/// A `TypeCheck` DSL value; Java's TypeCheck keeps its compiler, the handle stands for it.
struct NativeTypeCheck {
    compiler: crate::replay::replay_dsl::CompilerHandle,
    check: TypeCheck,
}

impl NativeObject for NativeTypeCheck {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.TypeCheck"
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn native(object: impl NativeObject) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(object)))
}

fn borrow(
    handle: &crate::replay::replay_dsl::CompilerHandle,
) -> Result<std::cell::RefMut<'_, Compiler>, Throwable> {
    handle.try_borrow_mut().map_err(|_| {
        Throwable::HarnessError("the compiler is already borrowed (DSL adapter)".into())
    })
}

/// The compiler handle a JSTypeRegistry DSL value belongs to is the processor's compiler: the
/// descriptors only pass `compiler.getTypeRegistry()`.
fn check_registry(value: &DslValue) -> Result<(), Throwable> {
    if value.class_name() == JS_TYPE_REGISTRY {
        Ok(())
    } else {
        Err(bad())
    }
}

// port: SemanticReverseAbstractInterpreter#SemanticReverseAbstractInterpreter
fn new_semantic_reverse_abstract_interpreter(
    args: &[DslValue],
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [registry] = args else {
        return Err(bad());
    };
    check_registry(registry)?;
    let interpreter = SemanticReverseAbstractInterpreter::new(compiler.get_type_registry());
    Ok(native(NativeReverseAbstractInterpreter(interpreter)))
}

// port: SemanticReverseAbstractInterpreter#SemanticReverseAbstractInterpreter
fn semantic_reverse_abstract_interpreter(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let handle = ctx.compiler.clone().ok_or_else(bad)?;
    new_semantic_reverse_abstract_interpreter(&args, &mut *borrow(&handle)?)
}

// port: SemanticReverseAbstractInterpreter#SemanticReverseAbstractInterpreter
fn semantic_reverse_abstract_interpreter_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    new_semantic_reverse_abstract_interpreter(&args, compiler)
}

// port: TypeCheck#TypeCheck(AbstractCompiler,ReverseAbstractInterpreter,JSTypeRegistry)
fn new_type_check(args: &[DslValue], compiler: &mut Compiler) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(handle),
        DslValue::Native(reverse),
        registry,
    ] = args
    else {
        return Err(bad());
    };
    check_registry(registry)?;
    let reverse = reverse
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<NativeReverseAbstractInterpreter>()
        .ok_or_else(bad)?
        .0
        .clone();
    let check = TypeCheck::new(compiler, reverse);
    Ok(native(NativeTypeCheck {
        compiler: handle.clone(),
        check,
    }))
}

// port: TypeCheck#TypeCheck(AbstractCompiler,ReverseAbstractInterpreter,JSTypeRegistry)
fn type_check(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(handle), ..] = args.as_slice() else {
        return Err(bad());
    };
    let handle = handle.clone();
    new_type_check(&args, &mut *borrow(&handle)?)
}

// port: TypeCheck#TypeCheck(AbstractCompiler,ReverseAbstractInterpreter,JSTypeRegistry)
fn type_check_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    new_type_check(&args, compiler)
}

// port: TypeCheck#processForTesting
fn run_process_for_testing(
    args: &[DslValue],
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(check), externs, DslValue::Node(root)] = args else {
        return Err(bad());
    };
    let externs = match externs {
        DslValue::Node(externs) => Some(*externs),
        DslValue::Null => None,
        _ => return Err(bad()),
    };
    let mut check = check.borrow_mut();
    let check = check
        .as_any_mut()
        .downcast_mut::<NativeTypeCheck>()
        .ok_or_else(bad)?;
    let scope = check.check.process_for_testing(compiler, externs, *root);
    Ok(native(NativeTypedScope {
        compiler: check.compiler.clone(),
        scope,
    }))
}

// port: TypeCheck#processForTesting
fn process_for_testing(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let handle = match args.first() {
        Some(DslValue::Native(check)) => check
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<NativeTypeCheck>()
            .ok_or_else(bad)?
            .compiler
            .clone(),
        _ => return Err(bad()),
    };
    run_process_for_testing(&args, &mut *borrow(&handle)?)
}

// port: TypeCheck#processForTesting
fn process_for_testing_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    run_process_for_testing(&args, compiler)
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
