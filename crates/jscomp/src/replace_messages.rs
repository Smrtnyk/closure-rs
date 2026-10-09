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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/ReplaceMessages.java.

//! Port of `com.google.javascript.jscomp.ReplaceMessages`: replaces user-visible messages with
//! alternatives. It uses Google specific JsMessageVisitor implementation.
//!
//! Java's inner pass classes (`MsgProtectionPass`, `ReplacementCompletionPass`,
//! `FullReplacementPass`) own their `ReplaceMessages` instance as `outer` (Java's
//! `ReplaceMessages.this`); every `get*Pass()` call in Java creates a new inner instance of a new
//! `ReplaceMessages`, so the Rust methods consume `self`.

use std::sync::Arc;

use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId, Prop, SideEffectFlags};
use closure_rhino::{check_not_null, check_state};

use crate::abstract_compiler::AbstractCompiler;
use crate::ast_factory::{AstFactory, Type};
use crate::change_tracker::ChangeTracker;
use crate::compiler_options::PropertyCollapseLevel;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::icu_template_definition::IcuTemplateDefinition;
use crate::js_error::JSError;
use crate::js_message::{Builder, GrammaticalGenderCase, IdGenerator, JsMessage, Part, StringPart};
use crate::js_message_definition::JsMessageDefinition;
use crate::js_message_visitor::{
    self, IcuMessageTemplateString, JsMessageVisitor, JsMessageVisitorBase, MESSAGE_TREE_MALFORMED,
    MalformedException, ObjectLiteralMap,
};
use crate::message_bundle::MessageBundle;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::replace_messages_constants as ReplaceMessagesConstants;

// port: ReplaceMessages#BUNDLE_DOES_NOT_HAVE_THE_MESSAGE
pub static BUNDLE_DOES_NOT_HAVE_THE_MESSAGE: DiagnosticType = DiagnosticType::error(
    "JSC_BUNDLE_DOES_NOT_HAVE_THE_MESSAGE",
    "Message with id = {0} could not be found in replacement bundle",
);

// port: ReplaceMessages#INVALID_ALTERNATE_MESSAGE_PLACEHOLDERS
pub static INVALID_ALTERNATE_MESSAGE_PLACEHOLDERS: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_ALTERNATE_MESSAGE_PLACEHOLDERS",
    "Alternate message ID={0} placeholders ({1}) differs from {2} placeholders ({3}).",
);

/// ReplaceMessages replaces user-visible messages with alternatives.
pub struct ReplaceMessages {
    bundle: Arc<dyn MessageBundle + Send + Sync>,
    strict_replacement: bool,
    collapse_properties_has_run: bool,
    ast_factory: AstFactory,
}

