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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/XtbMessageBundle.java.

//! Port of `com.google.javascript.jscomp.XtbMessageBundle`: a `MessageBundle` that parses
//! messages from an XML Translation Bundle (XTB) file.

use std::io::Read;
use std::sync::{Arc, LazyLock};

use closure_rhino::fx_hash::IndexMap;
use closure_rhino::java_lang::pattern::Pattern;
use closure_rhino::java_lang::sax_parser::{self, Attributes, ContentHandler, SaxParseException};
use closure_rhino::js_string::JsString;
use closure_rhino::{check_not_null, check_state};

use crate::google_js_message_id_generator::GoogleJsMessageIdGenerator;
use crate::js_message::{Builder, GrammaticalGenderCase, IdGenerator, JsMessage};
use crate::message_bundle::MessageBundle;

/// The exception `XtbMessageBundle`'s constructor wraps in a `RuntimeException`: an I/O error
/// reading the stream or the `SAXParseException` of a malformed document.
#[derive(Debug)]
pub enum XtbMessageBundleError {
    Io(std::io::Error),
    Sax(SaxParseException),
}

impl std::fmt::Display for XtbMessageBundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XtbMessageBundleError::Io(e) => write!(f, "{e}"),
            XtbMessageBundleError::Sax(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for XtbMessageBundleError {}

/// A MessageBundle that parses messages from an XML Translation Bundle (XTB) file.
pub struct XtbMessageBundle {
    messages: IndexMap<JsString, JsMessage>,
    id_generator: Arc<dyn IdGenerator>,
}

impl XtbMessageBundle {
    /// Creates an instance and initializes it with the messages in an XTB file.
    ///
    /// `xtb` is the XTB file as a byte stream, `project_id` the translation console project id
    /// (i.e. name).
    // port: XtbMessageBundle#XtbMessageBundle
    pub fn new(
        mut xtb: impl Read,
        project_id: Option<&str>,
    ) -> Result<XtbMessageBundle, XtbMessageBundleError> {
        let mut messages = IndexMap::<_, _>::default();
        let id_generator: Arc<dyn IdGenerator> = Arc::new(GoogleJsMessageIdGenerator::new(
            project_id.map(str::to_string),
        ));

        // Use a SAX parser for speed and less memory usage.
        //
        // createSAXParser(): a SAXParserFactory parser, not validating, not XInclude aware,
        // external general and parameter entities and the external DTD not loaded, secure
        // processing on, NOOP_RESOLVER as its EntityResolver. `sax_parser` implements exactly
        // that configuration. The resolver is never consulted in it (no external entity or DTD
        // is ever loaded), so SecureEntityResolver#resolveEntity has no Rust counterpart.
        // port: XtbMessageBundle#createSAXParser
        let mut bytes = Vec::new();
        xtb.read_to_end(&mut bytes)
            .map_err(XtbMessageBundleError::Io)?;
        let mut content_handler = Handler {
            messages: &mut messages,
            gender_case: None,
            lang: None,
            msg_builder: None,
        };
        sax_parser::parse(&bytes, &mut content_handler).map_err(XtbMessageBundleError::Sax)?;
        Ok(XtbMessageBundle {
            messages,
            id_generator,
        })
    }
}

impl MessageBundle for XtbMessageBundle {
    // port: XtbMessageBundle#getMessage
    fn get_message(&self, id: &JsString) -> Option<&JsMessage> {
        self.messages.get(id)
    }

    // port: XtbMessageBundle#idGenerator
    fn id_generator(&self) -> Option<Arc<dyn IdGenerator>> {
        Some(self.id_generator.clone())
    }

    // port: XtbMessageBundle#getAllMessages
    fn get_all_messages(&self) -> Vec<&JsMessage> {
        self.messages.values().collect()
    }
}

const BUNDLE_ELEM_NAME: &str = "translationbundle";
const LANG_ATT_NAME: &str = "lang";

const TRANSLATION_ELEM_NAME: &str = "translation";
const MESSAGE_ID_ATT_NAME: &str = "id";

const PLACEHOLDER_ELEM_NAME: &str = "ph";
const PLACEHOLDER_NAME_ATT_NAME: &str = "name";

const BRANCH_ELEM_NAME: &str = "branch";
const BRANCH_NAME_ATT_NAME: &str = "variants";
const MALE_GENDER_CASE: &str = "MASCULINE";
const FEMALE_GENDER_CASE: &str = "FEMININE";
const NEUTER_GENDER_CASE: &str = "NEUTER";
const OTHER_GENDER_CASE: &str = "OTHER";

const GENDER_VARIANT_PATTERN: &str = "grammatical_gender_case:\\s*(\\w+)";
static GET_STRING_PATTERN: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile(GENDER_VARIANT_PATTERN));

/// A `ContentHandler` that creates a `JsMessage` for each message parsed from an XML
/// Translation Bundle (XTB) file.
// port: XtbMessageBundle.Handler
struct Handler<'a> {
    messages: &'a mut IndexMap<JsString, JsMessage>,
    gender_case: Option<String>,

    lang: Option<JsString>,
    msg_builder: Option<Builder>,
}

impl Handler<'_> {
    fn gender_case(&self) -> GrammaticalGenderCase {
        GrammaticalGenderCase::value_of(self.gender_case.as_deref())
    }
}

