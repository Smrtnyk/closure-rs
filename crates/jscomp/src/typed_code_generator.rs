/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/TypedCodeGenerator.java.

#![allow(clippy::collapsible_if, clippy::nonminimal_bool)] // Retain Java branches.
use crate::{
    code_consumer::CodeConsumer,
    code_generator::{CodeGeneration, CodeGenerator, Context},
    compiler_options::CompilerOptions,
    js_doc_info_printer::JSDocInfoPrinter,
    node_util::NodeUtil,
};
use closure_jstype::{function_type::Parameter, js_type::Nullability, prelude::*};
use closure_rhino::{
    check_argument, check_state,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::collections::BTreeSet;

/// A code generator that outputs type annotations for functions and
/// constructors.
pub struct TypedCodeGenerator<'a> {
    base: CodeGenerator<'a>,
    registry: &'a mut JSTypeRegistry,
    js_doc_info_printer: JSDocInfoPrinter,
}
impl<'a> TypedCodeGenerator<'a> {
    // port: TypedCodeGenerator#TypedCodeGenerator
    pub fn new(
        consumer: &'a mut dyn CodeConsumer,
        options: &CompilerOptions,
        registry: &'a mut JSTypeRegistry,
    ) -> Self {
        Self {
            base: CodeGenerator::new(consumer, options),
            registry,
            js_doc_info_printer: JSDocInfoPrinter::new(options.get_use_original_names_in_output()),
        }
    }

    // port: TypedCodeGenerator#maybeAddTypeAnnotation
    fn maybe_add_type_annotation(&mut self, ast: &Ast, n: NodeId) {
        let parent = n.get_parent(ast);
        let Some(parent) = parent else {
            // root node cannot have a type annotation.
            return;
        };
        // Generate type annotations only for statements and class member functions.
        if parent.is_block(ast) || parent.is_script(ast) || parent.is_class_members(ast) {
            if n.is_class(ast) || n.is_function(ast) || n.is_member_function_def(ast) {
                let annotation = self.get_type_annotation(ast, n);
                self.add(&annotation);
            } else if n.is_expr_result(ast) && n.get_first_child(ast).unwrap().is_assign(ast) {
                let assign = n.get_first_child(ast).unwrap();
                if NodeUtil::is_namespace_decl(ast, assign.get_first_child(ast).unwrap()) {
                    self.add(
                        &self.js_doc_info_printer.print(
                            ast,
                            &assign
                                .get_jsdoc_info(ast)
                                .expect("NullPointerException: JSDocInfo"),
                        ),
                    );
                } else {
                    let rhs = assign.get_last_child(ast).unwrap();
                    let annotation = self.get_type_annotation(ast, rhs);
                    self.add(&annotation);
                }
            } else if NodeUtil::is_name_declaration(ast, Some(n))
                && n.get_first_first_child(ast).is_some()
            {
                // All namespace declarations except `const x = {};` are signified by @const JSDoc.
                if NodeUtil::is_namespace_decl(ast, n.get_first_child(ast).unwrap())
                    && n.get_jsdoc_info_ref(ast).is_some()
                {
                    self.add(
                        &self
                            .js_doc_info_printer
                            .print(ast, &n.get_jsdoc_info(ast).unwrap()),
                    );
                } else {
                    let annotation =
                        self.get_type_annotation(ast, n.get_first_first_child(ast).unwrap());
                    self.add(&annotation);
                }
            }
        }
    }

    // port: TypedCodeGenerator#getTypeAnnotation
    fn get_type_annotation(&mut self, ast: &Ast, node: NodeId) -> JsString {
        if node.is_member_function_def(ast) {
            // For a member function the type information is actually on the function it contains,
            // so just generate the type annotation for that.
            self.get_member_function_annotation(ast, node.get_only_child(ast))
        } else if node.is_class(ast) {
            self.get_class_annotation(ast, node.get_jstype(ast))
        } else if node.is_function(ast) {
            self.get_function_annotation(ast, node)
        } else {
            let node_originally_had_jsdoc = NodeUtil::get_best_jsdoc_info(ast, node).is_some();
            if !node_originally_had_jsdoc {
                // For nodes that don't inherently define a type, ony generate JSDoc if they originally
                // had some.
                return "".into();
            }
            let type_ = node.get_jstype(ast);
            if let Some(type_) = type_ {
                if type_.is_function_type(self.registry) {
                    self.get_function_annotation(ast, node)
                } else if type_.is_enum_type(self.registry) {
                    let enum_type = type_
                        .to_maybe_object_type(self.registry)
                        .unwrap()
                        .get_enumerated_type_of_enum_object(self.registry)
                        .unwrap();
                    format!(
                        "/** @enum {{{}}} */\n",
                        enum_type.to_annotation_string(self.registry, ast, Nullability::EXPLICIT)
                    )
                    .into()
                } else if !type_.is_unknown_type(self.registry, ast)
                    && !type_.is_empty_type(self.registry)
                    && !type_.is_void_type(self.registry)
                    && !type_.is_function_prototype_type(self.registry)
                {
                    format!(
                        "/** @type {{{}}} */\n",
                        node.get_jstype(ast).unwrap().to_annotation_string(
                            self.registry,
                            ast,
                            Nullability::EXPLICIT
                        )
                    )
                    .into()
                } else {
                    "".into()
                }
            } else {
                "".into()
            }
        }
    }

