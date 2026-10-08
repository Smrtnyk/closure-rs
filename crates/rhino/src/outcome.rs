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
 *   Annie Wang
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
//   src/com/google/javascript/rhino/Outcome.java.

//! Truthiness and nullishness of outcomes.
use crate::jscomp_base::Tri;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    TRUE,
    FALSE,
    FALSE_NOT_NULL,
    NULLISH,
}
impl Outcome {
    // port: Outcome#isTruthy
    pub fn is_truthy(self) -> bool {
        matches!(self, Self::TRUE)
    }
    // port: Outcome#isNullish
    pub fn is_nullish(self) -> Tri {
        match self {
            Self::TRUE | Self::FALSE_NOT_NULL => Tri::FALSE,
            Self::FALSE => Tri::UNKNOWN,
            Self::NULLISH => Tri::TRUE,
        }
    }
    // port: Outcome#not
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Self {
        match self {
            Self::TRUE => Self::FALSE,
            Self::FALSE | Self::FALSE_NOT_NULL | Self::NULLISH => Self::TRUE,
        }
    }
    // port: Outcome#forBoolean
    pub fn for_boolean(val: bool) -> Self {
        if val { Self::TRUE } else { Self::FALSE }
    }
}
