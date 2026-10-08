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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/graph/Annotatable.java.

use super::annotation::Annotation;
pub trait Annotatable<N, E, G>: Copy {
    // port: Annotatable#getAnnotation
    fn get_annotation(self, graph: &G) -> Option<&dyn Annotation>;
    // port: Annotatable#setAnnotation
    fn set_annotation(self, graph: &mut G, data: Option<Box<dyn Annotation>>);
    /// Arena transfer used by Graph.pushAnnotations, without cloning Java objects.
    fn take_annotation(self, graph: &mut G) -> Option<Box<dyn Annotation>>;
    // port: Annotatable#getAnnotation
    fn get_annotation_as<A: Annotation>(self, graph: &G) -> Option<&A> {
        self.get_annotation(graph)
            .map(|a| a.as_any().downcast_ref::<A>().expect("ClassCastException"))
    }
    // port: Annotatable#getAnnotation
    fn get_annotation_as_mut<A: Annotation>(self, graph: &mut G) -> Option<&mut A>;
}
