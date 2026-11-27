/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/Es6CheckModule.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    parallel_checks::{self, CheckContext, Collector, ReadOnlyCallback},
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

// port: Es6CheckModule#ES6_MODULE_REFERENCES_THIS
pub static ES6_MODULE_REFERENCES_THIS: DiagnosticType = DiagnosticType::warning(
    "ES6_MODULE_REFERENCES_THIS",
    "The body of an ES6 module cannot reference 'this'.",
);

// port: Es6CheckModule#IMPORT_CANNOT_BE_REASSIGNED
pub static IMPORT_CANNOT_BE_REASSIGNED: DiagnosticType = DiagnosticType::error(
    "JSC_IMPORT_CANNOT_BE_REASSIGNED",
    "Assignment to constant variable \"{0}\".",
);

/// Checks that ES6 modules do not reference `this` in their body and do not reassign imports.
pub struct Es6CheckModule;

impl Es6CheckModule {
    // port: Es6CheckModule#Es6CheckModule
    pub fn new() -> Self {
        Self
    }
}

impl Default for Es6CheckModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for Es6CheckModule {
    // port: Es6CheckModule#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for Es6CheckModule {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6CheckModule#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_this(t) {
            if t.in_module_hoist_scope() {
                t.report(n, &ES6_MODULE_REFERENCES_THIS, &[]);
            }
            return;
        }
        Self::visit_l_value(t, n, &mut |t, name| {
            let scope = t.get_scope();
            let var = scope.get_var(t.get_compiler(), name)?;
            var.get_name_node(t.get_compiler())
        });
    }
}

impl Es6CheckModule {
    // port: Es6CheckModule#visit
    /// The GETPROP, GETELEM and NAME cases of `visit`; `name_node_of` is
    /// `t.getScope().getVar(name)` followed by `getNameNode()` (None for no var or no name
    /// node).
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit_l_value<C: CheckContext>(
        t: &mut C,
        n: NodeId,
        name_node_of: &mut dyn FnMut(&mut C, &JsString) -> Option<NodeId>,
    ) {
        match n.get_token(t) {
            Token::GETPROP | Token::GETELEM => {
                if NodeUtil::is_l_value(t, n)
                    && !NodeUtil::is_declaration_l_value(t, n)
                    && n.get_first_child(t).unwrap().is_name(t)
                {
                    let name = n.get_first_child(t).unwrap().get_string(t);
                    if let Some(name_node) = name_node_of(t, &name)
                        && name_node.is_import_star(t)
                    {
                        let name = name_node.get_string(t).to_string();
                        let error =
                            JSError::make(t, n, &IMPORT_CANNOT_BE_REASSIGNED, &[name.as_str()]);
                        t.report_error(error);
                    }
                }
            }
            Token::NAME => {
                if NodeUtil::is_l_value(t, n) && !NodeUtil::is_declaration_l_value(t, n) {
                    let name = n.get_string(t);
                    if let Some(name_node) = name_node_of(t, &name)
                        && name_node != n
                        && NodeUtil::is_imported_name(t, name_node)
                    {
                        let name = name_node.get_string(t).to_string();
                        let error =
                            JSError::make(t, n, &IMPORT_CANNOT_BE_REASSIGNED, &[name.as_str()]);
                        t.report_error(error);
                    }
                }
            }
            _ => {}
        }
    }

    /// Rust-only: `NodeTraversal#inModuleHoistScope` for a node below a top-level statement, from
    /// its ancestors: the hoist scope root the traversal has set when it visits `n` (the nearest
    /// function, class static block, class field or MODULE_BODY around it, as the traversal's
    /// handlers set it).
    fn in_module_hoist_scope(ast: &Ast, n: NodeId) -> bool {
        let mut child = n;
        let mut hoist_root = None;
        while let Some(parent) = child.get_parent(ast) {
            match parent.get_token(ast) {
                Token::FUNCTION => {
                    hoist_root = Some(parent);
                    break;
                }
                Token::BLOCK if NodeUtil::is_class_static_block(ast, parent) => {
                    hoist_root = Some(parent);
                    break;
                }
                Token::MEMBER_FIELD_DEF
                    if parent
                        .get_parent(ast)
                        .is_some_and(|p| p.is_class_members(ast)) =>
                {
                    hoist_root = parent.get_parent(ast);
                    break;
                }
                // The key of a computed field is traversed with the hoist scope root around the
                // class, its value with the class members.
                Token::COMPUTED_FIELD_DEF
                    if parent
                        .get_parent(ast)
                        .is_some_and(|p| p.is_class_members(ast))
                        && parent.has_two_children(ast)
                        && parent.get_last_child(ast) == Some(child) =>
                {
                    hoist_root = parent.get_parent(ast);
                    break;
                }
                Token::MODULE_BODY => {
                    hoist_root = Some(parent);
                    break;
                }
                Token::SCRIPT | Token::ROOT => break,
                _ => {}
            }
            child = parent;
        }
        let Some(mut module_root) = hoist_root else {
            return false;
        };
        if module_root.is_function(ast) {
            module_root = module_root.get_last_child(ast).unwrap();
        }
        NodeUtil::is_module_scope_root(ast, module_root)
    }
}

/// Rust-only (performance divergence): `Es6CheckModule` on a worker, for the statements below one
/// SCRIPT or MODULE_BODY: a name can resolve to a var declared by an import only if the module
/// imports that name (an import declares in the module scope alone), so the walk gives up only
/// for an assigned name that the module imports.
struct ReadOnlyEs6CheckModule {
    imports: Option<std::sync::Arc<[JsString]>>,
}

impl ReadOnlyCallback for ReadOnlyEs6CheckModule {
    fn should_traverse(&mut self, _cx: &mut Collector<'_>, _n: NodeId, _: Option<NodeId>) -> bool {
        true
    }

    fn visit(&mut self, cx: &mut Collector<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_this(cx) {
            if Es6CheckModule::in_module_hoist_scope(cx, n) {
                cx.report(n, &ES6_MODULE_REFERENCES_THIS, &[]);
            }
            return;
        }
        let imports = self.imports.clone();
        Es6CheckModule::visit_l_value(cx, n, &mut |cx, name| {
            if imports
                .as_deref()
                .is_some_and(|imports| imports.contains(name))
            {
                cx.give_up();
            }
            None
        });
    }
}

/// Rust-only (performance divergence): the read-only twin for `parallel_checks`.
pub(crate) fn twin() -> parallel_checks::Twin {
    parallel_checks::Twin {
        make: |context, parent| {
            Box::new(ReadOnlyEs6CheckModule {
                imports: context.module_imports.get(&parent).cloned(),
            })
        },
        resolve: None,
    }
}
