/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
 * Copyright 2025 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/Es6NormalizeClasses.java,
//   src/com/google/javascript/jscomp/InlineAndCollapseProperties.java,
//   src/com/google/javascript/jscomp/PhaseOptimizer.java,
//   test/com/google/javascript/jscomp/AggressiveInlineAliasesTest.java,
//   test/com/google/javascript/jscomp/CollapsePropertiesAndModuleRewritingTest.java,
//   test/com/google/javascript/jscomp/CollapsePropertiesTest.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/InlineAndCollapsePropertiesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for `InlineAndCollapseProperties` (its static `builder` and the `Builder`
//! methods the descriptors call) and ports of the unit-corpus helpers of its tests
//! (oracle/replay/helpers): `CollapsePropertiesTestHelpers`, `InlineAndCollapsePropertiesTest_Helpers`,
//! `AggressiveInlineAliasesTest_Helpers` and `CollapsePropertiesAndModuleRewritingTestHelpers`.
use crate::{
    jscomp_api::Compiler,
    replay::{
        options_values::OptionValue,
        registry::Registry,
        replay_dsl::{Ctx, DslValue, Lambda, NativeObject, Object},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    compiler_options::{ChunkOutputType, PropertyCollapseLevel},
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    es6_normalize_classes::Es6NormalizeClasses,
    global_namespace::GlobalNamespace,
    inline_and_collapse_properties::{Builder, InlineAndCollapseProperties},
    pass_factory::PassFactory,
    phase_optimizer::PhaseOptimizer,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc, sync::Arc};

const BUILDER: &str = "com.google.javascript.jscomp.InlineAndCollapseProperties$Builder";
const GLOBAL_NAMESPACE: &str = "com.google.javascript.jscomp.GlobalNamespace";
const COLLAPSE_PROPERTIES_TEST_HELPERS: &str =
    "com.google.javascript.jscomp.CollapsePropertiesTestHelpers";
const AGGRESSIVE_INLINE_ALIASES_TEST_HELPERS: &str =
    "com.google.javascript.jscomp.AggressiveInlineAliasesTest_Helpers";
const COLLAPSE_PROPERTIES_AND_MODULE_REWRITING_TEST_HELPERS: &str =
    "com.google.javascript.jscomp.CollapsePropertiesAndModuleRewritingTestHelpers";

// port: ReplayDsl#invoke (native method registration)
pub fn register(registry: &mut Registry) {
    registry.register(
        "com.google.javascript.jscomp.InlineAndCollapseProperties#builder(com.google.javascript.jscomp.AbstractCompiler)",
        builder,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.InlineAndCollapseProperties#builder(com.google.javascript.jscomp.AbstractCompiler)",
        builder_with_compiler,
    );
    registry.register(
        "com.google.javascript.jscomp.InlineAndCollapseProperties$Builder#setPropertyCollapseLevel(com.google.javascript.jscomp.CompilerOptions$PropertyCollapseLevel)",
        set_property_collapse_level,
    );
    registry.register(
        "com.google.javascript.jscomp.InlineAndCollapseProperties$Builder#setChunkOutputType(com.google.javascript.jscomp.CompilerOptions$ChunkOutputType)",
        set_chunk_output_type,
    );
    registry.register(
        "com.google.javascript.jscomp.InlineAndCollapseProperties$Builder#setHaveModulesBeenRewritten(boolean)",
        set_have_modules_been_rewritten,
    );
    registry.register(
        "com.google.javascript.jscomp.InlineAndCollapseProperties$Builder#setModuleResolutionMode(com.google.javascript.jscomp.deps.ModuleLoader$ResolutionMode)",
        set_module_resolution_mode,
    );
    registry.register(
        "com.google.javascript.jscomp.InlineAndCollapseProperties$Builder#testAggressiveInliningOnly(java.util.function.Consumer)",
        test_aggressive_inlining_only,
    );
    registry.register(
        "com.google.javascript.jscomp.InlineAndCollapseProperties$Builder#build()",
        build,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.Es6NormalizeClasses#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_normalize_classes_with_compiler,
    );
    registry.register(
        "com.google.javascript.jscomp.CollapsePropertiesTestHelpers#<init>()",
        collapse_properties_test_helpers,
    );
    registry.register(
        "com.google.javascript.jscomp.CollapsePropertiesTestHelpers$Processor#<init>(com.google.javascript.jscomp.CollapsePropertiesTestHelpers,com.google.javascript.jscomp.Compiler)",
        collapse_properties_test_helpers_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.InlineAndCollapsePropertiesTest_Helpers#getProcessor(com.google.javascript.jscomp.Compiler)",
        inline_and_collapse_properties_test_helpers_get_processor,
    );
    registry.register(
        "com.google.javascript.jscomp.AggressiveInlineAliasesTest_Helpers#<init>()",
        aggressive_inline_aliases_test_helpers,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.AggressiveInlineAliasesTest_Helpers#validateGlobalNamespace(com.google.javascript.jscomp.GlobalNamespace)",
        validate_global_namespace,
    );
    registry.register(
        "com.google.javascript.jscomp.CollapsePropertiesAndModuleRewritingTestHelpers#<init>()",
        collapse_properties_and_module_rewriting_test_helpers,
    );
    registry.register(
        "com.google.javascript.jscomp.CollapsePropertiesAndModuleRewritingTestHelpers$Processor#<init>(com.google.javascript.jscomp.CollapsePropertiesAndModuleRewritingTestHelpers,com.google.javascript.jscomp.Compiler)",
        collapse_properties_and_module_rewriting_test_helpers_processor,
    );
}

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!(
        "InlineAndCollapseProperties replay adapter: {what}"
    ))
}

