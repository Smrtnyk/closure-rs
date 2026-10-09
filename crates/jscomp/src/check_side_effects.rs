/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckSideEffects.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    j2cl_source_utils::J2clSourceUtils,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    check_state,
    ir::IR,
    js_string::JsString,
    node::{NodeId, Prop},
};

// port: CheckSideEffects#USELESS_CODE_ERROR
pub static USELESS_CODE_ERROR: DiagnosticType =
    DiagnosticType::warning("JSC_USELESS_CODE", "Suspicious code. {0}");

/// Checks for non side effecting statements such as
///
/// ```text
/// var s = "this string is "
///         "continued on the next line but you forgot the +";
/// x == foo();  // should that be '='?
/// foo();;  // probably just a stray-semicolon. Doesn't hurt to check though
/// ```
///
/// and generates warnings.
pub struct CheckSideEffects {
    report: bool,
    problem_nodes: Vec<NodeId>,
    // Java Set<String> may hold the null returned by NodeUtil.getName.
    no_side_effect_externs: IndexSet<Option<JsString>>,
    protect_side_effect_free_code: bool,
    /// Whether the synthetic extern for JSCOMPILER_PRESERVE has been injected
    preserve_function_injected: bool,
}

impl CheckSideEffects {
    // Protect function is used to protect the underlying node from removal. At the very end, this
    // protector function will be completely removed from production but the underlying node will be
    // preserved.
    // port: CheckSideEffects#PROTECTOR_FN
    pub const PROTECTOR_FN: &'static str = "JSCOMPILER_PRESERVE";

    // J2CL protector function is used to protect the underlying node from removal until the very end
    // so that referred prototype properties won't be removed by the compiler. At the very end, this
    // protector function and its underyling node will be completely removed from production.
    // port: CheckSideEffects#J2CL_PROTECTOR_FN
    pub const J2CL_PROTECTOR_FN: &'static str = "$J2CL_PRESERVE$";

    // port: CheckSideEffects#CheckSideEffects
    pub fn new(report: bool, protect_side_effect_free_code: bool) -> Self {
        Self {
            report,
            problem_nodes: Vec::new(),
            no_side_effect_externs: IndexSet::<_>::default(),
            protect_side_effect_free_code,
            preserve_function_injected: false,
        }
    }

    /// Protect side-effect free nodes by making them parameters to a extern function call. This call
    /// will be removed after all the optimizations passes have run.
    // port: CheckSideEffects#protectSideEffects
    fn protect_side_effects(&mut self, compiler: &mut AbstractCompiler) {
        if !self.problem_nodes.is_empty() {
            if !self.preserve_function_injected {
                Self::add_extern(compiler);
            }
            for &n in &self.problem_nodes {
                let name = IR::name(compiler, Self::PROTECTOR_FN).srcref(compiler, n);
                name.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
                let replacement = IR::call(compiler, name, &[]).srcref(compiler, n);
                replacement.put_boolean_prop(compiler, Prop::FREE_CALL, true);
                n.replace_with(compiler, replacement);
                replacement.add_child_to_back(compiler, n);
                compiler.report_change_to_enclosing_scope(replacement);
            }
        }
    }

    /// Injects JSCOMPILER_PRESEVE into the synthetic externs
    // port: CheckSideEffects#addExtern
    pub fn add_extern(compiler: &mut AbstractCompiler) {
        NodeUtil::create_synthesized_externs_symbol(compiler, Self::PROTECTOR_FN);
    }
}

impl CompilerPass for CheckSideEffects {
    // port: CheckSideEffects#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, externs, &mut GetNoSideEffectExterns(self));

        NodeTraversal::traverse(compiler, root, self);

        // Code with hidden side-effect code is common, for example
        // accessing "el.offsetWidth" forces a reflow in browsers, to allow this
        // will still allowing local dead code removal in general,
        // protect the "side-effect free" code in the source.
        //
        // This also includes function calls such as with document.createElement
        if self.protect_side_effect_free_code {
            self.protect_side_effects(compiler);
        }
    }
}

impl Callback for CheckSideEffects {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckSideEffects#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        // VOID nodes appear when there are extra semicolons at the BLOCK level.
        // I've been unable to think of any cases where this indicates a bug,
        // and apparently some people like keeping these semicolons around,
        // so we'll allow it.
        if n.is_empty(t) || n.is_comma(t) {
            return;
        }

        if parent.is_none() {
            return;
        }

        // Do not try to remove a block or an expr result. We already handle
        // these cases when we visit the child, and the peephole passes will
        // fix up the tree in more clever ways when these are removed.
        if n.is_expr_result(t) || n.is_block(t) {
            return;
        }

        // This no-op statement was there so that JSDoc information could
        // be attached to the name. This check should not complain about it.
        if n.is_qualified_name(t) && n.get_jsdoc_info_ref(t).is_some() {
            return;
        }

