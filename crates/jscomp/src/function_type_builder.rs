/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/FunctionTypeBuilder.java.

//! FunctionTypeBuilder.java: builds the FunctionType of a function or class from its JSDoc, its
//! syntax and the function it overrides, and registers it in the type registry.
// Preserve Java branch structure.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::unnecessary_unwrap
)]

use crate::abstract_compiler::AbstractCompiler;
use crate::diagnostic_group::DiagnosticGroup;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_util::NodeUtil;
use crate::typed_scope::TypedScope;
use crate::typed_scope_creator::TypedScopeCreator;
use closure_jstype::{
    JSTypeNative, JSTypeRegistry, TypeId,
    function_param_builder::FunctionParamBuilder,
    function_type::{self, Parameter},
    js_type::TypeValidator,
    object_type,
    prelude::*,
    rhino::js_type_expression::JSTypeExpressionExt,
    static_typed_scope::StaticTypedScope,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    closure_primitive::ClosurePrimitive,
    ir::IR,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
};
use std::sync::{Arc, LazyLock};

// port: FunctionTypeBuilder#EXTENDS_WITHOUT_TYPEDEF
pub static EXTENDS_WITHOUT_TYPEDEF: DiagnosticType = DiagnosticType::warning(
    "JSC_EXTENDS_WITHOUT_TYPEDEF",
    "@extends used without @constructor or @interface for {0}",
);

// port: FunctionTypeBuilder#EXTENDS_NON_OBJECT
pub static EXTENDS_NON_OBJECT: DiagnosticType =
    DiagnosticType::warning("JSC_EXTENDS_NON_OBJECT", "{0} @extends non-object type {1}");

// port: FunctionTypeBuilder#RESOLVED_TAG_EMPTY
pub static RESOLVED_TAG_EMPTY: DiagnosticType = DiagnosticType::warning(
    "JSC_RESOLVED_TAG_EMPTY",
    "Could not resolve type in {0} tag of {1}",
);

// port: FunctionTypeBuilder#CONSTRUCTOR_REQUIRED
pub static CONSTRUCTOR_REQUIRED: DiagnosticType = DiagnosticType::warning(
    "JSC_CONSTRUCTOR_REQUIRED",
    "{0} used without @constructor for {1}",
);

// port: FunctionTypeBuilder#VAR_ARGS_MUST_BE_LAST
pub static VAR_ARGS_MUST_BE_LAST: DiagnosticType = DiagnosticType::warning(
    "JSC_VAR_ARGS_MUST_BE_LAST",
    "variable length argument must be last",
);

// port: FunctionTypeBuilder#OPTIONAL_ARG_AT_END
pub static OPTIONAL_ARG_AT_END: DiagnosticType = DiagnosticType::warning(
    "JSC_OPTIONAL_ARG_AT_END",
    "optional arguments must be at the end",
);

// port: FunctionTypeBuilder#INEXISTENT_PARAM
pub static INEXISTENT_PARAM: DiagnosticType = DiagnosticType::warning(
    "JSC_INEXISTENT_PARAM",
    "parameter {0} does not appear in {1}''s parameter list",
);

// port: FunctionTypeBuilder#TYPE_REDEFINITION
pub static TYPE_REDEFINITION: DiagnosticType = DiagnosticType::warning(
    "JSC_TYPE_REDEFINITION",
    "attempted re-definition of type {0}\nfound   : {1}\nexpected: {2}",
);

// port: FunctionTypeBuilder#TEMPLATE_TRANSFORMATION_ON_CLASS
pub static TEMPLATE_TRANSFORMATION_ON_CLASS: DiagnosticType = DiagnosticType::warning(
    "JSC_TEMPLATE_TRANSFORMATION_ON_CLASS",
    "Template type transformation {0} not allowed on classes or interfaces",
);

// port: FunctionTypeBuilder#TEMPLATE_TYPE_ILLEGAL_BOUND
pub static TEMPLATE_TYPE_ILLEGAL_BOUND: DiagnosticType = DiagnosticType::error(
    "JSC_TEMPLATE_TYPE_ILLEGAL_BOUND",
    "Illegal upper bound ''{0}'' on template type parameter {1}",
);

// port: FunctionTypeBuilder#ALL_DIAGNOSTICS
pub static ALL_DIAGNOSTICS: LazyLock<Arc<DiagnosticGroup>> = LazyLock::new(|| {
    Arc::new(DiagnosticGroup::new(&[
        &EXTENDS_WITHOUT_TYPEDEF,
        &EXTENDS_NON_OBJECT,
        &RESOLVED_TAG_EMPTY,
        &CONSTRUCTOR_REQUIRED,
        &VAR_ARGS_MUST_BE_LAST,
        &OPTIONAL_ARG_AT_END,
        &INEXISTENT_PARAM,
        &TYPE_REDEFINITION,
        &TEMPLATE_TRANSFORMATION_ON_CLASS,
        &TEMPLATE_TYPE_ILLEGAL_BOUND,
        &crate::type_check::SAME_INTERFACE_MULTIPLE_IMPLEMENTS,
    ]))
});

/// Rust-only: Java's `compiler.report(error)`, callable from the type validators the registry runs
/// when a NamedType resolves, where no `&mut AbstractCompiler` is in reach.
type CompilerReportFn = Arc<dyn Fn(JSError) + Send + Sync>;

/// Rust-only: a `CompilerReportFn` that reports to `compiler`. The error joins the queue of the
/// registry's own reporter (`QueueingOldRhinoErrorReporter`), which the Compiler reports, in order,
/// before it next reports or reads errors (the error managers sort, so the deferral changes no
/// output).
fn get_compiler_report_fn(compiler: &AbstractCompiler) -> CompilerReportFn {
    let queue = compiler.type_registry_error_queue();
    Arc::new(move |error: JSError| queue.push(error))
}

// port: FunctionTypeBuilder.ValidatorBase
struct ValidatorBase {
    compiler: CompilerReportFn,
    error_root: NodeId,
}

impl ValidatorBase {
    // port: FunctionTypeBuilder.ValidatorBase#ValidatorBase
    fn new(error_root: NodeId, compiler: CompilerReportFn) -> Self {
        Self {
            error_root,
            compiler,
        }
    }

    // port: FunctionTypeBuilder.ValidatorBase#reportWarning
    fn report_warning(&self, ast: &Ast, warning: &'static DiagnosticType, args: &[&str]) {
        (self.compiler)(JSError::make(ast, self.error_root, warning, args));
    }

    // port: FunctionTypeBuilder.ValidatorBase#reportError
    fn report_error(&self, ast: &Ast, error: &'static DiagnosticType, args: &[&str]) {
        (self.compiler)(JSError::make(ast, self.error_root, error, args));
    }
}

// port: FunctionTypeBuilder.ExtendedTypeValidator
struct ExtendedTypeValidator {
    base: ValidatorBase,
    formatted_fn_name: String,
}

impl ExtendedTypeValidator {
    // port: FunctionTypeBuilder.ExtendedTypeValidator#ExtendedTypeValidator
    fn new(error_root: NodeId, compiler: CompilerReportFn, formatted_fn_name: String) -> Self {
        Self {
            base: ValidatorBase::new(error_root, compiler),
            formatted_fn_name,
        }
    }

