/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/ProcessTweaks.java.

//! Port of `ProcessTweaks.java`.
//!
//! Process goog.tweak primitives. Checks that:
//!
//! - parameters to goog.tweak.register* are literals of the correct type.
//! - the parameter to goog.tweak.get* is a string literal.
//! - parameters to goog.tweak.overrideDefaultValue are literals of the correct type.
//! - tweak IDs passed to goog.tweak.get* and goog.tweak.overrideDefaultValue correspond to
//!   registered tweaks.
//! - all calls to goog.tweak.register* and goog.tweak.overrideDefaultValue are within the
//!   top-level context.
//! - each tweak is registered only once.
//! - calls to goog.tweak.overrideDefaultValue occur before the call to the corresponding
//!   goog.tweak.register* function.

use crate::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::Callback;
use crate::node_traversal::NodeTraversal;
use closure_rhino::check_not_null;
use closure_rhino::check_state;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::sync::LazyLock;

// port: ProcessTweaks#ID_MATCHER
fn id_matcher_matches(c: u16) -> bool {
    // CharMatcher.inRange('a', 'z').or(CharMatcher.inRange('A', 'Z'))
    //     .or(CharMatcher.anyOf("0123456789_."))
    (u16::from(b'a')..=u16::from(b'z')).contains(&c)
        || (u16::from(b'A')..=u16::from(b'Z')).contains(&c)
        || "0123456789_.".encode_utf16().any(|m| m == c)
}

// port: CharMatcher#matchesAllOf (ID_MATCHER)
fn id_matcher_matches_all_of(sequence: &JsString) -> bool {
    (0..sequence.length()).all(|i| id_matcher_matches(sequence.char_at(i)))
}

// Warnings and Errors.
// port: ProcessTweaks#TWEAK_MULTIPLY_REGISTERED_ERROR
pub static TWEAK_MULTIPLY_REGISTERED_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_TWEAK_MULTIPLY_REGISTERED_ERROR",
    "Tweak {0} has already been registered.",
);

// port: ProcessTweaks#NON_LITERAL_TWEAK_ID_ERROR
pub static NON_LITERAL_TWEAK_ID_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_NON_LITERAL_TWEAK_ID_ERROR",
    "tweak ID must be a string literal",
);

// port: ProcessTweaks#INVALID_TWEAK_DEFAULT_VALUE_WARNING
pub static INVALID_TWEAK_DEFAULT_VALUE_WARNING: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_TWEAK_DEFAULT_VALUE_WARNING",
    "tweak {0} registered with {1} must have a default value that is a literal of type {2}",
);

// port: ProcessTweaks#NON_GLOBAL_TWEAK_INIT_ERROR
pub static NON_GLOBAL_TWEAK_INIT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_NON_GLOBAL_TWEAK_INIT_ERROR",
    "tweak declaration {0} must occur in the global scope",
);

// port: ProcessTweaks#TWEAK_WRONG_GETTER_TYPE_WARNING
pub static TWEAK_WRONG_GETTER_TYPE_WARNING: DiagnosticType = DiagnosticType::warning(
    "JSC_TWEAK_WRONG_GETTER_TYPE_WARNING",
    "tweak getter function {0} used for tweak registered using {1}",
);

// port: ProcessTweaks#INVALID_TWEAK_ID_ERROR
pub static INVALID_TWEAK_ID_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_TWEAK_ID_ERROR",
    "tweak ID contains illegal characters. Only letters, numbers, _ and . are allowed",
);

/// An enum of goog.tweak functions.
// port: ProcessTweaks.TweakFunction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TweakFunction {
    REGISTER_BOOLEAN,
    REGISTER_NUMBER,
    REGISTER_STRING,
    GET_BOOLEAN,
    GET_NUMBER,
    GET_STRING,
}

