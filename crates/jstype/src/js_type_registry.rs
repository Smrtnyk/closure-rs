/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Bob Jervis
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/jstype/JSTypeRegistry.java.

// Preserve Java branch structure and API arity.
#![allow(clippy::collapsible_if, clippy::too_many_arguments)]
use crate::{
    TypeId,
    all_type::AllType,
    big_int_type::BigIntType,
    boolean_type::BooleanType,
    enum_type::EnumType,
    function_param_builder::FunctionParamBuilder,
    function_type::{FunctionType, FunctionTypeBuilder, Kind, Parameter},
    js_type::{HasPropertyKind, JSType, JSTypeData, JSTypeKind},
    js_type_native::JSTypeNative,
    js_type_resolver::JSTypeResolver,
    named_type::{NamedType, NamedTypeBuilder, ResolutionKind},
    no_object_type::NoObjectType,
    no_type::NoType,
    null_type::NullType,
    number_type::NumberType,
    object_type::{ObjectType, ObjectTypeData},
    property::{PropertyData, PropertyId},
    prototype_object_type::{PrototypeObjectType, PrototypeObjectTypeBuilder},
    record_type::RecordType,
    record_type_builder::RecordTypeBuilder,
    static_typed_scope::StaticTypedScope,
    static_typed_slot::StaticTypedSlot,
    string_type::StringType,
    symbol_type::SymbolType,
    template_type_map::TemplateTypeMap,
    union_type::{UnionType, UnionTypeBuilder},
    unknown_type::UnknownType,
    void_type::VoidType,
};
use closure_rhino::{
    check_argument, check_state,
    error_reporter::ErrorReporter,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::JSDocInfo,
    msg::Msg,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::sync::{Arc, Mutex};

pub const OBJECT_ELEMENT_TEMPLATE: &str = "IOBJECT_VALUE";

/// Rust-only: the `Set<String> forwardDeclaredTypes` Java's registry shares with its creator; the
/// registry only asks `contains`.
pub trait ForwardDeclaredTypeSet: Send + Sync {
    fn contains(&self, name: &JsString) -> bool;
}
impl ForwardDeclaredTypeSet for IndexSet<JsString> {
    fn contains(&self, name: &JsString) -> bool {
        IndexSet::contains(self, name)
    }
}
/// A set its owner keeps changing (Java's `LinkedHashSet` shared with the registry).
impl<T: ForwardDeclaredTypeSet + Send> ForwardDeclaredTypeSet for Mutex<T> {
    fn contains(&self, name: &JsString) -> bool {
        self.lock().unwrap().contains(name)
    }
}
pub struct JSTypeRegistry {
    pub(crate) types: Vec<JSTypeData>,
    pub(crate) properties: Vec<PropertyData>,
    pub(crate) resolver: JSTypeResolver,
    pub(crate) empty_template_type_map: Option<Arc<TemplateTypeMap>>,
    reporter: SharedErrorReporter,
    native_types: Vec<Option<TypeId>>,
    // HashBasedTable is lookup-only; no Java hash iteration is observable.
    scoped_name_table: IndexMap<(NodeId, JsString), TypeId>,
    closure_namespaces: IndexMap<JsString, ClosureNamespace>,
    name_table_global_root: NodeId,
    // Both hash-key and hash-value maps are lookup-only.
    non_nullable_type_names: IndexMap<NodeId, IndexSet<JsString>>,
    // Java Set is lookup-only; Java shares the creator's Set (the Compiler adds to it later).
    forward_declared_types: Arc<dyn ForwardDeclaredTypeSet>,
    // Java hash keys are lookup-only; value sets retain insertion hashes and Java equals.
    non_ref_types_indexed_by_property: IndexMap<JsString, Vec<(i32, TypeId)>>,
    sentinel_object_literal: Option<TypeId>,
    properties_of_supertypes_in_unions: IndexSet<JsString>,
    dropped_properties_of_unions: IndexSet<JsString>,
    // Java hash keys are lookup-only; value sets retain insertion hashes and Java equals.
    each_ref_type_indexed_by_property: IndexMap<JsString, Vec<(i32, TypeId)>>,
    i_object_index_template_key: Option<TypeId>,
    i_object_element_template_key: Option<TypeId>,
    iterable_value_template: Option<TypeId>,
    iterable_return_template: Option<TypeId>,
    iterator_iterable_value_template: Option<TypeId>,
    iterator_iterable_return_template: Option<TypeId>,
    iterator_like_value_template: Option<TypeId>,
    iterator_like_return_template: Option<TypeId>,
    iterator_value_template: Option<TypeId>,
    iterator_return_template: Option<TypeId>,
    iiterable_result_value_template: Option<TypeId>,
    async_iterable_value_template: Option<TypeId>,
    async_iterable_return_template: Option<TypeId>,
    async_iterator_value_template: Option<TypeId>,
    async_iterator_return_template: Option<TypeId>,
    generator_value_template: Option<TypeId>,
    generator_return_template: Option<TypeId>,
    i_thenable_template_key: Option<TypeId>,
    promise_template_key: Option<TypeId>,
    array_element_template_key: Option<TypeId>,
    readonly_array_element_template_key: Option<TypeId>,
    readonly_map_key_template_key: Option<TypeId>,
    readonly_map_value_template_key: Option<TypeId>,
    map_key_template_key: Option<TypeId>,
    map_value_template_key: Option<TypeId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropDefinitionKind {
    UNKNOWN,
    KNOWN,
    LOOSE,
    LOOSE_UNION,
}

#[derive(Clone)]
pub struct SharedErrorReporter {
    inner: Arc<Mutex<Box<dyn ErrorReporter + Send>>>,
}
impl SharedErrorReporter {
    // Registries are explicitly not thread-safe in Java; cloning retains reporter identity.
    #[allow(clippy::arc_with_non_send_sync)]
    fn new(reporter: Box<dyn ErrorReporter + Send>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(reporter)),
        }
    }
}
impl ErrorReporter for SharedErrorReporter {
    fn warning(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.inner
            .lock()
            .unwrap()
            .warning(message, source_name, line, line_offset);
    }
    fn error(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.inner
            .lock()
            .unwrap()
            .error(message, source_name, line, line_offset);
    }
}

impl JSTypeRegistry {
    // port: JSTypeRegistry#evaluateTypeExpressionInGlobalScope
    pub fn evaluate_type_expression_in_global_scope(
        &mut self,
        ast: &mut Ast,
        expr: &JSTypeExpression,
    ) -> TypeId {
        use crate::rhino::js_type_expression::JSTypeExpressionExt;
        expr.evaluate(self, ast, None)
    }
    // port: JSTypeRegistry#evaluateTypeExpression
    pub fn evaluate_type_expression(
        &mut self,
        ast: &Ast,
        expr: &JSTypeExpression,
        scope: Option<Arc<dyn StaticTypedScope>>,
    ) -> TypeId {
        self.create_type_from_comment_node_with_location(
            ast,
            expr.get_root(),
            expr.get_source_name(),
            scope,
        )
    }
    // port: JSTypeRegistry#createTypeFromCommentNode
    pub fn create_type_from_comment_node(&mut self, ast: &Ast, n: NodeId) -> TypeId {
        self.create_type_from_comment_node_with_location(ast, n, "[internal]", None)
    }
    // port: JSTypeRegistry#createTypeFromCommentNode(Node,String,StaticTypedScope)
    pub fn create_type_from_comment_node_with_location(
        &mut self,
        ast: &Ast,
        n: NodeId,
        source_name: &str,
        scope: Option<Arc<dyn StaticTypedScope>>,
    ) -> TypeId {
        match n.get_token(ast) {
            Token::LC => self.create_record_type_from_nodes(
                ast,
                n.get_first_child(ast).unwrap(),
                source_name,
                scope,
            ),
            Token::BANG => {
                let child = self.create_type_from_comment_node_with_location(
                    ast,
                    n.get_first_child(ast).unwrap(),
                    source_name,
                    scope,
                );
                if child.is_named_type(self) {
                    child.get_bang_type(self, ast)
                } else {
                    child.restrict_by_not_null_or_undefined(self, ast)
                }
            }
            Token::QMARK => {
                if let Some(first_child) = n.get_first_child(ast) {
                    let child = self.create_type_from_comment_node_with_location(
                        ast,
                        first_child,
                        source_name,
                        scope,
                    );
                    self.create_nullable_type(ast, child)
                } else {
                    self.get_native_type(JSTypeNative::UNKNOWN_TYPE)
                }
            }
            Token::EQUALS => {
                let child = self.create_type_from_comment_node_with_location(
                    ast,
                    n.get_first_child(ast).unwrap(),
                    source_name,
                    scope,
                );
                self.create_optional_type(ast, child)
            }
            Token::ITER_REST => self.create_type_from_comment_node_with_location(
                ast,
                n.get_first_child(ast).unwrap(),
                source_name,
                scope,
            ),
            Token::STAR => self.get_native_type(JSTypeNative::ALL_TYPE),
            Token::PIPE => {
                let mut builder = Vec::new();
                for child in n.children(ast) {
                    builder.push(self.create_type_from_comment_node_with_location(
                        ast,
                        child,
                        source_name,
                        scope.clone(),
                    ));
                }
                self.create_union_type(ast, &builder)
            }
            Token::EMPTY => self.get_native_type(JSTypeNative::UNKNOWN_TYPE),
            Token::VOID => self.get_native_type(JSTypeNative::VOID_TYPE),
            Token::TYPEOF => {
                let name = n.get_first_child(ast).unwrap().get_string(ast);
                let qname = QualifiedName::of(name.clone());
                let root = qname.get_root(ast);
                let scope = scope.expect("NullPointerException");
                let declaration_scope =
                    scope.get_topmost_scope_of_eventual_declaration(self, ast, &root);
                let root_slot = scope.get_slot_creating_implicit_vars(self, ast, &root);
                let type_ = scope.lookup_qualified_name(self, ast, &qname);
                if type_.is_none()
                    || type_.unwrap().is_unknown_type(self, ast)
                    || !match (
                        root_slot.expect("NullPointerException").get_scope(self),
                        declaration_scope,
                    ) {
                        (Some(a), Some(b)) => std::ptr::addr_eq(a, b),
                        (None, None) => true,
                        _ => false,
                    }
                {
                    return NamedTypeBuilder::new(self, JsString::from("typeof ").concat(&name))
                        .set_scope(Some(scope))
                        .set_resolution_kind(ResolutionKind::TYPEOF)
                        .set_error_reporting_location_from(ast, n)
                        .build(self, ast);
                }
                let mut type_ = type_.unwrap();
                if type_.is_literal_object(self) {
                    type_ = NamedTypeBuilder::new(self, JsString::from("typeof ").concat(&name))
                        .set_resolution_kind(ResolutionKind::NONE)
                        .set_referenced_type(type_)
                        .build(self, ast);
                }
                type_
            }
            Token::STRINGLIT => {
                let mut nominal_type = self.get_type_with_location(
                    ast,
                    scope.clone(),
                    n.get_string(ast),
                    source_name,
                    n.get_lineno(ast),
                    n.get_charno(ast),
                );
                let template_args =
                    self.parse_template_args(ast, nominal_type, n, source_name, scope.clone());
                if nominal_type.is_named_type(self) && !nominal_type.is_resolved(self) {
                    if let Some(template_args) = template_args {
                        nominal_type = NamedType::to_builder(nominal_type, self)
                            .set_template_types(template_args)
                            .build(self, ast);
                    }
                    return self.add_nullability_based_on_parse_context(
                        ast,
                        n,
                        nominal_type,
                        scope.as_deref(),
                    );
                }
                let Some(object_type) = nominal_type.to_maybe_object_type(self) else {
                    return nominal_type;
                };
                if template_args.is_none() || !nominal_type.is_raw_type_of_templatized_type(self) {
                    return self.add_nullability_based_on_parse_context(
                        ast,
                        n,
                        nominal_type,
                        scope.as_deref(),
                    );
                }
                let type_ = self.create_templatized_type(ast, object_type, &template_args.unwrap());
                self.add_nullability_based_on_parse_context(ast, n, type_, scope.as_deref())
            }
            Token::FUNCTION => {
                let mut this_type = None;
                let mut is_constructor = false;
                let mut current = n.get_first_child(ast).unwrap();
                if current.is_this(ast) || current.is_new(ast) {
                    let context_node = current.get_first_child(ast).unwrap();
                    let candidate_this_type = self.create_type_from_comment_node_with_location(
                        ast,
                        context_node,
                        source_name,
                        scope.clone(),
                    );
                    if candidate_this_type.is_null_type(self)
                        || candidate_this_type.is_void_type(self)
                    {
                        this_type = Some(candidate_this_type);
                    } else if current.is_this(ast) {
                        this_type =
                            Some(candidate_this_type.restrict_by_not_null_or_undefined(self, ast));
                    } else if current.is_new(ast) {
                        this_type = candidate_this_type
                            .restrict_by_not_null_or_undefined(self, ast)
                            .to_maybe_object_type(self);
                        if this_type.is_none() {
                            self.reporter.warning(
                                Msg::JSDOC_FUNCTION_NEWNOTOBJECT.format(),
                                source_name,
                                context_node.get_lineno(ast),
                                context_node.get_charno(ast),
                            );
                        }
                    }
                    is_constructor = current.get_token(ast) == Token::NEW;
                    current = current.get_next(ast).unwrap();
                }
                let mut param_builder = FunctionParamBuilder::new();
                if current.get_token(ast) == Token::PARAM_LIST {
                    for arg in current.children(ast) {
                        if arg.get_token(ast) == Token::ITER_REST {
                            let type_ = if let Some(child) = arg.get_first_child(ast) {
                                self.create_type_from_comment_node_with_location(
                                    ast,
                                    child,
                                    source_name,
                                    scope.clone(),
                                )
                            } else {
                                self.get_native_type(JSTypeNative::UNKNOWN_TYPE)
                            };
                            param_builder.add_var_args(type_);
                        } else {
                            let type_ = self.create_type_from_comment_node_with_location(
                                ast,
                                arg,
                                source_name,
                                scope.clone(),
                            );
                            if arg.get_token(ast) == Token::EQUALS {
                                let add_success =
                                    param_builder.add_optional_params(self, ast, &[type_]);
                                if !add_success {
                                    self.reporter.warning(
                                        Msg::JSDOC_FUNCTION_VARARGS.format(),
                                        source_name,
                                        arg.get_lineno(ast),
                                        arg.get_charno(ast),
                                    );
                                }
                            } else {
                                param_builder.add_required_params(&[type_]);
                            }
                        }
                    }
                    current = current.get_next(ast).unwrap();
                }
                let return_type = self.create_type_from_comment_node_with_location(
                    ast,
                    current,
                    source_name,
                    scope,
                );
                FunctionTypeBuilder::new()
                    .with_parameters(param_builder.build())
                    .with_return_type(return_type)
                    .with_type_of_this(this_type)
                    .with_kind(if is_constructor {
                        Kind::CONSTRUCTOR
                    } else {
                        Kind::ORDINARY
                    })
                    .build(self, ast)
            }
            _ => panic!("Unexpected node in type expression: {}", n.to_string(ast)),
        }
    }
    // port: JSTypeRegistry#addNullabilityBasedOnParseContext
    fn add_nullability_based_on_parse_context(
        &mut self,
        ast: &Ast,
        n: NodeId,
        type_: TypeId,
        scope: Option<&dyn StaticTypedScope>,
    ) -> TypeId {
        check_state!(n.is_name(ast) || n.is_string_lit(ast), &n.to_string(ast));
        if self.is_non_nullable_name(ast, scope, n.get_string(ast))
            || type_.is_template_type(self)
            || n.get_parent(ast)
                .is_some_and(|p| p.get_token(ast) == Token::BANG)
        {
            type_
        } else {
            self.create_nullable_type(ast, type_)
        }
    }
    // port: JSTypeRegistry#parseTemplateArgs
    fn parse_template_args(
        &mut self,
        ast: &Ast,
        nominal_type: TypeId,
        type_node: NodeId,
        source_name: &str,
        scope: Option<Arc<dyn StaticTypedScope>>,
    ) -> Option<Vec<TypeId>> {
        let type_list = type_node.get_first_child(ast)?;
        let mut template_args = Vec::new();
        for template_node in type_list.children(ast) {
            template_args.push(self.create_type_from_comment_node_with_location(
                ast,
                template_node,
                source_name,
                scope.clone(),
            ));
        }
        let name = type_node.get_string(ast);
        let is_object = name == "Object" || name == "window.Object";
        if is_object && template_args.len() == 1 {
            template_args.insert(0, self.get_native_type(JSTypeNative::UNKNOWN_TYPE));
        }
        if nominal_type.is_named_type(self) && !nominal_type.is_resolved(self) {
            return Some(template_args);
        }
        let required_template_arg_count = nominal_type.get_template_param_count(self);
        if template_args.len() <= required_template_arg_count {
            return Some(template_args);
        }
        if !nominal_type.is_unknown_type(self, ast)
            && !self.is_non_nullable_name(ast, scope.as_deref(), type_node.get_string(ast))
        {
            let first_extra_template_param = type_list
                .get_child_at_index(ast, required_template_arg_count as i32)
                .unwrap();
            let message = format!(
                "Too many template parameters\nFound {}, required at most {}",
                template_args.len(),
                required_template_arg_count
            );
            self.reporter.warning(
                &message,
                source_name,
                first_extra_template_param.get_lineno(ast),
                first_extra_template_param.get_charno(ast),
            );
        }
        template_args.truncate(required_template_arg_count);
        Some(template_args)
    }
    // port: JSTypeRegistry#createRecordTypeFromNodes
    fn create_record_type_from_nodes(
        &mut self,
        ast: &Ast,
        n: NodeId,
        source_name: &str,
        scope: Option<Arc<dyn StaticTypedScope>>,
    ) -> TypeId {
        let mut builder = RecordTypeBuilder::new();
        for field_type_node in n.children(ast) {
            let has_type = field_type_node.get_token(ast) == Token::COLON;
            let field_name_node = if has_type {
                field_type_node.get_first_child(ast).unwrap()
            } else {
                field_type_node
            };
            let mut field_name = field_name_node.get_string(ast);
            if field_name.starts_with("'") || field_name.starts_with("\"") {
                field_name = field_name.substring(1, field_name.length() - 1);
            }
            let field_type = if has_type {
                self.create_type_from_comment_node_with_location(
                    ast,
                    field_type_node.get_last_child(ast).unwrap(),
                    source_name,
                    scope.clone(),
                )
            } else {
                self.get_native_type(JSTypeNative::UNKNOWN_TYPE)
            };
            builder.add_property(field_name, field_type, Some(field_name_node));
        }
        builder.build(self, ast)
    }
}

