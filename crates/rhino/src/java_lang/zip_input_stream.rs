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
 * Copyright (c) 2013, 2022, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 1995, 2022, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/util/zip/CRC32.java,
//   java.base/java/util/zip/ZipEntry.java, java.base/java/util/zip/ZipInputStream.java,
//   java.base/java/util/zip/ZipUtils.java.

//! The ZipInputStream entry enumeration path used by SourceFile.fromZipInput.
use super::{charset::Charset, io_exception::IOException};
use std::collections::VecDeque;
use std::io::{self, BufReader, Read};
pub struct ZipInputStream<R: Read> {
    input: BufReader<R>,
    charset: Charset,
    entry: Option<Entry>,
    pushback: VecDeque<u8>,
}
struct Entry {
    name: String,
    flag: u16,
    method: u16,
    crc: u64,
    csize: u64,
    size: u64,
}
impl<R: Read> ZipInputStream<R> {
    // port: ZipInputStream#ZipInputStream(InputStream,Charset)
    pub fn new(input: R, charset: Charset) -> Self {
        Self {
            input: BufReader::new(input),
            charset,
            entry: None,
            pushback: VecDeque::new(),
        }
    }
    // port: ZipInputStream#getNextEntry
    pub fn get_next_entry(&mut self) -> Result<Option<String>, IOException> {
        if self.entry.is_some() {
            self.close_entry()?;
        }
        self.entry = self.read_loc()?;
        Ok(self.entry.as_ref().map(|e| e.name.clone()))
    }
    // port: ZipInputStream#readLOC
    fn read_loc(&mut self) -> Result<Option<Entry>, IOException> {
        let mut header = [0u8; 30];
        match self.read_fully(&mut header) {
            Err(e) if e.get_message() == "null" => return Ok(None),
            Err(e) => return Err(e),
            Ok(()) => {}
        }
        if get32(&header, 0) != 0x04034b50 {
            return Ok(None);
        }
        let flag = get16(&header, 6);
        let mut name = vec![0; get16(&header, 26) as usize];
        self.read_fully(&mut name)?;
        let charset = if flag & 0x800 != 0 {
            Charset::UTF_8
        } else {
            self.charset
        };
        // Charset#decode(..., false) rejects lone surrogates and all other malformed sequences.
        let name = charset
            .decode(&name, false)
            .unwrap_or_else(|e| panic!("java.nio.charset.MalformedInputException: {e}"))
            .to_string_lossy();
        if flag & 1 == 1 {
            return Err(IOException::new("encrypted ZIP entry not supported"));
        }
        let method = get16(&header, 8);
        if flag & 8 == 8 && method != 8 {
            return Err(IOException::new(
                "only DEFLATED entries can have EXT descriptor",
            ));
        }
        let mut entry = Entry {
            name,
            flag,
            method,
            crc: get32(&header, 14),
            csize: get32(&header, 18),
            size: get32(&header, 22),
        };
        let mut extra = vec![0; get16(&header, 28) as usize];
        self.read_fully(&mut extra)?;
        // port: ZipEntry#setExtra0 (ZIP64 local-header size fields)
        let mut p = 0;
        while p + 4 <= extra.len() {
            let tag = get16(&extra, p);
            let len = get16(&extra, p + 2) as usize;
            p += 4;
            if p + len > extra.len() {
                break;
            }
            if tag == 1
                && (entry.csize == u32::MAX as u64 || entry.size == u32::MAX as u64)
                && len >= 16
            {
                entry.size = get64(&extra, p);
                entry.csize = get64(&extra, p + 8);
            }
            p += len;
        }
        Ok(Some(entry))
    }
    // port: ZipInputStream#closeEntry / ZipInputStream#read(byte[],int,int)
    pub fn close_entry(&mut self) -> Result<(), IOException> {
        let Some(mut entry) = self.entry.take() else {
            return Ok(());
        };
        let mut crc = u32::MAX;
        let mut written = 0u64;
        let mut buf = [0; 8192];
        match entry.method {
            8 => {
                let read;
                {
                    let mut decoder = flate2::bufread::DeflateDecoder::new(&mut self.input);
                    loop {
                        let n = decoder
                            .read(&mut buf)
                            .map_err(|e| IOException::new(e.to_string()))?;
                        if n == 0 {
                            break;
                        }
                        written += n as u64;
                        crc = update_crc(crc, &buf[..n]);
                    }
                    read = decoder.total_in();
                }
                self.read_end(&mut entry, written, read, (!crc) as u64)?;
            }
            0 => {
                if entry.size == 0 {
                    return Ok(());
                }
                while written < entry.size {
                    let count = (entry.size - written).min(buf.len() as u64) as usize;
                    let n = self
                        .input
                        .read(&mut buf[..count])
                        .map_err(|e| IOException::new(e.to_string()))?;
                    if n == 0 {
                        return Err(IOException::new("unexpected EOF"));
                    }
                    written += n as u64;
                    crc = update_crc(crc, &buf[..n]);
                }
                if entry.crc != (!crc) as u64 {
                    return Err(IOException::new(format!(
                        "invalid entry CRC (expected 0x{:x} but got 0x{:x})",
                        entry.crc, !crc
                    )));
                }
            }
            _ => return Err(IOException::new("invalid compression method")),
        }
        Ok(())
    }
    // port: ZipInputStream#readEnd
    fn read_end(
        &mut self,
        entry: &mut Entry,
        written: u64,
        read: u64,
        crc: u64,
    ) -> Result<(), IOException> {
        if entry.flag & 8 == 8 {
            let zip64 = written > u32::MAX as u64 || read > u32::MAX as u64;
            let mut descriptor = vec![0; if zip64 { 24 } else { 16 }];
            self.read_fully(&mut descriptor)?;
            let sig = get32(&descriptor, 0);
            let offset = if sig == 0x08074b50 {
                4
            } else {
                self.pushback.extend(&descriptor[descriptor.len() - 4..]);
                0
            };
            entry.crc = get32(&descriptor, offset);
            if zip64 {
                entry.csize = get64(&descriptor, offset + 4);
                entry.size = get64(&descriptor, offset + 12);
            } else {
                entry.csize = get32(&descriptor, offset + 4);
                entry.size = get32(&descriptor, offset + 8);
            }
        }
        if entry.size != written {
            return Err(IOException::new(format!(
                "invalid entry size (expected {} but got {written} bytes)",
                entry.size
            )));
        }
        if entry.csize != read {
            return Err(IOException::new(format!(
                "invalid entry compressed size (expected {} but got {read} bytes)",
                entry.csize
            )));
        }
        if entry.crc != crc {
            return Err(IOException::new(format!(
                "invalid entry CRC (expected 0x{:x} but got 0x{crc:x})",
                entry.crc
            )));
        }
        Ok(())
    }
    // port: ZipInputStream#readFully
    fn read_fully(&mut self, bytes: &mut [u8]) -> Result<(), IOException> {
        let mut pos = 0;
        while pos < bytes.len() {
            if let Some(b) = self.pushback.pop_front() {
                bytes[pos] = b;
                pos += 1;
            } else {
                break;
            }
        }
        self.input.read_exact(&mut bytes[pos..]).map_err(|e| {
            IOException::new(if e.kind() == io::ErrorKind::UnexpectedEof {
                "null".into()
            } else {
                e.to_string()
            })
        })
    }
}
// port: ZipUtils#get16
fn get16(bytes: &[u8], p: usize) -> u16 {
    u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap())
}
// port: ZipUtils#get32
fn get32(bytes: &[u8], p: usize) -> u64 {
    u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) as u64
}
// port: ZipUtils#get64
fn get64(bytes: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(bytes[p..p + 8].try_into().unwrap())
}
// port: CRC32#updateBytes
fn update_crc(mut crc: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb88320
            } else {
                crc >> 1
            };
        }
    }
    crc
}
