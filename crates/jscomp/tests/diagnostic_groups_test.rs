/*
 * Copyright 2019 The Closure Compiler Authors.
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
/*
 * Copyright 2026 The closure-rs Authors.
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
//   test/com/google/javascript/jscomp/DiagnosticGroupsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   crates/jscomp/tests/java/CompilerDiagnosticGroups.java.

use closure_jscomp::diagnostic_groups::{
    DiagnosticGroups, LINT_CHECKS, LINT_VAR_DECLARATIONS, USE_OF_GOOG_PROVIDE,
};
use std::sync::Arc;

// port: DiagnosticGroupsTest#lintChecksGroupIsDisjointFromEveryOtherGroup
#[test]
fn lint_checks_group_is_disjoint_from_every_other_group() {
    for group in DiagnosticGroups::get_registered_groups().values() {
        if Arc::ptr_eq(group, &LINT_CHECKS)
            || Arc::ptr_eq(group, &USE_OF_GOOG_PROVIDE)
            || Arc::ptr_eq(group, &LINT_VAR_DECLARATIONS)
        {
            continue;
        }
        assert!(
            LINT_CHECKS
                .get_types()
                .iter()
                .all(|ty| !group.get_types().contains(ty)),
            "DiagnosticTypes common to DiagnosticGroups {:?} and {:?}",
            LINT_CHECKS.get_name(),
            group.get_name()
        );
    }
}

// port: DiagnosticGroupsTest#conformanceErrorsCannotBeDowngraded
#[test]
fn conformance_errors_cannot_be_downgraded() {
    for group in DiagnosticGroups::get_registered_groups().values() {
        assert!(
            group
                .get_types()
                .iter()
                .all(|ty| ty.key != "JSC_CONFORMANCE_ERROR"),
            "Group '{:?}' should not include JSC_CONFORMANCE_ERROR",
            group.get_name()
        );
    }
}

// port: CompilerDiagnosticGroups#main
#[test]
fn registry_and_public_fields_match_pinned_java() {
    use closure_jscomp::{diagnostic_group::DiagnosticGroup, diagnostic_groups as groups};
    use serde_json::{Value, json};
    fn dump(name: &str, group: &DiagnosticGroup) -> Value {
        json!([
            name,
            group.get_name(),
            group
                .get_types()
                .iter()
                .map(|ty| { json!([ty.key, ty.level.to_string(), ty.format]) })
                .collect::<Vec<_>>()
        ])
    }
    let registered: Vec<_> = DiagnosticGroups::get_registered_groups()
        .iter()
        .map(|(name, group)| dump(name, group))
        .collect();
    let fields = vec![
        dump("ACCESS_CONTROLS", &groups::ACCESS_CONTROLS),
        dump("ANALYZER_CHECKS", &groups::ANALYZER_CHECKS),
        dump(
            "ARTIFICIAL_FUNCTION_PURITY_VALIDATION",
            &groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION,
        ),
        dump("CHECK_PROTOTYPAL_TYPES", &groups::CHECK_PROTOTYPAL_TYPES),
        dump("CHECK_REGEXP", &groups::CHECK_REGEXP),
        dump("CHECK_STATIC_OVERRIDES", &groups::CHECK_STATIC_OVERRIDES),
        dump("CHECK_TYPES", &groups::CHECK_TYPES),
        dump("CHECK_USELESS_CODE", &groups::CHECK_USELESS_CODE),
        dump("CHECK_VARIABLES", &groups::CHECK_VARIABLES),
        dump("CLOSURE_CLASS_CHECKS", &groups::CLOSURE_CLASS_CHECKS),
        dump(
            "CLOSURE_DEP_METHOD_USAGE_CHECKS",
            &groups::CLOSURE_DEP_METHOD_USAGE_CHECKS,
        ),
        dump(
            "CLOSURE_UNAWARE_CODE_ANNOTATION_PRESENT",
            &groups::CLOSURE_UNAWARE_CODE_ANNOTATION_PRESENT,
        ),
        dump("CONFORMANCE_VIOLATIONS", &groups::CONFORMANCE_VIOLATIONS),
        dump("CONST", &groups::CONST),
        dump("CONSTANT_PROPERTY", &groups::CONSTANT_PROPERTY),
        dump(
            "DEBUGGER_STATEMENT_PRESENT",
            &groups::DEBUGGER_STATEMENT_PRESENT,
        ),
        dump(
            "DEFINE_WITHOUT_GOOG_DEFINE",
            &groups::DEFINE_WITHOUT_GOOG_DEFINE,
        ),
        dump("DEPRECATED", &groups::DEPRECATED),
        dump("DEPRECATED_ANNOTATIONS", &groups::DEPRECATED_ANNOTATIONS),
        dump("DUPLICATE_MESSAGE", &groups::DUPLICATE_MESSAGE),
        dump("DUPLICATE_NAMESPACES", &groups::DUPLICATE_NAMESPACES),
        dump("DUPLICATE_VARS", &groups::DUPLICATE_VARS),
        dump(
            "ES5_INHERITANCE_DIAGNOSTIC_GROUP",
            &groups::ES5_INHERITANCE_DIAGNOSTIC_GROUP,
        ),
        dump("ES5_STRICT", &groups::ES5_STRICT),
        dump("EXTERNS_VALIDATION", &groups::EXTERNS_VALIDATION),
        dump("EXTRA_REQUIRE", &groups::EXTRA_REQUIRE),
        dump("FUNCTION_PARAMS", &groups::FUNCTION_PARAMS),
        dump(
            "GLOBALLY_MISSING_PROPERTIES",
            &groups::GLOBALLY_MISSING_PROPERTIES,
        ),
        dump("GLOBAL_THIS", &groups::GLOBAL_THIS),
        dump("INVALID_CASTS", &groups::INVALID_CASTS),
        dump("INVALID_DEFINES", &groups::INVALID_DEFINES),
        dump("J2CL_CHECKS", &groups::J2CL_CHECKS),
        dump("JSDOC_MISSING_TYPE", &groups::JSDOC_MISSING_TYPE),
        dump("LATE_PROVIDE", &groups::LATE_PROVIDE),
        dump("LINT_CHECKS", &groups::LINT_CHECKS),
        dump("LINT_VAR_DECLARATIONS", &groups::LINT_VAR_DECLARATIONS),
        dump("MALFORMED_GOOG_MODULE", &groups::MALFORMED_GOOG_MODULE),
        dump("MESSAGE_DESCRIPTIONS", &groups::MESSAGE_DESCRIPTIONS),
        dump(
            "MISPLACED_MSG_ANNOTATION",
            &groups::MISPLACED_MSG_ANNOTATION,
        ),
        dump("MISPLACED_SUPPRESS", &groups::MISPLACED_SUPPRESS),
        dump(
            "MISPLACED_TYPE_ANNOTATION",
            &groups::MISPLACED_TYPE_ANNOTATION,
        ),
        dump("MISSING_OVERRIDE", &groups::MISSING_OVERRIDE),
        dump("MISSING_POLYFILL", &groups::MISSING_POLYFILL),
        dump("MISSING_PROPERTIES", &groups::MISSING_PROPERTIES),
        dump("MISSING_PROVIDE", &groups::MISSING_PROVIDE),
        dump("MISSING_REQUIRE", &groups::MISSING_REQUIRE),
        dump("MISSING_RETURN", &groups::MISSING_RETURN),
        dump(
            "MISSING_SOURCES_WARNINGS",
            &groups::MISSING_SOURCES_WARNINGS,
        ),
        dump("MODULE_IMPORT", &groups::MODULE_IMPORT),
        dump("MODULE_LOAD", &groups::MODULE_LOAD),
        dump("MSG_CONVENTIONS", &groups::MSG_CONVENTIONS),
        dump("NON_STANDARD_JSDOC", &groups::NON_STANDARD_JSDOC),
        dump("PARSING", &groups::PARSING),
        dump("PARTIAL_ALIAS", &groups::PARTIAL_ALIAS),
        dump("POLYMER", &groups::POLYMER),
        dump("REPORT_UNKNOWN_TYPES", &groups::REPORT_UNKNOWN_TYPES),
        dump("STRICT_CHECK_TYPES", &groups::STRICT_CHECK_TYPES),
        dump(
            "STRICT_MISSING_PROPERTIES",
            &groups::STRICT_MISSING_PROPERTIES,
        ),
        dump("STRICT_MODULE_CHECKS", &groups::STRICT_MODULE_CHECKS),
        dump("STRICT_MODULE_DEP_CHECK", &groups::STRICT_MODULE_DEP_CHECK),
        dump(
            "STRICT_PRIMITIVE_OPERATORS",
            &groups::STRICT_PRIMITIVE_OPERATORS,
        ),
        dump("SUSPICIOUS_CODE", &groups::SUSPICIOUS_CODE),
        dump("TOO_MANY_TYPE_PARAMS", &groups::TOO_MANY_TYPE_PARAMS),
        dump("TWEAKS", &groups::TWEAKS),
        dump(
            "TYPE_IMPORT_CODE_REFERENCES",
            &groups::TYPE_IMPORT_CODE_REFERENCES,
        ),
        dump("TYPE_INVALIDATION", &groups::TYPE_INVALIDATION),
        dump("UNDEFINED_VARIABLES", &groups::UNDEFINED_VARIABLES),
        dump("UNDERSCORE", &groups::UNDERSCORE),
        dump("UNKNOWN_DEFINES", &groups::UNKNOWN_DEFINES),
        dump("UNRECOGNIZED_TYPE_ERROR", &groups::UNRECOGNIZED_TYPE_ERROR),
        dump("UNTRANSPILABLE_FEATURES", &groups::UNTRANSPILABLE_FEATURES),
        dump("UNUSED_LOCAL_VARIABLE", &groups::UNUSED_LOCAL_VARIABLE),
        dump("VIOLATED_MODULE_DEP", &groups::VIOLATED_MODULE_DEP),
        dump("VISIBILITY", &groups::VISIBILITY),
        dump("WEAK_MODULE_GET", &groups::WEAK_MODULE_GET),
    ];
    let expected: Value =
        serde_json::from_str(include_str!("data/compiler_diagnostic_groups.json")).unwrap();
    let actual = json!([registered, fields]);
    assert_eq!(
        actual[0].as_array().unwrap().len(),
        expected[0].as_array().unwrap().len()
    );
    assert_eq!(
        actual[1].as_array().unwrap().len(),
        expected[1].as_array().unwrap().len()
    );
    for (kind, (actual, expected)) in actual
        .as_array()
        .unwrap()
        .iter()
        .zip(expected.as_array().unwrap())
        .enumerate()
    {
        for (actual, expected) in actual
            .as_array()
            .unwrap()
            .iter()
            .zip(expected.as_array().unwrap())
        {
            assert_eq!(actual, expected, "section {kind}, group {}", expected[0]);
        }
    }
}
