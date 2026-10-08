/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Es6NormalizeClasses.java,
//   src/com/google/javascript/jscomp/Es6NormalizeShorthandProperties.java,
//   src/com/google/javascript/jscomp/PeepholeTranspilationsPass.java,
//   src/com/google/javascript/jscomp/ReportUntranspilableFeatures.java,
//   src/com/google/javascript/jscomp/RewriteCatchWithNoBinding.java,
//   src/com/google/javascript/jscomp/RewriteNewDotTarget.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/Es6NormalizeShorthandPropertiesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for the transpile-infra classes: the peephole transpilations, the
//! PeepholeTranspilationsPass that runs them, and Es6NormalizeClasses.
use crate::{
    replay::{
        options_values::OptionValue,
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::node_util::NodeUtil;
use closure_jscomp::{
    abstract_peephole_transpilation::AbstractPeepholeTranspilation,
    compiler_options::BrowserFeaturesetYear, es6_normalize_classes::Es6NormalizeClasses,
    es6_normalize_shorthand_properties::Es6NormalizeShorthandProperties,
    peephole_transpilations_pass::PeepholeTranspilationsPass,
    report_untranspilable_features::ReportUntranspilableFeatures,
    rewrite_catch_with_no_binding::RewriteCatchWithNoBinding,
    rewrite_new_dot_target::RewriteNewDotTarget,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::node::{Ast, NodeId};
use std::{cell::RefCell, rc::Rc};

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn register(registry: &mut Registry) {
    registry.register(
        "com.google.javascript.jscomp.ReportUntranspilableFeatures#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.CompilerOptions$BrowserFeaturesetYear,com.google.javascript.jscomp.parsing.parser.FeatureSet)",
        report_untranspilable_features,
    );
    registry.register(
        "com.google.javascript.jscomp.RewriteCatchWithNoBinding#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        rewrite_catch_with_no_binding,
    );
    registry.register(
        "com.google.javascript.jscomp.RewriteNewDotTarget#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        rewrite_new_dot_target,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6NormalizeShorthandProperties#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_normalize_shorthand_properties,
    );
    registry.register(
        "com.google.javascript.jscomp.PeepholeTranspilationsPass#create(com.google.javascript.jscomp.AbstractCompiler,java.util.List)",
        peephole_transpilations_pass_create,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6NormalizeClasses#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_normalize_classes,
    );
    registry.register_with_compiler(
        "com.google.javascript.jscomp.Es6NormalizeClasses#<init>(com.google.javascript.jscomp.AbstractCompiler)",
        es6_normalize_classes_borrowed,
    );
    registry.register(
        "com.google.javascript.jscomp.Es6NormalizeShorthandPropertiesTest_Helpers#testNormalizationInSourcePostcondition()",
        test_normalization_in_source_postcondition,
    );
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

/// An AbstractPeepholeTranspilation value, held until PeepholeTranspilationsPass#create takes it.
struct NativeTranspilation {
    class: &'static str,
    transpilation: Option<Box<dyn AbstractPeepholeTranspilation>>,
}
impl NativeObject for NativeTranspilation {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        self.class
    }
    // port: UnitRecorder#isInstance
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class || class == "com.google.javascript.jscomp.AbstractPeepholeTranspilation"
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: ReplayDsl#invoke (AbstractPeepholeTranspilation return value)
fn transpilation(
    class: &'static str,
    transpilation: impl AbstractPeepholeTranspilation + 'static,
) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativeTranspilation {
        class,
        transpilation: Some(Box::new(transpilation)),
    })))
}

