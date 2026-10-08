/*
 * Copyright (c) 2008, 2023, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 1994, 2024, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 2007, 2023, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/String.java,
//   java.base/java/nio/file/Path.java, java.base/sun/nio/fs/UnixPath.java.

//! JDK 21 sun.nio.fs.UnixPath operations observable in jscomp.deps.
use std::fmt;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnixPath {
    path: String,
}
impl UnixPath {
    // port: UnixPath#UnixPath(UnixFileSystem,String), UnixPath#normalizeAndCheck
    pub fn of(input: &str) -> Self {
        assert!(!input.contains('\0'), "Nul character not allowed: {input}");
        let mut path = String::new();
        let mut prev = '\0';
        for c in input.chars() {
            if c == '/' && prev == '/' {
                continue;
            }
            path.push(c);
            prev = c;
        }
        while path.len() > 1 && path.ends_with('/') {
            path.pop();
        }
        Self { path }
    }
    // port: UnixPath#getParent
    pub fn get_parent(&self) -> Option<Self> {
        if self.path == "/" {
            return None;
        }
        self.path.rfind('/').map(|i| {
            if i == 0 {
                Self::of("/")
            } else {
                Self::of(&self.path[..i])
            }
        })
    }
    // port: UnixPath#isAbsolute
    pub fn is_absolute(&self) -> bool {
        self.path.starts_with('/')
    }
    // port: UnixPath#isEmpty
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }
    // port: UnixPath#initOffsets, UnixPath#getNameCount
    fn names(&self) -> Vec<&str> {
        if self.path == "/" {
            vec![]
        } else {
            self.path
                .strip_prefix('/')
                .unwrap_or(&self.path)
                .split('/')
                .collect()
        }
    }
    // port: UnixPath#hasDotOrDotDot
    fn has_dot_or_dot_dot(&self) -> bool {
        self.names().iter().any(|p| matches!(*p, "." | ".."))
    }
    // port: UnixPath#normalize
    pub fn normalize(&self) -> Self {
        let names = self.names();
        let count = names.len();
        if count == 0 || self.is_empty() {
            return self.clone();
        }
        let mut ignore = vec![false; count];
        let mut remaining = count;
        let mut has_dot_dot = false;
        let is_absolute = self.is_absolute();
        for (i, name) in names.iter().enumerate() {
            if *name == "." {
                ignore[i] = true;
                remaining -= 1;
            } else if name.starts_with("..") {
                has_dot_dot = true;
            }
        }
        if has_dot_dot {
            loop {
                let prev_remaining = remaining;
                let mut prev_name = None;
                for (i, name) in names.iter().enumerate() {
                    if ignore[i] {
                        continue;
                    }
                    if *name != ".." {
                        prev_name = Some(i);
                        continue;
                    }
                    if let Some(prev) = prev_name {
                        ignore[prev] = true;
                        ignore[i] = true;
                        remaining -= 2;
                        prev_name = None;
                    } else if is_absolute && ignore[..i].iter().all(|v| *v) {
                        ignore[i] = true;
                        remaining -= 1;
                    }
                }
                if prev_remaining <= remaining {
                    break;
                }
            }
        }
        if remaining == count {
            return self.clone();
        }
        if remaining == 0 {
            return Self::of(if is_absolute { "/" } else { "" });
        }
        let result: Vec<_> = names
            .iter()
            .enumerate()
            .filter(|(i, _)| !ignore[*i])
            .map(|(_, p)| *p)
            .collect();
        Self::of(&format!(
            "{}{}",
            if is_absolute { "/" } else { "" },
            result.join("/")
        ))
    }
    // port: UnixPath#relativize
    pub fn relativize(&self, obj: &Self) -> Self {
        self.try_relativize(obj)
            .unwrap_or_else(|message| panic!("{message}"))
    }
    /// Rust-only: `relativize` with its `IllegalArgumentException` as an `Err` holding the
    /// message, for callers that catch it as control flow.
    // port: UnixPath#relativize
    pub fn try_relativize(&self, obj: &Self) -> Result<Self, String> {
        if obj == self {
            return Ok(Self::of(""));
        }
        if self.is_absolute() != obj.is_absolute() {
            return Err("'other' is different type of Path".into());
        }
        if self.is_empty() {
            return Ok(obj.clone());
        }
        let (base, child) = if self.has_dot_or_dot_dot() || obj.has_dot_or_dot_dot() {
            (self.normalize(), obj.normalize())
        } else {
            (self.clone(), obj.clone())
        };
        let base_names = base.names();
        let child_names = child.names();
        let mut i = 0;
        while i < base_names.len().min(child_names.len()) && base_names[i] == child_names[i] {
            i += 1;
        }
        let child_remaining = child_names[i..].join("/");
        if i == base_names.len() {
            return Ok(Self::of(&child_remaining));
        }
        let base_remaining = Self::of(&base_names[i..].join("/"));
        if base_remaining.has_dot_or_dot_dot() {
            return Err(format!(
                "Unable to compute relative  path from {self} to {obj}"
            ));
        }
        if base_remaining.is_empty() {
            return Ok(Self::of(&child_remaining));
        }
        let mut result = vec![".."; base_remaining.names().len()];
        if !child_remaining.is_empty() {
            result.push(&child_remaining);
        }
        Ok(Self::of(&result.join("/")))
    }
    // port: UnixPath#resolve(Path)
    pub fn resolve(&self, obj: &Self) -> Self {
        let other = &obj.path;
        if other.starts_with('/') {
            return obj.clone();
        }
        Self {
            path: Self::resolve_bytes(&self.path, other),
        }
    }
    // port: UnixPath#resolve(byte[],byte[])
    fn resolve_bytes(base: &str, child: &str) -> String {
        if child.is_empty() {
            return base.to_owned();
        }
        if base.is_empty() || child.starts_with('/') {
            return child.to_owned();
        }
        if base == "/" {
            format!("/{child}")
        } else {
            format!("{base}/{child}")
        }
    }
    // port: Path#resolveSibling(Path)
    pub fn resolve_sibling(&self, other: &Self) -> Self {
        match self.get_parent() {
            None => other.clone(),
            Some(parent) => parent.resolve(other),
        }
    }
    // port: UnixPath#toAbsolutePath
    pub fn to_absolute_path(&self) -> Self {
        if self.is_absolute() {
            return self.clone();
        }
        let dir = std::env::current_dir()
            .expect("user.dir")
            .to_string_lossy()
            .into_owned();
        if self.is_empty() {
            Self::of(&dir)
        } else {
            Self::of(&format!("{dir}/{}", self.path))
        }
    }
}
impl fmt::Display for UnixPath {
    // port: UnixPath#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // port: UnixPathTest#depsPathAndSplitDifferential (JDK 21 generated fixtures)
    #[test]
    fn deps_path_and_split_differential() {
        let mut fixture_dir = String::new();
        let mut dictionary: Vec<String> = vec![];
        let mut case_count = 0;
        for line in include_str!("testdata/path_split.tsv").lines() {
            let raw: Vec<_> = line.split('\t').collect();
            if raw[0] == "t" {
                assert_eq!(raw[1].parse::<usize>().unwrap(), dictionary.len());
                dictionary.push(raw[2].to_owned());
                continue;
            }
            let f: Vec<_> = raw
                .into_iter()
                .map(|s| {
                    s.strip_prefix('@')
                        .map_or(s, |id| dictionary[id.parse::<usize>().unwrap()].as_str())
                })
                .collect();
            case_count += usize::from(f[0] != "d");
            match f[0] {
                "d" => fixture_dir = decode(f[1]),
                "p" => {
                    let p = UnixPath::of(&decode(f[1]));
                    assert_eq!(p.to_string(), decode(f[2]));
                    assert_eq!(
                        p.get_parent().map(|p| p.to_string()),
                        if f[3] == "-" {
                            None
                        } else {
                            Some(decode(f[3]))
                        }
                    );
                    assert_eq!(p.normalize().to_string(), decode(f[4]));
                    assert_eq!(
                        p.to_absolute_path().to_string(),
                        decode(f[5]).replacen(
                            &fixture_dir,
                            &std::env::current_dir().unwrap().to_string_lossy(),
                            1
                        )
                    );
                }
                "r" => {
                    let p = UnixPath::of(&decode(f[1]));
                    let q = UnixPath::of(&decode(f[2]));
                    let result = std::panic::catch_unwind(|| p.relativize(&q));
                    if let Some(e) = f[3].strip_prefix('e') {
                        let err = result.expect_err("Expected IllegalArgumentException");
                        let msg = err
                            .downcast_ref::<String>()
                            .map(String::as_str)
                            .or_else(|| err.downcast_ref::<&str>().copied())
                            .unwrap();
                        assert_eq!(msg, decode(e), "{p} to {q}");
                    } else {
                        assert_eq!(result.unwrap().to_string(), decode(f[3]), "{p} to {q}");
                    }
                }
                "s" => {
                    let actual =
                        super::super::string::split_units(&decode_units(f[2]), &decode(f[1]));
                    let expected: Vec<_> = f[3].split_terminator(',').map(decode_units).collect();
                    assert_eq!(actual, expected, "{line}");
                }
                _ => panic!("Invalid fixture"),
            }
        }
        assert_eq!(case_count, 806);
    }
    // port: String#charAt (fixture encoding)
    fn decode_units(s: &str) -> crate::js_string::JsString {
        crate::js_string::JsString::from_units(
            (0..s.len())
                .step_by(4)
                .map(|i| u16::from_str_radix(&s[i..i + 4], 16).unwrap())
                .collect::<Vec<_>>(),
        )
    }
    // port: String#charAt (fixture encoding)
    fn decode(s: &str) -> String {
        String::from_utf16(
            &(0..s.len())
                .step_by(4)
                .map(|i| u16::from_str_radix(&s[i..i + 4], 16).unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
}