impl ReplaceMessages {
    // port: ReplaceMessages#ReplaceMessages
    pub fn new(
        compiler: &mut AbstractCompiler,
        bundle: Arc<dyn MessageBundle + Send + Sync>,
        strict_replacement: bool,
    ) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let collapse_properties_has_run = compiler.get_options().get_property_collapse_level()
            == PropertyCollapseLevel::ALL
            && compiler.get_options().do_late_localization();
        Self {
            bundle,
            strict_replacement,
            collapse_properties_has_run,
            ast_factory,
        }
    }

    /// When the returned pass is executed, the original `goog.getMsg()` etc. calls will be
    /// replaced with a form that will survive unchanged through optimizations unless eliminated
    /// as unused.
    ///
    /// After all optimizations are complete, the pass returned by
    /// `getReplacementCompletionPass()`.
    // port: ReplaceMessages#getMsgProtectionPass
    pub fn get_msg_protection_pass(self) -> MsgProtectionPass {
        MsgProtectionPass::new(self)
    }

    // port: ReplaceMessages#getMsgOptionsFromDefinition
    fn get_msg_options_from_definition(&self, definition: &dyn JsMessageDefinition) -> MsgOptions {
        MsgOptions {
            is_icu_template: false,
            escape_less_than: definition.should_escape_less_than(),
            unescape_html_entities: definition.should_unescape_html_entities(),
        }
    }

    // port: ReplaceMessages#createMsgPropertiesNode
    fn create_msg_properties_node(
        &self,
        compiler: &mut AbstractCompiler,
        message: &JsMessage,
        msg_options: &MsgOptions,
    ) -> NodeId {
        let mut msg_props_builder = QuotedKeyObjectLitBuilder::new(&self.ast_factory);
        msg_props_builder.add_string(compiler, "key", message.get_key().clone());
        if msg_options.is_icu_template && !message.canonical_placeholder_names().is_empty() {
            // ICU messages created using `declareIcuTemplate` can get stored into the XMB file as
            // multiple parts if necessary to record example or original code text.
            // `icu_placeholder_names` stores these parts of the ICU message, which allows us to
            // correctly calculate the message ID in the protected message.
            let names_array_lit = self.ast_factory.create_arraylit(compiler, &[]);
            for name in message.canonical_placeholder_names() {
                let name_node = self.ast_factory.create_string(compiler, name.clone());
                names_array_lit.add_child_to_back(compiler, name_node);
            }
            // Example:
            // declareIcuTemplate('blah blah {PH1} blah {PH2}', ... );
            // icu_placeholder_names: ['PH1', 'PH2']
            msg_props_builder.add_node("icu_placeholder_names", names_array_lit);
        }
        if let Some(alt_id) = message.get_alternate_id() {
            msg_props_builder.add_string(compiler, "alt_id", alt_id.clone());
        }
        if let Some(meaning) = message.get_meaning() {
            msg_props_builder.add_string(compiler, "meaning", meaning.clone());
        }
        if msg_options.is_icu_template {
            msg_props_builder.add_string(compiler, "msg_text", message.as_icu_message_string());
            // Just being present is what records this option as true
            msg_props_builder.add_string(compiler, "isIcuTemplate", "");
        } else {
            msg_props_builder.add_string(compiler, "msg_text", message.as_js_message_string());
        }
        if msg_options.escape_less_than {
            // Just being present is what records this option as true
            msg_props_builder.add_string(compiler, "escapeLessThan", "");
        }
        if msg_options.unescape_html_entities {
            // Just being present is what records this option as true
            msg_props_builder.add_string(compiler, "unescapeHtmlEntities", "");
        }
        msg_props_builder.build(compiler)
    }

    /// When the returned pass is executed, the protected messages created by
    /// `getMsgProtectionPass()` will be replaced by the final message form read from the
    /// appropriate message bundle.
    // port: ReplaceMessages#getReplacementCompletionPass
    pub fn get_replacement_completion_pass(self) -> ReplacementCompletionPass {
        ReplacementCompletionPass {
            outer: self,
            translated_msg_keys: IndexSet::<_>::default(),
        }
    }

    // port: ReplaceMessages#lookupMessage
    fn lookup_message(
        &self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        bundle: &dyn MessageBundle,
        message: &JsMessage,
    ) -> Option<JsMessage> {
        if let Some(translated_message) = bundle.get_message(message.get_id()) {
            return Some(translated_message.clone());
        }

        let alternate_id = message.get_alternate_id()?;

        let alternate_message = bundle.get_message(alternate_id);
        if let Some(alternate_message) = alternate_message {
            // Validate that the alternate message is compatible with this message. Ideally we'd
            // also check meaning and description, but they're not populated by
            // `MessageBundle.getMessage`.
            let js_code_placeholder_names = message.js_placeholder_names();
            let alternate_msg_placeholder_names = alternate_message.js_placeholder_names();
            // ImmutableSet#equals is order-insensitive set equality.
            if !(js_code_placeholder_names.len() == alternate_msg_placeholder_names.len()
                && js_code_placeholder_names
                    .iter()
                    .all(|name| alternate_msg_placeholder_names.contains(name)))
            {
                // The JS code definition has no placeholders, but the message we got from the
                // translation bundle does have placeholders.
                //
                // This can happen for ICU - formatted messages. They can contain placeholders in
                // the message string in the form "{PH_NAME}", which this compiler ignores,
                // because they will be replaced at runtime by passing the whole string to a
                // message formatter method. However, sometimes the translation bundle will
                // represent the "{PH_NAME}" substring as an actual placeholder reference, because
                // it was generated by some tool other than closure-compiler. In that case, we need
                // to join all the parts of the message back together as a single string with the
                // "{PH_NAME}" references back in place.
                //
                // Messages with this form always start with a particular formula, which we can
                // test for here to make sure this isn't actually a case of something being broken
                // in the translation pipeline.
                if js_code_placeholder_names.is_empty()
                    && is_start_of_icu_message(&message.as_icu_message_string())
                {
                    return Some(alternate_message.clone());
                } else {
                    let alternate_names = java_set_to_string(alternate_msg_placeholder_names);
                    let js_code_names = java_set_to_string(js_code_placeholder_names);
                    compiler.report(JSError::make(
                        compiler,
                        call_node,
                        &INVALID_ALTERNATE_MESSAGE_PLACEHOLDERS,
                        &[
                            &alternate_id.to_string_lossy(),
                            &alternate_names,
                            &message.get_key().to_string_lossy(),
                            &js_code_names,
                        ],
                    ));
                    return None;
                }
            }
        }
        alternate_message.cloned()
    }

    /// When the returned pass is executed, the original `goog.getMsg()` etc. calls will all be
    /// replaced with the final message form read from the message bundle.
    ///
    /// This is the original way of running this pass as a single operation.
    // port: ReplaceMessages#getFullReplacementPass
    pub fn get_full_replacement_pass(self) -> FullReplacementPass {
        FullReplacementPass::new(self)
    }

    /// Outputs a message with a hook expression that returns the correct variant based on the
    /// value of `goog.viewerGrammaticalGender` from XTB files with gendered messages.
    ///
    /// With placeholders:
    ///
    /// ```text
    /// var WELCOME_MSG = function(name) {
    ///    return goog.msgKind.MASCULINE ? "Bienvenido " + name :
    ///           goog.msgKind.FEMININE ? "Bienvenida " + name :
    ///           goog.msgKind.NEUTER ? "Les damos la bienvenida " + name :
    ///           "Les damos la bienvenida " + name;
    /// }(user.getName());
    /// ```
    // port: ReplaceMessages#createNodeForGenderedMsgString
    fn create_node_for_gendered_msg_string(
        &self,
        compiler: &mut AbstractCompiler,
        message: &JsMessage,
        placeholder_map: &IndexMap<JsString, NodeId>,
        options: &MsgOptions,
        node_to_replace: NodeId,
    ) -> Result<NodeId, MalformedException> {
        // Dynamically build parameter list with unique ids for each placeholder
        let param_list = self.ast_factory.create_param_list(compiler, &[]);
        let type_ = AstFactory::type_node(node_to_replace);
        let input_id = NodeUtil::get_input_id(compiler, node_to_replace);
        let input = input_id
            .and_then(|input_id| compiler.get_input(&input_id).cloned())
            .expect("NullPointerException: compiler.getInput(NodeUtil.getInputId(nodeToReplace))");
        let unique_id = compiler.get_unique_id_supplier().get_unique_id(&input);
        let mut placeholder_map_ids: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();
        for placeholder_name in placeholder_map.keys() {
            let placeholder_id = placeholder_name.concat(&JsString::from(unique_id.as_str()));
            let name =
                self.ast_factory
                    .create_name(compiler, placeholder_id.clone(), type_.clone());
            param_list.add_child_to_back(compiler, name);
            placeholder_map_ids.insert(placeholder_name.clone(), placeholder_id);
        }

        let hook_expression = self.create_hook_expression_for_gendered_msg(
            compiler,
            &type_,
            message,
            options,
            node_to_replace,
            &placeholder_map_ids,
        )?;

        if placeholder_map.is_empty() {
            // The translated message read from the bundle is one of the following:
            // 1. has gendered variants and no placeholders
            // 2. has gendered variants and is an icu template because icu templates do not have
            // placeholders

            // Create the hook expression for the gendered message. This will be used to create
            // the call node if there are placeholders, or returned directly if there are no
            // placeholders.
            return Ok(hook_expression);
        }

        let return_node = IR::return_node_with_expression(compiler, hook_expression);
        let block = IR::block_with_child(compiler, return_node);
        let function =
            self.ast_factory
                .create_function(compiler, "", param_list, block, type_.clone());
        let color = node_to_replace.get_color(compiler);
        function.set_color(compiler, color);
        compiler.report_change_to_change_scope(function);

        let call_node = self.ast_factory.create_call(compiler, function, type_, &[]);

        // Add the placeholder values to the call
        for node in placeholder_map.values() {
            let clone = node.clone_tree(compiler);
            call_node.add_child_to_back(compiler, clone);
        }
        Ok(call_node)
    }

    /// Creates a node representing the grammatical gender condition.
    ///
    /// For example:
    ///
    /// ```text
    /// goog.msgKind.MASCULINE ? "Bienvenido + name" :
    /// goog.msgKind.FEMININE ? "Bienvenida + name" :
    /// goog.msgKind.NEUTER ? "Les damos la bienvenida + name" :
    /// "Les damos la bienvenida + name";
    /// ```
    // port: ReplaceMessages#createConditionForGrammaticalGender
    fn create_condition_for_grammatical_gender(
        &self,
        compiler: &mut AbstractCompiler,
        type_0: &Type,
        grammatical_gender: GrammaticalGenderCase,
        node_to_replace: NodeId,
    ) -> NodeId {
        // NOTE: Collapse properties isn't guarantee to collapse any given property but if
        // we get a partial collapse of "goog.msgKind" then something has gone very wrong
        // so this seems reasonable rather than the alternative (traversing the AST to find the
        // values).
        if self.collapse_properties_has_run {
            let reference_name = match grammatical_gender {
                GrammaticalGenderCase::MASCULINE => "goog$msgKind$MASCULINE",
                GrammaticalGenderCase::FEMININE => "goog$msgKind$FEMININE",
                GrammaticalGenderCase::NEUTER => "goog$msgKind$NEUTER",
                _ => "goog$msgKind$OTHER",
            };
            let node_type = AstFactory::type_node(node_to_replace);
            let result = self
                .ast_factory
                .create_name(compiler, reference_name, node_type);
            result.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
            result
        } else {
            let goog_node = self
                .ast_factory
                .create_name(compiler, "goog", type_0.clone());
            goog_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
            let node_type = AstFactory::type_node(node_to_replace);
            let msg_kind = self
                .ast_factory
                .create_get_prop(compiler, goog_node, "msgKind", node_type);
            let node_type = AstFactory::type_node(node_to_replace);
            self.ast_factory.create_get_prop(
                compiler,
                msg_kind,
                grammatical_gender.to_string(),
                node_type,
            )
        }
    }

    /// Creates a ternary expression with the gendered message variants.
    ///
    /// For example:
    ///
    /// ```text
    /// goog.msgKind.MASCULINE ? "Bienvenido + name" :
    /// goog.msgKind.FEMININE ? "Bienvenida + name" :
    /// goog.msgKind.NEUTER ? "Les damos la bienvenida + name" :
    /// "Les damos la bienvenida + name";
    /// ```
    // port: ReplaceMessages#createHookExpressionForGenderedMsg
    fn create_hook_expression_for_gendered_msg(
        &self,
        compiler: &mut AbstractCompiler,
        type_0: &Type,
        msg: &JsMessage,
        options: &MsgOptions,
        node_to_replace: NodeId,
        placeholder_map_ids: &IndexMap<JsString, JsString>,
    ) -> Result<NodeId, MalformedException> {
        let empty_name = IR::name(compiler, "");
        let empty_string1 = IR::string(compiler, "");
        let empty_string2 = IR::string(compiler, "");
        let mut hook_head = IR::hook(compiler, empty_name, empty_string1, empty_string2);
        let color = node_to_replace.get_color(compiler);
        hook_head.set_color(compiler, color.clone());
        // The previous hook condition needing to be replaced
        let mut placeholder_hook = hook_head;

        let ast_factory = &self.ast_factory;
        let mut placeholder_node_provider =
            |compiler: &mut AbstractCompiler, placeholder_name: &JsString| {
                placeholder_map_ids
                    .get(placeholder_name)
                    .map(|placeholder_id| {
                        let node_type = AstFactory::type_node(node_to_replace);
                        ast_factory.create_name(compiler, placeholder_id.clone(), node_type)
                    })
            };

        for grammatical_gender in msg.get_gendered_message_variants() {
            // Skip the OTHER case as it should be the last case in the hook expression
            if grammatical_gender == GrammaticalGenderCase::OTHER {
                continue;
            }
            // Hook condition ex: `goog.msgKind.MASCULINE?` or `goog.msgKind.FEMININE ?` or
            // `goog.msgKind.NEUTER ?`
            let condition = self.create_condition_for_grammatical_gender(
                compiler,
                type_0,
                grammatical_gender,
                node_to_replace,
            );

            // The last child of the hook expression will be replaced with the next currentHook
            let message_node = self.build_message_node(
                compiler,
                msg.get_gendered_message_parts(grammatical_gender),
                options,
                node_to_replace,
                &mut placeholder_node_provider,
                /* is_gendered_msg= */ true,
            )?;
            let empty = self.ast_factory.create_string(compiler, "");
            let current_hook = IR::hook(compiler, condition, message_node, empty);
            current_hook.set_color(compiler, color.clone());

            if let Some(parent) = placeholder_hook.get_parent(compiler) {
                // Replace the previous hook condition with the current hook condition
                let last_child = parent.get_last_child(compiler).unwrap();
                last_child.replace_with(compiler, current_hook);
                placeholder_hook = current_hook.get_last_child(compiler).unwrap();
            } else {
                hook_head = current_hook;
                placeholder_hook = current_hook.get_last_child(compiler).unwrap();
            }
        }
        // The OTHER is always the last case in the hook expression
        let other_node = self.build_message_node(
            compiler,
            msg.get_gendered_message_parts(GrammaticalGenderCase::OTHER),
            options,
            node_to_replace,
            &mut placeholder_node_provider,
            /* is_gendered_msg= */ true,
        )?;
        placeholder_hook
            .get_parent(compiler)
            .unwrap()
            .get_last_child(compiler)
            .unwrap()
            .replace_with(compiler, other_node);
        Ok(hook_head)
    }

    /// Creates a parse tree corresponding to a list of message parts. The result consists of one
    /// or more STRING nodes, placeholder replacement value nodes (which can be arbitrary
    /// expressions), and ADD nodes.
    // port: ReplaceMessages#buildMessageNode
    fn build_message_node(
        &self,
        compiler: &mut AbstractCompiler,
        msg_parts: &[Part],
        options: &MsgOptions,
        node_to_replace: NodeId,
        placeholder_node_provider: PlaceholderNodeProvider<'_>,
        is_gendered_msg: bool,
    ) -> Result<NodeId, MalformedException> {
        // A message might have normal string parts that are consecutive.
        // Join them together before processing them further. This allows split HTML entities
        // to be escaped.
        let parts = merge_string_parts(msg_parts);
        if parts.is_empty() {
            return Ok(self.ast_factory.create_string(compiler, ""));
        }
        let mut message: Option<NodeId> = None;
        for msg_part in &parts {
            let part_node;
            if msg_part.is_placeholder() {
                let js_placeholder_name = msg_part.get_js_placeholder_name();
                let value_node = placeholder_node_provider(compiler, &js_placeholder_name);
                match value_node {
                    None => {
                        if is_gendered_msg {
                            // Add the ICU placeholder directly to the message ex: 'Hello {NAME}'
                            let text = JsString::from("{")
                                .concat(&msg_part.get_canonical_placeholder_name())
                                .concat(&JsString::from("}"));
                            part_node = self.ast_factory.create_string(compiler, text);
                        } else {
                            return Err(MalformedException::new(
                                format!(
                                    "Unrecognized message placeholder referenced: {}",
                                    js_placeholder_name
                                ),
                                Some(node_to_replace),
                            ));
                        }
                    }
                    Some(value_node) => {
                        part_node = value_node.clone_tree(compiler);
                    }
                }
            } else {
                // The part is just a string literal.
                part_node =
                    self.create_node_for_msg_string(compiler, options, msg_part.get_string());
            }

            message = Some(match message {
                None => part_node,
                Some(message) if part_node.is_string(compiler) && message.is_string(compiler) => {
                    let joined = message
                        .get_string(compiler)
                        .concat(&part_node.get_string(compiler));
                    self.ast_factory.create_string(compiler, joined)
                }
                Some(message) => self.ast_factory.create_add(compiler, message, part_node),
            });
        }
        Ok(message.unwrap())
    }

    /// Creates a parse tree corresponding to the remaining message parts in an iteration. The
    /// result consists of one or more STRING nodes, placeholder replacement value nodes (which
    /// can be arbitrary expressions), and ADD nodes.
    // port: ReplaceMessages#constructStringExprNode
    fn construct_string_expr_node(
        &self,
        compiler: &mut AbstractCompiler,
        message: &JsMessage,
        placeholder_map: &IndexMap<JsString, NodeId>,
        options: &MsgOptions,
        node_to_replace: NodeId,
    ) -> Result<NodeId, MalformedException> {
        // If the message has gendered variants, create a hook expression containing the gendered
        // variants.
        if !message.get_gendered_message_variants().is_empty() {
            self.create_node_for_gendered_msg_string(
                compiler,
                message,
                placeholder_map,
                options,
                node_to_replace,
            )
        } else {
            self.construct_non_gendered_msg_node(
                compiler,
                message,
                placeholder_map,
                options,
                node_to_replace,
            )
        }
    }

    // port: ReplaceMessages#constructNonGenderedMsgNode
    fn construct_non_gendered_msg_node(
        &self,
        compiler: &mut AbstractCompiler,
        message: &JsMessage,
        placeholder_map: &IndexMap<JsString, NodeId>,
        options: &MsgOptions,
        node_to_replace: NodeId,
    ) -> Result<NodeId, MalformedException> {
        if placeholder_map.is_empty() {
            // The compiler does not expect to do any placeholder substitution, because the
            // message definition in the JS code doesn't have any placeholders.
            if options.is_icu_template {
                // This message was declared as an ICU template. Its placeholders, if any, will not
                // be replaced during compilation, but rather at runtime by a special localization
                // method. We just need to put the string back together again, if it was broken up
                // to include placeholders.
                return Ok(self.create_node_for_msg_string(
                    compiler,
                    options,
                    message.as_icu_message_string(),
                ));
            } else if message.js_placeholder_names().is_empty() {
                // The translated message read from the bundle also has no placeholders.
                // It doesn't really matter which asXMessageString() call we make, since they
                // return the same thing when there are no placeholders.
                return Ok(self.create_node_for_msg_string(
                    compiler,
                    options,
                    message.as_js_message_string(),
                ));
            } else {
                // The JS code definition has no placeholders, but the message we got from the
                // translation bundle does have placeholders.
                //
                // This can happen for ICU - formatted messages. They can contain placeholders in
                // the message string in the form "{PH_NAME}", which this compiler ignores,
                // because they will be replaced at runtime by passing the whole string to a
                // message formatter method. However, sometimes the translation bundle will
                // represent the "{PH_NAME}" substring as an actual placeholder reference, because
                // it was generated by some tool other than closure-compiler. In that case, we need
                // to join all the parts of the message back together as a single string with the
                // "{PH_NAME}" references back in place.
                //
                // Messages with this form always start with a particular formula, which we can
                // test for here to make sure this isn't actually a case of something being broken
                // in the translation pipeline.
                let icu_msg_string = message.as_icu_message_string();
                if is_start_of_icu_message(&icu_msg_string) {
                    return Ok(self.create_node_for_msg_string(compiler, options, icu_msg_string));
                } else {
                    return Err(MalformedException::new(
                        "The translated message has placeholders, but the definition in the JS \
                         code does not.",
                        Some(node_to_replace),
                    ));
                }
            }
        }

        self.build_message_node(
            compiler,
            message.get_parts(),
            options,
            node_to_replace,
            &mut |_compiler: &mut AbstractCompiler, name: &JsString| {
                placeholder_map.get(name).copied()
            },
            /* is_gendered_msg= */ false,
        )
    }

    // port: ReplaceMessages#createNodeForMsgString
    fn create_node_for_msg_string(
        &self,
        compiler: &mut AbstractCompiler,
        options: &MsgOptions,
        s: JsString,
    ) -> NodeId {
        let mut s = s;
        if options.escape_less_than {
            // Note that "&" is not replaced because the translation can contain HTML entities.
            s = s.replace(&JsString::from("<"), &JsString::from("&lt;"));
        }
        if options.unescape_html_entities {
            // Unescape entities that need to be escaped when embedding HTML or XML in
            // data/attributes of an HTML/XML document. See
            // https://www.w3.org/TR/xml/#sec-predefined-ent.
            // Note that "&amp;" must be the last to avoid "creating" new entities.
            // To print an html entity in the resulting message, double-escape: `&amp;amp;`.
            s = s
                .replace(&JsString::from("&lt;"), &JsString::from("<"))
                .replace(&JsString::from("&gt;"), &JsString::from(">"))
                .replace(&JsString::from("&apos;"), &JsString::from("'"))
                .replace(&JsString::from("&quot;"), &JsString::from("\""))
                .replace(&JsString::from("&amp;"), &JsString::from("&"));
        }
        self.ast_factory.create_string(compiler, s)
    }
}

