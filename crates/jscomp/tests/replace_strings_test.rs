/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/ReplaceStringsTest.java.

//! Port of `ReplaceStringsTest.java`: tests for `ReplaceStrings`.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    check_level::CheckLevel,
    compiler_options::{ChunkOutputType, CompilerOptions, PropertyCollapseLevel},
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    diagnostic_groups,
    disambiguate::disambiguate_properties::DisambiguateProperties,
    es6_normalize_classes::{self, Es6NormalizeClasses},
    inline_and_collapse_properties::InlineAndCollapseProperties,
    node_traversal::{Callback, NodeTraversal},
    replace_strings::{
        BAD_REPLACEMENT_CONFIGURATION, ReplaceStrings, STRING_REPLACEMENT_TAGGED_TEMPLATE,
    },
    source_information_annotator::SourceInformationAnnotator,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, MINIMAL_EXTERNS},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

// port: ReplaceStringsTest#DEFAULT_FUNCTIONS_TO_INSPECT
const DEFAULT_FUNCTIONS_TO_INSPECT: &[&str] = &[
    "Error(?)",
    "goog.debug.Trace.startTracer(*)",
    "goog.debug.Logger.getLogger(?)",
    "goog.log.getLogger(?)",
    "goog.log.info(,?)",
    "goog.log.multiString(,?,?,)",
    "Excluded(?):!testcode",
    "NotExcluded(?):!unmatchable",
];

// port: ReplaceStringsTest#EXTERNS (the text block appended to MINIMAL_EXTERNS)
const EXTERNS_SUFFIX: &str = concat!(
    "var goog = {};\n",
    "goog.debug = {};\n",
    "/** @constructor */\n",
    "goog.debug.Trace = function() {};\n",
    "goog.debug.Trace.startTracer = function (var_args) {};\n",
    "/** @constructor */\n",
    "goog.debug.Logger = function() {};\n",
    "goog.debug.Logger.prototype.info = function(msg, opt_ex) {};\n",
    "/**\n",
    " * @param {?} name\n",
    " * @return {!goog.debug.Logger}\n",
    " */\n",
    "goog.debug.Logger.getLogger = function(name){};\n",
    "goog.log = {}\n",
    "goog.log.getLogger = function(name){};\n",
    "goog.log.info = function(logger, msg, opt_ex) {};\n",
    "goog.log.multiString = function(logger, replace1, replace2, keep) {};\n",
);

/// The test-instance fields `pass`, `runDisambiguateProperties`, `rename` and
/// `functionsToInspect`, shared with the anonymous pass that reads them when it runs.
struct State {
    pass: Option<Rc<RefCell<ReplaceStrings>>>,
    run_disambiguate_properties: bool,
    rename: bool,
    functions_to_inspect: Vec<String>,
}

struct ReplaceStringsTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    state: Rc<RefCell<State>>,
}

/// `private static class Renamer extends AbstractPostOrderCallback`.
struct Renamer;

impl Callback for Renamer {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ReplaceStringsTest.Renamer#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_name(t) || n.is_get_prop(t) {
            let original_name = n.get_string(t);
            n.set_original_name(t, Some(original_name.clone()));
            n.set_string(t, JsString::from("renamed_").concat(&original_name));
            t.report_code_change();
        }
    }
}

