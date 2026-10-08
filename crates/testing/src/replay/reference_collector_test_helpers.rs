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
/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/helpers/com/google/javascript/jscomp/ReferenceCollectorTest_Helpers.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AbstractVar.java,
//   src/com/google/javascript/jscomp/Reference.java,
//   src/com/google/javascript/jscomp/ReferenceCollection.java,
//   src/com/google/javascript/jscomp/ReferenceCollector.java,
//   src/com/google/javascript/jscomp/ReferenceMap.java,
//   src/com/google/javascript/jscomp/Scope.java.

//! Port of the replay helper `oracle/replay/helpers/.../ReferenceCollectorTest_Helpers.java`
//! (DSL names `ReferenceCollectorTest_Helpers.Anon<N>` and
//! `ReferenceCollectorTest_Helpers.Lambda_<testMethod>`): the ReferenceCollector.Behavior
//! callbacks ReferenceCollectorTest passes to getProcessor, plus the replay constructor of
//! `ReferenceCollector(AbstractCompiler, Behavior, ScopeCreator)` that receives them.
//!
//! The callbacks only assert. Java throws the first failed assertion out of the traversal; the
//! Rust Behavior cannot return an error, so it keeps the first failure, skips its later checks
//! and the pass reports that failure after the traversal (a failed record either way).
use crate::{
    jscomp_api::Compiler,
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_var::AbstractVar,
    compiler_pass::CompilerPass,
    node_traversal::NodeTraversal,
    reference_collection::ReferenceCollection,
    reference_collector::{Behavior, ReferenceCollector},
    reference_map::ReferenceMap,
    syntactic_scope_creator::SyntacticScopeCreator,
    var::VarId,
};
use closure_rhino::{js_string::JsString, node::NodeId, token::Token};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

const PREFIX: &str = "com.google.javascript.jscomp.ReferenceCollectorTest_Helpers$";

type Check = fn(&mut NodeTraversal<'_>, &dyn ReferenceMap) -> Result<(), Throwable>;

/// One generated `static final class ... implements Behavior`.
pub struct NativeBehavior {
    class: String,
    check: Check,
    failure: Option<Throwable>,
}

impl NativeObject for NativeBehavior {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        &self.class
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Behavior for NativeBehavior {
    // port: ReferenceCollector.Behavior#afterExitScope
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap) {
        if self.failure.is_none()
            && let Err(e) = (self.check)(t, reference_map)
        {
            self.failure = Some(e);
        }
    }
}

// port: ReplayDsl#helper (ReferenceCollectorTest_Helpers nested Behavior constructors)
pub fn entry(signature: &str) -> Option<crate::replay::registry::Entry> {
    let name = signature.strip_prefix(PREFIX)?.strip_suffix("#<init>()")?;
    BEHAVIORS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, constructor)| *constructor)
}

// port: ReferenceCollectorTest_Helpers.Anon1#Anon1 (and every other generated Behavior)
fn new_behavior(args: &[DslValue], name: &str, check: Check) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(DslValue::Native(Rc::new(RefCell::new(NativeBehavior {
        class: format!("{PREFIX}{name}"),
        check,
        failure: None,
    }))))
}

/// The `ReferenceCollector` built by getProcessor; the collector borrows its behavior and scope
/// creator, so it is created when the pass runs. The views of its reference map that
/// UnitRecorder#postCallSnapshot reads (pass.referenceMap) are taken right after `process`, while
/// the compiler is at hand; the collector does not change them afterwards.
struct NativeReferenceCollector {
    behavior: Rc<RefCell<dyn NativeObject>>,
    reference_map: Vec<(DslValue, DslValue)>,
}