/// `record MsgProtectionData`.
// port: ReplaceMessages.MsgProtectionData
struct MsgProtectionData<'a> {
    message: &'a JsMessage,
    message_node: NodeId,
    template_text_node: NodeId,
    placeholder_values_node: Option<NodeId>,
    message_options: MsgOptions,
}

/// `class MsgProtectionPass extends JsMessageVisitor`.
pub struct MsgProtectionPass {
    outer: ReplaceMessages,
    base: JsMessageVisitorBase,
}

impl MsgProtectionPass {
    // port: ReplaceMessages.MsgProtectionPass#MsgProtectionPass
    fn new(outer: ReplaceMessages) -> Self {
        let base = JsMessageVisitorBase::new(outer.bundle.id_generator());
        Self { outer, base }
    }

    // port: ReplaceMessages.MsgProtectionPass#performMessageProtection
    fn perform_message_protection(
        &mut self,
        compiler: &mut AbstractCompiler,
        msg_protection_data: MsgProtectionData<'_>,
    ) {
        let message = msg_protection_data.message;
        let call_node = msg_protection_data.message_node;
        let original_message_string = msg_protection_data.template_text_node;
        let placeholders_node = msg_protection_data.placeholder_values_node;
        let msg_options = msg_protection_data.message_options;

        check_state!(
            call_node.is_call(compiler),
            "%s",
            call_node.to_string(compiler)
        );
        // `goog.getMsg('message string', {<substitutions>}, {<options>})`
        let goog_get_msg = call_node.get_first_child(compiler).unwrap();

        // Construct
        // `__jscomp_define_msg__({<msg properties>}, {<substitutions>})`
        let protection_function_name = ReplaceMessagesConstants::DEFINE_MSG_CALLEE;
        let new_callee = self
            .create_protection_function_callee(compiler, protection_function_name)
            .srcref(compiler, goog_get_msg);
        let msg_properties_node = self
            .outer
            .create_msg_properties_node(compiler, message, &msg_options)
            .srcref_tree(compiler, original_message_string);
        let call_type = AstFactory::type_node(call_node);
        let new_call_node = self
            .outer
            .ast_factory
            .create_call(compiler, new_callee, call_type, &[msg_properties_node])
            .srcref(compiler, call_node);
        // If the result of this call (the message) is unused, there is no reason for
        // optimizations to preserve it.
        new_call_node.set_side_effect_flags(compiler, SideEffectFlags::NO_SIDE_EFFECTS);
        if let Some(placeholders_node) = placeholders_node {
            check_state!(
                placeholders_node.is_object_lit(compiler),
                "%s",
                placeholders_node.to_string(compiler)
            );
            // put quotes around the keys so they won't get renamed.
            let mut str_key = placeholders_node.get_first_child(compiler);
            while let Some(sk) = str_key {
                check_state!(sk.is_string_key(compiler), "%s", sk.to_string(compiler));
                sk.set_quoted_string_key(compiler);
                str_key = sk.get_next(compiler);
            }
            let detached = placeholders_node.detach(compiler);
            new_call_node.add_child_to_back(compiler, detached);
        }
        call_node.replace_with(compiler, new_call_node);
        compiler.report_change_to_enclosing_scope(new_call_node);
    }

