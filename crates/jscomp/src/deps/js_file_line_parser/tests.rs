/*
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/JsFileLineParser.java,
//   test/com/google/javascript/jscomp/deps/JsFileLineParserTest.java.

use super::*;
use crate::black_hole_error_manager::BlackHoleErrorManager;
// port: JsFileLineParserTest#testSingleLine1
#[test]
fn test_single_line1() {
    assert_strip("2", "// 1\n2\n");
}
// port: JsFileLineParserTest#testSingleLine2
#[test]
fn test_single_line2() {
    assert_strip("2 ", "// 1\n2 // 3 // 4\n");
}
// port: JsFileLineParserTest#testMultiLine1
#[test]
fn test_multi_line1() {
    assert_strip("1", "/* hi */\n1\n");
}
// port: JsFileLineParserTest#testMultiLine2
#[test]
fn test_multi_line2() {
    assert_strip("123", "1/* hi */2\n3\n");
}
// port: JsFileLineParserTest#testMultiLine3
#[test]
fn test_multi_line3() {
    assert_strip("14", "1/* hi 2\n3*/4\n");
}
// port: JsFileLineParserTest#testMultiLine4
#[test]
fn test_multi_line4() {
    assert_strip("15", "1/* hi x\ny\nz*/5\n");
}
// port: JsFileLineParserTest#testMultiLine5
#[test]
fn test_multi_line5() {
    assert_strip("1234", "1/* hi */2/**/3/*\n/** bye */4\n");
}
// port: JsFileLineParserTest#testMultiLine6
#[test]
fn test_multi_line6() {
    assert_strip("12", "1/*** hi *** 3 **/2\n");
}
// port: JsFileLineParserTest#testMixedLine1
#[test]
fn test_mixed_line1() {
    assert_strip("14", "1// /** 2 **/ 3\n4\n");
}
// port: JsFileLineParserTest#testMixedLine2
#[test]
fn test_mixed_line2() {
    assert_strip("1 34", "1/** // 2 **/ 3\n4\n");
}
// port: JsFileLineParserTest#testBlockComment
#[test]
fn test_block_comment() {
    assert_blocks("/** one line */", "/** one line */");
}
// port: JsFileLineParserTest#testBlockComment_singleQuotes
#[test]
fn test_block_comment_single_quotes() {
    assert_blocks("", "var x = '/** one line */';");
}
// port: JsFileLineParserTest#testBlockComment_singleQuote
#[test]
fn test_block_comment_single_quote() {
    assert_blocks("/** one line */", "'/** one line */");
}
// port: JsFileLineParserTest#testBlockComment_singleQuotes_escaped
#[test]
fn test_block_comment_single_quotes_escaped() {
    assert_blocks("", "var x = '\\'/** one line */';");
}
// port: JsFileLineParserTest#testBlockComment_singleQuotes_escapedSlash
#[test]
fn test_block_comment_single_quotes_escaped_slash() {
    assert_blocks("", "var x = '\\'/** one line */\\\\';");
}
// port: JsFileLineParserTest#testBlockComment_doubleQuotes
#[test]
fn test_block_comment_double_quotes() {
    assert_blocks("", "var x = \"/** one line */\";");
}
// port: JsFileLineParserTest#testBlockComment_doubleQuote
#[test]
fn test_block_comment_double_quote() {
    assert_blocks("/** one line */", "\"/** one line */;");
}
// port: JsFileLineParserTest#testBlockComment_doubleQuotes_escaped
#[test]
fn test_block_comment_double_quotes_escaped() {
    assert_blocks("", "var x = \"\\\"/** one line */\";");
}
// port: JsFileLineParserTest#testBlockComment_doubleQuotes_escapedSlash
#[test]
fn test_block_comment_double_quotes_escaped_slash() {
    assert_blocks("", "var x = \"\\\"/** one line */\\\\\";");
}
// port: JsFileLineParserTest#testInlineBlockComment
#[test]
fn test_inline_block_comment() {
    assert_blocks("/** one line */", "var x; /** one line */");
    assert_blocks("/** one line */", "/** one line */ var y;");
    assert_blocks("/** one line */", "var x; /** one line */ var y;");
}
// port: JsFileLineParserTest#testMultipleBlockComments
#[test]
fn test_multiple_block_comments() {
    assert_blocks(
        "/** first *//** * second */",
        "/** first */\n/**\n * second\n */\n",
    );
}

// port: JsFileLineParserTest#assertStrip
fn assert_strip(expected: &str, input: &str) {
    let mut parser = TestParser::new(Arc::new(Mutex::new(BlackHoleErrorManager::new())));
    JsFileLineParser::do_parse(&mut parser, "file", input);
    assert_eq!(parser.to_string(), expected);
}
// port: JsFileLineParserTest#assertBlocks
fn assert_blocks(expected: &str, input: &str) {
    let mut parser = TestParser::new(Arc::new(Mutex::new(BlackHoleErrorManager::new())));
    JsFileLineParser::do_parse(&mut parser, "file", input);
    assert_eq!(parser.comments, expected);
}
struct TestParser {
    base: JsFileLineParser,
    sb: JsString,
    comments: JsString,
}
impl TestParser {
    // port: JsFileLineParserTest.TestParser#TestParser
    fn new(manager: SharedErrorManager) -> Self {
        Self {
            base: JsFileLineParser::new(manager),
            sb: JsString::from(""),
            comments: JsString::from(""),
        }
    }
}
impl LineParser for TestParser {
    // port: JsFileLineParser#JsFileLineParser (base fields)
    fn base(&mut self) -> &mut JsFileLineParser {
        &mut self.base
    }
    // port: JsFileLineParserTest.TestParser#parseLine
    fn parse_line(&mut self, line: &JsString) -> Result<bool, ParseException> {
        self.sb = self.sb.concat(line);
        Ok(true)
    }
    // port: JsFileLineParserTest.TestParser#parseJsDocCommentLine
    fn parse_js_doc_comment_line(&mut self, line: &JsString) -> bool {
        self.comments = self.comments.concat(line);
        true
    }
}
impl fmt::Display for TestParser {
    // port: JsFileLineParserTest.TestParser#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.sb.to_string_lossy())
    }
}

// port: JsFileLineParser#doParse (UTF-16 and BufferedReader line-ending regression)
#[test]
fn preserves_lone_surrogates_and_cr_lines() {
    let mut parser = TestParser::new(Arc::new(Mutex::new(BlackHoleErrorManager::new())));
    let source = JsString::from_units(vec![
        0xd800, 47, 42, 42, 0xdfff, 42, 47, 0xd83d, 0xde00, 13, 10, 0xdc00, 13, 65, 10,
    ]);
    JsFileLineParser::do_parse(&mut parser, "file", source);
    assert_eq!(parser.sb.as_units(), [0xd800, 0xd83d, 0xde00, 0xdc00, 65]);
    assert_eq!(parser.comments.as_units(), [47, 42, 42, 0xdfff, 42, 47]);
    assert_eq!(parser.base.line_num, 3);
    assert!(parser.base.did_parse_succeed());
}