// port: ReportUntranspilableFeatures#ReportUntranspilableFeatures
fn report_untranspilable_features(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [_, year, output_features] = args.as_slice() else {
        return Err(bad());
    };
    let year = Option::<BrowserFeaturesetYear>::decode_value(year)?;
    if matches!(output_features.untyped(), DslValue::Null) {
        return Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        });
    }
    let output_features = FeatureSet::decode_value(output_features)?;
    let pass = ReportUntranspilableFeatures::new(&c.borrow(), year, output_features);
    Ok(transpilation(
        "com.google.javascript.jscomp.ReportUntranspilableFeatures",
        pass,
    ))
}
// port: RewriteCatchWithNoBinding#RewriteCatchWithNoBinding
fn rewrite_catch_with_no_binding(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = RewriteCatchWithNoBinding::new(&mut c.borrow_mut());
    Ok(transpilation(
        "com.google.javascript.jscomp.RewriteCatchWithNoBinding",
        pass,
    ))
}
// port: RewriteNewDotTarget#RewriteNewDotTarget
fn rewrite_new_dot_target(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = RewriteNewDotTarget::new(&mut c.borrow_mut());
    Ok(transpilation(
        "com.google.javascript.jscomp.RewriteNewDotTarget",
        pass,
    ))
}
// port: Es6NormalizeShorthandProperties#Es6NormalizeShorthandProperties
fn es6_normalize_shorthand_properties(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = Es6NormalizeShorthandProperties::new(&c.borrow());
    Ok(transpilation(
        "com.google.javascript.jscomp.Es6NormalizeShorthandProperties",
        pass,
    ))
}
// port: PeepholeTranspilationsPass#create
fn peephole_transpilations_pass_create(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [_, list] = args.as_slice() else {
        return Err(bad());
    };
    let DslValue::List(items) = list.untyped() else {
        return Err(bad());
    };
    let mut transpilations: Vec<Box<dyn AbstractPeepholeTranspilation>> = Vec::new();
    for item in items {
        let DslValue::Native(native) = item.untyped() else {
            return Err(bad());
        };
        let mut native = native.borrow_mut();
        let Some(native) = native.as_any_mut().downcast_mut::<NativeTranspilation>() else {
            return Err(bad());
        };
        transpilations.push(native.transpilation.take().ok_or_else(bad)?);
    }
    let pass = PeepholeTranspilationsPass::create(&c.borrow(), transpilations);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: Es6NormalizeClasses#Es6NormalizeClasses
fn es6_normalize_classes(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let pass = Es6NormalizeClasses::new(&mut c.borrow_mut());
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
// port: Es6NormalizeClasses#Es6NormalizeClasses (inside a factory that lends the compiler)
fn es6_normalize_classes_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler_ref: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    let pass = Es6NormalizeClasses::new(compiler_ref);
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

const ES6_NORMALIZE_SHORTHAND_PROPERTIES_TEST_HELPERS: &str =
    "com.google.javascript.jscomp.Es6NormalizeShorthandPropertiesTest_Helpers";

/// The postcondition lambda of Es6NormalizeShorthandPropertiesTest#testNormalizationInSource:
/// `(compiler) -> NodeUtil.visitPreOrder(compiler.getRoot(), ...::assertNotShorthandProperty)`.
struct NormalizationInSourcePostcondition;

impl NativeObject for NormalizationInSourcePostcondition {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        ES6_NORMALIZE_SHORTHAND_PROPERTIES_TEST_HELPERS
    }
    // port: UnitRecorder#isInstance
    fn is_instance_of(&self, class: &str) -> bool {
        class == ES6_NORMALIZE_SHORTHAND_PROPERTIES_TEST_HELPERS
            || class == "com.google.javascript.jscomp.CompilerTestCase$Postcondition"
    }
    // port: CompilerTestCase.Postcondition#verify
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method != "verify" {
            return Err(Throwable::Unported(format!(
                "{ES6_NORMALIZE_SHORTHAND_PROPERTIES_TEST_HELPERS}#{method}"
            )));
        }
        let [DslValue::Compiler(compiler)] = args.as_slice() else {
            return Err(bad());
        };
        let mut compiler = compiler.borrow_mut();
        let root = compiler.get_root().ok_or_else(|| Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        })?;
        let mut failure: Option<Throwable> = None;
        NodeUtil::visit_pre_order(&mut compiler, root, &mut |ast: &mut Ast, node: NodeId| {
            if failure.is_none()
                && let Err(e) = assert_not_shorthand_property(ast, node)
            {
                failure = Some(e);
            }
        });
        match failure {
            Some(e) => Err(e),
            None => Ok(DslValue::Null),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: Es6NormalizeShorthandPropertiesTest_Helpers#assertNotShorthandProperty
fn assert_not_shorthand_property(ast: &Ast, node: NodeId) -> Result<(), Throwable> {
    crate::throwable::assert_that(
        !node.is_shorthand_property(ast),
        format!(
            "Detected shorthand property node <{}>.\nexpected to be false",
            node.to_string(ast)
        ),
    )
}

// port: Es6NormalizeShorthandPropertiesTest_Helpers#testNormalizationInSourcePostcondition
fn test_normalization_in_source_postcondition(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NormalizationInSourcePostcondition,
    ))))
}
