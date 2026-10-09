/*
 * Copyright 2023 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ReplaceToggles.java.

//! Port of `ReplaceToggles.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::ast_factory::AstFactory;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{
    AbstractPostOrderCallback, AbstractPostOrderCallbackInterface, NodeTraversal,
};
use crate::node_util::NodeUtil;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::qualified_name::QualifiedName;
use std::sync::LazyLock;

// NOTE: These diagnostics are only checked in Stage 2 (optimization), since none of this
// code should ever be hand-written: the `CLOSURE_TOGGLE_ORDINALS` bootstrap and the calls to
// `goog.readToggleInternalDoNotCallDirectly` are all generated automatically by the build
// system, so it's unexpected that anyone should ever run into these diagnostics when doing
// ordinary development.

// port: ReplaceToggles#INVALID_TOGGLE_PARAMETER
pub static INVALID_TOGGLE_PARAMETER: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_TOGGLE_PARAMETER",
    "goog.readToggleInternalDoNotCallDirectly must be called with a string literal.",
);

// port: ReplaceToggles#INVALID_ORDINAL_MAPPING
pub static INVALID_ORDINAL_MAPPING: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_ORDINAL_MAPPING",
    "CLOSURE_TOGGLE_ORDINALS must be initialized with an object literal mapping strings to booleans or unique whole numbers: {0}",
);

// NOTE: These values are chosen as negative integers because actual toggle ordinals must always
// be non-negative (at least zero).  Any negative integers would do to distinguish them from real
// toggle ordinals, but -1 and -2 are the simplest.
const TRUE_VALUE: i32 = -2;
const FALSE_VALUE: i32 = -1;

const ORDINAL_VAR_NAME: &str = "CLOSURE_TOGGLE_ORDINALS";
static READ_TOGGLE_FUNCTION_NAME: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.readToggleInternalDoNotCallDirectly"));

/// Replaces calls to id generators with ids.
///
/// Use this to get unique and short ids.
pub struct ReplaceToggles {
    ast_factory: AstFactory,
    ordinal_mapping: Option<IndexMap<JsString, i32>>,
}

impl ReplaceToggles {
    // port: ReplaceToggles#ReplaceToggles
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            ordinal_mapping: None,
        }
    }
}

impl CompilerPass for ReplaceToggles {
    // port: ReplaceToggles#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractPostOrderCallback::new(Traversal { outer: self }),
        );
    }
}

struct Traversal<'a> {
    outer: &'a mut ReplaceToggles,
}

impl AbstractPostOrderCallbackInterface for Traversal<'_> {
    // port: ReplaceToggles.Traversal#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        // Look for `var CLOSURE_TOGGLE_ORDINALS = {...};`, record the mapping, and then delete the
        // declaration from the AST since we should no longer need it (and the optimizer won't
        // delete it in case the global assignment was an intended side effect).
        if NodeUtil::is_name_declaration(t, Some(n))
            && n.get_first_child(t)
                .unwrap()
                .matches_name(t, ORDINAL_VAR_NAME)
        {
            let rhs = n.get_first_first_child(t);

            if rhs.is_none() && n.is_var(t) {
                // An empty var is fine; it should get deleted later.
                return;
            }
            let rhs = rhs.expect("ReplaceToggles rhs");
            if !rhs.is_object_lit(t) {
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &INVALID_ORDINAL_MAPPING,
                    &["not an object literal"],
                ));
                return;
            } else if self.outer.ordinal_mapping.is_some() {
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &INVALID_ORDINAL_MAPPING,
                    &["multiple initialized copies"],
                ));
                return;
            }

            let mut mapping: IndexMap<JsString, i32> = IndexMap::<_, _>::default();
            let mut ordinals: IndexSet<i32> = IndexSet::<_>::default();
            let mut c = rhs.get_first_child(t);
            while let Some(cur) = c {
                if !cur.is_string_key(t) && !cur.is_string_lit(t) {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(
                        compiler,
                        cur,
                        &INVALID_ORDINAL_MAPPING,
                        &["non-string key"],
                    ));
                    return;
                }
                let key = cur.get_string(t);
                if mapping.contains_key(&key) {
                    let compiler = t.get_compiler();
                    let message = format!("duplicate key: {}", key.to_string_lossy());
                    compiler.report(JSError::make(
                        compiler,
                        cur,
                        &INVALID_ORDINAL_MAPPING,
                        &[&message],
                    ));
                    return;
                }
                let child = cur.get_first_child(t).unwrap();
                let double_value = NodeUtil::get_number_value(t, child);
                // Java's Double#intValue: a narrowing primitive conversion.
                let mut int_value = double_value.map_or(-1, |d| d as i32);
                if child.is_true(t) || child.is_false(t) {
                    int_value = if child.is_true(t) {
                        TRUE_VALUE
                    } else {
                        FALSE_VALUE
                    };
                } else if !child.is_number(t)
                    || int_value < 0
                    || f64::from(int_value) != double_value.unwrap()
                {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(
                        compiler,
                        cur,
                        &INVALID_ORDINAL_MAPPING,
                        &["value not a boolean or whole number literal"],
                    ));
                    return;
                } else if ordinals.contains(&int_value) {
                    let compiler = t.get_compiler();
                    let message = format!("duplicate ordinal: {int_value}");
                    compiler.report(JSError::make(
                        compiler,
                        cur,
                        &INVALID_ORDINAL_MAPPING,
                        &[&message],
                    ));
                    return;
                }
                mapping.insert(key, int_value);
                ordinals.insert(int_value);
                c = cur.get_next(t);
            }
            self.outer.ordinal_mapping = Some(mapping);

            // NOTE: We do not support a simple assignment without `var` since reassignment or later
            // augmentation (i.e. `CLOSURE_TOGGLE_ORDINALS['foo'] = true`) is not allowed.
            return;
        }

        if !n.is_call(t) {
            return;
        }
        let qname = NodeUtil::get_call_target_resolving_indirect_calls(t, n);
        if !READ_TOGGLE_FUNCTION_NAME.matches(t, qname) {
            return;
        }

        let arg = n.get_second_child(t);
        if arg.is_none_or(|arg| !arg.is_string_lit(t)) || !n.has_two_children(t) {
            let compiler = t.get_compiler();
            compiler.report(JSError::make(compiler, n, &INVALID_TOGGLE_PARAMETER, &[]));
            return;
        }
        let arg = arg.unwrap();

        let ordinal = self
            .outer
            .ordinal_mapping
            .as_ref()
            .and_then(|m| m.get(&arg.get_string(t)).copied());
        if ordinal.is_none_or(|ordinal| ordinal < 0) {
            // No ordinals given: hard-code `true` if explicitly set as true, or `false` otherwise.
            let value = ordinal == Some(TRUE_VALUE);
            let compiler = t.get_compiler();
            let replacement = self
                .outer
                .ast_factory
                .create_boolean(compiler, value)
                .srcref_tree_if_missing(compiler, n);
            n.replace_with(compiler, replacement);
            t.report_code_change();
            return;
        }
        let ordinal = ordinal.unwrap();

        // Replace with a lookup into the data structure
        // Note: the choice of 30 here is per spec, since the bootstrap must be written in agreement
        // with this convention.  30 was chosen over 32 to ensure all numbers are SMI and will fit
        // neatly in 4 bytes, whereas larger ints require more space in the VM.
        let index = ordinal / 30;
        let bit = ordinal % 30;
        let f = &self.outer.ast_factory;
        let compiler = t.get_compiler();
        let receiver = f.create_qname_with_unknown_type(compiler, "goog.TOGGLES_");
        let index_node = f.create_number(compiler, f64::from(index));
        let get_elem = f.create_get_elem(compiler, receiver, index_node);
        // There are two different ways to write the bitwiseAnd:
        //  1. `getElem & mask`
        //  2. `getElem >> bit & 1` (note: sign extension is irrelevant here, and `>>` is shorter)
        // where x is the `goog.TOGGLES_[index]` lookup, and `mask` is `1 << bit`.
        // When `bit < 14`, option 1 is shorter.  When `bit > 16`, option 2 is shorter.
        // We arbitrarily prefer option 2 in the break-even range 14..16.
        let bit_and = if bit < 14 {
            let mask = f.create_number(compiler, f64::from(1 << bit));
            f.create_bitwise_and(compiler, get_elem, mask)
        } else {
            let bit_node = f.create_number(compiler, f64::from(bit));
            let shift = f.create_right_shift(compiler, get_elem, bit_node);
            let one = f.create_number(compiler, 1.0);
            f.create_bitwise_and(compiler, shift, one)
        };
        let inner_not = f.create_not(compiler, bit_and);
        let replacement = f
            .create_not(compiler, inner_not)
            .srcref_tree_if_missing(compiler, n);
        n.replace_with(compiler, replacement);
        t.report_code_change();
    }
}
