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
//   src/com/google/javascript/jscomp/PassNames.java.

// Constants from the pinned PassNames.java, in Java declaration order.
// port: PassNames#PassNames (empty private constructor; Rust module namespace)
pub const AFTER_EARLY_OPTIMIZATION_LOOP: &str = "afterEarlyOptimizationLoop";
pub const AFTER_MAIN_OPTIMIZATIONS: &str = "afterMainOptimizations";
pub const AFTER_STANDARD_CHECKS: &str = "afterStandardChecks";
pub const AMBIGUATE_PROPERTIES: &str = "ambiguateProperties";
pub const ANALYZER_CHECKS: &str = "analyzerChecks";
pub const ANGULAR_PASS: &str = "angularPass";
pub const BEFORE_EARLY_OPTIMIZATION_LOOP: &str = "beforeEarlyOptimizationLoop";
pub const BEFORE_EARLY_OPTIMIZATIONS_TRANSPILATION: &str = "beforeEarlyOptimizationsTranspilation";
pub const BEFORE_STANDARD_OPTIMIZATIONS: &str = "beforeStandardOptimizations";
pub const OBFUSCATION_PASS_MARKER: &str = "obfuscationPassMarker";
pub const BEFORE_MAIN_OPTIMIZATIONS: &str = "beforeMainOptimizations";
pub const BEFORE_TYPE_CHECKING: &str = "beforeTypeChecking";
pub const BEFORE_SERIALIZATION: &str = "beforeSerialization";
pub const BEFORE_VARIABLE_RENAMING: &str = "beforeVariableRenaming";
pub const BEFORE_EXTRACT_PROTOTYPE_MEMBER_DECLARATIONS: &str =
    "beforeExtractPrototypeMemberDeclarations";
pub const CHECK_CONFORMANCE: &str = "checkConformance";
pub const CHECK_REG_EXP: &str = "checkRegExp";
pub const CHECK_TYPES: &str = "checkTypes";
pub const CHECK_VARIABLE_REFERENCES: &str = "checkVariableReferences";
pub const CHECK_VARS: &str = "checkVars";
pub const COALESCE_VARIABLE_NAMES: &str = "coalesceVariableNames";
pub const COLLAPSE_ANONYMOUS_FUNCTIONS: &str = "collapseAnonymousFunctions";
pub const COLLAPSE_OBJECT_LITERALS: &str = "collapseObjectLiterals";
pub const COLLAPSE_PROPERTIES: &str = "collapseProperties";
pub const COLLAPSE_VARIABLE_DECLARATIONS: &str = "collapseVariableDeclarations";
pub const CONVERT_TO_DOTTED_PROPERTIES: &str = "convertToDottedProperties";
pub const CREATE_MODULE_MAP: &str = "createModuleMap";
pub const CROSS_CHUNK_CODE_MOTION: &str = "crossChunkCodeMotion";
pub const CROSS_CHUNK_METHOD_MOTION: &str = "crossChunkMethodMotion";
pub const DEAD_ASSIGNMENT_ELIMINATION: &str = "deadAssignmentsElimination";
pub const DECLARED_GLOBAL_EXTERNS_ON_WINDOW: &str = "declaredGlobalExternsOnWindow";
pub const DESERIALIZE_COMPILER_STATE: &str = "deserializeCompilerState";
pub const DEVIRTUALIZE_METHODS: &str = "devirtualizeMethods";
pub const DISAMBIGUATE_PROPERTIES: &str = "disambiguateProperties";
pub const ES6_NORMALIZE_CLASSES: &str = "es6NormalizeClasses";
pub const EXPORT_TEST_FUNCTIONS: &str = "exportTestFunctions";
pub const EXTERN_EXPORTS: &str = "externExports";
pub const EXTRACT_PROTOTYPE_MEMBER_DECLARATIONS: &str = "extractPrototypeMemberDeclarations";
pub const FLOW_SENSITIVE_INLINE_VARIABLES: &str = "flowSensitiveInlineVariables";
pub const GATHER_GETTERS_AND_SETTERS: &str = "gatherGettersAndSetters";
pub const GATHER_MODULE_METADATA: &str = "gatherModuleMetadata";
pub const GATHER_RAW_EXPORTS: &str = "gatherRawExports";
pub const GENERATE_EXPORTS: &str = "generateExports";
pub const INFER_CONSTS: &str = "inferConsts";
pub const INFER_TYPES: &str = "inferTypes";
pub const INLINE_FUNCTIONS: &str = "inlineFunctions";
pub const INLINE_PROPERTIES: &str = "inlineProperties";
pub const INLINE_TYPE_ALIASES: &str = "inlineTypeAliases";
pub const INLINE_VARIABLES: &str = "inlineVariables";
pub const LATE_PEEPHOLE_OPTIMIZATIONS: &str = "latePeepholeOptimizations";
pub const LINT_CHECKS: &str = "lintChecks";
pub const MARK_UNNORMALIZED: &str = "markUnnormalized";
pub const NORMALIZE: &str = "normalize";
pub const OPTIMIZATIONS_HALFWAY_POINT: &str = "optimizationsHalfwayPoint";
pub const OPTIMIZE_CALLS: &str = "optimizeCalls";
pub const PARSE_INPUTS: &str = "parseInputs";
pub const PEEPHOLE_OPTIMIZATIONS: &str = "peepholeOptimizations";
pub const POST_NORMALIZE_PEEPHOLE: &str = "postNormalizePeephole";
pub const REWRITE_COMMON_JS_MODULES: &str = "rewriteCommonJsModules";
pub const REWRITE_SCRIPTS_TO_ES6_MODULES: &str = "rewriteScriptsToEs6Modules";
pub const REMOVE_UNUSED_CODE: &str = "removeUnusedCode";
pub const REPLACE_ID_GENERATORS: &str = "replaceIdGenerators";
pub const REPLACE_MESSAGES: &str = "replaceMessages";
pub const RESOLVE_TYPES: &str = "resolveTypes";
pub const REWRITE_FUNCTION_EXPRESSIONS: &str = "rewriteFunctionExpressions";
pub const RENAME_PROPERTIES: &str = "renameProperties";
pub const BEFORE_RENAME_PROPERTIES: &str = "beforeRenameProperties";
pub const STRIP_SIDE_EFFECT_PROTECTION: &str = "stripSideEffectProtection";
pub const WIZ_PASS: &str = "wizPass";
