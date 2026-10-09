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
//   src/com/google/javascript/rhino/testing/TemplateTypeMapSubject.java.

use crate::TypeId;
use crate::js_type::JSType;
use crate::js_type_registry::JSTypeRegistry;
use crate::template_type_map::TemplateTypeMap;
use closure_rhino::node::Ast;
use std::sync::Arc;

pub struct TemplateTypeMapSubject {
    actual: Arc<TemplateTypeMap>,
}
impl TemplateTypeMapSubject {
    // port: TemplateTypeMapSubject#assertThat
    pub fn assert_that(actual: Arc<TemplateTypeMap>) -> Self {
        Self::new(actual)
    }
    // port: TemplateTypeMapSubject#typeMaps
    pub fn type_maps() -> impl Fn(Arc<TemplateTypeMap>) -> Self {
        Self::new
    }
    // port: TemplateTypeMapSubject#TemplateTypeMapSubject
    fn new(actual: Arc<TemplateTypeMap>) -> Self {
        Self { actual }
    }
    // port: TemplateTypeMapSubject#hasKeysAndValues
    pub fn has_keys_and_values(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        keys: &[TypeId],
        values: &[TypeId],
    ) {
        assert_eq!(self.actual.get_template_keys().len(), keys.len());
        for (&actual, &expected) in self.actual.get_template_keys().iter().zip(keys) {
            assert!(actual.equals(reg, ast, expected));
        }
        assert_eq!(self.actual.get_template_values().len(), values.len());
        for (&actual, &expected) in self.actual.get_template_values().iter().zip(values) {
            assert!(actual.equals(reg, ast, expected));
        }
    }
    // port: TemplateTypeMapSubject#hasSubmapsAt
    pub fn has_submaps_at(&self, indices: &[usize]) {
        let mut actual = self.actual.get_indices_of_submaps_for_testing();
        let mut expected = indices.to_vec();
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected);
    }
    pub fn is_same_instance_as(&self, expected: &Arc<TemplateTypeMap>) {
        assert!(Arc::ptr_eq(&self.actual, expected));
    }
    pub fn is_not_same_instance_as(&self, expected: &Arc<TemplateTypeMap>) {
        assert!(!Arc::ptr_eq(&self.actual, expected));
    }
}
