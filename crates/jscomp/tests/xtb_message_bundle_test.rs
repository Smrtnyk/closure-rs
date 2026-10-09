/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/XtbMessageBundleTest.java.

//! Port of `com.google.javascript.jscomp.XtbMessageBundleTest`.

use std::panic::{AssertUnwindSafe, catch_unwind};

use closure_jscomp::js_message::GrammaticalGenderCase;
use closure_jscomp::message_bundle::MessageBundle;
use closure_jscomp::xtb_message_bundle::XtbMessageBundle;
use closure_rhino::js_string::JsString;

const PROJECT_ID: &str = "TestProject";

fn s(value: &str) -> JsString {
    JsString::from(value)
}

fn new_bundle(xtb: &str) -> XtbMessageBundle {
    XtbMessageBundle::new(xtb.as_bytes(), Some(PROJECT_ID)).unwrap()
}

fn panic_message(f: impl FnOnce()) -> String {
    let error = catch_unwind(AssertUnwindSafe(f)).expect_err("expected an exception");
    if let Some(message) = error.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = error.downcast_ref::<&str>() {
        message.to_string()
    } else {
        String::new()
    }
}

fn assert_throws(f: impl FnOnce()) {
    assert!(catch_unwind(AssertUnwindSafe(f)).is_err());
}

// port: XtbMessageBundleTest#testXtbBundle
#[test]
fn test_xtb_bundle() {
    let xtb = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n",
        "<translationbundle lang=\"zh-HK\">\n",
        "<translation id=\"7639678437384034548\">descargar</translation>\n",
        "<translation id=\"2398375912250604550\">Se han\n",
        "ignorado <ph name=\"NUM\"/> conversaciones.</translation>\n",
        "<translation id=\"6323937743550839320\"><ph name=\"P_START\"/>Si,",
        " puede <ph name=\"LINK_START_1_3\"/>hacer",
        " clic<ph name=\"LINK_END_1_3\"/>",
        " para utilizar.<ph name=\"P_END\"/><ph name=\"P_START\"/>Esperamos",
        " poder ampliar.<ph name=\"P_END\"/></translation>\n",
        "<translation id=\"3945720239421293834\"></translation>\n",
        "</translationbundle>",
    );
    let bundle = new_bundle(xtb);

    let message = bundle.get_message(&s("7639678437384034548")).unwrap();
    assert_eq!(message.as_js_message_string(), s("descargar"));

    let message = bundle.get_message(&s("2398375912250604550")).unwrap();
    assert_eq!(
        message.as_js_message_string(),
        s("Se han\nignorado {$num} conversaciones.")
    );

    let message = bundle.get_message(&s("6323937743550839320")).unwrap();
    assert_eq!(
        message.as_js_message_string(),
        s(concat!(
            "{$pStart}Si, puede {$linkStart_1_3}hacer ",
            "clic{$linkEnd_1_3} para utilizar.{$pEnd}{$pStart}Esperamos ",
            "poder ampliar.{$pEnd}",
        ))
    );

    let message = bundle.get_message(&s("3945720239421293834")).unwrap();
    assert!(message.as_js_message_string().is_empty());
    assert!(!message.get_parts().is_empty());
}

// port: XtbMessageBundleTest#testXtbBundle_genderedMessageVariantsWithoutPlaceholders
#[test]
fn test_xtb_bundle_gendered_message_variants_without_placeholders() {
    let xtb = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n",
        "<translationbundle lang=\"es_ES\">\n",
        "<translation id=\"7639678437384034548\">\n",
        "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
        "MASCULINE } }\">Bienvenido!</branch>",
        "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
        "FEMININE } }\">Bienvenida!</branch>",
        "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
        "NEUTER } }\">Te damos la bienvenida!</branch>",
        "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
        "OTHER } }\">Te damos la bienvenida! - OTHER</branch>",
        "</translation>\n",
        "</translationbundle>\n",
    );
    let bundle = new_bundle(xtb);

    let message = bundle.get_message(&s("7639678437384034548")).unwrap();
    // Testing invalid calls to asJsMessageString() as it should call
    // asJsMessageString(GrammaticalGenderCase)
    assert_throws(|| {
        message.as_js_message_string();
    });

    assert_eq!(
        message.as_js_message_string_for_gender(GrammaticalGenderCase::MASCULINE),
        s("Bienvenido!")
    );
    assert_eq!(
        message.as_js_message_string_for_gender(GrammaticalGenderCase::FEMININE),
        s("Bienvenida!")
    );
    assert_eq!(
        message.as_js_message_string_for_gender(GrammaticalGenderCase::NEUTER),
        s("Te damos la bienvenida!")
    );
    assert_eq!(
        message.as_js_message_string_for_gender(GrammaticalGenderCase::OTHER),
        s("Te damos la bienvenida! - OTHER")
    );

    assert_eq!(
        message
            .get_gendered_message_parts(GrammaticalGenderCase::MASCULINE)
            .len(),
        1
    );
    assert_eq!(
        message.get_gendered_message_parts(GrammaticalGenderCase::MASCULINE)[0].get_string(),
        s("Bienvenido!")
    );
    assert_eq!(
        message.get_gendered_message_parts(GrammaticalGenderCase::FEMININE)[0].get_string(),
        s("Bienvenida!")
    );
    assert_eq!(
        message.get_gendered_message_parts(GrammaticalGenderCase::NEUTER)[0].get_string(),
        s("Te damos la bienvenida!")
    );
    assert_eq!(
        message.get_gendered_message_parts(GrammaticalGenderCase::OTHER)[0].get_string(),
        s("Te damos la bienvenida! - OTHER")
    );
}

