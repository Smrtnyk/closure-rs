/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ReplaceStrings.java.

//! Port of `ReplaceStrings.java`.
//!
//! Replaces JavaScript strings in the list of supplied functions with shortened forms. Useful for
//! replacing debug message such as: throw new Error("Something bad happened"); with generated codes
//! like: throw new Error("a"); This makes the compiled JavaScript smaller and prevents us from
//! leaking details about the source code.
//!
//! Based in concept on the work by Jared Jacobs.

use crate::AbstractCompiler;
use crate::default_name_generator::DefaultNameGenerator;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::name_generator::NameGenerator;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::variable_map::VariableMap;
use closure_rhino::check_state;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::ir::IR;
use closure_rhino::java_lang::string::split;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use std::sync::{Arc, RwLock};

// port: ReplaceStrings#BAD_REPLACEMENT_CONFIGURATION
pub static BAD_REPLACEMENT_CONFIGURATION: DiagnosticType = DiagnosticType::error(
    "JSC_BAD_REPLACEMENT_CONFIGURATION",
    "Bad replacement configuration \"{0}\": {1}",
);

// port: ReplaceStrings#STRING_REPLACEMENT_TAGGED_TEMPLATE
pub static STRING_REPLACEMENT_TAGGED_TEMPLATE: DiagnosticType = DiagnosticType::warning(
    "JSC_STRING_REPLACEMENT_TAGGED_TEMPLATE",
    "Cannot string-replace arguments of a template literal tag function.",
);

// port: ReplaceStrings#DEFAULT_PLACEHOLDER_TOKEN
const DEFAULT_PLACEHOLDER_TOKEN: &str = "`";
// port: ReplaceStrings#EXCLUSION_PREFIX
pub const EXCLUSION_PREFIX: &str = ":!";
// port: ReplaceStrings#REPLACE_ONE_MARKER
const REPLACE_ONE_MARKER: &str = "?";
// port: ReplaceStrings#REPLACE_ALL_MARKER
const REPLACE_ALL_MARKER: &str = "*";

/// Describes a function to look for a which parameters to replace.
// port: ReplaceStrings.Config
pub struct Config {
    // TODO(johnlenz): Support name "groups" so that unrelated strings can
    // reuse strings.  For example, event-id can reuse the names used for logger
    // classes.
    name: JsString,
    parameters: Vec<i32>,
    excluded_filename_suffixes: IndexSet<String>,
}

impl Config {
    // port: ReplaceStrings.Config#REPLACE_ALL_VALUE
    const REPLACE_ALL_VALUE: i32 = 0;

    // port: ReplaceStrings.Config#Config
    fn new(
        name: JsString,
        replacement_parameters: Vec<i32>,
        excluded_filename_suffixes: IndexSet<String>,
    ) -> Self {
        Self {
            name,
            parameters: replacement_parameters,
            excluded_filename_suffixes,
        }
    }

    // port: ReplaceStrings.Config#isReplaceAll
    fn is_replace_all(&self) -> bool {
        self.parameters.len() == 1 && self.parameters.contains(&Self::REPLACE_ALL_VALUE)
    }
}

/// Describes a replacement that occurred.
// port: ReplaceStrings.Result
#[derive(Debug, Clone)]
pub struct Result {
    /// The original message with non-static content replaced with `placeholderToken`.
    pub original: JsString,
    pub replacement: JsString,
    pub did_replacement: bool,
}

impl Result {
    // port: ReplaceStrings.Result#Result
    fn new(original: JsString, replacement: JsString) -> Self {
        Self {
            original,
            replacement,
            did_replacement: false,
        }
    }
}

// port: ReplaceStrings#USED_RESULTS
fn used_results(result: &Result) -> bool {
    result.did_replacement
}

pub struct ReplaceStrings {
    placeholder_token: JsString,
    functions: IndexMap<JsString, Config>,
    name_generator: DefaultNameGenerator,
    results: IndexMap<JsString, Result>,
}

