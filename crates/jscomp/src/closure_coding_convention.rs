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
//   src/com/google/javascript/jscomp/ClosureCodingConvention.java.

// Preserve the Java nested condition flow.
#![allow(clippy::collapsible_if)]
use crate::coding_conventions::{CodingConventions, Proxy};
use crate::diagnostic_type::DiagnosticType;
use crate::{coding_convention::*, node_util::NodeUtil};
use closure_jstype::{
    prelude::{FunctionType, JSTypeRegistry},
    rhino::nominal_type_builder::NominalTypeBuilder,
};
use closure_rhino::{
    js_string::JsString,
    jstype::TypeId,
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
};
use std::{fmt::Debug, sync::Arc};
pub static OBJECTLIT_EXPECTED: DiagnosticType = DiagnosticType::warning(
    "JSC_REFLECT_OBJECTLIT_EXPECTED",
    "Object literal expected as second argument",
);
#[derive(Debug)]
pub struct ClosureCodingConvention {
    proxy: Proxy,
}
impl ClosureCodingConvention {
    pub const SERIAL_VERSION_UID: i64 = 1;
    // port: ClosureCodingConvention#ClosureCodingConvention()
    pub fn new() -> Self {
        Self::with_convention(CodingConventions::get_default())
    }
    // port: ClosureCodingConvention#ClosureCodingConvention(CodingConvention)
    pub fn with_convention(wrapped: Arc<dyn CodingConvention>) -> Self {
        Self {
            proxy: Proxy::new(wrapped),
        }
    }
    // port: ClosureCodingConvention#typeofClassDefiningName
    fn typeof_class_defining_name(ast: &Ast, call_name: NodeId) -> Option<SubclassType> {
        let mut method_name = None;
        if call_name.is_get_prop(ast) {
            method_name = Some(call_name.get_string(ast));
        } else if call_name.is_name(ast) {
            let name = call_name.get_string(ast);
            let dollar_index = name.as_units().iter().rposition(|c| *c == b'$' as u16);
            if let Some(dollar_index) = dollar_index {
                method_name = Some(name.substring_from(dollar_index + 1));
            }
        }
        match method_name {
            Some(s) if s == "inherits" => Some(SubclassType::INHERITS),
            Some(s) if s == "mixin" => Some(SubclassType::MIXIN),
            _ => None,
        }
    }
    // port: ClosureCodingConvention#endsWithPrototype
    fn ends_with_prototype(ast: &Ast, qualified_name: NodeId) -> bool {
        qualified_name.is_get_prop(ast) && qualified_name.get_string_ref(ast) == "prototype"
    }
    // port: ClosureCodingConvention#extractClassNameIfGoog
    fn extract_class_name_if_goog(
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
        function_name: &str,
    ) -> Option<JsString> {
        let mut class_name = None;
        if NodeUtil::is_expr_call(ast, parent) {
            if let Some(callee) = node.get_first_child(ast) {
                if callee.is_get_prop(ast) && callee.matches_qualified_name(ast, function_name) {
                    if let Some(target) = callee.get_next(ast) {
                        if target.is_string_lit(ast) {
                            class_name = Some(target.get_string(ast));
                        }
                    }
                }
            }
        }
        class_name
    }
    // port: ClosureCodingConvention#createGoogAssertOnReturn
    fn create_goog_assert_on_return(asserted_type_name: &str) -> AssertionFunctionSpec {
        AssertionFunctionSpec::for_matches_return()
            .set_function_name(format!("goog.asserts.assert{asserted_type_name}"))
            .build()
    }
    // port: ClosureCodingConvention#matchesCacheMethodName
    fn matches_cache_method_name(&self, ast: &Ast, target: NodeId) -> bool {
        if target.is_get_prop(ast) {
            matches_qualified_name_pattern(ast, target, &GOOG_CACHE_REFLECT.0, GOOG_CACHE_REFLECT.1)
        } else if target.is_name(ast) {
            target.get_string_ref(ast) == "goog$reflect$cache"
        } else {
            false
        }
    }
    // port: ClosureCodingConvention#safeNext
    fn safe_next(ast: &Ast, n: Option<NodeId>) -> Option<NodeId> {
        n.and_then(|n| n.get_next(ast))
    }
}
impl Default for ClosureCodingConvention {
    // port: ClosureCodingConvention#ClosureCodingConvention()
    fn default() -> Self {
        Self::new()
    }
}
impl CodingConvention for ClosureCodingConvention {
    // port: ClosureCodingConvention#applySubclassRelationship
    fn apply_subclass_relationship(
        &self,
        ast: &Ast,
        reg: &mut JSTypeRegistry,
        parent: &NominalTypeBuilder,
        child: &NominalTypeBuilder,
        type_: SubclassType,
    ) {
        self.proxy
            .apply_subclass_relationship(ast, reg, parent, child, type_);
        if type_ == SubclassType::INHERITS {
            let child_ctor = child.constructor();
            let parent_proto = parent.prototype_or_instance();
            let source = child_ctor.get_source(reg);
            child.declare_constructor_property(reg, ast, "superClass_", parent_proto, source);
            let qmark_ctor = child_ctor.forget_parameter_and_return_types(reg, ast);
            let source = child_ctor.get_source(reg);
            child.declare_prototype_property(reg, ast, "constructor", qmark_ctor, source);
        }
    }
    // port: ClosureCodingConvention#getClassesDefinedByCall
    fn get_classes_defined_by_call(
        &self,
        ast: &Ast,
        call_node: NodeId,
    ) -> Option<SubclassRelationship> {
        let relationship = self.proxy.get_classes_defined_by_call(ast, call_node);
        if relationship.is_some() {
            return relationship;
        }
        let call_name = call_node.get_first_child(ast).unwrap();
        if let Some(type_) = Self::typeof_class_defining_name(ast, call_name) {
            if call_node.get_child_count(ast) < 3 {
                return None;
            }
            let mut subclass = call_name.get_next(ast).unwrap();
            let mut superclass = subclass.get_next(ast).unwrap();
            if type_ == SubclassType::MIXIN {
                if Self::ends_with_prototype(ast, superclass) {
                    superclass = superclass.get_first_child(ast).unwrap();
                }
                if Self::ends_with_prototype(ast, subclass) {
                    subclass = subclass.get_first_child(ast).unwrap();
                }
            }
            if subclass.is_unscoped_qualified_name(ast)
                && superclass.is_unscoped_qualified_name(ast)
            {
                return Some(SubclassRelationship::new(ast, type_, subclass, superclass));
            }
        }
        None
    }
    // port: ClosureCodingConvention#isSuperClassReference
    fn is_super_class_reference(&self, property_name: &JsString) -> bool {
        property_name == "superClass_" || self.proxy.is_super_class_reference(property_name)
    }
    // port: ClosureCodingConvention#extractIsModuleFile
    fn extract_is_module_file(&self, ast: &Ast, node: NodeId, parent: NodeId) -> bool {
        Self::extract_class_name_if_goog(ast, node, parent, "goog.module").is_some()
    }
    // port: ClosureCodingConvention#extractClassNameIfProvide
    fn extract_class_name_if_provide(
        &self,
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
    ) -> Option<JsString> {
        let mut namespace = Self::extract_class_name_if_goog(ast, node, parent, "goog.provide");
        if namespace.is_none() {
            namespace = Self::extract_class_name_if_goog(ast, node, parent, "goog.module");
        }
        namespace
    }
    // port: ClosureCodingConvention#extractClassNameIfRequire
    fn extract_class_name_if_require(
        &self,
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
    ) -> Option<JsString> {
        Self::extract_class_name_if_goog(ast, node, parent, "goog.require")
    }
    // port: ClosureCodingConvention#getExportPropertyFunction
    fn get_export_property_function(&self) -> Option<JsString> {
        Some("goog.exportProperty".into())
    }
    // port: ClosureCodingConvention#getExportSymbolFunction
    fn get_export_symbol_function(&self) -> Option<JsString> {
        Some("goog.exportSymbol".into())
    }
    // port: ClosureCodingConvention#identifyTypeDeclarationCall
    fn identify_type_declaration_call(&self, ast: &Ast, n: NodeId) -> Vec<JsString> {
        let call_name = n.get_first_child(ast).unwrap();
        if GOOG_FORWARDDECLARE.matches(ast, call_name) && n.has_two_children(ast) {
            let type_declaration = n.get_second_child(ast).unwrap();
            if type_declaration.is_string_lit(ast) {
                return vec![type_declaration.get_string(ast)];
            }
        }
        self.proxy.identify_type_declaration_call(ast, n)
    }
    // port: ClosureCodingConvention#getAbstractMethodName
    fn get_abstract_method_name(&self) -> Option<JsString> {
        Some("goog.abstractMethod".into())
    }
    // port: ClosureCodingConvention#getSingletonGetterClassName
    fn get_singleton_getter_class_name(&self, ast: &Ast, call_node: NodeId) -> Option<JsString> {
        let call_arg = call_node.get_first_child(ast).unwrap();
        if call_node.has_two_children(ast)
            && (GOOG_ADDSINGLETONGETTER.matches(ast, call_arg)
                || GOOG_ADDSINGLETONGETTER_MANGLED.matches(ast, call_arg))
        {
            return call_arg.get_next(ast).unwrap().get_qualified_name(ast);
        }
        self.proxy.get_singleton_getter_class_name(ast, call_node)
    }
    // port: ClosureCodingConvention#applySingletonGetter
    fn apply_singleton_getter(
        &self,
        ast: &Ast,
        reg: &mut JSTypeRegistry,
        class_type: &NominalTypeBuilder,
        getter_type: TypeId,
    ) {
        let def_site = class_type.constructor().get_source(reg);
        class_type.declare_constructor_property(reg, ast, "getInstance", getter_type, def_site);
        let instance = class_type.instance();
        class_type.declare_constructor_property(reg, ast, "instance_", instance, def_site);
    }
    // port: ClosureCodingConvention#isPropertyTestFunction
    fn is_property_test_function(&self, ast: &Ast, call: NodeId) -> bool {
        closure_rhino::check_argument!(call.is_call(ast));
        let target = call.get_first_child(ast).unwrap();
        if target.is_get_prop(ast) {
            let src = target.get_first_child(ast).unwrap();
            let prop = target.get_string(ast);
            if src.is_name(ast)
                && src.get_string_ref(ast) == "goog"
                && (prop == "isArrayLike" || prop == "isObject")
            {
                return true;
            }
        }
        self.proxy.is_property_test_function(ast, call)
    }
    // port: ClosureCodingConvention#isPropertyRenameFunction
    fn is_property_rename_function(&self, ast: &Ast, name_node: NodeId) -> bool {
        if self.proxy.is_property_rename_function(ast, name_node) {
            return true;
        }
        GOOG_REFLECT_OBJECTPROPERTY.matches(ast, name_node)
            || GOOG_REFLECT_OBJECTPROPERTY_MANGLED.matches(ast, name_node)
    }
    // port: ClosureCodingConvention#isFunctionCallThatAlwaysThrows
    fn is_function_call_that_always_throws(
        &self,
        ast: &Ast,
        reg: &JSTypeRegistry,
        n: NodeId,
    ) -> bool {
        self.proxy.is_function_call_that_always_throws(ast, reg, n)
            || CodingConventions::default_is_function_call_that_always_throws(
                ast,
                n,
                &JsString::from("goog.asserts.fail"),
            )
    }
    // port: ClosureCodingConvention#getObjectLiteralCast
    fn get_object_literal_cast(&self, ast: &Ast, call_node: NodeId) -> Option<ObjectLiteralCast> {
        closure_rhino::check_argument!(
            call_node.is_call(ast),
            "Expected call node but found %s",
            call_node.to_string(ast)
        );
        let proxy_cast = self.proxy.get_object_literal_cast(ast, call_node);
        if proxy_cast.is_some() {
            return proxy_cast;
        }
        let call_name = call_node.get_first_child(ast).unwrap();
        if !(GOOG_REFLECT_OBJECT.matches(ast, call_name)
            || JSCOMP_REFLECTOBJECT.matches(ast, call_name))
            || !call_node.has_x_children(ast, 3)
        {
            return None;
        }
        let type_node = call_name.get_next(ast).unwrap();
        if !type_node.is_qualified_name(ast) {
            return None;
        }
        let object_node = type_node.get_next(ast).unwrap();
        if !object_node.is_object_lit(ast) {
            return Some(ObjectLiteralCast::new(
                None,
                None,
                Some(&OBJECTLIT_EXPECTED),
            ));
        }
        Some(ObjectLiteralCast::new(
            type_node.get_qualified_name(ast),
            type_node.get_next(ast),
            None,
        ))
    }
    // port: ClosureCodingConvention#getAssertionFunctions
    fn get_assertion_functions(&self) -> Vec<AssertionFunctionSpec> {
        let mut specs = self.proxy.get_assertion_functions();
        let extra = std::iter::once(
            AssertionFunctionSpec::for_truthy()
                .set_function_name("goog.asserts.assert")
                .build(),
        )
        .chain(
            [
                "Array",
                "Boolean",
                "Element",
                "Function",
                "Instanceof",
                "Number",
                "Object",
                "String",
            ]
            .into_iter()
            .map(Self::create_goog_assert_on_return),
        );
        for spec in extra {
            if !specs.contains(&spec) {
                specs.push(spec);
            }
        }
        specs
    }
    // port: ClosureCodingConvention#describeFunctionBind
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
        if call_target.is_qualified_name(ast) {
            if GOOG_BIND.matches(ast, call_target) || call_target.matches_name(ast, "goog$bind") {
                let fn_ = call_target.get_next(ast)?;
                let this_value = Self::safe_next(ast, Some(fn_));
                let parameters = Self::safe_next(ast, this_value);
                return Some(Bind::new(fn_, this_value, parameters));
            }
            if GOOG_PARTIAL.matches(ast, call_target)
                || call_target.matches_name(ast, "goog$partial")
            {
                let fn_ = call_target.get_next(ast)?;
                let parameters = Self::safe_next(ast, Some(fn_));
                return Some(Bind::new(fn_, None, parameters));
            }
        }
        self.proxy.describe_function_bind(ast, reg, n, check_types)
    }
    // port: ClosureCodingConvention#describeCachingCall
    fn describe_caching_call(&self, ast: &Ast, node: NodeId) -> Option<Cache> {
        if !node.is_call(ast) {
            return None;
        }
        let call_target = node.get_first_child(ast).unwrap();
        if self.matches_cache_method_name(ast, call_target) {
            let param_count = node.get_child_count(ast) - 1;
            if (3..=4).contains(&param_count) {
                let cache_obj = call_target.get_next(ast).unwrap();
                let key_node = cache_obj.get_next(ast).unwrap();
                let value_fn = key_node.get_next(ast).unwrap();
                let key_fn = value_fn.get_next(ast);
                return Some(Cache::new(cache_obj, key_node, value_fn, key_fn));
            }
        }
        self.proxy.describe_caching_call(ast, node)
    }
    // port: ClosureCodingConvention#isConstant
    fn is_constant(&self, name: &JsString) -> bool {
        self.proxy.is_constant(name)
    }
    // port: ClosureCodingConvention#isConstantKey
    fn is_constant_key(&self, name: &JsString) -> bool {
        self.proxy.is_constant_key(name)
    }
    // port: ClosureCodingConvention#isValidEnumKey
    fn is_valid_enum_key(&self, key: Option<&JsString>) -> bool {
        self.proxy.is_valid_enum_key(key)
    }
    // port: ClosureCodingConvention#isOptionalParameter
    fn is_optional_parameter(&self, ast: &Ast, parameter: NodeId) -> bool {
        self.proxy.is_optional_parameter(ast, parameter)
    }
    // port: ClosureCodingConvention#isVarArgsParameter
    fn is_var_args_parameter(&self, ast: &Ast, parameter: NodeId) -> bool {
        self.proxy.is_var_args_parameter(ast, parameter)
    }
    // port: ClosureCodingConvention#isExported
    fn is_exported(&self, name: &JsString, local: bool) -> bool {
        self.proxy.is_exported(name, local)
    }
    // port: ClosureCodingConvention#getPackageName
    fn get_package_name(&self, source: &dyn StaticSourceFile) -> Option<String> {
        self.proxy.get_package_name(source)
    }
    // port: ClosureCodingConvention#isClassFactoryCall
    fn is_class_factory_call(&self, ast: &Ast, call_node: NodeId) -> bool {
        self.proxy.is_class_factory_call(ast, call_node)
    }
    // port: ClosureCodingConvention#isPrototypeAlias
    fn is_prototype_alias(&self, ast: &Ast, get_prop: NodeId) -> bool {
        self.proxy.is_prototype_alias(ast, get_prop)
    }
}

