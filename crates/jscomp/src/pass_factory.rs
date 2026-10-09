/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/PassFactory.java.

use crate::{
    abstract_compiler::AbstractCompiler, compiler_options::CompilerOptions,
    compiler_pass::CompilerPass,
};
use std::sync::Arc;
pub type Condition = Arc<dyn Fn(&CompilerOptions) -> bool + Send + Sync>;
pub type PreconditionCheck = Arc<dyn Fn(&CompilerOptions) -> PreconditionResult + Send + Sync>;
pub type InternalFactory =
    Arc<dyn Fn(&mut AbstractCompiler) -> Box<dyn CompilerPass> + Send + Sync>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreconditionResult {
    pub success: bool,
    pub message: Option<String>,
}
impl PreconditionResult {
    pub const SUCCESS: Self = Self {
        success: true,
        message: None,
    };
}
// port: PassFactory#PassFactory (empty AutoValue base constructor)
#[derive(Clone)]
pub struct PassFactory {
    name: String,
    condition: Condition,
    precondition_check: PreconditionCheck,
    run_in_fixed_point_loop: bool,
    internal_factory: InternalFactory,
    unported: Option<&'static str>,
}
impl PassFactory {
    // port: PassFactory#getName
    pub fn get_name(&self) -> &str {
        &self.name
    }
    // port: PassFactory#getCondition
    pub fn get_condition(&self) -> &Condition {
        &self.condition
    }
    // port: PassFactory#getPreconditionCheck
    pub fn get_precondition_check(&self) -> &PreconditionCheck {
        &self.precondition_check
    }
    // port: PassFactory#isRunInFixedPointLoop
    pub fn is_run_in_fixed_point_loop(&self) -> bool {
        self.run_in_fixed_point_loop
    }
    // port: PassFactory#getInternalFactory
    pub fn get_internal_factory(&self) -> &InternalFactory {
        &self.internal_factory
    }
    // port: PassFactory#toBuilder
    pub fn to_builder(&self) -> Builder {
        Builder {
            name: Some(self.name.clone()),
            condition: self.condition.clone(),
            precondition_check: self.precondition_check.clone(),
            run_in_fixed_point_loop: self.run_in_fixed_point_loop,
            internal_factory: Some(self.internal_factory.clone()),
            unported: self.unported,
        }
    }
    // port: PassFactory#builder
    pub fn builder() -> Builder {
        Builder {
            name: None,
            condition: Arc::new(|_| true),
            precondition_check: Arc::new(|_| PreconditionResult::SUCCESS),
            run_in_fixed_point_loop: false,
            internal_factory: None,
            unported: None,
        }
    }
    // port: PassFactory#validatePreconditions
    pub fn validate_preconditions(&self, options: &CompilerOptions) {
        let can_run = (self.precondition_check)(options);
        assert!(
            can_run.success,
            "Precondition for pass {} failed: {}",
            self.name,
            can_run.message.as_deref().unwrap_or("null")
        );
    }
    // port: PassFactory#createEmptyPass
    pub fn create_empty_pass(name: impl Into<String>) -> Self {
        Self::builder()
            .set_name(name)
            .set_internal_factory(Arc::new(|_| Box::new(|_: &mut AbstractCompiler, _, _| {})))
            .build()
    }
    // port: PassFactory#create
    pub fn create(&self, compiler: &mut AbstractCompiler) -> Box<dyn CompilerPass> {
        (self.internal_factory)(compiler)
    }
    /// Reports a pass class that is not ported yet, like an unported internal factory: the panic
    /// comes from this file with the message `unported: <class>`, which the replay harness
    /// classifies as Unported.
    pub fn report_unported(class: &'static str) -> ! {
        panic!("unported: {class}")
    }
    pub fn is_ported(&self) -> bool {
        self.unported.is_none()
    }
    pub fn get_unported_class(&self) -> Option<&'static str> {
        self.unported
    }
}
impl PartialEq for PassFactory {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.run_in_fixed_point_loop == other.run_in_fixed_point_loop
            && Arc::ptr_eq(&self.condition, &other.condition)
            && Arc::ptr_eq(&self.precondition_check, &other.precondition_check)
            && Arc::ptr_eq(&self.internal_factory, &other.internal_factory)
    }
}
impl Eq for PassFactory {}
impl std::fmt::Debug for PassFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassFactory")
            .field("name", &self.name)
            .field("run_in_fixed_point_loop", &self.run_in_fixed_point_loop)
            .field("unported", &self.unported)
            .finish()
    }
}
pub struct Builder {
    name: Option<String>,
    condition: Condition,
    precondition_check: PreconditionCheck,
    run_in_fixed_point_loop: bool,
    internal_factory: Option<InternalFactory>,
    unported: Option<&'static str>,
}
impl Builder {
    // port: PassFactory.Builder#setName
    pub fn set_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    // port: PassFactory.Builder#setRunInFixedPointLoop
    pub fn set_run_in_fixed_point_loop(mut self, value: bool) -> Self {
        self.run_in_fixed_point_loop = value;
        self
    }
    // port: PassFactory.Builder#setCondition
    pub fn set_condition(mut self, condition: Condition) -> Self {
        self.condition = condition;
        self
    }
    // port: PassFactory.Builder#setPreconditionCheck
    pub fn set_precondition_check(mut self, check: PreconditionCheck) -> Self {
        self.precondition_check = check;
        self
    }
    // port: PassFactory.Builder#setInternalFactory
    pub fn set_internal_factory(mut self, factory: InternalFactory) -> Self {
        self.internal_factory = Some(factory);
        self.unported = None;
        self
    }
    pub fn set_unported_internal_factory(mut self, class: &'static str) -> Self {
        self.unported = Some(class);
        self.internal_factory = Some(Arc::new(move |_| panic!("unported: {class}")));
        self
    }
    // port: PassFactory.Builder#autoBuild
    fn auto_build(self) -> PassFactory {
        PassFactory {
            name: self.name.expect("Missing required properties: name"),
            condition: self.condition,
            precondition_check: self.precondition_check,
            run_in_fixed_point_loop: self.run_in_fixed_point_loop,
            internal_factory: self
                .internal_factory
                .expect("Missing required properties: internalFactory"),
            unported: self.unported,
        }
    }
    // port: PassFactory.Builder#build
    pub fn build(self) -> PassFactory {
        let result = self.auto_build();
        assert!(!result.name.is_empty());
        result
    }
}
