/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Bob Jervis
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/jstype/Visitor.java.

use crate::{TypeId, js_type_registry::JSTypeRegistry};
use closure_rhino::node::Ast;

/// Java-shaped type visitor; allocation-capable cases receive the AST used for resolution.
pub trait Visitor<T> {
    // port: Visitor#caseNoType
    fn case_no_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseEnumElementType
    fn case_enum_element_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseAllType
    fn case_all_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseBooleanType
    fn case_boolean_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseNoObjectType
    fn case_no_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseFunctionType
    fn case_function_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseObjectType
    fn case_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseUnknownType
    fn case_unknown_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseNullType
    fn case_null_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseNamedType
    fn case_named_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseProxyObjectType
    fn case_proxy_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseNumberType
    fn case_number_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseBigIntType
    fn case_big_int_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseStringType
    fn case_string_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseSymbolType
    fn case_symbol_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseVoidType
    fn case_void_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T;
    // port: Visitor#caseUnionType
    fn case_union_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseTemplatizedType
    fn case_templatized_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
    // port: Visitor#caseTemplateType
    fn case_template_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T;
}

/// The default-case adapter from Visitor.WithDefaultCase.
pub trait WithDefaultCase<T> {
    // port: Visitor.WithDefaultCase#caseDefault
    fn case_default(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: Option<TypeId>) -> T;
    // port: Visitor.WithDefaultCase#caseNoType
    fn case_no_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseEnumElementType
    fn case_enum_element_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseAllType
    fn case_all_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseBooleanType
    fn case_boolean_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseNoObjectType
    fn case_no_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseFunctionType
    fn case_function_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseObjectType
    fn case_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseUnknownType
    fn case_unknown_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseNullType
    fn case_null_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseNamedType
    fn case_named_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseProxyObjectType
    fn case_proxy_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseNumberType
    fn case_number_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseBigIntType
    fn case_big_int_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseStringType
    fn case_string_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseSymbolType
    fn case_symbol_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseVoidType
    fn case_void_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        self.case_default(reg, ast, None)
    }
    // port: Visitor.WithDefaultCase#caseUnionType
    fn case_union_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseTemplatizedType
    fn case_templatized_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
    // port: Visitor.WithDefaultCase#caseTemplateType
    fn case_template_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        self.case_default(reg, ast, Some(type_))
    }
}

impl<T, V: WithDefaultCase<T>> Visitor<T> for V {
    fn case_no_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_no_type(self, reg, ast, type_)
    }
    fn case_enum_element_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_enum_element_type(self, reg, ast, type_)
    }
    fn case_all_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_all_type(self, reg, ast)
    }
    fn case_boolean_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_boolean_type(self, reg, ast)
    }
    fn case_no_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_no_object_type(self, reg, ast)
    }
    fn case_function_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_function_type(self, reg, ast, type_)
    }
    fn case_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_object_type(self, reg, ast, type_)
    }
    fn case_unknown_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_unknown_type(self, reg, ast)
    }
    fn case_null_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_null_type(self, reg, ast)
    }
    fn case_named_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_named_type(self, reg, ast, type_)
    }
    fn case_proxy_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_proxy_object_type(self, reg, ast, type_)
    }
    fn case_number_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_number_type(self, reg, ast)
    }
    fn case_big_int_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_big_int_type(self, reg, ast)
    }
    fn case_string_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_string_type(self, reg, ast)
    }
    fn case_symbol_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_symbol_type(self, reg, ast, type_)
    }
    fn case_void_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> T {
        WithDefaultCase::case_void_type(self, reg, ast)
    }
    fn case_union_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_union_type(self, reg, ast, type_)
    }
    fn case_templatized_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_templatized_type(self, reg, ast, type_)
    }
    fn case_template_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> T {
        WithDefaultCase::case_template_type(self, reg, ast, type_)
    }
}