impl ReplaceStrings {
    /// `placeholder_token`: Separator to use between string parts. Used to replace non-static
    /// string content. `functions_to_inspect`: A list of function configurations in the form of
    /// function($,,,):exclued_filename_suffix1,excluded_filename_suffix2,... or
    /// class.prototype.method($,,,):exclued_filename_suffix1,excluded_filename_suffix2,...
    ///
    /// Java keeps the compiler in a field; the Rust pass takes it per call.
    // port: ReplaceStrings#ReplaceStrings
    pub fn new(
        compiler: &mut AbstractCompiler,
        placeholder_token: &str,
        functions_to_inspect: &[String],
    ) -> Self {
        let mut this = Self {
            placeholder_token: JsString::from(if placeholder_token.is_empty() {
                DEFAULT_PLACEHOLDER_TOKEN
            } else {
                placeholder_token
            }),
            functions: IndexMap::<_, _>::default(),
            name_generator: Self::create_name_generator(),
            results: IndexMap::<_, _>::default(),
        };

        // Initialize the map of functions to inspect for renaming candidates.
        this.parse_configuration_list(compiler, functions_to_inspect);
        this
    }

    /// Get the list of all replacements performed.
    // port: ReplaceStrings#getResult
    pub fn get_result(&self) -> Vec<Result> {
        self.results
            .values()
            .filter(|r| used_results(r))
            .cloned()
            .collect()
    }

    /// Get the list of replaces as a VariableMap
    // port: ReplaceStrings#getStringMap
    pub fn get_string_map(&self) -> VariableMap {
        let mut map: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();
        for result in self.results.values().filter(|r| used_results(r)) {
            // ImmutableMap.Builder#buildOrThrow rejects duplicate keys.
            let previous = map.insert(result.replacement.clone(), result.original.clone());
            check_state!(previous.is_none());
        }

        VariableMap::new(&map)
    }

    // port: ReplaceStrings#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }

    /// `name`: The function name to find. `callsite_source_file_name`: the filename containing
    /// the callsite. Returns the Config object for the name or None if no match was found.
    // port: ReplaceStrings#findMatching
    fn find_matching(
        &self,
        name: &JsString,
        callsite_source_file_name: Option<&str>,
    ) -> Option<&Config> {
        let mut config = self.functions.get(name);
        if config.is_none() {
            let name = name.replace(&JsString::from("$"), &JsString::from("."));
            config = self.functions.get(&name);
        }
        if let Some(config) = config {
            for excluded_suffix in &config.excluded_filename_suffixes {
                // Java dereferences the (non-null) callsite file name here.
                if callsite_source_file_name
                    .unwrap()
                    .ends_with(excluded_suffix.as_str())
                {
                    return None;
                }
            }
        }
        config
    }

    /// Replace the parameters specified in the config, if possible.
    // port: ReplaceStrings#doSubstitutions
    fn do_substitutions(&mut self, t: &mut NodeTraversal<'_>, config_name: &JsString, n: NodeId) {
        if n.is_tagged_template_lit(t) {
            // This is currently not supported, since tagged template literals have a different
            // calling convention than ordinary functions, so it's unclear which arguments are
            // expected to be replaced. Specifically, there are no direct string arguments, and
            // for arbitrary tag functions it's not clear that it's safe to inline any constant
            // placeholders.
            let error = JSError::make(t, n, &STRING_REPLACEMENT_TAGGED_TEMPLATE, &[]);
            t.get_compiler().report(error);
            return;
        }
        check_state!(n.is_new(t) || n.is_call(t));

        let config = &self.functions[config_name];
        if !config.is_replace_all() {
            let parameters = config.parameters.clone();
            // Note: the first child is the function, but the parameter id is 1 based.
            for parameter in parameters {
                let arg = n.get_child_at_index(t, parameter);
                if let Some(arg) = arg {
                    self.replace_expression(t, arg);
                }
            }
        } else {
            // Replace all parameters.
            let first_param = n.get_second_child(t);
            let mut arg = first_param;
            while let Some(a) = arg {
                let replaced = self.replace_expression(t, a);
                arg = replaced.get_next(t);
            }
        }
    }

