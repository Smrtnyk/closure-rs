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
//   src/com/google/javascript/jscomp/TypeTransformation.java.

//! A class for processing type transformation expressions.
//!
//! Java's `com.google.javascript.jscomp.TypeTransformation`.
use crate::{
    abstract_compiler::AbstractCompiler, diagnostic_type::DiagnosticType, js_error::JSError,
    node_util::NodeUtil,
};
use closure_jstype::{
    JSTypeNative, TypeId,
    prelude::{FunctionType, JSType, ObjectType},
    static_typed_scope::StaticTypedScope,
};
use closure_parsing::type_transformation_parser::{Keywords, OperationKind};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{
    check_argument, js_string::JsString, js_type_expression::JSTypeExpression, node::NodeId,
};
use std::sync::Arc;

const VIRTUAL_FILE: &str = "<TypeTransformation.java>";

pub static UNKNOWN_TYPEVAR: DiagnosticType = DiagnosticType::warning(
    "TYPEVAR_UNDEFINED",
    "Reference to an unknown type variable {0}",
);
pub static UNKNOWN_STRVAR: DiagnosticType = DiagnosticType::warning(
    "UNKNOWN_STRVAR",
    "Reference to an unknown string variable {0}",
);
pub static UNKNOWN_TYPENAME: DiagnosticType = DiagnosticType::warning(
    "TYPENAME_UNDEFINED",
    "Reference to an unknown type name {0}",
);
pub static BASETYPE_INVALID: DiagnosticType =
    DiagnosticType::warning("BASETYPE_INVALID", "The type {0} cannot be templatized");
pub static TEMPTYPE_INVALID: DiagnosticType = DiagnosticType::warning(
    "TEMPTYPE_INVALID",
    "Expected templatized type in {0} found {1}",
);
pub static INDEX_OUTOFBOUNDS: DiagnosticType = DiagnosticType::warning(
    "INDEX_OUTOFBOUNDS",
    "Index out of bounds in templateTypeOf: expected a number less than {0}, found {1}",
);
pub static DUPLICATE_VARIABLE: DiagnosticType =
    DiagnosticType::warning("DUPLICATE_VARIABLE", "The variable {0} is already defined");
// This warning is never exercised.
pub static UNKNOWN_NAMEVAR: DiagnosticType = DiagnosticType::warning(
    "UNKNOWN_NAMEVAR",
    "Reference to an unknown name variable {0}",
);
pub static RECTYPE_INVALID: DiagnosticType = DiagnosticType::warning(
    "RECTYPE_INVALID",
    "The first parameter of a maprecord must be a record type, found {0}",
);
pub static MAPRECORD_BODY_INVALID: DiagnosticType = DiagnosticType::warning(
    "MAPRECORD_BODY_INVALID",
    "The body of a maprecord function must evaluate to a record type or a no type, found {0}",
);
pub static VAR_UNDEFINED: DiagnosticType =
    DiagnosticType::warning("VAR_UNDEFINED", "Variable {0} is undefined in the scope");
pub static INVALID_CTOR: DiagnosticType =
    DiagnosticType::warning("INVALID_CTOR", "Expected a constructor type, found {0}");
pub static RECPARAM_INVALID: DiagnosticType =
    DiagnosticType::warning("RECPARAM_INVALID", "Expected a record type, found {0}");
pub static PROPTYPE_INVALID: DiagnosticType =
    DiagnosticType::warning("PROPTYPE_INVALID", "Expected object type, found {0}");

/// Java's `ImmutableMap<String, JSType>` of type variables (insertion ordered).
pub type TypeVars = IndexMap<JsString, TypeId>;
/// Java's `ImmutableMap<String, String>` of name variables (insertion ordered).
pub type NameVars = IndexMap<JsString, JsString>;

/// A helper class for holding the information about the type variables and the name variables in
/// maprecord expressions.
struct NameResolver {
    type_vars: TypeVars,
    name_vars: NameVars,
}

impl NameResolver {
    // port: TypeTransformation.NameResolver#NameResolver
    fn new(type_vars: TypeVars, name_vars: NameVars) -> Self {
        Self {
            type_vars,
            name_vars,
        }
    }
}

/// The compiler and its registry are passed to each call (DESIGN §6).
pub struct TypeTransformation {
    type_env: Arc<dyn StaticTypedScope>,
}

impl TypeTransformation {
    // port: TypeTransformation#TypeTransformation
    pub fn new(type_env: Arc<dyn StaticTypedScope>) -> Self {
        Self { type_env }
    }

    // port: TypeTransformation#isTypeVar
    fn is_type_var(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        n.is_name(compiler)
    }

    // port: TypeTransformation#isTypeName
    fn is_type_name(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        n.is_string_lit(compiler)
    }

    // port: TypeTransformation#isBooleanOperation
    fn is_boolean_operation(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        n.is_and(compiler) || n.is_or(compiler) || n.is_not(compiler)
    }

