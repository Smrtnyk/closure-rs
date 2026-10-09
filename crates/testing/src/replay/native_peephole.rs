/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
 * Copyright 2026 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ExploitAssigns.java,
//   src/com/google/javascript/jscomp/MinimizeExitPoints.java,
//   src/com/google/javascript/jscomp/OptimizeLetAndConstPeephole.java,
//   src/com/google/javascript/jscomp/PeepholeCollectPropertyAssignments.java,
//   src/com/google/javascript/jscomp/PeepholeFoldConstants.java,
//   src/com/google/javascript/jscomp/PeepholeMinimizeConditions.java,
//   src/com/google/javascript/jscomp/PeepholeOptimizationsPass.java,
//   src/com/google/javascript/jscomp/PeepholeRemoveDeadCode.java,
//   src/com/google/javascript/jscomp/PeepholeReplaceKnownMethods.java,
//   src/com/google/javascript/jscomp/PeepholeSubstituteAlternateSyntax.java,
//   src/com/google/javascript/jscomp/StatementFusion.java,
//   test/com/google/javascript/jscomp/PeepholeOptimizationsPassTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for the peephole framework: `PeepholeOptimizationsPass`,
//! its optimizations, and the `PeepholeOptimizationsPassTest_Helpers` copies of the test-local
//! optimizations that the descriptors construct.
use crate::{
    replay::{
        registry::Entry,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    compiler_pass::CompilerPass,
    exploit_assigns::ExploitAssigns,
    minimize_exit_points::MinimizeExitPoints,
    optimize_let_and_const_peephole::OptimizeLetAndConstPeephole,
    peephole_collect_property_assignments::PeepholeCollectPropertyAssignments,
    peephole_fold_constants::PeepholeFoldConstants,
    peephole_minimize_conditions::PeepholeMinimizeConditions,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    peephole_remove_dead_code::PeepholeRemoveDeadCode,
    peephole_replace_known_methods::PeepholeReplaceKnownMethods,
    peephole_substitute_alternate_syntax::PeepholeSubstituteAlternateSyntax,
    statement_fusion::StatementFusion,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{node::NodeId, token::Token};
use std::{cell::RefCell, rc::Rc};

// port: ReplayDsl#invoke (resolved signatures backed by native peephole implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.PeepholeOptimizationsPass#<init>(com.google.javascript.jscomp.AbstractCompiler,java.lang.String,com.google.javascript.jscomp.AbstractPeepholeOptimization[])" => {
            peephole_optimizations_pass
        }
        "com.google.javascript.jscomp.MinimizeExitPoints#<init>()" => minimize_exit_points,
        "com.google.javascript.jscomp.PeepholeCollectPropertyAssignments#<init>()" => {
            peephole_collect_property_assignments
        }
        "com.google.javascript.jscomp.PeepholeRemoveDeadCode#<init>()" => peephole_remove_dead_code,
        "com.google.javascript.jscomp.PeepholeFoldConstants#<init>(boolean,boolean)" => {
            peephole_fold_constants
        }
        "com.google.javascript.jscomp.PeepholeMinimizeConditions#<init>(boolean)" => {
            peephole_minimize_conditions
        }
        "com.google.javascript.jscomp.PeepholeSubstituteAlternateSyntax#<init>(boolean)" => {
            peephole_substitute_alternate_syntax
        }
        "com.google.javascript.jscomp.PeepholeReplaceKnownMethods#<init>(boolean,boolean)" => {
            peephole_replace_known_methods
        }
        "com.google.javascript.jscomp.StatementFusion#<init>()" => statement_fusion,
        "com.google.javascript.jscomp.ExploitAssigns#<init>()" => exploit_assigns,
        "com.google.javascript.jscomp.OptimizeLetAndConstPeephole#<init>(boolean)" => {
            optimize_let_and_const_peephole
        }
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$RenameYToX#<init>()" => {
            rename_y_to_x
        }
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$RemoveParentVarsForNodesNamedX#<init>()" => {
            remove_parent_vars_for_nodes_named_x
        }
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$RemoveNodesNamedXOptimization#<init>()" => {
            remove_nodes_named_x_optimization
        }
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$RemoveNodesNamedXUnderVarOptimization#<init>()" => {
            remove_nodes_named_x_under_var_optimization
        }
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$Anon1#<init>(java.util.List)" => {
            anon1
        }
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$Anon2#<init>(java.util.List)" => {
            anon2
        }
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$Anon3#<init>()" => {
            anon3
        }
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$Anon4#<init>()" => {
            anon4
        }
        _ => return None,
    })
}