impl CompilerTestCaseHooks for Hooks {
    // port: ReplaceStringsTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        options.set_warning_level(
            diagnostic_groups::MISSING_PROPERTIES.clone(),
            CheckLevel::OFF,
        );
        Ok(options)
    }

    // port: ReplaceStringsTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let functions_to_inspect = self.state.borrow().functions_to_inspect.clone();
        let pass = Rc::new(RefCell::new(ReplaceStrings::new(
            &mut compiler.borrow_mut(),
            "`",
            &functions_to_inspect,
        )));
        self.state.borrow_mut().pass = Some(Rc::clone(&pass));

        let state = Rc::clone(&self.state);
        let processor: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, js: NodeId| {
                if state.borrow().rename {
                    NodeTraversal::traverse(compiler, js, &mut Renamer);
                }
                Es6NormalizeClasses::new(compiler).process(compiler, externs, js);
                InlineAndCollapseProperties::builder(compiler)
                    .set_property_collapse_level(PropertyCollapseLevel::ALL)
                    .set_chunk_output_type(ChunkOutputType::GLOBAL_NAMESPACE)
                    .set_have_modules_been_rewritten(false)
                    .set_module_resolution_mode(ResolutionMode::BROWSER)
                    .build()
                    .process(compiler, externs, js);
                if state.borrow().run_disambiguate_properties {
                    let mut sia = SourceInformationAnnotator::create();
                    NodeTraversal::traverse(compiler, js, &mut sia);

                    DisambiguateProperties::new(
                        compiler,
                        IndexSet::<_>::from_iter([JsString::from("foobar")]),
                    )
                    .process(compiler, externs, js);
                }
                pass.borrow_mut().process(compiler, externs, js);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(processor))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "ReplaceStringsTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl ReplaceStringsTest {
    // port: ReplaceStringsTest#ReplaceStringsTest
    // port: ReplaceStringsTest#setUp
    fn new() -> Self {
        let minimal_externs = MINIMAL_EXTERNS.clone().unwrap();
        let mut externs = minimal_externs.as_units().to_vec();
        externs.extend(EXTERNS_SUFFIX.encode_utf16());
        let mut harness = CompilerTestCase::new(JsString::from_units(externs));
        harness.set_up();
        harness.enable_type_check().unwrap();
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is
        // necessary.
        harness.enable_normalize_expected_output().unwrap();
        harness.enable_parse_type_info().unwrap();
        harness.set_generic_name_replacements(
            es6_normalize_classes::generic_name_replacements()
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "ReplaceStringsTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                state: Rc::new(RefCell::new(State {
                    pass: None,
                    run_disambiguate_properties: false,
                    rename: false,
                    functions_to_inspect: DEFAULT_FUNCTIONS_TO_INSPECT
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect(),
                })),
            },
        }
    }

    fn test(&mut self, js: &str, expected: &str) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_warning(
        &mut self,
        js: &str,
        warning: &'static closure_jscomp::diagnostic_type::DiagnosticType,
    ) {
        self.harness
            .test_warning(&mut self.hooks, js, warning)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_error(
        &mut self,
        js: &str,
        error: &'static closure_jscomp::diagnostic_type::DiagnosticType,
    ) {
        self.harness
            .test_error(&mut self.hooks, js, error)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    // port: ReplaceStringsTest#testDebugStrings
    fn test_debug_strings(&mut self, js: &str, expected: &str, substituted_strings: &[&str]) {
        // Verify that the strings are substituted correctly in the JS code.
        self.test(js, expected);

        let results = self
            .hooks
            .state
            .borrow()
            .pass
            .as_ref()
            .unwrap()
            .borrow()
            .get_result();
        assert_eq!(substituted_strings.len() % 2, 0);
        assert_eq!(results.len(), substituted_strings.len() / 2);

        // Verify that substituted strings are decoded correctly.
        for i in (0..substituted_strings.len()).step_by(2) {
            let result = &results[i / 2];
            let original = substituted_strings[i + 1];
            assert_eq!(result.original, original);

            let replacement = substituted_strings[i];
            assert_eq!(result.replacement, replacement);
        }
    }
}

// port: ReplaceStringsTest#testRenameName
#[test]
fn test_rename_name() {
    let mut t = ReplaceStringsTest::new();
    t.hooks.state.borrow_mut().rename = true;
    t.test_debug_strings("Error('xyz');", "renamed_Error('a');", &["a", "xyz"]);
}

// port: ReplaceStringsTest#testRenameStaticProp
#[test]
fn test_rename_static_prop() {
    let mut t = ReplaceStringsTest::new();
    t.hooks.state.borrow_mut().rename = true;
    t.test_debug_strings(
        "goog.debug.Trace.startTracer('HistoryManager.updateHistory');",
        "renamed_goog.renamed_debug.renamed_Trace.renamed_startTracer('a');",
        &["a", "HistoryManager.updateHistory"],
    );
}

// port: ReplaceStringsTest#testThrowError2
#[test]
fn test_throw_error2() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "throw Error('x' +\n    'yz');",
        "throw Error('a');",
        &["a", "xyz"],
    );
}