    // port: TypeTransformation#nameToKeyword
    fn name_to_keyword(&self, s: &JsString) -> Keywords {
        // Keywords.valueOf(Ascii.toUpperCase(s))
        let upper = s.to_string_lossy().to_ascii_uppercase();
        *Keywords::VALUES
            .iter()
            .find(|k| format!("{k:?}") == upper)
            .unwrap_or_else(|| {
                panic!(
                    "No enum constant com.google.javascript.jscomp.parsing.TypeTransformationParser.Keywords.{upper}"
                )
            })
    }

    // port: TypeTransformation#getType
    fn get_type(&self, compiler: &mut AbstractCompiler, type_name: &JsString) -> Option<TypeId> {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let type_ = registry.get_type(ast, Some(&*self.type_env), type_name.clone());
        if type_.is_some() {
            return type_;
        }
        let slot = self.type_env.get_slot(type_name);
        let type_ = slot.and_then(|slot| slot.get_type(registry));
        if let Some(type_) = type_ {
            if type_.is_constructor(registry) || type_.is_interface(registry) {
                return Some(
                    type_
                        .to_maybe_function_type(registry)
                        .unwrap()
                        .get_instance_type(registry)
                        .unwrap()
                        .get_raw_type(registry),
                );
            }
            if type_.is_enum_element_type(registry) {
                return type_.get_enumerated_type_of_enum_element(registry);
            }
            return Some(type_);
        }
        let jsdoc = slot.and_then(|slot| slot.get_jsdoc_info(registry));
        if let Some(jsdoc) = jsdoc
            && jsdoc.has_typedef_type()
        {
            let expr = jsdoc.get_typedef_type().unwrap().clone();
            return Some(registry.evaluate_type_expression(
                ast,
                &expr,
                Some(Arc::clone(&self.type_env)),
            ));
        }
        None
    }

    // port: TypeTransformation#getUnknownType
    fn get_unknown_type(&self, compiler: &mut AbstractCompiler) -> TypeId {
        compiler
            .get_type_registry()
            .get_native_object_type(JSTypeNative::UNKNOWN_TYPE)
    }

    // port: TypeTransformation#getNoType
    fn get_no_type(&self, compiler: &mut AbstractCompiler) -> TypeId {
        compiler
            .get_type_registry()
            .get_native_object_type(JSTypeNative::NO_TYPE)
    }

    // port: TypeTransformation#getAllType
    fn get_all_type(&self, compiler: &mut AbstractCompiler) -> TypeId {
        compiler
            .get_type_registry()
            .get_native_type(JSTypeNative::ALL_TYPE)
    }

    // port: TypeTransformation#getObjectType
    fn get_object_type(&self, compiler: &mut AbstractCompiler) -> TypeId {
        compiler
            .get_type_registry()
            .get_native_type(JSTypeNative::OBJECT_TYPE)
    }

    // port: TypeTransformation#createUnionType
    fn create_union_type(&self, compiler: &mut AbstractCompiler, variants: &[TypeId]) -> TypeId {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        registry.create_union_type(ast, variants)
    }

    // port: TypeTransformation#createRecordType
    fn create_record_type(
        &self,
        compiler: &mut AbstractCompiler,
        props: IndexMap<JsString, TypeId>,
    ) -> TypeId {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        registry.create_record_type(ast, props)
    }

    // port: TypeTransformation#reportWarning
    fn report_warning(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        msg: &'static DiagnosticType,
        param: &[&str],
    ) {
        compiler.report(JSError::make(compiler, n, msg, param));
    }

    // port: TypeTransformation#addNewEntry
    fn add_new_entry<T: Clone>(
        &self,
        map: &IndexMap<JsString, T>,
        name: JsString,
        type_: T,
    ) -> IndexMap<JsString, T> {
        // ImmutableMap.Builder#buildOrThrow: the callers check that the key is new.
        let mut out = map.clone();
        let previous = out.insert(name.clone(), type_);
        check_argument!(
            previous.is_none(),
            "Multiple entries with same key: {}",
            name
        );
        out
    }

    // port: TypeTransformation#getFunctionParameter
    fn get_function_parameter(&self, compiler: &AbstractCompiler, n: NodeId, i: i32) -> JsString {
        check_argument!(
            n.is_function(compiler),
            "Expected a function node, found {}",
            n.to_string(compiler)
        );
        n.get_second_child(compiler)
            .unwrap()
            .get_child_at_index(compiler, i)
            .unwrap()
            .get_string(compiler)
    }

    // port: TypeTransformation#getCallName
    fn get_call_name(&self, compiler: &AbstractCompiler, n: NodeId) -> JsString {
        check_argument!(
            n.is_call(compiler),
            "Expected a call node, found {}",
            n.to_string(compiler)
        );
        n.get_first_child(compiler).unwrap().get_string(compiler)
    }

    // port: TypeTransformation#getCallArgument
    fn get_call_argument(&self, compiler: &AbstractCompiler, n: NodeId, i: i32) -> NodeId {
        check_argument!(
            n.is_call(compiler),
            "Expected a call node, found {}",
            n.to_string(compiler)
        );
        n.get_child_at_index(compiler, i + 1).unwrap()
    }

