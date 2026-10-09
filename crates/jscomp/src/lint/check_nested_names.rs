/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/lint/CheckNestedNames.java.

//! Port of lint/CheckNestedNames.java: checks if code has a module-level static property
//! assignment (`X.Y`) inside a `goog.module`. If yes, reports a linter warning on the `X.Y` if X
//! is any name (object, class, function, interface, enum, typedef) and Y is a class, interface,
//! enum or typedef.
use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::node_traversal::{Callback, NodeTraversal};
use closure_jstype::prelude::*;
use closure_rhino::check_argument;
use closure_rhino::node::{Ast, NodeId, Prop};

// port: CheckNestedNames#NESTED_NAME_IN_GOOG_MODULE
pub static NESTED_NAME_IN_GOOG_MODULE: DiagnosticType = DiagnosticType::disabled(
    "JSC_NESTED_NAME_IN_GOOG_MODULE",
    "A nested {0} is created on the name `{1}`. Fix this linter finding by converting the module-level static property assignment on `{1}` into a module-level flat name (i.e. change `{1}.prop = ...` into `{1}_prop = ...`. You can (if required) export this flat name using named exports (`exports.{1}_prop = {1}_prop`).",
);

// port: CheckNestedNames.DeclarationKind
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeclarationKind {
    CLASS,
    ENUM,
    TYPEDEF,
    INTERFACE,
}

// port: CheckNestedNames#NESTED_KIND_TO_REPORT
static NESTED_KIND_TO_REPORT: [(DeclarationKind, &str); 4] = [
    (DeclarationKind::CLASS, "class"),
    (DeclarationKind::ENUM, "enum"),
    (DeclarationKind::INTERFACE, "interface"),
    (DeclarationKind::TYPEDEF, "typedef"),
];

// port: CheckNestedNames
pub struct CheckNestedNames;

impl CheckNestedNames {
    // port: CheckNestedNames#CheckNestedNames
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    /// Gives `a.b` from `a.b.c.d...`.
    // port: CheckNestedNames#getTargetGetProp
    fn get_target_get_prop(ast: &Ast, n: NodeId) -> NodeId {
        check_argument!(n.is_get_prop(ast), &n.to_string(ast));
        let first = n.get_first_child(ast).unwrap();
        if first.is_get_prop(ast) {
            return Self::get_target_get_prop(ast, first);
        }
        n
    }

    /// True for `someExpression.prototype`.
    // port: CheckNestedNames#isDotPrototype
    fn is_dot_prototype(ast: &Ast, get_prop: NodeId) -> bool {
        get_prop.is_get_prop(ast) && get_prop.get_string_ref(ast) == "prototype"
    }

    // port: CheckNestedNames#getNestedDeclarationKind
    fn get_nested_declaration_kind(
        compiler: &mut AbstractCompiler,
        lhs: NodeId,
        parent: NodeId,
    ) -> Option<DeclarationKind> {
        check_argument!(
            lhs.is_first_child_of(compiler, Some(parent)),
            &lhs.to_string(compiler)
        );
        // Handle `/** @typedef {...} */` X.Y` or `/** @const */ X.Y = SomeTypeDefName`
        if lhs.get_typedef_type_prop(compiler).is_some() {
            return Some(DeclarationKind::TYPEDEF);
        }

        let type_ = lhs.get_jstype(compiler);
        if let Some(type_) = type_ {
            let reg = compiler.get_type_registry();
            // check whether it's an enum, interface or typedef first.
            if type_.is_enum_type(reg) {
                return Some(DeclarationKind::ENUM);
            } else if type_.is_interface(reg) {
                return Some(DeclarationKind::INTERFACE);
            } else if type_.is_constructor(reg) {
                return Some(DeclarationKind::CLASS);
            }
        }

        None
    }
}

impl CompilerPass for CheckNestedNames {
    // port: CheckNestedNames#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckNestedNames {
    // port: CheckNestedNames#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(node_traversal) {
            // only traverse inside goog.modules
            return n.get_boolean_prop(node_traversal, Prop::GOOG_MODULE);
        }
        // only warn on static property assignments of module-level names
        node_traversal.in_global_or_module_scope()
    }

    // port: CheckNestedNames#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        check_argument!(t.in_global_or_module_scope());
        if !n.is_get_prop(t) || !n.is_qualified_name(t) {
            return;
        }
        let parent = parent.expect("NullPointerException");
        let target_get_prop = Self::get_target_get_prop(t, n);
        if (parent.is_assign(t) || parent.is_expr_result(t))
            && n.is_first_child_of(t, Some(parent))
            && !Self::is_dot_prototype(t, target_get_prop)
        {
            let owner = target_get_prop.get_first_child(t).unwrap();
            if !owner.is_name(t) || owner.get_string_ref(t) == "exports" {
                return; //  For example `this` or `super` or `exports.SomeEnum = {}`
            }
            let owner_name = owner.get_string(t).to_string();
            let declaration_kind = Self::get_nested_declaration_kind(t.get_compiler(), n, parent);
            if let Some((_, nested_kind)) = NESTED_KIND_TO_REPORT
                .iter()
                .find(|(kind, _)| Some(*kind) == declaration_kind)
            {
                // Found a nested name that created an inner class, interface, typedef or enum.
                t.report(
                    target_get_prop,
                    &NESTED_NAME_IN_GOOG_MODULE,
                    &[nested_kind, &owner_name],
                );
            }
        }
    }
}
