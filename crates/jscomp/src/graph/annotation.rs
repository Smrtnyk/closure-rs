/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/graph/Annotation.java.

use std::any::Any;
/// Java marker interface; `to_string` is the hook for Java Object.toString in DOT labels.
pub trait Annotation: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Object identity hashes have no JVM-independent numeric value. Marker
    /// annotations use 0; classes overriding Java hashCode override this hook.
    // port: Object#hashCode
    fn hash_code(&self) -> i32 {
        0
    }
    /// Java Object.toString, including the Java binary class name for marker objects.
    fn to_string(&self) -> String;
}
