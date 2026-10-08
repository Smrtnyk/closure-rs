/*
 * Copyright (c) 1994, 2023, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 1995, 2023, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Class.java,
//   java.base/java/util/zip/ZipFile.java.

//! The JDK class-loader behaviour that the resource-loading code relies on, over the embedded jar
//! contents ([`JAR_ENTRIES`]).
//!
//! Java's `Class<?> clazz` receiver is represented by the class's fully qualified binary name
//! (for example `"com.google.javascript.jscomp.Compiler"`). Java's `InputStream` result is the
//! entry's [`EntryContent`]: a file's bytes, an (empty) directory entry, or the pre-expanded
//! entries of the nested `externs.zip`.

use crate::jar_contents::{ArchiveEntry, EntryContent, JAR_ENTRIES};

/// The package of a class given by its binary name: everything before the last '.'.
// port: Class#getPackageName
fn get_package_name(class_name: &str) -> &str {
    match class_name.rfind('.') {
        Some(dot) => &class_name[..dot],
        None => "",
    }
}

/// Add a package name prefix if the name is not absolute. Remove leading "/" if name is absolute.
// port: Class#resolveName
pub fn resolve_name(class_name: &str, name: &str) -> String {
    if !name.starts_with('/') {
        let base_name = get_package_name(class_name);
        if !base_name.is_empty() {
            let mut sb = String::with_capacity(base_name.len() + 1 + name.len());
            sb.push_str(&base_name.replace('.', "/"));
            sb.push('/');
            sb.push_str(name);
            return sb;
        }
        name.to_string()
    } else {
        name[1..].to_string()
    }
}

/// The jar lookup behind `ClassLoader#getResource` for a resource in a jar on the class path:
/// an entry named exactly `name`, else (when `name` is not empty and does not end with '/') the
/// directory entry `name + "/"`.
// port: ZipFile#getEntry (ZipFile.Source#getEntryPos(String, boolean addSlash = true))
pub fn get_entry(name: &str) -> Option<&'static ArchiveEntry> {
    let mut dir_pos = None;
    for entry in JAR_ENTRIES {
        if entry.name == name {
            return Some(entry);
        }
        if !name.is_empty()
            && !name.ends_with('/')
            && entry.name.len() == name.len() + 1
            && entry.name.starts_with(name)
            && entry.name.ends_with('/')
        {
            dir_pos = Some(entry);
        }
    }
    dir_pos
}

/// Finds a resource with a given name; `None` when there is none.
// port: Class#getResource
pub fn get_resource(class_name: &str, name: &str) -> Option<&'static ArchiveEntry> {
    let name = resolve_name(class_name, name);
    get_entry(&name)
}

/// Finds a resource with a given name and opens it; `None` when there is none.
// port: Class#getResourceAsStream
pub fn get_resource_as_stream(class_name: &str, name: &str) -> Option<&'static EntryContent> {
    get_resource(class_name, name).map(|entry| &entry.content)
}

impl EntryContent {
    /// Reads the whole stream. A directory entry of a jar reads as an empty stream. The nested
    /// `externs.zip` is stored pre-expanded and is only read entry by entry (as Java does with a
    /// `ZipInputStream`), never as raw bytes.
    // port: InputStream#readAllBytes
    pub fn read_all_bytes(&self) -> &'static [u8] {
        match self {
            EntryContent::Directory => &[],
            EntryContent::File(bytes) => bytes,
            EntryContent::Zip(_) => {
                panic!("externs.zip is stored pre-expanded; read its entries instead")
            }
        }
    }
}

/// Decodes bytes as UTF-8 the way `new InputStreamReader(stream, UTF_8)` does: malformed input is
/// replaced by U+FFFD. Every embedded text resource is valid UTF-8 (checked by the tests), so the
/// replacement path is never taken for them.
// port: CharStreams#toString(new InputStreamReader(InputStream, UTF_8))
pub fn decode_utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    // port: Class#resolveName
    #[test]
    fn resolve_name() {
        assert_eq!(
            super::resolve_name("com.google.javascript.jscomp.Compiler", "js/base.js"),
            "com/google/javascript/jscomp/js/base.js"
        );
        assert_eq!(
            super::resolve_name(
                "com.google.javascript.jscomp.js.RuntimeJsLibManager$FieldsTable",
                "transpilation_libs.txt"
            ),
            "com/google/javascript/jscomp/js/transpilation_libs.txt"
        );
        assert_eq!(
            super::resolve_name("com.google.javascript.jscomp.Compiler", "/externs.zip"),
            "externs.zip"
        );
        assert_eq!(super::resolve_name("Unpackaged", "x/y.js"), "x/y.js");
    }

    // port: ZipFile#getEntry (exact entry first, then the directory entry name + "/")
    #[test]
    fn get_entry_directory_lookup() {
        assert_eq!(
            super::get_entry("com/google/javascript/jscomp/js/es6")
                .unwrap()
                .name,
            "com/google/javascript/jscomp/js/es6/"
        );
        assert_eq!(
            super::get_entry("com/google/javascript/jscomp/js/es6/")
                .unwrap()
                .name,
            "com/google/javascript/jscomp/js/es6/"
        );
        assert_eq!(
            super::get_entry("com/google/javascript/jscomp/js/base.js")
                .unwrap()
                .name,
            "com/google/javascript/jscomp/js/base.js"
        );
        assert!(super::get_entry("com/google/javascript/jscomp/js/base").is_none());
        assert!(super::get_entry("").is_none());
    }
}
