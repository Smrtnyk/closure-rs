/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/StripCode.java.

//! Port of `StripCode.java`.
//!
//! A pass for stripping a list of provided JavaScript object types.
//!
//! The stripping strategy is as follows:
//!
//! - Provide: 1) a list of types that should be stripped, and 2) a list of suffixes of
//!   field/variable names that should be stripped.
//! - Remove declarations of variables that are initialized using static methods of strip types
//!   (e.g. var x = goog.debug.Logger.getLogger(...);).
//! - Remove all references to variables that are stripped.
//! - Remove all object literal keys with strip names.
//! - Remove all assignments to 1) field names that are strip names and 2) qualified names that
//!   begin with strip types.
//! - Remove all statements containing calls to static methods of strip types.

use crate::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic::log_file::LogFile;
use crate::diagnostic_type::DiagnosticType;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::ir::IR;
use closure_rhino::java_lang;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_state};

// port: StripCode#STRIP_TYPE_INHERIT_ERROR
pub static STRIP_TYPE_INHERIT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_STRIP_TYPE_INHERIT_ERROR",
    "Non-strip type {0} cannot inherit from strip type {1}",
);

// port: StripCode#STRIP_ASSIGNMENT_ERROR
pub static STRIP_ASSIGNMENT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_STRIP_ASSIGNMENT_ERROR",
    "Unable to strip assignment to {0}",
);

/// `String#toLowerCase(Locale.ROOT)`: full Unicode lower-casing (final sigma and U+0130
/// included), keeping unpaired surrogates as they are.
// port: String#toLowerCase(Locale)
fn to_lower_case_root(s: &JsString) -> JsString {
    let mut decoded = String::new();
    let mut units: Vec<u16> = Vec::with_capacity(s.length());
    let flush = |decoded: &mut String, units: &mut Vec<u16>| {
        if !decoded.is_empty() {
            units.extend(decoded.to_lowercase().encode_utf16());
            decoded.clear();
        }
    };
    for c in char::decode_utf16(s.as_units().iter().copied()) {
        match c {
            Ok(c) => decoded.push(c),
            Err(e) => {
                flush(&mut decoded, &mut units);
                units.push(e.unpaired_surrogate());
            }
        }
    }
    flush(&mut decoded, &mut units);
    JsString::from_units(units)
}

/// Java's `"" + string` for a possibly-null String.
fn str_or_null(s: Option<&JsString>) -> String {
    s.map_or_else(|| "null".to_owned(), ToString::to_string)
}

pub struct StripCode {
    strip_name_suffixes: IndexSet<JsString>,
    strip_name_prefixes: IndexSet<JsString>,
    vars_to_remove: IndexSet<JsString>,

    strip_types_list: Vec<JsString>,
    strip_type_prefixes_list: Vec<JsString>,
    strip_name_prefixes_lower_case_list: Vec<JsString>,
    strip_name_suffixes_lower_case_list: Vec<JsString>,
}

impl StripCode {
    /// Returns a stream containing `s` and, if `s` contains a ".", also all the possible collapsed
    /// forms of `s`. (e.g. for `'a.b.c'` we generate `'a$b.c'` and `'a$b$c'`)
    ///
    /// StripCode now runs after `CollapseProperties`, so it needs to look for both the original and
    /// collapsed versions of qualified names.
    // port: StripCode#toStreamWithCollapsedVersions
    fn to_stream_with_collapsed_versions(s: &JsString) -> Vec<JsString> {
        let mut possible_forms: Vec<JsString> = Vec::new();
        possible_forms.push(s.clone());
        let mut chars: Vec<u16> = s.as_units().to_vec();
        for i in 0..chars.len() {
            if chars[i] == u16::from(b'.') {
                chars[i] = u16::from(b'$');
                possible_forms.push(JsString::from_units(chars.clone()));
            }
        }
        possible_forms
    }

