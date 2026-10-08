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
 *   Nick Santos
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
//   src/com/google/javascript/rhino/StaticSourceFile.java.

use std::{
    any::Any,
    fmt::{Debug, Display},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    STRONG,
    WEAK,
    EXTERN,
    NON_CODE,
}
pub trait StaticSourceFile: Any + Send + Sync + Debug + Display {
    // port: StaticSourceFile#getName
    fn get_name(&self) -> &str;
    // port: StaticSourceFile#getKind
    fn get_kind(&self) -> SourceKind;
    // port: StaticSourceFile#isStrong
    fn is_strong(&self) -> bool {
        self.get_kind() == SourceKind::STRONG
    }
    // port: StaticSourceFile#isWeak
    fn is_weak(&self) -> bool {
        self.get_kind() == SourceKind::WEAK
    }
    // port: StaticSourceFile#isExtern
    fn is_extern(&self) -> bool {
        self.get_kind() == SourceKind::EXTERN
    }
    // port: StaticSourceFile#isNonCode
    fn is_non_code(&self) -> bool {
        self.get_kind() == SourceKind::NON_CODE
    }
    // port: StaticSourceFile#isTypeScriptSource
    fn is_type_script_source(&self) -> bool {
        let n = self.get_name();
        n.ends_with(".closure.js") || n.ends_with(".tsx.cl.js")
    }
    // port: StaticSourceFile#isClosureUnawareCode
    fn is_closure_unaware_code(&self) -> bool;
    // port: StaticSourceFile#getLineOffset
    fn get_line_offset(&self, line_number: i32) -> i32;
    // port: StaticSourceFile#getLineOfOffset
    fn get_line_of_offset(&self, offset: i32) -> i32;
    // port: StaticSourceFile#getColumnOfOffset
    fn get_column_of_offset(&self, offset: i32) -> i32;
}