// port: ReplaceStringsTest#testThrowError3
#[test]
fn test_throw_error3() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "throw Error('Unhandled mail' + ' search type ' + type);",
        "throw Error('a' + '`' + type);",
        &["a", "Unhandled mail search type `"],
    );
}

// port: ReplaceStringsTest#testThrowError3a
#[test]
fn test_throw_error3a() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "/** @const */ var preposition = 'in';\n/** @const */ var action = 'search';\n/** @const */ var error = 'Unhandled ' + action;\nthrow Error(error + ' ' + type + ' ' + preposition + ' ' + search);\n",
        "/** @const */ var preposition = 'in';\n/** @const */ var action = 'search';\n/** @const */ var error = 'Unhandled ' + action;\nthrow Error('a' + '`' + type + '`' + search);\n",
        &["a", "Unhandled search ` in `"]);
}

// port: ReplaceStringsTest#testThrowError4
#[test]
fn test_throw_error4() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "/** @constructor */\nvar A = function() {};\nA.prototype.m = function(child) {\n  if (this.haveChild(child)) {\n    throw Error('Node: ' + this.getDataPath() +\n                ' already has a child named ' + child);\n  } else if (child.parentNode) {\n    throw Error('Node: ' + child.getDataPath() +\n                ' already has a parent');\n  }\n  child.parentNode = this;\n};\n",
        "/** @constructor */\nvar A = function(){};\nA.prototype.m = function(child) {\n  if (this.haveChild(child)) {\n    throw Error('a' + '`' + this.getDataPath() + '`' + child);\n  } else if (child.parentNode) {\n    throw Error('b' + '`' + child.getDataPath());\n  }\n  child.parentNode = this;\n};\n",
        &[
          "a", "Node: ` already has a child named `", "b", "Node: ` already has a parent",
        ]);
}

// port: ReplaceStringsTest#testThrowError_templateLiteral
#[test]
fn test_throw_error_template_literal() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "throw Error(`Unhandled search ${type} in ${search}`);",
        "throw Error('a' + '`' + type + '`' + search);",
        &["a", "Unhandled search ` in `"],
    );
}

// port: ReplaceStringsTest#testThrowError_templateLiteralWithConstantTerms
#[test]
fn test_throw_error_template_literal_with_constant_terms() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "const preposition = 'in';\nconst action = 'search';\nconst error = `Unhandled ${action}`;\nthrow Error(`${error} ${'type ' + getType()} ${preposition} ${search}`);\n",
        "const preposition = 'in';\nconst action = 'search';\nconst error = `Unhandled ${action}`;\nthrow Error('a' + '`' + getType() + '`' + search);\n",
        &["a", "Unhandled search type ` in `"]);
}

// port: ReplaceStringsTest#testThrowError_templateLiteralWithNonconstantTerms
#[test]
fn test_throw_error_template_literal_with_nonconstant_terms() {
    let mut t = ReplaceStringsTest::new();
    // NOTE: We can only inline error if it is *transitively* constant. Otherwise it is left alone,
    // since any transitive strings may have changed, and we certainly don't want to call functions
    // a second time.
    t.test_debug_strings(
        "const error = `Unhandled ${action}`;\nthrow Error(`${error} ${type} in ${search}`);\n",
        "const error = `Unhandled ${action}`;\nthrow Error('a' + '`' + error + '`' + type + '`' + search);\n",
        &["a", "` ` in `"]);
}

// port: ReplaceStringsTest#testThrowError_templateLiteralConcatenation
#[test]
fn test_throw_error_template_literal_concatenation() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "throw Error(`Unhandled mail` + ` search type ${type}`);",
        "throw Error('a' + '`' + type);",
        &["a", "Unhandled mail search type `"],
    );
}

// port: ReplaceStringsTest#testThrowNonStringError
#[test]
fn test_throw_non_string_error() {
    let mut t = ReplaceStringsTest::new();
    // No replacement is done when an error is neither a string literal nor
    // a string concatenation expression.
    t.test_debug_strings("throw Error(x('abc'));", "throw Error(x('abc'));", &[]);
}

// port: ReplaceStringsTest#testThrowError_taggedTemplateLiteral
#[test]
fn test_throw_error_tagged_template_literal() {
    let mut t = ReplaceStringsTest::new();
    // No replacement is done when there is a tag function.
    t.test_debug_strings(
        "throw Error(x`abc`);", //
        "throw Error(x`abc`);",
        &[],
    );
}