impl NativeObject for NativeReferenceCollector {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.ReferenceCollector"
    }
    // port: ReplayValues#findField (native object adapter)
    // UnitRecorder#collect reaches the behavior (no fields); the collector itself is the result
    // producer the recorder reads through getAllSymbols / getReferences.
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::new();
        fields.insert("behavior".into(), DslValue::Native(self.behavior.clone()));
        Ok(fields)
    }
    // port: ReferenceCollector#getAllSymbols / ReferenceCollector#getReferences
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.as_slice()) {
            ("getAllSymbols", []) => Ok(DslValue::List(
                self.reference_map.iter().map(|(v, _)| v.clone()).collect(),
            )),
            ("getReferences", [DslValue::Native(var)]) => Ok(self
                .reference_map
                .iter()
                .find(|(v, _)| matches!(v, DslValue::Native(o) if Rc::ptr_eq(o, var)))
                .map_or(DslValue::Null, |(_, refs)| refs.clone())),
            _ => Err(Throwable::Unported(format!(
                "com.google.javascript.jscomp.ReferenceCollector#{method}"
            ))),
        }
    }
    // port: ReferenceCollector#process
    fn process(
        &mut self,
        compiler: &mut Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        let mut behavior = self.behavior.borrow_mut();
        let class = behavior.class_name().to_string();
        let behavior = behavior
            .as_any_mut()
            .downcast_mut::<NativeBehavior>()
            .ok_or(Throwable::Unported(class))?;
        let mut scope_creator = SyntacticScopeCreator::new();
        let mut collector = ReferenceCollector::new(compiler, &mut *behavior, &mut scope_creator);
        CompilerPass::process(&mut collector, compiler, externs, root);
        self.reference_map.clear();
        for var in collector.get_all_symbols() {
            let refs = collector.get_references(var).ok_or_else(npe)?;
            let references = refs
                .iter()
                .map(|r| {
                    view(ReferenceView {
                        node: r.get_node(),
                        is_declaration: r.is_declaration(compiler),
                        is_initializing_declaration: r.is_initializing_declaration(compiler),
                        is_lvalue: r.is_lvalue(compiler),
                    })
                })
                .collect();
            let refs = view(ReferenceCollectionView {
                references,
                is_assigned_once_in_lifetime: refs.is_assigned_once_in_lifetime(compiler),
                is_well_defined: refs.is_well_defined(compiler),
            });
            let var = view(VarView {
                name: AbstractVar::get_name(var, compiler),
                scope_root: AbstractVar::get_scope_root(var, compiler),
            });
            self.reference_map.push((var, refs));
        }
        drop(collector);
        behavior.failure.take().map_or(Ok(()), Err)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplayDsl#invoke (native object identity)
fn view(o: impl NativeObject) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(o)))
}

/// `com.google.javascript.jscomp.Var`, read through `getName` and `getScopeRoot`.
struct VarView {
    name: JsString,
    scope_root: NodeId,
}

impl NativeObject for VarView {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.Var"
    }
    // port: AbstractVar#getName / AbstractVar#getScopeRoot
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.len()) {
            ("getName", 0) => Ok(DslValue::String(self.name.clone())),
            ("getScopeRoot", 0) => Ok(DslValue::Node(self.scope_root)),
            _ => Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            ))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `com.google.javascript.jscomp.ReferenceCollection`.
struct ReferenceCollectionView {
    references: Vec<DslValue>,
    is_assigned_once_in_lifetime: bool,
    is_well_defined: bool,
}

impl NativeObject for ReferenceCollectionView {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.ReferenceCollection"
    }
    // port: ReferenceCollection#isAssignedOnceInLifetime / ReferenceCollection#isWellDefined
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.len()) {
            ("isAssignedOnceInLifetime", 0) => {
                Ok(DslValue::Bool(self.is_assigned_once_in_lifetime))
            }
            ("isWellDefined", 0) => Ok(DslValue::Bool(self.is_well_defined)),
            _ => Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            ))),
        }
    }
    // port: ReplayValues#findField (ReferenceCollection#references)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "references".to_string(),
            DslValue::List(self.references.clone()),
        )]))
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `com.google.javascript.jscomp.Reference`.
struct ReferenceView {
    node: NodeId,
    is_declaration: bool,
    is_initializing_declaration: bool,
    is_lvalue: bool,
}

