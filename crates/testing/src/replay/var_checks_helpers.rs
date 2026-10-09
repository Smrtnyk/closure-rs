/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2005 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/ConstCheck.java,
//   src/com/google/javascript/jscomp/DeclaredGlobalExternsOnWindow.java,
//   src/com/google/javascript/jscomp/InferConsts.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/RemoveUnnecessarySyntheticExterns.java,
//   src/com/google/javascript/jscomp/StrictModeCheck.java,
//   src/com/google/javascript/jscomp/VariableReferenceCheck.java,
//   test/com/google/javascript/jscomp/RemoveUnnecessarySyntheticExternsTest.java,
//   test/com/google/javascript/jscomp/ValidityCheckTest.java,
//   test/com/google/javascript/jscomp/VarCheckTest.java.

//! Replay adapters for the var-checks passes (VarCheck, VariableReferenceCheck, ConstCheck,
//! ValidityCheck, StrictModeCheck, DeclaredGlobalExternsOnWindow) and ports of the unit-corpus
//! helpers `VarCheckTest_Helpers.java` and `ValidityCheckTest_Helpers.java`
//! (oracle/replay/helpers) that their descriptors call.
use crate::{
    replay::replay_dsl::{Ctx, DslValue, Object},
    testing::scope_subject::assert_scope,
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::{AbstractCompiler, LifeCycleStage},
    check_level::CheckLevel,
    compiler_pass::CompilerPass,
    const_check::ConstCheck,
    declared_global_externs_on_window::DeclaredGlobalExternsOnWindow,
    infer_consts::InferConsts,
    node_traversal::{Callback, NodeTraversal},
    remove_unnecessary_synthetic_externs::RemoveUnnecessarySyntheticExterns,
    strict_mode_check::StrictModeCheck,
    validity_check::ValidityCheck,
    var_check::VarCheck,
    variable_reference_check::VariableReferenceCheck,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    ir::IR,
    node::{NodeId, Prop},
    token::Token,
};
use std::{cell::RefCell, rc::Rc};

fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!(
        "{what}: arguments do not match the resolved Java signature"
    ))
}

fn pass(pass: impl CompilerPass + 'static) -> DslValue {
    let pass: Box<dyn CompilerPass> = Box::new(pass);
    DslValue::Pass(Rc::new(RefCell::new(pass)))
}

/// A pass whose runtime class (`Object#getClass`, used by `ReplayDsl#pick` for a `call` on it) is
/// the concrete Java class rather than the `CompilerPass` interface.
fn typed_pass(class: &str, p: impl CompilerPass + 'static) -> DslValue {
    DslValue::Typed {
        class: class.into(),
        value: Box::new(pass(p)),
    }
}

fn compiler_arg(
    args: &[DslValue],
    what: &str,
) -> Result<crate::replay::replay_dsl::CompilerHandle, Throwable> {
    match args.first() {
        Some(DslValue::Compiler(c)) => Ok(c.clone()),
        _ => Err(bad(what)),
    }
}

// port: VariableReferenceCheck#VariableReferenceCheck
pub fn variable_reference_check(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler_arg(&args, "VariableReferenceCheck#<init>")?;
    let check = VariableReferenceCheck::new(&c.borrow());
    Ok(pass(check))
}

// port: StrictModeCheck#StrictModeCheck
pub fn strict_mode_check(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler_arg(&args, "StrictModeCheck#<init>")?;
    let [_, DslValue::Enum { name, .. }] = args.as_slice() else {
        return Err(bad("StrictModeCheck#<init>"));
    };
    let default_level = match name.as_str() {
        "ERROR" => CheckLevel::ERROR,
        "WARNING" => CheckLevel::WARNING,
        "OFF" => CheckLevel::OFF,
        _ => return Err(bad("StrictModeCheck#<init> CheckLevel")),
    };
    let check = StrictModeCheck::new(&c.borrow(), default_level);
    Ok(pass(check))
}

// port: DeclaredGlobalExternsOnWindow#DeclaredGlobalExternsOnWindow
pub fn declared_global_externs_on_window(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler_arg(&args, "DeclaredGlobalExternsOnWindow#<init>")?;
    let check = DeclaredGlobalExternsOnWindow::new(&c.borrow());
    Ok(pass(check))
}

