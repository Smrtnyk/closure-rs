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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/JsMessageVisitor.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `com.google.javascript.jscomp.JsMessageVisitor`: locates JS code that is intended to
//! declare localizable messages.
//!
//! It passes each found message to either `process_js_message_definition` for `goog.getMsg()`
//! calls or `process_icu_template_definition` for `goog.i18n.messages.declareIcuTemplate()` calls.
//!
//! Java's abstract class becomes the trait [`JsMessageVisitor`] (its abstract and overridable
//! methods) plus the struct [`JsMessageVisitorBase`] (its private fields), which every subclass
//! owns and exposes through `js_message_visitor_base`. The base class's own method bodies are the
//! free functions of this module that take `this: &mut dyn JsMessageVisitor`; a subclass that
//! overrides `process` calls [`process`] where Java calls `super.process(externs, root)`.

use std::sync::{Arc, LazyLock};

use closure_rhino::java_lang::regex::Pattern;
use closure_rhino::js_string::JsString;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_rhino::{check_not_null, check_state};
use indexmap::{IndexMap, IndexSet};

use crate::abstract_compiler::AbstractCompiler;
use crate::diagnostic_type::DiagnosticType;
use crate::icu_template_definition::IcuTemplateDefinition;
use crate::js_error::JSError;
use crate::js_message::{
    self, Builder, Hash, IdGenerator, JsMessage, PH_JS_PREFIX, PH_JS_SUFFIX, Part,
    PlaceholderFormatException, PlaceholderReference, StringPart,
};
use crate::js_message_definition::JsMessageDefinition;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::var::Var;

// port: JsMessageVisitor#MSG_FUNCTION_NAME
const MSG_FUNCTION_NAME: &str = "getMsg";
// port: JsMessageVisitor#MSG_FUNCTION_QNAME
const MSG_FUNCTION_QNAME: &str = "goog.getMsg";
// port: JsMessageVisitor#ICU_MSG_FUNCTION_NAME
const ICU_MSG_FUNCTION_NAME: &str = "declareIcuTemplate";
// port: JsMessageVisitor#ICU_MSG_FUNCTION_QNAME (used by MESSAGE_NOT_INITIALIZED_CORRECTLY)
#[allow(dead_code)]
const ICU_MSG_FUNCTION_QNAME: &str = "goog.i18n.messages.declareIcuTemplate";
// port: JsMessageVisitor#MSG_FALLBACK_FUNCTION_NAME
const MSG_FALLBACK_FUNCTION_NAME: &str = "goog.getMsgWithFallback";

/// Expose a list of functions that are used in message extraction. Standalone tools that run
/// message extraction can use this list to filter out files that do not contain any of these
/// functions.
// port: JsMessageVisitor#FUNCTIONS_USED_IN_MESSAGE_EXTRACTION
pub const FUNCTIONS_USED_IN_MESSAGE_EXTRACTION: [&str; 2] =
    [MSG_FUNCTION_NAME, ICU_MSG_FUNCTION_NAME];

/// Identifies a message with a specific ID which doesn't get extracted from the JS code containing
/// its declaration.
// port: JsMessageVisitor#MSG_EXTERNAL_PREFIX
const MSG_EXTERNAL_PREFIX: &str = "MSG_EXTERNAL_";

// port: JsMessageVisitor#MESSAGE_HAS_NO_DESCRIPTION
pub static MESSAGE_HAS_NO_DESCRIPTION: DiagnosticType = DiagnosticType::warning(
    "JSC_MSG_HAS_NO_DESCRIPTION",
    "Message {0} has no description. Add @desc JsDoc tag.",
);

// port: JsMessageVisitor#MESSAGE_HAS_NO_TEXT
pub static MESSAGE_HAS_NO_TEXT: DiagnosticType = DiagnosticType::warning(
    "JSC_MSG_HAS_NO_TEXT",
    "Message value of {0} is just an empty string. Empty messages are forbidden.",
);

// port: JsMessageVisitor#MESSAGE_TREE_MALFORMED
pub static MESSAGE_TREE_MALFORMED: DiagnosticType = DiagnosticType::error(
    "JSC_MSG_TREE_MALFORMED",
    "Message parse tree malformed. {0}",
);

// port: JsMessageVisitor#MESSAGE_HAS_NO_VALUE
pub static MESSAGE_HAS_NO_VALUE: DiagnosticType =
    DiagnosticType::error("JSC_MSG_HAS_NO_VALUE", "message node {0} has no value");

// port: JsMessageVisitor#MESSAGE_DUPLICATE_KEY
pub static MESSAGE_DUPLICATE_KEY: DiagnosticType = DiagnosticType::error(
    "JSC_MSG_KEY_DUPLICATED",
    "duplicate message variable name found for {0}, initial definition {1}:{2}",
);

// port: JsMessageVisitor#MESSAGE_NODE_IS_ORPHANED
pub static MESSAGE_NODE_IS_ORPHANED: DiagnosticType = DiagnosticType::error(
    "JSC_MSG_ORPHANED_NODE",
    "{0}() function may be used only with MSG_* property or variable",
);

// port: JsMessageVisitor#MESSAGE_NOT_INITIALIZED_CORRECTLY
pub static MESSAGE_NOT_INITIALIZED_CORRECTLY: DiagnosticType = DiagnosticType::warning(
    "JSC_MSG_NOT_INITIALIZED_CORRECTLY",
    "Message must be initialized using a call to goog.getMsg or goog.i18n.messages.declareIcuTemplate",
);

// port: JsMessageVisitor#BAD_FALLBACK_SYNTAX
pub static BAD_FALLBACK_SYNTAX: DiagnosticType = DiagnosticType::error(
    "JSC_MSG_BAD_FALLBACK_SYNTAX",
    "Bad syntax. Expected syntax: goog.getMsgWithFallback(MSG_1, MSG_2)",
);

// port: JsMessageVisitor#FALLBACK_ARG_ERROR
pub static FALLBACK_ARG_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_MSG_FALLBACK_ARG_ERROR",
    "Could not find message entry for fallback argument {0}",
);

// port: JsMessageVisitor#MSG_PREFIX
pub const MSG_PREFIX: &str = "MSG_";

/// ScopedAliases pass transforms the goog.scope declarations to have a unique id as prefix.
///
/// After '$jscomp$scope$' expect some sequence of digits and dollar signs ending in '$MSG_
// port: JsMessageVisitor#SCOPED_ALIASES_PREFIX_PATTERN
static SCOPED_ALIASES_PREFIX_PATTERN: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("\\$jscomp\\$scope\\$\\S+\\$MSG_"));

/// Pattern for unnamed messages.
///
/// Soy generates messages with names MSG_UNNAMED.* . This pattern recognizes such messages.
// port: JsMessageVisitor#MSG_UNNAMED_PATTERN
static MSG_UNNAMED_PATTERN: LazyLock<Pattern> = LazyLock::new(|| Pattern::compile("MSG_UNNAMED.*"));

/// The private state of Java's abstract `JsMessageVisitor`.
///
/// Java's `final AbstractCompiler compiler` field is dropped (DESIGN.md §6): every method that
/// used it receives the compiler.
#[derive(Debug, Default)]
pub struct JsMessageVisitorBase {
    id_generator: Option<Arc<dyn IdGenerator>>,

    /// The names encountered associated with their defining node and source. We use it for
    /// tracking duplicated message ids in the source code.
    message_names: IndexMap<JsString, MessageLocation>,

    /// Track unnamed messages by Var, not string, as they are not guaranteed to be globally unique
    unnamed_messages: IndexMap<Var, JsMessage>,

    /// Set of found goog.getMsg call nodes.
    ///
    /// When we visit goog.getMsg() node we add it, and later when we visit its parent we remove
    /// it. All nodes that are left at the end of traversing are orphaned nodes. It means have no
    /// corresponding var or property node.
    goog_msg_nodes: IndexSet<NodeId>,
}

impl JsMessageVisitorBase {
    /// Creates JS message visitor.
    ///
    /// `id_generator`: generator that used for creating unique ID for the message
    // port: JsMessageVisitor#JsMessageVisitor
    pub fn new(id_generator: Option<Arc<dyn IdGenerator>>) -> Self {
        // TODO(anatol): add flag that decides whether to process UNNAMED messages.
        // Some projects would not want such functionality (unnamed) as they don't
        // use SOY templates.
        Self {
            id_generator,
            message_names: IndexMap::new(),
            unnamed_messages: IndexMap::new(),
            goog_msg_nodes: IndexSet::new(),
        }
    }
}

/// The abstract and overridable methods of Java's `JsMessageVisitor`.
pub trait JsMessageVisitor {
    /// Rust-only accessor for the fields of Java's abstract base class.
    fn js_message_visitor_base(&mut self) -> &mut JsMessageVisitorBase;

    /// Processes a found JS message that was defined with `goog.getMsg()`.
    // port: JsMessageVisitor#processJsMessageDefinition
    fn process_js_message_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        definition: Box<dyn JsMessageDefinition>,
    );

    /// Processes a found call to `goog.i18n.messages.declareIcuTemplate()`
    // port: JsMessageVisitor#processIcuTemplateDefinition
    fn process_icu_template_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        definition: Box<dyn IcuTemplateDefinition>,
    );

    /// Processes the goog.getMsgWithFallback primitive. goog.getMsgWithFallback(MSG_1, MSG_2);
    ///
    /// By default, does nothing.
    // port: JsMessageVisitor#processMessageFallback
    fn process_message_fallback(
        &mut self,
        _compiler: &mut AbstractCompiler,
        _call_node: NodeId,
        _message1: &JsMessage,
        _message2: &JsMessage,
    ) {
    }

    /// Returns whether the given JS identifier is a valid JS message name.
    // port: JsMessageVisitor#isMessageName
    fn is_message_name(&self, identifier: &JsString) -> bool {
        identifier.starts_with(&JsString::from(MSG_PREFIX)) || is_scoped_aliases_prefix(identifier)
    }
}

