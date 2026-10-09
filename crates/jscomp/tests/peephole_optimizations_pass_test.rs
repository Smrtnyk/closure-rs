/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/PeepholeOptimizationsPassTest.java.

//! Port of `PeepholeOptimizationsPassTest.java`: unit tests for PeepholeOptimizationsPass.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    compiler_pass::CompilerPass,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    node::{NodeId, ObjectProp, Prop},
    token::Token,
};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

type PassList = Vec<Box<dyn AbstractPeepholeOptimization>>;

struct PeepholeOptimizationsPassTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    /// Java's `currentPeepholePasses`; a factory because each `getProcessor` call moves the
    /// optimizations into its pass (Java shares the same objects).
    current_peephole_passes: Box<dyn FnMut() -> PassList>,
}

impl CompilerTestCaseHooks for Hooks {
    // port: PeepholeOptimizationsPassTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass: Box<dyn CompilerPass> = Box::new(PeepholeOptimizationsPass::new(
            self.get_name(),
            (self.current_peephole_passes)(),
        ));
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "PeepholeOptimizationsPassTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl PeepholeOptimizationsPassTest {
    // port: CompilerTestCase#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "PeepholeOptimizationsPassTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                current_peephole_passes: Box::new(Vec::new),
            },
        }
    }

    fn set_current_peephole_passes(&mut self, passes: impl FnMut() -> PassList + 'static) {
        self.hooks.current_peephole_passes = Box::new(passes);
    }

    fn test(&mut self, js: &str, expected: &str) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_same(&mut self, js: &str) {
        self.harness
            .test_same_string(&mut self.hooks, js)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

/// Declares a test-local optimization class with the base-class fields.
macro_rules! optimization_class {
    ($name:ident) => {
        #[derive(Default)]
        struct $name {
            fields: AbstractPeepholeOptimizationFields,
        }
    };
}

macro_rules! base_methods {
    ($class:literal) => {
        fn fields(&self) -> &AbstractPeepholeOptimizationFields {
            &self.fields
        }
        fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
            &mut self.fields
        }
        fn get_class_name(&self) -> &'static str {
            $class
        }
    };
}

/// PeepholeOptimizationsPass should handle the case when no peephole optimizations are turned
/// on.
// port: PeepholeOptimizationsPassTest#testEmptyPass
#[test]
fn test_empty_pass() {
    let mut t = PeepholeOptimizationsPassTest::new();
    t.set_current_peephole_passes(Vec::new);

    t.test_same("var x; var y;");
}

struct NoteApplied {
    fields: AbstractPeepholeOptimizationFields,
    visitation_log: Rc<RefCell<Vec<String>>>,
    suffix: &'static str,
}
impl AbstractPeepholeOptimization for NoteApplied {
    base_methods!("com.google.javascript.jscomp.PeepholeOptimizationsPassTest$1");

    // port: PeepholeOptimizationsPassTest$1#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_name(compiler) {
            self.visitation_log.borrow_mut().push(format!(
                "{}{}",
                node.get_string(compiler).to_string_lossy(),
                self.suffix
            ));
        }
        Some(node)
    }
}

// port: PeepholeOptimizationsPassTest#testOptimizationOrder
#[test]
fn test_optimization_order() {
    let mut t = PeepholeOptimizationsPassTest::new();
    /*
     * We need to make sure that: 1) We are only traversing the AST once 2) For
     * each node, we visit the optimizations in the client-supplied order
     *
     * To test this, we create two fake optimizations that each make an entry in
     * the visitationLog when they are passed a name node to optimize.
     *
     * Each entry is of the form nameX where 'name' is the name of the name node
     * visited and X is the identity of the optimization (1 or 2 in this case).
     * After the pass is run, we verify the correct ordering by querying the
     * log.
     *
     * Using a log, rather than, say, transforming nodes, allows us to ensure
     * not only that we are visiting each node but that our visits occur in the
     * right order (i.e. we need to make sure we're not traversing the entire
     * AST for the first optimization and then a second time for the second).
     */

    let visitation_log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let log = visitation_log.clone();
    t.set_current_peephole_passes(move || -> PassList {
        let note1_applied = NoteApplied {
            fields: AbstractPeepholeOptimizationFields::new(),
            visitation_log: log.clone(),
            suffix: "1",
        };
        let note2_applied = NoteApplied {
            fields: AbstractPeepholeOptimizationFields::new(),
            visitation_log: log.clone(),
            suffix: "2",
        };
        vec![Box::new(note1_applied), Box::new(note2_applied)]
    });

    t.test_same("var x; var y");

    /*
     * We expect the optimization order to be: "x" visited by optimization1 "x"
     * visited by optimization2 "y" visited by optimization1 "y" visited by
     * optimization2
     */
    assert_eq!(*visitation_log.borrow(), vec!["x1", "x2", "y1", "y2"]);
}