const GENDERED_WITH_PLACEHOLDERS: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n",
    "<translationbundle lang=\"es_ES\">\n",
    "<translation id=\"7639678437384034548\">\n",
    "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
    "MASCULINE } }\">Bienvenido, <ph name=\"NAME\"/></branch>",
    "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
    "FEMININE } }\">Bienvenida, <ph name=\"NAME\"/></branch>",
    "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
    "NEUTER } }\">Te damos la bienvenida, <ph name=\"NAME\"/></branch>",
    "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
    "OTHER } }\">Te damos la bienvenida - OTHER, <ph name=\"NAME\"/></branch>",
    "</translation>\n\n",
    "</translationbundle>\n",
);

// port: XtbMessageBundleTest#testXtbBundle_genderedMessageVariantsWithPlaceholders
#[test]
fn test_xtb_bundle_gendered_message_variants_with_placeholders() {
    let bundle = new_bundle(GENDERED_WITH_PLACEHOLDERS);

    let message = bundle.get_message(&s("7639678437384034548")).unwrap();
    // Testing invalid calls to asJsMessageString() as it should call
    // asJsMessageString(GrammaticalGenderCase)
    assert_throws(|| {
        message.as_js_message_string();
    });
    // Testing calls to asJsMessageString(GrammaticalGenderCase)
    let message_text = panic_message(|| {
        message.as_js_message_string_for_gender(GrammaticalGenderCase::value_of(Some("FOO")));
    });
    assert!(message_text.starts_with("No enum constant"));

    // Testing valid calls to asJsMessageString()
    assert_eq!(
        message.as_js_message_string_for_gender(GrammaticalGenderCase::MASCULINE),
        s("Bienvenido, {$name}")
    );
    assert_eq!(
        message.as_js_message_string_for_gender(GrammaticalGenderCase::FEMININE),
        s("Bienvenida, {$name}")
    );
    assert_eq!(
        message.as_js_message_string_for_gender(GrammaticalGenderCase::NEUTER),
        s("Te damos la bienvenida, {$name}")
    );
    assert_eq!(
        message.as_js_message_string_for_gender(GrammaticalGenderCase::OTHER),
        s("Te damos la bienvenida - OTHER, {$name}")
    );

    let expected = [
        (GrammaticalGenderCase::MASCULINE, "Bienvenido, "),
        (GrammaticalGenderCase::FEMININE, "Bienvenida, "),
        (GrammaticalGenderCase::NEUTER, "Te damos la bienvenida, "),
        (
            GrammaticalGenderCase::OTHER,
            "Te damos la bienvenida - OTHER, ",
        ),
    ];
    for (gender, text) in expected {
        let parts = message.get_gendered_message_parts(gender);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].get_string(), s(text));
        assert_eq!(parts[1].get_js_placeholder_name(), s("name"));
    }
}

