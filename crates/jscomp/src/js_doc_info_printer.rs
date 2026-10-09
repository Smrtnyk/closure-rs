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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/JSDocInfoPrinter.java.

#![allow(clippy::collapsible_if)] // Retain Java nested conditionals.
use closure_rhino::{
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::{JSDocInfo, Visibility},
    node::{Ast, NodeId},
    token::Token,
};

/// Prints a JSDocInfo, used for preserving type annotations in ES6 transpilation.
pub struct JSDocInfoPrinter {
    use_original_name: bool,
    print_desc: bool,
}
impl JSDocInfoPrinter {
    // port: JSDocInfoPrinter#JSDocInfoPrinter(boolean)
    pub fn new(use_original_name: bool) -> Self {
        Self::new_with_print_desc(use_original_name, false)
    }
    /// @param useOriginalName Whether to use the original name field when printing types.
    /// @param printDesc Whether to print block, param, and return descriptions.
    // port: JSDocInfoPrinter#JSDocInfoPrinter(boolean,boolean)
    pub fn new_with_print_desc(use_original_name: bool, print_desc: bool) -> Self {
        Self {
            use_original_name,
            print_desc,
        }
    }
    // port: JSDocInfoPrinter#print
    pub fn print(&self, ast: &Ast, info: &JSDocInfo) -> JsString {
        let mut multiline = false;
        let mut parts: Vec<Vec<u16>> = Vec::new();
        parts.push(units("/**"));
        if info.is_externs() {
            parts.push(units("@externs"));
        }
        if info.is_type_summary() {
            parts.push(units("@typeSummary"));
        }

        if info.is_export() {
            parts.push(units("@export"));
        } else if info.get_visibility() != Visibility::INHERITED {
            parts.push(units(&format!(
                "@{}",
                format!("{:?}", info.get_visibility()).to_ascii_lowercase()
            )));
        }
        if let Some(authors) = info.get_authors() {
            multiline = true;
            for name in authors {
                parts.push(prefixed("@author ", &name));
            }
        }
        if info.is_abstract() {
            parts.push(units("@abstract"));
        }

        if info.has_lends_name() {
            parts.push(
                self.build_annotation_with_type_node(
                    ast,
                    "lends",
                    info.get_lends_name().unwrap().get_root(),
                )
                .as_units()
                .to_vec(),
            );
        }
        if info.has_const_annotation() && !info.is_define() {
            parts.push(units("@const"));
        }
        if info.is_final() {
            parts.push(units("@final"));
        }

        if let Some(description) = info.get_description() {
            multiline = true;
            parts.push(prefixed("@desc ", &description));
        }
        if let Some(references) = info.get_references() {
            multiline = true;
            for desc in references {
                parts.push(prefixed("@see ", &desc));
            }
        }
        if info.is_wizaction() {
            parts.push(units("@wizaction"));
        }
        if info.is_no_side_effects() {
            parts.push(units("@nosideeffects"));
        }
        if info.is_no_compile() {
            parts.push(units("@nocompile"));
        }
        if info.is_no_inline() {
            parts.push(units("@noinline"));
        }
        if info.is_require_inlining() {
            parts.push(units("@requireInlining"));
        }
        if info.is_encourage_inlining() {
            parts.push(units("@encourageInlining"));
        }
        if info.is_provide_already_provided() {
            parts.push(units("@provideAlreadyProvided"));
        }
        if info.is_id_generator() {
            parts.push(units("@idGenerator {unique}"));
        }
        if info.is_consistent_id_generator() {
            parts.push(units("@idGenerator {consistent}"));
        }
        if info.is_stable_id_generator() {
            parts.push(units("@idGenerator {stable}"));
        }
        if info.is_xid_generator() {
            parts.push(units("@idGenerator {xid}"));
        }
        if info.is_mapped_id_generator() {
            parts.push(units("@idGenerator {mapped}"));
        }
        if info.makes_dicts() {
            parts.push(units("@dict"));
        }
        if info.makes_structs() {
            parts.push(units("@struct"));
        }
        if info.makes_unrestricted() {
            parts.push(units("@unrestricted "));
        }
        if info.is_constructor() {
            parts.push(units("@constructor"));
        }

        if info.is_interface() && !info.uses_implicit_match() {
            parts.push(units("@interface"));
        }
        if info.is_interface() && info.uses_implicit_match() {
            parts.push(units("@record"));
        }
        if info.is_closure_unaware_code() {
            parts.push(units("@closureUnaware"));
        }

        if info.has_base_type() {
            multiline = true;
            let type_node = self.strip_bang(ast, info.get_base_type().unwrap().get_root());
            parts.push(
                self.build_annotation_with_type_node(ast, "extends", type_node)
                    .as_units()
                    .to_vec(),
            );
        }
        for ty in info.get_extended_interfaces() {
            multiline = true;
            let type_node = self.strip_bang(ast, ty.get_root());
            parts.push(
                self.build_annotation_with_type_node(ast, "extends", type_node)
                    .as_units()
                    .to_vec(),
            );
        }
        for ty in info.get_implemented_interfaces() {
            multiline = true;
            let type_node = self.strip_bang(ast, ty.get_root());
            parts.push(
                self.build_annotation_with_type_node(ast, "implements", type_node)
                    .as_units()
                    .to_vec(),
            );
        }
        if info.has_this_type() {
            multiline = true;
            let type_node = self.strip_bang(ast, info.get_this_type().unwrap().get_root());
            parts.push(
                self.build_annotation_with_type_node(ast, "this", type_node)
                    .as_units()
                    .to_vec(),
            );
        }
        if info.get_parameter_count() > 0 {
            multiline = true;
            for name in info.get_parameter_names() {
                parts.push(prefixed(
                    "@param ",
                    &self.build_param_type(ast, info, &name),
                ));
            }
        }
        if info.has_return_type() {
            multiline = true;
            parts.push(
                self.build_annotation_with_type_and_description(
                    ast,
                    "return",
                    &info.get_return_type().unwrap(),
                    info.get_return_description().as_ref(),
                )
                .as_units()
                .to_vec(),
            );
        }
        let throws = info.get_throws_annotations();
        if !throws.is_empty() && !throws[0].is_empty() {
            multiline = true;
            parts.push(prefixed("@throws ", &throws[0]));
        }
        let templates = info.get_template_types();
        if !templates.is_empty() {
            multiline = true;
            for (name, bound_expr) in templates {
                let bound_expr = bound_expr.unwrap();
                let bound_root = bound_expr.get_root();
                if bound_root.get_token(ast) == Token::QMARK && !bound_root.has_children(ast) {
                    // If the bound of the expression is `?` (as it is after parsing an unbounded
                    // template) don't specify a bound.
                    // TODO(b/140187077): This case becomes redundant when fixed. It also only covers
                    // explicit `?` bounds, typedefs will remain explicit.
                    parts.push(prefixed("@template ", &name));
                } else {
                    parts.push(
                        self.build_annotation_with_type_and_description(
                            ast,
                            "template",
                            &bound_expr,
                            Some(&name),
                        )
                        .as_units()
                        .to_vec(),
                    );
                }
            }
        }
        let type_transformations = info.get_type_transformations();
        if !type_transformations.is_empty() {
            multiline = true;
            for (name, node) in type_transformations {
                let transformation_definition = crate::code_printer::Builder::new(node).build(ast);
                let mut part = prefixed("@template ", &name);
                part.extend(units(" := "));
                part.extend_from_slice(transformation_definition.as_units());
                part.extend(units(" =:"));
                parts.push(part);
            }
        }
        if info.is_override() {
            parts.push(units("@override"));
        }

        if info.has_type() && !info.is_define() {
            if info.is_inline_type() {
                parts.push(
                    self.type_node(ast, info.get_type().unwrap().get_root())
                        .as_units()
                        .to_vec(),
                );
            } else {
                parts.push(
                    self.build_annotation_with_type(ast, "type", &info.get_type().unwrap())
                        .as_units()
                        .to_vec(),
                );
            }
        }
        if info.is_define() {
            parts.push(
                self.build_annotation_with_type(ast, "define", &info.get_type().unwrap())
                    .as_units()
                    .to_vec(),
            );
        }
        if info.has_typedef_type() {
            parts.push(
                self.build_annotation_with_type(ast, "typedef", &info.get_typedef_type().unwrap())
                    .as_units()
                    .to_vec(),
            );
        }
        if info.has_enum_parameter_type() {
            parts.push(
                self.build_annotation_with_type(
                    ast,
                    "enum",
                    &info.get_enum_parameter_type().unwrap(),
                )
                .as_units()
                .to_vec(),
            );
        }
        if info.is_implicit_cast() {
            parts.push(units("@implicitCast"));
        }
        if info.is_no_collapse() {
            parts.push(units("@nocollapse"));
        }

        let suppressions = info.get_suppressions_and_their_description();
        if !suppressions.is_empty() {
            // With ImmutableMap, the iteration order will be same as insertion order (i.e. parse order).
            for (warnings, text) in suppressions {
                let mut warnings: Vec<_> = warnings.into_iter().collect();
                warnings.sort();
                let mut sb = units("@suppress {");
                for (index, warning) in warnings.iter().enumerate() {
                    // Even the warnings inside a suppress annotation are printed in natural order for
                    // consistency
                    if index != 0 {
                        sb.push(b',' as u16);
                    }
                    sb.extend_from_slice(warning.as_units());
                }
                sb.push(b'}' as u16);
                if !text.is_empty() {
                    sb.push(b' ' as u16);
                    sb.extend_from_slice(text.as_units());
                }
                parts.push(sb);
            }
            multiline = true;
        }
        if info.is_deprecated() {
            let reason = info.get_deprecation_reason();
            let mut part = units("@deprecated");
            if let Some(reason) = reason {
                part.push(b' ' as u16);
                part.extend_from_slice(reason.as_units());
            }
            parts.push(part);
            multiline = true;
        }
        if info.is_polymer() {
            multiline = true;
            parts.push(units("@polymer"));
        }
        if info.is_polymer_behavior() {
            multiline = true;
            parts.push(units("@polymerBehavior"));
        }
        if info.is_mixin_function() {
            multiline = true;
            parts.push(units("@mixinFunction"));
        }
        if info.is_mixin_class() {
            multiline = true;
            parts.push(units("@mixinClass"));
        }
        if info.is_custom_element() {
            multiline = true;
            parts.push(units("@customElement"));
        }

        if let Some(id) = info.get_closure_primitive_id() {
            let mut part = prefixed("@closurePrimitive {", &id);
            part.push(b'}' as u16);
            parts.push(part);
        }
        if info.is_ng_inject() {
            parts.push(units("@ngInject"));
        }

        for ts_type in info.get_ts_types() {
            parts.push(prefixed("@tsType ", &ts_type));
        }
        if self.print_desc {
            if let Some(description) = info.get_block_description() {
                let cleaned = clean_description(&description);
                if !cleaned.is_empty() {
                    multiline = true;
                    let mut cleaned = closure_rhino::java_lang::trim(&cleaned).as_units().to_vec();
                    if parts.len() > 1 {
                        // If there is more than one part - the opening "/**" - then add blank line between the
                        // description and everything else.
                        cleaned.push(b'\n' as u16);
                    }
                    parts.insert(1, cleaned);
                }
            }
        }
        let mut sb = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            if index != 0 {
                sb.push(if multiline { b'\n' as u16 } else { b' ' as u16 });
            }
            sb.extend_from_slice(part);
        }
        if !multiline {
            sb.extend(units(" */"));
        }
        // Ensure all lines start with " *", and then ensure all non blank lines have a space after
        // the *.
        let mut s = Vec::new();
        for (index, &c) in sb.iter().enumerate() {
            s.push(c);
            if c == b'\n' as u16 {
                s.extend(units(" *"));
                if sb
                    .get(index + 1)
                    .is_some_and(|&c| c != b' ' as u16 && c != b'\n' as u16)
                {
                    s.push(b' ' as u16);
                }
            }
        }
        s.extend(units(if multiline { "\n */\n" } else { " " }));
        JsString::from_units(s)
    }
    // port: JSDocInfoPrinter#stripBang
    fn strip_bang(&self, ast: &Ast, mut type_node: NodeId) -> NodeId {
        if type_node.get_token(ast) == Token::BANG {
            type_node = type_node.get_first_child(ast).unwrap();
        }
        type_node
    }
    // port: JSDocInfoPrinter#buildAnnotationWithType(String,JSTypeExpression)
    fn build_annotation_with_type(
        &self,
        ast: &Ast,
        annotation: &str,
        ty: &JSTypeExpression,
    ) -> JsString {
        self.build_annotation_with_type_and_description(ast, annotation, ty, None)
    }
    // port: JSDocInfoPrinter#buildAnnotationWithType(String,JSTypeExpression,String)
    fn build_annotation_with_type_and_description(
        &self,
        ast: &Ast,
        annotation: &str,
        ty: &JSTypeExpression,
        description: Option<&JsString>,
    ) -> JsString {
        self.build_annotation_with_type_node_and_description(
            ast,
            annotation,
            ty.get_root(),
            description,
        )
    }
    // port: JSDocInfoPrinter#buildAnnotationWithType(String,Node)
    fn build_annotation_with_type_node(&self, ast: &Ast, annotation: &str, ty: NodeId) -> JsString {
        self.build_annotation_with_type_node_and_description(ast, annotation, ty, None)
    }
    // port: JSDocInfoPrinter#buildAnnotationWithType(String,Node,String)
    fn build_annotation_with_type_node_and_description(
        &self,
        ast: &Ast,
        annotation: &str,
        ty: NodeId,
        description: Option<&JsString>,
    ) -> JsString {
        let mut sb = units("@");
        sb.extend(units(annotation));
        sb.extend(units(" {"));
        self.append_type_node(ast, &mut sb, ty);
        sb.push(b'}' as u16);
        if let Some(description) = description {
            sb.push(b' ' as u16);
            sb.extend_from_slice(description.as_units());
        }
        JsString::from_units(sb)
    }
    // port: JSDocInfoPrinter#buildParamType
    fn build_param_type(&self, ast: &Ast, info: &JSDocInfo, name: &JsString) -> JsString {
        let ty = info.get_parameter_type(name.clone());
        if let Some(ty) = ty {
            let mut p = units("{");
            p.extend_from_slice(self.type_node(ast, ty.get_root()).as_units());
            p.extend(units("} "));
            p.extend_from_slice(name.as_units());
            if self.print_desc {
                // Don't add a leading space; the parser retained it.
                if let Some(description) = info.get_description_for_parameter(name.clone()) {
                    p.extend_from_slice(description.as_units());
                }
            }
            closure_rhino::java_lang::trim(&JsString::from_units(p))
        } else {
            name.clone()
        }
    }
    // port: JSDocInfoPrinter#typeNode
    fn type_node(&self, ast: &Ast, type_node: NodeId) -> JsString {
        let mut sb = Vec::new();
        self.append_type_node(ast, &mut sb, type_node);
        JsString::from_units(sb)
    }
    // port: JSDocInfoPrinter#appendTypeNode
    fn append_type_node(&self, ast: &Ast, sb: &mut Vec<u16>, type_node: NodeId) {
        if self.use_original_name {
            if let Some(name) = type_node.get_original_name(ast) {
                sb.extend_from_slice(name.as_units());
                return;
            }
        }
        match type_node.get_token(ast) {
            Token::BANG => {
                sb.push(b'!' as u16);
                self.append_type_node(ast, sb, type_node.get_first_child(ast).unwrap());
            }
            Token::EQUALS => {
                self.append_type_node(ast, sb, type_node.get_first_child(ast).unwrap());
                sb.push(b'=' as u16);
            }
            Token::PIPE => {
                sb.push(b'(' as u16);
                let last_child = type_node.get_last_child(ast);
                for child in type_node.children(ast) {
                    self.append_type_node(ast, sb, child);
                    if Some(child) != last_child {
                        sb.push(b'|' as u16);
                    }
                }
                sb.push(b')' as u16);
            }
            Token::ITER_REST => {
                sb.extend(units("..."));
                if type_node.has_children(ast)
                    && !type_node.get_first_child(ast).unwrap().is_empty(ast)
                {
                    self.append_type_node(ast, sb, type_node.get_first_child(ast).unwrap());
                }
            }
            Token::STAR => sb.push(b'*' as u16),
            Token::QMARK => {
                sb.push(b'?' as u16);
                if type_node.has_children(ast) {
                    self.append_type_node(ast, sb, type_node.get_first_child(ast).unwrap());
                }
            }
            Token::FUNCTION => self.append_function_node(ast, sb, type_node),
            Token::LC => {
                sb.push(b'{' as u16);
                let lb = type_node.get_first_child(ast).unwrap();
                let last_colon = lb.get_last_child(ast);
                for colon in lb.children(ast) {
                    if colon.has_children(ast) {
                        sb.extend_from_slice(
                            colon
                                .get_first_child(ast)
                                .unwrap()
                                .get_string(ast)
                                .as_units(),
                        );
                        sb.push(b':' as u16);
                        self.append_type_node(ast, sb, colon.get_last_child(ast).unwrap());
                    } else {
                        sb.extend_from_slice(colon.get_string_ref(ast).as_units());
                    }
                    if Some(colon) != last_colon {
                        sb.push(b',' as u16);
                    }
                }
                sb.push(b'}' as u16);
            }
            Token::VOID => sb.extend(units("void")),
            Token::TYPEOF => {
                sb.extend(units("typeof "));
                self.append_type_node(ast, sb, type_node.get_first_child(ast).unwrap());
            }
            Token::BLOCK => {
                sb.push(b'<' as u16);
                let last = type_node.get_last_child(ast);
                for ty in type_node.children(ast) {
                    self.append_type_node(ast, sb, ty);
                    if Some(ty) != last {
                        sb.push(b',' as u16);
                    }
                }
                sb.push(b'>' as u16);
            }
            Token::STRINGLIT => {
                sb.extend_from_slice(type_node.get_string_ref(ast).as_units());
                if type_node.has_children(ast) {
                    self.append_type_node(ast, sb, type_node.get_only_child(ast));
                }
            }
            _ => panic!("Unexpected typeNode: {}", type_node.to_string(ast)),
        }
    }
    // port: JSDocInfoPrinter#appendFunctionNode
    fn append_function_node(&self, ast: &Ast, sb: &mut Vec<u16>, function: NodeId) {
        let mut has_new_or_this = false;
        sb.extend(units("function("));
        let first = function.get_first_child(ast).unwrap();
        if first.is_new(ast) {
            sb.extend(units("new:"));
            self.append_type_node(ast, sb, first.get_first_child(ast).unwrap());
            has_new_or_this = true;
        } else if first.is_this(ast) {
            sb.extend(units("this:"));
            self.append_type_node(ast, sb, first.get_first_child(ast).unwrap());
            has_new_or_this = true;
        } else if first.is_empty(ast) {
            sb.push(b')' as u16);
            return;
        } else if !first.is_param_list(ast) {
            sb.extend(units("):"));
            self.append_type_node(ast, sb, first);
            return;
        }
        let mut param_list = None;
        if first.is_param_list(ast) {
            param_list = Some(first);
        } else if first.get_next(ast).unwrap().is_param_list(ast) {
            param_list = first.get_next(ast);
        }
        if let Some(param_list) = param_list {
            let mut first_param = true;
            for param in param_list.children(ast) {
                if !first_param || has_new_or_this {
                    sb.push(b',' as u16);
                }
                self.append_type_node(ast, sb, param);
                first_param = false;
            }
        }
        sb.push(b')' as u16);
        let return_type = function.get_last_child(ast).unwrap();
        if !return_type.is_empty(ast) {
            sb.push(b':' as u16);
            self.append_type_node(ast, sb, return_type);
        }
    }
}
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn prefixed(prefix: &str, s: &JsString) -> Vec<u16> {
    let mut out = units(prefix);
    out.extend_from_slice(s.as_units());
    out
}
// Java replaceAll("\n\\s*\\*\\s*", "\n"), with Pattern's default ASCII whitespace.
fn clean_description(s: &JsString) -> JsString {
    let mut out = Vec::new();
    let input = s.as_units();
    let mut i = 0;
    while i < input.len() {
        out.push(input[i]);
        if input[i] == b'\n' as u16 {
            let mut p = i + 1;
            while p < input.len()
                && matches!(input[p], c if (b'\t' as u16..=b'\r' as u16).contains(&c) || c == b' ' as u16)
            {
                p += 1;
            }
            if input.get(p) == Some(&(b'*' as u16)) {
                p += 1;
                while p < input.len()
                    && matches!(input[p], c if (b'\t' as u16..=b'\r' as u16).contains(&c) || c == b' ' as u16)
                {
                    p += 1;
                }
                i = p;
                continue;
            }
        }
        i += 1;
    }
    JsString::from_units(out)
}
