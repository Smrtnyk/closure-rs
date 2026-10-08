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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/TokenStream.java.

//! Port of `com.google.javascript.rhino.TokenStream`.

use crate::js_identifier::JSIdentifier;
use crate::js_string::JsString;

/// This class implements the JavaScript scanner.
///
/// It is based on the C source files jsscan.c and jsscan.h in the jsref package.
pub struct TokenStream;

impl TokenStream {
    /// Is the string an ES3 keyword?
    ///
    /// Keywords for versions of JavaScript after ES3 are not included, because the parser would
    /// reject valid ES3 code that happened to newer keywords if we did that.
    ///
    /// Since isKeyword() is used repeatedly in scanning, for performance reasons, it is
    /// implemented to do the minimum number of character comparisons to ascertain whether the
    /// given name is a keyword, instead of doing string search of the name against JS keywords.
    // port: TokenStream#isKeyword
    #[allow(clippy::if_same_then_else)] // Java's hand-unrolled comparisons are kept as written.
    pub fn is_keyword(name: &JsString) -> bool {
        let mut id = false;
        let s = name;
        'complete: {
            let mut x: Option<&str> = None;
            let c: u16;
            let ch = |i: usize| s.char_at(i);
            let is = |c: u16, l: char| c == l as u16;
            'partial: {
                match s.length() {
                    2 => {
                        c = ch(1);
                        if is(c, 'f') {
                            if is(ch(0), 'i') {
                                id = true;
                                break 'complete;
                            }
                        } else if is(c, 'n') {
                            if is(ch(0), 'i') {
                                id = true;
                                break 'complete;
                            }
                        } else if is(c, 'o') && is(ch(0), 'd') {
                            id = true;
                            break 'complete;
                        }
                        break 'partial;
                    }
                    3 => {
                        match ch(0) {
                            c0 if is(c0, 'f') => {
                                if is(ch(2), 'r') && is(ch(1), 'o') {
                                    id = true;
                                    break 'complete;
                                }
                                break 'partial;
                            }
                            c0 if is(c0, 'i') => {
                                if is(ch(2), 't') && is(ch(1), 'n') {
                                    id = true;
                                    break 'complete;
                                }
                                break 'partial;
                            }
                            c0 if is(c0, 'n') => {
                                if is(ch(2), 'w') && is(ch(1), 'e') {
                                    id = true;
                                    break 'complete;
                                }
                                break 'partial;
                            }
                            c0 if is(c0, 't') => {
                                if is(ch(2), 'y') && is(ch(1), 'r') {
                                    id = true;
                                    break 'complete;
                                }
                                break 'partial;
                            }
                            c0 if is(c0, 'v') => {
                                if is(ch(2), 'r') && is(ch(1), 'a') {
                                    id = true;
                                    break 'complete;
                                }
                                break 'partial;
                            }
                            _ => {}
                        }
                        break 'partial;
                    }
                    4 => {
                        match ch(0) {
                            c0 if is(c0, 'b') => {
                                x = Some("byte");
                                id = true;
                                break 'partial;
                            }
                            c0 if is(c0, 'c') => {
                                c = ch(3);
                                if is(c, 'e') {
                                    if is(ch(2), 's') && is(ch(1), 'a') {
                                        id = true;
                                        break 'complete;
                                    }
                                } else if is(c, 'r') && is(ch(2), 'a') && is(ch(1), 'h') {
                                    id = true;
                                    break 'complete;
                                }
                                break 'partial;
                            }
                            c0 if is(c0, 'e') => {
                                c = ch(3);
                                if is(c, 'e') {
                                    if is(ch(2), 's') && is(ch(1), 'l') {
                                        id = true;
                                        break 'complete;
                                    }
                                } else if is(c, 'm') && is(ch(2), 'u') && is(ch(1), 'n') {
                                    id = true;
                                    break 'complete;
                                }
                                break 'partial;
                            }
                            c0 if is(c0, 'g') => {
                                x = Some("goto");
                                id = true;
                                break 'partial;
                            }
                            c0 if is(c0, 'l') => {
                                x = Some("long");
                                id = true;
                                break 'partial;
                            }
                            c0 if is(c0, 'n') => {
                                x = Some("null");
                                id = true;
                                break 'partial;
                            }
                            c0 if is(c0, 't') => {
                                c = ch(3);
                                if is(c, 'e') {
                                    if is(ch(2), 'u') && is(ch(1), 'r') {
                                        id = true;
                                        break 'complete;
                                    }
                                } else if is(c, 's') && is(ch(2), 'i') && is(ch(1), 'h') {
                                    id = true;
                                    break 'complete;
                                }
                                break 'partial;
                            }
                            c0 if is(c0, 'v') => {
                                x = Some("void");
                                id = true;
                                break 'partial;
                            }
                            c0 if is(c0, 'w') => {
                                x = Some("with");
                                id = true;
                                break 'partial;
                            }
                            _ => {}
                        }
                        break 'partial;
                    }
                    5 => {
                        match ch(2) {
                            c2 if is(c2, 'a') => {
                                x = Some("class");
                                id = true;
                            }
                            c2 if is(c2, 'e') => {
                                x = Some("break");
                                id = true;
                            }
                            c2 if is(c2, 'i') => {
                                x = Some("while");
                                id = true;
                            }
                            c2 if is(c2, 'l') => {
                                x = Some("false");
                                id = true;
                            }
                            c2 if is(c2, 'n') => {
                                c = ch(0);
                                if is(c, 'c') {
                                    x = Some("const");
                                    id = true;
                                } else if is(c, 'f') {
                                    x = Some("final");
                                    id = true;
                                }
                            }
                            c2 if is(c2, 'o') => {
                                c = ch(0);
                                if is(c, 'f') {
                                    x = Some("float");
                                    id = true;
                                } else if is(c, 's') {
                                    x = Some("short");
                                    id = true;
                                }
                            }
                            c2 if is(c2, 'p') => {
                                x = Some("super");
                                id = true;
                            }
                            c2 if is(c2, 'r') => {
                                x = Some("throw");
                                id = true;
                            }
                            c2 if is(c2, 't') => {
                                x = Some("catch");
                                id = true;
                            }
                            _ => {}
                        }
                        break 'partial;
                    }
                    6 => {
                        match ch(1) {
                            c1 if is(c1, 'a') => {
                                x = Some("native");
                                id = true;
                            }
                            c1 if is(c1, 'e') => {
                                c = ch(0);
                                if is(c, 'd') {
                                    x = Some("delete");
                                    id = true;
                                } else if is(c, 'r') {
                                    x = Some("return");
                                    id = true;
                                }
                            }
                            c1 if is(c1, 'h') => {
                                x = Some("throws");
                                id = true;
                            }
                            c1 if is(c1, 'm') => {
                                x = Some("import");
                                id = true;
                            }
                            c1 if is(c1, 'o') => {
                                x = Some("double");
                                id = true;
                            }
                            c1 if is(c1, 't') => {
                                x = Some("static");
                                id = true;
                            }
                            c1 if is(c1, 'u') => {
                                x = Some("public");
                                id = true;
                            }
                            c1 if is(c1, 'w') => {
                                x = Some("switch");
                                id = true;
                            }
                            c1 if is(c1, 'x') => {
                                x = Some("export");
                                id = true;
                            }
                            c1 if is(c1, 'y') => {
                                x = Some("typeof");
                                id = true;
                            }
                            _ => {}
                        }
                        break 'partial;
                    }
                    7 => {
                        match ch(1) {
                            c1 if is(c1, 'a') => {
                                x = Some("package");
                                id = true;
                            }
                            c1 if is(c1, 'e') => {
                                x = Some("default");
                                id = true;
                            }
                            c1 if is(c1, 'i') => {
                                x = Some("finally");
                                id = true;
                            }
                            c1 if is(c1, 'o') => {
                                x = Some("boolean");
                                id = true;
                            }
                            c1 if is(c1, 'r') => {
                                x = Some("private");
                                id = true;
                            }
                            c1 if is(c1, 'x') => {
                                x = Some("extends");
                                id = true;
                            }
                            _ => {}
                        }
                        break 'partial;
                    }
                    8 => {
                        match ch(0) {
                            c0 if is(c0, 'a') => {
                                x = Some("abstract");
                                id = true;
                            }
                            c0 if is(c0, 'c') => {
                                x = Some("continue");
                                id = true;
                            }
                            c0 if is(c0, 'd') => {
                                x = Some("debugger");
                                id = true;
                            }
                            c0 if is(c0, 'f') => {
                                x = Some("function");
                                id = true;
                            }
                            c0 if is(c0, 'v') => {
                                x = Some("volatile");
                                id = true;
                            }
                            _ => {}
                        }
                        break 'partial;
                    }
                    9 => {
                        c = ch(0);
                        if is(c, 'i') {
                            x = Some("interface");
                            id = true;
                        } else if is(c, 'p') {
                            x = Some("protected");
                            id = true;
                        } else if is(c, 't') {
                            x = Some("transient");
                            id = true;
                        }
                        break 'partial;
                    }
                    10 => {
                        c = ch(1);
                        if is(c, 'm') {
                            x = Some("implements");
                            id = true;
                        } else if is(c, 'n') {
                            x = Some("instanceof");
                            id = true;
                        }
                        break 'partial;
                    }
                    12 => {
                        x = Some("synchronized");
                        id = true;
                        break 'partial;
                    }
                    _ => {}
                }
            }
            // partial match validate the entire string the one possibility
            if let Some(x) = x
                && *s != x
            {
                return false;
            }
        }
        id
    }

    // port: TokenStream#isJSIdentifier
    pub fn is_js_identifier(s: &JsString) -> bool {
        JSIdentifier::is_js_identifier(s)
    }

    // port: TokenStream#TokenStream
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }
}
