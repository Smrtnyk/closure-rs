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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 51d8099:
//   src/com/google/javascript/jscomp/RescopeGlobalSymbolsRenameHelper.java.

use crate::{
    abstract_compiler::AbstractCompiler, compiler_options::OptimizeLocalAccess,
    rescope_global_symbols_info::RescopeGlobalSymbolsInfo, var::VarId,
};
use closure_rhino::check_argument;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId};

/// Appended to non-global variable names that conflict with `globalSymbolNamespace`.
const DISAMBIGUATION_SUFFIX: &str = "$";

/// Prepended to unwrapped local variable names in the defining chunk for wrapped reassignable
/// symbols to avoid collisions with local wrapper aliases across concatenated chunks.
const UNWRAPPED_LOCAL_NAME_PREFIX: &str = "_";

/// Helper class for disambiguating variable names in `RescopeGlobalSymbols` and
/// `RescopeGlobalSymbolsRewriteCallback`.
///
/// Consolidates two variable-renaming mechanisms:
///
/// 1. **Namespace-conflict disambiguation (`$` suffix):** Non-global variables whose names match
///    `globalSymbolNamespace` (e.g. `_`) or start with `globalSymbolNamespace + "$"` (e.g. `_$`)
///    have [`DISAMBIGUATION_SUFFIX`] (`$`) appended so they do not shadow the global symbol
///    namespace parameter.
/// 2. **Defining-chunk unwrapped local variable disambiguation (`_` prefix):** When
///    `CompilerOptions.OptimizeLocalAccess#shouldKeepLocalAliasForWrappedReassignables()` is
///    enabled, wrapped cross-chunk symbols that remain unwrapped local variables in their defining
///    chunk are renamed by prepending [`UNWRAPPED_LOCAL_NAME_PREFIX`] (`_`, e.g. `_a`) so they do
///    not collide with local wrapper aliases (`var {a} = _;`) when multiple chunks are
///    concatenated into the same function scope. If `_a` is already used (including by
///    post-`$`-disambiguation local variables), additional `_` prefixes are prepended until a free
///    identifier is found.
///
/// Java keeps the compiler in a field; here the methods that need it take it (crates/DESIGN.md
/// §6).
pub struct RescopeGlobalSymbolsRenameHelper {
    global_symbol_namespace: JsString,

    /// Map from global wrapped symbols that are kept as unwrapped local variables in their
    /// defining chunk to their disambiguated local variable name (e.g. `_a`), avoiding collisions
    /// with local wrapper aliases (`var {a} = _;`) when multiple chunks are concatenated into the
    /// same function scope.
    unwrapped_local_names_in_defining_chunk: IndexMap<JsString, JsString>,
}

/// Builder for [`RescopeGlobalSymbolsRenameHelper`] that records identifiers encountered during
/// the pre-traversal and collects the disambiguated unwrapped local variable names in the defining
/// chunk when [`HelperBuilder::build`] is called.
// port: RescopeGlobalSymbolsRenameHelper.HelperBuilder
pub struct HelperBuilder {
    global_symbol_namespace: JsString,
    optimize_local_access: OptimizeLocalAccess,

    /// Identifiers starting with `_` (including post-disambiguation names for local variables
    /// conflicting with `globalSymbolNamespace`) used to avoid collisions when naming unwrapped
    /// local variables in the defining chunk.
    names_starting_with_underscore: IndexSet<JsString>,
}

impl HelperBuilder {
    // port: RescopeGlobalSymbolsRenameHelper.HelperBuilder#HelperBuilder
    pub fn new(
        global_symbol_namespace: JsString,
        optimize_local_access: OptimizeLocalAccess,
    ) -> Self {
        Self {
            global_symbol_namespace,
            optimize_local_access,
            names_starting_with_underscore: IndexSet::<_>::default(),
        }
    }

