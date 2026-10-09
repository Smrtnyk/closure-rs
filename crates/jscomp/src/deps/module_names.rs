/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/ModuleNames.java.

use closure_rhino::java_lang::string::split;
pub struct ModuleNames;
impl ModuleNames {
    pub const MODULE_SLASH: &'static str = "/";
    // port: ModuleNames#fileToModuleName
    pub fn file_to_module_name(path: &str) -> String {
        Self::to_module_name(&Self::escape_path(path))
    }
    // port: ModuleNames#fileToJsIdentifier
    pub fn file_to_js_identifier(path: &str) -> String {
        Self::to_js_identifier(&Self::escape_path(path))
    }
    // port: ModuleNames#escapePath
    pub fn escape_path(input: &str) -> String {
        Self::canonicalize_path(
            &input
                .replace(':', "-")
                .replace('\\', "/")
                .replace(' ', "%20")
                .replace('[', "%5B")
                .replace(']', "%5D")
                .replace('<', "%3C")
                .replace('>', "%3E"),
        )
    }
    // port: ModuleNames#toJSIdentifier
    #[allow(clippy::collapsible_str_replace)] // Java's chain of String#replace calls, kept as written.
    pub fn to_js_identifier(path: &str) -> String {
        let stripped = Self::strip_js_extension(path);
        // replaceFirst("^\\./", ""): the anchored pattern can only match a leading "./".
        stripped
            .strip_prefix("./")
            .unwrap_or(&stripped)
            .replace(Self::MODULE_SLASH, "$")
            .replace('\\', "$")
            .replace('@', "$")
            .replace('+', "$")
            .replace('!', "$")
            .replace('-', "_")
            .replace(':', "_")
            .replace('.', "_")
            .replace("%20", "_")
    }
    // port: ModuleNames#toModuleName
    pub fn to_module_name(path: &str) -> String {
        format!(
            "module${}",
            Self::to_js_identifier(path.strip_prefix('/').unwrap_or(path))
        )
    }
    // port: ModuleNames#stripJsExtension
    fn strip_js_extension(file_name: &str) -> String {
        if let Some(name) = file_name.strip_suffix(".js") {
            name.strip_suffix(".js.i").unwrap_or(name).into()
        } else {
            file_name.into()
        }
    }
    // port: ModuleNames#canonicalizePath
    pub fn canonicalize_path(path: &str) -> String {
        let parts = split(path, Self::MODULE_SLASH);
        let mut buffer: Vec<Option<&str>> = vec![None; parts.len()];
        let mut position = 0;
        let mut available = 0;

        let absolute_path = parts.len() > 1 && parts[0].is_empty();
        if absolute_path {
            // If the path starts with "/" (so the left side, index zero, is empty), then the path will
            // always remain absolute. Make the first segment unavailable to touch.
            available -= 1;
        }

        for part in &parts {
            if part == "." {
                continue;
            }

            if part == ".." {
                if available > 0 {
                    // Consume the previous segment.
                    position -= 1;
                    available -= 1;
                    buffer[position] = None;
                } else if !absolute_path {
                    // If this is a relative path, retain "..", as it can't be consumed on the left.
                    buffer[position] = Some(part);
                    position += 1;
                }
                continue;
            }

            buffer[position] = Some(part);
            position += 1;
            available += 1;
        }

        if absolute_path && position == 1 {
            return Self::MODULE_SLASH.into(); // special-case single absolute segment as joining [""] doesn't work
        }
        // port: ModuleNames#MODULE_JOINER (Joiner.on(MODULE_SLASH).join(Arrays.copyOf(buffer, position)))
        buffer[..position]
            .iter()
            .map(|part| part.expect("null"))
            .collect::<Vec<_>>()
            .join(Self::MODULE_SLASH)
    }
}