// port: ConstCheck#ConstCheck (constructed inside ConstCheckTest's processor lambda, while the
// compiler is lent to the running pass)
pub fn const_check_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut AbstractCompiler,
) -> Result<DslValue, Throwable> {
    Ok(typed_pass(
        "com.google.javascript.jscomp.ConstCheck",
        ConstCheck::new(compiler),
    ))
}

// port: InferConsts#InferConsts (constructed inside ConstCheckTest's processor lambda)
pub fn infer_consts_with_compiler(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut AbstractCompiler,
) -> Result<DslValue, Throwable> {
    Ok(typed_pass(
        "com.google.javascript.jscomp.InferConsts",
        InferConsts::new(compiler),
    ))
}

// port: CompilerPass#process (ConstCheck#process / InferConsts#process called from a processor
// lambda on the lent compiler)
pub fn process_with_compiler(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut AbstractCompiler,
) -> Result<DslValue, Throwable> {
    let [receiver, DslValue::Node(externs), DslValue::Node(root)] = args.as_slice() else {
        return Err(bad("CompilerPass#process"));
    };
    let DslValue::Pass(p) = receiver.untyped() else {
        return Err(bad("CompilerPass#process"));
    };
    p.borrow_mut().process(compiler, *externs, *root);
    Ok(DslValue::Null)
}

const VAR_CHECK_HOLDER: &str = "com.google.javascript.jscomp.VarCheckTest_Helpers";

// port: VarCheckTest_Helpers#VarCheckTest_Helpers
pub fn var_check_test_helpers(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    // private boolean validityCheck = false;
    fields.insert("validityCheck".to_string(), DslValue::Bool(false));
    field_types.insert("validityCheck".to_string(), "boolean".to_string());
    // private boolean declarationCheck;
    fields.insert("declarationCheck".to_string(), DslValue::Bool(false));
    field_types.insert("declarationCheck".to_string(), "boolean".to_string());
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: VAR_CHECK_HOLDER.into(),
        fields,
        field_types,
    }))))
}

/// VarCheckTest_Helpers.GetProcessorPass: the anonymous CompilerPass of VarCheckTest#getProcessor;
/// it reads the outer holder's fields when it runs, as the anonymous class reads the test's.
struct GetProcessorPass {
    outer: Rc<RefCell<Object>>,
}

// port: VarCheckTest_Helpers.GetProcessorPass#GetProcessorPass
pub fn var_check_get_processor_pass(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad("VarCheckTest_Helpers$GetProcessorPass#<init>"));
    };
    Ok(pass(GetProcessorPass {
        outer: outer.clone(),
    }))
}

impl GetProcessorPass {
    // VarCheckTest_Helpers (boolean field read)
    fn field(&self, name: &str) -> bool {
        matches!(
            self.outer.borrow().fields.get(name),
            Some(DslValue::Bool(true))
        )
    }
}

impl CompilerPass for GetProcessorPass {
    // port: VarCheckTest_Helpers.GetProcessorPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let validity_check = self.field("validityCheck");
        VarCheck::new_with_validity_check(compiler, validity_check)
            .process(compiler, externs, root);
        if !validity_check && !compiler.has_errors() {
            // If the original test turned off sanity check, make sure our synthesized
            // code passes it.
            VarCheck::new_with_validity_check(compiler, true).process(compiler, externs, root);
        }
        if self.field("declarationCheck") {
            VariableTestCheck.process(compiler, externs, root);
        }
    }
}

pub struct VariableTestCheck;

impl CompilerPass for VariableTestCheck {
    // port: VarCheckTest_Helpers.VariableTestCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse_roots(compiler, &mut VariableTestCheckCallback, externs, root);
    }
}

struct VariableTestCheckCallback;

impl Callback for VariableTestCheckCallback {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: VarCheckTest_Helpers.VariableTestCheck#process (anonymous AbstractPostOrderCallback#visit)
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let parent = parent.unwrap();
        if n.is_name(t) && !parent.is_function(t) && !parent.is_label(t) {
            let scope = t.get_scope();
            let name = n.get_string(t);
            assert_scope(t.get_compiler(), scope).declares(&name.to_string());
        }
    }
}