impl ContentHandler for Handler<'_> {
    // port: XtbMessageBundle.Handler#setDocumentLocator
    fn set_document_locator(&mut self) {}

    // port: XtbMessageBundle.Handler#startDocument
    fn start_document(&mut self) {}

    // port: XtbMessageBundle.Handler#endDocument
    fn end_document(&mut self) {}

    // port: XtbMessageBundle.Handler#startPrefixMapping
    fn start_prefix_mapping(&mut self, _prefix: &str, _uri: &str) {}

    // port: XtbMessageBundle.Handler#endPrefixMapping
    fn end_prefix_mapping(&mut self, _prefix: &str) {}

    // port: XtbMessageBundle.Handler#startElement
    fn start_element(&mut self, _uri: &str, _local_name: &str, q_name: &str, atts: &Attributes) {
        match q_name {
            BUNDLE_ELEM_NAME => {
                check_state!(self.lang.is_none());
                self.lang = atts.get_value(LANG_ATT_NAME).cloned();
                check_state!(self.lang.as_ref().is_some_and(|lang| !lang.is_empty()));
            }
            TRANSLATION_ELEM_NAME => {
                check_state!(self.msg_builder.is_none());
                let id = atts.get_value(MESSAGE_ID_ATT_NAME).cloned();
                check_state!(id.as_ref().is_some_and(|id| !id.is_empty()));
                let id = id.unwrap();
                let mut builder = Builder::new();
                builder.set_key(id.clone()).set_id(id);
                self.msg_builder = Some(builder);
            }
            PLACEHOLDER_ELEM_NAME => {
                check_state!(self.msg_builder.is_some());
                let ph_ref = check_not_null!(atts.get_value(PLACEHOLDER_NAME_ATT_NAME).cloned());
                if self.msg_builder.as_ref().unwrap().has_gendered_variants() {
                    let gender_case = self.gender_case();
                    self.msg_builder
                        .as_mut()
                        .unwrap()
                        .append_canonical_placeholder_reference_for_gender(gender_case, ph_ref);
                } else {
                    self.msg_builder
                        .as_mut()
                        .unwrap()
                        .append_canonical_placeholder_reference(ph_ref);
                }
            }
            BRANCH_ELEM_NAME => {
                check_state!(self.msg_builder.is_some());
                let gender =
                    check_not_null!(atts.get_value(BRANCH_NAME_ATT_NAME)).to_string_lossy();
                // Gender case must be one of the following: MALE, FEMALE, NEUTER, or OTHER.

                let mut matcher = GET_STRING_PATTERN.matcher(&gender);
                if matcher.find() {
                    let gender_case = matcher.group(1).unwrap().to_string();
                    self.gender_case = Some(gender_case.clone());
                    check_state!(
                        gender_case == MALE_GENDER_CASE
                            || gender_case == FEMALE_GENDER_CASE
                            || gender_case == NEUTER_GENDER_CASE
                            || gender_case == OTHER_GENDER_CASE,
                        "Gender case must be one of the following: MASCULINE, FEMININE, NEUTER, or \
                         OTHER."
                    );
                    let gender_case = self.gender_case();
                    self.msg_builder
                        .as_mut()
                        .unwrap()
                        .add_gendered_message_key(gender_case);
                }
            }
            _ => {}
        }
    }

    // port: XtbMessageBundle.Handler#endElement
    fn end_element(&mut self, _uri: &str, _local_name: &str, q_name: &str) {
        if TRANSLATION_ELEM_NAME == q_name {
            check_state!(self.msg_builder.is_some());
            let msg_builder = self.msg_builder.as_mut().unwrap();
            if !msg_builder.has_parts() {
                msg_builder.append_string_part("");
            }
            let key = msg_builder.get_key().unwrap().clone();
            let message = msg_builder.build();
            self.messages.insert(key, message);
            self.msg_builder = None;
        }
    }

    // port: XtbMessageBundle.Handler#characters
    fn characters(&mut self, ch: &[u16]) {
        if let Some(msg_builder) = &mut self.msg_builder {
            let part = JsString::from_units(ch.to_vec());
            // Append a string literal to the message.
            if msg_builder.has_gendered_variants() {
                msg_builder.append_string_part_for_gender(
                    GrammaticalGenderCase::value_of(self.gender_case.as_deref()),
                    part,
                );
            } else {
                msg_builder.append_string_part(part);
            }
        }
    }

    // port: XtbMessageBundle.Handler#ignorableWhitespace
    fn ignorable_whitespace(&mut self, ch: &[u16]) {
        if let Some(msg_builder) = &mut self.msg_builder {
            // Preserve whitespace in messages.
            if msg_builder.has_gendered_variants() {
                msg_builder.append_string_part_for_gender(
                    GrammaticalGenderCase::value_of(self.gender_case.as_deref()),
                    JsString::from_units(ch.to_vec()),
                );
            } else {
                msg_builder.append_string_part(JsString::from_units(ch.to_vec()));
            }
        }
    }

    // port: XtbMessageBundle.Handler#processingInstruction
    fn processing_instruction(&mut self, _target: &str, _data: &JsString) {}

    // port: XtbMessageBundle.Handler#skippedEntity
    fn skipped_entity(&mut self, _name: &str) {}
}