    /// @param fnNode A node for a function for which to generate a type annotation
    // port: TypedCodeGenerator#getFunctionAnnotation
    fn get_function_annotation(&mut self, ast: &Ast, fn_node: NodeId) -> JsString {
        let type_ = fn_node.get_jstype(ast);
        check_state!(
            fn_node.is_function(ast)
                || type_
                    .expect("NullPointerException: JSType")
                    .is_function_type(self.registry)
        );
        let Some(type_) = type_ else {
            return "".into();
        };
        if type_.is_unknown_type(self.registry, ast) {
            return "".into();
        }
        let fun_type = type_.to_maybe_function_type(self.registry).unwrap();
        if type_.equals(
            self.registry,
            ast,
            self.registry.get_native_type(JSTypeNative::FUNCTION_TYPE),
        ) {
            return "/** @type {!Function} */\n".into();
        }
        let mut sb = JsString::from("/**\n");
        let mut param_node = None;
        // We need to use the child nodes of the function as the nodes for the
        // parameters of the function type do not have the real parameter names.
        // FUNCTION
        //   NAME
        //   PARAM_LIST
        //     NAME param1
        //     NAME param2
        if fn_node.is_function(ast) {
            param_node = NodeUtil::get_function_parameters(ast, fn_node).get_first_child(ast);
        }
        // Param types
        self.append_function_param_annotations(ast, &mut sb, fun_type, param_node);
        // Return type
        let ret_type = fun_type.get_return_type(self.registry);
        if !ret_type.is_empty_type(self.registry) // There is no annotation for the empty type.
            && !fun_type.is_interface(self.registry) // Interfaces never return a value.
            && !(fun_type.is_constructor(self.registry) && ret_type.is_void_type(self.registry))
        {
            sb = sb.concat(&" * ".into());
            Self::append_annotation(
                &mut sb,
                "return",
                &ret_type.to_annotation_string(self.registry, ast, Nullability::EXPLICIT),
            );
            sb = sb.concat(&"\n".into());
        }
        // This function could be defining an ES5-style class or interface.
        // If it isn't but still requires a type for `this`, then we need to explicitly add
        // an annotation for that.
        if fun_type.is_constructor(self.registry) {
            // This function is defining an ES5-style class, so include the class annotations here.
            self.append_class_annotations(ast, &mut sb, fun_type);
            sb = sb.concat(&" * @constructor\n".into());
        } else if fun_type.is_interface(self.registry) {
            self.append_interface_annotations(ast, &mut sb, fun_type);
        } else {
            let this_type = fun_type.get_type_of_this(self.registry);
            if let Some(this_type) = this_type {
                if !this_type.is_unknown_type(self.registry, ast)
                    && !this_type.is_void_type(self.registry)
                {
                    let method_owner = self.find_method_owner(ast, Some(fn_node));
                    if !this_type.equals(self.registry, ast, method_owner) {
                        sb = sb.concat(&" * ".into());
                        Self::append_annotation(
                            &mut sb,
                            "this",
                            &this_type.to_annotation_string(
                                self.registry,
                                ast,
                                Nullability::EXPLICIT,
                            ),
                        );
                        sb = sb.concat(&"\n".into());
                    }
                }
            }
        }
        self.append_template_annotations(
            ast,
            &mut sb,
            &fun_type.get_type_parameters(self.registry),
        );
        sb.concat(&" */\n".into())
    }

