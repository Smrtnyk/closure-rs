/*
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ConcretizeStaticInheritanceForInlining.java.

//! Port of `ConcretizeStaticInheritanceForInlining.java`.
//!
//! Rewrites static inheritance to explicitly copy inherited properties from superclass to
//! subclass so that the optimizer knows the subclass has those properties.
//!
//! For example, the main transpilation passes will convert this ES6 code:
//!
//! ```text
//!   class Foo { static f() {} }
//!   class Bar extends Foo {}
//! ```
//!
//! to this ES3 code:
//!
//! ```text
//!   function Foo() {}
//!   Foo.f = function() {};
//!   function Bar() {}
//!   $jscomp.inherits(Bar, Foo);
//! ```
//!
//! and then this class will convert that to
//!
//! ```text
//!   function Foo() {}
//!   Foo.f = function() {};
//!   function Bar() {}
//!   $jscomp.inherits(Bar, Foo);
//!   Bar.f = Foo.f;
//! ```
//!
//! Additionally, there are getter and setter fields which are transpiled from:
//!
//! ```text
//!   class Foo { static get prop() { return 1; } }
//!   class Bar extends Foo {}
//! ```
//!
//! to:
//!
//! ```text
//!   var Foo = function() {};
//!   Foo.prop; // stub declaration so that the optimizer knows about prop
//!   Object.defineProperties(Foo, {prop:{get:function() { return 1; }}});
//!
//!   var Bar = function() {};
//!   $jscomp.inherits(Bar, Foo);
//! ```
//!
//! The stub declaration of Foo.prop needs to be duplicated for Bar so that the optimizer knows
//! that Bar also has this property. (ES5 classes don't have class-side inheritance).
//!
//! ```text
//!   var Bar = function() {};
//!   Bar.prop;
//!   $jscomp.inherits(Bar, Foo);
//! ```
//!
//! In order to gather the stub declarations, this pass gathers all GETPROPs on a class. In order
//! to determine which of these are the stub declarations it filters them based on names
//! discovered in Object.defineProperties. Unfortunately, we cannot simply gather the defined
//! properties because they don't have the JSDoc, which may include optimization-relevant
//! annotations like @nocollapse.
//!
//! TODO(tdeegan): In the future the JSDoc for getter/setter properties could be stored in the
//! defineProperties functions. It would reduce the complexity of this pass significantly.
//!
//! NOTE: currently this pass only exists to prevent property collapsing from breaking some
//! simple class-side inheritance cases when transpiling.
//!
//! Rust: Java's `classByAlias` maps several aliases to one shared `JavascriptClass` object; here
//! the objects live in `classes` and `class_by_alias` maps each alias to its index.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js::runtime_js_lib_manager::JsLibField,
    js_error::JSError,
    node_traversal::{
        AbstractPostOrderCallback, AbstractPostOrderCallbackInterface, NodeTraversal,
    },
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jsdoc_info::Builder as JSDocInfoBuilder,
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::Arc;

// port: ConcretizeStaticInheritanceForInlining#DUPLICATE_CLASS
pub static DUPLICATE_CLASS: DiagnosticType = DiagnosticType::error(
    "DUPLICATE_CLASS",
    "Multiple classes cannot share the same name: {0}",
);

// Property names that may cause issues if they are concretized.
// port: ConcretizeStaticInheritanceForInlining#BANNED_PROP_NAMES
const BANNED_PROP_NAMES: [&str; 2] = ["prototype", "getInstance"];

#[derive(Default)]
struct JavascriptClass {
    // All static members to the class including get set properties.
    static_members: IndexSet<NodeId>,
    // Keep updated the set of static member names to avoid O(n^2) searches.
    static_member_names: IndexSet<JsString>,
    // Collect all get set properties as defined by Object.defineProperties(...)
    defined_properties: IndexSet<JsString>,
}

impl JavascriptClass {
    // port: ConcretizeStaticInheritanceForInlining.JavascriptClass#addStaticMember
    fn add_static_member(&mut self, ast: &Ast, node: NodeId) {
        self.static_members.insert(node);
        self.static_member_names
            .insert(node.get_first_child(ast).unwrap().get_string(ast));
    }
}

pub struct ConcretizeStaticInheritanceForInlining {
    duplicate_class_names: IndexSet<JsString>,
    inherits: Arc<dyn JsLibField>,
    /// Rust: the `JavascriptClass` objects `class_by_alias` refers to.
    classes: Vec<JavascriptClass>,
    class_by_alias: IndexMap<JsString, usize>,
}

impl ConcretizeStaticInheritanceForInlining {
    // port: ConcretizeStaticInheritanceForInlining#ConcretizeStaticInheritanceForInlining
    pub fn new(compiler: &AbstractCompiler) -> Self {
        let inherits = compiler
            .get_runtime_js_lib_manager()
            .lock()
            .unwrap()
            .get_js_lib_field("$jscomp.inherits");
        Self {
            duplicate_class_names: IndexSet::<_>::default(),
            inherits,
            classes: Vec::new(),
            class_by_alias: IndexMap::<_, _>::default(),
        }
    }

    /// Rust-only: `classByAlias.get(name)` for a nullable name.
    fn class_index(&self, name: Option<&JsString>) -> Option<usize> {
        name.and_then(|name| self.class_by_alias.get(name).copied())
    }

    // port: ConcretizeStaticInheritanceForInlining#processInherits
    fn process_inherits(
        &mut self,
        compiler: &mut AbstractCompiler,
        find_static_members: &FindStaticMembers,
    ) {
        for &inherits_call in &find_static_members.inherits_calls {
            let superclass_name_node = inherits_call.get_last_child(compiler).unwrap();
            let superclass_qname = superclass_name_node.get_qualified_name(compiler);
            let subclass_name_node = superclass_name_node.get_previous(compiler).unwrap();
            let subclass_qname = subclass_name_node.get_qualified_name(compiler);
            let super_class = self.class_index(superclass_qname.as_ref());
            let sub_class = self.class_index(subclass_qname.as_ref());
            if let Some(superclass_qname) = &superclass_qname
                && self.duplicate_class_names.contains(superclass_qname)
            {
                let superclass_qname = superclass_qname.to_string_lossy();
                let error = JSError::make(
                    compiler,
                    inherits_call,
                    &DUPLICATE_CLASS,
                    &[&superclass_qname],
                );
                compiler.report(error);
                return;
            }
            let (Some(super_class), Some(sub_class)) = (super_class, sub_class) else {
                continue;
            };
            self.copy_static_members(
                compiler,
                super_class,
                sub_class,
                inherits_call,
                find_static_members,
            );
        }
    }

    // port: ConcretizeStaticInheritanceForInlining#copyStaticMembers
    fn copy_static_members(
        &mut self,
        compiler: &mut AbstractCompiler,
        super_class: usize,
        sub_class: usize,
        inherits_call: NodeId,
        find_static_members: &FindStaticMembers,
    ) {
        // Rust: a snapshot; members are only added to `sub_class`, and when it is the
        // superclass itself every member is overridden, so nothing is added while iterating.
        let static_members: Vec<NodeId> = self.classes[super_class]
            .static_members
            .iter()
            .copied()
            .collect();
        for static_member in static_members {
            check_state!(
                static_member.is_assign(compiler),
                "%s",
                static_member.to_string(compiler)
            );
            let member_name = static_member
                .get_first_child(compiler)
                .unwrap()
                .get_string(compiler);
            if self.classes[super_class]
                .defined_properties
                .contains(&member_name)
            {
                continue;
            }
            if self.is_overriden(sub_class, &member_name) {
                continue;
            }
            if find_static_members.is_before(inherits_call, static_member) {
                // Don't copy members that are defined after the $jscomp.inherits call,
                // since they will not work correctly in IE<11, where static inheritance
                // is done by copying, rather than prototype manipulation.
                continue;
            }
            let mut info = JSDocInfoBuilder::maybe_copy_from(
                static_member.get_jsdoc_info(compiler).as_deref(),
            );
            let function = static_member.get_last_child(compiler).unwrap();
            let mut source_info_node = function;
            if function.is_function(compiler) {
                source_info_node = function.get_first_child(compiler).unwrap();
            }
            let subclass_name_node = inherits_call.get_second_child(compiler).unwrap();
            let superclass_name_node = subclass_name_node.get_next(compiler).unwrap();
            let subclass_clone = subclass_name_node.clone_tree(compiler);
            let sub_getprop = IR::getprop(compiler, subclass_clone, member_name.clone());
            let superclass_clone = superclass_name_node.clone_tree(compiler);
            let super_getprop = IR::getprop(compiler, superclass_clone, member_name);
            let assign = IR::assign(compiler, sub_getprop, super_getprop);
            assign.set_jsdoc_info(compiler, info.build());
            let expr_result = IR::expr_result(compiler, assign);
            expr_result.srcref_tree_if_missing(compiler, source_info_node);
            let inherits_expression_result = inherits_call.get_parent(compiler).unwrap();
            expr_result.insert_after(compiler, inherits_expression_result);
            compiler.report_change_to_enclosing_scope(inherits_expression_result);

            // Add the static member to the subclass so that subclasses also copy this member.
            self.classes[sub_class].add_static_member(compiler, assign);
        }
    }

    // port: ConcretizeStaticInheritanceForInlining#isOverriden
    fn is_overriden(&self, sub_class: usize, member_name: &JsString) -> bool {
        let sub_class = &self.classes[sub_class];
        if sub_class.static_member_names.contains(member_name) {
            // This subclass overrides the static method, so there is no need to copy the
            // method from the base class.
            return true;
        }
        if sub_class.defined_properties.contains(member_name) {
            return true;
        }
        false
    }

    // port: ConcretizeStaticInheritanceForInlining#isReferenceToClass
    fn is_reference_to_class(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let class_name = n.get_qualified_name(t);
        if self.class_index(class_name.as_ref()).is_none() {
            return false;
        }
        if !n.is_name(t) {
            return true;
        }
        let class_name = class_name.unwrap();
        let scope = t.get_scope();
        let compiler = t.get_compiler();
        let var = scope.get_var(compiler, &class_name);
        var.is_none_or(|var| !var.is_local(compiler))
    }
}

impl CompilerPass for ConcretizeStaticInheritanceForInlining {
    // port: ConcretizeStaticInheritanceForInlining#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut find_static_members = FindStaticMembers::new();
        // Since this is an optimization pass, running on externs would be unnecessary
        {
            let mut callback = AbstractPostOrderCallback::new(FindStaticMembersVisitor {
                outer: self,
                find_static_members: &mut find_static_members,
            });
            NodeTraversal::traverse(compiler, root, &mut callback);
        }
        self.process_inherits(compiler, &find_static_members);
    }
}

struct FindStaticMembers {
    inherits_calls: Vec<NodeId>,
    // Store the order we find class definitions and static fields.  Copied statics must occur
    // after both the namespace and the copied property are defined.
    node_order: IndexMap<NodeId, usize>,
}

impl FindStaticMembers {
    fn new() -> Self {
        Self {
            inherits_calls: Vec::new(),
            node_order: IndexMap::<_, _>::default(),
        }
    }

    // port: ConcretizeStaticInheritanceForInlining.FindStaticMembers#isBefore
    fn is_before(&self, earlier: NodeId, later: NodeId) -> bool {
        let earlier_position = self.node_order.get(&earlier);
        let later_position = self.node_order.get(&later);
        matches!((earlier_position, later_position), (Some(e), Some(l)) if e < l)
    }
}

/// Rust: `FindStaticMembers` as a traversal callback; Java's inner class also reads and writes
/// the outer pass's `classByAlias` and `duplicateClassNames`.
struct FindStaticMembersVisitor<'a> {
    outer: &'a mut ConcretizeStaticInheritanceForInlining,
    find_static_members: &'a mut FindStaticMembers,
}

impl FindStaticMembersVisitor<'_> {
    // port: ConcretizeStaticInheritanceForInlining.FindStaticMembers#visitDefinedPropertiesCall
    fn visit_defined_properties_call(
        &mut self,
        t: &mut NodeTraversal<'_>,
        define_properties_call: NodeId,
    ) {
        let object = define_properties_call.get_second_child(t).unwrap();
        if self.outer.is_reference_to_class(t, object) {
            let class_name = object.get_qualified_name(t);
            let c = self.outer.class_index(class_name.as_ref()).unwrap();
            for prop in NodeUtil::get_object_defined_properties_keys(t, define_properties_call) {
                let prop_string = prop.get_string(t);
                self.outer.classes[c].defined_properties.insert(prop_string);
            }
        }
    }

    // port: ConcretizeStaticInheritanceForInlining.FindStaticMembers#visitFunctionClassDef
    fn visit_function_class_def(&mut self, ast: &Ast, n: NodeId) {
        let class_info = NodeUtil::get_best_jsdoc_info(ast, n);
        if class_info.is_some_and(|class_info| class_info.is_constructor()) {
            let Some(name) = NodeUtil::get_name(ast, n) else {
                return;
            };
            if self.outer.class_by_alias.contains_key(&name) {
                self.outer.duplicate_class_names.insert(name);
            } else {
                self.outer.classes.push(JavascriptClass::default());
                let index = self.outer.classes.len() - 1;
                self.outer.class_by_alias.insert(name, index);
            }
        }
    }

    // port: ConcretizeStaticInheritanceForInlining.FindStaticMembers#setAlias
    fn set_alias(&mut self, original: Option<&JsString>, alias: Option<&JsString>) {
        let original = check_not_null!(original, "original is null");
        let alias = check_not_null!(alias, "alias is null");
        check_argument!(self.outer.class_by_alias.contains_key(original));
        let class = *self.outer.class_by_alias.get(original).unwrap();
        self.outer.class_by_alias.insert(alias.clone(), class);
    }

    // port: ConcretizeStaticInheritanceForInlining.FindStaticMembers#visitAssign
    fn visit_assign(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        // Alias for classes. We assume that the alias appears after the class declaration.
        let existing_class_qname = n.get_last_child(t).unwrap().get_qualified_name(t);
        if existing_class_qname.is_some()
            && self
                .outer
                .class_index(existing_class_qname.as_ref())
                .is_some()
        {
            let alias = n.get_first_child(t).unwrap().get_qualified_name(t);
            if alias.is_some() {
                self.set_alias(existing_class_qname.as_ref(), alias.as_ref());
            }
        } else if n.get_first_child(t).unwrap().is_get_prop(t) {
            let get_prop = n.get_first_child(t).unwrap();
            let class_node = get_prop.get_first_child(t).unwrap();
            if self.outer.is_reference_to_class(t, class_node)
                && !BANNED_PROP_NAMES.contains(&get_prop.get_string(t).to_string_lossy().as_str())
            {
                let class_qname = class_node.get_qualified_name(t);
                let c = self.outer.class_index(class_qname.as_ref()).unwrap();
                self.outer.classes[c].add_static_member(t, n);
                let size = self.find_static_members.node_order.len();
                self.find_static_members.node_order.insert(n, size);
            }
        }
    }

    // port: ConcretizeStaticInheritanceForInlining.FindStaticMembers#visitVariableDeclaration
    fn visit_variable_declaration(&mut self, ast: &Ast, n: NodeId) {
        let child = n.get_first_child(ast).unwrap();
        if !child.has_children(ast) {
            return;
        }
        let maybe_original_name = child.get_first_child(ast).unwrap().get_qualified_name(ast);
        if maybe_original_name.is_some()
            && self
                .outer
                .class_index(maybe_original_name.as_ref())
                .is_some()
        {
            let maybe_alias = child.get_qualified_name(ast);
            if maybe_alias.is_some() {
                self.set_alias(maybe_original_name.as_ref(), maybe_alias.as_ref());
            }
        }
    }
}

impl AbstractPostOrderCallbackInterface for FindStaticMembersVisitor<'_> {
    // port: ConcretizeStaticInheritanceForInlining.FindStaticMembers#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::CALL => {
                if self
                    .outer
                    .inherits
                    .matches(t, n.get_first_child(t).unwrap())
                {
                    self.find_static_members.inherits_calls.push(n);
                    let size = self.find_static_members.node_order.len();
                    self.find_static_members.node_order.insert(n, size);
                }
                if NodeUtil::is_object_define_properties_definition(t, n) {
                    self.visit_defined_properties_call(t, n);
                }
            }
            Token::CONST | Token::LET | Token::VAR => self.visit_variable_declaration(t, n),
            Token::ASSIGN => self.visit_assign(t, n),
            Token::FUNCTION => self.visit_function_class_def(t, n),
            _ => {}
        }
    }
}