/// Java's `JsMessageVisitor extends AbstractPostOrderCallback`: the callback that [`process`]
/// traverses with.
struct JsMessageVisitorCallback<'a> {
    this: &'a mut dyn JsMessageVisitor,
}

impl Callback for JsMessageVisitorCallback<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        visit(self.this, t, n, parent);
    }
}

// port: JsMessageVisitor#process
pub fn process(
    this: &mut dyn JsMessageVisitor,
    compiler: &mut AbstractCompiler,
    _externs: Option<NodeId>,
    root: NodeId,
) {
    NodeTraversal::traverse(compiler, root, &mut JsMessageVisitorCallback { this });

    let goog_msg_nodes: Vec<NodeId> = this
        .js_message_visitor_base()
        .goog_msg_nodes
        .iter()
        .copied()
        .collect();
    for msg_node in goog_msg_nodes {
        let qualified_name = msg_node
            .get_first_child(compiler)
            .unwrap()
            .get_qualified_name(compiler);
        let qualified_name = java_string_value_of(qualified_name.as_ref());
        compiler.report(JSError::make(
            compiler,
            msg_node,
            &MESSAGE_NODE_IS_ORPHANED,
            &[&qualified_name],
        ));
    }
}

// port: JsMessageVisitor#visit
pub fn visit(
    this: &mut dyn JsMessageVisitor,
    traversal: &mut NodeTraversal<'_>,
    node: NodeId,
    _unused: Option<NodeId>,
) {
    collect_get_msg_call(this, traversal, node);
    check_message_initialization(this, traversal, node);
}

/// This method is called for every Node in the sources AST.
// port: JsMessageVisitor#checkMessageInitialization
fn check_message_initialization(
    this: &mut dyn JsMessageVisitor,
    traversal: &mut NodeTraversal<'_>,
    node: NodeId,
) {
    let parent = node.get_parent(traversal);

    let original_message_key: Option<JsString>;
    let possibly_obfuscated_message_key: JsString;
    let msg_node: Option<NodeId>;
    let js_doc_info: Option<Arc<JSDocInfo>>;

    match node.get_token(traversal) {
        Token::NAME => {
            // Case: `var MSG_HELLO = 'Message';`
            if parent.is_none() || !NodeUtil::is_name_declaration(traversal, parent) {
                return;
            }

            possibly_obfuscated_message_key = node.get_string(traversal);
            original_message_key = node.get_original_name(traversal);
            msg_node = node.get_first_child(traversal);
            js_doc_info = parent.unwrap().get_jsdoc_info(traversal);
        }
        Token::ASSIGN => {
            // Case: `somenamespace.someclass.MSG_HELLO = 'Message';`
            let get_prop = node.get_first_child(traversal).unwrap();
            if !get_prop.is_get_prop(traversal) {
                return;
            }

            possibly_obfuscated_message_key = get_prop.get_string(traversal);
            original_message_key = get_prop.get_original_name(traversal);
            msg_node = node.get_last_child(traversal);
            js_doc_info = node.get_jsdoc_info(traversal);
        }
        Token::STRING_KEY => {
            // Case: `var t = {MSG_HELLO: 'Message'}`;
            let parent_node = parent.unwrap();
            if node.is_quoted_string_key(traversal)
                || !node.has_children(traversal)
                || parent_node.is_object_pattern(traversal)
            {
                // Don't require goog.getMsg() for quoted keys
                // Case: `var msgs = { 'MSG_QUOTED': anything };`
                //
                // Don't try to require goog.getMsg() for destructuring assignment targets.
                // goog.getMsg() needs to be used in a direct assignment to a variable or property
                // only.
                // Case: `var {MSG_HELLO} = anything;
                // Case: `var {something: MSG_HELLO} = anything;
                return;
            }
            check_state!(
                parent_node.is_object_lit(traversal),
                "%s",
                parent_node.to_string(traversal)
            );

            possibly_obfuscated_message_key = node.get_string(traversal);
            original_message_key = node.get_original_name(traversal);
            msg_node = node.get_first_child(traversal);
            js_doc_info = node.get_jsdoc_info(traversal);
        }
        Token::MEMBER_FIELD_DEF => {
            // Case: `class Foo { MSG_HELLO = 'Message'; }`
            possibly_obfuscated_message_key = node.get_string(traversal);
            original_message_key = node.get_original_name(traversal);
            msg_node = node.get_first_child(traversal);
            js_doc_info = node.get_jsdoc_info(traversal);
        }
        _ => {
            return;
        }
    }

    let message_key_from_lhs = original_message_key
        .clone()
        .unwrap_or_else(|| possibly_obfuscated_message_key.clone());

    // If we've reached this point, then messageKey is the name of a variable or a property that is
    // being assigned a value and msgNode is the Node representing the value being assigned.
    // However, we haven't actually determined yet that name looks like it should be a translatable
    // message or that the value is a call to goog.getMsg().

    // Is this a message name?
    let msg_node_is_a_call = msg_node.is_some_and(|m| m.is_call(traversal));

    if !this.is_message_name(&message_key_from_lhs) {
        return;
    }

    let Some(msg_node) = msg_node else {
        let key = message_key_from_lhs.to_string();
        let compiler = traversal.get_compiler();
        compiler.report(JSError::make(
            compiler,
            node,
            &MESSAGE_HAS_NO_VALUE,
            &[&key],
        ));
        return;
    };

    if is_legal_message_var_alias(traversal, msg_node) {
        return;
    }

    // Report a warning if a qualified messageKey that looks like a message (e.g. "a.b.MSG_X")
    // doesn't use goog.getMsg().
    if msg_node_is_a_call {
        this.js_message_visitor_base()
            .goog_msg_nodes
            .shift_remove(&msg_node);
    } else {
        let compiler = traversal.get_compiler();
        compiler.report(JSError::make(
            compiler,
            node,
            &MESSAGE_NOT_INITIALIZED_CORRECTLY,
            &[],
        ));
        return;
    }

    let traversal_source_name = traversal.get_source_name();
    let lineno = node.get_lineno(traversal);
    let charno = node.get_charno(traversal);
    let mapping = traversal.get_compiler().get_source_mapping(
        traversal_source_name.as_deref(),
        lineno,
        charno,
    );
    let source_name: String = match mapping {
        Some(mapping) => format!(
            "{}:{}",
            mapping.get_original_file(),
            mapping.get_line_number()
        ),
        None => format!(
            "{}:{}",
            traversal_source_name.as_deref().unwrap_or("null"),
            lineno
        ),
    };

    check_state!(msg_node.is_call(traversal));
    let fn_name_node = msg_node.get_first_child(traversal).unwrap();
    let result: Result<(), MalformedException> = (|| {
        if is_declare_icu_template_callee(traversal, fn_name_node) {
            let icu_template_definition = extract_icu_template_definition(
                this,
                traversal,
                msg_node,
                js_doc_info.as_deref(),
                &message_key_from_lhs,
                &source_name,
            )?;
            let extracted_message = icu_template_definition.get_message().clone();
            track_message(
                this,
                traversal,
                &possibly_obfuscated_message_key,
                msg_node,
                &extracted_message,
            );
            report_error_if_empty_message(traversal.get_compiler(), node, &extracted_message);
            this.process_icu_template_definition(
                traversal.get_compiler(),
                Box::new(icu_template_definition),
            );
        } else if fn_name_node.matches_qualified_name(traversal, MSG_FUNCTION_QNAME) {
            let js_message_definition = extract_js_message_definition(
                this,
                traversal,
                msg_node,
                js_doc_info.as_deref(),
                &message_key_from_lhs,
                &source_name,
            )?;
            let extracted_message = js_message_definition.get_message().clone();
            track_message(
                this,
                traversal,
                &possibly_obfuscated_message_key,
                msg_node,
                &extracted_message,
            );
            report_error_if_empty_message(traversal.get_compiler(), node, &extracted_message);

            // goog.getMsg() calls are required to have `@desc` unless they are external.
            let desc = extracted_message.get_desc();
            if desc.is_none_or(java_trim_is_empty) && !extracted_message.is_external() {
                let key = extracted_message.get_key().to_string();
                let compiler = traversal.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    node,
                    &MESSAGE_HAS_NO_DESCRIPTION,
                    &[&key],
                ));
            }

            this.process_js_message_definition(
                traversal.get_compiler(),
                Box::new(js_message_definition),
            );
        } else {
            let message = format!(
                "Message must be initialized using a call to {MSG_FUNCTION_QNAME} or \
                 {ICU_MSG_FUNCTION_NAME} (from goog.i18n.messages)."
            );
            let compiler = traversal.get_compiler();
            compiler.report(JSError::make(
                compiler,
                msg_node,
                &MESSAGE_TREE_MALFORMED,
                &[&message],
            ));
        }
        Ok(())
    })();
    if let Err(ex) = result {
        let compiler = traversal.get_compiler();
        compiler.report(JSError::make(
            compiler,
            check_not_null!(ex.get_node()),
            &MESSAGE_TREE_MALFORMED,
            &[ex.get_message()],
        ));
    }
}

// port: JsMessageVisitor#isDeclareIcuTemplateCallee
fn is_declare_icu_template_callee(ast: &Ast, callee_node: NodeId) -> bool {
    // NOTE: We're requiring that the imported ICU template function name match its original name.
    // This is an intentional limitation, since it should be less confusing to users as well as
    // to this program.
    // TODO(bradfordcsmith): Make this check more robust, possibly building on ModuleMetadata or
    // ProcessClosurePrimitives.
    let qualified_name = callee_node.get_qualified_name(ast);
    qualified_name.is_some_and(|q| q.ends_with(&JsString::from(ICU_MSG_FUNCTION_NAME)))
}