// A peephole optimization that, given a subtree consisting of a VAR node, removes children of
// that node named "x".
optimization_class!(RemoveNodesNamedXUnderVarOptimization);
impl AbstractPeepholeOptimization for RemoveNodesNamedXUnderVarOptimization {
    base_methods!(
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest$RemoveNodesNamedXUnderVarOptimization"
    );

    // port: PeepholeOptimizationsPassTest.RemoveNodesNamedXUnderVarOptimization#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_var(compiler) {
            let mut nodes_to_remove: IndexSet<NodeId> = IndexSet::<_>::default();

            let mut child = node.get_first_child(compiler);
            while let Some(c) = child {
                if c.get_string(compiler) == "x" {
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

// A peephole optimization that, given a subtree consisting of a name node named "x" removes
// that node.
optimization_class!(RemoveNodesNamedXOptimization);
impl AbstractPeepholeOptimization for RemoveNodesNamedXOptimization {
    base_methods!(
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest$RemoveNodesNamedXOptimization"
    );

    // port: PeepholeOptimizationsPassTest.RemoveNodesNamedXOptimization#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_name(compiler) && node.get_string(compiler) == "x" {
            self.report_change_to_enclosing_scope(compiler, node);
            node.detach(compiler);
            return None;
        }
        Some(node)
    }
}

// A peephole optimization that, given a subtree consisting of a name node named "x" whose
// parent is a VAR node, removes the parent VAR node.
optimization_class!(RemoveParentVarsForNodesNamedX);
impl AbstractPeepholeOptimization for RemoveParentVarsForNodesNamedX {
    base_methods!(
        "com.google.javascript.jscomp.PeepholeOptimizationsPassTest$RemoveParentVarsForNodesNamedX"
    );

    // port: PeepholeOptimizationsPassTest.RemoveParentVarsForNodesNamedX#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_name(compiler) && node.get_string(compiler) == "x" {
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

// A peephole optimization that, given a subtree consisting of a name node named "y", replaces
// it with a name node named "x";
optimization_class!(RenameYToX);
impl AbstractPeepholeOptimization for RenameYToX {
    base_methods!("com.google.javascript.jscomp.PeepholeOptimizationsPassTest$RenameYToX");

    // port: PeepholeOptimizationsPassTest.RenameYToX#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_name(compiler) && node.get_string(compiler) == "y" {
            let replacement = compiler.new_string_with_token(Token::NAME, "x");

            node.replace_with(compiler, replacement);
            self.report_change_to_enclosing_scope(compiler, replacement);

            return Some(replacement);
        }
        Some(node)
    }
}

// port: PeepholeOptimizationsPassTest#testOptimizationRemovingSubtreeChild
#[test]
fn test_optimization_removing_subtree_child() {
    let mut t = PeepholeOptimizationsPassTest::new();
    t.set_current_peephole_passes(|| -> PassList {
        vec![Box::new(RemoveNodesNamedXUnderVarOptimization::default())]
    });

    t.test("var x,y;", "var y;");
    t.test("var y,x;", "var y;");
    t.test("var x,y,x;", "var y;");
}