// port: ReplaceStringsTest#testThrowConstStringError
#[test]
fn test_throw_const_string_error() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "var AA = 'uvw', AB = AA + 'xyz'; throw Error(AB);",
        "var AA = 'uvw', AB = AA + 'xyz'; throw Error('a');",
        &["a", "uvwxyz"],
    );
}

// port: ReplaceStringsTest#testThrowConstStringError_templateLiteral
#[test]
fn test_throw_const_string_error_template_literal() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "var AA = 'uvw', AB = `${AA}xyz`; throw Error(AB);",
        "var AA = 'uvw', AB = `${AA}xyz`; throw Error('a');",
        &["a", "uvwxyz"],
    );
}

// port: ReplaceStringsTest#testThrowNewError1
#[test]
fn test_throw_new_error1() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "throw new Error('abc');",
        "throw new Error('a');",
        &["a", "abc"],
    );
}

// port: ReplaceStringsTest#testThrowNewError2
#[test]
fn test_throw_new_error2() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings("throw new Error();", "throw new Error();", &[]);
}

// port: ReplaceStringsTest#testStartTracer1
#[test]
fn test_start_tracer1() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "goog.debug.Trace.startTracer('HistoryManager.updateHistory');",
        "goog.debug.Trace.startTracer('a');",
        &["a", "HistoryManager.updateHistory"],
    );
}

// port: ReplaceStringsTest#testStartTracer2
#[test]
fn test_start_tracer2() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "goog$debug$Trace.startTracer('HistoryManager', 'updateHistory');",
        "goog$debug$Trace.startTracer('a', 'b');",
        &["a", "HistoryManager", "b", "updateHistory"],
    );
}

// port: ReplaceStringsTest#testStartTracer3
#[test]
fn test_start_tracer3() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "goog$debug$Trace.startTracer('ThreadlistView',\n                             'Updating ' + array.length + ' rows');\n",
        "goog$debug$Trace.startTracer('a', 'b' + '`' + array.length);",
        &["a", "ThreadlistView", "b", "Updating ` rows"]);
}

// port: ReplaceStringsTest#testStartTracer4
#[test]
fn test_start_tracer4() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "goog.debug.Trace.startTracer(s, 'HistoryManager.updateHistory');",
        "goog.debug.Trace.startTracer(s, 'a');",
        &["a", "HistoryManager.updateHistory"],
    );
}

// port: ReplaceStringsTest#testLoggerInitialization
#[test]
fn test_logger_initialization() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "goog$debug$Logger$getLogger('my.app.Application');",
        "goog$debug$Logger$getLogger('a');",
        &["a", "my.app.Application"],
    );
}

// port: ReplaceStringsTest#testLoggerOnVar
#[test]
fn test_logger_on_var() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "var logger = goog.debug.Logger.getLogger('foo');\nlogger.info('Some message');\n",
        "var logger = goog.debug.Logger.getLogger('a');\nlogger.info('Some message');\n",
        &["a", "foo"],
    );
}

// port: ReplaceStringsTest#testRepeatedErrorString1
#[test]
fn test_repeated_error_string1() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "Error('abc');Error('def');Error('abc');",
        "Error('a');Error('b');Error('a');",
        &["a", "abc", "b", "def"],
    );
}

// port: ReplaceStringsTest#testRepeatedErrorString2
#[test]
fn test_repeated_error_string2() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "Error('a:' + u + ', b:' + v); Error('a:' + x + ', b:' + y);",
        "Error('a' + '`' + u + '`' + v); Error('a' + '`' + x + '`' + y);",
        &["a", "a:`, b:`"],
    );
}

// port: ReplaceStringsTest#testRepeatedErrorString3
#[test]
fn test_repeated_error_string3() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "var AB = 'b'; throw Error(AB); throw Error(AB);",
        "var AB = 'b'; throw Error('a'); throw Error('a');",
        &["a", "b"],
    );
}

// port: ReplaceStringsTest#testRepeatedTracerString
#[test]
fn test_repeated_tracer_string() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "goog$debug$Trace.startTracer('A', 'B', 'A');",
        "goog$debug$Trace.startTracer('a', 'b', 'a');",
        &["a", "A", "b", "B"],
    );
}

