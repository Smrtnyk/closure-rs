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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/JsMessageTest.java.

//! Port of `com.google.javascript.jscomp.JsMessageTest`.

use std::panic::{AssertUnwindSafe, catch_unwind};

use closure_jscomp::js_message::{self, Builder, GrammaticalGenderCase, Hash, JsMessage};
use closure_rhino::js_string::JsString;

fn s(value: &str) -> JsString {
    JsString::from(value)
}

/// Return a new message builder that uses the given string as both its key and its ID.
// port: JsMessageTest#newTestMessageBuilder
fn new_test_message_builder(key_and_id: &str) -> Builder {
    let mut builder = Builder::new();
    builder.set_key(key_and_id).set_id(key_and_id);
    builder
}

fn assert_throws(f: impl FnOnce()) {
    assert!(catch_unwind(AssertUnwindSafe(f)).is_err());
}

// port: JsMessageTest#testIsLowerCamelCaseWithNumericSuffixes
#[test]
fn test_is_lower_camel_case_with_numeric_suffixes() {
    assert!(js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        "name"
    )));
    assert!(!js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        "NAME"
    )));
    assert!(!js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        "Name"
    )));
    assert!(js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        "a4Letter"
    )));
    assert!(!js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        "A4_LETTER"
    )));
    assert!(js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        "startSpan_1_23"
    )));
    assert!(!js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        "startSpan_1_23b"
    )));
    assert!(!js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        "START_SPAN_1_23"
    )));
    assert!(!js_message::is_lower_camel_case_with_numeric_suffixes(&s(
        ""
    )));
}

// port: JsMessageTest#testToLowerCamelCaseWithNumericSuffixes
#[test]
fn test_to_lower_camel_case_with_numeric_suffixes() {
    assert_eq!(
        js_message::to_lower_camel_case_with_numeric_suffixes(&s("NAME")),
        "name"
    );
    assert_eq!(
        js_message::to_lower_camel_case_with_numeric_suffixes(&s("A4_LETTER")),
        "a4Letter"
    );
    assert_eq!(
        js_message::to_lower_camel_case_with_numeric_suffixes(&s("START_SPAN_1_23")),
        "startSpan_1_23"
    );
}

// port: JsMessageTest#testIsEmpty
#[test]
fn test_is_empty() {
    assert!(new_test_message_builder("MSG_KEY").build().is_empty());
    assert!(
        new_test_message_builder("MSG_KEY")
            .append_string_part("")
            .build()
            .is_empty()
    );
    assert!(
        new_test_message_builder("MSG_KEY")
            .append_string_part("")
            .append_string_part("")
            .build()
            .is_empty()
    );
    assert!(
        !new_test_message_builder("MSG_KEY")
            .append_string_part("s")
            .append_string_part("")
            .build()
            .is_empty()
    );
    assert!(
        !new_test_message_builder("MSG_KEY")
            .append_js_placeholder_reference("ph")
            .build()
            .is_empty()
    );
}

// port: JsMessageTest#testHashValues
#[test]
fn test_hash_values() {
    let answer_string64: i64 = 0x43ec5d9731515874;
    assert_eq!(Hash::hash64(Some(&s("Hello, world"))), answer_string64);

    let answer_empty64: i64 = 0x468d9ea2c42361aa;
    assert_eq!(Hash::hash64(Some(&s(""))), answer_empty64);
}

// port: JsMessageTest#testNoAlternateId
#[test]
fn test_no_alternate_id() {
    let msg: JsMessage = new_test_message_builder("MSG_SOME_KEY")
        .set_desc(Some(s("Hello.")))
        .build();
    assert_eq!(msg.get_desc(), Some(&s("Hello.")));
    assert_eq!(msg.get_id(), &s("MSG_SOME_KEY"));
    assert!(msg.get_alternate_id().is_none());
}

// port: JsMessageTest#testSelfReferentialAlternateId
#[test]
fn test_self_referential_alternate_id() {
    let msg = new_test_message_builder("MSG_SOME_NAME")
        .set_desc(Some(s("Hello.")))
        .set_alternate_id(Some(s("MSG_SOME_NAME")))
        .build();
    assert_eq!(msg.get_desc(), Some(&s("Hello.")));
    assert_eq!(msg.get_id(), &s("MSG_SOME_NAME"));
    assert!(msg.get_alternate_id().is_none());
}

