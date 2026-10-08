/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/JSError.java.

use crate::{
    check_level::CheckLevel, conformance_config::Requirement, diagnostic_type::DiagnosticType,
    message_formatter::MessageFormatter,
};
use closure_rhino::{
    check_state,
    node::{Ast, NodeId},
};
use std::{fmt, sync::Arc};
// port: JSError#JSError
// Rust field types enforce the record constructor's non-null components.
#[derive(Clone, Debug, PartialEq)]
pub struct JSError {
    pub type_: &'static DiagnosticType,
    pub description: String,
    pub source_name: Option<String>,
    pub lineno: i32,
    pub charno: i32,
    pub length: i32,
    pub node: Option<NodeId>,
    pub default_level: CheckLevel,
    pub requirement: Option<Arc<Requirement>>,
}
impl JSError {
    // port: JSError#make(DiagnosticType,String...)
    pub fn make_without_location(type_: &'static DiagnosticType, arguments: &[&str]) -> Self {
        Self::builder(type_, arguments).build()
    }
    // port: JSError#make(String,int,int,DiagnosticType,String...)
    pub fn make_with_source_location(
        source_name: &str,
        lineno: i32,
        charno: i32,
        type_: &'static DiagnosticType,
        arguments: &[&str],
    ) -> Self {
        Self::builder(type_, arguments)
            .set_source_location(source_name, lineno, charno)
            .build()
    }
    // port: JSError#make(Node,DiagnosticType,String...)
    pub fn make(ast: &Ast, n: NodeId, type_: &'static DiagnosticType, arguments: &[&str]) -> Self {
        Self::builder(type_, arguments).set_node(ast, n).build()
    }
    // port: JSError#make(Node,Node,DiagnosticType,String...)
    pub fn make_with_node_range(
        ast: &Ast,
        start: NodeId,
        end: NodeId,
        type_: &'static DiagnosticType,
        arguments: &[&str],
    ) -> Self {
        Self::builder(type_, arguments)
            .set_node_range(ast, start, end)
            .build()
    }
    // port: JSError#make(Requirement,Node,DiagnosticType,String...)
    pub fn make_with_requirement(
        ast: &Ast,
        requirement: Arc<Requirement>,
        n: NodeId,
        type_: &'static DiagnosticType,
        arguments: &[&str],
    ) -> Self {
        Self::builder(type_, arguments)
            .set_node(ast, n)
            .set_requirement(requirement)
            .build()
    }
    // port: JSError#builder
    pub fn builder(type_: &'static DiagnosticType, arguments: &[&str]) -> Builder {
        Builder::new(type_, arguments)
    }
    // port: JSError#format
    pub fn format(
        &self,
        ast: &Ast,
        level: CheckLevel,
        formatter: &dyn MessageFormatter,
    ) -> Option<String> {
        match level {
            CheckLevel::ERROR => Some(formatter.format_error(ast, self)),
            CheckLevel::WARNING => Some(formatter.format_warning(ast, self)),
            CheckLevel::OFF => None,
        }
    }
    // port: JSError#getNodeSourceOffset
    pub fn get_node_source_offset(&self, ast: &Ast) -> i32 {
        self.node.map_or(-1, |n| n.get_source_offset(ast))
    }
    // port: JSError#getLineNumber
    pub fn get_line_number(&self) -> i32 {
        self.lineno
    }
    // port: JSError#type
    pub fn get_type(&self) -> &'static DiagnosticType {
        self.type_
    }
    // port: JSError#getType
    // get_type also implements the keyword-named record component accessor.

    // port: JSError#description
    pub fn description(&self) -> &str {
        &self.description
    }
    // port: JSError#getDescription
    pub fn get_description(&self) -> &str {
        self.description()
    }

    // port: JSError#sourceName
    pub fn source_name(&self) -> Option<&str> {
        self.source_name.as_deref()
    }
    // port: JSError#getSourceName
    pub fn get_source_name(&self) -> Option<&str> {
        self.source_name()
    }

    // port: JSError#lineno
    pub fn lineno(&self) -> i32 {
        self.lineno
    }
    // port: JSError#getLineno
    pub fn get_lineno(&self) -> i32 {
        self.lineno()
    }

    // port: JSError#charno
    pub fn charno(&self) -> i32 {
        self.charno
    }
    // port: JSError#getCharno
    pub fn get_charno(&self) -> i32 {
        self.charno()
    }

    // port: JSError#length
    pub fn length(&self) -> i32 {
        self.length
    }
    // port: JSError#getLength
    pub fn get_length(&self) -> i32 {
        self.length()
    }

    // port: JSError#node
    pub fn node(&self) -> Option<NodeId> {
        self.node
    }
    // port: JSError#getNode
    pub fn get_node(&self) -> Option<NodeId> {
        self.node()
    }

    // port: JSError#defaultLevel
    pub fn default_level(&self) -> CheckLevel {
        self.default_level
    }
    // port: JSError#getDefaultLevel
    pub fn get_default_level(&self) -> CheckLevel {
        self.default_level()
    }

    // port: JSError#requirement
    pub fn requirement(&self) -> Option<&Arc<Requirement>> {
        self.requirement.as_ref()
    }
    // port: JSError#getRequirement
    pub fn get_requirement(&self) -> Option<&Arc<Requirement>> {
        self.requirement()
    }
}
#[derive(Debug)]
pub struct Builder {
    type_: &'static DiagnosticType,
    args: Vec<String>,
    level: CheckLevel,
    n: Option<NodeId>,
    source_name: Option<String>,
    lineno: i32,
    charno: i32,
    length: i32,
    requirement: Option<Arc<Requirement>>,
}
impl Builder {
    // port: JSError.Builder#Builder
    fn new(type_: &'static DiagnosticType, args: &[&str]) -> Self {
        Self {
            type_,
            args: args.iter().map(|s| (*s).into()).collect(),
            level: type_.level,
            n: None,
            source_name: None,
            lineno: -1,
            charno: -1,
            length: 0,
            requirement: None,
        }
    }
    // port: JSError.Builder#setNode
    pub fn set_node(mut self, ast: &Ast, n: NodeId) -> Self {
        check_state!(
            self.source_name.is_none(),
            "Cannot provide a Node when there's already a source name"
        );
        self.n = Some(n);
        self.source_name = n.get_source_file_name(ast);
        self.lineno = n.get_lineno(ast);
        self.charno = n.get_charno(ast);
        self.length = n.get_length(ast);
        self
    }
    // port: JSError.Builder#setNodeRange
    pub fn set_node_range(mut self, ast: &Ast, start: NodeId, end: NodeId) -> Self {
        check_state!(
            self.source_name.is_none(),
            "Cannot provide a Node when there's already a source name"
        );
        self.n = Some(start);
        self.source_name = start.get_source_file_name(ast);
        self.lineno = start.get_lineno(ast);
        self.charno = start.get_charno(ast);
        let end_offset = end.get_source_offset(ast);
        let start_offset = start.get_source_offset(ast);
        if end_offset != -1 && start_offset != -1 {
            self.length = end_offset
                .wrapping_add(end.get_length(ast))
                .wrapping_sub(start_offset);
        }
        self
    }
    // port: JSError.Builder#setLevel
    pub fn set_level(mut self, level: CheckLevel) -> Self {
        self.level = level;
        self
    }
    // port: JSError.Builder#setRequirement
    pub fn set_requirement(mut self, requirement: Arc<Requirement>) -> Self {
        self.requirement = Some(requirement);
        self
    }
    // port: JSError.Builder#setSourceLocation
    pub fn set_source_location(mut self, source_name: &str, lineno: i32, charno: i32) -> Self {
        check_state!(
            self.n.is_none(),
            "Cannot provide a source location when there is already a Node"
        );
        self.source_name = Some(source_name.into());
        self.lineno = lineno;
        self.charno = charno;
        self
    }
    // port: JSError.Builder#build
    pub fn build(self) -> JSError {
        JSError {
            type_: self.type_,
            description: self
                .type_
                .format(&self.args.iter().map(String::as_str).collect::<Vec<_>>()),
            source_name: self.source_name,
            lineno: self.lineno,
            charno: self.charno,
            length: self.length,
            node: self.n,
            default_level: self.level,
            requirement: self.requirement,
        }
    }
}
impl fmt::Display for JSError {
    // port: JSError#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = self
            .source_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("(unknown source)");
        let line = if self.lineno == -1 {
            "(unknown line)".into()
        } else {
            self.lineno.to_string()
        };
        let column = if self.charno == -1 {
            "(unknown column)".into()
        } else {
            self.charno.to_string()
        };
        write!(
            f,
            "{}. {} at {} line {} : {}",
            self.type_.key, self.description, source, line, column
        )
    }
}