/// Java's `interface InputForBuildJsMsg`.
trait InputForBuildJsMsg {
    // port: JsMessageVisitor.InputForBuildJsMsg#getMessageKeyFromLhs
    fn get_message_key_from_lhs(&self) -> &JsString;

    // port: JsMessageVisitor.InputForBuildJsMsg#getSourceName
    fn get_source_name(&self) -> &str;

    // port: JsMessageVisitor.InputForBuildJsMsg#getMessageText
    fn get_message_text(&self) -> &JsString;

    // port: JsMessageVisitor.InputForBuildJsMsg#getMessageParts
    fn get_message_parts(&self) -> &[Part];

    // port: JsMessageVisitor.InputForBuildJsMsg#getPlaceholderExampleMap
    fn get_placeholder_example_map(&self) -> &IndexMap<JsString, JsString>;

    // port: JsMessageVisitor.InputForBuildJsMsg#getPlaceholderOriginalCodeMap
    fn get_placeholder_original_code_map(&self) -> &IndexMap<JsString, JsString>;

    // port: JsMessageVisitor.InputForBuildJsMsg#getDescription
    fn get_description(&self) -> Option<&JsString>;

    // port: JsMessageVisitor.InputForBuildJsMsg#getMeaning
    fn get_meaning(&self) -> Option<&JsString>;

    // port: JsMessageVisitor.InputForBuildJsMsg#getAlternateMessageId
    fn get_alternate_message_id(&self) -> Option<&JsString>;
}

/// The anonymous `InputForBuildJsMsg` classes of `extractJsMessageDefinition` and
/// `extractIcuTemplateDefinition`: both only return the values they captured.
struct CapturedInputForBuildJsMsg<'a> {
    message_key_from_lhs: &'a JsString,
    source_name: &'a str,
    message_text: &'a JsString,
    message_parts: &'a [Part],
    placeholder_example_map: &'a IndexMap<JsString, JsString>,
    placeholder_original_code_map: &'a IndexMap<JsString, JsString>,
    description: Option<&'a JsString>,
    meaning: Option<&'a JsString>,
    alternate_message_id: Option<&'a JsString>,
}

impl InputForBuildJsMsg for CapturedInputForBuildJsMsg<'_> {
    fn get_message_key_from_lhs(&self) -> &JsString {
        self.message_key_from_lhs
    }

    fn get_source_name(&self) -> &str {
        self.source_name
    }

    fn get_message_text(&self) -> &JsString {
        self.message_text
    }

    fn get_message_parts(&self) -> &[Part] {
        self.message_parts
    }

    fn get_placeholder_example_map(&self) -> &IndexMap<JsString, JsString> {
        self.placeholder_example_map
    }

    fn get_placeholder_original_code_map(&self) -> &IndexMap<JsString, JsString> {
        self.placeholder_original_code_map
    }

    fn get_description(&self) -> Option<&JsString> {
        self.description
    }

    fn get_meaning(&self) -> Option<&JsString> {
        self.meaning
    }

    fn get_alternate_message_id(&self) -> Option<&JsString> {
        self.alternate_message_id
    }
}

// port: JsMessageVisitor#buildJsMessage
fn build_js_message(this: &mut dyn JsMessageVisitor, input: &dyn InputForBuildJsMsg) -> JsMessage {
    let message_parts = input.get_message_parts();
    let message_key_from_lhs = input.get_message_key_from_lhs();

    // non-null for `MSG_EXTERNAL_12345`
    let external_message_id = get_external_message_id(message_key_from_lhs);

    let is_anonymous: bool;
    let is_external: bool;
    let message_id: JsString;
    let message_key_final: JsString;
    if let Some(external_message_id) = external_message_id {
        // MSG_EXTERNAL_12345 = ...
        is_anonymous = false;
        is_external = true;
        message_id = external_message_id;
        message_key_final = message_key_from_lhs.clone();
    } else {
        is_external = false;
        // We will need to generate the message ID from a combination of the message text and
        // its "meaning". If the code explicitly specifies a "meaning" string we'll use that,
        // otherwise we'll use the message key as the meaning
        let mut meaning_for_id_generation: Option<JsString> = input.get_meaning().cloned();
        if is_unnamed_message_name(message_key_from_lhs) {
            // MSG_UNNAMED_XXXX = goog.getMsg(....);
            // JS code that is automatically generated uses this, since it is harder for it to
            // create message variable names that are guaranteed to be unique.
            is_anonymous = true;
            message_key_final = generate_key_from_message_text(input.get_message_text());
            if meaning_for_id_generation.is_none() {
                meaning_for_id_generation = Some(message_key_final.clone());
            }
        } else {
            is_anonymous = false;
            message_key_final = message_key_from_lhs.clone();
            if meaning_for_id_generation.is_none() {
                // Transpilation of goog.scope() may have added a prefix onto the variable name,
                // which we need to strip off when we treat it as a "meaning" for ID generation
                // purposes. Otherwise, the message ID would change if a goog.scope() call were
                // added or removed.
                meaning_for_id_generation =
                    Some(remove_scoped_aliases_prefix(message_key_from_lhs));
            }
        }
        let meaning_for_id_generation = meaning_for_id_generation.unwrap();
        message_id = match &this.js_message_visitor_base().id_generator {
            None => meaning_for_id_generation,
            Some(id_generator) => {
                id_generator.generate_id(&meaning_for_id_generation, message_parts)
            }
        };
    }

    Builder::new()
        .append_parts(message_parts)
        .set_placeholder_name_to_example_map(input.get_placeholder_example_map().clone())
        .set_placeholder_name_to_original_code_map(
            input.get_placeholder_original_code_map().clone(),
        )
        .set_is_anonymous(is_anonymous)
        .set_is_external_msg(is_external)
        .set_key(message_key_final)
        .set_source_name(Some(input.get_source_name().to_string()))
        .set_desc(input.get_description().cloned())
        // NOTE: JsMessageXmbWriter does NOT just take this value and write it to the XMB
        // file as the message meaning. It has its own code that defaults a null value to
        // the value of the key field, then prefixes that with a project ID, if any.
        // The intention of that seems to have been to make sure the meaning value in the XMB
        // file matches what was actually used by the ID generator above.
        .set_meaning(input.get_meaning().cloned())
        .set_alternate_id(input.get_alternate_message_id().cloned())
        .set_id(message_id)
        .build()
}

// port: JsMessageVisitor#trackMessage
fn track_message(
    this: &mut dyn JsMessageVisitor,
    traversal: &mut NodeTraversal<'_>,
    possibly_obfuscated_message_key: &JsString,
    msg_node: NodeId,
    extracted_message: &JsMessage,
) {
    // If asked to check named internal messages.
    if !extracted_message.is_anonymous() && !extracted_message.is_external() {
        check_if_message_duplicated(
            this,
            traversal.get_compiler(),
            extracted_message.get_key(),
            msg_node,
        );
    }
    if extracted_message.is_anonymous() {
        track_unnamed_message(
            this,
            traversal,
            extracted_message,
            possibly_obfuscated_message_key,
        );
    } else {
        track_normal_message(
            this,
            extracted_message,
            extracted_message.get_key(),
            msg_node,
        );
    }
}

// port: JsMessageVisitor#reportErrorIfEmptyMessage
fn report_error_if_empty_message(
    compiler: &mut AbstractCompiler,
    node: NodeId,
    extracted_message: &JsMessage,
) {
    if extracted_message.is_empty() {
        // value of the message is an empty string. Translators do not like it.
        let key = extracted_message.get_key().to_string();
        compiler.report(JSError::make(compiler, node, &MESSAGE_HAS_NO_TEXT, &[&key]));
    }
}

/// Extracts an external message ID from the message key, if it contains one.
///
/// Returns the external ID if it is found, otherwise `None`.
// port: JsMessageVisitor#getExternalMessageId
pub fn get_external_message_id(message_key: &JsString) -> Option<JsString> {
    let prefix = JsString::from(MSG_EXTERNAL_PREFIX);
    if message_key.starts_with(&prefix) {
        let start = prefix.length();
        let mut end = start;
        while end < message_key.length() {
            let c = message_key.char_at(end);
            if c > u16::from(b'9') || c < u16::from(b'0') {
                break;
            }
            end += 1;
        }
        if end > start {
            return Some(message_key.substring(start, end));
        }
    }
    None
}

// port: JsMessageVisitor#generateKeyFromMessageText
fn generate_key_from_message_text(msg_text: &JsString) -> JsString {
    let nonnegative_hash: i64 = i64::MAX & Hash::hash64(Some(msg_text));
    JsString::from(format!(
        "{MSG_PREFIX}{}",
        java_long_to_string(nonnegative_hash, 36).to_ascii_uppercase()
    ))
}

// port: JsMessageVisitor#collectGetMsgCall
fn collect_get_msg_call(
    this: &mut dyn JsMessageVisitor,
    traversal: &mut NodeTraversal<'_>,
    call: NodeId,
) {
    if !call.is_call(traversal) {
        return;
    }

    // goog.getMsg()
    let callee = call.get_first_child(traversal).unwrap();
    if callee.matches_qualified_name(traversal, MSG_FUNCTION_QNAME)
        || is_declare_icu_template_callee(traversal, callee)
    {
        this.js_message_visitor_base().goog_msg_nodes.insert(call);
    } else if callee.matches_qualified_name(traversal, MSG_FALLBACK_FUNCTION_NAME) {
        visit_fallback_function_call(this, traversal, call);
    }
}

