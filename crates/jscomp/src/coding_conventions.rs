/*
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
//   src/com/google/javascript/jscomp/CodingConventions.java.

// Preserve the Java nested condition flow.
#![allow(clippy::collapsible_if)]
use crate::{coding_convention::*, node_util::NodeUtil};
use closure_jstype::{
    prelude::{FunctionType, JSType, JSTypeRegistry},
    rhino::nominal_type_builder::NominalTypeBuilder,
};
use closure_rhino::{
    closure_primitive::ClosurePrimitive,
    js_string::JsString,
    jstype::TypeId,
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
};
use std::sync::LazyLock;
use std::{fmt::Debug, sync::Arc};
pub struct CodingConventions;
static DEFAULT: LazyLock<Arc<dyn CodingConvention>> =
    LazyLock::new(|| Arc::new(DefaultCodingConvention));
impl CodingConventions {
    // port: CodingConventions#CodingConventions
    #[allow(dead_code)] // Java private utility-class constructor.
    fn new() -> Self {
        Self
    }
    // port: CodingConventions#getDefault
    pub fn get_default() -> Arc<dyn CodingConvention> {
        DEFAULT.clone()
    }
    // port: CodingConventions#defaultIsFunctionCallThatAlwaysThrows
    pub fn default_is_function_call_that_always_throws(
        ast: &Ast,
        mut n: NodeId,
        always_throws_function_name: &JsString,
    ) -> bool {
        if n.is_expr_result(ast) {
            if !n.get_first_child(ast).unwrap().is_call(ast) {
                return false;
            }
        } else if !n.is_call(ast) {
            return false;
        }
        if n.is_expr_result(ast) {
            n = n.get_first_child(ast).unwrap();
        }
        n.get_first_child(ast)
            .unwrap()
            .matches_qualified_name(ast, always_throws_function_name)
    }
}
#[derive(Debug)]
pub struct Proxy {
    pub(crate) next_convention: Arc<dyn CodingConvention>,
}
impl Proxy {
    // port: CodingConventions.Proxy#Proxy
    pub fn new(convention: Arc<dyn CodingConvention>) -> Self {
        Self {
            next_convention: convention,
        }
    }
}
impl CodingConvention for Proxy {
    // port: CodingConventions.Proxy#isExported(String)
    fn is_exported_name(&self, name: &JsString) -> bool {
        self.is_exported(name, true) || self.is_exported(name, false)
    }
    // port: CodingConventions.Proxy#isConstant
    fn is_constant(&self, name: &JsString) -> bool {
        self.next_convention.is_constant(name)
    }
    // port: CodingConventions.Proxy#isConstantKey
    fn is_constant_key(&self, name: &JsString) -> bool {
        self.next_convention.is_constant_key(name)
    }
    // port: CodingConventions.Proxy#isValidEnumKey
    fn is_valid_enum_key(&self, key: Option<&JsString>) -> bool {
        self.next_convention.is_valid_enum_key(key)
    }
    // port: CodingConventions.Proxy#isOptionalParameter
    fn is_optional_parameter(&self, ast: &Ast, parameter: NodeId) -> bool {
        self.next_convention.is_optional_parameter(ast, parameter)
    }
    // port: CodingConventions.Proxy#isVarArgsParameter
    fn is_var_args_parameter(&self, ast: &Ast, parameter: NodeId) -> bool {
        self.next_convention.is_var_args_parameter(ast, parameter)
    }
    // port: CodingConventions.Proxy#isFunctionCallThatAlwaysThrows
    fn is_function_call_that_always_throws(
        &self,
        ast: &Ast,
        reg: &JSTypeRegistry,
        n: NodeId,
    ) -> bool {
        self.next_convention
            .is_function_call_that_always_throws(ast, reg, n)
    }
    // port: CodingConventions.Proxy#isExported
    fn is_exported(&self, name: &JsString, local: bool) -> bool {
        self.next_convention.is_exported(name, local)
    }
    // port: CodingConventions.Proxy#getPackageName
    fn get_package_name(&self, source: &dyn StaticSourceFile) -> Option<String> {
        self.next_convention.get_package_name(source)
    }
    // port: CodingConventions.Proxy#getClassesDefinedByCall
    fn get_classes_defined_by_call(
        &self,
        ast: &Ast,
        call_node: NodeId,
    ) -> Option<SubclassRelationship> {
        self.next_convention
            .get_classes_defined_by_call(ast, call_node)
    }
    // port: CodingConventions.Proxy#isClassFactoryCall
    fn is_class_factory_call(&self, ast: &Ast, call_node: NodeId) -> bool {
        self.next_convention.is_class_factory_call(ast, call_node)
    }
    // port: CodingConventions.Proxy#isSuperClassReference
    fn is_super_class_reference(&self, property_name: &JsString) -> bool {
        self.next_convention.is_super_class_reference(property_name)
    }
    // port: CodingConventions.Proxy#extractIsModuleFile
    fn extract_is_module_file(&self, ast: &Ast, node: NodeId, parent: NodeId) -> bool {
        self.next_convention
            .extract_is_module_file(ast, node, parent)
    }
    // port: CodingConventions.Proxy#extractClassNameIfProvide
    fn extract_class_name_if_provide(
        &self,
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
    ) -> Option<JsString> {
        self.next_convention
            .extract_class_name_if_provide(ast, node, parent)
    }
    // port: CodingConventions.Proxy#extractClassNameIfRequire
    fn extract_class_name_if_require(
        &self,
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
    ) -> Option<JsString> {
        self.next_convention
            .extract_class_name_if_require(ast, node, parent)
    }
    // port: CodingConventions.Proxy#getExportPropertyFunction
    fn get_export_property_function(&self) -> Option<JsString> {
        self.next_convention.get_export_property_function()
    }
    // port: CodingConventions.Proxy#getExportSymbolFunction
    fn get_export_symbol_function(&self) -> Option<JsString> {
        self.next_convention.get_export_symbol_function()
    }
    // port: CodingConventions.Proxy#identifyTypeDeclarationCall
    fn identify_type_declaration_call(&self, ast: &Ast, n: NodeId) -> Vec<JsString> {
        self.next_convention.identify_type_declaration_call(ast, n)
    }
    // port: CodingConventions.Proxy#applySubclassRelationship
    fn apply_subclass_relationship(
        &self,
        ast: &Ast,
        reg: &mut JSTypeRegistry,
        parent: &NominalTypeBuilder,
        child: &NominalTypeBuilder,
        type_: SubclassType,
    ) {
        self.next_convention
            .apply_subclass_relationship(ast, reg, parent, child, type_)
    }
    // port: CodingConventions.Proxy#getAbstractMethodName
    fn get_abstract_method_name(&self) -> Option<JsString> {
        self.next_convention.get_abstract_method_name()
    }
    // port: CodingConventions.Proxy#getSingletonGetterClassName
    fn get_singleton_getter_class_name(&self, ast: &Ast, call_node: NodeId) -> Option<JsString> {
        self.next_convention
            .get_singleton_getter_class_name(ast, call_node)
    }
    // port: CodingConventions.Proxy#applySingletonGetter
    fn apply_singleton_getter(
        &self,
        ast: &Ast,
        reg: &mut JSTypeRegistry,
        class_type: &NominalTypeBuilder,
        getter_type: TypeId,
    ) {
        self.next_convention
            .apply_singleton_getter(ast, reg, class_type, getter_type)
    }
    // port: CodingConventions.Proxy#describeFunctionBind
    fn describe_function_bind(
        &self,
        ast: &Ast,
        reg: Option<&mut JSTypeRegistry>,
        n: NodeId,
        check_types: bool,
    ) -> Option<Bind> {
        self.next_convention
            .describe_function_bind(ast, reg, n, check_types)
    }
    // port: CodingConventions.Proxy#describeCachingCall
    fn describe_caching_call(&self, ast: &Ast, node: NodeId) -> Option<Cache> {
        self.next_convention.describe_caching_call(ast, node)
    }
    // port: CodingConventions.Proxy#isPropertyTestFunction
    fn is_property_test_function(&self, ast: &Ast, call: NodeId) -> bool {
        self.next_convention.is_property_test_function(ast, call)
    }
    // port: CodingConventions.Proxy#isPrototypeAlias
    fn is_prototype_alias(&self, _ast: &Ast, _get_prop: NodeId) -> bool {
        false
    }
    // port: CodingConventions.Proxy#isPropertyRenameFunction
    fn is_property_rename_function(&self, ast: &Ast, name_node: NodeId) -> bool {
        self.next_convention
            .is_property_rename_function(ast, name_node)
    }
    // port: CodingConventions.Proxy#getObjectLiteralCast
    fn get_object_literal_cast(&self, ast: &Ast, call_node: NodeId) -> Option<ObjectLiteralCast> {
        self.next_convention.get_object_literal_cast(ast, call_node)
    }
    // port: CodingConventions.Proxy#getAssertionFunctions
    fn get_assertion_functions(&self) -> Vec<AssertionFunctionSpec> {
        self.next_convention.get_assertion_functions()
    }
}
#[derive(Debug)]
pub struct DefaultCodingConvention;
impl DefaultCodingConvention {
    pub const SERIAL_VERSION_UID: i64 = 1;
    // port: CodingConventions.DefaultCodingConvention#safeNext
    fn safe_next(ast: &Ast, n: Option<NodeId>) -> Option<NodeId> {
        n.and_then(|n| n.get_next(ast))
    }
}
impl CodingConvention for DefaultCodingConvention {
    // port: CodingConventions.DefaultCodingConvention#isExported(String)
    fn is_exported_name(&self, name: &JsString) -> bool {
        self.is_exported(name, true) || self.is_exported(name, false)
    }
    // port: CodingConventions.DefaultCodingConvention#isConstant
    fn is_constant(&self, _name: &JsString) -> bool {
        false
    }
    // port: CodingConventions.DefaultCodingConvention#isConstantKey
    fn is_constant_key(&self, _name: &JsString) -> bool {
        false
    }
    // port: CodingConventions.DefaultCodingConvention#isValidEnumKey
    fn is_valid_enum_key(&self, key: Option<&JsString>) -> bool {
        key.is_some_and(|key| !key.is_empty())
    }
    // port: CodingConventions.DefaultCodingConvention#isOptionalParameter
    fn is_optional_parameter(&self, _ast: &Ast, _parameter: NodeId) -> bool {
        false
    }
    // port: CodingConventions.DefaultCodingConvention#isVarArgsParameter
    fn is_var_args_parameter(&self, ast: &Ast, parameter: NodeId) -> bool {
        parameter.is_rest(ast)
    }
    // port: CodingConventions.DefaultCodingConvention#isFunctionCallThatAlwaysThrows
    fn is_function_call_that_always_throws(
        &self,
        ast: &Ast,
        reg: &JSTypeRegistry,
        n: NodeId,
    ) -> bool {
        if NodeUtil::is_expr_call(ast, n) {
            let fn_type = n
                .get_first_first_child(ast)
                .unwrap()
                .get_jstype(ast)
                .and_then(|t| t.to_maybe_function_type(reg));
            return fn_type.is_some_and(|f| {
                f.get_closure_primitive(reg) == Some(ClosurePrimitive::ASSERTS_FAIL)
            });
        }
        false
    }
    // port: CodingConventions.DefaultCodingConvention#isExported
    fn is_exported(&self, name: &JsString, local: bool) -> bool {
        local && name.starts_with("$super")
    }
    // port: CodingConventions.DefaultCodingConvention#getPackageName
    fn get_package_name(&self, source: &dyn StaticSourceFile) -> Option<String> {
        let name = source.get_name();
        Some(
            name.rfind('/')
                .map_or_else(String::new, |last_slash| name[..last_slash].into()),
        )
    }
    // port: CodingConventions.DefaultCodingConvention#getClassesDefinedByCall
    fn get_classes_defined_by_call(
        &self,
        ast: &Ast,
        call_node: NodeId,
    ) -> Option<SubclassRelationship> {
        let call_name = call_node.get_first_child(ast).unwrap();
        if (JSCOMP_INHERITS.matches(ast, call_name)
            || call_name.matches_name(ast, "$jscomp$inherits"))
            && call_node.has_x_children(ast, 3)
        {
            let subclass = call_name.get_next(ast).unwrap();
            let superclass = subclass.get_next(ast).unwrap();
            if subclass.is_qualified_name(ast) && superclass.is_qualified_name(ast) {
                return Some(SubclassRelationship::new(
                    ast,
                    SubclassType::INHERITS,
                    subclass,
                    superclass,
                ));
            }
        }
        None
    }
    // port: CodingConventions.DefaultCodingConvention#isClassFactoryCall
    fn is_class_factory_call(&self, _ast: &Ast, _call_node: NodeId) -> bool {
        false
    }
    // port: CodingConventions.DefaultCodingConvention#isSuperClassReference
    fn is_super_class_reference(&self, _property_name: &JsString) -> bool {
        false
    }
    // port: CodingConventions.DefaultCodingConvention#extractIsModuleFile
    fn extract_is_module_file(&self, _ast: &Ast, _node: NodeId, _parent: NodeId) -> bool {
        panic!("only implemented in ClosureCodingConvention")
    }
    // port: CodingConventions.DefaultCodingConvention#extractClassNameIfProvide
    fn extract_class_name_if_provide(
        &self,
        _ast: &Ast,
        _node: NodeId,
        _parent: NodeId,
    ) -> Option<JsString> {
        panic!("only implemented in ClosureCodingConvention")
    }
    // port: CodingConventions.DefaultCodingConvention#extractClassNameIfRequire
    fn extract_class_name_if_require(
        &self,
        _ast: &Ast,
        _node: NodeId,
        _parent: NodeId,
    ) -> Option<JsString> {
        panic!("only implemented in ClosureCodingConvention")
    }
    // port: CodingConventions.DefaultCodingConvention#getExportPropertyFunction
    fn get_export_property_function(&self) -> Option<JsString> {
        None
    }
    // port: CodingConventions.DefaultCodingConvention#getExportSymbolFunction
    fn get_export_symbol_function(&self) -> Option<JsString> {
        None
    }
    // port: CodingConventions.DefaultCodingConvention#identifyTypeDeclarationCall
    fn identify_type_declaration_call(&self, _ast: &Ast, _n: NodeId) -> Vec<JsString> {
        Vec::new()
    }
    // port: CodingConventions.DefaultCodingConvention#applySubclassRelationship
    fn apply_subclass_relationship(
        &self,
        _ast: &Ast,
        _reg: &mut JSTypeRegistry,
        _parent: &NominalTypeBuilder,
        _child: &NominalTypeBuilder,
        _type_: SubclassType,
    ) {
    }
    // port: CodingConventions.DefaultCodingConvention#getAbstractMethodName
    fn get_abstract_method_name(&self) -> Option<JsString> {
        None
    }
    // port: CodingConventions.DefaultCodingConvention#getSingletonGetterClassName
    fn get_singleton_getter_class_name(&self, _ast: &Ast, _call_node: NodeId) -> Option<JsString> {
        None
    }
    // port: CodingConventions.DefaultCodingConvention#applySingletonGetter
    fn apply_singleton_getter(
        &self,
        _ast: &Ast,
        _reg: &mut JSTypeRegistry,
        _class_type: &NominalTypeBuilder,
        _getter_type: TypeId,
    ) {
    }
    // port: CodingConventions.DefaultCodingConvention#describeFunctionBind
    fn describe_function_bind(
        &self,
        ast: &Ast,
        reg: Option<&mut JSTypeRegistry>,
        n: NodeId,
        check_types: bool,
    ) -> Option<Bind> {
        if !n.is_call(ast) {
            return None;
        }
        let call_target = n.get_first_child(ast).unwrap();
        if call_target.is_qualified_name(ast)
            && FUNCTION_PROTOTYPE_BIND_CALL.matches(ast, call_target)
        {
            let fn_ = call_target.get_next(ast)?;
            let this_value = Self::safe_next(ast, Some(fn_));
            let parameters = Self::safe_next(ast, this_value);
            return Some(Bind::new(fn_, this_value, parameters));
        }
        if call_target.is_get_prop(ast) && call_target.get_string_ref(ast) == "bind" {
            let maybe_fn = call_target.get_first_child(ast).unwrap();
            let maybe_fn_type = maybe_fn.get_jstype(ast);
            let mut fn_type = None;
            if check_types {
                if let Some(t) = maybe_fn_type {
                    // Rust-only: a JSType on a node implies the registry that created it.
                    let reg = reg.expect("a node JSType implies its JSTypeRegistry");
                    fn_type = t
                        .restrict_by_not_null_or_undefined(reg, ast)
                        .to_maybe_function_type(reg);
                }
            }
            if fn_type.is_some() || maybe_fn.is_function(ast) {
                let this_value = call_target.get_next(ast);
                let parameters = Self::safe_next(ast, this_value);
                return Some(Bind::new(maybe_fn, this_value, parameters));
            }
        }
        None
    }
    // port: CodingConventions.DefaultCodingConvention#describeCachingCall
    fn describe_caching_call(&self, _ast: &Ast, _node: NodeId) -> Option<Cache> {
        None
    }
    // port: CodingConventions.DefaultCodingConvention#isPropertyTestFunction
    fn is_property_test_function(&self, ast: &Ast, call: NodeId) -> bool {
        let target = call.get_first_child(ast).unwrap();
        if target.is_get_prop(ast) {
            let src = target.get_first_child(ast).unwrap();
            let prop = target.get_string(ast);
            if src.is_name(ast) && src.get_string_ref(ast) == "Array" && prop == "isArray" {
                return true;
            }
        }
        false
    }
    // port: CodingConventions.DefaultCodingConvention#isPrototypeAlias
    fn is_prototype_alias(&self, _ast: &Ast, _get_prop: NodeId) -> bool {
        false
    }
    // port: CodingConventions.DefaultCodingConvention#isPropertyRenameFunction
    fn is_property_rename_function(&self, ast: &Ast, name_node: NodeId) -> bool {
        name_node.matches_name(ast, NodeUtil::JSC_PROPERTY_NAME_FN)
    }
    // port: CodingConventions.DefaultCodingConvention#getObjectLiteralCast
    fn get_object_literal_cast(&self, _ast: &Ast, _call_node: NodeId) -> Option<ObjectLiteralCast> {
        None
    }
    // port: CodingConventions.DefaultCodingConvention#getAssertionFunctions
    fn get_assertion_functions(&self) -> Vec<AssertionFunctionSpec> {
        vec![
            AssertionFunctionSpec::for_truthy()
                .set_closure_primitive(ClosurePrimitive::ASSERTS_TRUTHY)
                .build(),
            AssertionFunctionSpec::for_matches_return()
                .set_closure_primitive(ClosurePrimitive::ASSERTS_MATCHES_RETURN)
                .build(),
        ]
    }
}

static JSCOMP_INHERITS: std::sync::LazyLock<closure_rhino::qualified_name::QualifiedName> =
    std::sync::LazyLock::new(|| {
        closure_rhino::qualified_name::QualifiedName::of("$jscomp.inherits")
    });
static FUNCTION_PROTOTYPE_BIND_CALL: std::sync::LazyLock<
    closure_rhino::qualified_name::QualifiedName,
> = std::sync::LazyLock::new(|| {
    closure_rhino::qualified_name::QualifiedName::of("Function.prototype.bind.call")
});

// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
macro_rules! replay_proxy_fields {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct ProxyReplayFields<'a> { $(pub $name: &'a $ty,)* }
        pub struct ProxyReplayFieldsMut<'a> { $(pub $name: &'a mut $ty,)* }
        impl Proxy {
            // port: java.lang.reflect.Field#get (native replay access)
            pub fn replay_fields(&self) -> ProxyReplayFields<'_> {
                ProxyReplayFields { $($name: &self.$name,)* }
            }
            // port: java.lang.reflect.Field#set (native replay access)
            pub fn replay_fields_mut(&mut self) -> ProxyReplayFieldsMut<'_> {
                ProxyReplayFieldsMut { $($name: &mut self.$name,)* }
            }
        }
    };
}
replay_proxy_fields!(
    next_convention: Arc<dyn CodingConvention>,
);
