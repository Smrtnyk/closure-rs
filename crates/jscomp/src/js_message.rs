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
/*
 * Copyright (C) 2006 The Guava Authors
 * Copyright (C) 2010 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
 * in compliance with the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License
 * is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
 * or implied. See the License for the specific language governing permissions and limitations under
 * the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/JsMessage.java.
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/base/Ascii.java, com/google/common/base/CaseFormat.java.

//! Port of `com.google.javascript.jscomp.JsMessage`: a representation of a translatable message
//! in JavaScript source code.
//!
//! Strings that come from JS source (keys, ids, message text, placeholder names, descriptions
//! and meanings) are `JsString` (DESIGN.md §5); the source name is a file name and stays `String`.

use std::fmt;
use std::sync::LazyLock;

use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::java_lang;
use closure_rhino::java_lang::regex::Pattern;
use closure_rhino::java_lang::utf_8;
use closure_rhino::js_string::JsString;
use closure_rhino::{check_argument, check_not_null, check_state};

/// `record JsMessage(...)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsMessage {
    get_source_name: Option<String>,
    get_key: JsString,
    is_anonymous: bool,
    is_external: bool,
    get_id: JsString,
    get_parts: Vec<Part>,
    get_gendered_messages_map: IndexMap<GrammaticalGenderCase, Vec<Part>>,
    get_alternate_id: Option<JsString>,
    get_desc: Option<JsString>,
    get_meaning: Option<JsString>,
    get_placeholder_name_to_example_map: IndexMap<JsString, JsString>,
    get_placeholder_name_to_original_code_map: IndexMap<JsString, JsString>,
    js_placeholder_names: IndexSet<JsString>,
    canonical_placeholder_names: IndexSet<JsString>,
}

// port: JsMessage#PH_JS_PREFIX
pub const PH_JS_PREFIX: &str = "{$";
// port: JsMessage#PH_JS_SUFFIX
pub const PH_JS_SUFFIX: &str = "}";

/// Enum for grammatical gender cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GrammaticalGenderCase {
    MASCULINE,
    FEMININE,
    NEUTER,
    OTHER,
}

impl GrammaticalGenderCase {
    /// Returns the enum constant with the specified name; panics like Java's
    /// `IllegalArgumentException` (or `NullPointerException` for `None`) otherwise.
    // port: GrammaticalGenderCase#valueOf
    pub fn value_of(name: Option<&str>) -> GrammaticalGenderCase {
        match name {
            Some("MASCULINE") => GrammaticalGenderCase::MASCULINE,
            Some("FEMININE") => GrammaticalGenderCase::FEMININE,
            Some("NEUTER") => GrammaticalGenderCase::NEUTER,
            Some("OTHER") => GrammaticalGenderCase::OTHER,
            Some(name) => panic!(
                "No enum constant com.google.javascript.jscomp.JsMessage.GrammaticalGenderCase.{name}"
            ),
            None => panic!("Name is null"),
        }
    }
}

impl fmt::Display for GrammaticalGenderCase {
    // port: GrammaticalGenderCase#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            GrammaticalGenderCase::MASCULINE => "MASCULINE",
            GrammaticalGenderCase::FEMININE => "FEMININE",
            GrammaticalGenderCase::NEUTER => "NEUTER",
            GrammaticalGenderCase::OTHER => "OTHER",
        };
        f.write_str(name)
    }
}

/// Thrown when parsing a message string into parts fails because of a misformatted place holder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceholderFormatException {
    message: String,
}

impl PlaceholderFormatException {
    // port: PlaceholderFormatException#PlaceholderFormatException
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }

    // port: Throwable#getMessage
    pub fn get_message(&self) -> &str {
        &self.message
    }
}

impl JsMessage {
    // port: JsMessage#JsMessage
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        get_source_name: Option<String>,
        get_key: JsString,
        is_anonymous: bool,
        is_external: bool,
        get_id: JsString,
        get_parts: Vec<Part>,
        get_gendered_messages_map: IndexMap<GrammaticalGenderCase, Vec<Part>>,
        get_alternate_id: Option<JsString>,
        get_desc: Option<JsString>,
        get_meaning: Option<JsString>,
        get_placeholder_name_to_example_map: IndexMap<JsString, JsString>,
        get_placeholder_name_to_original_code_map: IndexMap<JsString, JsString>,
        js_placeholder_names: IndexSet<JsString>,
        canonical_placeholder_names: IndexSet<JsString>,
    ) -> Self {
        Self {
            get_source_name,
            get_key,
            is_anonymous,
            is_external,
            get_id,
            get_parts,
            get_gendered_messages_map,
            get_alternate_id,
            get_desc,
            get_meaning,
            get_placeholder_name_to_example_map,
            get_placeholder_name_to_original_code_map,
            js_placeholder_names,
            canonical_placeholder_names,
        }
    }

    // port: JsMessage#getSourceName
    pub fn get_source_name(&self) -> Option<&str> {
        self.get_source_name.as_deref()
    }

    // port: JsMessage#getKey
    pub fn get_key(&self) -> &JsString {
        &self.get_key
    }

    // port: JsMessage#isAnonymous
    pub fn is_anonymous(&self) -> bool {
        self.is_anonymous
    }

    // port: JsMessage#isExternal
    pub fn is_external(&self) -> bool {
        self.is_external
    }

    // port: JsMessage#getId
    pub fn get_id(&self) -> &JsString {
        &self.get_id
    }

    // port: JsMessage#getParts
    pub fn get_parts(&self) -> &[Part] {
        &self.get_parts
    }

    // port: JsMessage#getGenderedMessagesMap
    pub fn get_gendered_messages_map(&self) -> &IndexMap<GrammaticalGenderCase, Vec<Part>> {
        &self.get_gendered_messages_map
    }

    // port: JsMessage#getAlternateId
    pub fn get_alternate_id(&self) -> Option<&JsString> {
        self.get_alternate_id.as_ref()
    }

    // port: JsMessage#getDesc
    pub fn get_desc(&self) -> Option<&JsString> {
        self.get_desc.as_ref()
    }

    // port: JsMessage#getMeaning
    pub fn get_meaning(&self) -> Option<&JsString> {
        self.get_meaning.as_ref()
    }

    // port: JsMessage#getPlaceholderNameToExampleMap
    pub fn get_placeholder_name_to_example_map(&self) -> &IndexMap<JsString, JsString> {
        &self.get_placeholder_name_to_example_map
    }

    // port: JsMessage#getPlaceholderNameToOriginalCodeMap
    pub fn get_placeholder_name_to_original_code_map(&self) -> &IndexMap<JsString, JsString> {
        &self.get_placeholder_name_to_original_code_map
    }

    // port: JsMessage#jsPlaceholderNames
    pub fn js_placeholder_names(&self) -> &IndexSet<JsString> {
        &self.js_placeholder_names
    }

    // port: JsMessage#canonicalPlaceholderNames
    pub fn canonical_placeholder_names(&self) -> &IndexSet<JsString> {
        &self.canonical_placeholder_names
    }

    // port: JsMessage#getPlaceholderOriginalCode
    pub fn get_placeholder_original_code(
        &self,
        placeholder_reference: &PlaceholderReference,
    ) -> JsString {
        self.get_placeholder_name_to_original_code_map()
            .get(placeholder_reference.stored_placeholder_name())
            .cloned()
            .unwrap_or_else(|| JsString::from("-"))
    }

    // port: JsMessage#getPlaceholderExample
    pub fn get_placeholder_example(
        &self,
        placeholder_reference: &PlaceholderReference,
    ) -> JsString {
        self.get_placeholder_name_to_example_map()
            .get(placeholder_reference.stored_placeholder_name())
            .cloned()
            .unwrap_or_else(|| JsString::from("-"))
    }

    // port: JsMessage#getGenderedMessageParts
    pub fn get_gendered_message_parts(&self, gender_case: GrammaticalGenderCase) -> &[Part] {
        if self.get_gendered_messages_map().is_empty() {
            // UnsupportedOperationException
            panic!("Message does not contain grammatical gendered variants.");
        }
        self.get_gendered_messages_map()
            .get(&gender_case)
            .map(Vec::as_slice)
            .unwrap_or_else(|| panic!("NullPointerException"))
    }

    /// Gets the list of grammatical gender cases for the message.
    // port: JsMessage#getGenderedMessageVariants
    pub fn get_gendered_message_variants(&self) -> Vec<GrammaticalGenderCase> {
        self.get_gendered_messages_map().keys().copied().collect()
    }

    /// Returns a single string representing the message, with placeholders as
    /// `{$jsPlaceholderName}`.
    // port: JsMessage#asJsMessageString()
    pub fn as_js_message_string(&self) -> JsString {
        if !self.get_gendered_messages_map().is_empty() {
            // UnsupportedOperationException
            panic!(
                "asJsMessageString() is not supported for messages with gendered variants. Please provide a grammatical gender case as an argument."
            );
        }
        let mut sb: Vec<u16> = Vec::new();
        for p in self.get_parts() {
            if p.is_placeholder() {
                sb.extend(PH_JS_PREFIX.encode_utf16());
                sb.extend_from_slice(p.get_js_placeholder_name().as_units());
                sb.extend(PH_JS_SUFFIX.encode_utf16());
            } else {
                sb.extend_from_slice(p.get_string().as_units());
            }
        }

        JsString::from_units(sb)
    }

    /// Returns a single string representing the message for a given gender case.
    // port: JsMessage#asJsMessageString(GrammaticalGenderCase)
    pub fn as_js_message_string_for_gender(&self, gender_case: GrammaticalGenderCase) -> JsString {
        if !self.get_gendered_messages_map().contains_key(&gender_case) {
            // IllegalArgumentException
            panic!("No message for grammtical gender case: {gender_case}");
        }
        let mut sb: Vec<u16> = Vec::new();
        for p in &self.get_gendered_messages_map()[&gender_case] {
            if p.is_placeholder() {
                sb.extend(PH_JS_PREFIX.encode_utf16());
                sb.extend_from_slice(p.get_js_placeholder_name().as_units());
                sb.extend(PH_JS_SUFFIX.encode_utf16());
            } else {
                sb.extend_from_slice(p.get_string().as_units());
            }
        }
        JsString::from_units(sb)
    }

    /// Returns a single string representing the message, with placeholders as
    /// `{CANONICAL_PLACEHOLDER_NAME}`.
    // port: JsMessage#asIcuMessageString()
    pub fn as_icu_message_string(&self) -> JsString {
        if !self.get_gendered_messages_map().is_empty() {
            // UnsupportedOperationException
            panic!(
                "asIcuMessageString() is not supported for messages with gendered variants. Please provide a grammatical gender case as an argument."
            );
        }
        let mut sb: Vec<u16> = Vec::new();
        for p in self.get_parts() {
            if p.is_placeholder() {
                sb.push(u16::from(b'{'));
                sb.extend_from_slice(p.get_canonical_placeholder_name().as_units());
                sb.push(u16::from(b'}'));
            } else {
                sb.extend_from_slice(p.get_string().as_units());
            }
        }
        JsString::from_units(sb)
    }

    /// Returns a single string representing the message for a given gender case, in ICU form.
    // port: JsMessage#asIcuMessageString(GrammaticalGenderCase)
    pub fn as_icu_message_string_for_gender(&self, gender_case: GrammaticalGenderCase) -> JsString {
        if !self.get_gendered_messages_map().contains_key(&gender_case) {
            // IllegalArgumentException
            panic!("No message for grammtical gender case: {gender_case}");
        }
        let mut sb: Vec<u16> = Vec::new();
        for p in &self.get_gendered_messages_map()[&gender_case] {
            if p.is_placeholder() {
                sb.push(u16::from(b'{'));
                sb.extend_from_slice(p.get_canonical_placeholder_name().as_units());
                sb.push(u16::from(b'}'));
            } else {
                sb.extend_from_slice(p.get_string().as_units());
            }
        }
        JsString::from_units(sb)
    }

    /// Returns false iff the message is represented by empty string.
    // port: JsMessage#isEmpty
    pub fn is_empty(&self) -> bool {
        if self.get_gendered_messages_map().is_empty() {
            for part in self.get_parts() {
                if part.is_placeholder() || part.get_string().length() > 0 {
                    return false;
                }
            }
        } else {
            for parts in self.get_gendered_messages_map().values() {
                for part in parts {
                    if part.is_placeholder() || part.get_string().length() > 0 {
                        return false;
                    }
                }
            }
        }

        true
    }
}

/// `interface Part`: either a literal string part or a placeholder reference.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    StringPart(StringPart),
    PlaceholderReference(PlaceholderReference),
}

impl Part {
    /// True for placeholders, false for literal string parts.
    // port: JsMessage.Part#isPlaceholder
    pub fn is_placeholder(&self) -> bool {
        match self {
            Part::StringPart(p) => p.is_placeholder(),
            Part::PlaceholderReference(p) => p.is_placeholder(),
        }
    }

    /// Gets the name of the placeholder as it would appear in JS code.
    // port: JsMessage.Part#getJsPlaceholderName
    pub fn get_js_placeholder_name(&self) -> JsString {
        match self {
            Part::StringPart(p) => p.get_js_placeholder_name(),
            Part::PlaceholderReference(p) => p.get_js_placeholder_name(),
        }
    }

    /// Gets the name of the placeholder as it would appear in XMB or XTB files.
    // port: JsMessage.Part#getCanonicalPlaceholderName
    pub fn get_canonical_placeholder_name(&self) -> JsString {
        match self {
            Part::StringPart(p) => p.get_canonical_placeholder_name(),
            Part::PlaceholderReference(p) => p.get_canonical_placeholder_name(),
        }
    }

    /// Gets the literal string for this message part.
    // port: JsMessage.Part#getString
    pub fn get_string(&self) -> JsString {
        match self {
            Part::StringPart(p) => p.get_string(),
            Part::PlaceholderReference(p) => p.get_string(),
        }
    }
}

impl From<StringPart> for Part {
    fn from(part: StringPart) -> Self {
        Part::StringPart(part)
    }
}

impl From<PlaceholderReference> for Part {
    fn from(part: PlaceholderReference) -> Self {
        Part::PlaceholderReference(part)
    }
}

/// Represents a literal string part of a message.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StringPart {
    string: JsString,
}

impl StringPart {
    // port: JsMessage.StringPart#StringPart
    pub fn new(string: JsString) -> Self {
        Self { string }
    }

    // port: JsMessage.StringPart#string
    pub fn string(&self) -> &JsString {
        &self.string
    }

    // port: JsMessage.StringPart#getString
    pub fn get_string(&self) -> JsString {
        self.string().clone()
    }

    // port: JsMessage.StringPart#create
    pub fn create(str: impl Into<JsString>) -> StringPart {
        StringPart::new(str.into())
    }

    // port: JsMessage.StringPart#isPlaceholder
    pub fn is_placeholder(&self) -> bool {
        false
    }

    // port: JsMessage.StringPart#getJsPlaceholderName
    pub fn get_js_placeholder_name(&self) -> JsString {
        // UnsupportedOperationException
        panic!("not a placeholder: '{}'", self.string());
    }

    // port: JsMessage.StringPart#getCanonicalPlaceholderName
    pub fn get_canonical_placeholder_name(&self) -> JsString {
        // UnsupportedOperationException
        panic!("not a placeholder: '{}'", self.string());
    }
}

/// In JS code we expect placeholder names to be lowerCamelCase with an optional _123_456 suffix.
// port: JsMessage#JS_PLACEHOLDER_NAME_RE
static JS_PLACEHOLDER_NAME_RE: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("[a-z][a-zA-Z\\d]*[_\\d]*"));

/// Returns whether a string is nonempty, begins with a lowercase letter, and contains only digits
/// and underscores after the first underscore.
// port: JsMessage#isLowerCamelCaseWithNumericSuffixes
pub fn is_lower_camel_case_with_numeric_suffixes(input: &JsString) -> bool {
    JS_PLACEHOLDER_NAME_RE.matcher(input.clone()).matches()
}

/// Converts the given string from upper-underscore case to lower-camel case, preserving numeric
/// suffixes. For example: "NAME" -> "name" "A4_LETTER" -> "a4Letter" "START_SPAN_1_23" ->
/// "startSpan_1_23".
// port: JsMessage#toLowerCamelCaseWithNumericSuffixes
pub fn to_lower_camel_case_with_numeric_suffixes(input: &JsString) -> JsString {
    // Determine where the numeric suffixes begin
    let mut suffix_start = input.length();
    while suffix_start > 0 {
        let mut ch: u16 = 0;
        let mut number_start = suffix_start;
        while number_start > 0 {
            ch = input.char_at(number_start - 1);
            if java_lang::is_digit(ch) {
                number_start -= 1;
            } else {
                break;
            }
        }
        if (number_start > 0) && (number_start < suffix_start) && (ch == u16::from(b'_')) {
            suffix_start = number_start - 1;
        } else {
            break;
        }
    }

    if suffix_start == input.length() {
        case_format::upper_underscore_to_lower_camel(input)
    } else {
        case_format::upper_underscore_to_lower_camel(&input.substring(0, suffix_start))
            .concat(&input.substring_from(suffix_start))
    }
}

/// In XMB and XTB files we expect the placeholder name to be in UPPER_SNAKE_CASE with optional
/// _123_456 suffix.
// port: JsMessage#CANONICAL_PLACEHOLDER_NAME_RE
static CANONICAL_PLACEHOLDER_NAME_RE: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("[A-Z\\d_]*"));

/// Is the name in the canonical format for placeholder names in XTB and XMB files.
// port: JsMessage#isCanonicalPlaceholderNameFormat
pub fn is_canonical_placeholder_name_format(name: &JsString) -> bool {
    CANONICAL_PLACEHOLDER_NAME_RE
        .matcher(name.clone())
        .matches()
}

/// A reference to a placeholder in a translatable message.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlaceholderReference {
    stored_placeholder_name: JsString,
    canonical_format: bool,
}

impl PlaceholderReference {
    // port: JsMessage.PlaceholderReference#PlaceholderReference
    pub fn new(stored_placeholder_name: JsString, canonical_format: bool) -> Self {
        Self {
            stored_placeholder_name,
            canonical_format,
        }
    }

    // port: JsMessage.PlaceholderReference#storedPlaceholderName
    pub fn stored_placeholder_name(&self) -> &JsString {
        &self.stored_placeholder_name
    }

    // port: JsMessage.PlaceholderReference#canonicalFormat
    pub fn canonical_format(&self) -> bool {
        self.canonical_format
    }

    // port: JsMessage.PlaceholderReference#getStoredPlaceholderName
    pub fn get_stored_placeholder_name(&self) -> &JsString {
        self.stored_placeholder_name()
    }

    // port: JsMessage.PlaceholderReference#isCanonicalFormat
    pub fn is_canonical_format(&self) -> bool {
        self.canonical_format()
    }

    // port: JsMessage.PlaceholderReference#createForJsName
    pub fn create_for_js_name(name: impl Into<JsString>) -> PlaceholderReference {
        let name = name.into();
        check_argument!(
            is_lower_camel_case_with_numeric_suffixes(&name),
            "invalid JS placeholder name format: '%s'",
            name
        );
        PlaceholderReference::new(name, /* canonical_format= */ false)
    }

    // port: JsMessage.PlaceholderReference#createForCanonicalName
    pub fn create_for_canonical_name(name: impl Into<JsString>) -> PlaceholderReference {
        let name = name.into();
        check_argument!(
            is_canonical_placeholder_name_format(&name),
            "not a canonical placeholder name format: '%s'",
            name
        );
        PlaceholderReference::new(name, /* canonical_format= */ true)
    }

    // port: JsMessage.PlaceholderReference#isPlaceholder
    pub fn is_placeholder(&self) -> bool {
        true
    }

    // port: JsMessage.PlaceholderReference#getJsPlaceholderName
    pub fn get_js_placeholder_name(&self) -> JsString {
        let stored_placeholder_name = self.stored_placeholder_name();
        if self.canonical_format() {
            to_lower_camel_case_with_numeric_suffixes(stored_placeholder_name)
        } else {
            stored_placeholder_name.clone()
        }
    }

    // port: JsMessage.PlaceholderReference#getCanonicalPlaceholderName
    pub fn get_canonical_placeholder_name(&self) -> JsString {
        let stored_placeholder_name = self.stored_placeholder_name();
        if self.canonical_format() {
            stored_placeholder_name.clone()
        } else {
            case_format::lower_camel_to_upper_underscore(stored_placeholder_name)
        }
    }

    // port: JsMessage.PlaceholderReference#getString
    pub fn get_string(&self) -> JsString {
        // UnsupportedOperationException
        panic!("not a string part: '{}'", self.get_js_placeholder_name());
    }
}

