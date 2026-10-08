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
//   src/com/google/javascript/rhino/jstype/SubtypeChecker.java.

use crate::{
    JSTypeNative, JSTypeRegistry, TypeId,
    enum_element_type::EnumElementType,
    function_type::FunctionType,
    js_type::{JSType, JSTypeKind, MatchStatus, SubtypingMode},
    object_type::ObjectType,
    property::PropertyKey,
    proxy_object_type::ProxyObjectType,
    template_type::TemplateType,
    templatized_type::TemplatizedType,
};
use closure_rhino::{check_state, node::Ast};

const POTENTIALLY_CYCLIC_RECURSION_DEPTH: usize = 20;
const BIVARIANT_TYPES: [&str; 3] = ["Object", "IArrayLike", "Array"];
pub struct SubtypeChecker {
    initial_supertype: Option<TypeId>,
    initial_subtype: Option<TypeId>,
    is_using_structural_typing: Option<bool>,
    subtyping_mode: Option<SubtypingMode>,
    subtype_cache: Option<Vec<(CacheKey, MatchStatus)>>,
    has_run: bool,
    recursion_depth: usize,
}
impl Default for SubtypeChecker {
    fn default() -> Self {
        Self::new()
    }
}
impl SubtypeChecker {
    // port: SubtypeChecker#SubtypeChecker
    pub fn new() -> Self {
        Self {
            initial_supertype: None,
            initial_subtype: None,
            is_using_structural_typing: None,
            subtyping_mode: None,
            subtype_cache: None,
            has_run: false,
            recursion_depth: 0,
        }
    }
    // port: SubtypeChecker#setSupertype
    pub fn set_supertype(mut self, value: TypeId) -> Self {
        self.check_has_not_run();
        check_state!(self.initial_supertype.is_none());
        self.initial_supertype = Some(value);
        self
    }
    // port: SubtypeChecker#setSubtype
    pub fn set_subtype(mut self, value: TypeId) -> Self {
        self.check_has_not_run();
        check_state!(self.initial_subtype.is_none());
        self.initial_subtype = Some(value);
        self
    }
    // port: SubtypeChecker#setUsingStructuralSubtyping
    pub fn set_using_structural_subtyping(mut self, value: bool) -> Self {
        self.check_has_not_run();
        check_state!(self.is_using_structural_typing.is_none());
        self.is_using_structural_typing = Some(value);
        self
    }
    // port: SubtypeChecker#setSubtypingMode
    pub fn set_subtyping_mode(mut self, value: SubtypingMode) -> Self {
        self.check_has_not_run();
        check_state!(self.subtyping_mode.is_none());
        self.subtyping_mode = Some(value);
        self
    }
    // port: SubtypeChecker#checkHasNotRun
    fn check_has_not_run(&self) {
        check_state!(!self.has_run);
    }
    // port: SubtypeChecker#check
    pub fn check(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        self.check_has_not_run();
        self.has_run = true;
        self.is_subtype_caching(
            reg,
            ast,
            self.initial_subtype.expect("NullPointerException"),
            self.initial_supertype.expect("NullPointerException"),
        )
    }
    // port: SubtypeChecker#isSubtypeCaching
    fn is_subtype_caching(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        if self.recursion_depth > POTENTIALLY_CYCLIC_RECURSION_DEPTH && self.subtype_cache.is_none()
        {
            self.subtype_cache = Some(Vec::new());
        }
        if self.subtype_cache.is_none() {
            self.recursion_depth += 1;
            let result = self.is_subtype_dispatching(reg, ast, subtype, supertype);
            self.recursion_depth -= 1;
            return result;
        }
        let key = CacheKey::new(reg, subtype, supertype);
        let mut position = None;
        for (i, (old, _)) in self.subtype_cache.as_ref().unwrap().iter().enumerate() {
            if old.hash_code() == key.hash_code() && old.equals(reg, ast, &key) {
                position = Some(i);
                break;
            }
        }
        if let Some(position) = position {
            let cached = self.subtype_cache.as_ref().unwrap()[position].1;
            if cached == MatchStatus::PROCESSING {
                self.subtype_cache.as_mut().unwrap()[position].1 = MatchStatus::MATCH;
                return true;
            }
            return cached.subtype_value();
        }
        let position = self.subtype_cache.as_ref().unwrap().len();
        self.subtype_cache
            .as_mut()
            .unwrap()
            .push((key, MatchStatus::PROCESSING));
        let result = self.is_subtype_dispatching(reg, ast, subtype, supertype);
        self.subtype_cache.as_mut().unwrap()[position].1 = MatchStatus::value_of(result);
        result
    }
    // port: SubtypeChecker#isSubtypeDispatching
    fn is_subtype_dispatching(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        match subtype.get_type_class(reg) {
            crate::js_type_class::JSTypeClass::ARROW => {
                self.is_arrow_type_subtype(reg, ast, subtype, supertype)
            }
            crate::js_type_class::JSTypeClass::ENUM_ELEMENT => {
                self.is_enum_element_subtype(reg, ast, subtype, supertype)
            }
            crate::js_type_class::JSTypeClass::ENUM => {
                self.is_enum_subtype(reg, ast, subtype, supertype)
            }
            crate::js_type_class::JSTypeClass::NO_OBJECT
            | crate::js_type_class::JSTypeClass::NO => {
                self.is_various_bottoms_subtype(reg, ast, subtype, supertype)
            }
            crate::js_type_class::JSTypeClass::FUNCTION => {
                self.is_function_subtype(reg, ast, subtype, supertype)
            }
            crate::js_type_class::JSTypeClass::TEMPLATE => {
                self.is_template_subtype(reg, ast, subtype, supertype)
            }
            crate::js_type_class::JSTypeClass::PROXY_OBJECT => {
                self.is_proxy_object_subtype(reg, ast, subtype, supertype)
            }
            crate::js_type_class::JSTypeClass::WELL_KNOWN_SYMBOL => {
                self.is_well_known_symbol_subtype(reg, ast, subtype, supertype)
            }
            _ => self.is_subtype_helper(reg, ast, subtype, supertype),
        }
    }
    // port: SubtypeChecker#isWellKnownSymbolSubtype
    fn is_well_known_symbol_subtype(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        if supertype.is_known_symbol_value_type(reg) {
            return subtype.equals(reg, ast, supertype);
        }
        if supertype.is_symbol_value_type(reg) {
            return true;
        }
        self.is_subtype_helper(reg, ast, subtype, supertype)
    }
    // port: SubtypeChecker#isSubtypeHelper
    fn is_subtype_helper(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        if subtype == supertype
            || supertype.is_unknown_type(reg, ast)
            || supertype.is_all_type(reg)
            || subtype.is_unknown_type(reg, ast)
            || subtype.is_no_type(reg)
        {
            return true;
        }
        if self.subtyping_mode == Some(SubtypingMode::IGNORE_NULL_UNDEFINED)
            && (subtype.is_null_type(reg) || subtype.is_void_type(reg))
        {
            return true;
        }
        if subtype.equals(reg, ast, supertype) {
            return true;
        }
        if let Some(subtype) = subtype.to_maybe_named_type(reg) {
            let referenced = subtype.get_referenced_type_internal(reg);
            return self.is_subtype_dispatching(reg, ast, referenced, supertype);
        } else if let Some(supertype) = supertype.to_maybe_named_type(reg) {
            let referenced = supertype.get_referenced_type_internal(reg);
            return self.is_subtype_dispatching(reg, ast, subtype, referenced);
        }
        if let Some(union) = subtype.to_maybe_union_type(reg) {
            for sub in crate::union_type::get_alternates(union, reg, ast)
                .iter()
                .copied()
            {
                if !self.is_subtype_caching(reg, ast, sub, supertype) {
                    return false;
                }
            }
            return true;
        } else if let Some(union) = supertype.to_maybe_union_type(reg) {
            for sup in crate::union_type::get_alternates(union, reg, ast)
                .iter()
                .copied()
            {
                if self.is_subtype_caching(reg, ast, subtype, sup) {
                    return true;
                }
            }
            return false;
        }
        if !subtype.is_object_type(reg, ast) || !supertype.is_object_type(reg, ast) {
            return false;
        }
        let sub = subtype.assert_object_type(reg, ast);
        let sup = supertype.assert_object_type(reg, ast);
        self.is_object_subtype_helper(reg, ast, sub, sup)
    }
    // port: SubtypeChecker#isObjectSubtypeHelper
    fn is_object_subtype_helper(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        let subtype_params = subtype.get_template_type_map(reg);
        let supertype_params = supertype.get_template_type_map(reg);
        let mut sub_and_super_are_same_base_type = false;
        if let (Some(sub), Some(sup)) = (
            subtype.to_maybe_templatized_type(reg),
            supertype.to_maybe_templatized_type(reg),
        ) {
            let sr = sup.get_referenced_type(reg);
            let tr = sub.get_referenced_type(reg);
            if tr.equals(reg, ast, sr) {
                sub_and_super_are_same_base_type = true;
            }
        }
        let mut bivariant_match = false;
        if is_bivariant_type(supertype, reg) {
            let key = reg.get_object_element_key();
            let this_element = subtype_params.get_resolved_template_type(reg, ast, key);
            let that_element = supertype_params.get_resolved_template_type(reg, ast, key);
            if !self.meet_variance_constraint(
                reg,
                ast,
                Variance::BIVARIANT,
                this_element,
                that_element,
            ) {
                return false;
            }
            bivariant_match = true;
        }
        let covariant_key = if bivariant_match {
            None
        } else {
            get_template_key_if_covariant_type(supertype, reg)
        };
        if !sub_and_super_are_same_base_type
            && self
                .is_using_structural_typing
                .expect("NullPointerException")
            && supertype.is_structural_type(reg)
        {
            if covariant_key == Some(reg.get_readonly_array_element_key())
                && self.is_native_array_type(reg, ast, subtype)
            {
                let key = covariant_key.unwrap();
                let this_element = subtype_params.get_resolved_template_type(reg, ast, key);
                let that_element = supertype_params.get_resolved_template_type(reg, ast, key);
                return self.meet_variance_constraint(
                    reg,
                    ast,
                    Variance::COVARIANT,
                    this_element,
                    that_element,
                );
            }
            return self.is_structural_subtype_helper(
                reg,
                ast,
                subtype,
                supertype,
                PropertyOptionality::VOIDABLE_PROPS_ARE_OPTIONAL,
            );
        }
        if !bivariant_match {
            if let Some(key) = covariant_key {
                let this_element = subtype_params.get_resolved_template_type(reg, ast, key);
                let that_element = supertype_params.get_resolved_template_type(reg, ast, key);
                if !self.meet_variance_constraint(
                    reg,
                    ast,
                    Variance::COVARIANT,
                    this_element,
                    that_element,
                ) {
                    return false;
                }
            } else if !self.is_type_map_submap(reg, ast, subtype, supertype) {
                return false;
            }
        }
        let subtype_ctor = subtype.get_constructor(reg);
        let supertype_ctor = supertype.get_constructor(reg);
        if subtype_ctor.is_some_and(|c| c.is_interface(reg)) {
            for interface in subtype.get_ctor_extended_interfaces(reg, ast) {
                if self.is_subtype_caching(reg, ast, interface, supertype) {
                    return true;
                }
            }
        } else if supertype_ctor.is_some_and(|c| c.is_interface(reg)) {
            for interface in subtype.get_ctor_implemented_interfaces(reg, ast) {
                if self.is_subtype_caching(reg, ast, interface, supertype) {
                    return true;
                }
            }
        }
        supertype.is_implicit_prototype_of(reg, ast, subtype)
    }
    // port: SubtypeChecker#isStructuralSubtypeHelper
    fn is_structural_subtype_helper(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
        optionality: PropertyOptionality,
    ) -> bool {
        let (props, symbols) = if supertype.is_record_type(reg) {
            (
                supertype
                    .get_own_property_names(reg)
                    .into_iter()
                    .collect::<Vec<_>>(),
                supertype.get_own_property_known_symbols(reg),
            )
        } else {
            let keys = supertype.get_all_keys(reg, ast);
            (
                keys.string_keys.iter().cloned().collect(),
                keys.known_symbol_keys.as_ref().clone(),
            )
        };
        for property in props {
            if !self.check_property_presence(
                reg,
                ast,
                PropertyKey::String(property),
                subtype,
                supertype,
                optionality,
            ) {
                return false;
            }
        }
        for symbol in symbols {
            if !self.check_property_presence(
                reg,
                ast,
                PropertyKey::Symbol(symbol),
                subtype,
                supertype,
                optionality,
            ) {
                return false;
            }
        }
        true
    }
    // port: SubtypeChecker#checkPropertyPresence
    fn check_property_presence(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_key: PropertyKey,
        subtype: TypeId,
        supertype: TypeId,
        optionality: PropertyOptionality,
    ) -> bool {
        let supertype_prop = supertype.get_property_type(reg, ast, property_key.clone());
        if subtype.has_property(reg, ast, property_key.clone()) {
            let subtype_prop = subtype.get_property_type(reg, ast, property_key);
            if !self.is_subtype_caching(reg, ast, subtype_prop, supertype_prop) {
                return false;
            }
        } else if !optionality.is_optional(reg, ast, supertype_prop) {
            return false;
        }
        true
    }
    // port: SubtypeChecker#isNativeArrayType
    fn is_native_array_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        if let Some(unwrapped) = get_object_type_if_native(type_, reg) {
            let native = reg.get_native_type(JSTypeNative::ARRAY_TYPE);
            unwrapped.equals(reg, ast, native)
        } else {
            false
        }
    }
    // port: SubtypeChecker#isFunctionSubtype
    fn is_function_subtype(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        non_function_supertype: TypeId,
    ) -> bool {
        if self.is_subtype_helper(reg, ast, subtype, non_function_supertype) {
            return true;
        }
        if let Some(supertype) = non_function_supertype.to_maybe_function_type(reg) {
            if supertype.is_interface(reg) {
                return true;
            }
            if subtype.is_interface(reg) {
                return false;
            }
            return self.should_treat_this_types_as_covariant(reg, ast, subtype, supertype)
                && self.is_subtype_caching(
                    reg,
                    ast,
                    subtype.get_internal_arrow_type(reg),
                    supertype.get_internal_arrow_type(reg),
                );
        }
        let native = reg.get_native_type(JSTypeNative::FUNCTION_PROTOTYPE);
        self.is_subtype_caching(reg, ast, native, non_function_supertype)
    }
    // port: SubtypeChecker#isVariousBottomsSubtype
    fn is_various_bottoms_subtype(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        self.is_subtype_helper(reg, ast, subtype, supertype)
            || (supertype.is_object(reg, ast) && !supertype.is_no_type(reg))
    }
    // port: SubtypeChecker#isArrowTypeSubtype
    fn is_arrow_type_subtype(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        non_arrow_supertype: TypeId,
    ) -> bool {
        if !matches!(reg.data(non_arrow_supertype).kind, JSTypeKind::Arrow(_)) {
            return false;
        }
        let supertype = non_arrow_supertype;
        let subreturn = crate::arrow_type::get_return_type(subtype, reg);
        let supreturn = crate::arrow_type::get_return_type(supertype, reg);
        if !self.is_subtype_caching(reg, ast, subreturn, supreturn) {
            return false;
        }
        let sp = crate::arrow_type::get_parameter_list(subtype, reg);
        let tp = crate::arrow_type::get_parameter_list(supertype, reg);
        let mut si = sp.iter();
        let mut ti = tp.iter();
        let mut subtype_param = si.next();
        let mut supertype_param = ti.next();
        while let (Some(sub), Some(sup)) = (subtype_param, supertype_param) {
            let subtyp = sub.get_jstype();
            let suptyp = sup.get_jstype();
            if !self.is_subtype_caching(reg, ast, suptyp, subtyp) {
                return false;
            }
            let this_is_var_args = sub.is_variadic();
            let that_is_var_args = sup.is_variadic();
            let this_is_optional = this_is_var_args || sub.is_optional();
            let that_is_optional = that_is_var_args || sup.is_optional();
            if !this_is_optional && that_is_optional {
                let is_top_function = that_is_var_args
                    && (suptyp.is_unknown_type(reg, ast) || suptyp.is_no_type(reg));
                if !is_top_function {
                    return false;
                }
            }
            if !this_is_var_args {
                subtype_param = si.next();
            }
            if !that_is_var_args {
                supertype_param = ti.next();
            }
            if this_is_var_args && that_is_var_args {
                break;
            }
        }
        subtype_param.is_none_or(|p| p.is_optional() || p.is_variadic())
            || supertype_param.is_some()
    }
    // port: SubtypeChecker#isEnumSubtype
    fn is_enum_subtype(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        let obj = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
        let proto = reg.get_native_type(JSTypeNative::OBJECT_PROTOTYPE);
        supertype.equals(reg, ast, obj)
            || subtype.equals(reg, ast, proto)
            || self.is_subtype_helper(reg, ast, subtype, supertype)
    }
    // port: SubtypeChecker#isEnumElementSubtype
    #[allow(clippy::collapsible_if)] // Preserve the Java enum identity guard.
    fn is_enum_element_subtype(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        if let Some(sup) = supertype.to_maybe_enum_element_type(reg) {
            if subtype.get_enum_type(reg) == sup.get_enum_type(reg) {
                return self.is_subtype_caching(
                    reg,
                    ast,
                    subtype.get_primitive_type(reg),
                    sup.get_primitive_type(reg),
                );
            }
        }
        if self.is_subtype_helper(reg, ast, subtype, supertype) {
            true
        } else {
            let primitive = subtype.get_primitive_type(reg);
            self.is_subtype_caching(reg, ast, primitive, supertype)
        }
    }
    // port: SubtypeChecker#isProxyObjectSubtype
    fn is_proxy_object_subtype(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        let referenced = subtype.get_referenced_type_internal(reg);
        self.is_subtype_dispatching(reg, ast, referenced, supertype)
    }
    // port: SubtypeChecker#isTemplateSubtype
    fn is_template_subtype(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        let sub_bound = subtype.get_bound(reg);
        if !sub_bound.is_unknown_type(reg, ast) && supertype.is_template_type(reg) {
            let sup = supertype.to_maybe_template_type(reg).unwrap();
            let sup_bound = sup.get_bound(reg);
            if !sup_bound.is_unknown_type(reg, ast) {
                let mut visitor=crate::contains_upper_bound_super_type_visitor::ContainsUpperBoundSuperTypeVisitor::new(Some(supertype));
                return subtype.visit(reg, ast, &mut visitor)
                    == crate::contains_upper_bound_super_type_visitor::Result::PRESENT;
            }
        }
        self.is_proxy_object_subtype(reg, ast, subtype, supertype)
    }
    // port: SubtypeChecker#isTypeMapSubmap
    fn is_type_map_submap(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        if subtype.is_function_prototype_type(reg) {
            return true;
        }
        let submap = subtype.get_template_type_map(reg);
        let supermap = supertype.get_template_type_map(reg);
        for &key in supermap.get_template_keys() {
            let key_count =
                submap.get_template_key_count_this_should_always_be_one_or_zero_but_isnt(key);
            if key_count == 0 {
                if subtype.loosen_typechecking_due_to_forward_referenced_supertype(reg) {
                    continue;
                }
                return false;
            } else if key_count > 1 {
                continue;
            }
            let subvalue = submap.get_resolved_template_type(reg, ast, key);
            let supervalue = supermap.get_resolved_template_type(reg, ast, key);
            if !self.meet_variance_constraint(reg, ast, Variance::INVARIANT, subvalue, supervalue) {
                return false;
            }
        }
        true
    }
    // port: SubtypeChecker#meetVarianceConstraint
    fn meet_variance_constraint(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        variance: Variance,
        override_: TypeId,
        reference: TypeId,
    ) -> bool {
        match variance {
            Variance::COVARIANT => self.is_subtype_caching(reg, ast, override_, reference),
            Variance::CONTRAVARIANT => self.is_subtype_caching(reg, ast, reference, override_),
            Variance::BIVARIANT => {
                self.meet_variance_constraint(reg, ast, Variance::COVARIANT, reference, override_)
                    || self.meet_variance_constraint(
                        reg,
                        ast,
                        Variance::CONTRAVARIANT,
                        reference,
                        override_,
                    )
            }
            Variance::INVARIANT => {
                self.meet_variance_constraint(reg, ast, Variance::COVARIANT, reference, override_)
                    && self.meet_variance_constraint(
                        reg,
                        ast,
                        Variance::CONTRAVARIANT,
                        reference,
                        override_,
                    )
            }
        }
    }
    // port: SubtypeChecker#shouldTreatThisTypesAsCovariant
    fn should_treat_this_types_as_covariant(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) -> bool {
        let st = supertype
            .get_type_of_this(reg)
            .expect("NullPointerException");
        let tt = subtype.get_type_of_this(reg).expect("NullPointerException");
        if st
            .to_object_type(reg)
            .and_then(|t| t.get_constructor(reg))
            .is_some_and(|c| c.is_interface(reg))
        {
            return true;
        }
        self.hack_temporarily_change_subtyping_mode(SubtypingMode::NORMAL, |this| {
            this.is_subtype_caching(reg, ast, st, tt) || this.is_subtype_caching(reg, ast, tt, st)
        })
    }
    // port: SubtypeChecker#hackTemporarilyChangeSubtypingMode
    fn hack_temporarily_change_subtyping_mode(
        &mut self,
        temp_mode: SubtypingMode,
        callback: impl FnOnce(&mut Self) -> bool,
    ) -> bool {
        let original_mode = self.subtyping_mode;
        self.subtyping_mode = Some(temp_mode);
        let result = callback(self);
        self.subtyping_mode = original_mode;
        result
    }
}
// port: SubtypeChecker#isBivariantType
fn is_bivariant_type(type_: TypeId, reg: &JSTypeRegistry) -> bool {
    get_object_type_if_native(type_, reg)
        .and_then(|t| t.get_reference_name(reg))
        .is_some_and(|n| BIVARIANT_TYPES.iter().any(|&b| n == b))
}
// port: SubtypeChecker#getTemplateKeyIfCovariantType
pub fn get_template_key_if_covariant_type(type_: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    if let Some(t) = type_.to_maybe_templatized_type(reg) {
        let key = reg.get_i_thenable_template();
        if t.get_template_type_map(reg).has_template_key(key) {
            return Some(key);
        }
    }
    let unwrapped = get_object_type_if_native(type_, reg)?;
    let name = unwrapped.get_reference_name(reg)?;
    match name.to_string_lossy().as_str() {
        "ReadonlyArray" => Some(reg.get_readonly_array_element_key()),
        "IteratorLike" => Some(reg.get_iterator_like_value_template()),
        "Iterator" => Some(reg.get_iterator_value_template()),
        "Generator" => Some(reg.get_generator_value_template()),
        "AsyncIterator" => Some(reg.get_async_iterator_value_template()),
        "Iterable" => Some(reg.get_iterable_value_template()),
        "IteratorIterable" => Some(reg.get_iterator_iterable_value_template()),
        "IIterableResult" => Some(reg.get_i_iterable_result_value_template()),
        "AsyncIterable" => Some(reg.get_async_iterable_value_template()),
        _ => None,
    }
}
// port: SubtypeChecker#getObjectTypeIfNative
fn get_object_type_if_native(type_: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    let obj = type_.to_object_type(reg)?;
    let unwrapped = crate::object_type::deeply_unwrap(obj, reg)?;
    unwrapped.is_native_object_type(reg).then_some(unwrapped)
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PropertyOptionality {
    VOIDABLE_PROPS_ARE_OPTIONAL,
    ALL_PROPS_ARE_REQUIRED,
}
impl PropertyOptionality {
    // port: SubtypeChecker.PropertyOptionality#isOptional
    fn is_optional(self, reg: &mut JSTypeRegistry, ast: &Ast, prop_type: TypeId) -> bool {
        self == Self::VOIDABLE_PROPS_ARE_OPTIONAL && prop_type.is_explicitly_voidable(reg, ast)
    }
}
#[derive(Clone, Copy)]
enum Variance {
    COVARIANT,
    CONTRAVARIANT,
    BIVARIANT,
    INVARIANT,
}
struct CacheKey {
    left: TypeId,
    right: TypeId,
    hash_code: i32,
}
impl CacheKey {
    // port: SubtypeChecker.CacheKey#CacheKey
    fn new(reg: &JSTypeRegistry, left: TypeId, right: TypeId) -> Self {
        Self {
            left,
            right,
            hash_code: 31_i32
                .wrapping_mul(left.hash_code(reg))
                .wrapping_add(right.hash_code(reg)),
        }
    }
    // port: SubtypeChecker.CacheKey#hashCode
    fn hash_code(&self) -> i32 {
        self.hash_code
    }
    // port: SubtypeChecker.CacheKey#equals
    fn equals(&self, reg: &mut JSTypeRegistry, ast: &Ast, that: &Self) -> bool {
        (self.left == that.left && self.right == that.right)
            || (self.left.equals(reg, ast, that.left) && self.right.equals(reg, ast, that.right))
    }
}
