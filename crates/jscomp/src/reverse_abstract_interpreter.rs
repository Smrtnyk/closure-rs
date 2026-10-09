/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/type/ReverseAbstractInterpreter.java.

//! This interface defines what reversed abstract interpreters provide.
//!
//! Abstract interpretation is the process of interpreting a program at an abstracted level (such as
//! at the type level) instead of the concrete level (the flow of values). This reversed abstract
//! interpreter reverses the abstract interpretation process by knowing the outcome of some
//! computation and calculating a preciser view of the world than the view without knowing the
//! outcome of the computation.
//!
//! Java's `com.google.javascript.jscomp.type.ReverseAbstractInterpreter`. The interpreters keep no
//! registry (DESIGN §8): each call receives the compiler, which owns the registry, the Ast and the
//! typed scopes the flow scopes read.
use crate::{abstract_compiler::AbstractCompiler, flow_scope::FlowScope};
use closure_rhino::{node::NodeId, outcome::Outcome};
use std::sync::Arc;

pub trait ReverseAbstractInterpreter: Send + Sync {
    // port: ReverseAbstractInterpreter#getPreciserScopeKnowingConditionOutcome
    /// Calculates a precise version of the scope knowing the outcome of the condition.
    ///
    /// `condition` is the condition's expression, `blind_scope` the scope without knowledge about
    /// the outcome of the condition, `outcome` the outcome of the condition.
    fn get_preciser_scope_knowing_condition_outcome(
        &self,
        compiler: &mut AbstractCompiler,
        condition: NodeId,
        blind_scope: Arc<dyn FlowScope>,
        outcome: Outcome,
    ) -> Arc<dyn FlowScope>;
}