    /// Creates an instance.
    // port: StripCode#StripCode
    pub fn new(
        strip_types: &IndexSet<String>,
        strip_name_suffixes: &IndexSet<String>,
        strip_name_prefixes: &IndexSet<String>,
        enable_tweak_stripping: bool,
    ) -> Self {
        let strip_name_suffixes: IndexSet<JsString> = strip_name_suffixes
            .iter()
            .flat_map(|s| Self::to_stream_with_collapsed_versions(&JsString::from(s.as_str())))
            .collect();
        let strip_name_prefixes: IndexSet<JsString> = strip_name_prefixes
            .iter()
            .flat_map(|s| Self::to_stream_with_collapsed_versions(&JsString::from(s.as_str())))
            .collect();

        let mut strip_types_stream: Vec<JsString> = strip_types
            .iter()
            .map(|s| JsString::from(s.as_str()))
            .collect();
        // Add "tweak" class stripping if requested
        if enable_tweak_stripping {
            strip_types_stream.push(JsString::from("goog.tweak"));
        }
        let strip_types_adjusted: IndexSet<JsString> = strip_types_stream
            .iter()
            .flat_map(Self::to_stream_with_collapsed_versions)
            .collect();

        // Iteration overhead was a high cost in this pass. Using a native array
        // is trivial and avoid those costs.
        let strip_types_list: Vec<JsString> = strip_types_adjusted.iter().cloned().collect();

        // We want to strip types that are defined on a type that is being stripped, otherwise the
        // resulting code will be invalid. So, we'll also check for prefixes that indicate such child
        // names.
        // TODO(johnlenz): I'm not sure what the original intent of "type prefix" stripping was.
        // Verify that we can always assume a complete namespace and simplify this logic.
        let strip_type_prefixes_list: Vec<JsString> =
            // look for both non-collapsed and collapsed child names
            strip_types_adjusted
                .iter()
                .flat_map(|s| [with_suffix(s, b'.'), with_suffix(s, b'$')])
                .collect();

        // Precalculate the lowercase versions of the string to avoid repeated
        // lowercase conversions.
        let strip_name_prefixes_lower_case_list: Vec<JsString> =
            strip_name_prefixes.iter().map(to_lower_case_root).collect();

        let strip_name_suffixes_lower_case_list: Vec<JsString> =
            strip_name_suffixes.iter().map(to_lower_case_root).collect();

        Self {
            strip_name_suffixes,
            strip_name_prefixes,
            vars_to_remove: IndexSet::<_>::default(),
            strip_types_list,
            strip_type_prefixes_list,
            strip_name_prefixes_lower_case_list,
            strip_name_suffixes_lower_case_list,
        }
    }

    // port: StripCode#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        let mut decisions_log =
            compiler.create_or_reopen_indexed_log("StripCode", "decisions.log", &[]);
        decisions_log.log(&mut || self.strip_code_config_record());
        decisions_log.log_string("\n=== decisions ===\n");
        {
            let mut strip = Strip::new(self, &mut *decisions_log);
            NodeTraversal::traverse(compiler, root, &mut strip);
        }
        decisions_log.close();
    }

    // port: StripCode.StripCodeConfigRecord#get
    fn strip_code_config_record(&self) -> String {
        let mut builder = String::new();
        builder.push_str("=== stripNameSuffixes ===\n");
        for strip_name_suffix in &self.strip_name_suffixes {
            builder.push_str(&strip_name_suffix.to_string());
            builder.push('\n');
        }
        builder.push('\n');
        builder.push_str("=== stripNamePrefixes ===\n");
        for strip_name_prefix in &self.strip_name_prefixes {
            builder.push_str(&strip_name_prefix.to_string());
            builder.push('\n');
        }
        builder.push('\n');
        builder.push_str("=== stripTypesList ===\n");
        for strip_type in &self.strip_types_list {
            builder.push_str(&strip_type.to_string());
            builder.push('\n');
        }
        builder.push('\n');
        builder.push_str("=== stripTypePrefixesList ===\n");
        for strip_name_prefix in &self.strip_type_prefixes_list {
            builder.push_str(&strip_name_prefix.to_string());
            builder.push('\n');
        }
        builder.push('\n');
        builder
    }
}

/// Java's `s + "."` / `s + "$"`.
fn with_suffix(s: &JsString, c: u8) -> JsString {
    let mut units = s.as_units().to_vec();
    units.push(u16::from(c));
    JsString::from_units(units)
}

impl CompilerPass for StripCode {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::process(self, compiler, externs, root);
    }
}

/// A callback that strips debug code from a JavaScript parse tree.
// port: StripCode.Strip
struct Strip<'a> {
    pass: &'a mut StripCode,
    decisions_log: &'a mut dyn LogFile,
}

fn is_assign_op(token: Token) -> bool {
    matches!(
        token,
        Token::ASSIGN
            | Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
    )
}

