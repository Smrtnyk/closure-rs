/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ChangeVerifierTest.java.

use closure_jscomp::{Compiler, change_verifier::ChangeVerifier};
use closure_rhino::{ir::IR, node::NodeId};

// Trees are built with IR while ParserRunner is in flight, as allowed by the compiler brief.
fn compiler() -> Compiler {
    let mut c = Compiler::new();
    c.init_compiler_options_if_testing();
    c
}
fn function(c: &mut Compiler, name: &str) -> NodeId {
    let name = IR::name(c, name);
    let params = IR::param_list(c, &[]);
    let body = IR::block(c);
    IR::function(c, name, params, body)
}
fn function_script(c: &mut Compiler) -> NodeId {
    let f = function(c, "A");
    let script = IR::script(c);
    script.add_child_to_back(c, f);
    script
}
fn arrow_script(c: &mut Compiler) -> NodeId {
    let f = function(c, "");
    f.set_is_arrow_function(c, true);
    let statement = IR::expr_result(c, f);
    let script = IR::script(c);
    script.add_child_to_back(c, statement);
    script
}
fn with_if(c: &mut Compiler) -> (NodeId, NodeId) {
    let script = function_script(c);
    let target = IR::name(c, "A");
    let call = IR::call(c, target, &[]);
    let expr = IR::expr_result(c, call);
    let block = IR::block_with_child(c, expr);
    let condition = IR::number(c, 0.0);
    let if_node = IR::if_node(c, condition, block);
    script.add_child_to_back(c, if_node);
    (script, call)
}
fn add_statement(c: &mut Compiler, parent: NodeId) {
    let null = IR::null_node(c);
    let expr = IR::expr_result(c, null);
    parent.add_child_to_back(c, expr);
}
fn panic_message(f: impl FnOnce()) -> String {
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_err();
    if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        err.downcast_ref::<&str>().unwrap().to_string()
    }
}

