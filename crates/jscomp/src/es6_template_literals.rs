/*
 * Copyright 2014 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/Es6TemplateLiterals.java.

//! Port of `Es6TemplateLiterals.java`.
use crate::{
    AbstractCompiler, ast_factory::AstFactory, js::runtime_js_lib_manager::JsLibField,
    node_traversal::NodeTraversal, node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
};
use closure_rhino::{
    check_not_null, check_state,
    jscomp_colors::standard_colors,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId, Prop},
};
use std::sync::Arc;

const TEMPLATELIT_VAR: &str = "$jscomp$templatelit$";

/// Helper class for transpiling ES6 template literals.
///
/// Java keeps the compiler; here it is reached through the traversal (DESIGN §6).
pub struct Es6TemplateLiterals {
    ast_factory: AstFactory,
    namespace: TranspilationNamespace,
    create_template_tag_first_arg: Arc<dyn JsLibField>,
    create_template_tag_first_arg_with_raw: Arc<dyn JsLibField>,
}

impl Es6TemplateLiterals {
    // port: Es6TemplateLiterals#Es6TemplateLiterals
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let namespace = TranspilationNamespace::get(compiler);

        let runtime_js_lib_manager = compiler.get_runtime_js_lib_manager();
        let mut runtime_js_lib_manager = runtime_js_lib_manager.lock().unwrap();
        let create_template_tag_first_arg =
            runtime_js_lib_manager.get_js_lib_field("$jscomp.createTemplateTagFirstArg");
        let create_template_tag_first_arg_with_raw =
            runtime_js_lib_manager.get_js_lib_field("$jscomp.createTemplateTagFirstArgWithRaw");
        drop(runtime_js_lib_manager);
        Self {
            ast_factory,
            namespace,
            create_template_tag_first_arg,
            create_template_tag_first_arg_with_raw,
        }
    }

    /// Converts `${a} b ${c} d ${e}` to (a + " b " + c + " d " + e)
    ///
    /// `n`: a TEMPLATELIT node that is not prefixed with a tag
    // port: Es6TemplateLiterals#visitTemplateLiteral
    pub fn visit_template_literal(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        let length = n.get_child_count(t);
        if length == 0 {
            let s = self.ast_factory.create_string(t.get_compiler(), "\"\"");
            n.replace_with(t, s);
        } else {
            let first = check_not_null!(n.remove_first_child(t));
            check_state!(first.is_template_lit_string(t) && first.get_cooked_string(t).is_some());
            let first_cooked = first.get_cooked_string(t).unwrap();
            let first_str = self
                .ast_factory
                .create_string(t.get_compiler(), first_cooked.clone());
            if length == 1 {
                n.replace_with(t, first_str);
            } else {
                // Add the first string with the first substitution expression
                let sub = check_not_null!(n.remove_first_child(t));
                let sub_expr = check_not_null!(sub.remove_first_child(t));
                let mut add = self
                    .ast_factory
                    .create_add(t.get_compiler(), first_str, sub_expr);
                // Process the rest of the template literal
                for i in 2..length {
                    let child = check_not_null!(n.remove_first_child(t));
                    if child.is_template_lit_string(t) {
                        let cooked = child.get_cooked_string(t);
                        check_state!(cooked.is_some());
                        if cooked.unwrap().is_empty() {
                            continue;
                        } else if i == 2 && first_cooked.is_empty() {
                            // So that `${hello} world` gets translated into (hello + " world")
                            // instead of ("" + hello + " world").
                            add = add.get_second_child(t).unwrap().detach(t);
                        }
                    }
                    let right = if child.is_template_lit_string(t) {
                        let cooked = child.get_cooked_string(t).unwrap();
                        self.ast_factory.create_string(t.get_compiler(), cooked)
                    } else {
                        check_not_null!(child.remove_first_child(t))
                    };
                    add = self.ast_factory.create_add(t.get_compiler(), add, right);
                }
                let replacement = add.srcref_tree_if_missing(t, n);
                n.replace_with(t, replacement);
            }
        }
        t.report_code_change();
    }

    /// Converts a tagged template into a call to the tag.
    ///
    /// If the cooked and raw strings of the template literal are same, this will create a call to
    /// createtemplatetagfirstarg that simply calls slice() on the cooked array at runtime to make
    /// the raw array a copy of the cooked array, and returns the cooked array. Otherwise this will
    /// construct the raw strings array, and call `createtemplatetagfirstargwithraw` with it.
    ///
    /// `n`: a TAGGED_TEMPLATELIT node
    // port: Es6TemplateLiterals#visitTaggedTemplateLiteral
    pub fn visit_tagged_template_literal(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        insert_before: Option<NodeId>,
    ) {
        let template_lit = n.get_last_child(t).unwrap();
        let site_object = self.create_cooked_string_array(t.get_compiler(), template_lit);

        // Node holding the function call to the runtime injected function
        let call_template_tag_arg_creator = if Self::cooked_and_raw_strings_same(t, template_lit) {
            // The cooked and raw versions of the array are the same, so just call slice() on the
            // cooked array at runtime to make the raw array a copy of the cooked array.
            let callee = self.ast_factory.create_qname_for_field(
                t.get_compiler(),
                &self.namespace,
                self.create_template_tag_first_arg.as_ref(),
            );
            let site_clone = site_object.clone_tree(t);
            self.ast_factory.create_call(
                t.get_compiler(),
                callee,
                AstFactory::type_node(site_object),
                &[site_clone],
            )
        } else {
            // The raw string array is different, so we need to construct it.
            let raw = self.create_raw_string_array(t.get_compiler(), template_lit);
            let callee = self.ast_factory.create_qname_for_field(
                t.get_compiler(),
                &self.namespace,
                self.create_template_tag_first_arg_with_raw.as_ref(),
            );
            let site_clone = site_object.clone_tree(t);
            self.ast_factory.create_call(
                t.get_compiler(),
                callee,
                AstFactory::type_node(site_object),
                &[site_clone, raw],
            )
        };
        let mut js_doc_info_builder = JSDocInfo::builder();
        js_doc_info_builder.record_no_inline();
        let info = js_doc_info_builder.build();

        let input = check_not_null!(t.get_input().cloned());
        let unique_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        // var tagFnFirstArg = $jscomp.createTemplateTagFirstArg...
        let tag_fn_first_arg_declaration = self
            .ast_factory
            .create_single_var_name_declaration_with_value(
                t.get_compiler(),
                format!("{TEMPLATELIT_VAR}{unique_id}"),
                call_template_tag_arg_creator,
            );
        tag_fn_first_arg_declaration
            .set_jsdoc_info(t, info)
            .srcref_tree_if_missing(t, n);

        // For the first script, insertion point is right after the runtime injected function
        // definition as injecting to the top of the script causes runtime errors
        // https://github.com/google/closure-compiler/issues/3589. For any subsequent script(s), the
        // call is injected to the top of that script.
        tag_fn_first_arg_declaration.insert_before(t, check_not_null!(insert_before));
        t.report_code_change_at_node(tag_fn_first_arg_declaration);

        // Generate the call expression.
        let tag_fn_first_arg = tag_fn_first_arg_declaration
            .get_first_child(t)
            .unwrap()
            .clone_node(t);
        let tag = check_not_null!(n.remove_first_child(t));
        let call = self.ast_factory.create_call(
            t.get_compiler(),
            tag,
            AstFactory::type_node(n),
            &[tag_fn_first_arg],
        );
        let mut child = template_lit.get_first_child(t);
        while let Some(c) = child {
            if !c.is_template_lit_string(t) {
                let expr = check_not_null!(c.remove_first_child(t));
                call.add_child_to_back(t, expr);
            }
            child = c.get_next(t);
        }
        call.srcref_tree_if_missing(t, template_lit);
        let free_call = !NodeUtil::is_normal_or_opt_chain_get(t, call.get_first_child(t).unwrap());
        call.put_boolean_prop(t, Prop::FREE_CALL, free_call);
        n.replace_with(t, call);
        t.report_code_change();
    }

    // port: Es6TemplateLiterals#createRawStringArray
    fn create_raw_string_array(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let array = self.ast_factory.create_arraylit(compiler, &[]);
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            if c.is_template_lit_string(compiler) {
                let raw = c.get_raw_string(compiler);
                let s = self.ast_factory.create_string(compiler, raw);
                array.add_child_to_back(compiler, s);
            }
            child = c.get_next(compiler);
        }
        array
    }

    // port: Es6TemplateLiterals#createCookedStringArray
    fn create_cooked_string_array(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let template_array_type = if self.ast_factory.is_adding_colors() {
            Some(
                compiler
                    .get_color_registry()
                    .get(standard_colors::I_TEMPLATE_ARRAY_ID),
            )
        } else {
            None
        };
        let array = self.ast_factory.create_arraylit(compiler, &[]);
        // tighten the type from Array to ITemplateArray
        array.set_color(compiler, template_array_type);
        let mut child = n.get_first_child(compiler);
        while let Some(c) = child {
            if c.is_template_lit_string(compiler) {
                if let Some(cooked) = c.get_cooked_string(compiler) {
                    let s = self.ast_factory.create_string(compiler, cooked);
                    array.add_child_to_back(compiler, s);
                } else {
                    // undefined cooked string due to exception in template escapes
                    let zero = self.ast_factory.create_number(compiler, 0.0);
                    let void = self.ast_factory.create_void(compiler, zero);
                    array.add_child_to_back(compiler, void);
                }
            }
            child = c.get_next(compiler);
        }
        array
    }

    // port: Es6TemplateLiterals#cookedAndRawStringsSame
    fn cooked_and_raw_strings_same(ast: &Ast, n: NodeId) -> bool {
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            child = c.get_next(ast);
            if !c.is_template_lit_string(ast) {
                continue;
            }
            // getCookedString() returns null when the template literal has an illegal escape
            // sequence.
            match c.get_cooked_string(ast) {
                Some(cooked) if cooked == c.get_raw_string(ast) => {}
                _ => return false,
            }
        }
        true
    }
}