/// Contains functionality for creating JS messages.
#[derive(Clone, Debug, Default)]
pub struct Builder {
    key: Option<JsString>,

    meaning: Option<JsString>,

    desc: Option<JsString>,
    is_anonymous: bool,
    is_external: bool,

    id: Option<JsString>,
    alternate_id: Option<JsString>,

    parts: Vec<Part>,
    gendered_message_map: IndexMap<GrammaticalGenderCase, Vec<Part>>,
    // Placeholder names in JS code format (lowerCamelCase),
    // which is used for `goog.getMsg()` messages
    js_placeholder_names: IndexSet<JsString>,
    // Placeholder names in canonical format (UPPER_SNAKE_CASE)
    // which is used in XMB / XTB files and `declareIcuTemplate()` messages.
    canonical_placeholder_names: IndexSet<JsString>,
    placeholder_name_to_example_map: IndexMap<JsString, JsString>,
    placeholder_name_to_original_code_map: IndexMap<JsString, JsString>,

    source_name: Option<String>,
}

impl Builder {
    // port: JsMessage.Builder#Builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the message's key (e.g. `"MSG_HELLO"`).
    // port: JsMessage.Builder#getKey
    pub fn get_key(&self) -> Option<&JsString> {
        self.key.as_ref()
    }