impl Callback for Strip<'_> {
    // port: StripCode.Strip#shouldTraverse
    #[expect(clippy::collapsible_match, reason = "Java switch structure")]
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // Here we check for cases where we're going to remove a large chunk of code,
        // and so should not traverse into it to avoid both wasting time and causing
        // problems with logic duplication.
        match n.get_token(t) {
            Token::CALL | Token::NEW => {
                // If we're removing the whole call / new
                if self.is_method_or_ctor_call_that_triggers_removal(t, n, parent) {
                    self.decisions_log
                        .log(&mut || "removing function call".to_owned());
                    self.replace_highest_nested_call_with_null(t, n, parent.unwrap());
                    false
                } else {
                    true
                }
            }
            _ => true,
        }
    }

    // port: StripCode.Strip#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let token = n.get_token(t);
        match token {
            Token::VAR | Token::CONST | Token::LET => {
                self.remove_var_declarations_by_name_or_rvalue(t, n, parent.unwrap())
            }
            Token::NAME => self.maybe_remove_reference_to_removed_variable(t, n, parent.unwrap()),
            _ if is_assign_op(token) => {
                self.maybe_eliminate_assignment_by_lvalue_name(t, n, parent.unwrap())
            }
            Token::OBJECTLIT => self.eliminate_keys_with_strip_names_from_obj_lit(t, n),
            Token::EXPR_RESULT => self.maybe_eliminate_expression_by_name(t, n),
            Token::CLASS => self.maybe_eliminate_class_by_name_or_extends(t, n, parent.unwrap()),
            _ => {}
        }
    }
}

impl<'a> Strip<'a> {
    // port: StripCode.Strip#Strip
    fn new(pass: &'a mut StripCode, decisions_log: &'a mut dyn LogFile) -> Self {
        Self {
            pass,
            decisions_log,
        }
    }

    /// Removes declarations of any variables whose names are strip names or whose r-values are
    /// static method calls on strip types. Builds a set of removed variables so that all references
    /// to them can be removed.
    // port: StripCode.Strip#removeVarDeclarationsByNameOrRvalue
    fn remove_var_declarations_by_name_or_rvalue(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: NodeId,
    ) {
        let mut next;
        let mut name_node_opt = n.get_first_child(t);
        while let Some(name_node) = name_node_opt {
            next = name_node.get_next(t);
            name_node_opt = next;
            if name_node.is_destructuring_lhs(t) {
                continue;
            }
            check_state!(name_node.is_name(t), "%s", name_node.to_string(t));
            let name = name_node.get_string(t);
            // If this variable represents a collapsed property, it's the original property name we're
            // supposed to be matching against.
            let last_dollar_sign = name.last_index_of_char(u16::from(b'$'));
            let possible_strip_name = if last_dollar_sign != -1 {
                name.substring_from(last_dollar_sign as usize + 1)
            } else {
                name.clone()
            };
            if self.is_strip_name(&possible_strip_name)
                || self.qualified_name_begins_with_strip_type_node(t, name_node)
                || self
                    .is_call_whose_return_value_should_be_stripped(t, name_node.get_first_child(t))
            {
                // Remove the NAME.
                self.pass.vars_to_remove.insert(name.clone());
                if name.index_of_char(u16::from(b'$')) != -1 {
                    // We need to be careful with this code pattern that appears after
                    // collapsing properties.
                    // ```javascript
                    // /** @constructor */
                    // var a$b$C = function() {
                    //   this.nonStrippedName = a$b$C$strippedByNameOrValue;
                    // };
                    // var a$b$C$strippedByNameOrValue = strippedFunction();
                    // ```
                    // Note that the declaration of `a$b$C$nonStrippedByNameOrValue` will be visited
                    // **after** its first use, so it's too late to go back and remove the
                    // reference (without restructuring this class quite a bit). Instead,
                    // we'll preserve here the behavior you would get before collapsing and
                    // just replace the rhs with `null`.
                    self.decisions_log.log(&mut || {
                        format!("{name}: initialize with null ({possible_strip_name})")
                    });
                    if name_node.has_children(t) {
                        let only_child = name_node.get_only_child(t);
                        self.replace_with_null(t, only_child);
                    } else {
                        // `var my$name = null;` is a bit easier to optimize away than
                        // `var my$name;`, because we can clearly see that it is initialized, so we have
                        // a value to inline in later passes.
                        let null = IR::null_node(t).srcref(t, name_node);
                        name_node.add_child_to_front(t, null);
                    }
                    t.report_code_change();
                } else {
                    // Assume that the declaration comes before any reference.
                    // We will remove the references when we see them later.
                    self.decisions_log
                        .log(&mut || format!("{name}: removing declaration"));
                    name_node.detach(t);
                    NodeUtil::mark_functions_deleted(t.get_compiler(), name_node);
                }
            }
        }
        if !n.has_children(t) {
            // Must also remove the VAR.
            self.replace_with_empty(t, n, parent);
            t.report_code_change();
        }
    }

