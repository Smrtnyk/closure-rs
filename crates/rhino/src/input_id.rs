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
 *   John Lenz
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/rhino/InputId.java.

use crate::js_string::JsString;
use std::fmt;
use std::sync::{Mutex, OnceLock, PoisonError};
#[derive(Clone, Eq)]
pub struct InputId {
    id: String,
    /// Rust-only: `key()`, computed on first use.
    key: OnceLock<InputKey>,
}

/// Rust-only (performance divergence): a copyable stand-in for an `InputId` in records that keep
/// one per entry (`Reference`). Two ids have the same key exactly when they are equal (Java's
/// `InputId#equals`: the same name); the keys are numbered in a process-wide table of id names, so
/// that copying one needs no reference count (Java shares the object).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputKey(u32);

impl InputId {
    // port: InputId#InputId
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            key: OnceLock::new(),
        }
    }

    /// Rust-only: the key of this id (see `InputKey`).
    #[inline]
    pub fn key(&self) -> InputKey {
        *self.key.get_or_init(|| {
            static NAMES: Mutex<Option<crate::fast_hash::IndexSet<String>>> = Mutex::new(None);
            let mut names = NAMES.lock().unwrap_or_else(PoisonError::into_inner);
            let names = names.get_or_insert_with(Default::default);
            let index = match names.get_index_of(self.id.as_str()) {
                Some(index) => index,
                None => names.insert_full(self.id.clone()).0,
            };
            InputKey(u32::try_from(index).expect("fewer than 2^32 input names"))
        })
    }
    // port: InputId#getIdName
    pub fn get_id_name(&self) -> &str {
        &self.id
    }
    // port: InputId#hashCode
    pub fn hash_code(&self) -> i32 {
        JsString::from(self.id.as_str()).hash_code()
    }
}
impl PartialEq for InputId {
    // port: InputId#equals
    fn eq(&self, obj: &Self) -> bool {
        std::ptr::eq(self, obj) || self.id == obj.id
    }
}
impl fmt::Debug for InputId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputId").field("id", &self.id).finish()
    }
}
impl fmt::Display for InputId {
    // port: InputId#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InputId: {}", self.get_id_name())
    }
}

impl std::hash::Hash for InputId {
    // port: InputId#hashCode
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
