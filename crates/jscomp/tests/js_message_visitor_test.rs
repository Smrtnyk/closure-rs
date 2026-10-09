/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   test/com/google/javascript/jscomp/JsMessageVisitorTest.java.

//! Port of `com.google.javascript.jscomp.JsMessageVisitorTest`.

use closure_jscomp::abstract_compiler::AbstractCompiler;
use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_options::CompilerOptions;
use closure_jscomp::diagnostic_type::DiagnosticType;
use closure_jscomp::icu_template_definition::IcuTemplateDefinition;
use closure_jscomp::js_message::JsMessage;
use closure_jscomp::js_message_definition::JsMessageDefinition;
use closure_jscomp::js_message_visitor::{
    self, IcuMessageTemplateString, JsMessageVisitor, JsMessageVisitorBase, MESSAGE_HAS_NO_VALUE,
    MESSAGE_NOT_INITIALIZED_CORRECTLY, MESSAGE_TREE_MALFORMED,
};
use closure_jscomp::node_traversal::{Callback, NodeTraversal};
use closure_jscomp::source_file::SourceFile;
use closure_jscomp::source_map_input::SourceMapInput;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_sourcemap::file_position::FilePosition;
use closure_sourcemap::source_map_generator_v3::SourceMapGeneratorV3;
use std::sync::Arc;

fn s(value: &str) -> JsString {
    JsString::from(value)
}

/// Truth's `assertThat(collection).containsExactly(...)` (any order).
fn assert_contains_exactly<T: ToString>(actual: impl IntoIterator<Item = T>, expected: &[&str]) {
    let mut actual: Vec<String> = actual.into_iter().map(|a| a.to_string()).collect();
    let mut expected: Vec<String> = expected.iter().map(|e| e.to_string()).collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

/// `assertNode(n).isString(value)`.
fn assert_is_string(ast: &Ast, n: NodeId, value: &str) {
    assert!(n.is_string_lit(ast), "{}", n.to_string(ast));
    assert_eq!(n.get_string(ast), value);
}

/// Truth's `assertThat(nullableString).isEqualTo(...)` on a nullable getter.
fn opt(value: Option<impl ToString>) -> Option<String> {
    value.map(|v| v.to_string())
}

// port: JsMessageVisitorTest.RenameMessagesVisitor
struct RenameMessagesVisitor;

impl Callback for RenameMessagesVisitor {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: JsMessageVisitorTest.RenameMessagesVisitor#visit
    #[allow(clippy::if_same_then_else)] // Retain the Java control flow.
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) && n.get_string(t).starts_with(s("MSG_")) {
            let original_name = n.get_string(t);
            n.set_original_name(t, Some(original_name.clone()));
            n.set_string(t, s("some_prefix_").concat(&original_name));
        } else if n.is_get_prop(t)
            && parent.unwrap().is_assign(t)
            && n.get_qualified_name(t).unwrap().index_of(s(".MSG_")) >= 0
        {
            let original_name = n.get_string(t);
            n.set_original_name(t, Some(original_name.clone()));
            n.set_string(t, s("some_prefix_").concat(&original_name));
        }
    }
}

/// The fields of the Java test class (`@Before setUp` is `Fixture::new`).
struct Fixture {
    compiler_options: Option<CompilerOptions>,
    compiler: Compiler,
    messages: Vec<JsMessage>,
    message_definitions: Vec<Box<dyn JsMessageDefinition>>,
    icu_template_definitions: Vec<Box<dyn IcuTemplateDefinition>>,
    rename_messages: bool,
}

impl Fixture {
    // port: JsMessageVisitorTest#setUp
    fn new() -> Self {
        Self {
            compiler_options: None,
            compiler: Compiler::new(),
            messages: Vec::new(),
            message_definitions: Vec::new(),
            icu_template_definitions: Vec::new(),
            rename_messages: false,
        }
    }

    // port: JsMessageVisitorTest#assertOneMessage
    fn assert_one_message(&self) -> JsMessage {
        assert_eq!(self.messages.len(), 1);
        self.messages[0].clone()
    }

    // port: JsMessageVisitorTest#assertOneError(DiagnosticType,String)
    fn assert_one_error(&self, type_: &'static DiagnosticType, description: &str) {
        self.assert_one_error_type(type_);
        self.assert_one_error_description(description);
    }

    // port: JsMessageVisitorTest#assertOneError(DiagnosticType)
    fn assert_one_error_type(&self, type_: &'static DiagnosticType) {
        let errors = self.compiler.get_errors();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].get_type(), type_);
    }

    // port: JsMessageVisitorTest#assertOneError(String)
    fn assert_one_error_description(&self, description: &str) {
        let errors = self.compiler.get_errors();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].get_description(), description);
    }

    // port: JsMessageVisitorTest#assertOneWarning(DiagnosticType,String)
    fn assert_one_warning(&self, type_: &'static DiagnosticType, description: &str) {
        self.assert_one_warning_type(type_);
        self.assert_one_warning_description(description);
    }

    // port: JsMessageVisitorTest#assertOneWarning(DiagnosticType)
    fn assert_one_warning_type(&self, type_: &'static DiagnosticType) {
        let warnings = self.compiler.get_warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].get_type(), type_);
    }

    // port: JsMessageVisitorTest#assertOneWarning(String)
    fn assert_one_warning_description(&self, description: &str) {
        let warnings = self.compiler.get_warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].get_description(), description);
    }

    // port: JsMessageVisitorTest#extractMessagesSafely
    fn extract_messages_safely(&mut self, input: &str) {
        self.extract_messages(input);
        assert!(
            self.compiler.get_warnings().is_empty(),
            "{:?}",
            self.compiler.get_warnings()
        );
        assert!(
            self.compiler.get_errors().is_empty(),
            "{:?}",
            self.compiler.get_errors()
        );
    }

    // port: JsMessageVisitorTest#extractMessages
    fn extract_messages(&mut self, input: &str) {
        self.compiler = Compiler::new();
        if let Some(compiler_options) = &self.compiler_options {
            self.compiler.init_options(compiler_options.clone());
        }
        let root = self.compiler.parse_test_code(input);
        let mut visitor = CollectMessages::new();
        if self.rename_messages {
            let mut rename_messages_visitor = RenameMessagesVisitor;
            NodeTraversal::traverse(&mut self.compiler, root, &mut rename_messages_visitor);
        }
        js_message_visitor::process(&mut visitor, &mut self.compiler, None, root);
        self.messages.extend(visitor.messages);
        self.message_definitions.extend(visitor.message_definitions);
        self.icu_template_definitions
            .extend(visitor.icu_template_definitions);
    }
}

/// `private class CollectMessages extends JsMessageVisitor`: collects into its own lists, which
/// `extract_messages` appends to the fixture's (Java's inner class appends to the outer lists).
struct CollectMessages {
    base: JsMessageVisitorBase,
    messages: Vec<JsMessage>,
    message_definitions: Vec<Box<dyn JsMessageDefinition>>,
    icu_template_definitions: Vec<Box<dyn IcuTemplateDefinition>>,
}

impl CollectMessages {
    // port: JsMessageVisitorTest.CollectMessages#CollectMessages
    fn new() -> Self {
        Self {
            base: JsMessageVisitorBase::new(None),
            messages: Vec::new(),
            message_definitions: Vec::new(),
            icu_template_definitions: Vec::new(),
        }
    }
}

impl JsMessageVisitor for CollectMessages {
    fn js_message_visitor_base(&mut self) -> &mut JsMessageVisitorBase {
        &mut self.base
    }

    // port: JsMessageVisitorTest.CollectMessages#processJsMessageDefinition
    fn process_js_message_definition(
        &mut self,
        _compiler: &mut AbstractCompiler,
        definition: Box<dyn JsMessageDefinition>,
    ) {
        self.messages.push(definition.get_message().clone());
        self.message_definitions.push(definition);
    }