impl NativeObject for ReferenceView {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.Reference"
    }
    // port: Reference#getNode / #isDeclaration / #isInitializingDeclaration / #isLvalue
    fn call(&mut self, method: &str, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        match (method, args.len()) {
            ("getNode", 0) => Ok(DslValue::Node(self.node)),
            ("isDeclaration", 0) => Ok(DslValue::Bool(self.is_declaration)),
            ("isInitializingDeclaration", 0) => {
                Ok(DslValue::Bool(self.is_initializing_declaration))
            }
            ("isLvalue", 0) => Ok(DslValue::Bool(self.is_lvalue)),
            _ => Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            ))),
        }
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReferenceCollector#ReferenceCollector(AbstractCompiler,Behavior,ScopeCreator)
pub fn reference_collector(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(_),
        DslValue::Native(behavior),
        DslValue::Native(_creator),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeReferenceCollector {
            behavior: behavior.clone(),
            reference_map: vec![],
        },
    ))))
}

const BEHAVIORS: [(&str, crate::replay::registry::Entry); 27] = [
    ("Anon1", |_, a| new_behavior(&a, "Anon1", anon1)),
    ("Anon2", |_, a| new_behavior(&a, "Anon2", anon2)),
    ("Anon3", |_, a| new_behavior(&a, "Anon3", anon3)),
    ("Anon4", |_, a| new_behavior(&a, "Anon4", anon4)),
    ("Anon5", |_, a| new_behavior(&a, "Anon5", anon5)),
    ("Anon6", |_, a| new_behavior(&a, "Anon6", anon6)),
    ("Anon7", |_, a| new_behavior(&a, "Anon7", anon7)),
    ("Anon8", |_, a| new_behavior(&a, "Anon8", anon8)),
    ("Anon9", |_, a| new_behavior(&a, "Anon9", anon9)),
    ("Anon10", |_, a| new_behavior(&a, "Anon10", anon10)),
    ("Anon11", |_, a| new_behavior(&a, "Anon11", anon11)),
    ("Anon12", |_, a| new_behavior(&a, "Anon12", anon12)),
    ("Anon13", |_, a| new_behavior(&a, "Anon13", anon13)),
    ("Anon14", |_, a| new_behavior(&a, "Anon14", anon14)),
    ("Anon15", |_, a| new_behavior(&a, "Anon15", anon15)),
    ("Lambda_testIterableRest_declaration", |_, a| {
        new_behavior(
            &a,
            "Lambda_testIterableRest_declaration",
            lambda_test_iterable_rest_declaration,
        )
    }),
    ("Lambda_testObjectRest_declaration", |_, a| {
        new_behavior(
            &a,
            "Lambda_testObjectRest_declaration",
            lambda_test_object_rest_declaration,
        )
    }),
    ("Lambda_testClass", |_, a| {
        new_behavior(&a, "Lambda_testClass", lambda_test_class)
    }),
    ("Lambda_testClass_withInstantiation", |_, a| {
        new_behavior(
            &a,
            "Lambda_testClass_withInstantiation",
            lambda_test_class_with_instantiation,
        )
    }),
    (
        "Lambda_testClassExpression_assignedToConst_andInstantiated",
        |_, a| {
            new_behavior(
                &a,
                "Lambda_testClassExpression_assignedToConst_andInstantiated",
                lambda_test_class_expression_assigned_to_const_and_instantiated,
            )
        },
    ),
    (
        "Lambda_testNamedClassExpression_assignedToConst_andInstantiated",
        |_, a| {
            new_behavior(
                &a,
                "Lambda_testNamedClassExpression_assignedToConst_andInstantiated",
                lambda_test_named_class_expression_assigned_to_const_and_instantiated,
            )
        },
    ),
    ("Lambda_testImport1", |_, a| {
        new_behavior(&a, "Lambda_testImport1", lambda_test_import1)
    }),
    ("Lambda_testImport2", |_, a| {
        new_behavior(&a, "Lambda_testImport2", lambda_test_import2)
    }),
    ("Lambda_testImport2_alternate", |_, a| {
        new_behavior(
            &a,
            "Lambda_testImport2_alternate",
            lambda_test_import2_alternate,
        )
    }),
    ("Lambda_testImport3", |_, a| {
        new_behavior(&a, "Lambda_testImport3", lambda_test_import3)
    }),
    ("Lambda_testImport4", |_, a| {
        new_behavior(&a, "Lambda_testImport4", lambda_test_import4)
    }),
    ("Lambda_testLetInLoopAssignedOnceInLifetime", |_, a| {
        new_behavior(
            &a,
            "Lambda_testLetInLoopAssignedOnceInLifetime",
            lambda_test_let_in_loop_assigned_once_in_lifetime,
        )
    }),
];

