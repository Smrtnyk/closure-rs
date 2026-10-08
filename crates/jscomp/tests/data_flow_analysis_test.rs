/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/DataFlowAnalysisTest.java.

#![allow(
    dead_code,
    clippy::new_ret_no_self,
    clippy::upper_case_acronyms,
    clippy::collapsible_if
)] // Java constructors return the Instruction superclass enum.
use closure_jscomp::graph::annotatable::Annotatable;
use closure_jscomp::{
    Compiler,
    control_flow_graph::{Branch, ControlFlowGraph},
    data_flow_analysis::{
        DataFlowAnalysis, DataFlowAnalysisState, FlowBrancher, FlowJoiner, LatticeEquals,
        LinearFlowState, MAX_STEPS_PER_NODE,
    },
    graph::{di_graph::DiGraphNode, graph::Graph},
};
use closure_rhino::{java_lang::JavaHashCode, js_string::JsString};
use indexmap::IndexMap;
use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

#[derive(Clone, Debug)]
struct Variable(JsString);
impl Variable {
    // port: DataFlowAnalysisTest.Variable#Variable
    fn new(n: impl Into<JsString>) -> Self {
        Self(n.into())
    }
}
#[derive(Clone, Copy, Debug, Eq)]
struct NumberValue(i32);
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Value {
    Variable(Variable),
    Number(NumberValue),
}
impl From<Variable> for Value {
    fn from(v: Variable) -> Self {
        Self::Variable(v)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Number(NumberValue(v))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
enum Operation {
    ADD,
    SUB,
    DIV,
    MUL,
}
#[derive(Clone, Debug, Eq)]
struct ArithmeticInstruction {
    order: i32,
    result: Variable,
    operand1: Value,
    operation: Operation,
    operand2: Value,
}
impl ArithmeticInstruction {
    // port: DataFlowAnalysisTest.ArithmeticInstruction#ArithmeticInstruction
    fn new(
        res: Variable,
        op1: impl Into<Value>,
        o: Operation,
        op2: impl Into<Value>,
    ) -> Instruction {
        Instruction::Arithmetic(Self {
            order: 0,
            result: res,
            operand1: op1.into(),
            operation: o,
            operand2: op2.into(),
        })
    }
}
#[derive(Clone, Debug)]
struct BranchInstruction {
    identity: Arc<()>,
    condition: Value,
}
impl BranchInstruction {
    // port: DataFlowAnalysisTest.BranchInstruction#BranchInstruction
    fn new(cond: impl Into<Value>) -> Instruction {
        Instruction::Branch(Self {
            identity: Arc::new(()),
            condition: cond.into(),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Instruction {
    Arithmetic(ArithmeticInstruction),
    Branch(BranchInstruction),
}
// port: DataFlowAnalysisTest#newAssignNumberToVariableInstruction
fn new_assign_number_to_variable_instruction(res: Variable, num: i32) -> Instruction {
    ArithmeticInstruction::new(res, num, Operation::ADD, 0)
}
// port: DataFlowAnalysisTest#newAssignVariableToVariableInstruction
fn new_assign_variable_to_variable_instruction(lhs: Variable, rhs: Variable) -> Instruction {
    ArithmeticInstruction::new(lhs, rhs, Operation::ADD, 0)
}
#[derive(Clone, Default)]
struct ConstPropLatticeElement {
    const_map: IndexMap<Variable, i32>,
    is_top: bool,
}
#[derive(Default)]
struct ConstPropJoinOp {
    result: Option<ConstPropLatticeElement>,
}
impl ConstPropJoinOp {
    // port: DataFlowAnalysisTest.ConstPropJoinOp#apply
    fn apply(a: ConstPropLatticeElement, b: ConstPropLatticeElement) -> ConstPropLatticeElement {
        if a.is_top {
            return a.clone();
        }
        if b.is_top {
            return b.clone();
        }
        let mut result = ConstPropLatticeElement::default();
        for (var, number) in a.const_map {
            if b.const_map.get(&var) == Some(&number) {
                result.const_map.insert(var, number);
            }
        }
        result
    }
}
impl FlowJoiner<ConstPropLatticeElement> for ConstPropJoinOp {
    // port: DataFlowAnalysisTest.ConstPropJoinOp#joinFlow
    fn join_flow(&mut self, _compiler: &mut Compiler, input: ConstPropLatticeElement) {
        self.result = Some(if let Some(r) = self.result.take() {
            Self::apply(r, input)
        } else {
            input
        });
    }
    // port: DataFlowAnalysisTest.ConstPropJoinOp#finish
    fn finish(self: Box<Self>) -> ConstPropLatticeElement {
        self.result.unwrap()
    }
}
// port: DataFlowAnalysisTest#flowThroughArithmeticInstruction
fn flow_through_arithmetic_instruction(
    a_inst: ArithmeticInstruction,
    input: ConstPropLatticeElement,
) -> ConstPropLatticeElement {
    let mut out = input.clone();
    let constant = |v: Value| match v {
        Value::Number(n) => Some(n.0),
        Value::Variable(v) => input.const_map.get(&v).copied(),
    };
    let left = constant(a_inst.operand1.clone());
    let right = constant(a_inst.operand2.clone());
    if let (Some(left), Some(right)) = (left, right) {
        let result = match a_inst.operation {
            Operation::ADD => left.wrapping_add(right),
            Operation::SUB => left.wrapping_sub(right),
            Operation::MUL => left.wrapping_mul(right),
            Operation::DIV => left.wrapping_div(right),
        };
        out.const_map.insert(a_inst.result, result);
    } else {
        out.const_map.shift_remove(&a_inst.result);
    }
    out
}
struct DummyConstPropagation {
    state: DataFlowAnalysisState<Instruction>,
}
impl DummyConstPropagation {
    // port: DataFlowAnalysisTest.DummyConstPropagation#DummyConstPropagation
    fn new(cfg: ControlFlowGraph<Instruction>) -> Self {
        Self {
            state: DataFlowAnalysisState::new(cfg, true, false),
        }
    }
}
struct BranchedDummyConstPropagation {
    state: DataFlowAnalysisState<Instruction>,
}
impl BranchedDummyConstPropagation {
    // port: DataFlowAnalysisTest.BranchedDummyConstPropagation#BranchedDummyConstPropagation
    fn new(cfg: ControlFlowGraph<Instruction>) -> Self {
        Self {
            state: DataFlowAnalysisState::new(cfg, true, true),
        }
    }
}
impl DataFlowAnalysis<Instruction, ConstPropLatticeElement> for DummyConstPropagation {
    fn state(&self) -> &DataFlowAnalysisState<Instruction> {
        &self.state
    }
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<Instruction> {
        &mut self.state
    }
    // port: DataFlowAnalysisTest.DummyConstPropagation#isForward
    fn is_forward(&self) -> bool {
        true
    }
    // port: DataFlowAnalysisTest.DummyConstPropagation#createFlowJoiner
    fn create_flow_joiner(&self) -> Box<dyn FlowJoiner<ConstPropLatticeElement>> {
        Box::new(ConstPropJoinOp::default())
    }
    // port: DataFlowAnalysisTest.DummyConstPropagation#flowThrough
    fn flow_through(
        &mut self,
        _compiler: &mut Compiler,
        node: Instruction,
        input: ConstPropLatticeElement,
    ) -> ConstPropLatticeElement {
        match node {
            Instruction::Arithmetic(a) => flow_through_arithmetic_instruction(a, input),
            Instruction::Branch(_) => input.clone(),
        }
    }
    // port: DataFlowAnalysisTest.DummyConstPropagation#createEntryLattice
    fn create_entry_lattice(&mut self, _compiler: &mut Compiler) -> ConstPropLatticeElement {
        ConstPropLatticeElement::default()
    }
    // port: DataFlowAnalysisTest.DummyConstPropagation#createInitialEstimateLattice
    fn create_initial_estimate_lattice(&self) -> ConstPropLatticeElement {
        ConstPropLatticeElement {
            is_top: true,
            ..ConstPropLatticeElement::default()
        }
    }
}
impl DataFlowAnalysis<Instruction, ConstPropLatticeElement> for BranchedDummyConstPropagation {
    fn state(&self) -> &DataFlowAnalysisState<Instruction> {
        &self.state
    }
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<Instruction> {
        &mut self.state
    }
    // port: DataFlowAnalysisTest.BranchedDummyConstPropagation#isForward
    fn is_forward(&self) -> bool {
        true
    }
    // port: DataFlowAnalysisTest.BranchedDummyConstPropagation#isBranched
    fn is_branched(&self) -> bool {
        true
    }
    // port: DataFlowAnalysisTest.BranchedDummyConstPropagation#flowThrough
    fn flow_through(
        &mut self,
        _compiler: &mut Compiler,
        node: Instruction,
        input: ConstPropLatticeElement,
    ) -> ConstPropLatticeElement {
        if let Instruction::Arithmetic(a) = node {
            flow_through_arithmetic_instruction(a, input)
        } else {
            input.clone()
        }
    }
    // port: DataFlowAnalysisTest.BranchedDummyConstPropagation#createEntryLattice
    fn create_entry_lattice(&mut self, _compiler: &mut Compiler) -> ConstPropLatticeElement {
        ConstPropLatticeElement::new()
    }
    // port: DataFlowAnalysisTest.BranchedDummyConstPropagation#createInitialEstimateLattice
    fn create_initial_estimate_lattice(&self) -> ConstPropLatticeElement {
        ConstPropLatticeElement::new_with_top(true)
    }
    // port: DataFlowAnalysisTest.BranchedDummyConstPropagation#createFlowJoiner
    fn create_flow_joiner(&self) -> Box<dyn FlowJoiner<ConstPropLatticeElement>> {
        Box::new(ConstPropJoinOp::default())
    }
    // port: DataFlowAnalysisTest.BranchedDummyConstPropagation#createFlowBrancher
    fn create_flow_brancher(
        &mut self,
        _compiler: &mut Compiler,
        node: Instruction,
        input: ConstPropLatticeElement,
    ) -> Box<dyn FlowBrancher<ConstPropLatticeElement, Self>> {
        let a_result = if let Instruction::Arithmetic(a) = node.clone() {
            Some(flow_through_arithmetic_instruction(a, input.clone()))
        } else {
            None
        };
        Box::new(move |branch| {
            if let Some(a) = &a_result {
                return a.clone();
            }
            let Instruction::Branch(b) = &node else {
                panic!()
            };
            let mut result = input.clone();
            if branch == Branch::ON_FALSE {
                if let Value::Variable(v) = &b.condition {
                    result.const_map.insert(v.clone(), 0);
                }
            }
            result
        })
    }
}
// port: DataFlowAnalysisTest#verifyInHas
fn verify_in_has(
    analysis: &impl DataFlowAnalysis<Instruction, ConstPropLatticeElement>,
    node: DiGraphNode,
    var: Variable,
    constant: Option<i32>,
) {
    let state = node
        .get_annotation_as::<LinearFlowState<ConstPropLatticeElement>>(analysis.get_cfg())
        .unwrap();
    veritfy_lattice_element_has(state.get_in(), var, constant);
}
// port: DataFlowAnalysisTest#verifyOutHas
fn verify_out_has(
    analysis: &impl DataFlowAnalysis<Instruction, ConstPropLatticeElement>,
    node: DiGraphNode,
    var: Variable,
    constant: Option<i32>,
) {
    let state = node
        .get_annotation_as::<LinearFlowState<ConstPropLatticeElement>>(analysis.get_cfg())
        .unwrap();
    veritfy_lattice_element_has(state.get_out(), var, constant);
}
// port: DataFlowAnalysisTest#veritfyLatticeElementHas
fn veritfy_lattice_element_has(el: &ConstPropLatticeElement, var: Variable, constant: Option<i32>) {
    assert_eq!(el.const_map.get(&var).copied(), constant);
}

// port: DataFlowAnalysisTest#testSimpleIf
#[test]
fn test_simple_if() {
    // if (a) { b = 1; } else { b = 1; } c = b;
    let a = Variable::new("a");
    let b = Variable::new("b");
    let c = Variable::new("c");
    let inst1 = BranchInstruction::new(a.clone());
    let inst2 = new_assign_number_to_variable_instruction(b.clone(), 1);
    let inst3 = new_assign_number_to_variable_instruction(b.clone(), 1);
    let inst4 = new_assign_variable_to_variable_instruction(c.clone(), b.clone());
    let mut cfg = ControlFlowGraph::new(inst1.clone(), true, true);
    let n1 = cfg.create_node(Some(inst1.clone()));
    let n2 = cfg.create_node(Some(inst2.clone()));
    let n3 = cfg.create_node(Some(inst3.clone()));
    let n4 = cfg.create_node(Some(inst4.clone()));
    cfg.connect(Some(inst1.clone()), Branch::ON_FALSE, Some(inst2.clone()));
    cfg.connect(Some(inst1.clone()), Branch::ON_TRUE, Some(inst3.clone()));
    cfg.connect(Some(inst2.clone()), Branch::UNCOND, Some(inst4.clone()));
    cfg.connect(Some(inst3.clone()), Branch::UNCOND, Some(inst4.clone()));

    let mut const_prop = DummyConstPropagation::new(cfg);
    const_prop.analyze(&mut Compiler::new());

    // We cannot conclude anything from if (a).
    verify_in_has(&const_prop, n1, a.clone(), None);
    verify_in_has(&const_prop, n1, b.clone(), None);
    verify_in_has(&const_prop, n1, c.clone(), None);
    verify_out_has(&const_prop, n1, a.clone(), None);
    verify_out_has(&const_prop, n1, b.clone(), None);
    verify_out_has(&const_prop, n1, c.clone(), None);

    // We can conclude b = 1 after the instruction.
    verify_in_has(&const_prop, n2, a.clone(), None);
    verify_in_has(&const_prop, n2, b.clone(), None);
    verify_in_has(&const_prop, n2, c.clone(), None);
    verify_out_has(&const_prop, n2, a.clone(), None);
    verify_out_has(&const_prop, n2, b.clone(), Some(1));
    verify_out_has(&const_prop, n2, c.clone(), None);

    // Same as above.
    verify_in_has(&const_prop, n3, a.clone(), None);
    verify_in_has(&const_prop, n3, b.clone(), None);
    verify_in_has(&const_prop, n3, c.clone(), None);
    verify_out_has(&const_prop, n3, a.clone(), None);
    verify_out_has(&const_prop, n3, b.clone(), Some(1));
    verify_out_has(&const_prop, n3, c.clone(), None);

    // After the merge we should still have b = 1.
    verify_in_has(&const_prop, n4, a.clone(), None);
    verify_in_has(&const_prop, n4, b.clone(), Some(1));
    verify_in_has(&const_prop, n4, c.clone(), None);
    verify_out_has(&const_prop, n4, a.clone(), None);
    // After the instruction both b and c are 1.
    verify_out_has(&const_prop, n4, b.clone(), Some(1));
    verify_out_has(&const_prop, n4, c.clone(), Some(1));
}

// port: DataFlowAnalysisTest#testSimpleLoop
#[test]
fn test_simple_loop() {
    // a = 0; do { a = a + 1 } while (b); c = a;
    let a = Variable::new("a");
    let b = Variable::new("b");
    let c = Variable::new("c");
    let inst1 = new_assign_number_to_variable_instruction(a.clone(), 0);
    let inst2 = ArithmeticInstruction::new(a.clone(), a.clone(), Operation::ADD, 1);
    let inst3 = BranchInstruction::new(b.clone());
    let inst4 = new_assign_variable_to_variable_instruction(c.clone(), a.clone());
    let mut cfg = ControlFlowGraph::new(inst1.clone(), true, true);
    let n1 = cfg.create_node(Some(inst1.clone()));
    let n2 = cfg.create_node(Some(inst2.clone()));
    let n3 = cfg.create_node(Some(inst3.clone()));
    let n4 = cfg.create_node(Some(inst4.clone()));
    cfg.connect(Some(inst1.clone()), Branch::UNCOND, Some(inst2.clone()));
    cfg.connect(Some(inst2.clone()), Branch::UNCOND, Some(inst3.clone()));
    cfg.connect(Some(inst3.clone()), Branch::ON_TRUE, Some(inst2.clone()));
    cfg.connect(Some(inst3.clone()), Branch::ON_FALSE, Some(inst4.clone()));

    let mut const_prop = DummyConstPropagation::new(cfg);
    // This will also show that the framework terminates properly.
    const_prop.analyze(&mut Compiler::new());

    // a = 0 is the only thing we know.
    verify_in_has(&const_prop, n1, a.clone(), None);
    verify_in_has(&const_prop, n1, b.clone(), None);
    verify_in_has(&const_prop, n1, c.clone(), None);
    verify_out_has(&const_prop, n1, a.clone(), Some(0));
    verify_out_has(&const_prop, n1, b.clone(), None);
    verify_out_has(&const_prop, n1, c.clone(), None);

    // Nothing is provable in this program, so confirm that we haven't
    // erroneously "proven" something.
    verify_in_has(&const_prop, n2, a.clone(), None);
    verify_in_has(&const_prop, n2, b.clone(), None);
    verify_in_has(&const_prop, n2, c.clone(), None);
    verify_out_has(&const_prop, n2, a.clone(), None);
    verify_out_has(&const_prop, n2, b.clone(), None);
    verify_out_has(&const_prop, n2, c.clone(), None);

    verify_in_has(&const_prop, n3, a.clone(), None);
    verify_in_has(&const_prop, n3, b.clone(), None);
    verify_in_has(&const_prop, n3, c.clone(), None);
    verify_out_has(&const_prop, n3, a.clone(), None);
    verify_out_has(&const_prop, n3, b.clone(), None);
    verify_out_has(&const_prop, n3, c.clone(), None);

    verify_in_has(&const_prop, n4, a.clone(), None);
    verify_in_has(&const_prop, n4, b.clone(), None);
    verify_in_has(&const_prop, n4, c.clone(), None);
    verify_out_has(&const_prop, n4, a.clone(), None);
    verify_out_has(&const_prop, n4, b.clone(), None);
    verify_out_has(&const_prop, n4, c.clone(), None);
}

// port: DataFlowAnalysisTest#testBranchedSimpleIf
#[test]
fn test_branched_simple_if() {
    // if (a) { a = 0; } else { b = 0; } c = b;
    let a = Variable::new("a");
    let b = Variable::new("b");
    let c = Variable::new("c");
    let inst1 = BranchInstruction::new(a.clone());
    let inst2 = new_assign_number_to_variable_instruction(a.clone(), 0);
    let inst3 = new_assign_number_to_variable_instruction(b.clone(), 0);
    let inst4 = new_assign_variable_to_variable_instruction(c.clone(), b.clone());
    let mut cfg = ControlFlowGraph::new(inst1.clone(), true, true);
    let n1 = cfg.create_node(Some(inst1.clone()));
    let n2 = cfg.create_node(Some(inst2.clone()));
    let n3 = cfg.create_node(Some(inst3.clone()));
    let n4 = cfg.create_node(Some(inst4.clone()));
    cfg.connect(Some(inst1.clone()), Branch::ON_TRUE, Some(inst2.clone()));
    cfg.connect(Some(inst1.clone()), Branch::ON_FALSE, Some(inst3.clone()));
    cfg.connect(Some(inst2.clone()), Branch::UNCOND, Some(inst4.clone()));
    cfg.connect(Some(inst3.clone()), Branch::UNCOND, Some(inst4.clone()));

    let mut const_prop = BranchedDummyConstPropagation::new(cfg);
    const_prop.analyze(&mut Compiler::new());

    // We cannot conclude anything from if (a).
    verify_in_has(&const_prop, n1, a.clone(), None);
    verify_in_has(&const_prop, n1, b.clone(), None);
    verify_in_has(&const_prop, n1, c.clone(), None);

    // Nothing is known on the true branch.
    verify_in_has(&const_prop, n2, a.clone(), None);
    verify_in_has(&const_prop, n2, b.clone(), None);
    verify_in_has(&const_prop, n2, c.clone(), None);

    // Verify that we have a = 0 on the false branch.
    verify_in_has(&const_prop, n3, a.clone(), Some(0));
    verify_in_has(&const_prop, n3, b.clone(), None);
    verify_in_has(&const_prop, n3, c.clone(), None);

    // After the merge we should still have a = 0.
    verify_in_has(&const_prop, n4, a.clone(), Some(0));
}
#[derive(Clone, Debug)]
struct Counter(Arc<AtomicI32>);
impl PartialEq for Counter {
    fn eq(&self, b: &Self) -> bool {
        Arc::ptr_eq(&self.0, &b.0)
    }
}
impl Eq for Counter {}
impl std::hash::Hash for Counter {
    fn hash<H: std::hash::Hasher>(&self, s: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.0), s)
    }
}
#[derive(Clone)]
struct Step;
impl PartialEq for Step {
    // port: DataFlowAnalysisTest.DivergentAnalysis.Step#equals
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}
impl LatticeEquals for Step {
    // Rust-only (LatticeEquals): Step#equals, as DataFlowAnalysis#flow calls it
    fn lattice_equals(&self, _compiler: &mut Compiler, other: &Self) -> bool {
        self == other
    }
}
struct DivergentAnalysis {
    state: DataFlowAnalysisState<Counter>,
}
struct StepJoiner;
impl FlowJoiner<Step> for StepJoiner {
    // port: DataFlowAnalysisTest.DivergentAnalysis#createFlowJoiner.joinFlow
    fn join_flow(&mut self, _compiler: &mut Compiler, _x: Step) {}
    // port: DataFlowAnalysisTest.DivergentAnalysis#createFlowJoiner.finish
    fn finish(self: Box<Self>) -> Step {
        Step
    }
}
impl DataFlowAnalysis<Counter, Step> for DivergentAnalysis {
    fn state(&self) -> &DataFlowAnalysisState<Counter> {
        &self.state
    }
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<Counter> {
        &mut self.state
    }
    // port: DataFlowAnalysisTest.DivergentAnalysis#isForward
    fn is_forward(&self) -> bool {
        true
    }
    // port: DataFlowAnalysisTest.DivergentAnalysis#flowThrough
    fn flow_through(&mut self, _compiler: &mut Compiler, node: Counter, input: Step) -> Step {
        node.0.fetch_add(1, Ordering::Relaxed);
        input
    }
    // port: DataFlowAnalysisTest.DivergentAnalysis#createEntryLattice
    fn create_entry_lattice(&mut self, _compiler: &mut Compiler) -> Step {
        Step
    }
    // port: DataFlowAnalysisTest.DivergentAnalysis#createInitialEstimateLattice
    fn create_initial_estimate_lattice(&self) -> Step {
        Step
    }
    // port: DataFlowAnalysisTest.DivergentAnalysis#createFlowJoiner
    fn create_flow_joiner(&self) -> Box<dyn FlowJoiner<Step>> {
        Box::new(StepJoiner)
    }
}
// port: DataFlowAnalysisTest#testMaxIterationsExceededException
#[test]
fn test_max_iterations_exceeded_exception() {
    let entrypoint = Counter(Arc::new(AtomicI32::new(0)));
    let mut cfg = ControlFlowGraph::new(entrypoint.clone(), true, true);
    cfg.connect(
        Some(entrypoint.clone()),
        Branch::UNCOND,
        Some(entrypoint.clone()),
    );
    let mut const_prop = DivergentAnalysis {
        state: DataFlowAnalysisState::new(cfg, true, false),
    };
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        const_prop.analyze(&mut Compiler::new())
    }))
    .unwrap_err();
    assert_eq!(entrypoint.0.load(Ordering::Relaxed), MAX_STEPS_PER_NODE + 1);
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(message.starts_with("Dataflow analysis appears to diverge around: "));
}

impl closure_jscomp::graph::annotation::Annotation for ConstPropLatticeElement {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    // port: DataFlowAnalysisTest.ConstPropLatticeElement#toString
    fn to_string(&self) -> String {
        if self.is_top {
            return "TOP".into();
        }
        let mut out = "{".to_string();
        for (var, value) in &self.const_map {
            out.push_str(&format!("{}={} ", var.0, value));
        }
        out.push('}');
        out
    }
}
impl closure_jscomp::graph::annotation::Annotation for Step {
    // port: Object#toString
    fn to_string(&self) -> String {
        "com.google.javascript.jscomp.DataFlowAnalysisTest$DivergentAnalysis$Step@0".into()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl std::fmt::Display for Instruction {
    // port: DataFlowAnalysisTest.ArithmeticInstruction#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arithmetic(a) => {
                let val = |v: Value| match v {
                    Value::Variable(v) => v.0.to_string(),
                    Value::Number(n) => n.0.to_string(),
                };
                let op = match a.operation {
                    Operation::ADD => "+",
                    Operation::SUB => "-",
                    Operation::DIV => "/",
                    Operation::MUL => "*",
                };
                write!(
                    f,
                    "{} = {}{}{}",
                    a.result.0,
                    val(a.operand1.clone()),
                    op,
                    val(a.operand2.clone())
                )
            }
            Self::Branch(b) => write!(
                f,
                "com.google.javascript.jscomp.DataFlowAnalysisTest$BranchInstruction@{:x}",
                Arc::as_ptr(&b.identity) as usize as u32
            ),
        }
    }
}
impl std::fmt::Display for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.DataFlowAnalysisTest$DivergentAnalysis$Counter@{:x}",
            Arc::as_ptr(&self.0) as usize as u32
        )
    }
}