    // port: ReplaceMessages.MsgProtectionPass#createProtectionFunctionCallee
    fn create_protection_function_callee(
        &self,
        compiler: &mut AbstractCompiler,
        protection_function_name: &str,
    ) -> NodeId {
        let callee = self
            .outer
            .ast_factory
            .create_name_with_unknown_type(compiler, protection_function_name);
        // The name is declared constant in the externs definition we created, so all references
        // to it must also be marked as constant for consistency's sake.
        callee.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        callee
    }
}

impl CompilerPass for MsgProtectionPass {
    // port: ReplaceMessages.MsgProtectionPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Add externs declarations for the function names we use in our replacements.
        NodeUtil::create_synthesized_externs_symbol(
            compiler,
            ReplaceMessagesConstants::DEFINE_MSG_CALLEE,
        );
        NodeUtil::create_synthesized_externs_symbol(
            compiler,
            ReplaceMessagesConstants::FALLBACK_MSG_CALLEE,
        );

        // JsMessageVisitor.process() does the traversal that calls the processX() methods below.
        js_message_visitor::process(self, compiler, Some(externs), root);
    }
}

impl JsMessageVisitor for MsgProtectionPass {
    fn js_message_visitor_base(&mut self) -> &mut JsMessageVisitorBase {
        &mut self.base
    }

