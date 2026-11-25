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

use crate::{bundled_script, declarations};

/// Record JavaScript and JSDoc fields independently, in the same postorder as
/// GatherExternProperties. A class or destructuring traversal can visit children in a
/// specialized order. Leave those scripts to the ordinary walk.
pub(super) fn generate(
    ast: &closure_rhino::node::Ast,
    script: closure_rhino::node::NodeId,
    declaration_file: &str,
    declaration_records: &[u8],
) -> Option<String> {
    use closure_rhino::{
        fast_hash::IndexSet,
        jsdoc_info::Bit,
        node::{Ast, NodeId},
        token::Token,
    };
    #[derive(Default)]
    struct Records {
        properties: IndexSet<String>,
        fields: IndexSet<String>,
        protector: bool,
        goog: bool,
        global_property_roots: IndexSet<String>,
        other_name_reference: bool,
        declared_var_names: IndexSet<String>,
        declared_var_names_invalid: bool,
        extern_referenced_names: IndexSet<String>,
        extern_referenced_names_invalid: bool,
        normalize_clean: bool,
        name_hash_mask: u64,
    }
    impl Records {
        fn visit(
            &mut self,
            ast: &Ast,
            n: NodeId,
            in_global_expression: bool,
            in_function_scope: bool,
        ) -> Option<()> {
            let token = n.get_token(ast);
            if matches!(
                token,
                Token::CLASS
                    | Token::CLASS_MEMBERS
                    | Token::MODULE_BODY
                    | Token::DEFAULT_VALUE
                    | Token::DESTRUCTURING_LHS
            ) {
                return None;
            }
            if matches!(
                token,
                Token::WHILE
                    | Token::FOR
                    | Token::FOR_IN
                    | Token::FOR_OF
                    | Token::FOR_AWAIT_OF
                    | Token::LABEL
                    | Token::EXPORT
                    | Token::ASSIGN_OR
                    | Token::ASSIGN_AND
                    | Token::ASSIGN_COALESCE
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
                    | Token::ASSIGN_EXPONENT
            ) {
                self.normalize_clean = false;
            }
            if matches!(token, Token::VAR | Token::LET | Token::CONST)
                && (!n.has_children(ast) || n.get_first_child(ast) != n.get_last_child(ast))
            {
                self.normalize_clean = false;
            }
            if matches!(
                token,
                Token::ARRAYLIT
                    | Token::CALL
                    | Token::PARAM_LIST
                    | Token::NEW
                    | Token::OBJECTLIT
                    | Token::OPTCHAIN_CALL
            ) && n.has_trailing_comma(ast)
            {
                self.normalize_clean = false;
            }
            if token == Token::IMPORT_STAR {
                self.extern_referenced_names_invalid = true;
            }
            let child_in_function_scope = if token == Token::FUNCTION {
                let plain = declarations::is_plain_function_stub(ast, n);
                if plain {
                    let fn_name = n.get_first_child(ast).unwrap();
                    let params = fn_name.get_next(ast).unwrap();
                    let is_decl = n.get_parent(ast).is_some_and(|p| p.is_script(ast));
                    if !is_decl && !fn_name.get_string_ref(ast).is_empty() {
                        self.normalize_clean = false;
                    }
                    let mut seen_params: IndexSet<&[u16]> = IndexSet::default();
                    for param in params.children(ast) {
                        let units = param.get_string_ref(ast).as_units();
                        if units.is_empty()
                            || param.get_jsdoc_info_ref(ast).is_some()
                            || param.get_string_ref(ast) == "arguments"
                            || !seen_params.insert(units)
                            || (is_decl && param.get_string_ref(ast) == fn_name.get_string_ref(ast))
                        {
                            self.normalize_clean = false;
                        }
                    }
                } else {
                    self.normalize_clean = false;
                }
                // The name of a function expression and the plain parameters of any function
                // bind in the function's own scope, whatever its body holds.
                in_global_expression
            } else {
                in_function_scope
            };
            for child in n.children(ast) {
                if child.is_script(ast) {
                    return None;
                }
                // Only a property expression reached entirely through these nodes is
                // guaranteed to share the SCRIPT's scope. Creating a function or block
                // scope can itself invoke the redeclaration handler. GETELEM covers
                // `C.prototype[Symbol.iterator] = function() {};`.
                let global = in_global_expression
                    && matches!(
                        token,
                        Token::SCRIPT
                            | Token::EXPR_RESULT
                            | Token::GETPROP
                            | Token::GETELEM
                            | Token::ASSIGN
                            | Token::NAME
                            | Token::VAR
                            | Token::CONST
                            | Token::LET
                    );
                self.visit(ast, child, global, child_in_function_scope)?;
            }
            if let Some(name) = bundled_script::property(ast, n, n.get_parent(ast)) {
                self.properties.insert(name.to_string_lossy());
            }
            if n.is_name(ast) {
                let s = n.get_string_ref(ast);
                if !s.is_empty() {
                    self.name_hash_mask |= 1u64 << (s.hash_code() & 63);
                }
                self.protector |= s == "JSCOMPILER_PRESERVE";
                self.goog |= s == "goog";
                // These are the contexts in which NameRefInExternsCheck either ignores a
                // declaration or asks whether a property root is in scope. Any other use
                // keeps that visitor on its full diagnostic path.
                let parent = n.get_parent(ast)?;
                let parent_tok = parent.get_token(ast);
                // Non-param variable declarations (matching NodeUtil::collect_extern_variable_names).
                let is_non_param_var_decl =
                    matches!(parent_tok, Token::VAR | Token::LET | Token::CONST)
                        || (parent_tok == Token::FUNCTION
                            && parent.get_first_child(ast) == Some(n)
                            && !s.is_empty())
                        || (parent_tok == Token::CATCH && parent.get_first_child(ast) == Some(n));
                if is_non_param_var_decl {
                    if let Ok(name) = String::from_utf16(s.as_units()) {
                        self.declared_var_names.insert(name);
                    } else {
                        self.declared_var_names_invalid = true;
                    }
                }
                // Names that resolve in global scope for ReferenceCollector::skip_externs_when_unread.
                if !s.is_empty() {
                    let is_plain_param = in_function_scope
                        && parent_tok == Token::PARAM_LIST
                        && !n.has_children(ast);
                    let is_function_name = in_function_scope
                        && parent_tok == Token::FUNCTION
                        && parent.get_first_child(ast) == Some(n);
                    let is_global_fn_decl_name = is_function_name
                        && parent.get_parent(ast).is_some_and(|gp| gp.is_script(ast));
                    if is_plain_param || (is_function_name && !is_global_fn_decl_name) {
                        // Resolves to this extern function's own param Var, or for a named
                        // function expression to the name declared in its own function scope
                        // (always is_extern).
                    } else if in_global_expression || is_global_fn_decl_name {
                        if let Ok(name) = String::from_utf16(s.as_units()) {
                            self.extern_referenced_names.insert(name);
                        } else {
                            self.extern_referenced_names_invalid = true;
                        }
                    } else {
                        self.extern_referenced_names_invalid = true;
                    }
                }
                match parent_tok {
                    Token::VAR
                    | Token::LET
                    | Token::CONST
                    | Token::FUNCTION
                    | Token::CLASS
                    | Token::PARAM_LIST
                    | Token::DEFAULT_VALUE
                    | Token::ITER_REST
                    | Token::OBJECT_REST
                    | Token::ARRAY_PATTERN => {}
                    Token::GETPROP
                        if in_global_expression && parent.get_first_child(ast) == Some(n) =>
                    {
                        if let Ok(name) = String::from_utf16(s.as_units()) {
                            self.global_property_roots.insert(name);
                        } else {
                            self.other_name_reference = true;
                        }
                    }
                    Token::ASSIGN
                        if parent.get_last_child(ast) == Some(n)
                            && n.is_qualified_name(ast)
                            && parent.get_first_child(ast)?.is_qualified_name(ast) => {}
                    Token::NAME
                        if parent.get_parent(ast).is_some_and(|grand| {
                            matches!(grand.get_token(ast), Token::VAR | Token::LET | Token::CONST)
                        }) => {}
                    Token::STRING_KEY
                        if parent
                            .get_parent(ast)
                            .is_some_and(|grand| grand.is_object_pattern(ast)) => {}
                    _ => self.other_name_reference = true,
                }
            }
            // A script is visited after all of its appended statements at run time.
            if n.is_script(ast) {
                return Some(());
            }
            if let Some(doc) = n.get_jsdoc_info_ref(ast) {
                doc.for_each_type_node(|root| {
                    // Only the preparse arena's implicit template bound is outside the
                    // file. The real bound is checked by the target before using the fields.
                    if root.arena_index() < ast.node_count() as usize {
                        bundled_script::gather_type_fields(&mut self.fields, ast, root);
                    }
                });
            }
            Some(())
        }
    }
    let mut records = Records {
        normalize_clean: script
            .children(ast)
            .zip(declaration_records)
            .all(|(statement, &r)| {
                r != declarations::OTHER || declarations::clean_other(ast, statement).is_some()
            }),
        ..Records::default()
    };
    records.visit(ast, script, true, false)?;
    let properties: Vec<_> = records.properties.into_iter().collect();
    let fields: Vec<_> = records.fields.into_iter().collect();
    let may_have_calls = ast.has_seen_token(Token::CALL);
    let may_have_module_syntax = may_have_calls
        || records.goog
        || [Token::IMPORT, Token::EXPORT, Token::DYNAMIC_IMPORT]
            .iter()
            .any(|&token| ast.has_seen_token(token));
    let global_property_roots = if records.other_name_reference {
        "None".to_owned()
    } else {
        format!(
            "Some(&{:?})",
            records
                .global_property_roots
                .into_iter()
                .collect::<Vec<_>>()
        )
    };
    let declared_var_names = if records.declared_var_names_invalid {
        "None".to_owned()
    } else {
        format!(
            "Some(&{:?})",
            records.declared_var_names.into_iter().collect::<Vec<_>>()
        )
    };
    let extern_referenced_names = if records.extern_referenced_names_invalid {
        "None".to_owned()
    } else {
        format!(
            "Some(&{:?})",
            records
                .extern_referenced_names
                .into_iter()
                .collect::<Vec<_>>()
        )
    };
    let check_functions = records.protector || ast.has_seen_jsdoc_bit(Bit::NOSIDEEFFECTS);
    let normalize_clean = records.normalize_clean;
    let name_hash_mask = records.name_hash_mask;
    Some(format!(
        "BundledScript {{ declarations: include_bytes!({declaration_file:?}), properties: &{properties:?}, type_fields: &{fields:?}, check_function_annotations: {check_functions}, may_have_calls: {may_have_calls}, may_have_module_syntax: {may_have_module_syntax}, global_property_roots: {global_property_roots}, declared_var_names: {declared_var_names}, extern_referenced_names: {extern_referenced_names}, normalize_clean: {normalize_clean}, name_hash_mask: {name_hash_mask:#x}u64 }}"
    ))
}