/// Track a message for later retrieval.
///
/// This is used for tracking duplicates, and for figuring out message fallback. Not all message
/// types are trackable, because that would require a more sophisticated analysis. e.g., function
/// f(s) { s.MSG_UNNAMED_X = 'Some untrackable message'; }
// port: JsMessageVisitor#trackNormalMessage
fn track_normal_message(
    this: &mut dyn JsMessageVisitor,
    message: &JsMessage,
    msg_name: &JsString,
    msg_node: NodeId,
) {
    let location = MessageLocation::new(message.clone(), msg_node);
    this.js_message_visitor_base()
        .message_names
        .insert(msg_name.clone(), location);
}

/// Track an unnamed message for later retrieval.
///
/// This is used for figuring out message fallbacks. Message duplicates are allowed for unnamed
/// messages.
// port: JsMessageVisitor#trackUnnamedMessage
fn track_unnamed_message(
    this: &mut dyn JsMessageVisitor,
    t: &mut NodeTraversal<'_>,
    message: &JsMessage,
    msg_name_in_scope: &JsString,
) {
    let scope = t.get_scope();
    let var = scope.get_var(t.get_compiler(), msg_name_in_scope);
    if let Some(var) = var {
        this.js_message_visitor_base()
            .unnamed_messages
            .insert(var, message.clone());
    }
}

/// Defines any special cases that are exceptions to what would otherwise be illegal message
/// assignments.
///
/// These exceptions are generally due to the pass being designed before new syntax was
/// introduced.
///
/// `msg_node`: Node representing the value assigned to the message variable or property
// port: JsMessageVisitor#isLegalMessageVarAlias
fn is_legal_message_var_alias(ast: &Ast, msg_node: NodeId) -> bool {
    if msg_node.is_get_prop(ast)
        && msg_node.is_qualified_name(ast)
        && msg_node
            .get_string(ast)
            .starts_with(&JsString::from(MSG_PREFIX))
    {
        // Case: `foo.Thing.MSG_EXAMPLE_ALIAS = bar.OtherThing.MSG_EXAMPLE;`
        //
        // This kind of construct is created by TypeScript code generation and
        // ConcretizeStaticInheritanceForInlining. Just ignore it; the message will have already
        // been extracted
        // from the base class.
        return true;
    }

    if !msg_node.is_name(ast) {
        return false;
    }

    let original_name = msg_node
        .get_original_name(ast)
        .unwrap_or_else(|| msg_node.get_string(ast));

    if original_name.starts_with(&JsString::from(MSG_PREFIX)) {
        // Creating an alias for a message is also allowed, and sometimes happens in generated
        // code, including some of the code generated by this compiler's transpilations.
        // e.g.
        // `var MSG_EXAMPLE_ALIAS = MSG_EXAMPLE;`
        // `var {MSG_HELLO_ALIAS} = MSG_HELLO;
        // `var {MSG_HELLO_ALIAS: goog$module$my$module_MSG_HELLO} = x;`).
        // `exports = {MSG_FOO}`
        // or `exports = {MSG_FOO: MSG_FOO}` when used with declareLegacyNamespace.
        return true;
    }

    false
}

/// Get a previously tracked unnamed message
// port: JsMessageVisitor#getTrackedUnnamedMessage
fn get_tracked_unnamed_message(
    this: &mut dyn JsMessageVisitor,
    t: &mut NodeTraversal<'_>,
    msg_name_in_scope: &JsString,
) -> Option<JsMessage> {
    let scope = t.get_scope();
    let var = scope.get_var(t.get_compiler(), msg_name_in_scope);
    if let Some(var) = var {
        return this
            .js_message_visitor_base()
            .unnamed_messages
            .get(&var)
            .cloned();
    }
    None
}

/// Get a previously tracked message.
// port: JsMessageVisitor#getTrackedNormalMessage
fn get_tracked_normal_message(
    this: &mut dyn JsMessageVisitor,
    msg_name: &JsString,
) -> Option<JsMessage> {
    let location = this.js_message_visitor_base().message_names.get(msg_name);
    location.map(|location| location.message.clone())
}

/// Checks if message already processed. If so - it generates 'message duplicated' compiler error.
///
/// `msg_name`: the name of the message; `msg_node`: the node that represents JS message
// port: JsMessageVisitor#checkIfMessageDuplicated
fn check_if_message_duplicated(
    this: &mut dyn JsMessageVisitor,
    compiler: &mut AbstractCompiler,
    msg_name: &JsString,
    msg_node: NodeId,
) {
    let base = this.js_message_visitor_base();
    if base.message_names.contains_key(msg_name) {
        let location = &base.message_names[msg_name];
        let message_node = location.message_node;
        let name = msg_name.to_string();
        let source_file_name = message_node
            .get_source_file_name(compiler)
            .unwrap_or_else(|| "null".to_string());
        let lineno = message_node.get_lineno(compiler).to_string();
        compiler.report(JSError::make(
            compiler,
            msg_node,
            &MESSAGE_DUPLICATE_KEY,
            &[&name, &source_file_name, &lineno],
        ));
    }
}

/// Returns the string value associated with a node representing a JS string or several JS strings
/// added together (e.g. `'str'` or `'s' + 't' + 'r'`).
///
/// Returns `Err(MalformedException)` if the node is not a string literal or concatenation of them.
// port: JsMessageVisitor#extractStringFromStringExprNode
fn extract_string_from_string_expr_node(
    ast: &Ast,
    node: NodeId,
) -> Result<JsString, MalformedException> {
    match node.get_token(ast) {
        Token::STRINGLIT => Ok(node.get_string(ast)),
        Token::TEMPLATELIT => {
            if node.has_one_child(ast) {
                // Cooked string can be null only for tagged template literals.
                // A tagged template literal would hit the default case below.
                Ok(check_not_null!(
                    node.get_first_child(ast).unwrap().get_cooked_string(ast)
                ))
            } else {
                Err(MalformedException::new(
                    "Template literals with substitutions are not allowed.",
                    Some(node),
                ))
            }
        }
        Token::ADD => {
            let mut sb: Vec<u16> = Vec::new();
            let mut child = node.get_first_child(ast);
            while let Some(c) = child {
                sb.extend_from_slice(extract_string_from_string_expr_node(ast, c)?.as_units());
                child = c.get_next(ast);
            }
            Ok(JsString::from_units(sb))
        }
        _ => Err(MalformedException::new(
            "literal string or concatenation expected",
            Some(node),
        )),
    }
}

/// The anonymous `JsMessageDefinition` built by `extractJsMessageDefinition`.
struct ExtractedJsMessageDefinition {
    js_extracted_message: JsMessage,
    msg_node: NodeId,
    msg_text_node: NodeId,
    values_obj_lit: Option<NodeId>,
    placeholder_values_map: IndexMap<JsString, NodeId>,
    js_message_options: JsMessageOptions,
}

impl JsMessageDefinition for ExtractedJsMessageDefinition {
    fn get_message(&self) -> &JsMessage {
        &self.js_extracted_message
    }

    fn get_message_node(&self) -> NodeId {
        self.msg_node
    }

    fn get_template_text_node(&self) -> NodeId {
        self.msg_text_node
    }

    fn get_placeholder_values_node(&self) -> Option<NodeId> {
        self.values_obj_lit
    }

    fn get_placeholder_value_map(&self) -> &IndexMap<JsString, NodeId> {
        &self.placeholder_values_map
    }

    fn should_escape_less_than(&self) -> bool {
        self.js_message_options.is_escape_less_than()
    }

    fn should_unescape_html_entities(&self) -> bool {
        self.js_message_options.is_unescape_html_entities()
    }
}

/// Extract data from a call to `goog.getMsg(...)`.
///
/// Returns `Err(MalformedException)` if the code does not match the expected format.
// port: JsMessageVisitor#extractJsMessageDefinition
fn extract_js_message_definition(
    this: &mut dyn JsMessageVisitor,
    traversal: &mut NodeTraversal<'_>,
    msg_node: NodeId,
    js_doc_info: Option<&JSDocInfo>,
    message_key_from_lhs: &JsString,
    source_name: &str,
) -> Result<ExtractedJsMessageDefinition, MalformedException> {
    // Extract data from a call to `goog.getMsg(...)`
    // TODO(bradfordcsmith): Add these three fields to the options bag argument for goog.getMsg().
    // Specifying them there instead of in annotations will make them available to the uncompiled
    // `goog.getMsg()` call during runtime, which could enable runtime lookup of translated
    // messages in uncompiled code.
    let description: Option<JsString>;
    let meaning: Option<JsString>;
    let alternate_message_id: Option<JsString>;
    match js_doc_info {
        None => {
            description = None;
            meaning = None;
            alternate_message_id = None;
        }
        Some(js_doc_info) => {
            description = js_doc_info.get_description();
            meaning = js_doc_info.get_meaning();
            alternate_message_id = js_doc_info.get_alternate_message_id();
        }
    }

    // first child of the call is the `goog.getMsg` name
    // second is the message string value
    let Some(msg_text_node) = msg_node.get_second_child(traversal) else {
        return Err(MalformedException::new(
            "Message string literal expected",
            Some(msg_node),
        ));
    };
    // third is the optional object literal mapping placeholder names to value expressions
    let values_obj_lit = msg_text_node.get_next(traversal);
    // fourth is the optional options-bag argument
    let options_bag_argument = values_obj_lit.and_then(|v| v.get_next(traversal));

    let unexpected_argument = options_bag_argument.and_then(|o| o.get_next(traversal));
    if let Some(unexpected_argument) = unexpected_argument {
        return Err(MalformedException::new(
            "too many arguments",
            Some(unexpected_argument),
        ));
    }

    // Extract and parse the message text
    // The message string can be a string literal or a concatenation of string literals.
    // We want the whole thing as a single string here.
    let msg_text_string = extract_string_from_string_expr_node(traversal, msg_text_node)?;
    let goog_get_msg_parsed_text = match extract_goog_get_msg_parsed_text(&msg_text_string) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Err(MalformedException::new(
                e.get_message(),
                Some(msg_text_node),
            ));
        }
    };

    // Extract the placeholder values object
    // NOTE: The extract method returns an effectively "empty" value for `null`
    let mut placeholder_object_literal_map = extract_object_literal_map(traversal, values_obj_lit)?;

    // Confirm that we can find a value map entry for every placeholder name referenced in the
    // text.
    let placeholder_names_from_text = goog_get_msg_parsed_text.get_placeholder_names();
    placeholder_object_literal_map.check_for_required_keys(
        placeholder_names_from_text,
        &|placeholder_name: &JsString| {
            MalformedException::new(
                format!("Unrecognized message placeholder referenced: {placeholder_name}"),
                Some(msg_text_node),
            )
        },
    )?;

    // Confirm that every placeholder name for which we have a value is actually referenced in the
    // text.
    placeholder_object_literal_map.check_for_unexpected_keys(
        placeholder_names_from_text,
        &|placeholder_name: &JsString| format!("Unused message placeholder: {placeholder_name}"),
    )?;

    // Get a map from placeholder name to Node value
    let placeholder_values_map = placeholder_object_literal_map
        .extract_as_value_map(traversal)
        .clone();

    // Extract the Options bag data
    // NOTE: the extract method returns an "empty" value for `null`
    let mut js_message_options = extract_js_message_options(traversal, options_bag_argument)?;

    // Confirm that any example or orignal_code placeholder information we have refers only to
    // placeholder names that appear in the message text.
    js_message_options.check_for_unknown_placeholders(placeholder_names_from_text)?;

    let js_extracted_message = build_js_message(
        this,
        &CapturedInputForBuildJsMsg {
            message_key_from_lhs,
            source_name,
            message_text: goog_get_msg_parsed_text.get_text(),
            message_parts: goog_get_msg_parsed_text.get_parts(),
            placeholder_example_map: js_message_options.get_placeholder_example_map(),
            placeholder_original_code_map: js_message_options.get_placeholder_original_code_map(),
            description: description.as_ref(),
            meaning: meaning.as_ref(),
            alternate_message_id: alternate_message_id.as_ref(),
        },
    );

    Ok(ExtractedJsMessageDefinition {
        js_extracted_message,
        msg_node,
        msg_text_node,
        values_obj_lit,
        placeholder_values_map,
        js_message_options,
    })
}