    /// @param fnNode A function node child of a MEMBER_FUNCTION_DEF
    // port: TypedCodeGenerator#getMemberFunctionAnnotation
    fn get_member_function_annotation(&mut self, ast: &Ast, fn_node: NodeId) -> JsString {
        check_state!(
            fn_node.is_function(ast)
                && fn_node.get_parent(ast).unwrap().is_member_function_def(ast),
            "%s",
            fn_node.to_string(ast)
        );
        let type_ = fn_node.get_jstype(ast);
        let Some(type_) = type_ else {
            return "".into();
        };
        if type_.is_unknown_type(self.registry, ast) {
            return "".into();
        }
        let fun_type = type_.to_maybe_function_type(self.registry).unwrap();
        let mut sb = JsString::from("/**\n");
        // We need to use the child nodes of the function as the nodes for the
        // parameters of the function type do not have the real parameter names.
        // FUNCTION
        //   NAME
        //   PARAM_LIST
        //     NAME param1
        //     NAME param2
        let param_node = NodeUtil::get_function_parameters(ast, fn_node).get_first_child(ast);
        // Param types
        self.append_function_param_annotations(ast, &mut sb, fun_type, param_node);
        if NodeUtil::is_es6_constructor(ast, fn_node) {
            self.append_template_annotations(
                ast,
                &mut sb,
                &fun_type.get_constructor_only_template_parameters(self.registry),
            );
            // no return type for the constructor
        } else {
            self.append_template_annotations(
                ast,
                &mut sb,
                &fun_type.get_type_parameters(self.registry),
            );
            // Return type
            let ret_type = fun_type.get_return_type(self.registry);
            if !ret_type.is_empty_type(self.registry) {
                // There is no annotation for the empty type.
                sb = sb.concat(&" * ".into());
                Self::append_annotation(
                    &mut sb,
                    "return",
                    &ret_type.to_annotation_string(self.registry, ast, Nullability::EXPLICIT),
                );
                sb = sb.concat(&"\n".into());
            }
        }
        sb.concat(&" */\n".into())
    }

    /// Generates @param annotations.
    ///
    /// @param sb annotations will be appended here
    /// @param funType function type
    /// @param paramNode parameter names will be taken from here
    // port: TypedCodeGenerator#appendFunctionParamAnnotations
    fn append_function_param_annotations(
        &mut self,
        ast: &Ast,
        sb: &mut JsString,
        fun_type: TypeId,
        mut param_node: Option<NodeId>,
    ) {
        let min_arity = fun_type.get_min_arity(self.registry);
        let max_arity = fun_type.get_max_arity(self.registry);
        let formals = fun_type.get_parameters(self.registry);
        for i in 0..formals.len() {
            *sb = sb.concat(&" * ".into());
            Self::append_annotation(
                sb,
                "param",
                &self.get_parameter_jsdoc_type(ast, &formals, i, min_arity, max_arity),
            );
            let parameter_name = self.get_parameter_jsdoc_name(ast, param_node, i);
            *sb = sb
                .concat(&" ".into())
                .concat(&parameter_name)
                .concat(&"\n".into());
            if let Some(node) = param_node {
                param_node = node.get_next(ast);
            }
        }
    }

    // port: TypedCodeGenerator#getClassAnnotation
    fn get_class_annotation(&mut self, ast: &Ast, class_type: Option<TypeId>) -> JsString {
        let Some(class_type) = class_type else {
            return "".into();
        };
        if class_type.is_unknown_type(self.registry, ast) {
            return "".into();
        }
        check_state!(
            class_type.is_function_type(self.registry),
            "%s",
            class_type.to_string(self.registry, ast)
        );
        let fun_type = class_type.to_maybe_function_type(self.registry).unwrap();
        let mut sb = JsString::from("");
        if fun_type.is_interface(self.registry) {
            self.append_interface_annotations(ast, &mut sb, fun_type);
        } else {
            check_state!(
                fun_type.is_constructor(self.registry),
                "%s",
                fun_type.to_string(self.registry, ast)
            );
            self.append_class_annotations(ast, &mut sb, fun_type);
        }
        self.append_template_annotations(
            ast,
            &mut sb,
            &fun_type.get_type_parameters(self.registry),
        );
        let jsdoc_content = sb;
        // For simple class, it's possible we didn't end up generating any JSDoc at all.
        if jsdoc_content.is_empty() {
            jsdoc_content
        } else {
            JsString::from("/**\n")
                .concat(&jsdoc_content)
                .concat(&" */\n".into())
        }
    }

