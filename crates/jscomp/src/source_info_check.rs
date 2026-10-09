/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SourceInfoCheck.java.

//! Port of `SourceInfoCheck.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::{Ast, NodeId};

// port: SourceInfoCheck#MISSING_LINE_INFO
pub static MISSING_LINE_INFO: DiagnosticType = DiagnosticType::error(
    "JSC_MISSING_LINE_INFO",
    "No source location information associated with {0}.\nMost likely a Node has been created \
     without setting the source file and line/column location.  Usually this is done using \
     Node.srcrefIfMissing and supplying a Node from the source AST.",
);

// port: SourceInfoCheck#MISSING_LENGTH
pub static MISSING_LENGTH: DiagnosticType = DiagnosticType::error(
    "JSC_MISSING_LENGTH",
    "Negative length associated with {0}.\nMost likely a Node's source information was set \
     incorrectly at parse time.",
);

// port: SourceInfoCheck#MISSING_SOURCE_NAME
pub static MISSING_SOURCE_NAME: DiagnosticType = DiagnosticType::error(
    "JSC_MISSING_SOURCE_NAME",
    "No source name associated with {0}.\nMost likely a new type was created without setting \
     the source name.",
);

/// A simple pass to enforce some invariants about source information.
///
/// - All nodes in externs and code have a line number and non-zero length
/// - All JSTypeExpressions have a source name
/// - All nodes in a JSTYpeExpression have a source file attached (does not verify lineno or
///   charno)
///
/// This pass does not check the content of the line numbers for things like "being monotonically
/// increasing" because we often insert generated code with out-of-order line numbers. (e.g.
/// imagine inlining a function from a.js into b.js, but having the source information point to
/// a.js)
#[derive(Debug, Default)]
pub struct SourceInfoCheck {
    requires_line_numbers: bool,
}

impl SourceInfoCheck {
    // port: SourceInfoCheck#SourceInfoCheck
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            requires_line_numbers: false,
        }
    }

    // port: SourceInfoCheck#setCheckSubTree
    pub fn set_check_sub_tree(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        self.requires_line_numbers = true;
        NodeTraversal::traverse(compiler, root, self);
    }

    /// Verifies that all type nodes have source files (if not an actual length)
    // port: SourceInfoCheck#checkJSDoc
    fn check_jsdoc(&mut self, compiler: &mut AbstractCompiler, info: &JSDocInfo) {
        for expression in info.get_type_expressions() {
            // The Rust JSTypeExpression stores a non-null source name (rhino port), so Java's
            // `expression.getSourceName() == null` branch cannot be taken here.
            let errors = {
                let mut errors: Vec<JSError> = Vec::new();
                NodeUtil::visit_pre_order(
                    compiler,
                    expression.get_root(),
                    &mut |ast: &mut Ast, n: NodeId| {
                        if let Some(error) = Self::check_source_file(ast, n) {
                            errors.push(error);
                        }
                    },
                );
                errors
            };
            for error in errors {
                compiler.report(error);
            }
        }
    }

    /// Returns the error Java reports from `checkSourceFile`; the caller reports it in the same
    /// order (the visitor only lends the arena).
    // port: SourceInfoCheck#checkSourceFile
    fn check_source_file(ast: &Ast, n: NodeId) -> Option<JSError> {
        let source_name = n.get_static_source_file(ast);
        if source_name.is_none() {
            let message = format!("type node {}", n.to_string_tree(ast));
            return Some(JSError::make(ast, n, &MISSING_SOURCE_NAME, &[&message]));
        }
        None
    }
}

impl CompilerPass for SourceInfoCheck {
    // port: SourceInfoCheck#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        self.requires_line_numbers = false;
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for SourceInfoCheck {
    // port: SourceInfoCheck#shouldTraverse
    fn should_traverse(
        &mut self,
        _unused: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // Each JavaScript file is rooted in a script node, so we'll only
        // have line number information inside the script node.
        if n.is_script(_unused) {
            self.requires_line_numbers = true;
        }
        true
    }

    // port: SourceInfoCheck#visit
    fn visit(&mut self, unused: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = unused.get_compiler();
        if n.is_script(compiler) {
            self.requires_line_numbers = false;
        } else if self.requires_line_numbers {
            if n.get_lineno(compiler) == -1 {
                // The tree version of the node is really the best diagnostic
                // info we have to offer here.
                let tree = n.to_string_tree(compiler);
                compiler.report(JSError::make(compiler, n, &MISSING_LINE_INFO, &[&tree]));
            } else if n.get_length(compiler) < 0 {
                let tree = n.to_string_tree(compiler);
                compiler.report(JSError::make(compiler, n, &MISSING_LENGTH, &[&tree]));
            }
            let info = n.get_jsdoc_info(compiler);
            if let Some(info) = info {
                self.check_jsdoc(compiler, &info);
            }
        }
    }
}
