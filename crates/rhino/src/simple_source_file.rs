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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/SimpleSourceFile.java.

use crate::static_source_file::{SourceKind, StaticSourceFile};
use std::fmt;
#[derive(Debug)]
pub struct SimpleSourceFile {
    name: String,
    kind: SourceKind,
}
impl SimpleSourceFile {
    // port: SimpleSourceFile#SimpleSourceFile
    pub fn new(name: impl Into<String>, kind: SourceKind) -> Self {
        Self {
            name: name.into(),
            kind,
        }
    }
}
impl StaticSourceFile for SimpleSourceFile {
    // port: SimpleSourceFile#getName
    fn get_name(&self) -> &str {
        &self.name
    }
    // port: SimpleSourceFile#getKind
    fn get_kind(&self) -> SourceKind {
        self.kind
    }
    // port: SimpleSourceFile#isClosureUnawareCode
    fn is_closure_unaware_code(&self) -> bool {
        false
    }
    // port: SimpleSourceFile#getColumnOfOffset
    fn get_column_of_offset(&self, _offset: i32) -> i32 {
        0
    }
    // port: SimpleSourceFile#getLineOfOffset
    fn get_line_of_offset(&self, _offset: i32) -> i32 {
        1
    }
    // port: SimpleSourceFile#getLineOffset
    fn get_line_offset(&self, line: i32) -> i32 {
        if line < 1 {
            panic!("Should not call getLineOffset with line number {line}");
        }
        i32::MIN
    }
}
impl fmt::Display for SimpleSourceFile {
    // port: SimpleSourceFile#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}
