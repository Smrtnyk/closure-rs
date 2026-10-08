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
//   src/com/google/javascript/rhino/SourcePosition.java.

#[derive(Debug, Clone)]
pub struct SourcePosition<T> {
    item: Option<T>,
    start_lineno: i32,
    start_charno: i32,
    end_lineno: i32,
    end_charno: i32,
}
impl<T> Default for SourcePosition<T> {
    fn default() -> Self {
        Self {
            item: None,
            start_lineno: 0,
            start_charno: 0,
            end_lineno: 0,
            end_charno: 0,
        }
    }
}
impl<T> SourcePosition<T> {
    // port: SourcePosition#setItem
    pub fn set_item(&mut self, item: impl Into<Option<T>>) {
        self.item = item.into();
    }
    // port: SourcePosition#setPositionInformation
    pub fn set_position_information(
        &mut self,
        start_lineno: i32,
        start_charno: i32,
        end_lineno: i32,
        end_charno: i32,
    ) {
        if start_lineno > end_lineno {
            panic!(
                "Recorded bad position information\nstart-line: {start_lineno}\nend-line: {end_lineno}"
            );
        } else if start_lineno == end_lineno && start_charno >= end_charno {
            panic!(
                "Recorded bad position information\nline: {start_lineno}\nstart-char: {start_charno}\nend-char: {end_charno}"
            );
        }
        self.start_lineno = start_lineno;
        self.start_charno = start_charno;
        self.end_lineno = end_lineno;
        self.end_charno = end_charno;
    }
    // port: SourcePosition#getItem
    pub fn get_item(&self) -> Option<&T> {
        self.item.as_ref()
    }
    // port: SourcePosition#getStartLine
    pub fn get_start_line(&self) -> i32 {
        self.start_lineno
    }
    // port: SourcePosition#getPositionOnStartLine
    pub fn get_position_on_start_line(&self) -> i32 {
        self.start_charno
    }
    // port: SourcePosition#getEndLine
    pub fn get_end_line(&self) -> i32 {
        self.end_lineno
    }
    // port: SourcePosition#getPositionOnEndLine
    pub fn get_position_on_end_line(&self) -> i32 {
        self.end_charno
    }
    // port: SourcePosition#isSamePositionAs
    pub fn is_same_position_as<U>(&self, that: &SourcePosition<U>) -> bool {
        self.start_lineno == that.start_lineno
            && self.start_charno == that.start_charno
            && self.end_lineno == that.end_lineno
            && self.end_charno == that.end_charno
    }
}