const VALIDITY_CHECK_HOLDER: &str = "com.google.javascript.jscomp.ValidityCheckTest_Helpers";

// port: ValidityCheckTest_Helpers#ValidityCheckTest_Helpers
pub fn validity_check_test_helpers(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: VALIDITY_CHECK_HOLDER.into(),
        fields: IndexMap::<_, _>::default(),
        field_types: IndexMap::<_, _>::default(),
    }))))
}

/// The anonymous classes ValidityCheckTest assigns to its `otherPass` field.
#[derive(Clone, Copy)]
enum OtherPass {
    OtherPass2,
    OtherPass3,
    OtherPass4,
}

impl OtherPass {
    fn process(self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        match self {
            // port: ValidityCheckTest_Helpers.OtherPass2#process
            OtherPass::OtherPass2 => {
                let script = root.get_first_child(compiler).unwrap();
                let true_node = compiler.new_node(Token::TRUE);
                let empty = compiler.new_node(Token::EMPTY);
                let if_node = compiler.new_node_with_children2(Token::IF, true_node, empty);
                if_node.srcref_tree(compiler, script);
                root.get_first_child(compiler)
                    .unwrap()
                    .add_child_to_back(compiler, if_node);
                compiler.report_change_to_enclosing_scope(script);
            }
            // port: ValidityCheckTest_Helpers.OtherPass3#process
            OtherPass::OtherPass3 => {
                compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
            }
            // port: ValidityCheckTest_Helpers.OtherPass4#process
            OtherPass::OtherPass4 => {
                let script = root.get_first_child(compiler).unwrap();
                let name = compiler.new_string_with_token(Token::NAME, "x");
                name.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
                let expr_result = compiler.new_node_with_child(Token::EXPR_RESULT, name);
                let expr_result = expr_result.srcref_tree(compiler, script);
                script.add_child_to_back(compiler, expr_result);
                compiler.report_change_to_enclosing_scope(script);
                compiler.set_life_cycle_stage(LifeCycleStage::NORMALIZED);
            }
        }
    }
}

/// ValidityCheckTest_Helpers.GetProcessor: selects the copy of the test's `otherPass` by its
/// recorded class name, then delegates to getProcessor's anonymous pass.
struct GetProcessor {
    other_pass: OtherPass,
}

// port: ValidityCheckTest_Helpers.GetProcessor#GetProcessor
pub fn validity_check_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(_), DslValue::Compiler(_), other_pass_class] = args.as_slice() else {
        return Err(bad("ValidityCheckTest_Helpers$GetProcessor#<init>"));
    };
    let DslValue::String(other_pass_class) = other_pass_class.untyped() else {
        return Err(bad(
            "ValidityCheckTest_Helpers$GetProcessor#<init> otherPassClass",
        ));
    };
    let other_pass = match other_pass_class.to_string_lossy().as_str() {
        "com.google.javascript.jscomp.ValidityCheckTest$2" => OtherPass::OtherPass2,
        "com.google.javascript.jscomp.ValidityCheckTest$3" => OtherPass::OtherPass3,
        "com.google.javascript.jscomp.ValidityCheckTest$4" => OtherPass::OtherPass4,
        other => panic!("unknown otherPass class {other}"),
    };
    Ok(pass(GetProcessor { other_pass }))
}

impl CompilerPass for GetProcessor {
    // port: ValidityCheckTest_Helpers.GetProcessor#process (delegate: getProcessor's anonymous
    // CompilerPass#process)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.other_pass.process(compiler, externs, root);
        ValidityCheck::new(compiler).process(compiler, externs, root);
    }
}

const SYNTHETIC_EXTERNS_HOLDER: &str =
    "com.google.javascript.jscomp.RemoveUnnecessarySyntheticExternsTest_Helpers";

// port: RemoveUnnecessarySyntheticExternsTest_Helpers#RemoveUnnecessarySyntheticExternsTest_Helpers
pub fn remove_unnecessary_synthetic_externs_test_helpers(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    // use this set to simulate an earlier compiler pass declaring a synthetic extern
    // private @Nullable LinkedHashSet<Node> syntheticExternsToAdd = null;
    fields.insert("syntheticExternsToAdd".to_string(), DslValue::Null);
    field_types.insert(
        "syntheticExternsToAdd".to_string(),
        "java.util.LinkedHashSet".to_string(),
    );
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: SYNTHETIC_EXTERNS_HOLDER.into(),
        fields,
        field_types,
    }))))
}

