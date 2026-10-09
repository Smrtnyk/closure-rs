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
//   src/com/google/javascript/jscomp/CodingConvention.java.

// Preserve the Java nested condition flow.
#![allow(clippy::collapsible_if)]
use crate::diagnostic_type::DiagnosticType;
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
use std::fmt::Debug;
pub trait CodingConvention: std::any::Any + Send + Sync + Debug {
    // port: CodingConvention#isConstant
    fn is_constant(&self, name: &JsString) -> bool;
    // port: CodingConvention#isConstantKey
    fn is_constant_key(&self, name: &JsString) -> bool;
    // port: CodingConvention#isValidEnumKey
    fn is_valid_enum_key(&self, key: Option<&JsString>) -> bool;
    // port: CodingConvention#isOptionalParameter
    fn is_optional_parameter(&self, ast: &Ast, parameter: NodeId) -> bool;
    // port: CodingConvention#isVarArgsParameter
    fn is_var_args_parameter(&self, ast: &Ast, parameter: NodeId) -> bool;
    // port: CodingConvention#isFunctionCallThatAlwaysThrows
    fn is_function_call_that_always_throws(
        &self,
        ast: &Ast,
        reg: &JSTypeRegistry,
        n: NodeId,
    ) -> bool;
    // port: CodingConvention#isExported
    fn is_exported(&self, name: &JsString, local: bool) -> bool;
    // port: CodingConvention#getPackageName
    fn get_package_name(&self, source: &dyn StaticSourceFile) -> Option<String>;
    // port: CodingConvention#getClassesDefinedByCall
    fn get_classes_defined_by_call(
        &self,
        ast: &Ast,
        call_node: NodeId,
    ) -> Option<SubclassRelationship>;
    // port: CodingConvention#isClassFactoryCall
    fn is_class_factory_call(&self, ast: &Ast, call_node: NodeId) -> bool;
    // port: CodingConvention#isSuperClassReference
    fn is_super_class_reference(&self, property_name: &JsString) -> bool;
    // port: CodingConvention#extractIsModuleFile
    fn extract_is_module_file(&self, ast: &Ast, node: NodeId, parent: NodeId) -> bool;
    // port: CodingConvention#extractClassNameIfProvide
    fn extract_class_name_if_provide(
        &self,
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
    ) -> Option<JsString>;
    // port: CodingConvention#extractClassNameIfRequire
    fn extract_class_name_if_require(
        &self,
        ast: &Ast,
        node: NodeId,
        parent: NodeId,
    ) -> Option<JsString>;
    // port: CodingConvention#getExportPropertyFunction
    fn get_export_property_function(&self) -> Option<JsString>;
    // port: CodingConvention#getExportSymbolFunction
    fn get_export_symbol_function(&self) -> Option<JsString>;
    // port: CodingConvention#identifyTypeDeclarationCall
    fn identify_type_declaration_call(&self, ast: &Ast, n: NodeId) -> Vec<JsString>;
    // port: CodingConvention#applySubclassRelationship
    fn apply_subclass_relationship(
        &self,
        ast: &Ast,
        reg: &mut JSTypeRegistry,
        parent: &NominalTypeBuilder,
        child: &NominalTypeBuilder,
        type_: SubclassType,
    );
    // port: CodingConvention#getAbstractMethodName
    fn get_abstract_method_name(&self) -> Option<JsString>;
    // port: CodingConvention#getSingletonGetterClassName
    fn get_singleton_getter_class_name(&self, ast: &Ast, call_node: NodeId) -> Option<JsString>;
    // port: CodingConvention#applySingletonGetter
    fn apply_singleton_getter(
        &self,
        ast: &Ast,
        reg: &mut JSTypeRegistry,
        class_type: &NominalTypeBuilder,
        getter_type: TypeId,
    );
    /// Rust-only `reg`: the registry the `checkTypes` branch needs to restrict the callee's
    /// JSType; `None` when the caller has no registry (Java's
    /// `PeepholeSubstituteAlternateSyntax` passes `checkTypes = false`, which never reads it).
    // port: CodingConvention#describeFunctionBind
    fn describe_function_bind(
        &self,
        ast: &Ast,
        reg: Option<&mut JSTypeRegistry>,
        n: NodeId,
        check_types: bool,
    ) -> Option<Bind>;
    // port: CodingConvention#describeCachingCall
    fn describe_caching_call(&self, ast: &Ast, node: NodeId) -> Option<Cache>;
    // port: CodingConvention#isPropertyTestFunction
    fn is_property_test_function(&self, ast: &Ast, call: NodeId) -> bool;
    // port: CodingConvention#isPrototypeAlias
    fn is_prototype_alias(&self, ast: &Ast, get_prop: NodeId) -> bool;
    // port: CodingConvention#isPropertyRenameFunction
    fn is_property_rename_function(&self, ast: &Ast, name_node: NodeId) -> bool;
    // port: CodingConvention#getObjectLiteralCast
    fn get_object_literal_cast(&self, ast: &Ast, call_node: NodeId) -> Option<ObjectLiteralCast>;
    // port: CodingConvention#getAssertionFunctions
    fn get_assertion_functions(&self) -> Vec<AssertionFunctionSpec>;
    // port: CodingConvention#isExported(String) (one-argument overload)
    fn is_exported_name(&self, name: &JsString) -> bool {
        self.is_exported(name, true) || self.is_exported(name, false)
    }
    // port: CodingConvention#blockRenamingForProperty
    fn block_renaming_for_property(&self, _name: &JsString) -> bool {
        false
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bind {
    pub target: NodeId,
    pub this_value: Option<NodeId>,
    pub parameters: Option<NodeId>,
}
impl Bind {
    // port: CodingConvention.Bind#Bind
    pub fn new(target: NodeId, this_value: Option<NodeId>, parameters: Option<NodeId>) -> Self {
        Self {
            target,
            this_value,
            parameters,
        }
    }
    // port: CodingConvention.Bind#getBoundParameterCount
    pub fn get_bound_parameter_count(&self, ast: &Ast) -> i32 {
        let Some(parameters) = self.parameters else {
            return 0;
        };
        let param_parent = parameters.get_parent(ast).unwrap();
        param_parent.get_child_count(ast) - param_parent.get_index_of_child(ast, parameters)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cache {
    pub cache_obj: NodeId,
    pub key: NodeId,
    pub value_fn: NodeId,
    pub key_fn: Option<NodeId>,
}
impl Cache {
    // port: CodingConvention.Cache#Cache
    pub fn new(cache_obj: NodeId, key: NodeId, value_fn: NodeId, key_fn: Option<NodeId>) -> Self {
        Self {
            cache_obj,
            key,
            value_fn,
            key_fn,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubclassType {
    INHERITS,
    MIXIN,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubclassRelationship {
    pub r#type: SubclassType,
    pub subclass_name: JsString,
    pub superclass_name: JsString,
}
impl SubclassRelationship {
    // port: CodingConvention.SubclassRelationship#SubclassRelationship
    pub fn new(
        ast: &Ast,
        type_: SubclassType,
        subclass_node: NodeId,
        superclass_node: NodeId,
    ) -> Self {
        closure_rhino::check_argument!(
            subclass_node.is_qualified_name(ast),
            "Expected qualified name, found: %s",
            subclass_node.to_string(ast)
        );
        closure_rhino::check_argument!(
            superclass_node.is_qualified_name(ast),
            "Expected qualified name, found: %s",
            superclass_node.to_string(ast)
        );
        Self {
            r#type: type_,
            subclass_name: subclass_node.get_qualified_name(ast).unwrap(),
            superclass_name: superclass_node.get_qualified_name(ast).unwrap(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectLiteralCast {
    pub type_name: Option<JsString>,
    pub object_node: Option<NodeId>,
    pub diagnostic_type: Option<&'static DiagnosticType>,
}
impl ObjectLiteralCast {
    // port: CodingConvention.ObjectLiteralCast#ObjectLiteralCast
    pub fn new(
        type_name: Option<JsString>,
        object_node: Option<NodeId>,
        diagnostic_type: Option<&'static DiagnosticType>,
    ) -> Self {
        Self {
            type_name,
            object_node,
            diagnostic_type,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssertionKind {
    TRUTHY,
    MATCHES_RETURN_TYPE,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssertionFunctionSpec {
    function_name: Option<JsString>,
    closure_primitive: Option<ClosurePrimitive>,
    assertion_kind: AssertionKind,
    param_index: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssertionFunctionId {
    ClosurePrimitive(ClosurePrimitive),
    String(JsString),
}
impl std::hash::Hash for AssertionFunctionId {
    // port: CodingConvention.AssertionFunctionSpec#getId (key hash)
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::ClosurePrimitive(p) => {
                0u8.hash(state);
                (*p as u8).hash(state);
            }
            Self::String(s) => {
                1u8.hash(state);
                s.hash(state);
            }
        }
    }
}
impl AssertionFunctionSpec {
    // port: CodingConvention.AssertionFunctionSpec#getFunctionName
    pub fn get_function_name(&self) -> Option<&JsString> {
        self.function_name.as_ref()
    }
    // port: CodingConvention.AssertionFunctionSpec#getClosurePrimitive
    pub fn get_closure_primitive(&self) -> Option<ClosurePrimitive> {
        self.closure_primitive
    }
    // port: CodingConvention.AssertionFunctionSpec#getAssertionKind
    pub fn get_assertion_kind(&self) -> AssertionKind {
        self.assertion_kind
    }
    // port: CodingConvention.AssertionFunctionSpec#getParamIndex
    pub fn get_param_index(&self) -> i32 {
        self.param_index
    }
    // port: CodingConvention.AssertionFunctionSpec#builder
    pub fn builder() -> Builder {
        Builder::default().set_param_index(0)
    }
    // port: CodingConvention.AssertionFunctionSpec#forTruthy
    pub fn for_truthy() -> Builder {
        Self::builder().set_assertion_kind(AssertionKind::TRUTHY)
    }
    // port: CodingConvention.AssertionFunctionSpec#forMatchesReturn
    pub fn for_matches_return() -> Builder {
        Self::builder().set_assertion_kind(AssertionKind::MATCHES_RETURN_TYPE)
    }
    // port: CodingConvention.AssertionFunctionSpec#getId
    pub fn get_id(&self) -> AssertionFunctionId {
        self.closure_primitive.map_or_else(
            || AssertionFunctionId::String(self.function_name.clone().unwrap()),
            AssertionFunctionId::ClosurePrimitive,
        )
    }
    // port: CodingConvention.AssertionFunctionSpec#getAssertedArg
    pub fn get_asserted_arg(&self, ast: &Ast, mut first_arg: Option<NodeId>) -> Option<NodeId> {
        for _ in 0..self.get_param_index() {
            first_arg = first_arg?.get_next(ast);
        }
        first_arg
    }
}
#[derive(Default, Debug)]
pub struct Builder {
    function_name: Option<JsString>,
    closure_primitive: Option<ClosurePrimitive>,
    assertion_kind: Option<AssertionKind>,
    param_index: Option<i32>,
}
impl Builder {
    // port: CodingConvention.AssertionFunctionSpec.Builder#setFunctionName
    pub fn set_function_name(mut self, name: impl Into<JsString>) -> Self {
        self.function_name = Some(name.into());
        self
    }
    // port: CodingConvention.AssertionFunctionSpec.Builder#setClosurePrimitive
    pub fn set_closure_primitive(mut self, primitive: ClosurePrimitive) -> Self {
        self.closure_primitive = Some(primitive);
        self
    }
    // port: CodingConvention.AssertionFunctionSpec.Builder#setParamIndex
    pub fn set_param_index(mut self, param_index: i32) -> Self {
        self.param_index = Some(param_index);
        self
    }
    // port: CodingConvention.AssertionFunctionSpec.Builder#setAssertionKind
    pub fn set_assertion_kind(mut self, kind: AssertionKind) -> Self {
        self.assertion_kind = Some(kind);
        self
    }
    // port: CodingConvention.AssertionFunctionSpec.Builder#autoBuild
    pub fn auto_build(self) -> AssertionFunctionSpec {
        AssertionFunctionSpec {
            function_name: self.function_name,
            closure_primitive: self.closure_primitive,
            assertion_kind: self
                .assertion_kind
                .expect("Missing required properties: assertionKind"),
            param_index: self
                .param_index
                .expect("Missing required properties: paramIndex"),
        }
    }
    // port: CodingConvention.AssertionFunctionSpec.Builder#build
    pub fn build(self) -> AssertionFunctionSpec {
        let spec = self.auto_build();
        closure_rhino::check_state!(
            spec.get_function_name().is_some() || spec.get_closure_primitive().is_some(),
            "Must provide a function name or ClosurePrimitive for each spec"
        );
        spec
    }
}
#[derive(Clone, Debug)]
pub struct AssertionFunctionLookup {
    internal: closure_rhino::fx_hash::IndexMap<AssertionFunctionId, AssertionFunctionSpec>,
}
impl AssertionFunctionLookup {
    // port: CodingConvention.AssertionFunctionLookup#AssertionFunctionLookup
    fn new(
        internal: closure_rhino::fx_hash::IndexMap<AssertionFunctionId, AssertionFunctionSpec>,
    ) -> Self {
        Self { internal }
    }
    // port: CodingConvention.AssertionFunctionLookup#of
    pub fn of(specs: impl IntoIterator<Item = AssertionFunctionSpec>) -> Self {
        let mut internal = closure_rhino::fx_hash::IndexMap::<_, _>::default();
        for spec in specs {
            let id = spec.get_id();
            closure_rhino::check_argument!(
                !internal.contains_key(&id),
                "Multiple entries with same key"
            );
            internal.insert(id, spec);
        }
        Self::new(internal)
    }
    // port: CodingConvention.AssertionFunctionLookup#lookupByCallee
    pub fn lookup_by_callee(
        &self,
        ast: &Ast,
        reg: &JSTypeRegistry,
        callee: NodeId,
    ) -> Option<&AssertionFunctionSpec> {
        self.lookup_by_callee_with_optional_registry(ast, Some(reg), callee)
    }
    /// Rust-only: `lookupByCallee` for a compiler whose JSTypeRegistry is absent (never created,
    /// or cleared once types became colors); no node then carries a JSType.
    // port: CodingConvention.AssertionFunctionLookup#lookupByCallee
    pub fn lookup_by_callee_with_optional_registry(
        &self,
        ast: &Ast,
        reg: Option<&JSTypeRegistry>,
        callee: NodeId,
    ) -> Option<&AssertionFunctionSpec> {
        if let Some(fn_type) = callee
            .get_jstype(ast)
            .and_then(|t| t.to_maybe_function_type(reg.expect("a JSType without a JSTypeRegistry")))
        {
            if let Some(primitive) = fn_type.get_closure_primitive(reg.unwrap()) {
                if let Some(spec) = self
                    .internal
                    .get(&AssertionFunctionId::ClosurePrimitive(primitive))
                {
                    return Some(spec);
                }
            }
        }
        if callee.is_qualified_name(ast) {
            return self.internal.get(&AssertionFunctionId::String(
                callee.get_qualified_name(ast).unwrap(),
            ));
        }
        None
    }
}

impl SubclassType {
    pub const VALUES: &'static [Self] = &[Self::INHERITS, Self::MIXIN];
    // port: CodingConvention.SubclassType#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES
            .iter()
            .copied()
            .find(|value| value.to_string() == name)
    }
}
impl std::fmt::Display for SubclassType {
    // port: CodingConvention.SubclassType#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl AssertionKind {
    pub const VALUES: &'static [Self] = &[Self::TRUTHY, Self::MATCHES_RETURN_TYPE];
    // port: CodingConvention.AssertionFunctionSpec.AssertionKind#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES
            .iter()
            .copied()
            .find(|value| value.to_string() == name)
    }
}
impl std::fmt::Display for AssertionKind {
    // port: CodingConvention.AssertionFunctionSpec.AssertionKind#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