    /// Removes a reference if it is a reference to a removed variable.
    // port: StripCode.Strip#maybeRemoveReferenceToRemovedVariable
    fn maybe_remove_reference_to_removed_variable(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: NodeId,
    ) {
        let parent_token = parent.get_token(t);
        match parent_token {
            Token::VAR | Token::CONST | Token::LET => {
                // This is a variable declaration, not a reference.
            }
            Token::GETPROP | Token::GETELEM => {
                // GETPROP
                //   NAME
                //   STRING (property name)
                // GETELEM
                //   NAME
                //   NUMBER|STRING|NAME|...
                if parent.get_first_child(t) == Some(n) && self.is_reference_to_removed_var(t, n) {
                    let ast: &Ast = t;
                    self.decisions_log.log(&mut || {
                        format!("{}: removing getelem/getprop/call chain", n.get_string(ast))
                    });
                    let grandparent = parent.get_parent(t).unwrap();
                    self.replace_highest_nested_call_with_null(t, parent, grandparent);
                }
            }
            _ if is_assign_op(parent_token) => {
                if self.is_reference_to_removed_var(t, n) {
                    if parent.get_first_child(t) == Some(n) {
                        let grandparent = parent.get_parent(t).unwrap();
                        let ast: &Ast = t;
                        self.decisions_log.log(&mut || {
                            format!(
                                "{}: removing assignment to stripped var",
                                str_or_null(n.get_qualified_name(ast).as_ref())
                            )
                        });
                        if grandparent.is_expr_result(t) {
                            // Remove the assignment.
                            let great_grandparent = grandparent.get_parent(t).unwrap();
                            self.replace_with_empty(t, grandparent, great_grandparent);
                            t.report_code_change();
                        } else {
                            // Substitute the r-value for the assignment.
                            let rvalue = n.get_next(t).unwrap();
                            rvalue.detach(t);
                            parent.replace_with(t, rvalue);
                            t.report_code_change();
                        }
                    } else {
                        // The var reference is the r-value. Replace it with null.
                        let ast: &Ast = t;
                        self.decisions_log.log(&mut || {
                            format!(
                                "{}: replacing rhs reference with null",
                                str_or_null(n.get_qualified_name(ast).as_ref())
                            )
                        });
                        self.replace_with_null(t, n);
                        t.report_code_change();
                    }
                }
            }
            Token::NEW | Token::CALL => {
                if !n.is_first_child_of(t, Some(parent)) && self.is_reference_to_removed_var(t, n) {
                    // NOTE: the callee is handled when we visit the CALL or NEW node
                    let ast: &Ast = t;
                    self.decisions_log.log(&mut || {
                        format!(
                            "{}: replacing parameter reference with null",
                            str_or_null(n.get_qualified_name(ast).as_ref())
                        )
                    });
                    self.replace_with_null(t, n);
                    t.report_code_change();
                }
            }
            Token::COMMA => {
                let grandparent = parent.get_parent(t).unwrap();
                // The last child in a comma expression is its result, so we need to be careful replacing
                // it with null. We don't want to replace the entire comma expression with null because
                // there could be other elements in it with side-effects.
                let is_last_child = parent.get_last_child(t) == Some(n);
                // The only parent where replacing with null is an issue is likely where the comma
                // expression is the first child (callee) of a CALL or NEW node.
                let parent_is_callee = (grandparent.is_call(t) || grandparent.is_new(t))
                    && parent.is_first_child_of(t, Some(grandparent));
                let is_safe_to_remove = !is_last_child || !parent_is_callee;
                if is_safe_to_remove && self.is_reference_to_removed_var(t, n) {
                    let ast: &Ast = t;
                    self.decisions_log.log(&mut || {
                        format!(
                            "{}: replacing reference in comma expr with null",
                            str_or_null(n.get_qualified_name(ast).as_ref())
                        )
                    });
                    self.replace_with_null(t, n);
                    t.report_code_change();
                }
            }
            _ => {
                if self.is_reference_to_removed_var(t, n) {
                    let ast: &Ast = t;
                    self.decisions_log.log(&mut || {
                        format!(
                            "{}: replacing reference with null",
                            str_or_null(n.get_qualified_name(ast).as_ref())
                        )
                    });
                    self.replace_with_null(t, n);
                    t.report_code_change();
                }
            }
        }
    }