    /// Records an identifier seen during the pre-traversal so that unwrapped local variable names
    /// in the defining chunk (which start with [`UNWRAPPED_LOCAL_NAME_PREFIX`]) do not collide with
    /// existing identifiers or with local variables after [`DISAMBIGUATION_SUFFIX`] renaming.
    // port: RescopeGlobalSymbolsRenameHelper.HelperBuilder#recordExistingName
    pub fn record_existing_name(&mut self, name: &JsString) {
        if !self
            .optimize_local_access
            .should_keep_local_alias_for_wrapped_reassignables()
        {
            return;
        }
        if name.starts_with(UNWRAPPED_LOCAL_NAME_PREFIX) {
            self.names_starting_with_underscore.insert(name.clone());
        }
        // Non-global variables matching `globalSymbolNamespace` (e.g. `_`) or starting with
        // `globalSymbolNamespace + "$"` (e.g. `_$`) are renamed by appending `$` in
        // maybeDisambiguateLocalVar so they do not shadow the namespace parameter. Record that
        // post-disambiguation name (e.g. `_$`) so that an unwrapped local variable in the defining
        // chunk for a global symbol like `$` (which would otherwise be named `_$`) does not
        // collide with or get shadowed by the disambiguated local variable.
        if RescopeGlobalSymbolsRenameHelper::conflicts_with_global_symbol_namespace(
            name,
            &self.global_symbol_namespace,
        ) {
            let disambiguated_name =
                RescopeGlobalSymbolsRenameHelper::get_disambiguated_local_name(name);
            if disambiguated_name.starts_with(UNWRAPPED_LOCAL_NAME_PREFIX) {
                self.names_starting_with_underscore
                    .insert(disambiguated_name);
            }
        }
    }

    /// Collects the disambiguated local variable names in the defining chunk from all recorded
    /// names and creates the [`RescopeGlobalSymbolsRenameHelper`].
    // port: RescopeGlobalSymbolsRenameHelper.HelperBuilder#build
    pub fn build(self, symbol_info: &RescopeGlobalSymbolsInfo) -> RescopeGlobalSymbolsRenameHelper {
        let unwrapped_local_names_in_defining_chunk =
            RescopeGlobalSymbolsRenameHelper::collect_unwrapped_local_names_in_defining_chunk(
                self.optimize_local_access,
                symbol_info,
                &self.names_starting_with_underscore,
            );
        RescopeGlobalSymbolsRenameHelper::new(
            self.global_symbol_namespace,
            unwrapped_local_names_in_defining_chunk,
        )
    }
}

impl RescopeGlobalSymbolsRenameHelper {
    // port: RescopeGlobalSymbolsRenameHelper#RescopeGlobalSymbolsRenameHelper
    fn new(
        global_symbol_namespace: JsString,
        unwrapped_local_names_in_defining_chunk: IndexMap<JsString, JsString>,
    ) -> Self {
        Self {
            global_symbol_namespace,
            unwrapped_local_names_in_defining_chunk,
        }
    }

    /// Returns whether a non-global variable name conflicts with `globalSymbolNamespace` (either
    /// matching `globalSymbolNamespace` or starting with `globalSymbolNamespace + "$"`).
    // port: RescopeGlobalSymbolsRenameHelper#conflictsWithGlobalSymbolNamespace
    fn conflicts_with_global_symbol_namespace(
        name: &JsString,
        global_symbol_namespace: &JsString,
    ) -> bool {
        *name == *global_symbol_namespace
            || name
                .starts_with(global_symbol_namespace.concat(&JsString::from(DISAMBIGUATION_SUFFIX)))
    }

    /// Returns the disambiguated name for a non-global variable that conflicts with
    /// `globalSymbolNamespace`.
    // port: RescopeGlobalSymbolsRenameHelper#getDisambiguatedLocalName
    fn get_disambiguated_local_name(name: &JsString) -> JsString {
        name.concat(&JsString::from(DISAMBIGUATION_SUFFIX))
    }