impl JSTypeRegistry {
    // port: JSTypeRegistry#registerTemplateTypeNamesInScope
    pub fn register_template_type_names_in_scope(
        &mut self,
        keys: impl IntoIterator<Item = TypeId>,
        scope_root: NodeId,
    ) {
        for key in keys {
            self.scoped_name_table
                .insert((scope_root, key.get_reference_name(self).unwrap()), key);
        }
    }
    // port: JSTypeRegistry#createScopeWithTemplates
    pub fn create_scope_with_templates(
        &self,
        scope: Arc<dyn StaticTypedScope>,
        templates: impl IntoIterator<Item = TypeId>,
    ) -> Arc<dyn StaticTypedScope> {
        Arc::new(SyntheticTemplateScope::new(self, scope, templates))
    }
    // port: JSTypeRegistry#getJSTypeOrUnknown
    fn get_js_type_or_unknown(&self, ast: &Ast, n: NodeId) -> TypeId {
        n.get_jstype(ast)
            .unwrap_or_else(|| self.get_native_type(JSTypeNative::UNKNOWN_TYPE))
    }
    // port: JSTypeRegistry#isReadableObjectType
    fn is_readable_object_type(&self, type_: Option<TypeId>) -> bool {
        let Some(type_) = type_ else {
            return false;
        };
        if type_.is_instance_type(self) || type_.is_function_prototype_type(self) {
            true
        } else if type_.is_enum_element_type(self) {
            type_.get_constructor(self).is_some()
        } else {
            false
        }
    }
    // port: JSTypeRegistry#getSimpleReadableJSTypeName
    fn get_simple_readable_js_type_name(&mut self, ast: &Ast, type_: TypeId) -> Option<String> {
        match &self.data(type_).kind {
            JSTypeKind::All(_)
            | JSTypeKind::BigInt(_)
            | JSTypeKind::Boolean(_)
            | JSTypeKind::Null(_)
            | JSTypeKind::Number(_)
            | JSTypeKind::String(_)
            | JSTypeKind::Symbol(_)
            | JSTypeKind::KnownSymbol(_)
            | JSTypeKind::Void(_) => return Some(type_.to_string(self, ast)),
            _ => {}
        }
        if type_.is_function_prototype_type(self) {
            return Some(type_.to_string(self, ast));
        }
        if let Some(object) = type_.to_maybe_object_type(self) {
            if !self.is_readable_object_type(Some(object)) {
                return None;
            }
            if let Some(source) =
                FunctionType::get_source(object.get_constructor(self).unwrap(), self)
            {
                check_state!(
                    source.is_function(ast) || source.is_class(ast),
                    &source.to_string(ast)
                );
                if let Some(readable) = source.get_first_child(ast).unwrap().get_original_name(ast)
                {
                    return Some(readable.to_string_lossy());
                }
            }
            return Some(type_.to_string(self, ast));
        }
        if let Some(union) = type_.to_maybe_union_type(self) {
            let mut names = Vec::new();
            for alternate in union.get_alternates(self, ast).iter().copied() {
                names.push(self.get_simple_readable_js_type_name(ast, alternate)?);
            }
            return Some(format!("({})", names.join("|")));
        }
        None
    }
    // port: JSTypeRegistry#getReadableTypeName
    pub fn get_readable_type_name(&mut self, ast: &Ast, n: NodeId) -> String {
        self.get_readable_js_type_name(ast, n, true)
    }
    // port: JSTypeRegistry#getReadableTypeNameNoDeref
    pub fn get_readable_type_name_no_deref(&mut self, ast: &Ast, n: NodeId) -> String {
        self.get_readable_js_type_name(ast, n, false)
    }
    // port: JSTypeRegistry#getReadableJSTypeName
    pub fn get_readable_js_type_name(&mut self, ast: &Ast, n: NodeId, dereference: bool) -> String {
        let mut type_ = self.get_js_type_or_unknown(ast, n);
        if dereference {
            let autoboxed = type_.autobox(self, ast);
            if !autoboxed.is_no_type(self) {
                type_ = autoboxed;
            }
        }
        if let Some(name) = self.get_simple_readable_js_type_name(ast, type_) {
            return name;
        }
        if n.is_get_prop(ast) {
            let lhs = self.get_js_type_or_unknown(ast, n.get_first_child(ast).unwrap());
            if let Some(object_type) = lhs.dereference(self, ast) {
                let prop_name = n.get_string(ast);
                let defining = object_type.get_closest_defining_type(self, ast, prop_name.clone());
                if self.is_readable_object_type(defining) {
                    return format!("{}.{}", defining.unwrap().to_string(self, ast), prop_name);
                }
            }
        }
        if n.is_qualified_name(ast) {
            n.get_qualified_name(ast).unwrap().to_string_lossy()
        } else if type_.is_function_type(self) {
            "function".into()
        } else {
            type_.to_string(self, ast)
        }
    }
}