    // port: FunctionTypeBuilder.ExtendedTypeValidator#apply
    fn apply(&self, type_: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        let object_type = object_type::cast(reg, Some(type_));
        let Some(object_type) = object_type else {
            let type_string = type_.to_string(reg, ast);
            self.base.report_warning(
                ast,
                &EXTENDS_NON_OBJECT,
                &[&self.formatted_fn_name, &type_string],
            );
            return false;
        };
        if object_type.is_empty_type(reg) {
            self.base.report_warning(
                ast,
                &RESOLVED_TAG_EMPTY,
                &["@extends", &self.formatted_fn_name],
            );
            return false;
        }
        if object_type.is_unknown_type(reg, ast) {
            if FunctionTypeBuilder::has_more_tags_to_resolve(reg, ast, object_type)
                || type_.is_template_type(reg)
            {
                return true;
            } else {
                self.base.report_warning(
                    ast,
                    &RESOLVED_TAG_EMPTY,
                    &["@extends", &self.formatted_fn_name],
                );
                return false;
            }
        }
        true
    }
}

// port: FunctionTypeBuilder.ImplementedTypeValidator
struct ImplementedTypeValidator {
    base: ValidatorBase,
    formatted_fn_name: String,
}

impl ImplementedTypeValidator {
    // port: FunctionTypeBuilder.ImplementedTypeValidator#ImplementedTypeValidator
    fn new(error_root: NodeId, compiler: CompilerReportFn, formatted_fn_name: String) -> Self {
        Self {
            base: ValidatorBase::new(error_root, compiler),
            formatted_fn_name,
        }
    }

    // port: FunctionTypeBuilder.ImplementedTypeValidator#apply
    fn apply(&self, type_: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        let object_type = object_type::cast(reg, Some(type_));
        if object_type.is_none() {
            self.base.report_error(
                ast,
                &crate::type_check::BAD_IMPLEMENTED_TYPE,
                &[&self.formatted_fn_name],
            );
            false
        } else if object_type.unwrap().is_empty_type(reg) {
            self.base.report_warning(
                ast,
                &RESOLVED_TAG_EMPTY,
                &["@implements", &self.formatted_fn_name],
            );
            false
        } else if object_type.unwrap().is_unknown_type(reg, ast) {
            if FunctionTypeBuilder::has_more_tags_to_resolve(reg, ast, object_type.unwrap()) {
                true
            } else {
                self.base.report_warning(
                    ast,
                    &RESOLVED_TAG_EMPTY,
                    &["@implements", &self.formatted_fn_name],
                );
                false
            }
        } else {
            true
        }
    }
}

/// A builder for FunctionTypes, because FunctionTypes are so ridiculously complex. All methods
/// return {@code this} for ease of use.
///
/// The Java fields `compiler`, `codingConvention` and `typeRegistry` are not stored (DESIGN.md
/// section 6): every method that used them takes the compiler.
pub struct FunctionTypeBuilder {
    fn_name: JsString,
    error_root: NodeId,
    contents: Arc<dyn FunctionContents>,
    syntactic_fn_name: Option<JsString>,
    return_type: Option<TypeId>,
    return_type_inferred: bool,
    implemented_interfaces: Option<Vec<TypeId>>,
    extended_interfaces: Option<Vec<TypeId>>,
    base_type: Option<TypeId>,
    this_type: Option<TypeId>,
    is_class: bool,
    is_constructor: bool,
    makes_structs: bool,
    makes_unrestricted: bool,
    makes_dicts: bool,
    is_interface: bool,
    is_record: bool,
    is_abstract: bool,
    is_known_ambiguous: bool,
    parameters: Option<Vec<Parameter>>,
    closure_primitive_id: Option<ClosurePrimitive>,
    template_type_names: Vec<TypeId>,
    constructor_template_type_names: Vec<TypeId>,
    declaration_scope: Option<TypedScope>,
    template_scope: Arc<dyn StaticTypedScope>,
}

impl FunctionTypeBuilder {
    // port: FunctionTypeBuilder#createExtendedTypeValidator
    fn create_extended_type_validator(&self, compiler: &AbstractCompiler) -> TypeValidator {
        let validator = ExtendedTypeValidator::new(
            self.error_root,
            get_compiler_report_fn(compiler),
            self.format_fn_name(),
        );
        Arc::new(move |type_, reg, ast| validator.apply(type_, reg, ast))
    }

    // port: FunctionTypeBuilder#createImplementedTypeValidator
    fn create_implemented_type_validator(&self, compiler: &AbstractCompiler) -> TypeValidator {
        let validator = ImplementedTypeValidator::new(
            self.error_root,
            get_compiler_report_fn(compiler),
            self.format_fn_name(),
        );
        Arc::new(move |type_, reg, ast| validator.apply(type_, reg, ast))
    }

    /// @param fnName The function name to be used in error messages.
    /// @param errorRoot The node to associate with any warning generated by this builder.
    /// @param scope The syntactic scope.
    // port: FunctionTypeBuilder#FunctionTypeBuilder
    pub fn new(
        fn_name: Option<JsString>,
        compiler: &AbstractCompiler,
        error_root: NodeId,
        scope: TypedScope,
    ) -> Self {
        // checkNotNull(errorRoot): a NodeId is never null.
        Self {
            fn_name: fn_name.unwrap_or_default(),
            error_root,
            contents: UnknownFunctionContents::get(),
            syntactic_fn_name: None,
            return_type: None,
            return_type_inferred: false,
            implemented_interfaces: None,
            extended_interfaces: None,
            base_type: None,
            this_type: None,
            is_class: false,
            is_constructor: false,
            makes_structs: false,
            makes_unrestricted: false,
            makes_dicts: false,
            is_interface: false,
            is_record: false,
            is_abstract: false,
            is_known_ambiguous: false,
            parameters: None,
            closure_primitive_id: None,
            template_type_names: Vec::new(),
            constructor_template_type_names: Vec::new(),
            declaration_scope: None,
            template_scope: scope.as_static_typed_scope_arc(compiler),
        }
    }

    /// Format the function name for use in warnings.
    // port: FunctionTypeBuilder#formatFnName
    pub fn format_fn_name(&self) -> String {
        if self.fn_name.is_empty() {
            "<anonymous>".to_string()
        } else {
            self.fn_name.to_string_lossy()
        }
    }

    /// Sets the name with which this new type will be declared in the type registry.
    // port: FunctionTypeBuilder#setSyntacticFunctionName
    pub fn set_syntactic_function_name(
        &mut self,
        syntactic_fn_name: Option<JsString>,
    ) -> &mut Self {
        self.syntactic_fn_name = Some(syntactic_fn_name.unwrap_or_default());
        self
    }

    /// Sets the contents of this function.
    // port: FunctionTypeBuilder#setContents
    pub fn set_contents(&mut self, contents: Option<Arc<dyn FunctionContents>>) -> &mut Self {
        if let Some(contents) = contents {
            self.contents = contents;
        }
        self
    }

    /// Sets a declaration scope explicitly. This is important with block scopes because a function
    /// declared in an inner scope with 'var' needs to use the inner scope to resolve names, but
    /// needs to be declared in the outer scope.
    // port: FunctionTypeBuilder#setDeclarationScope
    pub fn set_declaration_scope(&mut self, declaration_scope: TypedScope) -> &mut Self {
        self.declaration_scope = Some(declaration_scope);
        self
    }

