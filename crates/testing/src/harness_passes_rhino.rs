/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Bob Jervis
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/JSTypeExpression.java,
//   src/com/google/javascript/rhino/jstype/JSTypeNative.java,
//   src/com/google/javascript/rhino/jstype/JSTypeRegistry.java,
//   src/com/google/javascript/rhino/jstype/RecordTypeBuilder.java,
//   src/com/google/javascript/rhino/testing/BaseJSTypeTestCase.java.

//! The harness's calls into Closure's Rhino-derived type system: `JSTypeRegistry` and
//! `RecordTypeBuilder` methods, `JSTypeNative#valueOf`, `JSTypeExpression`, and the
//! `BaseJSTypeTestCase` helpers. The DSL dispatch that reaches them is in `harness_passes.rs`.

use super::{
    NativeJSType, NativeRecordTypeBuilder, NativeTypeExpression, js_type_arg, js_type_value,
    non_null, registry_compiler, spread_args, with_type_registry,
};
use crate::{
    jscomp_api::Compiler,
    replay::replay_dsl::{CompilerHandle, DslValue},
    throwable::Throwable,
};
use closure_jstype::TypeId;
use closure_rhino::node::NodeId;

// port: JSTypeExpression#JSTypeExpression
pub fn js_type_expression(node: NodeId, source: &str) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(std::rc::Rc::new(std::cell::RefCell::new(
        NativeTypeExpression(closure_rhino::js_type_expression::JSTypeExpression::new(
            node, source,
        )),
    ))))
}

// port: BaseJSTypeTestCase#addNativeProperties (on the compiler's type registry)
pub fn add_native_properties(compiler: &mut Compiler) -> Result<(), Throwable> {
    let (registry, ast) = compiler.get_type_registry_and_ast();
    closure_jstype::testing::base_js_type_test_case::BaseJSTypeTestCase::add_native_properties(
        registry, ast,
    );
    Ok(())
}

// port: JSTypeNative#valueOf
fn js_type_native_arg(value: &DslValue) -> Result<closure_jstype::JSTypeNative, Throwable> {
    if let DslValue::Enum { class, name } = value.untyped()
        && class == "com.google.javascript.rhino.jstype.JSTypeNative"
    {
        return closure_jstype::JSTypeNative::VALUES
            .into_iter()
            .find(|native| format!("{native:?}") == *name)
            .ok_or_else(|| Throwable::HarnessError(format!("unknown JSTypeNative {name}")));
    }
    Err(Throwable::HarnessError(format!(
        "not a JSTypeNative: {}",
        value.class_name()
    )))
}

// port: BaseJSTypeTestCase#createRecordTypeBuilder
pub fn record_type_builder(registry: &DslValue) -> Result<DslValue, Throwable> {
    let compiler = registry_compiler(registry)?;
    let builder = std::rc::Rc::new(std::cell::RefCell::new(NativeRecordTypeBuilder {
        compiler,
        builder: closure_jstype::record_type_builder::RecordTypeBuilder::new(),
        this: std::rc::Weak::new(),
    }));
    builder.borrow_mut().this = std::rc::Rc::downgrade(&builder);
    Ok(DslValue::Native(builder))
}

// port: BaseJSTypeTestCase#assertTypeEquals(JSType,JSType) / assertTypeEquals(String,JSType,JSType)
pub fn assert_types_equal(
    message: Option<&str>,
    a: &DslValue,
    b: &DslValue,
) -> Result<(), Throwable> {
    let (a_type, b_type) = (js_type_arg(a)?, js_type_arg(b)?);
    let compiler = [a, b].into_iter().find_map(|v| match v.untyped() {
        DslValue::Native(o) => o
            .borrow_mut()
            .as_any_mut()
            .downcast_mut::<NativeJSType>()
            .map(|t| t.compiler.clone()),
        _ => None,
    });
    let Some(compiler) = compiler else {
        // Both null: assertType(null).isEqualTo(null) passes.
        return Ok(());
    };
    with_type_registry(&compiler, |registry, ast| {
        if let Some(message) = message {
            // assertWithMessage(message).about(types()).that(b).isEqualTo(a)
            let equal = match (a_type, b_type) {
                (Some(a), Some(b)) => {
                    use closure_jstype::prelude::*;
                    a.equals(registry, ast, b)
                }
                (None, None) => true,
                _ => false,
            };
            crate::throwable::assert_that(equal, message)?;
        }
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            closure_jstype::testing::type_subject::TypeSubject::assert_type(b_type)
                .is_equal_to(registry, ast, a_type);
        }))
        .map_err(|panic| Throwable::Assertion {
            message: panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default(),
        })
    })
}

// port: BaseJSTypeTestCase#resolve (JSTypeExpression#evaluate(null, registry))
pub fn evaluate_type_expression(
    expression: &DslValue,
    registry: &DslValue,
) -> Result<DslValue, Throwable> {
    use closure_jstype::rhino::js_type_expression::JSTypeExpressionExt;
    let compiler = registry_compiler(registry)?;
    let DslValue::Native(o) = expression.untyped() else {
        return Err(Throwable::HarnessError(format!(
            "not a JSTypeExpression: {}",
            expression.class_name()
        )));
    };
    let mut native = o.borrow_mut();
    let expression = native
        .as_any_mut()
        .downcast_mut::<NativeTypeExpression>()
        .ok_or_else(|| {
            Throwable::HarnessError(format!(
                "not a JSTypeExpression: {}",
                expression.class_name()
            ))
        })?;
    let id = with_type_registry(&compiler, |registry, ast| {
        Ok(expression.0.evaluate(registry, ast, None))
    })?;
    Ok(js_type_value(&compiler, id))
}

