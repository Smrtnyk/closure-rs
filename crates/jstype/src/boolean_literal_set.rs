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
//   src/com/google/javascript/rhino/jstype/BooleanLiteralSet.java.

use closure_rhino::jscomp_base::Tri;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanLiteralSet {
    EMPTY,
    TRUE,
    FALSE,
    BOTH,
}
impl BooleanLiteralSet {
    // port: BooleanLiteralSet#fromOrdinal
    pub fn from_ordinal(ordinal: i32) -> Self {
        match ordinal {
            0 => Self::EMPTY,
            1 => Self::TRUE,
            2 => Self::FALSE,
            3 => Self::BOTH,
            _ => panic!("Ordinal: {ordinal}"),
        }
    }
    // port: BooleanLiteralSet#intersection
    pub fn intersection(self, that: Self) -> Self {
        Self::from_ordinal(self as i32 & that as i32)
    }
    // port: BooleanLiteralSet#union
    pub fn union(self, that: Self) -> Self {
        Self::from_ordinal(self as i32 | that as i32)
    }
    // port: BooleanLiteralSet#contains
    pub fn contains(self, literal_value: bool) -> bool {
        match self {
            Self::EMPTY => false,
            Self::TRUE => literal_value,
            Self::FALSE => !literal_value,
            Self::BOTH => true,
        }
    }
    // port: BooleanLiteralSet#get
    pub fn get(literal_value: bool) -> Self {
        if literal_value {
            Self::TRUE
        } else {
            Self::FALSE
        }
    }
    // port: BooleanLiteralSet#toTri
    pub fn to_tri(self) -> Tri {
        if self == Self::TRUE {
            Tri::TRUE
        } else if self == Self::FALSE {
            Tri::FALSE
        } else {
            Tri::UNKNOWN
        }
    }
}
