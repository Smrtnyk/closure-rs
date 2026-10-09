/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckUnusedPrivateProperties.java.

//! Checks for unused @private properties.

#![allow(clippy::collapsible_match)] // Preserve Java switch cases with nested ifs.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_state,
    js_string::JsString,
    jsdoc_info::Visibility,
    node::{Ast, NodeId},
    token::Token,
};
use indexmap::IndexSet;

// port: CheckUnusedPrivateProperties#UNUSED_PRIVATE_PROPERTY
pub static UNUSED_PRIVATE_PROPERTY: DiagnosticType = DiagnosticType::disabled(
    "JSC_UNUSED_PRIVATE_PROPERTY",
    "Private property {0} is never read",
);

pub struct CheckUnusedPrivateProperties {
    used: IndexSet<JsString>,
    candidates: Vec<NodeId>,
    constructors_and_interfaces: IndexSet<JsString>,
}

impl CheckUnusedPrivateProperties {
    // port: CheckUnusedPrivateProperties#CheckUnusedPrivateProperties
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            used: IndexSet::new(),
            candidates: Vec::new(),
            constructors_and_interfaces: IndexSet::new(),
        }
    }

    // port: CheckUnusedPrivateProperties#reportUnused
    fn report_unused(&self, t: &mut NodeTraversal<'_>) {
        for &n in &self.candidates {
            let prop_name = Self::get_prop_name(t, n);
            if !self.used.contains(&prop_name) {
                t.report(n, &UNUSED_PRIVATE_PROPERTY, &[&prop_name.to_string()]);
            }
        }
    }

    // port: CheckUnusedPrivateProperties#getPropName
    fn get_prop_name(ast: &Ast, n: NodeId) -> JsString {
        match n.get_token(ast) {
            Token::GETPROP | Token::MEMBER_FUNCTION_DEF => {
                return n.get_string(ast);
            }
            _ => {}
        }
        panic!("Unexpected node type: {}", n.to_string(ast));
    }

    // port: CheckUnusedPrivateProperties#recordConstructorOrInterface
    fn record_constructor_or_interface(&mut self, ast: &Ast, n: NodeId) {
        let class_name = NodeUtil::get_best_l_value_name(ast, NodeUtil::get_best_l_value(ast, n));
        if let Some(class_name) = class_name {
            // If className is null, then this pass won't report any diagnostics on static members
            // of this class.
            self.constructors_and_interfaces.insert(class_name);
        }
    }

    // port: CheckUnusedPrivateProperties#isPrivatePropDecl
    fn is_private_prop_decl(ast: &Ast, n: NodeId) -> bool {
        // TODO(johnlenz): add support private by convention property definitions without JSDoc.
        // TODO(johnlenz): add support for checking protected properties in final classes
        // TODO(johnlenz): add support for checking "package" properties when checking an entire
        // library.
        let info = NodeUtil::get_best_jsdoc_info(ast, n);
        info.is_some_and(|info| info.get_visibility() == Visibility::PRIVATE)
    }

    // port: CheckUnusedPrivateProperties#isCheckablePrivatePropDecl
    fn is_checkable_private_prop_decl(ast: &Ast, n: NodeId) -> bool {
        // TODO(tbreisacher): Look for uses of the typedef/interface in type expressions; warn if
        // there are no uses.
        let info = NodeUtil::get_best_jsdoc_info(ast, n);
        Self::is_private_prop_decl(ast, n) && {
            let info = info.unwrap();
            !info.has_typedef_type() && !info.is_interface()
        }
    }

    // port: CheckUnusedPrivateProperties#isCandidatePropertyDefinition
    fn is_candidate_property_definition(&self, ast: &Ast, n: NodeId) -> bool {
        check_state!(n.is_get_prop(ast), "%s", n.to_string(ast));
        let target = n.get_first_child(ast).unwrap();
        target.is_this(ast)
            || target
                .get_qualified_name(ast)
                .is_some_and(|q| self.constructors_and_interfaces.contains(&q))
            || (target.is_get_prop(ast) && target.get_string_ref(ast) == "prototype")
    }

    /// @return Whether the property is used in a way that prevents its removal.
    // port: CheckUnusedPrivateProperties#isPinningPropertyUse
    fn is_pinning_property_use(ast: &Ast, n: NodeId) -> bool {
        // Rather than looking for cases that are uses, we assume all references are
        // pinning uses unless they are:
        //  - a simple assignment (x.a = 1)
        //  - a compound assignment or increment (x++, x += 1) whose result is
        //    otherwise unused

        let parent = n.get_parent(ast).unwrap();
        if Some(n) == parent.get_first_child(ast) {
            if parent.is_expr_result(ast) {
                // A stub declaration "this.x;" isn't a pinning use.
                return false;
            } else if parent.is_assign(ast) {
                // A simple assignment doesn't pin the property.
                return false;
            } else if NodeUtil::is_assignment_op(ast, parent)
                || parent.is_inc(ast)
                || parent.is_dec(ast)
            {
                // In general, compound assignments are both reads and writes, but
                // if the property is never otherwise read we can consider it simply
                // a write.
                // However if the assign expression is used as part of a larger
                // expression, we must consider it a read. For example:
                //    x = (y.a += 1);
                return NodeUtil::is_expression_result_used(ast, parent);
            }
        }
        true
    }
}

