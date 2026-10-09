/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/ClassOrObjectElementInfo.java.

use super::{token::Token, trees::parse_tree::Tree, util::SourcePosition};
/// Gathers information for ES6 class / Object literal methods, getters, setters and fields.
///
/// <p>Gathers: the start position, whether it is a class element (false for object literal), whether
/// it is static (only for class elements), and, after construction, the name IdentifierToken or
/// ParseTree.
///
/// <p>Used by {@link Parser} but separated out to enforce visibility.
pub struct ClassOrObjectElementInfo {
    // The start position of the element.
    pub start: SourcePosition,
    // Whether this is a class member. False for object literal elements.
    pub is_class_member: bool,
    // Whether this class element is static. Can only be true if {@link #isClassMember} is true.
    pub is_static: bool,
    name: Option<Token>,
    name_expr: Option<Tree>,
}

impl ClassOrObjectElementInfo {
    // port: ClassOrObjectElementInfo#<init>
    fn new(start: SourcePosition, is_class_member: bool, is_static: bool) -> Self {
        Self {
            start,
            is_class_member,
            is_static,
            name: None,
            name_expr: None,
        }
    }

    // port: ClassOrObjectElementInfo#createClassMemberInfo
    pub fn create_class_member_info(start: SourcePosition, is_static: bool) -> Self {
        Self::new(start, /* isClassMember= */ true, is_static)
    }

    // port: ClassOrObjectElementInfo#createObjectLiteralElementInfo
    pub fn create_object_literal_element_info(start: SourcePosition) -> Self {
        Self::new(
            start, /* isClassMember= */ false, /* isStatic= */ false,
        )
    }

    // port: ClassOrObjectElementInfo#setName
    pub fn set_name(&mut self, name: Token) {
        assert!(self.name_expr.is_none());
        self.name = Some(name);
    }

    // port: ClassOrObjectElementInfo#hasName
    pub fn has_name(&self) -> bool {
        self.name.is_some()
    }

    // port: ClassOrObjectElementInfo#getName
    pub fn get_name(&self) -> Token {
        self.name.as_ref().expect("null").clone()
    }

    // port: ClassOrObjectElementInfo#setNameExpr
    pub fn set_name_expr(&mut self, name_expr: Tree) {
        assert!(self.name.is_none());
        self.name_expr = Some(name_expr);
    }

    // port: ClassOrObjectElementInfo#hasNameExpr
    pub fn has_name_expr(&self) -> bool {
        self.name_expr.is_some()
    }

    // port: ClassOrObjectElementInfo#getNameExpr
    pub fn get_name_expr(&self) -> Tree {
        self.name_expr.as_ref().expect("null").clone()
    }
}