impl Variable {
    // port: DataFlowAnalysisTest.Variable#getName
    fn get_name(&self) -> JsString {
        self.0.clone()
    }
    // port: DataFlowAnalysisTest.Variable#hashCode
    fn hash_code(&self) -> i32 {
        self.0.hash_code()
    }
}
impl PartialEq for Variable {
    // port: DataFlowAnalysisTest.Variable#equals
    fn eq(&self, other: &Self) -> bool {
        other.0 == self.0
    }
}
impl Eq for Variable {}
impl std::hash::Hash for Variable {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        std::hash::Hash::hash(&self.hash_code(), h);
    }
}
impl std::fmt::Display for Variable {
    // port: DataFlowAnalysisTest.Variable#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl NumberValue {
    // port: DataFlowAnalysisTest.NumberValue#NumberValue
    fn new(v: i32) -> Self {
        Self(v)
    }
    // port: DataFlowAnalysisTest.NumberValue#getValue
    fn get_value(&self) -> i32 {
        self.0
    }
    // port: DataFlowAnalysisTest.NumberValue#hashCode
    fn hash_code(&self) -> i32 {
        self.0
    }
}
impl std::fmt::Display for NumberValue {
    // port: DataFlowAnalysisTest.NumberValue#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl Value {
    // port: DataFlowAnalysisTest.Value#isNumber
    fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }
    // port: DataFlowAnalysisTest.Value#isVariable
    fn is_variable(&self) -> bool {
        matches!(self, Self::Variable(_))
    }
}
impl Instruction {
    // port: DataFlowAnalysisTest.Instruction#isArithmetic
    fn is_arithmetic(&self) -> bool {
        matches!(self, Self::Arithmetic(_))
    }
    // port: DataFlowAnalysisTest.Instruction#isBranch
    fn is_branch(&self) -> bool {
        matches!(self, Self::Branch(_))
    }
}
impl std::fmt::Display for Operation {
    // port: DataFlowAnalysisTest.Operation#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ADD => "+",
            Self::SUB => "-",
            Self::DIV => "/",
            Self::MUL => "*",
        })
    }
}
impl ArithmeticInstruction {
    // port: DataFlowAnalysisTest.ArithmeticInstruction#getOperator
    fn get_operator(&self) -> Operation {
        self.operation
    }
    // port: DataFlowAnalysisTest.ArithmeticInstruction#setOperator
    fn set_operator(&mut self, op: Operation) {
        self.operation = op;
    }
    // port: DataFlowAnalysisTest.ArithmeticInstruction#getOperand1
    fn get_operand1(&self) -> Value {
        self.operand1.clone()
    }
    // port: DataFlowAnalysisTest.ArithmeticInstruction#setOperand1
    fn set_operand1(&mut self, operand1: Value) {
        self.operand1 = operand1;
    }
    // port: DataFlowAnalysisTest.ArithmeticInstruction#getOperand2
    fn get_operand2(&self) -> Value {
        self.operand2.clone()
    }
    // port: DataFlowAnalysisTest.ArithmeticInstruction#setOperand2
    fn set_operand2(&mut self, operand2: Value) {
        self.operand2 = operand2;
    }
    // port: DataFlowAnalysisTest.ArithmeticInstruction#getResult
    fn get_result(&self) -> Variable {
        self.result.clone()
    }
    // port: DataFlowAnalysisTest.ArithmeticInstruction#setResult
    fn set_result(&mut self, result: Variable) {
        self.result = result;
    }
    // port: DataFlowAnalysisTest.ArithmeticInstruction#hashCode
    fn hash_code(&self) -> i32 {
        Instruction::Arithmetic(self.clone())
            .to_string()
            .hash_code()
    }
}
impl BranchInstruction {
    // port: DataFlowAnalysisTest.BranchInstruction#getCondition
    fn get_condition(&self) -> Value {
        self.condition.clone()
    }
    // port: DataFlowAnalysisTest.BranchInstruction#setCondition
    fn set_condition(&mut self, condition: Value) {
        self.condition = condition;
    }
}
impl PartialEq for BranchInstruction {
    // port: Object#equals (BranchInstruction does not override equality)
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}
impl Eq for BranchInstruction {}
impl std::hash::Hash for BranchInstruction {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.identity), h);
    }
}
impl ConstPropLatticeElement {
    // port: DataFlowAnalysisTest.ConstPropLatticeElement#ConstPropLatticeElement()
    fn new() -> Self {
        Self::new_with_top(false)
    }
    // port: DataFlowAnalysisTest.ConstPropLatticeElement#ConstPropLatticeElement(boolean)
    fn new_with_top(is_top: bool) -> Self {
        Self {
            is_top,
            const_map: IndexMap::new(),
        }
    }
    // port: DataFlowAnalysisTest.ConstPropLatticeElement#ConstPropLatticeElement(ConstPropLatticeElement)
    fn copy(other: &Self) -> Self {
        Self {
            is_top: other.is_top,
            const_map: other.const_map.clone(),
        }
    }
    // port: DataFlowAnalysisTest.ConstPropLatticeElement#hashCode
    fn hash_code(&self) -> i32 {
        self.const_map.iter().fold(0i32, |sum, (var, num)| {
            sum.wrapping_add(var.hash_code() ^ *num)
        })
    }
}
impl PartialEq for ConstPropLatticeElement {
    // port: DataFlowAnalysisTest.ConstPropLatticeElement#equals
    fn eq(&self, other: &Self) -> bool {
        self.is_top == other.is_top && self.const_map == other.const_map
    }
}
impl LatticeEquals for ConstPropLatticeElement {
    // Rust-only (LatticeEquals): ConstPropLatticeElement#equals, as DataFlowAnalysis#flow calls it
    fn lattice_equals(&self, _compiler: &mut Compiler, other: &Self) -> bool {
        self == other
    }
}
impl Step {
    // port: DataFlowAnalysisTest.DivergentAnalysis.Step#hashCode
    fn hash_code(&self) -> i32 {
        0
    }
}
impl DivergentAnalysis {
    // port: DataFlowAnalysisTest.DivergentAnalysis#DivergentAnalysis
    fn new(cfg: ControlFlowGraph<Counter>) -> Self {
        Self {
            state: DataFlowAnalysisState::new(cfg, true, false),
        }
    }
}

