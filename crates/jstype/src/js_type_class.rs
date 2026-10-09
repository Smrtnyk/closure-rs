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
//   src/com/google/javascript/rhino/jstype/JSTypeClass.java.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JSTypeClass {
    ALL,
    ARROW,
    BOOLEAN,
    BIGINT,
    ENUM,
    ENUM_ELEMENT,
    FUNCTION,
    INSTANCE_OBJECT,
    NAMED,
    NO,
    NO_OBJECT,
    NO_RESOLVED,
    NULL,
    NUMBER,
    PROTOTYPE_OBJECT,
    PROXY_OBJECT,
    RECORD,
    STRING,
    SYMBOL,
    WELL_KNOWN_SYMBOL,
    TEMPLATE,
    TEMPLATIZED,
    UNION,
    UNKNOWN,
    VOID,
}
impl JSTypeClass {
    // port: JSTypeClass#isTypeOf
    pub fn is_type_of(self, reg: &crate::JSTypeRegistry, t: crate::TypeId) -> bool {
        use crate::js_type::JSType;
        self == t.get_type_class(reg)
    }
}