    /// Replaces a string expression with a short encoded string expression.
    ///
    /// `t`: The traversal; `expr`: The expression node. Returns the replacement node (or the
    /// original expression if no replacement is made)
    // port: ReplaceStrings#replaceExpression
    fn replace_expression(&mut self, t: &mut NodeTraversal<'_>, expr: NodeId) -> NodeId {
        let replacement: NodeId;
        let key: JsString;
        let replacement_string: JsString;
        match expr.get_token(t) {
            Token::STRINGLIT => {
                key = expr.get_string(t);
                replacement_string = self.get_replacement(&key);
                replacement = IR::string(t, replacement_string.clone());
            }
            Token::TEMPLATELIT | Token::ADD | Token::NAME => {
                let mut key_builder: Vec<u16> = Vec::new();
                let key_node = IR::string(t, "");
                replacement = self.build_replacement(t, expr, key_node, &mut key_builder);
                key = JsString::from_units(key_builder);
                if key == self.placeholder_token {
                    // There is no static text in expr - only a placeholder - so just return expr
                    // directly. In this case, replacement is just the string join ('`' + expr),
                    // which is not useful.
                    return expr;
                }
                replacement_string = self.get_replacement(&key);
                key_node.set_string(t, replacement_string.clone());
            }
            _ => {
                // This may be a function call or a variable reference. We don't
                // replace these.
                return expr;
            }
        }

        // checkNotNull(key); checkNotNull(replacementString): both are non-null by type.
        self.record_replacement(&key);

        replacement.srcref_tree_if_missing(t, expr);
        expr.replace_with(t, replacement);
        t.report_code_change();
        replacement
    }

    /// Get a replacement string for the provide key text.
    // port: ReplaceStrings#getReplacement
    fn get_replacement(&mut self, key: &JsString) -> JsString {
        if let Some(result) = self.results.get(key) {
            return result.replacement.clone();
        }

        let replacement = self.name_generator.generate_next_name();
        let result = Result::new(key.clone(), replacement.clone());
        self.results.insert(key.clone(), result);
        replacement
    }

    /// Record the location the replacement was made.
    // port: ReplaceStrings#recordReplacement
    fn record_replacement(&mut self, key: &JsString) {
        let result = self.results.get_mut(key);
        check_state!(result.is_some());

        result.unwrap().did_replacement = true;
    }