        // These are used by the compiler's built-in runtime libraries for dependency management
        // they have a special meaning used in Compiler.ensureLibraryInjected
        if n.is_string_lit(t)
            && n.get_parent(t).unwrap().is_expr_result(t)
            && n.get_grandparent(t).unwrap().is_script(t)
            && n.get_source_file_name(t)
                .unwrap()
                .starts_with(AbstractCompiler::RUNTIME_LIB_DIR)
            && n.get_string_ref(t).starts_with("require ")
        {
            return;
        }

        let is_result_used = NodeUtil::is_expression_result_used(t, n);
        let is_simple_op =
            NodeUtil::is_simple_operator(t, n) || n.is_get_prop(t) || n.is_get_elem(t);
        if !is_result_used && !n.is_void(t) {
            if is_simple_op || {
                let analyzer = t.get_compiler().get_ast_analyzer();
                !analyzer.may_have_side_effects(t.get_compiler(), n)
            } {
                if self.report {
                    let mut msg = String::from("This code lacks side-effects. Is there a bug?");
                    if n.is_string_lit(t) || n.is_template_lit(t) {
                        msg = String::from("Is there a missing '+' on the previous line?");
                    } else if is_simple_op {
                        msg = format!(
                            "The result of the '{}' operator is not being used.",
                            n.get_token(t).to_string().to_ascii_lowercase()
                        );
                    }

                    t.report(n, &USELESS_CODE_ERROR, &[&msg]);
                }
                // TODO(johnlenz): determine if it is necessary to
                // try to protect side-effect free statements as well.
                if !NodeUtil::is_statement(t, n) {
                    self.problem_nodes.push(n);
                }
            } else if n.is_call(t) && {
                let first = n.get_first_child(t).unwrap();
                first.is_get_prop(t) || first.is_name(t) || first.is_string_lit(t)
            } {
                let first = n.get_first_child(t).unwrap();
                let qname = first.get_qualified_name(t);

                // The name should not be defined in src scopes - only externs
                let mut is_defined_in_src = false;
                if let Some(qname) = &qname {
                    if first.is_get_prop(t) {
                        let root_name_node = NodeUtil::get_root_of_qualified_name(t, first);
                        is_defined_in_src = root_name_node.is_name(t) && {
                            let scope = t.get_scope();
                            let name = root_name_node.get_string(t);
                            scope.get_var(t.get_compiler(), name).is_some()
                        };
                    } else {
                        let scope = t.get_scope();
                        is_defined_in_src =
                            scope.get_var(t.get_compiler(), qname.clone()).is_some();
                    }
                }

                if let Some(qname) = qname
                    && self.no_side_effect_externs.contains(&Some(qname.clone()))
                    && !is_defined_in_src
                {
                    self.problem_nodes.push(n);
                    if self.report {
                        let msg = format!(
                            "The result of the extern function call '{qname}' is not being used."
                        );
                        t.report(n, &USELESS_CODE_ERROR, &[&msg]);
                    }
                }
            }
        }
    }
}

/// Remove side-effect sync functions.
pub struct StripProtection;

impl StripProtection {
    // port: CheckSideEffects.StripProtection#StripProtection
    pub fn new() -> Self {
        Self
    }
}

impl Default for StripProtection {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for StripProtection {
    // port: CheckSideEffects.StripProtection#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for StripProtection {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckSideEffects.StripProtection#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_call(t) {
            let target = n.get_first_child(t).unwrap();
            if !target.is_name(t) {
                return;
            }

            if target.get_string(t) == CheckSideEffects::J2CL_PROTECTOR_FN {
                // Do not let non-J2CL code abuse it.
                check_state!(
                    J2clSourceUtils::is_j2cl_source(n.get_source_file_name(t).as_deref()),
                    "Only allowed for J2CL code"
                );
                NodeUtil::delete_function_call(t.get_compiler(), n);
                return;
            }

            // TODO(johnlenz): add this to the coding convention
            // so we can remove goog.reflect.sinkValue as well.
            if target.get_string(t) == CheckSideEffects::PROTECTOR_FN {
                let expr = n.get_last_child(t).unwrap();
                n.detach_children(t);
                n.replace_with(t, expr);
                t.report_code_change();
            }
        }
    }
}

/// Get fully qualified function names which are marked with @nosideeffects
///
/// TODO(ChadKillingsworth) Add support for object literals
struct GetNoSideEffectExterns<'a>(&'a mut CheckSideEffects);

impl Callback for GetNoSideEffectExterns<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckSideEffects.GetNoSideEffectExterns#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_function(t) {
            let js_doc = NodeUtil::get_best_jsdoc_info(t, n);
            if js_doc.is_some_and(|js_doc| js_doc.is_no_side_effects()) {
                let name = NodeUtil::get_name(t, n);
                self.0.no_side_effect_externs.insert(name);
            }
        }

        if n.is_name(t) && n.get_string(t) == CheckSideEffects::PROTECTOR_FN {
            self.0.preserve_function_injected = true;
        }
    }
}