    // port: TypeTransformation#getCallParamCount
    fn get_call_param_count(&self, compiler: &AbstractCompiler, n: NodeId) -> i32 {
        check_argument!(
            n.is_call(compiler),
            "Expected a call node, found {}",
            n.to_string(compiler)
        );
        n.get_child_count(compiler) - 1
    }

    // port: TypeTransformation#getCallParams
    fn get_call_params(&self, compiler: &AbstractCompiler, n: NodeId) -> Vec<NodeId> {
        check_argument!(
            n.is_call(compiler),
            "Expected a call node, found {}",
            n.to_string(compiler)
        );
        let mut builder = Vec::new();
        for i in 0..self.get_call_param_count(compiler, n) {
            builder.push(self.get_call_argument(compiler, n, i));
        }
        builder
    }

    // port: TypeTransformation#getComputedPropValue
    fn get_computed_prop_value(&self, compiler: &AbstractCompiler, n: NodeId) -> NodeId {
        check_argument!(
            n.is_computed_prop(compiler),
            "Expected a computed property node, found {}",
            n.to_string(compiler)
        );
        n.get_second_child(compiler).unwrap()
    }

    // port: TypeTransformation#getComputedPropName
    fn get_computed_prop_name(&self, compiler: &AbstractCompiler, n: NodeId) -> JsString {
        check_argument!(
            n.is_computed_prop(compiler),
            "Expected a computed property node, found {}",
            n.to_string(compiler)
        );
        n.get_first_child(compiler).unwrap().get_string(compiler)
    }

    // port: TypeTransformation#eval(Node,ImmutableMap)
    /// Evaluates the type transformation expression and returns the resulting type.
    pub fn eval(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        type_vars: TypeVars,
    ) -> TypeId {
        self.eval_with_name_vars(compiler, ttl_ast, type_vars, IndexMap::<_, _>::default())
    }

    // port: TypeTransformation#eval(Node,ImmutableMap,ImmutableMap)
    /// Evaluates the type transformation expression and returns the resulting type.
    pub fn eval_with_name_vars(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        type_vars: TypeVars,
        name_vars: NameVars,
    ) -> TypeId {
        let result =
            self.eval_internal(compiler, ttl_ast, &NameResolver::new(type_vars, name_vars));
        if result.is_empty_type(compiler.get_type_registry()) {
            self.get_unknown_type(compiler)
        } else {
            result
        }
    }

    // port: TypeTransformation#evalInternal
    fn eval_internal(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        if self.is_type_name(compiler, ttl_ast) {
            return self.eval_type_name(compiler, ttl_ast);
        }
        if self.is_type_var(compiler, ttl_ast) {
            return self.eval_type_var(compiler, ttl_ast, name_resolver);
        }
        let name = self.get_call_name(compiler, ttl_ast);
        let keyword = self.name_to_keyword(&name);
        match keyword.properties().3 {
            OperationKind::TYPE_CONSTRUCTOR => {
                self.eval_type_expression(compiler, ttl_ast, name_resolver)
            }
            OperationKind::OPERATION => {
                self.eval_operation_expression(compiler, ttl_ast, name_resolver)
            }
            _ => panic!("Could not evaluate the type transformation expression"),
        }
    }

    // port: TypeTransformation#evalOperationExpression
    fn eval_operation_expression(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let name = self.get_call_name(compiler, ttl_ast);
        let keyword = self.name_to_keyword(&name);
        match keyword {
            Keywords::COND => self.eval_conditional(compiler, ttl_ast, name_resolver),
            Keywords::MAPUNION => self.eval_mapunion(compiler, ttl_ast, name_resolver),
            Keywords::MAPRECORD => self.eval_maprecord(compiler, ttl_ast, name_resolver),
            Keywords::TYPEOFVAR => self.eval_type_of_var(compiler, ttl_ast),
            Keywords::INSTANCEOF => self.eval_instance_of(compiler, ttl_ast, name_resolver),
            Keywords::PRINTTYPE => self.eval_print_type(compiler, ttl_ast, name_resolver),
            Keywords::PROPTYPE => self.eval_prop_type(compiler, ttl_ast, name_resolver),
            _ => panic!("Invalid type transformation operation"),
        }
    }

    // port: TypeTransformation#evalTypeExpression
    fn eval_type_expression(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let name = self.get_call_name(compiler, ttl_ast);
        let keyword = self.name_to_keyword(&name);
        match keyword {
            Keywords::TYPE => self.eval_templatized_type(compiler, ttl_ast, name_resolver),
            Keywords::UNION => self.eval_union_type(compiler, ttl_ast, name_resolver),
            Keywords::NONE => self.get_no_type(compiler),
            Keywords::ALL => self.get_all_type(compiler),
            Keywords::UNKNOWN => self.get_unknown_type(compiler),
            Keywords::RAWTYPEOF => self.eval_raw_type_of(compiler, ttl_ast, name_resolver),
            Keywords::TEMPLATETYPEOF => {
                self.eval_template_type_of(compiler, ttl_ast, name_resolver)
            }
            Keywords::RECORD => self.eval_record_type(compiler, ttl_ast, name_resolver),
            Keywords::TYPEEXPR => self.eval_native_type_expr(compiler, ttl_ast),
            _ => panic!("Invalid type expression"),
        }
    }

