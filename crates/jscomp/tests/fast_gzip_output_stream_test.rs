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

//! Rust-only: FastGzipOutputStream writes the bytes Java 21's GZIPOutputStream writes at
//! BEST_SPEED (expected bytes produced with the reference JDK, tools/env.sh).
use closure_jscomp::serialization::fast_gzip_output_stream::FastGzipOutputStream;
use std::io::Write;

#[test]
fn matches_java_gzip_output_stream_best_speed() {
    let mut out = FastGzipOutputStream::new(Vec::new()).unwrap();
    out.write_all(b"hello hello hello world typed ast").unwrap();
    let bytes = out.finish().unwrap();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex,
        "1f8b08000000000000ffcb48cdc9c957c84022cbf38b7252144a2a0b525314128b4b000cf8d13121000000"
    );
}