    // port: JsMessage.Builder#setKey
    pub fn set_key(&mut self, key: impl Into<JsString>) -> &mut Self {
        self.key = Some(key.into());
        self
    }

    // port: JsMessage.Builder#setSourceName
    pub fn set_source_name(&mut self, source_name: Option<String>) -> &mut Self {
        self.source_name = source_name;
        self
    }

    // port: JsMessage.Builder#appendPart
    pub fn append_part(&mut self, part: Part) -> &mut Self {
        self.parts.push(part.clone());
        if part.is_placeholder() {
            self.js_placeholder_names
                .insert(part.get_js_placeholder_name());
            self.canonical_placeholder_names
                .insert(part.get_canonical_placeholder_name());
        }
        self
    }

    // port: JsMessage.Builder#appendParts
    pub fn append_parts(&mut self, parts: &[Part]) -> &mut Self {
        for part in parts {
            self.append_part(part.clone());
        }
        self
    }

    /// Appends a placeholder reference to the message.
    // port: JsMessage.Builder#appendJsPlaceholderReference(String)
    pub fn append_js_placeholder_reference(&mut self, name: impl Into<JsString>) -> &mut Self {
        let name = name.into();
        self.parts.push(Part::PlaceholderReference(
            PlaceholderReference::create_for_js_name(name.clone()),
        ));
        self.js_placeholder_names.insert(name);
        self
    }