// port: JsMessageTest#testAlternateId
#[test]
fn test_alternate_id() {
    let msg = Builder::new()
        .set_key("MSG_KEY")
        .set_desc(Some(s("Hello.")))
        .set_alternate_id(Some(s("foo")))
        .set_meaning(Some(s("meaning")))
        .set_id("meaning")
        .append_js_placeholder_reference("placeholder0")
        .append_js_placeholder_reference("placeholder1")
        .append_string_part("part0")
        .append_string_part("part1")
        .append_string_part("part2")
        .build();

    assert_eq!(msg.get_desc(), Some(&s("Hello.")));
    assert_eq!(msg.get_id(), &s("meaning"));
    assert_eq!(msg.get_alternate_id(), Some(&s("foo")));
}

fn welcome_message_with_placeholders() -> JsMessage {
    Builder::new()
        .set_key("MSG_KEY")
        .set_id("1234567890")
        .set_desc(Some(s("A welcome message.")))
        .add_gendered_message_key(GrammaticalGenderCase::MASCULINE)
        .append_string_part_for_gender(GrammaticalGenderCase::MASCULINE, "Beinvenido, ")
        .append_js_placeholder_reference_for_gender(GrammaticalGenderCase::MASCULINE, "placeholder")
        .add_gendered_message_key(GrammaticalGenderCase::FEMININE)
        .append_string_part_for_gender(GrammaticalGenderCase::FEMININE, "Beinvenida, ")
        .append_js_placeholder_reference_for_gender(GrammaticalGenderCase::FEMININE, "placeholder")
        .add_gendered_message_key(GrammaticalGenderCase::NEUTER)
        .append_string_part_for_gender(GrammaticalGenderCase::NEUTER, "Te damos la bienvenida, ")
        .append_js_placeholder_reference_for_gender(GrammaticalGenderCase::NEUTER, "placeholder")
        .add_gendered_message_key(GrammaticalGenderCase::OTHER)
        .append_string_part_for_gender(
            GrammaticalGenderCase::OTHER,
            "Te damos la bienvenida - OTHER, ",
        )
        .append_js_placeholder_reference_for_gender(GrammaticalGenderCase::OTHER, "placeholder")
        .build()
}

// port: JsMessageTest#testMessageWithGenderedVariants
#[test]
fn test_message_with_gendered_variants() {
    let msg = welcome_message_with_placeholders();

    // MASCULINE
    let masculine_parts = msg.get_gendered_message_parts(GrammaticalGenderCase::MASCULINE);
    assert_eq!(masculine_parts.len(), 2);
    assert_eq!(masculine_parts[0].get_string(), "Beinvenido, ");
    assert_eq!(masculine_parts[1].get_js_placeholder_name(), "placeholder");

    // FEMININE
    let feminine_parts = msg.get_gendered_message_parts(GrammaticalGenderCase::FEMININE);
    assert_eq!(feminine_parts.len(), 2);
    assert_eq!(feminine_parts[0].get_string(), "Beinvenida, ");
    assert_eq!(feminine_parts[1].get_js_placeholder_name(), "placeholder");

    // NEUTER
    let neuter_parts = msg.get_gendered_message_parts(GrammaticalGenderCase::NEUTER);
    assert_eq!(neuter_parts.len(), 2);
    assert_eq!(neuter_parts[0].get_string(), "Te damos la bienvenida, ");
    assert_eq!(neuter_parts[1].get_js_placeholder_name(), "placeholder");

    // OTHER
    let other_parts = msg.get_gendered_message_parts(GrammaticalGenderCase::OTHER);
    assert_eq!(other_parts.len(), 2);
    assert_eq!(
        other_parts[0].get_string(),
        "Te damos la bienvenida - OTHER, "
    );
    assert_eq!(other_parts[1].get_js_placeholder_name(), "placeholder");
}

