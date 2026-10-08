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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/HamtPMap.java, src/com/google/javascript/rhino/PMap.java.

//! Port of Closure's persistent hash array mapped trie.
use crate::{
    check_not_null, check_state,
    java_lang::JavaHashCode,
    pmap::{PMap, Reconciler},
};
use std::{
    collections::VecDeque,
    fmt::{self, Display},
    sync::Arc,
};
const BITS: u32 = 4;
const BITS_SHIFT: u32 = 32 - BITS;
type Children<K, V> = Arc<[Arc<HamtNode<K, V>>]>;
type Tree<K, V> = Option<Arc<HamtNode<K, V>>>;
#[derive(Debug)]
struct HamtNode<K, V> {
    key: Option<K>,
    hash: u32,
    value: Option<V>,
    mask: u32,
    children: Children<K, V>,
}
#[derive(Debug, Clone)]
pub struct HamtPMap<K, V> {
    root: Tree<K, V>,
}
impl<K: Clone + PartialEq + JavaHashCode, V: Clone + PartialEq> HamtPMap<K, V> {
    // port: HamtPMap#empty
    pub fn empty() -> Self {
        Self { root: None }
    } // None is the shared generic EMPTY identity.
    pub fn ptr_eq(&self, that: &Self) -> bool {
        same_tree(&self.root, &that.root)
    }
    // port: HamtPMap#isEmpty
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
    // port: HamtPMap#values
    pub fn values(&self) -> impl Iterator<Item = &V> {
        Iter::new(self.root.as_deref()).map(|n| n.value.as_ref().unwrap())
    }
    // port: HamtPMap#keys
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        Iter::new(self.root.as_deref()).map(|n| n.key.as_ref().unwrap())
    }
    // port: HamtPMap#get
    pub fn get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|n| n.get(key, hash(key)))
    }
    // port: HamtPMap#plus
    pub fn plus(&self, key: K, value: V) -> Self {
        let hash = hash(&key);
        Self {
            root: Some(match &self.root {
                Some(n) => n.plus(key, hash, value),
                None => HamtNode::new(Some(key), hash, Some(value), 0, empty_children()),
            }),
        }
    }
    // port: HamtPMap#minus
    pub fn minus(&self, key: &K) -> Self {
        Self {
            root: self
                .root
                .as_ref()
                .and_then(|n| n.minus(key, hash(key), &mut None)),
        }
    }
    // port: HamtPMap#reconcile
    pub fn reconcile(&self, that: &Self, joiner: &mut impl Reconciler<K, V>) -> Self {
        self.reconcile_nullable(that, |k, a, b| Some(joiner.merge(k, a, b)))
    }
    /// Models Java's nullable callback result so its checkNotNull remains testable.
    // port: HamtPMap#reconcile(PMap, Reconciler)
    pub fn reconcile_nullable(
        &self,
        that: &Self,
        mut joiner: impl FnMut(&K, Option<&V>, Option<&V>) -> Option<V>,
    ) -> Self {
        Self {
            root: HamtNode::reconcile(self.root.clone(), that.root.clone(), &mut |k, a, b| {
                Some(check_not_null!(joiner(k, a, b)))
            }),
        }
    }
    // port: HamtPMap#equivalent
    pub fn equivalent(&self, that: &Self, mut equivalence: impl FnMut(&V, &V) -> bool) -> bool {
        HamtNode::equivalent(self.root.clone(), that.root.clone(), &mut equivalence)
    }
}
impl<K: Clone + PartialEq + JavaHashCode + Display, V: Clone + PartialEq + Display> HamtPMap<K, V> {
    // port: HamtPMap#assertCorrectStructure
    pub fn assert_correct_structure(&self) -> &Self {
        if let Some(n) = &self.root {
            n.assert_correct_structure();
        }
        self
    }
}
impl<K: Clone + PartialEq + JavaHashCode, V: Clone + PartialEq> PMap<K, V> for HamtPMap<K, V> {
    // port: PMap#isEmpty
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
    // port: PMap#values
    fn values<'a>(&'a self) -> impl Iterator<Item = &'a V>
    where
        V: 'a,
    {
        self.values()
    }
    // port: PMap#keys
    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where
        K: 'a,
    {
        self.keys()
    }
    // port: PMap#get
    fn get(&self, key: &K) -> Option<&V> {
        self.get(key)
    }
    // port: PMap#plus
    fn plus(&self, key: K, value: V) -> Self {
        self.plus(key, value)
    }
    // port: PMap#minus
    fn minus(&self, key: &K) -> Self {
        self.minus(key)
    }
    // port: PMap#reconcile
    fn reconcile(&self, that: &Self, joiner: &mut impl Reconciler<K, V>) -> Self {
        self.reconcile(that, joiner)
    }
    // port: PMap#equivalent
    fn equivalent(&self, that: &Self, equivalence: impl FnMut(&V, &V) -> bool) -> bool {
        self.equivalent(that, equivalence)
    }
}
fn same_tree<K, V>(a: &Tree<K, V>, b: &Tree<K, V>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}
// port: HamtPMap#emptyChildren
fn empty_children<K, V>() -> Children<K, V> {
    Arc::from([])
}
// port: HamtPMap#hash
fn hash(key: &impl JavaHashCode) -> u32 {
    (key.hash_code() as u32).reverse_bits()
}
// port: HamtPMap#bucket
fn bucket(hash: u32) -> u32 {
    hash >> BITS_SHIFT
}
// port: HamtPMap#shift
fn shift(hash: u32) -> u32 {
    hash << BITS
}
// port: HamtPMap#unshift
fn unshift(hash: u32, bucket: u32) -> u32 {
    (hash >> BITS) | (bucket << BITS_SHIFT)
}
// port: HamtPMap#compareUnsigned
fn compare_unsigned(left: u32, right: u32) -> i32 {
    let diff = (left >> 2) as i32 - (right >> 2) as i32;
    if diff != 0 {
        diff
    } else {
        (left & 3) as i32 - (right & 3) as i32
    }
}
impl<K: Clone + PartialEq + JavaHashCode, V: Clone + PartialEq> HamtNode<K, V> {
    // port: HamtPMap#HamtPMap
    fn new(
        key: Option<K>,
        hash: u32,
        value: Option<V>,
        mask: u32,
        children: Children<K, V>,
    ) -> Arc<Self> {
        let n = Arc::new(Self {
            key,
            hash,
            value,
            mask,
            children,
        });
        n.check_invariants();
        n
    }
    // port: HamtPMap#checkInvariants
    fn check_invariants(&self) {
        check_state!(self.mask.count_ones() as usize == self.children.len());
    }
    // port: HamtPMap#get(K, int)
    fn get(&self, key: &K, hash: u32) -> Option<&V> {
        if hash == self.hash && self.key.as_ref() == Some(key) {
            return self.value.as_ref();
        }
        let bucket_mask = 1 << bucket(hash);
        if self.mask & bucket_mask != 0 {
            self.children[self.index(bucket_mask)].get(key, shift(hash))
        } else {
            None
        }
    }
    // port: HamtPMap#plus(K, int, V)
    fn plus(self: &Arc<Self>, key: K, mut hash: u32, value: V) -> Arc<Self> {
        if hash == self.hash && self.key.as_ref() == Some(&key) {
            return if self.value.as_ref() == Some(&value) {
                self.clone()
            } else {
                Self::new(
                    Some(key),
                    hash,
                    Some(value),
                    self.mask,
                    self.children.clone(),
                )
            };
        }
        if compare_unsigned(hash, self.hash) < 0 {
            return self.replace_root(key, hash, value);
        }
        let bucket_mask = 1 << bucket(hash);
        hash = shift(hash);
        let index = self.index(bucket_mask);
        if self.mask & bucket_mask != 0 {
            let child = &self.children[index];
            let new_child = child.plus(key, hash, value);
            if Arc::ptr_eq(child, &new_child) {
                self.clone()
            } else {
                self.with_children(self.mask, replace_child(&self.children, index, new_child))
            }
        } else {
            let new_child = Self::new(Some(key), hash, Some(value), 0, empty_children());
            self.with_children(
                self.mask | bucket_mask,
                insert_child(&self.children, index, new_child),
            )
        }
    }
    // port: HamtPMap#replaceRoot
    fn replace_root(&self, key: K, hash: u32, value: V) -> Arc<Self> {
        let bucket_mask = 1 << bucket(self.hash);
        let leaf_hash = shift(self.hash);
        let index = self.index(bucket_mask);
        let new_children = if self.mask & bucket_mask != 0 {
            replace_child(
                &self.children,
                index,
                self.children[index].plus(
                    self.key.clone().unwrap(),
                    leaf_hash,
                    self.value.clone().unwrap(),
                ),
            )
        } else {
            insert_child(
                &self.children,
                index,
                Self::new(
                    self.key.clone(),
                    leaf_hash,
                    self.value.clone(),
                    0,
                    empty_children(),
                ),
            )
        };
        Self::new(
            Some(key),
            hash,
            Some(value),
            self.mask | bucket_mask,
            new_children,
        )
    }
    // port: HamtPMap#minus(K, int, V[])
    fn minus(self: &Arc<Self>, key: &K, hash: u32, value: &mut Option<V>) -> Tree<K, V> {
        if hash == self.hash && self.key.as_ref() == Some(key) {
            let result = Self::delete_root(self.mask, &self.children);
            *value = self.value.clone();
            return result;
        }
        let bucket_mask = 1 << bucket(hash);
        if self.mask & bucket_mask == 0 {
            return Some(self.clone());
        }
        let index = self.index(bucket_mask);
        let child = &self.children[index];
        let new_child = child.minus(key, shift(hash), value);
        match new_child {
            Some(new_child) if Arc::ptr_eq(child, &new_child) => Some(self.clone()),
            None => Some(self.with_children(
                self.mask & !bucket_mask,
                delete_child(&self.children, index),
            )),
            Some(new_child) => {
                Some(self.with_children(self.mask, replace_child(&self.children, index, new_child)))
            }
        }
    }
    // port: HamtPMap#reconcile(HamtPMap, HamtPMap, Reconciler)
    fn reconcile(
        t1: Tree<K, V>,
        t2: Tree<K, V>,
        joiner: &mut impl FnMut(&K, Option<&V>, Option<&V>) -> Option<V>,
    ) -> Tree<K, V> {
        if same_tree(&t1, &t2) {
            return t1;
        }
        let (mut t1, mut t2) = match (t1, t2) {
            (None, Some(t2)) => {
                let value = joiner(t2.key.as_ref().unwrap(), None, t2.value.as_ref());
                let children = t2
                    .children
                    .iter()
                    .map(|c| Self::reconcile(None, Some(c.clone()), joiner).unwrap())
                    .collect::<Vec<_>>()
                    .into();
                return match value {
                    Some(value) => Some(Self::new(
                        t2.key.clone(),
                        t2.hash,
                        Some(value),
                        t2.mask,
                        children,
                    )),
                    None => Self::delete_root(t2.mask, &children),
                };
            }
            (Some(t1), None) => {
                let value = joiner(t1.key.as_ref().unwrap(), t1.value.as_ref(), None);
                let children = t1
                    .children
                    .iter()
                    .map(|c| Self::reconcile(Some(c.clone()), None, joiner).unwrap())
                    .collect::<Vec<_>>()
                    .into();
                return match value {
                    Some(value) => Some(Self::new(
                        t1.key.clone(),
                        t1.hash,
                        Some(value),
                        t1.mask,
                        children,
                    )),
                    None => Self::delete_root(t1.mask, &children),
                };
            }
            (Some(t1), Some(t2)) => (t1, t2),
            _ => unreachable!(),
        };
        let mut same_children_as1 = true;
        let mut same_children_as2 = true;
        let hash_cmp = compare_unsigned(t1.hash, t2.hash);
        let mut key = t1.key.clone();
        let mut hash = t1.hash;
        if hash_cmp < 0 {
            t2 = t2.vacate_root();
            same_children_as2 = false;
        } else if hash_cmp > 0 {
            t1 = t1.vacate_root();
            same_children_as1 = false;
            key = t2.key.clone();
            hash = t2.hash;
        } else if t1.key != t2.key {
            t2 = t2.pivot(t1.key.as_ref().unwrap(), t1.hash);
            same_children_as2 = false;
        }
        let new_value = if t1.value == t2.value {
            t1.value.clone()
        } else {
            joiner(
                t1.key.as_ref().or(t2.key.as_ref()).unwrap(),
                t1.value.as_ref(),
                t2.value.as_ref(),
            )
        };
        let mut new_mask = t1.mask | t2.mask;
        same_children_as1 &= new_mask == t1.mask;
        same_children_as2 &= new_mask == t2.mask;
        let mut new_children = Vec::with_capacity(new_mask.count_ones() as usize);
        let mut mask = new_mask;
        while mask != 0 {
            let child_bit = mask & mask.wrapping_neg();
            mask &= !child_bit;
            let child1 = t1.get_child(child_bit);
            let child2 = t2.get_child(child_bit);
            let child = Self::reconcile(child1.clone(), child2.clone(), joiner);
            same_children_as1 &= same_tree(&child, &child1);
            same_children_as2 &= same_tree(&child, &child2);
            if let Some(child) = child {
                new_children.push(child);
            } else {
                new_mask &= !child_bit;
            }
        }
        if same_children_as1 && t1.value == new_value {
            Some(t1)
        } else if same_children_as2 && t2.value == new_value {
            Some(t2)
        } else if new_value.is_none() {
            Self::delete_root(new_mask, &new_children.into())
        } else {
            Some(Self::new(
                key,
                hash,
                new_value,
                new_mask,
                new_children.into(),
            ))
        }
    }
    // port: HamtPMap#equivalent(HamtPMap, HamtPMap, BiPredicate)
    fn equivalent(
        t1: Tree<K, V>,
        t2: Tree<K, V>,
        equivalence: &mut impl FnMut(&V, &V) -> bool,
    ) -> bool {
        if same_tree(&t1, &t2) {
            return true;
        }
        let (Some(t1), Some(mut t2)) = (t1, t2) else {
            return false;
        };
        if t1.hash != t2.hash {
            return false;
        } else if t1.key != t2.key {
            t2 = t2.pivot(t1.key.as_ref().unwrap(), t1.hash);
            if t2.value.is_none() {
                return false;
            }
        }
        if !equivalence(t1.value.as_ref().unwrap(), t2.value.as_ref().unwrap()) {
            return false;
        }
        let mut mask = t1.mask | t2.mask;
        while mask != 0 {
            let child_bit = mask & mask.wrapping_neg();
            mask &= !child_bit;
            if !Self::equivalent(
                t1.get_child(child_bit),
                t2.get_child(child_bit),
                equivalence,
            ) {
                return false;
            }
        }
        true
    }
    // port: HamtPMap#index
    fn index(&self, bit: u32) -> usize {
        (self.mask & (bit - 1)).count_ones() as usize
    }
    // port: HamtPMap#getChild
    fn get_child(&self, bit: u32) -> Tree<K, V> {
        if self.mask & bit != 0 {
            Some(self.children[self.index(bit)].clone())
        } else {
            None
        }
    }
    // port: HamtPMap#pivot(K, int)
    fn pivot(self: &Arc<Self>, key: &K, hash: u32) -> Arc<Self> {
        self.pivot_recursive(key, hash, None, &mut None)
    }
    // port: HamtPMap#pivot(K, int, HamtPMap, V[])
    fn pivot_recursive(
        self: &Arc<Self>,
        key: &K,
        hash: u32,
        parent: Option<&Self>,
        result: &mut Option<V>,
    ) -> Arc<Self> {
        let mut new_mask = self.mask;
        let mut new_children = self.children.clone();
        if hash == self.hash && self.key.as_ref() == Some(key) {
            *result = self.value.clone();
        } else {
            let search_bucket = bucket(hash);
            let replacement_bucket = bucket(self.hash);
            if search_bucket == replacement_bucket {
                let bucket_mask = 1 << search_bucket;
                if self.mask & bucket_mask != 0 {
                    let index = self.index(bucket_mask);
                    let child = &new_children[index];
                    let new_child = child.pivot_recursive(key, shift(hash), Some(self), result);
                    new_children = replace_child(&new_children, index, new_child);
                }
            } else {
                let search_mask = 1 << search_bucket;
                if self.mask & search_mask != 0 {
                    let index = self.index(search_mask);
                    let child = &new_children[index];
                    let new_child = child.minus(key, shift(hash), result);
                    if let Some(new_child) = new_child {
                        new_children = replace_child(&new_children, index, new_child);
                    } else {
                        new_children = delete_child(&new_children, index);
                        new_mask &= !search_mask;
                    }
                }
                let replacement_mask = 1 << replacement_bucket;
                let index = (new_mask & (replacement_mask - 1)).count_ones() as usize;
                if self.mask & replacement_mask != 0 {
                    let child = &new_children[index];
                    let new_child = child.plus(
                        self.key.clone().unwrap(),
                        shift(self.hash),
                        self.value.clone().unwrap(),
                    );
                    new_children = replace_child(&new_children, index, new_child);
                } else {
                    new_children = insert_child(
                        &new_children,
                        index,
                        Self::new(
                            self.key.clone(),
                            shift(self.hash),
                            self.value.clone(),
                            0,
                            empty_children(),
                        ),
                    );
                    new_mask |= replacement_mask;
                }
            }
        }
        if let Some(parent) = parent {
            Self::new(
                parent.key.clone(),
                shift(parent.hash),
                parent.value.clone(),
                new_mask,
                new_children,
            )
        } else {
            Self::new(
                Some(key.clone()),
                hash,
                result.clone(),
                new_mask,
                new_children,
            )
        }
    }
    // port: HamtPMap#vacateRoot
    fn vacate_root(&self) -> Arc<Self> {
        let bucket_mask = 1 << bucket(self.hash);
        let index = self.index(bucket_mask);
        if self.mask & bucket_mask != 0 {
            let child = self.children[index].plus(
                self.key.clone().unwrap(),
                shift(self.hash),
                self.value.clone().unwrap(),
            );
            return Self::new(
                None,
                0,
                None,
                self.mask,
                replace_child(&self.children, index, child),
            );
        }
        let child = Self::new(
            self.key.clone(),
            shift(self.hash),
            self.value.clone(),
            0,
            empty_children(),
        );
        Self::new(
            None,
            0,
            None,
            self.mask | bucket_mask,
            insert_child(&self.children, index, child),
        )
    }
    // port: HamtPMap#withChildren
    fn with_children(self: &Arc<Self>, mask: u32, children: Children<K, V>) -> Arc<Self> {
        if mask == self.mask && Arc::ptr_eq(&children, &self.children) {
            self.clone()
        } else {
            Self::new(
                self.key.clone(),
                self.hash,
                self.value.clone(),
                mask,
                children,
            )
        }
    }
    // port: HamtPMap#deleteRoot
    fn delete_root(mask: u32, children: &Children<K, V>) -> Tree<K, V> {
        if mask == 0 {
            return None;
        }
        let child = &children[0];
        let new_hash = unshift(child.hash, mask.trailing_zeros());
        let new_child = Self::delete_root(child.mask, &child.children);
        match new_child {
            None => Some(Self::new(
                child.key.clone(),
                new_hash,
                child.value.clone(),
                mask & !(mask & mask.wrapping_neg()),
                delete_child(children, 0),
            )),
            Some(new_child) => Some(Self::new(
                child.key.clone(),
                new_hash,
                child.value.clone(),
                mask,
                replace_child(children, 0, new_child),
            )),
        }
    }
}
// port: HamtPMap#insertChild
fn insert_child<K, V>(
    children: &Children<K, V>,
    index: usize,
    child: Arc<HamtNode<K, V>>,
) -> Children<K, V> {
    let mut new_children = children.to_vec();
    new_children.insert(index, child);
    new_children.into()
}
// port: HamtPMap#replaceChild
fn replace_child<K, V>(
    children: &Children<K, V>,
    index: usize,
    child: Arc<HamtNode<K, V>>,
) -> Children<K, V> {
    let mut new_children = children.to_vec();
    new_children[index] = child;
    new_children.into()
}
// port: HamtPMap#deleteChild
fn delete_child<K, V>(children: &Children<K, V>, index: usize) -> Children<K, V> {
    if children.len() == 1 {
        return empty_children();
    }
    let mut new_children = children.to_vec();
    new_children.remove(index);
    new_children.into()
}
struct Iter<'a, K, V> {
    queue: VecDeque<&'a HamtNode<K, V>>,
}
impl<'a, K, V> Iter<'a, K, V> {
    // port: HamtPMap.Iter#Iter
    fn new(map: Option<&'a HamtNode<K, V>>) -> Self {
        let mut queue = VecDeque::new();
        if let Some(map) = map {
            queue.push_back(map);
        }
        Self { queue }
    }
    // port: HamtPMap.Iter#hasNext
    fn has_next(&self) -> bool {
        !self.queue.is_empty()
    }
    // port: HamtPMap.Iter#remove
    #[allow(dead_code)]
    fn remove(&mut self) {
        panic!("UnsupportedOperationException");
    }
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = &'a HamtNode<K, V>;
    // port: HamtPMap.Iter#next
    fn next(&mut self) -> Option<Self::Item> {
        if !self.has_next() {
            return None;
        }
        let top = self.queue.pop_front().unwrap();
        for c in top.children.iter().rev() {
            self.queue.push_back(c);
        }
        Some(top)
    }
}
impl<K: Display, V: Display> HamtNode<K, V> {
    // port: HamtPMap#appendTo
    fn append_to(&self, sb: &mut String) {
        if sb.len() > 1 {
            sb.push_str(", ");
        }
        sb.push_str(&format!(
            "{}: {}",
            self.key.as_ref().unwrap(),
            self.value.as_ref().unwrap()
        ));
        for child in self.children.iter() {
            child.append_to(sb);
        }
    }
}
impl<K: Clone + PartialEq + JavaHashCode + Display, V: Clone + PartialEq + Display> HamtNode<K, V> {
    // port: HamtPMap#assertCorrectStructure
    fn assert_correct_structure(&self) {
        let key = self.key.as_ref().unwrap();
        let hash = hash(key);
        for child in self.children.iter() {
            let child_key = child.key.as_ref().unwrap();
            let child_hash = crate::hamt_pmap::hash(child_key);
            if compare_unsigned(child_hash, hash) < 0 {
                let mut sb = "{".to_string();
                self.append_to(&mut sb);
                sb.push('}');
                panic!(
                    "Invalid map has decreasing hash {child_key}({child_hash:x}) beneath {key}({hash:x}: {sb}"
                );
            }
            child.assert_correct_structure();
        }
    }
}
impl<K: Display, V: Display> Display for HamtPMap<K, V> {
    // port: HamtPMap#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sb = "{".to_string();
        if let Some(root) = &self.root {
            root.append_to(&mut sb);
        }
        sb.push('}');
        f.write_str(&sb)
    }
}
impl<K, V> PartialEq for HamtPMap<K, V> {
    // port: Object#equals
    // HamtPMap inherits identity equality; clones preserve the root identity.
    fn eq(&self, other: &Self) -> bool {
        same_tree(&self.root, &other.root)
    }
}
