/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/ijs/ClassUtil.java.

//! Port of `com.google.javascript.jscomp.ijs.ClassUtil`: static utility methods for dealing with
//! classes. The primary benefit is for papering over the differences between ES6 class syntax.

use crate::node_util::NodeUtil;
use closure_rhino::{
    check_argument, check_state,
    js_string::JsString,
    node::{Ast, NodeId},
};

/// Java `ClassUtil` is a static utility class with a private constructor.
pub struct ClassUtil;

impl ClassUtil {
    /// Return whether the given node represents a GETPROP with a first child THIS inside a named
    /// class.
    // port: ClassUtil#isThisPropInsideClassWithName
    pub fn is_this_prop_inside_class_with_name(ast: &Ast, maybe_get_prop: NodeId) -> bool {
        Self::get_class_name_of_this_prop(ast, maybe_get_prop).is_some()
    }

    /// Return the fully qualified name of a this.property inside a constructor. This method called
    /// should only be called if `isThisPropInsideClassWithName` returns true.
    // port: ClassUtil#getFullyQualifiedNameOfThisProp
    pub fn get_fully_qualified_name_of_this_prop(ast: &Ast, get_prop: NodeId) -> JsString {
        check_argument!(Self::is_this_prop_inside_class_with_name(ast, get_prop));
        let class_name = Self::get_class_name_of_this_prop(ast, get_prop).unwrap();
        class_name
            .concat(&JsString::from(".prototype."))
            .concat(&get_prop.get_string(ast))
    }

    // port: ClassUtil#getClassNameOfThisProp
    fn get_class_name_of_this_prop(ast: &Ast, getprop: NodeId) -> Option<JsString> {
        if !getprop.is_get_prop(ast) || !getprop.get_first_child(ast).unwrap().is_this(ast) {
            return None;
        }
        let function = NodeUtil::get_enclosing_function(ast, getprop)?;
        let class_name = Self::get_member_function_class_name(ast, function);
        match class_name {
            Some(class_name) if !class_name.is_empty() => Some(class_name),
            _ => None,
        }
    }

    /// Return whether the given node represents a MEMBER_FIELD_DEF that is inside a class with a
    /// name.
    // port: ClassUtil#isMemberFieldDefInsideClassWithName
    pub fn is_member_field_def_inside_class_with_name(ast: &Ast, field_node: NodeId) -> bool {
        Self::get_member_field_def_class_name(ast, field_node).is_some()
    }

    /// Return whether the given node represents a MEMBER_FIELD_DEF that is inside a class with a
    /// name.
    // port: ClassUtil#isComputedMemberInsideClassWithName
    pub fn is_computed_member_inside_class_with_name(ast: &Ast, field_node: NodeId) -> bool {
        field_node.get_parent(ast).unwrap().is_class_members(ast)
            && field_node.get_first_child(ast).unwrap().is_get_prop(ast)
            && field_node
                .get_first_first_child(ast)
                .unwrap()
                .matches_name(ast, "Symbol")
    }

    /// Return the fully qualified name of a MEMBER_FIELD_DEF. It is invalid to call this method
    /// for a field that belongs to a nameless class.
    // port: ClassUtil#getFullyQualifiedNameOfMemberFieldDef
    pub fn get_fully_qualified_name_of_member_field_def(ast: &Ast, field_node: NodeId) -> JsString {
        check_argument!(Self::is_member_field_def_inside_class_with_name(
            ast, field_node
        ));
        let class_name = Self::get_member_field_def_class_name(ast, field_node).unwrap();
        if field_node.is_static_member(ast) {
            class_name
                .concat(&JsString::from("."))
                .concat(&field_node.get_string(ast))
        } else {
            class_name
                .concat(&JsString::from(".prototype."))
                .concat(&field_node.get_string(ast))
        }
    }

    // port: ClassUtil#getMemberFieldDefClassName
    fn get_member_field_def_class_name(ast: &Ast, field_node: NodeId) -> Option<JsString> {
        check_argument!(field_node.is_member_field_def(ast));
        let class_node = field_node.get_grandparent(ast).unwrap();
        check_state!(class_node.is_class(ast));
        NodeUtil::get_name(ast, class_node)
    }

    // port: ClassUtil#getFullyQualifiedNameOfMethod
    pub fn get_fully_qualified_name_of_method(ast: &Ast, function: NodeId) -> JsString {
        check_argument!(Self::is_class_method(ast, function));
        let class_name = Self::get_member_function_class_name(ast, function);
        check_state!(class_name.as_ref().is_some_and(|c| !c.is_empty()));
        let class_name = class_name.unwrap();
        let member_function_def = function.get_parent(ast).unwrap();
        let method_name = member_function_def.get_string(ast);
        if member_function_def.is_static_member(ast) {
            class_name.concat(&JsString::from(".")).concat(&method_name)
        } else {
            class_name
                .concat(&JsString::from(".prototype."))
                .concat(&method_name)
        }
    }

    // port: ClassUtil#isClassMethod
    pub fn is_class_method(ast: &Ast, function_node: NodeId) -> bool {
        check_argument!(function_node.is_function(ast));
        let parent = function_node.get_parent(ast).unwrap();
        parent.is_member_function_def(ast) && parent.get_parent(ast).unwrap().is_class_members(ast)
    }

    /// Checks whether the given constructor/member function belongs to a named class, as opposed
    /// to an anonymous class.
    // port: ClassUtil#hasNamedClass
    pub fn has_named_class(ast: &Ast, function_node: NodeId) -> bool {
        check_argument!(function_node.is_function(ast));
        Self::get_member_function_class_name(ast, function_node).is_some()
    }

    // port: ClassUtil#getMemberFunctionClassName
    fn get_member_function_class_name(ast: &Ast, function_node: NodeId) -> Option<JsString> {
        check_argument!(function_node.is_function(ast));
        if Self::is_class_method(ast, function_node) {
            let parent = function_node.get_parent(ast).unwrap();
            check_state!(parent.is_member_function_def(ast));
            // ES6 class
            let class_node = function_node
                .get_grandparent(ast)
                .unwrap()
                .get_parent(ast)
                .unwrap();
            check_state!(class_node.is_class(ast));
            return NodeUtil::get_name(ast, class_node);
        }
        NodeUtil::get_name(ast, function_node)
    }

    // port: ClassUtil#isConstructor
    pub fn is_constructor(ast: &Ast, function_node: NodeId) -> bool {
        if Self::is_class_method(ast, function_node) {
            return NodeUtil::is_es6_constructor(ast, function_node);
        }
        let jsdoc = NodeUtil::get_best_jsdoc_info(ast, function_node);
        jsdoc.is_some_and(|jsdoc| jsdoc.is_constructor_or_interface())
    }
}