    // port: ReplaceMessages.MsgProtectionPass#processIcuTemplateDefinition
    fn process_icu_template_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        definition: Box<dyn IcuTemplateDefinition>,
    ) {
        self.perform_message_protection(
            compiler,
            MsgProtectionData {
                message: definition.get_message(),
                message_node: definition.get_message_node(),
                template_text_node: definition.get_template_text_node(),
                // There are no compile-time placeholder replacements for ICU templates
                placeholder_values_node: None,
                message_options: ICU_MSG_OPTIONS,
            },
        );
    }

    // port: ReplaceMessages.MsgProtectionPass#processJsMessageDefinition
    fn process_js_message_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        definition: Box<dyn JsMessageDefinition>,
    ) {
        // This is the currently preferred form.
        // `MSG_A = goog.getMsg('hello, {$name}', {name: getName()}, {html: true})`
        let message_options = self
            .outer
            .get_msg_options_from_definition(definition.as_ref());
        self.perform_message_protection(
            compiler,
            MsgProtectionData {
                message: definition.get_message(),
                message_node: definition.get_message_node(),
                template_text_node: definition.get_template_text_node(),
                placeholder_values_node: definition.get_placeholder_values_node(),
                message_options,
            },
        );
    }

    // port: ReplaceMessages.MsgProtectionPass#processMessageFallback
    fn process_message_fallback(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        message1: &JsMessage,
        message2: &JsMessage,
    ) {
        let original_callee = check_not_null!(
            call_node.get_first_child(compiler),
            "%s",
            call_node.to_string(compiler)
        );
        let fallback_callee = self
            .create_protection_function_callee(
                compiler,
                ReplaceMessagesConstants::FALLBACK_MSG_CALLEE,
            )
            .srcref(compiler, original_callee);

        let original_first_arg = check_not_null!(
            original_callee.get_next(compiler),
            "%s",
            call_node.to_string(compiler)
        );
        let first_msg_key = self
            .outer
            .ast_factory
            .create_string(compiler, message1.get_key().clone())
            .srcref(compiler, original_first_arg);

        let original_second_arg = check_not_null!(
            original_first_arg.get_next(compiler),
            "%s",
            call_node.to_string(compiler)
        );
        let second_msg_key = self
            .outer
            .ast_factory
            .create_string(compiler, message2.get_key().clone())
            .srcref(compiler, original_second_arg);

        // `__jscomp_msg_fallback__('MSG_ONE', MSG_ONE, 'MSG_TWO', MSG_TWO)`
        let call_type = AstFactory::type_node(call_node);
        let first_arg = original_first_arg.detach(compiler);
        let second_arg = original_second_arg.detach(compiler);
        let new_call_node = self
            .outer
            .ast_factory
            .create_call(
                compiler,
                fallback_callee,
                call_type,
                &[first_msg_key, first_arg, second_msg_key, second_arg],
            )
            .srcref(compiler, call_node);
        // If the result of this call (the message) is unused, there is no reason for
        // optimizations to preserve it.
        new_call_node.set_side_effect_flags(compiler, SideEffectFlags::NO_SIDE_EFFECTS);
        call_node.replace_with(compiler, new_call_node);
        compiler.report_change_to_enclosing_scope(new_call_node);
    }
}

/// `private final class QuotedKeyObjectLitBuilder`.
struct QuotedKeyObjectLitBuilder<'a> {
    ast_factory: &'a AstFactory,
    // LinkedHashMap to keep the keys in the order we set them so our output is deterministic.
    key_to_value_node_map: IndexMap<JsString, NodeId>,
}

impl<'a> QuotedKeyObjectLitBuilder<'a> {
    fn new(ast_factory: &'a AstFactory) -> Self {
        Self {
            ast_factory,
            key_to_value_node_map: IndexMap::<_, _>::default(),
        }
    }

    // port: ReplaceMessages.QuotedKeyObjectLitBuilder#addString
    fn add_string(
        &mut self,
        compiler: &mut AbstractCompiler,
        key: &str,
        value: impl Into<JsString>,
    ) -> &mut Self {
        let node = self.ast_factory.create_string(compiler, value);
        self.add_node(key, node)
    }

    // port: ReplaceMessages.QuotedKeyObjectLitBuilder#addNode
    fn add_node(&mut self, key: &str, node: NodeId) -> &mut Self {
        let key = JsString::from(key);
        check_state!(
            !self.key_to_value_node_map.contains_key(&key),
            "repeated key: %s",
            key
        );
        self.key_to_value_node_map.insert(key, node);
        self
    }

    // port: ReplaceMessages.QuotedKeyObjectLitBuilder#build
    fn build(&self, compiler: &mut AbstractCompiler) -> NodeId {
        let result = self.ast_factory.create_object_lit(compiler, &[]);
        for (key, value) in &self.key_to_value_node_map {
            let string_key =
                self.ast_factory
                    .create_quoted_string_key(compiler, key.clone(), *value);
            result.add_child_to_back(compiler, string_key);
        }
        result
    }
}

/// `class ReplacementCompletionPass implements CompilerPass`.
pub struct ReplacementCompletionPass {
    outer: ReplaceMessages,
    // Keep track of which messages actually got translated, so we know what do do when we
    // see a message fallback call.
    translated_msg_keys: IndexSet<JsString>,
}

/// The anonymous `AbstractPostOrderCallback` of `ReplacementCompletionPass#process`.
struct ReplacementCompletionCallback<'a> {
    pass: &'a mut ReplacementCompletionPass,
}

impl Callback for ReplacementCompletionCallback<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ReplaceMessages.ReplacementCompletionPass#process (anonymous visit)
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let id_generator = self.pass.outer.bundle.id_generator();
        let protected_js_message =
            ProtectedJsMessage::from_ast_node(t.get_compiler(), n, id_generator.as_deref());
        if let Some(protected_js_message) = protected_js_message {
            self.pass
                .visit_msg_definition(t.get_compiler(), protected_js_message);
        } else {
            let protected_msg_fallback = ProtectedMsgFallback::from_ast_node(t.get_compiler(), n);
            if let Some(protected_msg_fallback) = protected_msg_fallback {
                self.pass
                    .visit_msg_fallback(t.get_compiler(), protected_msg_fallback);
            }
        }
    }
}

impl CompilerPass for ReplacementCompletionPass {
    // port: ReplaceMessages.ReplacementCompletionPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // for each `__jscomp_define_msg__` call in post-order traversal
        // replace it with the appropriate expression
        NodeTraversal::traverse(
            compiler,
            root,
            &mut ReplacementCompletionCallback { pass: self },
        );
    }
}

