/*
 * Copyright 2016 The Closure Compiler Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/Xid.java.

use closure_rhino::js_string::JsString;
use std::sync::Arc;
pub trait HashFunction: Send + Sync {
    // port: Xid.HashFunction#hashCode
    fn hash_code(&self, value: &JsString) -> i32;
}
impl<F: Fn(&JsString) -> i32 + Send + Sync> HashFunction for F {
    // port: Xid.HashFunction#hashCode
    fn hash_code(&self, value: &JsString) -> i32 {
        self(value)
    }
}
pub struct Xid {
    hasher: Arc<dyn HashFunction>,
}
impl Xid {
    const START_CHARS: &'static [u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const CHARS: &'static [u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    const START_RADIX: i64 = Self::START_CHARS.len() as i64;
    const RADIX: i32 = Self::CHARS.len() as i32;
    // port: Xid#Xid()
    pub fn new() -> Self {
        Self {
            hasher: Arc::new(|value: &JsString| value.hash_code()),
        }
    }
    // port: Xid#Xid(HashFunction)
    pub fn with_hasher(hasher: Arc<dyn HashFunction>) -> Self {
        Self { hasher }
    }
    // port: Xid#get
    pub fn get(&self, key: impl Into<JsString>) -> String {
        Self::to_string(self.get_as_int(key))
    }
    // port: Xid#getAsInt
    pub fn get_as_int(&self, key: impl Into<JsString>) -> i32 {
        self.hasher.hash_code(&key.into())
    }
    // port: Xid#toString
    pub fn to_string(mut i: i32) -> String {
        let mut buf = String::with_capacity(6);
        let l = i as i64 - i32::MIN as i64;
        buf.push(Self::START_CHARS[(l % Self::START_RADIX) as usize] as char);
        i = (l / Self::START_RADIX) as i32;
        while i > 0 {
            buf.push(Self::CHARS[(i % Self::RADIX) as usize] as char);
            i /= Self::RADIX;
        }
        buf
    }
}
impl Default for Xid {
    // port: Xid#Xid()
    fn default() -> Self {
        Self::new()
    }
}