/// The constructor arguments of one `TweakFunction` constant.
struct TweakFunctionData {
    name: &'static str,
    expected_type_name: Option<&'static str>,
    valid_node_type_a: Token,
    valid_node_type_b: Token,
    register_function: Option<TweakFunction>,
}

impl TweakFunction {
    // port: ProcessTweaks.TweakFunction#values (EnumSet.allOf order)
    const VALUES: [TweakFunction; 6] = [
        TweakFunction::REGISTER_BOOLEAN,
        TweakFunction::REGISTER_NUMBER,
        TweakFunction::REGISTER_STRING,
        TweakFunction::GET_BOOLEAN,
        TweakFunction::GET_NUMBER,
        TweakFunction::GET_STRING,
    ];

    // port: ProcessTweaks.TweakFunction#TweakFunction
    fn data(self) -> TweakFunctionData {
        // TweakFunction(String name, String expectedTypeName, Token validNodeTypeA)
        //   -> this(name, expectedTypeName, validNodeTypeA, Token.EMPTY, null)
        // TweakFunction(String name, TweakFunction registerFunction)
        //   -> this(name, null, Token.EMPTY, Token.EMPTY, registerFunction)
        match self {
            TweakFunction::REGISTER_BOOLEAN => TweakFunctionData {
                name: "goog.tweak.registerBoolean",
                expected_type_name: Some("boolean"),
                valid_node_type_a: Token::TRUE,
                valid_node_type_b: Token::FALSE,
                register_function: None,
            },
            TweakFunction::REGISTER_NUMBER => TweakFunctionData {
                name: "goog.tweak.registerNumber",
                expected_type_name: Some("number"),
                valid_node_type_a: Token::NUMBER,
                valid_node_type_b: Token::EMPTY,
                register_function: None,
            },
            TweakFunction::REGISTER_STRING => TweakFunctionData {
                name: "goog.tweak.registerString",
                expected_type_name: Some("string"),
                valid_node_type_a: Token::STRINGLIT,
                valid_node_type_b: Token::EMPTY,
                register_function: None,
            },
            TweakFunction::GET_BOOLEAN => TweakFunctionData {
                name: "goog.tweak.getBoolean",
                expected_type_name: None,
                valid_node_type_a: Token::EMPTY,
                valid_node_type_b: Token::EMPTY,
                register_function: Some(TweakFunction::REGISTER_BOOLEAN),
            },
            TweakFunction::GET_NUMBER => TweakFunctionData {
                name: "goog.tweak.getNumber",
                expected_type_name: None,
                valid_node_type_a: Token::EMPTY,
                valid_node_type_b: Token::EMPTY,
                register_function: Some(TweakFunction::REGISTER_NUMBER),
            },
            TweakFunction::GET_STRING => TweakFunctionData {
                name: "goog.tweak.getString",
                expected_type_name: None,
                valid_node_type_a: Token::EMPTY,
                valid_node_type_b: Token::EMPTY,
                register_function: Some(TweakFunction::REGISTER_STRING),
            },
        }
    }

    // port: ProcessTweaks.TweakFunction#isValidNodeType
    fn is_valid_node_type(self, type_: Token) -> bool {
        let data = self.data();
        type_ == data.valid_node_type_a || type_ == data.valid_node_type_b
    }

    // port: ProcessTweaks.TweakFunction#isCorrectRegisterFunction
    fn is_correct_register_function(self, register_function: Option<TweakFunction>) -> bool {
        check_not_null!(register_function);
        self.data().register_function == register_function
    }

    // port: ProcessTweaks.TweakFunction#isGetterFunction
    fn is_getter_function(self) -> bool {
        self.data().register_function.is_some()
    }

