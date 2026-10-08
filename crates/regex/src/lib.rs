/*
 * Copyright 2026 The closure-rs Authors.
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

//! Port of com.google.javascript.jscomp.regex (RegExpTree, CharRanges, CaseCanonicalize).
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]
// Java writes range tests as `a <= x && x <= b` / `x < a || x >= b`; ports keep that shape.
#![allow(clippy::manual_range_contains)]
pub mod case_canonicalize;
pub mod char_ranges;
pub mod reg_exp_tree;
