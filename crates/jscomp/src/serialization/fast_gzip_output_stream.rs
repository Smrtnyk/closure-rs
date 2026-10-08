/*
 * Copyright 2026 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/FastGzipOutputStream.java.

//! Port of serialization/FastGzipOutputStream.java.
//!
//! Java's `GZIPOutputStream` writes a fixed 10-byte header (no mtime, XFL 0, OS 255 since JDK 16),
//! raw deflate data, then the CRC-32 and the input size modulo 2^32, both little-endian. This
//! writes the same bytes (flate2's `GzEncoder` would set XFL from the level), deflating with zlib
//! at `Deflater.BEST_SPEED` like Java's `Deflater(DEFAULT_COMPRESSION, true)` with `setLevel(1)`.
use flate2::Compression;
use flate2::Crc;
use flate2::write::DeflateEncoder;
use std::io::{self, Write};

// The java.util.zip.GZIPOutputStream parts (header, trailer, write, flush) and the Deflater
// constants, ported from OpenJDK (GPL-2.0 with the Classpath exception), are in their own file.
#[path = "fast_gzip_output_stream_jdk.rs"]
mod jdk;

/// port: FastGzipOutputStream
///
/// A GZIPOutputStream that uses BEST_SPEED compression level and a reasonable buffer size.
pub struct FastGzipOutputStream<W: Write> {
    def: DeflateEncoder<W>,
    crc: Crc,
}

impl<W: Write> FastGzipOutputStream<W> {
    pub const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;

    // port: FastGzipOutputStream#<init>(OutputStream)
    pub fn new(out: W) -> io::Result<Self> {
        Self::with_size(out, Self::DEFAULT_BUFFER_SIZE)
    }

    /// `size` is the deflater's output buffer size in Java; it does not change the bytes written.
    // port: FastGzipOutputStream#<init>(OutputStream,int)
    pub fn with_size(mut out: W, _size: usize) -> io::Result<Self> {
        jdk::write_header(&mut out)?;
        Ok(Self {
            def: DeflateEncoder::new(out, Compression::new(jdk::BEST_SPEED)),
            crc: Crc::new(),
        })
    }
}