    // port: TypeTransformation#evalTypeName
    fn eval_type_name(&self, compiler: &mut AbstractCompiler, ttl_ast: NodeId) -> TypeId {
        let type_name = ttl_ast.get_string(compiler);
        let resulting_type = self.get_type(compiler, &type_name);
        // If the type name is not defined then return UNKNOWN and report a warning
        let Some(resulting_type) = resulting_type else {
            self.report_warning(
                compiler,
                ttl_ast,
                &UNKNOWN_TYPENAME,
                &[&type_name.to_string()],
            );
            return self.get_unknown_type(compiler);
        };
        resulting_type
    }

    // port: TypeTransformation#evalTemplatizedType
    fn eval_templatized_type(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let params = self.get_call_params(compiler, ttl_ast);
        let first_param = self.eval_internal(compiler, params[0], name_resolver);
        if !first_param.is_raw_type_of_templatized_type(compiler.get_type_registry()) {
            let s = self.type_to_string(compiler, first_param);
            self.report_warning(compiler, ttl_ast, &BASETYPE_INVALID, &[&s]);
            return self.get_unknown_type(compiler);
        }
        // TODO(lpino): Check that the number of parameters correspond with the
        // number of template types that the base type can take when creating
        // a templatized type. For instance, if the base type is Array then there
        // must be just one parameter.
        let mut templatized_types = Vec::with_capacity(params.len() - 1);
        for i in 0..params.len() - 1 {
            templatized_types.push(self.eval_internal(compiler, params[i + 1], name_resolver));
        }
        let base_type = first_param
            .to_maybe_object_type(compiler.get_type_registry())
            .unwrap();
        let (registry, ast) = compiler.get_type_registry_and_ast();
        registry.create_templatized_type(ast, base_type, &templatized_types)
    }

    // port: TypeTransformation#evalTypeVar
    fn eval_type_var(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let type_var = ttl_ast.get_string(compiler);
        let resulting_type = name_resolver.type_vars.get(&type_var).copied();
        // If the type variable is not defined then return UNKNOWN and report a warning
        let Some(resulting_type) = resulting_type else {
            self.report_warning(
                compiler,
                ttl_ast,
                &UNKNOWN_TYPEVAR,
                &[&type_var.to_string()],
            );
            return self.get_unknown_type(compiler);
        };
        resulting_type
    }

    // port: TypeTransformation#evalUnionType
    fn eval_union_type(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        // Get the parameters of the union
        let params = self.get_call_params(compiler, ttl_ast);
        let param_count = params.len();
        // Create an array of types after evaluating each parameter
        let mut basic_types = Vec::with_capacity(param_count);
        for param in params.iter().take(param_count) {
            basic_types.push(self.eval_internal(compiler, *param, name_resolver));
        }
        self.create_union_type(compiler, &basic_types)
    }