    // port: ProcessTweaks.TweakFunction#getName
    fn get_name(self) -> &'static str {
        self.data().name
    }

    // port: ProcessTweaks.TweakFunction#getExpectedTypeName
    fn get_expected_type_name(self) -> Option<&'static str> {
        self.data().expected_type_name
    }

    // port: ProcessTweaks.TweakFunction#createDefaultValueNode
    fn create_default_value_node(self, ast: &mut Ast) -> NodeId {
        match self {
            TweakFunction::REGISTER_BOOLEAN => IR::false_node(ast),
            TweakFunction::REGISTER_NUMBER => IR::number(ast, 0.0),
            TweakFunction::REGISTER_STRING => IR::string(ast, ""),
            _ => panic!("IllegalStateException"),
        }
    }
}

// A map of function name -> TweakFunction.
// port: ProcessTweaks#TWEAK_FUNCTIONS_MAP
static TWEAK_FUNCTIONS_MAP: LazyLock<IndexMap<&'static str, TweakFunction>> = LazyLock::new(|| {
    let mut map = IndexMap::<_, _>::default();
    for func in TweakFunction::VALUES {
        map.insert(func.get_name(), func);
    }
    map
});

pub struct ProcessTweaks {
    strip_tweaks: bool,
}

impl ProcessTweaks {
    // port: ProcessTweaks#ProcessTweaks
    pub fn new(strip_tweaks: bool) -> Self {
        Self { strip_tweaks }
    }

    // port: ProcessTweaks#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let result = self.collect_tweaks(compiler, root);

        if self.strip_tweaks {
            self.strip_all_calls(compiler, &result.tweak_infos);
        }
    }

    /// Removes all CALL nodes in the given TweakInfos, replacing calls to getter functions with
    /// the tweak's default value.
    // port: ProcessTweaks#stripAllCalls
    fn strip_all_calls(
        &self,
        compiler: &mut AbstractCompiler,
        tweak_infos: &IndexMap<JsString, TweakInfo>,
    ) {
        for tweak_info in tweak_infos.values() {
            let is_registered = tweak_info.is_registered();
            for function_call in &tweak_info.function_calls {
                let call_node = function_call.call_node;
                let parent = call_node.get_parent(compiler).unwrap();
                if function_call.tweak_func.is_getter_function() {
                    let new_value = if is_registered {
                        let default_value_node = tweak_info.get_default_value_node(compiler);
                        default_value_node.clone_node(compiler)
                    } else {
                        // When we find a getter of an unregistered tweak, there has
                        // already been a warning about it, so now just use a default
                        // value when stripping.
                        let register_function =
                            function_call.tweak_func.data().register_function.unwrap();
                        register_function.create_default_value_node(compiler)
                    };
                    call_node.replace_with(compiler, new_value);
                    compiler.report_change_to_enclosing_scope(parent);
                } else {
                    let zero = IR::number(compiler, 0.0).srcref(compiler, call_node);
                    let void_zero_node = IR::void_node(compiler, zero).srcref(compiler, call_node);
                    call_node.replace_with(compiler, void_zero_node);
                    compiler.report_change_to_enclosing_scope(parent);
                }
            }
        }
    }

    /// Finds all calls to goog.tweak functions and emits warnings/errors if any of the calls
    /// have issues.
    ///
    /// Returns a map of `TweakInfo` structures, keyed by tweak ID.
    // port: ProcessTweaks#collectTweaks
    fn collect_tweaks(&self, compiler: &mut AbstractCompiler, root: NodeId) -> CollectTweaksResult {
        let mut pass = CollectTweaks {
            all_tweaks: IndexMap::<_, _>::default(),
        };
        NodeTraversal::traverse(compiler, root, &mut pass);

        let tweak_infos = pass.all_tweaks;
        for tweak_info in tweak_infos.values() {
            tweak_info.emit_all_warnings(compiler);
        }
        CollectTweaksResult::new(tweak_infos)
    }
}

impl CompilerPass for ProcessTweaks {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::process(self, compiler, externs, root);
    }
}

// port: ProcessTweaks.CollectTweaksResult
struct CollectTweaksResult {
    tweak_infos: IndexMap<JsString, TweakInfo>,
}

