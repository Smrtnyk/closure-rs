/*
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/GatherExternProperties.java.

//! Port of `GatherExternProperties.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::source_file::bundled_script::{BundledScript, gather_type_fields, property};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::node::{NodeId, SnapshotUse};

/// Gathers property names defined in externs.
///
/// The collection of these property names could easily happen during type checking. However,
/// when exporting local property definitions, the externs may be modified after type checking,
/// and we want to collect the new names as well.
///
/// Mode.CHECK is set when checksOnly=true and only collects the externs in the type expressions
/// in JSDoc. Mode.OPTIMIZE is set when checksOnly=false and only collects the externs in AST
/// properties.
///
/// To optimize build time, we collect only type expression externs in getChecks() and only AST
/// property externs in getOptimizations() instead of collecting all in both.
#[derive(Debug, Clone)]
pub struct GatherExternProperties {
    /// Java's `LinkedHashSet<String>`; the compiler stores extern properties as Rust strings.
    extern_properties: IndexSet<String>,
    mode: Mode,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    CHECK,
    OPTIMIZE,
    CHECK_AND_OPTIMIZE,
}

impl Mode {
    // port: GatherExternProperties.Mode#Mode (field check)
    const fn check(self) -> bool {
        match self {
            Self::CHECK => true,
            Self::OPTIMIZE => false,
            Self::CHECK_AND_OPTIMIZE => true,
        }
    }

    // port: GatherExternProperties.Mode#Mode (field optimize)
    const fn optimize(self) -> bool {
        match self {
            Self::CHECK => false,
            Self::OPTIMIZE => true,
            Self::CHECK_AND_OPTIMIZE => true,
        }
    }
}

impl GatherExternProperties {
    // port: GatherExternProperties#GatherExternProperties
    pub fn new(compiler: &AbstractCompiler, mode: Mode) -> Self {
        let mut extern_properties = IndexSet::<_>::default();
        if let Some(properties) = compiler.get_extern_properties() {
            extern_properties.extend(properties.iter().cloned());
        }
        Self {
            extern_properties,
            mode,
        }
    }
}

impl CompilerPass for GatherExternProperties {
    // port: GatherExternProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, _root: NodeId) {
        NodeTraversal::traverse(compiler, externs, self);
        compiler.set_extern_properties(self.extern_properties.clone());
    }
}

impl Callback for GatherExternProperties {
    // port: GatherExternProperties#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !n.is_script(t) || !NodeUtil::is_from_type_summary(t, n)
    }

    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if NodeUtil::is_from_type_summary(t, script) {
            return false;
        }
        let names = match self.mode {
            Mode::CHECK => BundledScript::prefix(t, script, SnapshotUse::SyntaxAndJsDoc)
                .map(|(s, first)| (s.type_fields, first)),
            Mode::OPTIMIZE => BundledScript::prefix(t, script, SnapshotUse::Syntax)
                .map(|(s, first)| (s.properties, first)),
            // This mode interleaves type and syntax fields in the same set.
            Mode::CHECK_AND_OPTIMIZE => None,
        };
        let Some((names, first)) = names else {
            return true;
        };
        self.extern_properties
            .extend(names.iter().map(|&name| name.to_owned()));
        t.traverse_script_suffix(script, parent, first, self);
        false
    }

    // port: GatherExternProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if self.mode.optimize()
            && let Some(name) = property(t, n, parent)
        {
            self.extern_properties.insert(name.to_string_lossy());
        }
        if self.mode.check()
            && let Some(js_doc_info) = n.get_jsdoc_info_ref(t)
        {
            js_doc_info.for_each_type_node(|root| {
                gather_type_fields(&mut self.extern_properties, t, root)
            });
        }
    }
}