    /// Collects disambiguated local variable names (e.g. `_a`) for wrapped cross-chunk symbols that
    /// remain unwrapped local variables in their defining chunk (because they are read in their
    /// defining chunk, have no unsafe writes in their defining chunk, and are never written to
    /// from another chunk).
    ///
    /// Prepending `_` avoids collisions with local wrapper aliases (`var {a} = _;`) declared in
    /// other chunks when multiple chunks are concatenated into the same function scope. If `_a` is
    /// already in use (including post-disambiguation names for local variables that conflict with
    /// `globalSymbolNamespace`), additional `_` prefixes are prepended until a free identifier is
    /// found.
    // port: RescopeGlobalSymbolsRenameHelper#collectUnwrappedLocalNamesInDefiningChunk
    fn collect_unwrapped_local_names_in_defining_chunk(
        optimize_local_access: OptimizeLocalAccess,
        symbol_info: &RescopeGlobalSymbolsInfo,
        names_starting_with_underscore: &IndexSet<JsString>,
    ) -> IndexMap<JsString, JsString> {
        if !optimize_local_access.should_keep_local_alias_for_wrapped_reassignables() {
            return IndexMap::<_, _>::default();
        }

        let mut used_names: IndexSet<JsString> = names_starting_with_underscore.clone();
        let mut builder: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();
        let prefix = JsString::from(UNWRAPPED_LOCAL_NAME_PREFIX);
        for name in symbol_info.wrapped_cross_chunk_names() {
            if !symbol_info
                .cross_chunk_names_with_write_from_other_chunk()
                .contains(name)
                && !symbol_info
                    .global_names_with_unsafe_write_for_local_alias_in_defining_chunk()
                    .contains(name)
                && symbol_info
                    .global_names_with_read_in_defining_chunk()
                    .contains(name)
            {
                let mut unwrapped_name = prefix.concat(name);
                while used_names.contains(&unwrapped_name)
                    || symbol_info.extern_names().contains(&unwrapped_name)
                    || unwrapped_name == *symbol_info.global_symbol_namespace()
                {
                    unwrapped_name = prefix.concat(&unwrapped_name);
                }
                used_names.insert(unwrapped_name.clone());
                // ImmutableMap.Builder#buildOrThrow
                check_argument!(
                    !builder.contains_key(name),
                    "Multiple entries with same key: %s",
                    name
                );
                builder.insert(name.clone(), unwrapped_name);
            }
        }
        builder
    }

    /// Returns whether the given wrapped cross-chunk symbol is kept as a disambiguated unwrapped
    /// local variable in its defining chunk.
    // port: RescopeGlobalSymbolsRenameHelper#hasUnwrappedLocalNameInDefiningChunk
    pub fn has_unwrapped_local_name_in_defining_chunk(&self, name: &JsString) -> bool {
        self.unwrapped_local_names_in_defining_chunk
            .contains_key(name)
    }

    /// Returns the local variable name to use in the defining chunk for `name` (returning the
    /// disambiguated name such as `_a` if `is_global_declaration` is true and `name` is an
    /// unwrapped wrapped symbol, or `name` otherwise).
    // port: RescopeGlobalSymbolsRenameHelper#getLocalNameInDefiningChunk
    pub fn get_local_name_in_defining_chunk(
        &self,
        name: &JsString,
        is_global_declaration: bool,
    ) -> JsString {
        if is_global_declaration {
            self.unwrapped_local_names_in_defining_chunk
                .get(name)
                .unwrap_or(name)
                .clone()
        } else {
            name.clone()
        }
    }

    /// When `globalSymbolNamespace` is used as a local variable name, appends
    /// [`DISAMBIGUATION_SUFFIX`] (`$`) to `n` to avoid shadowing the namespace (and likewise if the
    /// local variable name starts with `globalSymbolNamespace + "$"`).
    // port: RescopeGlobalSymbolsRenameHelper#maybeDisambiguateLocalVar
    pub fn maybe_disambiguate_local_var(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        var: VarId,
    ) {
        let name = n.get_string(compiler);
        if !var.is_global(compiler)
            && Self::conflicts_with_global_symbol_namespace(&name, &self.global_symbol_namespace)
        {
            n.set_string(compiler, Self::get_disambiguated_local_name(&name));
            compiler.report_change_to_enclosing_scope(n);
        }
    }

    /// Renames a global variable `n` in its defining chunk to its disambiguated unwrapped local
    /// variable name (e.g. `_a`) if one was assigned, and returns the local name in the defining
    /// chunk.
    // port: RescopeGlobalSymbolsRenameHelper#maybeRenameGlobalInDefiningChunk
    pub fn maybe_rename_global_in_defining_chunk(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> JsString {
        let name = n.get_string(compiler);
        let local_name = self
            .unwrapped_local_names_in_defining_chunk
            .get(&name)
            .unwrap_or(&name)
            .clone();
        if local_name != name {
            n.set_string(compiler, local_name.clone());
            compiler.report_change_to_enclosing_scope(n);
        }
        local_name
    }
}