/// `InlineAndCollapseProperties.Builder` as a DSL receiver. Java's setters return `this`, so the
/// adapter keeps one shared object and the Rust by-value builder inside it.
struct NativeBuilder {
    builder: Option<Builder>,
}

impl NativeObject for NativeBuilder {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        BUILDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayDsl#invoke (Builder receiver: apply a setter and return `this`)
fn with_builder(
    args: &[DslValue],
    update: impl FnOnce(Builder, &DslValue) -> Result<Builder, Throwable>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this), value] = args else {
        return Err(bad("Builder setter arguments"));
    };
    {
        let mut receiver = this.borrow_mut();
        let receiver = receiver
            .as_any_mut()
            .downcast_mut::<NativeBuilder>()
            .ok_or_else(|| bad("Builder receiver"))?;
        let builder = receiver
            .builder
            .take()
            .ok_or_else(|| bad("Builder already built"))?;
        receiver.builder = Some(update(builder, value)?);
    }
    Ok(DslValue::Native(this.clone()))
}

// port: InlineAndCollapseProperties#builder
fn builder(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad("builder arguments"));
    };
    let builder = InlineAndCollapseProperties::builder(&compiler.borrow());
    Ok(native_builder(builder))
}

// port: InlineAndCollapseProperties#builder (inside a pass factory that holds the compiler)
fn builder_with_compiler(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad("builder arguments"));
    };
    let builder = InlineAndCollapseProperties::builder(compiler);
    Ok(native_builder(builder))
}

fn native_builder(builder: Builder) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativeBuilder {
        builder: Some(builder),
    })))
}

// port: InlineAndCollapseProperties.Builder#setPropertyCollapseLevel
fn set_property_collapse_level(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    with_builder(&args, |b, v| {
        Ok(b.set_property_collapse_level(PropertyCollapseLevel::decode_value(v)?))
    })
}

// port: InlineAndCollapseProperties.Builder#setChunkOutputType
fn set_chunk_output_type(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    with_builder(&args, |b, v| {
        Ok(b.set_chunk_output_type(ChunkOutputType::decode_value(v)?))
    })
}

// port: InlineAndCollapseProperties.Builder#setHaveModulesBeenRewritten
fn set_have_modules_been_rewritten(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    with_builder(&args, |b, v| {
        Ok(b.set_have_modules_been_rewritten(bool::decode_value(v)?))
    })
}

// port: InlineAndCollapseProperties.Builder#setModuleResolutionMode
fn set_module_resolution_mode(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    with_builder(&args, |b, v| {
        Ok(b.set_module_resolution_mode(ResolutionMode::decode_value(v)?))
    })
}