struct SyntheticTemplateScope {
    delegate: Arc<dyn StaticTypedScope>,
    types: IndexMap<JsString, TypeId>,
}
impl SyntheticTemplateScope {
    // port: JSTypeRegistry.SyntheticTemplateScope#SyntheticTemplateScope
    fn new(
        reg: &JSTypeRegistry,
        delegate: Arc<dyn StaticTypedScope>,
        templates: impl IntoIterator<Item = TypeId>,
    ) -> Self {
        let mut types = IndexMap::new();
        for key in templates {
            types.insert(key.get_reference_name(reg).unwrap(), key);
        }
        Self { delegate, types }
    }
}
impl StaticTypedScope for SyntheticTemplateScope {
    // port: JSTypeRegistry.SyntheticTemplateScope#getRootNode
    fn get_root_node(&self) -> Option<NodeId> {
        self.delegate.get_root_node()
    }
    // port: JSTypeRegistry.SyntheticTemplateScope#getParentScope
    fn get_parent_scope(&self) -> Option<&dyn StaticTypedScope> {
        self.delegate.get_parent_scope()
    }
    // port: JSTypeRegistry.SyntheticTemplateScope#getSlot
    fn get_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.delegate.get_slot(name)
    }
    // port: JSTypeRegistry.SyntheticTemplateScope#getOwnSlot
    fn get_own_slot(&self, name: &JsString) -> Option<&dyn StaticTypedSlot> {
        self.delegate.get_own_slot(name)
    }
    // port: JSTypeRegistry.SyntheticTemplateScope#getSlot
    /// Rust-only twin of `get_slot` (see `StaticTypedScope::get_slot_creating_implicit_vars`).
    fn get_slot_creating_implicit_vars(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedSlot> {
        self.delegate
            .get_slot_creating_implicit_vars(reg, ast, name)
    }
    // port: JSTypeRegistry.SyntheticTemplateScope#getOwnSlot
    /// Rust-only twin of `get_own_slot` (see `StaticTypedScope::get_own_slot_creating_implicit_vars`).
    fn get_own_slot_creating_implicit_vars(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedSlot> {
        self.delegate
            .get_own_slot_creating_implicit_vars(reg, ast, name)
    }
    // port: JSTypeRegistry.SyntheticTemplateScope#getTypeOfThis
    fn get_type_of_this(&self, reg: &JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.delegate.get_type_of_this(reg, ast)
    }
    // port: JSTypeRegistry.SyntheticTemplateScope#getTemplateType
    fn get_template_type(&self, name: &JsString) -> Option<TypeId> {
        self.types
            .get(name)
            .copied()
            .or_else(|| self.delegate.get_template_type(name))
    }
    // port: JSTypeRegistry.SyntheticTemplateScope#getTopmostScopeOfEventualDeclaration
    fn get_topmost_scope_of_eventual_declaration(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &JsString,
    ) -> Option<&dyn StaticTypedScope> {
        if self.get_template_type(name).is_some() {
            Some(self)
        } else {
            self.delegate
                .get_topmost_scope_of_eventual_declaration(reg, ast, name)
        }
    }
}
impl JSTypeRegistry {
    // port: JSTypeRegistry#isObjectLiteralThatCanBeSkipped
    fn is_object_literal_that_can_be_skipped(&mut self, ast: &Ast, mut t: TypeId) -> bool {
        t = t.restrict_by_not_null_or_undefined(self, ast);
        t.is_record_type(self) || t.is_literal_object(self)
    }
    // port: JSTypeRegistry#registerDroppedPropertiesInUnion
    pub(crate) fn register_dropped_properties_in_union(
        &mut self,
        ast: &Ast,
        subtype: TypeId,
        supertype: TypeId,
    ) {
        let mut found_dropped_property = false;
        for pname in subtype.get_property_map(self).get_own_property_names() {
            if !supertype.has_property(self, ast, pname.clone()) {
                found_dropped_property = true;
                self.dropped_properties_of_unions.insert(pname);
            }
        }
        if found_dropped_property {
            self.properties_of_supertypes_in_unions
                .extend(supertype.get_property_map(self).get_own_property_names());
        }
    }
    // port: JSTypeRegistry#registerPropertyOnType
    pub fn register_property_on_type(
        &mut self,
        ast: &Ast,
        property_name: impl Into<JsString>,
        mut type_: TypeId,
    ) {
        let property_name = property_name.into();
        if let Some(union) = type_.to_maybe_union_type(self) {
            for alternate in union.get_alternates(self, ast).iter().copied() {
                self.register_property_on_type(ast, property_name.clone(), alternate);
            }
            return;
        }
        if self.is_object_literal_that_can_be_skipped(ast, type_) {
            type_ = self.get_sentinel_object_literal(ast);
        }
        let is_ref = type_.to_maybe_object_type(self).is_some() && type_.has_reference_name(self);
        let values = if is_ref {
            self.each_ref_type_indexed_by_property.get(&property_name)
        } else {
            self.non_ref_types_indexed_by_property.get(&property_name)
        }
        .cloned()
        .unwrap_or_default();
        let hash = type_.hash_code(self);
        let present = values.iter().any(|(stored_hash, t)| {
            *stored_hash == hash && (*t == type_ || type_.equals(self, ast, *t))
        });
        if !present {
            if is_ref {
                self.each_ref_type_indexed_by_property
                    .entry(property_name)
                    .or_default()
                    .push((hash, type_));
            } else {
                self.non_ref_types_indexed_by_property
                    .entry(property_name)
                    .or_default()
                    .push((hash, type_));
            }
        }
    }
    // port: JSTypeRegistry#canPropertyBeDefined
    pub fn can_property_be_defined(
        &mut self,
        ast: &Ast,
        type_: TypeId,
        property_name: impl Into<JsString>,
    ) -> PropDefinitionKind {
        let property_name = property_name.into();
        if type_.is_struct(self, ast) {
            return match type_.get_property_kind(self, ast, property_name) {
                HasPropertyKind::KNOWN_PRESENT => PropDefinitionKind::KNOWN,
                HasPropertyKind::MAYBE_PRESENT => PropDefinitionKind::LOOSE_UNION,
                HasPropertyKind::ABSENT => PropDefinitionKind::UNKNOWN,
            };
        }
        if !type_.is_empty_type(self) && !type_.is_unknown_type(self, ast) {
            match type_.get_property_kind(self, ast, property_name.clone()) {
                HasPropertyKind::KNOWN_PRESENT => return PropDefinitionKind::KNOWN,
                HasPropertyKind::MAYBE_PRESENT => return PropDefinitionKind::LOOSE_UNION,
                HasPropertyKind::ABSENT => {}
            }
        }
        let mut associated_types = self
            .non_ref_types_indexed_by_property
            .get(&property_name)
            .cloned()
            .unwrap_or_default();
        associated_types.extend(
            self.each_ref_type_indexed_by_property
                .get(&property_name)
                .cloned()
                .unwrap_or_default(),
        );
        for (_, alternative) in associated_types {
            let greatest_subtype = alternative.get_greatest_subtype(self, ast, type_);
            if !greatest_subtype.is_empty_type(self) {
                if greatest_subtype.to_maybe_record_type(self).is_some()
                    && greatest_subtype.is_synthetic(self)
                {
                    continue;
                }
                return PropDefinitionKind::LOOSE;
            }
        }
        if let Some(rec) = type_.to_maybe_record_type(self) {
            let may_be_in_union = rec
                .get_property_map(self)
                .get_own_property_names()
                .iter()
                .any(|pname| self.properties_of_supertypes_in_unions.contains(pname));
            if may_be_in_union && self.dropped_properties_of_unions.contains(&property_name) {
                return PropDefinitionKind::LOOSE;
            }
        }
        PropDefinitionKind::UNKNOWN
    }
    // port: JSTypeRegistry#getEachReferenceTypeWithProperty
    pub fn get_each_reference_type_with_property(
        &self,
        property_name: impl Into<JsString>,
    ) -> Vec<TypeId> {
        self.each_ref_type_indexed_by_property
            .get(&property_name.into())
            .map(|entries| entries.iter().map(|(_, type_)| *type_).collect())
            .unwrap_or_default()
    }
    // port: JSTypeRegistry#findCommonSuperObject
    pub fn find_common_super_object(&mut self, ast: &Ast, a: TypeId, b: TypeId) -> TypeId {
        let mut stack_a = self.get_super_stack(ast, a);
        let mut stack_b = self.get_super_stack(ast, b);
        let mut result = self.get_native_object_type(JSTypeNative::OBJECT_TYPE);
        while !stack_a.is_empty() && !stack_b.is_empty() {
            let current_a = stack_a.pop().unwrap();
            let current_b = stack_b.pop().unwrap();
            if current_a.equals(self, ast, current_b) {
                result = current_a;
            } else {
                break;
            }
        }
        result
    }
    // port: JSTypeRegistry#getSuperStack
    fn get_super_stack(&mut self, ast: &Ast, a: TypeId) -> Vec<TypeId> {
        let mut stack = Vec::with_capacity(5);
        let mut current = Some(a);
        while let Some(type_) = current {
            stack.push(type_);
            current = type_.get_implicit_prototype(self, ast);
        }
        stack
    }
}
impl JSTypeRegistry {
    pub(crate) fn prototype_data_opt(
        &self,
        id: TypeId,
    ) -> Option<&crate::prototype_object_type::PrototypeObjectTypeData> {
        crate::js_type::prototype_data_opt(id, self)
    }
    // port: JSTypeRegistry#nativeConstructorBuilder
    fn native_constructor_builder(&self, name: impl Into<JsString>) -> FunctionTypeBuilder {
        FunctionTypeBuilder::new()
            .for_native_type()
            .for_constructor()
            .with_name(name)
    }
    // port: JSTypeRegistry#nativeInterface
    fn native_interface(
        &mut self,
        ast: &Ast,
        name: impl Into<JsString>,
        template_keys: &[TypeId],
    ) -> TypeId {
        let mut builder = FunctionTypeBuilder::new()
            .for_native_type()
            .for_interface()
            .with_name(name);
        if !template_keys.is_empty() {
            builder = builder.with_template_keys(self, ast, template_keys.to_vec());
        }
        builder.build(self, ast)
    }
    // port: JSTypeRegistry#nativeRecord
    fn native_record(
        &mut self,
        ast: &Ast,
        name: impl Into<JsString>,
        template_keys: &[TypeId],
    ) -> TypeId {
        let type_ = self.native_interface(ast, name, template_keys);
        type_.set_implicit_match(self, true);
        type_
    }
    pub(crate) fn object_data(&self, id: TypeId) -> &ObjectTypeData {
        crate::js_type::object_data(id, self)
    }
    pub(crate) fn object_data_mut(&mut self, id: TypeId) -> &mut ObjectTypeData {
        crate::js_type::object_data_mut(id, self)
    }
    pub(crate) fn prototype_data(
        &self,
        id: TypeId,
    ) -> &crate::prototype_object_type::PrototypeObjectTypeData {
        crate::js_type::prototype_data(id, self)
    }
    pub(crate) fn prototype_data_mut(
        &mut self,
        id: TypeId,
    ) -> &mut crate::prototype_object_type::PrototypeObjectTypeData {
        crate::js_type::prototype_data_mut(id, self)
    }
    pub(crate) fn function_data(&self, id: TypeId) -> &crate::function_type::FunctionTypeData {
        crate::js_type::function_data(id, self)
    }
    pub(crate) fn function_data_mut(
        &mut self,
        id: TypeId,
    ) -> &mut crate::function_type::FunctionTypeData {
        crate::js_type::function_data_mut(id, self)
    }
    // port: JSTypeRegistry#JSTypeRegistry
    pub fn new(
        ast: &mut Ast,
        reporter: Box<dyn ErrorReporter + Send>,
        forward_declared_types: impl IntoIterator<Item = JsString>,
    ) -> Self {
        let forward_declared_types: IndexSet<JsString> =
            forward_declared_types.into_iter().collect();
        Self::new_with_shared_forward_declared_types(
            ast,
            reporter,
            Arc::new(forward_declared_types),
        )
    }
    // port: JSTypeRegistry#JSTypeRegistry
    /// Java's constructor keeps the caller's `Set<String>` itself, so names the caller adds after
    /// construction (`Compiler#forwardDeclareType`) are forward declared too.
    pub fn new_with_shared_forward_declared_types(
        ast: &mut Ast,
        reporter: Box<dyn ErrorReporter + Send>,
        forward_declared_types: Arc<dyn ForwardDeclaredTypeSet>,
    ) -> Self {
        let mut registry = Self {
            types: Vec::new(),
            properties: Vec::new(),
            resolver: JSTypeResolver::new(),
            empty_template_type_map: None,
            reporter: SharedErrorReporter::new(reporter),
            native_types: vec![None; JSTypeNative::VALUES.len()],
            scoped_name_table: IndexMap::new(),
            closure_namespaces: IndexMap::new(),
            name_table_global_root: ast.new_node(Token::ROOT),
            non_nullable_type_names: IndexMap::new(),
            forward_declared_types,
            non_ref_types_indexed_by_property: IndexMap::new(),
            sentinel_object_literal: None,
            properties_of_supertypes_in_unions: IndexSet::new(),
            dropped_properties_of_unions: IndexSet::new(),
            each_ref_type_indexed_by_property: IndexMap::new(),
            i_object_index_template_key: None,
            i_object_element_template_key: None,
            iterable_value_template: None,
            iterable_return_template: None,
            iterator_iterable_value_template: None,
            iterator_iterable_return_template: None,
            iterator_like_value_template: None,
            iterator_like_return_template: None,
            iterator_value_template: None,
            iterator_return_template: None,
            iiterable_result_value_template: None,
            async_iterable_value_template: None,
            async_iterable_return_template: None,
            async_iterator_value_template: None,
            async_iterator_return_template: None,
            generator_value_template: None,
            generator_return_template: None,
            i_thenable_template_key: None,
            promise_template_key: None,
            array_element_template_key: None,
            readonly_array_element_template_key: None,
            readonly_map_key_template_key: None,
            readonly_map_value_template_key: None,
            map_key_template_key: None,
            map_value_template_key: None,
        };
        registry.empty_template_type_map = Some(TemplateTypeMap::create_empty(&registry));
        let mut closer = registry.resolver.open_for_definition();
        registry.initialize_built_in_types(ast);
        registry.initialize_registry(ast);
        closer.close(&mut registry, ast);
        registry
    }
    pub(crate) fn data(&self, id: TypeId) -> &JSTypeData {
        &self.types[id.0 as usize]
    }
    pub(crate) fn data_mut(&mut self, id: TypeId) -> &mut JSTypeData {
        &mut self.types[id.0 as usize]
    }
    pub(crate) fn property(&self, id: PropertyId) -> &PropertyData {
        &self.properties[id.0 as usize]
    }
    pub(crate) fn property_mut(&mut self, id: PropertyId) -> &mut PropertyData {
        &mut self.properties[id.0 as usize]
    }
    pub(crate) fn alloc_property(&mut self, data: PropertyData) -> PropertyId {
        let id = PropertyId(self.properties.len() as u32);
        self.properties.push(data);
        id
    }
    pub(crate) fn alloc(&mut self, kind: JSTypeKind, map: Option<Arc<TemplateTypeMap>>) -> TypeId {
        let id = TypeId(self.types.len() as u32);
        let map = map.unwrap_or_else(|| self.get_empty_template_type_map());
        self.types.push(JSTypeData::new(kind, map));
        check_state!(!id.is_resolved(self));
        self.resolver.add_unresolved(id);
        id
    }
    pub(crate) fn finish_construction(&mut self, id: TypeId, ast: &Ast) {
        JSTypeResolver::resolve_if_closed(self, ast, id, id.get_type_class(self));
    }
    // port: JSTypeRegistry#getErrorReporter
    pub fn get_error_reporter(&self) -> SharedErrorReporter {
        self.reporter.clone()
    }
    // port: JSTypeRegistry#getResolver
    pub fn get_resolver(&mut self) -> &mut JSTypeResolver {
        &mut self.resolver
    }
    // port: JSTypeRegistry#getNativeType
    pub fn get_native_type(&self, type_id: JSTypeNative) -> TypeId {
        self.native_types[type_id as usize].expect("NullPointerException")
    }
    // port: JSTypeRegistry#getNativeObjectType
    pub fn get_native_object_type(&self, type_id: JSTypeNative) -> TypeId {
        self.get_native_type(type_id)
            .to_maybe_object_type(self)
            .expect("ClassCastException")
    }
    // port: JSTypeRegistry#getNativeFunctionType
    pub fn get_native_function_type(&self, type_id: JSTypeNative) -> TypeId {
        self.get_native_type(type_id)
            .to_maybe_function_type(self)
            .expect("ClassCastException")
    }
    // port: JSTypeRegistry#registerNativeType
    fn register_native_type(&mut self, type_id: JSTypeNative, type_: TypeId) {
        self.native_types[type_id as usize] = Some(type_);
    }
    // port: JSTypeRegistry#getEmptyTemplateTypeMap
    pub fn get_empty_template_type_map(&self) -> Arc<TemplateTypeMap> {
        self.empty_template_type_map
            .clone()
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getSentinelObjectLiteral
    fn get_sentinel_object_literal(&mut self, ast: &Ast) -> TypeId {
        if self.sentinel_object_literal.is_none() {
            self.sentinel_object_literal = Some(self.create_anonymous_object_type(ast, None));
        }
        self.sentinel_object_literal.unwrap()
    }
    // port: JSTypeRegistry#isForwardDeclaredType
    pub fn is_forward_declared_type(&self, name: impl Into<JsString>) -> bool {
        self.forward_declared_types.contains(&name.into())
    }
    // port: JSTypeRegistry#checkTypeName
    fn check_type_name(type_name: &JsString) {
        check_argument!(
            type_name.index_of_char(b'<' as u16) == -1,
            "Type names cannot contain template annotations."
        );
    }
    // port: JSTypeRegistry#getRootElementOfName
    fn get_root_element_of_name(name: &JsString) -> JsString {
        let index = name.index_of_char(b'.' as u16);
        if index == -1 {
            name.clone()
        } else {
            name.substring(0, index as usize)
        }
    }
    // port: JSTypeRegistry#getLookupScope
    /// Rust-only parameters: the lookup may create an implicit var (see
    /// `StaticTypedScope::get_topmost_scope_of_eventual_declaration`).
    fn get_lookup_scope<'a>(
        &mut self,
        ast: &Ast,
        scope: Option<&'a dyn StaticTypedScope>,
        name: &JsString,
    ) -> Option<&'a dyn StaticTypedScope> {
        match scope {
            Some(s) if s.get_parent_scope().is_some() => s
                .get_topmost_scope_of_eventual_declaration(
                    self,
                    ast,
                    &Self::get_root_element_of_name(name),
                ),
            _ => scope,
        }
    }
    // port: JSTypeRegistry#getRootNodeForScope
    fn get_root_node_for_scope(&self, ast: &Ast, scope: Option<&dyn StaticTypedScope>) -> NodeId {
        match scope.and_then(|s| s.get_root_node()) {
            Some(root) if !root.is_root(ast) && !root.is_script(ast) => root,
            _ => self.name_table_global_root,
        }
    }
    // port: JSTypeRegistry#getTypeForScopeInternal
    fn get_type_for_scope_internal(
        &self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        name: &JsString,
    ) -> Option<TypeId> {
        self.scoped_name_table
            .get(&(self.get_root_node_for_scope(ast, scope), name.clone()))
            .copied()
    }
    // port: JSTypeRegistry#registerForScope
    fn register_for_scope(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        type_: TypeId,
        name: JsString,
    ) {
        self.scoped_name_table
            .insert((self.get_root_node_for_scope(ast, scope), name), type_);
    }
    // port: JSTypeRegistry#register
    fn register(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        type_: TypeId,
        name: JsString,
    ) {
        Self::check_type_name(&name);
        let lookup_scope = self.get_lookup_scope(ast, scope, &name);
        self.register_for_scope(ast, lookup_scope, type_, name);
    }
    // port: JSTypeRegistry#registerGlobalType
    fn register_global_type(&mut self, ast: &Ast, type_: TypeId) {
        let name = type_.to_string(self, ast).into();
        self.register(ast, None, type_, name);
    }
    // port: JSTypeRegistry#registerGlobalType(JSType,String)
    fn register_global_type_with_name(
        &mut self,
        ast: &Ast,
        type_: TypeId,
        name: impl Into<JsString>,
    ) {
        self.register(ast, None, type_, name.into());
    }
    // port: JSTypeRegistry#isDeclaredForScope
    fn is_declared_for_scope(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        name: &JsString,
    ) -> bool {
        self.get_type_internal(ast, scope, name).is_some()
    }
    // port: JSTypeRegistry#declareType
    pub fn declare_type(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        name: impl Into<JsString>,
        type_: TypeId,
    ) -> bool {
        let name = name.into();
        check_state!(!name.is_empty());
        let lookup_scope = self.get_lookup_scope(ast, scope, &name);
        if self
            .get_type_for_scope_internal(ast, lookup_scope, &name)
            .is_some()
        {
            return false;
        }
        self.register(ast, scope, type_, name);
        true
    }
    // port: JSTypeRegistry#declareTypeForExactScope
    pub fn declare_type_for_exact_scope(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        name: impl Into<JsString>,
        type_: TypeId,
    ) -> bool {
        let name = name.into();
        check_state!(!name.is_empty());
        if self
            .get_type_for_scope_internal(ast, scope, &name)
            .is_some()
        {
            return false;
        }
        self.register_for_scope(ast, scope, type_, name);
        true
    }
    // port: JSTypeRegistry#overwriteDeclaredType
    pub fn overwrite_declared_type(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        name: impl Into<JsString>,
        type_: TypeId,
    ) {
        let name = name.into();
        check_state!(
            self.is_declared_for_scope(ast, scope, &name),
            "missing name %s",
            name
        );
        self.register(ast, scope, type_, name);
    }
    // port: JSTypeRegistry#identifyNonNullableName
    pub fn identify_non_nullable_name(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        name: impl Into<JsString>,
    ) {
        let name = name.into();
        let scope = self.get_lookup_scope(ast, scope, &name);
        let root = self.get_root_node_for_scope(ast, scope);
        self.non_nullable_type_names
            .entry(root)
            .or_default()
            .insert(name);
    }
    // port: JSTypeRegistry#isNonNullableName
    pub fn is_non_nullable_name(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        name: impl Into<JsString>,
    ) -> bool {
        let name = name.into();
        let scope = self.get_lookup_scope(ast, scope, &name);
        self.non_nullable_type_names
            .get(&self.get_root_node_for_scope(ast, scope))
            .is_some_and(|names| names.contains(&name))
    }
    // port: JSTypeRegistry#getGlobalType
    pub fn get_global_type(
        &mut self,
        ast: &Ast,
        js_type_name: impl Into<JsString>,
    ) -> Option<TypeId> {
        self.get_type(ast, None, js_type_name)
    }
    // port: JSTypeRegistry#getType
    pub fn get_type(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        js_type_name: impl Into<JsString>,
    ) -> Option<TypeId> {
        self.get_type_internal(ast, scope, &js_type_name.into())
    }
    // port: JSTypeRegistry#getTypeInternal
    fn get_type_internal(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        name: &JsString,
    ) -> Option<TypeId> {
        Self::check_type_name(name);
        if let Some(type_) = scope.and_then(|s| s.get_template_type(name)) {
            return Some(type_);
        }
        let declaration_scope = self.get_lookup_scope(ast, scope, name);
        if let Some(type_) = self.get_type_for_scope_internal(ast, declaration_scope, name) {
            return Some(type_);
        }
        self.resolve_via_components(ast, declaration_scope.or(scope), name)
    }
    // port: JSTypeRegistry#resolveViaComponents
    fn resolve_via_components(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        qualified_name: &JsString,
    ) -> Option<TypeId> {
        let resolution_scope = scope?;
        if qualified_name.is_empty() {
            return None;
        }
        if !self.is_name_defined_locally(
            ast,
            resolution_scope,
            &Self::get_root_element_of_name(qualified_name),
        ) {
            if let Some(type_) = self.resolve_via_closure_namespace(ast, qualified_name.clone()) {
                return Some(type_);
            }
        }
        self.resolve_via_properties(ast, resolution_scope, qualified_name)
    }
    // port: JSTypeRegistry#isNameDefinedLocally
    /// Rust-only parameters: the lookup may create an implicit var (see
    /// `StaticTypedScope::get_slot_creating_implicit_vars`).
    fn is_name_defined_locally(
        &mut self,
        ast: &Ast,
        resolution_scope: &dyn StaticTypedScope,
        reference: &JsString,
    ) -> bool {
        resolution_scope
            .get_slot_creating_implicit_vars(self, ast, reference)
            .and_then(|s| s.get_scope(self))
            .is_some_and(|s| s.get_parent_scope().is_some())
    }
    // port: JSTypeRegistry#resolveViaClosureNamespace
    pub fn resolve_via_closure_namespace(
        &mut self,
        ast: &Ast,
        reference: impl Into<JsString>,
    ) -> Option<TypeId> {
        let mut prefix = reference.into();
        let mut unused_components = Vec::new();
        loop {
            if let Some(namespace) = self.closure_namespaces.get(&prefix).cloned() {
                if namespace.is_legacy() {
                    return None;
                }
                unused_components.reverse();
                return self.resolve_via_property_given_slot(
                    ast,
                    namespace.type_(),
                    namespace.definition_node(),
                    &unused_components,
                );
            }
            let last_dot = prefix.as_units().iter().rposition(|u| *u == b'.' as u16)?;
            unused_components.push(prefix.substring_from(last_dot + 1));
            prefix = prefix.substring(0, last_dot);
        }
    }
    // port: JSTypeRegistry#resolveViaProperties
    fn resolve_via_properties(
        &mut self,
        ast: &Ast,
        declaration_scope: &dyn StaticTypedScope,
        qualified_name: &JsString,
    ) -> Option<TypeId> {
        check_argument!(!qualified_name.is_empty());
        let slot = declaration_scope.get_own_slot_creating_implicit_vars(
            self,
            ast,
            &Self::get_root_element_of_name(qualified_name),
        )?;
        let components: Vec<JsString> = qualified_name
            .as_units()
            .split(|c| *c == b'.' as u16)
            .skip(1)
            .map(|s| JsString::from_units(s.to_vec()))
            .collect();
        self.resolve_via_property_given_slot(ast, slot.get_type(self), None, &components)
    }
    // port: JSTypeRegistry#resolveTypeFromNodeIfTypedef
    fn resolve_type_from_node_if_typedef(ast: &Ast, node: Option<NodeId>) -> Option<TypeId> {
        node.and_then(|n| n.get_typedef_type_prop(ast))
    }
    // port: JSTypeRegistry#resolveViaPropertyGivenSlot
    fn resolve_via_property_given_slot(
        &mut self,
        ast: &Ast,
        mut slot_type: Option<TypeId>,
        definition_node: Option<NodeId>,
        component_names: &[JsString],
    ) -> Option<TypeId> {
        if component_names.is_empty() {
            if let Some(t) = Self::resolve_type_from_node_if_typedef(ast, definition_node) {
                return Some(t);
            }
        }
        let t = slot_type?;
        if t.is_all_type(self) || t.is_no_type(self) {
            return None;
        }
        for (i, component) in component_names.iter().enumerate() {
            let parent_obj = slot_type?.to_maybe_object_type(self)?;
            if component.is_empty() || !parent_obj.has_own_property(self, ast, component.clone()) {
                return None;
            }
            if i == component_names.len() - 1 {
                if let Some(t) = Self::resolve_type_from_node_if_typedef(
                    ast,
                    parent_obj.get_property_def_site(self, ast, component.clone()),
                ) {
                    return Some(t);
                }
            }
            slot_type = Some(parent_obj.get_property_type(self, ast, component.clone()));
        }
        let t = slot_type?;
        if t.is_function_type(self) && (t.is_constructor(self) || t.is_interface(self)) {
            t.get_instance_type(self)
        } else if t.is_no_object_type(self) {
            Some(self.get_native_object_type(JSTypeNative::NO_OBJECT_TYPE))
        } else if t.is_enum_type(self) {
            Some(t.get_elements_type(self))
        } else {
            None
        }
    }
    // port: JSTypeRegistry#registerNonLegacyClosureNamespace
    pub fn register_non_legacy_closure_namespace(
        &mut self,
        module_name: impl Into<JsString>,
        definition_node: impl Into<Option<NodeId>>,
        type_: impl Into<Option<TypeId>>,
    ) {
        self.closure_namespaces.insert(
            module_name.into(),
            ClosureNamespace::create(false, definition_node.into(), type_.into()),
        );
    }
    // port: JSTypeRegistry#registerLegacyClosureNamespace
    pub fn register_legacy_closure_namespace(&mut self, module_name: impl Into<JsString>) {
        self.closure_namespaces.insert(
            module_name.into(),
            ClosureNamespace::create(true, None, None),
        );
    }
}