/// The anonymous `IcuTemplateDefinition` built by `extractIcuTemplateDefinition`.
struct ExtractedIcuTemplateDefinition {
    extracted_message: JsMessage,
    msg_node: NodeId,
    string_literal_expression: NodeId,
}

impl IcuTemplateDefinition for ExtractedIcuTemplateDefinition {
    fn get_message(&self) -> &JsMessage {
        &self.extracted_message
    }

    fn get_message_node(&self) -> NodeId {
        self.msg_node
    }

    fn get_template_text_node(&self) -> NodeId {
        self.string_literal_expression
    }
}

/// Extract message data from a `declareIcuTemplate()` call.
// port: JsMessageVisitor#extractIcuTemplateDefinition
fn extract_icu_template_definition(
    this: &mut dyn JsMessageVisitor,
    traversal: &mut NodeTraversal<'_>,
    msg_node: NodeId,
    js_doc_info: Option<&JSDocInfo>,
    message_key_from_lhs: &JsString,
    source_name: &str,
) -> Result<ExtractedIcuTemplateDefinition, MalformedException> {
    if let Some(js_doc_info) = js_doc_info {
        // For declareIcuTemplateData() it is not valid to use the @desc, @meaning, and
        // @alternateMessageId annotations.
        // Instead, that information should go into the options bag parameter.
        if js_doc_info.get_alternate_message_id().is_some() {
            return Err(MalformedException::new(
                "Use the 'alternate_message_id' option, not the '@alternateMessageId' annotation",
                Some(msg_node),
            ));
        }
        if js_doc_info.get_description().is_some() {
            return Err(MalformedException::new(
                "Use the 'description' option, not the '@desc' annotation",
                Some(msg_node),
            ));
        }
        if js_doc_info.get_meaning().is_some() {
            return Err(MalformedException::new(
                "Use the 'meaning' option, not the '@meaning' annotation",
                Some(msg_node),
            ));
        }
    }

    // The first argument is the message string
    let Some(string_literal_expression) = msg_node.get_second_child(traversal) else {
        return Err(MalformedException::new(
            "message string argument expected",
            Some(msg_node),
        ));
    };
    let icu_message_template_string =
        extract_icu_message_template_string(traversal, string_literal_expression)?;

    // The second argument is the options bag
    let Some(options_bag_node) = string_literal_expression.get_next(traversal) else {
        return Err(MalformedException::new(
            "options argument expected",
            Some(msg_node),
        ));
    };
    let mut icu_template_options = extract_icu_template_options(traversal, options_bag_node)?;

    // Parsing of the template string into parts depends on which placeholders are mentioned
    // in the options. Placeholders that do not have example or original_code text  will not be
    // extracted from the template as separate parts.
    let extracted_icu_template_parts =
        icu_message_template_string.extract_parts(icu_template_options.get_placeholder_names());
    icu_template_options.check_for_unknown_placeholders(
        &extracted_icu_template_parts.extracted_placeholder_names,
    )?;

    // There shouldn't be another argument
    let unexpected_argument = options_bag_node.get_next(traversal);
    if let Some(unexpected_argument) = unexpected_argument {
        return Err(MalformedException::new(
            "too many arguments",
            Some(unexpected_argument),
        ));
    }

    let extracted_message = build_js_message(
        this,
        &CapturedInputForBuildJsMsg {
            message_key_from_lhs,
            source_name,
            message_text: &icu_message_template_string.template,
            message_parts: &extracted_icu_template_parts.extracted_parts,
            placeholder_example_map: icu_template_options.get_placeholder_example_map(),
            placeholder_original_code_map: icu_template_options.get_placeholder_original_code_map(),
            description: Some(icu_template_options.get_description()),
            meaning: icu_template_options.get_meaning(),
            alternate_message_id: icu_template_options.get_alternate_message_id(),
        },
    );

    Ok(ExtractedIcuTemplateDefinition {
        extracted_message,
        msg_node,
        string_literal_expression,
    })
}

// port: JsMessageVisitor#ICU_PLACEHOLDER_RE
static ICU_PLACEHOLDER_RE: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("\\{([A-Z_0-9]+)\\}"));

/// `@VisibleForTesting static final class IcuMessageTemplateString`.
#[derive(Clone, Debug)]
pub struct IcuMessageTemplateString {
    pub template: JsString,
}

impl IcuMessageTemplateString {
    // port: JsMessageVisitor.IcuMessageTemplateString#IcuMessageTemplateString
    pub fn new(template: impl Into<JsString>) -> Self {
        Self {
            template: template.into(),
        }
    }

    /// Split the template string into plain string parts and placeholders.
    ///
    /// Although this method will recognize all ICU placeholders in the string ("{NAME}"), it will
    /// only extract as separate parts those that are named in `placholder_names_to_extract`.
    /// Those are the ones for which we will need to create placeholder elements, so we can attach
    /// example text or original code snippets to them when we put the message into an XMB file.
    // port: JsMessageVisitor.IcuMessageTemplateString#extractParts
    pub fn extract_parts(
        &self,
        placholder_names_to_extract: &IndexSet<JsString>,
    ) -> ExtractedIcuTemplateParts {
        let mut extracted_placeholder_names: IndexSet<JsString> = IndexSet::new();
        let mut parts_builder: Vec<Part> = Vec::new();
        let mut matcher = ICU_PLACEHOLDER_RE.matcher(self.template.clone());
        let mut remaining_template_start_index: usize = 0;
        while matcher.find() {
            let placeholder_name = matcher.group_units(1).unwrap();
            if placholder_names_to_extract.contains(&placeholder_name) {
                extracted_placeholder_names.insert(placeholder_name.clone());
                let non_placeholder_prefix = self
                    .template
                    .substring(remaining_template_start_index, matcher.start());
                remaining_template_start_index = matcher.end();
                parts_builder.push(Part::StringPart(StringPart::create(non_placeholder_prefix)));
                parts_builder.push(Part::PlaceholderReference(
                    PlaceholderReference::create_for_canonical_name(placeholder_name),
                ));
            } // else we have no need to create a separate part for this placeholder.
        }
        if remaining_template_start_index < self.template.length() {
            let remaining_template = self.template.substring_from(remaining_template_start_index);
            parts_builder.push(Part::StringPart(StringPart::create(remaining_template)));
        }
        ExtractedIcuTemplateParts::new(extracted_placeholder_names, parts_builder)
    }
}

/// `static class ExtractedIcuTemplateParts`.
#[derive(Clone, Debug)]
pub struct ExtractedIcuTemplateParts {
    pub extracted_placeholder_names: IndexSet<JsString>,
    pub extracted_parts: Vec<Part>,
}

impl ExtractedIcuTemplateParts {
    // port: JsMessageVisitor.ExtractedIcuTemplateParts#ExtractedIcuTemplateParts
    pub fn new(
        extracted_placeholder_names: IndexSet<JsString>,
        extracted_parts: Vec<Part>,
    ) -> Self {
        Self {
            extracted_placeholder_names,
            extracted_parts,
        }
    }
}

// port: JsMessageVisitor#extractIcuMessageTemplateString
fn extract_icu_message_template_string(
    ast: &Ast,
    string_expression: NodeId,
) -> Result<IcuMessageTemplateString, MalformedException> {
    let template_string = extract_string_from_string_expr_node(ast, string_expression)?;
    Ok(IcuMessageTemplateString::new(template_string))
}

