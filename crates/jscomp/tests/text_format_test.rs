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

//! Checks the `TextFormat` port against goldens recorded with the reference jar's protobuf-java
//! (data/text_format/TfGolden.java: `TextFormat.merge` into a `ConformanceConfig.Builder`, then
//! `TextFormat.printer().printToString`, or the ParseException's class and message).

use closure_jscomp::conformance_config::{ConformanceConfig, Requirement, RequirementScopeEntry};
use closure_jscomp::protobuf::text_format;

fn run_case(input: &str) -> String {
    let mut builder = ConformanceConfig::new_builder();
    match text_format::merge(input, &mut builder) {
        Ok(()) => format!(
            "OK\n{}",
            text_format::printer().print_to_string(&builder.build())
        ),
        Err(e) => format!(
            "ERR com.google.protobuf.TextFormat$ParseException\n{}",
            e.get_message()
        ),
    }
}

fn check_golden(cases: &str, golden: &str) {
    let inputs: Vec<&str> = cases.split("\n=====\n").collect();
    let mut expected: Vec<&str> = golden.split("\n=====\n").collect();
    assert_eq!(expected.pop(), Some(""));
    assert_eq!(inputs.len(), expected.len());
    for (input, expected) in inputs.iter().zip(expected) {
        assert_eq!(run_case(input), expected, "input: {input:?}");
    }
}

#[test]
fn golden_cases1() {
    check_golden(
        include_str!("data/text_format/cases1.txt"),
        include_str!("data/text_format/golden1.txt"),
    );
}

#[test]
fn golden_cases2() {
    check_golden(
        include_str!("data/text_format/cases2.txt"),
        include_str!("data/text_format/golden2.txt"),
    );
}

#[test]
fn golden_cases3() {
    check_golden(
        include_str!("data/text_format/cases3.txt"),
        include_str!("data/text_format/golden3.txt"),
    );
}

#[test]
fn builder_api_and_defaults() {
    let mut r = Requirement::new_builder();
    assert_eq!(
        r.get_type(),
        closure_jscomp::conformance_config::Type::CUSTOM
    );
    assert_eq!(
        r.get_severity(),
        closure_jscomp::conformance_config::Severity::WARNING
    );
    assert_eq!(
        r.get_type_matching_strategy(),
        closure_jscomp::conformance_config::TypeMatchingStrategy::LOOSE
    );
    assert!(r.get_report_loose_type_violations());
    assert!(!r.has_rule_id());
    r.set_rule_id("x")
        .add_whitelist("a")
        .add_allowlist_entry(RequirementScopeEntry::new_builder().add_prefix("p").build());
    let names: Vec<&str> = r.get_all_fields().iter().map(|f| f.get_name()).collect();
    assert_eq!(names, ["whitelist", "rule_id", "allowlist_entry"]);
    assert_eq!(
        r.to_string(),
        "whitelist: \"a\"\nrule_id: \"x\"\nallowlist_entry {\n  prefix: \"p\"\n}\n"
    );
}

#[test]
fn error_manager_should_report_conformance_violation() {
    use closure_jscomp::conformance_config::LibraryLevelNonAllowlistedConformanceViolationsBehavior as Behavior;
    use closure_jscomp::error_manager::ErrorManager;
    use closure_jscomp::thread_safe_delegating_error_manager::ThreadSafeDelegatingErrorManager;
    let requirement = Requirement::new_builder();
    let diagnostic = closure_jscomp::js_error::JSError::builder(
        &closure_jscomp::check_conformance::CONFORMANCE_VIOLATION,
        &["a", "b", "c"],
    )
    .build();
    let mut manager = ThreadSafeDelegatingErrorManager::new(Box::new(
        closure_jscomp::black_hole_error_manager::BlackHoleErrorManager::new(),
    ));
    for (behavior, expected) in [
        (Behavior::UNSPECIFIED, true),
        (Behavior::REPORT_AS_BUILD_ERROR, true),
        (Behavior::RECORD_ONLY, false),
    ] {
        for allowlisted in [false, true] {
            assert_eq!(
                manager.should_report_conformance_violation(
                    &requirement,
                    None,
                    &diagnostic,
                    behavior,
                    allowlisted
                ),
                expected
            );
        }
    }
}

/// `Requirement#toByteArray` and `Requirement#parseFrom`: the bytes are those the Java oracle
/// printed for the same text-format input
/// (corpus-cache/conformance/wire/ReqBytes.java, expected.hex).
#[test]
fn requirement_binary_wire_format_matches_java() {
    use closure_jscomp::serialization::protobuf::Message as _;
    let text = "requirement: { error_message: 'h\u{e9}llo' whitelist: 'a' whitelist: 'b' type: BANNED_NAME value: 'x' java_class: 'C' rule_id: 'r' extends: 'e' report_loose_type_violations: false severity: ERROR type_matching_strategy: EXACT whitelist_entry: { reason: LEGACY prefix: 'p' regexp: 'q' explanation: 'w' comment: 'c' automatically_prune: true do_not_record_violations_in_summary_because_this_is_gencode_and_always_safe: false } allow_extending_value: true config_file: 'f' allowlist: 'g' allowlist_regexp: 'h' allowlist_entry: { prefix: 'z' } only_apply_to: 'o' only_apply_to_regexp: 'oo' }";
    let mut builder = ConformanceConfig::new_builder();
    text_format::merge(text, &mut builder).unwrap();
    let requirement: Requirement = builder.build().get_requirement(0).clone();
    let bytes = requirement.to_byte_array();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex,
        "0a0668c3a96c6c6f12016112016222016f2a026f6f30033a01784201434a0172520165580060026804721208011201701a01712201772a0163300138007801820101668a010167920101689a010312017a"
    );
    assert_eq!(requirement.get_serialized_size(), bytes.len());
    assert_eq!(Requirement::parse_from(&bytes).unwrap(), requirement);
    assert_eq!(
        RequirementScopeEntry::parse_from(&[]).unwrap(),
        *RequirementScopeEntry::default_instance_ref()
    );
}