    // port: TypeTransformation#evalTypeParams
    fn eval_type_params(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> Vec<TypeId> {
        let params = self.get_call_params(compiler, ttl_ast);
        let param_count = params.len();
        let mut result = Vec::with_capacity(param_count);
        for param in params.iter().take(param_count) {
            result.push(self.eval_internal(compiler, *param, name_resolver));
        }
        result
    }

    // port: TypeTransformation#evalString
    fn eval_string(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> JsString {
        if ttl_ast.is_name(compiler) {
            // Return the empty string if the name variable cannot be resolved
            let name = ttl_ast.get_string(compiler);
            if !name_resolver.name_vars.contains_key(&name) {
                self.report_warning(compiler, ttl_ast, &UNKNOWN_STRVAR, &[&name.to_string()]);
                return JsString::from("");
            }
            return name_resolver.name_vars.get(&name).unwrap().clone();
        }
        ttl_ast.get_string(compiler)
    }

    // port: TypeTransformation#evalStringParams
    fn eval_string_params(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> Vec<JsString> {
        let params = self.get_call_params(compiler, ttl_ast);
        let param_count = params.len();
        let mut result = Vec::with_capacity(param_count);
        for param in params.iter().take(param_count) {
            result.push(self.eval_string(compiler, *param, name_resolver));
        }
        result
    }

    // port: TypeTransformation#evalTypePredicate
    fn eval_type_predicate(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> bool {
        let params = self.eval_type_params(compiler, ttl_ast, name_resolver);
        let name = self.get_call_name(compiler, ttl_ast);
        let keyword = self.name_to_keyword(&name);
        let type_ = params[0];
        let (registry, ast) = compiler.get_type_registry_and_ast();
        match keyword {
            Keywords::EQ => type_.equals(registry, ast, params[1]),
            Keywords::SUB => type_.is_subtype_of(registry, ast, params[1]),
            Keywords::ISCTOR => type_.is_constructor(registry),
            Keywords::ISTEMPLATIZED => type_.is_templatized_type(registry),
            Keywords::ISRECORD => type_.is_record_type(registry),
            Keywords::ISUNKNOWN => type_.is_unknown_type(registry, ast),
            _ => panic!("Invalid type predicate in the type transformation"),
        }
    }

    // port: TypeTransformation#evalStringPredicate
    fn eval_string_predicate(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> bool {
        let params = self.eval_string_params(compiler, ttl_ast, name_resolver);
        // If any of the parameters evaluates to the empty string then they were
        // not resolved by the name resolver. In this case we always return false.
        for param in &params {
            if param.is_empty() {
                return false;
            }
        }
        let name = self.get_call_name(compiler, ttl_ast);
        let keyword = self.name_to_keyword(&name);
        match keyword {
            Keywords::STREQ => params[0] == params[1],
            _ => panic!("Invalid string predicate in the type transformation"),
        }
    }

    // port: TypeTransformation#evalTypevarPredicate
    fn eval_typevar_predicate(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> bool {
        let name = self.get_call_name(compiler, ttl_ast);
        let keyword = self.name_to_keyword(&name);
        match keyword {
            Keywords::ISDEFINED => name_resolver.type_vars.contains_key(
                &self
                    .get_call_argument(compiler, ttl_ast, 0)
                    .get_string(compiler),
            ),
            _ => panic!("Invalid typevar predicate in the type transformation"),
        }
    }

    // port: TypeTransformation#evalBooleanOperation
    fn eval_boolean_operation(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> bool {
        let param0 = self.eval_boolean(
            compiler,
            ttl_ast.get_first_child(compiler).unwrap(),
            name_resolver,
        );
        if ttl_ast.is_not(compiler) {
            return !param0;
        }
        if ttl_ast.is_and(compiler) {
            return param0
                && self.eval_boolean(
                    compiler,
                    ttl_ast.get_last_child(compiler).unwrap(),
                    name_resolver,
                );
        }
        if ttl_ast.is_or(compiler) {
            return param0
                || self.eval_boolean(
                    compiler,
                    ttl_ast.get_last_child(compiler).unwrap(),
                    name_resolver,
                );
        }
        panic!("Invalid boolean predicate in the type transformation");
    }

    // port: TypeTransformation#evalBoolean
    fn eval_boolean(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> bool {
        if self.is_boolean_operation(compiler, ttl_ast) {
            return self.eval_boolean_operation(compiler, ttl_ast, name_resolver);
        }
        let name = self.get_call_name(compiler, ttl_ast);
        let keyword = self.name_to_keyword(&name);
        match keyword.properties().3 {
            OperationKind::STRING_PREDICATE => {
                self.eval_string_predicate(compiler, ttl_ast, name_resolver)
            }
            OperationKind::TYPE_PREDICATE => {
                self.eval_type_predicate(compiler, ttl_ast, name_resolver)
            }
            OperationKind::TYPEVAR_PREDICATE => {
                self.eval_typevar_predicate(compiler, ttl_ast, name_resolver)
            }
            _ => panic!("Invalid boolean predicate in the type transformation"),
        }
    }

    // port: TypeTransformation#evalConditional
    fn eval_conditional(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let params = self.get_call_params(compiler, ttl_ast);
        if self.eval_boolean(compiler, params[0], name_resolver) {
            self.eval_internal(compiler, params[1], name_resolver)
        } else {
            self.eval_internal(compiler, params[2], name_resolver)
        }
    }

    // port: TypeTransformation#evalMapunion
    fn eval_mapunion(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let params = self.get_call_params(compiler, ttl_ast);
        let union_param = params[0];
        let map_function = params[1];
        let param_name = self.get_function_parameter(compiler, map_function, 0);

        // The mapunion variable must not be defined in the environment
        if name_resolver.type_vars.contains_key(&param_name) {
            self.report_warning(
                compiler,
                ttl_ast,
                &DUPLICATE_VARIABLE,
                &[&param_name.to_string()],
            );
            return self.get_unknown_type(compiler);
        }

        let map_function_body = NodeUtil::get_function_body(compiler, map_function);
        let union_type = self.eval_internal(compiler, union_param, name_resolver);
        // If the first parameter does not correspond to a union type then
        // consider it as a union with a single type and evaluate
        if !union_type.is_union_type(compiler.get_type_registry()) {
            let new_name_resolver = NameResolver::new(
                self.add_new_entry(&name_resolver.type_vars, param_name, union_type),
                name_resolver.name_vars.clone(),
            );
            return self.eval_internal(compiler, map_function_body, &new_name_resolver);
        }

        // Otherwise obtain the elements in the union type. Note that the block
        // above guarantees the casting to be safe
        let union_elms: Vec<TypeId> = {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            union_type
                .get_union_members(registry, ast)
                .unwrap()
                .to_vec()
        };
        // Evaluate the map function body using each element in the union type
        let union_size = union_elms.len();
        let mut new_union_elms = Vec::with_capacity(union_size);
        for elm in union_elms {
            let new_name_resolver = NameResolver::new(
                self.add_new_entry(&name_resolver.type_vars, param_name.clone(), elm),
                name_resolver.name_vars.clone(),
            );
            new_union_elms.push(self.eval_internal(
                compiler,
                map_function_body,
                &new_name_resolver,
            ));
        }

        self.create_union_type(compiler, &new_union_elms)
    }

    // port: TypeTransformation#evalRawTypeOf
    fn eval_raw_type_of(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let params = self.get_call_params(compiler, ttl_ast);
        let type_ = self.eval_internal(compiler, params[0], name_resolver);
        if !type_.is_templatized_type(compiler.get_type_registry()) {
            let s = self.type_to_string(compiler, type_);
            self.report_warning(compiler, ttl_ast, &TEMPTYPE_INVALID, &["rawTypeOf", &s]);
            return self.get_unknown_type(compiler);
        }
        let registry = compiler.get_type_registry();
        type_
            .to_maybe_object_type(registry)
            .unwrap()
            .get_raw_type(registry)
    }

    // port: TypeTransformation#evalTemplateTypeOf
    fn eval_template_type_of(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let params = self.get_call_params(compiler, ttl_ast);
        let type_ = self.eval_internal(compiler, params[0], name_resolver);
        if !type_.is_templatized_type(compiler.get_type_registry()) {
            let s = self.type_to_string(compiler, type_);
            self.report_warning(
                compiler,
                ttl_ast,
                &TEMPTYPE_INVALID,
                &["templateTypeOf", &s],
            );
            return self.get_unknown_type(compiler);
        }
        // (int) params.get(1).getDouble()
        let index = params[1].get_double(compiler) as i32;
        let registry = compiler.get_type_registry();
        let template_types = type_
            .to_maybe_object_type(registry)
            .unwrap()
            .get_template_types(registry)
            .unwrap();
        if index >= template_types.len() as i32 {
            self.report_warning(
                compiler,
                ttl_ast,
                &INDEX_OUTOFBOUNDS,
                &[&template_types.len().to_string(), &index.to_string()],
            );
            return self.get_unknown_type(compiler);
        }
        template_types[index as usize]
    }

    // port: TypeTransformation#evalRecord
    fn eval_record(
        &self,
        compiler: &mut AbstractCompiler,
        record: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let mut props: IndexMap<JsString, TypeId> = IndexMap::<_, _>::default();
        let mut prop_node = record.get_first_child(compiler);
        while let Some(prop) = prop_node {
            // If it is a computed property then find the property name using the resolver
            if prop.is_computed_prop(compiler) {
                let comp_prop_name = self.get_computed_prop_name(compiler, prop);
                // If the name does not exist then report a warning
                if !name_resolver.name_vars.contains_key(&comp_prop_name) {
                    self.report_warning(
                        compiler,
                        record,
                        &UNKNOWN_NAMEVAR,
                        &[&comp_prop_name.to_string()],
                    );
                    return self.get_unknown_type(compiler);
                }
                // Otherwise add the property
                let prop_value = self.get_computed_prop_value(compiler, prop);
                let resolved_name = name_resolver
                    .name_vars
                    .get(&comp_prop_name)
                    .unwrap()
                    .clone();
                let resulting_type = self.eval_internal(compiler, prop_value, name_resolver);
                props.insert(resolved_name, resulting_type);
            } else {
                let prop_name = prop.get_string(compiler);
                let resulting_type = self.eval_internal(
                    compiler,
                    prop.get_first_child(compiler).unwrap(),
                    name_resolver,
                );
                props.insert(prop_name, resulting_type);
            }
            prop_node = prop.get_next(compiler);
        }
        let (registry, ast) = compiler.get_type_registry_and_ast();
        registry.create_record_type(ast, props)
    }

    // port: TypeTransformation#evalRecordParam
    fn eval_record_param(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        if ttl_ast.is_object_lit(compiler) {
            return self.eval_record(compiler, ttl_ast, name_resolver);
        }
        // The parameter of record can be a type transformation expression
        self.eval_internal(compiler, ttl_ast, name_resolver)
    }

    // port: TypeTransformation#evalRecordType
    fn eval_record_type(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let param_count = self.get_call_param_count(compiler, ttl_ast);
        let mut rec_types_builder = Vec::new();
        for i in 0..param_count {
            let arg = self.get_call_argument(compiler, ttl_ast, i);
            let type_ = self.eval_record_param(compiler, arg, name_resolver);
            // Check that each parameter evaluates to an object
            let obj_type = type_.to_maybe_object_type(compiler.get_type_registry());
            let invalid = match obj_type {
                None => true,
                Some(obj_type) => {
                    let (registry, ast) = compiler.get_type_registry_and_ast();
                    obj_type.is_unknown_type(registry, ast)
                }
            };
            if invalid {
                let s = self.type_to_string(compiler, type_);
                self.report_warning(compiler, ttl_ast, &RECPARAM_INVALID, &[&s]);
                return self.get_unknown_type(compiler);
            }
            let rec_type = {
                let (registry, ast) = compiler.get_type_registry_and_ast();
                registry.build_record_type_from_object(ast, obj_type.unwrap())
            };
            let object_type = self.get_object_type(compiler);
            let (registry, ast) = compiler.get_type_registry_and_ast();
            if !rec_type.equals(registry, ast, object_type) {
                rec_types_builder.push(rec_type.to_maybe_object_type(registry).unwrap());
            }
        }
        self.join_record_types(compiler, &rec_types_builder)
    }

    // port: TypeTransformation#putNewPropInPropertyMap
    fn put_new_prop_in_property_map(
        &self,
        compiler: &mut AbstractCompiler,
        props: &mut IndexMap<JsString, TypeId>,
        new_prop_name: JsString,
        new_prop_value: TypeId,
    ) {
        // TODO(lpino): Decide if the best strategy is to collapse the properties
        // to a union type or not. So far, new values replace the old ones except
        // if they are two record types in which case the properties are joined
        // together

        // Three cases:
        // (i) If the key does not exist then add it to the map with the new value
        // (ii) If the key to be added already exists in the map and the new value
        // is not a record type then the current value is replaced with the new one
        // (iii) If the new value is a record type and the current is not then
        // the current value is replaced with the new one
        let registry = compiler.get_type_registry();
        if !props.contains_key(&new_prop_name)
            || !new_prop_value.is_record_type(registry)
            || !props.get(&new_prop_name).unwrap().is_record_type(registry)
        {
            props.insert(new_prop_name, new_prop_value);
            return;
        }
        // Otherwise join the current value with the new one since both are records
        let current = *props.get(&new_prop_name).unwrap();
        let joined = self.join_record_types(compiler, &[current, new_prop_value]);
        props.insert(new_prop_name, joined);
    }

    // port: TypeTransformation#joinRecordTypes
    /// Merges a list of record types.
    /// Example
    /// {r:{s:string, n:number}} and {a:boolean}
    /// is transformed into {r:{s:string, n:number}, a:boolean}
    fn join_record_types(&self, compiler: &mut AbstractCompiler, rec_types: &[TypeId]) -> TypeId {
        let mut props: IndexMap<JsString, TypeId> = IndexMap::<_, _>::default();
        for &rec_type in rec_types {
            let names = rec_type.get_own_property_names(compiler.get_type_registry());
            for new_prop_name in names {
                let new_prop_value = {
                    let (registry, ast) = compiler.get_type_registry_and_ast();
                    rec_type.get_property_type(registry, ast, new_prop_name.clone())
                };
                // Put the new property depending if it already exists in the map
                self.put_new_prop_in_property_map(
                    compiler,
                    &mut props,
                    new_prop_name,
                    new_prop_value,
                );
            }
        }
        self.create_record_type(compiler, props)
    }

    // port: TypeTransformation#evalMaprecord
    fn eval_maprecord(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let record_node = ttl_ast.get_second_child(compiler).unwrap();
        let map_function = ttl_ast.get_child_at_index(compiler, 2).unwrap();
        let type_ = self.eval_internal(compiler, record_node, name_resolver);

        // If it is an empty record type (Object) then return
        let object_type = self.get_object_type(compiler);
        {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            if type_.equals(registry, ast, object_type) {
                return object_type;
            }
        }

        // The parameter must be a valid record type
        if !type_.is_record_type(compiler.get_type_registry()) {
            // TODO(lpino): Decide how to handle non-record types
            let s = self.type_to_string(compiler, type_);
            self.report_warning(compiler, record_node, &RECTYPE_INVALID, &[&s]);
            return self.get_unknown_type(compiler);
        }

        let objtype = type_
            .to_maybe_object_type(compiler.get_type_registry())
            .unwrap();
        // Fetch the information of the map function
        let param_key = self.get_function_parameter(compiler, map_function, 0);
        let param_value = self.get_function_parameter(compiler, map_function, 1);

        // The maprecord variables must not be defined in the environment
        if name_resolver.name_vars.contains_key(&param_key) {
            self.report_warning(
                compiler,
                ttl_ast,
                &DUPLICATE_VARIABLE,
                &[&param_key.to_string()],
            );
            return self.get_unknown_type(compiler);
        }
        if name_resolver.type_vars.contains_key(&param_value) {
            self.report_warning(
                compiler,
                ttl_ast,
                &DUPLICATE_VARIABLE,
                &[&param_value.to_string()],
            );
            return self.get_unknown_type(compiler);
        }

        // Compute the new properties using the map function
        let map_fn_body = NodeUtil::get_function_body(compiler, map_function);
        let mut new_props: IndexMap<JsString, TypeId> = IndexMap::<_, _>::default();
        let prop_names = objtype.get_own_property_names(compiler.get_type_registry());
        for prop_name in prop_names {
            // The value of the current property
            let prop_value = {
                let (registry, ast) = compiler.get_type_registry_and_ast();
                objtype.get_property_type(registry, ast, prop_name.clone())
            };

            // Evaluate the map function body with paramValue and paramKey replaced
            // by the values of the current property
            let new_name_resolver = NameResolver::new(
                self.add_new_entry(&name_resolver.type_vars, param_value.clone(), prop_value),
                self.add_new_entry(&name_resolver.name_vars, param_key.clone(), prop_name),
            );
            let body = self.eval_internal(compiler, map_fn_body, &new_name_resolver);

            // If the body returns unknown then the whole expression returns unknown
            {
                let (registry, ast) = compiler.get_type_registry_and_ast();
                if body.is_unknown_type(registry, ast) {
                    return self.get_unknown_type(compiler);
                }
            }

            // Skip the property when the body evaluates to NO_TYPE
            // or the empty record (Object)
            let object_type = self.get_object_type(compiler);
            {
                let (registry, ast) = compiler.get_type_registry_and_ast();
                if body.is_empty_type(registry) || body.equals(registry, ast, object_type) {
                    continue;
                }
            }

            // Otherwise the body must evaluate to a record type
            if !body.is_record_type(compiler.get_type_registry()) {
                let s = self.type_to_string(compiler, body);
                self.report_warning(compiler, ttl_ast, &MAPRECORD_BODY_INVALID, &[&s]);
                return self.get_unknown_type(compiler);
            }

            // Add the properties of the resulting record type to the original one
            let body_as_obj = body
                .to_maybe_object_type(compiler.get_type_registry())
                .unwrap();
            let body_names = body_as_obj.get_own_property_names(compiler.get_type_registry());
            for new_prop_name in body_names {
                let new_prop_value = {
                    let (registry, ast) = compiler.get_type_registry_and_ast();
                    body_as_obj.get_property_type(registry, ast, new_prop_name.clone())
                };
                // If the key already exists then we have to mix it with the current property value
                self.put_new_prop_in_property_map(
                    compiler,
                    &mut new_props,
                    new_prop_name,
                    new_prop_value,
                );
            }
        }
        self.create_record_type(compiler, new_props)
    }

    // port: TypeTransformation#evalTypeOfVar
    fn eval_type_of_var(&self, compiler: &mut AbstractCompiler, ttl_ast: NodeId) -> TypeId {
        let name = self
            .get_call_argument(compiler, ttl_ast, 0)
            .get_string(compiler);
        let type_ = {
            let registry = compiler.get_type_registry();
            let slot = self.type_env.get_slot(&name);
            slot.and_then(|slot| slot.get_type(registry))
        };
        let Some(type_) = type_ else {
            self.report_warning(compiler, ttl_ast, &VAR_UNDEFINED, &[&name.to_string()]);
            return self.get_unknown_type(compiler);
        };
        type_
    }

    // port: TypeTransformation#evalInstanceOf
    fn eval_instance_of(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let arg = self.get_call_argument(compiler, ttl_ast, 0);
        let type_ = self.eval_internal(compiler, arg, name_resolver);
        let invalid = {
            let (registry, ast) = compiler.get_type_registry_and_ast();
            type_.is_unknown_type(registry, ast) || !type_.is_constructor(registry)
        };
        if invalid {
            let display_name = type_
                .get_display_name(compiler.get_type_registry())
                .unwrap_or_else(|| "null".to_owned());
            self.report_warning(compiler, ttl_ast, &INVALID_CTOR, &[&display_name]);
            return self.get_unknown_type(compiler);
        }
        let registry = compiler.get_type_registry();
        type_
            .to_maybe_function_type(registry)
            .unwrap()
            .get_instance_type(registry)
            .unwrap()
    }

    // port: TypeTransformation#evalNativeTypeExpr
    fn eval_native_type_expr(&self, compiler: &mut AbstractCompiler, ttl_ast: NodeId) -> TypeId {
        let expr =
            JSTypeExpression::new(self.get_call_argument(compiler, ttl_ast, 0), VIRTUAL_FILE);
        let (registry, ast) = compiler.get_type_registry_and_ast();
        registry.evaluate_type_expression(ast, &expr, Some(Arc::clone(&self.type_env)))
    }

    // port: TypeTransformation#evalPrintType
    fn eval_print_type(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let arg = self.get_call_argument(compiler, ttl_ast, 1);
        let type_ = self.eval_internal(compiler, arg, name_resolver);
        let msg = format!(
            "{}{}",
            self.get_call_argument(compiler, ttl_ast, 0)
                .get_string(compiler),
            self.type_to_string(compiler, type_)
        );
        println!("{msg}");
        type_
    }

    // port: TypeTransformation#evalPropType
    fn eval_prop_type(
        &self,
        compiler: &mut AbstractCompiler,
        ttl_ast: NodeId,
        name_resolver: &NameResolver,
    ) -> TypeId {
        let arg = self.get_call_argument(compiler, ttl_ast, 1);
        let type_ = self.eval_internal(compiler, arg, name_resolver);
        let Some(obj_type) = type_.to_maybe_object_type(compiler.get_type_registry()) else {
            let s = self.type_to_string(compiler, type_);
            self.report_warning(compiler, ttl_ast, &PROPTYPE_INVALID, &[&s]);
            return self.get_unknown_type(compiler);
        };
        let prop_name = self
            .get_call_argument(compiler, ttl_ast, 0)
            .get_string(compiler);
        let (registry, ast) = compiler.get_type_registry_and_ast();
        // firstNonNull(propType, getUnknownType()): getPropertyType never returns null here.
        obj_type.get_property_type(registry, ast, prop_name)
    }

    /// Rust-only: Java's `JSType#toString` (the registry and arena are passed explicitly).
    fn type_to_string(&self, compiler: &mut AbstractCompiler, type_: TypeId) -> String {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        type_.to_string(registry, ast)
    }
}
