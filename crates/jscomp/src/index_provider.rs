/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/IndexProvider.java.

use std::any::{Any, TypeId};
pub trait IndexProvider<T>: Send
where
    T: Any + Send,
{
    // port: IndexProvider#get
    fn get(&mut self) -> T;
    // port: IndexProvider#getType
    fn get_type(&self) -> TypeId {
        TypeId::of::<T>()
    }
}
pub(crate) trait ErasedIndexProvider: Send {
    fn get(&mut self) -> Box<dyn Any + Send>;
}
pub(crate) struct Provider<T: Any + Send, P: IndexProvider<T>>(
    pub P,
    pub std::marker::PhantomData<T>,
);
impl<T: Any + Send, P: IndexProvider<T>> ErasedIndexProvider for Provider<T, P> {
    fn get(&mut self) -> Box<dyn Any + Send> {
        Box::new(self.0.get())
    }
}