    /// Appends a placeholder reference to the corresponding gendered message.
    // port: JsMessage.Builder#appendJsPlaceholderReference(GrammaticalGenderCase,String)
    pub fn append_js_placeholder_reference_for_gender(
        &mut self,
        gender_case: GrammaticalGenderCase,
        name: impl Into<JsString>,
    ) -> &mut Self {
        let name = name.into();
        let placeholder = PlaceholderReference::create_for_js_name(name.clone());
        check_not_null!(self.gendered_message_map.get_mut(&gender_case))
            .push(Part::PlaceholderReference(placeholder));
        self.js_placeholder_names.insert(name);
        self
    }

    /// Appends a placeholder reference to the message.
    // port: JsMessage.Builder#appendCanonicalPlaceholderReference(String)
    pub fn append_canonical_placeholder_reference(
        &mut self,
        name: impl Into<JsString>,
    ) -> &mut Self {
        let placeholder = PlaceholderReference::create_for_canonical_name(name);
        let js_name = placeholder.get_js_placeholder_name();
        self.parts.push(Part::PlaceholderReference(placeholder));
        self.js_placeholder_names.insert(js_name);
        self
    }

    /// Appends a placeholder reference to the corresponding gendered message.
    // port: JsMessage.Builder#appendCanonicalPlaceholderReference(GrammaticalGenderCase,String)
    pub fn append_canonical_placeholder_reference_for_gender(
        &mut self,
        gender_case: GrammaticalGenderCase,
        name: impl Into<JsString>,
    ) -> &mut Self {
        let placeholder = PlaceholderReference::create_for_canonical_name(name);
        let js_name = placeholder.get_js_placeholder_name();
        check_not_null!(self.gendered_message_map.get_mut(&gender_case))
            .push(Part::PlaceholderReference(placeholder));
        self.js_placeholder_names.insert(js_name);
        self
    }

