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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CodePrinter.java.

#![allow(clippy::collapsible_if, clippy::too_many_arguments)] // Retain Java control flow and signatures.
use crate::{
    code_consumer::{CodeConsumer, CodeConsumerState},
    compiler_options::CompilerOptions,
    node_util::NodeUtil,
    source_file::SourceFile,
    source_map::{DetailLevel, Mapping},
};
use closure_jstype::JSTypeRegistry;
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
};
use closure_sourcemap::file_position::FilePosition;
use std::{any::Any, collections::VecDeque, ops::Deref};

/// CodePrinter prints out JS code in either pretty format or compact format.
///
/// @see CodeGenerator
pub struct CodePrinter;
impl CodePrinter {
    // port: CodePrinter#CodePrinter
    #[allow(dead_code)] // Java utility-class constructor is intentionally private.
    fn new() -> Self {
        Self
    }
}

// There are two separate CodeConsumers, one for pretty-printing and
// another for compact printing.
// There are two implementations because the CompactCodePrinter
// potentially has a very different implementation to the pretty
// version.
pub struct MappedCodePrinter<'a> {
    state: CodeConsumerState,
    mappings: Option<VecDeque<usize>>,
    all_mappings: Option<Vec<Mapping>>,
    // The ordered list of finalized mappings since the last line break. See #reportLineCut.
    complete_mappings: Option<Vec<usize>>,
    // The index into allMappings to find the mappings added since the last line
    // break. See #reportLineCut.
    first_candidate_mapping_for_cut: usize,
    create_src_map: bool,
    source_map_detail_level: DetailLevel,
    license_tracker: Option<&'a mut dyn LicenseTracker>,
    pub code: Vec<u16>,
    line_length_threshold: i32,
    line_length: i32,
    line_index: i32,
}
impl<'a> MappedCodePrinter<'a> {
    // port: CodePrinter.MappedCodePrinter#MappedCodePrinter
    pub fn new(
        line_length_threshold: i32,
        create_src_map: bool,
        source_map_detail_level: DetailLevel,
        license_tracker: Option<&'a mut dyn LicenseTracker>,
    ) -> Self {
        Self {
            state: CodeConsumerState::default(),
            mappings: create_src_map.then(VecDeque::new),
            all_mappings: create_src_map.then(Vec::new),
            complete_mappings: create_src_map.then(Vec::new),
            first_candidate_mapping_for_cut: 0,
            create_src_map,
            source_map_detail_level,
            license_tracker,
            code: Vec::with_capacity(1024),
            line_length_threshold: if line_length_threshold <= 0 {
                i32::MAX
            } else {
                line_length_threshold
            },
            line_length: 0,
            line_index: 0,
        }
    }
    /// Returns a list of sourcemap mappings that were generated while printing this code. Only
    /// useful if createSrcMap was true for this MappedCodePrinter.
    // port: CodePrinter.MappedCodePrinter#getSourceMappings
    pub fn get_source_mappings(&self, code: &JsString) -> Option<Vec<Mapping>> {
        if !self.create_src_map {
            return None;
        }
        let line_lengths = Self::compute_line_lengths(code);
        let mut fixed_mappings = Vec::new();
        for mapping in self.all_mappings.as_ref().unwrap() {
            let adjusted = Mapping {
                node: mapping.node,
                start: mapping.start,
                end: Some(Self::adjust_end_position(
                    &line_lengths,
                    mapping
                        .end
                        .expect("NullPointerException: source mapping end"),
                )),
            };
            fixed_mappings.push(adjusted);
        }
        Some(fixed_mappings)
    }
    /// Reports to the code consumer that the given line has been cut at the given position, i.e. a
    /// \n has been inserted there. All mappings in the source maps after that position will be
    /// renormalized as needed.
    // port: CodePrinter.MappedCodePrinter#reportLineCut
    pub fn report_line_cut(&mut self, line_index: i32, char_index: i32) {
        if self.create_src_map {
            let all_mappings = self.all_mappings.as_mut().unwrap();
            // To avoid iterating over every mapping, every time we cut a line (which can get
            // excessively expensive for large files), we keep track of mappings that must be
            // before the next cut. For the start of mappings, we can use the order in allMappings.
            // However, mapping ends do not have their own entry in the list so we must track those
            // separately.
            let mapping_count = all_mappings.len();
            for mapping in &mut all_mappings[self.first_candidate_mapping_for_cut..mapping_count] {
                mapping.start =
                    Self::convert_position_after_line_cut(mapping.start, line_index, char_index);
            }
            self.first_candidate_mapping_for_cut = mapping_count;
            for &mapping in self.complete_mappings.as_ref().unwrap() {
                all_mappings[mapping].end = Some(Self::convert_position_after_line_cut(
                    all_mappings[mapping]
                        .end
                        .expect("NullPointerException: source mapping end"),
                    line_index,
                    char_index,
                ));
            }
            // To avoid iterating over every mapping, every time we cut a line, keep track of
            // mappings that must end before the next cut.
            self.complete_mappings.as_mut().unwrap().clear();
        }
    }
    /// Converts the given position by normalizing it against the insertion at the given line and
    /// character position.
    ///
    /// @param position The existing position before the newline was inserted.
    /// @param lineIndex The index of the line at which the newline was inserted.
    /// @param characterPosition The position on the line at which the newline was inserted.
    /// @return The normalized position.
    /// @throws IllegalStateException if an attempt to reverse a line cut is made on a previous line
    /// rather than the current line.
    // port: CodePrinter.MappedCodePrinter#convertPositionAfterLineCut
    pub fn convert_position_after_line_cut(
        position: FilePosition,
        line_index: i32,
        character_position: i32,
    ) -> FilePosition {
        let original_line = position.get_line();
        let original_char = position.get_column();
        if original_line == line_index && original_char >= character_position {
            // If the position falls on the line itself, then normalize it
            // if it falls at or after the place the newline was inserted.
            FilePosition::new(original_line + 1, original_char - character_position)
        } else {
            position
        }
    }
    // port: CodePrinter.MappedCodePrinter#getCode
    pub fn get_code(&self) -> JsString {
        JsString::from_units(self.code.clone())
    }
    // port: CodePrinter.MappedCodePrinter#getCurrentCharIndex
    pub fn get_current_char_index(&self) -> i32 {
        self.line_length
    }
    // port: CodePrinter.MappedCodePrinter#getCurrentLineIndex
    pub fn get_current_line_index(&self) -> i32 {
        self.line_index
    }
    /// Calculates length of each line in compiled code.
    // port: CodePrinter.MappedCodePrinter#computeLineLengths
    fn compute_line_lengths(code: &JsString) -> Vec<i32> {
        let mut builder = Vec::new();
        let mut line_start_pos = 0;
        let mut line_end_pos = code.index_of_char(b'\n' as u16);
        while line_end_pos > -1 {
            builder.push(line_end_pos - line_start_pos);
            // Next line starts where current line ends + 1 to skip "\n" character.
            line_start_pos = line_end_pos + 1;
            line_end_pos = code.index_of_from("\n", line_start_pos);
        }
        builder
    }
    /// Adjusts end position of a mapping. End position points to a column *after* the last character
    /// that is covered by a mapping. And if it's end of the line there are 2 possibilities: either
    /// point to the non-existent character after the last char on a line or point to the first
    /// character on the next line. In some cases we end up with 2 mappings which should have the
    /// same end position, but they use different styles as described above it leads to invalid
    /// source maps.
    ///
    /// This method adjusts all such end positions, so if it points to the non-existing character
    /// at the end of line - it is changed to point to the first character on the next line.
    ///
    /// @param lineLengths List of all line lengths in compiled code.
    /// @param endPosition End position of a mapping.
    // port: CodePrinter.MappedCodePrinter#adjustEndPosition
    fn adjust_end_position(line_lengths: &[i32], end_position: FilePosition) -> FilePosition {
        let line = end_position.get_line();
        // if position points to non-existing line, return it unmodified
        if line >= line_lengths.len() as i32 {
            return end_position;
        }
        check_state!(
            end_position.get_column() <= line_lengths[line as usize],
            "End position %s points to a column larger than line length %s",
            end_position,
            line_lengths[line as usize]
        );
        // if end position points to the column just after the last character on the line -
        // change it to point the first character on the next line
        if end_position.get_column() == line_lengths[line as usize] {
            return FilePosition::new(line + 1, 0);
        }
        end_position
    }
}
impl CodeConsumer for MappedCodePrinter<'_> {
    fn state(&self) -> &CodeConsumerState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut CodeConsumerState {
        &mut self.state
    }
    /// Appends a string to the code, keeping track of the current line length.
    // port: CodePrinter.MappedCodePrinter#append
    fn append(&mut self, str: &JsString) {
        self.code.extend_from_slice(str.as_units());
        self.line_length += str.length() as i32;
    }
    // port: CodePrinter.MappedCodePrinter#trackLicenses
    fn track_licenses(&mut self, ast: &Ast, node: NodeId) {
        if let Some(tracker) = &mut self.license_tracker {
            tracker.track_licenses_for_node(ast, node);
        }
    }
    /// Starts the source mapping for the given
    /// node at the current position.
    // port: CodePrinter.MappedCodePrinter#startSourceMapping
    fn start_source_mapping(&mut self, ast: &Ast, node: NodeId) {
        if self.create_src_map
            // getSourceFileName() != null, without copying the name.
            && node.get_static_source_file_ref(ast).is_some()
            && node.get_lineno(ast) > 0
            && self.source_map_detail_level.apply(ast, node)
        {
            let line = self.get_current_line_index();
            let index = self.get_current_char_index();
            check_state!(line >= 0);
            let mapping = Mapping {
                node,
                start: FilePosition::new(line, index),
                end: None,
            };
            self.mappings
                .as_mut()
                .unwrap()
                .push_front(self.all_mappings.as_ref().unwrap().len());
            self.all_mappings.as_mut().unwrap().push(mapping);
        }
    }
    /// Finishes the source mapping for the given
    /// node at the current position.
    // port: CodePrinter.MappedCodePrinter#endSourceMapping
    fn end_source_mapping(&mut self, _ast: &Ast, node: NodeId) {
        if self.create_src_map
            && !self.mappings.as_ref().unwrap().is_empty()
            && self.all_mappings.as_ref().unwrap()
                [*self.mappings.as_ref().unwrap().front().unwrap()]
            .node
                == node
        {
            let mapping = self.mappings.as_mut().unwrap().pop_front().unwrap();
            let line = self.get_current_line_index();
            let index = self.get_current_char_index();
            check_state!(line >= 0);
            self.all_mappings.as_mut().unwrap()[mapping].end = Some(FilePosition::new(line, index));
            self.complete_mappings.as_mut().unwrap().push(mapping);
        }
    }
    // port: CodePrinter.MappedCodePrinter#getLastChar
    fn get_last_char(&self) -> u16 {
        self.code.last().copied().unwrap_or(0)
    }
}