fn synthetic_externs_holder(
    args: &[DslValue],
    what: &str,
) -> Result<Rc<RefCell<Object>>, Throwable> {
    match args {
        [DslValue::Object(o)] if o.borrow().class == SYNTHETIC_EXTERNS_HOLDER => Ok(o.clone()),
        _ => Err(bad(what)),
    }
}

/// `this.syntheticExternsToAdd.add(node)` (a LinkedHashSet of distinct Node instances).
fn add_synthetic_extern(holder: &Rc<RefCell<Object>>, node: NodeId) -> Result<(), Throwable> {
    let mut holder = holder.borrow_mut();
    let Some(DslValue::Set(set)) = holder.fields.get_mut("syntheticExternsToAdd") else {
        return Err(Throwable::Exception {
            class: "java.lang.NullPointerException".into(),
            message: None,
        });
    };
    if !set
        .iter()
        .any(|v| matches!(v, DslValue::Node(existing) if *existing == node))
    {
        set.push(DslValue::Node(node));
    }
    Ok(())
}

// port: RemoveUnnecessarySyntheticExternsTest_Helpers#createUnfulfilledDeclaration
fn create_unfulfilled_declaration(ctx: &Ctx, name: &str) -> Result<NodeId, Throwable> {
    // only VAR nodes are allowed to be marked "synthesized unfulfilled" so no need to test other
    // kinds of declarations.
    let compiler = ctx
        .compiler
        .as_ref()
        .ok_or_else(|| bad("createUnfulfilledDeclaration"))?;
    let ast = &mut compiler.borrow_mut().ast;
    let name = IR::name(ast, name);
    let declaration = IR::var(ast, name);
    declaration.set_is_synthesized_unfulfilled_name_declaration(ast, true);
    Ok(declaration)
}

/// `IR.var(IR.name(name))`, a declaration not marked as an 'unfulfilled declaration'.
fn create_var(ctx: &Ctx, name: &str) -> Result<NodeId, Throwable> {
    let compiler = ctx.compiler.as_ref().ok_or_else(|| bad("IR.var"))?;
    let ast = &mut compiler.borrow_mut().ast;
    let name = IR::name(ast, name);
    Ok(IR::var(ast, name))
}

// port: RemoveUnnecessarySyntheticExternsTest_Helpers#setUp
pub fn synthetic_externs_set_up(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let holder =
        synthetic_externs_holder(&args, "RemoveUnnecessarySyntheticExternsTest_Helpers#setUp")?;
    holder.borrow_mut().fields.insert(
        "syntheticExternsToAdd".to_string(),
        DslValue::Set(Vec::new()),
    );
    Ok(DslValue::Object(holder))
}

/// A test-method prologue of RemoveUnnecessarySyntheticExternsTest_Helpers: the
/// `syntheticExternsToAdd.add(...)` statements, in order; `true` adds
/// `createUnfulfilledDeclaration(name)`, `false` adds `IR.var(IR.name(name))`.
fn synthetic_externs_prologue(
    ctx: &mut Ctx,
    args: Vec<DslValue>,
    what: &str,
    adds: &[(bool, &str)],
) -> Result<DslValue, Throwable> {
    let holder = synthetic_externs_holder(&args, what)?;
    for (unfulfilled, name) in adds {
        let node = if *unfulfilled {
            create_unfulfilled_declaration(ctx, name)?
        } else {
            create_var(ctx, name)?
        };
        add_synthetic_extern(&holder, node)?;
    }
    Ok(DslValue::Object(holder))
}

