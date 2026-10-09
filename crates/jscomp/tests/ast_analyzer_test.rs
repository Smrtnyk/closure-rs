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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/AstAnalyzerTest.java.

//! Port of `AstAnalyzerTest.java`.

use closure_jscomp::{
    Compiler,
    accessor_summary::{AccessorSummary, PropertyAccessKind},
    ast_analyzer::{AstAnalyzer, Options},
    check_level::CheckLevel,
    compiler_options::{CompilerOptions, LanguageMode},
    diagnostic_groups::ES5_STRICT,
    node_util::NodeUtil,
};
use closure_jstype::js_type_native::JSTypeNative;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_state,
    js_string::JsString,
    jscomp_colors::standard_colors,
    node::{NodeId, SideEffectFlags},
    token::Token,
};
use std::sync::Arc;

#[derive(Clone)]
struct AnalysisCase {
    expect: bool,
    js: String,
    token: Option<Token>,
    global_reg_exp: bool,
    assume_getters_are_pure: bool,
    assume_builtins_pure: bool,
}

impl AnalysisCase {
    // port: AstAnalyzerTest.AnalysisCase#expect
    fn expect(mut self, b: bool) -> Self {
        self.expect = b;
        self
    }
    // port: AstAnalyzerTest.AnalysisCase#js
    fn js(mut self, s: &str) -> Self {
        self.js = s.to_string();
        self
    }
    // port: AstAnalyzerTest.AnalysisCase#token
    fn token(mut self, t: Token) -> Self {
        self.token = Some(t);
        self
    }
    // port: AstAnalyzerTest.AnalysisCase#globalRegExp
    fn global_reg_exp(mut self, b: bool) -> Self {
        self.global_reg_exp = b;
        self
    }
    // port: AstAnalyzerTest.AnalysisCase#assumeGettersArePure
    fn assume_getters_are_pure(mut self, b: bool) -> Self {
        self.assume_getters_are_pure = b;
        self
    }
    // port: AstAnalyzerTest.AnalysisCase#assumeBuiltinsPure
    fn assume_builtins_pure(mut self, b: bool) -> Self {
        self.assume_builtins_pure = b;
        self
    }
}

impl std::fmt::Display for AnalysisCase {
    // port: AstAnalyzerTest.AnalysisCase#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let token = match self.token {
            Some(t) => format!("{t:?}"),
            None => "null".to_string(),
        };
        write!(f, "{} node in `{}` -> {}", token, self.js, self.expect)
    }
}

// port: AstAnalyzerTest#kase
fn kase() -> AnalysisCase {
    AnalysisCase {
        expect: false,
        js: String::new(),
        token: None,
        global_reg_exp: false,
        assume_getters_are_pure: false,
        assume_builtins_pure: true,
    }
}

/// Provides methods for parsing and accessing the compiler used for the parsing.
struct ParseHelper {
    use_types_for_local_optimizations: bool,
    has_global_regexp_references: bool,
    assume_getters_are_pure: bool,
    assume_builtins_pure: bool,
    accessor_summary: Arc<AccessorSummary>,
}

impl ParseHelper {
    fn new() -> Self {
        let mut accessors = IndexMap::<_, _>::default();
        accessors.insert(JsString::from("getter"), PropertyAccessKind::GETTER_ONLY);
        accessors.insert(JsString::from("setter"), PropertyAccessKind::SETTER_ONLY);
        Self {
            use_types_for_local_optimizations: false,
            has_global_regexp_references: true,
            assume_getters_are_pure: true,
            assume_builtins_pure: true,
            accessor_summary: Arc::new(AccessorSummary::create(accessors)),
        }
    }

    // port: AstAnalyzerTest.ParseHelper#newCompiler
    fn new_compiler(&self) -> Compiler {
        let mut options = CompilerOptions::new();

        // To allow octal literals such as 0123 to be parsed.
        options.set_strict_mode_input(false);
        options.set_warning_level(ES5_STRICT.clone(), CheckLevel::OFF);

        options.set_language_in(LanguageMode::UNSUPPORTED);

        let mut compiler = Compiler::new();
        compiler.init_options(options);

        compiler
    }

    // port: AstAnalyzerTest.ParseHelper#parseInternal
    fn parse_internal(&mut self, js: &str) -> (Compiler, NodeId) {
        let mut compiler = self.new_compiler();
        let n = compiler.parse_test_code(js);
        assert!(
            compiler.get_errors().is_empty(),
            "{:?}",
            compiler.get_errors()
        );
        // this.typeRegistry = compiler.getTypeRegistry(): getAstAnalyzer's analyzers read the
        // compiler's registry field.
        compiler.get_type_registry();
        (compiler, n)
    }

    /// Parse a string of JavaScript and return the first node found with the given token in a
    /// preorder DFS.
    // port: AstAnalyzerTest.ParseHelper#parseFirst
    fn parse_first(&mut self, token: Token, js: &str) -> (Compiler, NodeId) {
        let (compiler, root) = self.parse_internal(js);
        let n = find_first(&compiler, token, root).unwrap();
        (compiler, n)
    }

    // port: AstAnalyzerTest.ParseHelper#parseCase
    fn parse_case(&mut self, kase: &AnalysisCase) -> (Compiler, NodeId) {
        self.has_global_regexp_references = kase.global_reg_exp;
        self.assume_getters_are_pure = kase.assume_getters_are_pure;
        self.assume_builtins_pure = kase.assume_builtins_pure;
        let (compiler, root) = self.parse_internal(&kase.js);
        let n = match kase.token {
            None => root.get_first_child(&compiler).unwrap(),
            Some(token) => find_first(&compiler, token, root).unwrap(),
        };
        (compiler, n)
    }

    // port: AstAnalyzerTest.ParseHelper#getAstAnalyzer
    fn get_ast_analyzer(&self) -> AstAnalyzer {
        AstAnalyzer::new(
            Options::builder()
                .set_use_types_for_local_optimization(self.use_types_for_local_optimizations)
                .set_has_regexp_global_references(self.has_global_regexp_references)
                .set_assume_getters_are_pure(self.assume_getters_are_pure)
                .set_assume_known_builtins_are_pure(self.assume_builtins_pure)
                .build(),
            Some(self.accessor_summary.clone()),
        )
    }
}

/// Does a preorder DFS, returning the first node found that has the given token.
// port: AstAnalyzerTest#findFirst
fn find_first(compiler: &Compiler, token: Token, root: NodeId) -> Option<NodeId> {
    let mut stack = vec![root];

    while let Some(cur) = stack.pop() {
        if cur.get_token(compiler) == token {
            return Some(cur);
        }

        let mut child = cur.get_last_child(compiler);
        while let Some(c) = child {
            stack.push(c);
            child = c.get_previous(compiler);
        }
    }

    None
}