/// Java's private `interface JsMessageOptions` and its one anonymous implementation in
/// `extractJsMessageOptions`.
struct JsMessageOptions {
    is_escape_less_than: bool,
    is_unescape_html_entities: bool,
    placeholder_examples_map: IndexMap<JsString, JsString>,
    placeholder_original_code_map: IndexMap<JsString, JsString>,
    example_object_literal_map: ObjectLiteralMapImpl,
    original_code_object_literal_map: ObjectLiteralMapImpl,
}

impl JsMessageOptions {
    // Replace `'<'` with `'&lt;'` in the message.
    // port: JsMessageVisitor.JsMessageOptions#isEscapeLessThan
    fn is_escape_less_than(&self) -> bool {
        self.is_escape_less_than
    }

    // Replace these escaped entities with their literal characters in the message
    // (Overrides escapeLessThan)
    // '&lt;' -> '<'
    // '&gt;' -> '>'
    // '&apos;' -> "'"
    // '&quot;' -> '"'
    // '&amp;' -> '&'
    // port: JsMessageVisitor.JsMessageOptions#isUnescapeHtmlEntities
    fn is_unescape_html_entities(&self) -> bool {
        self.is_unescape_html_entities
    }

    // port: JsMessageVisitor.JsMessageOptions#getPlaceholderExampleMap
    fn get_placeholder_example_map(&self) -> &IndexMap<JsString, JsString> {
        &self.placeholder_examples_map
    }

    // port: JsMessageVisitor.JsMessageOptions#getPlaceholderOriginalCodeMap
    fn get_placeholder_original_code_map(&self) -> &IndexMap<JsString, JsString> {
        &self.placeholder_original_code_map
    }

    // port: JsMessageVisitor.JsMessageOptions#checkForUnknownPlaceholders
    fn check_for_unknown_placeholders(
        &mut self,
        known_placeholders: &IndexSet<JsString>,
    ) -> Result<(), MalformedException> {
        self.example_object_literal_map
            .check_for_unexpected_keys(known_placeholders, &|unknown_name: &JsString| {
                format!("Unknown placeholder: {unknown_name}")
            })?;
        self.original_code_object_literal_map
            .check_for_unexpected_keys(known_placeholders, &|unknown_name: &JsString| {
                format!("Unknown placeholder: {unknown_name}")
            })
    }
}

// port: JsMessageVisitor#MESSAGE_OPTION_NAMES
static MESSAGE_OPTION_NAMES: LazyLock<IndexSet<JsString>> = LazyLock::new(|| {
    ["html", "unescapeHtmlEntities", "example", "original_code"]
        .into_iter()
        .map(JsString::from)
        .collect()
});

// port: JsMessageVisitor#extractJsMessageOptions
fn extract_js_message_options(
    ast: &Ast,
    options_bag: Option<NodeId>,
) -> Result<JsMessageOptions, MalformedException> {
    let mut object_literal_map = extract_object_literal_map(ast, options_bag)?;
    object_literal_map
        .check_for_unexpected_keys(&MESSAGE_OPTION_NAMES, &|option_name: &JsString| {
            format!("Unknown option: {option_name}")
        })?;

    let is_escape_less_than = object_literal_map.get_boolean_value_or_false(ast, "html")?;
    let is_unescape_html_entities =
        object_literal_map.get_boolean_value_or_false(ast, "unescapeHtmlEntities")?;

    let example_value_node = object_literal_map.get_value_node(ast, "example");
    let mut example_object_literal_map = extract_object_literal_map(ast, example_value_node)?;
    let placeholder_examples_map = example_object_literal_map
        .extract_as_string_to_string_map(ast)?
        .clone();

    let original_code_value_node = object_literal_map.get_value_node(ast, "original_code");
    let mut original_code_object_literal_map =
        extract_object_literal_map(ast, original_code_value_node)?;
    let placeholder_original_code_map = original_code_object_literal_map
        .extract_as_string_to_string_map(ast)?
        .clone();

    // NOTE: The getX() methods below should all do little to no computation.
    // In particular, all checking for MalformedExceptions must be done before creating this
    // object.
    Ok(JsMessageOptions {
        is_escape_less_than,
        is_unescape_html_entities,
        placeholder_examples_map,
        placeholder_original_code_map,
        example_object_literal_map,
        original_code_object_literal_map,
    })
}

/// Java's private `interface IcuTemplateOptions` and its one anonymous implementation in
/// `extractIcuTemplateOptions`.
struct IcuTemplateOptions {
    description: JsString,
    meaning: Option<JsString>,
    alternate_message_id: Option<JsString>,
    placeholder_examples_map: IndexMap<JsString, JsString>,
    placeholder_original_code_map: IndexMap<JsString, JsString>,
    placeholder_names: IndexSet<JsString>,
    example_object_literal_map: ObjectLiteralMapImpl,
    original_code_object_literal_map: ObjectLiteralMapImpl,
}

impl IcuTemplateOptions {
    // port: JsMessageVisitor.IcuTemplateOptions#getDescription
    fn get_description(&self) -> &JsString {
        &self.description
    }

    // port: JsMessageVisitor.IcuTemplateOptions#getMeaning
    fn get_meaning(&self) -> Option<&JsString> {
        self.meaning.as_ref()
    }

    // port: JsMessageVisitor.IcuTemplateOptions#getAlternateMessageId
    fn get_alternate_message_id(&self) -> Option<&JsString> {
        self.alternate_message_id.as_ref()
    }

    // port: JsMessageVisitor.IcuTemplateOptions#getPlaceholderExampleMap
    fn get_placeholder_example_map(&self) -> &IndexMap<JsString, JsString> {
        &self.placeholder_examples_map
    }

    // port: JsMessageVisitor.IcuTemplateOptions#getPlaceholderOriginalCodeMap
    fn get_placeholder_original_code_map(&self) -> &IndexMap<JsString, JsString> {
        &self.placeholder_original_code_map
    }

    /// All the placeholder names mentioned in the options.
    // port: JsMessageVisitor.IcuTemplateOptions#getPlaceholderNames
    fn get_placeholder_names(&self) -> &IndexSet<JsString> {
        &self.placeholder_names
    }

    // port: JsMessageVisitor.IcuTemplateOptions#checkForUnknownPlaceholders
    fn check_for_unknown_placeholders(
        &mut self,
        known_placeholders: &IndexSet<JsString>,
    ) -> Result<(), MalformedException> {
        self.example_object_literal_map
            .check_for_unexpected_keys(known_placeholders, &|unknown_name: &JsString| {
                format!("Unknown placeholder: {unknown_name}")
            })?;
        self.original_code_object_literal_map
            .check_for_unexpected_keys(known_placeholders, &|unknown_name: &JsString| {
                format!("Unknown placeholder: {unknown_name}")
            })
    }
}

/// Property names that are expected to appear in the options bag argument of
/// declareIcuTemplate().
// port: JsMessageVisitor#ICU_TEMPLATE_OPTION_NAMES
static ICU_TEMPLATE_OPTION_NAMES: LazyLock<IndexSet<JsString>> = LazyLock::new(|| {
    [
        "description",
        "meaning",
        "alternate_message_id",
        "example",
        "original_code",
    ]
    .into_iter()
    .map(JsString::from)
    .collect()
});

/// Extract the options from the second argument to a `declareIcuTemplate()` call
// port: JsMessageVisitor#extractIcuTemplateOptions
fn extract_icu_template_options(
    ast: &Ast,
    options_bag: NodeId,
) -> Result<IcuTemplateOptions, MalformedException> {
    let mut object_literal_map = extract_object_literal_map(ast, Some(options_bag))?;
    object_literal_map
        .check_for_unexpected_keys(&ICU_TEMPLATE_OPTION_NAMES, &|option_name: &JsString| {
            format!("Unknown option: {option_name}")
        })?;

    // required description string
    let Some(description_node) = object_literal_map.get_value_node(ast, "description") else {
        return Err(MalformedException::new(
            "'description' option field is missing",
            Some(options_bag),
        ));
    };
    let description = extract_string_from_string_expr_node(ast, description_node)?;

    // optional meaning string
    let meaning_node = object_literal_map.get_value_node(ast, "meaning");
    let meaning = match meaning_node {
        None => None,
        Some(meaning_node) => Some(extract_string_from_string_expr_node(ast, meaning_node)?),
    };

    // optional alternate message ID
    let alternate_message_id_node = object_literal_map.get_value_node(ast, "alternate_message_id");
    let alternate_message_id = match alternate_message_id_node {
        None => None,
        Some(alternate_message_id_node) => Some(extract_string_from_string_expr_node(
            ast,
            alternate_message_id_node,
        )?),
    };

    // optional map of placeholder names to example string values
    let example_value_node = object_literal_map.get_value_node(ast, "example");
    let mut example_object_literal_map = extract_object_literal_map(ast, example_value_node)?;
    let placeholder_examples_map = example_object_literal_map
        .extract_as_string_to_string_map(ast)?
        .clone();

    // optional map of placeholder names to original_code string values
    let original_code_value_node = object_literal_map.get_value_node(ast, "original_code");
    let mut original_code_object_literal_map =
        extract_object_literal_map(ast, original_code_value_node)?;
    let placeholder_original_code_map = original_code_object_literal_map
        .extract_as_string_to_string_map(ast)?
        .clone();

    // All the placeholder names mentioned in the 2 optional maps.
    let mut placeholder_names: IndexSet<JsString> = IndexSet::new();
    placeholder_names.extend(placeholder_examples_map.keys().cloned());
    placeholder_names.extend(placeholder_original_code_map.keys().cloned());

    for placeholder_name in &placeholder_names {
        if !js_message::is_canonical_placeholder_name_format(placeholder_name) {
            return Err(MalformedException::new(
                format!("Placeholder not in UPPER_SNAKE_CASE: {placeholder_name}"),
                Some(options_bag),
            ));
        }
    }

    // NOTE: The getX() methods below should all do little to no computation.
    // In particular, all checking for MalformedExceptions must be done before creating this
    // object.
    Ok(IcuTemplateOptions {
        description,
        meaning,
        alternate_message_id,
        placeholder_examples_map,
        placeholder_original_code_map,
        placeholder_names,
        example_object_literal_map,
        original_code_object_literal_map,
    })
}