impl PartialEq for NumberValue {
    // port: DataFlowAnalysisTest.NumberValue#equals
    fn eq(&self, other: &Self) -> bool {
        other.0 == self.0
    }
}

impl PartialEq for ArithmeticInstruction {
    // port: DataFlowAnalysisTest.ArithmeticInstruction#equals
    fn eq(&self, other: &Self) -> bool {
        other.order == self.order
            && other.operation == self.operation
            && other.operand1 == self.operand1
            && other.operand2 == self.operand2
            && other.result == self.result
    }
}
impl std::hash::Hash for ArithmeticInstruction {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        std::hash::Hash::hash(&self.hash_code(), h);
    }
}

impl std::hash::Hash for NumberValue {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        std::hash::Hash::hash(&self.hash_code(), h);
    }
}

mod escaped_locals_tests {
    use closure_jscomp::{
        Compiler, abstract_compiler::LifeCycleStage, compiler_options::CompilerOptions,
        control_flow_analysis::ControlFlowAnalysis, data_flow_analysis::DataFlowAnalysis,
        google_coding_convention::GoogleCodingConvention,
        live_variables_analysis::LiveVariablesAnalysis, node_util::NodeUtil, scope::ScopeId,
        syntactic_scope_creator::SyntacticScopeCreator, var::VarId,
    };
    use closure_rhino::{input_id::InputId, token::Token};
    use indexmap::IndexSet;
    use std::sync::Arc;

