/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTest.java.

use closure_jscomp::compiler_options::CompilerOptions;
// port: CompilerTest#testDeadPropertyAssignmentElimination_defaultsToOffIfPolymerEnabled
#[test]
fn test_dead_property_assignment_elimination_defaults_to_off_if_polymer_enabled() {
    let mut options = CompilerOptions::new();
    options.set_dead_assignment_elimination(true);
    assert!(options.should_run_dead_assignment_elimination());
    assert!(options.should_run_dead_property_assignment_elimination());
    options.set_polymer_version(Some(1));
    assert!(options.should_run_dead_assignment_elimination());
    assert!(!options.should_run_dead_property_assignment_elimination());
}
// port: CompilerTest#testDeadPropertyAssignmentElimination_canBeExplicitlyDisabled
#[test]
fn test_dead_property_assignment_elimination_can_be_explicitly_disabled() {
    let mut options = CompilerOptions::new();
    options.set_dead_assignment_elimination(true);
    assert!(options.should_run_dead_assignment_elimination());
    options.set_dead_property_assignment_elimination(false);
    assert!(!options.should_run_dead_property_assignment_elimination());
}