/// `GlobalNamespace` handed to a DSL `Consumer<GlobalNamespace>`: the pass's namespace is moved
/// in for the duration of the call and moved back afterwards.
struct NativeGlobalNamespace {
    namespace: Option<GlobalNamespace>,
}

impl NativeObject for NativeGlobalNamespace {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        GLOBAL_NAMESPACE
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: InlineAndCollapseProperties.Builder#testAggressiveInliningOnly
fn test_aggressive_inlining_only(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    // The consumer runs inside the pass, while the replay context that built the processor is
    // borrowed by the running pass factory; it is invoked in a fresh context of the same
    // descriptor (a DSL lambda carries its captured variables).
    let descriptor = ctx.descriptor.clone();
    let class_map = ctx.class_map.clone();
    let registry = ctx.registry.clone();
    with_builder(&args, move |b, v| {
        let DslValue::Lambda(consumer) = v else {
            return Err(bad("testAggressiveInliningOnly consumer"));
        };
        let consumer: Rc<Lambda> = consumer.clone();
        Ok(b.test_aggressive_inlining_only(Box::new(
            move |compiler: &mut AbstractCompiler, namespace: &mut GlobalNamespace| {
                let mut context = Ctx::new(
                    descriptor.clone(),
                    crate::json::JsonValue::Null,
                    class_map.clone(),
                    registry.clone(),
                );
                accept_global_namespace(&consumer, &mut context, compiler, namespace);
            },
        )))
    })
}

// port: Consumer#accept (a DSL lambda over the pass's GlobalNamespace)
fn accept_global_namespace(
    consumer: &Rc<Lambda>,
    context: &mut Ctx,
    compiler: &mut AbstractCompiler,
    namespace: &mut GlobalNamespace,
) {
    let root = compiler.get_js_root().unwrap();
    let placeholder = GlobalNamespace::new_without_externs(compiler, root);
    let handle = Rc::new(RefCell::new(NativeGlobalNamespace {
        namespace: Some(std::mem::replace(namespace, placeholder)),
    }));
    let value = DslValue::Native(handle.clone());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::replay::replay_dsl::invoke_lambda_with_compiler(
            consumer,
            vec![value],
            context,
            Some(compiler),
        )
    }));
    *namespace = handle
        .borrow_mut()
        .namespace
        .take()
        .expect("GlobalNamespace returned by the consumer");
    match outcome {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => std::panic::panic_any(error),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

// port: InlineAndCollapseProperties.Builder#build
fn build(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(this)] = args.as_slice() else {
        return Err(bad("build arguments"));
    };
    let mut receiver = this.borrow_mut();
    let receiver = receiver
        .as_any_mut()
        .downcast_mut::<NativeBuilder>()
        .ok_or_else(|| bad("Builder receiver"))?;
    let builder = receiver
        .builder
        .take()
        .ok_or_else(|| bad("Builder already built"))?;
    let pass: Box<dyn CompilerPass> = Box::new(builder.build());
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: Es6NormalizeClasses#Es6NormalizeClasses (inside a pass factory that holds the compiler:
// AggressiveInlineAliasesTest and InlineAliasesTest build it in a DSL makePassFactory lambda)
fn es6_normalize_classes_with_compiler(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad("Es6NormalizeClasses arguments"));
    };
    let pass: Box<dyn CompilerPass> = Box::new(Es6NormalizeClasses::new(compiler));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: CompilerTestCase#makePassFactory
fn make_pass_factory(
    name: &str,
    pass: impl Fn(&mut AbstractCompiler) -> Box<dyn CompilerPass> + Send + Sync + 'static,
) -> PassFactory {
    PassFactory::builder()
        .set_name(name)
        .set_internal_factory(Arc::new(pass))
        .build()
}

/// The `PhaseOptimizer` returned by the tests' `getProcessor`, as a pass.
struct OptimizerPass(PhaseOptimizer);

impl CompilerPass for OptimizerPass {
    // port: PhaseOptimizer#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.0.process(compiler, externs, root);
    }
}

