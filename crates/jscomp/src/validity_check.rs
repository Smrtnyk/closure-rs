/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/ValidityCheck.java.

//! Port of `ValidityCheck.java`. port/normalize ported the nested `VerifyConstants` pass ahead of
//! the rest of the class (the unit-test harness runs it after every normalizeEnabled replay);
//! port/var-checks completed the module around it.
use crate::{
    abstract_compiler::AbstractCompiler,
    ast_validator::AstValidator,
    code_change_handler::CodeChangeHandler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    forbidden_change::ForbiddenChange,
    gather_extern_properties::{GatherExternProperties, Mode},
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    normalize::Normalize,
    var_check::VarCheck,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, check_state,
    js_string::JsString,
    node::{NodeId, Prop},
};
use std::sync::{Arc, Mutex};

// port: ValidityCheck#EXTERN_PROPERTIES_CHANGED
pub static EXTERN_PROPERTIES_CHANGED: DiagnosticType = DiagnosticType::error(
    "JSC_EXTERN_PROPERTIES_CHANGED",
    "Internal compiler error. Extern properties modified from:\n{0}\nto:\n{1}",
);

/// A compiler pass that verifies the structure of the AST conforms to a number of invariants.
/// Because this can add a lot of overhead, we only run this in development mode.
pub struct ValidityCheck {
    ast_validator: AstValidator<'static>,
}

impl ValidityCheck {
    // port: ValidityCheck#ValidityCheck
    pub fn new(compiler: &AbstractCompiler) -> Self {
        // Java also keeps compiler.getChangeTracker() in a field; the Rust pass reads it from the
        // compiler where it is used (DESIGN 6: a pass never stores the compiler).
        Self {
            ast_validator: AstValidator::new(compiler),
        }
    }

    /// Check that the AST is structurally accurate.
    // port: ValidityCheck#checkAst
    fn check_ast(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.ast_validator.validate_code_root(compiler, externs);
        self.ast_validator.validate_code_root(compiler, root);
    }

    // port: ValidityCheck#checkVars
    fn check_vars(compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        if compiler.get_life_cycle_stage().is_normalized() {
            // TODO(rishipal): Why is VarCheck only run when AST is normalized?
            VarCheck::new_with_validity_check(compiler, true).process(compiler, externs, root);
        }
    }

    /// Verifies that the normalization pass does nothing on an already-normalized tree.
    // port: ValidityCheck#checkNormalization
    fn check_normalization(compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Verify nothing has inappropriately denormalize the AST.
        let handler: Arc<Mutex<dyn CodeChangeHandler>> = Arc::new(Mutex::new(ForbiddenChange));
        compiler
            .get_change_tracker()
            .add_change_handler(handler.clone());

        // TODO(johnlenz): Change these normalization checks Preconditions and
        // Exceptions into Errors so that it is easier to find the root cause
        // when there are cascading issues.
        if compiler.get_life_cycle_stage().is_normalized() {
            Normalize::builder(compiler)
                .assert_on_change(true)
                .build()
                .process(compiler, externs, root);

            if compiler.get_life_cycle_stage().is_normalized_unobfuscated() {
                let check_user_declarations = true;
                let mut pass = VerifyConstants::new(compiler, check_user_declarations);
                pass.process(compiler, externs, root);
            }
        }

        compiler
            .get_change_tracker()
            .remove_change_handler(&handler);
    }

    // port: ValidityCheck#checkExternProperties
    fn check_extern_properties(compiler: &mut AbstractCompiler, externs: NodeId) {
        let Some(extern_properties) = compiler.get_extern_properties().cloned() else {
            // GatherExternProperties hasn't run yet. Don't report a violation.
            return;
        };
        // Java passes a null root; the Rust GatherExternProperties#process does not read it.
        GatherExternProperties::new(compiler, Mode::CHECK).process(compiler, externs, externs);
        let current = compiler
            .get_extern_properties()
            .cloned()
            .unwrap_or_default();
        if current != extern_properties {
            let error = JSError::make_without_location(
                &EXTERN_PROPERTIES_CHANGED,
                &[
                    &immutable_set_to_string(&extern_properties),
                    &immutable_set_to_string(&current),
                ],
            );
            compiler.report(error);
            // Throw an exception, so that the infrastructure will tell us which pass violated the
            // check.
            panic!(
                "Validity Check failed: Extern properties changed from:\n{}\nto:\n{}",
                immutable_set_to_string(&extern_properties),
                immutable_set_to_string(&current)
            );
        }
    }
}

/// Java's `AbstractCollection#toString` of an `ImmutableSet<String>`: `[a, b]`.
fn immutable_set_to_string(set: &IndexSet<String>) -> String {
    let elements: Vec<&str> = set.iter().map(String::as_str).collect();
    format!("[{}]", elements.join(", "))
}

impl CompilerPass for ValidityCheck {
    // port: ValidityCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.check_ast(compiler, externs, root);
        Self::check_normalization(compiler, externs, root);
        Self::check_vars(compiler, externs, root);
        Self::check_extern_properties(compiler, externs);
    }
}

/// Walk the AST tree and verify that constant names are used consistently.
pub struct VerifyConstants {
    check_user_declarations: bool,
    constant_map: IndexMap<JsString, bool>,
}

impl VerifyConstants {
    // port: ValidityCheck.VerifyConstants#VerifyConstants
    pub fn new(_compiler: &AbstractCompiler, check_user_declarations: bool) -> Self {
        Self {
            check_user_declarations,
            constant_map: IndexMap::<_, _>::default(),
        }
    }
}

impl CompilerPass for VerifyConstants {
    // port: ValidityCheck.VerifyConstants#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let externs_and_js = root.get_parent(compiler);
        check_state!(externs_and_js.is_some());
        let externs_and_js = check_not_null!(externs_and_js);
        check_state!(externs_and_js.has_child(compiler, externs));
        NodeTraversal::traverse_roots(compiler, self, externs, root);
    }
}

impl Callback for VerifyConstants {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ValidityCheck.VerifyConstants#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_name(t) {
            let name = n.get_string(t);
            if n.get_string_ref(t).is_empty() {
                return;
            }

            let is_const = n.get_boolean_prop(t, Prop::IS_CONSTANT_NAME);
            if self.check_user_declarations {
                let expected_const;
                let compiler: &AbstractCompiler = t.get_compiler();
                let convention = compiler.get_coding_convention();
                if NodeUtil::is_constant_name(compiler, n)
                    || NodeUtil::is_constant_by_convention(compiler, convention, n)
                {
                    expected_const = true;
                } else {
                    let mut info = None;
                    let scope = t.get_scope();
                    let var_name = n.get_string(t);
                    let var = scope.get_var(t.get_compiler(), var_name);
                    if let Some(var) = var {
                        info = var.get_jsdoc_info(t.get_compiler());
                    }

                    expected_const = info.is_some_and(|info| info.is_constant());
                }

                if expected_const {
                    check_state!(
                        expected_const == is_const,
                        "The name %s is not annotated as constant.",
                        name
                    );
                } else {
                    check_state!(
                        expected_const == is_const,
                        "The name %s should not be annotated as constant.",
                        name
                    );
                }
            }

            let value = self.constant_map.get(&name).copied();
            match value {
                None => {
                    self.constant_map.insert(name, is_const);
                }
                Some(value) if value != is_const => {
                    panic!(
                        "The name {} is not consistently annotated as constant. Current constness: {}, previous constness: {}\n\nAt node: {}",
                        name,
                        is_const,
                        value,
                        n.to_string(t)
                    );
                }
                Some(_) => {}
            }
        }
    }
}