impl CollectTweaksResult {
    // port: ProcessTweaks.CollectTweaksResult#CollectTweaksResult
    fn new(tweak_infos: IndexMap<JsString, TweakInfo>) -> Self {
        Self { tweak_infos }
    }
}

/// Processes all calls to goog.tweak functions.
// port: ProcessTweaks.CollectTweaks
struct CollectTweaks {
    all_tweaks: IndexMap<JsString, TweakInfo>,
}

impl Callback for CollectTweaks {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ProcessTweaks.CollectTweaks#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !n.is_call(t) {
            return;
        }

        let call_name = n.get_first_child(t).unwrap().get_qualified_name(t);
        let Some(tweak_func) = call_name
            .and_then(|call_name| TWEAK_FUNCTIONS_MAP.get(call_name.to_string_lossy().as_str()))
            .copied()
        else {
            return;
        };

        // Ensure the first parameter (the tweak ID) is a string literal.
        let tweak_id_node = n.get_second_child(t).unwrap();
        if !tweak_id_node.is_string_lit(t) {
            let error = JSError::make(t, tweak_id_node, &NON_LITERAL_TWEAK_ID_ERROR, &[]);
            t.get_compiler().report(error);
            return;
        }
        let tweak_id = tweak_id_node.get_string(t);

        // Make sure there is a TweakInfo structure for it.
        let tweak_info = self
            .all_tweaks
            .entry(tweak_id.clone())
            .or_insert_with(|| TweakInfo::new(tweak_id.clone()));

        match tweak_func {
            TweakFunction::REGISTER_BOOLEAN
            | TweakFunction::REGISTER_NUMBER
            | TweakFunction::REGISTER_STRING => {
                // Ensure the ID contains only valid characters.
                if !id_matcher_matches_all_of(&tweak_id) {
                    let error = JSError::make(t, tweak_id_node, &INVALID_TWEAK_ID_ERROR, &[]);
                    t.get_compiler().report(error);
                }

                // Ensure tweaks are registered in the global scope.
                if !t.in_global_hoist_scope() {
                    let error =
                        JSError::make(t, n, &NON_GLOBAL_TWEAK_INIT_ERROR, &[&tweak_id.to_string()]);
                    t.get_compiler().report(error);
                    return;
                }

                // Ensure tweaks are registered only once.
                if tweak_info.is_registered() {
                    let error = JSError::make(
                        t,
                        n,
                        &TWEAK_MULTIPLY_REGISTERED_ERROR,
                        &[&tweak_id.to_string()],
                    );
                    t.get_compiler().report(error);
                    return;
                }

                let tweak_default_value_node =
                    tweak_id_node.get_next(t).and_then(|next| next.get_next(t));
                tweak_info.add_register_call(tweak_func, n, tweak_default_value_node);
            }
            TweakFunction::GET_BOOLEAN | TweakFunction::GET_NUMBER | TweakFunction::GET_STRING => {
                tweak_info.add_getter_call(tweak_func, n)
            }
        }
    }
}

/// Holds information about a call to a goog.tweak function.
// port: ProcessTweaks.TweakFunctionCall
#[derive(Clone, Copy)]
struct TweakFunctionCall {
    tweak_func: TweakFunction,
    call_node: NodeId,
    value_node: Option<NodeId>,
}

impl TweakFunctionCall {
    // port: ProcessTweaks.TweakFunctionCall#TweakFunctionCall(TweakFunction,Node)
    fn new(tweak_func: TweakFunction, call_node: NodeId) -> Self {
        Self::new_with_value(tweak_func, call_node, None)
    }

    // port: ProcessTweaks.TweakFunctionCall#TweakFunctionCall(TweakFunction,Node,Node)
    fn new_with_value(
        tweak_func: TweakFunction,
        call_node: NodeId,
        value_node: Option<NodeId>,
    ) -> Self {
        Self {
            call_node,
            tweak_func,
            value_node,
        }
    }
}

