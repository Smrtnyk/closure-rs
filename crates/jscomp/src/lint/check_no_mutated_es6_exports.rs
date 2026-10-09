/*
 * Copyright 2018 The Closure Compiler Authors.
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
/*
 * Copyright (C) 2007 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/lint/CheckNoMutatedEs6Exports.java.
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/collect/HashMultimap.java.

//! Checks that exports of ES6 modules are not mutated outside of module initialization.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{NodeId, Prop},
    token::Token,
};
use indexmap::{IndexMap, IndexSet};

// port: CheckNoMutatedEs6Exports#MUTATED_EXPORT
pub static MUTATED_EXPORT: DiagnosticType = DiagnosticType::warning(
    "JSC_MUTATED_EXPORT",
    "The name \"{0}\" is exported and should not be mutated outside of module initialization. Mutable exports are generally difficult to reason about. You can work around this by exporting getter/setter functions, or an object with mutable properties instead.",
);

/// Java's `HashMultimap<String, Node>` (`MultimapBuilder.hashKeys().hashSetValues()`).
///
/// Key iteration follows `java.util.HashMap`: buckets in index order (`spread(hashCode) &
/// (capacity - 1)`), keys of one bucket in insertion order. `Maps.newHashMapWithExpectedSize(8)`
/// gives a 16-bucket table that doubles once the size exceeds 3/4 of it; `clear()` keeps the
/// table size. Values are a `HashSet<Node>` keyed by identity hash codes, which have no
/// reproducible order; insertion order is used for them.
struct MutatedNames {
    entries: IndexMap<JsString, IndexSet<NodeId>>,
    table_capacity: usize,
}

impl MutatedNames {
    // port: HashMultimap#put
    fn put(&mut self, key: JsString, value: NodeId) {
        let is_new_key = !self.entries.contains_key(&key);
        self.entries.entry(key).or_default().insert(value);
        if is_new_key {
            self.grow_table_after_new_key();
        }
    }

    // port: HashMultimap#get
    fn get(&self, key: &JsString) -> Vec<NodeId> {
        self.entries
            .get(key)
            .map(|v| v.iter().copied().collect())
            .unwrap_or_default()
    }

    // port: HashMultimap#clear
    fn clear(&mut self) {
        self.entries.clear();
    }
}

// The java.util.HashMap parts of MutatedNames (table size, hash spreading, key order), ported
// from OpenJDK (GPL-2.0 with the Classpath exception), are in their own file.
#[path = "check_no_mutated_es6_exports_jdk.rs"]
mod jdk;

pub struct CheckNoMutatedEs6Exports {
    mutated_names: MutatedNames,
    exported_local_names: IndexSet<JsString>,
}

impl CheckNoMutatedEs6Exports {
    // port: CheckNoMutatedEs6Exports#CheckNoMutatedEs6Exports
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            mutated_names: MutatedNames::new(),
            exported_local_names: IndexSet::new(),
        }
    }

    // port: CheckNoMutatedEs6Exports#checkNoMutations
    fn check_no_mutations(&mut self, compiler: &mut AbstractCompiler) {
        // Sets.intersection(mutatedNames.keySet(), exportedLocalNames) iterates the first set.
        let mutated_exports: Vec<JsString> = self
            .mutated_names
            .key_set()
            .into_iter()
            .filter(|k| self.exported_local_names.contains(k))
            .collect();
        for mutated_export in mutated_exports {
            for mutation in self.mutated_names.get(&mutated_export) {
                compiler.report(JSError::make(
                    compiler,
                    mutation,
                    &MUTATED_EXPORT,
                    &[&mutated_export.to_string()],
                ));
            }
        }
        self.mutated_names.clear();
        self.exported_local_names.clear();
    }

    // port: CheckNoMutatedEs6Exports#visitExport
    fn visit_export(&mut self, compiler: &mut AbstractCompiler, export: NodeId) {
        if export.has_one_child(compiler)
            && export
                .get_first_child(compiler)
                .unwrap()
                .is_export_specs(compiler)
        {
            // export {a, b as c};
            let mut export_spec = export.get_first_first_child(compiler);
            while let Some(spec) = export_spec {
                check_state!(spec.has_two_children(compiler));
                self.exported_local_names
                    .insert(spec.get_first_child(compiler).unwrap().get_string(compiler));
                export_spec = spec.get_next(compiler);
            }
        } else if export.has_one_child(compiler)
            && !export.get_boolean_prop(compiler, Prop::EXPORT_ALL_FROM)
        {
            let declaration = export.get_first_child(compiler).unwrap();
            if NodeUtil::is_name_declaration(compiler, Some(declaration)) {
                // export const x = 0;
                // export let a, b, c;
                let exported_local_names = &mut self.exported_local_names;
                NodeUtil::visit_lhs_nodes_in_node(
                    compiler,
                    declaration,
                    &mut |compiler: &mut AbstractCompiler, lhs: NodeId| {
                        check_state!(lhs.is_name(compiler));
                        exported_local_names.insert(lhs.get_string(compiler));
                    },
                );
            } else if export.get_boolean_prop(compiler, Prop::EXPORT_DEFAULT) {
                // export default function() {}
                // export default function foo() {}
                // export default 0;
                // Note: default exports may or may not be declarations.
                if !declaration.is_class(compiler) && !declaration.is_function(compiler) {
                    return;
                }
                let name_node = declaration.get_first_child(compiler).unwrap();
                if !name_node.is_empty(compiler) && !name_node.get_string(compiler).is_empty() {
                    self.exported_local_names
                        .insert(name_node.get_string(compiler));
                }
            } else {
                // export function foo() {}
                // export class Bar {}
                check_state!(declaration.is_class(compiler) || declaration.is_function(compiler));
                let name_node = declaration.get_first_child(compiler).unwrap();
                self.exported_local_names
                    .insert(name_node.get_string(compiler));
            }
        }
    }

    // port: CheckNoMutatedEs6Exports#visitName
    fn visit_name(&mut self, t: &mut NodeTraversal<'_>, name: NodeId) {
        let scope = t.get_scope();
        if NodeUtil::is_l_value(t, name)
            && !scope
                .get_closest_hoist_scope(t.get_compiler())
                .unwrap()
                .is_module_scope(t.get_compiler())
        {
            let name_string = name.get_string(t);
            let var = scope.get_var(t.get_compiler(), &name_string);
            if let Some(var) = var
                && var
                    .get_scope(t.get_compiler())
                    .is_module_scope(t.get_compiler())
            {
                self.mutated_names.put(name_string, name);
            }
        }
    }
}

impl CompilerPass for CheckNoMutatedEs6Exports {
    // port: CheckNoMutatedEs6Exports#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckNoMutatedEs6Exports {
    // port: CheckNoMutatedEs6Exports#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::SCRIPT => n.get_boolean_prop(t, Prop::ES6_MODULE),
            Token::EXPORT => {
                self.visit_export(t.get_compiler(), n);
                true
            }
            Token::NAME => {
                self.visit_name(t, n);
                true
            }
            _ => true,
        }
    }

    // port: CheckNoMutatedEs6Exports#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_module_body(t) {
            self.check_no_mutations(t.get_compiler());
        }
    }
}