// port: CollapsePropertiesTest#getProcessor / InlineAndCollapsePropertiesTest#getProcessor
// (identical but for the collapse level, which InlineAndCollapsePropertiesTest fixes to ALL)
fn get_processor(
    compiler: &AbstractCompiler,
    property_collapse_level: PropertyCollapseLevel,
) -> PhaseOptimizer {
    let mut optimizer = PhaseOptimizer::new(compiler, None);
    optimizer.add_one_time_pass(make_pass_factory("es6NormalizeClasses", |c| {
        Box::new(Es6NormalizeClasses::new(c))
    }));
    optimizer.add_one_time_pass(make_pass_factory(
        "inlineAndCollapseProperties",
        move |comp| {
            Box::new(
                InlineAndCollapseProperties::builder(comp)
                    .set_property_collapse_level(property_collapse_level)
                    .set_chunk_output_type(ChunkOutputType::GLOBAL_NAMESPACE)
                    .set_have_modules_been_rewritten(false)
                    .set_module_resolution_mode(ResolutionMode::BROWSER)
                    .build(),
            )
        },
    ));
    optimizer
}

// port: ReplayValues#instantiate (helper holder with typed fields)
fn holder(class: &str, fields: &[(&str, &str, DslValue)]) -> DslValue {
    let mut values = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    for (name, r#type, value) in fields {
        values.insert((*name).to_string(), value.clone());
        field_types.insert((*name).to_string(), (*r#type).to_string());
    }
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: class.into(),
        fields: values,
        field_types,
    })))
}

// port: CollapsePropertiesTestHelpers#CollapsePropertiesTestHelpers
fn collapse_properties_test_helpers(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(holder(
        COLLAPSE_PROPERTIES_TEST_HELPERS,
        &[(
            "propertyCollapseLevel",
            "com.google.javascript.jscomp.CompilerOptions$PropertyCollapseLevel",
            DslValue::Enum {
                class: "com.google.javascript.jscomp.CompilerOptions$PropertyCollapseLevel".into(),
                name: "ALL".into(),
            },
        )],
    ))
}

// port: CollapsePropertiesTestHelpers.Processor#Processor
fn collapse_properties_test_helpers_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad("CollapsePropertiesTestHelpers$Processor arguments"));
    };
    let property_collapse_level = PropertyCollapseLevel::decode_value(
        outer
            .borrow()
            .fields
            .get("propertyCollapseLevel")
            .ok_or_else(|| bad("propertyCollapseLevel"))?,
    )?;
    let delegate = get_processor(&compiler.borrow(), property_collapse_level);
    let pass: Box<dyn CompilerPass> = Box::new(OptimizerPass(delegate));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: InlineAndCollapsePropertiesTest_Helpers#getProcessor
fn inline_and_collapse_properties_test_helpers_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad(
            "InlineAndCollapsePropertiesTest_Helpers#getProcessor arguments",
        ));
    };
    let optimizer = get_processor(&compiler.borrow(), PropertyCollapseLevel::ALL);
    let pass: Box<dyn CompilerPass> = Box::new(OptimizerPass(optimizer));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

// port: AggressiveInlineAliasesTest_Helpers#AggressiveInlineAliasesTest_Helpers
fn aggressive_inline_aliases_test_helpers(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(holder(
        AGGRESSIVE_INLINE_ALIASES_TEST_HELPERS,
        &[(
            "lastCompiler",
            "com.google.javascript.jscomp.Compiler",
            DslValue::Null,
        )],
    ))
}

// port: Truth assertWithMessage(message).that(actual).isEqualTo(expected)
fn assert_equal<T: PartialEq + std::fmt::Debug>(
    message: &str,
    actual: T,
    expected: T,
) -> Result<(), Throwable> {
    if actual == expected {
        Ok(())
    } else {
        Err(Throwable::Assertion {
            message: format!("{message}\nexpected: {expected:?}\nbut was : {actual:?}"),
        })
    }
}

// port: Truth assertWithMessage(message).that(actual).isNotNull()
fn assert_not_null<T>(message: &str, actual: Option<T>) -> Result<T, Throwable> {
    actual.ok_or_else(|| Throwable::Assertion {
        message: format!("{message}\nexpected not to be: null"),
    })
}

