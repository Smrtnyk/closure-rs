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
//   src/com/google/javascript/rhino/NonJSDocComment.java.

use crate::{js_string::JsString, jscomp_parsing_parser::util::source_position::SourcePosition};
use std::fmt;
#[derive(Debug, Clone)]
pub struct NonJSDocComment {
    start_position: SourcePosition,
    end_position: SourcePosition,
    contents: Option<JsString>,
    ends_as_line_comment: bool,
    is_inline: bool,
}
impl NonJSDocComment {
    // port: NonJSDocComment#NonJSDocComment
    pub fn new(start: SourcePosition, end: SourcePosition, s: Option<JsString>) -> Self {
        Self {
            start_position: start,
            end_position: end,
            contents: s,
            ends_as_line_comment: false,
            is_inline: false,
        }
    }
    // port: NonJSDocComment#getCommentString
    pub fn get_comment_string(&self) -> JsString {
        self.contents.clone().unwrap_or_default()
    }
    // port: NonJSDocComment#getStartPosition
    pub fn get_start_position(&self) -> &SourcePosition {
        &self.start_position
    }
    // port: NonJSDocComment#getEndPosition
    pub fn get_end_position(&self) -> &SourcePosition {
        &self.end_position
    }
    // port: NonJSDocComment#setEndsAsLineComment
    pub fn set_ends_as_line_comment(&mut self, b: bool) {
        self.ends_as_line_comment = b;
    }
    // port: NonJSDocComment#setIsInline
    pub fn set_is_inline(&mut self, b: bool) {
        self.is_inline = b;
    }
    // port: NonJSDocComment#isEndingAsLineComment
    pub fn is_ending_as_line_comment(&self) -> bool {
        self.ends_as_line_comment
    }
    // port: NonJSDocComment#isInline
    pub fn is_inline(&self) -> bool {
        self.is_inline
    }
}
impl fmt::Display for NonJSDocComment {
    // port: NonJSDocComment#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NonJSDocComment : {}", self.get_comment_string())
    }
}