// port: JsMessageTest#testGetGenderedVariants
#[test]
fn test_get_gendered_variants() {
    let msg = Builder::new()
        .set_key("MSG_KEY")
        .set_id("1234567890")
        .set_desc(Some(s("A welcome message.")))
        .add_gendered_message_key(GrammaticalGenderCase::MASCULINE)
        .append_string_part_for_gender(GrammaticalGenderCase::MASCULINE, "Bienvenido, ")
        .add_gendered_message_key(GrammaticalGenderCase::FEMININE)
        .append_string_part_for_gender(GrammaticalGenderCase::FEMININE, "Bienvenida, ")
        .add_gendered_message_key(GrammaticalGenderCase::NEUTER)
        .append_string_part_for_gender(GrammaticalGenderCase::NEUTER, "Te damos la bienvenida, ")
        .add_gendered_message_key(GrammaticalGenderCase::OTHER)
        .append_string_part_for_gender(
            GrammaticalGenderCase::OTHER,
            "Te damos la bienvenida - OTHER, ",
        )
        .build();

    let expected_gendered_variants = vec![
        GrammaticalGenderCase::MASCULINE,
        GrammaticalGenderCase::FEMININE,
        GrammaticalGenderCase::NEUTER,
        GrammaticalGenderCase::OTHER,
    ];
    assert_eq!(
        msg.get_gendered_message_variants(),
        expected_gendered_variants
    );
}

fn welcome_message_without_neuter() -> JsMessage {
    Builder::new()
        .set_key("MSG_KEY")
        .set_id("1234567890")
        .set_desc(Some(s("A welcome message.")))
        .add_gendered_message_key(GrammaticalGenderCase::MASCULINE)
        .append_string_part_for_gender(GrammaticalGenderCase::MASCULINE, "Bienvenido, ")
        .add_gendered_message_key(GrammaticalGenderCase::FEMININE)
        .append_string_part_for_gender(GrammaticalGenderCase::FEMININE, "Bienvenida, ")
        .add_gendered_message_key(GrammaticalGenderCase::OTHER)
        .append_string_part_for_gender(
            GrammaticalGenderCase::OTHER,
            "Te damos la bienvenida - OTHER, ",
        )
        .build()
}

// port: JsMessageTest#testGetGenderedVariants_noNeuter
#[test]
fn test_get_gendered_variants_no_neuter() {
    let msg = welcome_message_without_neuter();

    let expected_gendered_variants = vec![
        GrammaticalGenderCase::MASCULINE,
        GrammaticalGenderCase::FEMININE,
        GrammaticalGenderCase::OTHER,
    ];
    assert_eq!(
        msg.get_gendered_message_variants(),
        expected_gendered_variants
    );
}

// port: JsMessageTest#testGenderedMessageAsString
#[test]
fn test_gendered_message_as_string() {
    let msg = welcome_message_with_placeholders();

    assert_eq!(
        msg.as_js_message_string_for_gender(GrammaticalGenderCase::MASCULINE),
        "Beinvenido, {$placeholder}"
    );
    assert_eq!(
        msg.as_js_message_string_for_gender(GrammaticalGenderCase::FEMININE),
        "Beinvenida, {$placeholder}"
    );
    assert_eq!(
        msg.as_js_message_string_for_gender(GrammaticalGenderCase::NEUTER),
        "Te damos la bienvenida, {$placeholder}"
    );
    assert_eq!(
        msg.as_js_message_string_for_gender(GrammaticalGenderCase::OTHER),
        "Te damos la bienvenida - OTHER, {$placeholder}"
    );

    assert_eq!(
        msg.as_icu_message_string_for_gender(GrammaticalGenderCase::MASCULINE),
        "Beinvenido, {PLACEHOLDER}"
    );
    assert_eq!(
        msg.as_icu_message_string_for_gender(GrammaticalGenderCase::FEMININE),
        "Beinvenida, {PLACEHOLDER}"
    );
    assert_eq!(
        msg.as_icu_message_string_for_gender(GrammaticalGenderCase::NEUTER),
        "Te damos la bienvenida, {PLACEHOLDER}"
    );
    assert_eq!(
        msg.as_icu_message_string_for_gender(GrammaticalGenderCase::OTHER),
        "Te damos la bienvenida - OTHER, {PLACEHOLDER}"
    );
}

// port: JsMessageTest#testGenderedMessageAsString_invalidCall
#[test]
fn test_gendered_message_as_string_invalid_call() {
    let gendered_msg = welcome_message_without_neuter();

    // No grammatical gender argument provided
    assert_throws(|| {
        gendered_msg.as_js_message_string();
    });
    assert_throws(|| {
        gendered_msg.as_icu_message_string();
    });

    // No message for the provided grammatical gender
    assert_throws(|| {
        gendered_msg.as_js_message_string_for_gender(GrammaticalGenderCase::NEUTER);
    });
    assert_throws(|| {
        gendered_msg.as_icu_message_string_for_gender(GrammaticalGenderCase::NEUTER);
    });
}