// port: JsMessageVisitor#extractBooleanStringKeyValue
fn extract_boolean_string_key_value(
    ast: &Ast,
    string_key_node: Option<NodeId>,
) -> Result<bool, MalformedException> {
    match string_key_node {
        None => Ok(false),
        Some(string_key_node) => {
            let value_node = string_key_node.get_only_child(ast);
            if value_node.is_true(ast) {
                Ok(true)
            } else if value_node.is_false(ast) {
                Ok(false)
            } else {
                Err(MalformedException::new(
                    format!(
                        "{}: Literal true or false expected",
                        string_key_node.get_string(ast)
                    ),
                    Some(value_node),
                ))
            }
        }
    }
}

/// Represents the contents of a object literal Node in the AST.
///
/// The object literal may not have any computed keys or methods.
///
/// Use [`extract_object_literal_map`] to get an instance.
pub trait ObjectLiteralMap {
    // port: JsMessageVisitor.ObjectLiteralMap#getBooleanValueOrFalse
    fn get_boolean_value_or_false(&self, ast: &Ast, key: &str) -> Result<bool, MalformedException>;

    /// Returns a map from object property names the Node values they have in the AST, building
    /// the map first, if necessary.
    // port: JsMessageVisitor.ObjectLiteralMap#extractAsValueMap
    fn extract_as_value_map(&mut self, ast: &Ast) -> &IndexMap<JsString, NodeId>;

    /// Returns a map from object property names to string values, building it first, if
    /// necessary.
    ///
    /// Returns `Err(MalformedException)` if any of the values are not actually simple string
    /// literals or concatenations of string literals.
    // port: JsMessageVisitor.ObjectLiteralMap#extractAsStringToStringMap
    fn extract_as_string_to_string_map(
        &mut self,
        ast: &Ast,
    ) -> Result<&IndexMap<JsString, JsString>, MalformedException>;

    /// Get the value node for a key.
    ///
    /// This method avoids the work of extracting the full value map.
    // port: JsMessageVisitor.ObjectLiteralMap#getValueNode
    fn get_value_node(&self, ast: &Ast, key: &str) -> Option<NodeId>;

    /// Throws a `MalformedException` if the object literal has keys not in the expected set.
    // port: JsMessageVisitor.ObjectLiteralMap#checkForUnexpectedKeys
    fn check_for_unexpected_keys(
        &mut self,
        expected_keys: &IndexSet<JsString>,
        create_error_message: &dyn Fn(&JsString) -> String,
    ) -> Result<(), MalformedException>;

    /// Throws a `MalformedException` if the object literal does not have one all the keys in the
    /// required set.
    // port: JsMessageVisitor.ObjectLiteralMap#checkForRequiredKeys
    fn check_for_required_keys(
        &mut self,
        required_keys: &IndexSet<JsString>,
        create_exception: &dyn Fn(&JsString) -> MalformedException,
    ) -> Result<(), MalformedException>;
}

/// `private static class ObjectLiteralMapImpl implements ObjectLiteralMap`.
#[derive(Debug)]
pub struct ObjectLiteralMapImpl {
    string_to_string_key_map: IndexMap<JsString, NodeId>,
    // The result of extractValueMap(). It will be populated if requested.
    value_map: Option<IndexMap<JsString, NodeId>>,
    // The result of extractStringMap(). It will be populated if requested.
    string_map: Option<IndexMap<JsString, JsString>>,
}

impl ObjectLiteralMapImpl {
    // port: JsMessageVisitor.ObjectLiteralMapImpl#ObjectLiteralMapImpl
    fn new(string_to_string_key_map: IndexMap<JsString, NodeId>) -> Self {
        Self {
            string_to_string_key_map,
            value_map: None,
            string_map: None,
        }
    }
}

impl ObjectLiteralMap for ObjectLiteralMapImpl {
    // port: JsMessageVisitor.ObjectLiteralMapImpl#getValueNode
    fn get_value_node(&self, ast: &Ast, key: &str) -> Option<NodeId> {
        let string_key_node = self.string_to_string_key_map.get(&JsString::from(key));
        string_key_node.map(|string_key_node| string_key_node.get_only_child(ast))
    }

    // port: JsMessageVisitor.ObjectLiteralMapImpl#getBooleanValueOrFalse
    fn get_boolean_value_or_false(&self, ast: &Ast, key: &str) -> Result<bool, MalformedException> {
        let string_key_node = self
            .string_to_string_key_map
            .get(&JsString::from(key))
            .copied();
        extract_boolean_string_key_value(ast, string_key_node)
    }

    // port: JsMessageVisitor.ObjectLiteralMapImpl#extractAsValueMap
    fn extract_as_value_map(&mut self, ast: &Ast) -> &IndexMap<JsString, NodeId> {
        if self.value_map.is_none() {
            let mut builder: IndexMap<JsString, NodeId> = IndexMap::new();
            for (key, value) in &self.string_to_string_key_map {
                builder.insert(key.clone(), value.get_only_child(ast));
            }
            self.value_map = Some(builder);
        }
        self.value_map.as_ref().unwrap()
    }

    // port: JsMessageVisitor.ObjectLiteralMapImpl#extractAsStringToStringMap
    fn extract_as_string_to_string_map(
        &mut self,
        ast: &Ast,
    ) -> Result<&IndexMap<JsString, JsString>, MalformedException> {
        if self.string_map.is_none() {
            let mut builder: IndexMap<JsString, JsString> = IndexMap::new();
            for (key, value) in &self.string_to_string_key_map {
                builder.insert(
                    key.clone(),
                    extract_string_from_string_expr_node(ast, value.get_only_child(ast))?,
                );
            }
            self.string_map = Some(builder);
        }
        Ok(self.string_map.as_ref().unwrap())
    }

    // port: JsMessageVisitor.ObjectLiteralMapImpl#checkForUnexpectedKeys
    fn check_for_unexpected_keys(
        &mut self,
        expected_keys: &IndexSet<JsString>,
        create_error_message: &dyn Fn(&JsString) -> String,
    ) -> Result<(), MalformedException> {
        for (key, string_key) in &self.string_to_string_key_map {
            if !expected_keys.contains(key) {
                return Err(MalformedException::new(
                    create_error_message(key),
                    Some(*string_key),
                ));
            }
        }
        Ok(())
    }

    // port: JsMessageVisitor.ObjectLiteralMapImpl#checkForRequiredKeys
    fn check_for_required_keys(
        &mut self,
        required_keys: &IndexSet<JsString>,
        create_exception: &dyn Fn(&JsString) -> MalformedException,
    ) -> Result<(), MalformedException> {
        for required_key in required_keys {
            if !self.string_to_string_key_map.contains_key(required_key) {
                return Err(create_exception(required_key));
            }
        }
        Ok(())
    }
}

/// Returns an object to represent an object literal Node from the AST.
///
/// The object literal's members must all be `STRING_KEY` nodes.
///
/// No duplicate string keys are allowed.
///
/// A `None` argument is treated as if it were an empty object literal.
///
/// Returns `Err(MalformedException)` if the Node does not meet the above requirements.
// port: JsMessageVisitor#extractObjectLiteralMap
pub fn extract_object_literal_map(
    ast: &Ast,
    obj_lit: Option<NodeId>,
) -> Result<ObjectLiteralMapImpl, MalformedException> {
    let Some(obj_lit) = obj_lit else {
        return Ok(ObjectLiteralMapImpl::new(IndexMap::new()));
    };
    if !obj_lit.is_object_lit(ast) {
        return Err(MalformedException::new(
            "object literal expected",
            Some(obj_lit),
        ));
    }
    let mut string_to_string_key_map: IndexMap<JsString, NodeId> = IndexMap::new();
    let mut string_key = obj_lit.get_first_child(ast);
    while let Some(sk) = string_key {
        if !sk.is_string_key(ast) {
            return Err(MalformedException::new("string key expected", Some(sk)));
        }
        let key = sk.get_string(ast);
        if string_to_string_key_map.contains_key(&key) {
            return Err(MalformedException::new(
                format!("duplicate string key: {key}"),
                Some(sk),
            ));
        }
        string_to_string_key_map.insert(sk.get_string(ast), sk);
        string_key = sk.get_next(ast);
    }
    Ok(ObjectLiteralMapImpl::new(string_to_string_key_map))
}

// port: JsMessageVisitor#parseJsMessageTextIntoParts
pub fn parse_js_message_text_into_parts(
    original_msg_text: &JsString,
) -> Result<Vec<Part>, PlaceholderFormatException> {
    let goog_get_msg_parsed_text = extract_goog_get_msg_parsed_text(original_msg_text)?;
    Ok(goog_get_msg_parsed_text.parts)
}