static GOOG_FORWARDDECLARE: std::sync::LazyLock<closure_rhino::qualified_name::QualifiedName> =
    std::sync::LazyLock::new(|| {
        closure_rhino::qualified_name::QualifiedName::of("goog.forwardDeclare")
    });
static GOOG_ADDSINGLETONGETTER: std::sync::LazyLock<closure_rhino::qualified_name::QualifiedName> =
    std::sync::LazyLock::new(|| {
        closure_rhino::qualified_name::QualifiedName::of("goog.addSingletonGetter")
    });
static GOOG_ADDSINGLETONGETTER_MANGLED: std::sync::LazyLock<
    closure_rhino::qualified_name::QualifiedName,
> = std::sync::LazyLock::new(|| {
    closure_rhino::qualified_name::QualifiedName::of("goog$addSingletonGetter")
});
static GOOG_REFLECT_OBJECTPROPERTY: std::sync::LazyLock<
    closure_rhino::qualified_name::QualifiedName,
> = std::sync::LazyLock::new(|| {
    closure_rhino::qualified_name::QualifiedName::of("goog.reflect.objectProperty")
});
static GOOG_REFLECT_OBJECTPROPERTY_MANGLED: std::sync::LazyLock<
    closure_rhino::qualified_name::QualifiedName,