impl CompilerPass for CheckUnusedPrivateProperties {
    // port: CheckUnusedPrivateProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckUnusedPrivateProperties {
    // port: CheckUnusedPrivateProperties#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            self.used.clear();
            // Even if a constructor is unused , we still want to prevent the construction of the
            // class outside the file.  i.e. a container of static methods.
            self.used.insert(JsString::from("constructor"));
            self.candidates.clear();
        }
        true
    }

    // port: CheckUnusedPrivateProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            // exiting the script, report any privates not used in the file.
            Token::SCRIPT => self.report_unused(t),
            Token::GETPROP => {
                let prop_name = n.get_string(t);
                if Self::is_pinning_property_use(t, n)
                    || !self.is_candidate_property_definition(t, n)
                {
                    self.used.insert(prop_name);
                } else {
                    // Only consider "private" properties.
                    if Self::is_checkable_private_prop_decl(t, n) {
                        self.candidates.push(n);
                    }
                }
            }
            Token::MEMBER_FUNCTION_DEF => {
                // Only consider "private" methods.
                if Self::is_checkable_private_prop_decl(t, n) {
                    self.candidates.push(n);
                }
            }
            Token::OBJECTLIT => {
                // Assume any object literal definition might be a reflection on the
                // class property.
                let mut c = n.get_first_child(t);
                while let Some(child) = c {
                    if child.is_string_key(t)
                        || child.is_getter_def(t)
                        || child.is_setter_def(t)
                        || child.is_member_function_def(t)
                    {
                        self.used.insert(child.get_string(t));
                    }
                    c = child.get_next(t);
                }
            }
            Token::CALL => {
                // Look for properties referenced through a property rename function.
                let target = n.get_first_child(t).unwrap();
                if n.has_more_than_one_child(t) && {
                    let compiler: &AbstractCompiler = t.get_compiler();
                    compiler
                        .get_coding_convention()
                        .is_property_rename_function(compiler, target)
                } {
                    let prop_name = target.get_next(t).unwrap();
                    if prop_name.is_string_lit(t) {
                        self.used.insert(prop_name.get_string(t));
                    }
                }
            }
            Token::FUNCTION => {
                let info = NodeUtil::get_best_jsdoc_info(t, n);
                if info.is_some_and(|info| info.is_constructor() || info.is_interface()) {
                    self.record_constructor_or_interface(t, n);
                }
            }
            Token::CLASS => self.record_constructor_or_interface(t, n),
            _ => {}
        }
    }
}