// port: XtbMessageBundleTest#testXtbBundleWithIcu_genderedMessageVariantsWithPlaceholders
#[test]
fn test_xtb_bundle_with_icu_gendered_message_variants_with_placeholders() {
    let bundle = new_bundle(GENDERED_WITH_PLACEHOLDERS);

    let message = bundle.get_message(&s("7639678437384034548")).unwrap();

    assert!(!message.is_empty());
    assert_throws(|| {
        message.as_icu_message_string();
    });

    assert_eq!(
        message.as_icu_message_string_for_gender(GrammaticalGenderCase::MASCULINE),
        s("Bienvenido, {NAME}")
    );
    assert_eq!(
        message.as_icu_message_string_for_gender(GrammaticalGenderCase::FEMININE),
        s("Bienvenida, {NAME}")
    );
    assert_eq!(
        message.as_icu_message_string_for_gender(GrammaticalGenderCase::NEUTER),
        s("Te damos la bienvenida, {NAME}")
    );
    assert_eq!(
        message.as_icu_message_string_for_gender(GrammaticalGenderCase::OTHER),
        s("Te damos la bienvenida - OTHER, {NAME}")
    );

    let expected = [
        (GrammaticalGenderCase::MASCULINE, "Bienvenido, "),
        (GrammaticalGenderCase::FEMININE, "Bienvenida, "),
        (GrammaticalGenderCase::NEUTER, "Te damos la bienvenida, "),
        (
            GrammaticalGenderCase::OTHER,
            "Te damos la bienvenida - OTHER, ",
        ),
    ];
    for (gender, text) in expected {
        let parts = message.get_gendered_message_parts(gender);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].get_string(), s(text));
        assert_eq!(parts[1].get_canonical_placeholder_name(), s("NAME"));
    }
}

// port: XtbMessageBundleTest#testXtbBundle_genderedMessageVariantsWithInvalidGenderCase_throwsException
#[test]
fn test_xtb_bundle_gendered_message_variants_with_invalid_gender_case_throws_exception() {
    let xtb = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n",
        "<translationbundle lang=\"es_ES\">\n",
        "<translation id=\"7639678437384034548\">\n",
        "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
        "  MASCULINE } }\">Bienvenido!</branch>",
        "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
        "  FEMININE } }\">Bienvenida!</branch>",
        "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
        "  FOO } }\">Foo bar!</branch>",
        "<branch variants=\"variants { grammatical_gender_variant { grammatical_gender_case:\n",
        "  OTHER } }\">Te damos la bienvenida! - OTHER</branch>",
        "</translation>\n\n",
        "</translationbundle>\n",
    );

    let e = panic_message(|| {
        let _ = XtbMessageBundle::new(xtb.as_bytes(), Some(PROJECT_ID));
    });
    assert!(
        e.contains(
            "Gender case must be one of the following: MASCULINE, FEMININE, NEUTER, or OTHER."
        ),
        "{e}"
    );
}

/// When using EXTERNAL messages with plurals/selects, the XTB files may contain a mix of ICU
/// style placeholders (i.e. `{foo}`) and regular placeholders (i.e. `<ph name="foo"/>`).
/// However, JsMessage and the Closure Library runtime don't expect to see regular placeholders,
/// so they must be rewritten.
// port: XtbMessageBundleTest#testXtbBundle_mixedPlaceholders
#[test]
fn test_xtb_bundle_mixed_placeholders() {
    let xtb_with_mixed_placeholders = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n",
        "<translationbundle lang=\"ru_RU\">\n",
        "<translation id=\"123456\">",
        "{USER_GENDER,select,",
        "female{Hello <ph name=\"USER_IDENTIFIER\"/>.}",
        "male{Hello <ph name=\"USER_IDENTIFIER\"/>.}",
        "other{Hello <ph name=\"USER_IDENTIFIER\"/>.}}",
        "</translation>\n",
        "<translation id=\"123457\">",
        "<ph name=\"START_PARAGRAPH\"/>p1<ph name=\"END_PARAGRAPH\"/>",
        "<ph name=\"START_PARAGRAPH\"/>p1<ph name=\"END_PARAGRAPH\"/>",
        "</translation>",
        "</translationbundle>",
    );
    let bundle = new_bundle(xtb_with_mixed_placeholders);

    assert_eq!(bundle.get_all_messages().len(), 2);
    let icu_msg = bundle.get_message(&s("123456")).unwrap();
    assert_eq!(
        icu_msg.as_icu_message_string(),
        s(concat!(
            "{USER_GENDER,select,",
            "female{Hello {USER_IDENTIFIER}.}",
            "male{Hello {USER_IDENTIFIER}.}",
            "other{Hello {USER_IDENTIFIER}.}}",
        ))
    );
    // For an ICU selector formatted message, XtbMessageBundle automatically converts all the
    // placeholders into normal strings.
    let icu_msg_parts = icu_msg.get_parts();
    assert_eq!(icu_msg_parts.len(), 7);
    assert_eq!(
        icu_msg_parts[0].get_string(),
        s("{USER_GENDER,select,female{Hello ")
    );
    assert_eq!(
        icu_msg_parts[1].get_canonical_placeholder_name(),
        s("USER_IDENTIFIER")
    );
    assert_eq!(icu_msg_parts[2].get_string(), s(".}male{Hello "));
    assert_eq!(
        icu_msg_parts[3].get_canonical_placeholder_name(),
        s("USER_IDENTIFIER")
    );
    assert_eq!(icu_msg_parts[4].get_string(), s(".}other{Hello "));
    assert_eq!(
        icu_msg_parts[5].get_canonical_placeholder_name(),
        s("USER_IDENTIFIER")
    );
    assert_eq!(icu_msg_parts[6].get_string(), s(".}}"));

    // Previous ICU message should not to affect next message
    let normal_msg = bundle.get_message(&s("123457")).unwrap();
    assert_eq!(
        normal_msg.as_js_message_string(),
        s("{$startParagraph}p1{$endParagraph}{$startParagraph}p1{$endParagraph}")
    );
    let normal_msg_parts = normal_msg.get_parts();
    // For a normal message the placeholders are not turned into strings
    assert_eq!(normal_msg_parts.len(), 6);
    assert_eq!(
        normal_msg_parts[0].get_js_placeholder_name(),
        s("startParagraph")
    );
    assert_eq!(normal_msg_parts[1].get_string(), s("p1"));
    assert_eq!(
        normal_msg_parts[2].get_js_placeholder_name(),
        s("endParagraph")
    );
    assert_eq!(
        normal_msg_parts[3].get_js_placeholder_name(),
        s("startParagraph")
    );
    assert_eq!(normal_msg_parts[4].get_string(), s("p1"));
    assert_eq!(
        normal_msg_parts[5].get_js_placeholder_name(),
        s("endParagraph")
    );
}