    // port: JsMessageVisitorTest.CollectMessages#processIcuTemplateDefinition
    fn process_icu_template_definition(
        &mut self,
        _compiler: &mut AbstractCompiler,
        definition: Box<dyn IcuTemplateDefinition>,
    ) {
        self.messages.push(definition.get_message().clone());
        self.icu_template_definitions.push(definition);
    }
}

// port: JsMessageVisitorTest.DummyJsVisitor
struct DummyJsVisitor {
    base: JsMessageVisitorBase,
}

impl DummyJsVisitor {
    // port: JsMessageVisitorTest.DummyJsVisitor#DummyJsVisitor
    fn new() -> Self {
        Self {
            base: JsMessageVisitorBase::new(None),
        }
    }
}

impl JsMessageVisitor for DummyJsVisitor {
    fn js_message_visitor_base(&mut self) -> &mut JsMessageVisitorBase {
        &mut self.base
    }

    // port: JsMessageVisitorTest.DummyJsVisitor#processJsMessageDefinition
    fn process_js_message_definition(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _definition: Box<dyn JsMessageDefinition>,
    ) {
        // no-op
    }

    // port: JsMessageVisitorTest.DummyJsVisitor#processIcuTemplateDefinition
    fn process_icu_template_definition(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _definition: Box<dyn IcuTemplateDefinition>,
    ) {
        // no-op
    }
}

// port: JsMessageVisitorTest#testIcuTemplateParsing
#[test]
fn test_icu_template_parsing() {
    // Some placeholders are ignored, because there's no additional information for them
    // (original code or example text). There's no need to generate placeholder parts for
    // those.
    let icu_message_template_string = IcuMessageTemplateString::new(concat!(
        "{NUM_PEOPLE, plural, offset:1\n",
        "=0 {I see {START_BOLD}no one at all{END_BOLD} in {INTERPOLATION_2}.}\n",
        "=1 {I see {INTERPOLATION_1} in {INTERPOLATION_2}.}\n",
        "=2 {I see {INTERPOLATION_1} and one other person in {INTERPOLATION_2}.}\n",
        "other {I see {INTERPOLATION_1} and # other people in {INTERPOLATION_2}.}\n",
        "}\n"
    ));
    let extracted_parts = icu_message_template_string.extract_parts(&IndexSet::<_>::from_iter([
        s("INTERPOLATION_1"),
        s("INTERPOLATION_2"),
        s("MISSING"),
    ]));

    // The non-existent placeholder name "MISSING" should be missing from this list.
    // JsMessageVisitor will notice the name is missing from the resulting set of
    // seen placeholders and report an error, though we won't do that in this test.
    assert_contains_exactly(
        &extracted_parts.extracted_placeholder_names,
        &["INTERPOLATION_1", "INTERPOLATION_2"],
    );
    let parts = &extracted_parts.extracted_parts;
    assert_eq!(
        parts[0].get_string(),
        concat!(
            "{NUM_PEOPLE, plural, offset:1\n",
            "=0 {I see {START_BOLD}no one at all{END_BOLD} in "
        )
    );
    assert_eq!(parts[1].get_canonical_placeholder_name(), "INTERPOLATION_2");
    assert_eq!(parts[2].get_string(), concat!(".}\n", "=1 {I see "));
    assert_eq!(parts[3].get_canonical_placeholder_name(), "INTERPOLATION_1");
    assert_eq!(parts[4].get_string(), " in ");
    assert_eq!(parts[5].get_canonical_placeholder_name(), "INTERPOLATION_2");
    assert_eq!(parts[6].get_string(), concat!(".}\n", "=2 {I see "));
    assert_eq!(parts[7].get_canonical_placeholder_name(), "INTERPOLATION_1");
    assert_eq!(parts[8].get_string(), " and one other person in ");
    assert_eq!(parts[9].get_canonical_placeholder_name(), "INTERPOLATION_2");
    assert_eq!(parts[10].get_string(), concat!(".}\n", "other {I see "));
    assert_eq!(
        parts[11].get_canonical_placeholder_name(),
        "INTERPOLATION_1"
    );
    assert_eq!(parts[12].get_string(), " and # other people in ");
    assert_eq!(
        parts[13].get_canonical_placeholder_name(),
        "INTERPOLATION_2"
    );
    assert_eq!(parts[14].get_string(), concat!(".}\n", "}\n"));
}

// port: JsMessageVisitorTest#testIcuTemplate
#[test]
fn test_icu_template() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "const MSG_ICU_EXAMPLE = declareIcuTemplate(\n",
        "    `{NUM_PEOPLE, plural, offset:1\n",
        "        =0 {I see no one at all in {INTERPOLATION_2}.}\n",
        "        =1 {I see {INTERPOLATION_1} in {INTERPOLATION_2}.}\n",
        "        =2 {I see {INTERPOLATION_1} and one other person in {INTERPOLATION_2}.}\n",
        "        other {I see {INTERPOLATION_1} and # other people in {INTERPOLATION_2}.}\n",
        "    }`,\n",
        "    {\n",
        "      description: 'ICU example message',\n",
        "      original_code: {\n",
        "        'INTERPOLATION_1': '{{getPerson()}}',\n",
        "        'INTERPOLATION_2': '{{getPlaceName()}}',\n",
        "      },\n",
        "      example: {\n",
        "        'INTERPOLATION_1': 'Jane Doe',\n",
        "        'INTERPOLATION_2': 'Paris, France',\n",
        "      }\n",
        "    });\n"
    ));
    assert_eq!(fx.icu_template_definitions.len(), 1);
    let definition = &fx.icu_template_definitions[0];
    let message_node = definition.get_message_node();
    assert!(message_node.is_call(&fx.compiler));
    let callee = message_node.get_first_child(&fx.compiler).unwrap();
    assert!(callee.is_name(&fx.compiler));
    assert_eq!(callee.get_string(&fx.compiler), "declareIcuTemplate");
    let template_text_node = definition.get_template_text_node();
    assert_eq!(
        template_text_node.get_token(&fx.compiler),
        Token::TEMPLATELIT
    );
    let msg = definition.get_message().clone();
    assert_eq!(msg.get_key(), "MSG_ICU_EXAMPLE");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("ICU example message"));
    assert_eq!(
        msg.as_icu_message_string(),
        concat!(
            "{NUM_PEOPLE, plural, offset:1\n",
            "        =0 {I see no one at all in {INTERPOLATION_2}.}\n",
            "        =1 {I see {INTERPOLATION_1} in {INTERPOLATION_2}.}\n",
            "        =2 {I see {INTERPOLATION_1} and one other person in {INTERPOLATION_2}.}\n",
            "        other {I see {INTERPOLATION_1} and # other people in {INTERPOLATION_2}.}\n",
            "    }"
        )
    );
    // NOTE: "testcode" is the file name used by compiler.parseTestCode(code)
    assert_eq!(opt(msg.get_source_name()).as_deref(), Some("testcode:1"));
}