// port: java.lang.NullPointerException (dereferencing a null result)
fn npe() -> Throwable {
    Throwable::Exception {
        class: "java.lang.NullPointerException".into(),
        message: None,
    }
}

// port: Scope#getVar
fn get_var(t: &mut NodeTraversal<'_>, name: &str) -> Option<VarId> {
    let scope = t.get_scope();
    scope.get_var(t.get_compiler(), JsString::from(name))
}

// port: ReferenceMap#getReferences (rm.getReferences(t.getScope().getVar(name)), null-checked
// where the test dereferences it)
fn refs<'m>(
    t: &mut NodeTraversal<'_>,
    rm: &'m dyn ReferenceMap,
    name: &str,
) -> Result<&'m ReferenceCollection, Throwable> {
    let var = get_var(t, name).ok_or_else(npe)?;
    rm.get_references(var).ok_or_else(npe)
}

// port: Truth BooleanSubject#isTrue / #isFalse
fn assert_bool(actual: bool, expected: bool) -> Result<(), Throwable> {
    crate::throwable::assert_that(actual == expected, format!("expected to be {expected}"))
}

// port: ReferenceCollection#isAssignedOnceInLifetime
fn assigned_once(t: &mut NodeTraversal<'_>, x: &ReferenceCollection) -> bool {
    x.is_assigned_once_in_lifetime(t.get_compiler())
}

// port: ReferenceCollection#isWellDefined
fn well_defined(t: &NodeTraversal<'_>, x: &ReferenceCollection) -> bool {
    x.is_well_defined(t)
}

// port: assertThat(x).comparingElementsUsing(IS_DECLARATION).containsExactly(...).inOrder()
fn declarations_are(
    t: &NodeTraversal<'_>,
    x: &ReferenceCollection,
    expected: &[bool],
) -> Result<(), Throwable> {
    let actual = x.iter().map(|r| r.is_declaration(t)).collect::<Vec<_>>();
    crate::throwable::assert_that(
        actual == expected,
        format!("isDeclaration() is {actual:?}, expected {expected:?} in order"),
    )
}

// port: assertNode(x.references.get(i).getBasicBlock().getRoot()).hasType(token)
fn block_root_has_type(
    t: &NodeTraversal<'_>,
    x: &ReferenceCollection,
    i: usize,
    token: Token,
) -> Result<(), Throwable> {
    let reference = x.references.get(i).ok_or_else(|| Throwable::Exception {
        class: "java.lang.IndexOutOfBoundsException".into(),
        message: None,
    })?;
    let root = reference.get_basic_block().ok_or_else(npe)?.get_root();
    crate::throwable::assert_that(
        root.get_token(t) == token,
        format!("expected node of type {token:?}"),
    )
}

// port: IterableSubject#hasSize
fn has_size(x: &ReferenceCollection, size: usize) -> Result<(), Throwable> {
    crate::throwable::assert_that(
        x.references.len() == size,
        format!("expected to have size {size}"),
    )
}

