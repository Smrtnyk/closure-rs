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
//   src/com/google/javascript/rhino/jstype/UnionType.java.

use crate::{
    JSTypeNative, JSTypeRegistry, TypeId,
    boolean_literal_set::BooleanLiteralSet,
    function_type::FunctionType,
    js_type::{HasPropertyKind, JSType, JSTypeKind, TypePair},
    property::PropertyKey,
    proxy_object_type::ProxyObjectType,
    templatized_type::TemplatizedType,
    type_string_builder::TypeStringBuilder,
};
use closure_rhino::{
    check_state, error_reporter::ErrorReporter, js_string::JsString, jscomp_base::Tri, node::Ast,
    outcome::Outcome,
};
use std::sync::Arc;

const MAX_UNION_SIZE: usize = 30;
pub(crate) struct UnionTypeData {
    pub(crate) alternates: Arc<Vec<TypeId>>,
    pub(crate) alternates_resolved_before_build: bool,
    pub(crate) resolved_alternates_at_last_build: i32,
}
pub struct UnionTypeBuilder {
    rebuild_target: Option<TypeId>,
    alternates: Vec<TypeId>,
    final_alternates: Option<Arc<Vec<TypeId>>>,
    contains_void_type: bool,
    is_all_type: bool,
    is_native_unknown_type: bool,
    are_all_unknowns_checked: bool,
    alternates_resolved_before_build: bool,
    function_type_position: i32,
}
pub type Builder = UnionTypeBuilder;
impl Default for UnionTypeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
// port: UnionType#builder
pub fn builder() -> UnionTypeBuilder {
    UnionTypeBuilder::new()
}
impl UnionTypeBuilder {
    // port: UnionType.Builder#Builder
    pub fn new() -> Self {
        Self {
            rebuild_target: None,
            alternates: Vec::new(),
            final_alternates: None,
            contains_void_type: false,
            is_all_type: false,
            is_native_unknown_type: false,
            are_all_unknowns_checked: true,
            alternates_resolved_before_build: true,
            function_type_position: -1,
        }
    }
    // port: UnionType.Builder#Builder
    pub fn for_rebuild(rebuild_target: TypeId) -> Self {
        Self {
            rebuild_target: Some(rebuild_target),
            ..Self::new()
        }
    }
    // port: UnionType.Builder#isSubtype
    fn is_subtype(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        right_type: TypeId,
        left_type: TypeId,
    ) -> bool {
        right_type.is_subtype_without_structural_typing(reg, ast, left_type)
    }
    // port: UnionType.Builder#addAlternates
    pub fn add_alternates(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        list: &[TypeId],
    ) -> &mut Self {
        for &t in list {
            self.add_alternate(reg, ast, t);
        }
        self
    }
    // port: UnionType.Builder#addAlternate
    pub fn add_alternate(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        mut alternate: TypeId,
    ) -> &mut Self {
        self.check_has_not_built();
        if let Some(union) = alternate.to_maybe_union_type(reg) {
            let alternates = get_alternates(union, reg, ast);
            self.add_alternates(reg, ast, &alternates);
            return self;
        }
        if self.alternates.len() > MAX_UNION_SIZE {
            return self;
        }
        if !alternate.is_resolved(reg) {
            self.alternates_resolved_before_build = false;
            if self.alternates.contains(&alternate) {
                return self;
            }
            self.alternates.push(alternate);
            return self;
        }
        if alternate.is_no_type(reg) {
            return self;
        }
        self.is_all_type |= alternate.is_all_type(reg);
        self.contains_void_type |= alternate.is_void_type(reg);
        let is_alternate_unknown = matches!(reg.data(alternate).kind, JSTypeKind::Unknown(_));
        self.is_native_unknown_type |= is_alternate_unknown;
        if is_alternate_unknown {
            self.are_all_unknowns_checked &= alternate.is_checked_unknown_type(reg);
        }
        if self.is_all_type || self.is_native_unknown_type {
            return self;
        }
        if alternate.is_function_type(reg) && self.function_type_position != -1 {
            let other = self.alternates[self.function_type_position as usize]
                .to_maybe_function_type(reg)
                .unwrap();
            let supremum = alternate
                .to_maybe_function_type(reg)
                .unwrap()
                .sup_and_inf_helper(reg, ast, other, true);
            self.alternates[self.function_type_position as usize] = supremum;
            return self;
        }
        let mut index = 0;
        while index < self.alternates.len() {
            let mut remove_current = false;
            let current = self.alternates[index];
            if !current.is_resolved(reg) {
                index += 1;
                continue;
            }
            if alternate.is_unknown_type(reg, ast)
                || current.is_unknown_type(reg, ast)
                || alternate.has_any_template_types(reg, ast)
                || current.has_any_template_types(reg, ast)
            {
                if alternate.equals(reg, ast, current) {
                    return self;
                }
            } else if alternate.is_templatized_type(reg) || current.is_templatized_type(reg) {
                if !current.is_templatized_type(reg) {
                    if Self::is_subtype(reg, ast, alternate, current) {
                        return self;
                    }
                } else if !alternate.is_templatized_type(reg) {
                    if Self::is_subtype(reg, ast, current, alternate) {
                        remove_current = true;
                    }
                } else {
                    let ta = alternate.to_maybe_templatized_type(reg).unwrap();
                    let tc = current.to_maybe_templatized_type(reg).unwrap();
                    if tc.wraps_same_raw_type(reg, ast, ta) {
                        if current.equals(reg, ast, alternate) {
                            return self;
                        } else if !tc.get_referenced_type(reg).is_readonly_array_type(reg) {
                            let raw = tc.get_referenced_obj_type_internal(reg).unwrap();
                            alternate = reg.create_templatized_type(ast, raw, &[]);
                            remove_current = true;
                        }
                    }
                }
            } else if Self::is_subtype(reg, ast, alternate, current) {
                self.may_register_dropped_properties(reg, ast, alternate, current);
                return self;
            } else if Self::is_subtype(reg, ast, current, alternate) {
                self.may_register_dropped_properties(reg, ast, current, alternate);
                remove_current = true;
            }
            if remove_current {
                self.alternates.remove(index);
                if index as i32 == self.function_type_position {
                    self.function_type_position = -1;
                } else if (index as i32) < self.function_type_position {
                    self.function_type_position -= 1;
                }
            } else {
                index += 1;
            }
        }
        if alternate.is_function_type(reg) {
            check_state!(self.function_type_position == -1);
            self.function_type_position = self.alternates.len() as i32;
        }
        self.alternates.push(alternate);
        self
    }
    // port: UnionType.Builder#mayRegisterDroppedProperties
    fn may_register_dropped_properties(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) {
        if let (Some(sub), Some(sup)) = (
            subtype.to_maybe_record_type(reg),
            supertype.to_maybe_record_type(reg),
        ) {
            reg.register_dropped_properties_in_union(ast, sub, sup);
        }
    }
    // port: UnionType.Builder#build
    pub fn build(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        self.build_internal(reg);
        if let Some(target) = self.rebuild_target {
            fill_from_builder(target, reg, self);
        }
        let alts = self.final_alternates.as_ref().unwrap();
        if alts.len() == 1 {
            alts[0]
        } else if let Some(target) = self.rebuild_target {
            target
        } else {
            new(reg, ast, self)
        }
    }
    // port: UnionType.Builder#buildInternal
    fn build_internal(&mut self, reg: &JSTypeRegistry) -> &mut Self {
        self.check_has_not_built();
        let wildcard = self.get_native_wildcard_type(reg);
        self.final_alternates = Some(Arc::new(if let Some(wildcard) = wildcard {
            if self.contains_void_type {
                vec![wildcard, reg.get_native_type(JSTypeNative::VOID_TYPE)]
            } else {
                vec![wildcard]
            }
        } else if self.alternates.is_empty() {
            vec![reg.get_native_type(JSTypeNative::NO_TYPE)]
        } else if self.alternates.len() > MAX_UNION_SIZE {
            vec![reg.get_native_type(JSTypeNative::UNKNOWN_TYPE)]
        } else {
            self.alternates.clone()
        }));
        self
    }
    // port: UnionType.Builder#getNativeWildcardType
    fn get_native_wildcard_type(&self, reg: &JSTypeRegistry) -> Option<TypeId> {
        if self.is_all_type {
            Some(reg.get_native_type(JSTypeNative::ALL_TYPE))
        } else if self.is_native_unknown_type {
            Some(reg.get_native_type(if self.are_all_unknowns_checked {
                JSTypeNative::CHECKED_UNKNOWN_TYPE
            } else {
                JSTypeNative::UNKNOWN_TYPE
            }))
        } else {
            None
        }
    }
    // port: UnionType.Builder#checkHasNotBuilt
    fn check_has_not_built(&self) {
        check_state!(
            self.final_alternates.is_none(),
            "Cannot reuse a `UnionType.Builder` that has already built."
        );
    }
}
// port: UnionType#UnionType
fn new(reg: &mut JSTypeRegistry, ast: &Ast, builder: &UnionTypeBuilder) -> TypeId {
    let t = reg.alloc(
        JSTypeKind::Union(UnionTypeData {
            alternates: Arc::new(Vec::new()),
            alternates_resolved_before_build: false,
            resolved_alternates_at_last_build: -1,
        }),
        None,
    );
    fill_from_builder(t, reg, builder);
    reg.finish_construction(t, ast);
    t
}
pub(crate) fn data(t: TypeId, reg: &JSTypeRegistry) -> &UnionTypeData {
    match &reg.data(t).kind {
        JSTypeKind::Union(d) => d,
        _ => panic!("ClassCastException"),
    }
}
fn data_mut(t: TypeId, reg: &mut JSTypeRegistry) -> &mut UnionTypeData {
    match &mut reg.data_mut(t).kind {
        JSTypeKind::Union(d) => d,
        _ => panic!("ClassCastException"),
    }
}
// port: UnionType#fillFromBuilder
fn fill_from_builder(t: TypeId, reg: &mut JSTypeRegistry, builder: &UnionTypeBuilder) {
    check_state!(!data(t, reg).alternates_resolved_before_build);
    let final_alternates = builder.final_alternates.as_ref().unwrap();
    check_state!(!final_alternates.is_empty());
    let d = data_mut(t, reg);
    d.alternates = final_alternates.clone();
    d.alternates_resolved_before_build = builder.alternates_resolved_before_build;
}
// port: UnionType#countResolvedAlternates
fn count_resolved_alternates(t: TypeId, reg: &JSTypeRegistry) -> i32 {
    data(t, reg)
        .alternates
        .iter()
        .filter(|t| t.is_resolved(reg))
        .count() as i32
}
// port: UnionType#getAlternates
pub fn get_alternates(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> Arc<Vec<TypeId>> {
    if !t.is_resolved(reg) && !data(t, reg).alternates_resolved_before_build {
        let resolved = count_resolved_alternates(t, reg);
        if resolved != data(t, reg).resolved_alternates_at_last_build {
            data_mut(t, reg).resolved_alternates_at_last_build = resolved;
            let mut b = UnionTypeBuilder::for_rebuild(t);
            let alternates = data(t, reg).alternates.clone();
            b.add_alternates(reg, ast, &alternates);
            if !t.is_resolved(reg) && !data(t, reg).alternates_resolved_before_build {
                b.build(reg, ast);
                let count = count_resolved_alternates(t, reg);
                data_mut(t, reg).resolved_alternates_at_last_build = count;
            }
        }
    }
    data(t, reg).alternates.clone()
}
pub trait UnionType {
    fn get_alternates(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Arc<Vec<TypeId>>;
    fn contains(self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool;
    fn get_restricted_union(self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId;
}
impl UnionType for TypeId {
    // port: UnionType#getAlternates
    fn get_alternates(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Arc<Vec<TypeId>> {
        get_alternates(self, reg, ast)
    }
    // port: UnionType#contains
    fn contains(self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        for t in get_alternates(self, reg, ast).iter().copied() {
            if type_.equals(reg, ast, t) {
                return true;
            }
        }
        false
    }
    // port: UnionType#getRestrictedUnion
    fn get_restricted_union(self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId {
        let mut restricted = UnionTypeBuilder::new();
        for t in data(self, reg).alternates.clone().iter().copied() {
            if t.is_unknown_type(reg, ast) || !t.is_subtype_of(reg, ast, type_) {
                restricted.add_alternate(reg, ast, t);
            }
        }
        restricted.build(reg, ast)
    }
}
// port: UnionType#getTypeClass
pub(crate) fn get_type_class(
    _t: TypeId,
    _reg: &JSTypeRegistry,
) -> crate::js_type_class::JSTypeClass {
    crate::js_type_class::JSTypeClass::UNION
}
// port: UnionType#findPropertyTypeWithoutConsideringTemplateTypes
pub(crate) fn find_property_type_without_considering_template_types(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: PropertyKey,
) -> Option<TypeId> {
    let mut property_type: Option<TypeId> = None;
    for alternate in data(t, reg).alternates.clone().iter().copied() {
        if alternate.is_null_type(reg) || alternate.is_void_type(reg) {
            continue;
        }
        let Some(alt_property_type) = alternate.find_property_type(reg, ast, name.clone()) else {
            continue;
        };
        property_type = Some(if let Some(property_type) = property_type {
            property_type.get_least_supertype(reg, ast, alt_property_type)
        } else {
            alt_property_type
        });
    }
    property_type
}
// port: UnionType#testForEquality
pub(crate) fn test_for_equality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<Tri> {
    let mut result = None;
    for element in data(t, reg).alternates.clone().iter().copied() {
        let test = element.test_for_equality(reg, ast, that);
        if result.is_none() {
            result = test;
        } else if result != test {
            return Some(Tri::UNKNOWN);
        }
    }
    result
}
// port: UnionType#getLeastSupertype
pub(crate) fn get_least_supertype(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> TypeId {
    if !that.is_unknown_type(reg, ast) && !that.is_union_type(reg) {
        for alternate in data(t, reg).alternates.clone().iter().copied() {
            if !alternate.is_unknown_type(reg, ast) && that.is_subtype_of(reg, ast, alternate) {
                return t;
            }
        }
    }
    crate::js_type::get_least_supertype(t, reg, ast, that)
}
// port: UnionType#getGreatestSubtype
pub(crate) fn get_greatest_subtype(
    union: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> TypeId {
    let mut builder = UnionTypeBuilder::new();
    for alternate in data(union, reg).alternates.clone().iter().copied() {
        if alternate.is_subtype_of(reg, ast, that) {
            builder.add_alternate(reg, ast, alternate);
        }
    }
    if let Some(those) = that.to_maybe_union_type(reg) {
        for alternate in get_alternates(those, reg, ast).iter().copied() {
            if alternate.is_subtype_of(reg, ast, union) {
                builder.add_alternate(reg, ast, alternate);
            }
        }
    } else if that.is_subtype_of(reg, ast, union) {
        builder.add_alternate(reg, ast, that);
    }
    let result = builder.build(reg, ast);
    if !result.is_no_type(reg) {
        result
    } else if union.is_object(reg, ast) && that.is_object(reg, ast) && !that.is_no_type(reg) {
        reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE)
    } else {
        reg.get_native_type(JSTypeNative::NO_TYPE)
    }
}
// port: UnionType#getPropertyKind
pub(crate) fn get_property_kind(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: PropertyKey,
    autobox: bool,
) -> HasPropertyKind {
    let mut found = false;
    let mut always = true;
    for alternate in data(t, reg).alternates.clone().iter().copied() {
        if alternate.is_null_type(reg) || alternate.is_void_type(reg) {
            continue;
        }
        match alternate.get_property_kind_with_autobox(reg, ast, name.clone(), autobox) {
            HasPropertyKind::KNOWN_PRESENT => found = true,
            HasPropertyKind::ABSENT => always = false,
            HasPropertyKind::MAYBE_PRESENT => {
                found = true;
                always = false;
            }
        }
        if found && !always {
            break;
        }
    }
    if found {
        if always {
            HasPropertyKind::KNOWN_PRESENT
        } else {
            HasPropertyKind::MAYBE_PRESENT
        }
    } else {
        HasPropertyKind::ABSENT
    }
}
// port: UnionType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(t: TypeId, reg: &JSTypeRegistry) -> i32 {
    let mut hash = data(t, reg).alternates.len() as i32;
    for &t in data(t, reg).alternates.iter() {
        hash = hash.wrapping_mul(t.hash_code(reg));
    }
    hash
}
// port: UnionType#toMaybeUnionType
pub(crate) fn to_maybe_union_type(t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(t)
}
// port: UnionType#appendTo
pub(crate) fn append_to(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
) {
    sb.append("(");
    let mut names = Vec::new();
    for alt in data(t, reg).alternates.clone().iter().copied() {
        names.push(sb.clone_with_config().append_type(reg, ast, alt).build());
    }
    names.sort_by(|a, b| JsString::from(a.as_str()).cmp(&JsString::from(b.as_str())));
    sb.append_all(names, "|").append(")");
}
// port: UnionType#getPossibleToBooleanOutcomes
pub(crate) fn get_possible_to_boolean_outcomes(
    t: TypeId,
    reg: &JSTypeRegistry,
) -> BooleanLiteralSet {
    let mut literals = BooleanLiteralSet::EMPTY;
    for &element in data(t, reg).alternates.iter() {
        literals = literals.union(element.get_possible_to_boolean_outcomes(reg));
        if literals == BooleanLiteralSet::BOTH {
            break;
        }
    }
    literals
}
// port: UnionType#visit
pub(crate) fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    visitor.case_union_type(reg, ast, t)
}
// port: UnionType#visit
pub(crate) fn visit_relationship<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_union_type(reg, ast, t, that)
}
// port: UnionType#resolveInternal
pub(crate) fn resolve_internal(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn ErrorReporter,
) -> TypeId {
    if data(t, reg).alternates_resolved_before_build {
        return t;
    }
    let alternates = data(t, reg).alternates.clone();
    for &alt in alternates.iter() {
        alt.resolve_with_reporter(reg, ast, reporter);
    }
    let mut builder = UnionTypeBuilder::for_rebuild(t);
    builder.add_alternates(reg, ast, &alternates);
    builder.build(reg, ast)
}
// port: UnionType#collapseUnion
pub(crate) fn collapse_union(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
    let mut current_value = None;
    let mut current_common_super = None;
    for a in data(t, reg).alternates.clone().iter().copied() {
        if a.is_unknown_type(reg, ast) {
            return Some(reg.get_native_type(JSTypeNative::UNKNOWN_TYPE));
        }
        if let Some(obj) = a.to_object_type(reg) {
            if current_value.is_some() {
                return Some(reg.get_native_type(JSTypeNative::ALL_TYPE));
            }
            current_common_super = Some(if let Some(current) = current_common_super {
                reg.find_common_super_object(ast, current, obj)
            } else {
                obj
            });
        } else if current_value.is_none() && current_common_super.is_none() {
            current_value = Some(a);
        } else {
            return Some(reg.get_native_type(JSTypeNative::ALL_TYPE));
        }
    }
    current_common_super
}
// port: UnionType#matchConstraint
pub(crate) fn match_constraint(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast, constraint: TypeId) {
    for alternate in data(t, reg).alternates.clone().iter().copied() {
        alternate.match_constraint(reg, ast, constraint);
    }
}
// port: UnionType#matchesNumberContext
pub(crate) fn matches_number_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.matches_number_context(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#matchesStringContext
pub(crate) fn matches_string_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.matches_string_context(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#matchesSymbolContext
pub(crate) fn matches_symbol_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.matches_symbol_context(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#matchesObjectContext
pub(crate) fn matches_object_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.matches_object_context(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#canBeCalled
pub(crate) fn can_be_called(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if !t.can_be_called(reg, ast) {
            return false;
        }
    }
    true
}
// port: UnionType#isNullable
pub(crate) fn is_nullable(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.is_nullable(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#isVoidable
pub(crate) fn is_voidable(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.is_voidable(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#isExplicitlyVoidable
pub(crate) fn is_explicitly_voidable(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.is_explicitly_voidable(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#isUnknownType
pub(crate) fn is_unknown_type(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.is_unknown_type(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#isStruct
pub(crate) fn is_struct(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.is_struct(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#isDict
pub(crate) fn is_dict(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.is_dict(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#isObject
pub(crate) fn is_object(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if !t.is_object(reg, ast) {
            return false;
        }
    }
    true
}
// port: UnionType#hasAnyTemplateTypesInternal
pub(crate) fn has_any_template_types_internal(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> bool {
    for t in get_alternates(t, reg, ast).iter().copied() {
        if t.has_any_template_types(reg, ast) {
            return true;
        }
    }
    false
}
// port: UnionType#autobox
pub(crate) fn autobox(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
    let mut builder = UnionTypeBuilder::new();
    for alt in get_alternates(t, reg, ast).iter().copied() {
        let mapped = alt.autobox(reg, ast);
        builder.add_alternate(reg, ast, mapped);
    }
    builder.build(reg, ast)
}
// port: UnionType#restrictByNotNullOrUndefined
pub(crate) fn restrict_by_not_null_or_undefined(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> TypeId {
    let mut builder = UnionTypeBuilder::new();
    for alt in get_alternates(t, reg, ast).iter().copied() {
        let mapped = alt.restrict_by_not_null_or_undefined(reg, ast);
        builder.add_alternate(reg, ast, mapped);
    }
    builder.build(reg, ast)
}
// port: UnionType#restrictByNotUndefined
pub(crate) fn restrict_by_not_undefined(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
    let mut builder = UnionTypeBuilder::new();
    for alt in get_alternates(t, reg, ast).iter().copied() {
        let mapped = alt.restrict_by_not_undefined(reg, ast);
        builder.add_alternate(reg, ast, mapped);
    }
    builder.build(reg, ast)
}
// port: UnionType#restrictByNotNull
pub(crate) fn restrict_by_not_null(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
    let mut builder = UnionTypeBuilder::new();
    for alt in get_alternates(t, reg, ast).iter().copied() {
        let mapped = alt.restrict_by_not_null(reg, ast);
        builder.add_alternate(reg, ast, mapped);
    }
    builder.build(reg, ast)
}
// port: UnionType#getRestrictedTypeGivenOutcome
pub(crate) fn get_restricted_type_given_outcome(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    outcome: Outcome,
) -> TypeId {
    let mut builder = UnionTypeBuilder::new();
    for alt in get_alternates(t, reg, ast).iter().copied() {
        let mapped = alt.get_restricted_type_given_outcome(reg, ast, outcome);
        builder.add_alternate(reg, ast, mapped);
    }
    builder.build(reg, ast)
}
// port: UnionType#getTypesUnderEquality
pub(crate) fn get_types_under_equality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> TypePair {
    let mut this_restricted = UnionTypeBuilder::new();
    let mut that_restricted = UnionTypeBuilder::new();
    for element in data(t, reg).alternates.clone().iter().copied() {
        let p = element.get_types_under_equality(reg, ast, that);
        if let Some(a) = p.type_a {
            this_restricted.add_alternate(reg, ast, a);
        }
        if let Some(b) = p.type_b {
            that_restricted.add_alternate(reg, ast, b);
        }
    }
    TypePair::new(
        Some(this_restricted.build(reg, ast)),
        Some(that_restricted.build(reg, ast)),
    )
}
// port: UnionType#getTypesUnderInequality
pub(crate) fn get_types_under_inequality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> TypePair {
    let mut this_restricted = UnionTypeBuilder::new();
    let mut that_restricted = UnionTypeBuilder::new();
    for element in data(t, reg).alternates.clone().iter().copied() {
        let p = element.get_types_under_inequality(reg, ast, that);
        if let Some(a) = p.type_a {
            this_restricted.add_alternate(reg, ast, a);
        }
        if let Some(b) = p.type_b {
            that_restricted.add_alternate(reg, ast, b);
        }
    }
    TypePair::new(
        Some(this_restricted.build(reg, ast)),
        Some(that_restricted.build(reg, ast)),
    )
}
// port: UnionType#getTypesUnderShallowInequality
pub(crate) fn get_types_under_shallow_inequality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> TypePair {
    let mut this_restricted = UnionTypeBuilder::new();
    let mut that_restricted = UnionTypeBuilder::new();
    for element in data(t, reg).alternates.clone().iter().copied() {
        let p = element.get_types_under_shallow_inequality(reg, ast, that);
        if let Some(a) = p.type_a {
            this_restricted.add_alternate(reg, ast, a);
        }
        if let Some(b) = p.type_b {
            that_restricted.add_alternate(reg, ast, b);
        }
    }
    TypePair::new(
        Some(this_restricted.build(reg, ast)),
        Some(that_restricted.build(reg, ast)),
    )
}