    /// Builds a replacement abstract syntax tree for the string expression `expr`. Appends any
    /// string literal values that are encountered to `key_builder`, to build the expression's
    /// replacement key.
    ///
    /// `expr`: A JS expression that evaluates to a string value. `prefix`: The JS expression to
    /// which `expr`'s replacement is logically being concatenated. It is a partial solution to
    /// the problem at hand and will either be this method's return value or a descendant of it.
    /// `key_builder`: A builder of the string expression's replacement key. Returns the abstract
    /// syntax tree that should replace `expr`.
    // port: ReplaceStrings#buildReplacement
    fn build_replacement(
        &mut self,
        t: &mut NodeTraversal<'_>,
        expr: NodeId,
        mut prefix: NodeId,
        key_builder: &mut Vec<u16>,
    ) -> NodeId {
        match expr.get_token(t) {
            Token::ADD => {
                let left = expr.get_first_child(t).unwrap();
                let right = left.get_next(t).unwrap();
                prefix = self.build_replacement(t, left, prefix, key_builder);
                return self.build_replacement(t, right, prefix, key_builder);
            }
            Token::TEMPLATELIT => {
                let mut child = expr.get_first_child(t);
                while let Some(c) = child {
                    match c.get_token(t) {
                        Token::TEMPLATELIT_STRING => {
                            // StringBuilder#append(String) appends "null" for null.
                            match c.get_cooked_string(t) {
                                Some(cooked) => key_builder.extend_from_slice(cooked.as_units()),
                                None => key_builder.extend("null".encode_utf16()),
                            }
                        }
                        Token::TEMPLATELIT_SUB => {
                            let sub = c.get_first_child(t).unwrap();
                            prefix = self.build_replacement(t, sub, prefix, key_builder);
                        }
                        _ => panic!(
                            "IllegalStateException: Unexpected TEMPLATELIT child: {}",
                            c.to_string(t)
                        ),
                    }
                    child = c.get_next(t);
                }
                return prefix;
            }
            Token::STRINGLIT => {
                key_builder.extend_from_slice(expr.get_string_ref(t).as_units());
                return prefix;
            }
            Token::NAME => {
                // If the referenced variable is a constant, use its value.
                let name = expr.get_string(t);
                let scope = t.get_scope();
                let var = scope.get_var(t.get_compiler(), &name);
                if let Some(var) = var {
                    let compiler = t.get_compiler();
                    if var.is_declared_or_inferred_const(compiler) || var.is_const(compiler) {
                        let initial_value = var.get_initial_value(compiler);
                        if let Some(initial_value) = initial_value {
                            let new_key_node = IR::string(t, "");
                            let mut new_key_builder: Vec<u16> = Vec::new();
                            let replacement = self.build_replacement(
                                t,
                                initial_value,
                                new_key_node,
                                &mut new_key_builder,
                            );
                            if replacement == new_key_node {
                                key_builder.extend_from_slice(&new_key_builder);
                                return prefix;
                            }
                        }
                        // Not a simple string constant.
                    }
                }
                // fall-through
            }
            _ => {}
        }
        key_builder.extend_from_slice(self.placeholder_token.as_units());
        let placeholder = IR::string(t, self.placeholder_token.clone());
        prefix = IR::add(t, prefix, placeholder);
        let clone = expr.clone_tree(t);
        IR::add(t, prefix, clone)
    }

    /// Build the data structures need by this pass from the provided list of functions.
    // port: ReplaceStrings#parseConfiguration(List)
    fn parse_configuration_list(
        &mut self,
        compiler: &mut AbstractCompiler,
        functions_to_inspect: &[String],
    ) {
        for function in functions_to_inspect {
            let config = self.parse_configuration(compiler, function);
            if let Some(config) = config {
                self.functions.insert(config.name.clone(), config);
            }
        }
    }

    /// Convert the provide string into a Config. The string can be a variable or static function:
    /// foo(,,?) foo.bar(?) (but not a prototype method) and is allowed to either replace all
    /// parameters using "*" or one parameter "?". "," is used as a placeholder for ignored
    /// parameters.
    ///
    /// Returns None if this is an invalid function, otherwise the Config
    // port: ReplaceStrings#parseConfiguration(String)
    fn parse_configuration(
        &mut self,
        compiler: &mut AbstractCompiler,
        function: &str,
    ) -> Option<Config> {
        // Looks like this function_name(,$,)
        let function_units = JsString::from(function);
        let first = function_units.index_of_char(u16::from(b'('));
        let last = function_units.index_of_char(u16::from(b')'));
        let colon = function_units.index_of(JsString::from(EXCLUSION_PREFIX));

        // TODO(johnlenz): Make parsing precondition checks JSErrors reports.
        check_state!(first != -1 && last != -1);

        if function.contains(".prototype.") {
            compiler.report(JSError::make_without_location(
                &BAD_REPLACEMENT_CONFIGURATION,
                &[
                    function,
                    "Cannot replace strings passed to prototype methods.",
                ],
            ));
            return None;
        }

        let name = function_units.substring(0, first as usize);
        let params = function_units
            .substring(first as usize + 1, last as usize)
            .to_string_lossy();

        let mut param_count = 0;
        let mut replacement_parameters: Vec<i32> = Vec::new();
        // Splitter.on(',').splitToList keeps empty parts.
        let parts: Vec<&str> = params.split(',').collect();
        for param in &parts {
            param_count += 1;
            if *param == REPLACE_ALL_MARKER {
                check_state!(param_count == 1 && parts.len() == 1);
                replacement_parameters.push(Config::REPLACE_ALL_VALUE);
            } else if *param == REPLACE_ONE_MARKER {
                // TODO(johnlenz): Support multiple.
                check_state!(!replacement_parameters.contains(&Config::REPLACE_ALL_VALUE));
                replacement_parameters.push(param_count);
            } else {
                // TODO(johnlenz): report an error.
                check_state!(param.is_empty(), "Unknown marker ({})", param);
            }
        }

        check_state!(!replacement_parameters.is_empty());

        Some(Config::new(
            name,
            replacement_parameters,
            if colon == -1 {
                IndexSet::<_>::default()
            } else {
                split(
                    &function_units
                        .substring_from(colon as usize + EXCLUSION_PREFIX.len())
                        .to_string_lossy(),
                    ",",
                )
                .into_iter()
                .collect()
            },
        ))
    }