// port: JsMessageVisitorTest#testIcuTemplatePlaceholderTypo
#[test]
fn test_icu_template_placeholder_typo() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "const MSG_ICU_EXAMPLE = declareIcuTemplate(\n",
        "    `{NUM_PEOPLE, plural, offset:1\n",
        "        =0 {I see no one at all in {INTERPOLATION_2}.}\n",
        "        =1 {I see {INTERPOLATION_1} in {INTERPOLATION_2}.}\n",
        "        =2 {I see {INTERPOLATION_1} and one other person in {INTERPOLATION_2}.}\n",
        "        other {I see {INTERPOLATION_1} and # other people in {INTERPOLATION_2}.}\n",
        "    }`,\n",
        "    {\n",
        "      description: 'ICU example message',\n",
        "      example: {\n",
        "        'INTERPOLATION_1': 'Jane Doe',\n",
        "// This placeholder name doesn't match any placeholders in the message, so it should\n",
        "// be reported as an error\n",
        "        'INTERPOLATION_TYPO_2': 'Paris, France',\n",
        "      }\n",
        "    });\n"
    ));
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. Unknown placeholder: INTERPOLATION_TYPO_2",
    );
    assert!(fx.compiler.get_warnings().is_empty());
    // The malformed message is skipped.
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testIcuTemplatePlaceholderUpperSnakeCase
#[test]
fn test_icu_template_placeholder_upper_snake_case() {
    let mut fx = Fixture::new();
    // For ICU templates you must always use UPPER_SNAKE_CASE for your placeholder names.
    // This matches the way the placeholder names are formatted in the XMB/XTB files.
    // Note that `goog.getMsg()` requires lowerCamelCase for placeholder names, but
    // actually converts them to UPPER_SNAKE_CASE when storing them in XMB and must convert
    // the UPPER_SNAKE_CASE to lowerCamelCase when reading from XTB files. We want to avoid
    // this needless complication for `declareIcuTemplate()` message declarations.
    fx.extract_messages_safely(concat!(
        "const MSG_ICU_EXAMPLE = declareIcuTemplate(\n",
        "    'Email: {USER_EMAIL}',\n",
        "    {\n",
        "      description: 'Labeled email address',\n",
        "      example: {\n",
        "        'USER_EMAIL': 'jane@doe.com',\n",
        "      }\n",
        "    });\n"
    ));
    let js_message = fx.assert_one_message();
    assert_contains_exactly(js_message.canonical_placeholder_names(), &["USER_EMAIL"]);
}

// port: JsMessageVisitorTest#testIcuTemplatePlaceholderLowerCamelCase
#[test]
fn test_icu_template_placeholder_lower_camel_case() {
    let mut fx = Fixture::new();
    // For ICU templates you must always use UPPER_SNAKE_CASE for your placeholder names.
    // This matches the way the placeholder names are formatted in the XMB/XTB files.
    // Note that `goog.getMsg()` requires lowerCamelCase for placeholder names, but
    // actually converts them to UPPER_SNAKE_CASE when storing them in XMB and must convert
    // the UPPER_SNAKE_CASE to lowerCamelCase when reading from XTB files. We want to avoid
    // this needless complication for `declareIcuTemplate()` message declarations.
    fx.extract_messages(concat!(
        "const MSG_ICU_EXAMPLE = declareIcuTemplate(\n",
        "    'Email: {userEmail}', // should be USER_EMAIL\n",
        "    {\n",
        "      description: 'Labeled email address',\n",
        "      example: {\n",
        "        'userEmail': 'jane@doe.com', // should be USER_EMAIL\n",
        "      }\n",
        "    });\n"
    ));
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. Placeholder not in UPPER_SNAKE_CASE: userEmail",
    );
    assert!(fx.compiler.get_warnings().is_empty());
    // The malformed message is skipped.
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testJsMessageOnVar
#[test]
fn test_js_message_on_var() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("/** @desc Hello */ var MSG_HELLO = goog.getMsg('a')");
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_HELLO");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("Hello"));
    // NOTE: "testcode" is the file name used by compiler.parseTestCode(code)
    assert_eq!(opt(msg.get_source_name()).as_deref(), Some("testcode:1"));
}

// port: JsMessageVisitorTest#testJsMessageOnLet
#[test]
fn test_js_message_on_let() {
    let mut fx = Fixture::new();
    fx.compiler_options = Some(CompilerOptions::new());
    fx.extract_messages_safely("/** @desc Hello */ let MSG_HELLO = goog.getMsg('a')");
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_HELLO");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("Hello"));
    // NOTE: "testcode" is the file name used by compiler.parseTestCode(code)
    assert_eq!(opt(msg.get_source_name()).as_deref(), Some("testcode:1"));
}

// port: JsMessageVisitorTest#testJsMessageOnConst
#[test]
fn test_js_message_on_const() {
    let mut fx = Fixture::new();
    fx.compiler_options = Some(CompilerOptions::new());
    fx.extract_messages_safely("/** @desc Hello */ const MSG_HELLO = goog.getMsg('a')");
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_HELLO");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("Hello"));
    // NOTE: "testcode" is the file name used by compiler.parseTestCode(code)
    assert_eq!(opt(msg.get_source_name()).as_deref(), Some("testcode:1"));
}

// port: JsMessageVisitorTest#testJsMessagesWithSrcMap
#[test]
fn test_js_messages_with_src_map() {
    let mut fx = Fixture::new();
    let mut source_map = SourceMapGeneratorV3::new();
    source_map.add_mapping(
        Some(s("source1.html")),
        None,
        FilePosition::new(10, 0),
        FilePosition::new(0, 0),
        FilePosition::new(0, 100),
    );
    source_map.add_mapping(
        Some(s("source2.html")),
        None,
        FilePosition::new(10, 0),
        FilePosition::new(1, 0),
        FilePosition::new(1, 100),
    );
    let mut output = String::new();
    source_map
        .append_to(&mut output, Some(s("unused.js")))
        .unwrap();

    fx.compiler_options = Some(CompilerOptions::new());
    // NOTE: "testcode" is the file name used by compiler.parseTestCode(code)
    fx.compiler_options
        .as_mut()
        .unwrap()
        .set_input_source_maps(IndexMap::<_, _>::from_iter([(
            "testcode".to_string(),
            Arc::new(SourceMapInput::new(Arc::new(SourceFile::from_code(
                "example.srcmap",
                output.as_str(),
            )))),
        )]));
    fx.extract_messages_safely(concat!(
        "/** @desc Hello */ var MSG_HELLO = goog.getMsg('a');\n",
        "/** @desc Hi */ var MSG_HI = goog.getMsg('b');\n"
    ));
    assert_eq!(fx.messages.len(), 2);
    let msg1 = fx.messages[0].clone();
    assert_eq!(msg1.get_key(), "MSG_HELLO");
    assert_eq!(opt(msg1.get_desc()).as_deref(), Some("Hello"));
    assert_eq!(
        opt(msg1.get_source_name()).as_deref(),
        Some("source1.html:11")
    );
    let msg2 = fx.messages[1].clone();
    assert_eq!(msg2.get_key(), "MSG_HI");
    assert_eq!(opt(msg2.get_desc()).as_deref(), Some("Hi"));
    assert_eq!(
        opt(msg2.get_source_name()).as_deref(),
        Some("source2.html:11")
    );
}

// port: JsMessageVisitorTest#testJsMessageOnProperty
#[test]
fn test_js_message_on_property() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(
        "/** @desc a */ pint.sub.MSG_MENU_MARK_AS_UNREAD = goog.getMsg('a')",
    );
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_MENU_MARK_AS_UNREAD");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("a"));
}

// port: JsMessageVisitorTest#testJsMessageOnPublicField
#[test]
fn test_js_message_on_public_field() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "class Foo {\n",
        "  /** @desc overflow menu */\n",
        "  MSG_OVERFLOW_MENU = goog.getMsg('More options');\n",
        "}\n"
    ));
    let mut msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_OVERFLOW_MENU");
    assert_eq!(msg.as_js_message_string(), "More options");
    // Anonymous class
    fx.extract_messages_safely(concat!(
        "foo(class {\n",
        "  /** @desc hi */\n",
        "  MSG_HELLO = goog.getMsg('Greetings');\n",
        "});\n"
    ));
    assert_eq!(fx.messages.len(), 2);
    msg = fx.messages[1].clone();
    assert_eq!(msg.get_key(), "MSG_HELLO");
    assert_eq!(msg.as_js_message_string(), "Greetings");
}

// port: JsMessageVisitorTest#testJsMessageOnPublicField_directAlias
#[test]
fn test_js_message_on_public_field_direct_alias() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc Something */\n",
        "const MSG_SOMETHING = goog.getMsg('Something');\n",
        "class Foo {\n",
        "  MSG_SOMETHING = MSG_SOMETHING;\n",
        "}\n"
    ));
    assert_eq!(fx.assert_one_message().get_key(), "MSG_SOMETHING");
}