// port: ReplaceStringsTest#testRepeatedLoggerString
#[test]
fn test_repeated_logger_string() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "goog$debug$Logger$getLogger('goog.net.XhrTransport');\ngoog$debug$Logger$getLogger('my.app.Application');\ngoog$debug$Logger$getLogger('my.app.Application');\n",
        "goog$debug$Logger$getLogger('a');\ngoog$debug$Logger$getLogger('b');\ngoog$debug$Logger$getLogger('b');\n",
        &["a", "goog.net.XhrTransport", "b", "my.app.Application"]);
}

// port: ReplaceStringsTest#testRepeatedStringsWithDifferentMethods
#[test]
fn test_repeated_strings_with_different_methods() {
    let mut t = ReplaceStringsTest::new();
    t.test(
        "throw Error('A');\ngoog$debug$Trace.startTracer('B', 'A');\ngoog$debug$Logger$getLogger('C');\ngoog$debug$Logger$getLogger('B');\ngoog$debug$Logger$getLogger('A');\nthrow Error('D');\nthrow Error('C');\nthrow Error('B');\nthrow Error('A');\n",
        "throw Error('a');\ngoog$debug$Trace.startTracer('b', 'a');\ngoog$debug$Logger$getLogger('c');\ngoog$debug$Logger$getLogger('b');\ngoog$debug$Logger$getLogger('a');\nthrow Error('d');\nthrow Error('c');\nthrow Error('b');\nthrow Error('a');\n");
}

// port: ReplaceStringsTest#testLoggerWithNoReplacedParam
#[test]
fn test_logger_with_no_replaced_param() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "var x = {};\nx.logger_ = goog.log.getLogger('foo');\ngoog.log.info(x.logger_, 'Some message');\n",
        "var x$logger_ = goog.log.getLogger('a');\ngoog.log.info(x$logger_, 'b');\n",
        &[
          "a", "foo",
          "b", "Some message"
        ]);
}

// port: ReplaceStringsTest#testLoggerWithSomeParametersNotReplaced
#[test]
fn test_logger_with_some_parameters_not_replaced() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings(
        "var x = {};\nx.logger_ = goog.log.getLogger('foo');\ngoog.log.multiString(x.logger_, 'Some message', 'Some message2',\n'Do not replace');\n",
        "var x$logger_ = goog.log.getLogger('a');\ngoog.log.multiString(x$logger_, 'b', 'c', 'Do not replace');\n",
        &[
          "a", "foo",
          "b", "Some message",
          "c", "Some message2"
        ]);
}

// port: ReplaceStringsTest#testWarningForTaggedTemplates_unqualifiedName
#[test]
fn test_warning_for_tagged_templates_unqualified_name() {
    let mut t = ReplaceStringsTest::new();
    t.test_warning(
        "throw Error`Unhandled mail search type ${type}`;",
        &STRING_REPLACEMENT_TAGGED_TEMPLATE,
    );
}

// port: ReplaceStringsTest#testWarningForTaggedTemplates_qualifiedName
#[test]
fn test_warning_for_tagged_templates_qualified_name() {
    let mut t = ReplaceStringsTest::new();
    t.test_warning(
        "goog.debug.Logger.getLogger`foo`;", //
        &STRING_REPLACEMENT_TAGGED_TEMPLATE,
    );
}

// port: ReplaceStringsTest#testWarnsIfPassingPrototypeMethod
#[test]
fn test_warns_if_passing_prototype_method() {
    let mut t = ReplaceStringsTest::new();
    // ReplaceStrings supported this configuration until November 2020, so make sure users don't
    // pass it thinking it is still supported.
    t.harness.allow_sourceless_warnings().unwrap();

    let builder = vec!["A.prototype.f(?)".to_string()];
    t.hooks.state.borrow_mut().functions_to_inspect = builder;
    t.test_error("", &BAD_REPLACEMENT_CONFIGURATION);
}

// port: ReplaceStringsTest#testExcludedFile
#[test]
fn test_excluded_file() {
    let mut t = ReplaceStringsTest::new();
    t.test_debug_strings("Excluded('xyz');", "Excluded('xyz');", &[]);
    t.test_debug_strings("NotExcluded('xyz');", "NotExcluded('a');", &["a", "xyz"]);
}