    // port: DataFlowAnalysisTest#testEscaped
    #[test]
    fn test_escaped() {
        assert_eq!(
            (compute_escaped_locals(
                r###"function f() {
    var x = 0;
    setTimeout(function() { x++; });
    alert(x);
}
"###,
            ))
            .len(),
            1
        );
        assert_eq!(
            compute_escaped_locals(r###"function f() {var _x}"###).len(),
            1
        );
        assert_eq!(
            compute_escaped_locals(r###"function f() {try{} catch(e){}}"###).len(),
            1
        );
    }

    // port: DataFlowAnalysisTest#testEscapedFunctionLayered
    #[test]
    fn test_escaped_function_layered() {
        assert!(
            (compute_escaped_locals(
                r###"function f() {
    function ff() {
        var x = 0;
        setTimeout(function() { x++; });
        alert(x);
    }
}
"###,
            ))
            .is_empty()
        );
    }

    // port: DataFlowAnalysisTest#testEscapedLetConstSimple
    #[test]
    fn test_escaped_let_const_simple() {
        assert!(compute_escaped_locals(r###"function f() { let x = 0; x ++; x; }"###).is_empty());
    }

    // port: DataFlowAnalysisTest#testEscapedFunctionAssignment
    #[test]
    fn test_escaped_function_assignment() {
        assert!(
            compute_escaped_locals(r###"function f() {var x = function () { return 1; }; }"###)
                .is_empty()
        );
        assert!(
            compute_escaped_locals(r###"function f() {var x = function (y) { return y; }; }"###)
                .is_empty()
        );
        assert!(
            compute_escaped_locals(r###"function f() {let x = function () { return 1; }; }"###)
                .is_empty()
        );
    }

    // port: DataFlowAnalysisTest#testEscapedArrowFunction
    #[test]
    fn test_escaped_arrow_function() {
        assert!(
            (compute_escaped_locals(
                r###"function f() {const value = () => {
    var x = 0;
    setTimeout(function() { x++; });
    alert(x);
 };}
"###,
            ))
            .is_empty()
        );
    }

    // port: DataFlowAnalysisTest#testEscapedInstanceClassField
    #[test]
    fn test_escaped_instance_class_field() {
        assert_eq!(
            (compute_escaped_locals(
                r###"function f() {
  let s = 1;
  class Box {
    html = s;
  }
  return new Box();
}
"###,
            ))
            .len(),
            1
        );
    }

    // port: DataFlowAnalysisTest#testEscapedComputedInstanceClassField
    #[test]
    fn test_escaped_computed_instance_class_field() {
        assert_eq!(
            (compute_escaped_locals(
                r###"function f() {
  let s = 1;
  class Box {
    ['html'] = s;
  }
  return new Box();
}
"###,
            ))
            .len(),
            1
        );
    }

    // port: DataFlowAnalysisTest#testNotEscapedStaticClassField
    #[test]
    fn test_not_escaped_static_class_field() {
        assert!(
            (compute_escaped_locals(
                r###"function f() {
  let s = 1;
  class Box {
    static html = s;
  }
  return Box;
}
"###,
            ))
            .is_empty()
        );
    }

