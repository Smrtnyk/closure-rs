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

use crate::bundled_script;

/// Record JavaScript and JSDoc fields independently, in the same postorder as
/// GatherExternProperties. A class or destructuring traversal can visit children in a
/// specialized order. Leave those scripts to the ordinary walk.
pub(super) fn generate(
    ast: &closure_rhino::node::Ast,
    script: closure_rhino::node::NodeId,
    declaration_file: &str,
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
    }
    impl Records {
        fn visit(&mut self, ast: &Ast, n: NodeId, in_global_expression: bool) -> Option<()> {
            if matches!(
                n.get_token(ast),
                Token::CLASS
                    | Token::CLASS_MEMBERS
                    | Token::MODULE_BODY
                    | Token::DEFAULT_VALUE
                    | Token::DESTRUCTURING_LHS
            ) {
                return None;
            }
            for child in n.children(ast) {
                if child.is_script(ast) {
                    return None;
                }
                // Only a property expression reached entirely through these nodes is
                // guaranteed to share the SCRIPT's scope. Creating a function or block
                // scope can itself invoke the redeclaration handler.
                let global = in_global_expression
                    && matches!(
                        n.get_token(ast),
                        Token::SCRIPT
                            | Token::EXPR_RESULT
                            | Token::GETPROP
                            | Token::ASSIGN
                            | Token::NAME
                            | Token::VAR
                            | Token::CONST
                            | Token::LET
                    );
                self.visit(ast, child, global)?;
            }
            if let Some(name) = bundled_script::property(ast, n, n.get_parent(ast)) {
                self.properties.insert(name.to_string_lossy());
            }
            if n.is_name(ast) {
                self.protector |= n.get_string_ref(ast) == "JSCOMPILER_PRESERVE";
                self.goog |= n.get_string_ref(ast) == "goog";
                // These are the contexts in which NameRefInExternsCheck either ignores a
                // declaration or asks whether a property root is in scope. Any other use
                // keeps that visitor on its full diagnostic path.
                let parent = n.get_parent(ast)?;
                match parent.get_token(ast) {
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
                        if let Ok(name) = String::from_utf16(n.get_string_ref(ast).as_units()) {
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
    let mut records = Records::default();
    records.visit(ast, script, true)?;
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
    let check_functions = records.protector || ast.has_seen_jsdoc_bit(Bit::NOSIDEEFFECTS);
    Some(format!(
        "BundledScript {{ declarations: include_bytes!({declaration_file:?}), properties: &{properties:?}, type_fields: &{fields:?}, check_function_annotations: {check_functions}, may_have_calls: {may_have_calls}, may_have_module_syntax: {may_have_module_syntax}, global_property_roots: {global_property_roots} }}"
    ))
}
