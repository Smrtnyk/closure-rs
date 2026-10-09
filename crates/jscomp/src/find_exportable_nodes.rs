/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/FindExportableNodes.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `FindExportableNodes.java`.
#![allow(clippy::collapsible_match)] // Keep Java's switch/if control flow.

use crate::abstract_compiler::AbstractCompiler;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use closure_rhino::{check_not_null, check_state};

// port: FindExportableNodes#NON_GLOBAL_ERROR
pub static NON_GLOBAL_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_NON_GLOBAL_ERROR",
    "@export only allowed on symbols/properties defined in the global scope.\nConvert to a global definition or enable --export_local_property_definitions/ options.setExportLocalPropertyDefinitions(true)",
);

// port: FindExportableNodes#EXPORT_ANNOTATION_NOT_ALLOWED
pub static EXPORT_ANNOTATION_NOT_ALLOWED: DiagnosticType = DiagnosticType::error(
    "JSC_EXPORT_ANNOTATION_NOT_ALLOWED",
    "@export is not supported on this expression.",
);

/// Records all of the symbols and properties that should be exported.
///
/// Currently applies to: - function foo() {} - var foo = function() {} - foo.bar = function() {}
/// - var FOO = ...; - foo.BAR = ...;
///
/// FOO = BAR = 5; and var FOO = BAR = 5; are not supported because the annotation is ambiguous to
/// whether it applies to all the variables or only the first one.
pub struct FindExportableNodes {
    /// The set of node with @export annotations and their associated fully qualified names
    exports: IndexMap<JsString, NodeId>,

    /// The set of class member function nodes with @export annotations and the fully qualified
    /// name of the owning class (not including the member fn name).
    es6_class_exports: IndexMap<NodeId, JsString>,

    /// The set of property names associated with @export annotations that do not have an
    /// associated fully qualified name.
    local_exports: IndexSet<JsString>,

    allow_local_exports: bool,
}

// port: FindExportableNodes.Mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    EXPORT,
    EXTERN,
}

impl FindExportableNodes {
    // port: FindExportableNodes#FindExportableNodes
    pub fn new(_compiler: &AbstractCompiler, allow_local_exports: bool) -> Self {
        Self {
            exports: IndexMap::<_, _>::default(),
            es6_class_exports: IndexMap::<_, _>::default(),
            local_exports: IndexSet::<_>::default(),
            allow_local_exports,
        }
    }

    // port: FindExportableNodes#getExports
    pub fn get_exports(&self) -> &IndexMap<JsString, NodeId> {
        &self.exports
    }

    // port: FindExportableNodes#getLocalExports
    pub fn get_local_exports(&self) -> &IndexSet<JsString> {
        &self.local_exports
    }

    // port: FindExportableNodes#getEs6ClassExports
    pub fn get_es6_class_exports(&self) -> &IndexMap<NodeId, JsString> {
        &self.es6_class_exports
    }
}

impl Callback for FindExportableNodes {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: FindExportableNodes#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let doc_info = n.get_jsdoc_info(t);
        if doc_info.is_none_or(|doc_info| !doc_info.is_export()) {
            return;
        }
        let parent = parent.unwrap();

        if parent.is_assign(t) && (n.is_function(t) || n.is_class(t)) {
            let parent_info = parent.get_jsdoc_info(t);
            if parent_info.is_some_and(|parent_info| parent_info.is_export()) {
                // ScopedAliases produces export annotations on both the function/class
                // node and assign node, we only want to visit the assign node.
                return;
            }
        }

        let mut mode: Option<Mode> = None;
        let mut export: Option<JsString> = None;
        let mut context: Option<NodeId> = None;

