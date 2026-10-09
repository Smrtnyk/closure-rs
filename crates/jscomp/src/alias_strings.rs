/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AliasStrings.java.

//! Port of `AliasStrings.java`.
//!
//! A compiler pass for aliasing strings. String declarations contribute to garbage collection,
//! which becomes a problem in large applications. Strings that should be aliased occur many times
//! in the code, or occur on codepaths that get executed frequently.
//!
//! 2017/09/17 Notes: - Turning on this pass usually hurts code size after gzip. - It was
//! originally written to deal with performance problems on some older browser VMs. - However,
//! projects that make heavy use of jslayout may need to enable this pass even for modern
//! browsers, because jslayout generates so many duplicate strings.

use crate::AbstractCompiler;
use crate::compiler_options::AliasStringsMode;
use crate::compiler_pass::CompilerPass;
use crate::js_chunk::JSChunk;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::replace_messages_constants;
use closure_rhino::check_state;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use std::collections::BTreeMap;

/// Prefix for variable names for the aliased strings
// port: AliasStrings#STRING_ALIAS_PREFIX
const STRING_ALIAS_PREFIX: &str = "$$S_";

/// Alias strings longer than 100 characters if aliasStringsMode=AliasStringsMode.LARGE
// port: AliasStrings#ALIAS_LARGE_STRINGS_LENGTH
const ALIAS_LARGE_STRINGS_LENGTH: usize = 100;

pub struct AliasStrings {
    output_string_usage: bool,
    /// Java `TreeMap<String, StringInfo>`: `JsString` orders like `String#compareTo`.
    string_info_map: BTreeMap<JsString, StringInfo>,
    used_hashed_aliases: IndexSet<String>,
    alias_strings_mode: AliasStringsMode,
    /// Map from chunk to the node in that chunk that should parent any string variable
    /// declarations that have to be moved into that chunk
    chunk_var_parent_map: IndexMap<Option<JSChunk>, NodeId>,
    /// package private. This value is AND-ed with the hash function to allow unit tests to reduce
    /// the range of hash values to test collision cases
    pub unit_test_hash_reduction_mask: i32,
}

impl AliasStrings {
    /// Creates an instance.
    ///
    /// Java also takes the chunk graph (`compiler.getChunkGraph()` at every call site); the
    /// compiler owns it, so the pass reads it from the compiler when it needs it.
    // port: AliasStrings#AliasStrings
    pub fn new(output_string_usage: bool, alias_strings_mode: AliasStringsMode) -> Self {
        check_state!(alias_strings_mode != AliasStringsMode::NONE);
        Self {
            output_string_usage,
            string_info_map: BTreeMap::new(),
            used_hashed_aliases: IndexSet::<_>::default(),
            alias_strings_mode,
            chunk_var_parent_map: IndexMap::<_, _>::default(),
            unit_test_hash_reduction_mask: !0,
        }
    }

    // port: AliasStrings#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // logger.fine("Aliasing common strings");

        // Traverse the tree and collect strings
        NodeTraversal::traverse(compiler, root, self);

        // 1st edit pass: replace some strings with aliases
        self.replace_strings_with_aliases(compiler);

        // 2nd edit pass: add variable declarations for aliased strings.
        self.add_alias_declaration_nodes(compiler);

