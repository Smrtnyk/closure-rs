/*
 * Copyright (C) 2010 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/stream/JsonScope.java.

// Gson JsonScope constants.
pub const EMPTY_ARRAY: i32 = 1;
pub const NONEMPTY_ARRAY: i32 = 2;
pub const EMPTY_OBJECT: i32 = 3;
pub const DANGLING_NAME: i32 = 4;
pub const NONEMPTY_OBJECT: i32 = 5;
pub const EMPTY_DOCUMENT: i32 = 6;
pub const NONEMPTY_DOCUMENT: i32 = 7;
pub const CLOSED: i32 = 8;