// port: ChangeVerifierTest#testCorrectValidationOfScriptWithChangeAfterFunction
#[test]
fn test_correct_validation_of_script_with_change_after_function() {
    let mut c = compiler();
    let (script, call) = with_if(&mut c);
    assert!(script.is_script(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    call.detach(&mut c);
    c.report_change_to_change_scope(script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
}
// port: ChangeVerifierTest#testChangeToScriptNotReported_newChild
#[test]
fn test_change_to_script_not_reported_new_child() {
    let mut c = compiler();
    let (script, _) = with_if(&mut c);
    assert!(script.is_script(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    add_statement(&mut c, script);
    let msg = panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script));
    assert!(msg.contains("changed scope not marked as changed"));
    assert!(msg.contains("differing child count"));
}
// port: ChangeVerifierTest#testChangeToScriptNotReported_changeToFunctionName
#[test]
fn test_change_to_script_not_reported_change_to_function_name() {
    let mut c = compiler();
    let (script, _) = with_if(&mut c);
    assert!(script.is_script(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    let f = script.get_first_child(&c).unwrap();
    let name = f.get_first_child(&c).unwrap();
    let replacement = IR::name(&mut c, "B");
    name.replace_with(&mut c, replacement);
    let msg = panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script));
    assert!(msg.contains("changed scope not marked as changed"));
    assert!(msg.contains("function name changed\nBefore: A\nAfter:  B\n"));
}
// port: ChangeVerifierTest#testChangeToFunction_notReported
#[test]
fn test_change_to_function_not_reported() {
    let mut c = compiler();
    let script = function_script(&mut c);
    assert!(script.is_script(&c));
    let f = script.get_first_child(&c).unwrap();
    assert!(f.is_function(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    add_statement(&mut c, f);
    assert!(
        panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script))
            .contains("changed scope not marked as changed")
    );
}
// port: ChangeVerifierTest#testChangeToArrowFunction_notReported
#[test]
fn test_change_to_arrow_function_not_reported() {
    let mut c = compiler();
    let script = arrow_script(&mut c);
    assert!(script.is_script(&c));
    let f = script.get_first_first_child(&c).unwrap();
    assert!(f.is_arrow_function(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    add_statement(&mut c, f);
    assert!(
        panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script))
            .contains("changed scope not marked as changed")
    );
}
// port: ChangeVerifierTest#testChangeToArrowFunction_correctlyReportedChange
#[test]
fn test_change_to_arrow_function_correctly_reported_change() {
    let mut c = compiler();
    let script = arrow_script(&mut c);
    assert!(script.is_script(&c));
    let f = script.get_first_first_child(&c).unwrap();
    assert!(f.is_arrow_function(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    add_statement(&mut c, f);
    c.report_change_to_change_scope(f);
    verifier.check_recorded_changes_named(&mut c, "test2", script);
}
// port: ChangeVerifierTest#testDeletedFunction
#[test]
fn test_deleted_function() {
    let mut c = compiler();
    let script = function_script(&mut c);
    assert!(script.is_script(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    let f = script.get_first_child(&c).unwrap();
    f.detach(&mut c);
    c.report_change_to_change_scope(script);
    assert!(
        panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script))
            .contains("deleted scope was not reported")
    );
    c.report_function_deleted(f);
    verifier.check_recorded_changes_named(&mut c, "test2", script);
}
// port: ChangeVerifierTest#testNotDeletedFunction
#[test]
fn test_not_deleted_function() {
    let mut c = compiler();
    let script = function_script(&mut c);
    assert!(script.is_script(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    let f = script.get_first_child(&c).unwrap();
    c.report_function_deleted(f);
    assert!(
        panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script))
            .contains("existing scope is improperly marked as deleted")
    );
}
// port: ChangeVerifierTest#testChangeVerification
#[test]
fn test_change_verification() {
    let mut c = compiler();
    let script = IR::script(&mut c);
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes(&mut c, script);
    let f = function(&mut c, "A");
    script.add_child_to_front(&mut c, f);
    c.report_change_to_change_scope(script);
    assert!(
        panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script))
            .contains("new scope not explicitly marked as changed:")
    );
    c.report_change_to_change_scope(f);
    verifier.check_recorded_changes(&mut c, script);
}
// port: ChangeVerifierTest#testChangeToScriptNotReported_changeToChild
#[test]
fn test_change_to_script_not_reported_change_to_child() {
    let mut c = compiler();
    let script = c.parse_test_code("function A() {} if (0) { A(); }");
    assert!(c.get_errors().is_empty());
    assert!(script.is_script(&c));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    let if_statement = script.get_last_child(&c).unwrap();
    let condition = if_statement.get_first_child(&c).unwrap();
    let replacement = IR::number(&mut c, 1.0);
    condition.replace_with(&mut c, replacement);
    let msg = panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script));
    assert!(msg.contains("test2: changed scope not marked as changed: SCRIPT: testcode.\nshallow inequivalence\nBefore: NUMBER 0.0 1:20  [length: 1] [source_file: testcode]\nAfter:  NUMBER 1.0 1:20  [length: 1] [source_file: testcode]\n\nAncestor nodes:\nSCRIPT 1:0  [length: 31] [source_file: testcode] [input_id: InputId: testcode] [feature_set: []]\n  IF 1:16  [length: 15] [source_file: testcode]\n    NUMBER 1.0 1:20  [length: 1] [source_file: testcode]\n"),"{msg}");
}
// port: ChangeVerifierTest#testChangeToFunction_newlyUnusedParameter
#[test]
fn test_change_to_function_newly_unused_parameter() {
    let mut c = compiler();
    let script = c.parse_test_code("function f(x) {}");
    assert!(c.get_errors().is_empty());
    assert!(script.is_script(&c));
    let function = script.get_first_child(&c).unwrap();
    assert!(function.is_function(&c));
    let x_param = function.get_second_child(&c).unwrap().get_only_child(&c);
    assert!(x_param.matches_name(&c, "x"));
    let verifier = ChangeVerifier::new(&c).snapshot(&mut c, script);
    verifier.check_recorded_changes_named(&mut c, "test1", script);
    x_param.set_unused_parameter(&mut c, true);
    let msg = panic_message(|| verifier.check_recorded_changes_named(&mut c, "test2", script));
    assert!(msg.contains("changed scope not marked as changed"));
    assert!(msg.contains("shallow inequivalence\nBefore: NAME x 1:11  [length: 1] [source_file: testcode]\nAfter:  NAME x 1:11  [length: 1] [source_file: testcode] [is_unused_parameter: 1]\n\nAncestor nodes:\nFUNCTION f 1:0  [length: 16] [source_file: testcode]\n  PARAM_LIST 1:10  [length: 3] [source_file: testcode]\n    NAME x 1:11  [length: 1] [source_file: testcode] [is_unused_parameter: 1]\n"),"{msg}");
}
