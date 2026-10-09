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
 *   Roger Lawrence
 *   Mike McCabe
 *   Igor Bukanov
 *   Ethan Hugg
 *   Bob Jervis
 *   Terry Lucas
 *   Milen Nankov
 *   Pascal-Louis Perez
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
//   src/com/google/javascript/rhino/TokenUtil.java.

use crate::{java_lang, jscomp_base::Tri};
pub struct TokenUtil;
impl TokenUtil {
    // port: TokenUtil#isJSSpace
    pub fn is_js_space(c: i32) -> bool {
        if c <= 127 {
            c == 0x20 || c == 9 || c == 12 || c == 11
        } else {
            c == 0xa0 || java_lang::is_space_separator(c as u16 as i32)
        }
    }
    // port: TokenUtil#isJSFormatChar
    pub fn is_js_format_char(c: i32) -> bool {
        c > 127 && java_lang::is_format(c)
    }
    // port: TokenUtil#isWhitespace
    pub fn is_whitespace(c: i32) -> bool {
        java_lang::is_whitespace(c)
    }
    // port: TokenUtil#isStrWhiteSpaceChar
    pub fn is_str_white_space_char(c: i32) -> Tri {
        match c {
            11 => Tri::UNKNOWN,
            32 | 10 | 13 | 9 | 0xa0 | 12 | 0x2028 | 0x2029 | 0xfeff => Tri::TRUE,
            _ => {
                if java_lang::is_space_separator(c) {
                    Tri::TRUE
                } else {
                    Tri::FALSE
                }
            }
        }
    }
    // port: TokenUtil#TokenUtil
    fn new() -> Self {
        Self
    }
}
impl Default for TokenUtil {
    fn default() -> Self {
        Self::new()
    }
}