impl ReplacementCompletionPass {
    // port: ReplaceMessages.ReplacementCompletionPass#visitMsgDefinition
    fn visit_msg_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        protected_js_message: ProtectedJsMessage,
    ) {
        let result = (|| -> Result<(), MalformedException> {
            let original_msg = &protected_js_message.js_message;
            let node_to_replace = protected_js_message.definition_node;
            let bundle = self.outer.bundle.clone();
            let translated_msg = self.outer.lookup_message(
                compiler,
                protected_js_message.definition_node,
                bundle.as_ref(),
                original_msg,
            );
            let msg_to_use;
            if let Some(translated_msg) = translated_msg {
                msg_to_use = translated_msg;
                // Remember that this one got translated in case it is used in a fallback.
                self.translated_msg_keys
                    .insert(original_msg.get_key().clone());
            } else {
                if self.outer.strict_replacement {
                    compiler.report(JSError::make(
                        compiler,
                        node_to_replace,
                        &BUNDLE_DOES_NOT_HAVE_THE_MESSAGE,
                        &[&original_msg.get_id().to_string()],
                    ));
                }
                msg_to_use = original_msg.clone();
            }
            let msg_options = protected_js_message.get_msg_options();
            let placeholder_map = extract_placeholder_values_map_or_throw(
                compiler,
                protected_js_message.substitutions_node,
            );
            let final_msg_construction_expression = self.outer.construct_string_expr_node(
                compiler,
                &msg_to_use,
                &placeholder_map,
                &msg_options,
                node_to_replace,
            )?;
            final_msg_construction_expression.srcref_tree_if_missing(compiler, node_to_replace);
            node_to_replace.replace_with(compiler, final_msg_construction_expression);
            compiler.report_change_to_enclosing_scope(final_msg_construction_expression);
            Ok(())
        })();
        if let Err(e) = result {
            compiler.report(JSError::make(
                compiler,
                check_not_null!(e.get_node()),
                &MESSAGE_TREE_MALFORMED,
                &[e.get_message()],
            ));
        }
    }

    // port: ReplaceMessages.ReplacementCompletionPass#visitMsgFallback
    fn visit_msg_fallback(
        &mut self,
        compiler: &mut AbstractCompiler,
        protected_msg_fallback: ProtectedMsgFallback,
    ) {
        let value_node_to_use;
        if self
            .translated_msg_keys
            .contains(&protected_msg_fallback.first_msg_key)
        {
            // Obviously use the first message, if it is translated.
            value_node_to_use = protected_msg_fallback.first_msg_value;
        } else if self
            .translated_msg_keys
            .contains(&protected_msg_fallback.second_msg_key)
        {
            // Fallback to the second message if it has a translation.
            value_node_to_use = protected_msg_fallback.second_msg_value;
        } else {
            // If neither is translated, then use the first message as it is defined in the
            // source code.
            value_node_to_use = protected_msg_fallback.first_msg_value;
        }
        value_node_to_use.detach(compiler);
        protected_msg_fallback
            .call_node
            .replace_with(compiler, value_node_to_use);
        compiler.report_change_to_enclosing_scope(value_node_to_use);
    }
}

// port: ReplaceMessages#extractPlaceholderValuesMapOrThrow
pub(crate) fn extract_placeholder_values_map_or_throw(
    ast: &Ast,
    values_obj_lit: Option<NodeId>,
) -> IndexMap<JsString, NodeId> {
    match js_message_visitor::extract_object_literal_map(ast, values_obj_lit) {
        Ok(mut map) => map.extract_as_value_map(ast).clone(),
        Err(e) => panic!(
            "java.lang.IllegalStateException: \
             com.google.javascript.jscomp.JsMessageVisitor$MalformedException: {}",
            e.get_message()
        ),
    }
}

/// `private static class ProtectedMsgFallback`.
struct ProtectedMsgFallback {
    call_node: NodeId,
    first_msg_key: JsString,
    first_msg_value: NodeId,
    second_msg_key: JsString,
    second_msg_value: NodeId,
}

impl ProtectedMsgFallback {
    // port: ReplaceMessages.ProtectedMsgFallback#fromAstNode
    fn from_ast_node(ast: &Ast, n: NodeId) -> Option<ProtectedMsgFallback> {
        if !n.is_call(ast) {
            return None;
        }
        let callee = n.get_first_child(ast).unwrap();
        if !callee.matches_name(ast, ReplaceMessagesConstants::FALLBACK_MSG_CALLEE) {
            return None;
        }
        check_state!(
            n.has_x_children(ast, 5),
            "bad message fallback call: %s",
            n.to_string(ast)
        );
        let first_msg_key_node = callee.get_next(ast).unwrap();
        let first_msg_key = first_msg_key_node.get_string(ast);
        let first_msg_value = first_msg_key_node.get_next(ast).unwrap();
        let second_msg_key_node = first_msg_value.get_next(ast).unwrap();
        let second_msg_key = second_msg_key_node.get_string(ast);
        let second_msg_value = second_msg_key_node.get_next(ast).unwrap();
        // port: ReplaceMessages.ProtectedMsgFallback#ProtectedMsgFallback
        Some(ProtectedMsgFallback {
            call_node: n,
            first_msg_key,
            first_msg_value,
            second_msg_key,
            second_msg_value,
        })
    }
}

/// `record FullReplacementMsgData`.
// port: ReplaceMessages.FullReplacementMsgData
struct FullReplacementMsgData<'a> {
    message: &'a JsMessage,
    message_node: NodeId,
    message_options: MsgOptions,
    placeholder_value_map: IndexMap<JsString, NodeId>,
}

/// `class FullReplacementPass extends JsMessageVisitor`.
pub struct FullReplacementPass {
    outer: ReplaceMessages,
    base: JsMessageVisitorBase,
}

impl FullReplacementPass {
    // port: ReplaceMessages.FullReplacementPass#FullReplacementPass
    fn new(outer: ReplaceMessages) -> Self {
        let base = JsMessageVisitorBase::new(outer.bundle.id_generator());
        Self { outer, base }
    }

    // port: ReplaceMessages.FullReplacementPass#processFullReplacement
    fn process_full_replacement(
        &mut self,
        compiler: &mut AbstractCompiler,
        full_replacement_msg_data: FullReplacementMsgData<'_>,
    ) {
        let message = full_replacement_msg_data.message;
        let msg_node = full_replacement_msg_data.message_node;
        let options = full_replacement_msg_data.message_options;
        let placeholder_value_map = full_replacement_msg_data.placeholder_value_map;

        // Get the replacement.
        let bundle = self.outer.bundle.clone();
        let mut replacement =
            self.outer
                .lookup_message(compiler, msg_node, bundle.as_ref(), message);
        if replacement.is_none() {
            if self.outer.strict_replacement {
                compiler.report(JSError::make(
                    compiler,
                    msg_node,
                    &BUNDLE_DOES_NOT_HAVE_THE_MESSAGE,
                    &[&message.get_id().to_string()],
                ));
                // Fallback to the default message
                return;
            } else {
                // In case if it is not a strict replacement we could leave original
                // message.
                replacement = Some(message.clone());
            }
        }
        let replacement = replacement.unwrap();

        // Replace the message.
        // Build the replacement tree.
        let new_value = match self.outer.construct_string_expr_node(
            compiler,
            &replacement,
            &placeholder_value_map,
            &options,
            msg_node,
        ) {
            Ok(new_value) => new_value,
            Err(e) => {
                compiler.report(JSError::make(
                    compiler,
                    check_not_null!(e.get_node()),
                    &MESSAGE_TREE_MALFORMED,
                    &[e.get_message()],
                ));
                msg_node
            }
        };

        if new_value != msg_node {
            new_value.srcref_tree_if_missing(compiler, msg_node);
            msg_node.replace_with(compiler, new_value);
            compiler.report_change_to_enclosing_scope(new_value);
        }
    }
}

impl CompilerPass for FullReplacementPass {
    // port: JsMessageVisitor#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        js_message_visitor::process(self, compiler, Some(externs), root);
    }
}

impl JsMessageVisitor for FullReplacementPass {
    fn js_message_visitor_base(&mut self) -> &mut JsMessageVisitorBase {
        &mut self.base
    }

