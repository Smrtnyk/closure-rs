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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/rhino/RhinoStringPool.java.

//! Interning pool used by Rhino nodes.
use crate::js_string::JsString;
use indexmap::map::raw_entry_v1::{RawEntryApiV1, RawEntryMut};
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

pub struct RhinoStringPool;
// Java's weak interner. Rust-only (performance divergence): split into independently locked shards
// by hash, so that inputs parsed on several threads (jscomp `parallel_parse`) rarely wait for each
// other; which shard holds a string never matters, only that equal strings share one entry. Entries
// are never removed: an interned string lives for the rest of the process (see `JsString`).
// Keep just the canonical object. Lookups borrow its WTF-16 code units, including when
// the optional reference-counted pool is active.
#[derive(PartialEq, Eq)]
struct PoolEntry(JsString);

impl Hash for PoolEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_units().hash(state);
    }
}

impl std::borrow::Borrow<[u16]> for PoolEntry {
    fn borrow(&self) -> &[u16] {
        self.0.as_units()
    }
}

type Interner = crate::fast_hash::IndexMap<PoolEntry, ()>;
const SHARD_BITS: usize = 6;
static INTERNER: OnceLock<Vec<Mutex<Interner>>> = OnceLock::new();
thread_local! {
    static THREAD_CACHE: std::cell::RefCell<Option<Interner>> =
        const { std::cell::RefCell::new(None) };
}
/// Rust-only: with `CLOSURE_RS_REFCOUNTED_NAMES` set in the environment, the pool keeps the
/// reference-counted strings it is given instead of permanent copies (to compare the two).
fn refcounted_names() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("CLOSURE_RS_REFCOUNTED_NAMES").is_some())
}
fn hash_units(units: &[u16]) -> u64 {
    crate::fast_hash::FastBuildHasher::default().hash_one(units)
}

fn shard(hash: u64) -> &'static Mutex<Interner> {
    let shards = INTERNER.get_or_init(|| {
        (0..1 << SHARD_BITS)
            .map(|_| Mutex::new(Interner::default()))
            .collect()
    });
    // The table uses low bits for the bucket and high bits for the control tag. Keep
    // both available within each shard.
    &shards[(hash >> 32) as usize & ((1 << SHARD_BITS) - 1)]
}
impl RhinoStringPool {
    // port: RhinoStringPool#uncheckedEquals
    pub fn unchecked_equals(a: &JsString, b: &JsString) -> bool {
        a.ptr_eq(b)
    }
    /// A snapshot must restore canonical identity even when the pool uses reference-counted
    /// entries. A private string with the same spelling remains a separate object.
    pub(crate) fn is_pool_entry(s: &JsString) -> bool {
        s.is_interned()
            || (refcounted_names() && {
                let units = s.as_units();
                let hash = hash_units(units);
                shard(hash)
                    .lock()
                    .unwrap()
                    .raw_entry_v1()
                    .from_key_hashed_nocheck(hash, units)
                    .is_some_and(|(entry, ())| s.ptr_eq(&entry.0))
            })
    }
    // port: RhinoStringPool#addOrGet
    pub fn add_or_get(s: impl Into<JsString>) -> JsString {
        let s = s.into();
        if s.is_interned() {
            return s;
        }
        let units = s.as_units();
        Self::intern_cached(units, || {
            if refcounted_names() {
                s.clone()
            } else {
                s.leak_interned()
            }
        })
    }

    fn intern_cached(units: &[u16], new: impl FnOnce() -> JsString) -> JsString {
        let hash = hash_units(units);
        THREAD_CACHE.with_borrow_mut(|cache| match cache {
            Some(cache) => match cache
                .raw_entry_mut_v1()
                .from_key_hashed_nocheck(hash, units)
            {
                RawEntryMut::Occupied(found) => found.key().0.clone(),
                RawEntryMut::Vacant(vacant) => {
                    let s = Self::intern_shared(units, hash, new);
                    vacant.insert_hashed_nocheck(hash, PoolEntry(s.clone()), ());
                    s
                }
            },
            None => Self::intern_shared(units, hash, new),
        })
    }
    /// Rust-only (performance divergence): runs `f` with a cache, private to this thread, in front
    /// of the shared pool. The cache holds pool entries (so it returns exactly what the pool would)
    /// and keeps them alive while `f` runs; it spares a thread that parses many strings (jscomp
    /// `parallel_parse`) a shared lock per string.
    pub fn with_thread_cache<R>(f: impl FnOnce() -> R) -> R {
        let previous = THREAD_CACHE.replace(Some(Default::default()));
        let result = f();
        THREAD_CACHE.set(previous);
        result
    }
    /// Rust-only: `addOrGet` of the string of `units` without a temporary string on a hit, through
    /// the thread's cache when it has one (parse workers), so that it takes no shared lock per call.
    pub(crate) fn add_or_get_units_cached(units: &[u16]) -> JsString {
        Self::intern_cached(units, || {
            if refcounted_names() {
                JsString::from_slice(units)
            } else {
                JsString::interned_copy(units)
            }
        })
    }
    /// Intern a bundled spelling without allocating a temporary string on a pool hit.
    pub(crate) fn add_or_get_units(units: &[u16]) -> JsString {
        Self::intern_shared(units, hash_units(units), || {
            if refcounted_names() {
                JsString::from_slice(units)
            } else {
                JsString::interned_copy(units)
            }
        })
    }

    fn intern_shared(units: &[u16], hash: u64, new: impl FnOnce() -> JsString) -> JsString {
        let mut pool = shard(hash).lock().unwrap();
        match pool.raw_entry_mut_v1().from_key_hashed_nocheck(hash, units) {
            RawEntryMut::Occupied(found) => found.key().0.clone(),
            RawEntryMut::Vacant(vacant) => {
                let interned = new();
                vacant.insert_hashed_nocheck(hash, PoolEntry(interned.clone()), ());
                interned
            }
        }
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