        let token = n.get_token(t);
        // MEMBER_FUNCTION_DEF falls through to the STRING_KEY/GETTER_DEF/SETTER_DEF group.
        let mut fallthrough = false;
        match token {
            Token::FUNCTION | Token::CLASS => {
                if parent.is_script(t) {
                    export = NodeUtil::get_name(t, n);
                    context = Some(n);
                    mode = Some(Mode::EXPORT);
                }
            }

            Token::ASSIGN => {
                let grandparent = parent.get_parent(t);
                let child = n.get_first_child(t).unwrap();
                if parent.is_expr_result(t) && !n.get_last_child(t).unwrap().is_assign(t) {
                    if grandparent.is_some_and(|g| g.is_script(t)) && child.is_qualified_name(t) {
                        export = child.get_qualified_name(t);
                        context = Some(n);
                        mode = Some(Mode::EXPORT);
                    } else if self.allow_local_exports && child.is_get_prop(t) {
                        export = Some(child.get_string(t));
                        mode = Some(Mode::EXTERN);
                    }
                }
            }

            Token::VAR | Token::LET | Token::CONST => {
                if parent.is_script(t) {
                    let first = n.get_first_child(t).unwrap();
                    if first.has_children(t) && !n.get_first_first_child(t).unwrap().is_assign(t) {
                        export = Some(first.get_string(t));
                        context = Some(n);
                        mode = Some(Mode::EXPORT);
                    }
                }
            }

            Token::GETPROP => {
                // TODO(b/63582201): currently, the IF body is executed even for code in the global
                // scope, e.g., when the pass looks at code transpiled from ES6 that uses getters.
                // This means that the top-level code we want to rewrite works by accident, only
                // when allowLocalExports happens to be true.
                if self.allow_local_exports && parent.is_expr_result(t) {
                    export = Some(n.get_string(t));
                    mode = Some(Mode::EXTERN);
                }
            }

            Token::MEMBER_FIELD_DEF => {
                if n.is_static_member(t) {
                    let class_node = parent.get_parent(t).unwrap();
                    let class_name = NodeUtil::get_name(t, class_node);
                    let Some(class_name) = class_name else {
                        t.report(n, &EXPORT_ANNOTATION_NOT_ALLOWED, &[]);
                        return;
                    };
                    export = Some(
                        class_name
                            .concat(&JsString::from("."))
                            .concat(&n.get_string(t)),
                    );
                    mode = Some(Mode::EXPORT);
                    context = Some(n);
                } else if self.allow_local_exports {
                    export = Some(n.get_string(t));
                    mode = Some(Mode::EXTERN);
                }
            }

            Token::MEMBER_FUNCTION_DEF => {
                let class_node = parent.get_parent(t).unwrap();
                if class_node.is_class(t) {
                    let method_owner_name = NodeUtil::get_best_l_value_name(
                        t,
                        NodeUtil::get_best_l_value(t, class_node),
                    );
                    let Some(method_owner_name) = method_owner_name else {
                        t.report(n, &EXPORT_ANNOTATION_NOT_ALLOWED, &[]);
                        return;
                    };
                    let suffix = if n.is_static_member(t) {
                        ""
                    } else {
                        ".prototype"
                    };
                    self.es6_class_exports
                        .insert(n, method_owner_name.concat(&JsString::from(suffix)));
                    return;
                }
                // fallthrough
                fallthrough = true;
            }

            Token::STRING_KEY | Token::GETTER_DEF | Token::SETTER_DEF => {
                fallthrough = true;
            }
            _ => {}
        }
        if fallthrough && self.allow_local_exports {
            export = Some(n.get_string(t));
            mode = Some(Mode::EXTERN);
        }

        if let Some(export) = export {
            if mode == Some(Mode::EXPORT) {
                let context = check_not_null!(context);
                self.exports.insert(export, context);
            } else {
                check_state!(context.is_none());
                check_state!(mode == Some(Mode::EXTERN));
                check_state!(!export.is_empty());
                self.local_exports.insert(export);
            }
        } else if !(n.is_get_prop(t) && parent.is_expr_result(t)) {
            // Silently ignore exports of the form:
            // /** @export */ Foo.prototype.myprop;
            // They are currently used on interfaces and records and have no effect.
            // If we cleanup the code base in the future, we can warn again.

            // Don't produce extra warnings for functions values of object literals
            if !n.is_function(t) || !NodeUtil::may_be_object_lit_key(t, parent) {
                let compiler = t.get_compiler();
                if self.allow_local_exports {
                    compiler.report(JSError::make(
                        compiler,
                        n,
                        &EXPORT_ANNOTATION_NOT_ALLOWED,
                        &[],
                    ));
                } else {
                    compiler.report(JSError::make(compiler, n, &NON_GLOBAL_ERROR, &[]));
                }
            }
        }
    }
}