// Note: This may be undesirable but represents the current behavior.
// Ideally the assignment to this MSG in the constructor should be sufficient to avoid the error.
// port: JsMessageVisitorTest#testJsMessageOnPublicField_indirectAliasPresentlyErrors
#[test]
fn test_js_message_on_public_field_indirect_alias_presently_errors() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc Anything */\n",
        "const MSG_ANYTHING = goog.getMsg('Anything');\n",
        "class Baz {\n",
        "  MSG_ANYTHING;\n",
        "  constructor() {\n",
        "    this.MSG_ANYTHING = MSG_ANYTHING;\n",
        "  }\n",
        "}\n"
    ));
    fx.assert_one_error_type(&MESSAGE_HAS_NO_VALUE);
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testErrorOnPublicFields
#[test]
fn test_error_on_public_fields() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "class Foo {\n",
        "  /** @desc */\n",
        "  MSG_WITH_NO_RHS;\n",
        "}\n"
    ));
    fx.assert_one_error_type(&MESSAGE_HAS_NO_VALUE);
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testErrorOnStaticField
#[test]
fn test_error_on_static_field() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "class Bar {\n",
        "  /** @desc */\n",
        "  static MSG_STATIC_FIELD_WITH_NO_RHS;\n",
        "}\n"
    ));
    fx.assert_one_error_type(&MESSAGE_HAS_NO_VALUE);
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testJsMessageOnPublicStaticField
#[test]
fn test_js_message_on_public_static_field() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "class Bar {\n",
        "  /** @desc menu */\n",
        "  static MSG_MENU = goog.getMsg('Options');\n",
        "}\n"
    ));
    let mut msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_MENU");
    assert_eq!(msg.as_js_message_string(), "Options");
    fx.extract_messages_safely(concat!(
        "let G = class {\n",
        "  /** @desc apples */\n",
        "  static MSG_FRUIT = goog.getMsg('Apples');\n",
        "}\n"
    ));
    assert_eq!(fx.messages.len(), 2);
    msg = fx.messages[1].clone();
    assert_eq!(msg.get_key(), "MSG_FRUIT");
    assert_eq!(msg.as_js_message_string(), "Apples");
}

// port: JsMessageVisitorTest#testStaticInheritance
#[test]
fn test_static_inheritance() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc a */\n",
        "foo.bar.BaseClass.MSG_MENU = goog.getMsg('hi');\n",
        "/**\n",
        " * @desc a\n",
        " * @suppress {visibility}\n",
        " */\n",
        "foo.bar.Subclass.MSG_MENU = foo.bar.BaseClass.MSG_MENU;\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_MENU");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("a"));
}

// port: JsMessageVisitorTest#testMsgInEnum
#[test]
fn test_msg_in_enum() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/**\n",
        " * @enum {number}\n",
        " */\n",
        "var MyEnum = {\n",
        "  MSG_ONE: 0\n",
        "};\n"
    ));
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_type(&MESSAGE_NOT_INITIALIZED_CORRECTLY);
}

// port: JsMessageVisitorTest#testMsgInEnumWithSuppression
#[test]
fn test_msg_in_enum_with_suppression() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @fileoverview\n",
        " * @suppress {messageConventions}\n",
        " */\n",
        "\n",
        "/**\n",
        " * @enum {number}\n",
        " */\n",
        "var MyEnum = {\n",
        "  MSG_ONE: 0\n",
        "};\n"
    ));
}

// port: JsMessageVisitorTest#testJsMessageOnObjLit
#[test]
fn test_js_message_on_obj_lit() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "pint.sub = {\n",
        "/** @desc a */ MSG_MENU_MARK_AS_UNREAD: goog.getMsg('a')}\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_MENU_MARK_AS_UNREAD");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("a"));
}

// port: JsMessageVisitorTest#testInvalidJsMessageOnObjLit
#[test]
fn test_invalid_js_message_on_obj_lit() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "pint.sub = {\n",
        "  /** @desc a */ MSG_MENU_MARK_AS_UNREAD: undefined\n",
        "}\n"
    ));
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_type(&MESSAGE_NOT_INITIALIZED_CORRECTLY);
}

// port: JsMessageVisitorTest#testJsMessageAliasOnObjLit
#[test]
fn test_js_message_alias_on_obj_lit() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "pint.sub = {\n",
        "  MSG_MENU_MARK_AS_UNREAD: another.namespace.MSG_MENU_MARK_AS_UNREAD\n",
        "}\n"
    ));
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testMessageAliasedToObject
#[test]
fn test_message_aliased_to_object() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("a.b.MSG_FOO = MSG_FOO;");
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testMsgPropertyAliasesMsgVariable_mismatchedMSGNameIsAllowed
#[test]
fn test_msg_property_aliases_msg_variable_mismatched_msg_name_is_allowed() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("a.b.MSG_FOO_ALIAS = MSG_FOO;");
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testMsgPropertyAliasesMsgProperty_mismatchedMSGNameIsAllowed
#[test]
fn test_msg_property_aliases_msg_property_mismatched_msg_name_is_allowed() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("a.b.MSG_FOO_ALIAS = c.d.MSG_FOO;");
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testMessageAliasedToObject_nonMSGNameIsNotAllowed
#[test]
fn test_message_aliased_to_object_non_msg_name_is_not_allowed() {
    let mut fx = Fixture::new();
    fx.extract_messages("a.b.MSG_FOO_ALIAS = someVarName;");
    assert!(fx.messages.is_empty());
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_type(&MESSAGE_NOT_INITIALIZED_CORRECTLY);
}

// port: JsMessageVisitorTest#testMessageExport_shortHand
#[test]
fn test_message_export_short_hand() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("exports = {MSG_FOO};");
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testMessageExport_longHand
#[test]
fn test_message_export_long_hand() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("exports = {MSG_FOO: MSG_FOO};");
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testMessageDefinedInExportsIsNotOrphaned
#[test]
fn test_message_defined_in_exports_is_not_orphaned() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "exports = {\n",
        "  /** @desc Description. */\n",
        "  MSG_FOO: goog.getMsg('Foo'),\n",
        "};\n"
    ));
}

// port: JsMessageVisitorTest#testJsMessageAlias_fromObjectDestrucuturing_longhand
#[test]
fn test_js_message_alias_from_object_destrucuturing_longhand() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("({MSG_MENU_MARK_AS_UNREAD: MSG_MENU_MARK_AS_UNREAD} = x);");
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testJsMessageAlias_fromObjectDestrucuturing_MSGlonghand_allowed
#[test]
fn test_js_message_alias_from_object_destrucuturing_ms_glonghand_allowed() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(
        "({MSG_FOO: MSG_FOO_ALIAS} = {/** @desc Foo */ MSG_FOO: goog.getMsg('Foo')});",
    );
    fx.assert_one_message();
}

// port: JsMessageVisitorTest#testJsMessageAlias_fromObjectDestrucuturing_shorthand
#[test]
fn test_js_message_alias_from_object_destrucuturing_shorthand() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("({MSG_MENU_MARK_AS_UNREAD} = x);");
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testJsMessageOnRHSOfVar
#[test]
fn test_js_message_on_rhs_of_var() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(
        "var MSG_MENU_MARK_AS_UNREAD = a.name.space.MSG_MENU_MARK_AS_UNREAD;",
    );
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testOrphanedJsMessage
#[test]
fn test_orphaned_js_message() {
    let mut fx = Fixture::new();
    fx.extract_messages("goog.getMsg('a')");
    assert!(fx.messages.is_empty());
    fx.assert_one_error_type(&js_message_visitor::MESSAGE_NODE_IS_ORPHANED);
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testOrphanedIcuTemplate
#[test]
fn test_orphaned_icu_template() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "const {declareIcuTemplate} = goog.require('goog.i18n.messages');\n",
        "\n",
        "declareIcuTemplate('a')\n"
    ));
    assert!(fx.messages.is_empty());
    fx.assert_one_error_type(&js_message_visitor::MESSAGE_NODE_IS_ORPHANED);
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testMessageWithoutDescription
#[test]
fn test_message_without_description() {
    let mut fx = Fixture::new();
    fx.extract_messages("var MSG_HELLO = goog.getMsg('a')");
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_HELLO");
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_type(&js_message_visitor::MESSAGE_HAS_NO_DESCRIPTION);
}