    /// Appends a translatable string literal to the message.
    // port: JsMessage.Builder#appendStringPart(String)
    pub fn append_string_part(&mut self, part: impl Into<JsString>) -> &mut Self {
        self.parts.push(Part::StringPart(StringPart::create(part)));
        self
    }

    /// Appends a translatable string literal to the corresponding gendered message.
    // port: JsMessage.Builder#appendStringPart(GrammaticalGenderCase,String)
    pub fn append_string_part_for_gender(
        &mut self,
        gender_case: GrammaticalGenderCase,
        part: impl Into<JsString>,
    ) -> &mut Self {
        check_not_null!(self.gendered_message_map.get_mut(&gender_case))
            .push(Part::StringPart(StringPart::create(part)));
        self
    }

    /// Adds a gendered message key to genderedMessageParts map.
    // port: JsMessage.Builder#addGenderedMessageKey
    pub fn add_gendered_message_key(&mut self, part: GrammaticalGenderCase) -> &mut Self {
        self.gendered_message_map.insert(part, Vec::new());
        self
    }

    // port: JsMessage.Builder#setPlaceholderNameToExampleMap
    pub fn set_placeholder_name_to_example_map(
        &mut self,
        map: IndexMap<JsString, JsString>,
    ) -> &mut Self {
        self.placeholder_name_to_example_map = map;
        self
    }

    // port: JsMessage.Builder#setPlaceholderNameToOriginalCodeMap
    pub fn set_placeholder_name_to_original_code_map(
        &mut self,
        map: IndexMap<JsString, JsString>,
    ) -> &mut Self {
        self.placeholder_name_to_original_code_map = map;
        self
    }

    /// Sets the description of the message, which helps translators.
    // port: JsMessage.Builder#setDesc
    pub fn set_desc(&mut self, desc: Option<JsString>) -> &mut Self {
        self.desc = desc;
        self
    }

    /// Sets the programmer-specified meaning of this message.
    // port: JsMessage.Builder#setMeaning
    pub fn set_meaning(&mut self, meaning: Option<JsString>) -> &mut Self {
        self.meaning = meaning;
        self
    }

    /// Sets the alternate message ID, to be used if the primary ID is not yet translated.
    // port: JsMessage.Builder#setAlternateId
    pub fn set_alternate_id(&mut self, alternate_id: Option<JsString>) -> &mut Self {
        self.alternate_id = alternate_id;
        self
    }