// port: JSTypeRegistry#getNativeType
pub(super) fn get_native_type(
    compiler: &CompilerHandle,
    args: &[DslValue],
) -> Result<TypeId, Throwable> {
    let native = js_type_native_arg(&args[0])?;
    with_type_registry(compiler, |registry, _| Ok(registry.get_native_type(native)))
}

// port: JSTypeRegistry#getNativeObjectType
pub(super) fn get_native_object_type(
    compiler: &CompilerHandle,
    args: &[DslValue],
) -> Result<TypeId, Throwable> {
    let native = js_type_native_arg(&args[0])?;
    with_type_registry(compiler, |registry, _| {
        Ok(registry.get_native_object_type(native))
    })
}

// port: JSTypeRegistry#getNativeFunctionType
pub(super) fn get_native_function_type(
    compiler: &CompilerHandle,
    args: &[DslValue],
) -> Result<TypeId, Throwable> {
    let native = js_type_native_arg(&args[0])?;
    with_type_registry(compiler, |registry, _| {
        Ok(registry.get_native_function_type(native))
    })
}

// port: JSTypeRegistry#createUnionType(JSType...) / createUnionType(JSTypeNative...)
pub(super) fn create_union_type(
    compiler: &CompilerHandle,
    args: &[DslValue],
) -> Result<TypeId, Throwable> {
    let variants = spread_args(args);
    if variants
        .first()
        .is_some_and(|v| matches!(v.untyped(), DslValue::Enum { .. }))
    {
        let natives = variants
            .iter()
            .map(js_type_native_arg)
            .collect::<Result<Vec<_>, _>>()?;
        with_type_registry(compiler, |registry, ast| {
            Ok(registry.create_union_type_from_native(ast, &natives))
        })
    } else {
        let types = variants
            .iter()
            .map(|v| js_type_arg(v).and_then(non_null))
            .collect::<Result<Vec<_>, _>>()?;
        with_type_registry(compiler, |registry, ast| {
            Ok(registry.create_union_type(ast, &types))
        })
    }
}

// port: JSTypeRegistry#createNullableType
pub(super) fn create_nullable_type(
    compiler: &CompilerHandle,
    args: &[DslValue],
) -> Result<TypeId, Throwable> {
    let type_ = non_null(js_type_arg(&args[0])?)?;
    with_type_registry(compiler, |registry, ast| {
        Ok(registry.create_nullable_type(ast, type_))
    })
}

// port: JSTypeRegistry#createOptionalType
pub(super) fn create_optional_type(
    compiler: &CompilerHandle,
    args: &[DslValue],
) -> Result<TypeId, Throwable> {
    let type_ = non_null(js_type_arg(&args[0])?)?;
    with_type_registry(compiler, |registry, ast| {
        Ok(registry.create_optional_type(ast, type_))
    })
}

// port: JSTypeRegistry#createTemplatizedType(ObjectType,ImmutableList) / (ObjectType,JSType...)
pub(super) fn create_templatized_type(
    compiler: &CompilerHandle,
    args: &[DslValue],
) -> Result<TypeId, Throwable> {
    let base = non_null(js_type_arg(&args[0])?)?;
    let types = spread_args(&args[1..])
        .iter()
        .map(|v| js_type_arg(v).and_then(non_null))
        .collect::<Result<Vec<_>, _>>()?;
    with_type_registry(compiler, |registry, ast| {
        Ok(registry.create_templatized_type(ast, base, &types))
    })
}

// port: RecordTypeBuilder#addProperty
pub(super) fn add_property(
    native: &mut NativeRecordTypeBuilder,
    name: &DslValue,
    type_: &DslValue,
    property_node: &DslValue,
) -> Result<DslValue, Throwable> {
    let DslValue::String(name) = name.untyped() else {
        return Err(Throwable::HarnessError("property name not a String".into()));
    };
    let type_ = non_null(js_type_arg(type_)?)?;
    let property_node = match property_node.untyped() {
        DslValue::Null => None,
        DslValue::Node(n) => Some(*n),
        other => {
            return Err(Throwable::HarnessError(format!(
                "not a Node: {}",
                other.class_name()
            )));
        }
    };
    native
        .builder
        .add_property(name.clone(), type_, property_node);
    let this = native
        .this
        .upgrade()
        .ok_or_else(|| Throwable::HarnessError("RecordTypeBuilder dropped".into()))?;
    Ok(DslValue::Native(this))
}

// port: RecordTypeBuilder#setSynthesized
pub(super) fn set_synthesized(
    native: &mut NativeRecordTypeBuilder,
    synthesized: &DslValue,
) -> Result<DslValue, Throwable> {
    let DslValue::Bool(synthesized) = synthesized.untyped() else {
        return Err(Throwable::HarnessError("not a boolean".into()));
    };
    native.builder.set_synthesized(*synthesized);
    let this = native
        .this
        .upgrade()
        .ok_or_else(|| Throwable::HarnessError("RecordTypeBuilder dropped".into()))?;
    Ok(DslValue::Native(this))
}

// port: RecordTypeBuilder#build
pub(super) fn build(native: &mut NativeRecordTypeBuilder) -> Result<DslValue, Throwable> {
    let builder = &native.builder;
    let id = with_type_registry(&native.compiler, |registry, ast| {
        Ok(builder.build(registry, ast))
    })?;
    Ok(js_type_value(&native.compiler, id))
}