// port: JsMessageVisitorTest#testIncorrectMessageReporting
#[test]
fn test_incorrect_message_reporting() {
    let mut fx = Fixture::new();
    fx.extract_messages("var MSG_HELLO = goog.getMsg('a' + + 'b')");
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. literal string or concatenation expected",
    );
    assert!(fx.compiler.get_warnings().is_empty());
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testTemplateLiteral
#[test]
fn test_template_literal() {
    let mut fx = Fixture::new();
    fx.compiler_options = Some(CompilerOptions::new());
    fx.extract_messages_safely("/** @desc Hello */ var MSG_HELLO = goog.getMsg(`hello`);");
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_HELLO");
    assert_eq!(msg.as_js_message_string(), "hello");
}

// port: JsMessageVisitorTest#testTemplateLiteralWithSubstitution
#[test]
fn test_template_literal_with_substitution() {
    let mut fx = Fixture::new();
    fx.compiler_options = Some(CompilerOptions::new());
    fx.extract_messages("/** @desc Hello */ var MSG_HELLO = goog.getMsg(`hello ${name}`);");
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. Template literals with substitutions are not allowed.",
    );
    assert!(fx.compiler.get_warnings().is_empty());
    assert!(fx.messages.is_empty());
}

// port: JsMessageVisitorTest#testClosureMessageWithHelpPostfix
#[test]
fn test_closure_message_with_help_postfix() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc help text */\n",
        "var MSG_FOO_HELP = goog.getMsg('Help!');\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_FOO_HELP");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("help text"));
    assert_eq!(msg.as_js_message_string(), "Help!");
}

// port: JsMessageVisitorTest#testClosureMessageWithoutGoogGetmsg
#[test]
fn test_closure_message_without_goog_getmsg() {
    let mut fx = Fixture::new();
    fx.extract_messages("var MSG_FOO_HELP = 'I am a bad message';");
    assert!(fx.messages.is_empty());
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_type(&js_message_visitor::MESSAGE_NOT_INITIALIZED_CORRECTLY);
}

// port: JsMessageVisitorTest#testAllowOneMSGtoAliasAnotherMSG
#[test]
fn test_allow_one_ms_gto_alias_another_msg() {
    let mut fx = Fixture::new();
    // NOTE: tsickle code generation can end up creating new MSG_* variables that are temporary
    // aliases of existing ones that were defined correctly using goog.getMsg(). Don't complain
    // about them.
    fx.extract_messages_safely(concat!(
        "/** @desc A foo message */\n",
        "var MSG_FOO = goog.getMsg('Foo message');\n",
        "var MSG_FOO_1 = MSG_FOO;\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_FOO");
    assert_eq!(msg.as_js_message_string(), "Foo message");
}

// port: JsMessageVisitorTest#testDisallowOneMSGtoAliasNONMSG
#[test]
fn test_disallow_one_ms_gto_alias_nonmsg() {
    let mut fx = Fixture::new();
    // NOTE: tsickle code generation can end up creating new MSG_* variables that are temporary
    // aliases of existing ones that were defined correctly using goog.getMsg(). Don't complain
    // about them.
    fx.extract_messages(concat!(
        "/** @desc A foo message */\n",
        "var mymsg = 'Foo message';\n",
        "var MSG_FOO_1 = mymsg;\n"
    ));
    assert!(fx.messages.is_empty());
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_type(&js_message_visitor::MESSAGE_NOT_INITIALIZED_CORRECTLY);
}

// port: JsMessageVisitorTest#testClosureFormatParametizedFunction
#[test]
fn test_closure_format_parametized_function() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc help text */\n",
        "var MSG_SILLY = goog.getMsg('{$adjective} ' + 'message',\n",
        "{'adjective': 'silly'});\n"
    ));
    assert_eq!(fx.message_definitions.len(), 1);
    let js_message_definition = &fx.message_definitions[0];
    // `goog.getMsg(...)`
    let message_node = js_message_definition.get_message_node();
    assert!(message_node.is_call(&fx.compiler));
    assert!(
        message_node
            .get_first_child(&fx.compiler)
            .unwrap()
            .matches_qualified_name(&fx.compiler, "goog.getMsg")
    );
    // `'{$adjective} ' + 'message'`
    let template_text_node = js_message_definition.get_template_text_node();
    assert_eq!(template_text_node.get_token(&fx.compiler), Token::ADD);
    assert_is_string(
        &fx.compiler,
        template_text_node.get_first_child(&fx.compiler).unwrap(),
        "{$adjective} ",
    );
    // `{'adjective': ...}`
    let placeholder_values_node = js_message_definition.get_placeholder_values_node();
    let placeholder_values_node = placeholder_values_node.unwrap();
    assert!(placeholder_values_node.is_object_lit(&fx.compiler));
    let first_key = placeholder_values_node
        .get_first_child(&fx.compiler)
        .unwrap();
    assert_eq!(first_key.get_token(&fx.compiler), Token::STRING_KEY);
    assert_eq!(first_key.get_string(&fx.compiler), "adjective");
    // Map containing "adjective" -> string node containing "silly"
    let placeholder_value_map = js_message_definition.get_placeholder_value_map();
    assert_eq!(placeholder_value_map.len(), 1);
    assert_is_string(
        &fx.compiler,
        placeholder_value_map[&s("adjective")],
        "silly",
    );
    let msg = js_message_definition.get_message().clone();
    assert_eq!(msg.get_key(), "MSG_SILLY");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("help text"));
    assert_eq!(msg.as_js_message_string(), "{$adjective} message");
}

// port: JsMessageVisitorTest#testHugeMessage
#[test]
fn test_huge_message() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/**\n",
        " * @desc A message with lots of stuff.\n",
        " */\n",
        "var MSG_HUGE = goog.getMsg(\n",
        "    '{$startLink_1}Google{$endLink}' +\n",
        "    '{$startLink_2}blah{$endLink}{$boo}{$foo_001}{$boo}' +\n",
        "    '{$foo_002}{$xxx_001}{$image}{$image_001}{$xxx_002}',\n",
        "    {'startLink_1': '<a href=http://www.google.com/>',\n",
        "     'endLink': '</a>',\n",
        "     'startLink_2': '<a href=\"' + opt_data.url + '\">',\n",
        "     'boo': opt_data.boo,\n",
        "     'foo_001': opt_data.foo,\n",
        "     'foo_002': opt_data.boo.foo,\n",
        "     'xxx_001': opt_data.boo + opt_data.foo,\n",
        "     'image': htmlTag7,\n",
        "     'image_001': opt_data.image,\n",
        "     'xxx_002': foo.callWithOnlyTopLevelKeys(\n",
        "         bogusFn, opt_data, null, 'bogusKey1',\n",
        "         opt_data.moo, 'bogusKey2', param10)});\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_HUGE");
    assert_eq!(
        opt(msg.get_desc()).as_deref(),
        Some("A message with lots of stuff.")
    );
    assert_eq!(
        msg.as_js_message_string(),
        "{$startLink_1}Google{$endLink}{$startLink_2}blah{$endLink}{$boo}{$foo_001}{$boo}{$foo_002}{$xxx_001}{$image}{$image_001}{$xxx_002}"
    );
}