/// Runs every parameterized case (Java runs each as its own test) and reports all failures.
fn run_cases(
    cases: Vec<AnalysisCase>,
    check: impl Fn(&AstAnalyzer, &mut Compiler, NodeId) -> bool,
) {
    let mut failures = Vec::new();
    for (index, kase) in cases.iter().enumerate() {
        let mut helper = ParseHelper::new();
        let (mut compiler, node) = helper.parse_case(kase);
        let analyzer = helper.get_ast_analyzer();
        let actual = check(&analyzer, &mut compiler, node);
        if actual != kase.expect {
            failures.push(format!("#{index} {kase}: got {actual}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// port: AstAnalyzerTest.MayEffectMutableStateTest#cases
fn may_effect_mutable_state_cases() -> Vec<AnalysisCase> {
    vec![
        kase().js("i++").token(Token::INC).expect(true),
        kase()
            .js("[b, [a, i++]]")
            .token(Token::ARRAYLIT)
            .expect(true),
        kase().js("i=3").token(Token::ASSIGN).expect(true),
        kase().js("[0, i=3]").token(Token::ARRAYLIT).expect(true),
        kase().js("b()").token(Token::CALL).expect(true),
        kase().js("b?.()").token(Token::OPTCHAIN_CALL).expect(true),
        kase().js("void b()").token(Token::VOID).expect(true),
        kase().js("[1, b()]").token(Token::ARRAYLIT).expect(true),
        kase().js("b.b=4").token(Token::ASSIGN).expect(true),
        kase().js("b.b--").token(Token::DEC).expect(true),
        kase().js("i--").token(Token::DEC).expect(true),
        kase().js("a[0][i=4]").token(Token::GETELEM).expect(true),
        kase()
            .js("a?.[0][i=4]")
            .token(Token::OPTCHAIN_GETELEM)
            .expect(true),
        kase().js("a += 3").token(Token::ASSIGN_ADD).expect(true),
        kase().js("a ||= b").token(Token::ASSIGN_OR).expect(true),
        kase().js("a &&= b").token(Token::ASSIGN_AND).expect(true),
        kase()
            .js("a ??= b")
            .token(Token::ASSIGN_COALESCE)
            .expect(true),
        kase().js("a, b, z += 4").token(Token::COMMA).expect(true),
        kase().js("a ? c : d++").token(Token::HOOK).expect(true),
        kase().js("a ?? b++").token(Token::COALESCE).expect(true),
        kase().js("a + c++").token(Token::ADD).expect(true),
        kase().js("a + c - d()").token(Token::SUB).expect(true),
        kase()
            .js("function foo() {}")
            .token(Token::FUNCTION)
            .expect(true),
        kase().js("while(true);").token(Token::WHILE).expect(true),
        kase().js("if(true){a()}").token(Token::IF).expect(true),
        kase().js("if(true){a}").token(Token::IF).expect(false),
        kase()
            .js("(function() { })")
            .token(Token::FUNCTION)
            .expect(true),
        kase()
            .js("(function() { i++ })")
            .token(Token::FUNCTION)
            .expect(true),
        kase()
            .js("[function a(){}]")
            .token(Token::ARRAYLIT)
            .expect(true),
        kase().js("a").token(Token::NAME).expect(false),
        kase()
            .js("[b, c [d, [e]]]")
            .token(Token::ARRAYLIT)
            .expect(true),
        kase()
            .js("({a: x, b: y, c: z})")
            .token(Token::OBJECTLIT)
            .expect(true),
        // Note: RegExp objects are not immutable, for instance, the exec
        // method maintains state for "global" searches.
        kase().js("/abc/gi").token(Token::REGEXP).expect(true),
        kase().js("'a'").token(Token::STRINGLIT).expect(false),
        kase().js("0").token(Token::NUMBER).expect(false),
        kase().js("1n").token(Token::BIGINT).expect(false),
        kase().js("a + c").token(Token::ADD).expect(false),
        kase().js("'c' + a[0]").token(Token::ADD).expect(false),
        kase().js("a[0][1]").token(Token::GETELEM).expect(false),
        kase()
            .js("a?.[0][1]")
            .token(Token::OPTCHAIN_GETELEM)
            .expect(false),
        kase().js("'a' + c").token(Token::ADD).expect(false),
        kase().js("'a' + a.name").token(Token::ADD).expect(false),
        kase().js("1, 2, 3").token(Token::COMMA).expect(false),
        kase().js("a, b, 3").token(Token::COMMA).expect(false),
        kase()
            .js("(function(a, b) {  })")
            .token(Token::FUNCTION)
            .expect(true),
        kase().js("a ? c : d").token(Token::HOOK).expect(false),
        kase().js("a ?? b").token(Token::COALESCE).expect(false),
        kase()
            .js("'1' + navigator.userAgent")
            .token(Token::ADD)
            .expect(false),
        kase()
            .js("new RegExp('foobar', 'i')")
            .token(Token::NEW)
            .expect(true),
        kase()
            .js("new RegExp(SomethingWacky(), 'i')")
            .token(Token::NEW)
            .expect(true),
        kase().js("new Array()").token(Token::NEW).expect(true),
        kase().js("new Array").token(Token::NEW).expect(true),
        kase().js("new Array(4)").token(Token::NEW).expect(true),
        kase()
            .js("new Array('a', 'b', 'c')")
            .token(Token::NEW)
            .expect(true),
        kase()
            .js("new SomeClassINeverHeardOf()")
            .token(Token::NEW)
            .expect(true),
        // Getters and setters - object rest and spread
        kase()
            .js("({...x});")
            .token(Token::OBJECT_SPREAD)
            .assume_getters_are_pure(false)
            .expect(true),
        kase()
            .js("const {...x} = y;")
            .token(Token::OBJECT_REST)
            .assume_getters_are_pure(false)
            .expect(true),
        kase()
            .js("({...x});")
            .token(Token::OBJECT_SPREAD)
            .assume_getters_are_pure(true)
            .expect(false),
        kase()
            .js("const {...x} = y;")
            .token(Token::OBJECT_REST)
            .assume_getters_are_pure(true)
            .expect(false),
        kase()
            .js("({...f().x});")
            .token(Token::OBJECT_SPREAD)
            .assume_getters_are_pure(true)
            .expect(true),
        kase()
            .js("({...f().x} = y);")
            .token(Token::OBJECT_REST)
            .assume_getters_are_pure(true)
            .expect(true),
        // the presence of `a` affects what gets put into `x`
        kase()
            .js("({a, ...x} = y);")
            .token(Token::STRING_KEY)
            .assume_getters_are_pure(true)
            .expect(true),
        // Getters and setters
        kase().js("x.getter;").token(Token::GETPROP).expect(true),
        kase()
            .js("x?.getter;")
            .token(Token::OPTCHAIN_GETPROP)
            .expect(true),
        // Overapproximation to avoid inspecting the parent.
        kase().js("x.setter;").token(Token::GETPROP).expect(true),
        kase()
            .js("x?.setter;")
            .token(Token::OPTCHAIN_GETPROP)
            .expect(true),
        kase().js("x.normal;").token(Token::GETPROP).expect(false),
        kase()
            .js("x?.normal;")
            .token(Token::OPTCHAIN_GETPROP)
            .expect(false),
        kase()
            .js("const {getter} = x;")
            .token(Token::STRING_KEY)
            .expect(true),
        // Overapproximation to avoid inspecting the parent.
        kase()
            .js("const {setter} = x;")
            .token(Token::STRING_KEY)
            .expect(false),
        kase()
            .js("const {normal} = x;")
            .token(Token::STRING_KEY)
            .expect(false),
        kase()
            .js("x.getter = 0;")
            .token(Token::GETPROP)
            .expect(true),
        kase()
            .js("x.setter = 0;")
            .token(Token::GETPROP)
            .expect(true),
        kase()
            .js("x.normal = 0;")
            .token(Token::GETPROP)
            .expect(false),
        // Default values delegates to children.
        kase()
            .js("({x = 0} = y);")
            .token(Token::DEFAULT_VALUE)
            .expect(false),
        kase()
            .js("([x = 0] = y);")
            .token(Token::DEFAULT_VALUE)
            .expect(false),
        kase()
            .js("function f(x = 0) { };")
            .token(Token::DEFAULT_VALUE)
            .expect(false),
        // Dynamic import can mutate global state
        kase()
            .js("import('./module.js')")
            .token(Token::DYNAMIC_IMPORT)
            .expect(true),
    ]
}

// port: AstAnalyzerTest.MayEffectMutableStateTest#mayEffectMutableState
#[test]
fn may_effect_mutable_state() {
    run_cases(
        may_effect_mutable_state_cases(),
        |analyzer, compiler, node| analyzer.may_effect_mutable_state(compiler, node),
    );
}

// port: AstAnalyzerTest.MayHaveSideEffects#cases
fn may_have_side_effects_cases() -> Vec<AnalysisCase> {
    vec![
        // Cases in need of differentiation.
        kase().expect(false).js("[1]"),
        kase().expect(false).js("[1, 2]"),
        kase()
            .expect(false)
            .js("[1, 2]")
            .assume_builtins_pure(false),
        kase().expect(true).js("i++"),
        kase().expect(true).js("[b, [a, i++]]"),
        kase().expect(true).js("i=3"),
        kase().expect(true).js("[0, i=3]"),
        kase().expect(true).js("b()"),
        kase().expect(true).js("[1, b()]"),
        kase().expect(true).js("b.b=4"),
        kase().expect(true).js("b.b--"),
        kase().expect(true).js("i--"),
        kase().expect(true).js("a[0][i=4]"),
        kase().expect(true).js("a += 3"),
        kase().expect(true).js("a, b, z += 4"),
        kase().expect(true).js("a ? c : d++"),
        kase().expect(true).js("a ?? b++"),
        kase().expect(true).js("a + c++"),
        kase().expect(true).js("a + c - d()"),
        kase().expect(true).js("a + c - d()"),
        kase().expect(true).js("function foo() {}"),
        kase().expect(true).js("class Foo {}"),
        kase().expect(true).js("while(true);"),
        kase().expect(true).js("if(true){a()}"),
        kase().expect(false).js("if(true){a}"),
        kase().expect(false).js("(function() { })"),
        kase().expect(false).js("(function() { i++ })"),
        kase().expect(false).js("[function a(){}]"),
        kase().expect(false).js("(class { })"),
        kase().expect(false).js("(class { method() { i++ } })"),
        kase().expect(true).js("(class { [computedName()]() {} })"),
        kase().expect(false).js("(class { [computedName]() {} })"),
        kase().expect(false).js("(class Foo extends Bar { })"),
        kase().expect(true).js("(class extends foo() { })"),
        kase().expect(false).js("a"),
        kase().expect(false).js("a").assume_builtins_pure(false),
        kase().expect(false).js("a.b"),
        kase().expect(false).js("a.b.c"),
        kase().expect(false).js("[b, c [d, [e]]]"),
        kase().expect(false).js("({a: x, b: y, c: z})"),
        kase().expect(false).js("({a, b, c})"),
        kase().expect(false).js("/abc/gi"),
        kase().expect(false).js("'a'"),
        kase().expect(false).js("0"),
        kase().expect(false).js("a + c"),
        kase().expect(false).js("'c' + a[0]"),
        kase().expect(false).js("a[0][1]"),
        kase().expect(false).js("'a' + c"),
        kase().expect(false).js("'a' + a.name"),
        kase().expect(false).js("1, 2, 3"),
        kase().expect(false).js("a, b, 3"),
        kase().expect(false).js("(function(a, b) {  })"),
        kase().expect(false).js("a ? c : d"),
        kase().expect(false).js("a ?? b"),
        kase().expect(false).js("'1' + navigator.userAgent"),
        kase().expect(false).js("`template`"),
        kase().expect(false).js("`template${name}`"),
        kase().expect(false).js("`${name}template`"),
        kase().expect(true).js("`${naming()}template`"),
        kase().expect(true).js("templateFunction`template`"),
        kase().expect(true).js("st = `${name}template`"),
        kase()
            .expect(true)
            .js("tempFunc = templateFunction`template`"),
        kase().expect(false).js("new RegExp('foobar', 'i')"),
        kase().expect(true).js("new RegExp(SomethingWacky(), 'i')"),
        kase().expect(false).js("new Array()"),
        kase().expect(false).js("new Array"),
        kase().expect(false).js("new Array(4)"),
        kase().expect(false).js("new Array('a', 'b', 'c')"),
        kase()
            .expect(true)
            .js("new Array()")
            .assume_builtins_pure(false),
        kase().expect(true).js("new SomeClassINeverHeardOf()"),
        kase().expect(true).js("new SomeClassINeverHeardOf()"),
        kase().expect(false).js("({}).foo = 4"),
        kase().expect(false).js("([]).foo = 4"),
        kase().expect(false).js("(function() {}).foo = 4"),
        kase().expect(true).js("this.foo = 4"),
        kase().expect(true).js("a.foo = 4"),
        kase()
            .expect(true)
            .js("(function() { return n; })().foo = 4"),
        kase().expect(true).js("([]).foo = bar()"),
        kase().expect(false).js("undefined"),
        kase().expect(false).js("void 0"),
        kase().expect(true).js("void foo()"),
        kase().expect(false).js("-Infinity"),
        kase().expect(false).js("Infinity"),
        kase().expect(false).js("NaN"),
        kase().expect(false).js("({}||[]).foo = 2;"),
        kase().expect(false).js("(true ? {} : []).foo = 2;"),
        kase().expect(false).js("({},[]).foo = 2;"),
        kase().expect(true).js("delete a.b"),
        kase().expect(false).js("Math.random();"),
        kase()
            .expect(true)
            .js("Math.random();")
            .assume_builtins_pure(false),
        kase().expect(true).js("Math.random(seed);"),
        kase()
            .expect(true)
            .js("Math.random(seed);")
            .assume_builtins_pure(false),
        kase().expect(false).js("Math.sin(1, 10);"),
        kase()
            .expect(true)
            .js("Math.sin(1, 10);")
            .assume_builtins_pure(false),
        kase().expect(false).js("[1, 1].foo;"),
        kase().expect(true).js("export var x = 0;"),
        kase().expect(true).js("export let x = 0;"),
        kase().expect(true).js("export const x = 0;"),
        kase().expect(true).js("export class X {};"),
        kase().expect(true).js("export function x() {};"),
        kase().expect(true).js("export {x};"),
        // ARRAYLIT-ITER_SPREAD
        kase().expect(false).js("[...[]]"),
        kase().expect(false).js("[...[1]]"),
        kase().expect(true).js("[...[i++]]"),
        kase().expect(false).js("[...'string']"),
        kase().expect(false).js("[...`templatelit`]"),
        kase().expect(false).js("[...`templatelit ${safe}`]"),
        kase().expect(true).js("[...`templatelit ${unsafe()}`]"),
        kase().expect(true).js("[...f()]"),
        kase().expect(true).js("[...5]"),
        kase().expect(true).js("[...null]"),
        kase().expect(true).js("[...true]"),
        // CALL-ITER_SPREAD
        kase().expect(false).js("Math.sin(...[])"),
        kase().expect(false).js("Math.sin(...[1])"),
        kase().expect(true).js("Math.sin(...[i++])"),
        kase().expect(false).js("Math.sin(...'string')"),
        kase().expect(false).js("Math.sin(...`templatelit`)"),
        kase()
            .expect(false)
            .js("Math.sin(...`templatelit ${safe}`)"),
        kase()
            .expect(true)
            .js("Math.sin(...`templatelit ${unsafe()}`)"),
        kase().expect(true).js("Math.sin(...f())"),
        kase().expect(true).js("Math.sin(...5)"),
        kase().expect(true).js("Math.sin(...null)"),
        kase().expect(true).js("Math.sin(...true)"),
        // NEW-ITER_SPREAD
        kase().expect(false).js("new Object(...[])"),
        kase().expect(false).js("new Object(...[1])"),
        kase().expect(true).js("new Object(...[i++])"),
        kase().expect(false).js("new Object(...'string')"),
        kase().expect(false).js("new Object(...`templatelit`)"),
        kase()
            .expect(false)
            .js("new Object(...`templatelit ${safe}`)"),
        kase()
            .expect(true)
            .js("new Object(...`templatelit ${unsafe()}`)"),
        kase().expect(true).js("new Object(...f())"),
        kase().expect(true).js("new Object(...5)"),
        kase().expect(true).js("new Object(...null)"),
        kase().expect(true).js("new Object(...true)"),
        // OBJECT_SPREAD
        // These could all invoke getters.
        kase().expect(true).js("({...x})"),
        kase().expect(true).js("({...{}})"),
        kase().expect(true).js("({...{a:1}})"),
        kase().expect(true).js("({...{a:i++}})"),
        kase().expect(true).js("({...{a:f()}})"),
        kase().expect(true).js("({...f()})"),
        // OBJECT_REST
        // This could invoke getters.
        kase()
            .expect(true)
            .token(Token::OBJECT_REST)
            .js("({...x} = something)"),
        // the presence of `a` affects what goes into `x`
        kase()
            .expect(true)
            .token(Token::STRING_KEY)
            .js("({a, ...x} = something)"),
        // ITER_REST
        // We currently assume all iterable-rests are side-effectful.
        kase()
            .expect(true)
            .token(Token::ITER_REST)
            .js("([...x] = 'safe')"),
        kase()
            .expect(false)
            .token(Token::ITER_REST)
            .js("(function(...x) { })"),
        // Context switch
        kase()
            .expect(true)
            .token(Token::AWAIT)
            .js("async function f() { await 0; }"),
        kase()
            .expect(true)
            .token(Token::FOR_AWAIT_OF)
            .js("(async()=>{ for await (let x of []) {} })"),
        kase()
            .expect(true)
            .token(Token::THROW)
            .js("function f() { throw 'something'; }"),
        kase()
            .expect(true)
            .token(Token::YIELD)
            .js("function* f() { yield 'something'; }"),
        kase()
            .expect(true)
            .token(Token::YIELD)
            .js("function* f() { yield* 'something'; }"),
        // Enhanced for loop
        // These edge cases are actually side-effect free. We include them to confirm we just give
        // up
        // on enhanced for loops.
        kase().expect(true).js("for (const x in []) { }"),
        kase().expect(true).js("for (const x of []) { }"),
        // COMPUTED_PROP - OBJECTLIT
        kase()
            .expect(false)
            .token(Token::COMPUTED_PROP)
            .js("({[a]: x})"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_PROP)
            .js("({[a()]: x})"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_PROP)
            .js("({[a]: x()})"),
        // computed property getters and setters are modeled as COMPUTED_PROP with an
        // annotation to indicate getter or setter.
        kase()
            .expect(false)
            .token(Token::COMPUTED_PROP)
            .js("({ get [a]() {} })"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_PROP)
            .js("({ get [a()]() {} })"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_PROP)
            .js("({ set [a](x) {} })"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_PROP)
            .js("({ set [a()](x) {} })"),
        // COMPUTED_PROP - CLASS
        kase()
            .expect(false)
            .token(Token::COMPUTED_PROP)
            .js("class C { [a]() {} }"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_PROP)
            .js("class C { [a()]() {} }"),
        // computed property getters and setters are modeled as COMPUTED_PROP with an
        // annotation to indicate getter or setter.
        kase()
            .expect(false)
            .token(Token::COMPUTED_PROP)
            .js("class C { get [a]() {} }"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_PROP)
            .js("class C { get [a()]() {} }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_PROP)
            .js("class C { set [a](x) {} }"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_PROP)
            .js("class C { set [a()](x) {} }"),
        // GETTER_DEF
        kase()
            .expect(false)
            .token(Token::GETTER_DEF)
            .js("({ get a() {} })"),
        kase()
            .expect(false)
            .token(Token::GETTER_DEF)
            .js("class C { get a() {} }"),
        // Getter use
        kase().expect(true).token(Token::GETPROP).js("x.getter;"),
        kase()
            .expect(true)
            .token(Token::OPTCHAIN_GETPROP)
            .js("x?.getter;"),
        // Overapproximation because to avoid inspecting the parent.
        kase().expect(true).token(Token::GETPROP).js("x.setter;"),
        kase()
            .expect(true)
            .token(Token::OPTCHAIN_GETPROP)
            .js("x?.setter;"),
        kase().expect(false).token(Token::GETPROP).js("x.normal;"),
        kase()
            .expect(false)
            .token(Token::OPTCHAIN_GETPROP)
            .js("x?.normal;"),
        kase()
            .expect(true)
            .token(Token::STRING_KEY)
            .js("({getter} = foo());"),
        kase()
            .expect(false)
            .token(Token::STRING_KEY)
            .js("({setter} = foo());"),
        kase()
            .expect(false)
            .token(Token::STRING_KEY)
            .js("({normal} = foo());"),
        // SETTER_DEF
        kase()
            .expect(false)
            .token(Token::SETTER_DEF)
            .js("({ set a(x) {} })"),
        kase()
            .expect(false)
            .token(Token::SETTER_DEF)
            .js("class C { set a(x) {} }"),
        // SETTER_USE
        // Overapproximation because to avoid inspecting the parent.
        kase()
            .expect(true)
            .token(Token::GETPROP)
            .js("x.getter = 0;"),
        kase()
            .expect(true)
            .token(Token::GETPROP)
            .js("x.setter = 0;"),
        kase()
            .expect(false)
            .token(Token::GETPROP)
            .js("x.normal = 0;"),
        // MEMBER_FUNCTION_DEF
        kase()
            .expect(false)
            .token(Token::MEMBER_FUNCTION_DEF)
            .js("({ a(x) {} })"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FUNCTION_DEF)
            .js("class C { a(x) {} }"),
        // MEMBER_FIELD_DEF
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { x=2; }"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { x; }"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { x }"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { x \n y }"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { static x=2; }"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { static x; }"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { static x }"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { static x \n static y }"),
        kase()
            .expect(false)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { x = alert(1); }"),
        kase()
            .expect(true)
            .token(Token::MEMBER_FIELD_DEF)
            .js("class C { static x = alert(1); }"),
        // COMPUTED_FIELD_DEF
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { [x]; }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { ['x']=2; }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { 'x'=2; }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { 1=2; }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { static [x]; }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { static ['x']=2; }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { static 'x'=2; }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { static 1=2; }"),
        kase()
            .expect(false)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { ['x'] = alert(1); }"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { static ['x'] = alert(1); }"),
        kase()
            .expect(true)
            .token(Token::COMPUTED_FIELD_DEF)
            .js("class C { static [alert(1)] = 2; }"),
        // CLASS_STATIC_BLOCK
        kase()
            .expect(false)
            .token(Token::BLOCK)
            .js("class C { static {} }"),
        kase()
            .expect(false)
            .token(Token::BLOCK)
            .js("class C { static { [1]; } }"),
        kase()
            .expect(true)
            .token(Token::BLOCK)
            .js("class C { static { let x; } }"),
        kase()
            .expect(true)
            .token(Token::BLOCK)
            .js("class C { static { const x =1 ; } }"),
        kase()
            .expect(true)
            .token(Token::BLOCK)
            .js("class C { static { var x; } }"),
        kase()
            .expect(true)
            .token(Token::BLOCK)
            .js("class C { static { this.x = 1; } }"),
        kase()
            .expect(true)
            .token(Token::BLOCK)
            .js("class C { static { function f() { } } }"),
        kase()
            .expect(false)
            .token(Token::BLOCK)
            .js("class C { static { (function () {} )} }"),
        kase()
            .expect(false)
            .token(Token::BLOCK)
            .js("class C { static { ()=>{} } }"),
        // SUPER calls
        kase().expect(false).token(Token::SUPER).js("super()"),
        kase().expect(false).token(Token::SUPER).js("super.foo()"),
        // Some common Object methods are assumed to be side-effect free when called
        // with their standard builtin arity.
        kase().expect(false).js("o.toString()"),
        kase().expect(false).js("o.toString(16)"),
        kase().expect(false).js("(0).toString(16)"),
        kase().expect(false).js("(0n).toString(16)"),
        kase().expect(false).js("o?.toString(16)"),
        kase().expect(false).js("(0)?.toString(16)"),
        kase().expect(false).js("(0n)?.toString(16)"),
        kase().expect(false).js("o.valueOf()"),
        kase().expect(true).js("o.toString(16, 2)"),
        kase().expect(true).js("o.valueOf(1)"),
        // These builtin-method assumptions are disabled when assumeKnownBuiltinsArePure is false.
        kase()
            .expect(true)
            .js("o.toString()")
            .assume_builtins_pure(false),
        kase()
            .expect(true)
            .js("o.toString(16)")
            .assume_builtins_pure(false),
        kase()
            .expect(true)
            .js("(0).toString(16)")
            .assume_builtins_pure(false),
        kase()
            .expect(true)
            .js("(0n).toString(16)")
            .assume_builtins_pure(false),
        kase()
            .expect(true)
            .js("o?.toString(16)")
            .assume_builtins_pure(false),
        kase()
            .expect(true)
            .js("(0)?.toString(16)")
            .assume_builtins_pure(false),
        kase()
            .expect(true)
            .js("(0n)?.toString(16)")
            .assume_builtins_pure(false),
        kase()
            .expect(true)
            .js("o.valueOf()")
            .assume_builtins_pure(false),
        // other methods depend on the extern definitions
        kase().expect(true).js("o.watch()"),
        // A RegExp Object by itself doesn't have any side-effects
        kase().expect(false).js("/abc/gi").global_reg_exp(true),
        kase().expect(false).js("/abc/gi").global_reg_exp(false),
        // RegExp instance methods have global side-effects, so whether they are
        // considered side-effect free depends on whether the global properties
        // are referenced and whether we assume that the built-in methods are not overwritten
        // with impure variants.
        kase()
            .expect(true)
            .js("(/abc/gi).test('')")
            .global_reg_exp(true),
        kase()
            .expect(false)
            .js("(/abc/gi).test('')")
            .global_reg_exp(false),
        kase()
            .expect(true)
            .js("(/abc/gi).test('')")
            .global_reg_exp(false)
            .assume_builtins_pure(false),
        kase()
            .expect(true)
            .js("(/abc/gi).test(a)")
            .global_reg_exp(true),
        kase()
            .expect(false)
            .js("(/abc/gi).test(b)")
            .global_reg_exp(false),
        kase()
            .expect(true)
            .js("(/abc/gi).exec('')")
            .global_reg_exp(true),
        kase()
            .expect(false)
            .js("(/abc/gi).exec('')")
            .global_reg_exp(false),
        // Some RegExp object method that may have side-effects.
        kase()
            .expect(true)
            .js("(/abc/gi).foo('')")
            .global_reg_exp(true),
        kase()
            .expect(true)
            .js("(/abc/gi).foo('')")
            .global_reg_exp(false),
        // Try the string RegExp ops.
        kase().expect(true).js("''.match('a')").global_reg_exp(true),
        kase()
            .expect(false)
            .js("''.match('a')")
            .global_reg_exp(false),
        kase()
            .expect(true)
            .js("''.match(/(a)/)")
            .global_reg_exp(true),
        kase()
            .expect(false)
            .js("''.match(/(a)/)")
            .global_reg_exp(false),
        kase()
            .expect(true)
            .js("''.replace('a')")
            .global_reg_exp(true),
        kase()
            .expect(false)
            .js("''.replace('a')")
            .global_reg_exp(false),
        kase()
            .expect(true)
            .js("''.search('a')")
            .global_reg_exp(true),
        kase()
            .expect(false)
            .js("''.search('a')")
            .global_reg_exp(false),
        kase().expect(true).js("''.split('a')").global_reg_exp(true),
        kase()
            .expect(false)
            .js("''.split('a')")
            .global_reg_exp(false),
        // Some non-RegExp string op that may have side-effects.
        kase().expect(true).js("''.foo('a')").global_reg_exp(true),
        kase().expect(true).js("''.foo('a')").global_reg_exp(false),
        // 'a' might be a RegExp object with the 'g' flag, in which case
        // the state might change by running any of the string ops.
        // Specifically, using these methods resets the "lastIndex" if used
        // in combination with a RegExp instance "exec" method.
        kase().expect(true).js("''.match(a)").global_reg_exp(true),
        kase().expect(true).js("''.match(a)").global_reg_exp(false),
        kase()
            .expect(true)
            .js("'a'.replace(/a/, function (s) {alert(s)})")
            .global_reg_exp(false),
        kase()
            .expect(false)
            .js("'a'.replace(/a/, 'x')")
            .global_reg_exp(false),
        // Dynamic import changes global state
        kase()
            .expect(true)
            .token(Token::DYNAMIC_IMPORT)
            .js("import('./module.js')"),
        // Symbol() call expressions are side-effect free
        kase().expect(false).js("Symbol()"),
        kase().expect(false).js("Symbol('desc')"),
        kase().expect(true).js("Symbol(foo())"),
        kase()
            .expect(true)
            .js("Symbol()")
            .assume_builtins_pure(false),
    ]
}

// port: AstAnalyzerTest.MayHaveSideEffects#test
#[test]
fn may_have_side_effects_test() {
    run_cases(may_have_side_effects_cases(), |analyzer, compiler, node| {
        analyzer.may_have_side_effects(compiler, node)
    });
}

// port: AstAnalyzerTest.NodeTypeMayHaveSideEffects#cases
fn node_type_may_have_side_effects_cases() -> Vec<AnalysisCase> {
    vec![
        kase().js("x = y").token(Token::ASSIGN).expect(true),
        kase().js("x += y").token(Token::ASSIGN_ADD).expect(true),
        kase().js("delete x.y").token(Token::DELPROP).expect(true),
        kase().js("x++").token(Token::INC).expect(true),
        kase().js("x--").token(Token::DEC).expect(true),
        kase()
            .js("for (prop in obj) {}")
            .token(Token::FOR_IN)
            .expect(true),
        kase()
            .js("for (item of iterable) {}")
            .token(Token::FOR_OF)
            .expect(true),
        // name declaration has a side effect when a value is actually assigned
        kase().js("var x = 1;").token(Token::NAME).expect(true),
        kase().js("let x = 1;").token(Token::NAME).expect(true),
        kase().js("const x = 1;").token(Token::NAME).expect(true),
        // don't consider name declaration to have a side effect if there's no assignment
        kase().js("var x;").token(Token::NAME).expect(false),
        kase().js("let x;").token(Token::NAME).expect(false),
        // destructuring declarations and assignments are always considered side effectful even
        // when empty
        kase()
            .js("var {x} = {};")
            .token(Token::DESTRUCTURING_LHS)
            .expect(true),
        kase()
            .js("var {} = {};")
            .token(Token::DESTRUCTURING_LHS)
            .expect(true),
        kase()
            .js("var [x] = [];")
            .token(Token::DESTRUCTURING_LHS)
            .expect(true),
        kase()
            .js("var {y: [x]} = {};")
            .token(Token::OBJECT_PATTERN)
            .expect(false),
        kase()
            .js("var {y: [x]} = {};")
            .token(Token::ARRAY_PATTERN)
            .expect(false),
        kase().js("[x] = arr;").token(Token::ASSIGN).expect(true),
        // NOTE: CALL and NEW nodes are delegated to functionCallHasSideEffects() and
        // constructorCallHasSideEffects(), respectively. The cases below are just a few
        // representative examples that are convenient to test here.
        //
        // in general function and constructor calls are assumed to have side effects
        kase().js("foo();").token(Token::CALL).expect(true),
        kase()
            .js("foo?.();")
            .token(Token::OPTCHAIN_CALL)
            .expect(true),
        kase().js("new Foo();").token(Token::NEW).expect(true),
        // Object() is known not to have side-effects, though
        kase().js("Object();").token(Token::CALL).expect(false),
        kase()
            .js("Object?.();")
            .token(Token::OPTCHAIN_CALL)
            .expect(false),
        kase().js("new Object();").token(Token::NEW).expect(false),
        // Symbol() is known not to have side-effects
        kase().js("Symbol();").token(Token::CALL).expect(false),
        kase()
            .js("Symbol('desc');")
            .token(Token::CALL)
            .expect(false),
        // TAGGED_TEMPLATELIT is just a special syntax for a CALL.
        kase()
            .js("foo`template`;")
            .token(Token::TAGGED_TEMPLATELIT)
            .expect(true),
        // NOTE: ITER_REST and ITER_SPREAD are delegated to NodeUtil.iteratesImpureIterable()
        // Test cases here are just easy to test representative examples.
        kase()
            .js("[...[1, 2, 3]]")
            .token(Token::ITER_SPREAD)
            .expect(false),
        // unknown iterable, so assume side-effects
        kase()
            .js("[...someIterable]")
            .token(Token::ITER_SPREAD)
            .expect(true),
        // we just assume the rhs of an array pattern may have iteration side-effects
        // without looking too closely.
        kase()
            .js("let [...rest] = [1, 2, 3];")
            .token(Token::ITER_REST)
            .expect(true),
        // ITER_REST in parameter list does not trigger iteration at the function definition, so
        // it has no side effects.
        kase()
            .js("function foo(...rest) {}")
            .token(Token::ITER_REST)
            .expect(false),
        // defining a class or a function is not considered to be a side-effect
        kase()
            .js("function foo() {}")
            .token(Token::FUNCTION)
            .expect(false),
        kase().js("class Foo {}").token(Token::CLASS).expect(false),
        // arithmetic, logic, and bitwise operations do not have side-effects
        kase().js("x + y").token(Token::ADD).expect(false),
        kase().js("x || y").token(Token::OR).expect(false),
        kase().js("x | y").token(Token::BITOR).expect(false),
        kase().js("x ?? y").token(Token::COALESCE).expect(false),
        // Getters and setters
        kase()
            .js("({...x});")
            .token(Token::OBJECT_SPREAD)
            .expect(true),
        kase()
            .js("const {...x} = y;")
            .token(Token::OBJECT_REST)
            .expect(true),
        kase().js("y.getter;").token(Token::GETPROP).expect(true),
        kase()
            .js("y?.getter;")
            .token(Token::OPTCHAIN_GETPROP)
            .expect(true),
        kase().js("y.setter;").token(Token::GETPROP).expect(true),
        kase()
            .js("y?.setter;")
            .token(Token::OPTCHAIN_GETPROP)
            .expect(true),
        kase().js("y.normal;").token(Token::GETPROP).expect(false),
        kase()
            .js("y?.normal;")
            .token(Token::OPTCHAIN_GETPROP)
            .expect(false),
        kase()
            .js("const {getter} = y;")
            .token(Token::STRING_KEY)
            .expect(true),
        kase()
            .js("const {setter} = y;")
            .token(Token::STRING_KEY)
            .expect(false),
        kase()
            .js("const {normal} = y;")
            .token(Token::STRING_KEY)
            .expect(false),
        kase()
            .js("y.getter = 0;")
            .token(Token::GETPROP)
            .expect(true),
        kase()
            .js("y.setter = 0;")
            .token(Token::GETPROP)
            .expect(true),
        kase()
            .js("y.normal = 0;")
            .token(Token::GETPROP)
            .expect(false),
        // Dynamic import causes side effects
        kase()
            .js("import('./module.js')")
            .token(Token::DYNAMIC_IMPORT)
            .expect(true),
    ]
}

// port: AstAnalyzerTest.NodeTypeMayHaveSideEffects#test
#[test]
fn node_type_may_have_side_effects_test() {
    run_cases(
        node_type_may_have_side_effects_cases(),
        |analyzer, compiler, node| analyzer.node_type_may_have_side_effects(compiler, node),
    );
}

// port: AstAnalyzerTest.FunctionCallHasSideEffects#testCallSideEffects
#[test]
fn test_call_side_effects() {
    let mut helper = ParseHelper::new();

    // Parens force interpretation as an expression.
    let (mut compiler, new_x_dot_method_call) =
        helper.parse_first(Token::CALL, "(new x().method());");
    let ast_analyzer = helper.get_ast_analyzer();
    assert!(ast_analyzer.function_call_has_side_effects(&mut compiler, new_x_dot_method_call));

    let new_expr = new_x_dot_method_call
        .get_first_first_child(&compiler)
        .unwrap();
    check_state!(new_expr.is_new(&compiler));
    let mut flags = SideEffectFlags::new();

    // No side effects, local result
    flags.clear_all_flags();
    new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
    flags.clear_all_flags();
    new_x_dot_method_call.set_side_effect_flags_from_flags(&mut compiler, &flags);

    // Cannot determine this evaluates to a local value (even though it does in practice).
    assert!(!NodeUtil::evaluates_to_local_value(
        &compiler,
        new_x_dot_method_call
    ));
    assert!(!ast_analyzer.function_call_has_side_effects(&mut compiler, new_x_dot_method_call));
    assert!(!ast_analyzer.may_have_side_effects(&mut compiler, new_x_dot_method_call));

    // Modifies this, local result
    flags.clear_all_flags();
    new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
    flags.clear_all_flags();
    flags.set_mutates_this();
    new_x_dot_method_call.set_side_effect_flags_from_flags(&mut compiler, &flags);

    assert!(!NodeUtil::evaluates_to_local_value(
        &compiler,
        new_x_dot_method_call
    ));
    assert!(!ast_analyzer.function_call_has_side_effects(&mut compiler, new_x_dot_method_call));
    assert!(!ast_analyzer.may_have_side_effects(&mut compiler, new_x_dot_method_call));

    // Modifies this, non-local result
    flags.clear_all_flags();
    new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
    flags.clear_all_flags();
    flags.set_mutates_this();
    new_x_dot_method_call.set_side_effect_flags_from_flags(&mut compiler, &flags);

    assert!(!NodeUtil::evaluates_to_local_value(
        &compiler,
        new_x_dot_method_call
    ));
    assert!(!ast_analyzer.function_call_has_side_effects(&mut compiler, new_x_dot_method_call));
    assert!(!ast_analyzer.may_have_side_effects(&mut compiler, new_x_dot_method_call));

    // No modifications, non-local result
    flags.clear_all_flags();
    new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
    flags.clear_all_flags();
    new_x_dot_method_call.set_side_effect_flags_from_flags(&mut compiler, &flags);

    assert!(!NodeUtil::evaluates_to_local_value(
        &compiler,
        new_x_dot_method_call
    ));
    assert!(!ast_analyzer.function_call_has_side_effects(&mut compiler, new_x_dot_method_call));
    assert!(!ast_analyzer.may_have_side_effects(&mut compiler, new_x_dot_method_call));

    // The new modifies global state, no side-effect call
    // This call could be removed, but not the new.
    flags.clear_all_flags();
    flags.set_mutates_global_state();
    new_expr.set_side_effect_flags_from_flags(&mut compiler, &flags);
    flags.clear_all_flags();
    new_x_dot_method_call.set_side_effect_flags_from_flags(&mut compiler, &flags);

    // This does evaluate to a local value but NodeUtil does not know that
    assert!(!NodeUtil::evaluates_to_local_value(
        &compiler,
        new_x_dot_method_call
    ));
    assert!(!ast_analyzer.function_call_has_side_effects(&mut compiler, new_x_dot_method_call));
    assert!(ast_analyzer.may_have_side_effects(&mut compiler, new_x_dot_method_call));
}

const BUILT_IN_FUNCTIONS_WITHOUT_SIDE_EFFECTS_CASES: [&str; 9] = [
    "Object", "Array", "String", "Number", "BigInt", "Boolean", "RegExp", "Error", "Symbol",
];

// port: AstAnalyzerTest.FunctionCallHasSideEffects#testStringMethodCallSideEffects_noTypesForLocalOptimizations
#[test]
fn test_string_method_call_side_effects_no_types_for_local_optimizations() {
    let mut helper = ParseHelper::new();

    let (mut compiler, x_dot_replace_call) =
        helper.parse_first(Token::CALL, "x.replace(/xyz/g, '');");
    let ast_analyzer = helper.get_ast_analyzer();
    assert!(ast_analyzer.function_call_has_side_effects(&mut compiler, x_dot_replace_call));

    helper.has_global_regexp_references = false;
    let ast_analyzer = helper.get_ast_analyzer();
    assert!(ast_analyzer.function_call_has_side_effects(&mut compiler, x_dot_replace_call));

    let x_node = x_dot_replace_call.get_first_first_child(&compiler).unwrap();
    let string_type = compiler
        .get_type_registry()
        .get_native_type(JSTypeNative::STRING_TYPE);
    x_node.set_jstype(&mut compiler, Some(string_type));
    assert!(ast_analyzer.function_call_has_side_effects(&mut compiler, x_dot_replace_call));
}

// port: AstAnalyzerTest.FunctionCallHasSideEffects#testTypeBasedStringMethodCallSideEffects_useTypesForLocalOptimziations
#[test]
fn test_type_based_string_method_call_side_effects_use_types_for_local_optimziations() {
    let mut helper = ParseHelper::new();
    helper.use_types_for_local_optimizations = true;
    helper.has_global_regexp_references = false;

    let (mut compiler, x_dot_replace_call) =
        helper.parse_first(Token::CALL, "x.replace(/xyz/g, '');");
    let x_dot_replace_call_string_type = x_dot_replace_call.clone_tree(&mut compiler);
    let x_dot_replace_call_string_color = x_dot_replace_call.clone_tree(&mut compiler);

    let string_type = compiler
        .get_type_registry()
        .get_native_type(JSTypeNative::STRING_TYPE);
    x_dot_replace_call_string_type
        .get_first_first_child(&compiler)
        .unwrap()
        .set_jstype(&mut compiler, Some(string_type));
    x_dot_replace_call_string_color
        .get_first_first_child(&compiler)
        .unwrap()
        .set_color(&mut compiler, Some(standard_colors::STRING.clone()));
    let ast_analyzer = helper.get_ast_analyzer();

    assert!(ast_analyzer.function_call_has_side_effects(&mut compiler, x_dot_replace_call));
    assert!(
        !ast_analyzer.function_call_has_side_effects(&mut compiler, x_dot_replace_call_string_type)
    );
    assert!(
        !ast_analyzer
            .function_call_has_side_effects(&mut compiler, x_dot_replace_call_string_color)
    );
}

// port: AstAnalyzerTest.BuiltInFunctionWithoutSideEffects#test_assumingPureBuiltins
#[test]
fn test_assuming_pure_builtins() {
    for function_name in BUILT_IN_FUNCTIONS_WITHOUT_SIDE_EFFECTS_CASES {
        let mut parse_helper = ParseHelper::new();
        parse_helper.assume_builtins_pure = true;
        let (mut compiler, func) =
            parse_helper.parse_first(Token::CALL, &format!("{function_name}(1);"));
        assert!(
            !parse_helper
                .get_ast_analyzer()
                .function_call_has_side_effects(&mut compiler, func),
            "{function_name}"
        );
    }
}

// port: AstAnalyzerTest.BuiltInFunctionWithoutSideEffects#test_noAssumePureBuiltins
#[test]
fn test_no_assume_pure_builtins() {
    for function_name in BUILT_IN_FUNCTIONS_WITHOUT_SIDE_EFFECTS_CASES {
        let mut parse_helper = ParseHelper::new();
        parse_helper.assume_builtins_pure = false;
        let (mut compiler, func) =
            parse_helper.parse_first(Token::CALL, &format!("{function_name}(1);"));
        assert!(
            parse_helper
                .get_ast_analyzer()
                .function_call_has_side_effects(&mut compiler, func),
            "{function_name}"
        );
    }
}

// port: AstAnalyzerTest.ConstructorCallHasSideEffects#byDefaultAConstructorCallHasSideEffects
#[test]
fn by_default_a_constructor_call_has_side_effects() {
    let mut parse_helper = ParseHelper::new();
    let (compiler, new_node) = parse_helper.parse_first(Token::NEW, "new SomeClass();");
    let ast_analyzer = parse_helper.get_ast_analyzer();
    // we know nothing about the class being instantiated, so assume side effects occur.
    assert!(ast_analyzer.constructor_call_has_side_effects(&compiler, new_node));
}

// port: AstAnalyzerTest.ConstructorCallHasSideEffects#constructorCallMarkedAsNoSideEffectsHasNone
#[test]
fn constructor_call_marked_as_no_side_effects_has_none() {
    let mut parse_helper = ParseHelper::new();
    let (mut compiler, new_node) = parse_helper.parse_first(Token::NEW, "new SomeClass();");
    let ast_analyzer = parse_helper.get_ast_analyzer();
    // simulate PureFunctionIdentifier marking the call as having no side effects.
    let mut flags = SideEffectFlags::new();
    flags.clear_all_flags();
    new_node.set_side_effect_flags_from_flags(&mut compiler, &flags);

    assert!(!ast_analyzer.constructor_call_has_side_effects(&compiler, new_node));
}

// port: AstAnalyzerTest.ConstructorCallHasSideEffects#modifyingALocalArgumentIsNotASideEffect
#[test]
fn modifying_a_local_argument_is_not_a_side_effect() {
    let mut parse_helper = ParseHelper::new();
    // object literal is considered a local value. Modifying it isn't a side effect, since
    // nothing else looks at it.
    let (mut compiler, new_node) = parse_helper.parse_first(Token::NEW, "new SomeClass({});");
    let ast_analyzer = parse_helper.get_ast_analyzer();
    // simulate PureFunctionIdentifier marking the call as only modifying its arguments
    let mut flags = SideEffectFlags::new();
    flags.clear_all_flags();
    flags.set_mutates_arguments();
    new_node.set_side_effect_flags_from_flags(&mut compiler, &flags);

    assert!(!ast_analyzer.constructor_call_has_side_effects(&compiler, new_node));
}

// port: AstAnalyzerTest.ConstructorCallHasSideEffects#modifyingANonLocalArgumentIsASideEffect
#[test]
fn modifying_a_non_local_argument_is_a_side_effect() {
    let mut parse_helper = ParseHelper::new();
    // variable name is a non-local value. Modifying it is a side effect.
    let (mut compiler, new_node) = parse_helper.parse_first(Token::NEW, "new SomeClass(x);");
    let ast_analyzer = parse_helper.get_ast_analyzer();
    // simulate PureFunctionIdentifier marking the call as only modifying its arguments
    let mut flags = SideEffectFlags::new();
    flags.clear_all_flags();
    flags.set_mutates_arguments();
    new_node.set_side_effect_flags_from_flags(&mut compiler, &flags);

    assert!(ast_analyzer.constructor_call_has_side_effects(&compiler, new_node));
}

// port: AstAnalyzerTest.ConstructorsKnownToHaveNoSideEffects#noSideEffectsForKnownConstructor
#[test]
fn no_side_effects_for_known_constructor() {
    for constructor_name in [
        "Array",
        "Date",
        "Error",
        "Object",
        "RegExp",
        "XMLHttpRequest",
    ] {
        let mut parse_helper = ParseHelper::new();
        let (compiler, new_node) =
            parse_helper.parse_first(Token::NEW, &format!("new {constructor_name}();"));
        let ast_analyzer = parse_helper.get_ast_analyzer();
        // we know nothing about the class being instantiated, so assume side effects occur.
        assert!(
            !ast_analyzer.constructor_call_has_side_effects(&compiler, new_node),
            "{constructor_name}"
        );
    }
}