pub struct PrettyCodePrinter<'a> {
    pub mapped: MappedCodePrinter<'a>,
    indent: i32,
}
impl<'a> PrettyCodePrinter<'a> {
    pub const INDENT: &'static str = "  ";
    /// @param lineLengthThreshold The length of a line after which we force a newline when possible.
    /// @param createSourceMap Whether to generate source map data.
    /// @param sourceMapDetailLevel A filter to control which nodes get mapped into the source map.
    /// @param licenseTracker A license tracking implementation to manage license text emit. The
    /// CodePrinter will never emit license information directly.
    // port: CodePrinter.PrettyCodePrinter#PrettyCodePrinter
    pub fn new(
        line_length_threshold: i32,
        create_source_map: bool,
        source_map_detail_level: DetailLevel,
        license_tracker: Option<&'a mut dyn LicenseTracker>,
    ) -> Self {
        Self {
            mapped: MappedCodePrinter::new(
                line_length_threshold,
                create_source_map,
                source_map_detail_level,
                license_tracker,
            ),
            indent: 0,
        }
    }
    /// @return The TRY node for the specified CATCH node.
    // port: CodePrinter.PrettyCodePrinter#getTryForCatch
    fn get_try_for_catch(ast: &Ast, n: NodeId) -> NodeId {
        n.get_grandparent(ast).unwrap()
    }
    // port: CodePrinter.PrettyCodePrinter#getNumberFromSource
    pub fn get_number_from_source(ast: &Ast, n: NodeId) -> Option<JsString> {
        if !n.is_number(ast) {
            return None;
        }
        let static_src = NodeUtil::get_source_file(ast, Some(n))?;
        let src = (static_src.as_ref() as &dyn Any).downcast_ref::<SourceFile>()?;
        if src.is_stub_source_file_for_already_provided_input() {
            // source file is a stub file, so we can not get number from source.
            return None;
        }
        let src_code = src.get_code().ok()?;
        // Mirrors Java's catch (IllegalArgumentException e) from SourceFile.getLineOffset.
        // Its checkArgument requires 1 <= lineno <= lineOffsets.length.
        let lineno = n.get_lineno(ast);
        if lineno >= 0 && (lineno < 1 || lineno > src.get_num_lines()) {
            return None;
        }
        let offset = n.get_source_offset(ast);
        let end_offset = offset.wrapping_add(n.get_length(ast));
        if offset < 0 || end_offset > src_code.length() as i32 {
            return None;
        }
        Some(src_code.substring(offset as usize, end_offset as usize))
    }
}
impl CodeConsumer for PrettyCodePrinter<'_> {
    fn state(&self) -> &CodeConsumerState {
        self.mapped.state()
    }
    fn state_mut(&mut self) -> &mut CodeConsumerState {
        self.mapped.state_mut()
    }
    fn get_last_char(&self) -> u16 {
        self.mapped.get_last_char()
    }
    fn start_source_mapping(&mut self, ast: &Ast, node: NodeId) {
        self.mapped.start_source_mapping(ast, node);
    }
    fn end_source_mapping(&mut self, ast: &Ast, node: NodeId) {
        self.mapped.end_source_mapping(ast, node);
    }
    fn track_licenses(&mut self, ast: &Ast, node: NodeId) {
        self.mapped.track_licenses(ast, node);
    }
    /// Appends an appropriately indented string to the code, keeping track of the current line count
    /// and line length.
    // port: CodePrinter.PrettyCodePrinter#append
    fn append(&mut self, str: &JsString) {
        // For pretty printing: indent at the beginning of the line, except template literal lines.
        if self.mapped.line_length == 0 && !self.is_in_template_literal() {
            for _ in 0..self.indent {
                self.mapped.code.extend(Self::INDENT.encode_utf16());
                self.mapped.line_length += Self::INDENT.len() as i32;
            }
        }
        self.mapped.append(str);
    }
    /// Attempt to read the number format out of the original source location, falling back to the
    /// default behavior if we cannot locate it.
    // port: CodePrinter.PrettyCodePrinter#addNumber
    fn add_number(&mut self, x: f64, ast: &Ast, n: Option<NodeId>) {
        check_state!(
            closure_rhino::jscomp_base::js_comp_doubles::JSCompDoubles::is_positive(x),
            "%s",
            closure_rhino::java_lang::double_to_string(x)
        );
        // Java getNumberFromSource dereferences n before checking its token.
        let number_from_source =
            Self::get_number_from_source(ast, n.expect("NullPointerException: number node"));
        let Some(number_from_source) = number_from_source else {
            crate::code_consumer::add_number(self, x);
            return;
        };
        // The string we extract from the source code is not always a number.
        // Conservatively, we only use it if we can verify that it is as a number
        // with the right value. This excludes some valid constants (hex, etc.)
        // for simplicity.
        let d = match closure_rhino::java_lang::double::parse_double(&number_from_source) {
            Ok(d) => d,
            Err(_) => {
                crate::code_consumer::add_number(self, x);
                return;
            }
        };
        if x != d {
            crate::code_consumer::add_number(self, x);
            return;
        }
        self.add_constant(&number_from_source);
    }
    /// Adds a newline to the code, resetting the line length and handling indenting for pretty
    /// printing.
    // port: CodePrinter.PrettyCodePrinter#startNewLine
    fn start_new_line(&mut self) {
        if self.mapped.line_length <= 0 && !self.is_in_template_literal() {
            return;
        }
        self.mapped.code.push(b'\n' as u16);
        self.mapped.line_index += 1;
        self.mapped.line_length = 0;
    }
    // port: CodePrinter.PrettyCodePrinter#maybeLineBreak
    fn maybe_line_break(&mut self) {
        self.maybe_cut_line();
    }
    /// This may start a new line if the current line is longer than the line
    /// length threshold.
    // port: CodePrinter.PrettyCodePrinter#maybeCutLine
    fn maybe_cut_line(&mut self) {
        if self.mapped.line_length > self.mapped.line_length_threshold {
            self.start_new_line();
        }
    }
    // port: CodePrinter.PrettyCodePrinter#endLine
    fn end_line(&mut self) {
        self.start_new_line();
    }
    // port: CodePrinter.PrettyCodePrinter#appendBlockStart
    fn append_block_start(&mut self) {
        self.maybe_insert_space();
        self.add(&"{".into());
        self.indent += 1;
    }
    // port: CodePrinter.PrettyCodePrinter#appendBlockEnd
    fn append_block_end(&mut self) {
        self.maybe_end_statement();
        self.end_line();
        self.indent -= 1;
        self.add(&"}".into());
    }
    // port: CodePrinter.PrettyCodePrinter#listSeparator
    fn list_separator(&mut self) {
        self.add(&", ".into());
        self.maybe_line_break();
    }
    // port: CodePrinter.PrettyCodePrinter#optionalListSeparator
    fn optional_list_separator(&mut self) {
        self.add(&",".into());
        self.maybe_line_break();
    }
    // port: CodePrinter.PrettyCodePrinter#endFunction
    fn end_function(&mut self, statement_context: bool) {
        self.state_mut().saw_function = true;
        if statement_context {
            self.end_line();
        }
        if statement_context {
            self.start_new_line();
        }
    }
    // port: CodePrinter.PrettyCodePrinter#beginCaseBody
    fn begin_case_body(&mut self) {
        self.append(&":".into());
        self.indent += 1;
        self.end_line();
    }
    // port: CodePrinter.PrettyCodePrinter#endCaseBody
    fn end_case_body(&mut self) {
        self.indent -= 1;
    }
    // port: CodePrinter.PrettyCodePrinter#appendOp
    fn append_op(&mut self, op: &str, bin_op: bool) {
        if self.get_last_char() != b' ' as u16 && bin_op && !op.starts_with(',') {
            self.add(&" ".into());
        }
        self.add(&op.into());
        if bin_op {
            self.add(&" ".into());
        }
    }
    /// If the body of a for loop or the then clause of an if statement has a single statement,
    /// should it be wrapped in a block? And similar. {@inheritDoc}
    // port: CodePrinter.PrettyCodePrinter#shouldPreserveExtras
    fn should_preserve_extras(&self, ast: &Ast, n: NodeId) -> bool {
        // When pretty-printing, always place the statement in its own block so it is printed on a
        // separate line. This allows breakpoints to be placed on the statement.
        // The only exception is an added block around an else-if statement. Example code:
        // if (0) {
        // 0;
        // } else if (1) {
        // 1;
        // }
        // Resulting node tree:
        // IF
        // NUMBER
        // BLOCK
        // EXPR_RESULT
        // NUMBER
        // BLOCK [added_block: 1] <-- we don't want to print this block
        // IF
        // NUMBER
        // BLOCK
        // EXPR_RESULT
        // NUMBER
        if !n.is_block(ast) || !n.is_added_block(ast) || !n.has_parent(ast) {
            return true;
        }
        let parent = n.get_parent(ast).unwrap();
        let is_else = parent.is_if(ast)
            && parent.has_x_children(ast, 3)
            && Some(n) == parent.get_last_child(ast);
        let only_child_is_if = n.has_one_child(ast) && n.get_first_child(ast).unwrap().is_if(ast);
        if is_else && only_child_is_if {
            return false;
        }
        true
    }
    // port: CodePrinter.PrettyCodePrinter#maybeInsertSpace
    fn maybe_insert_space(&mut self) {
        if self.get_last_char() != b' ' as u16 && self.get_last_char() != b'\n' as u16 {
            self.add(&" ".into());
        }
    }
    /// @return Whether the a line break should be added after the specified
    /// BLOCK.
    // port: CodePrinter.PrettyCodePrinter#breakAfterBlockFor
    fn break_after_block_for(&self, ast: &Ast, n: NodeId, _is_statement_context: bool) -> bool {
        check_state!(n.is_block(ast), "%s", n.to_string(ast));
        let parent = n.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            closure_rhino::token::Token::DO => false,
            // Don't break before 'while' in DO-WHILE statements.
            closure_rhino::token::Token::FUNCTION => false,
            // FUNCTIONs are handled separately, don't break here.
            closure_rhino::token::Token::TRY => Some(n) != parent.get_first_child(ast),
            // Don't break before catch
            closure_rhino::token::Token::CATCH => {
                !NodeUtil::has_finally(ast, Self::get_try_for_catch(ast, parent))
            }
            // Don't break before finally
            closure_rhino::token::Token::IF => Some(n) == parent.get_last_child(ast),
            // Don't break before else
            _ => true,
        }
    }
    // port: CodePrinter.PrettyCodePrinter#endStatement
    fn end_statement_with_semicolon(
        &mut self,
        _needs_semicolon: bool,
        has_trailing_comment_on_same_line: bool,
    ) {
        self.add(&";".into());
        if !has_trailing_comment_on_same_line {
            self.end_line();
        }
        self.state_mut().statement_needs_ended = false;
    }
    // port: CodePrinter.PrettyCodePrinter#endFile
    fn end_file(&mut self) {
        self.maybe_end_statement();
    }
}
pub struct CompactCodePrinter<'a> {
    pub mapped: MappedCodePrinter<'a>,
    // The CompactCodePrinter tries to emit just enough newlines to stop there
    // being lines longer than the threshold.  Since the output is going to be
    // gzipped, it makes sense to try to make the newlines appear in similar
    // contexts so that gzip can encode them for 'free'.
    // This version tries to break the lines at 'preferred' places, which are
    // between the top-level forms.  This works because top-level forms tend to
    // be more uniform than arbitrary legal contexts.  Better compression would
    // probably require explicit modeling of the gzip algorithm.
    line_break: bool,
    line_start_position: i32,
    preferred_break_position: i32,
}
impl<'a> CompactCodePrinter<'a> {
    /// @param lineBreak break the lines a bit more aggressively
    /// @param lineLengthThreshold The length of a line after which we force a newline when possible.
    /// @param createSrcMap Whether to gather source position mapping information when printing.
    /// @param sourceMapDetailLevel A filter to control which nodes get mapped into the source map.
    /// @param licenseTracker A license tracking implementation to manage license text emit. The
    /// CodePrinter will never emit license information directly - it only ever passes nodes to
    /// the license tracker to request tracking.
    // port: CodePrinter.CompactCodePrinter#CompactCodePrinter
    pub fn new(
        line_break: bool,
        line_length_threshold: i32,
        create_src_map: bool,
        source_map_detail_level: DetailLevel,
        license_tracker: Option<&'a mut dyn LicenseTracker>,
    ) -> Self {
        Self {
            mapped: MappedCodePrinter::new(
                line_length_threshold,
                create_src_map,
                source_map_detail_level,
                license_tracker,
            ),
            line_break,
            line_start_position: 0,
            preferred_break_position: 0,
        }
    }
}
impl CodeConsumer for CompactCodePrinter<'_> {
    fn state(&self) -> &CodeConsumerState {
        self.mapped.state()
    }
    fn state_mut(&mut self) -> &mut CodeConsumerState {
        self.mapped.state_mut()
    }
    fn get_last_char(&self) -> u16 {
        self.mapped.get_last_char()
    }
    fn append(&mut self, str: &JsString) {
        self.mapped.append(str);
    }
    fn start_source_mapping(&mut self, ast: &Ast, node: NodeId) {
        self.mapped.start_source_mapping(ast, node);
    }
    fn end_source_mapping(&mut self, ast: &Ast, node: NodeId) {
        self.mapped.end_source_mapping(ast, node);
    }
    fn track_licenses(&mut self, ast: &Ast, node: NodeId) {
        self.mapped.track_licenses(ast, node);
    }
    /// Adds a newline to the code, resetting the line length.
    // port: CodePrinter.CompactCodePrinter#startNewLine
    fn start_new_line(&mut self) {
        if self.mapped.line_length <= 0 && !self.is_in_template_literal() {
            return;
        }
        self.mapped.code.push(b'\n' as u16);
        self.mapped.line_length = 0;
        self.mapped.line_index += 1;
        self.line_start_position = self.mapped.code.len() as i32;
    }
    // port: CodePrinter.CompactCodePrinter#maybeLineBreak
    fn maybe_line_break(&mut self) {
        if self.line_break {
            if self.state().saw_function {
                self.start_new_line();
                self.state_mut().saw_function = false;
            }
        }
        // Since we are at a legal line break, can we upgrade the
        // preferred break position?  We prefer to break after a
        // semicolon rather than before it.
        let len = self.mapped.code.len() as i32;
        if self.preferred_break_position == len - 1 {
            let ch = self.mapped.code[(len - 1) as usize];
            if ch == b';' as u16 {
                self.preferred_break_position = len;
            }
        }
        self.maybe_cut_line();
    }
    /// This may start a new line if the current line is longer than the line
    /// length threshold.
    // port: CodePrinter.CompactCodePrinter#maybeCutLine
    fn maybe_cut_line(&mut self) {
        if self.mapped.line_length <= self.mapped.line_length_threshold {
            return;
        }
        // Use the preferred position provided it will break the line.
        if self.preferred_break_position > self.line_start_position
            && self.preferred_break_position < self.line_start_position + self.mapped.line_length
        {
            // If the preferred break position is on the current line.
            self.mapped
                .code
                .insert(self.preferred_break_position as usize, b'\n' as u16);
            self.mapped.report_line_cut(
                self.mapped.line_index,
                self.preferred_break_position - self.line_start_position,
            );
            self.mapped.line_index += 1;
            self.mapped.line_length -= self.preferred_break_position - self.line_start_position;
            // Jump over the inserted newline.
            self.line_start_position = self.preferred_break_position + 1;
        } else {
            self.start_new_line();
        }
    }
    // port: CodePrinter.CompactCodePrinter#notePreferredLineBreak
    fn note_preferred_line_break(&mut self) {
        self.preferred_break_position = self.mapped.code.len() as i32;
    }
}
/// License Trackers are responsible for ensuring that any licensing information attached to nodes
/// is retained in the final output of JSCompiler.
///
/// <p>The Code Printer will visit every node being printed and call trackLicensesForNode for that
/// node. Later, some other code can call emitLicenses to retrieve all relevant licenses. Deciding
/// what is relevant varies between implementations of License Trackers - read their javadoc to
/// determine how they are intended to be used.
pub trait LicenseTracker {
    // port: CodePrinter.LicenseTracker#trackLicensesForNode
    fn track_licenses_for_node(&mut self, ast: &Ast, node: NodeId);
    // port: CodePrinter.LicenseTracker#emitLicenses
    fn emit_licenses(&self) -> IndexSet<JsString>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Specifies a format for code generation.
pub enum Format {
    COMPACT,
    PRETTY,
    TYPED,
}
impl Format {
    // port: CodePrinter.Format#fromOptions
    pub fn from_options(options: &CompilerOptions, output_types: bool, pretty_print: bool) -> Self {
        if output_types {
            return Self::TYPED;
        }
        if pretty_print
            || options
                .get_output_feature_set()
                .contains(Feature::TYPE_ANNOTATION)
        {
            return Self::PRETTY;
        }
        Self::COMPACT
    }
}
// Java CodePrinter.SourceAndMappings (fields and implicit constructor).
/// SourceAndMappings bundles together the source and generated SourceMap Mappings for that source.
pub struct SourceAndMappings {
    pub source: JsString,
    pub mappings: Option<Vec<Mapping>>,
}
pub trait CodeGeneratorFactory {
    // port: CodePrinter.Builder.CodeGeneratorFactory#getCodeGenerator
    fn get_code_generator<'a>(
        &'a self,
        output_format: Format,
        cc: &'a mut dyn CodeConsumer,
        options: &CompilerOptions,
        registry: Option<&'a mut JSTypeRegistry>,
    ) -> Box<dyn crate::code_generator::CodeGeneration<'a> + 'a>;
}
pub struct DefaultCodeGeneratorFactory;
impl CodeGeneratorFactory for DefaultCodeGeneratorFactory {
    // port: CodePrinter.Builder.anonymous#getCodeGenerator
    fn get_code_generator<'a>(
        &'a self,
        output_format: Format,
        cc: &'a mut dyn CodeConsumer,
        options: &CompilerOptions,
        registry: Option<&'a mut JSTypeRegistry>,
    ) -> Box<dyn crate::code_generator::CodeGeneration<'a> + 'a> {
        if output_format == Format::TYPED {
            Box::new(crate::typed_code_generator::TypedCodeGenerator::new(
                cc,
                options,
                registry.expect("NullPointerException: JSTypeRegistry"),
            ))
        } else {
            Box::new(crate::code_generator::CodeGenerator::new(cc, options))
        }
    }
}
// Java stores either its default options or the caller's options by reference.
enum CompilerOptionsRef<'a> {
    Owned(Box<CompilerOptions>),
    Borrowed(&'a CompilerOptions),
}
impl Deref for CompilerOptionsRef<'_> {
    type Target = CompilerOptions;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(options) => options,
            Self::Borrowed(options) => options,
        }
    }
}
pub struct Builder<'a> {
    root: NodeId,
    options: CompilerOptionsRef<'a>,
    line_break: bool,
    pretty_print: bool,
    output_types: bool,
    tag_as_type_summary: bool,
    tag_as_strict: bool,
    license_tracker: Option<&'a mut dyn LicenseTracker>,
    registry: Option<&'a mut JSTypeRegistry>,
    // may be null unless using Format.TYPED
    code_generator_factory: Box<dyn CodeGeneratorFactory + 'a>,
}
impl<'a> Builder<'a> {
    /// Sets the root node from which to generate the source code.
    /// @param node The root node.
    // port: CodePrinter.Builder#Builder
    pub fn new(node: NodeId) -> Self {
        Self {
            root: node,
            options: CompilerOptionsRef::Owned(Box::default()),
            line_break: false,
            pretty_print: false,
            output_types: false,
            tag_as_type_summary: false,
            tag_as_strict: false,
            license_tracker: None,
            registry: None,
            code_generator_factory: Box::new(DefaultCodeGeneratorFactory),
        }
    }
    /// Sets the output options from compiler options.
    // port: CodePrinter.Builder#setCompilerOptions
    pub fn set_compiler_options(mut self, options: &'a CompilerOptions) -> Self {
        self.options = CompilerOptionsRef::Borrowed(options);
        self.pretty_print = options.is_pretty_print();
        self.line_break = options.should_add_line_break();
        self
    }
    // port: CodePrinter.Builder#setTypeRegistry
    pub fn set_type_registry(mut self, registry: &'a mut JSTypeRegistry) -> Self {
        self.registry = Some(registry);
        self
    }
    /// Sets whether pretty printing should be used.
    ///
    /// @param prettyPrint If true, pretty printing will be used.
    // port: CodePrinter.Builder#setPrettyPrint
    pub fn set_pretty_print(mut self, pretty_print: bool) -> Self {
        self.pretty_print = pretty_print;
        self
    }
    /// Sets whether line breaking should be done automatically.
    ///
    /// @param lineBreak If true, line breaking is done automatically.
    // port: CodePrinter.Builder#setLineBreak
    pub fn set_line_break(mut self, line_break: bool) -> Self {
        self.line_break = line_break;
        self
    }
    /// Sets whether to output closure-style type annotations.
    ///
    /// @param outputTypes If true, outputs closure-style type annotations.
    // port: CodePrinter.Builder#setOutputTypes
    pub fn set_output_types(mut self, output_types: bool) -> Self {
        self.output_types = output_types;
        self
    }
    /// Sets the license tracker to use when printing.
    ///
    /// @param licenseTracker The tracker to use. Can be null to disable license tracking.
    // port: CodePrinter.Builder#setLicenseTracker
    pub fn set_license_tracker(
        mut self,
        license_tracker: Option<&'a mut dyn LicenseTracker>,
    ) -> Self {
        self.license_tracker = license_tracker;
        self
    }
    /// Set whether the output should be tagged as an .i.js file.
    // port: CodePrinter.Builder#setTagAsTypeSummary
    pub fn set_tag_as_type_summary(mut self, tag_as_type_summary: bool) -> Self {
        self.tag_as_type_summary = tag_as_type_summary;
        self
    }
    /// Set whether the output should be tags as ECMASCRIPT 5 Strict.
    // port: CodePrinter.Builder#setTagAsStrict
    pub fn set_tag_as_strict(mut self, tag_as_strict: bool) -> Self {
        self.tag_as_strict = tag_as_strict;
        self
    }
    /// Set a custom code generator factory to enable custom code generation.
    // port: CodePrinter.Builder#setCodeGeneratorFactory
    pub fn set_code_generator_factory(mut self, factory: impl CodeGeneratorFactory + 'a) -> Self {
        self.code_generator_factory = Box::new(factory);
        self
    }
    /// Generates the source code and returns it.
    // port: CodePrinter.Builder#build
    pub fn build(self, ast: &Ast) -> JsString {
        self.build_with_source_mappings(ast).source
    }
    // port: CodePrinter.Builder#buildWithSourceMappings
    pub fn build_with_source_mappings(self, ast: &Ast) -> SourceAndMappings {
        to_source(
            ast,
            self.root,
            Format::from_options(&self.options, self.output_types, self.pretty_print),
            &self.options,
            self.license_tracker,
            self.registry,
            self.tag_as_type_summary,
            self.tag_as_strict,
            self.line_break,
            self.code_generator_factory.as_ref(),
        )
    }
}
trait MappedConsumer<'a>: CodeConsumer {
    fn mapped(&self) -> &MappedCodePrinter<'a>;
}
impl<'a> MappedConsumer<'a> for CompactCodePrinter<'a> {
    fn mapped(&self) -> &MappedCodePrinter<'a> {
        &self.mapped
    }
}
impl<'a> MappedConsumer<'a> for PrettyCodePrinter<'a> {
    fn mapped(&self) -> &MappedCodePrinter<'a> {
        &self.mapped
    }
}
/// Converts a tree to JS code
// port: CodePrinter#toSource
fn to_source(
    ast: &Ast,
    root: NodeId,
    output_format: Format,
    options: &CompilerOptions,
    license_tracker: Option<&mut dyn LicenseTracker>,
    registry: Option<&mut JSTypeRegistry>,
    tag_as_type_summary: bool,
    tag_as_strict: bool,
    line_break: bool,
    code_generator_factory: &dyn CodeGeneratorFactory,
) -> SourceAndMappings {
    let mut mcp: Box<dyn MappedConsumer<'_> + '_> = if output_format == Format::COMPACT {
        Box::new(CompactCodePrinter::new(
            line_break,
            options.get_line_length_threshold(),
            options.should_gather_source_map_info(),
            options.get_source_map_detail_level(),
            license_tracker,
        ))
    } else {
        Box::new(PrettyCodePrinter::new(
            options.get_line_length_threshold(),
            options.should_gather_source_map_info(),
            options.get_source_map_detail_level(),
            license_tracker,
        ))
    };
    {
        let mut cg = code_generator_factory.get_code_generator(
            output_format,
            mcp.as_mut(),
            options,
            registry,
        );
        if tag_as_type_summary {
            cg.tag_as_type_summary();
        }
        if tag_as_strict {
            cg.tag_as_strict();
        }
        cg.add_node(ast, root);
    }
    mcp.end_file();
    let source = mcp.mapped().get_code();
    let mappings = if options.should_gather_source_map_info() {
        mcp.mapped().get_source_mappings(&source)
    } else {
        None
    };
    SourceAndMappings { source, mappings }
}
