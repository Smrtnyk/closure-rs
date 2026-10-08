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
//   src/com/google/javascript/jscomp/ChromeCodingConvention.java.

// STAND-IN: ChromeCodingConvention is Chrome-specific (out of scope, docs/PORTING.md §2); not ported
use closure_jscomp::coding_convention::*;
use closure_jstype::{prelude::*, rhino::nominal_type_builder::NominalTypeBuilder};
use closure_rhino::{
    js_string::JsString,
    jstype::TypeId,
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
};
#[derive(Debug, Default)]
pub struct ChromeCodingConvention;
impl ChromeCodingConvention {
    // port: ChromeCodingConvention#ChromeCodingConvention
    pub fn new() -> Self {
        Self
    }
}
impl CodingConvention for ChromeCodingConvention {
    // port: ChromeCodingConvention#isConstant
    fn is_constant(&self, _name: &JsString) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isConstant not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isConstantKey
    fn is_constant_key(&self, _name: &JsString) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isConstantKey not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isValidEnumKey
    fn is_valid_enum_key(&self, _key: Option<&JsString>) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isValidEnumKey not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isOptionalParameter
    fn is_optional_parameter(&self, _ast: &Ast, _parameter: NodeId) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isOptionalParameter not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isVarArgsParameter
    fn is_var_args_parameter(&self, _ast: &Ast, _parameter: NodeId) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isVarArgsParameter not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isFunctionCallThatAlwaysThrows
    fn is_function_call_that_always_throws(
        &self,
        _ast: &Ast,
        _reg: &JSTypeRegistry,
        _n: NodeId,
    ) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isFunctionCallThatAlwaysThrows not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isExported
    fn is_exported(&self, _name: &JsString, _local: bool) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isExported not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#getPackageName
    fn get_package_name(&self, _source: &dyn StaticSourceFile) -> Option<String> {
        panic!(
            "STAND-IN: ChromeCodingConvention#getPackageName not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#getClassesDefinedByCall
    fn get_classes_defined_by_call(
        &self,
        _ast: &Ast,
        _call_node: NodeId,
    ) -> Option<SubclassRelationship> {
        panic!(
            "STAND-IN: ChromeCodingConvention#getClassesDefinedByCall not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isClassFactoryCall
    fn is_class_factory_call(&self, _ast: &Ast, _call_node: NodeId) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isClassFactoryCall not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isSuperClassReference
    fn is_super_class_reference(&self, _property_name: &JsString) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isSuperClassReference not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#extractIsModuleFile
    fn extract_is_module_file(&self, _ast: &Ast, _node: NodeId, _parent: NodeId) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#extractIsModuleFile not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#extractClassNameIfProvide
    fn extract_class_name_if_provide(
        &self,
        _ast: &Ast,
        _node: NodeId,
        _parent: NodeId,
    ) -> Option<JsString> {
        panic!(
            "STAND-IN: ChromeCodingConvention#extractClassNameIfProvide not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#extractClassNameIfRequire
    fn extract_class_name_if_require(
        &self,
        _ast: &Ast,
        _node: NodeId,
        _parent: NodeId,
    ) -> Option<JsString> {
        panic!(
            "STAND-IN: ChromeCodingConvention#extractClassNameIfRequire not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#getExportPropertyFunction
    fn get_export_property_function(&self) -> Option<JsString> {
        panic!(
            "STAND-IN: ChromeCodingConvention#getExportPropertyFunction not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#getExportSymbolFunction
    fn get_export_symbol_function(&self) -> Option<JsString> {
        panic!(
            "STAND-IN: ChromeCodingConvention#getExportSymbolFunction not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#identifyTypeDeclarationCall
    fn identify_type_declaration_call(&self, _ast: &Ast, _n: NodeId) -> Vec<JsString> {
        panic!(
            "STAND-IN: ChromeCodingConvention#identifyTypeDeclarationCall not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#applySubclassRelationship
    fn apply_subclass_relationship(
        &self,
        _ast: &Ast,
        _reg: &mut JSTypeRegistry,
        _parent: &NominalTypeBuilder,
        _child: &NominalTypeBuilder,
        _type_: SubclassType,
    ) {
        panic!(
            "STAND-IN: ChromeCodingConvention#applySubclassRelationship not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#getAbstractMethodName
    fn get_abstract_method_name(&self) -> Option<JsString> {
        panic!(
            "STAND-IN: ChromeCodingConvention#getAbstractMethodName not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#getSingletonGetterClassName
    fn get_singleton_getter_class_name(&self, _ast: &Ast, _call_node: NodeId) -> Option<JsString> {
        panic!(
            "STAND-IN: ChromeCodingConvention#getSingletonGetterClassName not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#applySingletonGetter
    fn apply_singleton_getter(
        &self,
        _ast: &Ast,
        _reg: &mut JSTypeRegistry,
        _class_type: &NominalTypeBuilder,
        _getter_type: TypeId,
    ) {
        panic!(
            "STAND-IN: ChromeCodingConvention#applySingletonGetter not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#describeFunctionBind
    fn describe_function_bind(
        &self,
        _ast: &Ast,
        _reg: Option<&mut JSTypeRegistry>,
        _n: NodeId,
        _check_types: bool,
    ) -> Option<Bind> {
        panic!(
            "STAND-IN: ChromeCodingConvention#describeFunctionBind not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#describeCachingCall
    fn describe_caching_call(&self, _ast: &Ast, _node: NodeId) -> Option<Cache> {
        panic!(
            "STAND-IN: ChromeCodingConvention#describeCachingCall not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isPropertyTestFunction
    fn is_property_test_function(&self, _ast: &Ast, _call: NodeId) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isPropertyTestFunction not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isPrototypeAlias
    fn is_prototype_alias(&self, _ast: &Ast, _get_prop: NodeId) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isPrototypeAlias not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#isPropertyRenameFunction
    fn is_property_rename_function(&self, _ast: &Ast, _name_node: NodeId) -> bool {
        panic!(
            "STAND-IN: ChromeCodingConvention#isPropertyRenameFunction not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#getObjectLiteralCast
    fn get_object_literal_cast(&self, _ast: &Ast, _call_node: NodeId) -> Option<ObjectLiteralCast> {
        panic!(
            "STAND-IN: ChromeCodingConvention#getObjectLiteralCast not ported (Chrome-specific; out of scope)"
        );
    }
    // port: ChromeCodingConvention#getAssertionFunctions
    fn get_assertion_functions(&self) -> Vec<AssertionFunctionSpec> {
        panic!(
            "STAND-IN: ChromeCodingConvention#getAssertionFunctions not ported (Chrome-specific; out of scope)"
        );
    }
}
