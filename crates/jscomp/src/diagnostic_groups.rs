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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/DiagnosticGroups.java.

#![allow(clippy::vec_init_then_push)] // Preserve Java statement order and borrowing branches.
use crate::{
    check_level::CheckLevel, compiler_options::CompilerOptions, diagnostic_group::DiagnosticGroup,
    diagnostic_type::DiagnosticType,
};
use closure_rhino::fx_hash::IndexMap;
use std::sync::{Arc, LazyLock, Mutex};
pub static UNUSED: DiagnosticType = DiagnosticType::warning("JSC_UNUSED", "{0}");
pub const WILDCARD_EXCLUDED_GROUPS: &[&str] = &[
    "reportUnknownTypes",
    "analyzerChecks",
    "missingSourcesWarnings",
    "closureUnawareCodeAnnotationPresent",
];
pub const DIAGNOSTIC_GROUP_NAMES: &str = "accessControls, checkPrototypalTypes, checkRegExp, checkTypes, checkVars, conformanceViolations, const, constantProperty, deprecated, deprecatedAnnotations, defineWithoutGoogDefine, duplicateMessage, es5Strict, externsValidation, functionParams, globalThis, invalidCasts, lintVarDeclarations, misplacedTypeAnnotation, missingOverride, missingPolyfill, missingProperties, missingProvide, missingRequire, missingReturn, missingSourcesWarnings, moduleLoad, moduleImport, msgDescriptions, nonStandardJsDocs, partialAlias, polymer, reportUnknownTypes, strictCheckTypes, strictMissingProperties, strictModuleDepCheck, strictPrimitiveOperators, suspiciousCode, typeInvalidation, undefinedVars, underscore, unknownDefines, unusedLocalVariables, uselessCode, untranspilableFeatures,visibility, weakModuleGet";
struct Builtins {
    fields: IndexMap<&'static str, Arc<DiagnosticGroup>>,
    groups_by_name: IndexMap<String, Arc<DiagnosticGroup>>,
}
static BUILTINS: LazyLock<Builtins> = LazyLock::new(|| {
    let mut fields: IndexMap<&'static str, Arc<DiagnosticGroup>> = IndexMap::<_, _>::default();
    let mut groups_by_name = IndexMap::<_, _>::default();
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::report_untranspilable_features::UNTRANSPILABLE_FEATURE_PRESENT);
    let group = Arc::new(DiagnosticGroup::new_named("untranspilableFeatures", &types));
    groups_by_name.insert("untranspilableFeatures".into(), group.clone());
    fields.insert("UNTRANSPILABLE_FEATURES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::deps::module_loader::LOAD_WARNING);
    types.push(&crate::process_common_js_modules::SUSPICIOUS_EXPORTS_ASSIGNMENT);
    types.push(&crate::process_common_js_modules::UNKNOWN_REQUIRE_ENSURE);
    let group = Arc::new(DiagnosticGroup::new_named("moduleLoad", &types));
    groups_by_name.insert("moduleLoad".into(), group.clone());
    fields.insert("MODULE_LOAD", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::modules::module_map_creator::DOES_NOT_HAVE_EXPORT);
    types.push(&crate::modules::module_map_creator::DOES_NOT_HAVE_EXPORT_WITH_DETAILS);
    let group = Arc::new(DiagnosticGroup::new_named("moduleImport", &types));
    groups_by_name.insert("moduleImport".into(), group.clone());
    fields.insert("MODULE_IMPORT", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_global_this::GLOBAL_THIS);
    let group = Arc::new(DiagnosticGroup::new_named("globalThis", &types));
    groups_by_name.insert("globalThis".into(), group.clone());
    fields.insert("GLOBAL_THIS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_access_controls::DEPRECATED_NAME);
    types.push(&crate::check_access_controls::DEPRECATED_NAME_REASON);
    types.push(&crate::check_access_controls::DEPRECATED_PROP);
    types.push(&crate::check_access_controls::DEPRECATED_PROP_REASON);
    types.push(&crate::check_access_controls::DEPRECATED_CLASS);
    types.push(&crate::check_access_controls::DEPRECATED_CLASS_REASON);
    let group = Arc::new(DiagnosticGroup::new_named("deprecated", &types));
    groups_by_name.insert("deprecated".into(), group.clone());
    fields.insert("DEPRECATED", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&UNUSED);
    let group = Arc::new(DiagnosticGroup::new_named("underscore", &types));
    groups_by_name.insert("underscore".into(), group.clone());
    fields.insert("UNDERSCORE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_access_controls::BAD_PRIVATE_GLOBAL_ACCESS);
    types.push(&crate::check_access_controls::BAD_PRIVATE_PROPERTY_ACCESS);
    types.push(&crate::check_access_controls::BAD_PACKAGE_PROPERTY_ACCESS);
    types.push(&crate::check_access_controls::BAD_PROTECTED_PROPERTY_ACCESS);
    types.push(&crate::check_access_controls::EXTEND_FINAL_CLASS);
    types.push(&crate::check_access_controls::FINAL_PROPERTY_OVERRIDDEN);
    types.push(&crate::check_access_controls::PRIVATE_OVERRIDE);
    types.push(&crate::check_access_controls::VISIBILITY_MISMATCH);
    let group = Arc::new(DiagnosticGroup::new_named("visibility", &types));
    groups_by_name.insert("visibility".into(), group.clone());
    fields.insert("VISIBILITY", group);
    // port: DiagnosticGroups#registerGroup(String,DiagnosticGroup)
    // This Java overload registers an alias of the same group object.
    let group = fields["VISIBILITY"].clone();
    groups_by_name.insert("accessControls".into(), group.clone());
    fields.insert("ACCESS_CONTROLS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rhino_error_reporter::BAD_JSDOC_ANNOTATION);
    types.push(&crate::rhino_error_reporter::INVALID_PARAM);
    types.push(&crate::rhino_error_reporter::JSDOC_IMPORT_TYPE_WARNING);
    types.push(&crate::check_jsdoc::JSDOC_IN_BLOCK_COMMENT);
    let group = Arc::new(DiagnosticGroup::new_named("nonStandardJsDocs", &types));
    groups_by_name.insert("nonStandardJsDocs".into(), group.clone());
    fields.insert("NON_STANDARD_JSDOC", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::type_validator::INVALID_CAST);
    let group = Arc::new(DiagnosticGroup::new_named("invalidCasts", &types));
    groups_by_name.insert("invalidCasts".into(), group.clone());
    fields.insert("INVALID_CASTS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::var_check::STRICT_CHUNK_DEP_ERROR);
    types.push(&crate::check_closure_imports::CROSS_CHUNK_REQUIRE_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("strictModuleDepCheck", &types));
    groups_by_name.insert("strictModuleDepCheck".into(), group.clone());
    fields.insert("STRICT_MODULE_DEP_CHECK", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::var_check::VIOLATED_CHUNK_DEP_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("violatedModuleDep", &types));
    groups_by_name.insert("violatedModuleDep".into(), group.clone());
    fields.insert("VIOLATED_MODULE_DEP", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::var_check::NAME_REFERENCE_IN_EXTERNS_ERROR);
    types.push(&crate::var_check::UNDEFINED_EXTERN_VAR_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("externsValidation", &types));
    groups_by_name.insert("externsValidation".into(), group.clone());
    fields.insert("EXTERNS_VALIDATION", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::process_defines::UNKNOWN_DEFINE_WARNING);
    let group = Arc::new(DiagnosticGroup::new_named("unknownDefines", &types));
    groups_by_name.insert("unknownDefines".into(), group.clone());
    fields.insert("UNKNOWN_DEFINES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::process_defines::DEFINE_WITHOUT_GOOG_DEFINE);
    let group = Arc::new(DiagnosticGroup::new_named(
        "defineWithoutGoogDefine",
        &types,
    ));
    groups_by_name.insert("defineWithoutGoogDefine".into(), group.clone());
    fields.insert("DEFINE_WITHOUT_GOOG_DEFINE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::process_tweaks::INVALID_TWEAK_DEFAULT_VALUE_WARNING);
    types.push(&crate::process_tweaks::TWEAK_WRONG_GETTER_TYPE_WARNING);
    let group = Arc::new(DiagnosticGroup::new_named("tweakValidation", &types));
    groups_by_name.insert("tweakValidation".into(), group.clone());
    fields.insert("TWEAKS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::type_check::HIDDEN_INTERFACE_PROPERTY);
    types.push(&crate::type_check::HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY);
    types.push(&crate::type_check::HIDDEN_SUPERCLASS_PROPERTY);
    let group = Arc::new(DiagnosticGroup::new_named("missingOverride", &types));
    groups_by_name.insert("missingOverride".into(), group.clone());
    fields.insert("MISSING_OVERRIDE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::type_check::INEXISTENT_PROPERTY);
    types.push(&crate::type_check::INEXISTENT_PROPERTY_WITH_SUGGESTION);
    types.push(&crate::type_check::POSSIBLE_INEXISTENT_PROPERTY);
    let group = Arc::new(DiagnosticGroup::new_named("missingProperties", &types));
    groups_by_name.insert("missingProperties".into(), group.clone());
    fields.insert("MISSING_PROPERTIES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::type_check::POSSIBLE_INEXISTENT_PROPERTY);
    let group = Arc::new(DiagnosticGroup::new_named(
        "globallyMissingProperties",
        &types,
    ));
    groups_by_name.insert("globallyMissingProperties".into(), group.clone());
    fields.insert("GLOBALLY_MISSING_PROPERTIES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::j2cl_checks_pass::J2CL_REFERENCE_EQUALITY);
    let group = Arc::new(DiagnosticGroup::new_named("j2clChecks", &types));
    groups_by_name.insert("j2clChecks".into(), group.clone());
    fields.insert("J2CL_CHECKS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_missing_return::MISSING_RETURN_STATEMENT);
    let group = Arc::new(DiagnosticGroup::new_named("missingReturn", &types));
    groups_by_name.insert("missingReturn".into(), group.clone());
    fields.insert("MISSING_RETURN", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::var_check::UNDEFINED_VAR_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("undefinedVars", &types));
    groups_by_name.insert("undefinedVars".into(), group.clone());
    fields.insert("UNDEFINED_VARIABLES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_debugger_statement::DEBUGGER_STATEMENT_PRESENT);
    let group = Arc::new(DiagnosticGroup::new_named("checkDebuggerStatement", &types));
    groups_by_name.insert("checkDebuggerStatement".into(), group.clone());
    fields.insert("DEBUGGER_STATEMENT_PRESENT", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_reg_exp::REGEXP_REFERENCE);
    types.push(&crate::check_reg_exp::MALFORMED_REGEXP);
    let group = Arc::new(DiagnosticGroup::new_named("checkRegExp", &types));
    groups_by_name.insert("checkRegExp".into(), group.clone());
    fields.insert("CHECK_REGEXP", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.extend(
        crate::type_validator::ALL_DIAGNOSTICS
            .get_types()
            .iter()
            .copied(),
    );
    types.extend(
        crate::type_check::ALL_DIAGNOSTICS
            .get_types()
            .iter()
            .copied(),
    );
    types.extend(
        crate::function_type_builder::ALL_DIAGNOSTICS
            .get_types()
            .iter()
            .copied(),
    );
    types.extend(fields["GLOBAL_THIS"].get_types().iter().copied());
    types.push(&crate::check_jsdoc::DEFAULT_PARAM_MUST_BE_MARKED_OPTIONAL);
    types.push(&crate::check_jsdoc::BAD_REST_PARAMETER_ANNOTATION);
    let group = Arc::new(DiagnosticGroup::new_named("checkTypes", &types));
    groups_by_name.insert("checkTypes".into(), group.clone());
    fields.insert("CHECK_TYPES", group);
    let group = crate::type_check::ES5_INHERITANCE_DIAGNOSTIC_GROUP.clone();
    groups_by_name.insert(
        "checkEs5InheritanceCorrectnessConditions".into(),
        group.clone(),
    );
    fields.insert("ES5_INHERITANCE_DIAGNOSTIC_GROUP", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::type_check::UNKNOWN_PROTOTYPAL_OVERRIDE);
    types.push(&crate::type_check::HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY);
    types.push(&crate::type_check::HIDDEN_PROTOTYPAL_SUPERTYPE_PROPERTY_MISMATCH);
    let group = Arc::new(DiagnosticGroup::new_named("checkPrototypalTypes", &types));
    groups_by_name.insert("checkPrototypalTypes".into(), group.clone());
    fields.insert("CHECK_PROTOTYPAL_TYPES", group);
    fields.insert(
        "CHECK_STATIC_OVERRIDES",
        fields["CHECK_PROTOTYPAL_TYPES"].clone(),
    );
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rhino_error_reporter::TOO_MANY_TEMPLATE_PARAMS);
    let group = Arc::new(DiagnosticGroup::new_named("tooManyTypeParams", &types));
    groups_by_name.insert("tooManyTypeParams".into(), group.clone());
    fields.insert("TOO_MANY_TYPE_PARAMS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::type_check::STRICT_INEXISTENT_PROPERTY);
    types.push(&crate::type_check::STRICT_INEXISTENT_PROPERTY_WITH_SUGGESTION);
    types.push(&crate::type_check::STRICT_INEXISTENT_UNION_PROPERTY);
    types.push(&crate::type_check::ILLEGAL_PROPERTY_CREATION_ON_UNION_TYPE);
    let group = Arc::new(DiagnosticGroup::new_named(
        "strictMissingProperties",
        &types,
    ));
    groups_by_name.insert("strictMissingProperties".into(), group.clone());
    fields.insert("STRICT_MISSING_PROPERTIES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::type_validator::INVALID_OPERAND_TYPE);
    let group = Arc::new(DiagnosticGroup::new_named(
        "strictPrimitiveOperators",
        &types,
    ));
    groups_by_name.insert("strictPrimitiveOperators".into(), group.clone());
    fields.insert("STRICT_PRIMITIVE_OPERATORS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.extend(
        fields["STRICT_MISSING_PROPERTIES"]
            .get_types()
            .iter()
            .copied(),
    );
    types.extend(
        fields["STRICT_PRIMITIVE_OPERATORS"]
            .get_types()
            .iter()
            .copied(),
    );
    let group = Arc::new(DiagnosticGroup::new_named("strictCheckTypes", &types));
    groups_by_name.insert("strictCheckTypes".into(), group.clone());
    fields.insert("STRICT_CHECK_TYPES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::type_check::UNKNOWN_EXPR_TYPE);
    let group = Arc::new(DiagnosticGroup::new_named("reportUnknownTypes", &types));
    groups_by_name.insert("reportUnknownTypes".into(), group.clone());
    fields.insert("REPORT_UNKNOWN_TYPES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::var_check::UNDEFINED_VAR_ERROR);
    types.push(&crate::var_check::VAR_MULTIPLY_DECLARED_ERROR);
    types.push(&crate::variable_reference_check::EARLY_REFERENCE);
    types.push(&crate::variable_reference_check::REDECLARED_VARIABLE);
    let group = Arc::new(DiagnosticGroup::new_named("checkVars", &types));
    groups_by_name.insert("checkVars".into(), group.clone());
    fields.insert("CHECK_VARIABLES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_side_effects::USELESS_CODE_ERROR);
    types.push(&crate::check_unreachable_code::UNREACHABLE_CODE);
    let group = Arc::new(DiagnosticGroup::new_named("uselessCode", &types));
    groups_by_name.insert("uselessCode".into(), group.clone());
    fields.insert("CHECK_USELESS_CODE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_access_controls::CONST_PROPERTY_DELETED);
    types.push(&crate::check_access_controls::CONST_PROPERTY_REASSIGNED_VALUE);
    types.push(&crate::const_check::CONST_REASSIGNED_VALUE_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("const", &types));
    groups_by_name.insert("const".into(), group.clone());
    fields.insert("CONST", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_access_controls::CONST_PROPERTY_DELETED);
    types.push(&crate::check_access_controls::CONST_PROPERTY_REASSIGNED_VALUE);
    let group = Arc::new(DiagnosticGroup::new_named("constantProperty", &types));
    groups_by_name.insert("constantProperty".into(), group.clone());
    fields.insert("CONSTANT_PROPERTY", group);
    let group = fields["CONSTANT_PROPERTY"].clone();
    groups_by_name.insert("accessControlsConst".into(), group.clone());
    fields.insert("ACCESS_CONTROLS_CONST", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::disambiguate::disambiguate_properties::PROPERTY_INVALIDATION);
    let group = Arc::new(DiagnosticGroup::new_named("typeInvalidation", &types));
    groups_by_name.insert("typeInvalidation".into(), group.clone());
    fields.insert("TYPE_INVALIDATION", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::inline_and_collapse_properties::NAMESPACE_REDEFINED_WARNING);
    types.push(&crate::type_validator::DUP_VAR_DECLARATION);
    types.push(&crate::type_validator::DUP_VAR_DECLARATION_TYPE_MISMATCH);
    types.push(&crate::type_check::FUNCTION_MASKS_VARIABLE);
    types.push(&crate::var_check::VAR_MULTIPLY_DECLARED_ERROR);
    types.push(&crate::variable_reference_check::REDECLARED_VARIABLE);
    let group = Arc::new(DiagnosticGroup::new_named("duplicate", &types));
    groups_by_name.insert("duplicate".into(), group.clone());
    fields.insert("DUPLICATE_VARS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rhino_error_reporter::INVALID_OCTAL_LITERAL);
    types.push(&crate::rhino_error_reporter::DUPLICATE_PARAM);
    types.push(&crate::strict_mode_check::ARGUMENTS_ASSIGNMENT);
    types.push(&crate::strict_mode_check::ARGUMENTS_DECLARATION);
    types.push(&crate::strict_mode_check::ARGUMENTS_CALLEE_FORBIDDEN);
    types.push(&crate::strict_mode_check::ARGUMENTS_CALLER_FORBIDDEN);
    types.push(&crate::strict_mode_check::DELETE_VARIABLE);
    types.push(&crate::strict_mode_check::DUPLICATE_MEMBER);
    types.push(&crate::strict_mode_check::EVAL_ASSIGNMENT);
    types.push(&crate::strict_mode_check::EVAL_DECLARATION);
    types.push(&crate::strict_mode_check::FUNCTION_ARGUMENTS_PROP_FORBIDDEN);
    types.push(&crate::strict_mode_check::FUNCTION_CALLER_FORBIDDEN);
    types.push(&crate::strict_mode_check::USE_OF_WITH);
    let group = Arc::new(DiagnosticGroup::new_named("es5Strict", &types));
    groups_by_name.insert("es5Strict".into(), group.clone());
    fields.insert("ES5_STRICT", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::closure_primitive_errors::GOOG_MODULE_GET_OF_WEAK_MODULE);
    let group = Arc::new(DiagnosticGroup::new_named("weakModuleGet", &types));
    groups_by_name.insert("weakModuleGet".into(), group.clone());
    fields.insert("WEAK_MODULE_GET", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::closure_primitive_errors::MISSING_MODULE_OR_PROVIDE_FOR_FORWARD_DECLARE);
    let group = Arc::new(DiagnosticGroup::new_named("missingProvide", &types));
    groups_by_name.insert("missingProvide".into(), group.clone());
    fields.insert("MISSING_PROVIDE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rhino_error_reporter::UNRECOGNIZED_TYPE_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("unrecognizedTypeError", &types));
    groups_by_name.insert("unrecognizedTypeError".into(), group.clone());
    fields.insert("UNRECOGNIZED_TYPE_ERROR", group);
    let group = fields["UNRECOGNIZED_TYPE_ERROR"].clone();
    groups_by_name.insert("dangerousUnrecognizedTypeError".into(), group.clone());
    fields.insert("DANGEROUS_UNRECOGNIZED_TYPE_ERROR", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_missing_requires::MISSING_REQUIRE);
    types.push(&crate::check_missing_requires::MISSING_REQUIRE_IN_PROVIDES_FILE);
    types.push(&crate::check_missing_requires::MISSING_REQUIRE_IN_GOOG_SCOPE);
    types.push(&crate::check_missing_requires::MISSING_REQUIRE_TYPE_IN_GOOG_SCOPE);
    types.push(&crate::check_missing_requires::MISSING_REQUIRE_TYPE);
    types.push(&crate::check_missing_requires::MISSING_REQUIRE_TYPE_IN_PROVIDES_FILE);
    types.push(&crate::check_missing_requires::INCORRECT_NAMESPACE_ALIAS_REQUIRE);
    types.push(&crate::check_missing_requires::INCORRECT_NAMESPACE_ALIAS_REQUIRE_TYPE);
    types.push(&crate::check_missing_requires::INDIRECT_NAMESPACE_REF_REQUIRE);
    types.push(&crate::check_missing_requires::INDIRECT_NAMESPACE_REF_REQUIRE_TYPE);
    let group = Arc::new(DiagnosticGroup::new_named("missingRequire", &types));
    groups_by_name.insert("missingRequire".into(), group.clone());
    fields.insert("MISSING_REQUIRE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.extend(fields["REPORT_UNKNOWN_TYPES"].get_types().iter().copied());
    types.extend(fields["UNDEFINED_VARIABLES"].get_types().iter().copied());
    types.extend(fields["MISSING_PROVIDE"].get_types().iter().copied());
    types.push(&crate::closure_primitive_errors::MISSING_MODULE_OR_PROVIDE);
    types.extend(fields["MISSING_PROPERTIES"].get_types().iter().copied());
    types.extend(fields["DUPLICATE_VARS"].get_types().iter().copied());
    types.push(&crate::process_defines::INVALID_DEFINE_VALUE);
    let group = Arc::new(DiagnosticGroup::new_named("missingSourcesWarnings", &types));
    groups_by_name.insert("missingSourcesWarnings".into(), group.clone());
    fields.insert("MISSING_SOURCES_WARNINGS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::lint::check_extra_requires::EXTRA_REQUIRE_WARNING);
    let group = Arc::new(DiagnosticGroup::new_named("extraRequire", &types));
    groups_by_name.insert("extraRequire".into(), group.clone());
    fields.insert("EXTRA_REQUIRE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::js_message_visitor::MESSAGE_DUPLICATE_KEY);
    let group = Arc::new(DiagnosticGroup::new_named("duplicateMessage", &types));
    groups_by_name.insert("duplicateMessage".into(), group.clone());
    fields.insert("DUPLICATE_MESSAGE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::js_message_visitor::MESSAGE_HAS_NO_DESCRIPTION);
    let group = Arc::new(DiagnosticGroup::new_named("msgDescriptions", &types));
    groups_by_name.insert("msgDescriptions".into(), group.clone());
    fields.insert("MESSAGE_DESCRIPTIONS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::js_message_visitor::MESSAGE_HAS_NO_DESCRIPTION);
    types.push(&crate::js_message_visitor::MESSAGE_HAS_NO_TEXT);
    types.push(&crate::js_message_visitor::MESSAGE_TREE_MALFORMED);
    types.push(&crate::js_message_visitor::MESSAGE_HAS_NO_VALUE);
    types.push(&crate::js_message_visitor::MESSAGE_DUPLICATE_KEY);
    types.push(&crate::js_message_visitor::MESSAGE_NOT_INITIALIZED_CORRECTLY);
    let group = Arc::new(DiagnosticGroup::new_named("messageConventions", &types));
    groups_by_name.insert("messageConventions".into(), group.clone());
    fields.insert("MSG_CONVENTIONS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_jsdoc::ARROW_FUNCTION_AS_CONSTRUCTOR);
    types.push(&crate::check_jsdoc::BAD_REST_PARAMETER_ANNOTATION);
    types.push(&crate::check_jsdoc::DEFAULT_PARAM_MUST_BE_MARKED_OPTIONAL);
    types.push(&crate::check_jsdoc::DISALLOWED_MEMBER_JSDOC);
    types.push(&crate::check_jsdoc::INVALID_NO_SIDE_EFFECT_ANNOTATION);
    types.push(&crate::check_jsdoc::INVALID_MODIFIES_ANNOTATION);
    types.push(&crate::check_jsdoc::JSDOC_ON_RETURN);
    types.push(&crate::check_jsdoc::MISPLACED_ANNOTATION);
    types.push(&crate::check_jsdoc::MISPLACED_MSG_ANNOTATION);
    let group = Arc::new(DiagnosticGroup::new_named(
        "misplacedTypeAnnotation",
        &types,
    ));
    groups_by_name.insert("misplacedTypeAnnotation".into(), group.clone());
    fields.insert("MISPLACED_TYPE_ANNOTATION", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_jsdoc::MISPLACED_MSG_ANNOTATION);
    let group = Arc::new(DiagnosticGroup::new_named("misplacedMsgAnnotation", &types));
    groups_by_name.insert("misplacedMsgAnnotation".into(), group.clone());
    fields.insert("MISPLACED_MSG_ANNOTATION", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_jsdoc::MISPLACED_SUPPRESS);
    let group = Arc::new(DiagnosticGroup::new_named("misplacedSuppress", &types));
    groups_by_name.insert("misplacedSuppress".into(), group.clone());
    fields.insert("MISPLACED_SUPPRESS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::lint::check_duplicate_case::DUPLICATE_CASE);
    types.push(&crate::check_suspicious_code::SUSPICIOUS_SEMICOLON);
    types.push(&crate::check_suspicious_code::SUSPICIOUS_BREAKING_OUT_OF_OPTIONAL_CHAIN);
    types.push(&crate::check_suspicious_code::SUSPICIOUS_COMPARISON_WITH_NAN);
    types.push(&crate::check_suspicious_code::SUSPICIOUS_IN_OPERATOR);
    types.push(&crate::check_suspicious_code::SUSPICIOUS_INSTANCEOF_LEFT_OPERAND);
    types.push(&crate::check_suspicious_code::SUSPICIOUS_LEFT_OPERAND_OF_LOGICAL_OPERATOR);
    types.push(&crate::check_suspicious_code::SUSPICIOUS_NEGATED_LEFT_OPERAND_OF_IN_OPERATOR);
    types.push(&crate::process_common_js_modules::SUSPICIOUS_EXPORTS_ASSIGNMENT);
    types.push(&crate::type_check::DETERMINISTIC_TEST);
    let group = Arc::new(DiagnosticGroup::new_named("suspiciousCode", &types));
    groups_by_name.insert("suspiciousCode".into(), group.clone());
    fields.insert("SUSPICIOUS_CODE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::function_type_builder::INEXISTENT_PARAM);
    types.push(&crate::function_type_builder::OPTIONAL_ARG_AT_END);
    let group = Arc::new(DiagnosticGroup::new_named("functionParams", &types));
    groups_by_name.insert("functionParams".into(), group.clone());
    fields.insert("FUNCTION_PARAMS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_jsdoc::ANNOTATION_DEPRECATED);
    let group = Arc::new(DiagnosticGroup::new_named("deprecatedAnnotations", &types));
    groups_by_name.insert("deprecatedAnnotations".into(), group.clone());
    fields.insert("DEPRECATED_ANNOTATIONS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::variable_reference_check::UNUSED_LOCAL_ASSIGNMENT);
    let group = Arc::new(DiagnosticGroup::new_named("unusedLocalVariables", &types));
    groups_by_name.insert("unusedLocalVariables".into(), group.clone());
    fields.insert("UNUSED_LOCAL_VARIABLE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rhino_error_reporter::JSDOC_MISSING_TYPE_WARNING);
    let group = Arc::new(DiagnosticGroup::new_named("jsdocMissingType", &types));
    groups_by_name.insert("jsdocMissingType".into(), group.clone());
    fields.insert("JSDOC_MISSING_TYPE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_type_import_code_references::TYPE_IMPORT_CODE_REFERENCE);
    let group = Arc::new(DiagnosticGroup::new_named(
        "typeImportCodeReferences",
        &types,
    ));
    groups_by_name.insert("typeImportCodeReferences".into(), group.clone());
    fields.insert("TYPE_IMPORT_CODE_REFERENCES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::inline_and_collapse_properties::PARTIAL_NAMESPACE_WARNING);
    let group = Arc::new(DiagnosticGroup::new_named("partialAlias", &types));
    groups_by_name.insert("partialAlias".into(), group.clone());
    fields.insert("PARTIAL_ALIAS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::closure_check_module::USE_OF_GOOG_PROVIDE);
    let group = Arc::new(DiagnosticGroup::new_named("useOfGoogProvide", &types));
    groups_by_name.insert("useOfGoogProvide".into(), group.clone());
    fields.insert("USE_OF_GOOG_PROVIDE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::lint::check_var::VAR);
    let group = Arc::new(DiagnosticGroup::new_named("lintVarDeclarations", &types));
    groups_by_name.insert("lintVarDeclarations".into(), group.clone());
    fields.insert("LINT_VAR_DECLARATIONS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.extend(
        crate::lint::check_jsdoc_style::LINT_DIAGNOSTICS
            .get_types()
            .iter()
            .copied(),
    );
    types.extend(fields["USE_OF_GOOG_PROVIDE"].get_types().iter().copied());
    types.extend(fields["LINT_VAR_DECLARATIONS"].get_types().iter().copied());
    types.push(&crate::check_closure_imports::LET_CLOSURE_IMPORT);
    types.push(&crate::lint::check_const_private_properties::MISSING_CONST_PROPERTY);
    types.push(&crate::lint::check_constant_case_names::REASSIGNED_CONSTANT_CASE_NAME);
    types.push(&crate::lint::check_constant_case_names::MISSING_CONST_PROPERTY);
    types.push(&crate::lint::check_empty_statements::USELESS_EMPTY_STATEMENT);
    types.push(&crate::lint::check_enums::COMPUTED_PROP_NAME_IN_ENUM);
    types.push(&crate::lint::check_enums::DUPLICATE_ENUM_VALUE);
    types.push(&crate::lint::check_enums::ENUM_PROP_NOT_CONSTANT);
    types.push(&crate::lint::check_enums::ENUM_TYPE_NOT_STRING_OR_NUMBER);
    types.push(&crate::lint::check_enums::NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM);
    types.push(&crate::lint::check_enums::SHORTHAND_ASSIGNMENT_IN_ENUM);
    types.push(&crate::lint::check_es6_module_file_structure::MUST_COME_BEFORE);
    types.push(&crate::lint::check_es6_modules::DUPLICATE_IMPORT);
    types.push(&crate::lint::check_es6_modules::NO_DEFAULT_EXPORT);
    types.push(&crate::lint::check_goog_module_type_script_name::MODULE_NAMESPACE_MISMATCHES_TYPESCRIPT_NAMESPACE);
    types.push(&crate::lint::check_interfaces::INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY);
    types.push(&crate::lint::check_interfaces::INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS);
    types.push(&crate::lint::check_interfaces::INTERFACE_DEFINED_WITH_EXTENDS);
    types.push(&crate::lint::check_interfaces::NON_DECLARATION_STATEMENT_IN_INTERFACE);
    types.push(&crate::lint::check_interfaces::MISSING_JSDOC_IN_DECLARATION_STATEMENT);
    types.push(&crate::lint::check_interfaces::STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS);
    types.push(&crate::lint::check_missing_semicolon::MISSING_SEMICOLON);
    types.push(&crate::lint::check_no_mutated_es6_exports::MUTATED_EXPORT);
    types.push(&crate::lint::check_nullability_modifiers::MISSING_NULLABILITY_MODIFIER_JSDOC);
    types.push(&crate::lint::check_nullability_modifiers::NULL_MISSING_NULLABILITY_MODIFIER_JSDOC);
    types.push(&crate::lint::check_nullability_modifiers::REDUNDANT_NULLABILITY_MODIFIER_JSDOC);
    types.push(&crate::lint::check_primitive_as_object::NEW_PRIMITIVE_OBJECT);
    types.push(&crate::lint::check_primitive_as_object::PRIMITIVE_OBJECT_DECLARATION);
    types.push(&crate::lint::check_prototype_properties::ILLEGAL_PROTOTYPE_MEMBER);
    types.push(&crate::lint::check_provides_sorted::PROVIDES_NOT_SORTED);
    types.push(&crate::lint::check_requires_sorted::REQUIRES_NOT_SORTED);
    types.push(&crate::lint::check_unused_labels::UNUSED_LABEL);
    types.push(&crate::lint::check_unused_private_properties::UNUSED_PRIVATE_PROPERTY);
    types.push(&crate::lint::check_useless_blocks::USELESS_BLOCK);
    types.push(&crate::closure_check_module::DECLARE_LEGACY_NAMESPACE_IN_NON_MODULE);
    types.push(&crate::closure_check_module::GOOG_MODULE_IN_NON_MODULE);
    types.push(&crate::closure_check_module::INCORRECT_SHORTNAME_CAPITALIZATION);
    types.push(&crate::closure_rewrite_module::USELESS_USE_STRICT_DIRECTIVE);
    types.push(&crate::rhino_error_reporter::JSDOC_MISSING_BRACES_WARNING);
    types.push(&crate::rhino_error_reporter::UNNECESSARY_ESCAPE);
    types.push(&crate::rhino_error_reporter::STRING_CONTINUATION);
    let group = Arc::new(DiagnosticGroup::new_named("lintChecks", &types));
    groups_by_name.insert("lintChecks".into(), group.clone());
    fields.insert("LINT_CHECKS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::closure_check_module::AT_EXPORT_IN_NON_LEGACY_GOOG_MODULE);
    types.push(&crate::closure_check_module::LET_GOOG_REQUIRE);
    types.push(&crate::closure_check_module::REFERENCE_TO_FULLY_QUALIFIED_IMPORT_NAME);
    types.push(
        &crate::closure_check_module::REFERENCE_TO_SHORT_IMPORT_BY_LONG_NAME_INCLUDING_SHORT_NAME,
    );
    let group = Arc::new(DiagnosticGroup::new_named("strictModuleChecks", &types));
    groups_by_name.insert("strictModuleChecks".into(), group.clone());
    fields.insert("STRICT_MODULE_CHECKS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::lint::check_array_with_goog_object::ARRAY_PASSED_TO_GOOG_OBJECT);
    types.push(&crate::implicit_nullability_check::IMPLICITLY_NONNULL_JSDOC);
    types.push(&crate::implicit_nullability_check::IMPLICITLY_NULLABLE_JSDOC);
    types.push(&crate::lint::check_nested_names::NESTED_NAME_IN_GOOG_MODULE);
    let group = Arc::new(DiagnosticGroup::new_named("analyzerChecks", &types));
    groups_by_name.insert("analyzerChecks".into(), group.clone());
    fields.insert("ANALYZER_CHECKS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::process_closure_primitives::CLOSURE_CALL_CANNOT_BE_ALIASED_ERROR);
    types.push(
        &crate::process_closure_primitives::CLOSURE_CALL_CANNOT_BE_ALIASED_OUTSIDE_MODULE_ERROR,
    );
    types.push(&crate::closure_primitive_errors::INVALID_CLOSURE_CALL_SCOPE_ERROR);
    types.push(&crate::closure_primitive_errors::INVALID_GET_CALL_SCOPE);
    let group = Arc::new(DiagnosticGroup::new_named(
        "closureDepMethodUsageChecks",
        &types,
    ));
    groups_by_name.insert("closureDepMethodUsageChecks".into(), group.clone());
    fields.insert("CLOSURE_DEP_METHOD_USAGE_CHECKS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::process_closure_primitives::POSSIBLE_BASE_CLASS_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("closureClassChecks", &types));
    groups_by_name.insert("closureClassChecks".into(), group.clone());
    fields.insert("CLOSURE_CLASS_CHECKS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::closure_check_module::GOOG_MODULE_MISPLACED);
    types.push(&crate::closure_check_module::LEGACY_NAMESPACE_NOT_AFTER_GOOG_MODULE);
    let group = Arc::new(DiagnosticGroup::new(&types));
    fields.insert("MALFORMED_GOOG_MODULE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::peephole_fold_constants::FRACTIONAL_BITWISE_OPERAND);
    let group = Arc::new(DiagnosticGroup::new_named(
        "transitionalSuspiciousCodeWarnings",
        &types,
    ));
    groups_by_name.insert("transitionalSuspiciousCodeWarnings".into(), group.clone());
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_conformance::CONFORMANCE_VIOLATION);
    types.push(&crate::check_conformance::CONFORMANCE_POSSIBLE_VIOLATION);
    let group = Arc::new(DiagnosticGroup::new_named("conformanceViolations", &types));
    groups_by_name.insert("conformanceViolations".into(), group.clone());
    fields.insert("CONFORMANCE_VIOLATIONS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::check_closure_imports::LATE_PROVIDE_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("lateProvide", &types));
    groups_by_name.insert("lateProvide".into(), group.clone());
    fields.insert("LATE_PROVIDE", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::closure_primitive_errors::DUPLICATE_MODULE);
    types.push(&crate::closure_primitive_errors::DUPLICATE_NAMESPACE);
    types.push(&crate::closure_primitive_errors::DUPLICATE_NAMESPACE_AND_MODULE);
    let group = Arc::new(DiagnosticGroup::new(&types));
    fields.insert("DUPLICATE_NAMESPACES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::process_defines::INVALID_DEFINE_VALUE);
    types.push(&crate::process_defines::INVALID_DEFINE_TYPE);
    let group = Arc::new(DiagnosticGroup::new(&types));
    fields.insert("INVALID_DEFINES", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rewrite_polyfills::INSUFFICIENT_OUTPUT_VERSION_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("missingPolyfill", &types));
    groups_by_name.insert("missingPolyfill".into(), group.clone());
    fields.insert("MISSING_POLYFILL", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::polymer_pass_errors::POLYMER_DESCRIPTOR_NOT_VALID);
    let group = Arc::new(DiagnosticGroup::new_named("polymer", &types));
    groups_by_name.insert("polymer".into(), group.clone());
    fields.insert("POLYMER", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rhino_error_reporter::UNSUPPORTED_BOUNDED_GENERIC_TYPES);
    types.push(&crate::rhino_error_reporter::BOUNDED_GENERIC_TYPE_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("boundedGenerics", &types));
    groups_by_name.insert("boundedGenerics".into(), group.clone());
    fields.insert("BOUNDED_GENERICS", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rhino_error_reporter::PARSE_ERROR);
    let group = DiagnosticGroup::for_type(&crate::rhino_error_reporter::PARSE_ERROR);
    fields.insert("PARSING", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::rhino_error_reporter::CLOSURE_UNAWARE_ANNOTATION_PRESENT);
    let group = Arc::new(DiagnosticGroup::new_named(
        "closureUnawareCodeAnnotationPresent",
        &types,
    ));
    groups_by_name.insert("closureUnawareCodeAnnotationPresent".into(), group.clone());
    fields.insert("CLOSURE_UNAWARE_CODE_ANNOTATION_PRESENT", group);
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::polymer_pass_errors::POLYMER_UNQUALIFIED_BEHAVIOR);
    let group = Arc::new(DiagnosticGroup::new_named("polymerBehavior", &types));
    groups_by_name.insert("polymerBehavior".into(), group.clone());
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::process_closure_primitives::INVALID_PROVIDE_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("invalidProvide", &types));
    groups_by_name.insert("invalidProvide".into(), group.clone());
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::ijs::ijs_errors::CONFLICTING_IJS_FILE);
    let group = Arc::new(DiagnosticGroup::new_named("conflictingIjsFile", &types));
    groups_by_name.insert("conflictingIjsFile".into(), group.clone());
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::js_chunk_graph::IMPLICIT_WEAK_ENTRY_POINT_ERROR);
    let group = Arc::new(DiagnosticGroup::new_named("implicitWeakEntryPoint", &types));
    groups_by_name.insert("implicitWeakEntryPoint".into(), group.clone());
    let mut types: Vec<&'static DiagnosticType> = vec![];
    types.push(&crate::pure_function_identifier::UNUSED_ARTIFICIAL_PURE_ANNOTATION);
    let group = Arc::new(DiagnosticGroup::new_named(
        "artificialFunctionPurityValidation",
        &types,
    ));
    groups_by_name.insert("artificialFunctionPurityValidation".into(), group.clone());
    fields.insert("ARTIFICIAL_FUNCTION_PURITY_VALIDATION", group);
    Builtins {
        fields,
        groups_by_name,
    }
});
static GROUPS_BY_NAME: LazyLock<Mutex<IndexMap<String, Arc<DiagnosticGroup>>>> =
    LazyLock::new(|| Mutex::new(BUILTINS.groups_by_name.clone()));
pub static UNTRANSPILABLE_FEATURES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["UNTRANSPILABLE_FEATURES"].clone());
pub static MODULE_LOAD: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MODULE_LOAD"].clone());
pub static MODULE_IMPORT: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MODULE_IMPORT"].clone());
pub static GLOBAL_THIS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["GLOBAL_THIS"].clone());
pub static DEPRECATED: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["DEPRECATED"].clone());
pub static UNDERSCORE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["UNDERSCORE"].clone());
pub static VISIBILITY: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["VISIBILITY"].clone());
pub static ACCESS_CONTROLS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["ACCESS_CONTROLS"].clone());
pub static NON_STANDARD_JSDOC: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["NON_STANDARD_JSDOC"].clone());
pub static INVALID_CASTS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["INVALID_CASTS"].clone());
pub static STRICT_MODULE_DEP_CHECK: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["STRICT_MODULE_DEP_CHECK"].clone());
pub static VIOLATED_MODULE_DEP: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["VIOLATED_MODULE_DEP"].clone());
pub static EXTERNS_VALIDATION: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["EXTERNS_VALIDATION"].clone());
pub static UNKNOWN_DEFINES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["UNKNOWN_DEFINES"].clone());
pub static DEFINE_WITHOUT_GOOG_DEFINE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["DEFINE_WITHOUT_GOOG_DEFINE"].clone());
pub static TWEAKS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["TWEAKS"].clone());
pub static MISSING_OVERRIDE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISSING_OVERRIDE"].clone());
pub static MISSING_PROPERTIES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISSING_PROPERTIES"].clone());
pub static GLOBALLY_MISSING_PROPERTIES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["GLOBALLY_MISSING_PROPERTIES"].clone());
pub static J2CL_CHECKS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["J2CL_CHECKS"].clone());
pub static MISSING_RETURN: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISSING_RETURN"].clone());
pub static UNDEFINED_VARIABLES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["UNDEFINED_VARIABLES"].clone());
pub static DEBUGGER_STATEMENT_PRESENT: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["DEBUGGER_STATEMENT_PRESENT"].clone());
pub static CHECK_REGEXP: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CHECK_REGEXP"].clone());
pub static CHECK_TYPES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CHECK_TYPES"].clone());
pub static ES5_INHERITANCE_DIAGNOSTIC_GROUP: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["ES5_INHERITANCE_DIAGNOSTIC_GROUP"].clone());
pub static CHECK_PROTOTYPAL_TYPES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CHECK_PROTOTYPAL_TYPES"].clone());
pub static CHECK_STATIC_OVERRIDES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CHECK_STATIC_OVERRIDES"].clone());
pub static TOO_MANY_TYPE_PARAMS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["TOO_MANY_TYPE_PARAMS"].clone());
pub static STRICT_MISSING_PROPERTIES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["STRICT_MISSING_PROPERTIES"].clone());
pub static STRICT_PRIMITIVE_OPERATORS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["STRICT_PRIMITIVE_OPERATORS"].clone());
pub static STRICT_CHECK_TYPES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["STRICT_CHECK_TYPES"].clone());
pub static REPORT_UNKNOWN_TYPES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["REPORT_UNKNOWN_TYPES"].clone());
pub static CHECK_VARIABLES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CHECK_VARIABLES"].clone());
pub static CHECK_USELESS_CODE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CHECK_USELESS_CODE"].clone());
pub static CONST: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CONST"].clone());
pub static CONSTANT_PROPERTY: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CONSTANT_PROPERTY"].clone());
pub static ACCESS_CONTROLS_CONST: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["ACCESS_CONTROLS_CONST"].clone());
pub static TYPE_INVALIDATION: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["TYPE_INVALIDATION"].clone());
pub static DUPLICATE_VARS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["DUPLICATE_VARS"].clone());
pub static ES5_STRICT: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["ES5_STRICT"].clone());
pub static WEAK_MODULE_GET: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["WEAK_MODULE_GET"].clone());
pub static MISSING_PROVIDE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISSING_PROVIDE"].clone());
pub static UNRECOGNIZED_TYPE_ERROR: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["UNRECOGNIZED_TYPE_ERROR"].clone());
pub static DANGEROUS_UNRECOGNIZED_TYPE_ERROR: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["DANGEROUS_UNRECOGNIZED_TYPE_ERROR"].clone());
pub static MISSING_REQUIRE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISSING_REQUIRE"].clone());
pub static MISSING_SOURCES_WARNINGS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISSING_SOURCES_WARNINGS"].clone());
pub static EXTRA_REQUIRE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["EXTRA_REQUIRE"].clone());
pub static DUPLICATE_MESSAGE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["DUPLICATE_MESSAGE"].clone());
pub static MESSAGE_DESCRIPTIONS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MESSAGE_DESCRIPTIONS"].clone());
pub static MSG_CONVENTIONS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MSG_CONVENTIONS"].clone());
pub static MISPLACED_TYPE_ANNOTATION: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISPLACED_TYPE_ANNOTATION"].clone());
pub static MISPLACED_MSG_ANNOTATION: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISPLACED_MSG_ANNOTATION"].clone());
pub static MISPLACED_SUPPRESS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISPLACED_SUPPRESS"].clone());
pub static SUSPICIOUS_CODE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["SUSPICIOUS_CODE"].clone());
pub static FUNCTION_PARAMS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["FUNCTION_PARAMS"].clone());
pub static DEPRECATED_ANNOTATIONS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["DEPRECATED_ANNOTATIONS"].clone());
pub static UNUSED_LOCAL_VARIABLE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["UNUSED_LOCAL_VARIABLE"].clone());
pub static JSDOC_MISSING_TYPE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["JSDOC_MISSING_TYPE"].clone());
pub static TYPE_IMPORT_CODE_REFERENCES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["TYPE_IMPORT_CODE_REFERENCES"].clone());
pub static PARTIAL_ALIAS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["PARTIAL_ALIAS"].clone());
pub static USE_OF_GOOG_PROVIDE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["USE_OF_GOOG_PROVIDE"].clone());
pub static LINT_VAR_DECLARATIONS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["LINT_VAR_DECLARATIONS"].clone());
pub static LINT_CHECKS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["LINT_CHECKS"].clone());
pub static STRICT_MODULE_CHECKS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["STRICT_MODULE_CHECKS"].clone());
pub static ANALYZER_CHECKS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["ANALYZER_CHECKS"].clone());
pub static CLOSURE_DEP_METHOD_USAGE_CHECKS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CLOSURE_DEP_METHOD_USAGE_CHECKS"].clone());
pub static CLOSURE_CLASS_CHECKS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CLOSURE_CLASS_CHECKS"].clone());
pub static MALFORMED_GOOG_MODULE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MALFORMED_GOOG_MODULE"].clone());
pub static CONFORMANCE_VIOLATIONS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CONFORMANCE_VIOLATIONS"].clone());
pub static LATE_PROVIDE: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["LATE_PROVIDE"].clone());
pub static DUPLICATE_NAMESPACES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["DUPLICATE_NAMESPACES"].clone());
pub static INVALID_DEFINES: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["INVALID_DEFINES"].clone());
pub static MISSING_POLYFILL: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["MISSING_POLYFILL"].clone());
pub static POLYMER: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["POLYMER"].clone());
pub static BOUNDED_GENERICS: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["BOUNDED_GENERICS"].clone());
pub static PARSING: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["PARSING"].clone());
pub static CLOSURE_UNAWARE_CODE_ANNOTATION_PRESENT: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["CLOSURE_UNAWARE_CODE_ANNOTATION_PRESENT"].clone());
pub static ARTIFICIAL_FUNCTION_PURITY_VALIDATION: LazyLock<Arc<DiagnosticGroup>> =
    LazyLock::new(|| BUILTINS.fields["ARTIFICIAL_FUNCTION_PURITY_VALIDATION"].clone());