    /// Use a while loop to get up out of any nested calls. For example, if we have just detected
    /// that we need to remove the a.b() call in a.b().c().d(), we'll have to remove all of the
    /// calls, and it will take a few iterations through this loop to get up to d().
    // port: StripCode.Strip#replaceHighestNestedCallWithNull
    fn replace_highest_nested_call_with_null(
        &mut self,
        t: &mut NodeTraversal<'_>,
        node: NodeId,
        parent: NodeId,
    ) {
        let mut ancestor = parent;
        let mut ancestor_child = node;
        let mut ancestor_parent;
        loop {
            ancestor_parent = ancestor.get_parent(t);
            let Some(ancestor_parent) = ancestor_parent else {
                return; // the call was already removed from the AST
            };

            if ancestor.get_first_child(t) != Some(ancestor_child) {
                self.replace_with_null(t, ancestor_child);
                break;
            }
            if ancestor.is_expr_result(t) {
                // Remove the entire expression statement.
                self.replace_with_empty(t, ancestor, ancestor_parent);
                break;
            }
            if ancestor.is_assign(t) {
                let last = ancestor.get_last_child(t).unwrap().detach(t);
                ancestor.replace_with(t, last);
                break;
            }
            if !NodeUtil::is_normal_get(t, ancestor) && !ancestor.is_call(t) {
                self.replace_with_null(t, ancestor_child);
                break;
            }

            // Is not executed on the last iteration so can't be used for change reporting.
            ancestor_child = ancestor;
            ancestor = ancestor_parent;
        }
        t.report_code_change();
    }