// port: ReferenceCollectorTest_Helpers.Anon1#afterExitScope
fn anon1(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_block_scope(t.get_compiler())
        && scope
            .get_parent(t.get_compiler())
            .ok_or_else(npe)?
            .is_function_block_scope(t.get_compiler())
    {
        let y = refs(t, rm, "y")?;
        assert_bool(assigned_once(t, y), true)?;
        assert_bool(well_defined(t, y), true)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon2#afterExitScope
fn anon2(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_block_scope(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), false)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon3#afterExitScope
fn anon3(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_global(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), false)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon4#afterExitScope
fn anon4(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_function_scope(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), true)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon5#afterExitScope
fn anon5(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_function_scope(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), false)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon6#afterExitScope
fn anon6(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_function_block_scope(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), true)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon7#afterExitScope
fn anon7(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_block_scope(t.get_compiler()) && !scope.is_function_block_scope(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), true)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon8#afterExitScope
fn anon8(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    catch_e_and_y(t, rm)
}

// port: ReferenceCollectorTest_Helpers.Anon9#afterExitScope
fn anon9(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    catch_e_and_y(t, rm)
}

// port: ReferenceCollectorTest_Helpers.Anon10#afterExitScope
fn anon10(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    catch_e_and_y(t, rm)
}

// port: ReferenceCollectorTest_Helpers.Anon8#afterExitScope (body shared verbatim by Anon9, Anon10)
fn catch_e_and_y(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_catch_scope(t.get_compiler()) {
        let e = refs(t, rm, "e")?;
        assert_bool(assigned_once(t, e), true)?;
        let y = refs(t, rm, "y")?;
        assert_bool(assigned_once(t, y), true)?;
        assert_bool(well_defined(t, y), true)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon11#afterExitScope
fn anon11(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_global(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        has_size(x, 3)?;
        block_root_has_type(t, x, 0, Token::ROOT)?;
        block_root_has_type(t, x, 1, Token::ROOT)?;
        block_root_has_type(t, x, 2, Token::CASE)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon12#afterExitScope
fn anon12(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_global(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        has_size(x, 4)?;
        // first child of || is not a boundary, but the second child is.
        block_root_has_type(t, x, 0, Token::ROOT)?;
        block_root_has_type(t, x, 1, Token::ROOT)?;
        block_root_has_type(t, x, 2, Token::NAME)?;
        // second child of `y = (x = 1)` is a boundary
        block_root_has_type(t, x, 3, Token::ASSIGN)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon13#afterExitScope
fn anon13(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_global(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        has_size(x, 3)?;
        block_root_has_type(t, x, 0, Token::ROOT)?;
        block_root_has_type(t, x, 1, Token::ROOT)?;
        // first child of ?? is not a boundary, but the second child is.
        block_root_has_type(t, x, 2, Token::ASSIGN)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon14#afterExitScope
fn anon14(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_global(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        has_size(x, 3)?;
        block_root_has_type(t, x, 0, Token::ROOT)?;
        block_root_has_type(t, x, 1, Token::ROOT)?;
        // first child of `?.` is not a boundary, but the second child is.
        block_root_has_type(t, x, 2, Token::ASSIGN)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Anon15#afterExitScope
fn anon15(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_function_block_scope(t.get_compiler()) && {
        let root = t.get_scope_root().ok_or_else(npe)?;
        let parent = root.get_parent(t).ok_or_else(npe)?;
        let first = parent.get_first_child(t).ok_or_else(npe)?;
        first.matches_name(t, "m")
    } {
        let this = refs(t, rm, "self")?;
        let escaped = this.is_escaped(t.get_compiler());
        assert_bool(escaped, false)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Lambda_testIterableRest_declaration#afterExitScope
fn lambda_test_iterable_rest_declaration(
    t: &mut NodeTraversal<'_>,
    rm: &dyn ReferenceMap,
) -> Result<(), Throwable> {
    let arr_reference_collection = refs(t, rm, "arr")?;
    declarations_are(t, arr_reference_collection, &[true, false])
}

// port: ReferenceCollectorTest_Helpers.Lambda_testObjectRest_declaration#afterExitScope
fn lambda_test_object_rest_declaration(
    t: &mut NodeTraversal<'_>,
    rm: &dyn ReferenceMap,
) -> Result<(), Throwable> {
    let obj_reference_collection = refs(t, rm, "obj")?;
    declarations_are(t, obj_reference_collection, &[true, false])
}

// port: ReferenceCollectorTest_Helpers.Lambda_testClass#afterExitScope
fn lambda_test_class(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_global(t.get_compiler()) {
        let x = refs(t, rm, "Foo")?;
        assert_bool(assigned_once(t, x), true)?;
        assert_bool(well_defined(t, x), true)?;
        declarations_are(t, x, &[true])?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Lambda_testClass_withInstantiation#afterExitScope
fn lambda_test_class_with_instantiation(
    t: &mut NodeTraversal<'_>,
    rm: &dyn ReferenceMap,
) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_global(t.get_compiler()) {
        let x = refs(t, rm, "Foo")?;
        assert_bool(assigned_once(t, x), true)?;
        // TODO(lharker): we should consider this well-defined.
        assert_bool(well_defined(t, x), false)?;
        declarations_are(t, x, &[true, false])?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Lambda_testClassExpression_assignedToConst_andInstantiated#afterExitScope
fn lambda_test_class_expression_assigned_to_const_and_instantiated(
    t: &mut NodeTraversal<'_>,
    rm: &dyn ReferenceMap,
) -> Result<(), Throwable> {
    global_foo_assigned_once_and_well_defined(t, rm)
}

// port: ReferenceCollectorTest_Helpers.Lambda_testNamedClassExpression_assignedToConst_andInstantiated#afterExitScope
fn lambda_test_named_class_expression_assigned_to_const_and_instantiated(
    t: &mut NodeTraversal<'_>,
    rm: &dyn ReferenceMap,
) -> Result<(), Throwable> {
    global_foo_assigned_once_and_well_defined(t, rm)
}

// port: ReferenceCollectorTest_Helpers.Lambda_testClassExpression_assignedToConst_andInstantiated#afterExitScope
// (body shared verbatim by Lambda_testNamedClassExpression_assignedToConst_andInstantiated)
fn global_foo_assigned_once_and_well_defined(
    t: &mut NodeTraversal<'_>,
    rm: &dyn ReferenceMap,
) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_global(t.get_compiler()) {
        let x = refs(t, rm, "Foo")?;
        assert_bool(assigned_once(t, x), true)?;
        assert_bool(well_defined(t, x), true)?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Lambda_testImport1#afterExitScope
fn lambda_test_import1(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    module_x_import(t, rm)
}

// port: ReferenceCollectorTest_Helpers.Lambda_testImport2#afterExitScope
fn lambda_test_import2(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    module_x_import(t, rm)
}

// port: ReferenceCollectorTest_Helpers.Lambda_testImport2_alternate#afterExitScope
fn lambda_test_import2_alternate(
    t: &mut NodeTraversal<'_>,
    rm: &dyn ReferenceMap,
) -> Result<(), Throwable> {
    module_x_import(t, rm)
}

// port: ReferenceCollectorTest_Helpers.Lambda_testImport1#afterExitScope (body shared verbatim
// by Lambda_testImport2 and Lambda_testImport2_alternate)
fn module_x_import(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_module_scope(t.get_compiler()) {
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), true)?;
        assert_bool(well_defined(t, x), true)?;
        declarations_are(t, x, &[true])?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Lambda_testImport3#afterExitScope
fn lambda_test_import3(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_module_scope(t.get_compiler()) {
        crate::throwable::assert_that(get_var(t, "y").is_none(), "expected to be null")?;
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), true)?;
        assert_bool(well_defined(t, x), true)?;
        declarations_are(t, x, &[true])?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Lambda_testImport4#afterExitScope
fn lambda_test_import4(t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_module_scope(t.get_compiler()) {
        let var = get_var(t, "x");
        var.ok_or_else(npe)?; // checkNotNull(var)
        // checkNotNull(x)
        let x = refs(t, rm, "x")?;
        assert_bool(assigned_once(t, x), true)?;
        assert_bool(well_defined(t, x), true)?;
        declarations_are(t, x, &[true])?;
    }
    Ok(())
}

// port: ReferenceCollectorTest_Helpers.Lambda_testLetInLoopAssignedOnceInLifetime#afterExitScope
fn lambda_test_let_in_loop_assigned_once_in_lifetime(
    t: &mut NodeTraversal<'_>,
    rm: &dyn ReferenceMap,
) -> Result<(), Throwable> {
    let scope = t.get_scope();
    if scope.is_block_scope(t.get_compiler()) {
        let x_var = get_var(t, "x");
        crate::throwable::assert_that(x_var.is_some(), "expected not to be null")?;
        let x_var = x_var.unwrap();
        assert_bool(x_var.is_let(t.get_compiler()), true)?;
        let x = rm.get_references(x_var).ok_or_else(npe)?;
        assert_bool(assigned_once(t, x), true)?;
    }
    Ok(())
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
