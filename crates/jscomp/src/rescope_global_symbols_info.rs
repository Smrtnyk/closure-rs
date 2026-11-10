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
//   src/com/google/javascript/jscomp/RescopeGlobalSymbolsInfo.java.

use closure_rhino::fast_hash::IndexSet;
use closure_rhino::js_string::JsString;

/// Holds cross-chunk and defining-chunk symbol analysis information collected during the
/// pre-traversal of `RescopeGlobalSymbols`.
// port: RescopeGlobalSymbolsInfo
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RescopeGlobalSymbolsInfo {
    global_symbol_namespace: JsString,
    extern_names: IndexSet<JsString>,
    cross_chunk_names: IndexSet<JsString>,
    cross_chunk_names_with_write_from_other_chunk: IndexSet<JsString>,
    cross_chunk_names_used_in_non_dependent_chunk: IndexSet<JsString>,
    global_names_with_read_in_defining_chunk: IndexSet<JsString>,
    global_names_with_inner_scope_write_in_defining_chunk: IndexSet<JsString>,
    global_names_with_unsafe_write_for_local_alias_in_defining_chunk: IndexSet<JsString>,
    maybe_references_this: IndexSet<JsString>,
    wrapped_cross_chunk_names: IndexSet<JsString>,
}

impl RescopeGlobalSymbolsInfo {
    // port: RescopeGlobalSymbolsInfo#RescopeGlobalSymbolsInfo(String, Set, Set, Set, Set, Set, Set, Set, Set)
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        global_symbol_namespace: JsString,
        extern_names: &IndexSet<JsString>,
        cross_chunk_names: &IndexSet<JsString>,
        cross_chunk_names_with_write_from_other_chunk: &IndexSet<JsString>,
        cross_chunk_names_used_in_non_dependent_chunk: &IndexSet<JsString>,
        global_names_with_read_in_defining_chunk: &IndexSet<JsString>,
        global_names_with_inner_scope_write_in_defining_chunk: &IndexSet<JsString>,
        global_names_with_unsafe_write_for_local_alias_in_defining_chunk: &IndexSet<JsString>,
        maybe_references_this: &IndexSet<JsString>,
    ) -> Self {
        Self {
            global_symbol_namespace,
            extern_names: extern_names.clone(),
            cross_chunk_names: cross_chunk_names.clone(),
            cross_chunk_names_with_write_from_other_chunk:
                cross_chunk_names_with_write_from_other_chunk.clone(),
            cross_chunk_names_used_in_non_dependent_chunk:
                cross_chunk_names_used_in_non_dependent_chunk.clone(),
            global_names_with_read_in_defining_chunk: global_names_with_read_in_defining_chunk
                .clone(),
            global_names_with_inner_scope_write_in_defining_chunk:
                global_names_with_inner_scope_write_in_defining_chunk.clone(),
            global_names_with_unsafe_write_for_local_alias_in_defining_chunk:
                global_names_with_unsafe_write_for_local_alias_in_defining_chunk.clone(),
            maybe_references_this: maybe_references_this.clone(),
            wrapped_cross_chunk_names: Self::collect_wrapped_cross_chunk_names(
                extern_names,
                cross_chunk_names,
                cross_chunk_names_with_write_from_other_chunk,
                cross_chunk_names_used_in_non_dependent_chunk,
                global_names_with_inner_scope_write_in_defining_chunk,
            ),
        }
    }

    // port: RescopeGlobalSymbolsInfo#collectWrappedCrossChunkNames
    fn collect_wrapped_cross_chunk_names(
        extern_names: &IndexSet<JsString>,
        cross_chunk_names: &IndexSet<JsString>,
        cross_chunk_names_with_write_from_other_chunk: &IndexSet<JsString>,
        cross_chunk_names_used_in_non_dependent_chunk: &IndexSet<JsString>,
        global_names_with_inner_scope_write_in_defining_chunk: &IndexSet<JsString>,
    ) -> IndexSet<JsString> {
        let mut candidates: IndexSet<JsString> = IndexSet::<_>::default();
        candidates.extend(
            cross_chunk_names_with_write_from_other_chunk
                .iter()
                .cloned(),
        );
        candidates.extend(
            cross_chunk_names_used_in_non_dependent_chunk
                .iter()
                .cloned(),
        );
        for name in global_names_with_inner_scope_write_in_defining_chunk {
            if cross_chunk_names.contains(name) {
                candidates.insert(name.clone());
            }
        }
        // Java: Set#removeAll (LinkedHashSet keeps the order of the remaining names).
        candidates.retain(|name| !extern_names.contains(name));
        candidates
    }

    // port: RescopeGlobalSymbolsInfo#globalSymbolNamespace
    pub fn global_symbol_namespace(&self) -> &JsString {
        &self.global_symbol_namespace
    }

    // port: RescopeGlobalSymbolsInfo#externNames
    pub fn extern_names(&self) -> &IndexSet<JsString> {
        &self.extern_names
    }

    // port: RescopeGlobalSymbolsInfo#crossChunkNames
    pub fn cross_chunk_names(&self) -> &IndexSet<JsString> {
        &self.cross_chunk_names
    }

    // port: RescopeGlobalSymbolsInfo#crossChunkNamesWithWriteFromOtherChunk
    pub fn cross_chunk_names_with_write_from_other_chunk(&self) -> &IndexSet<JsString> {
        &self.cross_chunk_names_with_write_from_other_chunk
    }

    // port: RescopeGlobalSymbolsInfo#crossChunkNamesUsedInNonDependentChunk
    pub fn cross_chunk_names_used_in_non_dependent_chunk(&self) -> &IndexSet<JsString> {
        &self.cross_chunk_names_used_in_non_dependent_chunk
    }

    // port: RescopeGlobalSymbolsInfo#globalNamesWithReadInDefiningChunk
    pub fn global_names_with_read_in_defining_chunk(&self) -> &IndexSet<JsString> {
        &self.global_names_with_read_in_defining_chunk
    }

    // port: RescopeGlobalSymbolsInfo#globalNamesWithInnerScopeWriteInDefiningChunk
    pub fn global_names_with_inner_scope_write_in_defining_chunk(&self) -> &IndexSet<JsString> {
        &self.global_names_with_inner_scope_write_in_defining_chunk
    }

    // port: RescopeGlobalSymbolsInfo#globalNamesWithUnsafeWriteForLocalAliasInDefiningChunk
    pub fn global_names_with_unsafe_write_for_local_alias_in_defining_chunk(
        &self,
    ) -> &IndexSet<JsString> {
        &self.global_names_with_unsafe_write_for_local_alias_in_defining_chunk
    }

    // port: RescopeGlobalSymbolsInfo#maybeReferencesThis
    pub fn maybe_references_this(&self) -> &IndexSet<JsString> {
        &self.maybe_references_this
    }

    // port: RescopeGlobalSymbolsInfo#wrappedCrossChunkNames
    pub fn wrapped_cross_chunk_names(&self) -> &IndexSet<JsString> {
        &self.wrapped_cross_chunk_names
    }
}
