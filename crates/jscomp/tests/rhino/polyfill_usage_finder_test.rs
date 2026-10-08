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
//   src/com/google/javascript/rhino/testing/NodeSubject.java.

//! The `NodeSubject` assertions of the PolyfillUsageFinder tests' `NodeAssertions`.

use super::*;

impl NodeAssertions<'_> {
    // port: NodeSubject#isName
    pub(super) fn is_name(&self, name: &str) -> &Self {
        assert!(self.actual.is_name(self.ast), "isName()");
        assert_eq!(self.actual.get_string(self.ast), name, "getString()");
        self
    }

    // port: NodeSubject#isGetProp
    pub(super) fn is_get_prop(&self) -> &Self {
        assert!(self.actual.is_get_prop(self.ast), "isGetProp()");
        self
    }

    // port: NodeSubject#hasToken
    pub(super) fn has_token(&self, token: Token) -> &Self {
        assert_eq!(self.actual.get_token(self.ast), token, "getToken()");
        self
    }

    // port: NodeSubject#matchesQualifiedName
    pub(super) fn matches_qualified_name(&self, qname: &str) -> &Self {
        assert!(
            self.actual.matches_qualified_name(self.ast, qname),
            "matchesQualifiedName({qname})"
        );
        self
    }

    // port: NodeSubject#hasLineno
    pub(super) fn has_lineno(&self, lineno: i32) -> &Self {
        assert_eq!(self.actual.get_lineno(self.ast), lineno, "getLineno()");
        self
    }
}