/// To ensure that as we modify the AST, the GlobalNamespace stays up-to-date, we do a
/// consistency check after every unit test.
///
/// This check compares the names in the global namespace in the pass with a freshly-created
/// global namespace.
///
/// `getLastCompiler()` is the compiler the pass runs on (the holder's `lastCompiler` field is set
/// to it by the descriptor), which is the compiler this borrowed entry receives.
// port: AggressiveInlineAliasesTest_Helpers#validateGlobalNamespace
fn validate_global_namespace(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [
        DslValue::Object(_this),
        DslValue::Native(pass_global_namespace),
    ] = args.as_slice()
    else {
        return Err(bad("validateGlobalNamespace arguments"));
    };
    let mut pass_global_namespace = pass_global_namespace.borrow_mut();
    let pass_global_namespace = pass_global_namespace
        .as_any_mut()
        .downcast_mut::<NativeGlobalNamespace>()
        .and_then(|n| n.namespace.as_mut())
        .ok_or_else(|| bad("validateGlobalNamespace GlobalNamespace"))?;
    let js_root = compiler.get_js_root().unwrap();
    let mut expected_global_namespace = GlobalNamespace::new_without_externs(compiler, js_root);
    // GlobalNamespace (understandably) does not override equals. It would be silly to put it in
    // a datastructure. Neither does GlobalNamespace.Name (which probably could?)
    // So to compare equality: we verify that
    //  1. the two namespaces have the same qualified names, bar extern names
    //  2. each name has the same number of references in both namespaces
    //  3. each name has the same computed `Inlinability`
    let expected_names = expected_global_namespace.get_name_forest(compiler).to_vec();
    for expected_name in expected_names {
        let expected = &expected_global_namespace;
        if expected_name.in_externs(expected) {
            continue;
        }
        let full_name: JsString = expected_name.get_full_name(expected);
        let message = full_name.to_string_lossy();
        let actual_name = pass_global_namespace.get_slot(compiler, &full_name);
        let actual_name = assert_not_null(&message, actual_name)?;
        let actual = &*pass_global_namespace;
        assert_equal(
            &message,
            actual_name.get_aliasing_gets(actual),
            expected_name.get_aliasing_gets(expected),
        )?;
        assert_equal(
            &message,
            actual_name.get_subclassing_gets(actual),
            expected_name.get_subclassing_gets(expected),
        )?;
        assert_equal(
            &message,
            actual_name.get_local_sets(actual),
            expected_name.get_local_sets(expected),
        )?;
        assert_equal(
            &message,
            actual_name.get_global_sets(actual),
            expected_name.get_global_sets(expected),
        )?;
        assert_equal(
            &message,
            actual_name.get_delete_props(actual),
            expected_name.get_delete_props(expected),
        )?;
        assert_equal(
            &message,
            actual_name.get_call_gets(actual),
            expected_name.get_call_gets(expected),
        )?;
        assert_equal(
            &format!("{message}: canCollapseOrInline()"),
            actual_name.can_collapse_or_inline(actual, compiler),
            expected_name.can_collapse_or_inline(expected, compiler),
        )?;
        assert_equal(
            &format!("{message}: canCollapseOrInlineChildNames()"),
            actual_name.can_collapse_or_inline_child_names(actual, compiler),
            expected_name.can_collapse_or_inline_child_names(expected, compiler),
        )?;
    }
    // Verify that no names in the actual name forest are not present in the expected name forest
    let actual_names = pass_global_namespace.get_name_forest(compiler).to_vec();
    for actual_name in actual_names {
        let actual_full_name = actual_name.get_full_name(pass_global_namespace);
        assert_not_null(
            &actual_full_name.to_string_lossy(),
            expected_global_namespace.get_slot(compiler, &actual_full_name),
        )?;
    }
    Ok(DslValue::Null)
}

// port: CollapsePropertiesAndModuleRewritingTestHelpers#CollapsePropertiesAndModuleRewritingTestHelpers
fn collapse_properties_and_module_rewriting_test_helpers(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(holder(
        COLLAPSE_PROPERTIES_AND_MODULE_REWRITING_TEST_HELPERS,
        &[
            (
                "collapseLevel",
                "com.google.javascript.jscomp.CompilerOptions$PropertyCollapseLevel",
                DslValue::Enum {
                    class: "com.google.javascript.jscomp.CompilerOptions$PropertyCollapseLevel"
                        .into(),
                    name: "ALL".into(),
                },
            ),
            (
                "chunkOutputType",
                "com.google.javascript.jscomp.CompilerOptions$ChunkOutputType",
                DslValue::Enum {
                    class: "com.google.javascript.jscomp.CompilerOptions$ChunkOutputType".into(),
                    name: "ES_MODULES".into(),
                },
            ),
        ],
    ))
}