// port: PeepholeOptimizationsPassTest#testOptimizationRemovingSubtree
#[test]
fn test_optimization_removing_subtree() {
    let mut t = PeepholeOptimizationsPassTest::new();
    t.set_current_peephole_passes(|| -> PassList {
        vec![Box::new(RemoveNodesNamedXOptimization::default())]
    });

    t.test("var x,y;", "var y;");
    t.test("var y,x;", "var y;");
    t.test("var x,y,x;", "var y;");
}

// port: PeepholeOptimizationsPassTest#testOptimizationRemovingSubtreeParent
#[test]
fn test_optimization_removing_subtree_parent() {
    let mut t = PeepholeOptimizationsPassTest::new();
    t.set_current_peephole_passes(|| -> PassList {
        vec![Box::new(RemoveParentVarsForNodesNamedX::default())]
    });

    t.test("var x; var y", "var y");
}

/// Test the case where the first peephole optimization removes a node and the second wants to
/// remove (the now nonexistent) parent of that node.
// port: PeepholeOptimizationsPassTest#testOptimizationsRemoveParentAfterRemoveChild
#[test]
fn test_optimizations_remove_parent_after_remove_child() {
    let mut t = PeepholeOptimizationsPassTest::new();
    t.set_current_peephole_passes(|| -> PassList {
        vec![
            Box::new(RemoveNodesNamedXOptimization::default()),
            Box::new(RemoveParentVarsForNodesNamedX::default()),
        ]
    });

    t.test("var x,y; var z;", "var y; var z;");
}

// port: PeepholeOptimizationsPassTest#testOptimizationReplacingNode
#[test]
fn test_optimization_replacing_node() {
    let mut t = PeepholeOptimizationsPassTest::new();
    t.set_current_peephole_passes(|| -> PassList {
        vec![
            Box::new(RenameYToX::default()),
            Box::new(RemoveParentVarsForNodesNamedX::default()),
        ]
    });

    t.test("var y; var z;", "var z;");
}

optimization_class!(AddLetAndClasses);
impl AbstractPeepholeOptimization for AddLetAndClasses {
    base_methods!("com.google.javascript.jscomp.PeepholeOptimizationsPassTest$3");

    // port: PeepholeOptimizationsPassTest$3#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_add(compiler) {
            self.add_feature_to_enclosing_script(Feature::LET_DECLARATIONS);
            self.add_feature_to_enclosing_script(Feature::LET_DECLARATIONS);
            self.add_feature_to_enclosing_script(Feature::CLASSES);
        }
        Some(node)
    }
}

optimization_class!(AddConst);
impl AbstractPeepholeOptimization for AddConst {
    base_methods!("com.google.javascript.jscomp.PeepholeOptimizationsPassTest$4");

    // port: PeepholeOptimizationsPassTest$4#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        if node.is_sub(compiler) {
            self.add_feature_to_enclosing_script(Feature::CONST_DECLARATIONS);
        }
        Some(node)
    }
}

// port: PeepholeOptimizationsPassTest#testAddFeatureToEnclosingScript
#[test]
fn test_add_feature_to_enclosing_script() {
    let mut t = PeepholeOptimizationsPassTest::new();
    t.set_current_peephole_passes(|| -> PassList {
        vec![
            Box::new(AddLetAndClasses::default()),
            Box::new(AddConst::default()),
        ]
    });

    t.test_same("(3 + 4); function sub() { return 3 - 4; }");
    let compiler = t.harness.get_last_compiler().unwrap();
    let compiler = compiler.borrow();
    let script = compiler.get_script_node("testcode").unwrap();
    let feature_set = script
        .get_prop(&compiler, Prop::FEATURE_SET)
        .map(|prop| match prop {
            ObjectProp::Opaque(prop) => *prop
                .as_ref()
                .as_any()
                .downcast_ref::<FeatureSet>()
                .expect("ClassCastException"),
            _ => panic!("ClassCastException"),
        });
    assert_eq!(
        feature_set,
        Some(FeatureSet::BARE_MINIMUM.with_features(&[
            Feature::LET_DECLARATIONS,
            Feature::CLASSES,
            Feature::CONST_DECLARATIONS,
        ]))
    );
}