macro_rules! synthetic_externs_prologues {
    ($($(#[$m:meta])* $fn_name:ident => $java:literal, [$(($u:expr, $n:literal)),*];)*) => {
        $(
            $(#[$m])*
            pub fn $fn_name(ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
                synthetic_externs_prologue(ctx, args, $java, &[$(($u, $n)),*])
            }
        )*
        /// (Java method name, adapter) of every prologue, for the registry.
        pub const SYNTHETIC_EXTERNS_PROLOGUES: &[(&str, fn(&mut Ctx, Vec<DslValue>) -> Result<DslValue, Throwable>)] =
            &[$(($java, $fn_name)),*];
    };
}

synthetic_externs_prologues! {
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#doesntChangeSyntheticExternThatIsNotDeclaredInCode
    doesnt_change_synthetic_extern_that_is_not_declared_in_code
        => "doesntChangeSyntheticExternThatIsNotDeclaredInCode", [(true, "x"), (true, "y")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#doesntRemoveSyntheticExternIfShadowedInCode
    doesnt_remove_synthetic_extern_if_shadowed_in_code
        => "doesntRemoveSyntheticExternIfShadowedInCode", [(true, "x")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#removesSyntheticExternDeclaredInCode
    removes_synthetic_extern_declared_in_code
        => "removesSyntheticExternDeclaredInCode", [(true, "x"), (true, "y")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#removesSyntheticExternDeclaredInGoogProvide
    removes_synthetic_extern_declared_in_goog_provide
        => "removesSyntheticExternDeclaredInGoogProvide", [(true, "x"), (true, "y")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#doesNotRemovesSyntheticExternForLegacyGoogModule
    does_not_removes_synthetic_extern_for_legacy_goog_module
        => "doesNotRemovesSyntheticExternForLegacyGoogModule", [(true, "x"), (true, "y")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#removesSyntheticExternDeclaredInLegacyGoogModuleNamespace
    removes_synthetic_extern_declared_in_legacy_goog_module_namespace
        => "removesSyntheticExternDeclaredInLegacyGoogModuleNamespace", [(true, "x"), (true, "y")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#removesSyntheticExternDeclaredInOtherExterns
    removes_synthetic_extern_declared_in_other_externs
        => "removesSyntheticExternDeclaredInOtherExterns", [(true, "x"), (true, "y")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#removesDistinctSyntheticExternsDeclaredInCode
    removes_distinct_synthetic_externs_declared_in_code
        => "removesDistinctSyntheticExternsDeclaredInCode", [(true, "x"), (true, "y")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#removesDuplicateUnfulfilledSyntheticDeclarations
    removes_duplicate_unfulfilled_synthetic_declarations
        => "removesDuplicateUnfulfilledSyntheticDeclarations",
        [(true, "x"), (true, "y"), (true, "x"), (true, "x")];
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#removesSyntheticExternDeclaredInCode_multipleCodeDeclarations
    removes_synthetic_extern_declared_in_code_multiple_code_declarations
        => "removesSyntheticExternDeclaredInCode_multipleCodeDeclarations", [(true, "x"), (true, "y")];
    // add an extern that is not marked as an 'unfulfilled declaration'
    // we assume this extern was added to prevent renaming, not just enforce that all referenced
    // names are declared.
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#doesntChangeSyntheticExtern_declaredInCodeNotMarkedUnfulfilled
    doesnt_change_synthetic_extern_declared_in_code_not_marked_unfulfilled
        => "doesntChangeSyntheticExtern_declaredInCodeNotMarkedUnfulfilled", [(false, "x")];
    // add an extern that is not marked as an 'unfulfilled declaration'
    // we assume this extern was added to actually prevent renaming, and so must not be removed
    // even if a duplicate of a non-synthetic extern.
    // add a duplicate declaration of 'x', where the second and third only are unfulfilled.
    // port: RemoveUnnecessarySyntheticExternsTest_Helpers#removesOnlyUnfulfilledSyntheticExterns_ifMixOfFulfilledAndUnfulfilled
    removes_only_unfulfilled_synthetic_externs_if_mix_of_fulfilled_and_unfulfilled
        => "removesOnlyUnfulfilledSyntheticExterns_ifMixOfFulfilledAndUnfulfilled",
        [(false, "x"), (true, "x"), (true, "x")];
}

// port: RemoveUnnecessarySyntheticExterns#RemoveUnnecessarySyntheticExterns
pub fn remove_unnecessary_synthetic_externs(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
    compiler: &mut AbstractCompiler,
) -> Result<DslValue, Throwable> {
    Ok(typed_pass(
        "com.google.javascript.jscomp.RemoveUnnecessarySyntheticExterns",
        RemoveUnnecessarySyntheticExterns::new(compiler),
    ))
}
