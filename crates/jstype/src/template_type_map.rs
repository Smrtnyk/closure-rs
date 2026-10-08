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
//   src/com/google/javascript/rhino/jstype/TemplateTypeMap.java.

use crate::TypeId;
use crate::js_type::JSType;
use crate::js_type_native::JSTypeNative;
use crate::js_type_registry::JSTypeRegistry;
use crate::object_type::ObjectType;
use crate::template_type::TemplateType;
use crate::template_type_replacer::TemplateTypeReplacer;
use closure_rhino::check_argument;
use closure_rhino::js_string::JsString;
use closure_rhino::node::Ast;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TemplateTypeMap {
    template_keys: Vec<TypeId>,
    template_values: Vec<TypeId>,
    resolved_template_values: Vec<TypeId>,
    sub_map_starts: Vec<bool>,
}

impl TemplateTypeMap {
    // port: TemplateTypeMap#createEmpty
    pub fn create_empty(reg: &JSTypeRegistry) -> Arc<Self> {
        check_argument!(reg.empty_template_type_map.is_none());
        Self::new_empty()
    }

    /// Only registry initialization uses this constructor, before the native types exist.
    pub(crate) fn new_empty() -> Arc<Self> {
        Arc::new(Self {
            template_keys: Vec::new(),
            template_values: Vec::new(),
            resolved_template_values: Vec::new(),
            sub_map_starts: Vec::new(),
        })
    }

    // port: TemplateTypeMap#TemplateTypeMap
    pub fn new(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        keys: Vec<TypeId>,
        values: Vec<TypeId>,
    ) -> Arc<Self> {
        let mut starts = vec![false; keys.len()];
        if !starts.is_empty() {
            starts[0] = true;
        }
        Self::new_with_submaps(reg, ast, keys, values, starts)
    }

    // port: TemplateTypeMap#TemplateTypeMap
    fn new_with_submaps(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        template_keys: Vec<TypeId>,
        template_values: Vec<TypeId>,
        sub_map_starts: Vec<bool>,
    ) -> Arc<Self> {
        check_argument!(template_values.len() <= template_keys.len());
        check_argument!(template_keys.len() <= sub_map_starts.len());
        let mut map = Self {
            template_keys,
            template_values,
            resolved_template_values: Vec::new(),
            sub_map_starts,
        };
        let mut replacer = TemplateTypeReplacer::for_total_replacement(Arc::new(map.clone()));
        for i in 0..map.template_keys.len() {
            let value = if i < map.template_values.len() {
                let next_sub_map = map.next_sub_map_start(i + 1);
                replacer.set_key_type(map.template_keys[i], next_sub_map as i32);
                map.template_values[i].visit(reg, ast, &mut replacer)
            } else {
                map.template_keys[i].get_bound(reg)
            };
            map.resolved_template_values.push(value);
        }
        Arc::new(map)
    }

    // port: TemplateTypeMap#nextSubMapStart
    fn next_sub_map_start(&self, mut index: usize) -> usize {
        while index < self.template_keys.len() && !self.sub_map_starts[index] {
            index += 1;
        }
        index
    }

    // port: TemplateTypeMap#copyFilledWithValues
    pub fn copy_filled_with_values(
        self: &Arc<Self>,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        values: &[TypeId],
    ) -> Arc<Self> {
        let required_unknown_count = self.num_unfilled_template_keys() as i32 - values.len() as i32;
        check_argument!(required_unknown_count >= 0, "%s", required_unknown_count);
        if self.num_unfilled_template_keys() == 0 {
            return self.clone();
        }
        let mut extended_values = self.template_values.clone();
        extended_values.extend_from_slice(values);
        self.pad_to_same_length(reg, ast, &self.template_keys, &mut extended_values);
        Self::new_with_submaps(
            reg,
            ast,
            self.template_keys.clone(),
            extended_values,
            self.sub_map_starts.clone(),
        )
    }