impl JSTypeRegistry {
    // port: JSTypeRegistry#createOptionalType
    pub fn create_optional_type(&mut self, ast: &Ast, type_: TypeId) -> TypeId {
        if matches!(self.data(type_).kind, JSTypeKind::Unknown(_)) || type_.is_all_type(self) {
            type_
        } else {
            self.create_union_type(ast, &[type_, self.get_native_type(JSTypeNative::VOID_TYPE)])
        }
    }
    // port: JSTypeRegistry#createNullableType
    pub fn create_nullable_type(&mut self, ast: &Ast, type_: TypeId) -> TypeId {
        self.create_union_type(ast, &[type_, self.get_native_type(JSTypeNative::NULL_TYPE)])
    }
    // port: JSTypeRegistry#createOptionalNullableType
    pub fn create_optional_nullable_type(&mut self, ast: &Ast, type_: TypeId) -> TypeId {
        self.create_union_type(
            ast,
            &[
                type_,
                self.get_native_type(JSTypeNative::VOID_TYPE),
                self.get_native_type(JSTypeNative::NULL_TYPE),
            ],
        )
    }
    // port: JSTypeRegistry#createUnionType
    pub fn create_union_type(&mut self, ast: &Ast, variants: &[TypeId]) -> TypeId {
        let mut builder = UnionTypeBuilder::new();
        builder.add_alternates(self, ast, variants);
        builder.build(self, ast)
    }
    // port: JSTypeRegistry#createUnionType(JSTypeNative...)
    pub fn create_union_type_from_native(
        &mut self,
        ast: &Ast,
        variants: &[JSTypeNative],
    ) -> TypeId {
        let mut builder = UnionTypeBuilder::new();
        for type_ in variants {
            builder.add_alternate(self, ast, self.get_native_type(*type_));
        }
        builder.build(self, ast)
    }
    // port: JSTypeRegistry#createFunctionType
    pub fn create_function_type(
        &mut self,
        ast: &Ast,
        return_type: TypeId,
        parameter_types: &[TypeId],
    ) -> TypeId {
        let parameters = self.create_parameters(parameter_types);
        self.create_function_type_with_parameters(ast, return_type, parameters)
    }
    // port: JSTypeRegistry#createFunctionType(JSType,List)
    pub fn create_function_type_with_parameters(
        &mut self,
        ast: &Ast,
        return_type: impl Into<Option<TypeId>>,
        parameters: impl Into<Option<Vec<Parameter>>>,
    ) -> TypeId {
        FunctionTypeBuilder::new()
            .with_parameters(parameters.into())
            .with_return_type(return_type.into())
            .build(self, ast)
    }
    // port: JSTypeRegistry#createFunctionTypeWithVarArgs
    pub fn create_function_type_with_var_args(
        &mut self,
        ast: &Ast,
        return_type: TypeId,
        parameter_types: &[TypeId],
    ) -> TypeId {
        let parameters = self.create_parameters_with_var_args(parameter_types);
        self.create_function_type_with_parameters(ast, return_type, parameters)
    }
    // port: JSTypeRegistry#createNativeFunctionTypeWithVarArgs
    fn create_native_function_type_with_var_args(
        &mut self,
        ast: &Ast,
        return_type: TypeId,
        parameter_types: &[TypeId],
    ) -> TypeId {
        let parameters = self.create_parameters_with_var_args(parameter_types);
        self.create_native_function_type(ast, return_type, parameters)
    }
    // port: JSTypeRegistry#createFunctionTypeWithInstanceType
    pub fn create_function_type_with_instance_type(
        &mut self,
        ast: &Ast,
        instance_type: TypeId,
        return_type: TypeId,
        parameter_types: &[TypeId],
    ) -> TypeId {
        let params_node = self.create_parameters(parameter_types);
        FunctionTypeBuilder::new()
            .with_parameters(params_node)
            .with_return_type(return_type)
            .with_type_of_this(instance_type)
            .build(self, ast)
    }
    // port: JSTypeRegistry#createParameters
    pub fn create_parameters(&self, parameter_types: &[TypeId]) -> Vec<Parameter> {
        self.create_parameters_with_last_var_args(false, parameter_types)
    }
    // port: JSTypeRegistry#createParameters(boolean,JSType...)
    fn create_parameters_with_last_var_args(
        &self,
        last_var_args: bool,
        parameter_types: &[TypeId],
    ) -> Vec<Parameter> {
        let mut builder = FunctionParamBuilder::new();
        let max = parameter_types.len().wrapping_sub(1);
        for (i, parameter_type) in parameter_types.iter().enumerate() {
            if last_var_args && i == max {
                builder.add_var_args(*parameter_type);
            } else {
                builder.add_required_params(&[*parameter_type]);
            }
        }
        builder.build()
    }
    // port: JSTypeRegistry#createParametersWithVarArgs
    pub fn create_parameters_with_var_args(&self, parameter_types: &[TypeId]) -> Vec<Parameter> {
        self.create_parameters_with_last_var_args(true, parameter_types)
    }
    // port: JSTypeRegistry#createOptionalParameters
    pub fn create_optional_parameters(
        &mut self,
        ast: &Ast,
        parameter_types: &[TypeId],
    ) -> Vec<Parameter> {
        let mut builder = FunctionParamBuilder::new();
        builder.add_optional_params(self, ast, parameter_types);
        builder.build()
    }
    // port: JSTypeRegistry#createFunctionTypeWithNewReturnType
    pub fn create_function_type_with_new_return_type(
        &mut self,
        ast: &Ast,
        existing_function_type: TypeId,
        return_type: TypeId,
    ) -> TypeId {
        FunctionType::to_builder(existing_function_type, self)
            .with_return_type(return_type)
            .build(self, ast)
    }
    // port: JSTypeRegistry#createNativeFunctionType
    fn create_native_function_type(
        &mut self,
        ast: &Ast,
        return_type: TypeId,
        parameters: Vec<Parameter>,
    ) -> TypeId {
        FunctionTypeBuilder::new()
            .with_parameters(parameters)
            .with_return_type(return_type)
            .for_native_type()
            .build(self, ast)
    }
    // port: JSTypeRegistry#buildRecordTypeFromObject
    pub fn build_record_type_from_object(&mut self, ast: &Ast, obj_type: TypeId) -> TypeId {
        if let Some(rec_type) = obj_type.to_maybe_record_type(self) {
            return rec_type;
        }
        let prop_names = obj_type.get_own_property_names(self);
        if prop_names.is_empty() {
            return self.get_native_type(JSTypeNative::OBJECT_TYPE);
        }
        let mut props = Vec::new();
        for prop_name in prop_names {
            props.push((
                prop_name.clone(),
                obj_type.get_property_type(self, ast, prop_name),
            ));
        }
        self.create_record_type(ast, props)
    }
    // port: JSTypeRegistry#createRecordType
    pub fn create_record_type(
        &mut self,
        ast: &Ast,
        props: impl IntoIterator<Item = (JsString, TypeId)>,
    ) -> TypeId {
        let mut builder = RecordTypeBuilder::new();
        for (name, type_) in props {
            builder.add_property(name, type_, None);
        }
        builder.build(self, ast)
    }
    // port: JSTypeRegistry#createObjectType
    pub fn create_object_type(
        &mut self,
        ast: &Ast,
        name: impl Into<JsString>,
        implicit_prototype: Option<TypeId>,
    ) -> TypeId {
        PrototypeObjectTypeBuilder::new()
            .set_name(name)
            .set_implicit_prototype(implicit_prototype)
            .build(self, ast)
    }
    // port: JSTypeRegistry#createAnonymousObjectType
    pub fn create_anonymous_object_type(
        &mut self,
        ast: &Ast,
        info: Option<Arc<JSDocInfo>>,
    ) -> TypeId {
        let type_ = PrototypeObjectTypeBuilder::new()
            .set_anonymous(true)
            .build(self, ast);
        type_.set_pretty_print(self, true);
        type_.set_jsdoc_info(self, info);
        type_
    }
    // port: JSTypeRegistry#resetImplicitPrototype
    pub fn reset_implicit_prototype(&mut self, type_: TypeId, new_implicit_proto: Option<TypeId>) {
        if crate::js_type::prototype_data_opt(type_, self).is_some() {
            type_.clear_cached_values(self);
            type_.set_implicit_prototype(self, new_implicit_proto);
        }
    }
    // port: JSTypeRegistry#createConstructorType
    pub fn create_constructor_type(
        &mut self,
        ast: &Ast,
        name: Option<JsString>,
        source: Option<NodeId>,
        parameters: Option<Vec<Parameter>>,
        return_type: Option<TypeId>,
        template_keys: Option<Vec<TypeId>>,
        is_abstract: bool,
    ) -> TypeId {
        check_argument!(source.is_none_or(|s| s.is_function(ast) || s.is_class(ast)));
        FunctionTypeBuilder::new()
            .for_constructor()
            .with_name_option(name)
            .with_source_node(source)
            .with_parameters(parameters)
            .with_return_type(return_type)
            .with_template_keys(self, ast, template_keys.unwrap_or_default())
            .with_is_abstract(is_abstract)
            .build(self, ast)
    }
    // port: JSTypeRegistry#createTemplateType
    pub fn create_template_type(&mut self, ast: &Ast, name: impl Into<JsString>) -> TypeId {
        crate::template_type::create(self, ast, name, None, None)
    }
    // port: JSTypeRegistry#createTemplateType(String,JSType)
    pub fn create_template_type_with_bound(
        &mut self,
        ast: &Ast,
        name: impl Into<JsString>,
        bound: TypeId,
    ) -> TypeId {
        crate::template_type::create(self, ast, name, Some(bound), None)
    }
    // port: JSTypeRegistry#createTemplateTypeWithTransformation
    pub fn create_template_type_with_transformation(
        &mut self,
        ast: &Ast,
        name: impl Into<JsString>,
        expr: NodeId,
    ) -> TypeId {
        crate::template_type::create(self, ast, name, None, Some(expr))
    }
    // port: JSTypeRegistry#createTemplatizedType
    pub fn create_templatized_type(
        &mut self,
        ast: &Ast,
        base_type: TypeId,
        templatized_types: &[TypeId],
    ) -> TypeId {
        crate::templatized_type::create(self, ast, base_type, templatized_types)
    }
    // port: JSTypeRegistry#createTemplatizedType(ObjectType,Map)
    pub fn create_templatized_type_from_map(
        &mut self,
        ast: &Ast,
        base_type: TypeId,
        templatized_types: &[(TypeId, TypeId)],
    ) -> TypeId {
        let unknown_type = self.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        let mut values = Vec::new();
        for key in base_type.get_type_parameters(self) {
            let mut value = unknown_type;
            for (candidate, type_) in templatized_types {
                if candidate.hash_code(self) == key.hash_code(self)
                    && key.equals(self, ast, *candidate)
                {
                    value = *type_;
                    break;
                }
            }
            values.push(value);
        }
        self.create_templatized_type(ast, base_type, &values)
    }
    // port: JSTypeRegistry#createNamedType
    pub fn create_named_type(
        &mut self,
        ast: &Ast,
        scope: Option<Arc<dyn StaticTypedScope>>,
        reference: impl Into<JsString>,
        source_name: impl Into<String>,
        lineno: i32,
        charno: i32,
    ) -> TypeId {
        NamedTypeBuilder::new(self, reference)
            .set_scope(scope)
            .set_resolution_kind(ResolutionKind::TYPE_NAME)
            .set_error_reporting_location(source_name, lineno, charno)
            .build(self, ast)
    }
    // port: JSTypeRegistry#getType(StaticTypedScope,String,String,int,int)
    pub fn get_type_with_location(
        &mut self,
        ast: &Ast,
        scope: Option<Arc<dyn StaticTypedScope>>,
        js_type_name: impl Into<JsString>,
        source_name: impl Into<String>,
        lineno: i32,
        charno: i32,
    ) -> TypeId {
        let js_type_name = js_type_name.into();
        let primitive = match js_type_name.as_units() {
            [98, 111, 111, 108, 101, 97, 110] => Some(JSTypeNative::BOOLEAN_TYPE),
            [110, 117, 109, 98, 101, 114] => Some(JSTypeNative::NUMBER_TYPE),
            [98, 105, 103, 105, 110, 116] => Some(JSTypeNative::BIGINT_TYPE),
            [115, 116, 114, 105, 110, 103] => Some(JSTypeNative::STRING_TYPE),
            [117, 110, 100, 101, 102, 105, 110, 101, 100] | [118, 111, 105, 100] => {
                Some(JSTypeNative::VOID_TYPE)
            }
            _ => None,
        };
        if let Some(p) = primitive {
            return self.get_native_type(p);
        }
        if let Some(this_type) = scope
            .as_ref()
            .and_then(|s| s.get_type_of_this(self, ast))
            .and_then(|t| t.to_maybe_object_type(self))
        {
            if let Some(type_) = this_type
                .get_template_type_map(self)
                .get_last_template_type_key_by_name(self, &js_type_name)
            {
                check_state!(
                    type_.is_template_type(self),
                    "expected:%s",
                    type_.to_string(self, ast)
                );
                return type_;
            }
        }
        if let Some(type_) = self.get_type(ast, scope.as_deref(), js_type_name.clone()) {
            type_
        } else {
            self.create_named_type(ast, scope, js_type_name, source_name, lineno, charno)
        }
    }
    // port: JSTypeRegistry#maybeGetTemplateTypesOfBuiltin
    pub fn maybe_get_template_types_of_builtin(
        &mut self,
        ast: &Ast,
        scope: Option<&dyn StaticTypedScope>,
        fn_name: impl Into<JsString>,
    ) -> Option<Vec<TypeId>> {
        let obj_type = self
            .get_type(ast, scope, fn_name)?
            .to_maybe_object_type(self)?;
        if obj_type.is_native_object_type(self) {
            Some(obj_type.get_type_parameters(self))
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct ClosureNamespace {
    is_legacy: bool,
    definition_node: Option<NodeId>,
    type_: Option<TypeId>,
}
impl ClosureNamespace {
    // port: JSTypeRegistry.ClosureNamespace#isLegacy
    fn is_legacy(&self) -> bool {
        self.is_legacy
    }
    // port: JSTypeRegistry.ClosureNamespace#definitionNode
    fn definition_node(&self) -> Option<NodeId> {
        self.definition_node
    }
    // port: JSTypeRegistry.ClosureNamespace#type
    fn type_(&self) -> Option<TypeId> {
        self.type_
    }
    // port: JSTypeRegistry.ClosureNamespace#create
    fn create(is_legacy: bool, definition_node: Option<NodeId>, type_: Option<TypeId>) -> Self {
        Self {
            is_legacy,
            definition_node,
            type_,
        }
    }
}

impl JSTypeRegistry {
    // port: JSTypeRegistry#getArrayElementKey
    pub fn get_array_element_key(&self) -> TypeId {
        self.array_element_template_key
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getReadonlyArrayElementKey
    pub fn get_readonly_array_element_key(&self) -> TypeId {
        self.readonly_array_element_template_key
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getReadonlyMapKey
    pub fn get_readonly_map_key(&self) -> TypeId {
        self.readonly_map_key_template_key
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getReadonlyMapValue
    pub fn get_readonly_map_value(&self) -> TypeId {
        self.readonly_map_value_template_key
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getObjectElementKey
    pub fn get_object_element_key(&self) -> TypeId {
        self.i_object_element_template_key
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getObjectIndexKey
    pub fn get_object_index_key(&self) -> TypeId {
        self.i_object_index_template_key
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIterableValueTemplate
    pub fn get_iterable_value_template(&self) -> TypeId {
        self.iterable_value_template.expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIterableReturnTemplate
    pub fn get_iterable_return_template(&self) -> TypeId {
        self.iterable_return_template.expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIteratorIterableValueTemplate
    pub fn get_iterator_iterable_value_template(&self) -> TypeId {
        self.iterator_iterable_value_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIteratorIterableReturnTemplate
    pub fn get_iterator_iterable_return_template(&self) -> TypeId {
        self.iterator_iterable_return_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIIterableResultValueTemplate
    pub fn get_i_iterable_result_value_template(&self) -> TypeId {
        self.iiterable_result_value_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIteratorLikeValueTemplate
    pub fn get_iterator_like_value_template(&self) -> TypeId {
        self.iterator_like_value_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIteratorLikeReturnTemplate
    pub fn get_iterator_like_return_template(&self) -> TypeId {
        self.iterator_like_return_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIteratorValueTemplate
    pub fn get_iterator_value_template(&self) -> TypeId {
        self.iterator_value_template.expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIteratorReturnTemplate
    pub fn get_iterator_return_template(&self) -> TypeId {
        self.iterator_return_template.expect("NullPointerException")
    }
    // port: JSTypeRegistry#getGeneratorValueTemplate
    pub fn get_generator_value_template(&self) -> TypeId {
        self.generator_value_template.expect("NullPointerException")
    }
    // port: JSTypeRegistry#getGeneratorReturnTemplate
    pub fn get_generator_return_template(&self) -> TypeId {
        self.generator_return_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getAsyncIterableValueTemplate
    pub fn get_async_iterable_value_template(&self) -> TypeId {
        self.async_iterable_value_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getAsyncIterableReturnTemplate
    pub fn get_async_iterable_return_template(&self) -> TypeId {
        self.async_iterable_return_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getAsyncIteratorValueTemplate
    pub fn get_async_iterator_value_template(&self) -> TypeId {
        self.async_iterator_value_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getAsyncIteratorReturnTemplate
    pub fn get_async_iterator_return_template(&self) -> TypeId {
        self.async_iterator_return_template
            .expect("NullPointerException")
    }
    // port: JSTypeRegistry#getIThenableTemplate
    pub fn get_i_thenable_template(&self) -> TypeId {
        self.i_thenable_template_key.expect("NullPointerException")
    }
}

impl JSTypeRegistry {
    // port: JSTypeRegistry#initializeBuiltInTypes
    fn initialize_built_in_types(&mut self, ast: &Ast) {
        let builtin_1 = BooleanType::new(self, ast);
        let boolean_type = builtin_1;
        self.register_native_type(JSTypeNative::BOOLEAN_TYPE, boolean_type);
        let builtin_3 = NullType::new(self, ast);
        let null_type = builtin_3;
        self.register_native_type(JSTypeNative::NULL_TYPE, null_type);
        let builtin_5 = BigIntType::new(self, ast);
        let big_int_type = builtin_5;
        self.register_native_type(JSTypeNative::BIGINT_TYPE, big_int_type);
        let builtin_7 = NumberType::new(self, ast);
        let number_type = builtin_7;
        self.register_native_type(JSTypeNative::NUMBER_TYPE, number_type);
        let builtin_9 = StringType::new(self, ast);
        let string_type = builtin_9;
        self.register_native_type(JSTypeNative::STRING_TYPE, string_type);
        let builtin_11 = SymbolType::new(self, ast);
        let symbol_type = builtin_11;
        self.register_native_type(JSTypeNative::SYMBOL_TYPE, symbol_type);
        let builtin_13 = UnknownType::new(self, ast, false);
        let unknown_type = builtin_13;
        self.register_native_type(JSTypeNative::UNKNOWN_TYPE, unknown_type);
        let builtin_15 = UnknownType::new(self, ast, true);
        let checked_unknown_type = builtin_15;
        self.register_native_type(JSTypeNative::CHECKED_UNKNOWN_TYPE, checked_unknown_type);
        let builtin_17 = VoidType::new(self, ast);
        let void_type = builtin_17;
        self.register_native_type(JSTypeNative::VOID_TYPE, void_type);
        let builtin_19 = AllType::new(self, ast);
        let all_type = builtin_19;
        self.register_native_type(JSTypeNative::ALL_TYPE, all_type);
        let builtin_21 = self.create_template_type(ast, "IOBJECT_KEY");
        self.i_object_index_template_key = Some(builtin_21);
        let builtin_22 = self.create_template_type(ast, OBJECT_ELEMENT_TEMPLATE);
        self.i_object_element_template_key = Some(builtin_22);
        let builtin_23 = self.create_template_type(ast, "VALUE2");
        let i_array_like_template = builtin_23;
        let builtin_24 = self.create_template_type(ast, "T");
        self.array_element_template_key = Some(builtin_24);
        let builtin_25 = self.create_template_type(ast, "T");
        self.readonly_array_element_template_key = Some(builtin_25);
        let builtin_26 = self.create_template_type(ast, "K");
        self.readonly_map_key_template_key = Some(builtin_26);
        let builtin_27 = self.create_template_type(ast, "V");
        self.readonly_map_value_template_key = Some(builtin_27);
        let builtin_28 = self.create_template_type(ast, "K");
        self.map_key_template_key = Some(builtin_28);
        let builtin_29 = self.create_template_type(ast, "V");
        self.map_value_template_key = Some(builtin_29);
        let builtin_30 = self.create_template_type(ast, "TYPE");
        self.i_thenable_template_key = Some(builtin_30);
        let builtin_31 = self.create_template_type(ast, "TYPE");
        self.promise_template_key = Some(builtin_31);
        let builtin_32 = PrototypeObjectTypeBuilder::new().set_name("Function.prototype");
        let builtin_33 = builtin_32.set_native(true);
        let builtin_34 = builtin_33.build(self, ast);
        let function_prototype = builtin_34;
        self.register_native_type(JSTypeNative::FUNCTION_PROTOTYPE, function_prototype);
        let builtin_36 = PrototypeObjectTypeBuilder::new().set_name("?.prototype");
        let builtin_37 = builtin_36.set_native(true);
        let builtin_38 = builtin_37.build(self, ast);
        let function_instance_prototype = builtin_38;
        self.register_native_type(
            JSTypeNative::FUNCTION_INSTANCE_PROTOTYPE,
            function_instance_prototype,
        );
        let builtin_40 = self.create_parameters_with_var_args(&[unknown_type]);
        let builtin_41 = FunctionTypeBuilder::new().with_name("Function");
        let builtin_42 = builtin_41.for_constructor();
        let builtin_43 = builtin_42.for_native_type();
        let builtin_44 = builtin_43.with_parameters(builtin_40);
        let builtin_45 = builtin_44.with_type_of_this(unknown_type);
        let builtin_46 = builtin_45.with_return_type(unknown_type);
        let builtin_47 = builtin_46.build(self, ast);
        let function_type = builtin_47;
        function_type.set_prototype(self, ast, Some(function_instance_prototype), None);
        self.register_native_type(JSTypeNative::FUNCTION_TYPE, function_type);
        let builtin_50 = self.create_parameters_with_var_args(&[all_type]);
        let builtin_51 = FunctionTypeBuilder::new().with_name("Function");
        let builtin_52 = builtin_51.for_constructor();
        let builtin_53 = builtin_52.for_native_type();
        let builtin_54 = builtin_53.with_parameters(builtin_50);
        let builtin_55 = builtin_54.with_type_of_this(function_type);
        let builtin_56 = builtin_55.build(self, ast);
        let function_function_type = builtin_56;
        function_function_type.set_prototype(self, ast, Some(function_prototype), None);
        self.register_native_type(JSTypeNative::FUNCTION_FUNCTION_TYPE, function_function_type);
        let builtin_59 = PrototypeObjectTypeBuilder::new().set_name("Object.prototype");
        let builtin_60 = builtin_59.set_native(true);
        let builtin_61 = builtin_60.set_implicit_prototype(None);
        let builtin_62 = builtin_61.build(self, ast);
        let object_prototype = builtin_62;
        self.register_native_type(JSTypeNative::OBJECT_PROTOTYPE, object_prototype);
        let builtin_64 = self.create_optional_parameters(ast, &[all_type]);
        let builtin_65 = self.native_constructor_builder("Object");
        let builtin_66 = builtin_65.with_parameters(builtin_64);
        let builtin_67 = builtin_66.with_returns_own_instance_type();
        let builtin_68_template_keys = vec![
            self.i_object_index_template_key.unwrap(),
            self.i_object_element_template_key.unwrap(),
        ];
        let builtin_68 = builtin_67.with_template_keys(self, ast, builtin_68_template_keys);
        let builtin_69 = builtin_68.build(self, ast);
        let object_function_type = builtin_69;
        object_function_type.set_prototype(self, ast, Some(object_prototype), None);
        self.register_native_type(JSTypeNative::OBJECT_FUNCTION_TYPE, object_function_type);
        let builtin_72 = object_function_type.get_instance_type(self).unwrap();
        let object_type = builtin_72;
        self.register_native_type(JSTypeNative::OBJECT_TYPE, object_type);
        function_prototype.clear_cached_values(self);
        function_prototype.set_implicit_prototype(self, Some(object_type));
        function_instance_prototype.clear_cached_values(self);
        function_instance_prototype.set_implicit_prototype(self, Some(object_type));
        let builtin_78 = self.native_interface(
            ast,
            "IObject",
            &[
                self.i_object_index_template_key.unwrap(),
                self.i_object_element_template_key.unwrap(),
            ],
        );
        let i_object_function_type = builtin_78;
        self.register_native_type(JSTypeNative::I_OBJECT_FUNCTION_TYPE, i_object_function_type);
        let builtin_80 = i_object_function_type.get_instance_type(self).unwrap();
        let i_object_type = builtin_80;
        self.register_native_type(JSTypeNative::I_OBJECT_TYPE, i_object_type);
        let builtin_82 = NoType::new(self, ast);
        let no_type = builtin_82;
        self.register_native_type(JSTypeNative::NO_TYPE, no_type);
        let builtin_84 = NoObjectType::new(self, ast);
        let no_object_type = builtin_84;
        self.register_native_type(JSTypeNative::NO_OBJECT_TYPE, no_object_type);
        let builtin_86 = self.create_template_type(ast, "T");
        self.iterable_value_template = Some(builtin_86);
        let builtin_87 = self.create_template_type(ast, "TReturn");
        self.iterable_return_template = Some(builtin_87);
        let builtin_88 = self.create_template_type(ast, "TNext");
        let iterable_next_template = builtin_88;
        let builtin_89 = self.native_interface(
            ast,
            "Iterable",
            &[
                self.iterable_value_template.unwrap(),
                self.iterable_return_template.unwrap(),
                iterable_next_template,
            ],
        );
        let iterable_function_type = builtin_89;
        self.register_native_type(JSTypeNative::ITERABLE_FUNCTION_TYPE, iterable_function_type);
        let builtin_91 = iterable_function_type.get_instance_type(self).unwrap();
        let iterable_type = builtin_91;
        self.register_native_type(JSTypeNative::ITERABLE_TYPE, iterable_type);
        let builtin_93 = self.create_template_type(ast, "T");
        self.iterator_like_value_template = Some(builtin_93);
        let builtin_94 = self.create_template_type(ast, "TReturn");
        self.iterator_like_return_template = Some(builtin_94);
        let builtin_95 = self.create_template_type(ast, "TNext");
        let iterator_like_next_template = builtin_95;
        let builtin_96 = self.native_interface(
            ast,
            "IteratorLike",
            &[
                self.iterator_like_value_template.unwrap(),
                self.iterator_like_return_template.unwrap(),
                iterator_like_next_template,
            ],
        );
        let iterator_like_function_type = builtin_96;
        self.register_native_type(
            JSTypeNative::ITERATOR_LIKE_FUNCTION_TYPE,
            iterator_like_function_type,
        );
        let builtin_98 = iterator_like_function_type.get_instance_type(self).unwrap();
        let iterator_like_type = builtin_98;
        self.register_native_type(JSTypeNative::ITERATOR_LIKE_TYPE, iterator_like_type);
        let builtin_100 = self.create_template_type(ast, "T");
        self.iterator_value_template = Some(builtin_100);
        let builtin_101 = self.create_template_type(ast, "TReturn");
        self.iterator_return_template = Some(builtin_101);
        let builtin_102 = self.create_template_type(ast, "TNext");
        let iterator_next_template = builtin_102;
        let builtin_103 = self.create_template_type(ast, "T");
        self.iterator_iterable_value_template = Some(builtin_103);
        let builtin_104 = self.create_template_type(ast, "TReturn");
        self.iterator_iterable_return_template = Some(builtin_104);
        let builtin_105 = self.create_template_type(ast, "TNext");
        let iterator_iterable_next_template = builtin_105;
        let builtin_106 = self.native_interface(
            ast,
            "IteratorIterable",
            &[
                self.iterator_iterable_value_template.unwrap(),
                self.iterator_iterable_return_template.unwrap(),
                iterator_iterable_next_template,
            ],
        );
        let iterator_iterable_function_type = builtin_106;
        self.register_native_type(
            JSTypeNative::ITERATOR_ITERABLE_FUNCTION_TYPE,
            iterator_iterable_function_type,
        );
        let builtin_108 = self.create_templatized_type(
            ast,
            iterable_type,
            &[
                self.iterator_iterable_value_template.unwrap(),
                self.iterator_iterable_return_template.unwrap(),
                iterator_iterable_next_template,
            ],
        );
        let builtin_109 = self.create_templatized_type(
            ast,
            iterator_like_type,
            &[
                self.iterator_iterable_value_template.unwrap(),
                self.iterator_iterable_return_template.unwrap(),
                iterator_iterable_next_template,
            ],
        );
        iterator_iterable_function_type.set_extended_interfaces(
            self,
            ast,
            vec![builtin_108, builtin_109],
        );
        let builtin_111 = iterator_iterable_function_type
            .get_instance_type(self)
            .unwrap();
        let iterator_iterable_type = builtin_111;
        self.register_native_type(JSTypeNative::ITERATOR_ITERABLE_TYPE, iterator_iterable_type);
        let builtin_113 = self.native_constructor_builder("Iterator");
        let builtin_114 = builtin_113.with_parameters(vec![]);
        let builtin_115_template_keys = vec![
            self.iterator_value_template.unwrap(),
            self.iterator_return_template.unwrap(),
            iterator_next_template,
        ];
        let builtin_115 = builtin_114.with_template_keys(self, ast, builtin_115_template_keys);
        let builtin_116 = builtin_115.build(self, ast);
        let iterator_function_type = builtin_116;
        self.register_native_type(JSTypeNative::ITERATOR_FUNCTION_TYPE, iterator_function_type);
        iterator_function_type.get_prototype(self, ast);
        let builtin_119 = self.create_templatized_type(
            ast,
            iterator_iterable_type,
            &[
                self.iterator_value_template.unwrap(),
                self.iterator_return_template.unwrap(),
                iterator_next_template,
            ],
        );
        iterator_function_type.set_implemented_interfaces(self, ast, vec![builtin_119]);
        let builtin_121 = iterator_function_type.get_instance_type(self).unwrap();
        let iterator_type = builtin_121;
        self.register_native_type(JSTypeNative::ITERATOR_TYPE, iterator_type);
        let builtin_123 = self.create_template_type(ast, "TYield");
        self.iiterable_result_value_template = Some(builtin_123);
        let builtin_124 = self.native_record(
            ast,
            "IIterableResult",
            &[self.iiterable_result_value_template.unwrap()],
        );
        let iiterable_result_function_type = builtin_124;
        self.register_native_type(
            JSTypeNative::I_ITERABLE_RESULT_FUNCTION_TYPE,
            iiterable_result_function_type,
        );
        let builtin_126 = iiterable_result_function_type
            .get_instance_type(self)
            .unwrap();
        let iiterable_result_type = builtin_126;
        self.register_native_type(JSTypeNative::I_ITERABLE_RESULT_TYPE, iiterable_result_type);
        let builtin_128 = self.native_record(ast, "IArrayLike", &[i_array_like_template]);
        let i_array_like_function_type = builtin_128;
        let builtin_129 =
            self.create_templatized_type(ast, i_object_type, &[number_type, i_array_like_template]);
        i_array_like_function_type.set_extended_interfaces(self, ast, vec![builtin_129]);
        self.register_native_type(
            JSTypeNative::I_ARRAY_LIKE_FUNCTION_TYPE,
            i_array_like_function_type,
        );
        let builtin_132 = i_array_like_function_type.get_instance_type(self).unwrap();
        let i_array_like_type = builtin_132;
        self.register_native_type(JSTypeNative::I_ARRAY_LIKE_TYPE, i_array_like_type);
        let builtin_134 = self.native_record(
            ast,
            "ReadonlyArray",
            &[self.readonly_array_element_template_key.unwrap()],
        );
        let readonly_array_function_type = builtin_134;
        self.register_native_type(
            JSTypeNative::READONLY_ARRAY_FUNCTION_TYPE,
            readonly_array_function_type,
        );
        let builtin_136 = self.create_templatized_type(
            ast,
            i_array_like_type,
            &[self.readonly_array_element_template_key.unwrap()],
        );
        let builtin_137 = self.create_templatized_type(
            ast,
            iterable_type,
            &[self.readonly_array_element_template_key.unwrap()],
        );
        readonly_array_function_type.set_extended_interfaces(
            self,
            ast,
            vec![builtin_136, builtin_137],
        );
        let builtin_139 = readonly_array_function_type
            .get_instance_type(self)
            .unwrap();
        let readonly_array_type = builtin_139;
        self.register_native_type(JSTypeNative::READONLY_ARRAY_TYPE, readonly_array_type);
        let builtin_141 = self.create_parameters_with_var_args(&[all_type]);
        let builtin_142 = self.native_constructor_builder("Array");
        let builtin_143 = builtin_142.with_parameters(builtin_141);
        let builtin_144 = builtin_143.with_returns_own_instance_type();
        let builtin_145_template_keys = vec![self.array_element_template_key.unwrap()];
        let builtin_145 = builtin_144.with_template_keys(self, ast, builtin_145_template_keys);
        let builtin_146 = builtin_145.build(self, ast);
        let array_function_type = builtin_146;
        array_function_type.get_prototype(self, ast);
        let builtin_148 = self.create_templatized_type(
            ast,
            readonly_array_type,
            &[self.array_element_template_key.unwrap()],
        );
        array_function_type.set_implemented_interfaces(self, ast, vec![builtin_148]);
        self.register_native_type(JSTypeNative::ARRAY_FUNCTION_TYPE, array_function_type);
        let builtin_151 = array_function_type.get_instance_type(self).unwrap();
        let array_type = builtin_151;
        self.register_native_type(JSTypeNative::ARRAY_TYPE, array_type);
        let builtin_153 = self.create_templatized_type(ast, array_type, &[string_type]);
        let array_of_string = builtin_153;
        let builtin_154 = self.native_constructor_builder("ITemplateArray");
        let builtin_155 = builtin_154.with_parameters(vec![]);
        let builtin_156 = builtin_155.build(self, ast);
        let i_template_array_function_type = builtin_156;
        let builtin_157 = PrototypeObjectTypeBuilder::new().set_name("ITemplateArray.prototype");
        let builtin_158 = builtin_157.set_native(true);
        let builtin_159 = builtin_158.set_implicit_prototype(Some(array_of_string));
        let builtin_160 = builtin_159.build(self, ast);
        let i_template_array_prototype = builtin_160;
        i_template_array_function_type.set_prototype(
            self,
            ast,
            Some(i_template_array_prototype),
            None,
        );
        let builtin_162 = i_template_array_function_type
            .get_instance_type(self)
            .unwrap();
        builtin_162.merge_supertype_template_types(self, ast, array_of_string);
        let builtin_164 = i_template_array_function_type
            .get_instance_type(self)
            .unwrap();
        self.register_native_type(JSTypeNative::I_TEMPLATE_ARRAY_TYPE, builtin_164);
        let builtin_166 = self.create_template_type(ast, "T");
        self.generator_value_template = Some(builtin_166);
        let builtin_167 = self.create_template_type(ast, "TReturn");
        self.generator_return_template = Some(builtin_167);
        let builtin_168 = self.create_template_type(ast, "TNext");
        let generator_next_template = builtin_168;
        let builtin_169 = self.native_interface(
            ast,
            "Generator",
            &[
                self.generator_value_template.unwrap(),
                self.generator_return_template.unwrap(),
                generator_next_template,
            ],
        );
        let generator_function_type = builtin_169;
        let builtin_170 = self.create_templatized_type(
            ast,
            iterator_iterable_type,
            &[
                self.generator_value_template.unwrap(),
                self.generator_return_template.unwrap(),
                generator_next_template,
            ],
        );
        generator_function_type.set_extended_interfaces(self, ast, vec![builtin_170]);
        self.register_native_type(
            JSTypeNative::GENERATOR_FUNCTION_TYPE,
            generator_function_type,
        );
        let builtin_173 = generator_function_type.get_instance_type(self).unwrap();
        self.register_native_type(JSTypeNative::GENERATOR_TYPE, builtin_173);
        let builtin_175 = self.create_template_type(ast, "T");
        self.async_iterator_value_template = Some(builtin_175);
        let builtin_176 = self.create_template_type(ast, "TReturn");
        self.async_iterator_return_template = Some(builtin_176);
        let builtin_177 = self.create_template_type(ast, "TNext");
        let async_iterator_next_template = builtin_177;
        let builtin_178 = self.native_interface(
            ast,
            "AsyncIterator",
            &[
                self.async_iterator_value_template.unwrap(),
                self.async_iterator_return_template.unwrap(),
                async_iterator_next_template,
            ],
        );
        let async_iterator_function_type = builtin_178;
        self.register_native_type(
            JSTypeNative::ASYNC_ITERATOR_FUNCTION_TYPE,
            async_iterator_function_type,
        );
        let builtin_180 = async_iterator_function_type
            .get_instance_type(self)
            .unwrap();
        self.register_native_type(JSTypeNative::ASYNC_ITERATOR_TYPE, builtin_180);
        let builtin_182 = self.create_template_type(ast, "T");
        self.async_iterable_value_template = Some(builtin_182);
        let builtin_183 = self.create_template_type(ast, "TReturn");
        self.async_iterable_return_template = Some(builtin_183);
        let builtin_184 = self.create_template_type(ast, "TNext");
        let async_iterable_next_template = builtin_184;
        let builtin_185 = self.native_interface(
            ast,
            "AsyncIterable",
            &[
                self.async_iterable_value_template.unwrap(),
                self.async_iterable_return_template.unwrap(),
                async_iterable_next_template,
            ],
        );
        let async_iterable_function_type = builtin_185;
        self.register_native_type(
            JSTypeNative::ASYNC_ITERABLE_FUNCTION_TYPE,
            async_iterable_function_type,
        );
        let builtin_187 = async_iterable_function_type
            .get_instance_type(self)
            .unwrap();
        self.register_native_type(JSTypeNative::ASYNC_ITERABLE_TYPE, builtin_187);
        let builtin_189 = self.create_template_type(ast, "T");
        let async_iterator_iterable_template = builtin_189;
        let builtin_190 = self.create_template_type(ast, "TReturn");
        let async_iterator_iterable_return_template = builtin_190;
        let builtin_191 = self.create_template_type(ast, "TNext");
        let async_iterator_iterable_next_template = builtin_191;
        let builtin_192 = self.native_interface(
            ast,
            "AsyncIteratorIterable",
            &[
                async_iterator_iterable_template,
                async_iterator_iterable_return_template,
                async_iterator_iterable_next_template,
            ],
        );
        let async_iterator_iterable_function_type = builtin_192;
        let builtin_193 = async_iterator_function_type
            .get_instance_type(self)
            .unwrap();
        let builtin_194 = self.create_templatized_type(
            ast,
            builtin_193,
            &[
                async_iterator_iterable_template,
                async_iterator_iterable_return_template,
                async_iterator_iterable_next_template,
            ],
        );
        let builtin_195 = async_iterable_function_type
            .get_instance_type(self)
            .unwrap();
        let builtin_196 = self.create_templatized_type(
            ast,
            builtin_195,
            &[
                async_iterator_iterable_template,
                async_iterator_iterable_return_template,
                async_iterator_iterable_next_template,
            ],
        );
        async_iterator_iterable_function_type.set_extended_interfaces(
            self,
            ast,
            vec![builtin_194, builtin_196],
        );
        self.register_native_type(
            JSTypeNative::ASYNC_ITERATOR_ITERABLE_FUNCTION_TYPE,
            async_iterator_iterable_function_type,
        );
        let builtin_199 = async_iterator_iterable_function_type
            .get_instance_type(self)
            .unwrap();
        self.register_native_type(JSTypeNative::ASYNC_ITERATOR_ITERABLE_TYPE, builtin_199);
        let builtin_201 = self.create_template_type(ast, "T");
        let async_generator_value_template = builtin_201;
        let builtin_202 = self.create_template_type(ast, "TReturn");
        let async_generator_return_template = builtin_202;
        let builtin_203 = self.create_template_type(ast, "TNext");
        let async_generator_next_template = builtin_203;
        let builtin_204 = self.native_interface(
            ast,
            "AsyncGenerator",
            &[
                async_generator_value_template,
                async_generator_return_template,
                async_generator_next_template,
            ],
        );
        let async_generator_function_type = builtin_204;
        self.register_native_type(
            JSTypeNative::ASYNC_GENERATOR_FUNCTION_TYPE,
            async_generator_function_type,
        );
        let builtin_206 = async_iterator_iterable_function_type
            .get_instance_type(self)
            .unwrap();
        let builtin_207 = self.create_templatized_type(
            ast,
            builtin_206,
            &[
                async_generator_value_template,
                async_generator_return_template,
                async_generator_next_template,
            ],
        );
        async_generator_function_type.set_extended_interfaces(self, ast, vec![builtin_207]);
        let builtin_209 = async_generator_function_type
            .get_instance_type(self)
            .unwrap();
        self.register_native_type(JSTypeNative::ASYNC_GENERATOR_TYPE, builtin_209);
        let builtin_211 =
            self.native_interface(ast, "IThenable", &[self.i_thenable_template_key.unwrap()]);
        let ithenable_function_type = builtin_211;
        ithenable_function_type.set_struct(self);
        self.register_native_type(
            JSTypeNative::I_THENABLE_FUNCTION_TYPE,
            ithenable_function_type,
        );
        let builtin_214 = ithenable_function_type.get_instance_type(self).unwrap();
        let ithenable_type = builtin_214;
        self.register_native_type(JSTypeNative::I_THENABLE_TYPE, ithenable_type);
        let builtin_216 = self.create_record_type(ast, vec![("then".into(), unknown_type)]);
        let thenable_type = builtin_216;
        self.identify_non_nullable_name(ast, None, "Thenable");
        self.register_native_type(JSTypeNative::THENABLE_TYPE, thenable_type);
        let builtin_219 = self.create_templatized_type(
            ast,
            ithenable_type,
            &[self.promise_template_key.unwrap()],
        );
        let builtin_220 = self.create_union_type(
            ast,
            &[
                self.promise_template_key.unwrap(),
                builtin_219,
                thenable_type,
                null_type,
            ],
        );
        let builtin_221 = self.create_optional_parameters(ast, &[builtin_220]);
        let builtin_222 = self.create_function_type_with_parameters(ast, unknown_type, builtin_221);
        let builtin_223 = self.create_optional_parameters(ast, &[all_type]);
        let builtin_224 = self.create_function_type_with_parameters(ast, unknown_type, builtin_223);
        let builtin_225 = self.create_function_type(ast, unknown_type, &[builtin_222, builtin_224]);
        let promise_parameter_type = builtin_225;
        let builtin_226 = self.create_parameters(&[promise_parameter_type]);
        let builtin_227 = self.native_constructor_builder("Promise");
        let builtin_228 = builtin_227.with_parameters(builtin_226);
        let builtin_229_template_keys = vec![self.promise_template_key.unwrap()];
        let builtin_229 = builtin_228.with_template_keys(self, ast, builtin_229_template_keys);
        let builtin_230 = builtin_229.build(self, ast);
        let promise_function_type = builtin_230;
        let builtin_231 = self.create_templatized_type(
            ast,
            ithenable_type,
            &[self.promise_template_key.unwrap()],
        );
        promise_function_type.set_implemented_interfaces(self, ast, vec![builtin_231]);
        self.register_native_type(JSTypeNative::PROMISE_FUNCTION_TYPE, promise_function_type);
        let builtin_234 = promise_function_type.get_instance_type(self).unwrap();
        self.register_native_type(JSTypeNative::PROMISE_TYPE, builtin_234);
        let builtin_236 = self.native_constructor_builder("Arguments");
        let builtin_237 = builtin_236.with_parameters(vec![]);
        let builtin_238 = builtin_237.build(self, ast);
        let arguments_function_type = builtin_238;
        let builtin_239 = self.create_templatized_type(ast, i_array_like_type, &[unknown_type]);
        let builtin_240 = self.create_templatized_type(ast, iterable_type, &[unknown_type]);
        arguments_function_type.set_implemented_interfaces(
            self,
            ast,
            vec![builtin_239, builtin_240],
        );
        self.register_native_type(
            JSTypeNative::ARGUMENTS_FUNCTION_TYPE,
            arguments_function_type,
        );
        let builtin_243 = arguments_function_type.get_instance_type(self).unwrap();
        self.register_native_type(JSTypeNative::ARGUMENTS_TYPE, builtin_243);
        let builtin_245 = self.create_union_type(ast, &[big_int_type, number_type, string_type]);
        let bigint_number_string = builtin_245;
        self.register_native_type(JSTypeNative::BIGINT_NUMBER_STRING, bigint_number_string);
        let builtin_247 = self.create_parameters(&[bigint_number_string]);
        let builtin_248 = self.native_constructor_builder("BigInt");
        let builtin_249 = builtin_248.with_parameters(builtin_247);
        let builtin_250 = builtin_249.with_return_type(big_int_type);
        let builtin_251 = builtin_250.build(self, ast);
        let big_int_object_function_type = builtin_251;
        big_int_object_function_type.get_prototype(self, ast);
        self.register_native_type(
            JSTypeNative::BIGINT_OBJECT_FUNCTION_TYPE,
            big_int_object_function_type,
        );
        let builtin_254 = big_int_object_function_type
            .get_instance_type(self)
            .unwrap();
        let big_int_object_type = builtin_254;
        self.register_native_type(JSTypeNative::BIGINT_OBJECT_TYPE, big_int_object_type);
        let builtin_256 = self.create_optional_parameters(ast, &[all_type]);
        let builtin_257 = self.native_constructor_builder("Boolean");
        let builtin_258 = builtin_257.with_parameters(builtin_256);
        let builtin_259 = builtin_258.with_return_type(boolean_type);
        let builtin_260 = builtin_259.build(self, ast);
        let boolean_object_function_type = builtin_260;
        boolean_object_function_type.get_prototype(self, ast);
        self.register_native_type(
            JSTypeNative::BOOLEAN_OBJECT_FUNCTION_TYPE,
            boolean_object_function_type,
        );
        let builtin_263 = boolean_object_function_type
            .get_instance_type(self)
            .unwrap();
        let boolean_object_type = builtin_263;
        self.register_native_type(JSTypeNative::BOOLEAN_OBJECT_TYPE, boolean_object_type);
        let builtin_265 = self.create_optional_parameters(
            ast,
            &[
                unknown_type,
                unknown_type,
                unknown_type,
                unknown_type,
                unknown_type,
                unknown_type,
                unknown_type,
            ],
        );
        let builtin_266 = self.native_constructor_builder("Date");
        let builtin_267 = builtin_266.with_parameters(builtin_265);
        let builtin_268 = builtin_267.with_return_type(string_type);
        let builtin_269 = builtin_268.build(self, ast);
        let date_function_type = builtin_269;
        date_function_type.get_prototype(self, ast);
        self.register_native_type(JSTypeNative::DATE_FUNCTION_TYPE, date_function_type);
        let builtin_272 = date_function_type.get_instance_type(self).unwrap();
        let date_type = builtin_272;
        self.register_native_type(JSTypeNative::DATE_TYPE, date_type);
        let builtin_274 = self.create_optional_parameters(ast, &[all_type]);
        let builtin_275 = self.native_constructor_builder("Number");
        let builtin_276 = builtin_275.with_parameters(builtin_274);
        let builtin_277 = builtin_276.with_return_type(number_type);
        let builtin_278 = builtin_277.build(self, ast);
        let number_object_function_type = builtin_278;
        number_object_function_type.get_prototype(self, ast);
        self.register_native_type(
            JSTypeNative::NUMBER_OBJECT_FUNCTION_TYPE,
            number_object_function_type,
        );
        let builtin_281 = number_object_function_type.get_instance_type(self).unwrap();
        let number_object_type = builtin_281;
        self.register_native_type(JSTypeNative::NUMBER_OBJECT_TYPE, number_object_type);
        let builtin_283 = self.create_optional_parameters(ast, &[all_type, all_type]);
        let builtin_284 = self.native_constructor_builder("RegExp");
        let builtin_285 = builtin_284.with_parameters(builtin_283);
        let builtin_286 = builtin_285.with_returns_own_instance_type();
        let builtin_287 = builtin_286.build(self, ast);
        let regexp_function_type = builtin_287;
        regexp_function_type.get_prototype(self, ast);
        self.register_native_type(JSTypeNative::REGEXP_FUNCTION_TYPE, regexp_function_type);
        let builtin_290 = regexp_function_type.get_instance_type(self).unwrap();
        let regexp_type = builtin_290;
        self.register_native_type(JSTypeNative::REGEXP_TYPE, regexp_type);
        let builtin_292 = self.create_optional_parameters(ast, &[all_type]);
        let builtin_293 = self.native_constructor_builder("String");
        let builtin_294 = builtin_293.with_parameters(builtin_292);
        let builtin_295 = builtin_294.with_return_type(string_type);
        let builtin_296 = builtin_295.build(self, ast);
        let string_object_function_type = builtin_296;
        string_object_function_type.get_prototype(self, ast);
        self.register_native_type(
            JSTypeNative::STRING_OBJECT_FUNCTION_TYPE,
            string_object_function_type,
        );
        let builtin_299 = string_object_function_type.get_instance_type(self).unwrap();
        let string_object_type = builtin_299;
        self.register_native_type(JSTypeNative::STRING_OBJECT_TYPE, string_object_type);
        let builtin_301 = self.create_optional_parameters(ast, &[all_type]);
        let builtin_302 = self.native_constructor_builder("Symbol");
        let builtin_303 = builtin_302.with_parameters(builtin_301);
        let builtin_304 = builtin_303.with_return_type(symbol_type);
        let builtin_305 = builtin_304.build(self, ast);
        let symbol_object_function_type = builtin_305;
        symbol_object_function_type.get_prototype(self, ast);
        self.register_native_type(
            JSTypeNative::SYMBOL_OBJECT_FUNCTION_TYPE,
            symbol_object_function_type,
        );
        let builtin_308 = symbol_object_function_type.get_instance_type(self).unwrap();
        let symbol_object_type = builtin_308;
        self.register_native_type(JSTypeNative::SYMBOL_OBJECT_TYPE, symbol_object_type);
        let builtin_310 = self.native_interface(ast, "gbigint", &[]);
        let gbigint_function_type = builtin_310;
        gbigint_function_type.get_prototype(self, ast);
        let builtin_312 = gbigint_function_type.get_instance_type(self).unwrap();
        self.register_native_type(JSTypeNative::GBIGINT_TYPE, builtin_312);
        let builtin_314 = self.create_union_type(ast, &[null_type, void_type]);
        let null_void = builtin_314;
        self.register_native_type(JSTypeNative::NULL_VOID, null_void);
        let builtin_316 = self.create_union_type(ast, &[number_type, string_type, boolean_type]);
        let number_string_boolean = builtin_316;
        self.register_native_type(JSTypeNative::NUMBER_STRING_BOOLEAN, number_string_boolean);
        let builtin_318 =
            self.create_union_type(ast, &[number_type, string_type, boolean_type, symbol_type]);
        let value_types = builtin_318;
        self.register_native_type(JSTypeNative::VALUE_TYPES, value_types);
        let builtin_320 = self.create_union_type(ast, &[number_type, symbol_type]);
        let number_symbol = builtin_320;
        self.register_native_type(JSTypeNative::NUMBER_SYMBOL, number_symbol);
        let builtin_322 = self.create_union_type(ast, &[string_type, symbol_type]);
        let string_symbol = builtin_322;
        self.register_native_type(JSTypeNative::STRING_SYMBOL, string_symbol);
        let builtin_324 = self.create_union_type(ast, &[number_type, string_type]);
        let number_string = builtin_324;
        self.register_native_type(JSTypeNative::NUMBER_STRING, number_string);
        let builtin_326 = self.create_union_type(ast, &[big_int_type, number_type]);
        let bigint_number = builtin_326;
        self.register_native_type(JSTypeNative::BIGINT_NUMBER, bigint_number);
        let builtin_328 = self.create_union_type(ast, &[big_int_object_type, number_object_type]);
        let bigint_number_object = builtin_328;
        self.register_native_type(JSTypeNative::BIGINT_NUMBER_OBJECT, bigint_number_object);
        let builtin_330 = self.create_union_type(
            ast,
            &[big_int_object_type, number_object_type, string_object_type],
        );
        let bigint_number_string_object = builtin_330;
        self.register_native_type(
            JSTypeNative::BIGINT_NUMBER_STRING_OBJECT,
            bigint_number_string_object,
        );
        let builtin_332 = self.create_union_type(ast, &[number_type, string_type, symbol_type]);
        let number_string_symbol = builtin_332;
        self.register_native_type(JSTypeNative::NUMBER_STRING_SYMBOL, number_string_symbol);
        let builtin_334 = self.create_union_type_from_native(
            ast,
            &[
                JSTypeNative::VOID_TYPE,
                JSTypeNative::NULL_TYPE,
                JSTypeNative::NUMBER_TYPE,
                JSTypeNative::NUMBER_OBJECT_TYPE,
                JSTypeNative::BOOLEAN_TYPE,
                JSTypeNative::BOOLEAN_OBJECT_TYPE,
            ],
        );
        let number_addition_supertype = builtin_334;
        self.register_native_type(
            JSTypeNative::NUMBER_ADDITION_SUPERTYPE,
            number_addition_supertype,
        );
        let builtin_336 = self.create_native_function_type_with_var_args(ast, no_type, &[all_type]);
        let least_function_type = builtin_336;
        self.register_native_type(JSTypeNative::LEAST_FUNCTION_TYPE, least_function_type);
        let builtin_338 = self.create_parameters(&[all_type]);
        let builtin_339 = self.native_constructor_builder("global this");
        let builtin_340 = builtin_339.with_parameters(builtin_338);
        let builtin_341 = builtin_340.with_return_type(number_type);
        let builtin_342 = builtin_341.build(self, ast);
        let global_this_ctor = builtin_342;
        let builtin_343 = global_this_ctor.get_instance_type(self).unwrap();
        let global_this = builtin_343;
        self.register_native_type(JSTypeNative::GLOBAL_THIS, global_this);
        let builtin_345 = self.create_native_function_type_with_var_args(ast, all_type, &[no_type]);
        let greatest_function_type = builtin_345;
        self.register_native_type(JSTypeNative::GREATEST_FUNCTION_TYPE, greatest_function_type);
        self.register_property_on_type(ast, "prototype", object_function_type);
        let builtin_348 = self.native_interface(
            ast,
            "ReadonlyMap",
            &[
                self.readonly_map_key_template_key.unwrap(),
                self.readonly_map_value_template_key.unwrap(),
            ],
        );
        let readonly_map_function_type = builtin_348;
        self.register_native_type(
            JSTypeNative::READONLY_MAP_FUNCTION_TYPE,
            readonly_map_function_type,
        );
        let builtin_350 = self.create_union_type(
            ast,
            &[
                self.readonly_map_key_template_key.unwrap(),
                self.readonly_map_value_template_key.unwrap(),
            ],
        );
        let builtin_351 = self.create_templatized_type(ast, array_type, &[builtin_350]);
        let builtin_352 = self.create_templatized_type(ast, iterable_type, &[builtin_351]);
        readonly_map_function_type.set_extended_interfaces(self, ast, vec![builtin_352]);
        let builtin_354 = readonly_map_function_type.get_instance_type(self).unwrap();
        let readonly_map_type = builtin_354;
        self.register_native_type(JSTypeNative::READONLY_MAP_TYPE, readonly_map_type);
        let builtin_356 = self.create_union_type(
            ast,
            &[
                self.map_key_template_key.unwrap(),
                self.map_value_template_key.unwrap(),
            ],
        );
        let builtin_357 = self.create_templatized_type(ast, array_type, &[builtin_356]);
        let builtin_358 = self.create_templatized_type(ast, iterable_type, &[builtin_357]);
        let builtin_359 = self.create_union_type(
            ast,
            &[
                self.map_key_template_key.unwrap(),
                self.map_value_template_key.unwrap(),
            ],
        );
        let builtin_360 = self.create_templatized_type(ast, array_type, &[builtin_359]);
        let builtin_361 = self.create_templatized_type(ast, array_type, &[builtin_360]);
        let builtin_362 = self.create_union_type(ast, &[null_type, builtin_358, builtin_361]);
        let builtin_363 = self.create_optional_parameters(ast, &[builtin_362]);
        let builtin_364 = self.native_constructor_builder("Map");
        let builtin_365 = builtin_364.with_parameters(builtin_363);
        let builtin_366_template_keys = vec![
            self.map_key_template_key.unwrap(),
            self.map_value_template_key.unwrap(),
        ];
        let builtin_366 = builtin_365.with_template_keys(self, ast, builtin_366_template_keys);
        let builtin_367 = builtin_366.build(self, ast);
        let map_function_type = builtin_367;
        self.register_native_type(JSTypeNative::MAP_FUNCTION_TYPE, map_function_type);
        map_function_type.get_prototype(self, ast);
        let builtin_370 = self.create_templatized_type(
            ast,
            readonly_map_type,
            &[
                self.map_key_template_key.unwrap(),
                self.map_value_template_key.unwrap(),
            ],
        );
        map_function_type.set_implemented_interfaces(self, ast, vec![builtin_370]);
        let builtin_372 = map_function_type.get_instance_type(self).unwrap();
        let map_type = builtin_372;
        self.register_native_type(JSTypeNative::MAP_TYPE, map_type);
    }
}

impl JSTypeRegistry {
    // port: JSTypeRegistry#initializeRegistry
    fn initialize_registry(&mut self, ast: &Ast) {
        self.register_global_type(ast, self.get_native_type(JSTypeNative::ARGUMENTS_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::ARRAY_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::READONLY_ARRAY_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::MAP_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::READONLY_MAP_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::ASYNC_ITERABLE_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::ASYNC_ITERATOR_TYPE));
        self.register_global_type(
            ast,
            self.get_native_type(JSTypeNative::ASYNC_ITERATOR_ITERABLE_TYPE),
        );
        self.register_global_type(
            ast,
            self.get_native_type(JSTypeNative::ASYNC_GENERATOR_TYPE),
        );
        self.register_global_type(ast, self.get_native_type(JSTypeNative::BIGINT_OBJECT_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::BIGINT_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::GBIGINT_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::BOOLEAN_OBJECT_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::BOOLEAN_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::I_ARRAY_LIKE_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::ITERABLE_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::ITERATOR_LIKE_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::ITERATOR_TYPE));
        self.register_global_type(
            ast,
            self.get_native_type(JSTypeNative::ITERATOR_ITERABLE_TYPE),
        );
        self.register_global_type(ast, self.get_native_type(JSTypeNative::GENERATOR_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::DATE_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::I_OBJECT_TYPE));
        self.register_global_type(
            ast,
            self.get_native_type(JSTypeNative::I_ITERABLE_RESULT_TYPE),
        );
        self.register_global_type(
            ast,
            self.get_native_type(JSTypeNative::I_TEMPLATE_ARRAY_TYPE),
        );
        self.register_global_type(ast, self.get_native_type(JSTypeNative::I_THENABLE_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::NULL_TYPE));
        self.register_global_type_with_name(
            ast,
            self.get_native_type(JSTypeNative::NULL_TYPE),
            "Null",
        );
        self.register_global_type(ast, self.get_native_type(JSTypeNative::NUMBER_OBJECT_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::NUMBER_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::OBJECT_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::PROMISE_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::REGEXP_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::STRING_OBJECT_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::STRING_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::SYMBOL_OBJECT_TYPE));
        self.register_global_type(ast, self.get_native_type(JSTypeNative::SYMBOL_TYPE));
        self.register_global_type_with_name(
            ast,
            self.get_native_type(JSTypeNative::THENABLE_TYPE),
            "Thenable",
        );
        self.register_global_type(ast, self.get_native_type(JSTypeNative::VOID_TYPE));
        self.register_global_type_with_name(
            ast,
            self.get_native_type(JSTypeNative::VOID_TYPE),
            "Undefined",
        );
        self.register_global_type_with_name(
            ast,
            self.get_native_type(JSTypeNative::VOID_TYPE),
            "void",
        );
        self.register_global_type_with_name(
            ast,
            self.get_native_type(JSTypeNative::FUNCTION_TYPE),
            "Function",
        );
        self.register_global_type_with_name(
            ast,
            self.get_native_type(JSTypeNative::GLOBAL_THIS),
            "Global",
        );
    }
}
