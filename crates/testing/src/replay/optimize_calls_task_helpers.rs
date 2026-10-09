/*
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DevirtualizeMethods.java,
//   src/com/google/javascript/jscomp/InlineObjectLiterals.java,
//   src/com/google/javascript/jscomp/InlineProperties.java,
//   src/com/google/javascript/jscomp/InlineSimpleMethods.java,
//   src/com/google/javascript/jscomp/OptimizeCalls.java,
//   src/com/google/javascript/jscomp/OptimizeConstructors.java,
//   src/com/google/javascript/jscomp/OptimizeParameters.java,
//   src/com/google/javascript/jscomp/OptimizeReturns.java,
//   test/com/google/javascript/jscomp/InlinePropertiesTest.java,
//   test/com/google/javascript/jscomp/OptimizeCallsIntegrationTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the passes of port/optimize-calls (OptimizeReturns, OptimizeParameters,
//! OptimizeConstructors, DevirtualizeMethods, InlineObjectLiterals, InlineProperties,
//! InlineSimpleMethods) and the ports of the unit-corpus helpers
//! `OptimizeCallsIntegrationTest_Helpers.java` and `InlinePropertiesTest_Helpers.java`
//! (oracle/replay/helpers).
use crate::{
    jscomp_api::Compiler,
    replay::{
        native_registry::NameSupplier,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    devirtualize_methods::DevirtualizeMethods,
    inline_object_literals::InlineObjectLiterals,
    inline_properties::InlineProperties,
    inline_simple_methods::InlineSimpleMethods,
    optimize_calls::{CallGraphCompilerPass, ReferenceMap},
    optimize_constructors::OptimizeConstructors,
    optimize_parameters::OptimizeParameters,
    optimize_returns::OptimizeReturns,
};
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const DEVIRTUALIZE_METHODS: &str = "com.google.javascript.jscomp.DevirtualizeMethods";
const CALL_GRAPH_COMPILER_PASS: &str =
    "com.google.javascript.jscomp.OptimizeCalls$CallGraphCompilerPass";

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!("{what} arguments"))
}

// port: ReplayDsl#invoke (AbstractCompiler argument)
fn compiler_arg<'a>(args: &'a [DslValue], what: &str) -> Result<&'a CompilerHandle, Throwable> {
    match args.first() {
        Some(DslValue::Compiler(c)) => Ok(c),
        _ => Err(bad(what)),
    }
}

fn pass(pass: impl CompilerPass + 'static) -> DslValue {
    let pass: Box<dyn CompilerPass> = Box::new(pass);
    DslValue::Pass(Rc::new(RefCell::new(pass)))
}

// port: OptimizeReturns#OptimizeReturns
pub fn optimize_returns(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler_arg(&args, "OptimizeReturns")?;
    Ok(pass(OptimizeReturns::new()))
}

// port: OptimizeParameters#OptimizeParameters
pub fn optimize_parameters(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let compiler = compiler_arg(&args, "OptimizeParameters")?;
    Ok(pass(OptimizeParameters::new(&compiler.borrow())))
}

// port: OptimizeConstructors#OptimizeConstructors
pub fn optimize_constructors(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let compiler = compiler_arg(&args, "OptimizeConstructors")?;
    Ok(pass(OptimizeConstructors::new(&compiler.borrow())))
}

// port: InlineSimpleMethods#InlineSimpleMethods
pub fn inline_simple_methods(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let compiler = compiler_arg(&args, "InlineSimpleMethods")?;
    Ok(pass(InlineSimpleMethods::new(&compiler.borrow())))
}

// port: InlineProperties#InlineProperties
pub fn inline_properties(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let compiler = compiler_arg(&args, "InlineProperties")?;
    Ok(pass(InlineProperties::new(&compiler.borrow())))
}

// port: InlineObjectLiterals#InlineObjectLiterals
pub fn inline_object_literals(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_), DslValue::Native(supplier)] = args.as_slice() else {
        return Err(bad("InlineObjectLiterals"));
    };
    let mut supplier = supplier.borrow_mut();
    let Some(supplier) = supplier.as_any_mut().downcast_mut::<NameSupplier>() else {
        return Err(Throwable::Unported(format!(
            "{}#get",
            supplier.class_name()
        )));
    };
    Ok(pass(InlineObjectLiterals::new(supplier.0.clone())))
}

/// `new DevirtualizeMethods(compiler)`: a CallGraphCompilerPass only, which the test hands to
/// `OptimizeCalls.Builder#addPass`.
pub struct CallGraphPassObject {
    class: &'static str,
    pub pass: Box<dyn CallGraphCompilerPass>,
}

impl NativeObject for CallGraphPassObject {
    fn class_name(&self) -> &str {
        self.class
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class || class == CALL_GRAPH_COMPILER_PASS
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl CallGraphCompilerPass for CallGraphPassObject {
    // port: OptimizeCalls.CallGraphCompilerPass#process (DSL receiver)
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: NodeId,
        root: NodeId,
        references: &mut ReferenceMap,
    ) {
        self.pass.process(compiler, externs, root, references);
    }
}

// port: DevirtualizeMethods#DevirtualizeMethods
pub fn devirtualize_methods(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler_arg(&args, "DevirtualizeMethods")?;
    Ok(DslValue::Native(Rc::new(RefCell::new(
        CallGraphPassObject {
            class: DEVIRTUALIZE_METHODS,
            pass: Box::new(DevirtualizeMethods::new()),
        },
    ))))
}

/// InlinePropertiesTest_Helpers.Anon1: InlinePropertiesTest$1 (getProcessor,
/// runSmartNameRemoval), an anonymous CompilerPass.
struct Anon1 {
    pass: Rc<RefCell<Box<dyn CompilerPass>>>,
    removal_pass: Rc<RefCell<Box<dyn CompilerPass>>>,
}

impl CompilerPass for Anon1 {
    // port: InlinePropertiesTest_Helpers.Anon1#process
    fn process(&mut self, compiler: &mut Compiler, externs: NodeId, root: NodeId) {
        self.removal_pass
            .borrow_mut()
            .process(compiler, externs, root);
        self.pass.borrow_mut().process(compiler, externs, root);
    }
}

// port: InlinePropertiesTest_Helpers.Anon1#Anon1
pub fn inline_properties_anon1(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [pass_value, removal_pass] = args.as_slice() else {
        return Err(bad("InlinePropertiesTest_Helpers$Anon1"));
    };
    let as_pass = |value: &DslValue| match value.untyped() {
        DslValue::Pass(p) => Ok(p.clone()),
        DslValue::Native(n) => Err(Throwable::Unported(format!(
            "{}#process",
            n.borrow().class_name()
        ))),
        _ => Err(bad("InlinePropertiesTest_Helpers$Anon1")),
    };
    Ok(pass(Anon1 {
        pass: as_pass(pass_value)?,
        removal_pass: as_pass(removal_pass)?,
    }))
}

/// OptimizeCallsIntegrationTest_Helpers.GetProcessorPass: the anonymous CompilerPass returned by
/// OptimizeCallsIntegrationTest#getProcessor. Java's `compiler` field is dropped (DESIGN §6).
struct GetProcessorPass;

impl CompilerPass for GetProcessorPass {
    // port: OptimizeCallsIntegrationTest_Helpers.GetProcessorPass#process
    fn process(&mut self, compiler: &mut Compiler, externs: NodeId, root: NodeId) {
        closure_jscomp::pure_function_identifier::Driver::new().process(compiler, externs, root);
        closure_jscomp::remove_unused_code::Builder::new(compiler)
            .remove_local_vars(true)
            .remove_globals(true)
            .build()
            .process(compiler, externs, root);
        closure_jscomp::optimize_calls::OptimizeCalls::builder()
            .set_compiler(compiler)
            .set_consider_externs(false)
            .add_pass(Box::new(OptimizeReturns::new()))
            .add_pass(Box::new(OptimizeParameters::new(compiler)))
            .build()
            .process(compiler, externs, root);
    }
}

// port: OptimizeCallsIntegrationTest_Helpers.GetProcessorPass#GetProcessorPass
pub fn optimize_calls_integration_get_processor_pass(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler_arg(
        &args,
        "OptimizeCallsIntegrationTest_Helpers$GetProcessorPass",
    )?;
    Ok(pass(GetProcessorPass))
}