    // port: TemplateTypeMap#copyWithExtension
    pub fn copy_with_extension_map(
        self: &Arc<Self>,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        extension: &Self,
    ) -> Arc<Self> {
        self.copy_with_extension_submaps(
            reg,
            ast,
            &extension.template_keys,
            &extension.template_values,
            &extension.sub_map_starts,
        )
    }

    // port: TemplateTypeMap#copyWithExtension
    pub fn copy_with_extension(
        self: &Arc<Self>,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        keys: &[TypeId],
        values: &[TypeId],
    ) -> Arc<Self> {
        let mut starts = vec![false; keys.len()];
        if !starts.is_empty() {
            starts[0] = true;
        }
        self.copy_with_extension_submaps(reg, ast, keys, values, &starts)
    }

    // port: TemplateTypeMap#copyWithExtension
    fn copy_with_extension_submaps(
        self: &Arc<Self>,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        keys: &[TypeId],
        values: &[TypeId],
        new_sub_map_starts: &[bool],
    ) -> Arc<Self> {
        let extended_unfilled_count = keys.len() as i32 - values.len() as i32;
        check_argument!(extended_unfilled_count >= 0, "%s", extended_unfilled_count);
        if self.num_unfilled_template_keys() == 0 && keys.is_empty() {
            return self.clone();
        }
        let mut extended_keys = self.template_keys.clone();
        extended_keys.extend_from_slice(keys);
        let mut extended_values = self.template_values.clone();
        self.pad_to_same_length(reg, ast, &self.template_keys, &mut extended_values);
        extended_values.extend_from_slice(values);
        let mut extended_sub_map_starts = self.sub_map_starts.clone();
        extended_sub_map_starts.resize(self.template_keys.len(), false);
        extended_sub_map_starts.extend_from_slice(new_sub_map_starts);
        Self::new_with_submaps(
            reg,
            ast,
            extended_keys,
            extended_values,
            extended_sub_map_starts,
        )
    }

    // port: TemplateTypeMap#copyWithoutKeys
    pub fn copy_without_keys(
        self: &Arc<Self>,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        removals: &[TypeId],
    ) -> Arc<Self> {
        let mut keys = self.template_keys[..self.template_values.len()].to_vec();
        let mut starts = self.sub_map_starts[..self.template_values.len()].to_vec();
        let mut in_new_submap = false;
        for i in self.template_values.len()..self.template_keys.len() {
            let key = self.template_keys[i];
            in_new_submap |= self.sub_map_starts[i];
            // TemplateType equality is identity, matching Java's Set<TemplateType> membership.
            if !removals.contains(&key) {
                keys.push(key);
                starts.push(in_new_submap);
                in_new_submap = false;
            }
        }
        if keys.len() == self.template_keys.len() {
            return self.clone();
        }
        Self::new_with_submaps(reg, ast, keys, self.template_values.clone(), starts)
    }

