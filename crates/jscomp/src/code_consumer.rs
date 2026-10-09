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
//   src/com/google/javascript/jscomp/CodeConsumer.java.

#![allow(clippy::if_same_then_else)] // Retain Java token-boundary branches.
use closure_rhino::{
    check_state, java_lang,
    js_string::JsString,
    jscomp_base::js_comp_doubles::JSCompDoubles,
    node::{Ast, NodeId},
};
use num_bigint::BigInt;

#[derive(Default)]
pub struct CodeConsumerState {
    pub statement_needs_ended: bool,
    pub statement_started: bool,
    pub saw_function: bool,
    // State tracking for template literals. Remember that template literal substitutions can contain
    // additional template literals.
    template_lit_depth: i32,
    template_lit_sub_depth: i32,
}

/// Abstracted consumer of the CodeGenerator output.
///
/// @see CodeGenerator
/// @see CodePrinter
/// @see InlineCostEstimator
pub trait CodeConsumer {
    fn state(&self) -> &CodeConsumerState;
    fn state_mut(&mut self) -> &mut CodeConsumerState;
    /// Starts the source mapping for the given
    /// node at the current position.
    // port: CodeConsumer#startSourceMapping
    fn start_source_mapping(&mut self, _ast: &Ast, _node: NodeId) {}
    /// Finishes the source mapping for the given
    /// node at the current position.
    // port: CodeConsumer#endSourceMapping
    fn end_source_mapping(&mut self, _ast: &Ast, _node: NodeId) {}
    /// Indicates to the CodeConsumer that this Node might carry licensing information, and allows the
    /// code consumer to manage licenses as it sees fit.
    // port: CodeConsumer#trackLicenses
    fn track_licenses(&mut self, _ast: &Ast, _node: NodeId) {}
    /// Provides a means of interrupting the CodeGenerator. Derived classes should return false to stop
    /// further processing.
    // port: CodeConsumer#continueProcessing
    fn continue_processing(&self) -> bool {
        true
    }
    /// Retrieve the last character of the last string sent to append.
    // port: CodeConsumer#getLastChar
    fn get_last_char(&self) -> u16;
    /// Appends a (possibly multiline) string to the code, keeping track of the current line length and
    /// count.
    ///
    /// <p>Clients should use this method rather than {@link #append()}. It sanitizes the input to
    /// {@link #append()}.
    // port: CodeConsumer#add
    fn add(&mut self, newcode: &JsString) {
        self.maybe_end_statement();
        if newcode.is_empty() {
            return;
        }
        let c = newcode.char_at(0);
        if (is_word_char(c) || c == b'\\' as u16) && is_word_char(self.get_last_char()) {
            // need space to separate. This is not pretty printing.
            // For example: "return foo;"
            self.append(&" ".into());
        } else if c == b'/' as u16
            && (self.get_last_char() == b'/' as u16 || self.get_last_char() == b'<' as u16)
        {
            // Do not allow a forward slash to appear after a DIV or LT.
            // For example,
            // REGEXP DIV REGEXP
            // is valid and should print like
            // / // / /
            self.append(&" ".into());
        } else if (c == b'"' as u16 || c == b'\'' as u16) && is_word_char(self.get_last_char()) {
            self.maybe_insert_space();
        }
        // Iterate through the new code and add each contained line, followed by a break. Remember that
        // the string may start and end in the middle of a line. We do this rather primitively because
        // this method is called frequently and most invocations have only one line.
        // TODO(nickreid): There are other possible newline characters recognized by the JS spec. We
        // should also be considering them.
        let mut start_of_line = 0;
        let mut end_of_line = newcode.index_of_char(b'\n' as u16);
        while end_of_line >= 0 {
            if end_of_line as usize > start_of_line {
                // Append line only if it is non-empty.
                self.append(&newcode.substring(start_of_line, end_of_line as usize));
            }
            // Breaking is non-optional. Newlines added this way must be preserved (e.g. newlines in
            // template literals).
            self.start_new_line();
            start_of_line = end_of_line as usize + 1; // Jump over the newline char.
            end_of_line = newcode.index_of_from("\n", start_of_line as i32);
        }
        if newcode.length() > start_of_line {
            // Append line only if it is non-empty.
            // Append the last or only line without breaking.
            self.append(&newcode.substring_from(start_of_line));
        }
    }
    /// Appends a string to the code, keeping track of the current line length.
    ///
    /// <p>Clients should not call this method directly, but instead call add {@link #add()}.
    ///
    /// <p>The string must be a complete token; partial strings or partial regexes will run the risk of
    /// being split across lines.
    ///
    /// <p>Implementations of this method need not consider newline characters in {@code str}. Such
    /// characters should either be ignored or cause an {@link Exception}.
    // port: CodeConsumer#append
    fn append(&mut self, str: &JsString);
    // port: CodeConsumer#addIdentifier
    fn add_identifier(&mut self, identifier: &JsString) {
        self.add(identifier);
    }
    // port: CodeConsumer#appendBlockStart
    fn append_block_start(&mut self) {
        self.append(&"{".into());
    }
    // port: CodeConsumer#appendBlockEnd
    fn append_block_end(&mut self) {
        self.append(&"}".into());
    }
    // port: CodeConsumer#startNewLine
    fn start_new_line(&mut self) {}
    // port: CodeConsumer#maybeLineBreak
    fn maybe_line_break(&mut self) {
        self.maybe_cut_line();
    }
    // port: CodeConsumer#maybeCutLine
    fn maybe_cut_line(&mut self) {}
    // port: CodeConsumer#endLine
    fn end_line(&mut self) {}
    // port: CodeConsumer#notePreferredLineBreak
    fn note_preferred_line_break(&mut self) {}
    // port: CodeConsumer#beginBlock
    fn begin_block(&mut self) {
        if self.state().statement_needs_ended {
            self.append(&";".into());
            self.maybe_line_break();
        }
        self.append_block_start();
        self.end_line();
        self.state_mut().statement_needs_ended = false;
    }
    // port: CodeConsumer#endBlock()
    fn end_block(&mut self) {
        self.end_block_with_end_line(false);
    }
    // port: CodeConsumer#endBlock(boolean)
    fn end_block_with_end_line(&mut self, should_end_line: bool) {
        self.append_block_end();
        if should_end_line {
            self.end_line();
        }
        self.state_mut().statement_needs_ended = false;
    }
    // port: CodeConsumer#listSeparator
    fn list_separator(&mut self) {
        self.add(&",".into());
        self.maybe_line_break();
    }
    // port: CodeConsumer#optionalListSeparator
    fn optional_list_separator(&mut self) {}
    /// Indicates the end of a statement and a ';' may need to be added. But we don't add it now, in
    /// case we're at the end of a block (in which case we don't have to add the ';'). See
    /// maybeEndStatement()
    // port: CodeConsumer#endStatement(boolean)
    fn end_statement(&mut self, has_trailing_comment_on_same_line: bool) {
        self.end_statement_with_semicolon(false, has_trailing_comment_on_same_line);
    }
    // port: CodeConsumer#endStatement(boolean,boolean)
    fn end_statement_with_semicolon(
        &mut self,
        need_semi_colon: bool,
        has_trailing_comment_on_same_line: bool,
    ) {
        if need_semi_colon {
            self.append(&";".into());
            if !has_trailing_comment_on_same_line {
                self.maybe_line_break();
            }
            self.state_mut().statement_needs_ended = false;
        } else if self.state().statement_started {
            self.state_mut().statement_needs_ended = true;
        }
    }
    /// This is to be called when we're in a statement. If the prev statement
    /// needs to be ended, add a ';'.
    // port: CodeConsumer#maybeEndStatement
    fn maybe_end_statement(&mut self) {
        // Add a ';' if we need to.
        if self.state().statement_needs_ended {
            self.append(&";".into());
            self.maybe_line_break();
            self.end_line();
            self.state_mut().statement_needs_ended = false;
        }
        self.state_mut().statement_started = true;
    }
    // port: CodeConsumer#endFunction
    fn end_function(&mut self, statement_context: bool) {
        self.state_mut().saw_function = true;
        if statement_context {
            self.end_line();
        }
    }
    // port: CodeConsumer#endClass
    fn end_class(&mut self, statement_context: bool) {
        if statement_context {
            self.end_line();
        }
    }
    // port: CodeConsumer#beginCaseBody
    fn begin_case_body(&mut self) {
        self.append(&":".into());
    }
    // port: CodeConsumer#endCaseBody
    fn end_case_body(&mut self) {}
    // port: CodeConsumer#beginTemplateLit
    fn begin_template_lit(&mut self) {
        check_state!(self.state().template_lit_depth == self.state().template_lit_sub_depth);
        self.maybe_end_statement();
        self.append(&"`".into());
        self.state_mut().template_lit_depth += 1;
    }
    // port: CodeConsumer#beginTemplateLitSub
    fn begin_template_lit_sub(&mut self) {
        // This method pair exists because '$' behaves differently inside template literals. We want to
        // append it without sanitizing in the generic way {@link #add()} would.
        check_state!(self.is_in_template_literal());
        self.append(&"${".into());
        self.state_mut().template_lit_sub_depth += 1;
    }
    // port: CodeConsumer#endTemplateLitSub
    fn end_template_lit_sub(&mut self) {
        // This method pair exists because '$' behaves differently inside template literals. We want to
        // append it without sanitizing in the generic way {@link #add()} would.
        check_state!(self.state().template_lit_sub_depth > 0);
        check_state!(self.state().template_lit_depth == self.state().template_lit_sub_depth);
        self.append(&"}".into());
        self.state_mut().template_lit_sub_depth -= 1;
    }
    // port: CodeConsumer#endTemplateLit
    fn end_template_lit(&mut self) {
        check_state!(self.state().template_lit_depth > 0);
        check_state!(self.is_in_template_literal());
        self.append(&"`".into());
        self.state_mut().template_lit_depth -= 1;
    }
    // port: CodeConsumer#isInTemplateLiteral
    fn is_in_template_literal(&self) -> bool {
        // We're inside a template literal but not a substitution within that literal.
        self.state().template_lit_depth == self.state().template_lit_sub_depth + 1
    }
    // port: CodeConsumer#appendOp
    fn append_op(&mut self, op: &str, _bin_op: bool) {
        self.append(&op.into());
    }
    // port: CodeConsumer#addOp
    fn add_op(&mut self, op: &str, bin_op: bool) {
        self.maybe_end_statement();
        let first = op.encode_utf16().next().unwrap();
        let prev = self.get_last_char();
        if (first == b'+' as u16 || first == b'-' as u16) && prev == first {
            // This is not pretty printing. This is to prevent misparsing of
            // things like "x + ++y" or "x++ + ++y"
            self.append(&" ".into());
        } else if java_lang::is_letter(first) && is_word_char(prev) {
            // Make sure there is a space after e.g. instanceof , typeof
            self.append(&" ".into());
        } else if (prev == b'-' as u16 && first == b'>' as u16)
            || (prev == b'<' as u16 && first == b'!' as u16)
        {
            // Make sure that we don't emit "<!--" or "-->"
            self.append(&" ".into());
        }
        // Allow formatting around the operator.
        self.append_op(op, bin_op);
        // Line breaking after an operator is always safe. Line breaking before an
        // operator on the other hand is not. We only line break after a bin op
        // because it looks strange.
        if bin_op {
            self.maybe_cut_line();
        }
    }
    // port: CodeConsumer#addNumber
    fn add_number(&mut self, x: f64, _ast: &Ast, _n: Option<NodeId>) {
        add_number(self, x);
    }
    // port: CodeConsumer#addBigInt
    fn add_big_int(&mut self, bi: &BigInt) {
        let hex_encoded = format!("0x{}n", bi.to_str_radix(16));
        let decimal_encoded = format!("{bi}n");
        self.add_constant(
            &if hex_encoded.len() < decimal_encoded.len() {
                hex_encoded
            } else {
                decimal_encoded
            }
            .into(),
        );
    }
    // port: CodeConsumer#addConstant
    fn add_constant(&mut self, newcode: &JsString) {
        self.add(newcode);
    }
    /// If the body of a for loop or the then clause of an if statement has a single statement, should
    /// it be wrapped in a block? Doing so can help when pretty-printing the code, and permits putting
    /// a debugging breakpoint on the statement inside the condition.
    ///
    /// @param n node to process
    /// @return {@boolean true} if such expressions should be wrapped
    // port: CodeConsumer#shouldPreserveExtras
    fn should_preserve_extras(&self, _ast: &Ast, _n: NodeId) -> bool {
        false
    }
    /// Allows a consumer to insert spaces in locations where it is unnecessary
    /// but may improve the readability of the code. This will be called in such
    /// places as after a statement and before opening parentheses, or after the
    /// end of a if block before the start of an else block.
    // port: CodeConsumer#maybeInsertSpace
    fn maybe_insert_space(&mut self) {}
    /// @return Whether the a line break can be added after the specified BLOCK.
    // port: CodeConsumer#breakAfterBlockFor
    fn break_after_block_for(&self, _ast: &Ast, _n: NodeId, statement_context: bool) -> bool {
        statement_context
    }
    /// Called when we're at the end of a file.
    // port: CodeConsumer#endFile
    fn end_file(&mut self) {}
}
// port: CodeConsumer#addNumber
pub fn add_number<C: CodeConsumer + ?Sized>(cc: &mut C, x: f64) {
    check_state!(
        JSCompDoubles::is_positive(x),
        "%s",
        java_lang::double_to_string(x)
    );
    if (x as i64) as f64 != x {
        let value = java_lang::double_to_string(x).replace(".0E", "E");
        cc.add_constant(
            &value
                .strip_prefix("0.")
                .map_or_else(|| value.clone(), |tail| format!(".{tail}"))
                .into(),
        );
        return;
    }
    let value = x as i64;
    let mut mantissa = value;
    let mut exp = 0;
    if x >= 100.0 {
        while mantissa % 10 == 0 {
            mantissa /= 10;
            exp += 1;
        }
    }
    if exp > 2 {
        cc.add_constant(&format!("{mantissa}E{exp}").into());
        return;
    }
    let dec_value_string = value.to_string();
    // Values <1E12 are shorter in decimal
    if value <= 1_000_000_000_000 {
        cc.add_constant(&dec_value_string.into());
        return;
    }
    let hex_value_string = format!("{value:x}");
    if hex_value_string.len() + 2 < dec_value_string.len() {
        cc.add_constant(&format!("0x{hex_value_string}").into());
    } else {
        cc.add_constant(&dec_value_string.into());
    }
}
// port: CodeConsumer#isWordChar
pub fn is_word_char(ch: u16) -> bool {
    ch == b'_' as u16 || ch == b'$' as u16 || java_lang::is_letter(ch) || java_lang::is_digit(ch)
}
