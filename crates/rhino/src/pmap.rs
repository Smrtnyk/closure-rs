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
//   src/com/google/javascript/rhino/PMap.java.

//! Port of the persistent immutable map interface.
pub trait PMap<K, V>: Sized {
    // port: PMap#isEmpty
    fn is_empty(&self) -> bool;
    // port: PMap#values
    fn values<'a>(&'a self) -> impl Iterator<Item = &'a V>
    where
        V: 'a;
    // port: PMap#keys
    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where
        K: 'a;
    // port: PMap#get
    fn get(&self, key: &K) -> Option<&V>;
    // port: PMap#plus
    fn plus(&self, key: K, value: V) -> Self;
    // port: PMap#minus
    fn minus(&self, key: &K) -> Self;
    // port: PMap#reconcile
    fn reconcile(&self, that: &Self, joiner: &mut impl Reconciler<K, V>) -> Self;
    // port: PMap#equivalent
    fn equivalent(&self, that: &Self, equivalence: impl FnMut(&V, &V) -> bool) -> bool;
}
pub trait Reconciler<K, V> {
    // port: PMap.Reconciler#merge
    fn merge(&mut self, key: &K, this_val: Option<&V>, that_val: Option<&V>) -> V;
}
impl<K, V, F: FnMut(&K, Option<&V>, Option<&V>) -> V> Reconciler<K, V> for F {
    // port: PMap.Reconciler#merge
    fn merge(&mut self, key: &K, this_val: Option<&V>, that_val: Option<&V>) -> V {
        self(key, this_val, that_val)
    }
}