// port: JsMessageVisitorTest#testUnnamedGoogleMessage
#[test]
fn test_unnamed_google_message() {
    let mut fx = Fixture::new();
    fx.extract_messages("var MSG_UNNAMED = goog.getMsg('Hullo');");
    let msg = fx.assert_one_message();
    assert!(msg.get_desc().is_none());
    assert_eq!(msg.get_key(), "MSG_16LJMYKCXT84X");
    assert_eq!(msg.get_id(), "MSG_16LJMYKCXT84X");
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_type(&js_message_visitor::MESSAGE_HAS_NO_DESCRIPTION);
}

// port: JsMessageVisitorTest#testUnnamedIcuTemplate
#[test]
fn test_unnamed_icu_template() {
    let mut fx = Fixture::new();
    // Unlike `goog.getMsg()` we do require a description for anonymous ICU templates.
    // Requiring the description is less complicated, and there doesn't seem to be any reason
    // not to require them.
    fx.extract_messages_safely(
        "var MSG_UNNAMED = declareIcuTemplate('Hullo', {description: 'description'});",
    );
    assert_eq!(fx.icu_template_definitions.len(), 1);
    let msg = fx.icu_template_definitions[0].get_message().clone();
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("description"));
    assert_eq!(msg.get_key(), "MSG_16LJMYKCXT84X");
    assert_eq!(msg.get_id(), "MSG_16LJMYKCXT84X");
}

// port: JsMessageVisitorTest#testMeaningGetsUsedAsIdIfTheresNoGenerator
#[test]
fn test_meaning_gets_used_as_id_if_theres_no_generator() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/**\n",
        " * @desc some description\n",
        " * @meaning some meaning\n",
        " */\n",
        "var MSG_HULLO = goog.getMsg('Hullo');\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("some description"));
    assert_eq!(msg.get_key(), "MSG_HULLO");
    assert_eq!(opt(msg.get_meaning()).as_deref(), Some("some meaning"));
    assert_eq!(msg.get_id(), "some meaning");
}

// port: JsMessageVisitorTest#testEmptyTextMessage
#[test]
fn test_empty_text_message() {
    let mut fx = Fixture::new();
    fx.extract_messages("/** @desc text */ var MSG_FOO = goog.getMsg('');");
    fx.assert_one_message();
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_description(
        "Message value of MSG_FOO is just an empty string. Empty messages are forbidden.",
    );
}

// port: JsMessageVisitorTest#testEmptyTextComplexMessage
#[test]
fn test_empty_text_complex_message() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc text */ var MSG_BAR = goog.getMsg(\n",
        "'' + '' + ''     + '' +'');\n"
    ));
    fx.assert_one_message();
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning_description(
        "Message value of MSG_BAR is just an empty string. Empty messages are forbidden.",
    );
}

// port: JsMessageVisitorTest#testMsgVarWithoutAssignment
#[test]
fn test_msg_var_without_assignment() {
    let mut fx = Fixture::new();
    fx.extract_messages("var MSG_SILLY;");
    fx.assert_one_error_type(&js_message_visitor::MESSAGE_HAS_NO_VALUE);
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testRegularVarWithoutAssignment
#[test]
fn test_regular_var_without_assignment() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("var SILLY;");
    assert!(fx.messages.is_empty());
}

// JsMessageVisitorTest#testMsgPropertyWithoutAssignment is @Ignore in Java ("Currently
// unimplemented"; its assertion fails against the pinned Java too): not ported.

// port: JsMessageVisitorTest#testMsgVarWithIncorrectRightSide
#[test]
fn test_msg_var_with_incorrect_right_side() {
    let mut fx = Fixture::new();
    fx.extract_messages("var MSG_SILLY = 0;");
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning(&MESSAGE_NOT_INITIALIZED_CORRECTLY, "Message must be initialized using a call to goog.getMsg or goog.i18n.messages.declareIcuTemplate");
}

// port: JsMessageVisitorTest#testIncorrectMessage
#[test]
fn test_incorrect_message() {
    let mut fx = Fixture::new();
    fx.extract_messages("DP_DatePicker.MSG_DATE_SELECTION = {};");
    assert!(fx.messages.is_empty());
    assert!(fx.compiler.get_errors().is_empty());
    fx.assert_one_warning(&MESSAGE_NOT_INITIALIZED_CORRECTLY, "Message must be initialized using a call to goog.getMsg or goog.i18n.messages.declareIcuTemplate");
}

// port: JsMessageVisitorTest#testUnrecognizedFunction
#[test]
fn test_unrecognized_function() {
    let mut fx = Fixture::new();
    fx.extract_messages("DP_DatePicker.MSG_DATE_SELECTION = somefunc('a')");
    assert!(fx.messages.is_empty());
    fx.assert_one_error_description("Message parse tree malformed. Message must be initialized using a call to goog.getMsg or declareIcuTemplate (from goog.i18n.messages).");
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testExtractPropertyMessage
#[test]
fn test_extract_property_message() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/**\n",
        " * @desc A message that demonstrates placeholders\n",
        " */\n",
        "a.b.MSG_SILLY = goog.getMsg(\n",
        "    '{$adjective} ' + '{$someNoun}',\n",
        "    {'adjective': adj, 'someNoun': noun});\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_SILLY");
    assert_eq!(msg.as_js_message_string(), "{$adjective} {$someNoun}");
    assert_eq!(
        opt(msg.get_desc()).as_deref(),
        Some("A message that demonstrates placeholders")
    );
}

// port: JsMessageVisitorTest#testExtractPropertyMessageInFunction
#[test]
fn test_extract_property_message_in_function() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "function f() {\n",
        "  /**\n",
        "   * @desc A message that demonstrates placeholders\n",
        "   */\n",
        "  a.b.MSG_SILLY = goog.getMsg(\n",
        "      '{$adjective} ' + '{$someNoun}',\n",
        "      {'adjective': adj, 'someNoun': noun});\n",
        "}\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_SILLY");
    assert_eq!(msg.as_js_message_string(), "{$adjective} {$someNoun}");
    assert_eq!(
        opt(msg.get_desc()).as_deref(),
        Some("A message that demonstrates placeholders")
    );
}

// port: JsMessageVisitorTest#testAlmostButNotExternalMessage
#[test]
fn test_almost_but_not_external_message() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("/** @desc External */ var MSG_EXTERNAL = goog.getMsg('External');");
    let msg = fx.assert_one_message();
    assert!(!msg.is_external());
    assert_eq!(msg.get_key(), "MSG_EXTERNAL");
}

// port: JsMessageVisitorTest#testExternalMessage
#[test]
fn test_external_message() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("var MSG_EXTERNAL_111 = goog.getMsg('Hello World');");
    let msg = fx.assert_one_message();
    assert!(msg.is_external());
    assert_eq!(msg.get_id(), "111");
}

// port: JsMessageVisitorTest#testExternalMessage_customSuffix
#[test]
fn test_external_message_custom_suffix() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely("var MSG_EXTERNAL_111$$1 = goog.getMsg('Hello World');");
    let msg = fx.assert_one_message();
    assert!(msg.is_external());
    assert_eq!(msg.get_id(), "111");
}

// port: JsMessageVisitorTest#testIsValidMessageNameStrict
#[test]
fn test_is_valid_message_name_strict() {
    let visitor = DummyJsVisitor::new();
    assert!(visitor.is_message_name(&s("MSG_HELLO")));
    assert!(visitor.is_message_name(&s("MSG_")));
    assert!(visitor.is_message_name(&s("MSG_HELP")));
    assert!(visitor.is_message_name(&s("MSG_FOO_HELP")));
    assert!(!visitor.is_message_name(&s("_FOO_HELP")));
    assert!(!visitor.is_message_name(&s("MSGFOOP")));
}

