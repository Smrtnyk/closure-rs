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
//   src/com/google/javascript/rhino/testing/NodeSubject.java.

//! The `NodeSubject` assertions of the RenameVars tests' source-info checks.

use super::*;

// port: NodeSubject#hasSourceFileName / #hasLineno / #hasCharno / #hasLength
pub(super) fn assert_position(
    compiler: &Compiler,
    n: NodeId,
    source_file_name: Option<&Option<String>>,
    lineno: i32,
    charno: i32,
    length: i32,
) {
    if let Some(source_file_name) = source_file_name {
        assert_eq!(&n.get_source_file_name(compiler), source_file_name);
    }
    assert_eq!(n.get_lineno(compiler), lineno);
    assert_eq!(n.get_charno(compiler), charno);
    assert_eq!(n.get_length(compiler), length);
}

// port: NodeSubject#isName(String)
pub(super) fn assert_name(compiler: &Compiler, n: NodeId, name: &str) {
    assert!(n.is_name(compiler));
    assert_eq!(n.get_string(compiler), name);
}

// port: NodeSubject#hasOneChildThat
pub(super) fn one_child(compiler: &Compiler, n: NodeId) -> NodeId {
    assert!(n.has_one_child(compiler));
    n.get_first_child(compiler).unwrap()
}