/// An ICU message using `phex` to describe its variables should result in the same JavaScript
/// code as the same ICU message without those `phex` attributes.
// port: XtbMessageBundleTest#testXtbBundle_icuPluralWithAndWithoutPhex
#[test]
fn test_xtb_bundle_icu_plural_with_and_without_phex() {
    let xtb = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE translationbundle SYSTEM \"translationbundle.dtd\">\n",
        "<translationbundle lang=\"de_CH\">\n",
        // With phex
        "<translation id=\"123456\">",
        "{NUM,plural, ",
        "=1{Setting: {START_STRONG}<ph name=\"UDC_SETTING\"/>{END_STRONG}",
        " Products: {START_STRONG}<ph name=\"PRODUCT_LIST\"/>{END_STRONG}.}",
        "other{Settings: {START_STRONG}<ph name=\"UDC_SETTING_LIST\"/>{END_STRONG}",
        " Products: {START_STRONG}<ph name=\"PRODUCT_LIST\"/>{END_STRONG}.}",
        "}",
        "</translation>\n",
        // Without phex
        "<translation id=\"987654\">",
        "{NUM,plural, ",
        "=1{Setting: {START_STRONG}{UDC_SETTING}{END_STRONG}",
        " Products: {START_STRONG}{PRODUCT_LIST}{END_STRONG}.}",
        "other{Settings: {START_STRONG}{UDC_SETTING_LIST}{END_STRONG}",
        " Products: {START_STRONG}{PRODUCT_LIST}{END_STRONG}.}",
        "}",
        "</translation>\n",
        "</translationbundle>",
    );
    let bundle = new_bundle(xtb);

    assert_eq!(bundle.get_all_messages().len(), 2);
    assert_eq!(
        bundle
            .get_message(&s("123456"))
            .unwrap()
            .as_icu_message_string(),
        s(concat!(
            "{NUM,plural, ",
            "=1{Setting: {START_STRONG}{UDC_SETTING}{END_STRONG}",
            " Products: {START_STRONG}{PRODUCT_LIST}{END_STRONG}.}",
            "other{Settings: {START_STRONG}{UDC_SETTING_LIST}{END_STRONG}",
            " Products: {START_STRONG}{PRODUCT_LIST}{END_STRONG}.}}",
        ))
    );
    // Both translation entries should result into the same message in JavaScript.
    assert_eq!(
        bundle
            .get_message(&s("987654"))
            .unwrap()
            .as_icu_message_string(),
        bundle
            .get_message(&s("123456"))
            .unwrap()
            .as_icu_message_string()
    );
}