// port: JsMessageVisitor#extractGoogGetMsgParsedText
fn extract_goog_get_msg_parsed_text(
    original_msg_text: &JsString,
) -> Result<GoogGetMsgParsedText, PlaceholderFormatException> {
    let ph_js_prefix = JsString::from(PH_JS_PREFIX);
    let ph_js_suffix = JsString::from(PH_JS_SUFFIX);
    let mut msg_text = original_msg_text.clone();
    let mut parts_builder: Vec<Part> = Vec::new();
    let mut placeholder_names_builder: IndexSet<JsString> = IndexSet::new();
    loop {
        let ph_begin = msg_text.index_of(&ph_js_prefix);
        if ph_begin < 0 {
            // Just a string literal
            parts_builder.push(Part::StringPart(StringPart::create(msg_text.clone())));
            break;
        } else {
            if ph_begin > 0 {
                // A string literal followed by a placeholder
                parts_builder.push(Part::StringPart(StringPart::create(
                    msg_text.substring(0, ph_begin as usize),
                )));
            }

            // A placeholder. Find where it ends
            let ph_end = msg_text.index_of_from(&ph_js_suffix, ph_begin);
            if ph_end < 0 {
                return Err(PlaceholderFormatException::new(
                    "Placeholder incorrectly formatted",
                ));
            }

            let ph_name =
                msg_text.substring(ph_begin as usize + ph_js_prefix.length(), ph_end as usize);
            if !js_message::is_lower_camel_case_with_numeric_suffixes(&ph_name) {
                return Err(PlaceholderFormatException::new(format!(
                    "Placeholder name not in lowerCamelCase: {ph_name}"
                )));
            }
            placeholder_names_builder.insert(ph_name.clone());
            parts_builder.push(Part::PlaceholderReference(
                PlaceholderReference::create_for_js_name(ph_name),
            ));
            let next_pos = ph_end as usize + ph_js_suffix.length();
            if next_pos < msg_text.length() {
                // Iterate on the rest of the message value
                msg_text = msg_text.substring_from(next_pos);
            } else {
                // The message is parsed
                break;
            }
        }
    }
    // NOTE: The methods defined below should do little to no computation, and definitely should
    // not throw exceptions.
    Ok(GoogGetMsgParsedText {
        text: original_msg_text.clone(),
        placeholder_names: placeholder_names_builder,
        parts: parts_builder,
    })
}

/// Java's private `interface GoogGetMsgParsedText` and its one anonymous implementation.
struct GoogGetMsgParsedText {
    text: JsString,
    placeholder_names: IndexSet<JsString>,
    parts: Vec<Part>,
}

impl GoogGetMsgParsedText {
    // port: JsMessageVisitor.GoogGetMsgParsedText#getText
    fn get_text(&self) -> &JsString {
        &self.text
    }

    // port: JsMessageVisitor.GoogGetMsgParsedText#getPlaceholderNames
    fn get_placeholder_names(&self) -> &IndexSet<JsString> {
        &self.placeholder_names
    }

    // port: JsMessageVisitor.GoogGetMsgParsedText#getParts
    fn get_parts(&self) -> &[Part] {
        &self.parts
    }
}

/// Visit a call to goog.getMsgWithFallback.
// port: JsMessageVisitor#visitFallbackFunctionCall
fn visit_fallback_function_call(
    this: &mut dyn JsMessageVisitor,
    t: &mut NodeTraversal<'_>,
    call: NodeId,
) {
    // Check to make sure the function call looks like:
    // goog.getMsgWithFallback(MSG_1, MSG_2);
    // or:
    // goog.getMsgWithFallback(some.import.MSG_1, some.import.MSG_2);
    if !call.has_x_children(t, 3)
        || !is_message_identifier(t, call.get_second_child(t).unwrap())
        || !is_message_identifier(t, call.get_last_child(t).unwrap())
    {
        let compiler = t.get_compiler();
        compiler.report(JSError::make(compiler, call, &BAD_FALLBACK_SYNTAX, &[]));
        return;
    }

    let first_arg = call.get_second_child(t).unwrap();
    let first_message = get_js_message_from_node(this, t, first_arg);
    let Some(first_message) = first_message else {
        let qualified_name = java_string_value_of(first_arg.get_qualified_name(t).as_ref());
        let compiler = t.get_compiler();
        compiler.report(JSError::make(
            compiler,
            first_arg,
            &FALLBACK_ARG_ERROR,
            &[&qualified_name],
        ));
        return;
    };

    let second_arg = first_arg.get_next(t).unwrap();
    let second_message = get_js_message_from_node(this, t, second_arg);
    let Some(second_message) = second_message else {
        let qualified_name = java_string_value_of(second_arg.get_qualified_name(t).as_ref());
        let compiler = t.get_compiler();
        compiler.report(JSError::make(
            compiler,
            second_arg,
            &FALLBACK_ARG_ERROR,
            &[&qualified_name],
        ));
        return;
    };

    this.process_message_fallback(t.get_compiler(), call, &first_message, &second_message);
}

// port: JsMessageVisitor#isMessageIdentifier
fn is_message_identifier(ast: &Ast, node: NodeId) -> bool {
    let qname = node.get_qualified_name(ast);
    qname.is_some_and(|q| q.index_of(&JsString::from(MSG_PREFIX)) >= 0)
}

/// Extracts a message name (e.g. MSG_FOO) from either a NAME node or a GETPROP node. This should
/// cover all of the following cases:
///
/// 1. a NAME node (e.g. MSG_FOO)
/// 2. a NAME node which is the product of renaming (e.g. $module$contents$MSG_FOO)
/// 3. a GETPROP node (e.g. some.import.MSG_FOO)
// port: JsMessageVisitor#getJsMessageFromNode
fn get_js_message_from_node(
    this: &mut dyn JsMessageVisitor,
    t: &mut NodeTraversal<'_>,
    node: NodeId,
) -> Option<JsMessage> {
    let message_name = node.get_qualified_name(t);
    let msg_prefix = JsString::from(MSG_PREFIX);
    let message_name = match message_name {
        Some(message_name) if message_name.index_of(&msg_prefix) >= 0 => message_name,
        _ => return None,
    };

    let message_key = message_name.substring_from(message_name.index_of(&msg_prefix) as usize);
    if is_unnamed_message_name(&message_key) {
        get_tracked_unnamed_message(this, t, &message_name)
    } else {
        get_tracked_normal_message(this, &message_key)
    }
}

/// Returns whether the given message name is in the unnamed namespace.
// port: JsMessageVisitor#isUnnamedMessageName
fn is_unnamed_message_name(identifier: &JsString) -> bool {
    MSG_UNNAMED_PATTERN.matcher(identifier.clone()).matches()
}

/// Checks a node's type.
///
/// Returns `Err(MalformedException)` if the node is null or the wrong type
// port: JsMessageVisitor#checkNode
pub fn check_node(ast: &Ast, node: Option<NodeId>, type_: Token) -> Result<(), MalformedException> {
    let Some(n) = node else {
        return Err(MalformedException::new(
            format!("Expected node type {type_}; found: null"),
            node,
        ));
    };
    if n.get_token(ast) != type_ {
        return Err(MalformedException::new(
            format!("Expected node type {type_}; found: {}", n.get_token(ast)),
            node,
        ));
    }
    Ok(())
}

/// `static class MalformedException extends Exception`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MalformedException {
    message: String,
    node: Option<NodeId>,
}

impl MalformedException {
    // port: JsMessageVisitor.MalformedException#MalformedException
    pub fn new(message: impl Into<String>, node: Option<NodeId>) -> Self {
        Self {
            message: message.into(),
            node,
        }
    }

    // port: JsMessageVisitor.MalformedException#getNode
    pub fn get_node(&self) -> Option<NodeId> {
        self.node
    }

    // port: Throwable#getMessage
    pub fn get_message(&self) -> &str {
        &self.message
    }
}

/// `private static class MessageLocation`.
#[derive(Clone, Debug)]
struct MessageLocation {
    message: JsMessage,
    message_node: NodeId,
}

impl MessageLocation {
    // port: JsMessageVisitor.MessageLocation#MessageLocation
    fn new(message: JsMessage, message_node: NodeId) -> Self {
        Self {
            message,
            message_node,
        }
    }
}

// port: JsMessageVisitor#isScopedAliasesPrefix
pub fn is_scoped_aliases_prefix(name: &JsString) -> bool {
    SCOPED_ALIASES_PREFIX_PATTERN
        .matcher(name.clone())
        .looking_at()
}

// port: JsMessageVisitor#removeScopedAliasesPrefix
pub fn remove_scoped_aliases_prefix(name: &JsString) -> JsString {
    matcher_replace_first(&SCOPED_ALIASES_PREFIX_PATTERN, name, "MSG_")
}

/// Java's `pattern.matcher(text).replaceFirst(replacement)` for a replacement without group
/// references or escapes.
// port: Matcher#replaceFirst(String)
fn matcher_replace_first(pattern: &Pattern, text: &JsString, replacement: &str) -> JsString {
    let mut matcher = pattern.matcher(text.clone());
    if !matcher.find() {
        return text.clone();
    }
    let mut sb: Vec<u16> = Vec::new();
    sb.extend_from_slice(text.substring(0, matcher.start()).as_units());
    sb.extend(replacement.encode_utf16());
    sb.extend_from_slice(text.substring_from(matcher.end()).as_units());
    JsString::from_units(sb)
}

/// Java's `String.trim().isEmpty()`: every code unit is `<= ' '`.
// port: String#trim
fn java_trim_is_empty(s: &JsString) -> bool {
    s.as_units().iter().all(|&c| c <= u16::from(b' '))
}

/// Java's `Long.toString(long, int)`.
// port: Long#toString(long,int)
fn java_long_to_string(i: i64, radix: i64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf: Vec<u8> = Vec::with_capacity(65);
    let negative = i < 0;
    let mut i = if negative { i } else { -i };
    while i <= -radix {
        buf.push(DIGITS[(-(i % radix)) as usize]);
        i /= radix;
    }
    buf.push(DIGITS[(-i) as usize]);
    if negative {
        buf.push(b'-');
    }
    buf.reverse();
    String::from_utf8(buf).unwrap()
}

/// Java's `String.valueOf(Object)` for a nullable string (`"null"` for null).
fn java_string_value_of(s: Option<&JsString>) -> String {
    match s {
        Some(s) => s.to_string(),
        None => "null".to_string(),
    }
}