/// A constructed `AbstractPeepholeOptimization`, held until a `PeepholeOptimizationsPass`
/// constructor takes it.
struct NativePeepholeOptimization {
    class: &'static str,
    optimization: Option<Box<dyn AbstractPeepholeOptimization>>,
}
impl NativeObject for NativePeepholeOptimization {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        self.class
    }
    // port: ReplayDsl#invoke (native method receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: ReplayDsl#invoke (constructor result of a native peephole optimization)
fn optimization(
    class: &'static str,
    optimization: Box<dyn AbstractPeepholeOptimization>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativePeepholeOptimization {
            class,
            optimization: Some(optimization),
        },
    ))))
}
// port: PeepholeOptimizationsPass#PeepholeOptimizationsPass(AbstractCompiler, String, AbstractPeepholeOptimization...)
fn peephole_optimizations_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(_),
        DslValue::String(pass_name),
        optimizations,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let DslValue::Array { items, .. } = optimizations.untyped() else {
        return Err(bad());
    };
    let mut peephole_optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = Vec::new();
    for item in items {
        let DslValue::Native(native) = item else {
            return Err(bad());
        };
        let mut native = native.borrow_mut();
        let Some(native) = native
            .as_any_mut()
            .downcast_mut::<NativePeepholeOptimization>()
        else {
            return Err(bad());
        };
        peephole_optimizations.push(native.optimization.take().ok_or_else(|| {
            Throwable::HarnessError("peephole optimization already used by another pass".into())
        })?);
    }
    let pass: Box<dyn CompilerPass> = Box::new(PeepholeOptimizationsPass::new(
        pass_name.to_string_lossy(),
        peephole_optimizations,
    ));
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
// port: MinimizeExitPoints#MinimizeExitPoints
fn minimize_exit_points(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    optimization(
        "com.google.javascript.jscomp.MinimizeExitPoints",
        Box::new(MinimizeExitPoints::new()),
    )
}
// port: PeepholeRemoveDeadCode#PeepholeRemoveDeadCode
fn peephole_remove_dead_code(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    optimization(
        "com.google.javascript.jscomp.PeepholeRemoveDeadCode",
        Box::new(PeepholeRemoveDeadCode::new()),
    )
}
// port: PeepholeFoldConstants#PeepholeFoldConstants
fn peephole_fold_constants(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [late, should_use_types] = args.as_slice() else {
        return Err(bad());
    };
    let (DslValue::Bool(late), DslValue::Bool(should_use_types)) =
        (late.untyped(), should_use_types.untyped())
    else {
        return Err(bad());
    };
    optimization(
        "com.google.javascript.jscomp.PeepholeFoldConstants",
        Box::new(PeepholeFoldConstants::new(*late, *should_use_types)),
    )
}
// port: PeepholeCollectPropertyAssignments#PeepholeCollectPropertyAssignments
fn peephole_collect_property_assignments(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    optimization(
        "com.google.javascript.jscomp.PeepholeCollectPropertyAssignments",
        Box::new(PeepholeCollectPropertyAssignments::new()),
    )
}
// port: ReplayDsl#invoke (a single boolean constructor argument)
fn bool_arg(args: &[DslValue]) -> Result<bool, Throwable> {
    let [value] = args else {
        return Err(bad());
    };
    let DslValue::Bool(value) = value.untyped() else {
        return Err(bad());
    };
    Ok(*value)
}
// port: PeepholeMinimizeConditions#PeepholeMinimizeConditions
fn peephole_minimize_conditions(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let late = bool_arg(&args)?;
    optimization(
        "com.google.javascript.jscomp.PeepholeMinimizeConditions",
        Box::new(PeepholeMinimizeConditions::new(late)),
    )
}
// port: PeepholeSubstituteAlternateSyntax#PeepholeSubstituteAlternateSyntax
fn peephole_substitute_alternate_syntax(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let late = bool_arg(&args)?;
    optimization(
        "com.google.javascript.jscomp.PeepholeSubstituteAlternateSyntax",
        Box::new(PeepholeSubstituteAlternateSyntax::new(late)),
    )
}
// port: PeepholeReplaceKnownMethods#PeepholeReplaceKnownMethods
fn peephole_replace_known_methods(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [late, use_types] = args.as_slice() else {
        return Err(bad());
    };
    let (DslValue::Bool(late), DslValue::Bool(use_types)) = (late.untyped(), use_types.untyped())
    else {
        return Err(bad());
    };
    optimization(
        "com.google.javascript.jscomp.PeepholeReplaceKnownMethods",
        Box::new(PeepholeReplaceKnownMethods::new(*late, *use_types)),
    )
}
// port: StatementFusion#StatementFusion
fn statement_fusion(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    optimization(
        "com.google.javascript.jscomp.StatementFusion",
        Box::new(StatementFusion::new()),
    )
}
// port: ExploitAssigns#ExploitAssigns
fn exploit_assigns(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    optimization(
        "com.google.javascript.jscomp.ExploitAssigns",
        Box::new(ExploitAssigns::new()),
    )
}
// port: OptimizeLetAndConstPeephole#OptimizeLetAndConstPeephole
fn optimize_let_and_const_peephole(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let assume_output_is_wrapped = bool_arg(&args)?;
    optimization(
        "com.google.javascript.jscomp.OptimizeLetAndConstPeephole",
        Box::new(OptimizeLetAndConstPeephole::new(assume_output_is_wrapped)),
    )
}
/// `PeepholeOptimizationsPass#setRetraverseOnChange`, which the descriptors model as
/// `withFields` on the constructed pass (its body only assigns `retraverseOnChange`).
// port: ReplayValues#setField (PeepholeOptimizationsPass#retraverseOnChange)
pub fn set_pass_field(
    pass: &mut Box<dyn CompilerPass>,
    name: &str,
    value: &DslValue,
) -> Option<Result<(), Throwable>> {
    let pass = pass
        .as_any_mut()?
        .downcast_mut::<PeepholeOptimizationsPass>()?;
    Some(match (name, value.untyped()) {
        ("retraverseOnChange", DslValue::Bool(retraverse)) => {
            pass.set_retraverse_on_change(*retraverse);
            Ok(())
        }
        _ => Err(Throwable::Unported(format!(
            "com.google.javascript.jscomp.PeepholeOptimizationsPass#{name}"
        ))),
    })
}
// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