// port: JsMessageVisitorTest#testUnexistedPlaceholders
#[test]
fn test_unexisted_placeholders() {
    let mut fx = Fixture::new();
    fx.extract_messages("var MSG_FOO = goog.getMsg('{$foo}:', {});");
    assert!(fx.messages.is_empty());
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. Unrecognized message placeholder referenced: foo",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testUnusedReferenesAreNotOK
#[test]
fn test_unused_referenes_are_not_ok() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc AA */\n",
        "var MSG_FOO = goog.getMsg('lalala:', {foo:1});\n"
    ));
    assert!(fx.messages.is_empty());
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. Unused message placeholder: foo",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testDuplicatePlaceHoldersAreBad
#[test]
fn test_duplicate_place_holders_are_bad() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "var MSG_FOO = goog.getMsg(\n",
        "'{$foo}:', {'foo': 1, 'foo' : 2});\n"
    ));
    assert!(fx.messages.is_empty());
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. duplicate string key: foo",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testDuplicatePlaceholderReferencesAreOk
#[test]
fn test_duplicate_placeholder_references_are_ok() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc Sample description */\n",
        "var MSG_FOO = goog.getMsg('{$foo}:, {$foo}', {'foo': 1});\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.as_js_message_string(), "{$foo}:, {$foo}");
}

// port: JsMessageVisitorTest#testCamelcasePlaceholderNamesAreOk
#[test]
fn test_camelcase_placeholder_names_are_ok() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!("/** @desc Hello */\n", "var MSG_WITH_CAMELCASE = goog.getMsg('Slide {$slideNumber}:', {'slideNumber': opt_index + 1});\n"));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_WITH_CAMELCASE");
    assert_eq!(msg.as_js_message_string(), "Slide {$slideNumber}:");
    let parts = msg.get_parts();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[1].get_js_placeholder_name(), "slideNumber");
    assert_eq!(parts[1].get_canonical_placeholder_name(), "SLIDE_NUMBER");
}

// port: JsMessageVisitorTest#testNonCamelcasePlaceholderNamesAreNotOkInMsgText
#[test]
fn test_non_camelcase_placeholder_names_are_not_ok_in_msg_text() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "var MSG_WITH_CAMELCASE = goog.getMsg(\n",
        "    'Slide {$SLIDE_NUMBER}:',\n",
        "    {'slideNumber': opt_index + 1});\n"
    ));
    assert!(fx.messages.is_empty());
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. Placeholder name not in lowerCamelCase: SLIDE_NUMBER",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testNonCamelcasePlaceholderNamesAreNotOkInPlaceholderObject
#[test]
fn test_non_camelcase_placeholder_names_are_not_ok_in_placeholder_object() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "var MSG_WITH_CAMELCASE = goog.getMsg(\n",
        "    'Slide {$slideNumber}:',\n",
        "    {'SLIDE_NUMBER': opt_index + 1});\n"
    ));
    assert!(fx.messages.is_empty());
    fx.assert_one_error(
        &MESSAGE_TREE_MALFORMED,
        "Message parse tree malformed. Unrecognized message placeholder referenced: slideNumber",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testUnquotedPlaceholdersAreOk
#[test]
fn test_unquoted_placeholders_are_ok() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc Hello */\n",
        "var MSG_FOO = goog.getMsg('foo {$unquoted}:', {unquoted: 12});\n"
    ));
    fx.assert_one_message();
}

// port: JsMessageVisitorTest#testDuplicateMessageError
#[test]
fn test_duplicate_message_error() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "(function () {/** @desc Hello */ var MSG_HELLO = goog.getMsg('a')})\n",
        "(function () {/** @desc Hello2 */ var MSG_HELLO = goog.getMsg('a')})\n"
    ));
    fx.assert_one_error_type(&js_message_visitor::MESSAGE_DUPLICATE_KEY);
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testNoDuplicateErrorOnExternMessage
#[test]
fn test_no_duplicate_error_on_extern_message() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "(function () {/** @desc Hello */\n",
        "var MSG_EXTERNAL_2 = goog.getMsg('a')})\n",
        "(function () {/** @desc Hello2 */\n",
        "var MSG_EXTERNAL_2 = goog.getMsg('a')})\n"
    ));
}

// port: JsMessageVisitorTest#testUsingMsgPrefixWithFallback
#[test]
fn test_using_msg_prefix_with_fallback() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "function f() {\n",
        "/** @desc Hello */ var MSG_UNNAMED_1 = goog.getMsg('hello');\n",
        "/** @desc Hello */ var MSG_UNNAMED_2 = goog.getMsg('hello');\n",
        "var x = goog.getMsgWithFallback(\n",
        "    MSG_UNNAMED_1, MSG_UNNAMED_2);\n",
        "}\n"
    ));
}

// port: JsMessageVisitorTest#testUsingMsgPrefixWithFallback_rename
#[test]
fn test_using_msg_prefix_with_fallback_rename() {
    let mut fx = Fixture::new();
    fx.rename_messages = true;
    fx.extract_messages_safely(concat!(
        "function f() {\n",
        "/** @desc Hello */ var MSG_A = goog.getMsg('hello');\n",
        "/** @desc Hello */ var MSG_B = goog.getMsg('hello!');\n",
        "var x = goog.getMsgWithFallback(MSG_A, MSG_B);\n",
        "}\n"
    ));
}

// port: JsMessageVisitorTest#testUsingMsgPrefixWithFallback_duplicateUnnamedKeys_rename
#[test]
fn test_using_msg_prefix_with_fallback_duplicate_unnamed_keys_rename() {
    let mut fx = Fixture::new();
    fx.rename_messages = true;
    fx.extract_messages_safely(concat!(
        "function f() {\n",
        "  /** @desc Hello */ var MSG_UNNAMED_1 = goog.getMsg('hello');\n",
        "  /** @desc Hello */ var MSG_UNNAMED_2 = goog.getMsg('hello');\n",
        "  var x = goog.getMsgWithFallback(\n",
        "      MSG_UNNAMED_1, MSG_UNNAMED_2);\n",
        "}\n",
        "function g() {\n",
        "  /** @desc Hello */ var MSG_UNNAMED_1 = goog.getMsg('hello');\n",
        "  /** @desc Hello */ var MSG_UNNAMED_2 = goog.getMsg('hello');\n",
        "  var x = goog.getMsgWithFallback(\n",
        "      MSG_UNNAMED_1, MSG_UNNAMED_2);\n",
        "}\n"
    ));
}

// port: JsMessageVisitorTest#testUsingMsgPrefixWithFallback_module
#[test]
fn test_using_msg_prefix_with_fallback_module() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc Hello */ var MSG_A = goog.getMsg('hello');\n",
        "/** @desc Hello */ var MSG_B = goog.getMsg('hello!');\n",
        "var x = goog.getMsgWithFallback(messages.MSG_A, MSG_B);\n"
    ));
}

// port: JsMessageVisitorTest#testUsingMsgPrefixWithFallback_moduleRenamed
#[test]
fn test_using_msg_prefix_with_fallback_module_renamed() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc Hello */ var MSG_A = goog.getMsg('hello');\n",
        "/** @desc Hello */ var MSG_B = goog.getMsg('hello!');\n",
        "var x = goog.getMsgWithFallback(module$exports$messages$MSG_A, MSG_B);\n"
    ));
}

// port: JsMessageVisitorTest#testErrorWhenUsingMsgPrefixWithFallback
#[test]
fn test_error_when_using_msg_prefix_with_fallback() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc Hello */ var MSG_HELLO_1 = goog.getMsg('hello');\n",
        "/** @desc Hello */ var MSG_HELLO_2 = goog.getMsg('hello');\n",
        "/** @desc Hello */\n",
        "var MSG_HELLO_3 = goog.getMsgWithFallback(MSG_HELLO_1, MSG_HELLO_2);\n"
    ));
    fx.assert_one_error_type(&MESSAGE_TREE_MALFORMED);
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testRenamedMessages_var
#[test]
fn test_renamed_messages_var() {
    let mut fx = Fixture::new();
    fx.rename_messages = true;
    fx.extract_messages_safely("/** @desc Hello */ var MSG_HELLO = goog.getMsg('a')");
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_HELLO");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("Hello"));
    // NOTE: "testcode" is the file name used by compiler.parseTestCode(code)
    assert_eq!(opt(msg.get_source_name()).as_deref(), Some("testcode:1"));
}