    // port: DataFlowAnalysisTest#testNotEscapedStaticComputedClassField
    #[test]
    fn test_not_escaped_static_computed_class_field() {
        assert!(
            (compute_escaped_locals(
                r###"function f() {
  let s = 1;
  class Box {
    static ['html'] = s;
  }
  return Box;
}
"###,
            ))
            .is_empty()
        );
    }

    // port: DataFlowAnalysisTest#testNotEscapedComputedClassFieldKey
    #[test]
    fn test_not_escaped_computed_class_field_key() {
        assert!(
            (compute_escaped_locals(
                r###"function f() {
  let s = 'key';
  class Box {
    [s] = 1;
  }
  return new Box();
}
"###,
            ))
            .is_empty()
        );
    }

    // port: DataFlowAnalysisTest#computeEscapedLocals
    fn compute_escaped_locals(src: &str) -> IndexSet<VarId> {
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        compiler.init_options(options);
        compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
        let parsed = compiler.parse_test_code(src);
        let n = parsed.remove_first_child(&mut compiler).unwrap();
        let script = compiler.new_node_with_child(Token::SCRIPT, n);
        script.set_input_id(&mut compiler, Some(Arc::new(InputId::new("test"))));
        assert!(compiler.get_errors().is_empty());
        let mut scope_creator = SyntacticScopeCreator::new();
        let global = ScopeId::create_global_scope(&mut compiler, script);
        let scope = scope_creator.create_scope(&mut compiler, n, Some(global));
        let child_scope = if script
            .get_first_child(&compiler)
            .unwrap()
            .is_function(&compiler)
        {
            let body = NodeUtil::get_function_body(&compiler, n);
            Some(scope_creator.create_scope(&mut compiler, body, Some(scope)))
        } else {
            None
        };
        let cfg = ControlFlowAnalysis::builder()
            .set_cfg_root(script)
            .set_include_edge_annotations(true)
            .compute_cfg(&mut compiler);
        let all_vars =
            NodeUtil::get_all_vars_declared_in_function(&mut compiler, &mut scope_creator, scope);
        let mut analysis = LiveVariablesAnalysis::new(
            &mut compiler,
            cfg,
            scope,
            child_scope,
            &mut scope_creator,
            all_vars,
        );
        analysis.analyze(&mut compiler);
        analysis.get_escaped_locals().clone()
    }
}