    // port: TemplateTypeMap#size
    pub fn size(&self) -> usize {
        self.template_keys.len()
    }
    // port: TemplateTypeMap#isEmpty
    pub fn is_empty(&self) -> bool {
        self.template_keys.is_empty()
    }
    // port: TemplateTypeMap#getTemplateKeys
    pub fn get_template_keys(&self) -> &[TypeId] {
        &self.template_keys
    }
    // port: TemplateTypeMap#getTemplateValues
    pub fn get_template_values(&self) -> &[TypeId] {
        &self.template_values
    }
    // port: TemplateTypeMap#hasTemplateKey
    pub fn has_template_key(&self, key: TypeId) -> bool {
        self.has_template_key_from(key, 0)
    }
    // port: TemplateTypeMap#hasTemplateKey
    pub(crate) fn has_template_key_from(&self, key: TypeId, next_sub_map: i32) -> bool {
        let start = next_sub_map.max(0) as usize;
        self.template_keys
            .iter()
            .skip(start)
            .any(|entry| *entry == key)
    }
    // port: TemplateTypeMap#getTemplateKeyCountThisShouldAlwaysBeOneOrZeroButIsnt
    pub fn get_template_key_count_this_should_always_be_one_or_zero_but_isnt(
        &self,
        key: TypeId,
    ) -> usize {
        self.template_keys
            .iter()
            .filter(|entry| **entry == key)
            .count()
    }
    // port: TemplateTypeMap#numUnfilledTemplateKeys
    pub fn num_unfilled_template_keys(&self) -> usize {
        self.template_keys.len() - self.template_values.len()
    }
    // port: TemplateTypeMap#hasTemplateType
    pub fn has_template_type(&self, key: TypeId) -> bool {
        self.get_template_type_index(key).is_some()
    }
    // port: TemplateTypeMap#getUnresolvedOriginalTemplateType
    pub(crate) fn get_unresolved_original_template_type(
        &self,
        reg: &JSTypeRegistry,
        key: TypeId,
    ) -> TypeId {
        self.get_template_type_index(key).map_or_else(
            || reg.get_native_type(JSTypeNative::UNKNOWN_TYPE),
            |index| self.template_values[index],
        )
    }
    // port: TemplateTypeMap#getLastTemplateTypeKeyByName
    pub fn get_last_template_type_key_by_name(
        &self,
        reg: &JSTypeRegistry,
        name: impl Into<JsString>,
    ) -> Option<TypeId> {
        let name = name.into();
        self.template_keys
            .iter()
            .rev()
            .find(|key| key.get_reference_name(reg) == Some(name.clone()))
            .copied()
    }
    // port: TemplateTypeMap#getTemplateTypeIndex
    fn get_template_type_index(&self, key: TypeId) -> Option<usize> {
        (0..self.template_values.len().min(self.template_keys.len()))
            .rev()
            .find(|i| self.template_keys[*i] == key)
    }
    // port: TemplateTypeMap#getResolvedTemplateType
    pub fn get_resolved_template_type(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        key: TypeId,
    ) -> TypeId {
        self.get_template_type_index(key).map_or_else(
            || self.default_value_type(reg, ast, key),
            |index| self.resolved_template_values[index],
        )
    }
    // port: TemplateTypeMap#hasAnyTemplateTypesInternal
    pub(crate) fn has_any_template_types_internal(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> bool {
        self.resolved_template_values
            .iter()
            .any(|value| value.has_any_template_types(reg, ast))
    }
    // port: TemplateTypeMap#toString
    pub fn to_string(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> String {
        let mut s = String::from("{ ");
        for (i, key) in self.template_keys.iter().enumerate() {
            s.push('(');
            s.push_str(&key.to_string(reg, ast));
            s.push(',');
            if let Some(value) = self.template_values.get(i) {
                s.push_str(&value.to_string(reg, ast));
            }
            s.push(',');
            if let Some(value) = self.resolved_template_values.get(i) {
                s.push_str(&value.to_string(reg, ast));
            }
            s.push_str(") ");
        }
        s.push('}');
        s
    }
    // port: TemplateTypeMap#padToSameLength
    fn pad_to_same_length(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        keys: &[TypeId],
        builder: &mut Vec<TypeId>,
    ) {
        check_argument!(builder.len() <= keys.len());
        for key in keys.iter().skip(builder.len()) {
            builder.push(self.default_value_type(reg, ast, *key));
        }
    }
    // port: TemplateTypeMap#defaultValueType
    pub(crate) fn default_value_type(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> TypeId {
        if type_.get_bound(reg).is_unknown_type(reg, ast) {
            reg.get_native_type(JSTypeNative::UNKNOWN_TYPE)
        } else {
            type_
        }
    }
    // port: TemplateTypeMap#getIndicesOfSubmapsForTesting
    pub fn get_indices_of_submaps_for_testing(&self) -> Vec<usize> {
        self.sub_map_starts
            .iter()
            .enumerate()
            .filter_map(|(i, start)| start.then_some(i))
            .collect()
    }
}
