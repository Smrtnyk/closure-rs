/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CreateSyntheticBlocks.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `CreateSyntheticBlocks.java`.
//!
//! Creates synthetic blocks to simplify preventing optimizations from moving code past "markers"
//! in the source.
//!
//! This supports a serving infrastructure that selectively removes parts that the code, that are
//! tagged using the start/end section markers. This removal happens at serving time not
//! compilation time. The conventional markers are "_START_TEMPLATE_SECTION" and
//! "_END_TEMPLATE_SECTION" respectively.

use crate::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::check_state;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use std::collections::VecDeque;

// port: CreateSyntheticBlocks#UNMATCHED_START_MARKER
pub static UNMATCHED_START_MARKER: DiagnosticType =
    DiagnosticType::error("JSC_UNMATCHED_START_MARKER", "Unmatched {0}");

// port: CreateSyntheticBlocks#UNMATCHED_END_MARKER
pub static UNMATCHED_END_MARKER: DiagnosticType = DiagnosticType::error(
    "JSC_UNMATCHED_END_MARKER",
    "Unmatched {1} - {0} not in the same block",
);

// port: CreateSyntheticBlocks#INVALID_MARKER_USAGE
pub static INVALID_MARKER_USAGE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_MARKER_USAGE",
    "Marker {0} can only be used in a simple call expression",
);

// port: CreateSyntheticBlocks.Marker
struct Marker {
    start_marker: NodeId,
    end_marker: NodeId,
}

impl Marker {
    // port: CreateSyntheticBlocks.Marker#Marker
    fn new(start_marker: NodeId, end_marker: NodeId) -> Self {
        Self {
            start_marker,
            end_marker,
        }
    }
}

pub struct CreateSyntheticBlocks {
    /// Name of the start marker.
    start_marker_name: JsString,

    /// Name of the end marker (Java may pass null: `options.getSyntheticBlockEndMarker()`).
    end_marker_name: Option<JsString>,

    /// Markers can be nested.
    marker_stack: VecDeque<NodeId>,

    valid_markers: Vec<Marker>,
}

/// Java's `"" + string` for a possibly-null String.
fn str_or_null(s: Option<&JsString>) -> String {
    s.map_or_else(|| "null".to_owned(), ToString::to_string)
}

impl CreateSyntheticBlocks {
    // port: CreateSyntheticBlocks#CreateSyntheticBlocks
    pub fn new(start_marker_name: impl Into<JsString>, end_marker_name: Option<JsString>) -> Self {
        Self {
            start_marker_name: start_marker_name.into(),
            end_marker_name,
            marker_stack: VecDeque::new(),
            valid_markers: Vec::new(),
        }
    }

    // port: CreateSyntheticBlocks#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // Find and validate the markers.
        NodeTraversal::traverse(compiler, root, self);

        // Complain about any unmatched markers.
        let start_marker_name = self.start_marker_name.to_string();
        for &node in &self.marker_stack {
            let error = JSError::make(
                compiler,
                node,
                &UNMATCHED_START_MARKER,
                &[&start_marker_name],
            );
            compiler.report(error);
        }