        if self.output_string_usage {
            self.output_string_usage();
        }
    }

    /// Looks up the `StringInfo` object for a JavaScript string. Creates it if necessary.
    // port: AliasStrings#getOrCreateStringInfo
    fn get_or_create_string_info(&mut self, string: &JsString) -> &mut StringInfo {
        let size = self.string_info_map.len() as i32;
        self.string_info_map
            .entry(string.clone())
            .or_insert_with(|| StringInfo::new(size))
    }

    /// Replace strings with references to alias variables.
    // port: AliasStrings#replaceStringsWithAliases
    fn replace_strings_with_aliases(&mut self, compiler: &mut AbstractCompiler) {
        let literals: Vec<JsString> = self.string_info_map.keys().cloned().collect();
        for literal in literals {
            let info = &self.string_info_map[&literal];
            if self.should_replace_with_alias(&literal, info) {
                let occurrences = info.occurrences.clone();
                for node in occurrences {
                    let name = self.get_variable_name(&literal);
                    self.replace_string_with_alias_name(compiler, node, &name, &literal);
                }
            }
        }
    }

    /// Creates a var declaration for each aliased string. Var declarations are inserted as close
    /// to the first use of the string as possible.
    // port: AliasStrings#addAliasDeclarationNodes
    fn add_alias_declaration_nodes(&mut self, compiler: &mut AbstractCompiler) {
        let literals: Vec<JsString> = self.string_info_map.keys().cloned().collect();
        for literal in literals {
            if !self.string_info_map[&literal].is_aliased {
                continue;
            }
            let alias = self.get_variable_name(&literal);
            let info = &self.string_info_map[&literal];
            let name = IR::name(compiler, alias.as_str());
            let value = IR::string(compiler, literal.clone());
            let var = IR::var_with_value(compiler, name, value);
            let first_use = info.occurrences[0];
            var.srcref_tree(compiler, first_use);
            match info.sibling_to_insert_var_decl_before {
                None => info
                    .parent_for_new_var_decl
                    .unwrap()
                    .add_child_to_front(compiler, var),
                Some(sibling) => var.insert_before(compiler, sibling),
            }
            compiler.report_change_to_enclosing_scope(var);
        }
    }

    /// Dictates the policy for replacing a string with an alias.
    ///
    /// `str`: The string literal; `info`: Accumulated information about a string
    // port: AliasStrings#shouldReplaceWithAlias
    fn should_replace_with_alias(&self, str: &JsString, info: &StringInfo) -> bool {
        // Always alias strings if the mode is ALL_AGGRESSIVE.
        if self.alias_strings_mode == AliasStringsMode::ALL_AGGRESSIVE {
            return true;
        }

        // Optimize for code size.  Are aliases smaller than strings?
        //
        // This logic optimizes for the size of uncompressed code, but it tends to
        // get good results for the size of the gzipped code too.
        //
        // gzip actually prefers that strings are not aliased - it compresses N
        // string literals better than 1 string literal and N+1 short variable
        // names, provided each string is within 32k of the previous copy.  We
        // follow the uncompressed logic as insurance against there being multiple
        // strings more than 32k apart.

        let size_of_literal: i32 = 2 + str.length() as i32;
        let count = info.occurrences.len() as i32;
        let size_of_strings = count.wrapping_mul(size_of_literal);
        let size_of_variable = 3;
        //  '6' comes from: 'var =;' in var XXX="...";
        let size_of_aliases = 6
            + size_of_variable
            + size_of_literal // declaration
            + count * size_of_variable; // + uses

        size_of_aliases < size_of_strings
    }

    /// Replaces a string literal with a reference to the string's alias variable.
    // port: AliasStrings#replaceStringWithAliasName
    fn replace_string_with_alias_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        name: &str,
        literal: &JsString,
    ) {
        let name_node = IR::name(compiler, name);
        n.replace_with(compiler, name_node);
        self.string_info_map.get_mut(literal).unwrap().is_aliased = true;
        compiler.report_change_to_enclosing_scope(name_node);
    }

    /// Outputs a log of all strings used more than once in the code.
    // port: AliasStrings#outputStringUsage
    fn output_string_usage(&self) {
        let mut sb = String::from("Strings used more than once:\n");
        for (key, info) in &self.string_info_map {
            let count = info.occurrences.len();
            if count > 1 {
                sb.push_str(&count.to_string());
                sb.push_str(": ");
                sb.push_str(&key.to_string_lossy());
                sb.push('\n');
            }
        }
        // TODO(user): Make this save to file OR output to the application
        // logger.fine(sb.toString()); (the Rust port has no java.util.logging sink)
        let _ = sb;
    }

    /// Returns the JS variable name to be substituted for this string.
    // port: AliasStrings.StringInfo#getVariableName
    fn get_variable_name(&mut self, string_literal: &JsString) -> String {
        if self.string_info_map[string_literal].alias_name.is_none() {
            let id = self.string_info_map[string_literal].id;
            let alias = self.encode_string_as_identifier(id, STRING_ALIAS_PREFIX, string_literal);
            self.string_info_map
                .get_mut(string_literal)
                .unwrap()
                .alias_name = Some(alias);
        }
        self.string_info_map[string_literal]
            .alias_name
            .clone()
            .unwrap()
    }

    /// Returns a legal identifier that uniquely characterizes string 's'.
    ///
    /// We want the identifier to be a function of the string value because that makes the
    /// identifiers stable as the program is changed.
    ///
    /// The digits of a good hash function would be adequate, but for short strings the following
    /// algorithm is easier to work with for unit tests.
    ///
    /// ASCII alphanumerics are mapped to themselves. Other characters are mapped to $XXX or $XXX_
    /// where XXX is a variable number of hex digits. The underscore is inserted as necessary to
    /// avoid ambiguity when the character following is a hex digit. E.g. '\n1' maps to '$a_1',
    /// distinguished by the underscore from '¡' which maps to '$a1'.
    ///
    /// If the string is short enough, this is sufficient. Longer strings are truncated after
    /// encoding an initial prefix and appended with a hash value.
    ///
    /// `id` is the enclosing `StringInfo`'s id.
    // port: AliasStrings.StringInfo#encodeStringAsIdentifier
    fn encode_string_as_identifier(&mut self, id: i32, prefix: &str, s: &JsString) -> String {
        // Limit to avoid generating very long identifiers
        let max_limit: usize = 20;
        let length = s.length();
        let limit = length.min(max_limit);

        let mut sb = String::new();
        sb.push_str(prefix);
        let mut protect_hex = false;

        for i in 0..limit {
            let ch = s.char_at(i);

            if protect_hex {
                if (ch >= b'0' as u16 && ch <= b'9' as u16)
                    || (ch >= b'a' as u16 && ch <= b'f' as u16)
                {
                    // toHexString generate lowercase
                    sb.push('_');
                }
                protect_hex = false;
            }

            if (ch >= b'0' as u16 && ch <= b'9' as u16)
                || (ch >= b'A' as u16 && ch <= b'Z' as u16)
                || (ch >= b'a' as u16 && ch <= b'z' as u16)
            {
                sb.push(ch as u8 as char);
            } else {
                sb.push('$');
                // Integer.toHexString(ch)
                sb.push_str(&format!("{:x}", u32::from(ch)));
                protect_hex = true;
            }
        }

        if length == limit {
            return sb;
        }

        // The identifier is not unique because we omitted part, so add a
        // checksum as a hashcode.
        let hash = s.hash_code() & self.unit_test_hash_reduction_mask;
        sb.push('_');
        // Integer.toHexString(hash)
        sb.push_str(&format!("{:x}", hash as u32));
        let mut encoded = sb;
        if !self.used_hashed_aliases.insert(encoded.clone()) {
            // A collision has been detected (which is very rare). Use the sequence
            // id to break the tie. This means that the name is no longer invariant
            // across source code changes and recompilations.
            encoded += &format!("_{id}");
        }
        encoded
    }
}