// The helper copies of PeepholeOptimizationsPassTest's test-local optimizations.

macro_rules! helper_optimization {
    ($name:ident, $class:literal) => {
        #[derive(Default)]
        struct $name {
            fields: AbstractPeepholeOptimizationFields,
        }
        impl $name {
            fn base_fields(&self) -> &AbstractPeepholeOptimizationFields {
                &self.fields
            }
            fn base_fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
                &mut self.fields
            }
            fn class(&self) -> &'static str {
                $class
            }
        }
    };
}

helper_optimization!(
    RemoveNodesNamedXUnderVarOptimization,
    "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$RemoveNodesNamedXUnderVarOptimization"
);
impl AbstractPeepholeOptimization for RemoveNodesNamedXUnderVarOptimization {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        self.base_fields()
    }
    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        self.base_fields_mut()
    }
    fn get_class_name(&self) -> &'static str {
        self.class()
    }
    // port: PeepholeOptimizationsPassTest.RemoveNodesNamedXUnderVarOptimization#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_var(compiler) {
            // Java's HashSet<Node>; the removal order does not change the result.
            let mut nodes_to_remove: IndexSet<NodeId> = IndexSet::<_>::default();
            let mut child = node.get_first_child(compiler);
            while let Some(c) = child {
                if c.get_string_ref(compiler) == "x" {
                    nodes_to_remove.insert(c);
                }
                child = c.get_next(compiler);
            }
            for child_to_remove in nodes_to_remove {
                self.report_change_to_enclosing_scope(compiler, node);
                child_to_remove.detach(compiler);
            }
        }
        Some(node)
    }
}

helper_optimization!(
    RemoveNodesNamedXOptimization,
    "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$RemoveNodesNamedXOptimization"
);
impl AbstractPeepholeOptimization for RemoveNodesNamedXOptimization {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        self.base_fields()
    }
    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        self.base_fields_mut()
    }
    fn get_class_name(&self) -> &'static str {
        self.class()
    }
    // port: PeepholeOptimizationsPassTest.RemoveNodesNamedXOptimization#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_name(compiler) && node.get_string_ref(compiler) == "x" {
            self.report_change_to_enclosing_scope(compiler, node);
            node.detach(compiler);
            return None;
        }
        Some(node)
    }
}

helper_optimization!(
    RemoveParentVarsForNodesNamedX,
    "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$RemoveParentVarsForNodesNamedX"
);
impl AbstractPeepholeOptimization for RemoveParentVarsForNodesNamedX {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        self.base_fields()
    }
    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        self.base_fields_mut()
    }
    fn get_class_name(&self) -> &'static str {
        self.class()
    }
    // port: PeepholeOptimizationsPassTest.RemoveParentVarsForNodesNamedX#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_name(compiler) && node.get_string_ref(compiler) == "x" {
            let parent = node.get_parent(compiler).unwrap();
            if parent.is_var(compiler) {
                self.report_change_to_enclosing_scope(compiler, parent);
                parent.detach(compiler);
                return None;
            }
        }
        Some(node)
    }
}

