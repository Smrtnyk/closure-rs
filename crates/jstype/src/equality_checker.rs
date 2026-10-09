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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/jstype/EqualityChecker.java.

use crate::{
    JSTypeRegistry, TypeId,
    function_type::{FunctionType, Kind},
    js_type::{JSType, JSTypeKind, MatchStatus},
    object_type::ObjectType,
    proxy_object_type::ProxyObjectType,
    template_type_map::TemplateTypeMap,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{check_state, js_string::JsString, node::Ast};

const POTENTIALLY_CYCLIC_RECURSION_DEPTH: usize = 20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EqMethod {
    IDENTITY,
    DATA_FLOW,
}
pub struct EqualityChecker {
    eq_method: Option<EqMethod>,
    eq_cache: Option<IndexMap<CacheKey, MatchStatus>>,
    recursion_depth: usize,
    has_run: bool,
}
impl Default for EqualityChecker {
    fn default() -> Self {
        Self::new()
    }
}
impl EqualityChecker {
    // port: EqualityChecker#EqualityChecker
    pub fn new() -> Self {
        Self {
            eq_method: None,
            eq_cache: None,
            recursion_depth: 0,
            has_run: false,
        }
    }
    // port: EqualityChecker#setEqMethod
    pub fn set_eq_method(mut self, x: EqMethod) -> Self {
        self.check_has_not_run();
        check_state!(self.eq_method.is_none());
        self.eq_method = Some(x);
        self
    }
    // port: EqualityChecker#checkHasNotRun
    fn check_has_not_run(&self) {
        check_state!(!self.has_run);
    }
    // port: EqualityChecker#check
    pub fn check(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: impl Into<Option<TypeId>>,
        right: impl Into<Option<TypeId>>,
    ) -> bool {
        self.check_has_not_run();
        self.has_run = true;
        self.are_equal_caching(reg, ast, left.into(), right.into())
    }
    // port: EqualityChecker#checkParameters
    pub fn check_parameters(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: TypeId,
        right: TypeId,
    ) -> bool {
        self.check_has_not_run();
        self.has_run = true;
        left == right || self.are_arrow_parameter_equal(reg, ast, left, right)
    }
    // port: EqualityChecker#areEqualCaching
    fn are_equal_caching(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: Option<TypeId>,
        right: Option<TypeId>,
    ) -> bool {
        if self.recursion_depth > POTENTIALLY_CYCLIC_RECURSION_DEPTH && self.eq_cache.is_none() {
            self.eq_cache = Some(IndexMap::<_, _>::default());
        }
        if self.eq_cache.is_none() {
            self.recursion_depth += 1;
            let result = self.are_equal_internal(reg, ast, left, right);
            self.recursion_depth -= 1;
            return result;
        }
        let key = CacheKey::new(left, right);
        let cached = self.eq_cache.as_mut().unwrap().get(&key).copied();
        if let Some(cached) = cached {
            if cached == MatchStatus::PROCESSING {
                self.eq_cache
                    .as_mut()
                    .unwrap()
                    .insert(key, MatchStatus::MATCH);
                return true;
            }
            return cached.subtype_value();
        }
        self.eq_cache
            .as_mut()
            .unwrap()
            .insert(key, MatchStatus::PROCESSING);
        let result = self.are_equal_internal(reg, ast, left, right);
        self.eq_cache
            .as_mut()
            .unwrap()
            .insert(key, MatchStatus::value_of(result));
        result
    }
    // port: EqualityChecker#areEqualInternal
    fn are_equal_internal(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: Option<TypeId>,
        right: Option<TypeId>,
    ) -> bool {
        if left == right {
            return true;
        }
        let (Some(left), Some(right)) = (left, right) else {
            return false;
        };
        if left.is_no_resolved_type(reg) && right.is_no_resolved_type(reg) {
            return left.to_object_type(reg).unwrap().get_reference_name(reg)
                == right.to_object_type(reg).unwrap().get_reference_name(reg);
        }
        let left_unknown = left.is_unknown_type(reg, ast);
        let right_unknown = right.is_unknown_type(reg, ast);
        if left_unknown || right_unknown {
            if self.eq_method == Some(EqMethod::DATA_FLOW) {
                return left_unknown && right_unknown;
            } else if left_unknown
                && right_unknown
                && (left.is_nominal_type(reg) ^ right.is_nominal_type(reg))
            {
                return false;
            }
        }
        if let (Some(l), Some(r)) = (
            left.to_maybe_union_type(reg),
            right.to_maybe_union_type(reg),
        ) {
            return self.are_union_equal(reg, ast, l, r);
        } else if let Some(l) = left.to_maybe_union_type(reg) {
            let alts = crate::union_type::get_alternates(l, reg, ast);
            if alts.len() == 1 {
                return self.are_equal_internal(reg, ast, Some(alts[0]), Some(right));
            }
        } else if let Some(r) = right.to_maybe_union_type(reg) {
            let alts = crate::union_type::get_alternates(r, reg, ast);
            if alts.len() == 1 {
                return self.are_equal_internal(reg, ast, Some(left), Some(alts[0]));
            }
        }
        if let (Some(l), Some(r)) = (
            left.to_maybe_function_type(reg),
            right.to_maybe_function_type(reg),
        ) {
            return self.are_function_equal(reg, ast, l, r);
        }
        if matches!(reg.data(left).kind, JSTypeKind::Arrow(_))
            && matches!(reg.data(right).kind, JSTypeKind::Arrow(_))
        {
            return self.are_arrow_equal(reg, ast, left, right);
        }
        let lm = left.get_template_type_map(reg);
        let rm = right.get_template_type_map(reg);
        if !self.are_type_map_equal(reg, ast, &lm, &rm) {
            return false;
        }
        if let (Some(l), Some(r)) = (
            left.to_maybe_record_type(reg),
            right.to_maybe_record_type(reg),
        ) {
            return self.are_record_equal(reg, ast, l, r);
        }
        if left.is_nominal_type(reg) && right.is_nominal_type(reg) {
            let l = unwrap_nominal_type_proxies(left.to_object_type(reg).unwrap(), reg);
            let r = unwrap_nominal_type_proxies(right.to_object_type(reg).unwrap(), reg);
            check_state!(l.is_nominal_type(reg) && r.is_nominal_type(reg));
            if left.is_resolved(reg) && right.is_resolved(reg) {
                return l == r;
            }
            let ln = l.get_reference_name(reg).expect("NullPointerException");
            let rn = r.get_reference_name(reg).expect("NullPointerException");
            return ln == rn && get_goog_module_id(l, reg) == get_goog_module_id(r, reg);
        }
        if is_proxy(left, reg) && !matches!(reg.data(left).kind, JSTypeKind::Template(_)) {
            return self.are_equal_caching(
                reg,
                ast,
                Some(left.get_referenced_type_internal(reg)),
                Some(right),
            );
        }
        if is_proxy(right, reg) && !matches!(reg.data(right).kind, JSTypeKind::Template(_)) {
            return self.are_equal_caching(
                reg,
                ast,
                Some(left),
                Some(right.get_referenced_type_internal(reg)),
            );
        }
        false
    }
    // port: EqualityChecker#areUnionEqual
    fn are_union_equal(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: TypeId,
        right: TypeId,
    ) -> bool {
        let la = crate::union_type::get_alternates(left, reg, ast);
        let ra = crate::union_type::get_alternates(right, reg, ast);
        if self.eq_method == Some(EqMethod::IDENTITY) && la.len() != ra.len() {
            return false;
        }
        'outer: for &r in ra.iter() {
            for &l in la.iter() {
                if self.are_equal_caching(reg, ast, Some(l), Some(r)) {
                    continue 'outer;
                }
            }
            return false;
        }
        true
    }
    // port: EqualityChecker#areFunctionEqual
    fn are_function_equal(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: TypeId,
        right: TypeId,
    ) -> bool {
        if left == right {
            return true;
        }
        let lk = reg.function_data(left).kind;
        let rk = reg.function_data(right).kind;
        if lk != rk {
            return false;
        }
        match lk {
            Kind::CONSTRUCTOR | Kind::INTERFACE => false,
            Kind::ORDINARY => {
                let lt = left.get_type_of_this(reg);
                let rt = right.get_type_of_this(reg);
                let la = left.get_internal_arrow_type(reg);
                let ra = right.get_internal_arrow_type(reg);
                self.are_equal_caching(reg, ast, lt, rt)
                    && self.are_equal_caching(reg, ast, Some(la), Some(ra))
                    && left.get_closure_primitive(reg) == right.get_closure_primitive(reg)
            }
            _ => panic!("AssertionError"),
        }
    }
    // port: EqualityChecker#areArrowEqual
    fn are_arrow_equal(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: TypeId,
        right: TypeId,
    ) -> bool {
        let l = crate::arrow_type::get_return_type(left, reg);
        let r = crate::arrow_type::get_return_type(right, reg);
        self.are_equal_caching(reg, ast, Some(l), Some(r))
            && self.are_arrow_parameter_equal(reg, ast, left, right)
    }
    // port: EqualityChecker#areArrowParameterEqual
    fn are_arrow_parameter_equal(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: TypeId,
        right: TypeId,
    ) -> bool {
        let lp = crate::arrow_type::get_parameter_list(left, reg);
        let rp = crate::arrow_type::get_parameter_list(right, reg);
        if lp.len() != rp.len() {
            return false;
        }
        for (l, r) in lp.iter().zip(rp) {
            if !self.are_equal_caching(reg, ast, Some(l.get_jstype()), Some(r.get_jstype()))
                || l.is_optional() != r.is_optional()
                || l.is_variadic() != r.is_variadic()
            {
                return false;
            }
        }
        true
    }
    // port: EqualityChecker#areRecordEqual
    fn are_record_equal(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: TypeId,
        right: TypeId,
    ) -> bool {
        let lk = left.get_own_property_names(reg);
        let rk = right.get_own_property_names(reg);
        if lk.len() != rk.len() || !lk.iter().all(|k| rk.contains(k)) {
            return false;
        }
        for key in lk {
            let l = left.get_property_type(reg, ast, &key);
            let r = right.get_property_type(reg, ast, &key);
            if !self.are_equal_caching(reg, ast, Some(l), Some(r)) {
                return false;
            }
        }
        true
    }
    // port: EqualityChecker#areTypeMapEqual
    fn are_type_map_equal(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        left: &TemplateTypeMap,
        right: &TemplateTypeMap,
    ) -> bool {
        let lk = left.get_template_keys();
        let lv = left.get_template_values();
        let rk = right.get_template_keys();
        let rv = right.get_template_values();
        'outer: for (i, &l) in lk.iter().enumerate() {
            for (j, &r) in rk.iter().enumerate() {
                if l != r {
                    continue;
                }
                let l = lv
                    .get(i)
                    .copied()
                    .unwrap_or_else(|| left.default_value_type(reg, ast, l));
                let r = rv
                    .get(j)
                    .copied()
                    .unwrap_or_else(|| right.default_value_type(reg, ast, r));
                if self.are_equal_caching(reg, ast, Some(l), Some(r)) {
                    continue 'outer;
                }
            }
            return false;
        }
        true
    }
}
#[derive(Clone, Copy)]
struct CacheKey {
    left: Option<TypeId>,
    right: Option<TypeId>,
    hash_code: i32,
}
impl CacheKey {
    // port: EqualityChecker.CacheKey#CacheKey
    fn new(left: Option<TypeId>, right: Option<TypeId>) -> Self {
        Self {
            left,
            right,
            hash_code: left.map_or(0, |t| t.0 as i32) ^ right.map_or(0, |t| t.0 as i32),
        }
    }
}
impl std::hash::Hash for CacheKey {
    // port: EqualityChecker.CacheKey#hashCode
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash_code.hash(state);
    }
}
impl PartialEq for CacheKey {
    // port: EqualityChecker.CacheKey#equals
    fn eq(&self, other: &Self) -> bool {
        (self.left == other.left && self.right == other.right)
            || (self.left == other.right && self.right == other.left)
    }
}
impl Eq for CacheKey {}
// port: EqualityChecker#getGoogModuleId
fn get_goog_module_id(t: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    if matches!(
        reg.data(t).kind,
        JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
    ) {
        return t.get_goog_module_id(reg);
    }
    t.get_constructor(reg)
        .and_then(|c| c.get_goog_module_id(reg))
}
// port: EqualityChecker#unwrapNominalTypeProxies
fn unwrap_nominal_type_proxies(obj_type: TypeId, reg: &JSTypeRegistry) -> TypeId {
    if !obj_type.is_resolved(reg)
        || (!obj_type.is_named_type(reg) && !obj_type.is_templatized_type(reg))
    {
        obj_type
    } else {
        unwrap_nominal_type_proxies(
            obj_type
                .get_referenced_obj_type_internal(reg)
                .expect("NullPointerException"),
            reg,
        )
    }
}
pub(crate) fn is_proxy(t: TypeId, reg: &JSTypeRegistry) -> bool {
    matches!(
        reg.data(t).kind,
        JSTypeKind::ProxyObject(_)
            | JSTypeKind::Named(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_)
    )
}
