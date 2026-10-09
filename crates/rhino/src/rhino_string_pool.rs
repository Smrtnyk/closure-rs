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
//   src/com/google/javascript/rhino/RhinoStringPool.java.

//! Interning pool used by Rhino nodes.
use crate::js_string::JsString;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub struct RhinoStringPool;
// Java's weak interner. Rust-only (D-025): split into independently locked shards by hash, so
// that inputs parsed on several threads (jscomp `parallel_parse`) rarely wait for each other;
// which shard holds a string never matters, only that equal strings share one entry.
type Interner = crate::fast_hash::IndexMap<Box<[u16]>, Weak<[u16]>>;
const SHARD_BITS: u32 = 6;
static INTERNER: OnceLock<Vec<Mutex<Interner>>> = OnceLock::new();
thread_local! {
    static THREAD_CACHE: std::cell::RefCell<Option<crate::fast_hash::IndexMap<Box<[u16]>, JsString>>> =
        const { std::cell::RefCell::new(None) };
}
fn shard(units: &[u16]) -> &'static Mutex<Interner> {
    let shards = INTERNER.get_or_init(|| {
        (0..1 << SHARD_BITS)
            .map(|_| Mutex::new(Interner::default()))
            .collect()
    });
    let mut hasher = crate::fast_hash::FastHasher::default();
    units.hash(&mut hasher);
    &shards[(hasher.finish() >> (64 - SHARD_BITS)) as usize]
}
impl RhinoStringPool {
    // port: RhinoStringPool#uncheckedEquals
    pub fn unchecked_equals(a: &JsString, b: &JsString) -> bool {
        a.ptr_eq(b)
    }
    // port: RhinoStringPool#addOrGet
    pub fn add_or_get(s: impl Into<JsString>) -> JsString {
        let s = s.into();
        // Rust-only: a parser thread's own cache of pool entries (see `with_thread_cache`).
        if let Some(cached) = THREAD_CACHE
            .with_borrow(|cache| cache.as_ref().map(|cache| cache.get(s.as_units()).cloned()))
        {
            return cached.unwrap_or_else(|| {
                let interned = Self::add_or_get_shared(s);
                THREAD_CACHE.with_borrow_mut(|cache| {
                    cache
                        .as_mut()
                        .unwrap()
                        .insert(interned.as_units().into(), interned.clone())
                });
                interned
            });
        }
        Self::add_or_get_shared(s)
    }
    /// Rust-only (D-025): runs `f` with a cache, private to this thread, in front of the shared
    /// pool. The cache holds pool entries (so it returns exactly what the pool would) and keeps
    /// them alive while `f` runs; it spares a thread that parses many strings (jscomp
    /// `parallel_parse`) a shared lock per string.
    pub fn with_thread_cache<R>(f: impl FnOnce() -> R) -> R {
        let previous = THREAD_CACHE.replace(Some(Default::default()));
        let result = f();
        THREAD_CACHE.set(previous);
        result
    }
    fn add_or_get_shared(s: JsString) -> JsString {
        let mut pool = shard(s.as_units()).lock().unwrap();
        if let Some(interned) = pool.get(s.as_units()).and_then(Weak::upgrade) {
            return JsString::from_shared(interned);
        }
        // Weak entries cannot retain the string. Remove their keys as well.
        if pool.len().is_multiple_of(1024) {
            pool.retain(|_, v| v.strong_count() != 0);
        }
        pool.insert(s.as_units().into(), Arc::downgrade(s.shared()));
        s
    }
    // port: RhinoStringPool#RhinoStringPool
    fn new() -> Self {
        Self
    }
}
impl Default for RhinoStringPool {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LazyInternedStringList {
    pool: Vec<Mutex<JsString>>,
    is_interned: WriteOnlyBitset,
}
impl LazyInternedStringList {
    // port: LazyInternedStringList#LazyInternedStringList
    pub fn new(pool: Vec<JsString>) -> Self {
        let size = pool.len();
        Self {
            pool: pool.into_iter().map(Mutex::new).collect(),
            is_interned: WriteOnlyBitset::new(size as i32),
        }
    }
    // port: LazyInternedStringList#get
    pub fn get(&self, offset: i32) -> JsString {
        if !self.is_interned.get(offset) {
            let mut slot = self.pool[offset as usize].lock().unwrap();
            let interned = RhinoStringPool::add_or_get(slot.clone());
            *slot = interned.clone();
            self.is_interned.set(offset);
            return interned;
        }
        self.pool[offset as usize].lock().unwrap().clone()
    }
    // port: LazyInternedStringList#stream
    pub fn stream(&self) -> impl Iterator<Item = JsString> + '_ {
        self.pool.iter().map(|s| s.lock().unwrap().clone())
    }
}
pub struct WriteOnlyBitset {
    bits: Vec<AtomicI32>,
}
impl WriteOnlyBitset {
    const ADDRESS_BITS_PER_WORD: i32 = 5;
    // port: WriteOnlyBitset#WriteOnlyBitset
    pub fn new(size: i32) -> Self {
        let length = Self::word_index(size.wrapping_sub(1)).wrapping_add(1);
        assert!(length >= 0, "NegativeArraySizeException");
        Self {
            bits: (0..length).map(|_| AtomicI32::new(0)).collect(),
        }
    }
    // port: WriteOnlyBitset#set
    pub fn set(&self, offset: i32) {
        self.bits[Self::word_index(offset) as usize]
            .fetch_or(1i32.wrapping_shl(offset as u32), Ordering::SeqCst);
    }
    // port: WriteOnlyBitset#get
    pub fn get(&self, offset: i32) -> bool {
        self.bits[Self::word_index(offset) as usize].load(Ordering::SeqCst)
            & 1i32.wrapping_shl(offset as u32)
            != 0
    }
    // port: WriteOnlyBitset#wordIndex
    fn word_index(bit_index: i32) -> i32 {
        bit_index >> Self::ADDRESS_BITS_PER_WORD
    }
}
