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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/GatherExternProperties.java.

//! Port of `GatherExternProperties.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::check_state;
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;

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

    // port: GatherExternProperties#gatherPropertiesFromJSDocInfo
    fn gather_properties_from_jsdoc_info(&mut self, ast: &Ast, js_doc_info: &JSDocInfo) {
        for js_type_expression_node in js_doc_info.get_type_nodes() {
            self.gather_properties_from_js_type_expression_node(ast, js_type_expression_node);
        }
    }

    // port: GatherExternProperties#gatherPropertiesFromJsTypeExpressionNode
    fn gather_properties_from_js_type_expression_node(
        &mut self,
        ast: &Ast,
        js_type_expression_node: NodeId,
    ) {
        match js_type_expression_node.get_token(ast) {
            Token::LB => {
                self.gather_properties_from_js_doc_record_type(ast, js_type_expression_node)
            }
            _ => {
                let mut child = js_type_expression_node.get_first_child(ast);
                while let Some(c) = child {
                    self.gather_properties_from_js_type_expression_node(ast, c);
                    child = c.get_next(ast);
                }
            }
        }
    }

    // port: GatherExternProperties#gatherPropertiesFromJsDocRecordType
    fn gather_properties_from_js_doc_record_type(&mut self, ast: &Ast, js_doc_record_node: NodeId) {
        check_state!(
            js_doc_record_node.get_token(ast) == Token::LB,
            &js_doc_record_node.to_string(ast)
        );
        let mut field_node = js_doc_record_node.get_first_child(ast);
        while let Some(field) = field_node {
            let field_name_node: NodeId;
            let field_type_node: Option<NodeId>;
            if field.get_token(ast) == Token::COLON {
                field_name_node = field.get_first_child(ast).unwrap();
                field_type_node = field_name_node.get_next(ast);
            } else {
                field_name_node = field;
                field_type_node = None;
            }
            check_state!(
                field_name_node.is_string_key(ast),
                &field_name_node.to_string(ast)
            );
            let mut field_name = field_name_node.get_string(ast);
            // TODO(bradfordcsmith): The JSDoc parser should do this.
            if field_name.starts_with("'") || field_name.starts_with("\"") {
                field_name = field_name.substring(1, field_name.length() - 1);
            }
            self.extern_properties.insert(field_name.to_string_lossy());
            if let Some(field_type_node) = field_type_node {
                self.gather_properties_from_js_type_expression_node(ast, field_type_node);
            }
            field_node = field.get_next(ast);
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

    // port: GatherExternProperties#visit
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if self.mode.optimize() {
            match n.get_token(t) {
                Token::GETPROP => {
                    // Gathers "name" from (someObject.name).
                    self.extern_properties
                        .insert(n.get_string(t).to_string_lossy());
                }
                Token::STRING_KEY => {
                    if parent.unwrap().is_object_lit(t) {
                        self.extern_properties
                            .insert(n.get_string(t).to_string_lossy());
                    }
                }
                Token::MEMBER_FUNCTION_DEF
                | Token::MEMBER_FIELD_DEF
                | Token::GETTER_DEF
                | Token::SETTER_DEF => {
                    self.extern_properties
                        .insert(n.get_string(t).to_string_lossy());
                }
                _ => {}
            }
        }
        // (The mode is tested first, so the JSDoc is only copied when it is read.)
        if self.mode.check()
            && let Some(js_doc_info) = n.get_jsdoc_info(t)
        {
            self.gather_properties_from_jsdoc_info(t, &js_doc_info);
        }
    }
}
