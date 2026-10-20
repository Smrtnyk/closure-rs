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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit ff925ed:
//   test/com/google/javascript/jscomp/regex/RegExpTreeTest.java.

//! Port of `com.google.javascript.jscomp.regex.RegExpTreeTest`.

use closure_regex::char_ranges::{self, CharRanges};
use closure_regex::reg_exp_tree::{DecomposedCharset, RegExpException, RegExpTree};
use closure_rhino::js_string::JsString;

// port: RegExpTreeTest#parseRegExpAndPrintPattern
fn parse_reg_exp_and_print_pattern(regex: &str, flags: &str) -> Result<String, RegExpException> {
    let tree = RegExpTree::parse_reg_exp(&JsString::from(regex), &JsString::from(flags))?;
    let mut sb: Vec<u16> = Vec::new();
    tree.append_source_code(&mut sb);
    Ok(String::from_utf16(&sb).unwrap())
}

// port: RegExpTreeTest#exceptionFrom
fn exception_from(regex: &str, flags: &str) -> RegExpException {
    match parse_reg_exp_and_print_pattern(regex, flags) {
        Ok(printed) => {
            panic!("Expected exception, but none was thrown. Instead got back: {printed}")
        }
        Err(thrown_exception) => thrown_exception,
    }
}

// port: RegExpTreeTest#assertRegexCompilesTo
fn assert_regex_compiles_to(regex: &str, flags: &str, expected: &str) {
    match parse_reg_exp_and_print_pattern(regex, flags) {
        Ok(printed) => assert_eq!(printed, expected, "regex {regex:?} flags {flags:?}"),
        Err(e) => panic!("regex {regex:?} flags {flags:?} threw {e:?}"),
    }
}

/// `assertRegexThrowsExceptionThat(regex, flags).hasMessageThat().isEqualTo(message)`
// port: RegExpTreeTest#assertRegexThrowsExceptionThat
fn assert_regex_throws_exception_with_message(regex: &str, flags: &str, message: &str) {
    let e = exception_from(regex, flags);
    assert_eq!(
        e.get_message().map(JsString::to_string_lossy).as_deref(),
        Some(message),
        "regex {regex:?} flags {flags:?}"
    );
}

// port: RegExpTreeTest#assertRegexCompilesToSame
fn assert_regex_compiles_to_same(regex: &str, flags: &str) {
    assert_regex_compiles_to(regex, flags, regex);
}

// port: RegExpTreeTest#testValidEs2018LookbehindAssertions
#[test]
fn test_valid_es2018_lookbehind_assertions() {
    assert_regex_compiles_to_same("(?<=asdf)", "");
    assert_regex_compiles_to_same("(?<!asdf)", "");
    assert_regex_compiles_to_same("(?<=(?<!asdf))", "");
    assert_regex_compiles_to_same("(?<=(?<!asdf))", "");
}

// port: RegExpTreeTest#testInvalidEs2018LookbehindAssertions
#[test]
fn test_invalid_es2018_lookbehind_assertions() {
    assert_regex_throws_exception_with_message(
        "(?<asdf)",
        "",
        "Invalid capture group name: <asdf)",
    );
}

// port: RegExpTreeTest#testValidEs2018UnicodePropertyEscapes
#[test]
fn test_valid_es2018_unicode_property_escapes() {
    assert_regex_compiles_to_same("\\p{Script=Greek}", "u");
    assert_regex_compiles_to_same("\\P{Script=Greek}", "u");
    assert_regex_compiles_to_same("\\p{Letter}", "u");
    assert_regex_compiles_to_same("\\P{Letter}", "u");
    assert_regex_compiles_to("\\p", "", "p"); // Without 'u' flag, '\p' is just 'p'
    assert_regex_compiles_to("\\P", "", "P");
}

// port: RegExpTreeTest#testInvalidEs2018UnicodePropertyEscapes
#[test]
fn test_invalid_es2018_unicode_property_escapes() {
    assert_regex_throws_exception_with_message(
        "\\p{",
        "u",
        "Malformed Unicode Property Escape: expected '=' or '}' after \\p{",
    );

    assert_regex_throws_exception_with_message(
        "\\P{",
        "u",
        "Malformed Unicode Property Escape: expected '=' or '}' after \\P{",
    );

    assert_regex_throws_exception_with_message(
        "\\p{=Greek}",
        "u",
        "if '=' is present in a unicode property escape, the name cannot be empty",
    );

    assert_regex_throws_exception_with_message(
        "\\P{=Greek}",
        "u",
        "if '=' is present in a unicode property escape, the name cannot be empty",
    );

    assert_regex_throws_exception_with_message(
        "\\p{}",
        "u",
        "unicode property escape value cannot be empty",
    );

    assert_regex_throws_exception_with_message(
        "\\P{}",
        "u",
        "unicode property escape value cannot be empty",
    );
}