    // port: TypedCodeGenerator#appendTemplateAnnotations
    fn append_template_annotations(
        &mut self,
        ast: &Ast,
        sb: &mut JsString,
        type_params: &[TypeId],
    ) {
        if !type_params.is_empty() {
            *sb = sb.concat(&" * @template ".into());
            *sb = sb.concat(
                &type_params
                    .iter()
                    .map(|&var| self.format_type_var(ast, var))
                    .collect::<Vec<_>>()
                    .join(",")
                    .into(),
            );
            *sb = sb.concat(&"\n".into());
        }
    }

    /// Return the name of the parameter to be used in JSDoc, generating one for destructuring
    /// parameters.
    ///
    /// @param paramNode child node of a parameter list
    /// @param paramIndex position of child in the list
    /// @return name to use in JSDoc
    // port: TypedCodeGenerator#getParameterJSDocName
    fn get_parameter_jsdoc_name(
        &self,
        ast: &Ast,
        mut param_node: Option<NodeId>,
        param_index: usize,
    ) -> JsString {
        let mut name_node = None;
        if let Some(mut node) = param_node {
            check_argument!(
                node.get_parent(ast).unwrap().is_param_list(ast),
                "%s",
                node.to_string(ast)
            );
            if node.is_rest(ast) {
                // use `restParam` of `...restParam`
                // restParam might still be a destructuring pattern
                node = node.get_only_child(ast);
                param_node = Some(node);
            } else if node.is_default_value(ast) {
                // use `defaultParam` of `defaultParam = something`
                // defaultParam might still be a destructuring pattern
                param_node = node.get_first_child(ast);
                node = param_node.unwrap();
            }
            if node.is_name(ast) {
                name_node = param_node;
            } else {
                check_state!(
                    node.is_object_pattern(ast) || node.is_array_pattern(ast),
                    "%s",
                    node.to_string(ast)
                );
                name_node = None; // must generate a fake name
            }
        }
        if let Some(name_node) = name_node {
            check_state!(name_node.is_name(ast), "%s", name_node.to_string(ast));
            name_node.get_string(ast)
        } else {
            format!("p{param_index}").into()
        }
    }

    // port: TypedCodeGenerator#formatTypeVar
    fn format_type_var(&mut self, ast: &Ast, var: TypeId) -> String {
        var.to_annotation_string(self.registry, ast, Nullability::IMPLICIT)
    }

    // TODO(dimvar): it's awkward that we print @constructor after the extends/implements;
    // we should print it first, like users write it. Same for @interface and @record.
    // port: TypedCodeGenerator#appendClassAnnotations
    fn append_class_annotations(&mut self, ast: &Ast, sb: &mut JsString, fun_type: TypeId) {
        let super_constructor = fun_type
            .get_instance_type(self.registry)
            .unwrap()
            .get_super_class_constructor(self.registry, ast);
        if let Some(super_constructor) = super_constructor {
            let super_instance = super_constructor.get_instance_type(self.registry).unwrap();
            if super_instance.to_string(self.registry, ast) != "Object" {
                *sb = sb.concat(&" * ".into());
                Self::append_annotation(
                    sb,
                    "extends",
                    &super_instance.to_annotation_string(self.registry, ast, Nullability::IMPLICIT),
                );
                *sb = sb.concat(&"\n".into());
            }
        }
        // Avoid duplicates, add implemented type to a set first
        let mut interfaces = BTreeSet::<JsString>::new();
        for interfaze in fun_type.get_ancestor_interfaces(self.registry, ast) {
            interfaces.insert(
                interfaze
                    .to_annotation_string(self.registry, ast, Nullability::IMPLICIT)
                    .into(),
            );
        }
        for interfaze in interfaces {
            *sb = sb.concat(&" * ".into());
            Self::append_annotation(sb, "implements", &interfaze.to_string_lossy());
            *sb = sb.concat(&"\n".into());
        }
    }

    // port: TypedCodeGenerator#appendInterfaceAnnotations
    fn append_interface_annotations(&mut self, ast: &Ast, sb: &mut JsString, fun_type: TypeId) {
        let mut interfaces = BTreeSet::<JsString>::new();
        for interface_type in fun_type.get_ancestor_interfaces(self.registry, ast) {
            interfaces.insert(
                interface_type
                    .to_annotation_string(self.registry, ast, Nullability::IMPLICIT)
                    .into(),
            );
        }
        for interfaze in interfaces {
            *sb = sb.concat(&" * ".into());
            Self::append_annotation(sb, "extends", &interfaze.to_string_lossy());
            *sb = sb.concat(&"\n".into());
        }
        *sb = sb.concat(
            &if fun_type.is_structural_interface(self.registry) {
                " * @record\n"
            } else {
                " * @interface\n"
            }
            .into(),
        );
    }