    // port: ReplaceMessages.FullReplacementPass#processMessageFallback
    fn process_message_fallback(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        message1: &JsMessage,
        message2: &JsMessage,
    ) {
        let bundle = self.outer.bundle.clone();
        let is_first_message_translated = self
            .outer
            .lookup_message(compiler, call_node, bundle.as_ref(), message1)
            .is_some();
        let is_second_message_translated = self
            .outer
            .lookup_message(compiler, call_node, bundle.as_ref(), message2)
            .is_some();
        let replacement_node = if is_second_message_translated && !is_first_message_translated {
            call_node.get_child_at_index(compiler, 2).unwrap()
        } else {
            call_node.get_second_child(compiler).unwrap()
        };
        let detached = replacement_node.detach(compiler);
        call_node.replace_with(compiler, detached);
        let change_scope =
            ChangeTracker::get_enclosing_change_scope_root(compiler, Some(replacement_node));
        if let Some(change_scope) = change_scope {
            compiler.report_change_to_change_scope(change_scope);
        }
    }

    // port: ReplaceMessages.FullReplacementPass#processIcuTemplateDefinition
    fn process_icu_template_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        definition: Box<dyn IcuTemplateDefinition>,
    ) {
        self.process_full_replacement(
            compiler,
            FullReplacementMsgData {
                message: definition.get_message(),
                message_node: definition.get_message_node(),
                message_options: ICU_MSG_OPTIONS,
                // There are no compile-time placeholder replacements for an ICU template message.
                placeholder_value_map: IndexMap::<_, _>::default(),
            },
        );
    }

    // port: ReplaceMessages.FullReplacementPass#processJsMessageDefinition
    fn process_js_message_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        definition: Box<dyn JsMessageDefinition>,
    ) {
        let message_options = self
            .outer
            .get_msg_options_from_definition(definition.as_ref());
        self.process_full_replacement(
            compiler,
            FullReplacementMsgData {
                message: definition.get_message(),
                message_node: definition.get_message_node(),
                message_options,
                placeholder_value_map: definition.get_placeholder_value_map().clone(),
            },
        );
    }
}

/// `interface PlaceholderNodeProvider`: returns the value Node for a given placeholder name.
// port: ReplaceMessages.PlaceholderNodeProvider#get
type PlaceholderNodeProvider<'a> =
    &'a mut dyn FnMut(&mut AbstractCompiler, &JsString) -> Option<NodeId>;

/// Options for escaping characters in translated messages.
// port: ReplaceMessages.MsgOptions
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MsgOptions {
    /// True if the message is defined using the ICU template declaration method.
    is_icu_template: bool,
    /// Replace `'<'` with `'&lt;'` in the message.
    escape_less_than: bool,
    /// Replace these escaped entities with their literal characters in the message (Overrides
    /// escapeLessThan)
    ///
    /// ```text
    /// '&lt;' -> '<'
    /// '&gt;' -> '>'
    /// '&apos;' -> "'"
    /// '&quot;' -> '"'
    /// '&amp;' -> '&'
    /// ```
    unescape_html_entities: bool,
}

// port: ReplaceMessages#ICU_MSG_OPTIONS
const ICU_MSG_OPTIONS: MsgOptions = MsgOptions {
    is_icu_template: true,
    escape_less_than: false,
    unescape_html_entities: false,
};

/// Merges consecutive string parts in the list of message parts.
// port: ReplaceMessages#mergeStringParts
fn merge_string_parts(parts: &[Part]) -> Vec<Part> {
    let mut result: Vec<Part> = Vec::new();
    for part in parts {
        if part.is_placeholder() {
            result.push(part.clone());
        } else {
            let last_part = result.last();
            match last_part {
                Some(last_part) if !last_part.is_placeholder() => {
                    let merged = last_part.get_string().concat(&part.get_string());
                    let last_index = result.len() - 1;
                    result[last_index] = Part::StringPart(StringPart::create(merged));
                }
                _ => result.push(part.clone()),
            }
        }
    }
    result
}

/// Holds information about the protected form of a translatable message that appears in the
/// AST.
///
/// The original translatable messages are replaced with this protected form by the logic in
/// `ReplaceMessages` to protect the message information through the optimization passes.
///
/// ```text
///   // original
///   var MSG_GREETING = goog.getMsg('Hello, {$name}!', {name: person.getName()}, {html: false});
///   // protected form
///   var MSG_GREETING = __jscomp_define_msg__(
///       {
///         'key': 'MSG_GREETING',
///         'msg_text': 'Hello, {$name}!',
///         'escapeLessThan': ''
///       },
///       {'name': person.getName()});
/// ```
pub struct ProtectedJsMessage {
    js_message: JsMessage,
    // The expression Node that defines the message and should be replaced with the localized
    // message.
    definition_node: NodeId,
    // e.g. `{ name: x.getName(), age: x.getAgeString() }`
    substitutions_node: Option<NodeId>,
    // The message was defined in JS code using the ICU template method.
    is_icu_template: bool,
    // Replace `'<'` with `'&lt;'` in the message.
    escape_less_than: bool,
    // Replace these escaped entities with their literal characters in the message
    // (Overrides escapeLessThan)
    unescape_html_entities: bool,
}