    // port: JsMessage.Builder#setIsAnonymous
    pub fn set_is_anonymous(&mut self, is_anonymous: bool) -> &mut Self {
        self.is_anonymous = is_anonymous;
        self
    }

    // port: JsMessage.Builder#setId
    pub fn set_id(&mut self, id: impl Into<JsString>) -> &mut Self {
        let id = id.into();
        check_state!(
            self.id.is_none(),
            "id already set to '%s': cannot change it to '%s'",
            java_string_value_of(self.id.as_ref()),
            id
        );
        self.id = Some(id);
        self
    }

    // port: JsMessage.Builder#setIsExternalMsg
    pub fn set_is_external_msg(&mut self, is_external_msg: bool) -> &mut Self {
        self.is_external = is_external_msg;
        self
    }

    /// Gets whether at least one part has been appended.
    // port: JsMessage.Builder#hasParts
    pub fn has_parts(&self) -> bool {
        !self.parts.is_empty()
    }

    /// Gets whether the message has gendered variants.
    // port: JsMessage.Builder#hasGenderedVariants
    pub fn has_gendered_variants(&self) -> bool {
        !self.gendered_message_map.is_empty()
    }

    // port: JsMessage.Builder#getParts
    pub fn get_parts(&self) -> &[Part] {
        &self.parts
    }

    // port: JsMessage.Builder#build
    pub fn build(&mut self) -> JsMessage {
        let key = check_not_null!(self.key.clone(), "key has not been set");
        let id = check_not_null!(self.id.clone(), "id has not been set");
        check_state!(
            !self.is_external || !self.is_anonymous,
            "a message cannot be both anonymous and external"
        );

        // An alternate ID that points to itself is a no-op, so just omit it.
        if self.alternate_id.as_ref().is_some_and(|a| *a == id) {
            self.alternate_id = None;
        }

        let mut immutable_gendered_message_map: IndexMap<GrammaticalGenderCase, Vec<Part>> =
            IndexMap::<_, _>::default();
        for (k, v) in &self.gendered_message_map {
            immutable_gendered_message_map.insert(*k, v.clone());
        }

        JsMessage::new(
            self.source_name.clone(),
            key,
            self.is_anonymous,
            self.is_external,
            id,
            self.parts.clone(),
            immutable_gendered_message_map,
            self.alternate_id.clone(),
            self.desc.clone(),
            self.meaning.clone(),
            self.placeholder_name_to_example_map.clone(),
            self.placeholder_name_to_original_code_map.clone(),
            self.js_placeholder_names.clone(),
            self.canonical_placeholder_names.clone(),
        )
    }
}

/// Java's `String.valueOf(Object)` for a nullable string (`"null"` for null).
fn java_string_value_of(s: Option<&JsString>) -> String {
    match s {
        Some(s) => s.to_string(),
        None => "null".to_string(),
    }
}

/// This class contains routines for hashing (Bob Jenkins' evahash, 64 bit).
pub struct Hash;

// port: JsMessage.Hash#SEED64
const SEED64: i64 = 0x2b992ddfa23249d6_u64 as i64; // part of pi, arbitrary

// port: JsMessage.Hash#CONSTANT64
const CONSTANT64: i64 = 0xe08c1d668b756f82_u64 as i64; // part of golden ratio, arbitrary

/// Java's `>>>` on `long`.
fn urs64(x: i64, n: u32) -> i64 {
    ((x as u64) >> n) as i64
}

impl Hash {
    /// Hash a string to a 64 bit value. The digits of pi are used for the hash seed.
    // port: JsMessage.Hash#hash64(String)
    pub fn hash64(value: Option<&JsString>) -> i64 {
        Self::hash64_with_seed(value, SEED64)
    }

    // port: JsMessage.Hash#hash64(String,long)
    fn hash64_with_seed(value: Option<&JsString>, seed: i64) -> i64 {
        match value {
            None => Self::hash64_bytes_range(&[], 0, 0, seed),
            Some(value) => Self::hash64_bytes(&utf_8::encode(value), seed),
        }
    }

    // port: JsMessage.Hash#hash64(byte[],long)
    fn hash64_bytes(value: &[u8], seed: i64) -> i64 {
        Self::hash64_bytes_range(value, 0, value.len() as i32, seed)
    }