    // TODO(sdh): This whole method could be deleted if we don't mind adding
    // additional @this annotations where they're not actually necessary.
    /// Given a method definition node, returns the {@link ObjectType} corresponding to the class the
    /// method is defined on, or null if it is not a prototype method.
    // port: TypedCodeGenerator#findMethodOwner
    fn find_method_owner(&mut self, ast: &Ast, n: Option<NodeId>) -> Option<TypeId> {
        let n = n?;
        let parent = n.get_parent(ast).unwrap();
        let mut ctor = None;
        if parent.is_assign(ast) {
            let target = parent.get_first_child(ast).unwrap();
            if NodeUtil::is_prototype_property(ast, target) {
                // TODO(johnlenz): handle non-global types
                let type_ = self.registry.get_global_type(
                    ast,
                    target
                        .get_first_first_child(ast)
                        .unwrap()
                        .get_qualified_name(ast)
                        .unwrap(),
                );
                ctor = type_.and_then(|type_| {
                    // Java (ObjectType) is a checked cast, not toMaybeObjectType.
                    let object = type_
                        .to_maybe_object_type(self.registry)
                        .expect("ClassCastException: ObjectType");
                    object.get_constructor(self.registry)
                });
            }
        } else if parent.is_class(ast) {
            // TODO(sdh): test this case once the type checker understands ES6 classes
            ctor = parent
                .get_jstype(ast)
                .unwrap()
                .to_maybe_function_type(self.registry);
        }
        ctor.and_then(|ctor| ctor.get_instance_type(self.registry))
    }

    // port: TypedCodeGenerator#appendAnnotation
    fn append_annotation(sb: &mut JsString, name: &str, type_: &str) {
        *sb = sb
            .concat(&"@".into())
            .concat(&name.into())
            .concat(&" {".into())
            .concat(&type_.into())
            .concat(&"}".into());
    }

    /// Creates a JSDoc-suitable String representation of the type of a parameter.
    // port: TypedCodeGenerator#getParameterJSDocType
    fn get_parameter_jsdoc_type(
        &mut self,
        ast: &Ast,
        parameters: &[Parameter],
        index: usize,
        min_args: usize,
        max_args: usize,
    ) -> String {
        let type_ = parameters[index].get_jstype();
        if index < min_args {
            return type_.to_annotation_string(self.registry, ast, Nullability::EXPLICIT);
        }
        let is_rest_argument = max_args == i32::MAX as usize && index == parameters.len() - 1;
        let restricted = self.restrict_by_undefined(ast, type_);
        if is_rest_argument {
            format!(
                "...{}",
                restricted.to_annotation_string(self.registry, ast, Nullability::EXPLICIT)
            )
        } else {
            format!(
                "{}=",
                restricted.to_annotation_string(self.registry, ast, Nullability::EXPLICIT)
            )
        }
    }

    /// Removes undefined from a union type.
    // port: TypedCodeGenerator#restrictByUndefined
    fn restrict_by_undefined(&mut self, ast: &Ast, type_: TypeId) -> TypeId {
        // If not voidable, there's nothing to do. If not nullable then the easiest
        // thing is to simply remove both null and undefined. If nullable, then add
        // null back into the union after removing null and undefined.
        if !type_.is_voidable(self.registry, ast) {
            return type_;
        }
        let restricted = type_.restrict_by_not_null_or_undefined(self.registry, ast);
        if type_.is_nullable(self.registry, ast) {
            let null_type = self.registry.get_native_type(JSTypeNative::NULL_TYPE);
            return self
                .registry
                .create_union_type(ast, &[restricted, null_type]);
        }
        // The bottom type cannot appear in a jsdoc
        if restricted.is_empty_type(self.registry) {
            type_
        } else {
            restricted
        }
    }
}
impl<'a> CodeGeneration<'a> for TypedCodeGenerator<'a> {
    fn code_generator(&self) -> &CodeGenerator<'a> {
        &self.base
    }
    fn code_generator_mut(&mut self) -> &mut CodeGenerator<'a> {
        &mut self.base
    }
    // port: TypedCodeGenerator#add
    fn add_node_with_context(&mut self, ast: &Ast, n: NodeId, context: Context) {
        self.maybe_add_type_annotation(ast, n);
        self.add_node_with_comments(ast, n, context, true);
    }
}