        // Add the block for the valid marker sets.
        for marker in &self.valid_markers {
            Self::add_blocks(compiler, marker);
        }
    }

    /// Rewrites the function declaration from: function x() {} FUNCTION NAME x PARAM_LIST BLOCK
    /// to: var x = function() {}; VAR NAME x FUNCTION NAME (w/ empty string) PARAM_LIST BLOCK
    // port: CreateSyntheticBlocks#rewriteFunctionDeclaration
    fn rewrite_function_declaration(compiler: &mut AbstractCompiler, n: NodeId) {
        // Prepare a variable for the function.
        let old_name_node = n.get_first_child(compiler).unwrap();
        let fn_name_node = old_name_node.clone_node(compiler);
        let var = IR::var(compiler, fn_name_node).srcref(compiler, n);

        // Prepare the function.
        old_name_node.set_string(compiler, "");
        compiler.report_change_to_enclosing_scope(old_name_node);

        // Move the function to the front of the parent.
        let parent = n.get_parent(compiler).unwrap();
        n.detach(compiler);
        parent.add_child_to_front(compiler, var);
        compiler.report_change_to_enclosing_scope(var);
        fn_name_node.add_child_to_front(compiler, n);
    }

    // port: CreateSyntheticBlocks#rewriteFunctionDeclarationsInBlock
    fn rewrite_function_declarations_in_block(compiler: &mut AbstractCompiler, block: NodeId) {
        check_state!(block.is_block(compiler));

        let mut next;
        let mut current = block.get_first_child(compiler);
        while let Some(c) = current {
            // Get the next node now, because the current node will move and that will change its
            // next sibling.
            next = c.get_next(compiler);
            if NodeUtil::is_function_declaration(compiler, c) {
                Self::rewrite_function_declaration(compiler, c);
            }
            current = next;
        }
    }

    /// `marker`: The marker to add synthetic blocks for.
    // port: CreateSyntheticBlocks#addBlocks
    fn add_blocks(compiler: &mut AbstractCompiler, marker: &Marker) {
        // Add a block around the template section so that
        //   START
        //   BODY
        //   END
        // is transformed to:
        //   BLOCK (synthetic)
        //     START
        //       BLOCK (synthetic)
        //         BODY
        //     END
        // This prevents the start or end markers from mingling with the code in the block body.

        let outer_block = IR::block(compiler).srcref(compiler, marker.start_marker);
        outer_block.set_is_synthetic_block(compiler, true);
        outer_block.insert_before(compiler, marker.start_marker);

        let inner_block = IR::block(compiler).srcref(compiler, marker.start_marker);
        inner_block.set_is_synthetic_block(compiler, true);
        // Move everything after the start node up to the end node into the inner block.
        Self::move_sibling_exclusive(
            compiler,
            inner_block,
            marker.start_marker,
            marker.end_marker,
        );

        // Add the start node.
        let start = outer_block.get_next(compiler).unwrap().detach(compiler);
        outer_block.add_child_to_back(compiler, start);
        // Add the inner block.
        outer_block.add_child_to_back(compiler, inner_block);
        // And finally the end node.
        let end = outer_block.get_next(compiler).unwrap().detach(compiler);
        outer_block.add_child_to_back(compiler, end);

        // NOTE: Moving the code into a block made sense prior to ES2015 when declarations were
        // never block scoped.  But with ES2015+ function, class, let and const are all block
        // scoped. Here we are only rewriting functions because this is the behavior that
        // "normalize" previously had and we aren't currently (July 2020) trying to improve this
        // code's behavior. This maintains the status quo and allows the normalization of function
        // to be removed.
        Self::rewrite_function_declarations_in_block(compiler, inner_block);

        let outer_parent = outer_block.get_parent(compiler).unwrap();
        check_state!(
            outer_parent.is_block(compiler) || outer_parent.is_script(compiler),
            "%s",
            outer_parent.to_string(compiler)
        );
        check_state!(inner_block.get_parent(compiler).unwrap().is_block(compiler));
        compiler.report_change_to_enclosing_scope(outer_block);
    }

    /// Move the nodes between start and end from the source block to the destination block. If
    /// start is null, move the first child of the block. If end is null, move the last child of
    /// the block.
    // port: CreateSyntheticBlocks#moveSiblingExclusive
    fn move_sibling_exclusive(
        compiler: &mut AbstractCompiler,
        dest: NodeId,
        start: NodeId,
        end: NodeId,
    ) {
        // checkNotNull(start); checkNotNull(end): a NodeId is never null.
        while start.get_next(compiler) != Some(end) {
            let child = start.get_next(compiler).unwrap().detach(compiler);
            dest.add_child_to_back(compiler, child);
        }
    }
}

impl Callback for CreateSyntheticBlocks {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CreateSyntheticBlocks#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if !n.is_call(t) || !n.get_first_child(t).unwrap().is_name(t) {
            return;
        }
        let parent = parent.unwrap();

        let call_target = n.get_first_child(t).unwrap();
        let call_name = call_target.get_string(t);

        let start_marker_name = self.start_marker_name.to_string();
        if self.start_marker_name == call_name {
            if !parent.is_expr_result(t) {
                let error = JSError::make(t, n, &INVALID_MARKER_USAGE, &[&start_marker_name]);
                t.get_compiler().report(error);
                return;
            }
            self.marker_stack.push_front(parent);
            return;
        }

        // Java: endMarkerName.equals(callName) throws NullPointerException for a null end marker.
        let end_marker = self
            .end_marker_name
            .as_ref()
            .expect("NullPointerException: endMarkerName");
        if *end_marker != call_name {
            return;
        }
        let end_marker_name = str_or_null(self.end_marker_name.as_ref());

        let end_marker_node = parent;
        if !end_marker_node.is_expr_result(t) {
            let error = JSError::make(t, n, &INVALID_MARKER_USAGE, &[&end_marker_name]);
            t.get_compiler().report(error);
            return;
        }

        if self.marker_stack.is_empty() {
            let error = JSError::make(
                t,
                n,
                &UNMATCHED_END_MARKER,
                &[&start_marker_name, &end_marker_name],
            );
            t.get_compiler().report(error);
            return;
        }

        let start_marker_node = self.marker_stack.pop_front().unwrap();
        if end_marker_node.get_parent(t) != start_marker_node.get_parent(t) {
            // The end marker isn't in the same block as the start marker.
            let error = JSError::make(
                t,
                n,
                &UNMATCHED_END_MARKER,
                &[&start_marker_name, &end_marker_name],
            );
            t.get_compiler().report(error);
            return;
        }

        // This is a valid marker, add it to the list of markers to process.
        self.valid_markers
            .push(Marker::new(start_marker_node, end_marker_node));
    }
}

impl CompilerPass for CreateSyntheticBlocks {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::process(self, compiler, externs, root);
    }
}