// port: RegExpTreeTest#testValidEs2018RegexNamedCaptureGroups
#[test]
fn test_valid_es2018_regex_named_capture_groups() {
    assert_regex_compiles_to_same("(?<name>)", "");
    assert_regex_compiles_to_same("(?<h$h1h_>)", "u");
    assert_regex_compiles_to_same("(?<$var_name>blah)", "");
    assert_regex_compiles_to_same("(?<_var_name>>>>)", "");
}

// port: RegExpTreeTest#testInvalidEs2018RegexNamedCaptureGroups
#[test]
fn test_invalid_es2018_regex_named_capture_groups() {
    assert_regex_throws_exception_with_message(
        "(?<name)",
        "",
        "Invalid capture group name: <name)",
    );
    assert_regex_throws_exception_with_message("(?<1b>)", "", "Invalid capture group name: <1b>)");
    assert_regex_throws_exception_with_message("(?<>)", "", "Invalid capture group name: <>)");
    assert_regex_throws_exception_with_message(
        "(?<.name>)",
        "",
        "Invalid capture group name: <.name>)",
    );
}

// port: RegExpTreeTest#testNumCapturingGroups
#[test]
fn test_num_capturing_groups() {
    assert_regex_compiles_to_same("(h(i))\\2", "");
    // TODO(b/116048051): reference to non-existent capture group should be an error.
    assert_regex_compiles_to("(h(i))\\3", "", "(h(i))\\x03");

    assert_regex_compiles_to_same("(?<foo>.*(?<bar>))", "");
}

// port: RegExpTreeTest#testValidEs2018CaptureNameBackreferencing
#[test]
fn test_valid_es2018_capture_name_backreferencing() {
    assert_regex_compiles_to_same("(?<name>)\\k<name>", "");
    // Note that (?: ) only used for printing purposes to indicate the nesting structure of
    // Concatenation nodes. It is not actually what the compiler prints out as source code.
    assert_regex_compiles_to(
        "(?<foo>(?<bar>))\\k<foo>\\k<bar>",
        "",
        "(?:(?<foo>(?<bar>))\\k<foo>)\\k<bar>",
    );
    assert_regex_compiles_to(
        "(?<foo>(?<bar>)\\k<bar>)\\k<foo>",
        "",
        "(?<foo>(?<bar>)\\k<bar>)\\k<foo>",
    );

    // The below examples where the backreference comes before the definition of named groups is
    // allowed syntactically, although it is not able to reference the original group
    // semantically
    assert_regex_compiles_to_same("\\k<foo>(?<foo>)", "");
    assert_regex_compiles_to_same("\\k<foo>(?<foo>\\k<bar>(?<bar>))", "");

    // Backreferencing the name in the group it is defined is also allowed
    assert_regex_compiles_to_same("(?<foo>\\k<foo>)", "");
}

// port: RegExpTreeTest#testInvalidEs2018CaptureNameBackreferencing
#[test]
fn test_invalid_es2018_capture_name_backreferencing() {
    assert_regex_throws_exception_with_message(
        "(?<foo>)\\k<bar>",
        "",
        "Invalid named capture referenced: \\k<bar>",
    );
    assert_regex_throws_exception_with_message(
        "(?<foo>)\\k<foo",
        "",
        "Malformed named capture group: <foo",
    );
    assert_regex_throws_exception_with_message(
        "\\k<1b>(?<foo>)",
        "",
        "Invalid capture group name: <1b>(?<foo>)",
    );

    // Even though enclosed in (?<>), 'foo' not a capture name definition
    assert_regex_throws_exception_with_message(
        "[(?<foo>)](?<bar>)\\k<foo>",
        "",
        "Invalid named capture referenced: \\k<foo>",
    );
}

// port: RegExpTreeTest#testBackreferencingTreatedAsStringIfNoGroup
#[test]
fn test_backreferencing_treated_as_string_if_no_group() {
    // Backreferencing without named group definitions is just treated as normal string
    assert_regex_compiles_to("\\k<foo>", "", "k<foo>");

    // Note that the (?: ) that is wrapped around "k<" at expected output is only used for
    // printing purposes to indicate the nesting structure of Concatenation nodes.
    // It is not actually what the compiler prints out as source code.
    assert_regex_compiles_to("\\k<.", "", "(?:k<).");

    // Even though enclosed in (?<>), 'foo' not a capture name definition
    // (?: ) in expected output serves same purpose as above test
    assert_regex_compiles_to("[(?<foo>)]\\k<foo>", "", "(?:[()<>?fo]k)<foo>");
}