// port: JsMessageVisitorTest#testRenamedMessages_getprop
#[test]
fn test_renamed_messages_getprop() {
    let mut fx = Fixture::new();
    fx.rename_messages = true;
    fx.extract_messages_safely(
        "/** @desc a */ pint.sub.MSG_MENU_MARK_AS_UNREAD = goog.getMsg('a')",
    );
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_key(), "MSG_MENU_MARK_AS_UNREAD");
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("a"));
}

// port: JsMessageVisitorTest#testGetMsgWithOptions
#[test]
fn test_get_msg_with_options() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc Hello */\n",
        "var MSG_HELLO =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        {\n",
        "          'name': getName(),\n",
        "        },\n",
        "        {\n",
        "          html: false,\n",
        "          unescapeHtmlEntities: true,\n",
        "          example: {\n",
        "            'name': 'George',\n",
        "          },\n",
        "          original_code: {\n",
        "            'name': 'getName()',\n",
        "          },\n",
        "        })\n"
    ));
    let msg = fx.assert_one_message();
    // MSG_HELLO =
    assert_eq!(msg.get_key(), "MSG_HELLO");
    // * @desc Hello
    assert_eq!(opt(msg.get_desc()).as_deref(), Some("Hello"));
    let msg_definition = &fx.message_definitions[0];
    // goog.getMsg(...)
    let call_node = msg_definition.get_message_node();
    assert!(call_node.is_call(&fx.compiler));
    assert!(
        call_node
            .get_first_child(&fx.compiler)
            .unwrap()
            .matches_qualified_name(&fx.compiler, "goog.getMsg")
    );
    assert_is_string(
        &fx.compiler,
        msg_definition.get_template_text_node(),
        "Hello, {$name}",
    );
    // `{ 'name': getName() }`
    let placeholder_values_node = msg_definition.get_placeholder_values_node();
    let placeholder_values_node = placeholder_values_node.unwrap();
    assert!(placeholder_values_node.is_object_lit(&fx.compiler));
    assert_eq!(
        call_node.get_child_at_index(&fx.compiler, 2),
        Some(placeholder_values_node)
    );
    // placeholder name 'name' maps to the `getName()` call in the values map
    let placeholder_value_map = msg_definition.get_placeholder_value_map();
    assert_contains_exactly(placeholder_value_map.keys(), &["name"]);
    let name_value_node = placeholder_value_map[&s("name")];
    assert!(name_value_node.is_call(&fx.compiler));
    assert!(name_value_node.has_one_child(&fx.compiler));
    let only_child = name_value_node.get_first_child(&fx.compiler).unwrap();
    assert!(only_child.is_name(&fx.compiler));
    assert_eq!(only_child.get_string(&fx.compiler), "getName");
    assert_eq!(
        name_value_node.get_grandparent(&fx.compiler),
        Some(placeholder_values_node)
    );
    // `html: false`
    assert!(!msg_definition.should_escape_less_than());
    // `unescapeHtmlEntities: true`
    assert!(msg_definition.should_unescape_html_entities());
    // `example: { 'name': 'George' }`
    assert_eq!(
        msg.get_placeholder_name_to_example_map(),
        &IndexMap::<_, _>::from_iter([(s("name"), s("George"))])
    );
    // `original_code: {'name': 'getName()' }`
    assert_eq!(
        msg.get_placeholder_name_to_original_code_map(),
        &IndexMap::<_, _>::from_iter([(s("name"), s("getName()"))])
    );
}

// port: JsMessageVisitorTest#testGoogGetMsgWithNoArgs
#[test]
fn test_goog_get_msg_with_no_args() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X = goog.getMsg(); // no arguments\n"
    ));
    fx.assert_one_error_description(
        "Message parse tree malformed. Message string literal expected",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithBadValuesArg
#[test]
fn test_goog_get_msg_with_bad_values_arg() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        getName()); // should be an object literal\n"
    ));
    fx.assert_one_error_description("Message parse tree malformed. object literal expected");
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithBadValuesKey
#[test]
fn test_goog_get_msg_with_bad_values_key() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        {\n",
        "          [name]: getName() // a computed key is not allowed\n",
        "        });\n"
    ));
    fx.assert_one_error_description("Message parse tree malformed. string key expected");
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithBadOptionsArg
#[test]
fn test_goog_get_msg_with_bad_options_arg() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        { 'name': getName() },\n",
        "        options); // options bag must be an object literal\n"
    ));
    fx.assert_one_error_description("Message parse tree malformed. object literal expected");
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithComputedKeyInOptions
#[test]
fn test_goog_get_msg_with_computed_key_in_options() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        { 'name': getName() },\n",
        "        {\n",
        "          [options]: true // option names cannot be computed keys\n",
        "        });\n"
    ));
    fx.assert_one_error_description("Message parse tree malformed. string key expected");
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithUnknownOption
#[test]
fn test_goog_get_msg_with_unknown_option() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        { 'name': getName() },\n",
        "        {\n",
        "          unknownOption: true // not a valid option name\n",
        "        });\n"
    ));
    fx.assert_one_error_description("Message parse tree malformed. Unknown option: unknownOption");
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithInvalidBooleanOption
#[test]
fn test_goog_get_msg_with_invalid_boolean_option() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        { 'name': getName() },\n",
        "        {\n",
        "          html: 'true' // boolean option value must be a boolean literal\n",
        "        });\n"
    ));
    fx.assert_one_error_description(
        "Message parse tree malformed. html: Literal true or false expected",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithOriginalCodeForInvalidExample
#[test]
fn test_goog_get_msg_with_original_code_for_invalid_example() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        { 'name': getName() },\n",
        "        {\n",
        "          example: 'name: something' // not an object literal\n",
        "        });\n"
    ));
    fx.assert_one_error_description("Message parse tree malformed. object literal expected");
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithInvalidOriginalCodeValue
#[test]
fn test_goog_get_msg_with_invalid_original_code_value() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        { 'name': getName() },\n",
        "        {\n",
        "          original_code: {\n",
        "            'name': getName() // value is not a string\n",
        "          }\n",
        "        });\n"
    ));
    fx.assert_one_error_description(
        "Message parse tree malformed. literal string or concatenation expected",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGoogGetMsgWithOriginalCodeForUnknownPlaceholder
#[test]
fn test_goog_get_msg_with_original_code_for_unknown_placeholder() {
    let mut fx = Fixture::new();
    fx.extract_messages(concat!(
        "/** @desc something */\n",
        "const MSG_X =\n",
        "    goog.getMsg(\n",
        "        'Hello, {$name}',\n",
        "        { 'name': getName() },\n",
        "        {\n",
        "          original_code: {\n",
        "            'unknownPlaceholder': 'something' // not a valid placeholder name\n",
        "          }\n",
        "        });\n"
    ));
    fx.assert_one_error_description(
        "Message parse tree malformed. Unknown placeholder: unknownPlaceholder",
    );
    assert!(fx.compiler.get_warnings().is_empty());
}

// port: JsMessageVisitorTest#testGetMsgWithGoogScope
#[test]
fn test_get_msg_with_goog_scope() {
    let mut fx = Fixture::new();
    fx.extract_messages_safely(concat!(
        "/** @desc Suggestion Code found outside of <head> tag. */\n",
        "var $jscomp$scope$12345$0$MSG_CONSUMER_SURVEY_CODE_OUTSIDE_BODY_TAG =\n",
        "goog.getMsg('Code should be added to <body> tag.');\n"
    ));
    let msg = fx.assert_one_message();
    assert_eq!(msg.get_id(), "MSG_CONSUMER_SURVEY_CODE_OUTSIDE_BODY_TAG");
    assert_eq!(
        msg.get_key(),
        "$jscomp$scope$12345$0$MSG_CONSUMER_SURVEY_CODE_OUTSIDE_BODY_TAG"
    );
}