    // port: JsMessage.Hash#hash64(byte[],int,int,long)
    fn hash64_bytes_range(value: &[u8], offset: i32, length: i32, seed: i64) -> i64 {
        let mut offset = offset as usize;
        let mut a: i64 = CONSTANT64;
        let mut b: i64 = a;
        let mut c: i64 = seed;
        let mut keylen: i32 = length;

        // Java's byte & 0xffL
        let ub = |i: usize| -> i64 { i64::from(value[i]) };

        while keylen >= 24 {
            a = a.wrapping_add(Self::word64_at(value, offset));
            b = b.wrapping_add(Self::word64_at(value, offset + 8));
            c = c.wrapping_add(Self::word64_at(value, offset + 16));

            // Mix
            a = a.wrapping_sub(b);
            a = a.wrapping_sub(c);
            a ^= urs64(c, 43);
            b = b.wrapping_sub(c);
            b = b.wrapping_sub(a);
            b ^= a << 9;
            c = c.wrapping_sub(a);
            c = c.wrapping_sub(b);
            c ^= urs64(b, 8);
            a = a.wrapping_sub(b);
            a = a.wrapping_sub(c);
            a ^= urs64(c, 38);
            b = b.wrapping_sub(c);
            b = b.wrapping_sub(a);
            b ^= a << 23;
            c = c.wrapping_sub(a);
            c = c.wrapping_sub(b);
            c ^= urs64(b, 5);
            a = a.wrapping_sub(b);
            a = a.wrapping_sub(c);
            a ^= urs64(c, 35);
            b = b.wrapping_sub(c);
            b = b.wrapping_sub(a);
            b ^= a << 49;
            c = c.wrapping_sub(a);
            c = c.wrapping_sub(b);
            c ^= urs64(b, 11);
            a = a.wrapping_sub(b);
            a = a.wrapping_sub(c);
            a ^= urs64(c, 12);
            b = b.wrapping_sub(c);
            b = b.wrapping_sub(a);
            b ^= a << 18;
            c = c.wrapping_sub(a);
            c = c.wrapping_sub(b);
            c ^= urs64(b, 22);

            keylen -= 24;
            offset += 24;
        }

        c = c.wrapping_add(i64::from(length));
        if keylen >= 16 {
            if keylen == 23 {
                // Java: ((long) value[offset + 22]) << 56 (sign-extended byte)
                c = c.wrapping_add(i64::from(value[offset + 22] as i8) << 56);
            }
            if keylen >= 22 {
                c = c.wrapping_add(ub(offset + 21) << 48);
            }
            if keylen >= 21 {
                c = c.wrapping_add(ub(offset + 20) << 40);
            }
            if keylen >= 20 {
                c = c.wrapping_add(ub(offset + 19) << 32);
            }
            if keylen >= 19 {
                c = c.wrapping_add(ub(offset + 18) << 24);
            }
            if keylen >= 18 {
                c = c.wrapping_add(ub(offset + 17) << 16);
            }
            if keylen >= 17 {
                c = c.wrapping_add(ub(offset + 16) << 8);
                // the first byte of c is reserved for the length
            }
            if keylen >= 16 {
                b = b.wrapping_add(Self::word64_at(value, offset + 8));
                a = a.wrapping_add(Self::word64_at(value, offset));
            }
        } else if keylen >= 8 {
            if keylen == 15 {
                b = b.wrapping_add(ub(offset + 14) << 48);
            }
            if keylen >= 14 {
                b = b.wrapping_add(ub(offset + 13) << 40);
            }
            if keylen >= 13 {
                b = b.wrapping_add(ub(offset + 12) << 32);
            }
            if keylen >= 12 {
                b = b.wrapping_add(ub(offset + 11) << 24);
            }
            if keylen >= 11 {
                b = b.wrapping_add(ub(offset + 10) << 16);
            }
            if keylen >= 10 {
                b = b.wrapping_add(ub(offset + 9) << 8);
            }
            if keylen >= 9 {
                b = b.wrapping_add(ub(offset + 8));
            }
            if keylen >= 8 {
                a = a.wrapping_add(Self::word64_at(value, offset));
            }
        } else {
            if keylen == 7 {
                a = a.wrapping_add(ub(offset + 6) << 48);
            }
            if keylen >= 6 {
                a = a.wrapping_add(ub(offset + 5) << 40);
            }
            if keylen >= 5 {
                a = a.wrapping_add(ub(offset + 4) << 32);
            }
            if keylen >= 4 {
                a = a.wrapping_add(ub(offset + 3) << 24);
            }
            if keylen >= 3 {
                a = a.wrapping_add(ub(offset + 2) << 16);
            }
            if keylen >= 2 {
                a = a.wrapping_add(ub(offset + 1) << 8);
            }
            if keylen >= 1 {
                a = a.wrapping_add(ub(offset));
                // case 0: nothing left to add
            }
        }
        Self::mix64(a, b, c)
    }

    // port: JsMessage.Hash#word64At
    fn word64_at(bytes: &[u8], offset: usize) -> i64 {
        let ub = |i: usize| -> i64 { i64::from(bytes[i]) };
        ub(offset)
            .wrapping_add(ub(offset + 1) << 8)
            .wrapping_add(ub(offset + 2) << 16)
            .wrapping_add(ub(offset + 3) << 24)
            .wrapping_add(ub(offset + 4) << 32)
            .wrapping_add(ub(offset + 5) << 40)
            .wrapping_add(ub(offset + 6) << 48)
            .wrapping_add(ub(offset + 7) << 56)
    }

    /// Mixes longs a, b, and c, and returns the final value of c.
    // port: JsMessage.Hash#mix64
    fn mix64(mut a: i64, mut b: i64, mut c: i64) -> i64 {
        a = a.wrapping_sub(b);
        a = a.wrapping_sub(c);
        a ^= urs64(c, 43);
        b = b.wrapping_sub(c);
        b = b.wrapping_sub(a);
        b ^= a << 9;
        c = c.wrapping_sub(a);
        c = c.wrapping_sub(b);
        c ^= urs64(b, 8);
        a = a.wrapping_sub(b);
        a = a.wrapping_sub(c);
        a ^= urs64(c, 38);
        b = b.wrapping_sub(c);
        b = b.wrapping_sub(a);
        b ^= a << 23;
        c = c.wrapping_sub(a);
        c = c.wrapping_sub(b);
        c ^= urs64(b, 5);
        a = a.wrapping_sub(b);
        a = a.wrapping_sub(c);
        a ^= urs64(c, 35);
        b = b.wrapping_sub(c);
        b = b.wrapping_sub(a);
        b ^= a << 49;
        c = c.wrapping_sub(a);
        c = c.wrapping_sub(b);
        c ^= urs64(b, 11);
        a = a.wrapping_sub(b);
        a = a.wrapping_sub(c);
        a ^= urs64(c, 12);
        b = b.wrapping_sub(c);
        b = b.wrapping_sub(a);
        b ^= a << 18;
        c = c.wrapping_sub(a);
        c = c.wrapping_sub(b);
        c ^= urs64(b, 22);
        let _ = (a, b);
        c
    }
}

