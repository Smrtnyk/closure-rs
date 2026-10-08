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
//   src/com/google/javascript/rhino/jstype/ContainsUpperBoundSuperTypeVisitor.java.

use crate::TypeId;
use crate::js_type::JSType;
use crate::js_type_registry::JSTypeRegistry;
use crate::proxy_object_type::ProxyObjectType;
use crate::template_type::TemplateType;
use crate::templatized_type::TemplatizedType;
use crate::union_type::UnionType;
use crate::visitor::WithDefaultCase;
use closure_rhino::node::Ast;
use indexmap::IndexSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Result {
    PRESENT,
    ABSENT,
    CYCLE,
}

pub struct ContainsUpperBoundSuperTypeVisitor {
    target: Option<TypeId>,
    seen: IndexSet<TypeId>,
}

impl ContainsUpperBoundSuperTypeVisitor {
    // port: ContainsUpperBoundSuperTypeVisitor#ContainsUpperBoundSuperTypeVisitor
    pub fn new(target: Option<TypeId>) -> Self {
        Self {
            target,
            seen: IndexSet::new(),
        }
    }
    // port: ContainsUpperBoundSuperTypeVisitor#caseForwardingType
    fn case_forwarding_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
        reference: TypeId,
    ) -> Result {
        if Some(type_) == self.target {
            Result::PRESENT
        } else if self.seen.contains(&type_) {
            Result::CYCLE
        } else {
            self.seen.insert(type_);
            reference.visit(reg, ast, self)
        }
    }
}
impl WithDefaultCase<Result> for ContainsUpperBoundSuperTypeVisitor {
    // port: ContainsUpperBoundSuperTypeVisitor#caseDefault
    fn case_default(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: Option<TypeId>,
    ) -> Result {
        if self.target.is_some() && self.target == type_ {
            Result::PRESENT
        } else {
            Result::ABSENT
        }
    }
    // port: ContainsUpperBoundSuperTypeVisitor#caseTemplateType
    fn case_template_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> Result {
        let reference = type_.get_bound(reg);
        self.case_forwarding_type(reg, ast, type_, reference)
    }
    // port: ContainsUpperBoundSuperTypeVisitor#caseNamedType
    fn case_named_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> Result {
        let reference = type_.get_referenced_type_internal(reg);
        self.case_forwarding_type(reg, ast, type_, reference)
    }
    // port: ContainsUpperBoundSuperTypeVisitor#caseTemplatizedType
    fn case_templatized_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> Result {
        let reference = type_.get_referenced_type(reg);
        self.case_forwarding_type(reg, ast, type_, reference)
    }
    // port: ContainsUpperBoundSuperTypeVisitor#caseUnionType
    fn case_union_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> Result {
        if Some(type_) == self.target {
            return Result::PRESENT;
        }
        for alt in type_.get_alternates(reg, ast).iter().copied() {
            let found_in_alt = alt.visit(reg, ast, self);
            if found_in_alt != Result::ABSENT {
                return found_in_alt;
            }
        }
        Result::ABSENT
    }
}