    /// Infer the parameter and return types of a function from the parameter and return types of
    /// the function it is overriding.
    ///
    /// @param oldType The function being overridden. Does nothing if this is null.
    /// @param paramsParent The PARAM_LIST node of the function that we're assigning to. If null,
    ///     that just means we're not initializing this to a function literal.
    // port: FunctionTypeBuilder#inferFromOverriddenFunction
    pub fn infer_from_overridden_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        old_type: Option<TypeId>,
        params_parent: Option<NodeId>,
    ) -> &mut Self {
        let Some(old_type) = old_type else {
            return self;
        };

        let (reg, _) = compiler.get_type_registry_and_ast();
        // Propagate the template types, if they exist.
        self.template_type_names = old_type
            .get_template_type_map(reg)
            .get_template_keys()
            .to_vec();

        self.return_type = Some(old_type.get_return_type(reg));
        self.return_type_inferred = old_type.is_return_type_inferred(reg);
        if params_parent.is_none() {
            // Not a function literal.
            self.parameters = Some(old_type.get_parameters(reg));
            if self.parameters.is_none() {
                self.parameters = Some(FunctionParamBuilder::new().build());
            }
        } else {
            // We're overriding with a function literal. Apply type information
            // to each parameter of the literal.
            let mut param_builder = FunctionParamBuilder::new();
            let mut old_params = old_type.get_parameters(reg).into_iter().peekable();
            let mut warned_about_arg_list = false;
            let mut old_params_list_hit_opt_args = false;
            let mut current_param = params_parent.unwrap().get_first_child(compiler);
            while let Some(current) = current_param {
                if old_params.peek().is_some() {
                    let old_param = old_params.next().unwrap();

                    old_params_list_hit_opt_args = old_params_list_hit_opt_args
                        || old_param.is_variadic()
                        || old_param.is_optional();

                    // The subclass method might write its var_args as individual arguments.

                    let mut is_optional_arg = old_param.is_optional();
                    let mut is_var_args = old_param.is_variadic();
                    if current.get_next(compiler).is_some() && is_var_args {
                        is_var_args = false;
                        is_optional_arg = true;
                    }
                    // The subclass method might also make a required parameter into an optional
                    // parameter with a default value
                    if current.is_default_value(compiler) {
                        is_optional_arg = true;
                    }
                    param_builder.new_parameter_from(Parameter::create(
                        old_param.get_jstype(),
                        is_optional_arg,
                        is_var_args,
                    ));
                } else {
                    let unknown_type = {
                        let (reg, _) = compiler.get_type_registry_and_ast();
                        reg.get_native_type(JSTypeNative::UNKNOWN_TYPE)
                    };
                    let is_optional = compiler
                        .get_coding_convention()
                        .is_optional_parameter(compiler, current)
                        || old_params_list_hit_opt_args
                        || current.is_default_value(compiler);
                    let is_var_args = compiler
                        .get_coding_convention()
                        .is_var_args_parameter(compiler, current);
                    warned_about_arg_list |= self.add_parameter(
                        compiler,
                        &mut param_builder,
                        unknown_type,
                        warned_about_arg_list,
                        is_optional,
                        is_var_args,
                    );
                }
                current_param = current.get_next(compiler);
            }

            // Clone any remaining params that aren't in the function literal,
            // but make them optional.
            for old_param in old_params {
                param_builder.new_optional_parameter_from(old_param);
            }

            self.parameters = Some(param_builder.build());
        }
        self
    }

    /// Infer the return type from JSDocInfo.
    ///
    /// @param fromInlineDoc Indicates whether return type is inferred from inline doc attached to
    ///     function name
    // port: FunctionTypeBuilder#inferReturnType
    pub fn infer_return_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<&JSDocInfo>,
        from_inline_doc: bool,
    ) -> &mut Self {
        if let Some(info) = info {
            let return_type_expr = if from_inline_doc {
                info.get_type()
            } else {
                info.get_return_type()
            };
            if let Some(return_type_expr) = return_type_expr {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                self.return_type =
                    Some(return_type_expr.evaluate(reg, ast, Some(self.template_scope.clone())));
                self.return_type_inferred = false;
            }
        }

        self
    }

    // port: FunctionTypeBuilder#usingClassSyntax
    pub fn using_class_syntax(&mut self) -> &mut Self {
        self.is_class = true;
        self
    }

    /// Infer whether the function is a normal function, a constructor, or an interface.
    // port: FunctionTypeBuilder#inferKind
    pub fn infer_kind(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<&JSDocInfo>,
    ) -> &mut Self {
        if let Some(info) = info {
            if !NodeUtil::is_method_declaration(compiler, self.error_root) {
                self.is_constructor = info.is_constructor();
                self.is_interface = info.is_interface();
                self.is_record = info.uses_implicit_match();
                self.makes_structs = info.makes_structs();
                self.makes_unrestricted = info.makes_unrestricted();
                self.makes_dicts = info.makes_dicts();
            }
            self.is_abstract = info.is_abstract();
        }
        if self.is_class {
            // If a CLASS literal has not been explicitly declared an interface, it's a constructor.
            // If it's not expicitly @dict or @unrestricted then it's @struct.
            self.is_constructor = !self.is_interface;
            self.makes_structs =
                info.is_none_or(|info| !self.makes_dicts && !info.makes_unrestricted());
        }

        if self.makes_structs && !(self.is_constructor || self.is_interface) {
            let fn_name = self.format_fn_name();
            self.report_warning(compiler, &CONSTRUCTOR_REQUIRED, &["@struct", &fn_name]);
        } else if self.makes_dicts && !self.is_constructor {
            let fn_name = self.format_fn_name();
            self.report_warning(compiler, &CONSTRUCTOR_REQUIRED, &["@dict", &fn_name]);
        }
        self
    }

    /// Clobber the templateTypeNames from the JSDoc with builtin ones for native types.
    // port: FunctionTypeBuilder#maybeUseNativeClassTemplateNames
    fn maybe_use_native_class_template_names(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: &JSDocInfo,
    ) -> bool {
        let declaration_scope = self
            .declaration_scope
            .map(|scope| scope.as_static_typed_scope_arc(compiler));
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let native_keys = reg.maybe_get_template_types_of_builtin(
            ast,
            declaration_scope.as_deref(),
            self.fn_name.clone(),
        );
        // TODO(b/73386087): Make infoTemplateTypeNames.size() == nativeKeys.size() a
        // Preconditions check. It currently fails for "var symbol" in the externs.
        if native_keys.is_some()
            && info.get_template_type_names().len() == native_keys.as_ref().unwrap().len()
        {
            self.template_type_names = native_keys.unwrap();
            return true;
        }
        false
    }

    /// Infer any supertypes from the JSDocInfo or the passed-in base type.
    ///
    /// @param info JSDoc info that is attached to the type declaration, if any
    /// @param classExtendsType The type of the extends clause in `class C extends SuperClass {}`,
    ///     if present.
    /// @return this object
    // port: FunctionTypeBuilder#inferInheritance
    pub fn infer_inheritance(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<&JSDocInfo>,
        class_extends_type: Option<TypeId>,
    ) -> &mut Self {
        let mut class_extends_type = class_extends_type;

        if info.is_some() && info.unwrap().has_base_type() {
            if self.is_constructor || self.is_interface {
                let validator = self.create_extended_type_validator(compiler);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let info_base_type = info.unwrap().get_base_type().unwrap().evaluate(
                    reg,
                    ast,
                    Some(self.template_scope.clone()),
                );
                let annotated = info_base_type.to_maybe_object_type(reg);
                if !Self::are_compatible_extends_types(reg, annotated, class_extends_type) {
                    self.is_known_ambiguous = true;
                }
                if info_base_type.set_validator(reg, ast, validator) {
                    self.base_type = info_base_type.to_object_type(reg);
                } else {
                    self.is_known_ambiguous = true;
                }
            } else {
                let fn_name = self.format_fn_name();
                self.report_warning(compiler, &EXTENDS_WITHOUT_TYPEDEF, &[&fn_name]);
                self.is_known_ambiguous = true;
            }
        } else if class_extends_type.is_some() && (self.is_constructor || self.is_interface) {
            // This case is:
            // // no JSDoc here
            // class extends astBaseType {...}
            //
            // It may well be that astBaseType is something dynamically created, like a value passed
            // into a function. A common pattern is:
            //
            // function mixinX(superClass) {
            //   return class extends superClass {
            //     ...
            //   };
            // }
            // The ExtendedTypeValidator() used in the JSDocInfo case above will report errors for
            // these cases, and we don't want that.
            // Since astBaseType is an actual value in code rather than an annotation, we can
            // rely on validation elsewhere to ensure it is actually defined.
            self.base_type = class_extends_type;
        }

        // Implemented interfaces (for constructors only).
        if info.is_some() && info.unwrap().get_implemented_interface_count() > 0 {
            if self.is_constructor {
                self.implemented_interfaces = Some(Vec::new());
                for t in info.unwrap().get_implemented_interfaces() {
                    let validator = self.create_implemented_type_validator(compiler);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let maybe_inter_type = t.evaluate(reg, ast, Some(self.template_scope.clone()));

                    if maybe_inter_type.set_validator(reg, ast, validator) {
                        self.implemented_interfaces
                            .as_mut()
                            .unwrap()
                            .push(maybe_inter_type);
                    }
                }
            } else if self.is_interface {
                let fn_name = self.format_fn_name();
                self.report_warning(
                    compiler,
                    &crate::type_check::CONFLICTING_IMPLEMENTED_TYPE,
                    &[&fn_name],
                );
            } else {
                let fn_name = self.format_fn_name();
                self.report_warning(compiler, &CONSTRUCTOR_REQUIRED, &["@implements", &fn_name]);
            }
        }

        // extended interfaces (for interfaces only)
        // We've already emitted a warning if this is not an interface.
        if self.is_interface {
            self.extended_interfaces = Some(Vec::new());
            if let Some(info) = info {
                for t in info.get_extended_interfaces() {
                    let validator = self.create_extended_type_validator(compiler);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let maybe_interface_type =
                        t.evaluate(reg, ast, Some(self.template_scope.clone()));
                    // (Java checks `maybeInterfaceType != null`; evaluate always returns a type.)
                    {
                        // setValidator runs validation and returns whether validation was
                        // successful (except for not-yet resolved named types, where validation is
                        // delayed).
                        // This code must run even for non-object types (which we know are invalid)
                        // to generate and record the user error message.
                        let is_valid = maybe_interface_type.set_validator(reg, ast, validator);
                        // ExtendedTypeValidator guarantees that maybeInterfaceType is an object
                        // type, but setValidator might not (e.g. due to delayed execution).
                        if is_valid && maybe_interface_type.to_maybe_object_type(reg).is_some() {
                            self.extended_interfaces
                                .as_mut()
                                .unwrap()
                                .push(maybe_interface_type.to_maybe_object_type(reg).unwrap());
                        }
                    }
                    // de-dupe baseType (from extends keyword) if it's also in @extends jsdoc.
                    if class_extends_type.is_some()
                        && maybe_interface_type.is_subtype_of(reg, ast, class_extends_type.unwrap())
                    {
                        class_extends_type = None;
                    }
                }
            }
            if class_extends_type.is_some() && {
                let validator = self.create_extended_type_validator(compiler);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                class_extends_type
                    .unwrap()
                    .set_validator(reg, ast, validator)
            } {
                // case is:
                // /**
                //  * @interface
                //  * @extends {OtherInterface}
                //  */
                // class SomeInterface extends astBaseType {}
                // Add the explicit extends type to the extended interfaces listed in JSDoc.
                self.extended_interfaces
                    .as_mut()
                    .unwrap()
                    .push(class_extends_type.unwrap());
            }
        }

        self
    }

    /// Decide if the types in the @extends annotation and the extends clause of a class are a
    /// "sensible" combination.
    ///
    /// Sensible is vague here, depending on how dynamic we allow types to be. It's not just a
    /// supertype/subtype check. Generally, we want to trust user annotations as declarations of
    /// intent, but we also want to protect users from dangerous lies. UNKNOWN as one of the types
    /// is a particularly uncertain case.
    // port: FunctionTypeBuilder#areCompatibleExtendsTypes
    fn are_compatible_extends_types(
        reg: &JSTypeRegistry,
        annotated: Option<TypeId>,
        extends_clause: Option<TypeId>,
    ) -> bool {
        // identical(): TypeId equality is Java object identity.
        if extends_clause.is_none() || annotated == extends_clause {
            return true;
        }

        // Allow `/** @extends {Foo<T>} */ class Bar extends Foo`
        let annotated = annotated.unwrap();
        annotated.is_templatized_type(reg)
            && Some(TemplatizedType::get_referenced_type(
                annotated.to_maybe_templatized_type(reg).unwrap(),
                reg,
            )) == extends_clause
    }

    /// Infers the type of {@code this}.
    ///
    /// @param type The type of this if the info is missing.
    // port: FunctionTypeBuilder#inferThisType(JSDocInfo,JSType)
    pub fn infer_this_type_with_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<&JSDocInfo>,
        type_: Option<TypeId>,
    ) -> &mut Self {
        // Look at the @this annotation first.
        self.infer_this_type(compiler, info);

        if self.this_type.is_none() {
            let obj_type = {
                let (reg, _) = compiler.get_type_registry_and_ast();
                object_type::cast(reg, type_)
            };
            if obj_type.is_some() && info.is_none_or(|info| !info.has_type()) {
                self.this_type = obj_type;
            }
        }

        self
    }

    /// Infers the type of {@code this}.
    ///
    /// @param info The JSDocInfo for this function.
    // port: FunctionTypeBuilder#inferThisType(JSDocInfo)
    pub fn infer_this_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<&JSDocInfo>,
    ) -> &mut Self {
        if info.is_some() && info.unwrap().has_this_type() {
            // TODO(johnlenz): In ES5 strict mode a function can have a null or
            // undefined "this" value, but all the existing "@this" annotations
            // don't declare restricted types.
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let maybe_this_type = info
                .unwrap()
                .get_this_type()
                .unwrap()
                .evaluate(reg, ast, Some(self.template_scope.clone()))
                .restrict_by_not_null_or_undefined(reg, ast);
            // (Java checks `maybeThisType != null`; a TypeId is never null.)
            self.this_type = Some(maybe_this_type);
        }

        self
    }

    /// Infer the parameter types from the doc info alone.
    // port: FunctionTypeBuilder#inferParameterTypes(JSDocInfo)
    pub fn infer_parameter_types_from_info(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: &JSDocInfo,
    ) -> &mut Self {
        // Create a fake args parent.
        let lp = IR::param_list(compiler, &[]);
        for name in info.get_parameter_names() {
            let name_node = IR::name(compiler, name);
            lp.add_child_to_back(compiler, name_node);
        }

        self.infer_parameter_types(compiler, Some(lp), Some(info))
    }

    /// Infer the parameter types from the list of parameter names and the JSDoc info.
    // port: FunctionTypeBuilder#inferParameterTypes(Node,JSDocInfo)
    pub fn infer_parameter_types(
        &mut self,
        compiler: &mut AbstractCompiler,
        params_parent: Option<NodeId>,
        info: Option<&JSDocInfo>,
    ) -> &mut Self {
        let Some(params_parent) = params_parent else {
            return match info {
                None => self,
                Some(info) => self.infer_parameter_types_from_info(compiler, info),
            };
        };

        // arguments
        let mut old_parameters: std::vec::IntoIter<Parameter>;
        let mut old_parameter_type: Option<Parameter> = None;
        if let Some(parameters) = &self.parameters {
            old_parameters = parameters.clone().into_iter();
            old_parameter_type = old_parameters.next();
        } else {
            old_parameters = Vec::new().into_iter();
        }

        let mut builder = FunctionParamBuilder::new();
        let mut warned_about_arg_list = false;
        let mut all_js_doc_params: IndexSet<JsString> = match info {
            None => IndexSet::<_>::default(),
            Some(info) => info.get_parameter_names(),
        };
        let mut is_var_args = false;
        let mut param_index: i32 = 0;
        let mut param = params_parent.get_first_child(compiler);
        while let Some(p) = param {
            let mut is_optional_param = false;
            let param_lhs: NodeId;

            if p.is_rest(compiler) {
                is_var_args = true;
                param_lhs = p.get_only_child(compiler);
            } else if p.is_default_value(compiler) {
                // The first child is the actual positional parameter
                param_lhs = check_not_null!(p.get_first_child(compiler), &p.to_string(compiler));
                is_optional_param = true;
            } else {
                is_var_args = self.is_var_args_parameter_by_convention(compiler, p);
                is_optional_param = self.is_optional_parameter_by_convention(compiler, p);
                param_lhs = p;
            }

            let mut param_name: Option<JsString> = None;
            if param_lhs.is_name(compiler) {
                param_name = Some(param_lhs.get_string(compiler));
            } else {
                check_state!(param_lhs.is_destructuring_pattern(compiler));
                // Right now, the only way to match a JSDoc param to a destructuring parameter is
                // through ordering the JSDoc parameters. So the third formal parameter will
                // correspond to the third JSDoc parameter.
                if let Some(info) = info {
                    param_name = info.get_parameter_name_at(param_index);
                }
            }
            if let Some(param_name) = &param_name {
                all_js_doc_params.shift_remove(param_name);
            }

            // type from JSDocInfo
            let parameter_type: TypeId;
            // (info.hasParameterType(null) is false in Java.)
            let has_parameter_type = match (info, &param_name) {
                (Some(info), Some(param_name)) => info.has_parameter_type(param_name.clone()),
                _ => false,
            };
            let param_lhs_info = param_lhs.get_jsdoc_info(compiler);
            if has_parameter_type {
                let parameter_type_expression = info
                    .unwrap()
                    .get_parameter_type(param_name.clone().unwrap())
                    .unwrap();
                let (reg, ast) = compiler.get_type_registry_and_ast();
                parameter_type =
                    parameter_type_expression.evaluate(reg, ast, Some(self.template_scope.clone()));
                is_optional_param =
                    is_optional_param || parameter_type_expression.is_optional_arg(ast);
                is_var_args = is_var_args || parameter_type_expression.is_var_args(ast);
            } else if param_lhs_info.is_some() && param_lhs_info.as_ref().unwrap().has_type() {
                let parameter_type_expression = param_lhs_info.unwrap().get_type().unwrap();
                let (reg, ast) = compiler.get_type_registry_and_ast();
                parameter_type =
                    parameter_type_expression.evaluate(reg, ast, Some(self.template_scope.clone()));
                is_optional_param = parameter_type_expression.is_optional_arg(ast);
                is_var_args = parameter_type_expression.is_var_args(ast);
            } else if let Some(old_parameter_type) = &old_parameter_type {
                // (Java also checks `oldParameterType.getJSType() != null`; a TypeId is never
                // null.)
                parameter_type = old_parameter_type.get_jstype();
                is_optional_param = old_parameter_type.is_optional();
                is_var_args = old_parameter_type.is_variadic();
            } else {
                let (reg, _) = compiler.get_type_registry_and_ast();
                parameter_type = reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
            }

            warned_about_arg_list |= self.add_parameter(
                compiler,
                &mut builder,
                parameter_type,
                warned_about_arg_list,
                is_optional_param,
                is_var_args,
            );

            old_parameter_type = old_parameters.next();
            param_index += 1;
            param = p.get_next(compiler);
        }
        // Copy over any old parameters that aren't in the param list.
        if !is_var_args {
            while old_parameter_type.is_some() && !is_var_args {
                builder.new_parameter_from(old_parameter_type.unwrap());
                old_parameter_type = old_parameters.next();
            }
        }

        for inexistent_name in &all_js_doc_params {
            let fn_name = self.format_fn_name();
            self.report_warning(
                compiler,
                &INEXISTENT_PARAM,
                &[&inexistent_name.to_string(), &fn_name],
            );
        }

        self.parameters = Some(builder.build());
        self
    }

    /// Register the template keys in a template scope and on the function node.
    // port: FunctionTypeBuilder#registerTemplates
    fn register_templates(
        &mut self,
        compiler: &mut AbstractCompiler,
        templates: &[TypeId],
        scope_root: Option<NodeId>,
    ) {
        if !templates.is_empty() {
            let (reg, _) = compiler.get_type_registry_and_ast();
            // Add any templates from JSDoc into our template scope.
            self.template_scope = reg.create_scope_with_templates(
                self.template_scope.clone(),
                templates.iter().copied(),
            );
            // Register the template types on the scope root node, if there is one.
            if let Some(scope_root) = scope_root {
                reg.register_template_type_names_in_scope(templates.iter().copied(), scope_root);
            }
        }
    }

    /// Infer parameters from the params list and info. Also maybe add extra templates.
    // port: FunctionTypeBuilder#inferConstructorParameters
    pub fn infer_constructor_parameters(
        &mut self,
        compiler: &mut AbstractCompiler,
        args_parent: NodeId,
        info: Option<&JSDocInfo>,
    ) -> &mut Self {
        // Look for template parameters in 'info': these will be added to anything from the class.
        if let Some(info) = info {
            let templates = self.build_template_types_from_js_doc_info(compiler, info, true);
            let ctor = args_parent.get_parent(compiler);
            self.set_constructor_template_type_names(compiler, templates, ctor);
        }

        self.infer_parameter_types(compiler, Some(args_parent), info);

        self
    }

    // port: FunctionTypeBuilder#inferImplicitConstructorParameters
    pub fn infer_implicit_constructor_parameters(
        &mut self,
        parameters: Vec<Parameter>,
    ) -> &mut Self {
        self.parameters = Some(parameters);
        self
    }

    // port: FunctionTypeBuilder#inferClosurePrimitive
    pub fn infer_closure_primitive(&mut self, info: Option<&JSDocInfo>) -> &mut Self {
        if info.is_some() && info.unwrap().has_closure_primitive_id() {
            let id = info
                .unwrap()
                .get_closure_primitive_id()
                .map(|id| id.to_string_lossy());
            self.closure_primitive_id = ClosurePrimitive::from_string_id(id.as_deref());
        }
        self
    }

    // port: FunctionTypeBuilder#setConstructorTemplateTypeNames
    fn set_constructor_template_type_names(
        &mut self,
        compiler: &mut AbstractCompiler,
        templates: Vec<TypeId>,
        ctor: Option<NodeId>,
    ) {
        if !templates.is_empty() {
            self.constructor_template_type_names = templates.clone();
            self.template_type_names = if self.template_type_names.is_empty() {
                templates.clone()
            } else {
                let mut builder = self.template_type_names.clone();
                builder.extend(self.constructor_template_type_names.iter().copied());
                builder
            };
            self.register_templates(compiler, &templates, ctor);
        }
    }

    /// @return Whether the given param is an optional param.
    // port: FunctionTypeBuilder#isOptionalParameterByConvention
    fn is_optional_parameter_by_convention(
        &self,
        compiler: &AbstractCompiler,
        param: NodeId,
    ) -> bool {
        if param.is_destructuring_pattern(compiler) {
            return false;
        }
        compiler
            .get_coding_convention()
            .is_optional_parameter(compiler, param)
    }

    /// Determine whether this is a var args parameter.
    ///
    /// @return Whether the given param is a var args param.
    // port: FunctionTypeBuilder#isVarArgsParameterByConvention
    fn is_var_args_parameter_by_convention(
        &self,
        compiler: &AbstractCompiler,
        param: NodeId,
    ) -> bool {
        if param.is_destructuring_pattern(compiler) {
            return false;
        }

        compiler
            .get_coding_convention()
            .is_var_args_parameter(compiler, param)
    }

    // port: FunctionTypeBuilder#buildTemplateTypesFromJSDocInfo
    fn build_template_types_from_js_doc_info(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: &JSDocInfo,
        allow_type_transformations: bool,
    ) -> Vec<TypeId> {
        let info_type_keys = info.get_template_types();
        let info_type_transformations = info.get_type_transformations();
        if info_type_keys.is_empty() && info_type_transformations.is_empty() {
            return Vec::new();
        }

        // Temporarily bootstrap the template environment with unbound (unknown bound) template
        // types
        let mut unbounded_templates = Vec::new();
        {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            for template_key in info_type_keys.keys() {
                unbounded_templates.push(reg.create_template_type(ast, template_key.clone()));
            }
            self.template_scope = reg.create_scope_with_templates(
                self.template_scope.clone(),
                unbounded_templates.iter().copied(),
            );
        }

        // Evaluate template type bounds with bootstrapped environment and reroute the bounds to
        // these
        let mut templates: Vec<TypeId> = Vec::new();
        // LinkedHashMap<TemplateType, JSType>: each key is a distinct TemplateType (one per
        // template name), so Java's equals/hashCode keying coincides with identity keying.
        let mut templates_to_bounds: IndexMap<TypeId, TypeId> = IndexMap::<_, _>::default();
        for (key, value) in &info_type_keys {
            // Template bounds are never null (JSDocInfo records IMPLICIT_TEMPLATE_BOUND).
            let expr = value.clone().unwrap();
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let type_bound =
                reg.evaluate_type_expression(ast, &expr, Some(self.template_scope.clone()));
            // It's an error to mark a template bound explicitly {?}. Unbounded templates have an
            // implicit unknown bound. Allowing explicit unknowns would make it more difficult to
            // stricten their treatment in the future, since "unknown" is currently used as a proxy
            // for "implicit".
            if expr.is_explicit_unknown_template_bound(ast) {
                let type_bound_string = type_bound.to_string(reg, ast);
                self.report_error(
                    compiler,
                    &TEMPLATE_TYPE_ILLEGAL_BOUND,
                    &[&type_bound_string, &key.to_string()],
                );
            }
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let template = reg
                .get_type(ast, Some(&*self.template_scope), key.clone())
                .unwrap()
                .to_maybe_template_type(reg);
            if let Some(template) = template {
                templates_to_bounds.insert(template, type_bound);
            } else {
                let template = reg.create_template_type_with_bound(ast, key.clone(), type_bound);
                templates_to_bounds.insert(template, type_bound);
            }
        }

        for (&template, &bound) in &templates_to_bounds {
            let (reg, _) = compiler.get_type_registry_and_ast();
            template.set_bound(reg, bound);
            templates.push(template);
        }

        for (key, &value) in &info_type_transformations {
            if allow_type_transformations {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                templates.push(reg.create_template_type_with_transformation(
                    ast,
                    key.clone(),
                    value,
                ));
            } else {
                self.report_warning(
                    compiler,
                    &TEMPLATE_TRANSFORMATION_ON_CLASS,
                    &[&key.to_string()],
                );
            }
        }

        let built_templates = templates;
        for &template in &built_templates {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if template.contains_cycle(reg, ast) {
                let reference_name = template
                    .get_reference_name(reg)
                    .map_or_else(|| "null".to_string(), |name| name.to_string_lossy());
                self.report_error(
                    compiler,
                    &crate::rhino_error_reporter::CYCLIC_INHERITANCE_ERROR,
                    &[&format!(
                        "Cycle detected in inheritance chain of type {reference_name}"
                    )],
                );
            }
        }

        built_templates
    }

    /// Infer the template type from the doc info.
    // port: FunctionTypeBuilder#inferTemplateTypeName
    pub fn infer_template_type_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        info: Option<&JSDocInfo>,
        owner_type: Option<TypeId>,
    ) -> &mut Self {
        // NOTE: these template type names may override a list
        // of inherited ones from an overridden function.

        if info.is_some() && !self.maybe_use_native_class_template_names(compiler, info.unwrap()) {
            let templates = self.build_template_types_from_js_doc_info(
                compiler,
                info.unwrap(),
                !(self.is_constructor || self.is_interface),
            );
            if !templates.is_empty() {
                self.template_type_names = templates;
            }
        }

        let owner_type_keys = if let Some(owner_type) = owner_type {
            let (reg, _) = compiler.get_type_registry_and_ast();
            owner_type
                .get_template_type_map(reg)
                .get_template_keys()
                .to_vec()
        } else {
            Vec::new()
        };

        if !self.template_type_names.is_empty() || !owner_type_keys.is_empty() {
            // TODO(sdh): The order of these should be switched to avoid class templates shadowing
            // method templates, but this currently loosens type checking of arrays more than we'd
            // like. See http://github.com/google/closure-compiler/issues/2973
            let templates = self
                .template_type_names
                .iter()
                .copied()
                .chain(owner_type_keys)
                .collect::<Vec<_>>();
            let scope_root = self.contents.get_source_node();
            self.register_templates(compiler, &templates, scope_root);
        }

        self
    }

    /// Add a parameter to the param list.
    ///
    /// @param builder A builder.
    /// @param paramType The parameter type.
    /// @param warnedAboutArgList Whether we've already warned about arg ordering issues (like if
    ///     optional args appeared before required ones).
    /// @param isOptional Is this an optional parameter?
    /// @param isVarArgs Is this a var args parameter?
    /// @return Whether a warning was emitted.
    // port: FunctionTypeBuilder#addParameter
    fn add_parameter(
        &self,
        compiler: &mut AbstractCompiler,
        builder: &mut FunctionParamBuilder,
        param_type: TypeId,
        warned_about_arg_list: bool,
        is_optional: bool,
        is_var_args: bool,
    ) -> bool {
        let mut emitted_warning = false;
        if is_optional {
            // Remembering that an optional parameter has been encountered
            // so that if a non optional param is encountered later, an
            // error can be reported.
            let added = {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                builder.add_optional_params(reg, ast, &[param_type])
            };
            if !added && !warned_about_arg_list {
                self.report_warning(compiler, &VAR_ARGS_MUST_BE_LAST, &[]);
                emitted_warning = true;
            }
        } else if is_var_args {
            if !builder.add_var_args(param_type) && !warned_about_arg_list {
                self.report_warning(compiler, &VAR_ARGS_MUST_BE_LAST, &[]);
                emitted_warning = true;
            }
        } else {
            if !builder.add_required_params(&[param_type]) && !warned_about_arg_list {
                // An optional parameter was seen and this argument is not an optional
                // or var arg so it is an error.
                if builder.has_var_args() {
                    self.report_warning(compiler, &VAR_ARGS_MUST_BE_LAST, &[]);
                } else {
                    self.report_warning(compiler, &OPTIONAL_ARG_AT_END, &[]);
                }
                emitted_warning = true;
            }
        }
        emitted_warning
    }

    /// Sets the returnType for this function using very basic type inference.
    // port: FunctionTypeBuilder#provideDefaultReturnType
    fn provide_default_return_type(&mut self, compiler: &mut AbstractCompiler) {
        if self.contents.get_source_node().is_some()
            && self
                .contents
                .get_source_node()
                .unwrap()
                .is_async_generator_function(compiler)
        {
            // Set the return type of a generator function to:
            //   @return {!AsyncGenerator<?>}
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let generator_type = reg.get_native_object_type(JSTypeNative::ASYNC_GENERATOR_TYPE);
            let unknown_type = reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
            self.return_type =
                Some(reg.create_templatized_type(ast, generator_type, &[unknown_type]));
            return;
        } else if self.contents.get_source_node().is_some()
            && self
                .contents
                .get_source_node()
                .unwrap()
                .is_generator_function(compiler)
        {
            // Set the return type of a generator function to:
            //   @return {!Generator<?>}
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let generator_type = reg.get_native_object_type(JSTypeNative::GENERATOR_TYPE);
            let unknown_type = reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
            self.return_type =
                Some(reg.create_templatized_type(ast, generator_type, &[unknown_type]));
            return;
        }

        let mut inferred_return_type = {
            let (reg, _) = compiler.get_type_registry_and_ast();
            reg.get_native_type(JSTypeNative::UNKNOWN_TYPE)
        };
        if !self.contents.may_have_non_empty_returns()
            && !self.contents.may_have_single_throw(compiler)
            && !self.contents.may_be_from_externs(compiler)
        {
            // Infer return types for non-generator functions.
            // We need to be extremely conservative about this, because of two
            // competing needs.
            // 1) If we infer the return type of f too widely, then we won't be able
            //    to assign f to other functions.
            // 2) If we infer the return type of f too narrowly, then we won't be
            //    able to override f in subclasses.
            // So we only infer in cases where the user doesn't expect to write
            // @return annotations--when it's very obvious that the function returns
            // nothing.
            let (reg, _) = compiler.get_type_registry_and_ast();
            inferred_return_type = reg.get_native_type(JSTypeNative::VOID_TYPE);
            self.return_type_inferred = true;
        }

        if self.contents.get_source_node().is_some()
            && self
                .contents
                .get_source_node()
                .unwrap()
                .is_async_function(compiler)
        {
            // Set the return type of an async function:
            //   @return {!Promise<?>} or @return {!Promise<undefined>}
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let promise_type = reg.get_native_object_type(JSTypeNative::PROMISE_TYPE);
            self.return_type =
                Some(reg.create_templatized_type(ast, promise_type, &[inferred_return_type]));
        } else {
            self.return_type = Some(inferred_return_type);
        }
    }

    /// Builds the function type, and puts it in the registry.
    // port: FunctionTypeBuilder#buildAndRegister
    pub fn build_and_register(&mut self, compiler: &mut AbstractCompiler) -> TypeId {
        if self.return_type.is_none() {
            self.provide_default_return_type(compiler);
            check_not_null!(self.return_type);
        }

        if self.parameters.is_none() {
            panic!("IllegalStateException: All Function types must have params and a return type");
        }

        let fn_type;
        if self.is_constructor {
            fn_type = self.get_or_create_constructor(compiler);
        } else if self.is_interface {
            fn_type = self.get_or_create_interface(compiler);
        } else {
            let builder = self.create_default_builder(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            fn_type = builder
                .with_parameters(self.parameters.clone())
                .with_return_type_inferred(self.return_type, self.return_type_inferred)
                .with_type_of_this(self.this_type)
                .with_is_abstract(self.is_abstract)
                .with_closure_primitive_id(self.closure_primitive_id)
                .build(reg, ast);
            self.maybe_set_base_type(compiler, fn_type);
        }

        let (reg, ast) = compiler.get_type_registry_and_ast();
        if self.implemented_interfaces.is_some() && fn_type.is_constructor(reg) {
            fn_type.set_implemented_interfaces(
                reg,
                ast,
                self.implemented_interfaces.clone().unwrap(),
            );
        }

        if self.extended_interfaces.is_some() {
            fn_type.set_extended_interfaces(reg, ast, self.extended_interfaces.clone().unwrap());
        }

        if self.is_record {
            fn_type.set_implicit_match(reg, true);
        }

        fn_type
    }

    // port: FunctionTypeBuilder#maybeSetBaseType
    fn maybe_set_base_type(&self, compiler: &mut AbstractCompiler, fn_type: TypeId) {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !fn_type.has_instance_type(reg) || self.base_type.is_none() {
            return;
        }

        fn_type.set_prototype_based_on(reg, ast, self.base_type.unwrap());
        fn_type
            .get_instance_type(reg)
            .unwrap()
            .merge_supertype_template_types(reg, ast, self.base_type.unwrap());
    }

    // port: FunctionTypeBuilder#createDefaultBuilder
    fn create_default_builder(&self, compiler: &mut AbstractCompiler) -> function_type::Builder {
        let builder = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            function_type::builder()
                .with_name(self.fn_name.clone())
                .with_source_node(self.contents.get_source_node())
                .with_template_keys(reg, ast, self.template_type_names.clone())
                .set_is_known_ambiguous(self.is_known_ambiguous)
        };
        builder.set_goog_module_id(TypedScopeCreator::containing_goog_module_id_of(
            compiler,
            self.declaration_scope.expect("NullPointerException"),
        ))
    }

    /// Returns a constructor function either by returning it from the registry if it exists or
    /// creating and registering a new type. If there is already a type, then warn if the existing
    /// type is different than the one we are creating, though still return the existing function
    /// if possible. The primary purpose of this is that registering a constructor will fail for all
    /// built-in types that are initialized in {@link JSTypeRegistry}. We a) want to make sure that
    /// the type information specified in the externs file matches what is in the registry and b)
    /// annotate the externs with the {@link JSType} from the registry so that there are not two
    /// separate JSType objects for one type.
    // port: FunctionTypeBuilder#getOrCreateConstructor
    fn get_or_create_constructor(&mut self, compiler: &mut AbstractCompiler) -> TypeId {
        let builder = self.create_default_builder(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let fn_type = builder
            .for_constructor()
            .with_parameters(self.parameters.clone())
            .with_return_type(self.return_type)
            .with_constructor_template_keys(self.constructor_template_type_names.iter().copied())
            .with_is_abstract(self.is_abstract)
            .build(reg, ast);

        if self.makes_structs {
            fn_type.set_struct(reg);
        } else if self.makes_dicts {
            fn_type.set_dict(reg);
        } else if self.makes_unrestricted {
            fn_type.set_explicit_unrestricted(reg);
        }

        // There are two cases where this type already exists in the current scope:
        //   1. The type is a built-in that we initialized in JSTypeRegistry and is also defined in
        //  externs.
        //   2. Cases like "class C {} C = class {}"
        // See https://github.com/google/closure-compiler/issues/2928 for some related bugs.
        let declaration_scope = self
            .declaration_scope
            .map(|scope| scope.as_static_typed_scope_arc(compiler));
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let existing_type = reg.get_type(ast, declaration_scope.as_deref(), self.fn_name.clone());
        if let Some(existing_type) = existing_type {
            let is_instance_object = existing_type.is_instance_type(reg);
            if is_instance_object || self.fn_name == "Function" {
                let existing_fn = if is_instance_object {
                    existing_type
                        .to_object_type(reg)
                        .unwrap()
                        .get_constructor(reg)
                        .expect("NullPointerException")
                } else {
                    reg.get_native_function_type(JSTypeNative::FUNCTION_FUNCTION_TYPE)
                };

                if FunctionType::get_source(existing_fn, reg).is_none() {
                    existing_fn.set_source(reg, self.contents.get_source_node());
                }

                if !existing_fn.has_equal_call_type(reg, ast, fn_type) {
                    let fn_type_string = JSType::to_string(fn_type, reg, ast);
                    let existing_fn_string = JSType::to_string(existing_fn, reg, ast);
                    let formatted_fn_name = self.format_fn_name();
                    self.report_warning(
                        compiler,
                        &TYPE_REDEFINITION,
                        &[&formatted_fn_name, &fn_type_string, &existing_fn_string],
                    );
                }

                return existing_fn;
            } else {
                // We fall through and return the created type, even though it will fail
                // to register. We have no choice as we have to return a function. We
                // issue an error elsewhere though, so the user should fix it.
            }
        }

        self.maybe_set_base_type(compiler, fn_type);

        // TODO(johnlenz): determine what we are supposed to do for:
        //   @constructor
        //   this.Foo = ...
        //
        let syntactic_fn_name = self
            .syntactic_fn_name
            .clone()
            .expect("NullPointerException");
        if !syntactic_fn_name.is_empty() && !syntactic_fn_name.starts_with("this.") {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let instance_type = fn_type.get_instance_type(reg).unwrap();
            reg.declare_type_for_exact_scope(
                ast,
                declaration_scope.as_deref(),
                syntactic_fn_name,
                instance_type,
            );
        }
        fn_type
    }

    // port: FunctionTypeBuilder#getOrCreateInterface
    fn get_or_create_interface(&mut self, compiler: &mut AbstractCompiler) -> TypeId {
        let mut fn_type = None;

        let declaration_scope = self
            .declaration_scope
            .map(|scope| scope.as_static_typed_scope_arc(compiler));
        let syntactic_fn_name = self
            .syntactic_fn_name
            .clone()
            .expect("NullPointerException");
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let type_ = reg.get_type(ast, declaration_scope.as_deref(), syntactic_fn_name.clone());
        if type_.is_some() && type_.unwrap().is_instance_type(reg) {
            let ctor = type_
                .unwrap()
                .to_maybe_object_type(reg)
                .unwrap()
                .get_constructor(reg)
                .expect("NullPointerException");
            if ctor.is_interface(reg) {
                fn_type = Some(ctor);
                ctor.set_source(reg, self.contents.get_source_node());
            }
        }

        if fn_type.is_none() {
            let builder = self.create_default_builder(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let new_fn_type = builder.for_interface().with_no_parameters().build(reg, ast);
            fn_type = Some(new_fn_type);
            if self.makes_structs {
                new_fn_type.set_struct(reg);
            }
            if !self.fn_name.is_empty() {
                let instance_type = new_fn_type.get_instance_type(reg).unwrap();
                reg.declare_type_for_exact_scope(
                    ast,
                    declaration_scope.as_deref(),
                    syntactic_fn_name,
                    instance_type,
                );
            }
            self.maybe_set_base_type(compiler, new_fn_type);
        }
        fn_type.unwrap()
    }

    // port: FunctionTypeBuilder#reportWarning
    fn report_warning(
        &self,
        compiler: &mut AbstractCompiler,
        warning: &'static DiagnosticType,
        args: &[&str],
    ) {
        compiler.report(JSError::make(compiler, self.error_root, warning, args));
    }

    // port: FunctionTypeBuilder#reportError
    fn report_error(
        &self,
        compiler: &mut AbstractCompiler,
        error: &'static DiagnosticType,
        args: &[&str],
    ) {
        compiler.report(JSError::make(compiler, self.error_root, error, args));
    }

    /// Determines whether the given JsDoc info declares a function type.
    // port: FunctionTypeBuilder#isFunctionTypeDeclaration
    pub fn is_function_type_declaration(info: &JSDocInfo) -> bool {
        info.get_parameter_count() > 0
            || info.has_return_type()
            || info.has_this_type()
            || info.is_constructor()
            || info.is_interface()
            || info.is_abstract()
    }

    /// Check whether a type is resolvable in the future If this has a supertype that hasn't been
    /// resolved yet, then we can assume this type will be OK once the super type resolves.
    ///
    /// @return true if objectType is resolvable in the future
    // port: FunctionTypeBuilder#hasMoreTagsToResolve
    pub fn has_more_tags_to_resolve(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        object_type: TypeId,
    ) -> bool {
        check_argument!(object_type.is_unknown_type(reg, ast));
        let ctor = object_type.get_constructor(reg);
        if let Some(ctor) = ctor {
            // interface extends interfaces
            for interface_type in ctor.get_extended_interfaces(reg) {
                if !interface_type.is_resolved(reg) {
                    return true;
                }
            }
        }
        if object_type.get_implicit_prototype(reg, ast).is_some() {
            // constructor extends class
            return !object_type
                .get_implicit_prototype(reg, ast)
                .unwrap()
                .is_resolved(reg);
        }
        false
    }
}

/// Holds data dynamically inferred about functions.
// port: FunctionTypeBuilder.FunctionContents
pub trait FunctionContents: Send + Sync {
    /// Returns the source node of this function. May be null.
    // port: FunctionTypeBuilder.FunctionContents#getSourceNode
    fn get_source_node(&self) -> Option<NodeId>;

    /// Returns if the function may be in externs.
    // port: FunctionTypeBuilder.FunctionContents#mayBeFromExterns
    fn may_be_from_externs(&self, ast: &Ast) -> bool;

    /// Returns if a return of a real value (not undefined) appears.
    // port: FunctionTypeBuilder.FunctionContents#mayHaveNonEmptyReturns
    fn may_have_non_empty_returns(&self) -> bool;

    /// Returns if this consists of a single throw.
    // port: FunctionTypeBuilder.FunctionContents#mayHaveSingleThrow
    fn may_have_single_throw(&self, ast: &Ast) -> bool;
}

// port: FunctionTypeBuilder.UnknownFunctionContents
#[derive(Debug)]
pub struct UnknownFunctionContents;

// port: FunctionTypeBuilder.UnknownFunctionContents#singleton
static UNKNOWN_FUNCTION_CONTENTS_SINGLETON: LazyLock<Arc<UnknownFunctionContents>> =
    LazyLock::new(|| Arc::new(UnknownFunctionContents));

impl UnknownFunctionContents {
    // port: FunctionTypeBuilder.UnknownFunctionContents#get
    pub fn get() -> Arc<dyn FunctionContents> {
        UNKNOWN_FUNCTION_CONTENTS_SINGLETON.clone()
    }
}

impl FunctionContents for UnknownFunctionContents {
    // port: FunctionTypeBuilder.UnknownFunctionContents#getSourceNode
    fn get_source_node(&self) -> Option<NodeId> {
        None
    }

    // port: FunctionTypeBuilder.UnknownFunctionContents#mayBeFromExterns
    fn may_be_from_externs(&self, _ast: &Ast) -> bool {
        true
    }

    // port: FunctionTypeBuilder.UnknownFunctionContents#mayHaveNonEmptyReturns
    fn may_have_non_empty_returns(&self) -> bool {
        true
    }

    // port: FunctionTypeBuilder.UnknownFunctionContents#mayHaveSingleThrow
    fn may_have_single_throw(&self, _ast: &Ast) -> bool {
        true
    }
}

// port: FunctionTypeBuilder.AstFunctionContents
#[derive(Clone, Debug)]
pub struct AstFunctionContents {
    n: NodeId,
    has_non_empty_returns: bool,
}

impl AstFunctionContents {
    // port: FunctionTypeBuilder.AstFunctionContents#AstFunctionContents
    pub fn new(n: NodeId) -> Self {
        Self {
            n,
            has_non_empty_returns: false,
        }
    }

    // port: FunctionTypeBuilder.AstFunctionContents#recordNonEmptyReturn
    pub fn record_non_empty_return(&mut self) {
        self.has_non_empty_returns = true;
    }
}

impl FunctionContents for AstFunctionContents {
    // port: FunctionTypeBuilder.AstFunctionContents#getSourceNode
    fn get_source_node(&self) -> Option<NodeId> {
        Some(self.n)
    }

    // port: FunctionTypeBuilder.AstFunctionContents#mayBeFromExterns
    fn may_be_from_externs(&self, ast: &Ast) -> bool {
        self.n.is_from_externs(ast)
    }

    // port: FunctionTypeBuilder.AstFunctionContents#mayHaveNonEmptyReturns
    fn may_have_non_empty_returns(&self) -> bool {
        self.has_non_empty_returns
    }

    // port: FunctionTypeBuilder.AstFunctionContents#mayHaveSingleThrow
    fn may_have_single_throw(&self, ast: &Ast) -> bool {
        let block = self.n.get_last_child(ast).unwrap();
        block.has_one_child(ast) && block.get_first_child(ast).unwrap().is_throw(ast)
    }
}