#[derive(Default)]
pub struct DiagnosticGroups;
impl DiagnosticGroups {
    // port: DiagnosticGroups#DiagnosticGroups
    pub fn new() -> Self {
        Self
    }
    // port: DiagnosticGroups#registerDeprecatedGroup
    pub fn register_deprecated_group(name: &str) -> Arc<DiagnosticGroup> {
        Self::register_group(name, Arc::new(DiagnosticGroup::new_named(name, &[&UNUSED])))
    }
    // port: DiagnosticGroups#registerUnsuppressibleGroup
    pub fn register_unsuppressible_group(
        types: &[&'static DiagnosticType],
    ) -> Arc<DiagnosticGroup> {
        Arc::new(DiagnosticGroup::new(types))
    }
    // port: DiagnosticGroups#registerGroup(String, DiagnosticGroup)
    pub fn register_group(name: &str, group: Arc<DiagnosticGroup>) -> Arc<DiagnosticGroup> {
        GROUPS_BY_NAME
            .lock()
            .unwrap()
            .insert(name.into(), group.clone());
        group
    }
    // port: DiagnosticGroups#registerGroup(String, DiagnosticType...)
    pub fn register_group_types(
        name: &str,
        types: &[&'static DiagnosticType],
    ) -> Arc<DiagnosticGroup> {
        Self::register_group(name, Arc::new(DiagnosticGroup::new_named(name, types)))
    }
    // port: DiagnosticGroups#registerGroup(String, DiagnosticGroup...)
    pub fn register_group_groups(
        name: &str,
        groups: &[Arc<DiagnosticGroup>],
    ) -> Arc<DiagnosticGroup> {
        Self::register_group(
            name,
            Arc::new(DiagnosticGroup::new_named_from_groups(name, groups)),
        )
    }
    // port: DiagnosticGroups#getRegisteredGroups
    pub fn get_registered_groups() -> IndexMap<String, Arc<DiagnosticGroup>> {
        GROUPS_BY_NAME.lock().unwrap().clone()
    }
    // port: DiagnosticGroups#forName
    pub fn for_name(name: &str) -> Option<Arc<DiagnosticGroup>> {
        GROUPS_BY_NAME.lock().unwrap().get(name).cloned()
    }
    // port: DiagnosticGroups#setWarningLevel
    pub fn set_warning_level(&self, options: &mut CompilerOptions, name: &str, level: CheckLevel) {
        let group =
            Self::for_name(name).unwrap_or_else(|| panic!("No warning class for name: {name}"));
        options.set_warning_level(group, level);
    }
}
