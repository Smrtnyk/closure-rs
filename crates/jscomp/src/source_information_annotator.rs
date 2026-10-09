/*
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
//   src/com/google/javascript/jscomp/SourceInformationAnnotator.java.

use crate::{
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

pub struct SourceInformationAnnotator {
    source_file_to_check: Option<String>,
}
impl SourceInformationAnnotator {
    // port: SourceInformationAnnotator#SourceInformationAnnotator
    fn new(source_file_to_check: Option<String>) -> Self {
        Self {
            source_file_to_check,
        }
    }
    // port: SourceInformationAnnotator#create
    pub fn create() -> Self {
        Self::new(None)
    }
    // port: SourceInformationAnnotator#createWithAnnotationChecks
    pub fn create_with_annotation_checks(source_file: &str) -> Self {
        Self::new(Some(source_file.into()))
    }
}

impl Callback for SourceInformationAnnotator {
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _node: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
    // port: SourceInformationAnnotator#visit
    #[allow(clippy::collapsible_if)] // Java tests the node kind before looking up its name.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if let Some(source_file_to_check) = &self.source_file_to_check {
            check_state!(n.get_source_file_name(t).as_ref() == Some(source_file_to_check));
        }
        if Self::is_string_node_requiring_original_name(t, n) {
            Self::set_original_name_from_node(t, n, n);
            return;
        }
        if n.is_function(t) {
            if let Some(function_name) = NodeUtil::get_nearest_function_name(t, n) {
                Self::set_original_name(t, n, function_name);
            }
        }
    }
}

impl SourceInformationAnnotator {
    // port: SourceInformationAnnotator#isStringNodeRequiringOriginalName
    pub fn is_string_node_requiring_original_name(ast: &Ast, node: NodeId) -> bool {
        match node.get_token(ast) {
            Token::GETPROP | Token::OPTCHAIN_GETPROP | Token::NAME => true,
            Token::MEMBER_FUNCTION_DEF
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::STRING_KEY => {
                node.get_parent(ast).unwrap().is_object_lit(ast) && !node.is_quoted_string_key(ast)
            }
            _ => false,
        }
    }
    // port: SourceInformationAnnotator#setOriginalName(Node,String)
    fn set_original_name(ast: &mut Ast, n: NodeId, name: JsString) {
        if !name.is_empty() && n.get_original_name(ast).is_none() {
            n.set_original_name(ast, Some(name));
        }
    }
    // port: SourceInformationAnnotator#setOriginalName(Node,Node)
    fn set_original_name_from_node(ast: &mut Ast, n: NodeId, name: NodeId) {
        if !name.get_string_ref(ast).is_empty() && n.get_original_name(ast).is_none() {
            n.set_original_name_from_name(ast, name);
        }
    }
}
