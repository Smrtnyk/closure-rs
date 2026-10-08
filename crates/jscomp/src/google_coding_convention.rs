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
//   src/com/google/javascript/jscomp/GoogleCodingConvention.java.

use crate::coding_convention::*;
use crate::{closure_coding_convention::ClosureCodingConvention, coding_conventions::Proxy};
use closure_jstype::{JSTypeRegistry, rhino::nominal_type_builder::NominalTypeBuilder};
use closure_rhino::java_lang::{self, pattern::Pattern};
use closure_rhino::{
    js_string::JsString,
    jstype::TypeId,
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
};
use std::sync::LazyLock;
use std::{fmt::Debug, sync::Arc};
static ENUM_KEY_PATTERN: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("[A-Z0-9][A-Z0-9_]*"));
static PACKAGE_WITH_TEST_DIR: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("^(.*)/(?:test|tests|testing)/(?:[^/]+)$"));
static GENFILES_DIR: LazyLock<Pattern> =
    LazyLock::new(|| Pattern::compile("-out/.*/(bin|genfiles)/(.*)$"));
#[derive(Debug)]
pub struct GoogleCodingConvention {
    proxy: Proxy,
}
impl GoogleCodingConvention {
    pub const SERIAL_VERSION_UID: i64 = 1;
    const OPTIONAL_ARG_PREFIX: &'static str = "opt_";
    const VAR_ARGS_NAME: &'static str = "var_args";
    // port: GoogleCodingConvention#GoogleCodingConvention()
    pub fn new() -> Self {
        Self::with_convention(Arc::new(ClosureCodingConvention::new()))
    }
    // port: GoogleCodingConvention#GoogleCodingConvention(CodingConvention)
    pub fn with_convention(convention: Arc<dyn CodingConvention>) -> Self {
        Self {
            proxy: Proxy::new(convention),
        }
    }
    // port: GoogleCodingConvention#isConstantKey(String,int,int)
    pub fn is_constant_key_range(name: &JsString, start: usize, end: usize) -> bool {
        if start >= end || !java_lang::is_upper_case(name.char_at(start)) {
            return false;
        }
        for i in start + 1..end {
            // check this instead of Character.isUpperCase - they have different results for some
            // characters like symbols. symbols are allowed unless at the start index.
            if java_lang::to_upper_case(name.char_at(i)) != name.char_at(i) {
                return false;
            }
        }
        true
    }
}
impl Default for GoogleCodingConvention {
    // port: GoogleCodingConvention#GoogleCodingConvention()
    fn default() -> Self {
        Self::new()
    }
}
impl CodingConvention for GoogleCodingConvention {
    // port: GoogleCodingConvention#isConstant
    fn is_constant(&self, name: &JsString) -> bool {
        if name.length() <= 1 {
            return false;
        }
        let pos = name.as_units().iter().rposition(|c| *c == b'$' as u16);
        let start = pos.map_or(0, |p| p + 1);
        Self::is_constant_key_range(name, start, name.length())
    }
    // port: GoogleCodingConvention#isConstantKey
    fn is_constant_key(&self, name: &JsString) -> bool {
        Self::is_constant_key_range(name, 0, name.length())
    }
    // port: GoogleCodingConvention#isValidEnumKey
    fn is_valid_enum_key(&self, key: Option<&JsString>) -> bool {
        ENUM_KEY_PATTERN
            .matcher(&key.expect("null").to_string_lossy())
            .matches()
    }
    // port: GoogleCodingConvention#isOptionalParameter
    fn is_optional_parameter(&self, ast: &Ast, parameter: NodeId) -> bool {
        self.proxy.is_optional_parameter(ast, parameter)
            || (parameter.is_name(ast)
                && parameter
                    .get_string(ast)
                    .starts_with(&JsString::from(Self::OPTIONAL_ARG_PREFIX)))
    }
    // port: GoogleCodingConvention#isVarArgsParameter
    fn is_var_args_parameter(&self, ast: &Ast, parameter: NodeId) -> bool {
        self.proxy.is_var_args_parameter(ast, parameter)
            || (parameter.is_name(ast) && parameter.get_string(ast) == Self::VAR_ARGS_NAME)
    }
    // port: GoogleCodingConvention#isExported
    fn is_exported(&self, name: &JsString, local: bool) -> bool {
        self.proxy.is_exported(name, local) || (!local && name.starts_with(&JsString::from("_")))
    }
    // port: GoogleCodingConvention#isClassFactoryCall
    fn is_class_factory_call(&self, ast: &Ast, call_node: NodeId) -> bool {
        let call_target = call_node.get_first_child(ast).unwrap();
        self.proxy.is_class_factory_call(ast, call_node)
            || (call_target.is_name(ast) && call_target.to_string(ast) == "Polymer")
    }
    // port: GoogleCodingConvention#getPackageName
    fn get_package_name(&self, source: &dyn StaticSourceFile) -> Option<String> {
        let mut name = source.get_name().to_string();
        let mut genfiles_matcher = GENFILES_DIR.matcher(&name);
        if genfiles_matcher.find() {
            name = genfiles_matcher.group(2).unwrap().into();
        }
        let mut m = PACKAGE_WITH_TEST_DIR.matcher(&name);
        if m.find() {
            Some(m.group(1).unwrap().into())
        } else {
            Some(
                name.rfind('/')
                    .map_or_else(String::new, |last_slash| name[..last_slash].into()),
            )
        }
    }
    // port: GoogleCodingConvention#isFunctionCallThatAlwaysThrows
    fn is_function_call_that_always_throws(
        &self,
        ast: &Ast,
        reg: &JSTypeRegistry,
        n: NodeId,
    ) -> bool {
        self.proxy.is_function_call_that_always_throws(ast, reg, n)
    }
    // port: GoogleCodingConvention#getClassesDefinedByCall
    fn get_classes_defined_by_call(
        &self,
        ast: &Ast,
        call_node: NodeId,
    ) -> Option<SubclassRelationship> {
        self.proxy.get_classes_defined_by_call(ast, call_node)
    }
    // port: GoogleCodingConvention#isSuperClassReference
    fn is_super_class_reference(&self, property_name: &JsString) -> bool {
        self.proxy.is_super_class_reference(property_name)
    }
    // port: GoogleCodingConvention#extractIsModuleFile
    fn extract_is_module_file(&self, ast: &Ast, node: NodeId, parent: NodeId) -> bool {
        self.proxy.extract_is_module_file(ast, node, parent)
    }
    // port: GoogleCodingConvention#extractClassNameIfProvide
    fn extract_class_name_if_provide(
        &self,
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
    ) -> Option<JsString> {
        self.proxy.extract_class_name_if_provide(ast, node, parent)
    }
    // port: GoogleCodingConvention#extractClassNameIfRequire
    fn extract_class_name_if_require(
        &self,
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
    ) -> Option<JsString> {
        self.proxy.extract_class_name_if_require(ast, node, parent)
    }
    // port: GoogleCodingConvention#getExportPropertyFunction
    fn get_export_property_function(&self) -> Option<JsString> {
        self.proxy.get_export_property_function()
    }
    // port: GoogleCodingConvention#getExportSymbolFunction
    fn get_export_symbol_function(&self) -> Option<JsString> {
        self.proxy.get_export_symbol_function()
    }
    // port: GoogleCodingConvention#identifyTypeDeclarationCall
    fn identify_type_declaration_call(&self, ast: &Ast, n: NodeId) -> Vec<JsString> {
        self.proxy.identify_type_declaration_call(ast, n)
    }
    // port: GoogleCodingConvention#applySubclassRelationship
    fn apply_subclass_relationship(
        &self,
        ast: &Ast,
        reg: &mut JSTypeRegistry,
        parent: &NominalTypeBuilder,
        child: &NominalTypeBuilder,
        type_: SubclassType,
    ) {
        self.proxy
            .apply_subclass_relationship(ast, reg, parent, child, type_)
    }
    // port: GoogleCodingConvention#getAbstractMethodName
    fn get_abstract_method_name(&self) -> Option<JsString> {
        self.proxy.get_abstract_method_name()
    }
    // port: GoogleCodingConvention#getSingletonGetterClassName
    fn get_singleton_getter_class_name(&self, ast: &Ast, call_node: NodeId) -> Option<JsString> {
        self.proxy.get_singleton_getter_class_name(ast, call_node)
    }
    // port: GoogleCodingConvention#applySingletonGetter
    fn apply_singleton_getter(
        &self,
        ast: &Ast,
        reg: &mut JSTypeRegistry,
        class_type: &NominalTypeBuilder,
        getter_type: TypeId,
    ) {
        self.proxy
            .apply_singleton_getter(ast, reg, class_type, getter_type)
    }
    // port: GoogleCodingConvention#describeFunctionBind
    fn describe_function_bind(
        &self,
        ast: &Ast,
        reg: Option<&mut JSTypeRegistry>,
        n: NodeId,
        check_types: bool,
    ) -> Option<Bind> {
        self.proxy.describe_function_bind(ast, reg, n, check_types)
    }
    // port: GoogleCodingConvention#describeCachingCall
    fn describe_caching_call(&self, ast: &Ast, node: NodeId) -> Option<Cache> {
        self.proxy.describe_caching_call(ast, node)
    }
    // port: GoogleCodingConvention#isPropertyTestFunction
    fn is_property_test_function(&self, ast: &Ast, call: NodeId) -> bool {
        self.proxy.is_property_test_function(ast, call)
    }
    // port: GoogleCodingConvention#isPrototypeAlias
    fn is_prototype_alias(&self, ast: &Ast, get_prop: NodeId) -> bool {
        self.proxy.is_prototype_alias(ast, get_prop)
    }
    // port: GoogleCodingConvention#isPropertyRenameFunction
    fn is_property_rename_function(&self, ast: &Ast, name_node: NodeId) -> bool {
        self.proxy.is_property_rename_function(ast, name_node)
    }
    // port: GoogleCodingConvention#getObjectLiteralCast
    fn get_object_literal_cast(&self, ast: &Ast, call_node: NodeId) -> Option<ObjectLiteralCast> {
        self.proxy.get_object_literal_cast(ast, call_node)
    }
    // port: GoogleCodingConvention#getAssertionFunctions
    fn get_assertion_functions(&self) -> Vec<AssertionFunctionSpec> {
        self.proxy.get_assertion_functions()
    }
}

// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
macro_rules! replay_google_coding_convention_fields {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct GoogleCodingConventionReplayFields<'a> { $(pub $name: &'a $ty,)* }
        pub struct GoogleCodingConventionReplayFieldsMut<'a> { $(pub $name: &'a mut $ty,)* }
        impl GoogleCodingConvention {
            // port: java.lang.reflect.Field#get (native replay access)
            pub fn replay_fields(&self) -> GoogleCodingConventionReplayFields<'_> {
                GoogleCodingConventionReplayFields { $($name: &self.$name,)* }
            }
            // port: java.lang.reflect.Field#set (native replay access)
            pub fn replay_fields_mut(&mut self) -> GoogleCodingConventionReplayFieldsMut<'_> {
                GoogleCodingConventionReplayFieldsMut { $($name: &mut self.$name,)* }
            }
        }
    };
}
replay_google_coding_convention_fields!(
    proxy: Proxy,
);