impl ProtectedJsMessage {
    // port: ReplaceMessages.ProtectedJsMessage#fromAstNode
    pub fn from_ast_node(
        ast: &Ast,
        node: NodeId,
        id_generator: Option<&dyn IdGenerator>,
    ) -> Option<ProtectedJsMessage> {
        if !node.is_call(ast) {
            return None;
        }
        let callee_node = check_not_null!(node.get_first_child(ast), "%s", node.to_string(ast));
        if !callee_node.matches_name(ast, ReplaceMessagesConstants::DEFINE_MSG_CALLEE) {
            return None;
        }
        let properties_node =
            check_not_null!(callee_node.get_next(ast), "%s", callee_node.to_string(ast));
        let substitutions_node = properties_node.get_next(ast);
        let mut escape_less_than_option = false;
        let mut unescape_html_entities_option = false;
        let mut is_icu_template = false;
        let mut js_message_builder = Builder::new();
        check_state!(
            properties_node.is_object_lit(ast),
            "%s",
            properties_node.to_string(ast)
        );
        let mut msg_key: Option<JsString> = None;
        let mut meaning: Option<JsString> = None;
        let mut icu_placeholder_names: IndexSet<JsString> = IndexSet::<_>::default();
        let mut message_text: Option<JsString> = None;
        let mut message_text_node: Option<NodeId> = None;
        let mut str_key = properties_node.get_first_child(ast);
        while let Some(sk) = str_key {
            check_state!(sk.is_string_key(ast), "%s", sk.to_string(ast));
            let key = sk.get_string(ast);
            let value_node = sk.get_only_child(ast);
            if key == "icu_placeholder_names" {
                check_state!(
                    value_node.is_array_lit(ast),
                    "icu_placeholder_names must be an array"
                );
                // If the message is an ICU template and `icu_placeholder_names` is present, then
                // there are placeholders in the message. These placeholders will be replaced at
                // runtime, but it is important that we keep track of these placeholders because
                // it means that the ICU template CANNOT be treated as a single string part,
                // because having placeholders means that the message has multiple parts.
                // When a message is a `declareIcuTemplate` with multiple parts, we generate a
                // `msg id` in the XMB, which is sent to the Translation Console so that the
                // translators can translate this. We generate the deterministic `msg id` using an
                // algorithm that takes into account how many parts the message has.
                // Now during JSCompiler compilation process, we protect the message by wrapping
                // it in a `__jscomp_define_msg__` (for safety because we don't want any of our
                // optimization passes to change the message).
                // Later in this method, we generate an ID (using `idGenerator.generateId()`) and
                // use this to lookup a message in the translated XTB file. As I mentioned
                // earlier, the algorithm for generating an ID needs to know the correct parts of
                // the message, so we fail to generate the same ID we did when we added the
                // message to the XMB.
                // This `icu_placeholder_names` field is necessary to help us figure out the
                // correct parts of the message, in order to generate the correct message ID that
                // matches the ID we generated when we added the message to the XMB (which is the
                // same ID in the XTB).
                for value_node_child in value_node.children(ast) {
                    icu_placeholder_names.insert(value_node_child.get_string(ast));
                }
                str_key = sk.get_next(ast);
                continue;
            }
            check_state!(
                value_node.is_string_lit(ast),
                "%s",
                value_node.to_string(ast)
            );
            let value = value_node.get_string(ast);
            match key.to_string_lossy().as_str() {
                "key" => {
                    js_message_builder.set_key(value.clone());
                    msg_key = Some(value);
                }
                "meaning" => {
                    js_message_builder.set_meaning(Some(value.clone()));
                    meaning = Some(value);
                }
                "alt_id" => {
                    js_message_builder.set_alternate_id(Some(value));
                }
                "msg_text" => {
                    // This may be an ICU template that also has the `icu_placeholder_names`
                    // property, which means we need to append multiple parts of the message to
                    // `jsMessageBuilder`. For now, we will save the message text and current
                    // node, and we'll parse it once we know if this is an ICU template with
                    // multiple parts (after this loop to run through all the properties is
                    // finished).
                    message_text = Some(value);
                    message_text_node = Some(value_node);
                }
                "isIcuTemplate" => is_icu_template = true,
                // Just being present enables this option
                "escapeLessThan" => escape_less_than_option = true,
                // just being present enables this option
                "unescapeHtmlEntities" => unescape_html_entities_option = true,
                _ => panic!("unknown protected message key: {}", sk.to_string(ast)),
            }
            str_key = sk.get_next(ast);
        }

        let parts_result = if !icu_placeholder_names.is_empty() {
            check_state!(
                is_icu_template,
                "Found icu_placeholder_names for a message that is not an ICU template."
            );
            // This is an ICU template with placeholders ("{$placeholderName}"). We cannot treat
            // this as a single string part, because it has multiple parts. Otherwise, we will
            // generate the wrong message id and we will not be able to find the correct
            // translated message in the XTB file (because when the XMB message was created
            // during message extraction, we treated this ICU template as having multiple parts).
            // Java passes a null messageText to the IcuMessageTemplateString constructor here,
            // which throws NullPointerException; the unwrap panics the same way.
            let icu_message_template_string =
                IcuMessageTemplateString::new(message_text.clone().unwrap());
            let extracted_icu_template_parts =
                icu_message_template_string.extract_parts(&icu_placeholder_names);

            // Append the parts of the ICU template to the jsMessageBuilder.
            js_message_builder.append_parts(&extracted_icu_template_parts.extracted_parts);
            Ok(())
        } else {
            // This message is a single string part. It may be an ICU template without
            // placeholders, or it may be a normal `goog.getMsg()` message.
            // Java's parseJsMessageTextIntoParts(null) throws NullPointerException.
            js_message_visitor::parse_js_message_text_into_parts(message_text.as_ref().unwrap())
                .map(|parts| {
                    js_message_builder.append_parts(&parts);
                })
        };
        if parts_result.is_err() {
            // Somehow we stored the protected message text incorrectly, which should never
            // happen 🙏
            panic!(
                "{}: Placeholder incorrectly formatted: >{}<",
                message_text_node.unwrap().get_location(ast),
                message_text.as_ref().unwrap()
            );
        }

        // Java's getExternalMessageId(null) throws NullPointerException.
        let msg_key = msg_key.unwrap();
        let external_message_id = js_message_visitor::get_external_message_id(&msg_key);
        if let Some(external_message_id) = external_message_id {
            // MSG_EXTERNAL_12345 = ...
            js_message_builder
                .set_is_external_msg(true)
                .set_id(external_message_id);
        } else {
            // NOTE: If the message was anonymous (assigned to a variable or property named
            // MSG_UNNAMED_XXX), the key we have here will be the one generated from the message
            // text, and we won't end up setting the isAnonymous flag. Nothing seems to use that
            // flag...
            // TODO(bradfordcsmith): Maybe remove the isAnonymous flag for jsMessage objects?
            let meaning_for_id_generation = match meaning {
                Some(meaning) => meaning,
                None => js_message_visitor::remove_scoped_aliases_prefix(&msg_key),
            };
            if let Some(id_generator) = id_generator {
                let id = id_generator
                    .generate_id(&meaning_for_id_generation, js_message_builder.get_parts());
                js_message_builder.set_id(id);
            } else {
                js_message_builder.set_id(meaning_for_id_generation);
            }
        }

        // port: ReplaceMessages.ProtectedJsMessage#ProtectedJsMessage
        Some(ProtectedJsMessage {
            js_message: js_message_builder.build(),
            definition_node: node,
            substitutions_node,
            is_icu_template,
            escape_less_than: escape_less_than_option,
            unescape_html_entities: unescape_html_entities_option,
        })
    }

    // port: ReplaceMessages.ProtectedJsMessage#getMsgOptions
    fn get_msg_options(&self) -> MsgOptions {
        MsgOptions {
            is_icu_template: self.is_icu_template,
            escape_less_than: self.escape_less_than,
            unescape_html_entities: self.unescape_html_entities,
        }
    }
}

/// Detects an ICU-formatted plural or select message. Any placeholders occurring inside these
/// messages must be rewritten in ICU format.
// port: ReplaceMessages#isStartOfIcuMessage
pub fn is_start_of_icu_message(part: &JsString) -> bool {
    // ICU messages start with a '{' followed by an identifier, followed by a ',' and then
    // 'plural' or 'select' followed by another comma.
    // the 'startsWith' check is redundant but should allow us to skip using the matcher
    if !part.starts_with("{") {
        return false;
    }
    let comma_index = part.index_of_from(",", 1);
    // if commaIndex == 1 that means the identifier is empty, which isn't allowed.
    if comma_index <= 1 {
        return false;
    }
    let next_bracket_index = part.index_of_from("{", 1);
    (next_bracket_index == -1 || next_bracket_index > comma_index)
        && (java_starts_with_at(part, "plural,", comma_index + 1)
            || java_starts_with_at(part, "select,", comma_index + 1))
}

/// Java's `String#startsWith(String prefix, int toffset)`.
fn java_starts_with_at(s: &JsString, prefix: &str, toffset: i32) -> bool {
    let prefix = JsString::from(prefix);
    let units = s.as_units();
    let prefix_units = prefix.as_units();
    // Note: toffset might be near -1>>>1.
    if toffset < 0 || i64::from(toffset) > units.len() as i64 - prefix_units.len() as i64 {
        return false;
    }
    let start = toffset as usize;
    &units[start..start + prefix_units.len()] == prefix_units
}

/// `String.valueOf(ImmutableSet<String>)`: `[a, b]`.
fn java_set_to_string(set: &IndexSet<JsString>) -> String {
    let items: Vec<String> = set.iter().map(|s| s.to_string_lossy()).collect();
    format!("[{}]", items.join(", "))
}
