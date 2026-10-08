/*
 * Copyright (c) 1996, 2021, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
/*
 * Copyright (c) 1996, 2023, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
/*
 * Copyright (c) 1996, 2022, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1):
//   java.base/java/util/zip/Deflater.java, java.base/java/util/zip/DeflaterOutputStream.java,
//   java.base/java/util/zip/GZIPOutputStream.java.

//! The `java.util.zip.GZIPOutputStream` behaviour of [`FastGzipOutputStream`]: the fixed header,
//! the CRC-32 and size trailer, and `write`/`flush` over the raw deflate stream.

use super::FastGzipOutputStream;
use std::io::{self, Write};

/// port: java.util.zip.GZIPOutputStream#GZIP_MAGIC
const GZIP_MAGIC: u16 = 0x8b1f;
/// port: java.util.zip.Deflater#DEFLATED
const DEFLATED: u8 = 8;
/// port: java.util.zip.GZIPOutputStream#OS_UNKNOWN
const OS_UNKNOWN: u8 = 255;
/// port: java.util.zip.Deflater#BEST_SPEED
pub(super) const BEST_SPEED: u32 = 1;

// port: GZIPOutputStream#writeHeader
pub(super) fn write_header<W: Write>(out: &mut W) -> io::Result<()> {
    out.write_all(&[
        GZIP_MAGIC as u8,        // Magic number (short)
        (GZIP_MAGIC >> 8) as u8, // Magic number (short)
        DEFLATED,                // Compression method (CM)
        0,                       // Flags (FLG)
        0,                       // Modification time MTIME (int)
        0,                       // Modification time MTIME (int)
        0,                       // Modification time MTIME (int)
        0,                       // Modification time MTIME (int)
        0,                       // Extra flags (XFLG)
        OS_UNKNOWN,              // Operating system (OS)
    ])
}

impl<W: Write> FastGzipOutputStream<W> {
    /// Finishes writing compressed data to the output stream without closing the underlying
    /// stream, and returns it.
    // port: GZIPOutputStream#finish
    pub fn finish(mut self) -> io::Result<W> {
        self.def.try_finish()?;
        let crc = self.crc.sum();
        let total_in = self.crc.amount();
        let mut out = self.def.finish()?;
        // port: GZIPOutputStream#writeTrailer
        out.write_all(&crc.to_le_bytes())?; // CRC-32 of uncompr. data
        out.write_all(&total_in.to_le_bytes())?; // Number of uncompr. bytes
        out.flush()?;
        Ok(out)
    }
}

impl<W: Write> Write for FastGzipOutputStream<W> {
    // port: GZIPOutputStream#write
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.def.write(buf)?;
        self.crc.update(&buf[..n]);
        Ok(n)
    }

    /// Java's `DeflaterOutputStream#flush` without `syncFlush` (the GZIPOutputStream default)
    /// only flushes the underlying stream: flate2's `flush` would emit a sync-flush block.
    // port: DeflaterOutputStream#flush
    fn flush(&mut self) -> io::Result<()> {
        self.def.get_mut().flush()
    }
}