impl Callback for AliasStrings {
    // port: AliasStrings#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::TEMPLATELIT | Token::TAGGED_TEMPLATELIT | Token::TEMPLATELIT_SUB => {
                // technically redundant, since it must be a child of the others
                // TODO(bradfordcsmith): Consider replacing long and/or frequently occurring
                // substrings within template literals with template substitutions.
                false
            }
            Token::CALL => !replace_messages_constants::is_protected_message(t, n),
            _ => true,
        }
    }

    // port: AliasStrings#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_string_lit(t) && !parent.unwrap().is_reg_exp(t) {
            let str = n.get_string(t);

            // "undefined" is special-cased, since it needs to be used when JS code
            // is unloading and therefore variable references aren't available.
            // This is because of a bug in Firefox.
            if str == "undefined" {
                return;
            }

            if self.alias_strings_mode == AliasStringsMode::LARGE
                && str.length() <= ALIAS_LARGE_STRINGS_LENGTH
            {
                return;
            }

            let occurrence = n;
            let info = self.get_or_create_string_info(&str);

            info.occurrences.push(occurrence);

            // The current chunk.
            let mut chunk = t.get_chunk();
            let info = &self.string_info_map[&str];
            if info.occurrences.len() != 1 {
                // Check whether the current chunk depends on the chunk containing
                // the declaration.
                if let (Some(current), Some(decl_chunk)) = (&chunk, &info.chunk_to_contain_decl)
                    && current != decl_chunk
                {
                    // We need to declare this string in the deepest chunk in the
                    // chunk dependency graph that both of these chunks depend on.
                    let chunk_graph = t.get_compiler().get_chunk_graph().unwrap();
                    chunk =
                        chunk_graph.get_deepest_common_dependency_inclusive(current, decl_chunk);
                } else {
                    // use the previously saved insertion location.
                    return;
                }
            }
            let var_parent = match self.chunk_var_parent_map.get(&chunk) {
                Some(&p) => p,
                None => {
                    let p = t.get_compiler().get_node_for_code_insertion(chunk.as_ref());
                    self.chunk_var_parent_map.insert(chunk.clone(), p);
                    p
                }
            };
            let first = var_parent.get_first_child(t);
            let info = self.string_info_map.get_mut(&str).unwrap();
            info.chunk_to_contain_decl = chunk;
            info.parent_for_new_var_decl = Some(var_parent);
            info.sibling_to_insert_var_decl_before = first;
        }
    }
}

impl CompilerPass for AliasStrings {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::process(self, compiler, externs, root);
    }
}

/// A class that holds information about a JavaScript string that might become aliased.
// port: AliasStrings.StringInfo
struct StringInfo {
    id: i32,

    /// set to 'true' when reference to alias created
    is_aliased: bool,

    occurrences: Vec<NodeId>,

    chunk_to_contain_decl: Option<JSChunk>,
    parent_for_new_var_decl: Option<NodeId>,
    sibling_to_insert_var_decl_before: Option<NodeId>,

    alias_name: Option<String>,
}

impl StringInfo {
    // port: AliasStrings.StringInfo#StringInfo
    fn new(id: i32) -> Self {
        Self {
            id,
            is_aliased: false,
            occurrences: Vec::new(),
            chunk_to_contain_decl: None,
            parent_for_new_var_decl: None,
            sibling_to_insert_var_decl_before: None,
            alias_name: None,
        }
    }
}