/// Stores information about a single tweak.
// port: ProcessTweaks.TweakInfo
struct TweakInfo {
    tweak_id: JsString,
    function_calls: Vec<TweakFunctionCall>,
    register_call: Option<TweakFunctionCall>,
    default_value_node: Option<NodeId>,
}

impl TweakInfo {
    // port: ProcessTweaks.TweakInfo#TweakInfo
    fn new(tweak_id: JsString) -> Self {
        Self {
            tweak_id,
            function_calls: Vec::new(),
            register_call: None,
            default_value_node: None,
        }
    }

    /// If this tweak is registered, then looks for type warnings in default value parameters
    /// and getter functions. If it is not registered, emits an error for each function call.
    // port: ProcessTweaks.TweakInfo#emitAllWarnings
    fn emit_all_warnings(&self, compiler: &mut AbstractCompiler) {
        if self.is_registered() {
            self.emit_all_type_warnings(compiler);
        }
    }

    /// Emits a warning for each default value parameter that has the wrong type and for each
    /// getter function that was used for the wrong type of tweak.
    // port: ProcessTweaks.TweakInfo#emitAllTypeWarnings
    fn emit_all_type_warnings(&self, compiler: &mut AbstractCompiler) {
        for call in &self.function_calls {
            let value_node = call.value_node;
            let tweak_func = call.tweak_func;
            let register_func = self.register_call.unwrap().tweak_func;
            if let Some(value_node) = value_node {
                // For register* and overrideDefaultValue calls, ensure the default
                // value is a literal of the correct type.
                if !register_func.is_valid_node_type(value_node.get_token(compiler)) {
                    let error = JSError::make(
                        compiler,
                        value_node,
                        &INVALID_TWEAK_DEFAULT_VALUE_WARNING,
                        &[
                            &self.tweak_id.to_string_lossy(),
                            register_func.get_name(),
                            register_func.get_expected_type_name().unwrap_or("null"),
                        ],
                    );
                    compiler.report(error);
                }
            } else if tweak_func.is_getter_function() {
                // For getter calls, ensure the correct getter was used.
                if !tweak_func.is_correct_register_function(Some(register_func)) {
                    let error = JSError::make(
                        compiler,
                        call.call_node,
                        &TWEAK_WRONG_GETTER_TYPE_WARNING,
                        &[tweak_func.get_name(), register_func.get_name()],
                    );
                    compiler.report(error);
                }
            }
        }
    }

    // port: ProcessTweaks.TweakInfo#addRegisterCall
    fn add_register_call(
        &mut self,
        tweak_func: TweakFunction,
        call_node: NodeId,
        default_value_node: Option<NodeId>,
    ) {
        let register_call =
            TweakFunctionCall::new_with_value(tweak_func, call_node, default_value_node);
        self.register_call = Some(register_call);
        self.function_calls.push(register_call);
    }

    // port: ProcessTweaks.TweakInfo#addGetterCall
    fn add_getter_call(&mut self, tweak_func: TweakFunction, call_node: NodeId) {
        self.function_calls
            .push(TweakFunctionCall::new(tweak_func, call_node));
    }

    // port: ProcessTweaks.TweakInfo#isRegistered
    fn is_registered(&self) -> bool {
        self.register_call.is_some()
    }

    // port: ProcessTweaks.TweakInfo#getDefaultValueNode
    fn get_default_value_node(&self, ast: &mut Ast) -> NodeId {
        check_state!(self.is_registered());
        // Use calls to goog.tweak.overrideDefaultValue() first.
        if let Some(default_value_node) = self.default_value_node {
            return default_value_node;
        }
        let register_call = self.register_call.unwrap();
        // Use the value passed to the register function next.
        if let Some(value_node) = register_call.value_node {
            return value_node;
        }
        // Otherwise, use the default value for the tweak's type.
        register_call.tweak_func.create_default_value_node(ast)
    }
}