    /// Use a name generate to create names so the names overlap with the names used for variable
    /// and properties.
    // port: ReplaceStrings#createNameGenerator
    fn create_name_generator() -> DefaultNameGenerator {
        let name_prefix = "";
        let reserved_chars: IndexSet<u16> = IndexSet::<_>::default();
        DefaultNameGenerator::with_reserved_characters(
            Arc::new(RwLock::new(IndexSet::<_>::default())),
            JsString::from(name_prefix),
            &reserved_chars,
        )
    }
}

impl Callback for ReplaceStrings {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ReplaceStrings#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        // TODO(johnlenz): Determine if it is necessary to support ".call" or ".apply".
        match n.get_token(t) {
            // e.g. new Error('msg');
            // e.g. Error('msg');
            // e.g. Error`msg` - not supported!
            Token::NEW | Token::CALL | Token::TAGGED_TEMPLATELIT => {
                let called_fn = n.get_first_child(t).unwrap();

                // Look for calls to static functions.
                let name = called_fn.get_original_qualified_name(t);
                if let Some(name) = name {
                    let source_file_name = n.get_source_file_name(t);
                    let config_name = self
                        .find_matching(&name, source_file_name.as_deref())
                        .map(|config| config.name.clone());
                    if let Some(config_name) = config_name {
                        self.do_substitutions(t, &config_name, n);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
pub struct ReplaceStringsReplayFields<'a> {
    pub placeholder_token: &'a JsString,
    pub functions: &'a IndexMap<JsString, Config>,
    pub name_generator: &'a DefaultNameGenerator,
    pub results: &'a IndexMap<JsString, Result>,
}

/// Typed borrowing view of a `ReplaceStrings.Config` for java.lang.reflect.Field replay.
pub struct ConfigReplayFields<'a> {
    pub name: &'a JsString,
    pub parameters: &'a [i32],
    pub excluded_filename_suffixes: &'a IndexSet<String>,
}

impl ReplaceStrings {
    // port: java.lang.reflect.Field#get (native replay access)
    pub fn replay_fields(&self) -> ReplaceStringsReplayFields<'_> {
        ReplaceStringsReplayFields {
            placeholder_token: &self.placeholder_token,
            functions: &self.functions,
            name_generator: &self.name_generator,
            results: &self.results,
        }
    }
}

impl Config {
    // port: java.lang.reflect.Field#get (native replay access)
    pub fn replay_fields(&self) -> ConfigReplayFields<'_> {
        ConfigReplayFields {
            name: &self.name,
            parameters: &self.parameters,
            excluded_filename_suffixes: &self.excluded_filename_suffixes,
        }
    }
}