/// ID generator.
pub trait IdGenerator: Send + Sync + fmt::Debug {
    /// Generate the ID for the message. Messages with the same messageParts and meaning will get
    /// the same id. Messages with the same id will get the same translation.
    // port: JsMessage.IdGenerator#generateId
    fn generate_id(&self, meaning: &JsString, message_parts: &[Part]) -> JsString;
}

/// The subset of Guava's `CaseFormat` that `JsMessage` uses (`UPPER_UNDERSCORE` <->
/// `LOWER_CAMEL`), on UTF-16 code units like Guava's `String` code.
mod case_format {
    use closure_rhino::js_string::JsString;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum CaseFormat {
        LOWER_CAMEL,
        UPPER_UNDERSCORE,
    }

    impl CaseFormat {
        // port: CaseFormat#wordBoundary (CharMatcher.indexIn)
        fn word_boundary_index_in(self, s: &[u16], start: usize) -> Option<usize> {
            (start..s.len()).find(|&i| match self {
                CaseFormat::LOWER_CAMEL => (u16::from(b'A')..=u16::from(b'Z')).contains(&s[i]),
                CaseFormat::UPPER_UNDERSCORE => s[i] == u16::from(b'_'),
            })
        }

        // port: CaseFormat#wordSeparator
        fn word_separator(self) -> &'static [u16] {
            match self {
                CaseFormat::LOWER_CAMEL => &[],
                CaseFormat::UPPER_UNDERSCORE => &[b'_' as u16],
            }
        }

        // port: CaseFormat#normalizeWord
        fn normalize_word(self, word: &[u16]) -> Vec<u16> {
            match self {
                CaseFormat::LOWER_CAMEL => first_char_only_to_upper(word),
                CaseFormat::UPPER_UNDERSCORE => ascii_to_upper_case(word),
            }
        }

        // port: CaseFormat#normalizeFirstWord
        fn normalize_first_word(self, word: &[u16]) -> Vec<u16> {
            match self {
                CaseFormat::LOWER_CAMEL => ascii_to_lower_case(word),
                CaseFormat::UPPER_UNDERSCORE => self.normalize_word(word),
            }
        }

        // port: CaseFormat#to
        fn to(self, format: CaseFormat, str: &[u16]) -> Vec<u16> {
            if format == self {
                str.to_vec()
            } else {
                self.convert(format, str)
            }
        }

        // port: CaseFormat#convert (UPPER_UNDERSCORE's override only special-cases
        // LOWER_HYPHEN and LOWER_UNDERSCORE, which are not used here)
        fn convert(self, format: CaseFormat, s: &[u16]) -> Vec<u16> {
            // deal with camel conversion
            let mut out: Vec<u16> = Vec::new();
            let mut i: usize = 0;
            let mut j: i64 = -1;
            loop {
                j += 1;
                match self.word_boundary_index_in(s, j as usize) {
                    Some(found) => j = found as i64,
                    None => break,
                }
                let ju = j as usize;
                if i == 0 {
                    // include some extra space for separators
                    out = Vec::with_capacity(s.len() + 4 * format.word_separator().len());
                    out.extend(format.normalize_first_word(&s[i..ju]));
                } else {
                    out.extend(format.normalize_word(&s[i..ju]));
                }
                out.extend_from_slice(format.word_separator());
                i = ju + self.word_separator().len();
            }
            if i == 0 {
                format.normalize_first_word(s)
            } else {
                out.extend(format.normalize_word(&s[i..]));
                out
            }
        }
    }

    // port: CaseFormat#firstCharOnlyToUpper
    fn first_char_only_to_upper(word: &[u16]) -> Vec<u16> {
        if word.is_empty() {
            Vec::new()
        } else {
            let mut out = ascii_to_upper_case(&word[..1]);
            out.extend(ascii_to_lower_case(&word[1..]));
            out
        }
    }

    // port: Ascii#toUpperCase(String)
    fn ascii_to_upper_case(s: &[u16]) -> Vec<u16> {
        s.iter()
            .map(|&c| {
                if (u16::from(b'a')..=u16::from(b'z')).contains(&c) {
                    c ^ 0x20
                } else {
                    c
                }
            })
            .collect()
    }

    // port: Ascii#toLowerCase(String)
    fn ascii_to_lower_case(s: &[u16]) -> Vec<u16> {
        s.iter()
            .map(|&c| {
                if (u16::from(b'A')..=u16::from(b'Z')).contains(&c) {
                    c ^ 0x20
                } else {
                    c
                }
            })
            .collect()
    }

    /// `CaseFormat.UPPER_UNDERSCORE.to(CaseFormat.LOWER_CAMEL, s)`.
    pub(super) fn upper_underscore_to_lower_camel(s: &JsString) -> JsString {
        JsString::from_units(CaseFormat::UPPER_UNDERSCORE.to(CaseFormat::LOWER_CAMEL, s.as_units()))
    }

    /// `CaseFormat.LOWER_CAMEL.to(CaseFormat.UPPER_UNDERSCORE, s)`.
    pub(super) fn lower_camel_to_upper_underscore(s: &JsString) -> JsString {
        JsString::from_units(CaseFormat::LOWER_CAMEL.to(CaseFormat::UPPER_UNDERSCORE, s.as_units()))
    }
}
