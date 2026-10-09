/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NameGenerator.java.

use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;

/// "This set is referenced rather than copied, so changes to the set will be reflected
/// in how names are generated."
pub type ReservedNames = std::sync::Arc<std::sync::RwLock<IndexSet<JsString>>>;

pub trait NameGenerator: std::any::Any + Send + Sync {
    // port: NameGenerator#reset(Set,String,Set)
    fn reset(
        &mut self,
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_characters: &IndexSet<u16>,
    );
    // port: NameGenerator#reset(Set,String,Set,Set)
    fn reset_with_first_and_non_first_characters(
        &mut self,
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_first_characters: &IndexSet<u16>,
        reserved_non_first_characters: &IndexSet<u16>,
    );
    // port: NameGenerator#clone(Set,String,Set)
    fn clone(
        &self,
        reserved_names: ReservedNames,
        prefix: JsString,
        reserved_characters: &IndexSet<u16>,
    ) -> Box<dyn NameGenerator>;
    // port: NameGenerator#generateNextName
    fn generate_next_name(&mut self) -> JsString;
}
