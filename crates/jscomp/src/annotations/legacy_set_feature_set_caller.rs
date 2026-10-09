/*
 * Copyright 2022 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/annotations/LegacySetFeatureSetCaller.java.

//! Annotation on methods that still call the deprecated `CompilerOptions.legacySetOutputFeatureSet`
//! API.
//!
//! Current code is advised to instead use the supported `setBrowserFeaturesetYear` or
//! `setLanguageOut` APIs.
//!
//! `@Target(ElementType.METHOD) @Retention(RetentionPolicy.CLASS)`: a marker with no runtime
//! behavior. Rust has no annotations; the methods Java marks with it carry a
//! `/// @LegacySetFeatureSetCaller` doc line instead.
// port: LegacySetFeatureSetCaller