    /// Eliminates an assignment if the l-value is:
    ///
    /// - A field name that's a strip name
    /// - A qualified name that begins with a strip type
    // port: StripCode.Strip#maybeEliminateAssignmentByLvalueName
    fn maybe_eliminate_assignment_by_lvalue_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: NodeId,
    ) {
        // ASSIGN
        //   l-value
        //   r-value
        let lvalue = n.get_first_child(t).unwrap();
        if self.name_includes_field_name_to_strip(t, Some(lvalue))
            || self.qualified_name_begins_with_strip_type_node(t, lvalue)
        {
            // Limit to EXPR_RESULT because it is not
            // safe to eliminate assignment in complex expressions,
            // e.g. in ((x = 7) + 8)
            if parent.is_expr_result(t) {
                let ast: &Ast = t;
                self.decisions_log.log(&mut || {
                    format!("{}: removing assignment statement", lvalue.get_string(ast))
                });
                let grandparent = parent.get_parent(t);
                // the assignment may already have been removed when visiting either the lhs
                // or the rhs.
                if let Some(grandparent) = grandparent {
                    self.replace_with_empty(t, parent, grandparent);
                    t.get_compiler()
                        .report_change_to_enclosing_scope(grandparent);
                }
            } else {
                let qname = str_or_null(lvalue.get_qualified_name(t).as_ref());
                t.report(n, &STRIP_ASSIGNMENT_ERROR, &[&qname]);
            }
        }
    }

    /// Eliminates an expression if it refers to:
    ///
    /// - A field name that's a strip name
    /// - A qualified name that begins with a strip type
    ///
    /// This gets rid of construct like: a.prototype.logger; (used instead of a.prototype.logger =
    /// null;) This expression is not an assignment and so will not be caught by
    /// maybeEliminateAssignmentByLvalueName.
    // port: StripCode.Strip#maybeEliminateExpressionByName
    fn maybe_eliminate_expression_by_name(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        // EXPR_RESULT
        //   expression
        check_argument!(n.is_expr_result(t), "%s", n.to_string(t));
        let Some(parent) = n.get_parent(t) else {
            // This EXPR_RESULT was already removed when one of its child nodes was visited.
            return;
        };
        let expression = n.get_first_child(t).unwrap();
        if self.name_includes_field_name_to_strip(t, Some(expression))
            || self.qualified_name_begins_with_strip_type_node(t, expression)
        {
            let ast: &Ast = t;
            self.decisions_log.log(&mut || {
                format!(
                    "{}: removing property declaration statement",
                    expression.get_string(ast)
                )
            });
            self.replace_with_empty(t, n, parent);
            t.get_compiler().report_change_to_enclosing_scope(parent);
        }
    }

    /// Eliminates any object literal keys in an object literal declaration that have strip names.
    // port: StripCode.Strip#eliminateKeysWithStripNamesFromObjLit
    fn eliminate_keys_with_strip_names_from_obj_lit(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
    ) {
        // OBJLIT
        //   key1
        //     value1
        //   key2
        //   ...
        let mut key = n.get_first_child(t);
        while let Some(k) = key {
            match k.get_token(t) {
                Token::GETTER_DEF
                | Token::SETTER_DEF
                | Token::STRING_KEY
                | Token::MEMBER_FUNCTION_DEF
                    if self.is_strip_name(&k.get_string(t)) =>
                {
                    let next = k.get_next(t);
                    k.detach(t);
                    NodeUtil::mark_functions_deleted(t.get_compiler(), k);
                    key = next;
                    t.get_compiler().report_change_to_enclosing_scope(n);
                }
                // fall through
                _ => {
                    key = k.get_next(t);
                }
            }
        }
    }

    /// Removes a class definition if the name is a strip type. Warns if a non-strippable class is
    /// extending a strippable type.
    // port: StripCode.Strip#maybeEliminateClassByNameOrExtends
    fn maybe_eliminate_class_by_name_or_extends(
        &mut self,
        t: &mut NodeTraversal<'_>,
        class_node: NodeId,
        parent: NodeId,
    ) {
        let name_node = NodeUtil::get_name_node(t, class_node);
        let class_name: JsString;
        // Replace class with null if it is a strip type
        if let Some(name_node) = name_node.filter(|nn| nn.is_qualified_name(t)) {
            class_name = name_node.get_qualified_name(t).unwrap();
            if self.qualified_name_begins_with_strip_type(Some(&class_name)) {
                self.decisions_log
                    .log(&mut || format!("{class_name}: removing class"));
                if NodeUtil::is_statement_parent(t, parent) {
                    self.replace_with_empty(t, class_node, parent);
                } else {
                    self.replace_with_null(t, class_node);
                }
                t.report_code_change();
                return;
            }
        } else {
            class_name = JsString::from("<anonymous>");
        }

        // If the class is not a strip type, the superclass also cannot be a strip type
        let superclass_node = class_node.get_second_child(t);
        if let Some(superclass_node) = superclass_node.filter(|s| s.is_qualified_name(t)) {
            let superclass_name = superclass_node.get_qualified_name(t).unwrap();
            if self.qualified_name_begins_with_strip_type(Some(&superclass_name)) {
                t.report(
                    class_node,
                    &STRIP_TYPE_INHERIT_ERROR,
                    &[&class_name.to_string(), &superclass_name.to_string()],
                );
            }
        }
    }

    /// Gets whether a node is a CALL node whose return value should be stripped. A call's return
    /// value should be stripped if the function getting called is a static method in a class that
    /// gets stripped. For example, if "goog.debug.Logger" is a strip name, then this function
    /// returns true for a call such as "goog.debug.Logger.getLogger(...)". It may also simply be a
    /// function that is getting stripped. For example, if "getLogger" is a strip name, but not
    /// "goog.debug.Logger", this will still return true.
    // port: StripCode.Strip#isCallWhoseReturnValueShouldBeStripped
    fn is_call_whose_return_value_should_be_stripped(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: Option<NodeId>,
    ) -> bool {
        let Some(n) = n else {
            return false;
        };
        if (!n.is_call(t) && !n.is_new(t)) || !n.has_children(t) {
            return false;
        }

        let function = NodeUtil::get_call_target_resolving_indirect_calls(t, n);
        self.qualified_name_begins_with_strip_type_node(t, function)
            || self.name_includes_field_name_to_strip(t, Some(function))
    }

    /// Gets whether a qualified name begins with a strip name. The names "goog.debug",
    /// "goog.debug.Logger", and "goog.debug.Logger.Level" are examples of strip names that would
    /// result in this function returning true for a node representing the name
    /// "goog.debug.Logger.Level".
    // port: StripCode.Strip#qualifiedNameBeginsWithStripType(Node)
    fn qualified_name_begins_with_strip_type_node(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
    ) -> bool {
        let name = n.get_qualified_name(t);
        self.qualified_name_begins_with_strip_type(name.as_ref())
    }

    /// Gets whether a qualified name begins with a strip name. The names "goog.debug",
    /// "goog.debug.Logger", and "goog.debug.Logger.Level" are examples of strip names that would
    /// result in this function returning true for a node representing the name
    /// "goog.debug.Logger.Level".
    // port: StripCode.Strip#qualifiedNameBeginsWithStripType(String)
    fn qualified_name_begins_with_strip_type(&mut self, name: Option<&JsString>) -> bool {
        if let Some(name) = name {
            for r#type in &self.pass.strip_types_list {
                if name == r#type {
                    Self::log_strip_name(self.decisions_log, name, "equals strip type");
                    return true;
                }
            }
            for r#type in &self.pass.strip_type_prefixes_list {
                if name.starts_with(r#type) {
                    Self::log_strip_name(self.decisions_log, name, "starts with strip type prefix");
                    return true;
                }
            }
        }
        self.log_not_a_strip_name(name, "does not begin with a strip type");
        false
    }

    /// Determines whether a NAME node represents a reference to a variable that has been removed.
    // port: StripCode.Strip#isReferenceToRemovedVar
    fn is_reference_to_removed_var(&self, t: &NodeTraversal<'_>, n: NodeId) -> bool {
        self.pass.vars_to_remove.contains(&n.get_string(t))
    }

    /// Gets whether a CALL node triggers statement removal, based on the name of the object whose
    /// method is being called, or the name of the method. Checks whether the name begins with a
    /// strip type, includes a field name that's a strip name, or belongs to the set of global
    /// class-defining functions (e.g. goog.inherits).
    // port: StripCode.Strip#isMethodOrCtorCallThatTriggersRemoval
    fn is_method_or_ctor_call_that_triggers_removal(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // CALL/NEW
        //   GETPROP (function)         <-- we're interested in this, the function
        //     GETPROP (callee object)  <-- or the object on which it is called
        //       ...
        //       STRING (field name)
        //     STRING (method name)
        //   ... (arguments)

        let function = NodeUtil::get_call_target_resolving_indirect_calls(t, n);
        if !function.is_qualified_name(t) {
            return false;
        }

        if let Some(parent) = parent
            && parent.is_name(t)
        {
            let grandparent = parent.get_parent(t);
            if grandparent.is_some() && NodeUtil::is_name_declaration(t, grandparent) {
                // The call's return value is being used to initialize a newly
                // declared variable. We should leave the call intact for now.
                // That way, when the traversal reaches the variable declaration,
                // we'll recognize that the variable and all references to it need
                // to be eliminated.
                return false;
            }
        }

        if function.is_name(t) && self.is_strip_name(&function.get_string(t)) {
            return true;
        }
        let callee = function.get_first_child(t);
        self.name_includes_field_name_to_strip(t, callee)
            || self.name_includes_field_name_to_strip(t, Some(function))
            || self.qualified_name_begins_with_strip_type_node(t, function)
            || self.acts_on_strip_type(t, n)
    }

    /// Returns whether a name includes a field name that should be stripped. E.g.,
    /// "foo.stripMe.bar", "(foo.bar).stripMe", etc.
    // port: StripCode.Strip#nameIncludesFieldNameToStrip
    fn name_includes_field_name_to_strip(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: Option<NodeId>,
    ) -> bool {
        let Some(n) = n else {
            return false;
        };
        if n.is_get_prop(t) {
            return self.is_strip_name(&n.get_string(t))
                || self.name_includes_field_name_to_strip(t, n.get_first_child(t));
        } else if n.is_name(t) {
            let name_string = n.get_string(t);
            let dollar = JsString::from("$");
            // CollapseProperties may have turned "a.b.c" into "a$b$c",
            // so split that up and match its parts.
            let mut end = name_string.index_of_char(u16::from(b'$'));
            if end == -1 {
                return false;
            }
            // iterate over all the $-deliminated parts of the name, where each part is from [start,end)
            let mut start = 0;
            loop {
                if self.is_strip_name(&name_string.substring(start as usize, end as usize)) {
                    return true;
                }
                if end as usize == name_string.length() {
                    break;
                }
                let new_start = end + 1;
                let new_end = name_string.index_of_from(&dollar, new_start);
                start = new_start;
                end = if new_end == -1 {
                    name_string.length() as i32
                } else {
                    new_end
                };
            }
        }
        false
    }

    /// Determines whether the given node helps to define a strip type. For example,
    /// goog.inherits(stripType, Object) would be such a call.
    ///
    /// Also reports an error if a non-strip type inherits from a strip type.
    // port: StripCode.Strip#actsOnStripType
    fn acts_on_strip_type(&mut self, t: &mut NodeTraversal<'_>, call_node: NodeId) -> bool {
        let classes = {
            let compiler: &AbstractCompiler = t.get_compiler();
            compiler
                .get_coding_convention()
                .get_classes_defined_by_call(compiler, call_node)
        };
        if let Some(classes) = classes {
            // It's okay to strip a type that inherits from a non-stripped type
            // e.g. goog.inherits(goog.debug.Logger, Object)
            if self.qualified_name_begins_with_strip_type(Some(&classes.subclass_name)) {
                Self::log_strip_name(
                    self.decisions_log,
                    &classes.subclass_name,
                    "class defining call",
                );
                return true;
            }

            // report an error if a non-strip type inherits from a
            // strip type.
            if self.qualified_name_begins_with_strip_type(Some(&classes.superclass_name)) {
                t.report(
                    call_node,
                    &STRIP_TYPE_INHERIT_ERROR,
                    &[
                        &classes.subclass_name.to_string(),
                        &classes.superclass_name.to_string(),
                    ],
                );
            }
        }

        false
    }

    /// Gets whether a JavaScript identifier is the name of a variable or property that should be
    /// stripped.
    // port: StripCode.Strip#isStripName
    fn is_strip_name(&mut self, name: &JsString) -> bool {
        if self.pass.strip_name_suffixes.contains(name) {
            Self::log_strip_name(self.decisions_log, name, "matches a suffix");
            return true;
        }

        if self.pass.strip_name_prefixes.contains(name) {
            Self::log_strip_name(self.decisions_log, name, "matches a prefix");
            return true;
        }

        if name.is_empty() || java_lang::is_upper_case(name.char_at(0)) {
            self.log_not_a_strip_name(Some(name), "empty or starts with uppercase");
            return false;
        }

        let lc_name = to_lower_case_root(name);
        for strip_name in &self.pass.strip_name_prefixes_lower_case_list {
            if lc_name.starts_with(strip_name) {
                Self::log_strip_name_supplier(self.decisions_log, name, &mut || {
                    format!("matches lc prefix: {strip_name}")
                });
                return true;
            }
        }

        for strip_name in &self.pass.strip_name_suffixes_lower_case_list {
            if lc_name.ends_with(strip_name) {
                Self::log_strip_name_supplier(self.decisions_log, name, &mut || {
                    format!("matches lc suffix: {strip_name}")
                });
                return true;
            }
        }

        self.log_not_a_strip_name(Some(name), "no matches");
        false
    }

    // port: StripCode.Strip#logNotAStripName
    fn log_not_a_strip_name(&mut self, name: Option<&JsString>, reason: &str) {
        if self.decisions_log.is_logging() {
            self.decisions_log
                .log(&mut || format!("{}\tnot a strip name: {reason}", str_or_null(name)));
        }
    }

    // port: StripCode.Strip#logStripName(String,String)
    fn log_strip_name(decisions_log: &mut dyn LogFile, name: &JsString, reason: &str) {
        if decisions_log.is_logging() {
            decisions_log.log(&mut || format!("{name}\tstrip name: {reason}"));
        }
    }

    // port: StripCode.Strip#logStripName(String,Supplier)
    fn log_strip_name_supplier(
        decisions_log: &mut dyn LogFile,
        name: &JsString,
        reason_supplier: &mut dyn FnMut() -> String,
    ) {
        if decisions_log.is_logging() {
            decisions_log.log(&mut || format!("{name}\tstrip name: {}", reason_supplier()));
        }
    }

    /// Replaces a node with a NULL node. This is useful where a value is expected.
    // port: StripCode.Strip#replaceWithNull
    fn replace_with_null(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let ast: &Ast = t;
        self.decisions_log
            .log(&mut || format!("replace with null: {}", n.get_location(ast)));
        let null = IR::null_node(t);
        n.replace_with(t, null);
        NodeUtil::mark_functions_deleted(t.get_compiler(), n);
    }

    /// Replaces a node with an EMPTY node. This is useful where a statement is expected.
    // port: StripCode.Strip#replaceWithEmpty
    fn replace_with_empty(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: NodeId) {
        let ast: &Ast = t;
        self.decisions_log
            .log(&mut || format!("replace with empty: {}", n.get_location(ast)));
        NodeUtil::remove_child(t, parent, n);
        NodeUtil::mark_functions_deleted(t.get_compiler(), n);
    }
}