// port: CollapsePropertiesAndModuleRewritingTestHelpers.Processor#Processor
fn collapse_properties_and_module_rewriting_test_helpers_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), DslValue::Compiler(_compiler)] = args.as_slice() else {
        return Err(bad(
            "CollapsePropertiesAndModuleRewritingTestHelpers$Processor arguments",
        ));
    };
    let pass: Box<dyn CompilerPass> = Box::new(ModuleRewritingProcessor {
        outer: outer.clone(),
    });
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

/// The anonymous `CompilerPass` of `CollapsePropertiesAndModuleRewritingTest#getProcessor`; it
/// reads the test's `collapseLevel` and `chunkOutputType` fields when its factories run.
struct ModuleRewritingProcessor {
    outer: Rc<RefCell<Object>>,
}

impl CompilerPass for ModuleRewritingProcessor {
    // port: CollapsePropertiesAndModuleRewritingTest#getProcessor (CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        use closure_jscomp::{
            gather_module_metadata::GatherModuleMetadata,
            modules::module_map_creator::ModuleMapCreator, pass_list_builder::PassListBuilder,
            pass_names, preprocessor_symbol_table::CachedInstanceFactory,
            rewrite_dynamic_imports::RewriteDynamicImports,
            transpilation_passes::TranspilationPasses,
        };
        let field = |name: &str| {
            self.outer
                .borrow()
                .fields
                .get(name)
                .cloned()
                .unwrap_or_else(|| panic!("CollapsePropertiesAndModuleRewritingTest#{name}"))
        };
        let collapse_level = PropertyCollapseLevel::decode_value(&field("collapseLevel"))
            .expect("CollapsePropertiesAndModuleRewritingTest#collapseLevel");
        let chunk_output_type = ChunkOutputType::decode_value(&field("chunkOutputType"))
            .expect("CollapsePropertiesAndModuleRewritingTest#chunkOutputType");
        let options = compiler.get_options().clone();
        let mut factories = PassListBuilder::new(options.clone());
        let module_resolution_mode = options.get_module_resolution_mode();
        factories.maybe_add(
            PassFactory::builder()
                .set_name(pass_names::GATHER_MODULE_METADATA)
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(move |_x| {
                    Box::new(GatherModuleMetadata::new(false, module_resolution_mode))
                }))
                .build(),
        );
        factories.maybe_add(
            PassFactory::builder()
                .set_name(pass_names::CREATE_MODULE_MAP)
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(|x| {
                    Box::new(ModuleMapCreator::new(closure_rhino::check_not_null!(
                        x.get_module_metadata_map().cloned()
                    )))
                }))
                .build(),
        );
        TranspilationPasses::add_es6_module_pass(&mut factories, &CachedInstanceFactory::default());
        factories.maybe_add(
            PassFactory::builder()
                .set_name("REWRITE_DYNAMIC_IMPORT")
                .set_internal_factory(Arc::new(|x| {
                    Box::new(RewriteDynamicImports::new(
                        x,
                        None,
                        ChunkOutputType::ES_MODULES,
                    ))
                }))
                .build(),
        );
        factories.maybe_add(make_pass_factory(pass_names::ES6_NORMALIZE_CLASSES, |c| {
            Box::new(Es6NormalizeClasses::new(c))
        }));
        factories.maybe_add(
            PassFactory::builder()
                .set_name(pass_names::COLLAPSE_PROPERTIES)
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(move |x| {
                    Box::new(
                        InlineAndCollapseProperties::builder(x)
                            .set_property_collapse_level(collapse_level)
                            .set_chunk_output_type(chunk_output_type)
                            .set_have_modules_been_rewritten(true)
                            .set_module_resolution_mode(module_resolution_mode)
                            .build(),
                    )
                }))
                .build(),
        );

        for factory in factories.build() {
            factory.create(compiler).process(compiler, externs, root);
        }
    }
}