helper_optimization!(
    RenameYToX,
    "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$RenameYToX"
);
impl AbstractPeepholeOptimization for RenameYToX {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        self.base_fields()
    }
    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        self.base_fields_mut()
    }
    fn get_class_name(&self) -> &'static str {
        self.class()
    }
    // port: PeepholeOptimizationsPassTest.RenameYToX#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_name(compiler) && node.get_string_ref(compiler) == "y" {
            let replacement = compiler.new_string_with_token(Token::NAME, "x");
            node.replace_with(compiler, replacement);
            self.report_change_to_enclosing_scope(compiler, replacement);
            return Some(replacement);
        }
        Some(node)
    }
}

/// `note1Applied` / `note2Applied` of `testOptimizationOrder`: log each visited name with the
/// optimization's number. The shared `visitationLog` is only read by the (unreplayed) test
/// assertion, so the copies keep their own log.
struct NoteApplied {
    fields: AbstractPeepholeOptimizationFields,
    class: &'static str,
    suffix: &'static str,
    visitation_log: Vec<String>,
}
impl AbstractPeepholeOptimization for NoteApplied {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }
    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }
    fn get_class_name(&self) -> &'static str {
        self.class
    }
    // port: PeepholeOptimizationsPassTest$1#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_name(compiler) {
            self.visitation_log.push(format!(
                "{}{}",
                node.get_string(compiler).to_string_lossy(),
                self.suffix
            ));
        }
        Some(node)
    }
}

/// The anonymous optimizations of `testAddFeatureToEnclosingScript`.
struct AddFeatures {
    fields: AbstractPeepholeOptimizationFields,
    class: &'static str,
    token: Token,
    features: &'static [Feature],
}
impl AbstractPeepholeOptimization for AddFeatures {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }
    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }
    fn get_class_name(&self) -> &'static str {
        self.class
    }
    // port: PeepholeOptimizationsPassTest$3#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.get_token(compiler) == self.token {
            for feature in self.features {
                self.add_feature_to_enclosing_script(*feature);
            }
        }
        Some(node)
    }
}

// port: PeepholeOptimizationsPassTest.RemoveNodesNamedXUnderVarOptimization#<init>
fn remove_nodes_named_x_under_var_optimization(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let o = RemoveNodesNamedXUnderVarOptimization::default();
    optimization(o.class(), Box::new(o))
}
// port: PeepholeOptimizationsPassTest.RemoveNodesNamedXOptimization#<init>
fn remove_nodes_named_x_optimization(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let o = RemoveNodesNamedXOptimization::default();
    optimization(o.class(), Box::new(o))
}
// port: PeepholeOptimizationsPassTest.RemoveParentVarsForNodesNamedX#<init>
fn remove_parent_vars_for_nodes_named_x(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let o = RemoveParentVarsForNodesNamedX::default();
    optimization(o.class(), Box::new(o))
}
// port: PeepholeOptimizationsPassTest.RenameYToX#<init>
fn rename_y_to_x(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let o = RenameYToX::default();
    optimization(o.class(), Box::new(o))
}
// port: PeepholeOptimizationsPassTest#testOptimizationOrder (note1Applied)
fn anon1(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let class = "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$Anon1";
    optimization(
        class,
        Box::new(NoteApplied {
            fields: AbstractPeepholeOptimizationFields::new(),
            class,
            suffix: "1",
            visitation_log: Vec::new(),
        }),
    )
}
// port: PeepholeOptimizationsPassTest#testOptimizationOrder (note2Applied)
fn anon2(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let class = "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$Anon2";
    optimization(
        class,
        Box::new(NoteApplied {
            fields: AbstractPeepholeOptimizationFields::new(),
            class,
            suffix: "2",
            visitation_log: Vec::new(),
        }),
    )
}
// port: PeepholeOptimizationsPassTest#testAddFeatureToEnclosingScript (first optimization)
fn anon3(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let class = "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$Anon3";
    optimization(
        class,
        Box::new(AddFeatures {
            fields: AbstractPeepholeOptimizationFields::new(),
            class,
            token: Token::ADD,
            features: &[
                Feature::LET_DECLARATIONS,
                Feature::LET_DECLARATIONS,
                Feature::CLASSES,
            ],
        }),
    )
}
// port: PeepholeOptimizationsPassTest#testAddFeatureToEnclosingScript (second optimization)
fn anon4(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let class = "com.google.javascript.jscomp.PeepholeOptimizationsPassTest_Helpers$Anon4";
    optimization(
        class,
        Box::new(AddFeatures {
            fields: AbstractPeepholeOptimizationFields::new(),
            class,
            token: Token::SUB,
            features: &[Feature::CONST_DECLARATIONS],
        }),
    )
}