> = std::sync::LazyLock::new(|| {
    closure_rhino::qualified_name::QualifiedName::of("goog$reflect$objectProperty")
});
static GOOG_REFLECT_OBJECT: std::sync::LazyLock<closure_rhino::qualified_name::QualifiedName> =
    std::sync::LazyLock::new(|| {
        closure_rhino::qualified_name::QualifiedName::of("goog.reflect.object")
    });
static JSCOMP_REFLECTOBJECT: std::sync::LazyLock<closure_rhino::qualified_name::QualifiedName> =
    std::sync::LazyLock::new(|| {
        closure_rhino::qualified_name::QualifiedName::of("$jscomp.reflectObject")
    });
static GOOG_BIND: std::sync::LazyLock<closure_rhino::qualified_name::QualifiedName> =
    std::sync::LazyLock::new(|| closure_rhino::qualified_name::QualifiedName::of("goog.bind"));
static GOOG_PARTIAL: std::sync::LazyLock<closure_rhino::qualified_name::QualifiedName> =
    std::sync::LazyLock::new(|| closure_rhino::qualified_name::QualifiedName::of("goog.partial"));
static GOOG_CACHE_REFLECT: std::sync::LazyLock<(Ast, NodeId)> = std::sync::LazyLock::new(|| {
    let mut ast = Ast::new();
    let goog = closure_rhino::ir::IR::name(&mut ast, "goog");
    let node = closure_rhino::ir::IR::getprop_with_more_props(
        &mut ast,
        goog,
        "reflect",
        &["cache".into()],
    );
    (ast, node)
});
// Node#matchesQualifiedName(Node) across two node arenas, ported from Closure's Rhino-derived
// Node (MPL-1.1 / GPL-2.0-or-later), is in its own file.
#[path = "closure_coding_convention_rhino.rs"]
mod rhino;
pub(crate) use rhino::matches_qualified_name_pattern;

// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
macro_rules! replay_closure_coding_convention_fields {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct ClosureCodingConventionReplayFields<'a> { $(pub $name: &'a $ty,)* }
        pub struct ClosureCodingConventionReplayFieldsMut<'a> { $(pub $name: &'a mut $ty,)* }
        impl ClosureCodingConvention {
            // port: java.lang.reflect.Field#get (native replay access)
            pub fn replay_fields(&self) -> ClosureCodingConventionReplayFields<'_> {
                ClosureCodingConventionReplayFields { $($name: &self.$name,)* }
            }
            // port: java.lang.reflect.Field#set (native replay access)
            pub fn replay_fields_mut(&mut self) -> ClosureCodingConventionReplayFieldsMut<'_> {
                ClosureCodingConventionReplayFieldsMut { $($name: &mut self.$name,)* }
            }
        }
    };
}
replay_closure_coding_convention_fields!(
    proxy: Proxy,
);