// port: RegExpTreeTest#testValidUnicodeEscape
#[test]
fn test_valid_unicode_escape() {
    assert_regex_compiles_to("\\u0061", "", "a");
    assert_regex_compiles_to("\\u10b1", "u", "\\u10b1");
    assert_regex_compiles_to("\\u{61}", "u", "a");
    assert_regex_compiles_to("\\u{10b1}", "u", "\\u10b1");
    assert_regex_compiles_to("\\u{1bc}", "u", "\\u01bc");
    assert_regex_compiles_to("\\u{100A3}", "u", "\\ud800\\udca3");
}

// port: RegExpTreeTest#testInvalidUnicodeEscape
#[test]
fn test_invalid_unicode_escape() {
    assert_regex_throws_exception_with_message(
        "\\u{a012",
        "u",
        "Malformed unicode escape: expected '}' after {a012",
    );
    assert_regex_throws_exception_with_message("\\u{}", "u", "Empty unicode escape");
    assert_regex_throws_exception_with_message("\\u{10za}", "u", "za}");
    assert_regex_throws_exception_with_message(
        "\\u{FFFFFF}",
        "u",
        "Unicode must be at most 0x10FFFF: FFFFFF",
    );
    assert_regex_throws_exception_with_message(
        "\\u{FF00FFFF}",
        "u",
        "Cannot parse hexadecimal encoding wider than 28 bits: FF00FFFF",
    );
}

// port: RegExpTreeTest#testDecomposedCharsetEqualsAndHashCode
#[test]
fn test_decomposed_charset_equals_and_hash_code() {
    let named = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    let empty1 = DecomposedCharset::new(false, char_ranges::EMPTY.clone(), named(""));
    let empty2 = DecomposedCharset::new(false, char_ranges::EMPTY.clone(), named(""));
    let inverted_empty = DecomposedCharset::new(true, char_ranges::EMPTY.clone(), named(""));

    let ranges_a = CharRanges::with_members(&['a' as i32, 'b' as i32]);
    let ranges_b = CharRanges::with_members(&['a' as i32, 'c' as i32]);
    let charset_a1 = DecomposedCharset::new(false, ranges_a.clone(), named("\\d"));
    let charset_a2 = DecomposedCharset::new(false, ranges_a.clone(), named("\\d"));
    let charset_a_inverted = DecomposedCharset::new(true, ranges_a.clone(), named("\\d"));
    let charset_diff_ranges = DecomposedCharset::new(false, ranges_b, named("\\d"));
    let charset_diff_groups = DecomposedCharset::new(false, ranges_a.clone(), named("\\s"));

    // Symmetric equality
    assert!(empty1.equals(&empty2));
    assert!(empty2.equals(&empty1));
    assert_eq!(empty1.hash_code(), empty2.hash_code());

    assert!(charset_a1.equals(&charset_a2));
    assert!(charset_a2.equals(&charset_a1));
    assert_eq!(charset_a1.hash_code(), charset_a2.hash_code());

    // Inverted true equality
    let charset_a_inverted2 = DecomposedCharset::new(true, ranges_a, named("\\d"));
    assert!(charset_a_inverted.equals(&charset_a_inverted2));
    assert_eq!(
        charset_a_inverted.hash_code(),
        charset_a_inverted2.hash_code()
    );

    // Inequality across inverted, ranges, and namedGroups
    assert!(!empty1.equals(&inverted_empty));
    assert!(!inverted_empty.equals(&empty1));
    assert!(!charset_a1.equals(&charset_a_inverted));
    assert!(!charset_a_inverted.equals(&charset_a1));
    assert!(!charset_a1.equals(&charset_diff_ranges));
    assert!(!charset_a1.equals(&charset_diff_groups));
    // assertThat(charsetA1).isNotNull(): nothing to check in Rust.

    // The JDK's record hashCode of these values (JDK 21, the reference JVM).
    assert_eq!(empty1.hash_code(), 1188757);
    assert_eq!(inverted_empty.hash_code(), 1182991);
    assert_eq!(charset_a1.hash_code(), 1206806);
    assert_eq!(charset_a_inverted.hash_code(), 1201040);
    assert_eq!(charset_diff_ranges.hash_code(), 1448141);
    assert_eq!(charset_diff_groups.hash_code(), 1206821);
}
