/*
 * Copyright 2026 The closure-rs Authors.
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
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/ReplaceMessages.java,
//   test/com/google/javascript/jscomp/ReplaceMessagesTest.java.

//! Port of the replay helper `ReplaceMessagesTest_Helpers`
//! (`oracle/replay/helpers/com/google/javascript/jscomp/ReplaceMessagesTest_Helpers.java`, whose
//! `TEST_ID_GENERATOR` and `SimpleMessageBundle` are copied from `ReplaceMessagesTest.java` lines
//! 42-61 and 2741-2756), plus the replay adapters for `ReplaceMessages` and its passes
//! (descriptor `ReplaceMessagesTest.json`: `new ReplaceMessages(compiler, new
//! SimpleMessageBundle(), strictReplacement).get*Pass()`).
//!
//! Rust-only: Java's `SimpleMessageBundle` is a non-static inner class that reads its holder's
//! `messages` and `useTestIdGenerator` fields on every call. The holder is created by the DSL
//! `helper` expression, its fields are restored from the record's testFields, and nothing writes
//! them afterwards, so the Rust bundle takes them when it is constructed (`MessageBundle` must be
//! `Send + Sync`, which the replay's `Rc<RefCell<..>>` holder is not).
use crate::{
    replay::{
        options_fields::enum_name,
        options_values::OptionValue,
        registry::{BorrowedEntry, Entry},
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler::Compiler,
    js_message::{
        GrammaticalGenderCase, IdGenerator, JsMessage, Part, PlaceholderReference, StringPart,
    },
    message_bundle::MessageBundle,
    replace_messages::ReplaceMessages,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use std::{cell::RefCell, rc::Rc, sync::Arc};

const HOLDER: &str = "com.google.javascript.jscomp.ReplaceMessagesTest_Helpers";
const SIMPLE_MESSAGE_BUNDLE: &str =
    "com.google.javascript.jscomp.ReplaceMessagesTest_Helpers$SimpleMessageBundle";
const REPLACE_MESSAGES: &str = "com.google.javascript.jscomp.ReplaceMessages";
const NEW_REPLACE_MESSAGES: &str = "com.google.javascript.jscomp.ReplaceMessages#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.MessageBundle,boolean)";

// port: ReplayValues.Undecodable#Undecodable
fn bad(what: &str) -> Throwable {
    Throwable::HarnessError(format!("ReplaceMessagesTest_Helpers: {what}"))
}

/// Generate IDs of the form `MEANING_PARTCOUNT[PARTCOUNT...]`
/// PARTCOUNT = 'sN' for a string part with N == string length
/// PARTCOUNT = 'pN' for a placeholder with N == length of the canonical placeholder name
// port: ReplaceMessagesTest_Helpers#TEST_ID_GENERATOR
#[derive(Debug)]
pub struct TestIdGenerator;

impl IdGenerator for TestIdGenerator {
    // port: ReplaceMessagesTest_Helpers#TEST_ID_GENERATOR (generateId)
    fn generate_id(&self, meaning: &JsString, message_parts: &[Part]) -> JsString {
        let mut id_builder = meaning.to_string();
        id_builder.push('_');
        for message_part in message_parts {
            if message_part.is_placeholder() {
                id_builder.push('p');
                id_builder.push_str(
                    &message_part
                        .get_canonical_placeholder_name()
                        .length()
                        .to_string(),
                );
            } else {
                id_builder.push('s');
                id_builder.push_str(&message_part.get_string().length().to_string());
            }
        }

        JsString::from(id_builder)
    }
}

/// `private class SimpleMessageBundle implements MessageBundle`.
// port: ReplaceMessagesTest_Helpers.SimpleMessageBundle
pub struct SimpleMessageBundle {
    // Messages returned from fake bundle, keyed by `JsMessage.id`.
    messages: Option<IndexMap<JsString, JsMessage>>,
    use_test_id_generator: bool,
}

impl SimpleMessageBundle {
    /// Rust-only: the bundle with its holder's two fields (see the module comment).
    pub fn new(
        messages: Option<IndexMap<JsString, JsMessage>>,
        use_test_id_generator: bool,
    ) -> Self {
        Self {
            messages,
            use_test_id_generator,
        }
    }
}

impl MessageBundle for SimpleMessageBundle {
    // port: ReplaceMessagesTest_Helpers.SimpleMessageBundle#getMessage
    fn get_message(&self, id: &JsString) -> Option<&JsMessage> {
        self.messages
            .as_ref()
            .expect("NullPointerException: messages")
            .get(id)
    }

    // port: ReplaceMessagesTest_Helpers.SimpleMessageBundle#getAllMessages
    fn get_all_messages(&self) -> Vec<&JsMessage> {
        self.messages
            .as_ref()
            .expect("NullPointerException: messages")
            .values()
            .collect()
    }

    // port: ReplaceMessagesTest_Helpers.SimpleMessageBundle#idGenerator
    fn id_generator(&self) -> Option<Arc<dyn IdGenerator>> {
        if self.use_test_id_generator {
            Some(Arc::new(TestIdGenerator))
        } else {
            None
        }
    }
}

/// The helper holder `ReplaceMessagesTest_Helpers` with its two instance fields.
// port: ReplaceMessagesTest_Helpers
pub struct ReplaceMessagesTestHelpers {
    messages: Option<IndexMap<JsString, JsMessage>>,
    use_test_id_generator: bool,
}

impl NativeObject for ReplaceMessagesTestHelpers {
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#setField (ReplaceMessagesTest_Helpers)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match name {
            "messages" => self.messages = Option::decode_value(&value)?,
            "useTestIdGenerator" => self.use_test_id_generator = bool::decode_value(&value)?,
            _ => return Err(bad(&format!("no field {name}"))),
        }
        Ok(())
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The native `SimpleMessageBundle` the DSL passes to `ReplaceMessages`.
struct NativeSimpleMessageBundle(Arc<SimpleMessageBundle>);

impl NativeObject for NativeSimpleMessageBundle {
    fn class_name(&self) -> &str {
        SIMPLE_MESSAGE_BUNDLE
    }
    // port: UnitRecorder#isInstance (SimpleMessageBundle implements MessageBundle)
    fn is_instance_of(&self, class: &str) -> bool {
        class == SIMPLE_MESSAGE_BUNDLE || class == "com.google.javascript.jscomp.MessageBundle"
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The native `ReplaceMessages`; each `get*Pass()` consumes it (see `replace_messages.rs`).
struct NativeReplaceMessages(Option<ReplaceMessages>);

impl NativeObject for NativeReplaceMessages {
    fn class_name(&self) -> &str {
        REPLACE_MESSAGES
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl OptionValue for GrammaticalGenderCase {
    // port: ReplayValues#enumValue (JsMessage.GrammaticalGenderCase)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        Ok(GrammaticalGenderCase::value_of(Some(enum_name(value)?)))
    }
    // port: UnitRecorder#value (enum)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::Enum {
            class: "com.google.javascript.jscomp.JsMessage$GrammaticalGenderCase".into(),
            name: self.to_string(),
        })
    }
}

// port: ReplayValues#fields (decoded Java object)
fn object_fields(value: &DslValue, class: &str) -> Result<IndexMap<String, DslValue>, Throwable> {
    match value.untyped() {
        DslValue::Object(o) if o.borrow().class == class => Ok(o.borrow().fields.clone()),
        _ => Err(bad(&format!("not a {class}"))),
    }
}

// port: ReplayValues#findField
fn field<'a>(fields: &'a IndexMap<String, DslValue>, key: &str) -> Result<&'a DslValue, Throwable> {
    fields
        .get(key)
        .ok_or_else(|| bad(&format!("no field {key}")))
}

impl OptionValue for Part {
    // port: ReplayValues#object (JsMessage.StringPart / JsMessage.PlaceholderReference)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        const STRING_PART: &str = "com.google.javascript.jscomp.JsMessage$StringPart";
        const PLACEHOLDER: &str = "com.google.javascript.jscomp.JsMessage$PlaceholderReference";
        match value.untyped() {
            DslValue::Object(o) if o.borrow().class == STRING_PART => {
                let fields = object_fields(value, STRING_PART)?;
                Ok(Part::from(StringPart::new(JsString::decode_value(field(
                    &fields, "string",
                )?)?)))
            }
            DslValue::Object(o) if o.borrow().class == PLACEHOLDER => {
                let fields = object_fields(value, PLACEHOLDER)?;
                Ok(Part::from(PlaceholderReference::new(
                    JsString::decode_value(field(&fields, "storedPlaceholderName")?)?,
                    bool::decode_value(field(&fields, "canonicalFormat")?)?,
                )))
            }
            DslValue::Object(o) => Err(Throwable::Unported(o.borrow().class.clone())),
            _ => Err(bad("not a JsMessage.Part")),
        }
    }
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Err(Throwable::Unported(
            "com.google.javascript.jscomp.JsMessage$Part".into(),
        ))
    }
}

impl OptionValue for JsMessage {
    // port: ReplayValues#object (JsMessage: the AutoValue fields the recorder dumps)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        let fields = object_fields(value, "com.google.javascript.jscomp.JsMessage")?;
        Ok(JsMessage::new(
            Option::<String>::decode_value(field(&fields, "getSourceName")?)?,
            JsString::decode_value(field(&fields, "getKey")?)?,
            bool::decode_value(field(&fields, "isAnonymous")?)?,
            bool::decode_value(field(&fields, "isExternal")?)?,
            JsString::decode_value(field(&fields, "getId")?)?,
            Vec::<Part>::decode_value(field(&fields, "getParts")?)?,
            IndexMap::<GrammaticalGenderCase, Vec<Part>>::decode_value(field(
                &fields,
                "getGenderedMessagesMap",
            )?)?,
            Option::<JsString>::decode_value(field(&fields, "getAlternateId")?)?,
            Option::<JsString>::decode_value(field(&fields, "getDesc")?)?,
            Option::<JsString>::decode_value(field(&fields, "getMeaning")?)?,
            IndexMap::<JsString, JsString>::decode_value(field(
                &fields,
                "getPlaceholderNameToExampleMap",
            )?)?,
            IndexMap::<JsString, JsString>::decode_value(field(
                &fields,
                "getPlaceholderNameToOriginalCodeMap",
            )?)?,
            IndexSet::<JsString>::decode_value(field(&fields, "jsPlaceholderNames")?)?,
            IndexSet::<JsString>::decode_value(field(&fields, "canonicalPlaceholderNames")?)?,
        ))
    }
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Err(Throwable::Unported(
            "com.google.javascript.jscomp.JsMessage".into(),
        ))
    }
}

// port: ReplayDsl#invoke (resolved signatures backed by the ReplaceMessagesTest helpers)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        NEW_REPLACE_MESSAGES => new_replace_messages,
        "com.google.javascript.jscomp.ReplaceMessages#getFullReplacementPass()" => {
            get_full_replacement_pass
        }
        "com.google.javascript.jscomp.ReplaceMessages#getMsgProtectionPass()" => {
            get_msg_protection_pass
        }
        "com.google.javascript.jscomp.ReplaceMessages#getReplacementCompletionPass()" => {
            get_replacement_completion_pass
        }
        "com.google.javascript.jscomp.ReplaceMessagesTest_Helpers$SimpleMessageBundle#<init>(com.google.javascript.jscomp.ReplaceMessagesTest_Helpers)" => {
            new_simple_message_bundle
        }
        _ => return None,
    })
}

/// Registrations outside the resolved TSV rows: the helper holder's no-argument constructor
/// (`ReplayValues#instantiate`) and the `ReplaceMessages` constructor under a factory's compiler
/// loan.
pub fn register(registry: &mut crate::replay::registry::Registry) {
    registry.register(&format!("{HOLDER}#<init>()"), new_holder);
    let borrowed: BorrowedEntry = new_replace_messages_borrowed;
    registry.register_with_compiler(NEW_REPLACE_MESSAGES, borrowed);
}

// port: ReplayValues#instantiate (ReplaceMessagesTest_Helpers)
fn new_holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        ReplaceMessagesTestHelpers {
            messages: None,
            use_test_id_generator: false,
        },
    ))))
}

// port: ReplaceMessagesTest_Helpers.SimpleMessageBundle#SimpleMessageBundle
fn new_simple_message_bundle(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Native(outer)] = args.as_slice() else {
        return Err(bad("SimpleMessageBundle needs its holder"));
    };
    let mut outer = outer.borrow_mut();
    let holder = outer
        .as_any_mut()
        .downcast_mut::<ReplaceMessagesTestHelpers>()
        .ok_or_else(|| bad("SimpleMessageBundle holder has the wrong class"))?;
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeSimpleMessageBundle(Arc::new(SimpleMessageBundle::new(
            holder.messages.clone(),
            holder.use_test_id_generator,
        ))),
    ))))
}

// port: ReplayValues#decode (MessageBundle argument)
fn message_bundle(value: &DslValue) -> Result<Arc<dyn MessageBundle + Send + Sync>, Throwable> {
    if let DslValue::Native(o) = value {
        let mut o = o.borrow_mut();
        if let Some(bundle) = o.as_any_mut().downcast_mut::<NativeSimpleMessageBundle>() {
            return Ok(bundle.0.clone());
        }
    }
    Arc::<dyn MessageBundle + Send + Sync>::decode_value(value)
}

// port: ReplaceMessages#ReplaceMessages (replay constructor)
fn construct(compiler: &mut Compiler, args: &[DslValue]) -> Result<DslValue, Throwable> {
    let [_, bundle, strict_replacement] = args else {
        return Err(bad("ReplaceMessages takes three arguments"));
    };
    let bundle = message_bundle(bundle)?;
    let strict_replacement = bool::decode_value(strict_replacement)?;
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeReplaceMessages(Some(ReplaceMessages::new(
            compiler,
            bundle,
            strict_replacement,
        ))),
    ))))
}

// port: ReplaceMessages#ReplaceMessages
fn new_replace_messages(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let Some(DslValue::Compiler(compiler)) = args.first() else {
        return Err(bad("ReplaceMessages needs a compiler"));
    };
    let compiler: CompilerHandle = compiler.clone();
    let mut compiler = compiler.borrow_mut();
    construct(&mut compiler, &args)
}

// port: ReplaceMessages#ReplaceMessages (under the compiler a pass factory has borrowed)
fn new_replace_messages_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    construct(compiler, &args)
}

// port: ReplayDsl#invoke (ReplaceMessages receiver cast)
fn take_receiver(args: &[DslValue]) -> Result<ReplaceMessages, Throwable> {
    let Some(DslValue::Native(receiver)) = args.first() else {
        return Err(bad("get*Pass needs a ReplaceMessages receiver"));
    };
    let mut receiver = receiver.borrow_mut();
    receiver
        .as_any_mut()
        .downcast_mut::<NativeReplaceMessages>()
        .ok_or_else(|| bad("receiver is not a ReplaceMessages"))?
        .0
        .take()
        .ok_or_else(|| bad("this ReplaceMessages already handed out its pass"))
}

// port: ReplaceMessages#getFullReplacementPass
fn get_full_replacement_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let pass = take_receiver(&args)?.get_full_replacement_pass();
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: ReplaceMessages#getMsgProtectionPass
fn get_msg_protection_pass(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let pass = take_receiver(&args)?.get_msg_protection_pass();
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}

// port: ReplaceMessages#getReplacementCompletionPass
fn get_replacement_completion_pass(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let pass = take_receiver(&args)?.get_replacement_completion_pass();
    Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(pass)))))
}
